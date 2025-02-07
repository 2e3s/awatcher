use super::{x11_connection::X11Client, Watcher};
use crate::report_client::ReportClient;
use anyhow::Context;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

pub struct WindowWatcher {
    client: X11Client,
    last_app_id: String,
    last_title: String,
    last_wm_instance: String,
}

impl WindowWatcher {
    pub async fn send_active_window_with_instance(
        &self,
        client: &ReportClient,
        app_id: &str,
        title: &str,
        wm_instance: &str,
    ) -> anyhow::Result<()> {
        let mut extra_data = HashMap::new();
        extra_data.insert("wm_instance".to_string(), wm_instance.to_string());
        client
            .send_active_window_with_extra(app_id, title, Some(extra_data))
            .await
    }

    async fn send_active_window(&mut self, client: &ReportClient) -> anyhow::Result<()> {
        let data = self.client.active_window_data()?;

        if data.app_id != self.last_app_id
            || data.title != self.last_title
            || data.wm_instance != self.last_wm_instance
        {
            debug!(
                r#"Changed window app_id="{}", title="{}", wm_instance="{}""#,
                data.app_id, data.title, data.wm_instance
            );
            self.last_app_id = data.app_id.clone();
            self.last_title = data.title.clone();
            self.last_wm_instance = data.wm_instance.clone();
        }

        self.send_active_window_with_instance(
            client,
            &self.last_app_id,
            &self.last_title,
            &self.last_wm_instance,
        )
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
