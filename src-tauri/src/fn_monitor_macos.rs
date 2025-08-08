use std::thread;
use std::time::{Duration, Instant};
use once_cell::sync::Lazy;
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager};
use crate::audio;
use crate::handle_stop_recording_workflow;

#[link(name = "vwisper_macos_fn_monitor", kind = "static")]
extern "C" {
    fn vwisper_start_fn_monitor(on_down: extern "C" fn(), on_up: extern "C" fn());
    fn vwisper_stop_fn_monitor();
}

static APP_HANDLE_FOR_FN: OnceLock<AppHandle> = OnceLock::new();
static HOLD_START_TIME: Lazy<Mutex<Option<Instant>>> = Lazy::new(|| Mutex::new(None));
static LAST_ACTION_TIME: Lazy<Mutex<Instant>> = Lazy::new(|| Mutex::new(Instant::now()));

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

    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.show();
    }
    let _ = app_handle.emit_to("main", "pill-state", "listening");
    let _ = app_handle.emit_to("main", "start-recording", "");
    let _ = audio::start_recording();
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

    let _ = app_handle.emit_to("main", "pill-state", "loading");
    let _ = app_handle.emit_to("main", "stop-recording", "");

    if let Some(hold_time) = hold_time_ms {
        let _ = app_handle.emit_to("main", "hold-time", hold_time);
    }

    let app_handle_clone = app_handle.clone();
    thread::spawn(move || {
        let result = handle_stop_recording_workflow(&app_handle_clone, None, hold_time_ms);
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
        }
    });
}

pub fn start_global_key_monitor(app_handle: AppHandle) {
    let _ = APP_HANDLE_FOR_FN.set(app_handle.clone());
    thread::spawn(move || unsafe {
        vwisper_start_fn_monitor(on_fn_down_callback, on_fn_up_callback);
    });
}


