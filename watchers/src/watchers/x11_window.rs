use super::{x11_connection::X11Client, Watcher};
use crate::report_client::ReportClient;
use anyhow::Context;
use async_trait::async_trait;
use std::sync::Arc;

pub struct WindowWatcher {
    client: X11Client,
    last_app_id: String,
    last_title: String,
    last_wm_instance: String,
}

impl WindowWatcher {
    async fn send_active_window(&mut self, client: &ReportClient) -> anyhow::Result<()> {
        let data = self.client.active_window_data()?;

        let (app_id, title, wm_instance) = match data {
            Some(window_data) => (
                window_data.app_id,
                window_data.title,
                window_data.wm_instance,
            ),
            None => {
                // No active window, set all values to "aw-none"
                ("aw-none".to_string(), "aw-none".to_string(), "aw-none".to_string())
            }
        };

        if app_id != self.last_app_id || title != self.last_title || wm_instance != self.last_wm_instance {
            debug!(
                r#"Changed window app_id="{}", title="{}", wm_instance="{}""#,
                app_id, title, wm_instance
            );
            self.last_app_id = app_id.clone();
            self.last_title = title.clone();
            self.last_wm_instance = wm_instance.clone();

        }

        client
            .send_active_window_with_instance(&app_id, &title, Some(&wm_instance))
            .await
            .with_context(|| "Failed to send heartbeat for active window")
    }
}

#[async_trait]
impl Watcher for WindowWatcher {
    async fn new(_: &Arc<ReportClient>) -> anyhow::Result<Self> {
        let mut client = X11Client::new()?;
        client.active_window_data()?;

        Ok(WindowWatcher {
            client,
            last_title: String::new(),
            last_app_id: String::new(),
            last_wm_instance: String::new(),
        })
    }

    async fn run_iteration(&mut self, client: &Arc<ReportClient>) -> anyhow::Result<()> {
        self.send_active_window(client).await
    }
}
