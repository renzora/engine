//! Project file transfer.
//!
//! A synced scene is a scene full of references — `models/tree.glb`,
//! `textures/bark.png`, `scripts/enemy.lua`. Send only the scene and a guest who
//! has never opened the project gets a world of correctly-placed nothing. This
//! module is what makes an invitation work for someone who has never seen the
//! project before.
//!
//! ## Manifest first, bytes on request
//!
//! The host sends a list — path, size, content hash — and the guest replies with
//! only what it is missing. A project is mostly large binary assets that rarely
//! change, so on a second visit the manifest is nearly all matches and almost
//! nothing moves. The hash is cheap and non-cryptographic on purpose: it decides
//! whether to *transfer* a file, never whether to *trust* one.
//!
//! ## The two rules about writing to someone else's disk
//!
//! Receiving files means a remote machine chooses paths that this one writes to,
//! which is a capability worth being paranoid about.
//!
//! 1. **Every path is resolved inside the project root and rejected if it lands
//!    outside it.** A path of `../../.ssh/authorized_keys` is otherwise a
//!    perfectly ordinary-looking manifest entry.
//! 2. **The transfer is opt-in and never automatic.** The guest sees what would
//!    be written and how much, and asks for it. A session should not be able to
//!    silently push gigabytes into someone's project the moment they connect.
//!
//! Nothing here ever deletes. A file the host does not have is simply left
//! alone: the failure mode of an over-eager sync is destroying work that was
//! never part of the session.

use std::collections::HashMap;
use std::hash::Hasher;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver, Sender};

use crate::protocol::{CollabMsg, FileEntry, MAX_CHUNK};
use crate::session::CollabSession;

/// Directories never worth syncing: build output, version control, and the
/// engine's own caches. Walking them would multiply a project's file count by
/// an order of magnitude for files the far side rebuilds anyway.
const SKIP_DIRS: &[&str] = &[".git", "target", "node_modules", ".renzora", "dist", ".cache"];

/// Files above this are skipped by the manifest walk entirely. A project has no
/// business shipping a single half-gigabyte asset over an editor session, and
/// the failure mode of trying is a link blocked for minutes.
const MAX_FILE: u64 = 512 * 1024 * 1024;

/// Where a transfer has got to.
#[derive(Default, PartialEq, Eq, Clone, Copy, Debug)]
pub enum SyncPhase {
    #[default]
    Idle,
    /// The manifest is being built or compared on a worker thread.
    Comparing,
    /// A comparison finished and is waiting for the user to accept it.
    Offered,
    Transferring,
    Done,
    Failed,
}

#[derive(Resource, Default)]
pub struct FileSync {
    pub phase: SyncPhase,
    /// What the guest is missing, and how much it is.
    pub missing: Vec<FileEntry>,
    pub missing_bytes: u64,
    pub received_files: u64,
    pub received_bytes: u64,
    pub message: String,
    /// Open handles for files mid-transfer, keyed by project-relative path.
    writing: HashMap<String, std::fs::File>,
    /// Results from the comparison worker.
    compare_rx: Option<Receiver<CompareResult>>,
}

struct CompareResult {
    missing: Vec<FileEntry>,
    bytes: u64,
    error: Option<String>,
}

impl FileSync {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Whether there is anything for the user to accept.
    pub fn has_offer(&self) -> bool {
        self.phase == SyncPhase::Offered && !self.missing.is_empty()
    }

    pub fn progress(&self) -> f32 {
        if self.missing_bytes == 0 {
            return 1.0;
        }
        (self.received_bytes as f32 / self.missing_bytes as f32).clamp(0.0, 1.0)
    }
}

// ── Message handling ────────────────────────────────────────────────────────

/// Dispatch a file (or otherwise unhandled) message. Called from the sync
/// inbox drain so ordering against scene messages is preserved.
pub fn handle(world: &mut World, from: u64, msg: CollabMsg) {
    match msg {
        CollabMsg::FileManifest { files } => on_manifest(world, files),
        CollabMsg::FileRequest { paths } => on_request(world, from, paths),
        CollabMsg::FileChunk { path, offset, bytes, last } => {
            on_chunk(world, path, offset, bytes, last)
        }
        CollabMsg::FileTouched { entry } => on_touched(world, entry),
        other => {
            log::debug!("[collab] ignoring {} from peer {from}", other.label());
        }
    }
}

/// Host→guest on join: build the manifest off-thread and send it.
pub fn send_manifest(world: &mut World, peer: u64) {
    let Some(root) = project_root(world) else {
        return;
    };
    let Some(tx) = world.resource::<CollabSession>().sender_for(peer) else {
        return;
    };
    std::thread::Builder::new()
        .name("collab-manifest".into())
        .spawn(move || {
            let files = walk_project(&root);
            log::info!("[collab] manifest: {} files", files.len());
            let _ = tx.send(CollabMsg::FileManifest { files });
        })
        .ok();
}

/// Guest: compare the host's manifest against the local project.
fn on_manifest(world: &mut World, files: Vec<FileEntry>) {
    let Some(root) = project_root(world) else {
        let mut sync = world.resource_mut::<FileSync>();
        sync.phase = SyncPhase::Failed;
        sync.message = "No project open — open one before syncing files".into();
        return;
    };
    let (tx, rx) = unbounded();
    {
        let mut sync = world.resource_mut::<FileSync>();
        sync.reset();
        sync.phase = SyncPhase::Comparing;
        sync.message = format!("Comparing {} files…", files.len());
        sync.compare_rx = Some(rx);
    }
    std::thread::Builder::new()
        .name("collab-compare".into())
        .spawn(move || {
            let mut missing = Vec::new();
            let mut bytes = 0u64;
            let mut error = None;
            for entry in files {
                match safe_join(&root, &entry.path) {
                    None => {
                        // A path that escapes the project root is not a file we
                        // failed to match — it is a manifest we should not be
                        // acting on at all. Report it and stop.
                        error = Some(format!("host sent an unsafe path: {}", entry.path));
                        missing.clear();
                        bytes = 0;
                        break;
                    }
                    Some(local) => {
                        let same = std::fs::metadata(&local)
                            .ok()
                            .filter(|m| m.len() == entry.size)
                            .is_some_and(|_| hash_file(&local) == Some(entry.hash));
                        if !same {
                            bytes += entry.size;
                            missing.push(entry);
                        }
                    }
                }
            }
            let _ = tx.send(CompareResult { missing, bytes, error });
        })
        .ok();
}

/// Pick up a finished comparison.
pub fn poll_compare(mut sync: ResMut<FileSync>, mut session: ResMut<CollabSession>) {
    let Some(rx) = sync.compare_rx.as_ref() else {
        return;
    };
    let Ok(result) = rx.try_recv() else {
        return;
    };
    sync.compare_rx = None;
    if let Some(error) = result.error {
        sync.phase = SyncPhase::Failed;
        sync.message = error.clone();
        session.note(error);
        return;
    }
    if result.missing.is_empty() {
        sync.phase = SyncPhase::Done;
        sync.message = "Project files already match".into();
        session.note("project files already match the host");
        return;
    }
    sync.missing_bytes = result.bytes;
    sync.message =
        format!("{} files to fetch ({})", result.missing.len(), human_bytes(result.bytes));
    sync.missing = result.missing;
    sync.phase = SyncPhase::Offered;
}

/// The guest accepted the offer — ask for the files.
pub fn accept_offer(sync: &mut FileSync, session: &CollabSession) {
    if !sync.has_offer() {
        return;
    }
    let paths: Vec<String> = sync.missing.iter().map(|f| f.path.clone()).collect();
    sync.phase = SyncPhase::Transferring;
    sync.message = format!("Fetching {} files…", paths.len());
    session.send_up(CollabMsg::FileRequest { paths });
}

/// Host: stream the requested files.
fn on_request(world: &mut World, peer: u64, paths: Vec<String>) {
    let Some(root) = project_root(world) else {
        return;
    };
    let Some(tx) = world.resource::<CollabSession>().sender_for(peer) else {
        return;
    };
    world
        .resource_mut::<CollabSession>()
        .note(format!("sending {} files", paths.len()));
    std::thread::Builder::new()
        .name("collab-send-files".into())
        .spawn(move || {
            for path in paths {
                // The *sender* validates too. A request is a remote-chosen path
                // just as a manifest entry is, and this is the side where a
                // traversal would read a file rather than write one.
                let Some(full) = safe_join(&root, &path) else {
                    log::warn!("[collab] refusing to send unsafe path {path}");
                    continue;
                };
                if let Err(e) = stream_file(&tx, &path, &full) {
                    log::warn!("[collab] could not send {path}: {e}");
                }
            }
        })
        .ok();
}

fn stream_file(tx: &Sender<CollabMsg>, rel: &str, full: &Path) -> std::io::Result<()> {
    let mut file = std::fs::File::open(full)?;
    let len = file.metadata()?.len();
    let mut offset = 0u64;
    let mut buf = vec![0u8; MAX_CHUNK];
    loop {
        let read = file.read(&mut buf)?;
        let last = offset + read as u64 >= len;
        if tx
            .send(CollabMsg::FileChunk {
                path: rel.to_string(),
                offset,
                bytes: buf[..read].to_vec(),
                last,
            })
            .is_err()
        {
            return Ok(()); // link gone
        }
        offset += read as u64;
        if read == 0 || last {
            return Ok(());
        }
    }
}

/// Guest: write one chunk.
fn on_chunk(world: &mut World, path: String, offset: u64, bytes: Vec<u8>, last: bool) {
    let Some(root) = project_root(world) else {
        return;
    };
    let Some(full) = safe_join(&root, &path) else {
        let mut session = world.resource_mut::<CollabSession>();
        session.note(format!("refused an unsafe path from the host: {path}"));
        return;
    };

    let mut sync = world.resource_mut::<FileSync>();
    let written = bytes.len() as u64;
    let result = (|| -> std::io::Result<()> {
        if !sync.writing.contains_key(&path) {
            if let Some(parent) = full.parent() {
                std::fs::create_dir_all(parent)?;
            }
            // Truncating on the first chunk: a partial file from an interrupted
            // earlier transfer must not be appended to, or the result is a file
            // that is the right length and wrong content.
            let file = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&full)?;
            sync.writing.insert(path.clone(), file);
        }
        let file = sync.writing.get_mut(&path).expect("just inserted");
        file.seek(SeekFrom::Start(offset))?;
        file.write_all(&bytes)?;
        Ok(())
    })();

    if let Err(e) = result {
        sync.phase = SyncPhase::Failed;
        sync.message = format!("Could not write {path}: {e}");
        return;
    }

    sync.received_bytes += written;
    if last {
        sync.writing.remove(&path);
        sync.received_files += 1;
        sync.missing.retain(|f| f.path != path);
        if sync.missing.is_empty() {
            sync.phase = SyncPhase::Done;
            sync.message = format!(
                "Synced {} files ({})",
                sync.received_files,
                human_bytes(sync.received_bytes)
            );
        } else {
            let left = sync.missing.len();
            sync.message = format!("Fetching… {left} files left");
        }
    }
}

/// The host saved something; note it so the guest can pick it up.
fn on_touched(world: &mut World, entry: FileEntry) {
    let mut sync = world.resource_mut::<FileSync>();
    if sync.phase == SyncPhase::Transferring {
        // Already fetching — fold it into the run in progress.
        if !sync.missing.iter().any(|f| f.path == entry.path) {
            sync.missing_bytes += entry.size;
            sync.missing.push(entry);
        }
        return;
    }
    if !sync.missing.iter().any(|f| f.path == entry.path) {
        sync.missing_bytes += entry.size;
        sync.missing.push(entry);
        sync.phase = SyncPhase::Offered;
        let n = sync.missing.len();
        sync.message = format!("{n} updated files available");
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn project_root(world: &World) -> Option<PathBuf> {
    world.get_resource::<renzora::core::CurrentProject>().map(|p| p.path.clone())
}

/// Resolve a project-relative path inside `root`, or `None` if it escapes.
///
/// Checked structurally (rejecting `..`, absolute paths and Windows path
/// prefixes) rather than by canonicalizing, because the file usually does not
/// exist yet on the receiving side and `canonicalize` fails on a path that isn't
/// there. The structural check is also the stricter of the two: it refuses a
/// traversal even when the target happens to resolve back inside the root.
pub fn safe_join(root: &Path, rel: &str) -> Option<PathBuf> {
    if rel.is_empty() {
        return None;
    }
    let candidate = Path::new(rel);
    for component in candidate.components() {
        match component {
            Component::Normal(_) => {}
            // Everything else — `..`, a leading `/`, `C:` — is a path trying to
            // leave the project, and there is no legitimate manifest entry that
            // needs one.
            _ => return None,
        }
    }
    Some(root.join(candidate))
}

/// Walk the project, skipping build output and oversized files.
fn walk_project(root: &Path) -> Vec<FileEntry> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if meta.is_dir() {
                if !SKIP_DIRS.contains(&name.as_str()) {
                    stack.push(path);
                }
                continue;
            }
            if !meta.is_file() || meta.len() > MAX_FILE {
                continue;
            }
            let Ok(rel) = path.strip_prefix(root) else {
                continue;
            };
            let Some(rel) = rel.to_str() else {
                continue;
            };
            let Some(hash) = hash_file(&path) else {
                continue;
            };
            out.push(FileEntry {
                // Forward slashes on the wire so a Windows host and a Linux
                // guest describe the same file the same way.
                path: rel.replace('\\', "/"),
                size: meta.len(),
                hash,
            });
        }
    }
    out
}

/// FNV-1a over the file's bytes, streamed so a large asset is never held whole
/// in memory. Not cryptographic — see the module docs.
fn hash_file(path: &Path) -> Option<u64> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = Fnv::default();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buf).ok()?;
        if read == 0 {
            return Some(hasher.finish());
        }
        hasher.write(&buf[..read]);
    }
}

#[derive(Default)]
struct Fnv(u64);

impl Hasher for Fnv {
    fn finish(&self) -> u64 {
        if self.0 == 0 {
            0xcbf2_9ce4_8422_2325
        } else {
            self.0
        }
    }
    fn write(&mut self, bytes: &[u8]) {
        let mut hash = if self.0 == 0 { 0xcbf2_9ce4_8422_2325 } else { self.0 };
        for &byte in bytes {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x1000_0000_01b3);
        }
        self.0 = hash;
    }
}

pub fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[0])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
