use super::wl_bindings::zdwl_ipc::{
    zdwl_ipc_manager_v2::ZdwlIpcManagerV2,
    zdwl_ipc_output_v2::{Event as ZdwlIpcOutputEvent, ZdwlIpcOutputV2},
};
use crate::watchers::wl_connection::WlEventConnection;
use crate::watchers::Watcher;
use crate::ReportClient;
use async_trait::async_trait;
use std::sync::Arc;
use wayland_client::globals::GlobalListContents;
use wayland_client::protocol::{wl_output::WlOutput, wl_registry, wl_registry::WlRegistry};
use wayland_client::{Connection, Dispatch, QueueHandle};

struct WindowData {
    app_id: String,
    title: String,
}

struct DwlState {
    current_window: Option<WindowData>,
    active: bool,
}

impl DwlState {
    fn new() -> Self {
        Self {
            current_window: None,
            active: false,
        }
    }
}

impl Dispatch<WlRegistry, ()> for DwlState {
    fn event(
        _state: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "zdwl_ipc_manager_v2" => {
                    registry.bind::<ZdwlIpcManagerV2, _, _>(name, version, qh, ());
                }
                "wl_output" => {
                    registry.bind::<WlOutput, _, _>(name, version, qh, ());
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<ZdwlIpcManagerV2, ()> for DwlState {
    fn event(
        _: &mut Self,
        _: &ZdwlIpcManagerV2,
        _: <ZdwlIpcManagerV2 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZdwlIpcOutputV2, ()> for DwlState {
    fn event(
        state: &mut Self,
        _: &ZdwlIpcOutputV2,
        event: ZdwlIpcOutputEvent,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ZdwlIpcOutputEvent::Active { active } => {
                state.active = active != 0;
            }
            ZdwlIpcOutputEvent::Title { title } => {
                state
                    .current_window
                    .get_or_insert(WindowData {
                        app_id: String::new(),
                        title: String::new(),
                    })
                    .title = title;
            }
            ZdwlIpcOutputEvent::Appid { appid } => {
                state
                    .current_window
                    .get_or_insert(WindowData {
                        app_id: String::new(),
                        title: String::new(),
                    })
                    .app_id = appid;
            }
            ZdwlIpcOutputEvent::Frame => { /* Optional handling */ }
            _ => {}
        }
    }
}

impl Dispatch<WlOutput, ()> for DwlState {
    fn event(
        _: &mut Self,
        _: &WlOutput,
        _: <WlOutput as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for DwlState {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

pub struct WindowWatcher {
    connection: WlEventConnection<DwlState>,
    toplevel_state: DwlState,
}

impl WindowWatcher {
    async fn send_active_window(&self, client: &Arc<ReportClient>) -> anyhow::Result<()> {
        if self.toplevel_state.active {
            if let Some(window) = &self.toplevel_state.current_window {
                client
                    .send_active_window(&window.app_id, &window.title)
                    .await?;
            }
        }
        Ok(())
    }
}

#[async_trait]
impl Watcher for WindowWatcher {
    async fn new(_: &Arc<ReportClient>) -> anyhow::Result<Self> {
        let mut connection: WlEventConnection<DwlState> = WlEventConnection::connect()?;
        let qh: QueueHandle<DwlState> = connection.queue_handle.clone();
        let ipc_manager: ZdwlIpcManagerV2 = connection.get_dwl_ipc_manager()?;

        connection.event_queue.roundtrip(&mut DwlState::new())?;

        let wl_output: WlOutput =
            connection
                .globals
                .bind::<WlOutput, _, _>(&qh, 1..=4, ())?;
        let _output: ZdwlIpcOutputV2 = ipc_manager.get_output(&wl_output, &qh, ());

        Ok(WindowWatcher {
            connection,
            toplevel_state: DwlState {
                current_window: None,
                active: false,
            },
        })
    }

    async fn run_iteration(&mut self, client: &Arc<ReportClient>) -> anyhow::Result<()> {
        self.connection
            .event_queue
            .roundtrip(&mut self.toplevel_state)?;
        self.send_active_window(client).await
    }
}
