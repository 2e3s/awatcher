use super::{idle, x11_connection::X11Client, Watcher};
use crate::report_client::ReportClient;
use std::{sync::Arc, thread};

pub struct IdleWatcher {
    client: X11Client,
}

impl idle::SinceLastInput for IdleWatcher {
    fn seconds_since_input(&mut self) -> anyhow::Result<u32> {
        self.client.seconds_since_last_input()
    }
}

impl Watcher for IdleWatcher {
    fn new() -> anyhow::Result<Self> {
        let mut client = X11Client::new()?;

        // Check if screensaver extension is supported
        client.seconds_since_last_input()?;

        Ok(IdleWatcher { client })
    }

    fn watch(&mut self, client: &Arc<ReportClient>) {
        info!("Starting idle watcher");
        let mut is_idle = false;
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
