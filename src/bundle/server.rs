use anyhow::anyhow;
use aw_server::endpoints::{build_rocket, embed_asset_resolver};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tokio::time::{sleep, Duration};

pub fn run(port: u32, is_stopped: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        let db_path = aw_server::dirs::db_path(false)
            .map_err(|_| anyhow!("DB path is not found"))
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let device_id = aw_server::device_id::get_device_id();
        let mut config = aw_server::config::create_config(false);
        config.address = "127.0.0.1".to_string();
        config.port = u16::try_from(port).unwrap();

        let legacy_import = false;
        let server_state = aw_server::endpoints::ServerState {
            datastore: Mutex::new(aw_datastore::Datastore::new(db_path, legacy_import)),
            asset_resolver: embed_asset_resolver!("$AW_WEBUI_DIST"),
            device_id,
        };
        let server = build_rocket(server_state, config).launch();

        let check = async {
            loop {
                if is_stopped.load(Ordering::Relaxed) {
                    warn!("Received an exit signal, stopping the server");
                    break;
                }

                sleep(Duration::from_secs(1)).await;
            }
        };

        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                tokio::select! (
                    r = server => {r.unwrap();},
                    _ = check => {},
                );
            });
    });
}
