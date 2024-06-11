use anyhow::anyhow;
use aw_server::endpoints::{build_rocket, AssetResolver, ServerState};
use std::sync::Mutex;

pub async fn run(port: u16) {
    let db_path = aw_server::dirs::db_path(false)
        .map_err(|()| anyhow!("DB path is not found"))
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let device_id = aw_server::device_id::get_device_id();
    let mut config = aw_server::config::create_config(false);
    config.address = "127.0.0.1".to_string();
    config.port = port;

    let legacy_import = false;
    let server_state = ServerState {
        datastore: Mutex::new(aw_datastore::Datastore::new(db_path, legacy_import)),
        asset_resolver: AssetResolver::new(None),
        device_id,
    };
    build_rocket(server_state, config).launch().await.unwrap();
}
