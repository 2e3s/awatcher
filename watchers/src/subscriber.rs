use async_trait::async_trait;
use chrono::{DateTime, TimeDelta, Utc};

#[async_trait]
pub trait IdleSubscriber: Sync + Send {
    async fn idle(
        &self,
        changed: bool,
        last_input_time: DateTime<Utc>,
        duration: TimeDelta,
    ) -> anyhow::Result<()>;

    async fn non_idle(&self, changed: bool, last_input_time: DateTime<Utc>) -> anyhow::Result<()>;
}
