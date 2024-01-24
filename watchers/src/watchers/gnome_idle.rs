use super::{gnome_wayland::load_watcher, gnome_wayland::GnomeWatcher, idle, Watcher};
use crate::report_client::ReportClient;
use anyhow::Context;
use async_trait::async_trait;
use std::sync::Arc;
use zbus::Connection;

pub struct IdleWatcher {
    dbus_connection: Connection,
    is_idle: bool,
}

#[async_trait]
impl idle::SinceLastInput for IdleWatcher {
    async fn seconds_since_input(&mut self) -> anyhow::Result<u32> {
        let ms = self
            .dbus_connection
            .call_method(
                Some("org.gnome.Mutter.IdleMonitor"),
                "/org/gnome/Mutter/IdleMonitor/Core",
                Some("org.gnome.Mutter.IdleMonitor"),
                "GetIdletime",
                &(),
            )
            .await?
            .body::<u64>()?;
        u32::try_from(ms / 1000).with_context(|| format!("Number {ms} is invalid"))
    }
}

impl GnomeWatcher for IdleWatcher {
    async fn load() -> anyhow::Result<Self> {
        let mut watcher = Self {
            dbus_connection: Connection::session().await?,
            is_idle: false,
        };
        idle::SinceLastInput::seconds_since_input(&mut watcher).await?;

        Ok(watcher)
    }
}

#[async_trait]
impl Watcher for IdleWatcher {
    async fn new(_: &Arc<ReportClient>) -> anyhow::Result<Self> {
        load_watcher().await
    }

    async fn run_iteration(&mut self, client: &Arc<ReportClient>) -> anyhow::Result<()> {
        self.is_idle = idle::ping_since_last_input(self, self.is_idle, client).await?;

        Ok(())
    }
}
