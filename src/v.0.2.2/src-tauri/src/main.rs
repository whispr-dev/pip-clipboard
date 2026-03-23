#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use anyhow::Result;
use arboard::{Clipboard, ImageData};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use image::{ColorType, ImageEncoder};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, WindowEvent,
};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Global state
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

static STATE: Lazy<Mutex<AppState>> = Lazy::new(|| Mutex::new(AppState::default()));
static PREVIEW_ACTIVE: AtomicBool = AtomicBool::new(false);

#[derive(Default)]
struct AppState {
    paused: bool,
    last_fingerprint: Option<u64>,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Event payload
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Clone, serde::Serialize)]
struct ClipPayload {
    kind: String,
    preview_text: String,
    img_base64: Option<String>,
    raw_len: usize,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Fingerprinting
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn hash_text(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

fn hash_image(img: &ImageData) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    img.width.hash(&mut h);
    img.height.hash(&mut h);
    let bytes = img.bytes.as_ref();
    let step = bytes.len().saturating_div(64).max(1);
    for i in (0..bytes.len()).step_by(step) {
        bytes[i].hash(&mut h);
    }
    h.finish()
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Image → PNG data-URL
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn encode_png_data_url(img: &ImageData, max_dim: u32) -> Result<String> {
    let w = img.width as u32;
    let h = img.height as u32;
    let raw = img.bytes.to_vec();
    let mut rgba =
        image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(w, h, raw).unwrap_or_else(|| {
            image::ImageBuffer::from_pixel(w, h, image::Rgba([200, 200, 200, 255]))
        });

    if w.max(h) > max_dim {
        let scale = max_dim as f32 / w.max(h) as f32;
        let tw = (w as f32 * scale).round().max(1.0) as u32;
        let th = (h as f32 * scale).round().max(1.0) as u32;
        let dyn_img = image::DynamicImage::ImageRgba8(rgba);
        rgba = dyn_img
            .resize_exact(tw, th, image::imageops::Lanczos3)
            .to_rgba8();
    }

    let mut buf: Vec<u8> = Vec::new();
    let enc = image::codecs::png::PngEncoder::new(&mut buf);
    enc.write_image(rgba.as_raw(), rgba.width(), rgba.height(), ColorType::Rgba8)?;
    Ok(format!("data:image/png;base64,{}", B64.encode(&buf)))
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Read current clipboard into a payload
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn read_clipboard(max_text: usize, max_img_dim: u32) -> Option<ClipPayload> {
    let mut clip = Clipboard::new().ok()?;

    if let Ok(text) = clip.get_text() {
        if !text.is_empty() {
            let raw_len = text.len();
            let mut preview = text;
            if preview.len() > max_text {
                preview.truncate(max_text);
                preview.push_str("…");
            }
            return Some(ClipPayload {
                kind: "text".into(),
                preview_text: preview,
                img_base64: None,
                raw_len,
            });
        }
    }

    if let Ok(img) = clip.get_image() {
        let iw = img.width;
        let ih = img.height;
        let raw_len = img.bytes.len();
        let b64 = encode_png_data_url(&img, max_img_dim).ok();
        return Some(ClipPayload {
            kind: "image".into(),
            preview_text: format!("Image ({}×{})", iw, ih),
            img_base64: b64,
            raw_len,
        });
    }

    None
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Keystroke injection via enigo 0.2
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn do_paste_injection() {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};

    let Ok(mut enigo) = Enigo::new(&Settings::default()) else {
        eprintln!("[pip-clip] failed to create enigo instance");
        return;
    };

    #[cfg(target_os = "macos")]
    let modifier = Key::Meta;
    #[cfg(not(target_os = "macos"))]
    let modifier = Key::Control;

    let _ = enigo.key(modifier, Direction::Press);
    let _ = enigo.key(Key::Unicode('v'), Direction::Click);
    let _ = enigo.key(modifier, Direction::Release);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Tauri commands (called from webview via invoke)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tauri::command]
fn confirm_paste(window: tauri::WebviewWindow) -> Result<(), String> {
    PREVIEW_ACTIVE.store(false, Ordering::SeqCst);
    window.hide().map_err(|e| e.to_string())?;

    thread::spawn(|| {
        thread::sleep(Duration::from_millis(150));
        do_paste_injection();
    });

    Ok(())
}

#[tauri::command]
fn cancel_paste(window: tauri::WebviewWindow) -> Result<(), String> {
    PREVIEW_ACTIVE.store(false, Ordering::SeqCst);
    window.hide().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn get_clipboard_now() -> Option<ClipPayload> {
    read_clipboard(2000, 640)
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Show the preview overlay (called from global shortcut handler)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn show_safe_paste_preview(app: &tauri::AppHandle) {
    let payload = read_clipboard(2000, 640);

    if let Some(w) = app.get_webview_window("hud") {
        // Position bottom-right, respecting HiDPI
        if let Ok(Some(monitor)) = w.current_monitor() {
            let sz = monitor.size();
            let sf = monitor.scale_factor();
            let win_w = 440.0_f64;
            let win_h = 300.0_f64;
            let margin = 24.0_f64;
            let x = (sz.width as f64 / sf - win_w - margin).max(0.0);
            let y = (sz.height as f64 / sf - win_h - margin).max(0.0);
            let _ = w.set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }));
            let _ = w.set_size(tauri::Size::Logical(tauri::LogicalSize {
                width: win_w,
                height: win_h,
            }));
        }

        let _ = w.show();
        let _ = w.set_always_on_top(true);
        let _ = w.set_focus();
        PREVIEW_ACTIVE.store(true, Ordering::SeqCst);
        let _ = w.emit("show_preview", &payload);
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  System tray (Tauri v2 API)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn setup_tray(app: &tauri::App) -> Result<()> {
    let toggle = MenuItemBuilder::with_id("toggle_hud", "Toggle HUD").build(app)?;
    let pause = MenuItemBuilder::with_id("pause", "Pause Monitoring").build(app)?;
    let autostart = MenuItemBuilder::with_id("autostart", "Toggle Auto-start").build(app)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit pip-clip").build(app)?;

    let menu = MenuBuilder::new(app)
        .items(&[&toggle, &pause, &autostart, &sep, &quit])
        .build()?;

    // Icon from tauri.conf.json bundle config, or 1x1 fallback
    let icon = app
        .default_window_icon()
        .cloned()
        .unwrap_or_else(|| tauri::image::Image::new(&[90, 167, 255, 255], 1, 1));

    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .menu_on_left_click(true)
        .icon(icon)
        .on_menu_event(|app, event| {
            match event.id().as_ref() {
                "toggle_hud" => {
                    if let Some(w) = app.get_webview_window("hud") {
                        if w.is_visible().unwrap_or(true) {
                            let _ = w.hide();
                        } else {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                }
                "pause" => {
                    let mut st = STATE.lock();
                    st.paused = !st.paused;
                    eprintln!(
                        "[pip-clip] monitoring {}",
                        if st.paused { "paused" } else { "resumed" }
                    );
                }
                "autostart" => {
                    use tauri_plugin_autostart::ManagerExt;
                    let mgr = app.autolaunch();
                    match mgr.is_enabled() {
                        Ok(true) => {
                            let _ = mgr.disable();
                            eprintln!("[pip-clip] auto-start disabled");
                        }
                        Ok(false) => {
                            let _ = mgr.enable();
                            eprintln!("[pip-clip] auto-start enabled");
                        }
                        Err(e) => eprintln!("[pip-clip] autostart check failed: {e}"),
                    }
                }
                "quit" => std::process::exit(0),
                _ => {}
            }
        })
        .build(app)?;

    Ok(())
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Global shortcut registration (Tauri v2 plugin)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn build_shortcut_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    use tauri_plugin_global_shortcut::ShortcutState;

    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(move |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                show_safe_paste_preview(app);
            }
        })
        .build()
}

/// Register the Ctrl+Shift+V shortcut. Must be called AFTER the plugin is loaded.
fn register_safe_paste_shortcut(app: &tauri::AppHandle) -> Result<()> {
    use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

    #[cfg(target_os = "macos")]
    let accel = Shortcut::new(Some(Modifiers::META | Modifiers::SHIFT), Code::KeyV);
    #[cfg(not(target_os = "macos"))]
    let accel = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyV);

    app.global_shortcut().register(accel)?;
    Ok(())
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Passive clipboard watcher
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn spawn_watcher(app: tauri::AppHandle) {
    thread::spawn(move || {
        let mut clip = match Clipboard::new() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[pip-clip] clipboard init failed: {e}");
                return;
            }
        };

        loop {
            {
                let paused = STATE.lock().paused;
                if paused || PREVIEW_ACTIVE.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(400));
                    continue;
                }
            }

            let mut changed_payload: Option<ClipPayload> = None;

            if let Ok(text) = clip.get_text() {
                if !text.is_empty() {
                    let fp = hash_text(&text);
                    let mut st = STATE.lock();
                    if st.last_fingerprint != Some(fp) {
                        st.last_fingerprint = Some(fp);
                        let raw_len = text.len();
                        let mut preview = text;
                        if preview.len() > 300 {
                            preview.truncate(300);
                            preview.push_str("…");
                        }
                        changed_payload = Some(ClipPayload {
                            kind: "text".into(),
                            preview_text: preview,
                            img_base64: None,
                            raw_len,
                        });
                    }
                }
            } else if let Ok(img) = clip.get_image() {
                let fp = hash_image(&img);
                let mut st = STATE.lock();
                if st.last_fingerprint != Some(fp) {
                    st.last_fingerprint = Some(fp);
                    let iw = img.width;
                    let ih = img.height;
                    let raw_len = img.bytes.len();
                    let b64 = encode_png_data_url(&img, 320).ok();
                    changed_payload = Some(ClipPayload {
                        kind: "image".into(),
                        preview_text: format!("Image ({}×{})", iw, ih),
                        img_base64: b64,
                        raw_len,
                    });
                }
            }

            if let Some(p) = changed_payload {
                let _ = app.emit("clipboard_changed", p);
            }

            thread::sleep(Duration::from_millis(250));
        }
    });
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Entry point
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn main() {
    tauri::Builder::default()
        .plugin(build_shortcut_plugin())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),  // launch with --hidden flag
        ))
        .setup(|app| {
            // Start hidden
            if let Some(w) = app.get_webview_window("hud") {
                let _ = w.hide();
            }

            setup_tray(app)?;
            register_safe_paste_shortcut(&app.handle())?;
            spawn_watcher(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            // Close-to-tray: intercept the close request, hide instead of quit
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            confirm_paste,
            cancel_paste,
            get_clipboard_now
        ])
        .run(tauri::generate_context!())
        .expect("error running pip-clip-osd");
}
