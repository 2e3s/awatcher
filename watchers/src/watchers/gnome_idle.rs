use super::{idle, Watcher};
use crate::report_client::ReportClient;
use anyhow::Context;
use std::{sync::Arc, thread};
use zbus::blocking::Connection;

pub struct IdleWatcher {
    dbus_connection: Connection,
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

impl Watcher for IdleWatcher {
    fn new() -> anyhow::Result<Self> {
        let mut watcher = Self {
            dbus_connection: Connection::session()?,
        };
        idle::SinceLastInput::seconds_since_input(&mut watcher)?;

        Ok(watcher)
    }

    fn watch(&mut self, client: &Arc<ReportClient>) {
        let mut is_idle = false;
        info!("Starting idle watcher");
        loop {
            match idle::ping_since_last_input(self, is_idle, client) {
                Ok(is_idle_again) => {
                    is_idle = is_idle_again;
                }
                Err(e) => error!("Error on idle iteration: {e}"),
            };
            thread::sleep(client.config.poll_time_idle);
        }
    }
}
