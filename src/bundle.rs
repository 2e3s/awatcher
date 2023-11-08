mod menu;
mod server;

pub use menu::Tray;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::sync::mpsc::UnboundedSender;

fn get_config_watchers(config_path: &Path) -> Option<Vec<String>> {
    let mut config_path = config_path.parent()?.to_path_buf();
    config_path.push("bundle-config.toml");
    debug!("Reading bundle config at {}", config_path.display());

    let config_content = std::fs::read_to_string(&config_path).ok()?;
    let toml_content: toml::Value = toml::from_str(&config_content).ok()?;

    trace!("Bundle config: {toml_content:?}");

    Some(
        toml_content
            .get("watchers")?
            .get("autostart")?
            .as_array()?
            .iter()
            .filter_map(|value| value.as_str())
            .map(std::string::ToString::to_string)
            .collect(),
    )
}

pub async fn run(
    host: String,
    port: u32,
    config_file: PathBuf,
    no_tray: bool,
    shutdown_sender: UnboundedSender<()>,
) {
    let watchers: Vec<String> =
        get_config_watchers(config_file.parent().unwrap()).unwrap_or_default();

    for watcher in &watchers {
        debug!("Starting an external watcher {}", watcher);
        let _ = Command::new(watcher).spawn();
    }

    if !no_tray {
        let service = ksni::TrayService::new(Tray::new(
            host,
            port,
            config_file,
            shutdown_sender,
            watchers,
        ));
        service.spawn();
    }

    server::run(port).await;
}
