pub mod defaults;
mod file_config;
mod filters;

use self::filters::{Filter, Replacement};
use chrono::Duration;
pub use file_config::FileConfig;

pub struct Config {
    pub port: u16,
    pub host: String,
    pub idle_timeout: Duration,
    pub poll_time_idle: Duration,
    pub poll_time_window: Duration,
    pub no_server: bool,
    pub filters: Vec<Filter>,
}

impl Config {
    pub fn window_data_replacement(&self, app_id: &str, title: &str) -> Replacement {
        for filter in &self.filters {
            if let Some(replacement) = filter.replacement(app_id, title) {
                return replacement;
            }
        }

        Replacement::default()
    }
}
