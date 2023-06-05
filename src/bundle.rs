mod menu;
mod server;

pub use menu::Tray;
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
};
use watchers::config::Config;

pub fn run(config: &Config, config_file: PathBuf, no_tray: bool, is_stopped: Arc<AtomicBool>) {
    if !no_tray {
        let service = ksni::TrayService::new(Tray {
            server_host: config.host.clone(),
            server_port: config.port,
            config_file,
            is_stopped: Arc::clone(&is_stopped),
        });
        service.spawn();
    }

    let port = config.port;
    server::run(port, is_stopped);
}
