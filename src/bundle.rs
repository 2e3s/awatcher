mod menu;
mod modules;
mod server;

pub use menu::Tray;
use std::path::PathBuf;
use tokio::sync::mpsc::UnboundedSender;

pub async fn run(
    host: String,
    port: u32,
    config_file: PathBuf,
    no_tray: bool,
    shutdown_sender: UnboundedSender<()>,
) {
    let manager = modules::Manager::new(
        &std::env::var("PATH").unwrap_or_default(),
        config_file.parent().unwrap(),
    );

    if !no_tray {
        let tray = Tray::new(host, port, config_file, shutdown_sender, manager);
        let service = ksni::TrayService::new(tray);
        service.spawn();
    }

    server::run(port).await;
}
