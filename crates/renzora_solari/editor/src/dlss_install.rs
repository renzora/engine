//! The Solari inspector's **Install DLSS** button: fetching NVIDIA's DLSS
//! libraries into `~/.renzora/dlss/` for a build that has DLSS compiled in.
//!
//! Offered only where it would help: a build with the `dlss` feature, an
//! NVIDIA GPU, and the libraries nowhere the loader looks. Anywhere else the
//! download would sit unused (no DLSS code to load it, or no GPU it runs on).
//! Where they go and why is `renzora::dlss_runtime`.
//!
//! DLSS support is decided while the GPU is set up, so a fresh install takes
//! effect on the next launch, and the inspector then offers the restart.

use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bevy::prelude::*;
use bevy::render::renderer::RenderAdapterInfo;
use renzora::dlss_runtime::{self, DlssInstall, Library};
use renzora::DlssSupport;
use sha2::{Digest, Sha256};

/// NVIDIA's PCI vendor id.
const NVIDIA: u32 = 0x10DE;

/// The install thread's progress, shared with [`sync_install_state`].
#[derive(Resource, Default)]
pub(crate) struct InstallTask(Arc<Mutex<DlssInstall>>);

/// Whether this run could use DLSS once the libraries are installed, and they
/// are not: the inspector's cue to offer them.
pub(crate) fn install_offered(world: &World) -> bool {
    let support = world.get_resource::<DlssSupport>().copied().unwrap_or_default();
    let nvidia = world
        .get_resource::<RenderAdapterInfo>()
        .is_some_and(|info| info.vendor == NVIDIA);
    support.compiled
        && !support.ray_reconstruction
        && nvidia
        && !dlss_runtime::libraries().is_empty()
        && !dlss_runtime::libraries_present()
}

/// Start the download, unless one is already running or done.
pub(crate) fn start_install(world: &mut World) {
    let state = world.get_resource_or_insert_with(InstallTask::default).0.clone();
    {
        let mut current = state.lock().unwrap_or_else(|e| e.into_inner());
        if matches!(*current, DlssInstall::Downloading { .. } | DlssInstall::Installed) {
            return;
        }
        let total = dlss_runtime::libraries().iter().map(|l| l.size).sum();
        *current = DlssInstall::Downloading { done: 0, total };
    }
    std::thread::spawn(move || {
        let result = install(&state);
        let mut current = state.lock().unwrap_or_else(|e| e.into_inner());
        *current = match result {
            Ok(()) => DlssInstall::Installed,
            Err(why) => DlssInstall::Failed(why),
        };
    });
}

/// Copy the thread's progress into [`DlssInstall`], only when it moved, so the
/// inspector redraws while the bar fills and not otherwise.
pub(crate) fn sync_install_state(task: Option<Res<InstallTask>>, mut shown: ResMut<DlssInstall>) {
    let Some(task) = task else { return };
    let now = task.0.lock().unwrap_or_else(|e| e.into_inner()).clone();
    shown.set_if_neq(now);
}

/// Download and check every library, then NVIDIA's licence beside them.
fn install(state: &Arc<Mutex<DlssInstall>>) -> Result<(), String> {
    let dir = dlss_runtime::install_dir().ok_or("there is no home folder to install DLSS into")?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("could not create {} ({e})", dir.display()))?;
    let mut done = 0u64;
    for library in dlss_runtime::libraries() {
        fetch(library, &dir, &mut done, state)?;
    }
    // The licence must travel with the libraries. Text, so not checksummed:
    // a line-ending conversion anywhere on the way would fail a hash and
    // change nothing that matters.
    if let Some(root) = dlss_runtime::install_root() {
        if let Ok(response) = renzora::net::Request::get(&dlss_runtime::license_url()).send() {
            if response.is_ok() {
                let _ = std::fs::write(root.join("LICENSE.txt"), response.text());
            }
        }
    }
    Ok(())
}

/// Fetch one library to a temporary name, check it, then move it into place.
/// A library that fails its check is deleted, never loaded.
fn fetch(
    library: &Library,
    dir: &Path,
    done: &mut u64,
    state: &Arc<Mutex<DlssInstall>>,
) -> Result<(), String> {
    let url = dlss_runtime::library_url(library).ok_or("DLSS is not available on this platform")?;
    let partial = dir.join(format!("{}.partial", library.name));
    let mut file = std::fs::File::create(&partial)
        .map_err(|e| format!("could not write {} ({e})", partial.display()))?;
    // A 60 MB file on a slow line is minutes, not the default minute.
    let mut stream = renzora::net::Request::get(&url)
        .timeout(Duration::from_secs(30 * 60))
        .max_bytes(library.size as u32 + 1024 * 1024)
        .send_stream()
        .map_err(|e| format!("the download failed ({e})"))?;
    let mut hasher = Sha256::new();
    for chunk in &mut stream {
        hasher.update(&chunk.data);
        file.write_all(&chunk.data)
            .map_err(|e| format!("could not write {} ({e})", partial.display()))?;
        *done += chunk.data.len() as u64;
        let mut current = state.lock().unwrap_or_else(|e| e.into_inner());
        if let DlssInstall::Downloading { done: shown, .. } = &mut *current {
            *shown = *done;
        }
    }
    drop(file);
    let fail = |why: String| {
        let _ = std::fs::remove_file(&partial);
        Err(why)
    };
    if let Some(e) = stream.error() {
        return fail(format!("the download failed ({e})"));
    }
    if stream.status() != 200 {
        return fail(format!("NVIDIA's server answered HTTP {}", stream.status()));
    }
    let got = hex(&hasher.finalize());
    if got != library.sha256 {
        return fail(format!("{} did not match its checksum, so it was not installed", library.name));
    }
    let target = dir.join(library.name);
    std::fs::rename(&partial, &target)
        .or_else(|_| fail(format!("could not move {} into place", library.name)))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
