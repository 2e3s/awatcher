use super::idle;
use super::wl_connection::{subscribe_state, WlEventConnection};
use super::Watcher;
use crate::report_client::ReportClient;
use anyhow::anyhow;
use async_trait::async_trait;
use chrono::TimeDelta;
use std::sync::Arc;
use wayland_client::{
    globals::GlobalListContents,
    protocol::{wl_registry, wl_seat::WlSeat},
    Connection, Dispatch, Proxy, QueueHandle,
};
use wayland_protocols::ext::idle_notify::v1::client::ext_idle_notification_v1::Event as IdleNotificationV1Event;
use wayland_protocols::ext::idle_notify::v1::client::ext_idle_notification_v1::ExtIdleNotificationV1;
use wayland_protocols::ext::idle_notify::v1::client::ext_idle_notifier_v1::ExtIdleNotifierV1;

struct WatcherState {
    idle_notification: ExtIdleNotificationV1,
    idle_state: idle::State,
}

impl Drop for WatcherState {
    fn drop(&mut self) {
        info!("Releasing idle notification");
        self.idle_notification.destroy();
    }
}

impl WatcherState {
    fn new(idle_notification: ExtIdleNotificationV1, idle_timeout: TimeDelta) -> Self {
        Self {
            idle_notification,
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
subscribe_state!(ExtIdleNotifierV1, (), WatcherState);

impl Dispatch<ExtIdleNotificationV1, ()> for WatcherState {
    fn event(
        state: &mut Self,
        _: &ExtIdleNotificationV1,
        event: <ExtIdleNotificationV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let IdleNotificationV1Event::Idled = event {
            state.idle();
        } else if let IdleNotificationV1Event::Resumed = event {
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
        connection.get_ext_idle()?;

        let timeout = u32::try_from(client.config.idle_timeout.num_milliseconds());
        let mut watcher_state = WatcherState::new(
            connection
                .get_ext_idle_notification(timeout.unwrap())
                .unwrap(),
            client.config.idle_timeout,
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
