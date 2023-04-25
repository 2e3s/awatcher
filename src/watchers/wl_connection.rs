use super::wl_bindings;
use anyhow::Context;
use wayland_client::{
    globals::{registry_queue_init, GlobalList, GlobalListContents},
    protocol::{wl_registry, wl_seat::WlSeat},
    Connection, Dispatch, EventQueue, Proxy, QueueHandle,
};
use wl_bindings::idle::org_kde_kwin_idle::OrgKdeKwinIdle;
use wl_bindings::idle::org_kde_kwin_idle_timeout::OrgKdeKwinIdleTimeout;
use wl_bindings::wlr_foreign_toplevel::zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1;

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
                1..=OrgKdeKwinIdle::interface().version,
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
