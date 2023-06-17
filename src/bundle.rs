mod menu;
mod server;

pub use menu::Tray;
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
};

pub async fn run(
    host: String,
    port: u32,
    config_file: PathBuf,
    no_tray: bool,
    is_stopped: Arc<AtomicBool>,
) {
    if !no_tray {
        let service = ksni::TrayService::new(Tray {
            server_host: host,
            server_port: port,
            config_file,
            is_stopped: Arc::clone(&is_stopped),
        });
        service.spawn();
    }

    server::run(port, is_stopped).await;
}
