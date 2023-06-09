#![forbid(improper_ctypes, unsafe_op_in_unsafe_fn)]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![cfg_attr(rustfmt, rustfmt_skip)]

pub mod idle {
    #![allow(dead_code,non_camel_case_types,unused_unsafe,unused_variables)]
    #![allow(non_upper_case_globals,non_snake_case,unused_imports)]
    #![allow(missing_docs, clippy::all)]
    #![allow(clippy::wildcard_imports)]

    //! Client-side API of this protocol
    use wayland_client;
    use wayland_client::protocol::*;

    pub mod __interfaces {
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("src/watchers/wl-protocols/idle.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_client_code!("src/watchers/wl-protocols/idle.xml");
}

pub mod wlr_foreign_toplevel {
    #![allow(dead_code,non_camel_case_types,unused_unsafe,unused_variables)]
    #![allow(non_upper_case_globals,non_snake_case,unused_imports)]
    #![allow(missing_docs, clippy::all)]
    #![allow(clippy::wildcard_imports)]

    //! Client-side API of this protocol
    use wayland_client;
    use wayland_client::protocol::*;

    pub mod __interfaces {
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("src/watchers/wl-protocols/wlr-foreign-toplevel-management-unstable-v1.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_client_code!("src/watchers/wl-protocols/wlr-foreign-toplevel-management-unstable-v1.xml");
}
