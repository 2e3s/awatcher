#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

#[macro_use]
extern crate log;

#[cfg(feature = "bundle")]
mod bundle;
mod config;

use std::error::Error;
use std::sync::Arc;
use tokio::signal::unix::{signal, SignalKind};
#[cfg(feature = "bundle")]
use tokio::sync::mpsc;
use watchers::{run_first_supported, ReportClient, WatcherType};

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<(), Box<dyn Error>> {
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
    #[cfg(feature = "bundle")]
    let (shutdown_send, mut shutdown_recv) = mpsc::unbounded_channel();
    #[cfg(feature = "bundle")]
    let bundle_handle = tokio::spawn(bundle::run(
        config.host.clone(),
        config.port,
        config_file,
        no_tray,
        shutdown_send,
    ));

    let client = Arc::new(ReportClient::new(config).await?);

    let idle_future = run_first_supported(Arc::clone(&client), &WatcherType::Idle);
    let active_window_future = run_first_supported(Arc::clone(&client), &WatcherType::ActiveWindow);
    let sigterm = async {
        signal(SignalKind::terminate()).unwrap().recv().await;
        warn!("Caught SIGTERM, shutting down...");
    };
    let sigint = async {
        signal(SignalKind::interrupt()).unwrap().recv().await;
        warn!("Caught SIGINT, shutting down...");
    };

    #[cfg(not(feature = "bundle"))]
    {
        tokio::select!(
            _ = tokio::spawn(idle_future) => Ok(()),
            _ = tokio::spawn(active_window_future) => Ok(()),
            () = sigterm => Ok(()),
            () = sigint => Ok(()),
        )
    }

    #[cfg(feature = "bundle")]
    {
        tokio::select!(
            _ = bundle_handle => Ok(()),
            _ = tokio::spawn(idle_future) => Ok(()),
            _ = tokio::spawn(active_window_future) => Ok(()),
            () = sigterm => Ok(()),
            () = sigint => Ok(()),
            _ = shutdown_recv.recv() => Ok(()),
        )
    }
}
