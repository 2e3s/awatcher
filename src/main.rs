#![warn(clippy::pedantic)]

#[macro_use]
extern crate log;

mod config;
mod report_client;
mod wl_bindings;
mod wl_connection;
mod wl_kwin_idle;
mod wl_kwin_window;

use config::Config;
use fern::colors::{Color, ColoredLevelConfig};
use report_client::ReportClient;
use std::env;
use std::{error::Error, str::FromStr, sync::Arc, thread};
use wl_kwin_idle::run as run_kwin_idle;
use wl_kwin_window::run as run_kwin_active_window;

type BoxedError = Box<dyn Error>;

fn setup_logger() -> Result<(), fern::InitError> {
    let log_setting = env::var("AWATCHER_LOG").unwrap_or("info".to_string());

    fern::Dispatch::new()
        .format(|out, message, record| {
            let colors = ColoredLevelConfig::new()
                .info(Color::Green)
                .debug(Color::Blue)
                .trace(Color::Cyan);
            out.finish(format_args!(
                "[{} {} {}] {}",
                chrono::Utc::now().format("%Y-%m-%d %H:%M:%S%.6f"),
                colors.color(record.level()),
                record.target(),
                message
            ));
        })
        .level(log::LevelFilter::Warn)
        .level_for(
            "awatcher",
            FromStr::from_str(&log_setting).unwrap_or(log::LevelFilter::Info),
        )
        .chain(std::io::stdout())
        .apply()?;
    Ok(())
}

fn main() {
    setup_logger().unwrap();

    let client = ReportClient::new(Config::from_cli());
    let client = Arc::new(client);

    info!(
        "Sending to server {}:{}",
        client.config.host, client.config.port
    );
    info!("Idle timeout: {} seconds", client.config.idle_timeout);
    info!("Polling period: {} seconds", client.config.poll_time_idle);

    let client1 = Arc::clone(&client);
    let idle_handler = thread::spawn(move || run_kwin_idle(&client1));

    let client2 = Arc::clone(&client);
    let active_window_handler = thread::spawn(move || run_kwin_active_window(&client2));

    idle_handler.join().expect("Idle thread failed");
    active_window_handler
        .join()
        .expect("Active window thread failed");
}
