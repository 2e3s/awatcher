pub mod zdwl_ipc {
    use wayland_client;
    use wayland_client::protocol::*;

    pub mod __interfaces {
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("src/watchers/wl-protocols/dwl_ipc_unstable_v2.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_client_code!("src/watchers/wl-protocols/dwl_ipc_unstable_v2.xml");
}
