use super::report_client::ReportClient;
use super::wl_bindings;
use super::wl_connection::WlEventConnection;
use super::BoxedError;
use chrono::{DateTime, Duration, Utc};
use std::{sync::Arc, thread, time};
use wayland_client::{
    globals::GlobalListContents,
    protocol::{wl_registry, wl_seat::WlSeat},
    Connection, Dispatch, Proxy, QueueHandle,
};
use wl_bindings::idle::org_kde_kwin_idle::OrgKdeKwinIdle;
use wl_bindings::idle::org_kde_kwin_idle_timeout::{
    Event as OrgKdeKwinIdleTimeoutEvent, OrgKdeKwinIdleTimeout,
};

struct IdleState {
    idle_timeout: OrgKdeKwinIdleTimeout,
    last_input_time: DateTime<Utc>,
    is_idle: bool,
    is_changed: bool,
    client: Arc<ReportClient>,
    bucket_name: String,
}

impl Drop for IdleState {
    fn drop(&mut self) {
        info!("Releasing idle timeout");
        self.idle_timeout.release();
    }
}

impl IdleState {
    fn new(
        idle_timeout: OrgKdeKwinIdleTimeout,
        client: Arc<ReportClient>,
        bucket_name: String,
    ) -> Self {
        Self {
            idle_timeout,
            last_input_time: Utc::now(),
            is_idle: false,
            is_changed: false,
            client,
            bucket_name,
        }
    }

    fn idle(&mut self) {
        self.is_idle = true;
        self.is_changed = true;
        debug!("Idle");
    }

    fn resume(&mut self) {
        self.is_idle = false;
        self.last_input_time = Utc::now();
        self.is_changed = true;
        debug!("Resumed");
    }

    fn run_loop(&mut self, connection: &mut WlEventConnection<Self>) -> Result<(), BoxedError> {
        // connection.event_queue.blocking_dispatch(self).unwrap();
        connection.event_queue.roundtrip(self).unwrap();
        let now = Utc::now();
        if !self.is_idle {
            self.last_input_time = now;
        }

        if self.is_changed {
            let result = if self.is_idle {
                debug!("Reporting as changed to idle");
                self.client.ping(
                    &self.bucket_name,
                    false,
                    self.last_input_time,
                    Duration::zero(),
                )?;
                self.client.ping(
                    &self.bucket_name,
                    true,
                    self.last_input_time + Duration::milliseconds(1),
                    Duration::zero(),
                )
            } else {
                debug!("Reporting as no longer idle");

                self.client.ping(
                    &self.bucket_name,
                    true,
                    self.last_input_time,
                    Duration::zero(),
                )?;
                self.client.ping(
                    &self.bucket_name,
                    false,
                    self.last_input_time + Duration::milliseconds(1),
                    Duration::zero(),
                )
            };
            self.is_changed = false;
            result
        } else if self.is_idle {
            trace!("Reporting as idle");
            self.client.ping(
                &self.bucket_name,
                true,
                self.last_input_time,
                now - self.last_input_time,
            )
        } else {
            trace!("Reporting as not idle");
            self.client.ping(
                &self.bucket_name,
                false,
                self.last_input_time,
                Duration::zero(),
            )
        }
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
        } else if let OrgKdeKwinIdleTimeoutEvent::Resumed = event {
            state.resume();
        }
    }
}

pub fn run(client: &Arc<ReportClient>) {
    let bucket_name = format!(
        "aw-watcher-afk_{}",
        gethostname::gethostname().into_string().unwrap()
    );

    client.create_bucket(&bucket_name, "afkstatus").unwrap();

    info!("Starting idle watcher");

    let mut connection = WlEventConnection::connect().unwrap();

    let mut idle_state = IdleState::new(
        connection
            .get_idle_timeout(client.config.idle_timeout * 1000)
            .unwrap(),
        Arc::clone(client),
        bucket_name,
    );
    connection.event_queue.roundtrip(&mut idle_state).unwrap();

    loop {
        if let Err(e) = idle_state.run_loop(&mut connection) {
            error!("Error on idle iteration {e}");
        }
        thread::sleep(time::Duration::from_secs(u64::from(
            client.config.poll_time_idle,
        )));
    }
}
