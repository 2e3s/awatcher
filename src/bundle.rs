mod menu;
mod server;

pub use menu::Tray;
use std::path::PathBuf;
use watchers::config::Config;

pub fn run(config: &Config, config_file: PathBuf, no_tray: bool) {
    if !no_tray {
        let service = ksni::TrayService::new(Tray {
            server_host: config.host.clone(),
            server_port: config.port,
            config_file,
        });
        service.spawn();
    }

    let port = config.port;
    server::run(port);
}
