use std::{sync::Arc, thread};

use super::{idle, x11_connection::X11Connection, BoxedError, Watcher};
use crate::report_client::ReportClient;

pub struct IdleWatcher {
    connection: X11Connection,
}

impl idle::SinceLastInput for IdleWatcher {
    fn seconds_since_input(&self) -> Result<u32, BoxedError> {
        self.connection.seconds_since_last_input()
    }
}

impl Watcher for IdleWatcher {
    fn new() -> Result<Self, BoxedError> {
        let connection = X11Connection::new()?;

        // Check if screensaver extension is supported
        connection.seconds_since_last_input()?;

        Ok(IdleWatcher { connection })
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
