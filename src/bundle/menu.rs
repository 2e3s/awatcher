use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::str::FromStr;
use tokio::sync::mpsc::UnboundedSender;

use super::modules::Manager;

pub struct Tray {
    server_host: String,
    server_port: u16,
    config_file: PathBuf,
    shutdown_sender: UnboundedSender<()>,
    watchers_manager: Manager,
    checks: HashMap<PathBuf, bool>,
}

impl Tray {
    pub fn new(
        server_host: String,
        server_port: u16,
        config_file: PathBuf,
        shutdown_sender: UnboundedSender<()>,
        watchers_manager: Manager,
    ) -> Self {
        let is_zero_first_octet = match Ipv4Addr::from_str(&server_host) {
            Ok(ip) => ip.octets()[0] == 0,
            Err(_) => false,
        };

        let server_host = if is_zero_first_octet {
            "localhost".to_string()
        } else {
            server_host
        };

        let checks = watchers_manager
            .path_watchers
            .iter()
            .map(|watcher| (watcher.path().to_owned(), watcher.started()))
            .collect();

        Self {
            server_host,
            server_port,
            config_file,
            shutdown_sender,
            watchers_manager,
            checks,
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

    fn id(&self) -> String {
        "awatcher-bundle".into()
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
        for watcher in &self.watchers_manager.path_watchers {
            let path = watcher.path().to_owned();

            watchers_submenu.push(
                ksni::menu::CheckmarkItem {
                    label: watcher.name(),
                    enabled: true,
                    checked: watcher.started(),
                    activate: Box::new(move |this: &mut Self| {
                        let current_checked = *this.checks.get(&path).unwrap_or(&false);
                        this.checks.insert(path.clone(), !current_checked);
                        if current_checked {
                            this.watchers_manager.stop_watcher(&path);
                        } else {
                            this.watchers_manager.start_watcher(&path);
                        }
                    }),
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
                activate: Box::new(move |this: &mut Self| {
                    let url = format!("http://{}:{}", this.server_host, this.server_port);

                    open::that(url).unwrap();
                }),
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
                activate: Box::new(move |this: &mut Self| {
                    this.shutdown_sender.send(()).unwrap();
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}
