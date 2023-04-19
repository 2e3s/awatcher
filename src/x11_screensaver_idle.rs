use std::{env, sync::Arc, thread};

use chrono::{Duration, Utc};
use x11rb::connection::Connection;
use x11rb::protocol::screensaver::ConnectionExt;
use x11rb::rust_connection::RustConnection;

use crate::{report_client::ReportClient, BoxedError, Watcher};

pub struct IdleWatcher {
    connection: RustConnection,
    screen_root: u32,
}

impl IdleWatcher {
    fn seconds_since_last_input(&self) -> Result<u32, BoxedError> {
        let a = self.connection.screensaver_query_info(self.screen_root)?;
        let b = a.reply()?;

        Ok(b.ms_since_user_input / 1000)
    }

    fn run(&self, is_idle: bool, client: &Arc<ReportClient>) -> Result<bool, BoxedError> {
        // The logic is rewritten from the original Python code:
        // https://github.com/ActivityWatch/aw-watcher-afk/blob/ef531605cd8238e00138bbb980e5457054e05248/aw_watcher_afk/afk.py#L73
        let duration_1ms: Duration = Duration::milliseconds(1);
        let duration_zero: Duration = Duration::zero();

        let seconds_since_input = self.seconds_since_last_input()?;
        let now = Utc::now();
        let time_since_input = Duration::seconds(i64::from(seconds_since_input));
        let last_input = now - time_since_input;
        let mut is_idle_again = is_idle;

        if is_idle && u64::from(seconds_since_input) < client.config.idle_timeout.as_secs() {
            debug!("No longer idle");
            client.ping(is_idle, last_input, duration_zero)?;
            is_idle_again = false;
            // ping with timestamp+1ms with the next event (to ensure the latest event gets retrieved by get_event)
            client.ping(is_idle, last_input + duration_1ms, duration_zero)?;
        } else if !is_idle && u64::from(seconds_since_input) >= client.config.idle_timeout.as_secs()
        {
            debug!("Idle again");
            client.ping(is_idle, last_input, duration_zero)?;
            is_idle_again = true;
            // ping with timestamp+1ms with the next event (to ensure the latest event gets retrieved by get_event)
            client.ping(is_idle, last_input + duration_1ms, time_since_input)?;
        } else {
            // Send a heartbeat if no state change was made
            if is_idle {
                trace!("Reporting as idle");
                client.ping(is_idle, last_input, time_since_input)?;
            } else {
                trace!("Reporting as not idle");
                client.ping(is_idle, last_input, duration_zero)?;
            }
        }

        Ok(is_idle_again)
    }
}

impl Watcher for IdleWatcher {
    fn new() -> Result<Self, BoxedError> {
        if env::var("DISPLAY").is_err() {
            warn!("DISPLAY is not set, setting to the default value \":0\"");
            env::set_var("DISPLAY", ":0");
        }

        let (connection, screen_num) = x11rb::connect(None)?;
        let screen_root = connection.setup().roots[screen_num].root;

        Ok(IdleWatcher {
            connection,
            screen_root,
        })
    }

    fn watch(&mut self, client: &Arc<ReportClient>) {
        info!("Starting idle watcher");
        let mut is_idle = false;
        loop {
            match self.run(is_idle, client) {
                Ok(is_idle_again) => {
                    is_idle = is_idle_again;
                }
                Err(e) => error!("Error on idle iteration: {e}"),
            };

            thread::sleep(client.config.poll_time_idle);
        }
    }
}
