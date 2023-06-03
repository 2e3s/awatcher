#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

#[macro_use]
extern crate log;

#[cfg(feature = "bundle")]
mod bundle;
mod config;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use watchers::ConstructorFilter;
use watchers::ReportClient;

fn main() -> anyhow::Result<()> {
    let is_stopped = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&is_stopped))?;
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&is_stopped))?;

    let config = config::from_cli()?;
    #[cfg(feature = "bundle")]
    let no_tray = config.no_tray;
    let config = config.watchers_config;

    if config.no_server {
        warn!("Not sending to server {}:{}", config.host, config.port);
    } else {
        info!("Sending to server {}:{}", config.host, config.port);
    }
    info!("Idle timeout: {} seconds", config.idle_timeout.as_secs());
    info!(
        "Idle polling period: {} seconds",
        config.poll_time_idle.as_secs()
    );
    info!(
        "Window polling period: {} seconds",
        config.poll_time_window.as_secs()
    );

    #[cfg(feature = "bundle")]
    bundle::run(&config, no_tray);

    let client = ReportClient::new(config)?;
    let client = Arc::new(client);

    let mut thread_handlers = Vec::new();

    if let Some(idle_handler) = watchers::IDLE.run_first_supported(&client, Arc::clone(&is_stopped))
    {
        thread_handlers.push(idle_handler);
    } else {
        warn!("No supported idle handler is found");
    }

    if let Some(active_window_handler) =
        watchers::ACTIVE_WINDOW.run_first_supported(&client, is_stopped)
    {
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
