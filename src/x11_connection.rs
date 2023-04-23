use std::{env, str};

use log::warn;
use x11rb::connection::Connection;
use x11rb::protocol::screensaver::ConnectionExt as ScreensaverConnectionExt;
use x11rb::protocol::xproto::{Atom, AtomEnum, ConnectionExt, GetPropertyReply, Window};
use x11rb::rust_connection::RustConnection;

use crate::BoxedError;

pub struct WindowData {
    pub title: String,
    pub app_id: String,
}

pub struct X11Connection {
    connection: RustConnection,
    screen_root: Window,
}

impl X11Connection {
    pub fn new() -> Result<Self, BoxedError> {
        if env::var("DISPLAY").is_err() {
            warn!("DISPLAY is not set, setting to the default value \":0\"");
            env::set_var("DISPLAY", ":0");
        }

        let (connection, screen_num) = x11rb::connect(None)?;
        let screen_root = connection.setup().roots[screen_num].root;

        Ok(X11Connection {
            connection,
            screen_root,
        })
    }

    pub fn seconds_since_last_input(&self) -> Result<u32, BoxedError> {
        let reply = self
            .connection
            .screensaver_query_info(self.screen_root)?
            .reply()?;

        Ok(reply.ms_since_user_input / 1000)
    }

    pub fn active_window_data(&self) -> Result<WindowData, BoxedError> {
        let focus: Window = self.find_active_window()?;

        let name = self.get_property(
            focus,
            self.intern_atom("_NET_WM_NAME")?,
            "_NET_WM_NAME",
            self.intern_atom("UTF8_STRING")?,
            u32::MAX,
        )?;
        let class = self.get_property(
            focus,
            AtomEnum::WM_CLASS.into(),
            "WM_CLASS",
            AtomEnum::STRING.into(),
            u32::MAX,
        )?;

        let title = str::from_utf8(&name.value).map_err(|e| format!("Invalid title UTF: {e}"))?;

        Ok(WindowData {
            title: title.to_string(),
            app_id: parse_wm_class(&class)?.to_string(),
        })
    }

    fn get_property(
        &self,
        window: Window,
        property: Atom,
        property_name: &str,
        property_type: Atom,
        long_length: u32,
    ) -> Result<GetPropertyReply, BoxedError> {
        self.connection
            .get_property(false, window, property, property_type, 0, long_length)
            .map_err(|e| format!("GetPropertyRequest[{property_name}] failed: {e}"))?
            .reply()
            .map_err(|e| format!("GetPropertyReply[{property_name}] failed: {e}").into())
    }

    fn intern_atom(&self, name: &str) -> Result<Atom, BoxedError> {
        Ok(self
            .connection
            .intern_atom(false, name.as_bytes())
            .map_err(|_| format!("InternAtomRequest[{name}] failed"))?
            .reply()
            .map_err(|_| format!("InternAtomReply[{name}] failed"))?
            .atom)
    }

    fn find_active_window(&self) -> Result<Window, BoxedError> {
        let window: Atom = AtomEnum::WINDOW.into();
        let net_active_window = self.intern_atom("_NET_ACTIVE_WINDOW")?;
        let active_window = self.get_property(
            self.screen_root,
            net_active_window,
            "_NET_ACTIVE_WINDOW",
            window,
            1,
        )?;

        if active_window.format == 32 && active_window.length == 1 {
            active_window
                .value32()
                .ok_or("Invalid message. Expected value with format = 32")?
                .next()
                .ok_or("Active window is not found".into())
        } else {
            // Query the input focus
            Ok(self
                .connection
                .get_input_focus()
                .map_err(|e| format!("Failed to get input focus: {e}"))?
                .reply()
                .map_err(|e| format!("Failed to read input focus from reply: {e}"))?
                .focus)
        }
    }
}

fn parse_wm_class(property: &GetPropertyReply) -> Result<&str, BoxedError> {
    if property.format != 8 {
        return Err("Malformed property: wrong format".into());
    }
    let value = &property.value;
    // The property should contain two null-terminated strings. Find them.
    if let Some(middle) = value.iter().position(|&b| b == 0) {
        let (_, class) = value.split_at(middle);
        // Skip the null byte at the beginning
        let mut class = &class[1..];
        // Remove the last null byte from the class, if it is there.
        if class.last() == Some(&0) {
            class = &class[..class.len() - 1];
        }
        Ok(std::str::from_utf8(class)?)
    } else {
        Err("Missing null byte".into())
    }
}
