# yabai-rs

A fast macOS tiling window manager written in Rust, inspired by [AeroSpace](https://github.com/nikitabobko/AeroSpace).

## Features

| Feature | Status |
|---------|--------|
| BSP tiling layout | ✅ |
| Accordion (stacked) layout | ✅ |
| Named workspaces (1–9+) | ✅ |
| Focus navigation (h/j/k/l) | ✅ |
| Window movement between tiles | ✅ |
| Move window to workspace | ✅ |
| Fullscreen toggle | ✅ |
| IPC via Unix socket | ✅ |
| CLI client (`yabai-rs-msg`) | ✅ |
| TOML config file | ✅ |
| macOS Accessibility API integration | 🚧 |
| Global hotkey daemon | 🚧 |
| Multi-display support | 🚧 |

## Architecture

```
yabai-rs/
├── src/
│   ├── main.rs          — entry point, CLI args, config loading
│   ├── config.rs        — TOML config structs + defaults
│   ├── ipc/
│   │   └── mod.rs       — Unix-socket IPC server + command parser
│   ├── layout/
│   │   ├── mod.rs       — shared types (Gaps, Rect)
│   │   ├── tree.rs      — BSP tree: insert/remove/navigate windows
│   │   ├── tiles.rs     — recursive tiling layout calculator
│   │   └── accordion.rs — stacked accordion layout calculator
│   └── wm/
│       ├── mod.rs       — WindowManager: orchestrates everything
│       ├── display.rs   — display enumeration (CGDisplay on macOS)
│       ├── window.rs    — window list + frame control (AX API on macOS)
│       └── workspace.rs — per-workspace tree + layout dispatch
└── src/bin/
    └── msg.rs           — yabai-rs-msg CLI client
```

## Getting Started

### Build

```bash
cargo build --release
```

Binaries land in `target/release/`:
- `yabai-rs` — the daemon
- `yabai-rs-msg` — the IPC client

### Install

```bash
cargo install --path .
```

### Config

Copy the example config:

```bash
mkdir -p ~/.config/yabai-rs
cp examples/yabai-rs.toml ~/.config/yabai-rs/yabai-rs.toml
```

Edit it to taste — full option reference is in [`examples/yabai-rs.toml`](examples/yabai-rs.toml).

### Run

```bash
# Start the daemon (reads ~/.config/yabai-rs/yabai-rs.toml)
yabai-rs

# Or specify a config path
yabai-rs --config /path/to/yabai-rs.toml
```

To start automatically at login, add a `launchd` plist:

```xml
<!-- ~/Library/LaunchAgents/com.user.yabai-rs.plist -->
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "...">
<plist version="1.0">
<dict>
  <key>Label</key>           <string>com.user.yabai-rs</string>
  <key>ProgramArguments</key>
  <array>
    <string>/usr/local/bin/yabai-rs</string>
  </array>
  <key>RunAtLoad</key>       <true/>
  <key>KeepAlive</key>       <true/>
</dict>
</plist>
```

```bash
launchctl load ~/Library/LaunchAgents/com.user.yabai-rs.plist
```

## IPC commands

Send any command with `yabai-rs-msg`:

```bash
yabai-rs-msg focus left
yabai-rs-msg focus right
yabai-rs-msg move left
yabai-rs-msg workspace 3
yabai-rs-msg move-node-to-workspace 2
yabai-rs-msg layout tiles
yabai-rs-msg layout accordion
yabai-rs-msg fullscreen
yabai-rs-msg close
yabai-rs-msg reload
yabai-rs-msg list
```

The socket lives at `/tmp/yabai-rs.sock`. You can also `nc -U /tmp/yabai-rs.sock` for
interactive testing.

## Default keybindings

| Key | Action |
|-----|--------|
| `Alt + h/j/k/l` | Focus window left/down/up/right |
| `Alt + Shift + h/j/k/l` | Move window left/down/up/right |
| `Alt + 1–9` | Switch to workspace |
| `Alt + Shift + 1–9` | Move window to workspace |
| `Alt + /` | Toggle tiles layout |
| `Alt + ,` | Toggle accordion layout |
| `Alt + f` | Fullscreen |
| `Alt + Shift + q` | Close focused window |

All keybindings are fully customizable in `yabai-rs.toml`.

## macOS Permissions

yabai-rs needs **Accessibility** access to control windows:

> System Settings → Privacy & Security → Accessibility → enable yabai-rs

## Differences from AeroSpace

- Written in Rust → lower latency, smaller binary, no JVM/Swift runtime
- IPC via Unix socket (compatible with shell scripts, `yabai-rs-msg`, and any language)
- Config is plain TOML — no domain-specific language
- Hotkey daemon is pluggable (use `skhd`, `karabiner`, or the built-in handler)
