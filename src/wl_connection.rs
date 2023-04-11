use super::wl_bindings;
use crate::BoxedError;
use wayland_client::{
    globals::{registry_queue_init, GlobalList, GlobalListContents},
    protocol::{wl_registry, wl_seat::WlSeat},
    Connection, Dispatch, EventQueue, Proxy, QueueHandle,
};
use wl_bindings::idle::org_kde_kwin_idle::OrgKdeKwinIdle;
use wl_bindings::idle::org_kde_kwin_idle_timeout::OrgKdeKwinIdleTimeout;

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
    pub fn connect() -> Result<Self, BoxedError> {
        let connection =
            Connection::connect_to_env().map_err(|_| "Unable to connect to Wayland compositor")?;
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

    pub fn get_idle_timeout(&self, timeout: u32) -> Result<OrgKdeKwinIdleTimeout, BoxedError>
    where
        T: Dispatch<OrgKdeKwinIdle, ()>
            + Dispatch<OrgKdeKwinIdleTimeout, ()>
            + Dispatch<WlSeat, ()>,
    {
        let seat: WlSeat =
            self.globals
                .bind(&self.queue_handle, 1..=WlSeat::interface().version, ())?;

        let idle: OrgKdeKwinIdle = self.globals.bind(
            &self.queue_handle,
            1..=OrgKdeKwinIdle::interface().version,
            (),
        )?;
        Ok(idle.get_idle_timeout(&seat, timeout, &self.queue_handle, ()))
    }
}
