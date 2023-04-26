use crate::report_client::ReportClient;
use anyhow::Context;
use serde::Deserialize;
use std::{sync::Arc, thread};
use zbus::blocking::Connection;

use super::Watcher;

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
    fn get_window_data(&self) -> anyhow::Result<WindowData> {
        let call_response = self.dbus_connection.call_method(
            Some("org.gnome.Shell"),
            "/org/gnome/shell/extensions/FocusedWindow",
            Some("org.gnome.shell.extensions.FocusedWindow"),
            "Get",
            &(),
        );

        match call_response {
            Ok(json) => {
                let json = json
                    .body::<String>()
                    .with_context(|| "DBus interface cannot be parsed as string")?;
                serde_json::from_str(&json).with_context(|| {
                    "DBus interface org.gnome.shell.extensions.FocusedWindow returned wrong JSON"
                })
            }
            Err(e) => {
                if e.to_string().contains("No window in focus") {
                    Ok(WindowData::default())
                } else {
                    Err(e.into())
                }
            }
        }
    }

    fn send_active_window(&mut self, client: &ReportClient) -> anyhow::Result<()> {
        let data = self.get_window_data()?;

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
            .with_context(|| "Failed to send heartbeat for active window")
    }
}

impl Watcher for WindowWatcher {
    fn new() -> anyhow::Result<Self> {
        let watcher = Self {
            dbus_connection: Connection::session()?,
            last_app_id: String::new(),
            last_title: String::new(),
        };

        Ok(watcher)
    }

    fn watch(&mut self, client: &Arc<ReportClient>) {
        info!("Starting active window watcher");
        loop {
            if let Err(error) = self.send_active_window(client) {
                error!("Error on active window: {error}");
            }
            thread::sleep(client.config.poll_time_window);
        }
    }
}
