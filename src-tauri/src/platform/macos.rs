// macOS Fn monitor moved to crate::fn_monitor_macos; this module can be left empty or retain non-mac paths.
use tauri::AppHandle;
pub fn start_global_key_monitor(_app_handle: AppHandle) {}
