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
fn is_input_or_placeholder(text: &str, is_edit: bool) -> bool {
    let lower = text.trim().to_lowercase();
    if lower.is_empty() {
        return true;
    }
    let placeholders = [
        "message input",
        "chat input",
        "prompt",
        "type a message",
        "ask anything",
        "ask a question",
        "ask follow-up",
        "send a message",
        "nachricht eingeben",
        "frage stellen",
        "nachricht an",
        "search",
        "suche",
    ];
    if placeholders.iter().any(|&p| lower == p || lower.starts_with(p)) {
        return true;
    }
    if is_edit
        && lower.len() < 50
        && (lower.contains("input")
            || lower.contains("message")
            || lower.contains("placeholder")
            || lower.contains("ask"))
    {
        return true;
    }
    false
}

#[cfg(windows)]
fn extract_text_from_element(element: &uiautomation::core::UIElement, is_edit: bool) -> Option<String> {
    use uiautomation::patterns::UITextPattern;
    if let Ok(text_pattern) = element.get_pattern::<UITextPattern>() {
        if let Ok(range) = text_pattern.get_document_range() {
            if let Ok(text) = range.get_text(-1) {
                let trimmed = text.trim();
                if !trimmed.is_empty() && !is_input_or_placeholder(trimmed, is_edit) {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    if let Ok(name) = element.get_name() {
        let trimmed = name.trim();
        if !trimmed.is_empty() && !is_input_or_placeholder(trimmed, is_edit) {
            return Some(trimmed.to_string());
        }
    }
    None
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
            if let Some(norm) = normalize(result) {
                if !is_input_or_placeholder(&norm, false) {
                    return Some(norm);
                }
            }
        }
    }
    None
}

#[cfg(not(windows))]
fn is_input_or_placeholder(_text: &str, _is_edit: bool) -> bool {
    false
}

#[cfg(not(windows))]
fn try_get_selection_uia() -> Option<String> {
    None
}

#[cfg(windows)]
pub fn try_get_document_text_uia() -> Option<String> {
    use uiautomation::controls::ControlType;
    use uiautomation::UIAutomation;

    let automation = UIAutomation::new().ok()?;
    let element = automation.get_focused_element().ok()?;
    let ctrl_type = element.get_control_type().ok();
    let is_edit = ctrl_type == Some(ControlType::Edit);

    // If it's not an edit control, try getting document text directly
    if !is_edit {
        if let Some(text) = extract_text_from_element(&element, false) {
            return Some(text);
        }
    }

    // If it's an edit control (or direct extraction yielded placeholder),
    // walk up the tree and look for preceding siblings (chat message containers)
    let walker = automation
        .get_control_view_walker()
        .or_else(|_| automation.get_raw_view_walker())
        .ok()?;

    let mut curr = element;
    for _ in 0..8 {
        // Check previous siblings of current container
        let mut prev_sibling = walker.get_previous_sibling(&curr);
        let mut sibling_count = 0;
        while let Ok(ref prev) = prev_sibling {
            if sibling_count > 6 {
                break;
            }
            if let Some(text) = extract_text_from_element(prev, false) {
                let blocks: Vec<&str> = text
                    .split("\n\n")
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty() && !is_input_or_placeholder(s, false))
                    .collect();
                if let Some(last_block) = blocks.last() {
                    return Some(last_block.to_string());
                }
                return Some(text);
            }

            // Drill down into the last child of this previous sibling
            let mut leaf = prev.clone();
            for _ in 0..6 {
                if let Ok(last_child) = walker.get_last_child(&leaf) {
                    if let Some(child_text) = extract_text_from_element(&last_child, false) {
                        return Some(child_text);
                    }
                    leaf = last_child;
                } else {
                    break;
                }
            }

            prev_sibling = walker.get_previous_sibling(prev);
            sibling_count += 1;
        }

        // Try getting document text from parent if it has text pattern
        if let Ok(parent) = walker.get_parent(&curr) {
            if let Some(parent_text) = extract_text_from_element(&parent, false) {
                let blocks: Vec<&str> = parent_text
                    .split("\n\n")
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty() && !is_input_or_placeholder(s, false))
                    .collect();
                if let Some(last_block) = blocks.last() {
                    return Some(last_block.to_string());
                }
            }
            curr = parent;
        } else {
            break;
        }
    }

    None
}

#[cfg(not(windows))]
pub fn try_get_document_text_uia() -> Option<String> {
    None
}

pub fn capture_latest_context(app: &AppHandle) -> Option<String> {
    // 1. Try selection first
    if let Some(sel) = try_get_selection_uia() {
        if !is_input_or_placeholder(&sel, false) {
            return Some(sel);
        }
    }
    // 2. Try whole document text and extract the last non-placeholder paragraph
    if let Some(doc) = try_get_document_text_uia() {
        let blocks: Vec<&str> = doc
            .split("\n\n")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty() && !is_input_or_placeholder(s, false))
            .collect();
        if let Some(last_block) = blocks.last() {
            return Some(last_block.to_string());
        }
    }
    // 3. Fallback to clipboard
    if let Some(clip) = read_clipboard(app) {
        if !is_input_or_placeholder(&clip, false) {
            return Some(clip);
        }
    }
    None
}

#[cfg(windows)]
pub fn release_physical_modifiers() {
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
pub fn release_physical_modifiers() {}

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
