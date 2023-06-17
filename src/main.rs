#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

#[macro_use]
extern crate log;

#[cfg(feature = "bundle")]
mod bundle;
mod config;

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use watchers::run_first_supported;
use watchers::ReportClient;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let is_stopped = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&is_stopped))?;
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&is_stopped))?;

    let config = config::from_cli()?;
    #[cfg(feature = "bundle")]
    let no_tray = config.no_tray;
    #[cfg(feature = "bundle")]
    let config_file = config.config_file;
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

    let client = ReportClient::new(config)?;
    let client = Arc::new(client);

    let idle_handler = run_first_supported(watchers::IDLE, &client, Arc::clone(&is_stopped));
    let active_window_handler =
        run_first_supported(watchers::ACTIVE_WINDOW, &client, Arc::clone(&is_stopped));

    #[cfg(not(feature = "bundle"))]
    {
        tokio::select!(
            _ = idle_handler => Ok(()),
            _ = active_window_handler => Ok(()),
        )
    }

    #[cfg(feature = "bundle")]
    {
        tokio::select!(
            _ = idle_handler => Ok(()),
            _ = active_window_handler => Ok(()),
            _ = bundle::run(client.config.host.clone(), client.config.port, config_file, no_tray, Arc::clone(&is_stopped)) => Ok(()),
        )
    }
}
