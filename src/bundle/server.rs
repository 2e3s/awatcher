use anyhow::anyhow;
use aw_server::endpoints::{build_rocket, AssetResolver, ServerState};
use std::net::ToSocketAddrs;
use std::sync::Mutex;

pub async fn run(host: String, port: u16) {
    let db_path = aw_server::dirs::db_path(false)
        .map_err(|()| anyhow!("DB path is not found"))
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let device_id = aw_server::device_id::get_device_id();
    let mut config = aw_server::config::create_config(false);

    let mut addrs_iter = (host + ":" + &port.to_string()).to_socket_addrs().unwrap();
    let address = addrs_iter.next().unwrap();

    info!("Starting server on {}", address);
    config.address = address.ip().to_string();
    config.port = address.port();

    let legacy_import = false;
    let server_state = ServerState {
        datastore: Mutex::new(aw_datastore::Datastore::new(db_path, legacy_import)),
        asset_resolver: AssetResolver::new(None),
        device_id,
    };
    build_rocket(server_state, config).launch().await.unwrap();
}
