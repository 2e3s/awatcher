mod gnome_idle;
mod gnome_window;
mod idle;
mod kwin_window;
mod wl_bindings;
mod wl_connection;
mod wl_foreign_toplevel;
mod wl_kwin_idle;
mod x11_connection;
mod x11_screensaver_idle;
mod x11_window;

use crate::report_client::ReportClient;
use std::sync::Arc;

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
}

impl ConstructorFilter for WatcherConstructors {
    fn filter_first_supported(&self) -> Option<BoxedWatcher> {
        self.iter().find_map(|(name, watcher)| match watcher() {
            Ok(watcher) => Some(watcher),
            Err(e) => {
                info!("{name} cannot run: {e}");
                None
            }
        })
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
    watcher!(gnome_idle::IdleWatcher),
];

pub const ACTIVE_WINDOW: &WatcherConstructors = &[
    watcher!(wl_foreign_toplevel::WindowWatcher),
    watcher!(x11_window::WindowWatcher),
    watcher!(kwin_window::WindowWatcher),
    watcher!(gnome_window::WindowWatcher),
];
