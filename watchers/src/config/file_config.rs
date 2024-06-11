use anyhow::{anyhow, Context};
use serde::Deserialize;
use serde_default::DefaultFromSerde;
use std::{fs, io::ErrorKind, path::PathBuf, time::Duration};

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
    pub port: u16,
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
    #[serde(default)]
    pub config_file: PathBuf,
}

impl FileConfig {
    pub fn new(config_override: Option<PathBuf>) -> anyhow::Result<Self> {
        let is_config_overridden = config_override.is_some();
        let config_path = if let Some(config_override) = config_override {
            if config_override.starts_with("~/") {
                dirs::home_dir()
                    .ok_or(anyhow!("Home directory is not found"))?
                    .join(config_override.strip_prefix("~").unwrap())
            } else {
                config_override
            }
        } else {
            let mut system_config_path: PathBuf =
                dirs::config_dir().ok_or(anyhow!("Config directory is unknown"))?;
            system_config_path.push("awatcher");
            system_config_path.push("config.toml");

            system_config_path
        };

        let mut config = if fs::metadata(&config_path).is_ok() {
            debug!("Reading config at {}", config_path.display());
            let config_content = std::fs::read_to_string(&config_path).with_context(|| {
                format!("Impossible to read config file {}", config_path.display())
            })?;

            toml::from_str(&config_content)?
        } else {
            if is_config_overridden {
                anyhow::bail!("Config file is not accessible at {}", config_path.display());
            }
            let config = default_config();
            let error = std::fs::create_dir(config_path.parent().unwrap());
            if let Err(e) = error {
                if e.kind() != ErrorKind::AlreadyExists {
                    Err(e)?;
                }
            }
            debug!("Creading config at {}", config_path.display());
            std::fs::write(&config_path, config)?;

            Self::default()
        };
        config.config_file = config_path;

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[rstest]
    fn all() {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            r#"
[server]
port = 1234
host = "http://address.com"

[awatcher]
idle-timeout-seconds=14
poll-time-idle-seconds=13
poll-time-window-seconds=12

# Add as many filters as needed.
# There should be at least 1 match field, and at least 1 replace field.
[[awatcher.filters]]
match-app-id = "firefox"
replace-title = "Unknown"

[[awatcher.filters]]
match-app-id = "code"
match-title = "title"
replace-app-id = "VSCode"
replace-title = "Title"
        "#
        )
        .unwrap();

        let config = FileConfig::new(Some(file.path().to_path_buf())).unwrap();

        assert_eq!(1234, config.server.port);
        assert_eq!("http://address.com", config.server.host);

        assert_eq!(14, config.client.idle_timeout_seconds);
        assert_eq!(13, config.client.poll_time_idle_seconds);
        assert_eq!(12, config.client.poll_time_window_seconds);

        assert_eq!(2, config.client.filters.len());
        let replacement1 = config.client.filters[0]
            .replacement("firefox", "any")
            .unwrap();
        assert_eq!(None, replacement1.replace_app_id);
        assert_eq!(Some("Unknown".to_string()), replacement1.replace_title);
        let replacement2 = config.client.filters[1]
            .replacement("code", "title")
            .unwrap();
        assert_eq!(Some("VSCode".to_string()), replacement2.replace_app_id);
        assert_eq!(Some("Title".to_string()), replacement2.replace_title);
    }

    #[rstest]
    fn empty() {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "[awatcher]").unwrap();

        let config = FileConfig::new(Some(file.path().to_path_buf())).unwrap();

        assert_eq!(defaults::port(), config.server.port);
        assert_eq!(defaults::host(), config.server.host);

        assert_eq!(
            defaults::idle_timeout_seconds(),
            config.client.idle_timeout_seconds
        );
        assert_eq!(
            defaults::poll_time_idle_seconds(),
            config.client.poll_time_idle_seconds
        );
        assert_eq!(
            defaults::poll_time_window_seconds(),
            config.client.poll_time_window_seconds
        );

        assert_eq!(0, config.client.filters.len());
    }

    #[rstest]
    fn wrong_file() {
        let file = PathBuf::new();

        let config = FileConfig::new(Some(file));

        assert!(config.is_err());
        assert_eq!(
            "Config file is not accessible at ",
            config.err().unwrap().to_string()
        );
    }
}
