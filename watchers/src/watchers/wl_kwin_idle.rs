use super::wl_bindings;
use super::wl_connection::{subscribe_state, WlEventConnection};
use super::Watcher;
use crate::report_client::ReportClient;
use anyhow::anyhow;
use chrono::{DateTime, Duration, Utc};
use std::sync::Arc;
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
}

impl Drop for IdleState {
    fn drop(&mut self) {
        info!("Releasing idle timeout");
        self.idle_timeout.release();
    }
}

impl IdleState {
    fn new(idle_timeout: OrgKdeKwinIdleTimeout) -> Self {
        Self {
            idle_timeout,
            last_input_time: Utc::now(),
            is_idle: false,
            is_changed: false,
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

    fn send_ping(&mut self, client: &Arc<ReportClient>) -> anyhow::Result<()> {
        let now = Utc::now();
        if !self.is_idle {
            self.last_input_time = now;
        }

        if self.is_changed {
            let result = if self.is_idle {
                debug!("Reporting as changed to idle");
                client.ping(false, self.last_input_time, Duration::zero())?;
                client.ping(
                    true,
                    self.last_input_time + Duration::milliseconds(1),
                    Duration::zero(),
                )
            } else {
                debug!("Reporting as no longer idle");

                client.ping(true, self.last_input_time, Duration::zero())?;
                client.ping(
                    false,
                    self.last_input_time + Duration::milliseconds(1),
                    Duration::zero(),
                )
            };
            self.is_changed = false;
            result
        } else if self.is_idle {
            trace!("Reporting as idle");
            client.ping(true, self.last_input_time, now - self.last_input_time)
        } else {
            trace!("Reporting as not idle");
            client.ping(false, self.last_input_time, Duration::zero())
        }
    }
}

subscribe_state!(wl_registry::WlRegistry, GlobalListContents, IdleState);
subscribe_state!(wl_registry::WlRegistry, (), IdleState);
subscribe_state!(WlSeat, (), IdleState);
subscribe_state!(OrgKdeKwinIdle, (), IdleState);

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

pub struct IdleWatcher {
    connection: WlEventConnection<IdleState>,
    idle_state: IdleState,
}

impl Watcher for IdleWatcher {
    fn new(client: &Arc<ReportClient>) -> anyhow::Result<Self> {
        let mut connection: WlEventConnection<IdleState> = WlEventConnection::connect()?;
        connection.get_kwin_idle()?;

        let timeout = u32::try_from(client.config.idle_timeout.as_secs() * 1000);
        let mut idle_state =
            IdleState::new(connection.get_kwin_idle_timeout(timeout.unwrap()).unwrap());
        connection.event_queue.roundtrip(&mut idle_state).unwrap();

        Ok(Self {
            connection,
            idle_state,
        })
    }

    fn run_iteration(&mut self, client: &Arc<ReportClient>) -> anyhow::Result<()> {
        self.connection
            .event_queue
            .roundtrip(&mut self.idle_state)
            .map_err(|e| anyhow!("Event queue is not processed: {e}"))?;

        self.idle_state.send_ping(client)
    }
}
