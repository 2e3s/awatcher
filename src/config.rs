pub struct Config {
    pub port: u32,
    pub host: String,
    pub timeout_ms: u32,
    pub poll_time: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            port: 5600,
            host: String::from("localhost"),
            timeout_ms: 3000,
            poll_time: 5,
        }
    }
}
