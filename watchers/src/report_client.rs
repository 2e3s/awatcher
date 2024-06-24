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
        for (attempt, &secs) in [1, 2].iter().enumerate() {
            match f().await {
                Ok(val) => return Ok(val),
                Err(e)
                    if e.to_string()
                        .contains("tcp connect error: Connection refused") =>
                {
                    warn!("Failed to connect on attempt #{attempt}, retrying: {}", e);

                    tokio::time::sleep(tokio::time::Duration::from_secs(secs)).await;
                }
                Err(e) => return Err(e),
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
}
