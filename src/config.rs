use clap::{arg, parser::ValueSource, value_parser, ArgMatches, Command};
use serde::Deserialize;
use serde_default::DefaultFromSerde;
use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
    time::Duration,
};

use crate::BoxedError;

pub struct Config {
    pub port: u32,
    pub host: String,
    pub idle_timeout: Duration,
    pub poll_time_idle: Duration,
    pub poll_time_window: Duration,
    pub idle_bucket_name: String,
    pub active_window_bucket_name: String,
}

fn default_idle_timeout_seconds() -> u32 {
    180
}
fn default_poll_time_idle_seconds() -> u32 {
    5
}
fn default_poll_time_window_seconds() -> u32 {
    1
}
fn default_port() -> u32 {
    5600
}
fn default_host() -> String {
    "localhost".to_string()
}

#[derive(Deserialize, DefaultFromSerde)]
struct ServerConfig {
    #[serde(default = "default_port")]
    port: u32,
    #[serde(default = "default_host")]
    host: String,
}

#[derive(Deserialize, DefaultFromSerde)]
struct ClientConfig {
    #[serde(default = "default_idle_timeout_seconds")]
    idle_timeout_seconds: u32,
    #[serde(default = "default_poll_time_idle_seconds")]
    poll_time_idle_seconds: u32,
    #[serde(default = "default_poll_time_window_seconds")]
    poll_time_window_seconds: u32,
}

#[derive(Deserialize, Default)]
struct FileConfig {
    #[serde(default)]
    server: ServerConfig,
    #[serde(default)]
    client: ClientConfig,
}

impl FileConfig {
    fn new(config_path: &Path) -> Result<Self, BoxedError> {
        if config_path.exists() {
            debug!("Reading config at {}", config_path.display());
            let config_content = std::fs::read_to_string(config_path)
                .map_err(|e| format!("Impossible to read config file: {e}"))?;

            Ok(toml::from_str(&config_content)?)
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
    "#,
                default_port(),
                default_host(),
                default_idle_timeout_seconds(),
                default_poll_time_idle_seconds(),
                default_poll_time_window_seconds(),
            );
            let error = std::fs::create_dir(config_path.parent().unwrap());
            if let Err(e) = error {
                if e.kind() != ErrorKind::AlreadyExists {
                    Err(e)?;
                }
            }
            debug!("Creading config at {}", config_path.display());
            std::fs::write(config_path, config)?;

            Ok(Self::default())
        }
    }

    fn merge_cli(&mut self, matches: &ArgMatches) {
        self.client.poll_time_idle_seconds = get_arg_value(
            "poll-time-idle",
            matches,
            self.client.poll_time_idle_seconds,
        );
        self.client.poll_time_window_seconds = get_arg_value(
            "poll-time-window",
            matches,
            self.client.poll_time_window_seconds,
        );
        self.client.idle_timeout_seconds =
            get_arg_value("idle-timeout", matches, self.client.idle_timeout_seconds);

        self.server.port = get_arg_value("port", matches, self.server.port);
        self.server.host = get_arg_value("host", matches, self.server.host.clone());
    }

    fn get_idle_timeout(&self) -> Duration {
        Duration::from_secs(u64::from(self.client.idle_timeout_seconds))
    }

    fn get_poll_time_idle(&self) -> Duration {
        Duration::from_secs(u64::from(self.client.poll_time_idle_seconds))
    }

    fn get_poll_time_window(&self) -> Duration {
        Duration::from_secs(u64::from(self.client.poll_time_window_seconds))
    }
}

fn get_arg_value<T>(id: &str, matches: &ArgMatches, config_value: T) -> T
where
    T: Clone + Send + Sync + 'static,
{
    if let Some(ValueSource::CommandLine) = matches.value_source(id) {
        matches.get_one::<T>(id).unwrap().clone()
    } else {
        config_value
    }
}

impl Config {
    pub fn from_cli() -> Result<Self, BoxedError> {
        let mut config_path: PathBuf = dirs::config_dir().ok_or("Config directory is unknown")?;
        config_path.push("awatcher");
        config_path.push("config.toml");

        let matches = Command::new("Activity Watcher")
            .version("0.1.0")
            .about("A set of ActivityWatch desktop watchers")
            .args([
                arg!(-c --config <FILE> "Custom config file").value_parser(value_parser!(PathBuf)),
                arg!(--port <PORT> "Custom server port")
                    .value_parser(value_parser!(u32))
                    .default_value(default_port().to_string()),
                arg!(--host <HOST> "Custom server host")
                    .value_parser(value_parser!(String))
                    .default_value(default_host()),
                arg!(--"idle-timeout" <SECONDS> "Time of inactivity to consider the user idle")
                    .value_parser(value_parser!(u32))
                    .default_value(default_idle_timeout_seconds().to_string()),
                arg!(--"poll-time-idle" <SECONDS> "Period between sending heartbeats to the server for idle activity")
                    .value_parser(value_parser!(u32))
                    .default_value(default_poll_time_idle_seconds().to_string()),
                arg!(--"poll-time-window" <SECONDS> "Period between sending heartbeats to the server for idle activity")
                    .value_parser(value_parser!(u32))
                    .default_value(default_poll_time_window_seconds().to_string()),
            ])
            .get_matches();

        let mut config = FileConfig::new(config_path.as_path())?;
        config.merge_cli(&matches);

        let hostname = gethostname::gethostname().into_string().unwrap();
        let idle_bucket_name = format!("aw-watcher-afk_{hostname}");
        let active_window_bucket_name = format!("aw-watcher-window_{hostname}");

        Ok(Self {
            port: config.server.port,
            host: config.server.host.clone(),
            idle_timeout: config.get_idle_timeout(),
            poll_time_idle: config.get_poll_time_idle(),
            poll_time_window: config.get_poll_time_window(),
            idle_bucket_name,
            active_window_bucket_name,
        })
    }
}
