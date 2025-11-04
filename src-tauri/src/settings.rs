use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use dirs::config_dir;
use tauri::command;

#[derive(Serialize, Deserialize, Default)]
pub struct Settings {
    pub groq_api_key: Option<String>,
    pub save_history: bool,
    pub save_audio: bool,
    pub mac_restart_warning_shown: bool,
}

fn settings_path() -> PathBuf {
    let mut path = config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("vwisper");
    fs::create_dir_all(&path).ok();
    path.push("settings.json");
    path
}

fn load_settings() -> Settings {
    let path = settings_path();
    if let Ok(data) = fs::read_to_string(path) {
        serde_json::from_str(&data).unwrap_or(Settings { 
            save_history: true, 
            save_audio: true, 
            mac_restart_warning_shown: false,
            ..Default::default() 
        })
    } else {
        Settings { 
            save_history: true, 
            save_audio: true, 
            mac_restart_warning_shown: false,
            ..Default::default() 
        }
    }
}

fn save_settings_to_file(settings: &Settings) -> Result<(), String> {
    let path = settings_path();
    let data = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| e.to_string())
}

#[command]
pub fn get_settings() -> Result<Settings, String> {
    Ok(load_settings())
}

#[command]
pub fn save_settings(groq_api_key: String, save_history: Option<bool>, save_audio: Option<bool>) -> Result<(), String> {
    let mut settings = load_settings();
    settings.groq_api_key = Some(groq_api_key);
    if let Some(val) = save_history {
        settings.save_history = val;
    }
    if let Some(val) = save_audio {
        settings.save_audio = val;
    }
    save_settings_to_file(&settings)
}

#[command]
pub fn mark_mac_restart_warning_shown() -> Result<(), String> {
    let mut settings = load_settings();
    settings.mac_restart_warning_shown = true;
    save_settings_to_file(&settings)
}

#[command]
pub fn reset_settings() -> Result<(), String> {
    let mut settings = load_settings();
    settings.groq_api_key = None;
    settings.save_history = true;
    settings.save_audio = true;
    settings.mac_restart_warning_shown = false;
    save_settings_to_file(&settings)
}

#[command]
pub fn get_settings_path() -> Result<String, String> {
    Ok(settings_path().to_string_lossy().to_string())
}
