use crate::report_client::ReportClient;
use chrono::{DateTime, Duration, Utc};
use std::{cmp::max, sync::Arc};

pub struct State {
    last_input_time: DateTime<Utc>,
    changed_time: DateTime<Utc>,
    is_idle: bool,
    is_changed: bool,
    idle_timeout: Duration,
}

impl State {
    pub fn new(idle_timeout: Duration) -> Self {
        Self {
            last_input_time: Utc::now(),
            changed_time: Utc::now(),
            is_idle: false,
            is_changed: false,
            idle_timeout,
        }
    }

    fn set_idle(&mut self, is_idle: bool, now: DateTime<Utc>) {
        self.is_idle = is_idle;
        self.is_changed = true;
        self.changed_time = now;
    }

    pub fn mark_not_idle(&mut self) {
        self.last_input_time = Utc::now();
        self.set_idle(false, self.last_input_time);
    }

    pub fn mark_idle(&mut self) {
        self.set_idle(true, Utc::now());
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
            self.set_idle(false, now);
        } else if !self.is_idle
            && u64::from(seconds_since_input) >= self.idle_timeout.num_seconds().try_into().unwrap()
        {
            debug!("Idle again");
            self.set_idle(true, now);
        }

        self.send_ping(now, client).await
    }

    pub async fn send_reactive(&mut self, client: &Arc<ReportClient>) -> anyhow::Result<()> {
        let now = Utc::now();
        if !self.is_idle {
            self.last_input_time = max(now - self.idle_timeout, self.changed_time);
        }

        self.send_ping(now, client).await
    }

    async fn send_ping(
        &mut self,
        now: DateTime<Utc>,
        client: &Arc<ReportClient>,
    ) -> anyhow::Result<()> {
        if self.is_changed {
            let result = if self.is_idle {
                debug!("Reporting as changed to idle");
                client
                    .ping(false, self.last_input_time, Duration::zero())
                    .await?;

                // ping with timestamp+1ms with the next event (to ensure the latest event gets retrieved by get_event)
                self.last_input_time += Duration::milliseconds(1);
                client
                    .ping(true, self.last_input_time, now - self.last_input_time)
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
