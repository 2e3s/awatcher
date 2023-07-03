use async_trait::async_trait;

use super::{idle, x11_connection::X11Client, Watcher};
use crate::report_client::ReportClient;
use std::sync::Arc;

pub struct IdleWatcher {
    client: X11Client,
    is_idle: bool,
}

#[async_trait]
impl idle::SinceLastInput for IdleWatcher {
    async fn seconds_since_input(&mut self) -> anyhow::Result<u32> {
        self.client.seconds_since_last_input()
    }
}

#[async_trait]
impl Watcher for IdleWatcher {
    async fn new(_: &Arc<ReportClient>) -> anyhow::Result<Self> {
        let mut client = X11Client::new()?;

        // Check if screensaver extension is supported
        client.seconds_since_last_input()?;

        Ok(IdleWatcher {
            client,
            is_idle: false,
        })
    }

    async fn run_iteration(&mut self, client: &Arc<ReportClient>) -> anyhow::Result<()> {
        self.is_idle = idle::ping_since_last_input(self, self.is_idle, client).await?;

        Ok(())
    }
}
