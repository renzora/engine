// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::Command;
use std::thread;

fn start_bridge_server() {
    thread::spawn(|| {
        if cfg!(debug_assertions) {
            // Development mode - bridge is already started by beforeDevCommand
            return;
        } else {
            // Production mode - use bundled executable  
            #[cfg(windows)]
            let bridge_exe = "bridge-server.exe";
            #[cfg(not(windows))]
            let bridge_exe = "bridge-server";
            
            if let Err(e) = Command::new(bridge_exe).spawn() {
                eprintln!("Failed to start bridge server: {}", e);
            }
        }
    });
}

fn main() {
    // Start bridge server
    start_bridge_server();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}