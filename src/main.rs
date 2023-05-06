#![warn(clippy::pedantic)]

#[macro_use]
extern crate log;

mod config;

use std::sync::Arc;
use watchers::ConstructorFilter;
use watchers::ReportClient;

fn main() -> anyhow::Result<()> {
    let config = config::from_cli()?;
    config::setup_logger(config.verbosity)?;

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

    if let Some(idle_handler) = watchers::IDLE.run_first_supported(&client) {
        thread_handlers.push(idle_handler);
    } else {
        warn!("No supported idle handler is found");
    }

    if let Some(active_window_handler) = watchers::ACTIVE_WINDOW.run_first_supported(&client) {
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
