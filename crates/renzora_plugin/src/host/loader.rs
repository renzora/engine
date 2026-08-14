//! Finds `renzora_plugin` cdylibs on disk and initialises them.
//!
//! Deliberately symbol-dispatched: a library is treated as a C-ABI plugin only
//! if it exports [`sys::INIT_SYMBOL`]. Anything else is skipped silently, which
//! is what lets these live in the same `plugins/` directory as the older
//! `dynamic_plugin_loader` dylibs during the migration — each loader recognises
//! its own and ignores the rest.

use bevy::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use libloading::{Library, Symbol};
#[cfg(target_arch = "wasm32")]
use wasm_dl::{Library, Symbol};
use crate::static_link::StaticPlugin;
use crate::sys;
use std::path::{Path, PathBuf};

/// `libloading`'s shape, for a platform that has no dynamic loading at all.
///
/// The web has no `dlopen`, no `LoadLibrary`, and no `plugins/` folder to scan —
/// a wasm build gets its plugins linked in (`static_link`) or not at all. The
/// alternative to this shim was `#[cfg]`-ing the whole module out, which would
/// have meant cfg-ing every caller of [`LoadedPlugins`] across the runtime for a
/// platform where they all correctly find nothing anyway.
///
/// So: opening always fails, `scan_plugins` finds no plugins, and the statically
/// linked ones are unaffected. `Symbol` can never be constructed (`get` only ever
/// returns `Err`), which is what makes its `Deref` unreachable rather than wrong.
#[cfg(target_arch = "wasm32")]
mod wasm_dl {
    use std::ffi::OsStr;
    use std::marker::PhantomData;
    use std::ops::Deref;

    #[derive(Debug)]
    pub struct Error;

    impl std::fmt::Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("dynamic library loading is not available on wasm")
        }
    }

    pub struct Library;

    impl Library {
        /// # Safety
        /// Never loads anything, so trivially sound; `unsafe` only to match
        /// `libloading::Library::new`'s signature.
        pub unsafe fn new<P: AsRef<OsStr>>(_path: P) -> Result<Self, Error> {
            Err(Error)
        }

        /// # Safety
        /// Unreachable — a `Library` cannot be constructed on this target.
        pub unsafe fn get<T>(&self, _symbol: &[u8]) -> Result<Symbol<T>, Error> {
            Err(Error)
        }
    }

    pub struct Symbol<T>(PhantomData<T>);

    impl<T> Deref for Symbol<T> {
        type Target = T;
        fn deref(&self) -> &T {
            unreachable!("Library::get never returns Ok on wasm")
        }
    }
}

/// One plugin path, across every load of it.
///
/// A slot's index in [`LoadedPlugins`] is its permanent identity: entries are
/// never removed, so an index stamped on a registration stays valid for the life
/// of the process. That is what the ownership tags on panels, render passes and
/// component schemas refer to.
pub struct PluginSlot {
    pub path: PathBuf,
    /// The scope this path reported, once anything has read it. `None` until a
    /// load got far enough to ask.
    ///
    /// Recorded so [`scan_plugins`] can answer "what scope is this plugin?"
    /// without mapping the image again — which in the editor would map a *second*
    /// instance of it, since the running one is a shadow copy under a different
    /// filename. See [`scan_plugins`] for why a second instance is not merely
    /// wasteful.
    pub scope: Option<sys::PluginScope>,
    /// Shared with every system this slot's plugins registered. Bumping it
    /// retires the previous load's systems — see `host::GenGate`.
    pub generation: super::PluginGeneration,
    /// The generation of the newest load that succeeded.
    pub loaded_at: u32,
    /// How many images have been loaded for this path. Zero means the next load is
    /// the first, which is what distinguishes "generation 0" from "reload to
    /// generation 1" — `loaded_at` alone cannot, since it starts at 0 too.
    pub images: usize,
    /// **Every** library ever loaded for this path, and none of them is ever
    /// dropped.
    ///
    /// Deliberate. Every function pointer a plugin registered — system entries,
    /// panel action thunks, render callbacks — points into its library, and a
    /// retired system is still *in* the schedule, merely returning early. Freeing
    /// the library would turn those into dangling pointers. Dropping a
    /// `libloading::Library` has also deadlocked in `FreeLibrary` here before.
    ///
    /// So a reload leaks one library image. A few MB per reload across a dev
    /// session is a fair price for never unmapping code that something might
    /// still call, and a restart reclaims all of it.
    _libraries: Vec<Library>,
}

/// Every plugin path the loader has seen, indexed by slot.
#[derive(Resource, Default)]
pub struct LoadedPlugins(pub Vec<PluginSlot>);

impl LoadedPlugins {
    /// The slot for `path`, creating one if this is the first sighting.
    fn slot_for(&mut self, path: &Path) -> usize {
        if let Some(i) = self.0.iter().position(|s| s.path.as_path() == path) {
            return i;
        }
        self.0.push(PluginSlot {
            path: path.to_path_buf(),
            scope: None,
            generation: super::PluginGeneration::default(),
            loaded_at: 0,
            images: 0,
            _libraries: Vec::new(),
        });
        self.0.len() - 1
    }

    /// The scope already read for `path`, if any load has read one.
    ///
    /// What lets [`scan_plugins`] skip mapping an image it has already seen.
    fn scope_of(&self, path: &Path) -> Option<sys::PluginScope> {
        self.0
            .iter()
            .find(|s| s.path.as_path() == path)
            .and_then(|s| s.scope)
    }
}

/// Outcome for one candidate file, for logging and the editor's plugin panel.
#[derive(Debug)]
pub enum LoadOutcome {
    Loaded,
    /// Not a C-ABI plugin — no init symbol. Expected for older dylib plugins.
    NotAPlugin,
    /// The plugin needs a newer host than this one.
    VersionTooOld,
    /// The plugin's own init returned a failure, or the library would not open.
    Failed(String),
    /// The plugin belongs in the other binary. Not an error — an editor plugin
    /// sitting in a game's `plugins/` directory is expected when both were
    /// staged from one build.
    WrongScope(sys::PluginScope),
}

/// Load every `renzora_plugin` cdylib in `dir`, except any already linked into
/// this binary.
///
/// Missing or unreadable directories are not an error — a build with no plugins
/// is normal.
///
/// `linked` holds the crate names of plugins compiled in (see [`load_static`]).
/// Loading a second copy of one is not a duplicate that resolves itself: the two
/// get separate slots, so BOTH sets of systems end up in the schedules and every
/// one of the plugin's systems runs twice a frame — and the second copy's
/// first-claim registrations (a script backend's extensions, a panel id) fail
/// with an error that reads like a conflict between two different plugins. An
/// export never produces this, because it skips copying what it linked; a user
/// pointing a game at the editor's `plugins/` folder produces it immediately.
pub fn load_dir(
    world: &mut World,
    dir: &Path,
    is_editor: bool,
    linked: &[&str],
) -> Vec<(PathBuf, LoadOutcome)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let ext = std::env::consts::DLL_EXTENSION;
    let mut results = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some(ext) {
            continue;
        }
        // `lib` is stripped because a cdylib is `lib<crate>.so` on Unix and
        // `<crate>.dll` on Windows, while a linked plugin is only ever known by
        // its crate name.
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let name = stem.strip_prefix("lib").unwrap_or(&stem);
        if linked.contains(&name) {
            info!("[plugin] ignoring {stem} in plugins/ — this build links {name} in");
            continue;
        }
        let outcome = load_one(world, &path, is_editor);
        results.push((path, outcome));
    }
    results
}

/// True if the file is a Rust **proc-macro** dylib.
///
/// These compile to a dylib for *rustc* to load, not for us, and `dlopen`ing one
/// into a process that is not the compiler crashes hard — before the splash,
/// with no panic and no crash report, because it is the OS loader failing rather
/// than Rust. We cannot detect that after `Library::new`, so detect it before:
/// every proc-macro dylib exports a `__rustc_proc_macro_decls_*` symbol, and the
/// name appears verbatim in the export table, so a plain byte search finds it
/// with no PE/ELF parsing.
///
/// Staging already filters these out (see xtask's `is_not_a_plugin`), but a dll
/// dropped into `plugins/` by hand must not be able to take the editor down.
fn is_proc_macro_dylib(path: &Path) -> bool {
    contains_symbol(path, b"__rustc_proc_macro_decls")
}

/// Whether this file is a C-ABI plugin, decided WITHOUT loading it.
///
/// Load-bearing, not an optimisation. `plugins/` also holds the older
/// Bevy-linking cdylibs, and those are already mapped by `dynamic_plugin_loader`.
/// Asking the OS to load one *by its original path* was harmless — same path, same
/// module, refcount++. But a plugin is now loaded from a **copy** under a fresh
/// filename (see [`shadow_copy`]), and the OS treats that as a different library:
/// it maps a whole second instance and re-runs its initialisers, including the
/// `inventory::submit!` ctors that register plugins. Doing that to seventy
/// Bevy-linking dylibs at boot is not a slow path, it is a broken one.
///
/// So the question "is this mine?" has to be answered from the bytes, before any
/// copy or load happens. Every C-ABI plugin exports [`sys::INIT_SYMBOL`], and an
/// exported name appears verbatim in the export table, so a byte search settles it
/// with no PE/ELF parsing — the same trick [`is_proc_macro_dylib`] uses.
fn exports_plugin_init(path: &Path) -> bool {
    contains_symbol(path, sys::INIT_SYMBOL.as_bytes())
}

fn contains_symbol(path: &Path, needle: &[u8]) -> bool {
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    bytes.windows(needle.len()).any(|w| w == needle)
}

/// Copy a plugin image somewhere private before loading it, and return where.
///
/// **This is what makes reload possible on Windows at all.** A mapped DLL is
/// locked, and the loader never unmaps one (retired systems still point into it),
/// so loading `plugins/drift.dll` directly would leave that file permanently
/// unwritable — `cargo build` could not overwrite it and the staging copy would
/// fail with "file in use". Loading a copy leaves the original free.
///
/// The generation is in the filename because the previous copy is *also* still
/// mapped and locked. Copies accumulate, one per reload, alongside the leaked
/// library images they belong to; the directory is cleared at startup.
///
/// `.reload` has no file extension, so [`load_dir`]'s extension filter skips it
/// and the copies are never mistaken for plugins to load.
///
/// **Editor-only.** A shipped game opens `plugins/<name>.dll` itself — see
/// [`load_one`] for why copying is actively harmful there.
fn shadow_copy(path: &Path, generation: u32) -> std::io::Result<PathBuf> {
    let dir = path.parent().unwrap_or_else(|| Path::new(".")).join(".reload");
    std::fs::create_dir_all(&dir)?;
    let stem = path.file_stem().unwrap_or_default().to_string_lossy().into_owned();
    let ext = std::env::consts::DLL_EXTENSION;
    let dst = dir.join(format!("{stem}-{generation}.{ext}"));
    std::fs::copy(path, &dst)?;
    Ok(dst)
}

/// Remove shadow copies left by earlier sessions.
///
/// Editor-only, like the copies themselves. Safe here and nowhere else: at
/// `build` time nothing is mapped yet, so no copy is locked. Skipping a file
/// that refuses to delete is deliberate — a stale image is harmless (nothing
/// scans this directory), and failing the whole boot over it would not be.
fn clear_shadow_dir(dir: &Path) {
    let shadow = dir.join(".reload");
    if let Ok(entries) = std::fs::read_dir(&shadow) {
        for entry in entries.flatten() {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

fn load_one(world: &mut World, path: &Path, is_editor: bool) -> LoadOutcome {
    if is_proc_macro_dylib(path) {
        return LoadOutcome::NotAPlugin;
    }
    // BEFORE any copy or load: `plugins/` is shared with the older Bevy-linking
    // dylibs, and copying one to a new filename would make the OS map a second
    // instance of it. See `exports_plugin_init`.
    if !exports_plugin_init(path) {
        return LoadOutcome::NotAPlugin;
    }

    // Resolved before the image is opened, because the generation is part of the
    // shadow copy's filename.
    let (slot, counter, generation) = {
        let mut loaded = world.get_resource_or_insert_with(LoadedPlugins::default);
        let slot = loaded.slot_for(path);
        let s = &loaded.0[slot];
        let first = s.images == 0;
        (
            slot,
            s.generation.clone(),
            if first { 0 } else { s.loaded_at + 1 },
        )
    };

    // Only the editor loads a copy. The copy exists so a rebuild can overwrite
    // the original while it is mapped (see [`shadow_copy`]) — a shipped game
    // never reloads a plugin, so it has nothing to buy there and one real cost:
    // `plugins/` is shared with whoever launched it. The editor spawns the game
    // as a child pointed at the same directory, and it already has every shadow
    // copy mapped and locked, so the child's `fs::copy` failed with "used by
    // another process" for EVERY plugin. The runtime window came up with no
    // audio backend, no scripting and no plugins at all — which reads as "audio
    // is broken outside the editor" rather than "the runtime loaded nothing".
    let image = if is_editor {
        match shadow_copy(path, generation) {
            Ok(p) => p,
            Err(e) => return LoadOutcome::Failed(format!("could not stage a copy to load: {e}")),
        }
    } else {
        path.to_path_buf()
    };

    // SAFETY: loading arbitrary native code is inherently unsafe — a plugin can
    // do anything the process can. That is the same trust model as the existing
    // dylib loader; the C ABI buys build-environment independence, not sandboxing.
    let library = match unsafe { Library::new(&image) } {
        Ok(l) => l,
        Err(e) => return LoadOutcome::Failed(format!("could not open: {e}")),
    };

    // Never unmapped, on ANY path out of here — including the ones that decide
    // this image is not wanted. `_libraries` already says a loaded plugin stays
    // mapped for the life of the process; this extends that to a rejected one,
    // because unloading is not merely wasteful, it **hangs**. `FreeLibrary` runs
    // the image's static destructors while holding the Windows loader lock, and
    // an image whose initialisers started a thread waits on that thread — which
    // cannot finish, because finishing needs the lock.
    //
    // Only a build that *rejects* a plugin can hit it, which is why the editor
    // never did and the game runtime always would: the runtime skips every
    // Editor-scope plugin, so it deadlocked partway through the plugins folder,
    // with no window, no message and no crash — a boot that simply stopped.
    let library = std::mem::ManuallyDrop::new(library);

    let init: Symbol<sys::ExtensionInit> =
        match unsafe { library.get(sys::INIT_SYMBOL.as_bytes()) } {
            Ok(s) => s,
            Err(_) => return LoadOutcome::NotAPlugin,
        };
    let init = *init;

    // Read the scope BEFORE calling init, so a plugin for the other binary never
    // gets the chance to register a system, a component or a panel. Checking
    // afterwards would mean unwinding registrations that already happened.
    let scope = match unsafe { library.get::<sys::ScopeEntry>(sys::SCOPE_SYMBOL.as_bytes()) } {
        Ok(f) => unsafe { f() },
        // No declaration means Runtime, matching `renzora::add!`'s default.
        Err(_) => sys::PluginScope::Runtime,
    };
    // Recorded here rather than on the success path, so the rejections below —
    // and a plugin whose own init fails — still leave the answer behind.
    // `scan_plugins` reads it instead of mapping the image a second time.
    world.resource_mut::<LoadedPlugins>().0[slot].scope = Some(scope);
    if !scope.is_known() {
        return LoadOutcome::Failed(format!(
            "declares scope {} which this build does not have",
            scope.0
        ));
    }
    if scope == sys::PluginScope::Editor && !is_editor {
        return LoadOutcome::WrongScope(scope);
    }

    // Take the slot's previous registrations back before the new build adds its
    // own, so a panel or a render pass is replaced rather than duplicated. Systems
    // are NOT in here — they retire themselves via the generation counter, because
    // Bevy cannot remove one from a schedule.
    if generation > 0 {
        super::retire_slot(world, slot);
    }

    match super::init_plugin_gen(world, init, counter.clone(), generation, slot) {
        sys::InitResult::Ok => {
            counter.store(generation, std::sync::atomic::Ordering::Relaxed);
            let mut loaded = world.resource_mut::<LoadedPlugins>();
            let s = &mut loaded.0[slot];
            s.loaded_at = generation;
            s.images += 1;
            // The one path that takes ownership rather than leaking in place —
            // and it only moves the handle somewhere that also never drops it.
            s._libraries.push(std::mem::ManuallyDrop::into_inner(library));
            LoadOutcome::Loaded
        }
        sys::InitResult::VersionTooOld => LoadOutcome::VersionTooOld,
        sys::InitResult::Failed => {
            LoadOutcome::Failed("plugin init returned Failed".to_string())
        }
        // The version matched and the shape did not, so the two were built from
        // headers that disagree about field order. Say that, rather than leaving
        // an author to wonder why a plugin with the right version number is
        // refused — the fix is a rebuild, not an engine update.
        sys::InitResult::AbiMismatch => LoadOutcome::Failed(
            "plugin was built against a differently-shaped interface table — its version \
             matches but a field was inserted, reordered or retyped. Rebuild the plugin \
             against this engine's `renzora_plugin`"
                .to_string(),
        ),
        // A value from a newer ABI. Reaching this arm at all is what the newtype
        // bought: as a real enum the match above would have been exhaustive, and
        // an out-of-range discriminant would have been undefined behaviour here
        // rather than a case to handle.
        //
        // Refused rather than assumed successful — a plugin that reports a result
        // this build has no name for has not told us it loaded.
        other => LoadOutcome::Failed(format!(
            "plugin init returned status {} which this engine does not know — it was built \
             against a newer ABI. Rebuild it against this engine's `renzora_plugin`",
            other.0
        )),
    }
}

/// Initialise a plugin that is compiled into this binary.
///
/// The short version of everything [`load_one`] does that this does not: there
/// is no file, so there is nothing to sniff for an init symbol, nothing to copy
/// aside before mapping, and no library to keep alive — the code is already in
/// the binary's `.text` and outlives the process's interest in it. What remains
/// is the part that actually matters: read the scope before init so a plugin for
/// the other binary never registers anything, then run init against a slot.
///
/// The slot is keyed by a synthetic path (`<linked>/<id>`) rather than a real
/// one. It exists because panels, render passes and materials are all tagged with
/// their owning slot; a linked plugin needs an owner tag exactly as much as a
/// loaded one does. `<` and `>` cannot appear in a Windows filename, so the key
/// can never collide with a plugin on disk — and the extension filter in
/// [`load_dir`] and [`PluginWatcher`] means nothing ever tries to stat it.
///
/// Generation stays 0 forever: linked code cannot be swapped, so nothing retires
/// and no system ever goes stale.
pub fn load_static(world: &mut World, plugin: &StaticPlugin, is_editor: bool) -> LoadOutcome {
    if !plugin.scope.is_known() {
        return LoadOutcome::Failed(format!(
            "declares scope {} which this build does not have",
            plugin.scope.0
        ));
    }
    if plugin.scope == sys::PluginScope::Editor && !is_editor {
        return LoadOutcome::WrongScope(plugin.scope);
    }

    let path = PathBuf::from(format!("<linked>/{}", plugin.id));
    let (slot, counter) = {
        let mut loaded = world.get_resource_or_insert_with(LoadedPlugins::default);
        let slot = loaded.slot_for(&path);
        loaded.0[slot].scope = Some(plugin.scope);
        let counter = loaded.0[slot].generation.clone();
        (slot, counter)
    };

    match super::init_plugin_gen(world, plugin.init, counter, 0, slot) {
        sys::InitResult::Ok => {
            let mut loaded = world.resource_mut::<LoadedPlugins>();
            loaded.0[slot].images += 1;
            LoadOutcome::Loaded
        }
        sys::InitResult::VersionTooOld => LoadOutcome::VersionTooOld,
        sys::InitResult::Failed => {
            LoadOutcome::Failed("plugin init returned Failed".to_string())
        }
        // Unreachable in practice, and deliberately still handled: a linked
        // plugin was compiled against the very `renzora_plugin` in this build, so
        // its idea of the table's shape cannot differ. If it somehow does, saying
        // so beats reporting success.
        sys::InitResult::AbiMismatch => LoadOutcome::Failed(
            "plugin was built against a differently-shaped interface table, which should be \
             impossible for a linked-in plugin — the export workspace is out of sync"
                .to_string(),
        ),
        other => LoadOutcome::Failed(format!(
            "plugin init returned status {} which this engine does not know",
            other.0
        )),
    }
}

/// Runs immediately before [`First`], and exists purely so a reload never mutates
/// a schedule that is mid-run.
///
/// A plugin's systems go into the five main-loop schedules, and `Schedules` hands
/// a schedule *out* while it runs — so a reload triggered from inside `Update`
/// would add the new build's systems to a fresh, empty `Update` that is discarded
/// the moment the real one is put back. The systems would vanish with no error.
/// Registration at `build` time avoided this by happening before any schedule ran;
/// this schedule is the same trick for a running app.
#[derive(bevy::ecs::schedule::ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PluginReload;

/// Plugin paths to reload at the next frame boundary.
///
/// A queue rather than an immediate call because the caller is usually a file
/// watcher or a UI button, neither of which holds `&mut World` at a safe moment.
#[derive(Resource, Default)]
pub struct PluginReloadQueue(pub Vec<PathBuf>);

/// Ask for `path` to be reloaded before the next frame.
///
/// Duplicates collapse: an editor save often produces several filesystem events
/// for one write, and rebuilding the same plugin three times in a frame would be
/// three sets of dead systems for no reason.
pub fn request_reload(world: &mut World, path: impl Into<PathBuf>) {
    let path = path.into();
    let mut queue = world.get_resource_or_insert_with(PluginReloadQueue::default);
    if !queue.0.contains(&path) {
        queue.0.push(path);
    }
}

/// Whether this binary is the editor, kept so a reload can apply the same scope
/// filter the initial load did.
#[derive(Resource)]
pub struct PluginHostConfig {
    pub is_editor: bool,
}

fn apply_reload_requests(world: &mut World) {
    let pending = match world.get_resource_mut::<PluginReloadQueue>() {
        Some(mut q) if !q.0.is_empty() => std::mem::take(&mut q.0),
        _ => return,
    };
    let is_editor = world
        .get_resource::<PluginHostConfig>()
        .is_some_and(|c| c.is_editor);

    for path in pending {
        let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        match load_one(world, &path, is_editor) {
            LoadOutcome::Loaded => {
                let generation = world
                    .get_resource::<LoadedPlugins>()
                    .and_then(|l| l.0.iter().find(|s| s.path == path))
                    .map(|s| s.loaded_at)
                    .unwrap_or(0);
                if generation == 0 {
                    info!("[plugin] loaded {name} (added while running)");
                } else {
                    info!("[plugin] reloaded {name} (generation {generation})");
                }
            }
            // Every failure leaves the previous build running — the generation
            // counter only moves on success — so these are warnings, not errors
            // that need the app to do anything about them.
            LoadOutcome::Failed(why) => {
                warn!("[plugin] reload of {name} failed, keeping the running build: {why}")
            }
            LoadOutcome::VersionTooOld => {
                warn!("[plugin] reload of {name} needs a newer ABI; keeping the running build")
            }
            LoadOutcome::WrongScope(scope) => {
                warn!("[plugin] reload of {name} declares {scope:?} scope, which this binary is not")
            }
            LoadOutcome::NotAPlugin => {
                warn!("[plugin] reload of {name}: no `{}` export", sys::INIT_SYMBOL)
            }
        }
    }
}

/// Notices a rebuilt plugin and queues it for reload.
///
/// Polls `mtime` + `size` rather than subscribing to filesystem events. Not for
/// lack of a watcher crate — `notify` is already in the tree behind `bevy_asset` —
/// but because polling answers the question that actually matters more directly.
/// A build writes a DLL in pieces, and loading a half-written one is the failure
/// mode to avoid; "the stamp has not changed since the last poll" is a settle test,
/// whereas an event stream needs debouncing to become one. It also keeps a crate
/// that publishes to crates.io from gaining a dependency to stat eight files.
#[derive(Resource)]
pub struct PluginWatcher {
    dir: PathBuf,
    /// Last-seen `(mtime, size)` per file.
    seen: std::collections::HashMap<PathBuf, (std::time::SystemTime, u64)>,
    /// Files whose stamp moved on the previous poll, waiting to stop moving.
    settling: std::collections::HashSet<PathBuf>,
    /// Seconds until the next poll.
    countdown: f32,
}

/// How often to stat the plugin directory. Two polls are needed to settle a file,
/// so this is half the reload latency.
const POLL_INTERVAL: f32 = 0.25;

/// `(mtime, size)` for every plugin-shaped file in `dir`.
fn stamp_dir(dir: &Path) -> std::collections::HashMap<PathBuf, (std::time::SystemTime, u64)> {
    let mut out = std::collections::HashMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    let ext = std::env::consts::DLL_EXTENSION;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some(ext) {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            if let Ok(mtime) = meta.modified() {
                out.insert(path, (mtime, meta.len()));
            }
        }
    }
    out
}

fn poll_plugin_dir(
    time: Res<Time>,
    mut watcher: ResMut<PluginWatcher>,
    mut queue: ResMut<PluginReloadQueue>,
) {
    watcher.countdown -= time.delta_secs();
    if watcher.countdown > 0.0 {
        return;
    }
    watcher.countdown = POLL_INTERVAL;

    let Ok(entries) = std::fs::read_dir(&watcher.dir) else {
        return;
    };
    let ext = std::env::consts::DLL_EXTENSION;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some(ext) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let Ok(mtime) = meta.modified() else { continue };
        let stamp = (mtime, meta.len());

        // Copied out rather than matched on in place: the arms below mutate
        // `watcher`, and holding a borrow of `seen` across them does not compile.
        let previous = watcher.seen.get(&path).copied();
        match previous {
            // A file that was not present at boot. `seen` is seeded from the
            // startup scan, so this can only be a plugin dropped in mid-session —
            // pick it up. Recording it and doing nothing (which is what this arm
            // used to do) meant a newly added plugin sat there until the next
            // restart, and "I put a dll in plugins/, why is nothing happening" is
            // the one question that behaviour guarantees.
            //
            // Still goes through the settle check: a file being copied in is
            // exactly as half-written as one being rebuilt.
            None => {
                watcher.seen.insert(path.clone(), stamp);
                watcher.settling.insert(path);
            }
            Some(prev) if prev != stamp => {
                watcher.seen.insert(path.clone(), stamp);
                watcher.settling.insert(path);
            }
            // Unchanged. If it moved last poll, the write has finished.
            Some(_) => {
                if watcher.settling.remove(&path) && !queue.0.contains(&path) {
                    info!(
                        "[plugin] {} changed on disk, reloading",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    );
                    queue.0.push(path);
                }
            }
        }
    }
}

/// Host components a plugin is allowed to resolve by type path.
///
/// This exists because Bevy registers components **lazily** — adding
/// `TransformPlugin` registers the *type* for reflection but does not allocate a
/// `ComponentId` until something actually spawns or queries one. A plugin
/// loading at startup would therefore ask for a perfectly real component and get
/// nothing back, which is indistinguishable from a typo.
///
/// Registering eagerly is cheap (it allocates an id and nothing else) and has a
/// useful side effect: this list IS the public component surface. A host type
/// not named here is not reachable from a plugin, which is a decision worth
/// making deliberately rather than by accident of what happened to be spawned
/// before load.
fn register_exposed_components(world: &mut World) {
    world.register_component::<Transform>();
    world.register_component::<GlobalTransform>();
    world.register_component::<Visibility>();
    world.register_component::<Name>();
    world.register_component::<Mesh3d>();
}

/// Loads C-ABI plugins from `<exe-dir>/plugins/` during app build.
///
/// Runs at `build` time rather than in a startup system because plugins insert
/// systems into schedules, and doing that before the first frame avoids any
/// question about mutating a schedule that is mid-run.
pub struct RenzoraPluginHostPlugin {
    /// Whether this binary is the editor. Editor-scope plugins load only when
    /// it is; runtime-scope plugins load either way.
    pub is_editor: bool,
    /// Plugins compiled into this binary, initialised before the ones on disk.
    ///
    /// Empty for every build except a lean export that chose to link its plugins
    /// in — see [`crate::static_link`]. The two paths coexist deliberately: a
    /// game can ship some plugins inside the binary and still read a `plugins/`
    /// folder for anything a player or a mod drops in.
    pub statics: Vec<StaticPlugin>,
}

impl Plugin for RenzoraPluginHostPlugin {
    fn build(&self, app: &mut App) {
        let dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("plugins")))
            .unwrap_or_else(|| PathBuf::from("plugins"));

        register_exposed_components(app.world_mut());
        // Nothing is mapped yet, so last session's shadow copies are still
        // deletable. After this they are not. Editor-only: only the editor
        // makes these, and a game runtime launched from the editor's own
        // directory must not delete images that editor is running on.
        if self.is_editor {
            clear_shadow_dir(&dir);
        }

        // Reload machinery, before the initial load so a plugin that somehow
        // requests a reload during its own init is queued rather than lost.
        // Input is snapshotted in `PreUpdate`, before any plugin system in `First`
        // could read a stale one, and unconditionally — a system declaring
        // `Res<PluginInput>` must find it even on a headless server, where the
        // snapshot stays zeroed and every key reads as up.
        app.init_resource::<super::input::PluginInput>()
            .add_systems(PreUpdate, super::input::collect_input);

        // Service calls are parked for whichever engine crate claims them. The
        // sweep runs at the very end of the frame so a build missing a bridge —
        // a dedicated server, a lean 2D export — clears those calls rather than
        // growing the queue every frame a plugin makes one.
        app.init_resource::<super::PluginServiceCalls>()
            .add_systems(Last, super::discard_unhandled_service_calls);

        app.insert_resource(PluginHostConfig { is_editor: self.is_editor })
            .init_resource::<PluginReloadQueue>()
            .init_schedule(PluginReload)
            .add_systems(PluginReload, apply_reload_requests);
        app.world_mut()
            .resource_mut::<bevy::app::MainScheduleOrder>()
            .insert_before(First, PluginReload);

        // Watching is editor-only. A shipped game has no reason to restat its
        // plugin directory forever, and swapping code under a player is not a
        // feature — it is how a save file gets corrupted by a half-written build.
        if self.is_editor {
            app.insert_resource(PluginWatcher {
                dir: dir.clone(),
                // Seeded with what is on disk right now, which is what lets the
                // poll treat an unseen path as "added since boot" and load it. An
                // empty map would make every plugin look new a quarter-second
                // after startup and reload the lot.
                seen: stamp_dir(&dir),
                settling: Default::default(),
                countdown: POLL_INTERVAL,
            })
            .add_systems(Last, poll_plugin_dir);

            // The other half of the loop: watch plugin SOURCE, rebuild it, and drop
            // the artifact here — where the watcher above then picks it up. Only
            // this ordering works, so the two are installed together.
            super::dev::install(app, dir.clone());
        }

        // Linked-in plugins first, and their names then suppress any loose copy
        // of the same plugin in `plugins/` — see [`load_dir`] for why loading
        // both is considerably worse than loading either.
        for plugin in &self.statics {
            let id = plugin.id;
            match load_static(app.world_mut(), plugin, self.is_editor) {
                LoadOutcome::Loaded => info!("[plugin] linked {id}"),
                LoadOutcome::WrongScope(scope) => {
                    debug!("[plugin] skipping linked {id} — {scope:?} scope")
                }
                LoadOutcome::VersionTooOld => warn!(
                    "[plugin] linked {id} needs a newer renzora_plugin ABI than this build \
                     (host is {}.{})",
                    sys::VERSION_MAJOR,
                    sys::VERSION_MINOR
                ),
                LoadOutcome::Failed(why) => error!("[plugin] linked {id} failed: {why}"),
                // Cannot happen: there is no file to fail the symbol sniff.
                LoadOutcome::NotAPlugin => {}
            }
        }

        let linked: Vec<&str> = self.statics.iter().map(|p| p.id).collect();
        for (path, outcome) in load_dir(app.world_mut(), &dir, self.is_editor, &linked) {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            match outcome {
                LoadOutcome::Loaded => info!("[plugin] loaded {name}"),
                LoadOutcome::NotAPlugin => {}
                LoadOutcome::VersionTooOld => warn!(
                    "[plugin] {name} needs a newer renzora_plugin ABI than this build \
                     (host is {}.{})",
                    sys::VERSION_MAJOR,
                    sys::VERSION_MINOR
                ),
                // Debug, not warn: a game staged alongside the editor sees every
                // editor plugin in its `plugins/` directory, and saying so at
                // warn level once per plugin per launch is noise about something
                // working correctly.
                LoadOutcome::WrongScope(scope) => {
                    debug!("[plugin] skipping {name} — {scope:?} scope")
                }
                LoadOutcome::Failed(why) => error!("[plugin] {name} failed: {why}"),
            }
        }
    }
}

/// One C-ABI plugin found on disk, without loading it into a `World`.
///
/// The export UI needs this: it lists what a game *could* ship so the user can
/// tick plugins on and off, which happens long before (and independently of)
/// anything being installed into a running app.
#[derive(Debug, Clone)]
pub struct PluginInfo {
    /// File stem — what the export UI shows and what a selection is keyed on.
    pub id: String,
    pub path: PathBuf,
    pub scope: sys::PluginScope,
}

/// Enumerate the C-ABI plugins in `dir` by probing their exported symbols.
///
/// Replaces `dynamic_plugin_loader::scan_plugins`, which probed for the old
/// Bevy-cdylib symbols (`plugin_create` / `plugin_scope`). Those no longer exist
/// — and note the old scanner read `plugin_scope` while C-ABI plugins export
/// `renzora_plugin_scope`, so every plugin silently came back as Runtime and
/// Editor-scope ones were offered as shippable. Reading the right symbol is the
/// fix.
///
/// A library that does not export `renzora_plugin_init` is simply not a plugin
/// and is skipped, so unrelated DLLs sitting in the folder are ignored.
///
/// **Answered without mapping anything, wherever possible.** This used to
/// `Library::new` every file in the folder and let the handle drop at the end of
/// the iteration, which froze the editor the moment the export dialog opened:
/// `FreeLibrary` runs an image's static destructors under the Windows loader
/// lock, and `tracy.dll` starts a profiler thread at map time, so the unload
/// waited on a thread that needed the lock the unload was holding. It is the same
/// deadlock [`load_one`]'s `ManuallyDrop` exists to prevent — see the comment
/// there.
///
/// Mapping was doubly wrong here, not merely fatal on the way out. The editor
/// runs each plugin from a shadow copy under a different filename (see
/// [`shadow_copy`]), so the OS treats `plugins/tracy.dll` as an unrelated library
/// and maps a **second** live instance of it, initialisers and all — a second
/// Tracy client, from a dialog that only wanted to list filenames.
///
/// So: [`exports_plugin_init`] settles "is this a plugin?" from the file's bytes,
/// and the scope comes from [`LoadedPlugins`], which recorded it when the plugin
/// was loaded for real. Only a plugin this process never loaded — one added to
/// the folder since boot, or one whose load failed before the scope was read —
/// falls through to [`probe_scope`].
pub fn scan_plugins(world: &World, dir: &Path) -> Vec<PluginInfo> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let known = world.get_resource::<LoadedPlugins>();
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some(std::env::consts::DLL_EXTENSION)
            || is_proc_macro_dylib(&path)
            || !exports_plugin_init(&path)
        {
            continue;
        }
        let Some(scope) = known
            .and_then(|k| k.scope_of(&path))
            .or_else(|| probe_scope(&path))
        else {
            continue;
        };
        let id = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        out.push(PluginInfo { id, path, scope });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Read a plugin's scope by mapping it, for the case where nothing else knows.
///
/// The image is **never unmapped** — `ManuallyDrop`, for the reason
/// [`load_one`] spells out at length. That makes this the expensive answer, and
/// it is why [`scan_plugins`] asks [`LoadedPlugins`] first: it costs one
/// permanently mapped image per plugin this process has not otherwise loaded.
///
/// `None` means the file would not open at all, which [`scan_plugins`] treats as
/// "not something we can offer to ship".
fn probe_scope(path: &Path) -> Option<sys::PluginScope> {
    let library = std::mem::ManuallyDrop::new(unsafe { Library::new(path) }.ok()?);
    Some(
        match unsafe { library.get::<sys::ScopeEntry>(sys::SCOPE_SYMBOL.as_bytes()) } {
            Ok(f) => unsafe { f() },
            // No declaration means Runtime, matching `renzora::add!`'s default.
            Err(_) => sys::PluginScope::Runtime,
        },
    )
}
