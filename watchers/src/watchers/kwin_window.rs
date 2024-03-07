/*
 * This uses a hack with KWin scripts in order to receive the active window.
 * For the moment of writing, KWin doesn't implement the appropriate protocols to get a top level window.
 * Inspired by https://github.com/k0kubun/xremap/
 */
use super::Watcher;
use crate::report_client::ReportClient;
use anyhow::{anyhow, Context};
use async_trait::async_trait;
use std::env::{self, temp_dir};
use std::path::Path;
use std::sync::{mpsc::channel, Arc};
use std::thread;
use tokio::sync::Mutex;
use zbus::dbus_interface;
use zbus::{Connection, ConnectionBuilder};

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

    async fn load(&mut self) -> anyhow::Result<()> {
        let path = temp_dir().join("kwin_window.js");
        std::fs::write(&path, KWIN_SCRIPT).unwrap();

        let number = self.get_registered_number(&path).await?;
        let result = self.start(number).await;
        std::fs::remove_file(&path)?;
        self.is_loaded = true;

        result
    }

    async fn is_loaded(&self) -> anyhow::Result<bool> {
        self.dbus_connection
            .call_method(
                Some("org.kde.KWin"),
                "/Scripting",
                Some("org.kde.kwin.Scripting"),
                "isScriptLoaded",
                &KWIN_SCRIPT_NAME,
            )
            .await?
            .body::<bool>()
            .map_err(std::convert::Into::into)
    }

    async fn get_registered_number(&self, path: &Path) -> anyhow::Result<i32> {
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
            )
            .await?
            .body::<i32>()
            .map_err(std::convert::Into::into)
    }

    async fn unload(&self) -> anyhow::Result<bool> {
        self.dbus_connection
            .call_method(
                Some("org.kde.KWin"),
                "/Scripting",
                Some("org.kde.kwin.Scripting"),
                "unloadScript",
                &KWIN_SCRIPT_NAME,
            )
            .await?
            .body::<bool>()
            .map_err(std::convert::Into::into)
    }

    async fn start(&self, script_number: i32) -> anyhow::Result<()> {
        debug!("Starting KWin script {script_number}");
        self.dbus_connection
            .call_method(
                Some("org.kde.KWin"),
                format!("/Scripting/Script{script_number}"),
                Some("org.kde.kwin.Script"),
                "run",
                &(),
            )
            .await
            .with_context(|| "Error on starting the script")?;
        Ok(())
    }
}

impl Drop for KWinScript {
    fn drop(&mut self) {
        if self.is_loaded {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    debug!("Unloading KWin script");
                    if let Err(e) = self.unload().await {
                        error!("Problem during stopping KWin script: {e}");
                    };
                });
        }
    }
}

async fn send_active_window(
    client: &ReportClient,
    active_window: &Arc<Mutex<ActiveWindow>>,
) -> anyhow::Result<()> {
    let active_window = active_window.lock().await;

    client
        .send_active_window(&active_window.resource_class, &active_window.caption)
        .await
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
    async fn notify_active_window(
        &mut self,
        caption: String,
        resource_class: String,
        resource_name: String,
    ) {
        debug!("Active window class: \"{resource_class}\", name: \"{resource_name}\", caption: \"{caption}\"");
        let mut active_window = self.active_window.lock().await;
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

#[async_trait]
impl Watcher for WindowWatcher {
    async fn new(_: &Arc<ReportClient>) -> anyhow::Result<Self> {
        let mut kwin_script = KWinScript::new(Connection::session().await?);
        if kwin_script.is_loaded().await? {
            debug!("KWin script is already loaded, unloading");
            kwin_script.unload().await?;
        }
        if env::var("WAYLAND_DISPLAY").is_err()
            && env::var_os("XDG_SESSION_TYPE").unwrap_or("".into()) == "x11"
        {
            return Err(anyhow!("X11 should be tried instead"));
        }

        kwin_script.load().await.unwrap();

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
            async fn get_connection(
                active_window_interface: ActiveWindowInterface,
            ) -> zbus::Result<Connection> {
                ConnectionBuilder::session()?
                    .name("com._2e3s.Awatcher")?
                    .serve_at("/com/_2e3s/Awatcher", active_window_interface)?
                    .build()
                    .await
            }

            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    match get_connection(active_window_interface).await {
                        Ok(connection) => {
                            tx.send(None).unwrap();
                            loop {
                                connection.monitor_activity().wait();
                            }
                        }
                        Err(e) => tx.send(Some(e)),
                    }
                })
                .unwrap();
        });
        if let Some(error) = rx.recv().unwrap() {
            panic!("Failed to run a DBus interface: {error}");
        }

        Ok(Self {
            active_window,
            _kwin_script: kwin_script,
        })
    }

    async fn run_iteration(&mut self, client: &Arc<ReportClient>) -> anyhow::Result<()> {
        send_active_window(client, &self.active_window).await
    }
}
