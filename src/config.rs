use std::path::Path;
use std::path::PathBuf;

use clap::parser::ValueSource;
use clap::{arg, value_parser, Arg, ArgAction, ArgMatches, Command};
use fern::colors::{Color, ColoredLevelConfig};
use log::LevelFilter;
use watchers::config::defaults;
use watchers::config::Config as WatchersConfig;
use watchers::config::FileConfig;

pub struct Config {
    pub watchers_config: WatchersConfig,
    verbosity: LevelFilter,
}

pub fn setup_logger(config: &Config) -> Result<(), fern::InitError> {
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
        .level(log::LevelFilter::Error)
        .level_for("watchers", config.verbosity)
        .level_for("awatcher", config.verbosity)
        .chain(std::io::stdout())
        .apply()?;
    Ok(())
}

pub fn from_cli() -> anyhow::Result<Config> {
    let matches = Command::new("Activity Watcher")
        .version("0.0.1")
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
            arg!(--"no-server" "Don't communicate to the ActivityWatch server")
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

    Ok(Config {
        watchers_config: WatchersConfig {
            port: config.server.port,
            host: config.server.host,
            idle_timeout: config.client.get_idle_timeout(),
            poll_time_idle: config.client.get_poll_time_idle(),
            poll_time_window: config.client.get_poll_time_window(),
            filters: config.client.filters,
            no_server: *matches.get_one("no-server").unwrap(),
        },
        verbosity,
    })
}

pub fn new_with_cli(matches: &ArgMatches) -> anyhow::Result<FileConfig> {
    let mut config_path = None;
    if matches.contains_id("config") {
        let config_file = matches.get_one::<String>("config");
        if let Some(path) = config_file {
            if let Err(e) = std::fs::metadata(path) {
                warn!("Invalid config filename, using the default config: {e}");
            } else {
                config_path = Some(Path::new(path).to_path_buf());
            }
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
