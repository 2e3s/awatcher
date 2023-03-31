use super::config::Config;
use super::wl_bindings;
use aw_client_rust::{AwClient, Event as AwEvent};
use chrono::{DateTime, Duration, Utc};
use serde_json::{Map, Value};
use std::{thread, time};
use wayland_client::Proxy;
use wayland_client::{
    globals::{registry_queue_init, GlobalListContents},
    protocol::wl_registry,
    protocol::wl_seat::WlSeat,
    Connection, Dispatch, QueueHandle,
};
use wl_bindings::idle::org_kde_kwin_idle::OrgKdeKwinIdle;
use wl_bindings::idle::org_kde_kwin_idle_timeout::Event as OrgKdeKwinIdleTimeoutEvent;
use wl_bindings::idle::org_kde_kwin_idle_timeout::OrgKdeKwinIdleTimeout;

struct IdleState {
    idle_timeout: OrgKdeKwinIdleTimeout,
    start_time: DateTime<Utc>,
    is_idle: bool,
}

impl Drop for IdleState {
    fn drop(&mut self) {
        println!("Releasing idle timeout");
        self.idle_timeout.release();
    }
}

impl IdleState {
    fn idle(&mut self) {
        self.is_idle = true;
        self.start_time = Utc::now();
    }

    fn resume(&mut self) {
        self.is_idle = false;
        self.start_time = Utc::now();
    }
}

macro_rules! subscribe_state {
    ($struct_name:ty, $data_name:ty) => {
        impl Dispatch<$struct_name, $data_name> for IdleState {
            fn event(
                _: &mut Self,
                _: &$struct_name,
                _: <$struct_name as Proxy>::Event,
                _: &$data_name,
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
            }
        }
    };
}

subscribe_state!(wl_registry::WlRegistry, ());
subscribe_state!(WlSeat, ());
subscribe_state!(OrgKdeKwinIdle, ());
subscribe_state!(wl_registry::WlRegistry, GlobalListContents);

impl Dispatch<OrgKdeKwinIdleTimeout, ()> for IdleState {
    fn event(
        state: &mut Self,
        _: &OrgKdeKwinIdleTimeout,
        event: <OrgKdeKwinIdleTimeout as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let OrgKdeKwinIdleTimeoutEvent::Idle = event {
            state.idle();
            println!("Idle");
        }
        if let OrgKdeKwinIdleTimeoutEvent::Resumed = event {
            state.resume();
            println!("Resumed");
        }
    }
}

fn send_heartbeat(client: &AwClient, state: &IdleState, bucket_name: &String, config: &Config) {
    let now = Utc::now();

    let timestamp = match state.is_idle {
        true => now,
        false => {
            let last_guaranteed_activity = now - Duration::milliseconds(config.timeout_ms as i64);
            match last_guaranteed_activity > state.start_time {
                true => last_guaranteed_activity,
                false => state.start_time,
            }
        }
    };

    let mut data = Map::new();
    let json_afk_state = match state.is_idle {
        true => Value::String("afk".to_string()),
        false => Value::String("not-afk".to_string()),
    };
    data.insert("status".to_string(), json_afk_state);

    let event = AwEvent {
        id: None,
        timestamp,
        duration: Duration::zero(),
        data,
    };

    let interval_margin: f64 = (config.poll_time + 1) as f64 / 1.0;
    if client
        .heartbeat(&bucket_name, &event, interval_margin)
        .is_err()
    {
        println!("Failed to send heartbeat");
    }
}

pub fn run(client: &AwClient, conf: &Config) {
    let hostname = gethostname::gethostname().into_string().unwrap();
    let bucket_name = format!("aw-watcher-afk_{}", hostname);

    client
        .create_bucket_simple(&bucket_name, "afkstatus")
        .expect("Failed to create afk bucket");

    println!("Starting activity watcher");

    let conn = Connection::connect_to_env().expect("Unable to connect to Wayland compositor");
    let display = conn.display();
    let (globals, mut event_queue) = registry_queue_init::<IdleState>(&conn).unwrap();

    let qh = event_queue.handle();

    let _registry = display.get_registry(&qh, ());

    let seat: WlSeat = globals
        .bind(&event_queue.handle(), 1..=WlSeat::interface().version, ())
        .unwrap();
    let idle: OrgKdeKwinIdle = globals
        .bind(
            &event_queue.handle(),
            1..=OrgKdeKwinIdle::interface().version,
            (),
        )
        .unwrap();

    let idle_timeout = idle.get_idle_timeout(&seat, 3000, &qh, ());

    let mut app_state = IdleState {
        idle_timeout,
        is_idle: false,
        start_time: Utc::now(),
    };
    event_queue.roundtrip(&mut app_state).unwrap();

    loop {
        event_queue.blocking_dispatch(&mut app_state).unwrap();
        send_heartbeat(client, &app_state, &bucket_name, conf);
        thread::sleep(time::Duration::from_secs(conf.poll_time as u64));
    }
}
