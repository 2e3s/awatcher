#[derive(Debug)]
pub struct Tray {
    pub server_host: String,
    pub server_port: u32,
}

impl ksni::Tray for Tray {
    fn icon_name(&self) -> String {
        // Taken from https://specifications.freedesktop.org/icon-naming-spec/icon-naming-spec-latest.html
        "appointment-new".into()
    }
    fn title(&self) -> String {
        "Awatcher".into()
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            ksni::menu::StandardItem {
                label: "Open".into(),
                icon_name: "document-properties".into(),
                activate: {
                    let url = format!("http://{}:{}", self.server_host, self.server_port);

                    Box::new(move |_| {
                        webbrowser::open(&url).unwrap();
                    })
                },
                ..Default::default()
            }
            .into(),
            ksni::menu::StandardItem {
                label: "Exit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|_| {
                    std::process::exit(0);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}
