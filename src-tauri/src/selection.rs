use serde::Serialize;
use std::thread;
use std::time::Duration;
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ContextPayload {
    pub selection: Option<String>,
    pub clipboard: Option<String>,
}

fn normalize(value: String) -> Option<String> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn read_clipboard(app: &AppHandle) -> Option<String> {
    app.clipboard()
        .read_text()
        .ok()
        .and_then(normalize)
}

fn write_clipboard(app: &AppHandle, value: &str) {
    let _ = app.clipboard().write_text(value);
}

#[cfg(windows)]
fn try_get_selection_uia() -> Option<String> {
    use uiautomation::patterns::UITextPattern;
    use uiautomation::UIAutomation;

    let automation = UIAutomation::new().ok()?;
    let element = automation.get_focused_element().ok()?;

    if let Ok(text_pattern) = element.get_pattern::<UITextPattern>() {
        if let Ok(selection) = text_pattern.get_selection() {
            let mut result = String::new();
            for range in selection {
                if let Ok(text) = range.get_text(-1) {
                    result.push_str(&text);
                }
            }
            return normalize(result);
        }
    }
    None
}

#[cfg(not(windows))]
fn try_get_selection_uia() -> Option<String> {
    None
}

#[cfg(windows)]
fn release_physical_modifiers() {
    #[link(name = "user32")]
    extern "system" {
        fn keybd_event(bVk: u8, bScan: u8, dwFlags: u32, dwExtraInfo: usize);
    }
    const KEYEVENTF_KEYUP: u32 = 0x0002;
    const VK_SHIFT: u8 = 0x10;
    const VK_CONTROL: u8 = 0x11;
    const VK_MENU: u8 = 0x12; // Alt
    const VK_LWIN: u8 = 0x5B;
    const VK_RWIN: u8 = 0x5C;
    const VK_NONAME: u8 = 0xFC; // Unassigned key to cancel Alt menu-bar activation

    unsafe {
        // Send a dummy keystroke so Windows knows Alt was a modifier combo and never beeps on menu-bar focus
        keybd_event(VK_NONAME, 0, 0, 0);
        keybd_event(VK_NONAME, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_MENU, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_SHIFT, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_LWIN, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_RWIN, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_CONTROL, 0, KEYEVENTF_KEYUP, 0);
    }
}

#[cfg(not(windows))]
fn release_physical_modifiers() {}

fn simulate_copy() -> Result<(), String> {
    release_physical_modifiers();
    thread::sleep(Duration::from_millis(15));
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    let mut enigo =
        Enigo::new(&Settings::default()).map_err(|e| format!("Tastatursteuerung fehlt: {e}"))?;
    enigo
        .key(Key::Control, Direction::Press)
        .map_err(|e| e.to_string())?;
    enigo
        .key(Key::Unicode('c'), Direction::Click)
        .map_err(|e| e.to_string())?;
    enigo
        .key(Key::Control, Direction::Release)
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn capture_context(app: &AppHandle) -> ContextPayload {
    // 1. Accessibility API (UI Automation): Zero side-effects, instant, reads focused selection
    if let Some(uia_text) = try_get_selection_uia() {
        return ContextPayload {
            selection: Some(uia_text),
            clipboard: read_clipboard(app),
        };
    }

    // 2. Fallback: Clean simulated copy (releasing held shortcut modifiers first)
    let original = read_clipboard(app);
    let _ = simulate_copy();
    thread::sleep(Duration::from_millis(85));
    let after = read_clipboard(app);

    if let Some(ref original_text) = original {
        write_clipboard(app, original_text);
    }

    let selection = match (&original, &after) {
        (_, None) => None,
        (Some(before), Some(after_text)) if before == after_text => None,
        (_, Some(after_text)) => Some(after_text.clone()),
    };

    ContextPayload {
        selection,
        clipboard: original,
    }
}
