// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{command, Emitter, Manager, PhysicalPosition};
mod platform {
    #[cfg(target_os = "windows")]
    pub mod windows;
    #[cfg(target_os = "macos")]
    pub mod macos;
}
mod audio;
mod tray;
mod settings;
mod transcription;
mod textinjection;
mod history;
mod update;
use history::{History, TranscriptionEntry};
use chrono::Utc;
use std::sync::OnceLock;
use uuid::Uuid;
use std::fs;
use base64;
use dirs::config_dir;
use std::time::Duration;

static HISTORY: OnceLock<History> = OnceLock::new();

pub fn handle_stop_recording_workflow(app: &tauri::AppHandle, restore_focus: Option<Box<dyn FnOnce()>>, hold_time_ms: Option<u64>) -> Result<(), String> {
    audio::stop_recording().map_err(|e| e.to_string())?;
    
    let settings = settings::get_settings().map_err(|e| e.to_string())?;
    let api_key = settings.groq_api_key.ok_or("No Groq API key set")?;
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join("vwisper_audio_latest.wav");
    let id = Uuid::new_v4().to_string();
    let mut wav_path = None;
    if settings.save_history && settings.save_audio {
        // Save audio to audio_out/{id}.wav in config dir
        if let Some(mut config_path) = config_dir() {
            config_path.push("vwisper");
            config_path.push("audio_out");
            std::fs::create_dir_all(&config_path).ok();
            let out_path = config_path.join(format!("{}.wav", id));
            std::fs::copy(&file_path, &out_path).ok();
            wav_path = Some(out_path.to_string_lossy().to_string());
        }
    }
    
    let result = transcription::transcribe_audio(file_path.to_str().unwrap(), &api_key);
    
    if result.status == "success" && !result.text.is_empty() {
        let _ = app.emit_to("main", "transcription-result", &result.text);
        
        // Restore focus to the original window before injecting text
        if let Some(restore_fn) = restore_focus {
            restore_fn();
            // Give the window a moment to gain focus
            std::thread::sleep(Duration::from_millis(100));
        }
        
        match textinjection::inject_text(&result.text) {
            Ok(_) => {
                let _ = app.emit_to("main", "injection-status", "success");
            }
            Err(e) => {
                eprintln!("Text injection failed: {}", e);
                let _ = app.emit_to("main", "injection-status", "error");
                let _ = app.emit_to("main", "injection-error", &e.to_string());
            }
        }
    } else {
        let error_msg = result.error.unwrap_or_else(|| "Transcription failed".to_string());
        eprintln!("Transcription failed: {}", error_msg);
        let _ = app.emit_to("main", "transcription-error", &error_msg);
    }
    
    if settings.save_history {
        let history = HISTORY.get_or_init(History::new);
        history.add_entry(TranscriptionEntry {
            id,
            timestamp: Utc::now(),
            text: result.text.clone(),
            round_trip_ms: Some(result.round_trip_ms),
            hold_time_ms: hold_time_ms,
            status: result.status.clone(),
            wav_path,
        });
    } else {
        let _ = std::fs::remove_file(&file_path);
    }
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .on_window_event(|window, event| {
            if window.label() == "dashboard" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    window.hide().unwrap();
                    api.prevent_close();
                }
            }
        })
        .setup(|app| {
            #[cfg(desktop)]
            let _ = app.handle().plugin(tauri_plugin_autostart::init(
                tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                Some(vec![]),
            ));
            
            let _tray = tray::create_system_tray(&app.handle());
            if let Some(window) = app.get_webview_window("main") {
                if let Some(monitor) = window.current_monitor().unwrap() {
                    let monitor_size = monitor.size();
                    let window_size = window.outer_size().unwrap();
                    let padding = 80;
                    let x = (monitor_size.width / 2).saturating_sub(window_size.width / 2);
                    let y = monitor_size.height.saturating_sub(window_size.height + padding);
                    let _ = window.set_position(PhysicalPosition::new(x as i32, y as i32));
                }
            }
            
            let app_handle = app.handle().clone();
            audio::start_audio_capture(app_handle);
            
            if let Err(e) = textinjection::init_text_injector() {
                eprintln!("Failed to initialize text injector: {}", e);
            }
            
            #[cfg(target_os = "windows")]
            {
                let app_handle = app.handle().clone();
                platform::windows::start_global_key_monitor(app_handle);
            }
            #[cfg(target_os = "macos")]
            {
                let app_handle = app.handle().clone();
                platform::macos::start_global_key_monitor(app_handle);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_audio_recording,
            stop_audio_recording,
            manual_stop_recording,
            settings::get_settings,
            settings::save_settings,
            settings::reset_settings,
            settings::get_settings_path,
            get_transcription_history,
            get_audio_base64,
            inject_text_manual,
            get_text_injector_status,
            test_text_injection,
            check_for_updates,
            download_and_install_update
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[command]
fn start_audio_recording() -> Result<(), String> {
    audio::start_recording().map_err(|e| e.to_string())
}

#[command]
fn stop_audio_recording(app: tauri::AppHandle) -> Result<(), String> {
    handle_stop_recording_workflow(&app, None, None)
}

#[command]
fn manual_stop_recording(app: tauri::AppHandle) -> Result<(), String> {
    // Check if recording is actually in progress
    if !audio::is_recording() {
        return Ok(());
    }
    
    // Stop the audio recording first
    audio::stop_recording().map_err(|e| e.to_string())?;
    
    // Emit loading state
    let _ = app.emit_to("main", "pill-state", "loading");
    
    // Handle the stop recording workflow in a separate thread
    let app_handle_clone = app.clone();
    std::thread::spawn(move || {
        let result = handle_stop_recording_workflow(&app_handle_clone, None, None);
        if let Err(e) = result {
            eprintln!("Error in handle_stop_recording_workflow: {}", e);
            let _ = app_handle_clone.emit_to("main", "pill-state", "error");
            // Keep error state visible for 3 seconds before hiding
            std::thread::sleep(Duration::from_secs(3));
        }
        let _ = app_handle_clone.emit_to("main", "pill-state", "idle");
        if let Some(window) = app_handle_clone.get_webview_window("main") {
            let _ = window.hide();
        }
    });
    
    Ok(())
}

#[command]
fn get_transcription_history() -> Vec<TranscriptionEntry> {
    let history = HISTORY.get_or_init(History::new);
    history.get_entries()
}

#[tauri::command]
fn get_audio_base64(path: String) -> Result<String, String> {
    let data = fs::read(path).map_err(|e| e.to_string())?;
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine;
    Ok(format!("data:audio/wav;base64,{}", STANDARD.encode(data)))
}

#[command]
fn inject_text_manual(text: String) -> Result<(), String> {
    textinjection::inject_text(&text).map_err(|e| e.to_string())
}

#[command]
fn get_text_injector_status() -> Result<bool, String> {
    Ok(textinjection::is_text_injector_initialized())
}

#[command]
fn test_text_injection() -> Result<(), String> {
    textinjection::test_text_injection().map_err(|e| e.to_string())
}

#[command]
fn check_for_updates() -> Result<update::UpdateInfo, String> {
    update::check_for_updates().map_err(|e| e.to_string())
}

#[command]
fn download_and_install_update(download_url: String, app: tauri::AppHandle) -> Result<update::UpdateResult, String> {
    let result = update::download_and_install_update(download_url).map_err(|e| e.to_string())?;
    
    if result.success {
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(2));
            app.exit(0);
        });
    }
    
    Ok(result)
}
