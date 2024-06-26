use crate::subscriber::IdleSubscriber;
use chrono::{DateTime, TimeDelta, Utc};
use std::{cmp::max, sync::Arc};

pub struct State {
    last_input_time: DateTime<Utc>,
    changed_time: DateTime<Utc>,
    is_idle: bool,
    is_changed: bool,
    idle_timeout: TimeDelta,
    subscriber: Arc<dyn IdleSubscriber>,

    idle_start: Option<DateTime<Utc>>,
    idle_end: Option<DateTime<Utc>>,
}

impl State {
    pub fn new(idle_timeout: TimeDelta, subscriber: Arc<dyn IdleSubscriber>) -> Self {
        Self {
            last_input_time: Utc::now(),
            changed_time: Utc::now(),
            is_idle: false,
            is_changed: false,
            idle_timeout,
            idle_start: None,
            idle_end: None,
            subscriber,
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

        self.idle_end = self.changed_time.into();
    }

    pub fn mark_idle(&mut self) {
        self.set_idle(true, Utc::now());

        self.idle_start = self.changed_time.into();
    }

    // The logic is rewritten from the original Python code:
    // https://github.com/ActivityWatch/aw-watcher-afk/blob/ef531605cd8238e00138bbb980e5457054e05248/aw_watcher_afk/afk.py#L73
    pub async fn send_with_last_input(&mut self, seconds_since_input: u32) -> anyhow::Result<()> {
        let now = Utc::now();
        let time_since_input = TimeDelta::seconds(i64::from(seconds_since_input));

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

        self.send_ping(now).await
    }

    pub async fn send_reactive(&mut self) -> anyhow::Result<()> {
        let now = Utc::now();
        if !self.is_idle {
            self.last_input_time = max(now - self.idle_timeout, self.changed_time);
            if let (Some(idle_start), Some(idle_end)) = (self.idle_start, self.idle_end) {
                if !self.is_changed
                    && idle_start <= self.last_input_time
                    && self.last_input_time <= idle_end
                {
                    warn!("Active time may not be accounted for.");

                    // TODO: send the correct timings.
                    // After idle_end there is some active time for idle_timeout which may be accounted as idle time if it becomes idle soon.
                    return Ok(());
                }
            }
        }

        self.send_ping(now).await
    }

    async fn send_ping(&mut self, now: DateTime<Utc>) -> anyhow::Result<()> {
        if self.is_changed {
            if self.is_idle {
                self.subscriber
                    .idle(
                        self.is_changed,
                        self.last_input_time,
                        now - self.last_input_time,
                    )
                    .await?;
            } else {
                self.subscriber
                    .non_idle(self.is_changed, self.last_input_time)
                    .await?;
            };
        } else if self.is_idle {
            self.subscriber
                .idle(
                    self.is_changed,
                    self.last_input_time,
                    now - self.last_input_time,
                )
                .await?;
        } else {
            self.subscriber
                .non_idle(self.is_changed, self.last_input_time)
                .await?;
        }
        self.is_changed = false;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use chrono::Duration;
    use mockall::mock;
    use rstest::rstest;

    mock! {
        pub Subscriber {}
        #[async_trait]
        impl IdleSubscriber for Subscriber {
            async fn idle(&self, changed: bool, last_input_time: DateTime<Utc>, duration: TimeDelta) -> anyhow::Result<()>;
            async fn non_idle(&self, changed: bool, last_input_time: DateTime<Utc>) -> anyhow::Result<()>;
        }
    }

    #[rstest]
    #[tokio::test]
    async fn test_mark_not_idle() {
        let subscriber = Arc::new(MockSubscriber::new());
        let mut state = State::new(Duration::seconds(300), subscriber.clone());

        state.mark_not_idle();
        assert!(!state.is_idle);
        assert!(state.is_changed);
    }

    #[rstest]
    #[tokio::test]
    async fn test_mark_idle() {
        let subscriber = Arc::new(MockSubscriber::new());
        let mut state = State::new(Duration::seconds(300), subscriber.clone());

        state.mark_idle();
        assert!(state.is_idle);
        assert!(state.is_changed);
    }
}
