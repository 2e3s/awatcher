use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

#[derive(Debug)]
pub struct Tray {
    pub server_host: String,
    pub server_port: u32,
    pub config_file: PathBuf,
    pub is_stopped: Arc<AtomicBool>,
}

impl ksni::Tray for Tray {
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![ksni::Icon {
            width: 128,
            height: 128,
            data: include_bytes!("./logo.argb32").to_vec(),
        }]
    }

    fn title(&self) -> String {
        "Awatcher".into()
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            ksni::menu::StandardItem {
                label: "ActivityWatch".into(),
                // https://specifications.freedesktop.org/icon-naming-spec/icon-naming-spec-latest.html
                icon_name: "document-properties".into(),
                activate: {
                    let url = format!("http://{}:{}", self.server_host, self.server_port);

                    Box::new(move |_| {
                        open::that(&url).unwrap();
                    })
                },
                ..Default::default()
            }
            .into(),
            ksni::menu::StandardItem {
                label: "Configuration".into(),
                icon_name: "preferences-other".into(),
                activate: {
                    let config_file = self.config_file.clone().into_os_string();

                    Box::new(move |_| {
                        open::that(&config_file).unwrap();
                    })
                },
                ..Default::default()
            }
            .into(),
            ksni::menu::StandardItem {
                label: "Exit".into(),
                icon_name: "application-exit".into(),
                activate: {
                    let is_stopped = Arc::clone(&self.is_stopped);

                    Box::new(move |_| {
                        is_stopped.store(true, Ordering::Relaxed);
                    })
                },
                ..Default::default()
            }
            .into(),
        ]
    }
}
