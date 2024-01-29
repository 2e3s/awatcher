use super::{x11_connection::X11Client, Watcher};
use crate::report_client::ReportClient;
use anyhow::Context;
use std::sync::Arc;

pub struct WindowWatcher {
    client: X11Client,
    last_title: String,
    last_app_id: String,
}

impl WindowWatcher {
    async fn send_active_window(&mut self, client: &ReportClient) -> anyhow::Result<()> {
        let data = self.client.active_window_data()?;

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
            .await
            .with_context(|| "Failed to send heartbeat for active window")
    }
}

impl WindowWatcher {
    pub async fn new(_: &Arc<ReportClient>) -> anyhow::Result<Self> {
        let mut client = X11Client::new()?;
        client.active_window_data()?;

        Ok(WindowWatcher {
            client,
            last_title: String::new(),
            last_app_id: String::new(),
        })
    }

}

impl Watcher for WindowWatcher {
    async fn run_iteration(&mut self, client: &Arc<ReportClient>) -> anyhow::Result<()> {
        self.send_active_window(client).await
    }
}
