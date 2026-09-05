use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_clipboard_manager::ClipboardExt;

pub const OVERLAY_LABEL: &str = "quick-ask";
pub const SETTINGS_LABEL: &str = "settings";
pub const TRAY_MENU_LABEL: &str = "tray-menu";
pub const SPLASH_LABEL: &str = "splash";
pub const OVERLAY_WIDTH: f64 = 728.0;
pub const OVERLAY_MIN_HEIGHT: f64 = 108.0;
const TRAY_MENU_WIDTH: f64 = 236.0;
const TRAY_MENU_HEIGHT: f64 = 176.0;
const TRAY_MENU_GAP: f64 = 8.0;

static OPENING_SETTINGS: AtomicBool = AtomicBool::new(false);
/// Physical cursor position of the tray click, so the popup can be re-anchored
/// after the frontend reports its real height.
static TRAY_ANCHOR: Mutex<Option<(f64, f64)>> = Mutex::new(None);
/// Clicking the tray icon leaves the taskbar in the foreground for a moment, so
/// the popup gets an immediate blur event. Ignore it while opening.
static TRAY_MENU_OPENING: AtomicBool = AtomicBool::new(false);
/// Height the frontend last reported, so reopening places the popup correctly
/// before the webview has measured itself again.
static TRAY_MENU_LAST_HEIGHT: Mutex<f64> = Mutex::new(TRAY_MENU_HEIGHT);

fn overlay(app: &AppHandle) -> Result<WebviewWindow, String> {
    app.get_webview_window(OVERLAY_LABEL)
        .ok_or_else(|| "Overlay-Fenster fehlt.".to_string())
}

fn overlay_visible(app: &AppHandle) -> bool {
    overlay(app)
        .ok()
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

pub fn position_overlay(app: &AppHandle) -> Result<(), String> {
    let window = overlay(app)?;
    let cursor = window.cursor_position().map_err(|e| e.to_string())?;
    let monitor = window
        .monitor_from_point(cursor.x, cursor.y)
        .ok()
        .flatten()
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten())
        .ok_or_else(|| "Kein Monitor gefunden.".to_string())?;

    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let width_px = OVERLAY_WIDTH * scale;
    let height_px = OVERLAY_MIN_HEIGHT * scale;
    let x = area.position.x as f64 + (area.size.width as f64 - width_px) / 2.0;
    let y = area.position.y as f64 + (area.size.height as f64 - height_px) / 2.0;
    window
        .set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32))
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Alt+Space is still down when the global shortcut fires on Pressed.
/// Focusing then makes the next letter an Alt+mnemonic → Windows beep, char gone.
async fn wait_modifiers_released() {
    #[cfg(windows)]
    {
        const VK_CONTROL: i32 = 0x11;
        const VK_MENU: i32 = 0x12;
        const VK_SPACE: i32 = 0x20;
        const VK_LWIN: i32 = 0x5B;
        const VK_RWIN: i32 = 0x5C;
        for _ in 0..80 {
            let down = |vk: i32| unsafe { win_user::GetAsyncKeyState(vk) as u16 & 0x8000 != 0 };
            if !down(VK_MENU)
                && !down(VK_CONTROL)
                && !down(VK_SPACE)
                && !down(VK_LWIN)
                && !down(VK_RWIN)
            {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }
}

#[cfg(windows)]
mod win_user {
    #[link(name = "user32")]
    extern "system" {
        pub fn GetAsyncKeyState(vKey: i32) -> i16;
        pub fn GetWindowLongPtrW(hWnd: isize, nIndex: i32) -> isize;
        pub fn SetWindowLongPtrW(hWnd: isize, nIndex: i32, dwNewLong: isize) -> isize;
    }
}

/// Frameless windows still keep a system menu. Alt+Space / Alt+letter then beeps.
pub fn harden_overlay_hwnd(app: &AppHandle) {
    #[cfg(windows)]
    {
        const GWL_STYLE: i32 = -16;
        const WS_SYSMENU: isize = 0x0008_0000;
        let Ok(window) = overlay(app) else {
            return;
        };
        let Ok(hwnd) = window.hwnd() else {
            return;
        };
        let hwnd = hwnd.0 as isize;
        unsafe {
            let style = win_user::GetWindowLongPtrW(hwnd, GWL_STYLE);
            win_user::SetWindowLongPtrW(hwnd, GWL_STYLE, style & !WS_SYSMENU);
        }
    }
    let _ = app;
}

fn show_overlay_window(app: &AppHandle) -> Result<(), String> {
    let window = overlay(app)?;
    position_overlay(app)?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn hide_overlay(app: &AppHandle) -> Result<(), String> {
    overlay(app)?.hide().map_err(|e| e.to_string())?;
    let _ = app.emit("overlay-hidden", ());
    Ok(())
}

pub fn present_overlay(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        wait_modifiers_released().await;
        let ctx = crate::selection::capture_context(&app);
        let _ = show_overlay_window(&app);
        let _ = app.emit("overlay-ready", ctx);
    });
}

pub fn on_hotkey(app: &AppHandle) {
    if overlay_visible(app) {
        let _ = hide_overlay(app);
        return;
    }
    present_overlay(app);
}

#[cfg(desktop)]
static REGISTERED_SHORTCUTS: Mutex<(
    Option<tauri_plugin_global_shortcut::Shortcut>,
    Option<tauri_plugin_global_shortcut::Shortcut>,
)> = Mutex::new((None, None));

#[cfg(desktop)]
pub fn handle_global_shortcut(
    app: &AppHandle,
    shortcut: &tauri_plugin_global_shortcut::Shortcut,
) {
    let (overlay_sc, tts_sc) = match REGISTERED_SHORTCUTS.lock() {
        Ok(guard) => (*guard).clone(),
        Err(_) => (None, None),
    };

    if let Some(ref sc) = tts_sc {
        if sc == shortcut || sc.id() == shortcut.id() {
            crate::tts::trigger_tts(app);
            return;
        }
    }

    if let Some(ref sc) = overlay_sc {
        if sc == shortcut || sc.id() == shortcut.id() {
            on_hotkey(app);
            return;
        }
    }

    on_hotkey(app);
}

pub fn normalize_hotkey(input: &str) -> String {
    let raw = input.trim();
    if raw.is_empty() {
        return String::new();
    }
    let parts: Vec<&str> = raw
        .split('+')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let mut mods = Vec::new();
    let mut key = "";

    for part in parts {
        let lower = part.to_lowercase();
        match lower.as_str() {
            "ctrl" | "control" | "strg" | "steuerung" => mods.push("ctrl"),
            "alt" | "option" | "altgr" => mods.push("alt"),
            "shift" | "umschalt" => mods.push("shift"),
            "super" | "win" | "windows" | "cmd" | "command" | "meta" => mods.push("super"),
            _ => key = part,
        }
    }

    if key.is_empty() {
        return raw.to_string();
    }

    let key_formatted = if key.len() >= 2
        && key.to_lowercase().starts_with('f')
        && key[1..].chars().all(|c| c.is_ascii_digit())
    {
        key.to_uppercase()
    } else if key.eq_ignore_ascii_case("space")
        || key.eq_ignore_ascii_case("leer")
        || key.eq_ignore_ascii_case("leertaste")
    {
        "Space".to_string()
    } else if key.len() == 1 {
        key.to_uppercase()
    } else {
        key.to_string()
    };

    if mods.is_empty() {
        key_formatted
    } else {
        format!("{}+{}", mods.join("+"), key_formatted)
    }
}

pub fn register_hotkeys(
    app: &AppHandle,
    overlay_accel: &str,
    tts_accel: &str,
) -> Result<(), String> {
    #[cfg(desktop)]
    {
        use crate::settings::{DEFAULT_HOTKEY, DEFAULT_TTS_HOTKEY};
        use std::str::FromStr;
        use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

        let _ = app.global_shortcut().unregister_all();

        let o_norm = normalize_hotkey(overlay_accel);
        let o_str = if o_norm.is_empty() {
            DEFAULT_HOTKEY
        } else {
            &o_norm
        };

        let t_norm = normalize_hotkey(tts_accel);
        let t_str = if t_norm.is_empty() {
            DEFAULT_TTS_HOTKEY
        } else {
            &t_norm
        };

        let o_sc = Shortcut::from_str(o_str)
            .map_err(|e| format!("Overlay-Hotkey '{o_str}' ungültig: {e}"))?;
        let t_sc = Shortcut::from_str(t_str)
            .map_err(|e| format!("TTS-Hotkey '{t_str}' ungültig: {e}"))?;

        app.global_shortcut()
            .register(o_sc)
            .map_err(|e| format!("Overlay-Hotkey '{o_str}' nicht registrierbar: {e}"))?;

        let mut tts_registered = false;
        if o_sc != t_sc {
            match app.global_shortcut().register(t_sc) {
                Ok(_) => {
                    tts_registered = true;
                }
                Err(err) => {
                    eprintln!("TTS-Hotkey '{t_str}' nicht registrierbar: {err}");
                    let locale = crate::i18n::app_locale(app);
                    let msg = match locale {
                        crate::i18n::Locale::De => format!("TTS-Hotkey '{t_str}' konnte vom System nicht registriert werden (evtl. belegt): {err}"),
                        crate::i18n::Locale::En => format!("TTS shortcut '{t_str}' could not be registered by the system (possibly in use): {err}"),
                    };
                    let _ = app.emit("hotkey-error", msg);
                }
            }
        } else {
            tts_registered = true;
        }

        if let Ok(mut guard) = REGISTERED_SHORTCUTS.lock() {
            *guard = (Some(o_sc), if tts_registered { Some(t_sc) } else { None });
        }
    }
    let _ = (app, overlay_accel, tts_accel);
    Ok(())
}

pub fn setup_blur_hide(app: &AppHandle) {
    let Ok(window) = overlay(app) else {
        return;
    };
    let handle = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Focused(false) = event {
            if OPENING_SETTINGS.load(Ordering::SeqCst) {
                return;
            }
            let handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(160)).await;
                if OPENING_SETTINGS.load(Ordering::SeqCst) {
                    return;
                }
                if let Ok(window) = overlay(&handle) {
                    if !window.is_focused().unwrap_or(true) {
                        let _ = hide_overlay(&handle);
                    }
                }
            });
        }
    });
}

fn attach_settings_lifecycle(window: &WebviewWindow) {
    let hide_target = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = hide_target.hide();
        }
    });
}

pub fn setup_settings_lifecycle(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(SETTINGS_LABEL) {
        attach_settings_lifecycle(&window);
    }
}

/// Centered splash on app start. The frontend closes the window itself after
/// the 1.5s animation ("splash" route in +page.svelte).
pub fn show_splash(app: &AppHandle) {
    let Some(monitor) = app
        .get_webview_window(OVERLAY_LABEL)
        .and_then(|w| w.primary_monitor().ok().flatten())
    else {
        return;
    };
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let width = 192.0 * scale;
    let height = 192.0 * scale;
    let x = area.position.x as f64 + (area.size.width as f64 - width) / 2.0;
    let y = area.position.y as f64 + (area.size.height as f64 - height) / 2.0;

    let _ = WebviewWindowBuilder::new(app, SPLASH_LABEL, WebviewUrl::App("/".into()))
        .title("Dumbo")
        .inner_size(192.0, 192.0)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .shadow(false)
        .focused(false)
        .visible(true)
        .position(x / scale, y / scale)
        .build();
}

pub fn ensure_settings_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(existing) = app.get_webview_window(SETTINGS_LABEL) {
        return Ok(existing);
    }
    let locale = crate::i18n::app_locale(app);
    let title = crate::i18n::t(locale, "window_settings");
    let window = WebviewWindowBuilder::new(app, SETTINGS_LABEL, WebviewUrl::App("/settings".into()))
        .title(title)
        .inner_size(720.0, 760.0)
        .min_inner_size(520.0, 480.0)
        .decorations(false)
        .transparent(true)
        .resizable(true)
        .center()
        .visible(false)
        .build()
        .map_err(|e| format!("Settings-Fenster nicht erstellbar: {e}"))?;

    attach_settings_lifecycle(&window);
    Ok(window)
}

pub fn ensure_tray_menu(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(existing) = app.get_webview_window(TRAY_MENU_LABEL) {
        return Ok(existing);
    }
    let locale = crate::i18n::app_locale(app);
    let title = crate::i18n::t(locale, "window_tray_menu");
    let window = WebviewWindowBuilder::new(app, TRAY_MENU_LABEL, WebviewUrl::App("/".into()))
        .title(title)
        .inner_size(TRAY_MENU_WIDTH, TRAY_MENU_HEIGHT)
        .decorations(false)
        .transparent(true)
        .background_color(tauri::window::Color(0, 0, 0, 0))
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .shadow(false)
        .visible(false)
        .build()
        .map_err(|e| format!("Tray-Menü nicht erstellbar: {e}"))?;

    let handle = app.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::Focused(false) => {
            if TRAY_MENU_OPENING.load(Ordering::SeqCst) {
                return;
            }
            if let Some(window) = handle.get_webview_window(TRAY_MENU_LABEL) {
                let _ = window.hide();
            }
        }
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            if let Some(window) = handle.get_webview_window(TRAY_MENU_LABEL) {
                let _ = window.hide();
            }
        }
        _ => {}
    });
    Ok(window)
}

/// Anchors the popup above the tray click, flipping below and clamping into the
/// work area so it never lands behind the taskbar or off screen. The height is
/// passed in logical pixels because `set_size` is applied asynchronously, so
/// `outer_size` would still report the previous value.
fn place_tray_menu(
    window: &WebviewWindow,
    anchor: (f64, f64),
    logical_height: f64,
) -> Result<(), String> {
    let monitor = window
        .monitor_from_point(anchor.0, anchor.1)
        .ok()
        .flatten()
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten())
        .ok_or_else(|| "Kein Monitor gefunden.".to_string())?;

    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let width = TRAY_MENU_WIDTH * scale;
    let height = logical_height * scale;
    let gap = TRAY_MENU_GAP * scale;

    let min_x = area.position.x as f64;
    let min_y = area.position.y as f64;
    let max_x = (min_x + area.size.width as f64 - width).max(min_x);
    let max_y = (min_y + area.size.height as f64 - height).max(min_y);

    let mut y = anchor.1 - height - gap;
    if y < min_y {
        y = anchor.1 + gap;
    }
    let x = (anchor.0 - width / 2.0).clamp(min_x, max_x);
    y = y.clamp(min_y, max_y);

    window
        .set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32))
        .map_err(|e| e.to_string())
}

fn toggle_tray_menu(app: &AppHandle) {
    let Ok(window) = ensure_tray_menu(app) else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }
    let anchor = window
        .cursor_position()
        .map(|position| (position.x, position.y))
        .unwrap_or((0.0, 0.0));
    if let Ok(mut slot) = TRAY_ANCHOR.lock() {
        *slot = Some(anchor);
    }
    TRAY_MENU_OPENING.store(true, Ordering::SeqCst);
    let height = TRAY_MENU_LAST_HEIGHT
        .lock()
        .ok()
        .map(|slot| *slot)
        .unwrap_or(TRAY_MENU_HEIGHT);
    let _ = place_tray_menu(&window, anchor, height);
    if window.show().is_err() {
        let _ = window.destroy();
        if let Ok(new_win) = ensure_tray_menu(app) {
            let _ = place_tray_menu(&new_win, anchor, height);
            let _ = new_win.show();
            let _ = new_win.set_focus();
        }
    } else {
        let _ = window.set_focus();
    }
    let _ = app.emit("tray-menu-shown", ());

    tauri::async_runtime::spawn(async move {
        for _ in 0..3 {
            tokio::time::sleep(std::time::Duration::from_millis(90)).await;
            if !window.is_focused().unwrap_or(false) {
                let _ = window.set_focus();
            }
        }
        TRAY_MENU_OPENING.store(false, Ordering::SeqCst);
    });
}

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let icon = tauri::include_image!("icons/128x128.png");
    TrayIconBuilder::new()
        .icon(icon)
        .tooltip("Dumbo")
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click {
                button,
                button_state: MouseButtonState::Up,
                ..
            } => match button {
                MouseButton::Left => {
                    on_hotkey(tray.app_handle());
                }
                MouseButton::Right => {
                    toggle_tray_menu(tray.app_handle());
                }
                MouseButton::Middle => {
                    tray.app_handle().exit(0);
                }
            },
            _ => {}
        })
        .build(app)?;
    Ok(())
}

#[tauri::command]
pub fn hide_overlay_cmd(app: AppHandle) -> Result<(), String> {
    hide_overlay(&app)
}

#[tauri::command]
pub fn set_overlay_height(app: AppHandle, height: f64) -> Result<(), String> {
    let window = overlay(&app)?;
    let clamped = height.max(OVERLAY_MIN_HEIGHT).min(640.0);
    window
        .set_size(LogicalSize::new(OVERLAY_WIDTH, clamped))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_settings(app: AppHandle) -> Result<(), String> {
    OPENING_SETTINGS.store(true, Ordering::SeqCst);
    let _ = hide_overlay(&app);
    let window = ensure_settings_window(&app)?;
    let _ = window.unminimize();
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    let _ = app.emit("settings-opened", ());
    tauri::async_runtime::spawn(async {
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        OPENING_SETTINGS.store(false, Ordering::SeqCst);
    });
    Ok(())
}

#[tauri::command]
pub fn hide_tray_menu(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(TRAY_MENU_LABEL) {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn resize_tray_menu(app: AppHandle, height: f64) -> Result<(), String> {
    let window = app
        .get_webview_window(TRAY_MENU_LABEL)
        .ok_or_else(|| crate::i18n::t(crate::i18n::app_locale(&app), "tray_menu_missing").to_string())?;
    let clamped = height.max(60.0).min(420.0);
    window
        .set_size(LogicalSize::new(TRAY_MENU_WIDTH, clamped))
        .map_err(|e| e.to_string())?;
    if let Ok(mut slot) = TRAY_MENU_LAST_HEIGHT.lock() {
        *slot = clamped;
    }
    let anchor = TRAY_ANCHOR.lock().ok().and_then(|slot| *slot);
    if let Some(anchor) = anchor {
        place_tray_menu(&window, anchor, clamped)?;
    }
    Ok(())
}

#[tauri::command]
pub fn tray_action(app: AppHandle, action: String) -> Result<(), String> {
    let _ = hide_tray_menu(app.clone());
    match action.as_str() {
        "open" => present_overlay(&app),
        "settings" => open_settings(app)?,
        "quit" => app.exit(0),
        other => return Err(format!("Unbekannte Aktion: {other}")),
    }
    Ok(())
}

#[tauri::command]
pub fn copy_text(app: AppHandle, text: String) -> Result<(), String> {
    app.clipboard()
        .write_text(text)
        .map_err(|e| format!("Zwischenablage nicht schreibbar: {e}"))
}

pub fn update_window_titles(app: &AppHandle, language_setting: &str) {
    let locale = crate::i18n::resolve_locale(language_setting);
    if let Some(settings_win) = app.get_webview_window(SETTINGS_LABEL) {
        let _ = settings_win.set_title(crate::i18n::t(locale, "window_settings"));
    }
    if let Some(tray_win) = app.get_webview_window(TRAY_MENU_LABEL) {
        let _ = tray_win.set_title(crate::i18n::t(locale, "window_tray_menu"));
    }
}
