mod menu;
mod server;
mod site_data;

pub use menu::Tray;
use site_data::unpack_data;
use watchers::config::Config;

pub fn run(config: &Config, no_tray: bool) -> anyhow::Result<()> {
    if !no_tray {
        let service = ksni::TrayService::new(Tray {
            server_host: config.host.clone(),
            server_port: config.port,
        });
        service.spawn();
    }

    let port = config.port;
    let data_dir = unpack_data()?;
    server::run(data_dir, port);

    Ok(())
}
