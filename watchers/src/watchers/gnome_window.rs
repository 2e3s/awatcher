use crate::report_client::ReportClient;
use anyhow::Context;
use async_trait::async_trait;
use serde::Deserialize;
use std::sync::Arc;
use zbus::Connection;

use super::{gnome_wayland::load_watcher, gnome_wayland::GnomeWatcher, Watcher};

pub struct WindowWatcher {
    dbus_connection: Connection,
    last_title: String,
    last_app_id: String,
}

#[derive(Deserialize, Default)]
struct WindowData {
    title: String,
    wm_class: String,
}

impl WindowWatcher {
    async fn get_window_data(&self) -> anyhow::Result<WindowData> {
        let call_response = self
            .dbus_connection
            .call_method(
                Some("org.gnome.Shell"),
                "/org/gnome/shell/extensions/FocusedWindow",
                Some("org.gnome.shell.extensions.FocusedWindow"),
                "Get",
                &(),
            )
            .await;

        match call_response {
            Ok(json) => {
                let json = json
                    .body::<String>()
                    .with_context(|| "DBus interface cannot be parsed as string")?;
                serde_json::from_str(&json).with_context(|| {
                    format!("DBus interface org.gnome.shell.extensions.FocusedWindow returned wrong JSON: {json}")
                })
            }
            Err(e) => {
                if e.to_string().contains("No window in focus") {
                    trace!("No window is active");
                    Ok(WindowData::default())
                } else {
                    Err(e.into())
                }
            }
        }
    }

    async fn send_active_window(&mut self, client: &ReportClient) -> anyhow::Result<()> {
        let data = self.get_window_data().await;
        if let Err(e) = data {
            if e.to_string().contains("Object does not exist at path") {
                trace!("The extension seems to have stopped");
                return Ok(());
            }
            return Err(e);
        }
        let data = data?;

        if data.wm_class != self.last_app_id || data.title != self.last_title {
            debug!(
                r#"Changed window app_id="{}", title="{}""#,
                data.wm_class, data.title
            );
            self.last_app_id = data.wm_class;
            self.last_title = data.title;
        }

        client
            .send_active_window(&self.last_app_id, &self.last_title)
            .await
            .with_context(|| "Failed to send heartbeat for active window")
    }
}

impl GnomeWatcher for WindowWatcher {
    async fn load() -> anyhow::Result<Self> {
        let watcher = Self {
            dbus_connection: Connection::session().await?,
            last_app_id: String::new(),
            last_title: String::new(),
        };
        watcher.get_window_data().await?;

        Ok(watcher)
    }
}

#[async_trait]
impl Watcher for WindowWatcher {
    async fn new(_: &Arc<ReportClient>) -> anyhow::Result<Self> {
        load_watcher().await
    }

    async fn run_iteration(&mut self, client: &Arc<ReportClient>) -> anyhow::Result<()> {
        self.send_active_window(client).await
    }
}
