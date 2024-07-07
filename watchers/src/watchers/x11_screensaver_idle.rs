use async_trait::async_trait;
use chrono::Utc;

use super::{idle, x11_connection::X11Client, Watcher};
use crate::report_client::ReportClient;
use std::sync::Arc;

pub struct IdleWatcher {
    client: X11Client,
    idle_state: idle::Tracker,
}

impl IdleWatcher {
    async fn seconds_since_input(&mut self) -> anyhow::Result<u32> {
        self.client.seconds_since_last_input()
    }
}

#[async_trait]
impl Watcher for IdleWatcher {
    async fn new(report_client: &Arc<ReportClient>) -> anyhow::Result<Self> {
        let mut client = X11Client::new()?;

        // Check if screensaver extension is supported
        client.seconds_since_last_input()?;

        Ok(IdleWatcher {
            client,
            idle_state: idle::Tracker::new(Utc::now(), report_client.config.idle_timeout),
        })
    }

    async fn run_iteration(&mut self, client: &Arc<ReportClient>) -> anyhow::Result<()> {
        let seconds = self.seconds_since_input().await?;

        client
            .handle_idle_status(self.idle_state.get_with_last_input(Utc::now(), seconds)?)
            .await
    }
}
