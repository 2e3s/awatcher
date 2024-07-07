use super::{gnome_wayland::load_watcher, idle, Watcher};
use crate::report_client::ReportClient;
use anyhow::Context;
use async_trait::async_trait;
use chrono::Utc;
use std::sync::Arc;
use zbus::Connection;

pub struct IdleWatcher {
    dbus_connection: Connection,
    idle_state: idle::Tracker,
}

impl IdleWatcher {
    async fn seconds_since_input(&mut self) -> anyhow::Result<u32> {
        let ms: u64 = self
            .dbus_connection
            .call_method(
                Some("org.gnome.Mutter.IdleMonitor"),
                "/org/gnome/Mutter/IdleMonitor/Core",
                Some("org.gnome.Mutter.IdleMonitor"),
                "GetIdletime",
                &(),
            )
            .await?
            .body()
            .deserialize()?;
        u32::try_from(ms / 1000).with_context(|| format!("Number {ms} is invalid"))
    }
}

#[async_trait]
impl Watcher for IdleWatcher {
    async fn new(client: &Arc<ReportClient>) -> anyhow::Result<Self> {
        let duration = client.config.idle_timeout;
        load_watcher(|| async move {
            let mut watcher = Self {
                dbus_connection: Connection::session().await?,
                idle_state: idle::Tracker::new(Utc::now(), duration),
            };
            watcher.seconds_since_input().await?;
            Ok(watcher)
        })
        .await
    }

    async fn run_iteration(&mut self, client: &Arc<ReportClient>) -> anyhow::Result<()> {
        let seconds = self.seconds_since_input().await?;
        client
            .handle_idle_status(self.idle_state.get_with_last_input(Utc::now(), seconds)?)
            .await
    }
}
