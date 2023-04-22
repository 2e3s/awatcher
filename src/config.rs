mod defaults;
mod file_config;
mod filters;

use crate::BoxedError;
use clap::{arg, value_parser, Command};
use file_config::FileConfig;
use std::{path::PathBuf, time::Duration};

use self::filters::{Filter, Replacement};

pub struct Config {
    pub port: u32,
    pub host: String,
    pub idle_timeout: Duration,
    pub poll_time_idle: Duration,
    pub poll_time_window: Duration,
    pub idle_bucket_name: String,
    pub active_window_bucket_name: String,
    filters: Vec<Filter>,
}

impl Config {
    pub fn from_cli() -> Result<Self, BoxedError> {
        let matches = Command::new("Activity Watcher")
            .version("0.1.0")
            .about("A set of ActivityWatch desktop watchers")
            .args([
                arg!(-c --config <FILE> "Custom config file").value_parser(value_parser!(PathBuf)),
                arg!(--port <PORT> "Custom server port")
                    .value_parser(value_parser!(u32))
                    .default_value(defaults::port().to_string()),
                arg!(--host <HOST> "Custom server host")
                    .value_parser(value_parser!(String))
                    .default_value(defaults::host()),
                arg!(--"idle-timeout" <SECONDS> "Time of inactivity to consider the user idle")
                    .value_parser(value_parser!(u32))
                    .default_value(defaults::idle_timeout_seconds().to_string()),
                arg!(--"poll-time-idle" <SECONDS> "Period between sending heartbeats to the server for idle activity")
                    .value_parser(value_parser!(u32))
                    .default_value(defaults::poll_time_idle_seconds().to_string()),
                arg!(--"poll-time-window" <SECONDS> "Period between sending heartbeats to the server for idle activity")
                    .value_parser(value_parser!(u32))
                    .default_value(defaults::poll_time_window_seconds().to_string()),
            ])
            .get_matches();

        let config = FileConfig::new(&matches)?;

        let hostname = gethostname::gethostname().into_string().unwrap();
        let idle_bucket_name = format!("aw-watcher-afk_{hostname}");
        let active_window_bucket_name = format!("aw-watcher-window_{hostname}");

        Ok(Self {
            port: config.server.port,
            host: config.server.host.clone(),
            idle_timeout: config.client.get_idle_timeout(),
            poll_time_idle: config.client.get_poll_time_idle(),
            poll_time_window: config.client.get_poll_time_window(),
            idle_bucket_name,
            active_window_bucket_name,
            filters: config.client.filters,
        })
    }

    pub fn window_data_replacement(&self, app_id: &str, title: &str) -> Replacement {
        for filter in &self.filters {
            if let Some(replacement) = filter.replacement(app_id, title) {
                return replacement;
            }
        }

        Replacement::default()
    }
}
