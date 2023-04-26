#![warn(clippy::pedantic)]

#[macro_use]
extern crate log;

mod config;
mod report_client;
mod watchers;

use config::Config;
use fern::colors::{Color, ColoredLevelConfig};
use log::LevelFilter;
use report_client::ReportClient;
use std::{sync::Arc, thread};
use watchers::ConstructorFilter;

fn setup_logger(verbosity: LevelFilter) -> Result<(), fern::InitError> {
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
        .level_for("awatcher", verbosity)
        .chain(std::io::stdout())
        .apply()?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let config = Config::from_cli()?;
    setup_logger(config.verbosity)?;

    let client = ReportClient::new(config)?;
    let client = Arc::new(client);

    if client.config.no_server {
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
        client.config.poll_time_window.as_secs()
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
