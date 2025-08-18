use serde::{Deserialize, Serialize};
use std::fs;
use std::process::Command;
use std::env;
use reqwest::blocking::Client;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub download_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateResult {
    pub success: bool,
    pub message: String,
}

pub fn get_current_version() -> Result<String, String> {
    Ok("1.0.5".to_string())
}

fn get_platform_download_url(version: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        format!(
            "https://github.com/xptea/VWisper/releases/download/{}/vwisper_{}_x64-setup.exe",
            version, version
        )
    }
    
    #[cfg(target_os = "macos")]
    {
        format!(
            "https://github.com/xptea/VWisper/releases/download/{}/vwisper_{}_x64.dmg",
            version, version
        )
    }
    
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        format!(
            "https://github.com/xptea/VWisper/releases/download/{}/vwisper_{}_x64-setup.exe",
            version, version
        )
    }
}

fn get_installer_filename() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "vwisper_update_installer.exe"
    }
    
    #[cfg(target_os = "macos")]
    {
        "vwisper_update_installer.dmg"
    }
    
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        "vwisper_update_installer.exe"
    }
}

fn install_update(installer_path: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let _result = Command::new(installer_path)
            .spawn()
            .map_err(|e| format!("Failed to start installer: {}", e))?;
        
        std::thread::sleep(std::time::Duration::from_millis(1000));
        Ok(())
    }
    
    #[cfg(target_os = "macos")]
    {
        let _result = Command::new("open")
            .arg(installer_path)
            .spawn()
            .map_err(|e| format!("Failed to open DMG file: {}", e))?;
        
        std::thread::sleep(std::time::Duration::from_millis(1000));
        Ok(())
    }
    
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _result = Command::new(installer_path)
            .spawn()
            .map_err(|e| format!("Failed to start installer: {}", e))?;
        
        std::thread::sleep(std::time::Duration::from_millis(1000));
        Ok(())
    }
}

pub fn check_for_updates() -> Result<UpdateInfo, String> {
    let current_version = get_current_version()?;
    
    let client = Client::new();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let random = (timestamp % 1000000) as u32;
    let url = format!("https://raw.githubusercontent.com/xptea/VWisper/refs/heads/main/src/version.txt?t={}&r={}", timestamp, random);
    
    let response = client
        .get(&url)
        .header("Cache-Control", "no-cache, no-store, must-revalidate")
        .header("Pragma", "no-cache")
        .header("Expires", "0")
        .header("User-Agent", "VWisper-Update-Checker/1.0")
        .send()
        .map_err(|e| format!("Failed to fetch latest version: {}", e))?;
    
    if !response.status().is_success() {
        return Err("Failed to fetch latest version from GitHub".to_string());
    }
    
    let latest_version = response
        .text()
        .map_err(|e| format!("Failed to read response: {}", e))?
        .trim()
        .to_string();
    
    println!("Update check - Current: {}, Latest: {}", current_version, latest_version);
    
    let has_update = compare_versions(&current_version, &latest_version) < 0;
    let download_url = if has_update {
        Some(get_platform_download_url(&latest_version))
    } else {
        None
    };
    
    Ok(UpdateInfo {
        current_version,
        latest_version,
        has_update,
        download_url,
    })
}

pub fn download_and_install_update(download_url: String) -> Result<UpdateResult, String> {
    let client = Client::new();
    
    let response = client
        .get(&download_url)
        .send()
        .map_err(|e| format!("Failed to download update: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("Failed to download update: HTTP {}", response.status()));
    }
    
    let temp_dir = env::temp_dir();
    let installer_filename = get_installer_filename();
    let installer_path = temp_dir.join(installer_filename);
    
    let mut file = fs::File::create(&installer_path)
        .map_err(|e| format!("Failed to create installer file: {}", e))?;
    
    let bytes = response
        .bytes()
        .map_err(|e| format!("Failed to read response bytes: {}", e))?;
    
    file.write_all(&bytes)
        .map_err(|e| format!("Failed to write installer file: {}", e))?;
    
    drop(file);
    
    let installer_path_str = installer_path.to_string_lossy().to_string();
    
    install_update(&installer_path_str)?;
    
    Ok(UpdateResult {
        success: true,
        message: "Update installer started successfully. The application will close and the installer will handle the update.".to_string(),
    })
}

fn compare_versions(current: &str, latest: &str) -> i32 {
    let current_parts: Vec<u32> = current
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    
    let latest_parts: Vec<u32> = latest
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    
    let max_len = current_parts.len().max(latest_parts.len());
    
    for i in 0..max_len {
        let current_part = current_parts.get(i).unwrap_or(&0);
        let latest_part = latest_parts.get(i).unwrap_or(&0);
        
        if current_part < latest_part {
            return -1;
        } else if current_part > latest_part {
            return 1;
        }
    }
    
    0
} 