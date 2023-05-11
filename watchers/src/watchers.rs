#[cfg(feature = "gnome")]
mod gnome_idle;
#[cfg(feature = "gnome")]
mod gnome_window;
mod idle;
#[cfg(feature = "kwin_window")]
mod kwin_window;
mod wl_bindings;
mod wl_connection;
mod wl_foreign_toplevel;
mod wl_kwin_idle;
mod x11_connection;
mod x11_screensaver_idle;
mod x11_window;

use crate::{config::Config, report_client::ReportClient};
use std::{
    fmt::Display,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

pub enum WatcherType {
    Idle,
    ActiveWindow,
}

impl WatcherType {
    fn sleep_time(&self, config: &Config) -> Duration {
        match self {
            WatcherType::Idle => config.poll_time_idle,
            WatcherType::ActiveWindow => config.poll_time_idle,
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

pub trait Watcher: Send {
    fn new(client: &Arc<ReportClient>) -> anyhow::Result<Self>
    where
        Self: Sized;

    fn run(
        &mut self,
        watcher_type: &WatcherType,
        client: &Arc<ReportClient>,
        is_stopped: Arc<AtomicBool>,
    ) {
        info!("Starting {watcher_type} watcher");
        loop {
            if is_stopped.load(Ordering::Relaxed) {
                warn!("Received an exit signal, shutting down {watcher_type}");
                break;
            }
            self.run_iteration(client);
            thread::sleep(watcher_type.sleep_time(&client.config));
        }
    }

    fn run_iteration(&mut self, client: &Arc<ReportClient>);
}

type BoxedWatcher = Box<dyn Watcher>;
type WatcherConstructors = [(
    &'static str,
    WatcherType,
    fn(&Arc<ReportClient>) -> anyhow::Result<BoxedWatcher>,
)];

pub trait ConstructorFilter {
    fn filter_first_supported(
        &self,
        client: &Arc<ReportClient>,
    ) -> Option<(&WatcherType, BoxedWatcher)>;

    fn run_first_supported(
        &'static self,
        client: &Arc<ReportClient>,
        is_stopped: Arc<AtomicBool>,
    ) -> Option<JoinHandle<()>>;
}

impl ConstructorFilter for WatcherConstructors {
    fn filter_first_supported(
        &self,
        client: &Arc<ReportClient>,
    ) -> Option<(&WatcherType, BoxedWatcher)> {
        self.iter()
            .find_map(|(name, watcher_type, watcher)| match watcher(client) {
                Ok(watcher) => {
                    info!("Selected {name} as {watcher_type} watcher");
                    Some((watcher_type, watcher))
                }
                Err(e) => {
                    debug!("{name} cannot run: {e}");
                    None
                }
            })
    }

    fn run_first_supported(
        &'static self,
        client: &Arc<ReportClient>,
        is_stopped: Arc<AtomicBool>,
    ) -> Option<JoinHandle<()>> {
        let idle_watcher = self.filter_first_supported(client);
        if let Some((watcher_type, mut watcher)) = idle_watcher {
            let thread_client = Arc::clone(client);
            let idle_handler =
                thread::spawn(move || watcher.run(watcher_type, &thread_client, is_stopped));
            Some(idle_handler)
        } else {
            None
        }
    }
}

macro_rules! watcher {
    ($watcher_struct:ty, $watcher_type:expr) => {
        (stringify!($watcher_struct), $watcher_type, |client| {
            Ok(Box::new(<$watcher_struct>::new(client)?))
        })
    };
}

pub const IDLE: &WatcherConstructors = &[
    watcher!(wl_kwin_idle::IdleWatcher, WatcherType::Idle),
    watcher!(x11_screensaver_idle::IdleWatcher, WatcherType::Idle),
    #[cfg(feature = "gnome")]
    watcher!(gnome_idle::IdleWatcher, WatcherType::Idle),
];

pub const ACTIVE_WINDOW: &WatcherConstructors = &[
    watcher!(
        wl_foreign_toplevel::WindowWatcher,
        WatcherType::ActiveWindow
    ),
    // XWayland gives _NET_WM_NAME on some windows in KDE, but not on others
    #[cfg(feature = "kwin_window")]
    watcher!(kwin_window::WindowWatcher, WatcherType::ActiveWindow),
    watcher!(x11_window::WindowWatcher, WatcherType::ActiveWindow),
    #[cfg(feature = "gnome")]
    watcher!(gnome_window::WindowWatcher, WatcherType::ActiveWindow),
];
