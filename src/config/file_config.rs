use clap::{parser::ValueSource, ArgMatches};
use serde::Deserialize;
use serde_default::DefaultFromSerde;
use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
    time::Duration,
};

use crate::{config::defaults, BoxedError};

use super::filters::Filter;

#[derive(Deserialize, DefaultFromSerde)]
pub struct ServerConfig {
    #[serde(default = "defaults::port")]
    pub port: u32,
    #[serde(default = "defaults::host")]
    pub host: String,
}

#[derive(Deserialize, DefaultFromSerde)]
#[serde(rename_all = "kebab-case")]
pub struct ClientConfig {
    #[serde(default = "defaults::idle_timeout_seconds")]
    idle_timeout_seconds: u32,
    #[serde(default = "defaults::poll_time_idle_seconds")]
    poll_time_idle_seconds: u32,
    #[serde(default = "defaults::poll_time_window_seconds")]
    poll_time_window_seconds: u32,
    #[serde(default)]
    pub filters: Vec<Filter>,
}

impl ClientConfig {
    pub fn get_idle_timeout(&self) -> Duration {
        Duration::from_secs(u64::from(self.idle_timeout_seconds))
    }

    pub fn get_poll_time_idle(&self) -> Duration {
        Duration::from_secs(u64::from(self.poll_time_idle_seconds))
    }

    pub fn get_poll_time_window(&self) -> Duration {
        Duration::from_secs(u64::from(self.poll_time_window_seconds))
    }
}

#[derive(Deserialize, Default)]
pub struct FileConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    #[serde(rename = "awatcher")]
    pub client: ClientConfig,
}

impl FileConfig {
    pub fn new(matches: &ArgMatches) -> Result<Self, BoxedError> {
        let mut config_path: PathBuf = dirs::config_dir().ok_or("Config directory is unknown")?;
        config_path.push("awatcher");
        config_path.push("config.toml");
        if matches.contains_id("config") {
            let config_file = matches.get_one::<String>("config");
            if let Some(path) = config_file {
                if let Err(e) = std::fs::metadata(path) {
                    warn!("Invalid config filename, using the default config: {e}");
                } else {
                    config_path = Path::new(path).to_path_buf();
                }
            }
        }

        let mut config = if config_path.exists() {
            debug!("Reading config at {}", config_path.display());
            let config_content = std::fs::read_to_string(config_path)
                .map_err(|e| format!("Impossible to read config file: {e}"))?;

            toml::from_str(&config_content)?
        } else {
            let config = format!(
                r#"# The commented values are the defaults on the file creation
[server]
# port = {}
# host = "{}"

[awatcher]
# idle-timeout-seconds={}
# poll-time-idle-seconds={}
# poll-time-window-seconds={}

# Add as many filters as needed. The first matching filter stops the replacement.
# There should be at least 1 match field, and at least 1 replace field.
# Matches are case sensitive regular expressions between implici ^ and $, e.g.
# - "." matches 1 any character
# - ".*" matches any number of any characters
# - ".+" matches 1 or more any characters.
# - "word" is an exact match.
# [[awatcher.filters]]
# match-app-id = "navigator"
# match-title = ".*Firefox.*"
# replace-app-id = "firefox"
# replace-title = "Unknown"
"#,
                defaults::port(),
                defaults::host(),
                defaults::idle_timeout_seconds(),
                defaults::poll_time_idle_seconds(),
                defaults::poll_time_window_seconds(),
            );
            let error = std::fs::create_dir(config_path.parent().unwrap());
            if let Err(e) = error {
                if e.kind() != ErrorKind::AlreadyExists {
                    Err(e)?;
                }
            }
            debug!("Creading config at {}", config_path.display());
            std::fs::write(config_path, config)?;

            Self::default()
        };
        config.merge_cli(matches);

        Ok(config)
    }

    fn merge_cli(&mut self, matches: &ArgMatches) {
        get_arg_value(
            "poll-time-idle",
            matches,
            &mut self.client.poll_time_idle_seconds,
        );
        get_arg_value(
            "poll-time-window",
            matches,
            &mut self.client.poll_time_window_seconds,
        );
        get_arg_value(
            "idle-timeout",
            matches,
            &mut self.client.idle_timeout_seconds,
        );
        get_arg_value("port", matches, &mut self.server.port);
        get_arg_value("host", matches, &mut self.server.host);
    }
}

fn get_arg_value<T>(id: &str, matches: &ArgMatches, config_value: &mut T)
where
    T: Clone + Send + Sync + 'static,
{
    if let Some(ValueSource::CommandLine) = matches.value_source(id) {
        let value = &mut matches.get_one::<T>(id).unwrap().clone();
        std::mem::swap(config_value, value);
    }
}
