# pip-clipboard


**Never again accidentally paste something you regret.**

pip-clipboard is a picture-in-picture clipboard preview that sits in your system tray and shows you what's on your clipboard before you paste it. Think of it as a safety net for your paste key — a small, translucent HUD that catches those moments where the clipboard contains something unexpected before it lands in a chat window, terminal, or email.

## How it works

**Ambient mode** — whenever you copy something, a translucent toast appears briefly in the bottom-right corner showing what's now on your clipboard. Text and images are both supported. The toast fades after ~3 seconds without stealing focus from whatever you're doing.

**Safe-paste mode** — press `Ctrl+Shift+V` (or `Cmd+Shift+V` on macOS) to bring up a persistent preview overlay. From there you can confirm the paste with `Enter`, `Ctrl+V`, or clicking the content, or cancel with `Escape` or clicking away. If you confirm, the app injects a native `Ctrl+V` into whatever application had focus before the preview appeared.

The window itself is always-on-top, frameless, and fully transparent when idle. It never appears in the taskbar. You interact with it through the system tray and the global hotkey.

## Features

- **Live clipboard preview** — text and images, with thumbnails scaled to fit
- **Safe-paste confirmation** — `Ctrl+Shift+V` shows what you're about to paste and lets you confirm or cancel
- **System tray** — pause monitoring, toggle auto-start, quit
- **Auto-start on login** — optional, toggled from the tray menu (Windows Registry / macOS LaunchAgent / XDG autostart)
- **Close-to-tray** — closing the window keeps the app running; quit from the tray menu
- **Cross-platform** — Windows 10/11, Linux (X11/Wayland), macOS
- **Tiny footprint** — Rust backend with a WebView frontend via Tauri; a few MB of RAM at idle

## Installation

### Pre-built binaries

Grab the latest release from the [Releases page](https://github.com/whisprer/pip-clipboard/releases).

| Platform | Format |
|----------|--------|
| Windows x64 | `.exe` standalone, `.msi` installer |
| Linux x64 | `.AppImage`, `.deb`, `.rpm` |
| macOS ARM (Apple Silicon) | `.dmg`, `.app.zip` |
| macOS Intel | `.dmg`, `.app.zip` |

### Build from source

Prerequisites: [Rust](https://rustup.rs/), [Tauri CLI v2](https://v2.tauri.app/start/prerequisites/), and platform-specific dependencies (see below).

```bash
cd src/v.0.2.2
cargo tauri dev        # development mode with hot-reload
cargo tauri build      # release build with installer
```

**Linux** requires additional system libraries:

```bash
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev \
  patchelf libxdo-dev libx11-dev libxext-dev libxtst-dev
```

**Windows** requires the Visual Studio C++ Build Tools (MSVC linker). WebView2 ships with Windows 10/11 by default.

**macOS** requires Xcode Command Line Tools. The first run will prompt for Accessibility permissions (required for keystroke injection).

## Usage

Once running, pip-clipboard lives in the system tray. There's no main window.

| Action | What happens |
|--------|-------------|
| Copy anything | Brief translucent toast shows the clipboard contents |
| `Ctrl+Shift+V` | Persistent preview overlay — confirm or cancel the paste |
| `Enter` / `Ctrl+V` / click content | Confirm paste (injects keystroke to the previous app) |
| `Escape` / click away | Cancel paste |
| Right-click tray icon | Toggle HUD, pause monitoring, toggle auto-start, quit |

### Running as a background process

pip-clipboard is not a Windows Service — it needs access to your desktop session for clipboard and keystroke injection. Instead, it runs as a normal user-session app with two features that give it service-like behaviour:

1. **Close-to-tray** — closing the window (or pressing Alt+F4) doesn't exit the app; it keeps running in the tray.
2. **Auto-start** — toggle from the tray menu. On Windows this creates a registry entry under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.

For a release build detached from any terminal:

```powershell
# Windows
Start-Process -WindowStyle Hidden ".\pip-clip-osd.exe"
```

```bash
# Linux
nohup ./pip-clip-osd &
```

## Architecture

```
src/v.0.2.2/
├── src-tauri/
│   ├── src/main.rs          # Rust backend: clipboard polling, keystroke injection,
│   │                        #   tray, global shortcut, window management
│   ├── Cargo.toml           # Rust dependencies
│   ├── tauri.conf.json      # Tauri v2 app config
│   ├── capabilities/        # Tauri v2 permission declarations
│   ├── build.rs             # Tauri build script
│   └── icons/               # App icons (.ico, .png)
├── ui/
│   ├── index.html           # Dual-mode layout (ambient toast + preview overlay)
│   ├── app.js               # Event handling, keyboard shortcuts, CSS transitions
│   └── styles.css           # Glass-morphism dark theme, all visibility via CSS
└── package.json
```

Key technical decisions:

- **Tauri v2** — small binary, native WebView, cross-platform tray and global shortcuts
- **arboard** — cross-platform clipboard access (text + images)
- **enigo 0.2** — keystroke injection for the paste confirmation flow
- **Window never hides** — WebView2 on Windows destroys page state when the host window is hidden. The window stays visible with a transparent background at all times; CSS handles all show/hide. Transparent pixels are click-through.
- **Fingerprint-based change detection** — the clipboard watcher polls at 250ms and uses a `DefaultHasher` fingerprint (with sampled bytes for images) to detect changes without full-content comparison.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## Security

See [SECURITY.md](SECURITY.md) for the security policy and how to report vulnerabilities.

## License

[MIT](LICENSE)
