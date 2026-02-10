//! Renzora Updater
//!
//! External updater binary that handles update installation on Windows.
//! This is necessary because a running executable cannot replace itself.
//!
//! Usage: update.exe <new_exe> <target_exe> <editor_pid>
//!
//! The updater will:
//! 1. Wait for the editor process to exit
//! 2. Back up the current editor exe
//! 3. Copy the new exe over the old one
//! 4. Relaunch the editor
//! 5. Clean up the backup and downloaded file on success

#![windows_subsystem = "windows"]

use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::Duration;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        show_error("Invalid arguments. Usage: update <new_exe> <target_exe> <pid>");
        return;
    }

    let new_exe = Path::new(&args[1]);
    let target_exe = Path::new(&args[2]);
    let pid: u32 = match args[3].parse() {
        Ok(p) => p,
        Err(_) => {
            show_error("Invalid PID");
            return;
        }
    };

    if let Err(e) = perform_update(new_exe, target_exe, pid) {
        show_error(&format!("Update failed: {}\n\nThe original application should still work.", e));
    }
}

fn perform_update(new_exe: &Path, target_exe: &Path, pid: u32) -> Result<(), String> {
    // Wait for the editor process to exit
    wait_for_process_exit(pid)?;

    // Small delay to ensure file handles are released
    thread::sleep(Duration::from_millis(500));

    // Back up the current editor exe before overwriting
    let backup_path = target_exe.with_extension("exe.backup");

    if target_exe.exists() {
        let _ = fs::remove_file(&backup_path);
        fs::rename(target_exe, &backup_path)
            .map_err(|e| format!("Failed to backup current exe: {}", e))?;
    }

    // Copy the new exe to the target location
    match fs::copy(new_exe, target_exe) {
        Ok(_) => {}
        Err(e) => {
            // Try to restore backup
            if backup_path.exists() {
                let _ = fs::rename(&backup_path, target_exe);
            }
            return Err(format!("Failed to install update: {}", e));
        }
    }

    // Clean up backup and downloaded file
    let _ = fs::remove_file(&backup_path);
    let _ = fs::remove_file(new_exe);

    // Relaunch the editor
    launch_editor(target_exe)?;

    Ok(())
}

#[cfg(windows)]
fn wait_for_process_exit(pid: u32) -> Result<(), String> {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE, INFINITE};

    unsafe {
        let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if handle.is_null() {
            // Process might already be gone
            return Ok(());
        }

        let result = WaitForSingleObject(handle, INFINITE);
        CloseHandle(handle);

        if result != WAIT_OBJECT_0 {
            return Err("Failed to wait for editor to exit".to_string());
        }
    }

    Ok(())
}

#[cfg(not(windows))]
fn wait_for_process_exit(pid: u32) -> Result<(), String> {
    // On non-Windows, just poll for process existence
    for _ in 0..300 {
        // 30 seconds max
        if !process_exists(pid) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err("Timeout waiting for editor to exit".to_string())
}

#[cfg(not(windows))]
fn process_exists(pid: u32) -> bool {
    use std::path::Path;
    Path::new(&format!("/proc/{}", pid)).exists()
}

fn launch_editor(exe_path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        const DETACHED_PROCESS: u32 = 0x00000008;
        Command::new(exe_path)
            .creation_flags(DETACHED_PROCESS)
            .spawn()
            .map_err(|e| format!("Failed to launch editor: {}", e))?;
    }

    #[cfg(not(windows))]
    {
        Command::new(exe_path)
            .spawn()
            .map_err(|e| format!("Failed to launch editor: {}", e))?;
    }

    Ok(())
}

#[cfg(windows)]
fn show_error(message: &str) {
    use std::ffi::OsStr;
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::null_mut;
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

    let title: Vec<u16> = OsStr::new("Renzora Updater")
        .encode_wide()
        .chain(once(0))
        .collect();

    let msg: Vec<u16> = OsStr::new(message)
        .encode_wide()
        .chain(once(0))
        .collect();

    unsafe {
        MessageBoxW(null_mut(), msg.as_ptr(), title.as_ptr(), MB_ICONERROR | MB_OK);
    }
}

#[cfg(not(windows))]
fn show_error(message: &str) {
    eprintln!("Error: {}", message);
}
