#![warn(clippy::pedantic)]

#[macro_use]
extern crate log;

mod config;
mod report_client;
mod wl_bindings;
mod wl_connection;
mod wl_foreign_toplevel;
mod wl_kwin_idle;
mod wl_kwin_window;

use config::Config;
use fern::colors::{Color, ColoredLevelConfig};
use report_client::ReportClient;
use std::env;
use std::{error::Error, str::FromStr, sync::Arc, thread};
use wl_kwin_idle::KwinIdleWatcher;
use wl_kwin_window::KwinWindowWatcher;

use crate::wl_foreign_toplevel::WlrForeignToplevelWatcher;

type BoxedError = Box<dyn Error>;

trait Watcher: Send {
    fn new() -> Result<Self, BoxedError>
    where
        Self: Sized;
    fn watch(&mut self, client: &Arc<ReportClient>);
}

type BoxedWatcher = Box<dyn Watcher>;

type WatcherConstructor = (&'static str, fn() -> Result<BoxedWatcher, BoxedError>);
type WatcherConstructors = [WatcherConstructor];

trait WatchersFilter {
    fn filter_first_supported(&self) -> Option<BoxedWatcher>;
}

impl WatchersFilter for WatcherConstructors {
    fn filter_first_supported(&self) -> Option<BoxedWatcher> {
        self.iter().find_map(|(name, watcher)| match watcher() {
            Ok(watcher) => Some(watcher),
            Err(e) => {
                info!("{name} cannot run: {e}");
                None
            }
        })
    }
}

macro_rules! watcher {
    ($watcher_struct:ty) => {
        (stringify!($watcher_struct), || {
            Ok(Box::new(<$watcher_struct>::new()?))
        })
    };
}

const IDLE_WATCHERS: &[WatcherConstructor] = &[watcher!(KwinIdleWatcher)];

const ACTIVE_WINDOW_WATCHERS: &[WatcherConstructor] = &[
    watcher!(WlrForeignToplevelWatcher),
    watcher!(KwinWindowWatcher),
];

fn setup_logger() -> Result<(), fern::InitError> {
    let log_setting = env::var("AWATCHER_LOG").unwrap_or("info".to_string());

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
        .level_for(
            "awatcher",
            FromStr::from_str(&log_setting).unwrap_or(log::LevelFilter::Info),
        )
        .chain(std::io::stdout())
        .apply()?;
    Ok(())
}

fn main() {
    setup_logger().unwrap();

    let client = ReportClient::new(Config::from_cli());
    let client = Arc::new(client);

    info!(
        "Sending to server {}:{}",
        client.config.host, client.config.port
    );
    info!("Idle timeout: {} seconds", client.config.idle_timeout);
    info!("Polling period: {} seconds", client.config.poll_time_idle);

    let mut thread_handlers = Vec::new();

    let idle_watcher = IDLE_WATCHERS.filter_first_supported();
    if let Some(mut watcher) = idle_watcher {
        let thread_client = Arc::clone(&client);
        let idle_handler = thread::spawn(move || watcher.watch(&thread_client));
        thread_handlers.push(idle_handler);
    } else {
        warn!("No supported idle handler is found");
    }

    let window_watcher = ACTIVE_WINDOW_WATCHERS.filter_first_supported();
    if let Some(mut watcher) = window_watcher {
        let thread_client = Arc::clone(&client);
        let active_window_handler = thread::spawn(move || watcher.watch(&thread_client));
        thread_handlers.push(active_window_handler);
    } else {
        warn!("No supported active window handler is found");
    }

    for handler in thread_handlers {
        if handler.join().is_err() {
            error!("Thread failed with error");
        }
    }
}
