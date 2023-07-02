use super::{idle, Watcher};
use crate::report_client::ReportClient;
use anyhow::Context;
use async_trait::async_trait;
use std::sync::Arc;
use zbus::blocking::Connection;

pub struct IdleWatcher {
    dbus_connection: Connection,
    is_idle: bool,
}

impl idle::SinceLastInput for IdleWatcher {
    fn seconds_since_input(&mut self) -> anyhow::Result<u32> {
        let ms = self
            .dbus_connection
            .call_method(
                Some("org.gnome.Mutter.IdleMonitor"),
                "/org/gnome/Mutter/IdleMonitor/Core",
                Some("org.gnome.Mutter.IdleMonitor"),
                "GetIdletime",
                &(),
            )?
            .body::<u64>()?;
        u32::try_from(ms / 1000).with_context(|| format!("Number {ms} is invalid"))
    }
}

#[async_trait]
impl Watcher for IdleWatcher {
    async fn new(_: &Arc<ReportClient>) -> anyhow::Result<Self> {
        let mut watcher = Self {
            dbus_connection: Connection::session()?,
            is_idle: false,
        };
        idle::SinceLastInput::seconds_since_input(&mut watcher)?;

        Ok(watcher)
    }

    async fn run_iteration(&mut self, client: &Arc<ReportClient>) -> anyhow::Result<()> {
        self.is_idle = idle::ping_since_last_input(self, self.is_idle, client).await?;

        Ok(())
    }
}
