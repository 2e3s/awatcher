use super::config::Config;
use anyhow::Context;
use aw_client_rust::{AwClient, Event as AwEvent};
use chrono::{DateTime, Duration, Utc};
use serde_json::{Map, Value};

pub struct ReportClient {
    pub client: AwClient,
    pub config: Config,
    idle_bucket_name: String,
    active_window_bucket_name: String,
}

impl ReportClient {
    pub async fn new(config: Config) -> anyhow::Result<Self> {
        let client = AwClient::new(&config.host, &config.port.to_string(), "awatcher");

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

    pub async fn ping(
        &self,
        is_idle: bool,
        timestamp: DateTime<Utc>,
        duration: Duration,
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

        let pulsetime = (self.config.idle_timeout + self.config.poll_time_idle).as_secs_f64();
        self.client
            .heartbeat(&self.idle_bucket_name, &event, pulsetime)
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
            // Added one nano second to ensure the event is not filtered out by https://github.com/ActivityWatch/aw-webui/blob/cf74080d12ffd53d37c83e3e0eddaf952fe14a62/src/visualizations/VisTimeline.vue#L108
            duration: Duration::from_std(self.config.poll_time_window).unwrap() + Duration::nanoseconds(1),
            data,
        };

        if self.config.no_server {
            return Ok(());
        }

        let interval_margin = self.config.poll_time_window.as_secs_f64() + 1.0;
        self.client
            .heartbeat(&self.active_window_bucket_name, &event, interval_margin)
            .await
            .with_context(|| "Failed to send heartbeat for active window")
    }

    async fn create_bucket(
        client: &AwClient,
        bucket_name: &str,
        bucket_type: &str,
    ) -> anyhow::Result<()> {
        client
            .create_bucket_simple(bucket_name, bucket_type)
            .await
            .with_context(|| format!("Failed to create bucket {bucket_name}"))
    }
}
