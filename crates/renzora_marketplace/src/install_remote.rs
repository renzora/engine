//! Installing from the marketplace in the companion process.
//!
//! An install downloads the whole asset into memory, unpacks it, and for a
//! plugin compiles it with rustc, which takes from seconds to minutes. It ran
//! on a thread of the editor's: the archive sat in the editor's memory and
//! rustc ran as the editor's child at the editor's priority. In the companion,
//! which runs at lower priority, all of that is out of the editor's process.
//!
//! The install overlay does not change. Its thread hands the install to the
//! companion and turns each progress report back into the phase and byte
//! counts its bar reads. With no companion connected the thread installs
//! itself, as before. Either way the same [`run_install`] does the work.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use serde::{Deserialize, Serialize};

use crate::auth::marketplace::AssetSummary;
use crate::auth::session::AuthSession;
use crate::install_overlay::{run_install, InstallShared, Phase};

/// The companion job kind.
pub const KIND: &str = "marketplace_install";

/// What the editor sends. Only the access token travels, not the session, as
/// for an upload.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InstallArgs {
    /// Set when signed in: the install uses the authenticated download.
    pub access_token: Option<String>,
    pub asset: AssetSummary,
    pub dest: PathBuf,
}

/// What comes back.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct InstallDone {
    /// The sentence the install notice shows.
    pub message: String,
    /// Where an asset landed, for the editor to import a model from.
    pub installed: Option<PathBuf>,
}

/// A progress note: the phase's word first, so the companion's job list reads
/// as a sentence and the editor can read it back.
fn note(phase: Phase, bytes: u64, total: u64) -> String {
    match phase {
        Phase::Resolving => "Resolving the download".to_string(),
        Phase::Downloading => format!("Downloading {bytes} of {total} bytes"),
        Phase::Writing => "Unpacking".to_string(),
        Phase::Building => "Compiling".to_string(),
    }
}

/// Read a [`note`] back.
fn read_note(note: &str) -> Option<(Phase, u64, u64)> {
    let mut words = note.split_whitespace();
    match words.next()? {
        "Resolving" => Some((Phase::Resolving, 0, 0)),
        "Downloading" => {
            let bytes = words.next()?.parse().ok()?;
            let total = words.nth(1)?.parse().ok()?;
            Some((Phase::Downloading, bytes, total))
        }
        "Unpacking" => Some((Phase::Writing, 0, 0)),
        "Compiling" => Some((Phase::Building, 0, 0)),
        _ => None,
    }
}

/// The editor's side: run the install in the companion, keeping `shared` up
/// to date for the overlay's bar. `None` when no companion is connected.
pub(crate) fn run(
    remote: &renzora::CompanionRemote,
    access_token: Option<String>,
    asset: &AssetSummary,
    dest: &Path,
    shared: &InstallShared,
) -> Option<Result<String, String>> {
    let args = InstallArgs { access_token, asset: asset.clone(), dest: dest.to_path_buf() };
    let mut on_progress = |_: Option<f32>, note: &str| {
        if let Some((phase, bytes, total)) = read_note(note) {
            shared.phase.store(phase as u8, Ordering::Relaxed);
            shared.bytes.store(bytes, Ordering::Relaxed);
            if total > 0 {
                shared.total.store(total, Ordering::Relaxed);
            }
        }
    };
    let answer = remote.run(KIND, asset.name.clone(), &args, &mut on_progress)?;
    Some(answer.and_then(|json| {
        let done: InstallDone = serde_json::from_str(&json).map_err(|e| format!("the install's answer was unreadable: {e}"))?;
        if let Ok(mut slot) = shared.installed.lock() {
            *slot = done.installed;
        }
        Ok(done.message)
    }))
}

/// The companion's side: install what `payload` describes, reporting through
/// `progress`. Answers with an [`InstallDone`] as JSON.
pub fn run_in_companion(payload: &str, progress: &mut dyn FnMut(Option<f32>, &str)) -> Result<String, String> {
    let args: InstallArgs =
        serde_json::from_str(payload).map_err(|e| format!("could not read the install's arguments: {e}"))?;
    let session = args
        .access_token
        .clone()
        .map(|token| AuthSession { user: None, access_token: Some(token), refresh_token: None });
    let shared = InstallShared::default();
    let result = std::thread::scope(|scope| {
        let worker = scope.spawn(|| run_install(session.as_ref(), &args.asset, &args.dest, &shared));
        // Sampled, as the overlay's bar samples it: only the latest matters.
        let mut last = String::new();
        while !worker.is_finished() {
            let phase = Phase::from_u8(shared.phase.load(Ordering::Relaxed));
            let (bytes, total) = (shared.bytes.load(Ordering::Relaxed), shared.total.load(Ordering::Relaxed));
            let now = note(phase, bytes, total);
            if now != last {
                let fraction = (phase == Phase::Downloading && total > 0).then(|| bytes as f32 / total as f32);
                progress(fraction, &now);
                last = now;
            }
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
        worker.join().unwrap_or_else(|_| Err("the install stopped unexpectedly".into()))
    })?;
    let installed = shared.installed.lock().ok().and_then(|p| p.clone());
    serde_json::to_string(&InstallDone { message: result, installed }).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_progress_note_reads_back() {
        for (phase, bytes, total) in [
            (Phase::Resolving, 0, 0),
            (Phase::Downloading, 1234, 5678),
            (Phase::Writing, 0, 0),
            (Phase::Building, 0, 0),
        ] {
            assert_eq!(read_note(&note(phase, bytes, total)), Some((phase, bytes, total)));
        }
        assert_eq!(read_note("something else"), None);
    }
}
