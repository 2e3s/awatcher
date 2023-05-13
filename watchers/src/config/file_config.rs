use anyhow::{anyhow, Context};
use serde::Deserialize;
use serde_default::DefaultFromSerde;
use std::{io::ErrorKind, path::PathBuf, time::Duration};

use crate::config::defaults;

use super::filters::Filter;

pub fn default_config() -> String {
    format!(
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

# Use captures for app-id or title in the regular form to use parts of the original text
# (parentheses for a capture, $1, $2 etc for each capture).
# The example rule removes the changed file indicator from the title in Visual Studio Code:
# "● file_config.rs - awatcher - Visual Studio Code" to "file_config.rs - awatcher - Visual Studio Code".
# [[awatcher.filters]]
# match-app-id = "code"
# match-title = "● (.*)"
# replace-title = "$1"
"#,
        defaults::port(),
        defaults::host(),
        defaults::idle_timeout_seconds(),
        defaults::poll_time_idle_seconds(),
        defaults::poll_time_window_seconds(),
    )
}

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
    pub idle_timeout_seconds: u32,
    #[serde(default = "defaults::poll_time_idle_seconds")]
    pub poll_time_idle_seconds: u32,
    #[serde(default = "defaults::poll_time_window_seconds")]
    pub poll_time_window_seconds: u32,
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
    pub fn new(config_override: Option<PathBuf>) -> anyhow::Result<Self> {
        let mut config_path: PathBuf =
            dirs::config_dir().ok_or(anyhow!("Config directory is unknown"))?;
        config_path.push("awatcher");
        config_path.push("config.toml");
        if let Some(config_override) = config_override {
            config_path = config_override;
        }

        let config = if config_path.exists() {
            debug!("Reading config at {}", config_path.display());
            let config_content = std::fs::read_to_string(&config_path).with_context(|| {
                format!("Impossible to read config file {}", config_path.display())
            })?;

            toml::from_str(&config_content)?
        } else {
            let config = default_config();
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

        Ok(config)
    }
}
