// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::{Command, Stdio};
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
            
#[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x08000000;
                
                if let Err(e) = Command::new(bridge_exe)
                    .creation_flags(CREATE_NO_WINDOW)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .stdin(Stdio::null())
                    .spawn() {
                    eprintln!("Failed to start bridge server: {}", e);
                }
            }
            #[cfg(not(windows))]
            {
                if let Err(e) = Command::new(bridge_exe)
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .stdin(Stdio::null())
                    .spawn() {
                    eprintln!("Failed to start bridge server: {}", e);
                }
            }
        }
    });
}


fn main() {
    // Start bridge server
    start_bridge_server();

    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}