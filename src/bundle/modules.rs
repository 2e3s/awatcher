// This repeats the functionality of aw-qt from ActivityWatch.

use serde::{Deserialize, Serialize};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

#[derive(Debug, Serialize, Deserialize, Default)]
struct Watchers {
    #[serde(default)]
    autostart: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct BundleConfig {
    watchers: Watchers,
}

pub struct ExternalWatcher {
    path: PathBuf,
    handle: Option<Child>,
}

impl ExternalWatcher {
    fn new(path: PathBuf) -> Option<Self> {
        if !path.is_file() {
            return None;
        }
        if path.metadata().ok()?.permissions().mode() & 0o111 == 0 {
            return None;
        }
        if !path.file_name()?.to_str()?.starts_with("aw-") {
            return None;
        }

        Some(Self { path, handle: None })
    }

    fn start(&mut self) -> bool {
        if self.started() {
            debug!("Watcher {} is already started", self.name());
            return true;
        }
        debug!("Starting an external watcher {}", self.name());

        let command = Command::new(&self.path).stdout(Stdio::null()).spawn();

        match command {
            Ok(handle) => {
                self.handle = Some(handle);
                true
            }
            Err(e) => {
                error!("Failed to start watcher {}: {e}", self.name());
                false
            }
        }
    }

    fn stop(&mut self) {
        self.handle = if let Some(mut handle) = self.handle.take() {
            debug!("Stopping an external watcher {}", self.name());
            if let Err(e) = handle.kill() {
                error!("Failed to kill watcher {}: {}", self.name(), e);

                Some(handle)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn started(&self) -> bool {
        self.handle.is_some()
    }

    pub fn name(&self) -> String {
        self.path.file_name().unwrap().to_string_lossy().to_string()
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
    }
}

pub struct Manager {
    config_path: PathBuf,
    config: BundleConfig,
    pub path_watchers: Vec<ExternalWatcher>,
}

impl Manager {
    pub fn new(path_env: &str, config_path: &Path) -> Self {
        let mut config_path = config_path.to_path_buf();
        config_path.push("bundle-config.toml");
        debug!("Processing bundle config at {}", config_path.display());

        let config = Self::get_config(&config_path);

        let mut path_watchers = Self::get_watchers_from_path_env(path_env);
        for watcher in &mut path_watchers {
            debug!("Found external watcher {}", watcher.name());
            let file_name = watcher.path.file_name().unwrap();
            if config
                .watchers
                .autostart
                .contains(&file_name.to_string_lossy().to_string())
            {
                watcher.start();
            } else {
                debug!(
                    "External watcher {} is not configured to autostart",
                    watcher.name()
                );
            }
        }

        Self {
            config_path,
            config,
            path_watchers,
        }
    }

    pub fn start_watcher(&mut self, watcher_path: &Path) -> bool {
        let watcher_name = if let Some(watcher) = self.get_watcher_by_path(watcher_path) {
            if watcher.start() {
                watcher.name().to_string()
            } else {
                return false;
            }
        } else {
            return false;
        };
        if !self.config.watchers.autostart.contains(&watcher_name) {
            self.config.watchers.autostart.push(watcher_name.clone());

            self.update_config_watchers();
        }
        true
    }

    pub fn stop_watcher(&mut self, watcher_path: &Path) {
        let watcher_name = if let Some(watcher) = self.get_watcher_by_path(watcher_path) {
            watcher.stop();
            Some(watcher.name().to_string())
        } else {
            None
        };
        if let Some(watcher_name) = watcher_name {
            self.config
                .watchers
                .autostart
                .retain(|check| check != &watcher_name);

            self.update_config_watchers();
        }
    }

    fn update_config_watchers(&mut self) {
        let toml_content = toml::to_string_pretty(&self.config).unwrap();
        std::fs::write(&self.config_path, toml_content).unwrap();
    }

    fn get_watcher_by_path(&mut self, watcher_path: &Path) -> Option<&mut ExternalWatcher> {
        self.path_watchers
            .iter_mut()
            .find(|watcher| watcher.path() == watcher_path)
            .or_else(|| {
                error!("Watcher is not found {}", watcher_path.display());
                None
            })
    }

    fn get_config(config_path: &Path) -> BundleConfig {
        let config_content = std::fs::read_to_string(config_path).ok();

        if let Some(content) = config_content {
            toml::from_str(&content).unwrap_or_default()
        } else {
            debug!(
                "No bundle config found at {}, creating new file",
                config_path.display()
            );
            let config = BundleConfig::default();

            let toml_content = toml::to_string_pretty(&config).unwrap();
            std::fs::write(config_path, toml_content).unwrap();

            config
        }
    }

    fn get_watchers_from_path_env(path_env: &str) -> Vec<ExternalWatcher> {
        path_env
            .split(':')
            .map(Path::new)
            .filter(|&path| path.is_dir())
            .filter_map(|path| path.read_dir().ok())
            .flat_map(Iterator::flatten)
            .map(|entry| entry.path())
            .filter_map(ExternalWatcher::new)
            .fold(Vec::new(), |mut acc, watcher| {
                if acc.iter().any(|check| check.name() == watcher.name()) {
                    warn!(
                        "Duplicate watcher {} found in PATH, not running",
                        watcher.path.display()
                    );
                } else {
                    acc.push(watcher);
                }
                acc
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};
    use std::fs::File;
    use std::io::Write;
    use tempfile::{tempdir, TempDir};

    #[test]
    fn test_get_watchers_from_path_env() {
        let dir = tempdir().unwrap();

        let path = dir.path().join("test");
        let test_file = File::create(path).unwrap();

        let path = dir.path().join("aw-test");
        let aw_test_file = File::create(path).unwrap();

        let watchers = Manager::get_watchers_from_path_env(dir.path().to_str().unwrap());
        assert_eq!(watchers.len(), 0);

        let mut permissions = test_file.metadata().unwrap().permissions();
        permissions.set_mode(0o111);
        test_file.set_permissions(permissions).unwrap();

        let mut permissions = aw_test_file.metadata().unwrap().permissions();
        permissions.set_mode(0o111);
        aw_test_file.set_permissions(permissions).unwrap();

        let watchers = Manager::get_watchers_from_path_env(dir.path().to_str().unwrap());
        assert_eq!(watchers.len(), 1);
        assert_eq!(watchers[0].name(), "aw-test");
    }

    #[rstest]
    fn test_manager(temp_dir: TempDir) {
        std::fs::write(
            temp_dir.path().join("bundle-config.toml").as_path(),
            b"[watchers]\nautostart = [\"aw-test\", \"absent\"]\n",
        )
        .unwrap();
        let mut manager = Manager::new(temp_dir.path().to_str().unwrap(), temp_dir.path());
        assert_eq!(manager.path_watchers.len(), 1);
        assert_eq!(manager.path_watchers[0].name(), "aw-test");
        assert!(manager.path_watchers[0].handle.is_some());

        assert!(!manager.start_watcher(&temp_dir.path().join("absent")));
        assert_autostart_content(&manager, &["aw-test", "absent"]);

        assert!(manager.start_watcher(&temp_dir.path().join("aw-test"))); // already started
        assert!(manager.path_watchers[0].handle.is_some());
        assert_autostart_content(&manager, &["aw-test", "absent"]);

        manager.stop_watcher(&temp_dir.path().join("absent"));
        assert_autostart_content(&manager, &["aw-test", "absent"]);

        manager.stop_watcher(&temp_dir.path().join("aw-test"));
        assert!(manager.path_watchers[0].handle.is_none());
        assert_autostart_content(&manager, &["absent"]);
    }

    #[rstest]
    fn test_broken_file(temp_dir: TempDir) {
        std::fs::write(
            temp_dir.path().join("bundle-config.toml").as_path(),
            b"[watchers]\n#autostart = [\"aw-test\"]\n",
        )
        .unwrap();
        let mut manager = Manager::new(temp_dir.path().to_str().unwrap(), temp_dir.path());
        assert_eq!(manager.path_watchers.len(), 1);
        assert_eq!(manager.path_watchers[0].name(), "aw-test");
        assert!(manager.path_watchers[0].handle.is_none()); // no starting in config

        assert!(manager.start_watcher(&temp_dir.path().join("aw-test")));
        assert!(manager.path_watchers[0].handle.is_some());
        assert_autostart_content(&manager, &["aw-test"]);
    }

    fn assert_autostart_content(manager: &Manager, watchers: &[&str]) {
        assert_eq!(manager.config.watchers.autostart, watchers);
        assert_eq!(
            std::fs::read_to_string(manager.config_path.as_path()).unwrap(),
            format!(
                "[watchers]\nautostart = [{}]\n",
                watchers
                    .iter()
                    .map(|w| format!("\"{w}\""))
                    .collect::<Vec<String>>()
                    .join(", "),
            )
        );
    }

    #[fixture]
    fn temp_dir() -> TempDir {
        let dir = tempdir().unwrap();

        create_test_watcher(dir.path());

        dir
    }

    fn create_test_watcher(bin_dir: &Path) {
        let exec_path = bin_dir.join("aw-test");
        let mut aw_test_file = File::create(exec_path).unwrap();
        // write a bash script with infinite loop and sleep into the file:
        aw_test_file
            .write_all(b"#!/bin/bash\nwhile true; do sleep 1; done")
            .unwrap();
        // set execution permissions:
        let mut permissions = aw_test_file.metadata().unwrap().permissions();
        permissions.set_mode(0o755);
        aw_test_file.set_permissions(permissions).unwrap();
        aw_test_file.flush().unwrap();
        aw_test_file.sync_all().unwrap();
    }
}
