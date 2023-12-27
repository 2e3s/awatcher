use super::wl_connection::WlEventConnection;
use super::{wl_connection::subscribe_state, Watcher};
use crate::report_client::ReportClient;
use anyhow::{anyhow, Context};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use wayland_client::{
    event_created_child, globals::GlobalListContents, protocol::wl_registry, Connection, Dispatch,
    Proxy, QueueHandle,
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_handle_v1::{
    Event as HandleEvent, State as HandleState, ZwlrForeignToplevelHandleV1,
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_manager_v1::{
    Event as ManagerEvent, ZwlrForeignToplevelManagerV1, EVT_TOPLEVEL_OPCODE,
};

struct WindowData {
    app_id: String,
    title: String,
}

struct ToplevelState {
    windows: HashMap<String, WindowData>,
    current_window_id: Option<String>,
}

impl ToplevelState {
    fn new() -> Self {
        Self {
            windows: HashMap::new(),
            current_window_id: None,
        }
    }
}

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for ToplevelState {
    fn event(
        state: &mut Self,
        _: &ZwlrForeignToplevelManagerV1,
        event: <ZwlrForeignToplevelManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ManagerEvent::Toplevel { toplevel } => {
                debug!("Toplevel handle is received {}", toplevel.id());
                state.windows.insert(
                    toplevel.id().to_string(),
                    WindowData {
                        app_id: "unknown".into(),
                        title: "unknown".into(),
                    },
                );
            }
            ManagerEvent::Finished => {
                error!("Toplevel manager is finished, the application may crash");
            }
            _ => (),
        };
    }

    event_created_child!(ToplevelState, ZwlrForeignToplevelManagerV1, [
        EVT_TOPLEVEL_OPCODE => (ZwlrForeignToplevelHandleV1, ()),
    ]);
}

subscribe_state!(wl_registry::WlRegistry, GlobalListContents, ToplevelState);
subscribe_state!(wl_registry::WlRegistry, (), ToplevelState);

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for ToplevelState {
    fn event(
        toplevel_state: &mut Self,
        handle: &ZwlrForeignToplevelHandleV1,
        event: <ZwlrForeignToplevelHandleV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let id = handle.id().to_string();
        let window = toplevel_state.windows.get_mut(&id);
        if let Some(window) = window {
            match event {
                HandleEvent::Title { title } => {
                    trace!("Title is changed for {id}: {title}");
                    window.title = title;
                }
                HandleEvent::AppId { app_id } => {
                    trace!("App ID is changed for {id}: {app_id}");
                    window.app_id = app_id;
                }
                HandleEvent::State { state } => {
                    trace!("State is changed for {id}: {:?}", state);
                    if state.contains(&(HandleState::Activated as u8)) {
                        trace!("Window is activated: {id}");
                        toplevel_state.current_window_id = Some(id);
                    }
                }
                HandleEvent::Done => trace!("Done: {id}"),
                HandleEvent::Closed => {
                    trace!("Window is closed: {id}");
                    if toplevel_state.windows.remove(&id).is_none() {
                        warn!("Window is already removed: {id}");
                    }
                }
                _ => (),
            };
        } else {
            error!("Window is not found: {id}");
        }
    }
}

pub struct WindowWatcher {
    connection: WlEventConnection<ToplevelState>,
    toplevel_state: ToplevelState,
}

impl WindowWatcher {
    async fn send_active_window(&self, client: &Arc<ReportClient>) -> anyhow::Result<()> {
        let active_window_id = self
            .toplevel_state
            .current_window_id
            .as_ref()
            .ok_or(anyhow!("Current window is unknown"))?;
        let active_window = self
            .toplevel_state
            .windows
            .get(active_window_id)
            .ok_or(anyhow!(
                "Current window is not found by ID {active_window_id}"
            ))?;

        client
            .send_active_window(&active_window.app_id, &active_window.title)
            .await
            .with_context(|| "Failed to send heartbeat for active window")
    }
}

#[async_trait]
impl Watcher for WindowWatcher {
    async fn new(_: &Arc<ReportClient>) -> anyhow::Result<Self> {
        let mut connection: WlEventConnection<ToplevelState> = WlEventConnection::connect()?;
        connection.get_foreign_toplevel_manager()?;

        let mut toplevel_state = ToplevelState::new();

        connection
            .event_queue
            .roundtrip(&mut toplevel_state)
            .unwrap();

        Ok(Self {
            connection,
            toplevel_state,
        })
    }

    async fn run_iteration(&mut self, client: &Arc<ReportClient>) -> anyhow::Result<()> {
        self.connection
            .event_queue
            .roundtrip(&mut self.toplevel_state)
            .map_err(|e| anyhow!("Event queue is not processed: {e}"))?;

        self.send_active_window(client).await
    }
}
