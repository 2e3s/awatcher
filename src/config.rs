use std::path::Path;
use std::path::PathBuf;

use clap::parser::ValueSource;
use clap::{arg, value_parser, Arg, ArgAction, ArgMatches, Command};
use fern::colors::{Color, ColoredLevelConfig};
use log::{info, warn, LevelFilter};
use serde::Deserialize;
use watchers::config::defaults;
use watchers::config::Config;
use watchers::config::FileConfig;

#[derive(Deserialize, Default)]
struct AwAuthConfig {
    #[serde(default)]
    api_key: Option<String>,
}

#[derive(Deserialize, Default)]
struct AwConfig {
    #[serde(default)]
    auth: AwAuthConfig,
}

/// Reads only the `[auth] api_key` field from the aw-server-rust config.toml.
/// Returns None if the file doesn't exist, cannot be read, or has no key set.
fn read_aw_server_api_key() -> Option<String> {
    let config_path = dirs::config_dir()?
        .join("activitywatch")
        .join("aw-server-rust")
        .join("config.toml");

    let content = std::fs::read_to_string(&config_path).ok()?;
    let config: AwConfig = toml::from_str(&content).ok()?;
    config.auth.api_key
}

pub struct RunnerConfig {
    pub watchers_config: Config,
    #[cfg(feature = "bundle")]
    pub config_file: PathBuf,
    #[cfg(feature = "bundle")]
    pub no_tray: bool,
}

pub fn setup_logger(verbosity: LevelFilter) -> Result<(), fern::InitError> {
    fern::Dispatch::new()
        .format(|out, message, record| {
            let colors = ColoredLevelConfig::new()
                .info(Color::Green)
                .debug(Color::Blue)
                .trace(Color::Cyan);
            out.finish(format_args!(
                "[{} {} {}] {}",
                chrono::Utc::now().format("%Y-%m-%d %H:%M:%S%.6f"),
                colors.color(record.level()),
                record.target(),
                message
            ));
        })
        .level(log::LevelFilter::Warn)
        .level_for("watchers", verbosity)
        .level_for("awatcher", verbosity)
        .chain(std::io::stdout())
        .apply()?;
    Ok(())
}

pub fn from_cli() -> anyhow::Result<RunnerConfig> {
    let matches = Command::new("Activity Watcher")
        .version(env!("CARGO_PKG_VERSION"))
        .about(
            #[cfg(not(feature = "bundle"))]
            "X11 and Wayland active window and idle watcher for ActivityWatch server",
            #[cfg(feature = "bundle")]
            "X11 and Wayland active window and idle watcher with a bundled ActivityWatch server",
        )
        .args([
            arg!(-c --config <FILE> "Custom config file").value_parser(value_parser!(PathBuf)),
            arg!(--port <PORT> "Custom server port")
                .value_parser(value_parser!(u16))
                .default_value(defaults::port().to_string()),
            #[cfg(not(feature = "bundle"))]
            arg!(--host <HOST> "Custom server host")
                .value_parser(value_parser!(String))
                .default_value(defaults::host()),
            arg!(--"idle-timeout" <SECONDS> "Time of inactivity to consider the user idle")
                .value_parser(value_parser!(u32))
                .default_value(defaults::idle_timeout_seconds().to_string()),
            arg!(--"poll-time-idle" <SECONDS> "Period between sending heartbeats to the server for idle activity")
                .value_parser(value_parser!(u32))
                .default_value(defaults::poll_time_idle_seconds().to_string()),
            arg!(--"poll-time-window" <SECONDS> "Period between sending heartbeats to the server for window activity")
                .value_parser(value_parser!(u32))
                .default_value(defaults::poll_time_window_seconds().to_string()),
            arg!(--"no-server" "Don't send data to the ActivityWatch server")
                .value_parser(value_parser!(bool))
                .action(ArgAction::SetTrue),
            #[cfg(feature = "bundle")]
            arg!(--"no-tray" "Don't use the bundled tray, run only server and watchers in the background")
                .value_parser(value_parser!(bool))
                .action(ArgAction::SetTrue),
            Arg::new("verbosity")
                .short('v')
                .help("Verbosity level: -v for warnings, -vv for info, -vvv for debug, -vvvv for trace")
                .action(ArgAction::Count),
        ])
        .get_matches();

    let config = new_with_cli(&matches)?;

    let verbosity = match matches.get_count("verbosity") {
        0 => LevelFilter::Error,
        1 => LevelFilter::Warn,
        2 => LevelFilter::Info,
        3 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };
    setup_logger(verbosity)?;

    let is_local = ["localhost", "127.0.0.1", "::1"].contains(&config.server.host.as_str());

    let api_key = config
        .server
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .map(|k| {
            info!("Loaded API key from awatcher config");
            k.to_string()
        })
        .or_else(|| {
            if is_local {
                let key = read_aw_server_api_key();
                match &key {
                    Some(_) => info!("Loaded API key from aw-server-rust config"),
                    None => warn!(
                        "No API key found in awatcher or aw-server-rust config, proceeding unauthenticated"
                    ),
                }
                key
            } else {
                warn!("No API key found in awatcher config and host is not local, proceeding unauthenticated");
                None
            }
        });

    Ok(RunnerConfig {
        watchers_config: Config {
            port: config.server.port,
            host: config.server.host,
            api_key,
            idle_timeout: config.client.get_idle_timeout(),
            poll_time_idle: config.client.get_poll_time_idle(),
            poll_time_window: config.client.get_poll_time_window(),
            filters: config.client.filters,
            no_server: *matches.get_one("no-server").unwrap(),
        },
        #[cfg(feature = "bundle")]
        config_file: config.config_file,
        #[cfg(feature = "bundle")]
        no_tray: *matches.get_one("no-tray").unwrap(),
    })
}

pub fn new_with_cli(matches: &ArgMatches) -> anyhow::Result<FileConfig> {
    let mut config_path = None;
    if matches.contains_id("config") {
        let config_file = matches.get_one::<PathBuf>("config");
        if let Some(path) = config_file {
            config_path = Some(Path::new(path).to_path_buf());
        }
    }
    let mut config = FileConfig::new(config_path)?;

    merge_cli(&mut config, matches);

    Ok(config)
}

fn merge_cli(config: &mut FileConfig, matches: &ArgMatches) {
    get_arg_value(
        "poll-time-idle",
        matches,
        &mut config.client.poll_time_idle_seconds,
    );
    get_arg_value(
        "poll-time-window",
        matches,
        &mut config.client.poll_time_window_seconds,
    );
    get_arg_value(
        "idle-timeout",
        matches,
        &mut config.client.idle_timeout_seconds,
    );
    get_arg_value("port", matches, &mut config.server.port);
    #[cfg(not(feature = "bundle"))]
    get_arg_value("host", matches, &mut config.server.host);
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
