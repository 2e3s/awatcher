use super::{x11_connection::X11Connection, Watcher};
use crate::report_client::ReportClient;
use anyhow::Context;
use std::thread;

pub struct WindowWatcher {
    connection: X11Connection,
    last_title: String,
    last_app_id: String,
}

impl WindowWatcher {
    fn send_active_window(&mut self, client: &ReportClient) -> anyhow::Result<()> {
        let data = self.connection.active_window_data()?;

        if data.app_id != self.last_app_id || data.title != self.last_title {
            debug!(
                r#"Changed window app_id="{}", title="{}""#,
                data.app_id, data.title
            );
            self.last_app_id = data.app_id;
            self.last_title = data.title;
        }

        client
            .send_active_window(&self.last_app_id, &self.last_title)
            .with_context(|| "Failed to send heartbeat for active window")
    }
}

impl Watcher for WindowWatcher {
    fn new() -> anyhow::Result<Self> {
        let connection = X11Connection::new()?;
        connection.active_window_data()?;

        Ok(WindowWatcher {
            connection,
            last_title: String::new(),
            last_app_id: String::new(),
        })
    }

    fn watch(&mut self, client: &std::sync::Arc<crate::report_client::ReportClient>) {
        info!("Starting active window watcher");
        loop {
            if let Err(error) = self.send_active_window(client) {
                error!("Error on sending active window: {error}");
            }

            thread::sleep(client.config.poll_time_window);
        }
    }
}
