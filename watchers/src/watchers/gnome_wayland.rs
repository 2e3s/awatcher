// The extension may not be loaded and available right away in Gnome, this mod will retry a few times.
pub trait GnomeWatcher {
    async fn load() -> anyhow::Result<Self>
    where
        Self: Sized;
}

fn is_gnome() -> bool {
    if let Ok(de) = std::env::var("XDG_CURRENT_DESKTOP") {
        de.to_lowercase().contains("gnome")
    } else {
        false
    }
}

fn is_wayland() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok()
        && !std::env::var("XDG_SESSION_TYPE")
            .unwrap_or("".into())
            .to_lowercase()
            .contains("x11")
}

pub async fn load_watcher<T: GnomeWatcher>() -> anyhow::Result<T> {
    if is_gnome() && is_wayland() {
        let mut watcher = Err(anyhow::anyhow!(""));
        for _ in 0..3 {
            watcher = T::load().await;
            if watcher.is_err() {
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
            }
        }

        watcher
    } else {
        T::load().await
    }
}
