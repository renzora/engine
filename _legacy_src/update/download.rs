//! Update download with progress tracking

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use super::UpdateState;

/// Get the updates directory path
fn updates_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|p| p.join("renzora").join("updates"))
}

/// Start downloading an update in a background thread
pub fn start_download(state: &mut UpdateState, url: &str, version: &str) {
    if state.downloading {
        return;
    }

    let Some(updates_dir) = updates_dir() else {
        state.error = Some("Could not determine updates directory".to_string());
        return;
    };

    // Create updates directory if it doesn't exist
    if let Err(e) = fs::create_dir_all(&updates_dir) {
        state.error = Some(format!("Could not create updates directory: {}", e));
        return;
    }

    let download_path = updates_dir.join(format!("renzora_update_{}.exe", version));

    // Set up shared state for progress tracking
    let progress = Arc::new(AtomicU64::new(0));
    let total = Arc::new(AtomicU64::new(0));
    let complete = Arc::new(AtomicBool::new(false));
    let error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    state.download_progress_shared = Some(Arc::clone(&progress));
    state.download_total_shared = Some(Arc::clone(&total));
    state.download_complete = Some(Arc::clone(&complete));
    state.download_error = Some(Arc::clone(&error));
    state.downloading = true;
    state.download_progress = Some(0.0);

    // The downloaded path is the .exe itself
    state.downloaded_path = Some(download_path.clone());

    let url = url.to_string();

    std::thread::spawn(move || {
        let result = perform_download(&url, &download_path, &progress, &total);

        if let Err(e) = result {
            if let Ok(mut guard) = error.lock() {
                *guard = Some(e);
            }
            let _ = fs::remove_file(&download_path);
        }

        complete.store(true, Ordering::SeqCst);
    });
}

/// Perform the actual download (runs in background thread)
fn perform_download(
    url: &str,
    path: &PathBuf,
    progress: &Arc<AtomicU64>,
    total: &Arc<AtomicU64>,
) -> Result<(), String> {
    let response = ureq::get(url)
        .set("User-Agent", "renzora-editor")
        .call()
        .map_err(|e| format!("Download failed: {}", e))?;

    let content_length: u64 = response
        .header("Content-Length")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    total.store(content_length, Ordering::SeqCst);

    let mut file = File::create(path)
        .map_err(|e| format!("Could not create download file: {}", e))?;

    let mut reader = response.into_reader();
    let mut buffer = [0u8; 8192];
    let mut downloaded: u64 = 0;

    loop {
        let bytes_read = reader
            .read(&mut buffer)
            .map_err(|e| format!("Error reading download: {}", e))?;

        if bytes_read == 0 {
            break;
        }

        file.write_all(&buffer[..bytes_read])
            .map_err(|e| format!("Error writing download: {}", e))?;

        downloaded += bytes_read as u64;
        progress.store(downloaded, Ordering::SeqCst);
    }

    file.flush()
        .map_err(|e| format!("Error finalizing download: {}", e))?;

    Ok(())
}

/// Launch the updater executable and signal it to perform the update
pub fn launch_updater(new_exe: &PathBuf) -> Result<(), String> {
    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Could not determine current exe path: {}", e))?;

    let exe_dir = current_exe
        .parent()
        .ok_or("Could not determine editor directory")?;

    // The updater lives next to the main binary as update.exe
    let updater_path = exe_dir.join("update.exe");

    if !updater_path.exists() {
        return Err("Updater not found. Please update manually.".to_string());
    }

    let pid = std::process::id();

    // Launch updater with arguments: <new_exe> <target_exe> <pid>
    std::process::Command::new(&updater_path)
        .arg(new_exe)
        .arg(&current_exe)
        .arg(pid.to_string())
        .spawn()
        .map_err(|e| format!("Failed to launch updater: {}", e))?;

    // Exit the editor - the updater will wait for us to die, then replace the exe
    std::process::exit(0);
}
