# Awatcher 
[![Build Status](https://github.com/2e3s/awatcher/workflows/check/badge.svg?branch=main)](https://github.com/2e3s/awatcher/actions?query=branch%3Amain) [![Dependency Status](https://deps.rs/repo/github/2e3s/awatcher/status.svg)](https://deps.rs/repo/github/2e3s/awatcher)

Awatcher is a window activity and idle watcher with an optional tray and UI for statistics.
The goal is to compensate the fragmentation of desktop environments on Linux, 
and to add more flexibility to reports.

The server and web UI are taken from [ActivityWatch](https://github.com/ActivityWatch) project,
which has a worse support of Linux environment, with a pretty bulky distribution.
The crate also provides a library with watchers which can send statistics to the server.

## Build

### Prerequisites

- Rust stable toolchain
- `pkg-config`
- `libssl-dev`

### Compile

- `cargo build --release` in the root of the repository.
- The target file will be located at `target/release/awatcher`.

Add `--no-default-features` to the build command if you want to opt out of the Gnome and KDE support,
add `--features=?` ("gnome" or "kwin_window") on top of that if you want to enable just one.

To track your activities in browsers install the plugin for your browser from 
[here](https://github.com/ActivityWatch/aw-watcher-web) (Firefox, Chrome etc).

#### Compile with bundle

The executable can be bundled with a tray icon, ActivityWatch server and, optionally, Web UI (if steps 1-2 are done):

1. Clone and follow the instruction in [ActivityWatch/aw-webui](https://github.com/ActivityWatch/aw-webui)
to build the "dist" folder, 
1. Then zip it with `zip -r dist.zip aw-webui/dist`.
2. Build the executable with `--features=bundle`.

This should be compiled on nightly. The complete bundled version is also built and released.

Gnome needs [the extension](https://extensions.gnome.org/extension/615/appindicator-support/) to support StatusNotifierItem specification.

The tray can be disabled with `--no-tray` option in the bundled version.

## Supported environments

ActivityWatch server should be run before `awatcher` is running.
At this moment only Linux is supported. The watcher type is selected automatically
by availability of necessary interfaces in the given environment.

| Environment     | Active window        | Idle                |
| --------------- | -------------------- | ------------------- |
| X11             | :green_circle:       | :green_circle:      |
| Wayland + Sway  | :green_circle: [^1]  | :green_circle: [^2] |
| Wayland + KDE   | :yellow_circle: [^3] | :green_circle:      |
| Wayland + Gnome | :yellow_circle: [^4] | :green_circle:      |

[^1]: A few other DEs besides Sway may implement [wlr foreign toplevel protocol](https://wayland.app/protocols/wlr-foreign-toplevel-management-unstable-v1),
[^2]: It implements [KWin idle protocol](https://wayland.app/protocols/kde-idle).
[^3]: KWin doesn't implement any toplevel protocol yet, KWin script is utilized instead (builtin, no actions required).
      KDE partially supports XWayland, but inconsistently, hence X11 is not utilized for it.
[^4]: Gnome doesn't implement any toplevel protocol yet, so [this extension](https://extensions.gnome.org/extension/5592/focused-window-d-bus/) should be installed.

## Configuration

The config file is in the default directory (`~/.config/awatcher`).
```toml
[server]
port = 5600
host = "localhost"

[awatcher]
idle-timeout-seconds=180
poll-time-idle-seconds=4
poll-time-window-seconds=1

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
There should be at least 1 match field, and at least 1 replace field for a valid filter.
Matches are case sensitive regular expressions between implici ^ and $:
- `.` matches 1 any character
- `.*` matches any number of any characters
- `.+` matches 1 or more any characters.
- `word` is an exact match.
- Use escapes to match special characters, e.g. `org\.kde\.Dolpin`

Run the command with "debug" or "trace" verbosity and without reporting to server in the terminal
to see what application names and titles are reported to the server.
```
$ awatcher -vvv --no-server
```