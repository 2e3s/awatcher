use async_trait::async_trait;

use super::{idle, x11_connection::X11Client, Watcher};
use crate::report_client::ReportClient;
use std::sync::Arc;

pub struct IdleWatcher {
    client: X11Client,
    idle_state: idle::State,
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
            idle_state: idle::State::new(report_client.config.idle_timeout),
        })
    }

    async fn run_iteration(&mut self, client: &Arc<ReportClient>) -> anyhow::Result<()> {
        let seconds = self.seconds_since_input().await?;
        self.idle_state
            .send_with_last_input(seconds, client)
            .await?;

        Ok(())
    }
}
