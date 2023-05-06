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

use crate::report_client::ReportClient;
use std::{
    sync::Arc,
    thread::{self, JoinHandle},
};

pub trait Watcher: Send {
    fn new() -> anyhow::Result<Self>
    where
        Self: Sized;
    fn watch(&mut self, client: &Arc<ReportClient>);
}

type BoxedWatcher = Box<dyn Watcher>;

type WatcherConstructor = (&'static str, fn() -> anyhow::Result<BoxedWatcher>);
type WatcherConstructors = [WatcherConstructor];

pub trait ConstructorFilter {
    fn filter_first_supported(&self) -> Option<BoxedWatcher>;

    fn run_first_supported(&self, client: &Arc<ReportClient>) -> Option<JoinHandle<()>>;
}

impl ConstructorFilter for WatcherConstructors {
    fn filter_first_supported(&self) -> Option<BoxedWatcher> {
        self.iter().find_map(|(name, watcher)| match watcher() {
            Ok(watcher) => Some(watcher),
            Err(e) => {
                debug!("{name} cannot run: {e}");
                None
            }
        })
    }

    fn run_first_supported(&self, client: &Arc<ReportClient>) -> Option<JoinHandle<()>> {
        let idle_watcher = self.filter_first_supported();
        if let Some(mut watcher) = idle_watcher {
            let thread_client = Arc::clone(client);
            let idle_handler = thread::spawn(move || watcher.watch(&thread_client));
            Some(idle_handler)
        } else {
            None
        }
    }
}

macro_rules! watcher {
    ($watcher_struct:ty) => {
        (stringify!($watcher_struct), || {
            Ok(Box::new(<$watcher_struct>::new()?))
        })
    };
}

pub const IDLE: &WatcherConstructors = &[
    watcher!(wl_kwin_idle::IdleWatcher),
    watcher!(x11_screensaver_idle::IdleWatcher),
    #[cfg(feature = "gnome")]
    watcher!(gnome_idle::IdleWatcher),
];

pub const ACTIVE_WINDOW: &WatcherConstructors = &[
    watcher!(wl_foreign_toplevel::WindowWatcher),
    // XWayland gives _NET_WM_NAME on some windows in KDE, but not on others
    #[cfg(feature = "kwin_window")]
    watcher!(kwin_window::WindowWatcher),
    watcher!(x11_window::WindowWatcher),
    #[cfg(feature = "gnome")]
    watcher!(gnome_window::WindowWatcher),
];
