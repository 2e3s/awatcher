#[macro_use]
extern crate log;

pub mod config;
mod report_client;
mod watchers;

pub use crate::report_client::ReportClient;
pub use crate::watchers::run_first_supported;
pub use crate::watchers::Watcher;
pub use crate::watchers::ACTIVE_WINDOW;
pub use crate::watchers::IDLE;
