pub mod defaults;
mod file_config;
mod filters;

use self::filters::Filter;
use chrono::Duration;
pub use file_config::FileConfig;
pub use filters::FilterResult;

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
    pub fn match_window_data(&self, app_id: &str, title: &str) -> FilterResult {
        for filter in &self.filters {
            let result = filter.apply(app_id, title);
            if matches!(result, FilterResult::Match | FilterResult::Replace(_)) {
                return result;
            }
        }

        FilterResult::Skip
    }
}
