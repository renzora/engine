//! Loads **native plugins** — Bevy plugins shipped as Rust source and compiled
//! on the machine that installs them, rebuilt whenever the engine moves.
//!
//! "Native" as in *native API*: one of these links the real Bevy and the real
//! contract crate, so it takes `&mut World`, calls `app.add_systems`, and sees
//! the same `Transform` the engine does. The C-ABI plugins in `renzora_plugin`
//! are equally native *code* — the distinction is that they share no types and
//! reach the engine through a fixed function table instead.
//!
//! # Both kinds live in `plugins/`, and neither loader needs to know
//!
//! To a user they are all just plugins, and the directory reflects that. The two
//! loaders never collide because both are symbol-dispatched and they do not even
//! look at the same shape of thing: the C-ABI loader does a non-recursive
//! `read_dir` filtered to the library extension, so a plugin *directory* is
//! invisible to it, while this one only considers directories containing
//! `src/lib.rs`. A built library sits under `<name>/build/`, one level below
//! anything the C-ABI scan reaches.
//!
//! ```text
//! <exe dir>/
//!   bevy_dylib-<hash>.dll   renzora_dylib.dll     the shared images
//!   sdk/                                          blueprints, optional
//!   plugins/
//!     grayscale.dll                               C-ABI: prebuilt, 52 KB
//!     lua.dll                                     C-ABI
//!     spin-thing/                                 native: shipped as source
//!       src/lib.rs                                  what the marketplace sent
//!       build/spin_thing.dll                        what rustc produced
//!       build/stamp.txt                             what it was built against
//! ```
//!
//! # Why the C-ABI mechanism is still needed
//!
//! Not a missing feature — a structural difference. A native plugin links the
//! shared `bevy_dylib` and `renzora_dylib`; a lean export drops
//! `dynamic_linking` and has neither, so a shipped game cannot load one, and nor
//! can wasm or mobile where dylibs do not exist at all. A C-ABI plugin links no
//! Bevy, is tens of KB, loads into any build, and can be linked *into* a static
//! binary via `static_plugins` — which is how `plugins/lua` reaches a shipped
//! game.
//!
//! So a native plugin extends the editor with full ECS access; a C-ABI plugin
//! ships inside the game. Two mechanisms because there are two deployments.
//!
//! # Why a stamp instead of trusting the file
//!
//! A plugin is bound to one engine build. Its crate metadata, its `TypeId`s
//! and its imports all come from artifacts whose filenames hash the build
//! configuration, so a plugin built against a different engine is not "probably
//! fine" — it is memory corruption with no diagnostic. There is no handshake to
//! catch it either: the boundary is a plain Rust `fn() -> Box<dyn Plugin>`, with
//! nothing to negotiate.
//!
//! So each build records what it was built against, and load compares. A
//! mismatch rebuilds (a second, and invisible) rather than loading. This is the
//! property that makes source-shipped plugins better than prebuilt ones: an
//! engine dev changes `crates/renzora`, the stamp stops matching, and every
//! installed plugin quietly rebuilds itself. Nothing has to be republished and
//! no plugin silently rots.
//!
//! # Why nothing is ever unloaded
//!
//! Every system a plugin registered is a function pointer into its image, and a
//! Bevy schedule holds those for the life of the `App`. Unmapping the image
//! turns them into dangling pointers. `renzora_plugin`'s loader learned this the
//! hard way twice — once as a `FreeLibrary` deadlock, once as a 0xC0000005 at
//! process teardown when a plain `Vec<Library>` was dropped with the World — so
//! the handles here are `ManuallyDrop` for the same reason: "never dropped" has
//! to include the last moment of the process, which is exactly the moment a
//! plain field does not give you.

use std::path::{Path, PathBuf};

pub mod prebuild;

use bevy::prelude::*;
use libloading::{Library, Symbol};
use renzora::{PluginKind, PluginState};
use renzora_plugin_build::{Error as BuildError, Sdk};

/// The one symbol a Rust plugin must export.
///
/// Symbol-dispatched like the C-ABI loader: a library without it is not a
/// plugin, and is skipped rather than treated as an error. Includes the trailing
/// NUL so it can go straight to `Library::get`.
pub const CTOR_SYMBOL: &[u8] = b"renzora_native_plugin_ctor\0";

/// The signature of [`CTOR_SYMBOL`].
///
/// A plain Rust `fn` returning a trait object, with no `extern "C"` and no
/// `#[repr(C)]` anywhere — which is sound *only* because both sides link one
/// shared `renzora_dylib` and one shared `bevy_dylib`, so `Plugin` is the same
/// trait and its vtable the same layout. Everything else in this crate exists to
/// guarantee that precondition still holds at the moment of the call.
type Ctor = fn() -> Box<dyn Plugin>;

renzora::add!(NativePluginLoader, Runtime);

/// Scans `<exe dir>/plugins/` for plugin directories, rebuilds what is stale,
/// and installs the rest. Loose `.dll` files there belong to the C-ABI loader
/// and are not looked at.
///
/// All of it happens in `build`, because a Bevy plugin can only be added while
/// the `App` is being assembled. A rebuild is therefore synchronous and holds
/// startup — about a second per stale plugin. That is the right trade for a
/// mechanism whose whole point is that plugins are never out of date, but it is
/// also why the editor should be showing progress by the time this ships to
/// people (see the install flow, not yet written).
#[derive(Default)]
pub struct NativePluginLoader {
    /// Where to look for `plugins/` and `sdk/`. `None` means beside the
    /// running executable, which is the shipped arrangement.
    ///
    /// Overridable because the alternative is a loader that can only be
    /// exercised by launching the whole engine — and the parts most worth
    /// testing here (stamp staleness, rebuild, the ctor call) have nothing to do
    /// with a running editor.
    pub root: Option<PathBuf>,
}

/// Adapts a `Box<dyn Plugin>` into something `add_plugins` accepts.
///
/// Bevy's own `App::add_boxed_plugin` is `pub(crate)`, and `Plugins` is a sealed
/// trait, so a trait object cannot be handed over directly. Every method
/// delegates — including `name`, which matters: Bevy's duplicate-plugin check
/// compares names, and returning this wrapper's own name would make two
/// unrelated plugins look like the same one.
struct Boxed(Box<dyn Plugin>);

impl Plugin for Boxed {
    fn build(&self, app: &mut App) {
        self.0.build(app);
    }
    fn ready(&self, app: &App) -> bool {
        self.0.ready(app)
    }
    fn finish(&self, app: &mut App) {
        self.0.finish(app);
    }
    fn cleanup(&self, app: &mut App) {
        self.0.cleanup(app);
    }
    fn name(&self) -> &str {
        self.0.name()
    }
    fn is_unique(&self) -> bool {
        self.0.is_unique()
    }
}

impl Plugin for NativePluginLoader {
    fn build(&self, app: &mut App) {
        // A host without the shared images cannot load these at all. The plugin
        // links `bevy_dylib`; a statically linked host has its own Bevy, so the
        // two disagree about what `App` is and passing one across is memory
        // corruption. It does not announce itself either — the first symptom is
        // something impossible, like `Schedules does not exist in the World` on
        // a `World` that has one.
        //
        // There is no runtime check available: the boundary is a plain Rust fn
        // with no handshake, and by the time anything could be inspected the
        // damage is done. So the gate is the same compile-time switch that puts
        // the shared images in the build.
        if !cfg!(feature = "dynamic_linking") {
            debug!("native plugins not loaded: this build links no shared engine image");
            return;
        }

        let Some(root) = self.root.clone().or_else(exe_dir) else {
            return;
        };
        let dir = root.join("plugins");
        if !dir.is_dir() {
            return;
        }

        // Absent SDK is normal, not an error: only a machine that has installed
        // a plugin has one. Already-built plugins with a matching stamp still
        // load — what is lost is the ability to *rebuild* a stale one.
        let sdk = Sdk::load(root.join("sdk")).ok();
        let expected = sdk.as_ref().map(Sdk::stamp);

        // Read once, off disk, because this runs during `App` assembly — there
        // is no settings resource yet, and there will not be one until long
        // after every plugin has been installed.
        let disabled = renzora::load_disabled_plugins();

        let mut libraries = Vec::new();
        for entry in read_dir_sorted(&dir) {
            let name = name_of(&entry);
            // Checked before anything else touches the directory, so a disabled
            // plugin costs nothing at all: no rebuild when its stamp is stale,
            // no `Library::new`, no static initializers. Turning a plugin off to
            // find out whether it is the one breaking your editor should not
            // leave it half-running.
            if disabled.iter().any(|d| d == &name) {
                if entry.join("src").join("lib.rs").is_file() {
                    info!("[plugin] {name} is disabled — Settings → Editor → Plugins");
                    record(app, &name, PluginState::Disabled);
                }
                continue;
            }
            match load_one(&entry, sdk.as_ref(), expected.as_deref()) {
                Ok(Some((plugin, lib))) => {
                    // Held for the life of the process. See the module doc.
                    libraries.push(std::mem::ManuallyDrop::new(lib));
                    app.add_plugins(Boxed(plugin));
                    record(app, &name, PluginState::Loaded);
                }
                Ok(None) => {}
                Err(e) => {
                    // Both, and neither is redundant. `warn!` reaches stdout and
                    // the Problems panel; the Console has no tracing layer and
                    // shows only what is pushed to it, and a plugin that failed
                    // to compile is exactly what its author is looking for.
                    warn!("plugin '{name}' not loaded: {e}");
                    renzora::core::console_log::console_error(
                        "Plugin",
                        format!("{name}\n{e}"),
                    );
                    record(app, &name, PluginState::Failed(e));
                }
            }
        }
        app.insert_resource(LoadedNativePlugins { _libraries: libraries });
    }
}

/// Keeps every loaded image mapped for the life of the process.
///
/// `ManuallyDrop` is load-bearing, not defensive — a resource is dropped when
/// the World is, which is every clean shutdown, and running `FreeLibrary` over
/// plugin images at teardown is what produced an access violation in the runtime
/// binary the last time this was done with a plain `Vec`.
#[derive(Resource)]
pub struct LoadedNativePlugins {
    _libraries: Vec<std::mem::ManuallyDrop<Library>>,
}

/// Note one plugin's fate for the Settings UI.
///
/// Reported from here rather than re-derived by the panel, because "is this
/// directory a plugin, and did it load?" is this loader's question — a second
/// implementation in the UI would list a different set the first time either
/// side's rules changed.
fn record(app: &mut App, name: &str, state: PluginState) {
    renzora::record_plugin(app.world_mut(), name, PluginKind::Native, state);
}

/// A constructed plugin and the image it came from, which must outlive it.
type Loaded = (Box<dyn Plugin>, Library);

/// Prepare and load one `plugins/<name>/` directory.
///
/// `Ok(None)` means "nothing to load here" — a stray file, or a directory that
/// is not a plugin. Only real failures are `Err`.
/// Where a plugin's artefacts live, and whether they need rebuilding.
struct Layout {
    lib_path: PathBuf,
    stamp_path: PathBuf,
    needs_build: bool,
}

/// Decide whether a plugin must be compiled before it can be loaded.
///
/// Shared by [`load_one`] and [`prebuild`], which MUST agree: the pre-boot pass
/// exists so that by the time the loader runs there is nothing left to build, and
/// if the two predicates diverged the loader would compile during `App` assembly
/// anyway — silently undoing the reason the pre-boot pass exists.
///
/// Rebuild when the stamp is absent, stale, or the artifact is missing.
/// `expected` is None only when no SDK is installed, in which case there is
/// nothing to rebuild against and an existing artifact is the best available.
/// Two independent reasons to rebuild, and both are load-bearing.
///
/// The stamp catches "the engine moved" — a user's case, where the source has
/// not changed at all but the artifacts it was built against have.
///
/// Source mtime catches "someone edited it", which the stamp cannot see because
/// the SDK did not move. That is not a niche case: a plugin author working from a
/// DOWNLOADED editor has no repository and no `xtask`. Editing
/// `plugins/<name>/src/lib.rs` and restarting is their entire loop, and without
/// this their edits would silently do nothing — the old library loads, behaves
/// exactly as before, and nothing reports why.
fn layout(dir: &Path, sdk: Option<&Sdk>, expected: Option<&str>) -> Layout {
    let name = name_of(dir);
    let build = dir.join("build");
    let ext = sdk
        .map(|s| s.manifest().lib_ext.clone())
        .unwrap_or_else(default_lib_ext);
    let lib_path = build.join(format!("{}.{ext}", name.replace('-', "_")));
    let stamp_path = build.join("stamp.txt");
    let current = std::fs::read_to_string(&stamp_path).ok();

    let stale = match (expected, current.as_deref()) {
        (Some(want), Some(have)) => want != have,
        (Some(_), None) => true,
        (None, _) => false,
    };
    let needs_build = stale || !lib_path.is_file() || source_newer_than(dir, &lib_path);
    Layout { lib_path, stamp_path, needs_build }
}

fn load_one(
    dir: &Path,
    sdk: Option<&Sdk>,
    expected: Option<&str>,
) -> Result<Option<Loaded>, String> {
    if !dir.is_dir() {
        return Ok(None);
    }
    // `src/lib.rs` is what makes a directory a plugin; `Sdk::compile` derives
    // the rest of the layout from the directory itself.
    if !dir.join("src").join("lib.rs").is_file() {
        return Ok(None);
    }

    let name = name_of(dir);
    let build = dir.join("build");
    let Layout { lib_path, stamp_path, needs_build } = layout(dir, sdk, expected);
    if needs_build {
        let Some(sdk) = sdk else {
            // Two quite different situations, and conflating them produces a
            // message that is actively wrong. Someone who has just dropped a
            // plugin folder in by hand has never built it against anything —
            // telling them it "was built for a different version" sends them
            // looking for a version problem that does not exist.
            return Err(if lib_path.is_file() {
                "was built for a different version of Renzora. Rebuilding it \
                 needs the plugin SDK, which is not installed — Settings → \
                 Plugins."
            } else {
                "ships as source and has not been compiled yet. Building it \
                 needs the plugin SDK, which is not installed — Settings → \
                 Plugins."
            }
            .to_string());
        };
        std::fs::create_dir_all(&build).map_err(|e| e.to_string())?;
        info!("compiling plugin '{name}' for this engine build");
        let stamp = sdk.compile(dir, &lib_path).map_err(|e| match e {
            // rustc's own diagnostics, written for the plugin author. Passing
            // them through unedited is more useful than any summary.
            BuildError::Compile(out) => format!("failed to compile:\n{out}"),
            // The toolchain states already phrase themselves for a person,
            // naming the compiler and where it would land. Relaying that beats
            // anything this layer could summarise.
            other => other.to_string(),
        })?;
        std::fs::write(&stamp_path, &stamp).map_err(|e| e.to_string())?;
    }

    // SAFETY: loading arbitrary native code, which runs the image's static
    // initializers. That is inherent to the mechanism; the protection is that
    // installation is an explicit act and the source is on disk to read.
    let lib = unsafe { Library::new(&lib_path) }.map_err(|e| e.to_string())?;
    let ctor: Symbol<Ctor> = match unsafe { lib.get(CTOR_SYMBOL) } {
        Ok(f) => f,
        // Not a plugin. Skipped silently, matching the C-ABI loader: a library
        // that does not export the entry point is simply not ours.
        //
        // Leaked rather than dropped, even here. `Library::new` has already run
        // the image's static initializers — a Rust dylib registers a panic hook
        // and touches std's lazily-initialised globals on the way in — and
        // unmapping it puts a `FreeLibrary` inside the loader lock on a
        // half-warmed image. That is the exact shape of the deadlock
        // `renzora_plugin`'s loader hit, and the "nothing registered anything
        // yet, so it must be safe" reasoning is what made it look fine there
        // too. One skipped library is a few hundred KB held until exit.
        Err(_) => {
            std::mem::forget(lib);
            return Ok(None);
        }
    };

    // A panic here would unwind across the library boundary, which is undefined.
    // Catching it turns a broken plugin into one that fails to load rather than
    // one that takes the editor with it. A segfault is still fatal — nothing at
    // this layer can help with that.
    let plugin = std::panic::catch_unwind(std::panic::AssertUnwindSafe(*ctor))
        .map_err(|_| "panicked while constructing".to_string())?;
    Ok(Some((plugin, lib)))
}

/// Whether anything under `dir/src` is newer than the built library.
///
/// A missing or unreadable library counts as stale — better to rebuild and find
/// out why than to skip and load something unexplained.
///
/// Recursive: a plugin large enough to have a `src/ui/panel.rs` still has to
/// rebuild when that file changes, and a one-level scan silently kept the old
/// library instead.
fn source_newer_than(dir: &Path, lib: &Path) -> bool {
    let Ok(built) = std::fs::metadata(lib).and_then(|m| m.modified()) else {
        return true;
    };
    fn any_newer(dir: &Path, built: std::time::SystemTime) -> bool {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return false;
        };
        entries.flatten().any(|e| {
            let p = e.path();
            if p.is_dir() {
                return any_newer(&p, built);
            }
            e.metadata()
                .and_then(|m| m.modified())
                .map(|t| t > built)
                .unwrap_or(true)
        })
    }
    any_newer(&dir.join("src"), built)
}

/// The directory holding `sdk/` and `plugins/`.
///
/// NOT simply the executable's parent: inside a Linux AppImage that is a
/// read-only temporary mount with none of this beside it. See
/// [`renzora_plugin_build::install`].
fn exe_dir() -> Option<PathBuf> {
    renzora_plugin_build::install::root()
}

/// Entries of `dir`, in a stable order.
///
/// Sorted because load order decides plugin-build order in the `App`, and a
/// directory iteration order that varies between machines would make a
/// misbehaving plugin reproduce for one person and not another.
fn read_dir_sorted(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    v.sort();
    v
}

fn name_of(p: &Path) -> String {
    p.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("?")
        .to_string()
}

fn default_lib_ext() -> String {
    if cfg!(target_os = "windows") {
        "dll"
    } else if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    }
    .to_string()
}
