# Awatcher
[![Check](https://github.com/2e3s/awatcher/actions/workflows/verify.yml/badge.svg)](https://github.com/2e3s/awatcher/actions/workflows/verify.yml)
[![Dependency Status](https://deps.rs/repo/github/2e3s/awatcher/status.svg)](https://deps.rs/repo/github/2e3s/awatcher)

Awatcher is a window activity and idle watcher with an optional tray and UI for statistics.
The goal is to compensate the fragmentation of desktop environments on Linux by supporting all reportable environments, 
to add more flexibility to reports with filters, and to have better UX with the distribution by a single executable.

The foundation is [ActivityWatch](https://github.com/ActivityWatch), which includes the provided server and web UI.
The unbundled watcher is supposed to replace the original idle and active window watchers from the original distribution.
The bundled executable can be used independently as it contains the server, UI and tray.

The binaries for the bundle, bundled DEB/RPM and ActivityWatch watchers replacement can be downloaded from
[releases](https://github.com/2e3s/awatcher/releases).
At this moment, neither Flatpak nor AppImage support quering Wayland activity.

### Module for ActivityWatch

- Run `sudo unzip aw-awatcher.zip -d /usr/local/bin` in the console to allow ActivityWatch to detect its presence.
  - Or install the provided **aw-awatcher_\*.deb** or **aw-awatcher_\*.rpm**.
- Remove `aw-watcher-window` and `aw-watcher-afk` from autostart at `aw-qt/aw-qt.toml` in [config directory](https://docs.activitywatch.net/en/latest/directories.html#config),
  add `aw-awatcher`.
- Restart ActivityWatch. In the Modules submenu there should be a new checked module **aw-awatcher**. Note that awatcher shows up in the Web UI under Timeline as `aw-watcher-window_$HOSTNAME`.
- Optionally, you can use systemd instead of ActivityWatch runner. In this case, skip adding `aw-awatcher` to `aw-qt.toml` and install this service [configuration](https://github.com/2e3s/awatcher/blob/main/config/aw-awatcher.service). In this case, ActivityWatch server also must be managed by systemd (as `aw-server.service` in the config).

### Bundle with built-in ActivityWatch

This is a single binary to run **awatcher** with the server without changing system and ActivityWatch configuration.
The bundle is **aw-server-rust** and **awatcher** as a single executable.
The data storage is compatible with ActivityWatch and **aw-server-rust** (**aw-server** has a different storage), so this can later be run as a module for ActivityWatch.

External modules are run like in the original ActivityWatch distribution
by looking at `$PATH` and running all executables whose name starts with `aw-`.
They are controled from the tray, no additional configuration is necessary.

#### Autostart

It is recommended to use `~/.config/autostart` for the bundle. This folder is employed by "Autostart" in KDE settings and Gnome Tweaks.
Systemd may require to sleep for a few seconds (`ExecStartPre=/bin/sleep 5`) in order to wait for the environment.
See this service [configuration](https://github.com/2e3s/awatcher/blob/main/config/awatcher.service).

## Supported environments

ActivityWatch server should be run before `awatcher` is running.
At this moment only Linux is supported. The watcher type is selected automatically
as soon as the environment has the necessary interfaces.

| Environment     | Active window        | Idle                |
| --------------- | -------------------- | ------------------- |
| X11             | :green_circle:       | :green_circle:      |
| Sway, Hyprland  | :green_circle: [^1]  | :green_circle: [^2] |
| Wayland + KDE   | :yellow_circle: [^3] | :green_circle:      |
| Wayland + Gnome | :yellow_circle: [^4] | :green_circle:      |

> [!IMPORTANT]
> Gnome watcher in Wayland requires [this extension](https://extensions.gnome.org/extension/5592/focused-window-d-bus/) to be installed.
> Also, if you have problems with tray icons in Gnome, you may try [this extension](https://extensions.gnome.org/extension/615/appindicator-support/) for the bundle (StatusNotifierItem specification).

[^1]: A few other DEs besides Sway may implement [wlr foreign toplevel protocol](https://wayland.app/protocols/wlr-foreign-toplevel-management-unstable-v1),
[^2]: [KWin idle](https://wayland.app/protocols/kde-idle) and [Idle notify](https://wayland.app/protocols/ext-idle-notify-v1) protocols are supported.
[^3]: KWin doesn't implement any toplevel protocol yet, KWin script is utilized instead (builtin, no actions required).
      KDE partially supports XWayland, but inconsistently, hence X11 is not utilized for it.
[^4]: Gnome doesn't implement any toplevel protocol yet, so [this extension](https://extensions.gnome.org/extension/5592/focused-window-d-bus/) should be installed.

## Configuration

The config file is in the default directory (`~/.config/awatcher`).
```toml
[server]
port = 5600
host = "127.0.0.1"

[awatcher]
idle-timeout-seconds=180
poll-time-idle-seconds=4
poll-time-window-seconds=1
disable-idle-watcher=false
disable-window-watcher=false

[[awatcher.filters]]
# match only "navigator"
match-app-id = "navigator"
# match any title which contains "Secret" or "secret" 
match-title = ".*[sS]ecret.*"
replace-app-id = "firefox"
replace-title = "Unknown"
```

- `server.port` and `server.host` address the ActivityWatch server instance.
- `awatcher.idle-timeout-seconds` is the time of inactivity when it is considered "idle".
- `awatcher.poll-time-idle-seconds` and `awatcher.poll-time-window-seconds` are 
  intervals between collecting and sending statistics.

All options of `server` and `awatcher` config file's sections can be overridden with command-line arguments, as well as the config path. See the builtin help in the command for details.

### Filters

`awatcher.filters` in the config file is an array of filters and replacements 
for the cases when the application name or title should be hidden, or the app is reported incorrectly.
Copy the section as many times as needed for every given filter.
  - `match-app-id` matches the application name.
  - `match-title` matches the title name.
  - `replace-app-id` replaces the application name with the provided value.
  - `replace-title` replaces the window title with the provided value.

The first matching filter stops the replacement.
There should be at least 1 match field for a filter to be valid.
If the replacement is not specified, the data is not reported when matched.
Matches are case sensitive regular expressions between implicit ^ and $:
- `.` matches 1 any character
- `.*` matches any number of any characters
- `.+` matches 1 or more any characters.
- `word` is an exact match.
- Use escapes `\` to match special characters, e.g. `org\.kde\.Dolphin`

#### Captures

The replacements in filters also support regexp captures.
A capture takes a string in parentheses from the match and replaces `$N` in the replacement.
Example to remove the changed file indicator in Visual Studio Code:
- Before: "● file_config.rs - awatcher - Visual Studio Code"
- After: "file_config.rs - awatcher - Visual Studio Code"
```toml
[[awatcher.filters]]
match-app-id = "code"
match-title = "● (.*)"
# Inserts the content within 1st parentheses, this can be in any form, e.g. "App $1 - $2/$3"
replace-title = "$1"
```

#### Debugging app-id and title

Run the command with "debug" or "trace" verbosity and without reporting to server in the terminal
to see what application names and titles are reported to the server.
```
$ awatcher -vvv --no-server
```

## Build

### Prerequisites

Names of packages are from Ubuntu, other distributions may have different names.

- Rust stable or nightly (for the bundle) toolchain
- pkg-config
- libssl-dev
- libdbus-1-dev (for the bundled version)
- build-essential

### Compile

- `cargo build --release` in the root of the repository.
- The target file will be located at `target/release/awatcher`.

Add `--no-default-features` to the build command if you want to opt out of the Gnome and KDE support,
add `--features=?` ("gnome" or "kwin_window") on top of that if you want to enable just one.

To track your activities in browsers install the plugin for your browser from 
[here](https://github.com/ActivityWatch/aw-watcher-web) (Firefox, Chrome etc).

#### Compile with bundle

The executable can be bundled with a tray icon, ActivityWatch server and, optionally, Web UI (if steps 1-2 are done):

1. Clone and follow the instruction in [ActivityWatch/aw-webui@839366e](https://github.com/ActivityWatch/aw-webui/commit/839366e66f859faadd7f9128de3bea14b25ce4ae)
to build the "dist" folder, 
1. Build the executable with `AW_WEBUI_DIR=/absolute/path/to/dist` and `--features=bundle`.

This should be compiled on nightly. The complete bundled version is also built and released.

The tray can be disabled with `--no-tray` option in the bundled version.
