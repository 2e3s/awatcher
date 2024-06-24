#[cfg(feature = "gnome")]
mod gnome_idle;
#[cfg(feature = "gnome")]
mod gnome_wayland;
#[cfg(feature = "gnome")]
mod gnome_window;
mod idle;
#[cfg(feature = "kwin_window")]
mod kwin_window;
mod wl_connection;
mod wl_ext_idle_notify;
mod wl_foreign_toplevel_management;
mod wl_kwin_idle;
mod x11_connection;
mod x11_screensaver_idle;
mod x11_window;

use crate::{config::Config, report_client::ReportClient};
use async_trait::async_trait;
use std::{fmt::Display, sync::Arc};
use tokio::time::{sleep, timeout, Duration};

pub enum WatcherType {
    Idle,
    ActiveWindow,
}

impl WatcherType {
    fn sleep_time(&self, config: &Config) -> Duration {
        match self {
            WatcherType::Idle => config.poll_time_idle.to_std().unwrap(),
            WatcherType::ActiveWindow => config.poll_time_window.to_std().unwrap(),
        }
    }
}

impl Display for WatcherType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WatcherType::Idle => write!(f, "idle"),
            WatcherType::ActiveWindow => write!(f, "active window"),
        }
    }
}

#[async_trait]
pub trait Watcher: Send {
    async fn new(client: &Arc<ReportClient>) -> anyhow::Result<Self>
    where
        Self: Sized;

    async fn run_iteration(&mut self, client: &Arc<ReportClient>) -> anyhow::Result<()>;
}

async fn create_watcher<T: Watcher>(client: &Arc<ReportClient>, name: &str) -> Option<T> {
    match T::new(client).await {
        Ok(watcher) => {
            info!("Selected watcher: {name}");
            Some(watcher)
        }
        Err(e) => {
            debug!("Watcher \"{name}\" cannot run: {e}");
            None
        }
    }
}

macro_rules! watch {
    ($watcher:expr) => {
        if let Some(watcher) = $watcher.await {
            return Some(Box::new(watcher));
        }
    };
}

async fn filter_first_supported(
    client: &Arc<ReportClient>,
    watcher_type: &WatcherType,
) -> Option<Box<dyn Watcher>> {
    match watcher_type {
        WatcherType::Idle => {
            watch!(create_watcher::<wl_ext_idle_notify::IdleWatcher>(
                client,
                "Wayland idle (ext-idle-notify-v1)"
            ));
            watch!(create_watcher::<wl_kwin_idle::IdleWatcher>(
                client,
                "Wayland idle (KDE)"
            ));
            watch!(create_watcher::<x11_screensaver_idle::IdleWatcher>(
                client,
                "X11 idle (screensaver)"
            ));
            #[cfg(feature = "gnome")]
            watch!(create_watcher::<gnome_idle::IdleWatcher>(
                client,
                "Gnome idle (Mutter/IdleMonitor)"
            ));
        }
        WatcherType::ActiveWindow => {
            watch!(create_watcher::<
                wl_foreign_toplevel_management::WindowWatcher,
            >(
                client,
                "Wayland window (wlr-foreign-toplevel-management-unstable-v1)"
            ));
            // XWayland gives _NET_WM_NAME on some windows in KDE, but not on others
            #[cfg(feature = "kwin_window")]
            watch!(create_watcher::<kwin_window::WindowWatcher>(
                client,
                "KWin window (script)"
            ));
            watch!(create_watcher::<x11_window::WindowWatcher>(
                client,
                "X11 window"
            ));
            #[cfg(feature = "gnome")]
            watch!(create_watcher::<gnome_window::WindowWatcher>(
                client,
                "Gnome window (extension)"
            ));
        }
    };

    None
}

pub async fn run_first_supported(client: Arc<ReportClient>, watcher_type: &WatcherType) -> bool {
    let supported_watcher = filter_first_supported(&client, watcher_type).await;
    if let Some(mut watcher) = supported_watcher {
        info!("Starting {watcher_type} watcher");
        loop {
            let sleep_time = watcher_type.sleep_time(&client.config);

            match timeout(sleep_time, watcher.run_iteration(&client)).await {
                Ok(Ok(())) => { /* Successfully completed. */ }
                Ok(Err(e)) => {
                    error!("Error on {watcher_type} iteration: {e}");
                }
                Err(_) => {
                    error!("Timeout on {watcher_type} iteration after {:?}", sleep_time);
                }
            }

            sleep(sleep_time).await;
        }
    }

    false
}
