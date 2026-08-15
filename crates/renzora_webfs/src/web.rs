//! The wasm half: `showDirectoryPicker()` and what comes back from it.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use bevy::log::{error, info, warn};
use wasm_bindgen::prelude::*;

// The picker is declared as inline JS rather than reached through `web-sys`,
// for two reasons. `web-sys` puts the File System Access API behind
// `--cfg=web_sys_unstable_apis`, which would mean a RUSTFLAGS entry that every
// build of the workspace has to carry; and `handle.entries()` is an async
// iterator, which is considerably less painful to consume in three lines of JS
// than through `js_sys` by hand.
//
// Returning a plain object keeps the Rust side to field reads.
#[wasm_bindgen(inline_js = r#"
export async function __rz_pick_directory() {
  // `readwrite` up front: asking for read now and write at save time would
  // mean a second permission prompt, at the least convenient moment.
  const handle = await window.showDirectoryPicker({ mode: 'readwrite' });
  const files = [];
  const dirs = [];
  for await (const [name, entry] of handle.entries()) {
    (entry.kind === 'directory' ? dirs : files).push(name);
  }
  return { name: handle.name, handle, files, dirs };
}

// Read one file, addressed relative to the picked directory. Walks the path
// segments because the API has no "open by relative path" — a directory handle
// only knows its immediate children.
export async function __rz_read_text(handle, path) {
  const parts = path.split('/').filter(s => s.length && s !== '.');
  const file = parts.pop();
  let dir = handle;
  for (const p of parts) dir = await dir.getDirectoryHandle(p);
  const fh = await dir.getFileHandle(file);
  return await (await fh.getFile()).text();
}

// ── Remembering a project across sessions ───────────────────────────────────
// A directory handle is structured-cloneable, so IndexedDB can store the thing
// itself rather than a path (which the browser never discloses). Permission
// does NOT survive: a restored handle comes back as 'prompt', and re-granting
// it needs a user gesture — which is why reopening happens on a click and not
// at startup.
function __rz_db() {
  return new Promise((res, rej) => {
    const r = indexedDB.open('renzora-webfs', 1);
    r.onupgradeneeded = () => r.result.createObjectStore('handles');
    r.onsuccess = () => res(r.result);
    r.onerror = () => rej(r.error);
  });
}

export async function __rz_remember(name, handle) {
  const db = await __rz_db();
  await new Promise((res, rej) => {
    const tx = db.transaction('handles', 'readwrite');
    tx.objectStore('handles').put(handle, name);
    tx.oncomplete = res;
    tx.onerror = () => rej(tx.error);
  });
}

export async function __rz_reopen(name) {
  const db = await __rz_db();
  const handle = await new Promise((res, rej) => {
    const tx = db.transaction('handles', 'readonly');
    const rq = tx.objectStore('handles').get(name);
    rq.onsuccess = () => res(rq.result);
    rq.onerror = () => rej(rq.error);
  });
  if (!handle) throw new Error('no remembered folder named ' + name);
  // Ask only if we do not already hold it, so a reopen in the same session is
  // silent rather than prompting again.
  let perm = await handle.queryPermission({ mode: 'readwrite' });
  if (perm !== 'granted') perm = await handle.requestPermission({ mode: 'readwrite' });
  if (perm !== 'granted') throw new Error('permission denied for ' + name);
  const files = [], dirs = [];
  for await (const [n, entry] of handle.entries()) {
    (entry.kind === 'directory' ? dirs : files).push(n);
  }
  return { name: handle.name, handle, files, dirs };
}

// Raw bytes, for assets. Separate from the text read because a .glb decoded as
// UTF-8 is nonsense, and because this is the path Bevy's AssetReader takes.
export async function __rz_read_bytes(handle, path) {
  const parts = path.split('/').filter(s => s.length && s !== '.');
  const file = parts.pop();
  let dir = handle;
  for (const p of parts) dir = await dir.getDirectoryHandle(p);
  const fh = await dir.getFileHandle(file);
  return new Uint8Array(await (await fh.getFile()).arrayBuffer());
}

// Write a file, creating any missing directories along the way — the
// equivalent of `create_dir_all` + `fs::write`, which is how callers use it.
//
// `createWritable()` writes to a swap file and only replaces the original on
// close, so a failure part-way leaves the previous contents intact rather than
// a truncated file.
export async function __rz_write_text(handle, path, contents) {
  const parts = path.split('/').filter(s => s.length && s !== '.');
  const file = parts.pop();
  let dir = handle;
  for (const p of parts) dir = await dir.getDirectoryHandle(p, { create: true });
  const fh = await dir.getFileHandle(file, { create: true });
  const w = await fh.createWritable();
  await w.write(contents);
  await w.close();
}

// Create a directory and its parents. Separate from the above because a new
// project needs empty folders that no file is being written into yet.
export async function __rz_create_dir(handle, path) {
  let dir = handle;
  for (const p of path.split('/').filter(s => s.length && s !== '.')) {
    dir = await dir.getDirectoryHandle(p, { create: true });
  }
}

// One directory's immediate children, with the metadata an asset browser wants.
// Size and mtime need `getFile()` per entry, which is why this is worth caching
// rather than calling on a rescan tick.
export async function __rz_list_dir(handle, path) {
  let dir = handle;
  for (const p of path.split('/').filter(s => s.length && s !== '.')) {
    dir = await dir.getDirectoryHandle(p);
  }
  const out = [];
  for await (const [name, entry] of dir.entries()) {
    if (entry.kind === 'directory') {
      out.push({ name, isDir: true, size: 0, modified: 0 });
    } else {
      const f = await entry.getFile();
      out.push({ name, isDir: false, size: f.size, modified: Math.floor(f.lastModified / 1000) });
    }
  }
  return out;
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    async fn __rz_pick_directory() -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch)]
    async fn __rz_read_text(handle: &JsValue, path: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch)]
    async fn __rz_list_dir(handle: &JsValue, path: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch)]
    async fn __rz_write_text(
        handle: &JsValue,
        path: &str,
        contents: &str,
    ) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch)]
    async fn __rz_create_dir(handle: &JsValue, path: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch)]
    async fn __rz_read_bytes(handle: &JsValue, path: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch)]
    async fn __rz_remember(name: &str, handle: &JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch)]
    async fn __rz_reopen(name: &str) -> Result<JsValue, JsValue>;
}

/// Reopen a folder remembered from a previous session, by name.
///
/// The counterpart to a "recent projects" entry. The browser will not reopen a
/// *path*, but it will reopen a stored handle — after the user re-grants
/// permission, which is why this must be called from a click just like
/// [`pick_directory`]. If the permission prompt is declined, or the folder has
/// since moved, this fails and the user can pick it again.
pub fn reopen_project(name: String) {
    wasm_bindgen_futures::spawn_local(async move {
        match __rz_reopen(&name).await {
            Ok(picked) => adopt(picked, false).await,
            Err(e) => error!("[webfs] could not reopen '{name}': {}", describe(&e)),
        }
    });
}

/// Read a file's raw bytes.
///
/// Genuinely async — unlike [`read_text_cached`], which fakes it with a cache
/// because its callers are on the frame loop. Bevy's `AssetReader::read` is
/// already an async trait method, so an asset load can await this directly and
/// needs no shim at all. That is the whole reason assets were never the hard
/// part of putting this editor on the web.
///
/// Not cached: assets are large and Bevy keeps its own. Caching them here would
/// mean holding a second copy of every mesh and texture in wasm's linear
/// memory, on top of the ones the renderer already uploaded.
pub async fn read_bytes(path: &std::path::Path) -> Result<Vec<u8>, String> {
    let rel = to_relative(path);
    let handle = HANDLE
        .with(|h| h.borrow().clone())
        .ok_or_else(|| "no directory picked".to_string())?;
    match __rz_read_bytes(&handle, &rel).await {
        Ok(v) => Ok(js_sys::Uint8Array::new(&v).to_vec()),
        Err(e) => Err(format!("read '{rel}': {}", describe(&e))),
    }
}

/// Whether a project folder has been picked at all.
pub fn has_project() -> bool {
    HANDLE.with(|h| h.borrow().is_some())
}

/// Write a UTF-8 file, creating parent directories as needed.
///
/// Async because the handle is. Callers on the frame loop should treat this as
/// fire-and-forget and watch the console for failures; a caller that needs to
/// know when it landed should await it inside its own task.
pub async fn write_text(path: &std::path::Path, contents: &str) -> Result<(), String> {
    let rel = to_relative(path);
    let handle = HANDLE
        .with(|h| h.borrow().clone())
        .ok_or_else(|| "no directory picked".to_string())?;
    __rz_write_text(&handle, &rel, contents)
        .await
        .map_err(|e| format!("write '{rel}': {}", describe(&e)))?;
    // The cached copies are now stale. Drop them rather than update them: the
    // browser is the source of truth and a re-read is cheap next to being
    // subtly wrong about what is on disk.
    FILES.with(|f| {
        f.borrow_mut().remove(&rel);
    });
    FAILED.with(|f| {
        f.borrow_mut().remove(&rel);
    });
    if let Some(parent) = std::path::Path::new(&rel).parent() {
        let p = parent.to_string_lossy().replace('\\', "/");
        DIRS.with(|d| {
            d.borrow_mut().remove(&p);
        });
    }
    Ok(())
}

/// [`write_text`], fire-and-forget, for callers on the frame loop.
///
/// Failures go to the console — a Bevy system has nothing to await with, and
/// no useful way to report an error that arrives three frames after the call
/// that caused it.
pub fn spawn_write_text(path: std::path::PathBuf, contents: String) {
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(e) = write_text(&path, &contents).await {
            error!("[webfs] {e}");
        }
    });
}

/// [`create_dir_all`], fire-and-forget.
pub fn spawn_create_dir(path: std::path::PathBuf) {
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(e) = create_dir_all(&path).await {
            error!("[webfs] {e}");
        }
    });
}

/// `create_dir_all`, through the handle.
pub async fn create_dir_all(path: &std::path::Path) -> Result<(), String> {
    let rel = to_relative(path);
    let handle = HANDLE
        .with(|h| h.borrow().clone())
        .ok_or_else(|| "no directory picked".to_string())?;
    __rz_create_dir(&handle, &rel)
        .await
        .map_err(|e| format!("mkdir '{rel}': {}", describe(&e)))?;
    DIRS.with(|d| d.borrow_mut().clear());
    Ok(())
}

/// One entry in a directory listing.
#[derive(Clone, Debug)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    /// Bytes; `0` for directories.
    pub size: u64,
    /// Unix seconds; `0` for directories (the API exposes no directory mtime).
    pub modified: u64,
}

/// List a directory, relative to the picked folder (`""` is the root).
///
/// **Returns what is cached, and fetches in the background when it is not.** A
/// caller on the frame loop cannot await, so the first call for a directory
/// returns `None` and starts the read; a later frame gets the contents. The
/// asset browser already rescans on a throttle, so this fits its existing shape
/// — the folder simply populates a beat after you navigate into it.
///
/// `None` means "not known yet", NOT "empty" — the distinction matters, because
/// showing an empty folder that is merely still loading looks like data loss.
pub fn list_dir(path: &std::path::Path) -> Option<Vec<DirEntry>> {
    let rel = to_relative(path);
    if let Some(hit) = DIRS.with(|d| d.borrow().get(&rel).cloned()) {
        return Some(hit);
    }
    fetch_dir(rel);
    None
}

/// Drop a cached listing so the next [`list_dir`] re-reads it. Call after
/// writing into that directory, or on an explicit refresh.
pub fn invalidate_dir(path: &std::path::Path) {
    let rel = to_relative(path);
    DIRS.with(|d| {
        d.borrow_mut().remove(&rel);
    });
}

fn fetch_dir(path: String) {
    // One read in flight per directory: `list_dir` is called from a rescan that
    // runs repeatedly, and without this every one of those ticks would queue
    // another traversal of the same folder.
    if !INFLIGHT.with(|p| p.borrow_mut().insert(path.clone())) {
        return;
    }
    let Some(handle) = HANDLE.with(|h| h.borrow().clone()) else {
        INFLIGHT.with(|p| {
            p.borrow_mut().remove(&path);
        });
        return;
    };
    wasm_bindgen_futures::spawn_local(async move {
        let result = __rz_list_dir(&handle, &path).await;
        INFLIGHT.with(|p| {
            p.borrow_mut().remove(&path);
        });
        match result {
            Ok(list) => {
                let entries = js_sys::Array::from(&list)
                    .iter()
                    .map(|e| DirEntry {
                        name: string_field(&e, "name").unwrap_or_default(),
                        is_dir: field(&e, "isDir").and_then(|v| v.as_bool()).unwrap_or(false),
                        size: num_field(&e, "size"),
                        modified: num_field(&e, "modified"),
                    })
                    .collect::<Vec<_>>();
                DIRS.with(|d| {
                    d.borrow_mut().insert(path, entries);
                });
            }
            // Cache nothing on failure, so a transient error retries rather
            // than pinning an empty folder in the UI forever.
            Err(e) => error!("[webfs] list '{path}': {}", describe(&e)),
        }
    });
}

fn num_field(obj: &JsValue, key: &str) -> u64 {
    field(obj, key).and_then(|v| v.as_f64()).unwrap_or(0.0).max(0.0) as u64
}

/// Read a UTF-8 file from the picked directory, by path relative to it
/// (`"project.toml"`, `"scenes/main.scn"`).
///
/// Async, like everything the handle offers — see the module docs for why that
/// is the constraint the whole VFS is designed around.
pub async fn read_text(path: &str) -> Result<String, String> {
    let handle = HANDLE
        .with(|h| h.borrow().clone())
        .ok_or_else(|| "no directory picked".to_string())?;
    match __rz_read_text(&handle, path).await {
        Ok(v) => v.as_string().ok_or_else(|| format!("{path}: not text")),
        Err(e) => Err(format!("{path}: {}", describe(&e))),
    }
}

thread_local! {
    /// The directory the user last picked.
    ///
    /// A `thread_local` rather than a Bevy resource because a
    /// `FileSystemDirectoryHandle` is a `JsValue`, which is neither `Send` nor
    /// `Sync` and so cannot live in the `World`. wasm is single-threaded, so
    /// this is reachable from anywhere the engine actually runs.
    static HANDLE: RefCell<Option<JsValue>> = const { RefCell::new(None) };

    /// A finished pick, waiting for a system to collect it.
    ///
    /// The handoff from the async task back into the frame loop. The picker
    /// resolves whenever the user gets round to it, which is no particular
    /// frame, so the task leaves its result here and [`take_picked_project`]
    /// collects it from an ordinary Bevy system.
    static PICKED: RefCell<Option<PickedProject>> = const { RefCell::new(None) };

    /// Directory listings already read, keyed by path relative to the picked
    /// folder. This is what makes an async API answerable from a sync caller.
    static DIRS: RefCell<HashMap<String, Vec<DirEntry>>> =
        RefCell::new(HashMap::new());

    /// Directories with a read in flight, so a repeating rescan does not queue
    /// a fresh traversal of the same folder on every tick.
    static INFLIGHT: RefCell<HashSet<String>> = RefCell::new(HashSet::new());

    /// File contents already read, keyed like [`DIRS`].
    static FILES: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());

    /// Paths whose read failed. Kept so a missing file is asked for ONCE
    /// instead of re-requested on every frame that redraws it — the difference
    /// between one warning and a console full of them.
    static FAILED: RefCell<HashSet<String>> = RefCell::new(HashSet::new());

    /// The picked folder's own name, e.g. `demo22`.
    static ROOT: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Convert an editor path into one relative to the picked directory.
///
/// The editor addresses files as `demo22/models/thing.glb`, because
/// `CurrentProject::path` is the folder's name — the browser discloses no real
/// path. The handle, meanwhile, IS `demo22`, so it wants `models/thing.glb`.
/// Every call site would otherwise re-derive that, and drift.
///
/// Backslashes are normalised because paths reach here having been through
/// `PathBuf`, which on a Windows-hosted build cheerfully produces `\`.
pub fn to_relative(path: &std::path::Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    let s = s.trim_start_matches("./").trim_start_matches('/');
    match ROOT.with(|r| r.borrow().clone()) {
        Some(root) => s
            .strip_prefix(&root)
            .map(|rest| rest.trim_start_matches('/'))
            .unwrap_or(s)
            .to_string(),
        None => s.to_string(),
    }
}

/// Read a UTF-8 file, cached, without awaiting.
///
/// The [`list_dir`] bargain applies: `None` means "not read yet" and starts the
/// read, so a caller on the frame loop gets the contents a frame or two later.
/// A path that failed is not retried — see [`FAILED`].
pub fn read_text_cached(path: &std::path::Path) -> Option<String> {
    let rel = to_relative(path);
    if let Some(hit) = FILES.with(|f| f.borrow().get(&rel).cloned()) {
        return Some(hit);
    }
    if FAILED.with(|f| f.borrow().contains(&rel)) {
        return None;
    }
    fetch_text(rel);
    None
}

fn fetch_text(rel: String) {
    if !INFLIGHT.with(|p| p.borrow_mut().insert(rel.clone())) {
        return;
    }
    let Some(handle) = HANDLE.with(|h| h.borrow().clone()) else {
        INFLIGHT.with(|p| {
            p.borrow_mut().remove(&rel);
        });
        return;
    };
    wasm_bindgen_futures::spawn_local(async move {
        let result = __rz_read_text(&handle, &rel).await;
        INFLIGHT.with(|p| {
            p.borrow_mut().remove(&rel);
        });
        match result.ok().and_then(|v| v.as_string()) {
            Some(text) => FILES.with(|f| {
                f.borrow_mut().insert(rel, text);
            }),
            None => FAILED.with(|f| {
                f.borrow_mut().insert(rel);
            }),
        };
    });
}

/// A project folder the user picked.
pub struct PickedProject {
    /// The folder's name — `demo22`. **Not a path**: the browser does not
    /// disclose one, so this is all there is to identify the project by.
    pub name: String,
    /// Verbatim `project.toml`, or `None` when the folder does not have one and
    /// the caller asked to create rather than open. Deciding what a new
    /// project's files contain stays with the caller: this crate knows how to
    /// write bytes through a handle and deliberately nothing about project
    /// layout.
    pub project_toml: Option<String>,
}

/// Collect a finished pick, if one is waiting. Returns `None` otherwise, which
/// is the normal case on almost every frame.
pub fn take_picked_project() -> Option<PickedProject> {
    PICKED.with(|p| p.borrow_mut().take())
}

/// Run `f` against the picked directory handle, if there is one.
pub fn with_handle<R>(f: impl FnOnce(&JsValue) -> R) -> Option<R> {
    HANDLE.with(|h| h.borrow().as_ref().map(f))
}

/// Open the browser's directory picker.
///
/// Returns immediately — the picker is asynchronous and the user may take as
/// long as they like over it, so this spawns and lets the result arrive later.
/// **Must be called from a click**; browsers refuse a picker that no gesture
/// asked for, and the refusal looks exactly like the user pressing cancel.
///
/// `allow_new` distinguishes New Project from Open Project: with it, a folder
/// without a `project.toml` is reported as a new project rather than rejected.
/// A folder that already HAS one is opened either way — New Project on an
/// existing project should adopt it, never overwrite it.
pub fn pick_directory(allow_new: bool) {
    wasm_bindgen_futures::spawn_local(async move {
        let picked = match __rz_pick_directory().await {
            Ok(v) => v,
            Err(e) => {
                // Cancelling the dialog rejects with AbortError, which is a
                // perfectly normal thing for a user to do — say so quietly
                // rather than reporting it as a failure.
                if is_abort(&e) {
                    info!("[webfs] directory pick cancelled");
                } else {
                    error!("[webfs] directory pick failed: {}", describe(&e));
                }
                return;
            }
        };

        adopt(picked, allow_new).await;
    });
}

/// Take a freshly picked (or reopened) directory as the current project.
///
/// Shared by [`pick_directory`] and [`reopen_project`] so the two cannot drift
/// on what "adopting a folder" means — installing the handle, remembering it,
/// and publishing a [`PickedProject`] for the frame loop to collect.
async fn adopt(picked: JsValue, allow_new: bool) {
    let name = string_field(&picked, "name").unwrap_or_else(|| "<unnamed>".into());
    let files = string_array(&picked, "files");
    let dirs = string_array(&picked, "dirs");

    let Some(handle) = field(&picked, "handle") else {
        error!("[webfs] no handle came back — cannot read '{name}'");
        return;
    };
    HANDLE.with(|h| *h.borrow_mut() = Some(handle.clone()));
    // Recorded so `to_relative` can strip it back off the editor's paths,
    // which are all prefixed with the project folder's name.
    ROOT.with(|r| *r.borrow_mut() = Some(name.clone()));
    // Anything cached belongs to the previous project.
    DIRS.with(|d| d.borrow_mut().clear());
    FILES.with(|f| f.borrow_mut().clear());
    FAILED.with(|f| f.borrow_mut().clear());

    // Remember it so a later session can reopen this exact folder. Best-effort:
    // failing to record a recent entry is no reason not to open the project.
    if let Err(e) = __rz_remember(&name, &handle).await {
        warn!("[webfs] could not remember '{name}': {}", describe(&e));
    }

    info!(
        "[webfs] opened '{name}': {} file(s), {} director(y/ies)",
        files.len(),
        dirs.len()
    );

    if !files.iter().any(|f| f == "project.toml") {
        if !allow_new {
            warn!("[webfs] '{name}' has no project.toml — not a Renzora project folder");
            return;
        }
        info!("[webfs] '{name}' has no project yet — creating one here");
        PICKED.with(|p| {
            *p.borrow_mut() = Some(PickedProject { name, project_toml: None });
        });
        return;
    }
    match read_text("project.toml").await {
        Ok(project_toml) => {
            info!("[webfs] read project.toml ({} bytes)", project_toml.len());
            prewarm(&handle).await;
            PICKED.with(|p| {
                *p.borrow_mut() = Some(PickedProject {
                    name,
                    project_toml: Some(project_toml),
                });
            });
        }
        Err(e) => error!("[webfs] {e}"),
    }
}

/// Directories never worth indexing — build output and caches, which are large,
/// uninteresting, and in `.cache`'s case owned by the desktop editor.
const SKIP_DIRS: &[&str] = &[".cache", ".git", "node_modules", "target", "dist"];

/// Small text files the editor reads *synchronously* and expects to be there.
/// Assets are excluded on purpose: they are large, and Bevy's async
/// `AssetReader` fetches them on demand anyway.
const PREREAD_EXTS: &[&str] = &["bsn", "toml", "json", "lua", "scn", "ron", "material"];

/// Cap on how much is pulled in at open, so a pathological tree cannot hang the
/// splash screen. Exceeding it is reported rather than silently truncating —
/// a half-indexed project that *looks* complete is worse than a warning.
const MAX_PREWALK_DIRS: usize = 4000;
const MAX_PREREAD_BYTES: u64 = 4 * 1024 * 1024;

/// Read the project's shape into cache before anyone asks for it.
///
/// This is what lets a browser project work at all. Almost every filesystem
/// caller in the editor is synchronous and one-shot — the scene load is an
/// `OnEnter` system, so a cache miss there means the scene simply never loads,
/// with no second attempt. Since `adopt` finishes before the project is
/// published to the frame loop, anything read here is guaranteed to be warm by
/// the time those callers run.
///
/// Directories are walked in full (they are what the asset browser, the tree
/// and the asset registry all enumerate); file *contents* are limited to small
/// text formats, since those are the ones read synchronously.
async fn prewarm(handle: &JsValue) {
    let mut queue = vec![String::new()];
    let mut dirs_seen = 0usize;
    let mut files_read = 0usize;
    let mut truncated = false;

    while let Some(dir) = queue.pop() {
        if dirs_seen >= MAX_PREWALK_DIRS {
            truncated = true;
            break;
        }
        dirs_seen += 1;

        let Ok(list) = __rz_list_dir(handle, &dir).await else {
            continue;
        };
        let entries: Vec<DirEntry> = js_sys::Array::from(&list)
            .iter()
            .map(|e| DirEntry {
                name: string_field(&e, "name").unwrap_or_default(),
                is_dir: field(&e, "isDir").and_then(|v| v.as_bool()).unwrap_or(false),
                size: num_field(&e, "size"),
                modified: num_field(&e, "modified"),
            })
            .collect();

        for e in &entries {
            let child = if dir.is_empty() {
                e.name.clone()
            } else {
                format!("{dir}/{}", e.name)
            };
            if e.is_dir {
                if !SKIP_DIRS.contains(&e.name.as_str()) {
                    queue.push(child);
                }
            } else if e.size <= MAX_PREREAD_BYTES
                && std::path::Path::new(&e.name)
                    .extension()
                    .and_then(|x| x.to_str())
                    .is_some_and(|x| PREREAD_EXTS.contains(&x.to_ascii_lowercase().as_str()))
            {
                if let Ok(v) = __rz_read_text(handle, &child).await {
                    if let Some(text) = v.as_string() {
                        files_read += 1;
                        FILES.with(|f| {
                            f.borrow_mut().insert(child, text);
                        });
                    }
                }
            }
        }

        DIRS.with(|d| {
            d.borrow_mut().insert(dir, entries);
        });
    }

    info!("[webfs] indexed {dirs_seen} folder(s), pre-read {files_read} file(s)");
    if truncated {
        warn!(
            "[webfs] stopped indexing at {MAX_PREWALK_DIRS} folders — deeper ones \
             load on demand and may appear empty for a frame"
        );
    }
}

fn field(obj: &JsValue, key: &str) -> Option<JsValue> {
    js_sys::Reflect::get(obj, &JsValue::from_str(key))
        .ok()
        .filter(|v| !v.is_undefined() && !v.is_null())
}

fn string_field(obj: &JsValue, key: &str) -> Option<String> {
    field(obj, key).and_then(|v| v.as_string())
}

fn string_array(obj: &JsValue, key: &str) -> Vec<String> {
    let Some(v) = field(obj, key) else {
        return Vec::new();
    };
    js_sys::Array::from(&v).iter().filter_map(|e| e.as_string()).collect()
}

/// Did the user simply cancel the dialog?
fn is_abort(err: &JsValue) -> bool {
    string_field(err, "name").as_deref() == Some("AbortError")
}

/// A readable message out of a `JsValue` error, which may be an `Error`, a
/// string, or something with no useful representation at all.
fn describe(err: &JsValue) -> String {
    string_field(err, "message")
        .or_else(|| err.as_string())
        .unwrap_or_else(|| format!("{err:?}"))
}
