#![warn(clippy::pedantic)]

#[macro_use]
extern crate log;

mod config;
mod report_client;
mod watchers;

use config::Config;
use fern::colors::{Color, ColoredLevelConfig};
use report_client::ReportClient;
use std::env;
use std::{error::Error, str::FromStr, sync::Arc, thread};

use crate::watchers::ConstructorFilter;

type BoxedError = Box<dyn Error>;

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

fn main() -> Result<(), BoxedError> {
    setup_logger()?;

    let client = ReportClient::new(Config::from_cli()?)?;
    let client = Arc::new(client);

    if client.config.mock_server {
        warn!(
            "Not sending to server {}:{}",
            client.config.host, client.config.port
        );
    } else {
        info!(
            "Sending to server {}:{}",
            client.config.host, client.config.port
        );
    }
    info!(
        "Idle timeout: {} seconds",
        client.config.idle_timeout.as_secs()
    );
    info!(
        "Idle polling period: {} seconds",
        client.config.poll_time_idle.as_secs()
    );
    info!(
        "Window polling period: {} seconds",
        client.config.poll_time_idle.as_secs()
    );

    let mut thread_handlers = Vec::new();

    let idle_watcher = watchers::IDLE.filter_first_supported();
    if let Some(mut watcher) = idle_watcher {
        let thread_client = Arc::clone(&client);
        let idle_handler = thread::spawn(move || watcher.watch(&thread_client));
        thread_handlers.push(idle_handler);
    } else {
        warn!("No supported idle handler is found");
    }

    let window_watcher = watchers::ACTIVE_WINDOW.filter_first_supported();
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
    Ok(())
}
