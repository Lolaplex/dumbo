//! Native failsafe for a dead or blank WebView2 after sleep.
//!
//! The tray menu and the overlay are webviews. When the renderer dies, nothing
//! on screen can quit the app. This thread lives in the Rust process: every
//! webview sends a pulse, sleep/resume reloads them, and a webview that does
//! not come back restarts the process (once). A second failure quits instead
//! of restart-looping.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Manager};

const WATCHED: &[&str] = &["quick-ask", "settings", "tray-menu"];
const STAMP_FILE: &str = "webview-watchdog";

static PULSES: LazyLock<Mutex<HashMap<String, Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static RECOVERY_AT: Mutex<Option<Instant>> = Mutex::new(None);
/// Pulses before this instant are the pre-reload page and do not count.
static ACCEPT_AFTER: Mutex<Option<Instant>> = Mutex::new(None);
static BOOT: OnceLock<Instant> = OnceLock::new();
static RESTARTING: AtomicBool = AtomicBool::new(false);
static RESUME_FLAG: AtomicBool = AtomicBool::new(false);
static STAMP_CLEARED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy)]
struct Limits {
    boot_grace: Duration,
    visible_dead: Duration,
    recovery_wait: Duration,
    sleep_gap: Duration,
    pulse_ignore: Duration,
}

fn limits() -> Limits {
    if cfg!(debug_assertions) {
        Limits {
            boot_grace: Duration::from_secs(30),
            visible_dead: Duration::from_secs(8),
            recovery_wait: Duration::from_secs(12),
            sleep_gap: Duration::from_secs(8),
            pulse_ignore: Duration::from_millis(500),
        }
    } else {
        Limits {
            boot_grace: Duration::from_secs(10),
            visible_dead: Duration::from_secs(4),
            recovery_wait: Duration::from_secs(6),
            sleep_gap: Duration::from_secs(8),
            pulse_ignore: Duration::from_millis(500),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Decision {
    None,
    Restart,
    /// Recovery finished and every watched webview pulsed again.
    Healthy,
}

struct Win {
    visible_for: Option<Duration>,
    /// `None` = this webview has never pulsed (or pulses were cleared).
    last_pulse_age: Option<Duration>,
}

/// Pure policy. Sleep/resume is handled by the caller (`begin_recovery`).
fn decide(
    limits: Limits,
    boot_age: Duration,
    recovery_age: Option<Duration>,
    windows: &[Win],
) -> Decision {
    if windows.is_empty() {
        return Decision::None;
    }
    if let Some(age) = recovery_age {
        if age < limits.recovery_wait {
            return Decision::None;
        }
        let all_back = windows
            .iter()
            .all(|win| pulsed_since_recovery(limits, age, win));
        return if all_back {
            Decision::Healthy
        } else {
            Decision::Restart
        };
    }
    if boot_age < limits.boot_grace {
        return Decision::None;
    }
    if windows.iter().any(|win| win.last_pulse_age.is_none()) {
        return Decision::Restart;
    }
    for win in windows {
        let Some(visible_for) = win.visible_for else {
            continue;
        };
        let Some(pulse_age) = win.last_pulse_age else {
            return Decision::Restart;
        };
        if visible_for >= limits.visible_dead && pulse_age >= limits.visible_dead {
            return Decision::Restart;
        }
    }
    Decision::None
}

fn pulsed_since_recovery(limits: Limits, recovery_age: Duration, win: &Win) -> bool {
    match win.last_pulse_age {
        Some(age) => age + limits.pulse_ignore < recovery_age,
        None => false,
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poison| poison.into_inner())
}

#[tauri::command]
pub fn webview_pulse(window: tauri::WebviewWindow) {
    note_pulse(window.label().to_string());
}

fn note_pulse(label: String) {
    let now = Instant::now();
    let accept = lock(&ACCEPT_AFTER);
    if accept.is_some_and(|after| now < after) {
        return;
    }
    lock(&*PULSES).insert(label, now);
}

/// `false` = do not show. A webview that never came up restarts the process
/// instead of painting a blank window the user cannot close.
pub fn allow_show(app: &AppHandle, label: &str) -> bool {
    let _ = app.emit("dumbo-probe", ());
    if RESTARTING.load(Ordering::SeqCst) {
        return false;
    }
    let limits = limits();
    let boot_age = BOOT.get().map(|boot| boot.elapsed()).unwrap_or_default();
    let recovering = lock(&RECOVERY_AT).is_some();
    if recovering || boot_age < limits.boot_grace {
        return true;
    }
    if lock(&*PULSES).get(label).is_none() {
        escalate(app);
        return false;
    }
    true
}

pub fn start(app: AppHandle) {
    let _ = BOOT.set(Instant::now());
    register_resume();
    std::thread::Builder::new()
        .name("dumbo-watchdog".into())
        .spawn(move || run(app))
        .ok();
}

fn run(app: AppHandle) {
    let mut last = Instant::now();
    let mut visible_since: HashMap<String, Instant> = HashMap::new();
    loop {
        std::thread::sleep(Duration::from_secs(1));
        if RESTARTING.load(Ordering::SeqCst) {
            return;
        }
        let gap = last.elapsed();
        last = Instant::now();
        let resume = RESUME_FLAG.swap(false, Ordering::SeqCst);
        if gap >= limits().sleep_gap || resume {
            begin_recovery(&app);
            continue;
        }

        let now = Instant::now();
        let boot_age = BOOT.get().map(|boot| boot.elapsed()).unwrap_or_default();
        let recovery_at = *lock(&RECOVERY_AT);
        let pulses = lock(&*PULSES).clone();
        let mut windows = Vec::new();

        for (label, window) in app.webview_windows() {
            if !WATCHED.contains(&label.as_str()) {
                continue;
            }
            let visible = window.is_visible().unwrap_or(false);
            let visible_for = if visible {
                Some(visible_since.entry(label.clone()).or_insert(now).elapsed())
            } else {
                visible_since.remove(&label);
                None
            };
            let last_pulse_age = pulses.get(&label).map(|at| at.elapsed());
            windows.push(Win {
                visible_for,
                last_pulse_age,
            });
        }

        let recovery_age = recovery_at.map(|at| at.elapsed());
        match decide(limits(), boot_age, recovery_age, &windows) {
            Decision::None => {
                if recovery_at.is_none()
                    && !windows.is_empty()
                    && windows.iter().all(|win| win.last_pulse_age.is_some())
                {
                    clear_stamp_once(&app);
                }
            }
            Decision::Healthy => {
                *lock(&RECOVERY_AT) = None;
                clear_stamp_once(&app);
            }
            Decision::Restart => escalate(&app),
        }
    }
}

fn begin_recovery(app: &AppHandle) {
    if RESTARTING.load(Ordering::SeqCst) {
        return;
    }
    let limits = limits();
    {
        let mut recovery = lock(&RECOVERY_AT);
        if recovery.is_some_and(|start| start.elapsed() < limits.recovery_wait) {
            return;
        }
        *recovery = Some(Instant::now());
        // Hold both locks so a pulse cannot land between the gate and the clear.
        let mut accept = lock(&ACCEPT_AFTER);
        *accept = Some(Instant::now() + Duration::from_secs(3600));
        lock(&*PULSES).clear();
    }
    eprintln!("dumbo watchdog: resume, reloading webviews");
    let runner = app.clone();
    let queued = app.run_on_main_thread(move || {
        for (label, window) in runner.webview_windows() {
            if WATCHED.contains(&label.as_str()) {
                let _ = window.reload();
            }
        }
        *lock(&ACCEPT_AFTER) = Some(Instant::now() + Duration::from_millis(800));
        let _ = runner.emit("dumbo-probe", ());
    });
    if queued.is_err() {
        *lock(&ACCEPT_AFTER) = Some(Instant::now());
    }
}

fn escalate(app: &AppHandle) {
    if RESTARTING.swap(true, Ordering::SeqCst) {
        return;
    }
    if restart_blocked(app) {
        eprintln!("dumbo watchdog: webview still dead after restart, quitting");
        tell_user(app);
        app.exit(1);
        return;
    }
    eprintln!("dumbo watchdog: webview dead, restarting");
    write_stamp(app);
    app.request_restart();
}

fn stamp_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|dir| dir.join(STAMP_FILE))
}

fn restart_blocked(app: &AppHandle) -> bool {
    let Some(path) = stamp_path(app) else {
        return false;
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(secs) = text.trim().parse::<u64>() else {
        return false;
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    now.saturating_sub(secs) < 45
}

fn write_stamp(app: &AppHandle) {
    let Some(path) = stamp_path(app) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let _ = std::fs::write(path, now.to_string());
}

fn clear_stamp_once(app: &AppHandle) {
    if STAMP_CLEARED.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Some(path) = stamp_path(app) {
        let _ = std::fs::remove_file(path);
    }
}

fn tell_user(app: &AppHandle) {
    let text = crate::i18n::t(crate::i18n::app_locale(app), "watchdog_give_up");
    #[cfg(windows)]
    {
        let body = wide(text);
        let caption = wide("Dumbo");
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                body.as_ptr(),
                caption.as_ptr(),
                0x0000_0030,
            );
        }
    }
    #[cfg(not(windows))]
    {
        eprintln!("{text}");
    }
}

#[cfg(windows)]
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
extern "system" {
    fn MessageBoxW(
        hwnd: *mut std::ffi::c_void,
        text: *const u16,
        caption: *const u16,
        typ: u32,
    ) -> i32;
}

#[cfg(windows)]
#[repr(C)]
struct PowerSub {
    callback: unsafe extern "system" fn(*mut std::ffi::c_void, u32, *mut std::ffi::c_void) -> u32,
    context: *const std::ffi::c_void,
}

#[cfg(windows)]
unsafe impl Sync for PowerSub {}

#[cfg(windows)]
unsafe extern "system" fn on_power(
    _ctx: *mut std::ffi::c_void,
    kind: u32,
    _setting: *mut std::ffi::c_void,
) -> u32 {
    const PBT_APMRESUMEAUTOMATIC: u32 = 0x0012;
    const PBT_APMRESUMESUSPEND: u32 = 0x0007;
    if kind == PBT_APMRESUMEAUTOMATIC || kind == PBT_APMRESUMESUSPEND {
        RESUME_FLAG.store(true, Ordering::SeqCst);
    }
    0
}

#[cfg(windows)]
fn register_resume() {
    #[link(name = "powrprof")]
    extern "system" {
        fn RegisterSuspendResumeNotification(
            recipient: *mut std::ffi::c_void,
            flags: u32,
        ) -> *mut std::ffi::c_void;
    }

    static SUB: PowerSub = PowerSub {
        callback: on_power,
        context: std::ptr::null(),
    };
    const DEVICE_NOTIFY_CALLBACK: u32 = 2;
    unsafe {
        RegisterSuspendResumeNotification(
            &SUB as *const PowerSub as *mut std::ffi::c_void,
            DEVICE_NOTIFY_CALLBACK,
        );
    }
}

#[cfg(not(windows))]
fn register_resume() {}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> Limits {
        Limits {
            boot_grace: Duration::from_secs(10),
            visible_dead: Duration::from_secs(4),
            recovery_wait: Duration::from_secs(6),
            sleep_gap: Duration::from_secs(8),
            pulse_ignore: Duration::from_millis(500),
        }
    }

    fn hidden_fresh() -> Win {
        Win {
            visible_for: None,
            last_pulse_age: Some(Duration::from_secs(1)),
        }
    }

    #[test]
    fn boot_grace_ignores_missing_pulse() {
        let win = Win {
            visible_for: None,
            last_pulse_age: None,
        };
        assert_eq!(
            decide(limits(), Duration::from_secs(3), None, &[win]),
            Decision::None
        );
    }

    #[test]
    fn hidden_dead_webview_restarts() {
        let win = Win {
            visible_for: None,
            last_pulse_age: None,
        };
        assert_eq!(
            decide(limits(), Duration::from_secs(20), None, &[win]),
            Decision::Restart
        );
    }

    #[test]
    fn hidden_throttled_pulse_does_not_restart() {
        let win = Win {
            visible_for: None,
            last_pulse_age: Some(Duration::from_secs(60)),
        };
        assert_eq!(
            decide(limits(), Duration::from_secs(120), None, &[win]),
            Decision::None
        );
    }

    #[test]
    fn visible_stale_pulse_restarts() {
        let win = Win {
            visible_for: Some(Duration::from_secs(5)),
            last_pulse_age: Some(Duration::from_secs(30)),
        };
        assert_eq!(
            decide(limits(), Duration::from_secs(60), None, &[win]),
            Decision::Restart
        );
    }

    #[test]
    fn visible_fresh_pulse_stays() {
        let win = Win {
            visible_for: Some(Duration::from_secs(30)),
            last_pulse_age: Some(Duration::from_millis(800)),
        };
        assert_eq!(
            decide(
                limits(),
                Duration::from_secs(60),
                None,
                &[win, hidden_fresh()]
            ),
            Decision::None
        );
    }

    #[test]
    fn recovery_waits_then_restarts_if_silent() {
        let win = Win {
            visible_for: None,
            last_pulse_age: None,
        };
        assert_eq!(
            decide(
                limits(),
                Duration::from_secs(100),
                Some(Duration::from_secs(2)),
                &[win]
            ),
            Decision::None
        );
        let win = Win {
            visible_for: None,
            last_pulse_age: None,
        };
        assert_eq!(
            decide(
                limits(),
                Duration::from_secs(100),
                Some(Duration::from_secs(7)),
                &[win]
            ),
            Decision::Restart
        );
    }

    #[test]
    fn recovery_accepts_a_pulse_after_the_ignore_window() {
        let win = Win {
            visible_for: None,
            last_pulse_age: Some(Duration::from_secs(1)),
        };
        assert_eq!(
            decide(
                limits(),
                Duration::from_secs(100),
                Some(Duration::from_secs(7)),
                &[win]
            ),
            Decision::Healthy
        );
    }

    #[test]
    fn straggler_pulse_does_not_count_as_recovery() {
        let win = Win {
            visible_for: None,
            last_pulse_age: Some(Duration::from_millis(6700)),
        };
        assert_eq!(
            decide(
                limits(),
                Duration::from_secs(100),
                Some(Duration::from_secs(7)),
                &[win]
            ),
            Decision::Restart
        );
    }

    #[test]
    fn empty_window_list_does_nothing() {
        assert_eq!(
            decide(limits(), Duration::from_secs(100), None, &[]),
            Decision::None
        );
    }
}
