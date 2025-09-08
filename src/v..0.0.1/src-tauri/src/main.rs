#![cfg_attr(all(not(debug_assertions), target_os = "windows"), windows_subsystem = "windows")]

use std::{thread, time::Duration};

use anyhow::Result;
use arboard::{Clipboard, ImageData};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as b64;
use enigo::{Enigo, KeyboardControllable, Key};
use image::{ImageBuffer, Rgba};
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use tauri::{CustomMenuItem, Manager, SystemTray, SystemTrayEvent, SystemTrayMenu, Wry};

static APP_STATE: Lazy<Mutex<State>> = Lazy::new(|| Mutex::new(State::default()));

#[derive(Default)]
struct State {
    paused: bool,
    last_fingerprint: Option<u64>,
}

#[derive(serde::Serialize)]
struct ClipPayload {
    kind: String,            // "text" | "image"
    preview_text: String,    // truncated text or "Image (WxH)"
    img_base64: Option<String>, // data URL like "data:image/png;base64,..."
}

fn text_fingerprint(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

fn image_fingerprint(img: &ImageData) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    img.width.hash(&mut h);
    img.height.hash(&mut h);
    // sample a few bytes to avoid full-cost hash
    let bytes = img.bytes.as_ref();
    for i in (0..bytes.len()).step_by(bytes.len().saturating_div(64).max(1)) {
        bytes[i].hash(&mut h);
    }
    h.finish()
}

fn image_to_png_base64(img: &ImageData, max_dim: u32) -> Result<String> {
    // arboard gives bytes in BGRA or RGBA depending on platform; treat as RGBA if len matches
    // We'll try to detect stride = width*4 and assume RGBA; if colors look wrong we can swap later.
    let width = img.width as u32;
    let height = img.height as u32;
    let mut rgba = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, img.bytes.clone())
        .unwrap_or_else(|| ImageBuffer::from_pixel(width, height, Rgba([200, 200, 200, 255])));

    // scale down if needed
    let (tw, th) = if width.max(height) > max_dim {
        let scale = max_dim as f32 / (width.max(height) as f32);
        let tw = (width as f32 * scale).round().max(1.0) as u32;
        let th = (height as f32 * scale).round().max(1.0) as u32;
        (tw, th)
    } else {
        (width, height)
    };

    if (tw, th) != (width, height) {
        let img_dyn = image::DynamicImage::ImageRgba8(rgba);
        rgba = img_dyn.resize_exact(tw, th, image::imageops::Lanczos3).to_rgba8();
    }

    let mut png_bytes: Vec<u8> = Vec::new();
    {
        let mut enc = image::codecs::png::PngEncoder::new(&mut png_bytes);
        enc.encode(
            &rgba,
            rgba.width(),
            rgba.height(),
            image::ColorType::Rgba8,
        )?;
    }
    Ok(format!("data:image/png;base64,{}", b64.encode(png_bytes)))
}

#[tauri::command]
fn confirm_paste() -> Result<(), String> {
    // Hide HUD momentarily (optional). We simply simulate the paste chord.
    if cfg!(target_os = "macos") {
        let mut e = Enigo::new();
        e.key_down(Key::Meta);
        e.key_click(Key::Layout('v'));
        e.key_up(Key::Meta);
    } else {
        let mut e = Enigo::new();
        e.key_down(Key::Control);
        e.key_click(Key::Layout('v'));
        e.key_up(Key::Control);
    }
    Ok(())
}

#[tauri::command]
fn cancel_paste() -> Result<(), String> {
    Ok(())
}

fn register_hotkey(app: &tauri::AppHandle<Wry>) -> Result<()> {
    let mut gsm = app.global_shortcut_manager();

    #[cfg(target_os = "macos")]
    let hot = "Command+Shift+V";
    #[cfg(not(target_os = "macos"))]
    let hot = "Ctrl+Shift+V";

    if gsm.is_registered(hot)? {
        gsm.unregister(hot)?;
    }
    let app_c = app.clone();
    gsm.register(hot, move || {
        // On safe-paste: fetch current clipboard and ask the UI to confirm.
        let mut cb = Clipboard::new().ok();
        let mut payload: Option<ClipPayload> = None;

        if let Some(ref mut clip) = cb {
            if let Ok(text) = clip.get_text() {
                let mut prev = text.clone();
                if prev.len() > 600 {
                    prev.truncate(600);
                    prev.push_str("…");
                }
                payload = Some(ClipPayload {
                    kind: "text".into(),
                    preview_text: prev,
                    img_base64: None,
                });
            } else if let Ok(img) = clip.get_image() {
                let w = img.width;
                let h = img.height;
                let b64 = image_to_png_base64(&img, 480).ok();
                payload = Some(ClipPayload {
                    kind: "image".into(),
                    preview_text: format!("Image ({}×{})", w, h),
                    img_base64: b64,
                });
            }
        }

        app_c.emit_all("show_confirm", &payload).ok();
    })?;

    Ok(())
}

fn tray() -> SystemTray {
    let toggle = CustomMenuItem::new("toggle_hud".to_string(), "Toggle HUD");
    let pause = CustomMenuItem::new("pause".to_string(), "Pause Monitoring");
    let quit = CustomMenuItem::new("quit".to_string(), "Quit");
    let menu = SystemTrayMenu::new()
        .add_item(toggle)
        .add_item(pause)
        .add_item(quit);
    SystemTray::new().with_menu(menu)
}

fn handle_tray(app: &tauri::AppHandle, event: SystemTrayEvent) {
    match event {
        SystemTrayEvent::MenuItemClick { id, .. } => {
            match id.as_str() {
                "toggle_hud" => {
                    if let Some(w) = app.get_window("hud") {
                        if w.is_visible().unwrap_or(true) {
                            w.hide().ok();
                        } else {
                            w.show().ok();
                            w.set_focus().ok();
                        }
                    }
                }
                "pause" => {
                    let mut st = APP_STATE.lock();
                    st.paused = !st.paused;
                    let item = app.tray_handle().get_item(&id);
                    item.set_title(if st.paused { "Resume Monitoring" } else { "Pause Monitoring" }).ok();
                }
                "quit" => {
                    std::process::exit(0);
                }
                _ => {}
            }
        }
        _ => {}
    }
}

fn spawn_clipboard_watcher(app: tauri::AppHandle<Wry>) {
    thread::spawn(move || {
        let mut clip = Clipboard::new().ok();
        loop {
            {
                let st = APP_STATE.lock();
                if st.paused {
                    thread::sleep(Duration::from_millis(400));
                    continue;
                }
            }

            let mut changed = None;

            if let Some(ref mut cb) = clip {
                if let Ok(text) = cb.get_text() {
                    let fp = text_fingerprint(&text);
                    let mut st = APP_STATE.lock();
                    if st.last_fingerprint != Some(fp) {
                        st.last_fingerprint = Some(fp);
                        let mut prev = text;
                        if prev.len() > 300 {
                            prev.truncate(300);
                            prev.push_str("…");
                        }
                        let payload = ClipPayload {
                            kind: "text".into(),
                            preview_text: prev,
                            img_base64: None,
                        };
                        changed = Some(payload);
                    }
                } else if let Ok(img) = cb.get_image() {
                    let fp = image_fingerprint(&img);
                    let mut st = APP_STATE.lock();
                    if st.last_fingerprint != Some(fp) {
                        st.last_fingerprint = Some(fp);
                        let w = img.width;
                        let h = img.height;
                        let b64 = image_to_png_base64(&img, 320).ok();
                        let payload = ClipPayload {
                            kind: "image".into(),
                            preview_text: format!("Image ({}×{})", w, h),
                            img_base64: b64,
                        };
                        changed = Some(payload);
                    }
                }
            }

            if let Some(p) = changed {
                app.emit_all("clipboard_changed", p).ok();
            }

            thread::sleep(Duration::from_millis(250));
        }
    });
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // position HUD to bottom-right on first start (light heuristic)
            if let Some(w) = app.get_window("hud") {
                if let Ok(m) = w.current_monitor() {
                    if let Some(m) = m {
                        let size = m.size();
                        let wpx = 420.0;
                        let hpx = 220.0;
                        // 24px margin
                        let x = (size.width as f64 - wpx - 24.0).max(0.0);
                        let y = (size.height as f64 - hpx - 48.0).max(0.0);
                        w.set_always_on_top(true).ok();
                        w.set_decorations(false).ok();
                        w.set_skip_taskbar(true).ok();
                        w.set_resizable(true).ok();
                        w.set_position(tauri::Position::Physical(tauri::PhysicalPosition { x: x as i32, y: y as i32 })).ok();
                    }
                }
            }

            register_hotkey(&app.handle()).ok();
            spawn_clipboard_watcher(app.handle());
            Ok(())
        })
        .system_tray(tray())
        .on_system_tray_event(handle_tray)
        .invoke_handler(tauri::generate_handler![confirm_paste, cancel_paste])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
