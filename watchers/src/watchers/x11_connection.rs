use anyhow::{anyhow, bail, Context};
use log::warn;
use std::{env, str};
use x11rb::connection::Connection;
use x11rb::protocol::screensaver::ConnectionExt as ScreensaverConnectionExt;
use x11rb::protocol::xproto::{Atom, AtomEnum, ConnectionExt, GetPropertyReply, Window};
use x11rb::rust_connection::RustConnection;

pub struct WindowData {
    pub title: String,
    pub app_id: String,
    pub wm_instance: String,
}

pub struct X11Client {
    connection: RustConnection,
    screen_root: Window,
}

impl X11Client {
    pub fn new() -> anyhow::Result<Self> {
        if env::var("DISPLAY").is_err() {
            warn!("DISPLAY is not set, setting to the default value \":0\"");
            env::set_var("DISPLAY", ":0");
        }

        let (connection, screen_num) = x11rb::connect(None)?;
        let screen_root = connection.setup().roots[screen_num].root;

        Ok(X11Client {
            connection,
            screen_root,
        })
    }

    fn reconnect(&mut self) {
        match x11rb::connect(None) {
            Ok((connection, screen_num)) => {
                self.screen_root = connection.setup().roots[screen_num].root;
                self.connection = connection;
            }
            Err(e) => error!("Failed to reconnect to X11: {e}"),
        };
    }

    fn execute_with_reconnect<T>(
        &mut self,
        action: fn(&Self) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        match action(self) {
            Ok(v) => Ok(v),
            Err(_) => {
                self.reconnect();
                action(self)
            }
        }
    }

    pub fn seconds_since_last_input(&mut self) -> anyhow::Result<u32> {
        self.execute_with_reconnect(|client| {
            let reply = client
                .connection
                .screensaver_query_info(client.screen_root)?
                .reply()?;

            Ok(reply.ms_since_user_input / 1000)
        })
    }

    pub fn active_window_data(&mut self) -> anyhow::Result<Option<WindowData>> {
        self.execute_with_reconnect(|client| {
            let focus = client.find_active_window()?;

            match focus {
                Some(window) => {
                    let name = client.get_property(
                        window,
                        client.intern_atom("_NET_WM_NAME")?,
                        "_NET_WM_NAME",
                        client.intern_atom("UTF8_STRING")?,
                        u32::MAX,
                    )?;
                    let class = client.get_property(
                        window,
                        AtomEnum::WM_CLASS.into(),
                        "WM_CLASS",
                        AtomEnum::STRING.into(),
                        u32::MAX,
                    )?;

                    let title = str::from_utf8(&name.value).with_context(|| "Invalid title UTF")?;
                    let (instance, class) = parse_wm_class(&class)?;

                    Ok(Some(WindowData {
                        title: title.to_string(),
                        app_id: class,
                        wm_instance: instance,
                    }))
                }
                None => Ok(None),
            }
        })
    }

    fn get_property(
        &self,
        window: Window,
        property: Atom,
        property_name: &str,
        property_type: Atom,
        long_length: u32,
    ) -> anyhow::Result<GetPropertyReply> {
        self.connection
            .get_property(false, window, property, property_type, 0, long_length)
            .with_context(|| format!("GetPropertyRequest[{property_name}] failed"))?
            .reply()
            .with_context(|| format!("GetPropertyReply[{property_name}] failed"))
    }

    fn intern_atom(&self, name: &str) -> anyhow::Result<Atom> {
        Ok(self
            .connection
            .intern_atom(false, name.as_bytes())
            .with_context(|| format!("InternAtomRequest[{name}] failed"))?
            .reply()
            .with_context(|| format!("InternAtomReply[{name}] failed"))?
            .atom)
    }

    fn find_active_window(&self) -> anyhow::Result<Option<Window>> {
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
            let window_id = active_window
                .value32()
                .ok_or(anyhow!("Invalid message. Expected value with format = 32"))?
                .next()
                .ok_or(anyhow!("Active window is not found"))?;

            // Check if the window_id is 0 (no active window)
            if window_id == 0 {
                return Ok(None);
            }

            Ok(Some(window_id))
        } else {
            // Query the input focus
            Ok(Some(self
                .connection
                .get_input_focus()
                .with_context(|| "Failed to get input focus")?
                .reply()
                .with_context(|| "Failed to read input focus from reply")?
                .focus))
        }
    }
}

fn parse_wm_class(property: &GetPropertyReply) -> anyhow::Result<(String, String)> {
    if property.format != 8 {
        bail!("Malformed property: wrong format");
    }
    let value = &property.value;
    // The property should contain two null-terminated strings. Find them.
    if let Some(middle) = value.iter().position(|&b| b == 0) {
        let (instance, class) = value.split_at(middle);
        // Remove the null byte at the end of the instance
        let instance = &instance[..instance.len()];
        // Skip the null byte at the beginning of the class
        let mut class = &class[1..];
        // Remove the last null byte from the class, if it is there.
        if class.last() == Some(&0) {
            class = &class[..class.len() - 1];
        }
        Ok((
            std::str::from_utf8(instance)?.to_string(),
            std::str::from_utf8(class)?.to_string(),
        ))
    } else {
        bail!("Missing null byte")
    }
}
