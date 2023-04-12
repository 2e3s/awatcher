use std::error::Error;

use super::BoxedError;
use super::Config;
use aw_client_rust::{AwClient, Event as AwEvent};
use chrono::{DateTime, Duration, Utc};
use serde_json::{Map, Value};

pub struct ReportClient {
    pub client: AwClient,
    pub config: Config,
}

impl ReportClient {
    pub fn new(config: Config) -> Self {
        let host = config.host.clone();
        let port = config.port.to_string();

        Self {
            config,
            client: AwClient::new(&host, &port, "awatcher"),
        }
    }

    pub fn ping(
        &self,
        bucket_name: &str,
        is_idle: bool,
        timestamp: DateTime<Utc>,
        duration: Duration,
    ) -> Result<(), Box<dyn Error>> {
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

        let pulsetime = f64::from(self.config.idle_timeout + self.config.poll_time_idle);
        self.client
            .heartbeat(bucket_name, &event, pulsetime)
            .map_err(|_| "Failed to send heartbeat")?;

        Ok(())
    }

    pub fn heartbeat(&self, bucket_name: &str, event: &AwEvent) -> Result<(), BoxedError> {
        let interval_margin: f64 = f64::from(self.config.poll_time_idle + 1);
        self.client
            .heartbeat(bucket_name, event, interval_margin)
            .map_err(|_| "Failed to send heartbeat for active window".into())
    }

    pub fn create_bucket(
        &self,
        bucket_name: &str,
        bucket_type: &str,
    ) -> Result<(), Box<dyn Error>> {
        self.client
            .create_bucket_simple(bucket_name, bucket_type)
            .map_err(|e| format!("Failed to create bucket {bucket_name}: {e}").into())
    }
}
