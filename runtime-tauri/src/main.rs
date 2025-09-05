// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn get_project_data() -> Result<String, String> {
    // In a real implementation, this would load the embedded project data
    // For now, return placeholder
    Ok(r#"{"project":{"name":"Runtime Project","version":"1.0.0"}}"#.to_string())
}

#[tauri::command]
fn get_asset_path(relative_path: String) -> Result<String, String> {
    // Convert relative asset path to tauri resource path
    let resource_path = format!("assets/{}", relative_path);
    Ok(resource_path)
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            
            // Set window title to project name when available
            window.set_title("Renzora Runtime").unwrap();
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_project_data,
            get_asset_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}