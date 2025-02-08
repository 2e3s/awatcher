#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

#[macro_use]
extern crate log;

#[cfg(feature = "bundle")]
mod bundle;
mod config;

use std::pin::Pin;
use std::sync::Arc;
use std::{error::Error, future::Future};
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
    let disable_idle_watcher = config.disable_idle_watcher;
    let disable_window_watcher = config.disable_window_watcher;
    let config = config.watchers_config;

    #[cfg(not(feature = "bundle"))]
    if disable_idle_watcher && disable_window_watcher {
        error!("Both watchers are disabled");
        return Err("At least one watcher must be enabled".into());
    }
    #[cfg(feature = "bundle")]
    if disable_idle_watcher && disable_window_watcher {
        warn!("Both watchers are disabled");
    }

    if config.no_server {
        warn!(
            "Not sending to server {}:{}",
            config.client_host(),
            config.port
        );
    } else {
        info!("Sending to server {}:{}", config.client_host(), config.port);
    }
    info!(
        "Idle timeout: {} seconds",
        config.idle_timeout.num_seconds()
    );
    info!(
        "Idle polling period: {} seconds",
        config.poll_time_idle.num_seconds()
    );
    info!(
        "Window polling period: {} seconds",
        config.poll_time_window.num_seconds()
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

    let idle_future: Pin<Box<dyn Future<Output = bool> + Send>> = if disable_idle_watcher {
        Box::pin(std::future::pending())
    } else {
        Box::pin(run_first_supported(Arc::clone(&client), &WatcherType::Idle))
    };

    let active_window_future: Pin<Box<dyn Future<Output = bool> + Send>> = if disable_window_watcher
    {
        Box::pin(std::future::pending::<bool>())
    } else {
        Box::pin(run_first_supported(
            Arc::clone(&client),
            &WatcherType::ActiveWindow,
        ))
    };

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
