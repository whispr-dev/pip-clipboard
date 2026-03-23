# pip-clipboard Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Planned
- Guard mode: optional `Ctrl+V` interception with per-app exclusion list
- Clipboard history ring with search and dedupe
- Per-app profiles and exclusions
- Blur-on-screen-share toggle
- Config file for corner, size, opacity, show duration
- Proper app icons (replacing generated placeholders)

---

## [0.2.2] - 2025-03-23

### Added
- **Auto-start on login** via `tauri-plugin-autostart` — toggle from the system tray menu. Uses Windows Registry (`HKCU\...\Run`), macOS LaunchAgent, or XDG autostart depending on platform.
- **Close-to-tray** — closing the window or pressing Alt+F4 keeps the app running in the system tray. The "Quit pip-clip" tray menu item is now the only way to exit.
- System tray menu item for toggling auto-start on/off.
- GitHub Actions CI workflow for manual-trigger cross-platform builds (Windows x64, Linux x64, macOS ARM, macOS Intel).

### Fixed
- **WebView2 page state destroyed on hide/show** — the "Hmmm... can't reach this" error. The window is now always visible with a transparent background; CSS handles all content visibility. Transparent pixels are click-through on Windows, so the window doesn't interfere when idle.
- **Exit error** (`Failed to unregister class Chrome_WidgetWin_0`) — replaced `std::process::exit(0)` with `app.exit(0)` for clean WebView2 teardown.
- Deprecated `.menu_on_left_click()` → `.show_menu_on_left_click()` (suppressed with `#[allow(deprecated)]` for cross-version compat).
- Ambient toasts no longer steal focus from the active application.

### Changed
- `confirm_paste` and `cancel_paste` commands no longer hide the window — they resize it back to compact toast dimensions instead.
- Removed the `hide_ambient` command entirely; ambient toast lifecycle is now pure CSS.

### Technical Details
- Window is permanently visible at the OS level. All show/hide transitions are CSS `opacity` + `display:none`. This is the only reliable approach on Windows where WebView2 loses its navigation state when the host HWND is hidden.
- `PREVIEW_ACTIVE` AtomicBool prevents the clipboard watcher from emitting events while the safe-paste overlay is open.

---

## [0.2.0] - 2025-03-23

### Added
- Full **Tauri v2** migration from v1 — new config schema, plugin-based global shortcuts, capability permissions, updated tray and menu APIs.
- **Ambient clipboard toast** — brief translucent notification in the bottom-right corner whenever clipboard content changes. Shows text preview or scaled image thumbnail with size badge.
- **Safe-paste preview overlay** — press `Ctrl+Shift+V` to see a persistent preview of clipboard contents. Confirm with Enter, Ctrl+V, or click; cancel with Escape or window blur.
- Multi-modal confirmation: keyboard (Enter, Ctrl+V), mouse (click content or Paste button), and dismissal (Escape, click Cancel/✕, or click away).
- Image clipboard support with downscaled PNG thumbnails encoded as data-URLs.
- Size badge showing line count / byte size of clipboard contents.
- System tray with Toggle HUD, Pause Monitoring, and Quit actions.
- `get_clipboard_now` command for on-demand clipboard reads from the webview.

### Fixed
- Missing `build.rs` (Tauri won't compile without it).
- Removed old lowercase `cargo.toml` (Cargo requires uppercase `Cargo.toml`).
- Fixed deprecated `PngEncoder::encode()` → `write_image()` with correct `ColorType` enum.
- Fixed unnecessary full image clone (`img.bytes.clone()` on `Cow<[u8]>` → `.to_vec()`).
- Empty tray icon path — now points to `icons/icon.png` with `.ico` for Windows.
- CSP updated to include `data:` scheme for inline base64 image previews.
- Added `skipTaskbar: true` in window config.

### Changed
- Upgraded from `enigo 0.1.3` to `0.2` — new builder-pattern API with `Direction::Press/Click/Release`.
- Global shortcut registration uses Tauri v2 plugin pattern (handler on builder, registration in `setup()` via `GlobalShortcutExt`).
- Window type changed from `tauri::Window` to `tauri::WebviewWindow`.
- Event emission uses `app.emit()` (Tauri v2 emits to all windows by default).
- `Emitter` trait must be imported explicitly.

### Technical Details
- Architecture: Rust backend (clipboard polling, fingerprinting, keystroke injection) + HTML/JS/CSS frontend via Tauri WebView.
- Clipboard change detection via `DefaultHasher` fingerprints with sampled bytes for images (~64 evenly-spaced samples to avoid hashing megabytes of RGBA).
- `tauri.conf.json` follows flat Tauri v2 schema — top-level `identifier`, `app.windows`, `build.frontendDist`.
- Tauri v2 capabilities file (`capabilities/default.json`) replaces v1's `allowlist`.

---

## [0.1.0] - 2025-09-08

**Initial development release** — scaffolded by Petey (ChatGPT).

### Added
- Tauri v1 project skeleton with Rust backend and HTML/CSS/JS frontend.
- Clipboard monitoring via `arboard` with text and image support.
- Global hotkey (`Ctrl+Shift+V`) for safe-paste confirmation dialog.
- Keystroke injection via `enigo` to paste into the previously-focused app.
- System tray with toggle HUD, pause monitoring, and quit.
- Basic glass-morphism dark theme with CSS transitions.

### Known Issues (at release)
- Missing `build.rs` file.
- `enigo 0.1.3` API incompatible with current releases.
- Deprecated `PngEncoder::encode()` call.
- Empty system tray icon path.
- `confirm_paste` could re-trigger global shortcut (infinite loop potential).
- CSP missing `data:` scheme for image previews.

---

## Roadmap

### ✅ Phase 1: Foundation (COMPLETE)
- [x] Tauri v2 Rust + WebView app structure
- [x] Live clipboard monitoring (text + images)
- [x] Ambient toast notifications on clipboard change
- [x] Safe-paste preview with multi-modal confirm/cancel
- [x] System tray with pause/quit
- [x] Cross-platform keystroke injection

### ✅ Phase 2: Background Operation (COMPLETE)
- [x] Close-to-tray behaviour
- [x] Auto-start on login
- [x] CI/CD with cross-platform builds

### 🚧 Phase 3: Guard Mode (PLANNED)
- [ ] Optional `Ctrl+V` interception (opt-in)
- [ ] Per-app exclusion list (by process name / window title)
- [ ] Distinct HUD border colour when guard is active
- [ ] Smart guard: skip preview for same-content repeat pastes

### 📋 Phase 4: History & Config (PLANNED)
- [ ] Clipboard history ring buffer (configurable depth, dedupe)
- [ ] History search and scrollback in the UI
- [ ] TOML config file for corner, size, opacity, duration
- [ ] Per-app profiles

### 📋 Phase 5: Privacy & Polish (FUTURE)
- [ ] Blur previews when screen sharing is detected
- [ ] Proper app icons and branding
- [ ] Code signing for Windows and macOS distribution
- [ ] Wayland-specific fallback paths for Linux

---

## Statistics

**Development Timeline:** September 2025 – present  
**Current Version:** 0.2.2  
**Lines of Code:** ~1,200 (Rust + JS + CSS + HTML)  
**Primary Languages:** Rust, JavaScript, CSS  
**Dependencies:** 11 Rust crates, 0 npm packages  
**Platform Support:** Windows 10/11, Linux (X11/Wayland), macOS (ARM + Intel)
