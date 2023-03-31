// extern crate wayland_client;

mod config;
mod kwin_idle;
mod wl_bindings;

use aw_client_rust::AwClient;
use config::Config;
use kwin_idle::run as run_kwin_idle;
use std::thread;

fn main() {
    let conf = Config::default();
    let client = AwClient::new(&conf.host, &conf.port.to_string(), "aw-watcher");

    let idle_handler = thread::spawn(move || {
        run_kwin_idle(&client, &conf);
    });

    idle_handler.join().expect("Error in the idle processing");
}
