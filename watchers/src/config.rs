pub mod defaults;
mod file_config;
mod filters;

use std::{net::Ipv4Addr, str::FromStr};

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

fn normalize_server_host(server_host: &str) -> String {
    let is_zero_first_octet = match Ipv4Addr::from_str(server_host) {
        Ok(ip) => ip.octets()[0] == 0,
        Err(_) => false,
    };
    if is_zero_first_octet {
        "127.0.0.1".to_string()
    } else {
        server_host.to_string()
    }
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

    pub fn client_host(&self) -> String {
        normalize_server_host(&self.host)
    }
}
