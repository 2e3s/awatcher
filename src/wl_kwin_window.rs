/*
 * This uses a hack with KWin scripts in order to receive the active window.
 * For the moment of writing, KWin doesn't implement the appropriate protocols to get a top level window.
 * Inspired by https://github.com/k0kubun/xremap/
 */
use super::report_client::ReportClient;
use super::BoxedError;
use aw_client_rust::Event as AwEvent;
use chrono::{Duration, Utc};
use serde_json::{Map, Value};
use std::env::temp_dir;
use std::path::Path;
use std::sync::{mpsc::channel, Arc, Mutex};
use std::thread;
use std::time;
use zbus::blocking::{Connection, ConnectionBuilder};
use zbus::dbus_interface;

const KWIN_SCRIPT_NAME: &str = "activity_watcher";
const KWIN_SCRIPT: &str = include_str!("kwin_window.js");

struct KWinScript {
    dbus_connection: Connection,
}

impl KWinScript {
    fn new(dbus_connection: Connection) -> Self {
        KWinScript { dbus_connection }
    }

    fn load(&self) -> Result<(), BoxedError> {
        if self.is_loaded()? {
            warn!("KWin script is already loaded, unloading");
            self.unload().unwrap();
        }

        let path = temp_dir().join("kwin_window.js");
        std::fs::write(&path, KWIN_SCRIPT).unwrap();

        let result = self
            .get_registered_number(&path)
            .and_then(|number| self.start(number));
        std::fs::remove_file(&path)?;

        result
    }

    fn is_loaded(&self) -> Result<bool, BoxedError> {
        self.dbus_connection
            .call_method(
                Some("org.kde.KWin"),
                "/Scripting",
                Some("org.kde.kwin.Scripting"),
                "isScriptLoaded",
                &KWIN_SCRIPT_NAME,
            )?
            .body::<bool>()
            .map_err(std::convert::Into::into)
    }

    fn get_registered_number(&self, path: &Path) -> Result<i32, BoxedError> {
        let temp_path = path
            .to_str()
            .ok_or::<BoxedError>("Temporary file path is not valid".into())?;

        self.dbus_connection
            .call_method(
                Some("org.kde.KWin"),
                "/Scripting",
                Some("org.kde.kwin.Scripting"),
                "loadScript",
                // since OsStr does not implement zvariant::Type, the temp-path must be valid utf-8
                &(temp_path, KWIN_SCRIPT_NAME),
            )?
            .body::<i32>()
            .map_err(std::convert::Into::into)
    }

    fn unload(&self) -> Result<bool, BoxedError> {
        self.dbus_connection
            .call_method(
                Some("org.kde.KWin"),
                "/Scripting",
                Some("org.kde.kwin.Scripting"),
                "unloadScript",
                &KWIN_SCRIPT_NAME,
            )?
            .body::<bool>()
            .map_err(std::convert::Into::into)
    }

    fn start(&self, script_number: i32) -> Result<(), BoxedError> {
        debug!("Starting KWin script {script_number}");
        self.dbus_connection
            .call_method(
                Some("org.kde.KWin"),
                format!("/{script_number}"),
                Some("org.kde.kwin.Script"),
                "run",
                &(),
            )
            .map_err(|e| format!("Error on starting the script {e}").into())
            .map(|_| ())
    }
}

impl Drop for KWinScript {
    fn drop(&mut self) {
        debug!("Unloading KWin script");
        if let Err(e) = self.unload() {
            error!("Problem during stopping KWin script: {e}");
        };
    }
}

fn send_heartbeat(
    client: &ReportClient,
    bucket_name: &str,
    active_window: &Arc<Mutex<ActiveWindow>>,
) -> Result<(), BoxedError> {
    let event = {
        let active_window = active_window.lock().map_err(|e| format!("{e}"))?;
        let mut data = Map::new();
        data.insert(
            "app".to_string(),
            Value::String(active_window.resource_class.clone()),
        );
        data.insert(
            "title".to_string(),
            Value::String(active_window.caption.clone()),
        );
        AwEvent {
            id: None,
            timestamp: Utc::now(),
            duration: Duration::zero(),
            data,
        }
    };

    client
        .heartbeat(bucket_name, &event)
        .map_err(|_| "Failed to send heartbeat for active window".into())
}

struct ActiveWindow {
    resource_class: String,
    resource_name: String,
    caption: String,
}

struct ActiveWindowInterface {
    active_window: Arc<Mutex<ActiveWindow>>,
}

#[dbus_interface(name = "com._2e3s.Awatcher")]
impl ActiveWindowInterface {
    fn notify_active_window(
        &mut self,
        caption: String,
        resource_class: String,
        resource_name: String,
    ) {
        debug!("Active window class: \"{resource_class}\", name: \"{resource_name}\", caption: \"{caption}\"");
        let mut active_window = self.active_window.lock().unwrap();
        active_window.caption = caption;
        active_window.resource_class = resource_class;
        active_window.resource_name = resource_name;
    }
}

pub fn run(client: &Arc<ReportClient>) {
    let hostname = gethostname::gethostname().into_string().unwrap();
    let bucket_name = format!("aw-watcher-window_{hostname}");
    let kwin_script = KWinScript::new(Connection::session().unwrap());

    kwin_script.load().unwrap();

    let active_window = Arc::new(Mutex::new(ActiveWindow {
        caption: String::new(),
        resource_name: String::new(),
        resource_class: String::new(),
    }));
    let active_window_interface = ActiveWindowInterface {
        active_window: Arc::clone(&active_window),
    };

    let (tx, rx) = channel();
    thread::spawn(move || {
        let result = (|| {
            ConnectionBuilder::session()?
                .name("com._2e3s.Awatcher")?
                .serve_at("/com/_2e3s/Awatcher", active_window_interface)?
                .build()
        })();
        match result {
            Ok(connection) => {
                tx.send(Ok(())).unwrap();
                loop {
                    connection.monitor_activity().wait();
                }
            }
            Err(e) => tx.send(Err(e)),
        }
    });
    let _ = rx.recv().unwrap();

    info!("Starting active window watcher");
    loop {
        if let Err(error) = send_heartbeat(client, &bucket_name, &active_window) {
            error!("Error on sending active window heartbeat: {error}");
        }
        thread::sleep(time::Duration::from_secs(u64::from(
            client.config.poll_time_window,
        )));
    }
}
