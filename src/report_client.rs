use super::config::Config;
use anyhow::Context;
use aw_client_rust::{AwClient, Event as AwEvent};
use chrono::{DateTime, Duration, Utc};
use serde_json::{Map, Value};

pub struct ReportClient {
    pub client: AwClient,
    pub config: Config,
}

impl ReportClient {
    pub fn new(config: Config) -> anyhow::Result<Self> {
        let client = AwClient::new(&config.host, &config.port.to_string(), "awatcher");

        if !config.no_server {
            Self::create_bucket(&client, &config.idle_bucket_name, "afkstatus")?;
            Self::create_bucket(&client, &config.active_window_bucket_name, "currentwindow")?;
        }

        Ok(Self { client, config })
    }

    pub fn ping(
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
            .heartbeat(&self.config.idle_bucket_name, &event, pulsetime)
            .with_context(|| "Failed to send heartbeat")
    }

    pub fn send_active_window(&self, app_id: &str, title: &str) -> anyhow::Result<()> {
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
        trace!("Reporting app_id: {}, title: {}", app_id, title);
        data.insert("app".to_string(), Value::String(inserted_app_id));
        data.insert("title".to_string(), Value::String(inserted_title));
        let event = AwEvent {
            id: None,
            timestamp: Utc::now(),
            duration: Duration::zero(),
            data,
        };

        if self.config.no_server {
            return Ok(());
        }

        let interval_margin = self.config.poll_time_idle.as_secs_f64() + 1.0;
        self.client
            .heartbeat(
                &self.config.active_window_bucket_name,
                &event,
                interval_margin,
            )
            .with_context(|| "Failed to send heartbeat for active window")
    }

    fn create_bucket(
        client: &AwClient,
        bucket_name: &str,
        bucket_type: &str,
    ) -> anyhow::Result<()> {
        client
            .create_bucket_simple(bucket_name, bucket_type)
            .with_context(|| format!("Failed to create bucket {bucket_name}"))
    }
}
