// This protocol extends ext_foreign_toplevel_v1
use super::wl_connection::WlEventConnection;
use super::{wl_connection::subscribe_state, Watcher};
use crate::report_client::ReportClient;
use anyhow::{anyhow, Context};
use async_trait::async_trait;
use cctk::cosmic_protocols::toplevel_info::v1::client::zcosmic_toplevel_handle_v1::{
    Event as CosmicHandleEvent, State as CosmicHandleState, ZcosmicToplevelHandleV1,
};
use cctk::cosmic_protocols::toplevel_info::v1::client::zcosmic_toplevel_info_v1::{
    Event as CosmicInfoEvent, ZcosmicToplevelInfoV1,
};
use std::collections::HashMap;
use std::sync::Arc;
use wayland_client::{
    event_created_child, globals::GlobalListContents, protocol::wl_registry, Connection, Dispatch,
    Proxy, QueueHandle,
};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::{
    Event as ForeignHandleEvent, ExtForeignToplevelHandleV1,
};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_list_v1::{
    Event as ForeignListEvent, ExtForeignToplevelListV1, EVT_TOPLEVEL_OPCODE,
};

struct WindowData {
    app_id: String,
    title: String,
    cosmic_handle: ZcosmicToplevelHandleV1,
    activated: bool,
}

struct ToplevelState {
    cosmic_info: ZcosmicToplevelInfoV1,
    windows: HashMap<String, WindowData>,
    current_window_id: Option<String>,
    initialized: bool,
}

impl ToplevelState {
    fn new(cosmic_info: ZcosmicToplevelInfoV1) -> Self {
        Self {
            cosmic_info,
            windows: HashMap::new(),
            current_window_id: None,
            initialized: false,
        }
    }

    fn get_active_window(&self) -> Option<&WindowData> {
        let active_window_id = self.current_window_id.as_ref()?;

        self.windows.get(active_window_id)
    }
}

impl Dispatch<ExtForeignToplevelListV1, ()> for ToplevelState {
    fn event(
        state: &mut Self,
        _: &ExtForeignToplevelListV1,
        event: <ExtForeignToplevelListV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            ForeignListEvent::Toplevel { toplevel } => {
                let id = toplevel.id().to_string();
                debug!("Foreign toplevel handle is received: {id}");

                let cosmic_handle = state.cosmic_info.get_cosmic_toplevel(&toplevel, qh, ());

                state.windows.insert(
                    id,
                    WindowData {
                        app_id: "unknown".into(),
                        title: "unknown".into(),
                        cosmic_handle,
                        activated: false,
                    },
                );
            }
            ForeignListEvent::Finished => {
                error!("Foreign toplevel list is finished");
            }
            _ => (),
        };
    }

    event_created_child!(ToplevelState, ExtForeignToplevelListV1, [
        EVT_TOPLEVEL_OPCODE => (ExtForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ExtForeignToplevelHandleV1, ()> for ToplevelState {
    fn event(
        toplevel_state: &mut Self,
        handle: &ExtForeignToplevelHandleV1,
        event: <ExtForeignToplevelHandleV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let id = handle.id().to_string();
        let window = toplevel_state.windows.get_mut(&id);
        if let Some(window) = window {
            match event {
                ForeignHandleEvent::Title { title } => {
                    trace!("Title is changed for {id}: {title}");
                    window.title = title;
                }
                ForeignHandleEvent::AppId { app_id } => {
                    trace!("App ID is changed for {id}: {app_id}");
                    window.app_id = app_id;
                }
                ForeignHandleEvent::Closed => {
                    trace!("Window is closed: {id}");
                    if toplevel_state.windows.remove(&id).is_none() {
                        warn!("Window is already removed: {id}");
                    }
                    if toplevel_state.current_window_id.as_deref() == Some(&id) {
                        toplevel_state.current_window_id = None;
                    }
                }
                _ => (),
            };
        } else {
            error!("Window is not found: {id}");
        }
    }
}

impl Dispatch<ZcosmicToplevelInfoV1, ()> for ToplevelState {
    fn event(
        toplevel_state: &mut Self,
        _: &ZcosmicToplevelInfoV1,
        event: <ZcosmicToplevelInfoV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            CosmicInfoEvent::Toplevel { toplevel } => {
                debug!("Cosmic toplevel handle is received: {}", toplevel.id());
            }
            CosmicInfoEvent::Done => {
                let current_is_still_active = toplevel_state
                    .current_window_id
                    .as_ref()
                    .and_then(|id| toplevel_state.windows.get(id))
                    .map(|w| w.activated)
                    .unwrap_or(false);

                if !current_is_still_active {
                    toplevel_state.current_window_id = toplevel_state
                        .windows
                        .iter()
                        .find_map(|(id, window)| window.activated.then(|| id.clone()));
                    if let Some(window) = toplevel_state.get_active_window() {
                        debug!(
                            "Active window is changed: {} - {}",
                            window.app_id, window.title
                        );
                    }
                }
                trace!(
                    "Cosmic toplevel info is done, \
                    current active window is still active = {}, \
                    current active window id = {:?}",
                    current_is_still_active,
                    toplevel_state.current_window_id,
                );
            }
            CosmicInfoEvent::Finished => {
                error!("Cosmic toplevel info is finished, the application may crash");
            }
            _ => (),
        };
    }
}

impl Dispatch<ZcosmicToplevelHandleV1, ()> for ToplevelState {
    fn event(
        toplevel_state: &mut Self,
        handle: &ZcosmicToplevelHandleV1,
        event: <ZcosmicToplevelHandleV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let entry = toplevel_state
            .windows
            .iter_mut()
            .find(|(_, w)| w.cosmic_handle == *handle);

        if let Some((id, window)) = entry {
            let id = id.clone();
            if let CosmicHandleEvent::State { state } = event {
                // State is encoded as an array of u32 values (4 bytes each)
                let activated = state.chunks_exact(4).any(|chunk| {
                    let value = u32::from_ne_bytes(chunk[0..4].try_into().unwrap());
                    value == CosmicHandleState::Activated as u32
                });

                window.activated = activated;

                if activated {
                    debug!("Window is marked activated: {id}");
                    if !toplevel_state.initialized {
                        toplevel_state.initialized = true;
                        toplevel_state.current_window_id = Some(id);
                    }
                } else if toplevel_state.current_window_id.as_deref() == Some(&id) {
                    debug!("Window is marked deactivated: {id}");
                }
            }
        } else {
            debug!("Cosmic handle event for unknown handle: {}", handle.id());
        }
    }
}

subscribe_state!(wl_registry::WlRegistry, GlobalListContents, ToplevelState);
subscribe_state!(wl_registry::WlRegistry, (), ToplevelState);

pub struct WindowWatcher {
    connection: WlEventConnection<ToplevelState>,
    toplevel_state: ToplevelState,
}

impl WindowWatcher {
    async fn send_active_window(&self, client: &Arc<ReportClient>) -> anyhow::Result<()> {
        let active_window_id = self.toplevel_state.current_window_id.as_ref();

        if let Some(active_window_id) = active_window_id {
            let active_window =
                self.toplevel_state
                    .windows
                    .get(active_window_id)
                    .ok_or(anyhow!(
                        "Current window is not found by ID {active_window_id}"
                    ))?;

            client
                .send_active_window(&active_window.app_id, &active_window.title)
                .await
                .with_context(|| "Failed to send heartbeat for active window")
        } else {
            // This happens when a toplevel handle for the new window is received but the window is not marked as activated yet
            info!("Current active window is unknown, skipping sending heartbeat");
            Ok(())
        }
    }
}

#[async_trait]
impl Watcher for WindowWatcher {
    async fn new(_: &Arc<ReportClient>) -> anyhow::Result<Self> {
        let mut connection: WlEventConnection<ToplevelState> = WlEventConnection::connect()?;

        let _foreign_list = connection.get_ext_foreign_toplevel_list()?;
        let cosmic_info = connection.get_cosmic_toplevel_info_v2()?;

        let mut toplevel_state = ToplevelState::new(cosmic_info);

        connection
            .roundtrip(&mut toplevel_state)
            .map_err(|e| anyhow!("Event queue is not processed: {e}"))?;

        Ok(Self {
            connection,
            toplevel_state,
        })
    }

    async fn run_iteration(&mut self, client: &Arc<ReportClient>) -> anyhow::Result<()> {
        self.connection
            .roundtrip(&mut self.toplevel_state)
            .map_err(|e| anyhow!("Event queue is not processed: {e}"))?;

        self.send_active_window(client).await
    }
}
