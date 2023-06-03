mod menu;
mod server;

pub use menu::Tray;
use watchers::config::Config;

pub fn run(config: &Config, no_tray: bool) {
    if !no_tray {
        let service = ksni::TrayService::new(Tray {
            server_host: config.host.clone(),
            server_port: config.port,
        });
        service.spawn();
    }

    let port = config.port;
    server::run(port);
}
