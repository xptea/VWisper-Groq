use std::thread;
use std::time::{Duration, Instant};
use once_cell::sync::Lazy;
use std::sync::{Mutex, OnceLock};
use std::process::Command;
use tauri::{AppHandle, Emitter, Manager};
use crate::audio;
use crate::handle_stop_recording_workflow;

#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};
#[cfg(target_os = "macos")]
use cocoa::base::id;

#[link(name = "vwisper_macos_fn_monitor", kind = "static")]
extern "C" {
    fn vwisper_start_fn_monitor(on_down: extern "C" fn(), on_up: extern "C" fn());
}

static APP_HANDLE_FOR_FN: OnceLock<AppHandle> = OnceLock::new();
static HOLD_START_TIME: Lazy<Mutex<Option<Instant>>> = Lazy::new(|| Mutex::new(None));
static LAST_ACTION_TIME: Lazy<Mutex<Instant>> = Lazy::new(|| Mutex::new(Instant::now()));
static FN_HELD: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));
static STARTING_RECORDING: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));
static FRONT_APP_NAME: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

fn get_frontmost_app_name() -> Option<String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg("tell application \"System Events\" to get name of (first process whose frontmost is true)")
        .output()
        .ok()?;
    if !output.status.success() { return None; }
    let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

#[cfg(target_os = "macos")]
pub fn set_dock_icon_visible(visible: bool) {
    unsafe {
        let app = cocoa::appkit::NSApp();
        if visible {
            // NSApplicationActivationPolicyRegular = 0
            let _: () = msg_send![app, setActivationPolicy: 0];
        } else {
            // NSApplicationActivationPolicyAccessory = 1  
            let _: () = msg_send![app, setActivationPolicy: 1];
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn set_dock_icon_visible(_visible: bool) {
    // No-op on non-macOS platforms
}

pub fn update_dock_icon_for_app(app: &AppHandle) {
    let main_visible = app.get_webview_window("main")
        .map(|w| w.is_visible().unwrap_or(false))
        .unwrap_or(false);
    
    let dashboard_visible = app.get_webview_window("dashboard")
        .map(|w| w.is_visible().unwrap_or(false))
        .unwrap_or(false);
    
    let any_window_visible = main_visible || dashboard_visible;
    set_dock_icon_visible(any_window_visible);
}

extern "C" fn on_fn_down_callback() {
    let app_handle = match APP_HANDLE_FOR_FN.get() {
        Some(h) => h.clone(),
        None => return,
    };

    let mut last = LAST_ACTION_TIME.lock().unwrap();
    let now = Instant::now();
    if now.duration_since(*last) <= Duration::from_millis(25) {
        return;
    }
    *last = now;

    {
        let mut hold = HOLD_START_TIME.lock().unwrap();
        *hold = Some(now);
    }

    {
        let name = get_frontmost_app_name();
        let mut front = FRONT_APP_NAME.lock().unwrap();
        *front = name;
    }

    {
        let mut held = FN_HELD.lock().unwrap();
        *held = true;
    }

    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.show();
        update_dock_icon_for_app(&app_handle);
    }
    let _ = app_handle.emit_to("main", "pill-state", "listening");
    let _ = app_handle.emit_to("main", "start-recording", "");

    if let Some(proc_name) = {
        let front = FRONT_APP_NAME.lock().unwrap();
        front.clone()
    } {
        thread::spawn(move || {
            let script = format!(
                "tell application \"System Events\" to set frontmost of process \"{}\" to true",
                proc_name
            );
            let _ = Command::new("osascript").arg("-e").arg(&script).output();
        });
    }

    {
        let mut starting = STARTING_RECORDING.lock().unwrap();
        if !*starting && !audio::is_recording() {
            *starting = true;
            let app_handle_clone = app_handle.clone();
            thread::spawn(move || {
                let start_deadline = Instant::now() + Duration::from_millis(2000);
                loop {
                    let held = { *FN_HELD.lock().unwrap() };
                    if !held { break; }
                    if audio::is_ready() {
                        if audio::start_recording().is_ok() {
                            break;
                        }
                    }
                    if Instant::now() >= start_deadline { break; }
                    thread::sleep(Duration::from_millis(20));
                }
                let mut starting = STARTING_RECORDING.lock().unwrap();
                *starting = false;
                let _ = app_handle_clone.emit_to("main", "noop", "");
            });
        }
    }
}

extern "C" fn on_fn_up_callback() {
    let app_handle = match APP_HANDLE_FOR_FN.get() {
        Some(h) => h.clone(),
        None => return,
    };

    let mut last = LAST_ACTION_TIME.lock().unwrap();
    let now = Instant::now();
    if now.duration_since(*last) <= Duration::from_millis(25) {
        return;
    }
    *last = now;

    let hold_time_ms = {
        let mut hold = HOLD_START_TIME.lock().unwrap();
        let ms = hold.map(|start| start.elapsed().as_millis() as u64);
        *hold = None;
        ms
    };

    {
        let mut held = FN_HELD.lock().unwrap();
        *held = false;
    }

    if !audio::is_recording() {
        let _ = app_handle.emit_to("main", "pill-state", "idle");
        if let Some(window) = app_handle.get_webview_window("main") {
            let _ = window.hide();
            update_dock_icon_for_app(&app_handle);
        }
        return;
    }
    let _ = app_handle.emit_to("main", "pill-state", "loading");
    let _ = app_handle.emit_to("main", "stop-recording", "");

    if let Some(hold_time) = hold_time_ms {
        let _ = app_handle.emit_to("main", "hold-time", hold_time);
    }

    let app_handle_clone = app_handle.clone();
    let restore_to = {
        let mut front = FRONT_APP_NAME.lock().unwrap();
        front.take()
    };
    thread::spawn(move || {
        let restore = restore_to.map(|proc_name| Box::new(move || {
            let script = format!(
                "tell application \"System Events\" to set frontmost of process \"{}\" to true",
                proc_name
            );
            let _ = Command::new("osascript").arg("-e").arg(&script).output();
            std::thread::sleep(Duration::from_millis(100));
        }) as Box<dyn FnOnce()>);

        let result = handle_stop_recording_workflow(&app_handle_clone, restore, hold_time_ms);
        if let Err(e) = result {
            eprintln!("Error in handle_stop_recording_workflow: {}", e);
            let _ = app_handle_clone.emit_to("main", "pill-state", "error");
            thread::sleep(Duration::from_secs(3));
        } else {
            let _ = app_handle_clone.emit_to("main", "pill-state", "success");
            thread::sleep(Duration::from_millis(500));
        }
        let _ = app_handle_clone.emit_to("main", "pill-state", "idle");
        if let Some(window) = app_handle_clone.get_webview_window("main") {
            let _ = window.hide();
            update_dock_icon_for_app(&app_handle_clone);
        }
    });
}

pub fn start_global_key_monitor(app_handle: AppHandle) {
    let _ = APP_HANDLE_FOR_FN.set(app_handle.clone());
    thread::spawn(move || unsafe {
        std::thread::sleep(Duration::from_millis(750));
        vwisper_start_fn_monitor(on_fn_down_callback, on_fn_up_callback);
    });
}
