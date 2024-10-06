use crate::watchers::idle::Status;

use super::config::Config;
use anyhow::Context;
use aw_client_rust::{AwClient, Event as AwEvent};
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::{Map, Value};
use std::error::Error;
use std::future::Future;

pub struct ReportClient {
    pub client: AwClient,
    pub config: Config,
    idle_bucket_name: String,
    active_window_bucket_name: String,
}

impl ReportClient {
    pub async fn new(config: Config) -> anyhow::Result<Self, Box<dyn Error>> {
        let client = AwClient::new(&config.host, config.port, "awatcher")?;

        let hostname = gethostname::gethostname().into_string().unwrap();
        let idle_bucket_name = format!("aw-watcher-afk_{hostname}");
        let active_window_bucket_name = format!("aw-watcher-window_{hostname}");
        if !config.no_server {
            Self::create_bucket(&client, &idle_bucket_name, "afkstatus").await?;
            Self::create_bucket(&client, &active_window_bucket_name, "currentwindow").await?;
        }

        Ok(Self {
            client,
            config,
            idle_bucket_name,
            active_window_bucket_name,
        })
    }

    async fn run_with_retries<F, Fut, T, E>(f: F) -> Result<T, E>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T, E>>,
        E: std::error::Error + Send + Sync + 'static,
    {
        for (attempt, secs) in [0.01, 0.1, 1., 2.].iter().enumerate() {
            match f().await {
                Ok(val) => {
                    if attempt > 0 {
                        debug!("OK at attempt #{}", attempt + 1);
                    }
                    return Ok(val);
                }
                Err(e) => {
                    warn!("Failed on attempt #{}, retrying in {:.1}s: {}", attempt + 1, secs, e);
                    tokio::time::sleep(tokio::time::Duration::from_secs_f64(*secs)).await;
                }
            }
        }

        f().await
    }

    pub async fn ping(
        &self,
        is_idle: bool,
        timestamp: DateTime<Utc>,
        duration: TimeDelta,
    ) -> anyhow::Result<()> {
        let mut data = Map::new();
        data.insert(
            "status".to_string(),
            Value::String((if is_idle { "afk" } else { "not-afk" }).to_string()),
        );

        let event = AwEvent {
            id: None,
            timestamp,
            duration,
            data,
        };

        if self.config.no_server {
            return Ok(());
        }

        let pulsetime = (self.config.idle_timeout + self.config.poll_time_idle).num_seconds();
        let request = || {
            self.client
                .heartbeat(&self.idle_bucket_name, &event, pulsetime as f64)
        };

        Self::run_with_retries(request)
            .await
            .with_context(|| "Failed to send heartbeat")
    }

    pub async fn send_active_window(&self, app_id: &str, title: &str) -> anyhow::Result<()> {
        self.send_active_window_with_instance(app_id, title, None).await
    }

    pub async fn send_active_window_with_instance(
        &self,
        app_id: &str,
        title: &str,
        wm_instance: Option<&str>,
    ) -> anyhow::Result<()> {
        let mut data = Map::new();

        let replacement = self.config.window_data_replacement(app_id, title);
        let inserted_app_id = if let Some(new_app_id) = replacement.replace_app_id {
            trace!("Replacing app_id by {new_app_id}");
            new_app_id
        } else {
            app_id.to_string()
        };
        let inserted_title = if let Some(new_title) = replacement.replace_title {
            trace!("Replacing title of {inserted_app_id} by {new_title}");
            new_title
        } else {
            title.to_string()
        };
        trace!(
            "Reporting app_id: {}, title: {}",
            inserted_app_id,
            inserted_title
        );
        data.insert("app".to_string(), Value::String(inserted_app_id));
        data.insert("title".to_string(), Value::String(inserted_title));

        if let Some(instance) = wm_instance {
            data.insert("wm_instance".to_string(), Value::String(instance.to_string()));
        }

        let event = AwEvent {
            id: None,
            timestamp: Utc::now(),
            duration: TimeDelta::zero(),
            data,
        };

        if self.config.no_server {
            return Ok(());
        }

        let interval_margin = self.config.poll_time_window.num_seconds() + 1;
        let request = || {
            self.client.heartbeat(
                &self.active_window_bucket_name,
                &event,
                interval_margin as f64,
            )
        };

        Self::run_with_retries(request)
            .await
            .with_context(|| "Failed to send heartbeat for active window")
    }

    async fn create_bucket(
        client: &AwClient,
        bucket_name: &str,
        bucket_type: &str,
    ) -> anyhow::Result<()> {
        let request = || client.create_bucket_simple(bucket_name, bucket_type);

        Self::run_with_retries(request)
            .await
            .with_context(|| format!("Failed to create bucket {bucket_name}"))
    }

    pub async fn handle_idle_status(&self, status: Status) -> anyhow::Result<()> {
        match status {
            Status::Idle {
                changed,
                last_input_time,
                duration,
            } => self.idle(changed, last_input_time, duration).await,
            Status::Active {
                changed,
                last_input_time,
            } => self.non_idle(changed, last_input_time).await,
        }
    }

    async fn idle(
        &self,
        changed: bool,
        last_input_time: DateTime<Utc>,
        duration: TimeDelta,
    ) -> anyhow::Result<()> {
        if changed {
            debug!(
                "Reporting as changed to idle for {} seconds since {}",
                duration.num_seconds(),
                last_input_time.format("%Y-%m-%d %H:%M:%S"),
            );
            self.ping(false, last_input_time, TimeDelta::zero()).await?;

            // ping with timestamp+1ms with the next event (to ensure the latest event gets retrieved by get_event)
            self.ping(true, last_input_time, duration + TimeDelta::milliseconds(1))
                .await
        } else {
            trace!(
                "Reporting as idle for {} seconds since {}",
                duration.num_seconds(),
                last_input_time.format("%Y-%m-%d %H:%M:%S"),
            );
            self.ping(true, last_input_time, duration).await
        }
    }

    async fn non_idle(&self, changed: bool, last_input_time: DateTime<Utc>) -> anyhow::Result<()> {
        if changed {
            debug!(
                "Reporting as no longer idle at {}",
                last_input_time.format("%Y-%m-%d %H:%M:%S")
            );

            self.ping(
                true,
                last_input_time - TimeDelta::milliseconds(1),
                TimeDelta::zero(),
            )
            .await?;

            self.ping(false, last_input_time, TimeDelta::zero()).await
        } else {
            trace!(
                "Reporting as not idle at {}",
                last_input_time.format("%Y-%m-%d %H:%M:%S")
            );
            self.ping(false, last_input_time, TimeDelta::zero()).await
        }
    }
}
