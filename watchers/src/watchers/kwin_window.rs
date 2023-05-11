/*
 * This uses a hack with KWin scripts in order to receive the active window.
 * For the moment of writing, KWin doesn't implement the appropriate protocols to get a top level window.
 * Inspired by https://github.com/k0kubun/xremap/
 */
use super::Watcher;
use crate::report_client::ReportClient;
use anyhow::{anyhow, Context};
use std::env::temp_dir;
use std::path::Path;
use std::sync::{mpsc::channel, Arc, Mutex};
use std::thread;
use zbus::blocking::{Connection, ConnectionBuilder};
use zbus::dbus_interface;

const KWIN_SCRIPT_NAME: &str = "activity_watcher";
const KWIN_SCRIPT: &str = include_str!("kwin_window.js");

struct KWinScript {
    dbus_connection: Connection,
    is_loaded: bool,
}

impl KWinScript {
    fn new(dbus_connection: Connection) -> Self {
        KWinScript {
            dbus_connection,
            is_loaded: false,
        }
    }

    fn load(&mut self) -> anyhow::Result<()> {
        let path = temp_dir().join("kwin_window.js");
        std::fs::write(&path, KWIN_SCRIPT).unwrap();

        let result = self
            .get_registered_number(&path)
            .and_then(|number| self.start(number));
        std::fs::remove_file(&path)?;
        self.is_loaded = true;

        result
    }

    fn is_loaded(&self) -> anyhow::Result<bool> {
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

    fn get_registered_number(&self, path: &Path) -> anyhow::Result<i32> {
        let temp_path = path
            .to_str()
            .ok_or(anyhow!("Temporary file path is not valid"))?;

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

    fn unload(&self) -> anyhow::Result<bool> {
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

    fn start(&self, script_number: i32) -> anyhow::Result<()> {
        debug!("Starting KWin script {script_number}");
        self.dbus_connection
            .call_method(
                Some("org.kde.KWin"),
                format!("/{script_number}"),
                Some("org.kde.kwin.Script"),
                "run",
                &(),
            )
            .with_context(|| "Error on starting the script")?;
        Ok(())
    }
}

impl Drop for KWinScript {
    fn drop(&mut self) {
        if self.is_loaded {
            debug!("Unloading KWin script");
            if let Err(e) = self.unload() {
                error!("Problem during stopping KWin script: {e}");
            };
        }
    }
}

fn send_active_window(
    client: &ReportClient,
    active_window: &Arc<Mutex<ActiveWindow>>,
) -> anyhow::Result<()> {
    let active_window = active_window.lock().expect("Lock cannot be acquired");

    client
        .send_active_window(&active_window.resource_class, &active_window.caption)
        .with_context(|| "Failed to send heartbeat for active window")
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

pub struct WindowWatcher {
    active_window: Arc<Mutex<ActiveWindow>>,
    // Prolong its lifetime
    _kwin_script: KWinScript,
}

impl Watcher for WindowWatcher {
    fn new(_: &Arc<ReportClient>) -> anyhow::Result<Self> {
        let mut kwin_script = KWinScript::new(Connection::session()?);
        if kwin_script.is_loaded()? {
            debug!("KWin script is already loaded, unloading");
            kwin_script.unload()?;
        }
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
                    tx.send(None).unwrap();
                    loop {
                        connection.monitor_activity().wait();
                    }
                }
                Err(e) => tx.send(Some(e)),
            }
        });
        if let Some(error) = rx.recv().unwrap() {
            panic!("Failed to run a DBus interface: {error}");
        }

        Ok(Self {
            active_window,
            _kwin_script: kwin_script,
        })
    }

    fn run_iteration(&mut self, client: &Arc<ReportClient>) -> anyhow::Result<()> {
        send_active_window(client, &self.active_window)
    }
}
