use std::path::PathBuf;

use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug)]
pub struct Tray {
    server_host: String,
    server_port: u32,
    config_file: PathBuf,
    shutdown_sender: UnboundedSender<()>,
    watchers: Vec<String>,
}

impl Tray {
    pub fn new(
        server_host: String,
        server_port: u32,
        config_file: PathBuf,
        shutdown_sender: UnboundedSender<()>,
        watchers: Vec<String>,
    ) -> Self {
        Self {
            server_host,
            server_port,
            config_file,
            shutdown_sender,
            watchers,
        }
    }
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
        let mut watchers_submenu: Vec<ksni::MenuItem<Self>> = vec![
            ksni::menu::CheckmarkItem {
                label: "Idle".into(),
                enabled: false,
                checked: true,
                activate: Box::new(|_this: &mut Self| {}),
                ..Default::default()
            }
            .into(),
            ksni::menu::CheckmarkItem {
                label: "Window".into(),
                enabled: false,
                checked: true,
                ..Default::default()
            }
            .into(),
        ];
        for watcher in &self.watchers {
            watchers_submenu.push(
                ksni::menu::CheckmarkItem {
                    label: watcher.clone(),
                    enabled: false,
                    checked: true,
                    ..Default::default()
                }
                .into(),
            );
        }

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
            ksni::menu::SubMenu {
                label: "Watchers".into(),
                submenu: watchers_submenu,
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
                    let shutdown_sender = self.shutdown_sender.clone();

                    Box::new(move |_| {
                        shutdown_sender.send(()).unwrap();
                    })
                },
                ..Default::default()
            }
            .into(),
        ]
    }
}
