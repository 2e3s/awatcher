mod menu;
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
    if !no_tray {
        let service = ksni::TrayService::new(Tray {
            server_host: host,
            server_port: port,
            config_file,
            shutdown_sender,
        });
        service.spawn();
    }

    server::run(port).await;
}
