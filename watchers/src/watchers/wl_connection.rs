use anyhow::Context;
use wayland_client::{
    globals::{registry_queue_init, GlobalList, GlobalListContents},
    protocol::{wl_registry, wl_seat::WlSeat},
    Connection, Dispatch, EventQueue, Proxy, QueueHandle,
};

use wayland_protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1::ExtIdleNotificationV1, ext_idle_notifier_v1::ExtIdleNotifierV1,
};
use wayland_protocols_plasma::idle::client::{
    org_kde_kwin_idle::OrgKdeKwinIdle, org_kde_kwin_idle_timeout::OrgKdeKwinIdleTimeout,
};
use wayland_protocols_wlr::foreign_toplevel::v1::client::zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1;

use super::wl_bindings::zdwl_ipc::zdwl_ipc_manager_v2::ZdwlIpcManagerV2;

macro_rules! subscribe_state {
    ($struct_name:ty, $data_name:ty, $state:ty) => {
        impl Dispatch<$struct_name, $data_name> for $state {
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
pub(crate) use subscribe_state;

pub struct WlEventConnection<T> {
    pub globals: GlobalList,
    pub event_queue: EventQueue<T>,
    pub queue_handle: QueueHandle<T>,
}

impl<T> WlEventConnection<T>
where
    T: Dispatch<wl_registry::WlRegistry, GlobalListContents>
        + Dispatch<wl_registry::WlRegistry, ()>
        + 'static,
{
    pub fn connect() -> anyhow::Result<Self> {
        let connection = Connection::connect_to_env()
            .with_context(|| "Unable to connect to Wayland compositor")?;
        let display = connection.display();
        let (globals, event_queue) = registry_queue_init::<T>(&connection)?;

        let queue_handle = event_queue.handle();

        let _registry = display.get_registry(&queue_handle, ());

        Ok(Self {
            globals,
            event_queue,
            queue_handle,
        })
    }

    pub fn get_foreign_toplevel_manager(&self) -> anyhow::Result<ZwlrForeignToplevelManagerV1>
    where
        T: Dispatch<ZwlrForeignToplevelManagerV1, ()>,
    {
        self.globals
            .bind::<ZwlrForeignToplevelManagerV1, T, ()>(
                &self.queue_handle,
                1..=ZwlrForeignToplevelManagerV1::interface().version,
                (),
            )
            .map_err(std::convert::Into::into)
    }

    pub fn get_dwl_ipc_manager(&self) -> anyhow::Result<ZdwlIpcManagerV2>
    where
        T: Dispatch<ZdwlIpcManagerV2, ()>,
    {
        self.globals
            .bind::<ZdwlIpcManagerV2, T, ()>(
                &self.queue_handle,
                1..=ZdwlIpcManagerV2::interface().version,
                (),
            )
            .map_err(std::convert::Into::into)
    }

    pub fn get_kwin_idle(&self) -> anyhow::Result<OrgKdeKwinIdle>
    where
        T: Dispatch<OrgKdeKwinIdle, ()>,
    {
        self.globals
            .bind::<OrgKdeKwinIdle, T, ()>(
                &self.queue_handle,
                1..=OrgKdeKwinIdle::interface().version,
                (),
            )
            .map_err(std::convert::Into::into)
    }

    pub fn get_ext_idle(&self) -> anyhow::Result<ExtIdleNotifierV1>
    where
        T: Dispatch<ExtIdleNotifierV1, ()>,
    {
        self.globals
            .bind::<ExtIdleNotifierV1, T, ()>(
                &self.queue_handle,
                1..=ExtIdleNotifierV1::interface().version,
                (),
            )
            .map_err(std::convert::Into::into)
    }

    pub fn get_ext_idle_notification(&self, timeout: u32) -> anyhow::Result<ExtIdleNotificationV1>
    where
        T: Dispatch<ExtIdleNotifierV1, ()>
            + Dispatch<WlSeat, ()>
            + Dispatch<ExtIdleNotificationV1, ()>,
    {
        let seat: WlSeat =
            self.globals
                .bind(&self.queue_handle, 1..=WlSeat::interface().version, ())?;

        let idle = self.get_ext_idle()?;
        Ok(idle.get_idle_notification(timeout, &seat, &self.queue_handle, ()))
    }

    pub fn get_kwin_idle_timeout(&self, timeout: u32) -> anyhow::Result<OrgKdeKwinIdleTimeout>
    where
        T: Dispatch<OrgKdeKwinIdle, ()>
            + Dispatch<OrgKdeKwinIdleTimeout, ()>
            + Dispatch<WlSeat, ()>,
    {
        let seat: WlSeat =
            self.globals
                .bind(&self.queue_handle, 1..=WlSeat::interface().version, ())?;

        let idle = self.get_kwin_idle()?;
        Ok(idle.get_idle_timeout(&seat, timeout, &self.queue_handle, ()))
    }
}
