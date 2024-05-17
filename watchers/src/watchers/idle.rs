use crate::report_client::ReportClient;
use chrono::{DateTime, Duration, Utc};
use std::sync::Arc;

pub struct State {
    last_input_time: DateTime<Utc>,
    is_idle: bool,
    is_changed: bool,
    idle_timeout: Duration,
}

impl State {
    pub fn new(idle_timeout: Duration) -> Self {
        Self {
            last_input_time: Utc::now(),
            is_idle: false,
            is_changed: false,
            idle_timeout,
        }
    }

    pub fn mark_not_idle(&mut self) {
        self.is_idle = false;
        self.is_changed = true;
        self.last_input_time = Utc::now();
    }

    pub fn mark_idle(&mut self) {
        self.is_idle = true;
        self.is_changed = true;
        self.last_input_time -= self.idle_timeout;
    }

    // The logic is rewritten from the original Python code:
    // https://github.com/ActivityWatch/aw-watcher-afk/blob/ef531605cd8238e00138bbb980e5457054e05248/aw_watcher_afk/afk.py#L73
    pub async fn send_with_last_input(
        &mut self,
        seconds_since_input: u32,
        client: &Arc<ReportClient>,
    ) -> anyhow::Result<()> {
        let now = Utc::now();
        let time_since_input = Duration::seconds(i64::from(seconds_since_input));

        self.last_input_time = now - time_since_input;

        if self.is_idle
            && u64::from(seconds_since_input) < self.idle_timeout.num_seconds().try_into().unwrap()
        {
            debug!("No longer idle");
            self.is_idle = false;
            self.is_changed = true;
        } else if !self.is_idle
            && u64::from(seconds_since_input) >= self.idle_timeout.num_seconds().try_into().unwrap()
        {
            debug!("Idle again");
            self.is_idle = true;
            self.is_changed = true;
        }

        self.send_ping(now, client).await
    }

    pub async fn send_reactive(&mut self, client: &Arc<ReportClient>) -> anyhow::Result<()> {
        let now = Utc::now();
        if !self.is_idle {
            self.last_input_time = now;
        }

        self.send_ping(now, client).await
    }

    async fn send_ping(&mut self, now: DateTime<Utc>, client: &Arc<ReportClient>) -> anyhow::Result<()> {
        if self.is_changed {
            let result = if self.is_idle {
                debug!("Reporting as changed to idle");
                client
                    .ping(false, self.last_input_time, Duration::zero())
                    .await?;

                // ping with timestamp+1ms with the next event (to ensure the latest event gets retrieved by get_event)
                client
                    .ping(
                        true,
                        self.last_input_time + Duration::milliseconds(1),
                        Duration::zero(),
                    )
                    .await
            } else {
                debug!("Reporting as no longer idle");

                client
                    .ping(true, self.last_input_time, Duration::zero())
                    .await?;
                client
                    .ping(
                        false,
                        self.last_input_time + Duration::milliseconds(1),
                        Duration::zero(),
                    )
                    .await
            };
            self.is_changed = false;
            result
        } else if self.is_idle {
            trace!("Reporting as idle");
            client
                .ping(true, self.last_input_time, now - self.last_input_time)
                .await
        } else {
            trace!("Reporting as not idle");
            client
                .ping(false, self.last_input_time, Duration::zero())
                .await
        }
    }
}
