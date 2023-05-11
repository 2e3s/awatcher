use super::{idle, x11_connection::X11Client, Watcher};
use crate::report_client::ReportClient;
use std::sync::Arc;

pub struct IdleWatcher {
    client: X11Client,
    is_idle: bool,
}

impl idle::SinceLastInput for IdleWatcher {
    fn seconds_since_input(&mut self) -> anyhow::Result<u32> {
        self.client.seconds_since_last_input()
    }
}

impl Watcher for IdleWatcher {
    fn new(_: &Arc<ReportClient>) -> anyhow::Result<Self> {
        let mut client = X11Client::new()?;

        // Check if screensaver extension is supported
        client.seconds_since_last_input()?;

        Ok(IdleWatcher {
            client,
            is_idle: false,
        })
    }

    fn run_iteration(&mut self, client: &Arc<ReportClient>) {
        match idle::ping_since_last_input(self, self.is_idle, client) {
            Ok(is_idle_again) => {
                self.is_idle = is_idle_again;
            }
            Err(e) => error!("Error on idle iteration: {e}"),
        };
    }
}
