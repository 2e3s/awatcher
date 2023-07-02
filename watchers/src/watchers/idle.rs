use crate::report_client::ReportClient;
use chrono::{Duration, Utc};
use std::sync::Arc;

pub trait SinceLastInput {
    fn seconds_since_input(&mut self) -> anyhow::Result<u32>;
}

pub async fn ping_since_last_input(
    watcher: &mut impl SinceLastInput,
    is_idle: bool,
    client: &Arc<ReportClient>,
) -> anyhow::Result<bool> {
    // The logic is rewritten from the original Python code:
    // https://github.com/ActivityWatch/aw-watcher-afk/blob/ef531605cd8238e00138bbb980e5457054e05248/aw_watcher_afk/afk.py#L73
    let duration_1ms: Duration = Duration::milliseconds(1);
    let duration_zero: Duration = Duration::zero();

    let seconds_since_input = watcher.seconds_since_input()?;
    let now = Utc::now();
    let time_since_input = Duration::seconds(i64::from(seconds_since_input));
    let last_input = now - time_since_input;
    let mut is_idle_again = is_idle;

    if is_idle && u64::from(seconds_since_input) < client.config.idle_timeout.as_secs() {
        debug!("No longer idle");
        client.ping(is_idle, last_input, duration_zero).await?;
        is_idle_again = false;
        // ping with timestamp+1ms with the next event (to ensure the latest event gets retrieved by get_event)
        client
            .ping(is_idle, last_input + duration_1ms, duration_zero)
            .await?;
    } else if !is_idle && u64::from(seconds_since_input) >= client.config.idle_timeout.as_secs() {
        debug!("Idle again");
        client.ping(is_idle, last_input, duration_zero).await?;
        is_idle_again = true;
        // ping with timestamp+1ms with the next event (to ensure the latest event gets retrieved by get_event)
        client
            .ping(is_idle, last_input + duration_1ms, time_since_input)
            .await?;
    } else {
        // Send a heartbeat if no state change was made
        if is_idle {
            trace!("Reporting as idle");
            client.ping(is_idle, last_input, time_since_input).await?;
        } else {
            trace!("Reporting as not idle");
            client.ping(is_idle, last_input, duration_zero).await?;
        }
    }

    Ok(is_idle_again)
}
