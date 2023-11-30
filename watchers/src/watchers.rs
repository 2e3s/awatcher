#[cfg(feature = "gnome")]
mod gnome_idle;
#[cfg(feature = "gnome")]
mod gnome_window;
mod idle;
#[cfg(feature = "kwin_window")]
mod kwin_window;
mod wl_bindings;
mod wl_connection;
mod wl_ext_idle_notify;
mod wl_foreign_toplevel;
mod wl_kwin_idle;
mod x11_connection;
mod x11_screensaver_idle;
mod x11_window;

use crate::{config::Config, report_client::ReportClient};
use async_trait::async_trait;
use std::{fmt::Display, sync::Arc};
use tokio::time;

pub enum WatcherType {
    Idle,
    ActiveWindow,
}

impl WatcherType {
    fn sleep_time(&self, config: &Config) -> time::Duration {
        match self {
            WatcherType::Idle => config.poll_time_idle,
            WatcherType::ActiveWindow => config.poll_time_window,
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

macro_rules! watch {
    ($client:expr, $watcher_type:expr, $watcher_struct:ty) => {
        match <$watcher_struct>::new($client).await {
            Ok(watcher) => {
                info!(
                    "Selected {} as {} watcher",
                    stringify!($watcher_struct),
                    $watcher_type
                );
                return Some(Box::new(watcher));
            }
            Err(e) => {
                debug!("{} cannot run: {e}", stringify!($watcher_struct));
            }
        };
    };
}

async fn filter_first_supported(
    client: &Arc<ReportClient>,
    watcher_type: &WatcherType,
) -> Option<Box<dyn Watcher>> {
    match watcher_type {
        WatcherType::Idle => {
            watch!(client, watcher_type, wl_ext_idle_notify::IdleWatcher);
            watch!(client, watcher_type, wl_kwin_idle::IdleWatcher);
            watch!(client, watcher_type, x11_screensaver_idle::IdleWatcher);
            #[cfg(feature = "gnome")]
            watch!(client, watcher_type, gnome_idle::IdleWatcher);
        }
        WatcherType::ActiveWindow => {
            watch!(client, watcher_type, wl_foreign_toplevel::WindowWatcher);
            // XWayland gives _NET_WM_NAME on some windows in KDE, but not on others
            #[cfg(feature = "kwin_window")]
            watch!(client, watcher_type, kwin_window::WindowWatcher);
            watch!(client, watcher_type, x11_window::WindowWatcher);
            #[cfg(feature = "gnome")]
            watch!(client, watcher_type, gnome_window::WindowWatcher);
        }
    };

    None
}

pub async fn run_first_supported(client: Arc<ReportClient>, watcher_type: &WatcherType) -> bool {
    let supported_watcher = filter_first_supported(&client, watcher_type).await;
    if let Some(mut watcher) = supported_watcher {
        info!("Starting {watcher_type} watcher");
        loop {
            if let Err(e) = watcher.run_iteration(&client).await {
                error!("Error on {watcher_type} iteration: {e}");
            }
            time::sleep(watcher_type.sleep_time(&client.config)).await;
        }
    }

    false
}
