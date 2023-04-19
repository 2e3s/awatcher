use std::{path::PathBuf, time::Duration};

use clap::{arg, value_parser, Command};

pub struct Config {
    pub port: u32,
    pub host: String,
    pub idle_timeout: Duration,
    pub poll_time_idle: Duration,
    pub poll_time_window: Duration,
    pub idle_bucket_name: String,
    pub active_window_bucket_name: String,
}

impl Config {
    pub fn from_cli() -> Self {
        let matches = Command::new("Activity Watcher")
            .version("0.1.0")
            .about("A set of ActivityWatch desktop watchers")
            .args([
                arg!(-c --config <FILE> "Custom config file").value_parser(value_parser!(PathBuf)),
                arg!(--port <PORT> "Custom server port")
                    .value_parser(value_parser!(u32))
                    .default_value("5600"),
                arg!(--host <HOST> "Custom server host")
                    .value_parser(value_parser!(String))
                    .default_value("localhost"),
                arg!(--"idle-timeout" <SECONDS> "Time of inactivity to consider the user idle")
                    .value_parser(value_parser!(u32))
                    .default_value("180"),
                arg!(--"poll-time-idle" <SECONDS> "Period between sending heartbeats to the server for idle activity")
                    .value_parser(value_parser!(u32))
                    .default_value("5"),
                arg!(--"poll-time-window" <SECONDS> "Period between sending heartbeats to the server for idle activity")
                    .value_parser(value_parser!(u32))
                    .default_value("1"),
            ])
            .get_matches();

        let hostname = gethostname::gethostname().into_string().unwrap();
        let idle_bucket_name = format!("aw-watcher-afk_{hostname}");
        let active_window_bucket_name = format!("aw-watcher-window_{hostname}");
        let poll_seconds_idle = *matches.get_one::<u32>("poll-time-idle").unwrap();
        let poll_seconds_window = *matches.get_one::<u32>("poll-time-window").unwrap();
        let idle_timeout_seconds = *matches.get_one::<u32>("idle-timeout").unwrap();

        Self {
            port: *matches.get_one("port").unwrap(),
            host: String::clone(matches.get_one("host").unwrap()),
            idle_timeout: Duration::from_secs(u64::from(idle_timeout_seconds)),
            poll_time_idle: Duration::from_secs(u64::from(poll_seconds_idle)),
            poll_time_window: Duration::from_secs(u64::from(poll_seconds_window)),
            idle_bucket_name,
            active_window_bucket_name,
        }
    }
}
