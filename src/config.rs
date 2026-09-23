use std::env;
use std::io;
use std::net::{IpAddr, ToSocketAddrs};
use std::path::Path;
use std::path::PathBuf;

use clap::parser::ValueSource;
use clap::{arg, value_parser, Arg, ArgAction, ArgMatches, Command};
use fern::colors::{Color, ColoredLevelConfig};
use log::LevelFilter;
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
            arg!(--"api-key" <APIKEY> "API key for the server")
                .value_parser(value_parser!(String))
                .env("AW_API_KEY")
                .default_value(None),
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

    let verbosity = match matches.get_count("verbosity") {
        0 => LevelFilter::Error,
        1 => LevelFilter::Warn,
        2 => LevelFilter::Info,
        3 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };
    setup_logger(verbosity)?;

    let config = new_with_cli(&matches)?;

    let api_key = resolve_api_key(matches.get_one("api-key").cloned(), &config);

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

fn is_local(host: &str) -> bool {
    let host = host.trim();
    let host = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host); // e.g. [::1]
    if let Ok(ip) = host.parse::<IpAddr>() {
        let ip = ip.to_canonical();
        return ip.is_loopback() || ip.is_unspecified();
    }

    match (host, 0u16).to_socket_addrs() {
        Ok(addrs) => {
            // None is empty array which isn't treated as local
            addrs
                .fold(None, |is_local: Option<bool>, addr| {
                    let ip = addr.ip().to_canonical();
                    Some(is_local.unwrap_or(true) && ip.is_loopback())
                })
                .unwrap_or(false)
        }
        Err(e) => {
            debug!("Could not resolve {host}: {e}; treating as remote");
            false
        }
    }
}

fn resolve_api_key(clap_api_key: Option<String>, config: &FileConfig) -> Option<String> {
    fn normalize(key: &str) -> Option<String> {
        let key = key.trim();
        (!key.is_empty()).then(|| key.to_owned())
    }

    if clap_api_key.is_some() {
        info!("Loaded API key from arguments or environment");
        return clap_api_key;
    }

    let file_api_key = config.server.api_key.as_deref().and_then(normalize);
    if file_api_key.is_some() {
        info!("Loaded API key from awatcher config");
        return file_api_key;
    }

    if !is_local(&config.server.host) {
        debug!("Server is not local, skipping aw-server-rust config");
        return None;
    }

    let config_path = dirs::config_dir()
        .or_else(|| {
            warn!("Config directory not found");
            None
        })?
        .join("activitywatch")
        .join("aw-server-rust")
        .join("config.toml");

    let content = std::fs::read_to_string(&config_path)
        .inspect_err(|e| {
            if e.kind() != io::ErrorKind::NotFound {
                warn!("Failed to read {}: {e}", config_path.display());
            }
        })
        .ok()?;
    let aw_config: AwConfig = toml::from_str(&content)
        .inspect_err(|_| {
            warn!("Failed to parse TOML at {}", config_path.display());
        })
        .ok()?;

    let server_api_key = aw_config.auth.api_key.as_deref().and_then(normalize);
    if server_api_key.is_some() {
        info!("Loaded API key from aw-server-rust config");
        return server_api_key;
    }

    debug!("No API key found in the local aw-server-rust configuration");
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::path::Path;
    use std::sync::Mutex;
    use tempfile::tempdir;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvGuard {
        key: &'static str,
        previous: Option<std::ffi::OsString>,
        _env_lock: std::sync::MutexGuard<'static, ()>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &std::ffi::OsStr) -> Self {
            let env_lock = ENV_LOCK.lock().unwrap();
            let previous = std::env::var_os(key);
            // Safe because tests serialize environment mutation with ENV_LOCK.
            std::env::set_var(key, value);
            Self {
                key,
                previous,
                _env_lock: env_lock,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // Safe because tests serialize environment mutation with ENV_LOCK.
            match &self.previous {
                Some(val) => std::env::set_var(self.key, val),
                None => std::env::remove_var(self.key),
            }
        }
    }

    fn write_aw_server_config(config_root: &Path, contents: &str) {
        let dir = config_root.join("activitywatch").join("aw-server-rust");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("config.toml"), contents).unwrap();
    }

    fn with_xdg_config(server_config: Option<&str>) -> (tempfile::TempDir, EnvGuard) {
        let temp_dir = tempdir().unwrap();
        let guard = EnvGuard::set("XDG_CONFIG_HOME", temp_dir.path().as_os_str());
        if let Some(contents) = server_config {
            write_aw_server_config(temp_dir.path(), contents);
        }
        (temp_dir, guard)
    }

    fn local_config() -> FileConfig {
        let mut config = FileConfig::default();
        config.server.host = "127.0.0.1".to_string();
        config.server.api_key = None;
        config
    }

    #[rstest]
    #[case("127.0.0.1", true)]
    #[case("127.0.0.50", true)]
    #[case("0.0.0.0", true)]
    #[case("::1", true)]
    #[case("::", true)]
    #[case("[::1]", true)]
    #[case("[::]", true)]
    #[case("::ffff:127.0.0.1", true)]
    #[case("::ffff:0.0.0.0", true)]
    #[case("[::ffff:127.0.0.1]", true)]
    #[case("localhost", true)]
    #[case("foo.localhost", true)]
    #[case("  127.0.0.1  ", true)]
    #[case("  localhost  ", true)]
    #[case("localhost.foo", false)]
    #[case("192.168.1.1", false)]
    #[case("10.0.0.1", false)]
    #[case("8.8.8.8", false)]
    #[case("example.com", false)]
    #[case("this-host-definitely-does-not-exist.invalid", false)]
    #[case("", false)]
    #[case("   ", false)]
    #[case("not an ip", false)]
    fn matches_is_local(#[case] host: &str, #[case] expected: bool) {
        assert_eq!(is_local(host), expected, "host: {host}");
    }

    #[rstest]
    fn resolve_api_key_cli_takes_precedence() {
        // CLI wins over file config and server config simultaneously
        let (_t, _g) = with_xdg_config(Some("[auth]\napi_key = \"server-key\"\n"));
        let mut config = local_config();
        config.server.api_key = Some("file-key".to_string());
        let result = resolve_api_key(Some("cli-key".to_string()), &config);
        assert_eq!(result, Some("cli-key".to_string()));
    }

    #[rstest]
    fn resolve_api_key_file_overrides_server_config() {
        let (_t, _g) = with_xdg_config(Some("[auth]\napi_key = \"server-key\"\n"));
        let mut config = local_config();
        config.server.api_key = Some("file-key".to_string());
        let result = resolve_api_key(None, &config);
        assert_eq!(result, Some("file-key".to_string()));
    }

    #[rstest]
    #[case("file-key", Some("file-key".to_string()))]
    #[case("  file-key  ", Some("file-key".to_string()))]
    #[case("", Some("server-key".to_string()))]
    #[case("   ", Some("server-key".to_string()))]
    fn resolve_api_key_file_normalization(
        #[case] file_key: &str,
        #[case] expected: Option<String>,
    ) {
        let (_t, _g) = with_xdg_config(Some("[auth]\napi_key = \"server-key\"\n"));
        let mut config = local_config();
        config.server.api_key = Some(file_key.to_string());
        let result = resolve_api_key(None, &config);
        // Valid file key wins; empty/whitespace falls through to server config
        assert_eq!(result, expected);
    }

    #[rstest]
    fn resolve_api_key_server_config_fallback() {
        let (_t, _g) = with_xdg_config(Some("[auth]\napi_key = \"  server-key  \"\n"));
        let result = resolve_api_key(None, &local_config());
        assert_eq!(result, Some("server-key".to_string()));
    }

    #[rstest]
    fn resolve_api_key_skips_server_config_for_remote_host() {
        let (_t, _g) = with_xdg_config(Some("[auth]\napi_key = \"server-key\"\n"));
        let mut config = local_config();
        config.server.host = "example.com".to_string();
        let result = resolve_api_key(None, &config);
        assert_eq!(result, None);
    }

    #[rstest]
    #[case("# no auth section\n", None)]
    #[case("not = valid = toml\n", None)]
    fn resolve_api_key_server_config_errors(
        #[case] contents: &str,
        #[case] expected: Option<String>,
    ) {
        let (_t, _g) = with_xdg_config(Some(contents));
        let result = resolve_api_key(None, &local_config());
        assert_eq!(result, expected);
    }

    #[rstest]
    fn resolve_api_key_no_sources_returns_none() {
        let (_t, _g) = with_xdg_config(None);
        let result = resolve_api_key(None, &local_config());
        assert_eq!(result, None);
    }

    #[rstest]
    fn resolve_api_key_empty_server_key_returns_none() {
        let (_t, _g) = with_xdg_config(Some("[auth]\napi_key = \"\"\n"));
        let result = resolve_api_key(None, &local_config());
        assert_eq!(result, None);
    }
}
