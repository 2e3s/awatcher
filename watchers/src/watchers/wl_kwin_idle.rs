use super::idle;
use super::wl_connection::{subscribe_state, WlEventConnection};
use super::Watcher;
use crate::report_client::ReportClient;
use anyhow::anyhow;
use async_trait::async_trait;
use chrono::Duration;
use std::sync::Arc;
use wayland_client::{
    globals::GlobalListContents,
    protocol::{wl_registry, wl_seat::WlSeat},
    Connection, Dispatch, Proxy, QueueHandle,
};
use wayland_protocols_plasma::idle::client::org_kde_kwin_idle::OrgKdeKwinIdle;
use wayland_protocols_plasma::idle::client::org_kde_kwin_idle_timeout::{
    Event as OrgKdeKwinIdleTimeoutEvent, OrgKdeKwinIdleTimeout,
};

struct WatcherState {
    kwin_idle_timeout: OrgKdeKwinIdleTimeout,
    idle_state: idle::State,
}

impl Drop for WatcherState {
    fn drop(&mut self) {
        info!("Releasing idle timeout");
        self.kwin_idle_timeout.release();
    }
}

impl WatcherState {
    fn new(kwin_idle_timeout: OrgKdeKwinIdleTimeout, idle_timeout: Duration) -> Self {
        Self {
            kwin_idle_timeout,
            idle_state: idle::State::new(idle_timeout),
        }
    }

    fn idle(&mut self) {
        self.idle_state.mark_idle();
        debug!("Idle");
    }

    fn resume(&mut self) {
        self.idle_state.mark_not_idle();
        debug!("Resumed");
    }
}

subscribe_state!(wl_registry::WlRegistry, GlobalListContents, WatcherState);
subscribe_state!(wl_registry::WlRegistry, (), WatcherState);
subscribe_state!(WlSeat, (), WatcherState);
subscribe_state!(OrgKdeKwinIdle, (), WatcherState);

impl Dispatch<OrgKdeKwinIdleTimeout, ()> for WatcherState {
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
    connection: WlEventConnection<WatcherState>,
    watcher_state: WatcherState,
}

#[async_trait]
impl Watcher for IdleWatcher {
    async fn new(client: &Arc<ReportClient>) -> anyhow::Result<Self> {
        let mut connection: WlEventConnection<WatcherState> = WlEventConnection::connect()?;
        connection.get_kwin_idle()?;

        let timeout = u32::try_from(client.config.idle_timeout.as_secs() * 1000);
        let mut watcher_state = WatcherState::new(
            connection.get_kwin_idle_timeout(timeout.unwrap()).unwrap(),
            Duration::from_std(client.config.idle_timeout).unwrap(),
        );
        connection
            .event_queue
            .roundtrip(&mut watcher_state)
            .unwrap();

        Ok(Self {
            connection,
            watcher_state,
        })
    }

    async fn run_iteration(&mut self, client: &Arc<ReportClient>) -> anyhow::Result<()> {
        self.connection
            .event_queue
            .roundtrip(&mut self.watcher_state)
            .map_err(|e| anyhow!("Event queue is not processed: {e}"))?;

        self.watcher_state.idle_state.send_reactive(client).await
    }
}
