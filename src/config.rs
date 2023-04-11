use std::path::PathBuf;

use clap::{arg, value_parser, Command};

pub struct Config {
    pub port: u32,
    pub host: String,
    pub idle_timeout: u32,
    pub poll_time_idle: u32,
    pub poll_time_window: u32,
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

        Self {
            port: *matches.get_one("port").unwrap(),
            host: String::clone(matches.get_one("host").unwrap()),
            idle_timeout: *matches.get_one("idle-timeout").unwrap(),
            poll_time_idle: *matches.get_one("poll-time-idle").unwrap(),
            poll_time_window: *matches.get_one("poll-time-window").unwrap(),
        }
    }
}
