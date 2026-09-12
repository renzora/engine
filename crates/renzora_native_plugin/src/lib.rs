//! Loads **native plugins** — Bevy plugins shipped as Rust source and compiled
//! on the machine that installs them, rebuilt whenever the engine moves.
//!
//! "Native" as in *native API*: one of these links the real Bevy and the real
//! contract crate, so it takes `&mut World`, calls `app.add_systems`, and sees
//! the same `Transform` the engine does.
//!
//! ```text
//! <exe dir>/
//!   bevy_dylib-<hash>.dll   renzora_dylib.dll     the shared images
//!   sdk/                                          what plugins compile against
//!   plugins/
//!     spin-thing/                                 shipped as source
//!       src/lib.rs                                  what the marketplace sent
//!       build/spin_thing.dll                        what rustc produced
//!       build/stamp.txt                             what it was built against
//! ```
//!
//! # A plugin needs the shared images, so a lean export loads none
//!
//! Linking the real Bevy is only sound because the engine and the plugin share
//! one `bevy_dylib` and one `renzora_dylib`. A lean export drops
//! `dynamic_linking` and has neither, and wasm and mobile have no dylibs at all,
//! so there is nothing there for a plugin to bind to. The loader still walks the
//! directory in those builds and records why each plugin was declined, rather
//! than finding nothing and saying nothing.
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
//! turns them into dangling pointers. This engine learned it the hard way twice
//! — once as a `FreeLibrary` deadlock, once as a 0xC0000005 at process teardown
//! when a plain `Vec<Library>` was dropped with the World — so the handles here
//! are `ManuallyDrop`: "never dropped" has to include the last moment of the
//! process, which is exactly the moment a plain field does not give you.

use std::path::{Path, PathBuf};

/// First-run setup: unpack the shipped SDK and build every source-only plugin.
///
/// Desktop-only. It decompresses a ~1.9 GB archive and drives `rustc`, neither
/// of which a browser can do — and leaving it un-gated pulled `tar`, `filetime`
/// and a zstd decoder into every web game, because this crate is a hard
/// dependency of `renzora_runtime`. Its callers (`renzora_app`'s `main` and
/// `renzora_editor_app`'s) are gated to match.
#[cfg(not(target_arch = "wasm32"))]
pub mod prebuild;

use bevy::prelude::*;
#[cfg(not(target_arch = "wasm32"))]
use libloading::{Library, Symbol};
#[cfg(target_arch = "wasm32")]
use wasm_dl::{Library, Symbol};
use renzora::PluginState;
use renzora_plugin_build::{Error as BuildError, Sdk};

/// `libloading`'s shape, for a platform that has no dynamic loading at all.
///
/// A native plugin is a dylib the host `dlopen`s and links against the shared
/// `bevy_dylib` — a browser has neither, and `dynamic_linking` is never on for
/// wasm, so [`NativePluginLoader::build`] already returns before reaching any of
/// this. What it does not do is stop the module from being *compiled*, and
/// `libloading` has no wasm backend.
///
/// The alternative was `#[cfg]`-ing the loading half of a 700-line file out,
/// which would have meant gating every caller for a platform where they all
/// correctly find nothing anyway.
///
/// Opening always fails, so no plugin is ever found, and `Symbol` can never be
/// constructed — which is what makes its `Deref` unreachable rather than wrong.
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

/// The one symbol a Rust plugin must export.
///
/// Symbol-dispatched: a library without it is not a plugin, and is skipped
/// rather than treated as an error. Includes the trailing NUL so it can go
/// straight to `Library::get`.
pub const CTOR_SYMBOL: &[u8] = b"renzora_native_plugin_ctor\0";

/// Where a plugin says it may load, written by `renzora::plugin!`.
///
/// Optional. A plugin built before scopes existed does not export it, and its
/// absence reads as [`NativePluginScope::Editor`] — the behaviour those plugins
/// were written for, and the safe direction to guess: an editor plugin missing
/// from a game is an absence, a runtime plugin that should not have shipped is
/// in the player's hands.
pub const SCOPE_SYMBOL: &[u8] = b"renzora_native_plugin_scope\0";

/// The signature of [`SCOPE_SYMBOL`]. A byte, not an enum: this crosses a
/// `dlopen` boundary, where a `#[repr(Rust)]` enum has no guaranteed layout.
type ScopeFn = extern "C" fn() -> u8;

/// Read a built plugin library's declared scope, without keeping it loaded.
///
/// For the exporter, which has to decide whether a plugin belongs in a shipped
/// game before copying it. The alternative — parsing the source for a
/// `plugin!(.., Runtime)` — would answer a question about the *source* when what
/// ships is the *library*, and the two can disagree if one was not rebuilt.
///
/// Returns `None` when the file is not a native plugin at all (no constructor),
/// so a caller can tell "not ours" from "ours, and editor-only".
///
/// The image is deliberately leaked rather than dropped, for the reason the
/// loader documents at length: `Library::new` has already run the image's static
/// initializers, and unmapping a warmed Rust dylib runs `FreeLibrary` inside the
/// loader lock. In the editor this is usually a second handle on an image
/// already mapped, so the cost is a refcount.
/// One native plugin that has been built and can be loaded or shipped.
#[derive(Clone, Debug)]
pub struct InstalledNativePlugin {
    /// The directory name, which is the id everything else keys on — the
    /// Settings disable list, the export selection, the thumbnail path.
    pub id: String,
    /// Read from the built library, not the source. See [`installed`].
    pub scope: renzora::NativePluginScope,
    pub lib: PathBuf,
}

/// Every built native plugin under `plugins_dir`, with its scope.
///
/// Two callers need this list and were deriving it independently: the exporter,
/// to decide what to ship, and its plugin picker, to show what there is to
/// choose from. The directory layout — `<dir>/build/<name with - as _>.<ext>` —
/// was written out inline in the first of those, which is exactly the kind of
/// duplication that lets a picker offer a plugin the build then cannot find.
///
/// **Scope comes from the library, never the source.** A `plugin!(.., Runtime)`
/// in `src/lib.rs` says what the source *would* build to; what ships is the
/// artefact, and the two disagree whenever one was edited without rebuilding.
///
/// A directory with no built library is skipped: it is a plugin that has not
/// been compiled yet, and there is nothing to read a scope from or copy.
/// Every plugin installed for the engine at `root`, across all its plugin
/// roots.
///
/// [`installed`] answers for one directory. This is the question callers
/// actually have — "what does this install have" — and an install can keep
/// plugins in two places: inside a macOS `.app` the bundled ones are sealed by
/// the code signature, so anything the user installs lives in Application
/// Support instead.
///
/// Deduplicated by id with the writable root winning, matching the precedence
/// the loader applies, so an export ships the same plugin the editor is running.
pub fn installed_for(root: &Path, lib_ext: &str) -> Vec<InstalledNativePlugin> {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out = Vec::new();
    for dir in renzora_plugin_build::install::plugin_dirs(root) {
        for plugin in installed(&dir, lib_ext) {
            if seen.insert(plugin.id.clone()) {
                out.push(plugin);
            }
        }
    }
    out
}

pub fn installed(plugins_dir: &Path, lib_ext: &str) -> Vec<InstalledNativePlugin> {
    let Ok(entries) = std::fs::read_dir(plugins_dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        let Some(id) = dir.file_name().and_then(|n| n.to_str()).map(str::to_string) else {
            continue;
        };
        let lib = dir.join("build").join(format!("{}.{lib_ext}", id.replace('-', "_")));
        if !lib.is_file() {
            continue;
        }
        let Some(scope) = read_scope(&lib) else {
            continue;
        };
        out.push(InstalledNativePlugin { id, scope, lib });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

pub fn read_scope(lib_path: &Path) -> Option<renzora::NativePluginScope> {
    // SAFETY: loading native code, which runs its static initializers. Same
    // exposure the loader accepts, and here the library is one the editor
    // produced from source in this very project.
    let lib = unsafe { Library::new(lib_path) }.ok()?;
    if unsafe { lib.get::<Ctor>(CTOR_SYMBOL) }.is_err() {
        std::mem::forget(lib);
        return None;
    }
    let scope = match unsafe { lib.get::<ScopeFn>(SCOPE_SYMBOL) } {
        Ok(f) => renzora::NativePluginScope::from_byte(f()),
        // Built before scopes existed — editor-only, same as the loader assumes.
        Err(_) => renzora::NativePluginScope::Editor,
    };
    std::mem::forget(lib);
    Some(scope)
}

/// The signature of [`CTOR_SYMBOL`].
///
/// A plain Rust `fn` returning a trait object, with no `extern "C"` and no
/// `#[repr(C)]` anywhere — which is sound *only* because both sides link one
/// shared `renzora_dylib` and one shared `bevy_dylib`, so `Plugin` is the same
/// trait and its vtable the same layout. Everything else in this crate exists to
/// guarantee that precondition still holds at the moment of the call.
type Ctor = fn() -> Box<dyn Plugin>;

// NOT `renzora::add!`. The two binaries add this by hand, late in assembly, so
// the order is visible where the rest of the boot sequence is rather than at
// whatever position a generated alphabetical list happened to put it.

/// Scans `<exe dir>/plugins/` for plugin directories, rebuilds what is stale,
/// and installs the rest.
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
        //
        // NOT an early return: a build without them still walks the directory,
        // so every plugin it declines gets a card in Settings saying why. The
        // worst shape this failure can take is silence — the picker listed them,
        // the export copied them, and the game has no effects and says nothing.
        let shared_images = cfg!(feature = "dynamic_linking");

        let Some(root) = self.root.clone().or_else(exe_dir) else {
            return;
        };
        // Both roots — the writable one and, inside a macOS bundle, the plugins
        // that shipped with the editor. `plugin_entries` settles precedence.
        let entries = plugin_entries(&root);
        if entries.is_empty() {
            return;
        }

        // Absent SDK is normal, not an error: only a machine that has installed
        // a plugin has one. Already-built plugins with a matching stamp still
        // load — what is lost is the ability to *rebuild* a stale one.
        let sdk = Sdk::load(sdk_dir(&root)).ok();
        let expected = sdk.as_ref().map(Sdk::stamp);

        // Read once, off disk, because this runs during `App` assembly — there
        // is no settings resource yet, and there will not be one until long
        // after every plugin has been installed.
        let disabled = renzora::load_disabled_plugins();

        // Which scopes this process admits. `EditorSession` is inserted by the
        // editor binary during assembly; a shipped game has none, which is the
        // engine's standard way to tell the two apart (a cargo feature cannot —
        // both binaries come out of one `--workspace` build).
        let in_editor = app
            .world()
            .get_resource::<renzora::core::EditorSession>()
            .is_some_and(|s| s.0);

        let mut libraries = Vec::new();

        // Asked per DIRECTORY rather than per root, because `plugin_entries` has
        // already settled which directory each plugin name resolves to — a
        // bundled plugin is invisible here once the user has installed their own
        // copy of it.
        //
        // Every entry, NOT only the ones with a compiled artefact. This loop used
        // to be built from a `filter_map` over `artefact_in`, which dropped any
        // plugin that had never been built before the disabled check below could
        // record it — and `prebuild` deliberately does not build a disabled one.
        // The two together made a disabled, never-built plugin unreachable: no
        // artefact, so no inventory entry, so no card in Settings, so no way to
        // turn it back on short of hand-editing `settings.toml`.
        for entry in &entries {
            let name = name_of(entry);
            let artefact = artefact_in(entry);

            // A directory is only a plugin if it says so in source or has
            // something built in it. `plugin_entries` returns every non-dot
            // directory, so without this an unrelated folder someone dropped in
            // `plugins/` would get a card of its own.
            if !is_native_source(entry) && artefact.is_none() {
                continue;
            }

            // Checked before anything else touches the directory, so a disabled
            // plugin costs nothing at all: no rebuild when its stamp is stale,
            // no `Library::new`, no static initializers. Turning a plugin off to
            // find out whether it is the one breaking your editor should not
            // leave it half-running.
            if disabled.iter().any(|d| d == &name) {
                info!("[plugin] {name} is disabled — Settings → Editor → Plugins");
                record(app, &name, PluginState::Disabled);
                continue;
            }

            // Enabled, with source, and nothing built. `prebuild` runs before the
            // `App` exists precisely so this does not happen, so reaching it means
            // it could not: no SDK yet, or the build failed and left its
            // `failed.txt`. Recorded rather than skipped silently, so the card
            // says why instead of the plugin simply not being there.
            let Some(artefact) = artefact else {
                let why = if sdk.is_none() {
                    "has not been built — no SDK is installed (Settings → Plugins)".to_string()
                } else {
                    "has not been built yet — restart to build it".to_string()
                };
                info!("[plugin] {name} {why}");
                record(app, &name, PluginState::Skipped(why));
                continue;
            };
            let _ = &artefact;

            // A native plugin links the real Bevy and can only load into a host
            // that shares the same image. A static build has its own, so the two
            // disagree about what `App` is and handing one across is memory
            // corruption — with no runtime check available, because the boundary
            // is a plain Rust fn with no handshake. The gate is therefore the
            // same compile-time switch that puts the shared images in the build.
            if !shared_images {
                let why = "is a native plugin and this build links no shared engine image";
                debug!("[plugin] skipping {name} — {why}");
                record(app, &name, PluginState::Skipped(why.to_string()));
                continue;
            }

            match load_one(entry, sdk.as_ref(), expected.as_deref(), in_editor) {
                Ok(Outcome::Skipped(why)) => {
                    info!("[plugin] {name} {why}");
                    record(app, &name,PluginState::Skipped(why));
                }
                Ok(Outcome::Loaded((plugin, lib))) => {
                    // Held for the life of the process. See the module doc.
                    libraries.push(std::mem::ManuallyDrop::new(lib));
                    app.add_plugins(Boxed(plugin));
                    record(app, &name,PluginState::Loaded);
                }
                Ok(Outcome::NotAPlugin) => {}
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
                    record(app, &name,PluginState::Failed(e));
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
    renzora::record_plugin(app.world_mut(), name, state);
}

/// A constructed plugin and the image it came from, which must outlive it.
type Loaded = (Box<dyn Plugin>, Library);

/// What [`load_one`] found in a directory.
///
/// `Skipped` exists so a plugin the loader declined is REPORTED rather than
/// treated as absent. It is the difference between "my plugin does not load and
/// nothing says why" and one line naming it — which is the only feedback
/// available in a shipped game, where there is no Settings panel to look at.
enum Outcome {
    /// Loaded and ready to install.
    Loaded(Loaded),
    /// Not a plugin at all: a stray file, or a directory that is not one.
    NotAPlugin,
    /// A plugin, declined for a reason worth saying out loud.
    Skipped(String),
}

/// Prepare and load one `plugins/<name>/` directory.
///
/// `Ok(Outcome::NotAPlugin)` means "nothing to load here" — a stray file, or a
/// directory that
/// is not a plugin. Only real failures are `Err`.
/// Where a plugin's artefacts live, and whether they need rebuilding.
struct Layout {
    lib_path: PathBuf,
    stamp_path: PathBuf,
    /// Records a build that failed, so the same failure is not retried on every
    /// launch forever. See [`layout`].
    fail_path: PathBuf,
    needs_build: bool,
    /// True when a build is wanted but a previous identical attempt failed.
    /// The loader reports this instead of compiling again.
    build_failed_before: bool,
}

/// Decide whether a plugin must be compiled before it can be loaded.
///
/// Shared by [`load_one`] and [`prebuild`], which MUST agree: the pre-boot pass
/// exists so that by the time the loader runs there is nothing left to build, and
/// if the two predicates diverged the loader would compile during `App` assembly
/// anyway — silently undoing the reason the pre-boot pass exists.
///
/// Rebuild when the stamp is absent, stale, or the artifact is missing.
///
/// `expected` is None when no SDK is installed: there is nothing to compare
/// against, so an existing artifact is the best available.
///
/// The stamp catches "the engine moved" — a native plugin's case, where the
/// source has not changed at all but the artifacts it was built against have.
///
/// Source mtime catches "someone edited it", which the stamp cannot see because
/// the SDK did not move. That is not a niche case: a plugin author working from a
/// DOWNLOADED editor has no repository and no `xtask`. Editing
/// `plugins/<name>/src/lib.rs` and restarting is their entire loop, and without
/// this their edits would silently do nothing — the old library loads, behaves
/// exactly as before, and nothing reports why.
///
/// # Why a failed build is remembered
///
/// A plugin that does not compile used to put the editor in an infinite restart
/// loop, and the mechanism is worth stating because nothing about it is obvious
/// from either end. `prebuild::needed()` answers "is there work to do" from this
/// predicate; `main` runs the setup window when it says yes and then restarts the
/// process. A build that fails writes no artifact and no stamp, so the next
/// launch asks the same question, gets the same answer, and shows the same
/// window — forever, with two windows the user cannot close because each is
/// respawned by the next iteration.
///
/// So a failure is recorded in `build/failed.txt`, and a build is not attempted
/// again while that record still describes the current inputs. It invalidates
/// itself the two ways that matter: the file holds the SDK stamp it failed
/// against, so a moved engine retries, and it is compared by mtime against the
/// source, so an edit retries. That is exactly the plugin author's loop — fix
/// the code, relaunch, it builds — while a plugin nobody is editing stops
/// costing a compile per launch.
fn layout(dir: &Path, sdk: Option<&Sdk>, expected: Option<&str>) -> Layout {
    let name = name_of(dir);
    let build = dir.join("build");
    let ext = sdk
        .map(|s| s.manifest().lib_ext.clone())
        .unwrap_or_else(default_lib_ext);
    let lib_path = build.join(format!("{}.{ext}", name.replace('-', "_")));
    let stamp_path = build.join("stamp.txt");
    let fail_path = build.join("failed.txt");
    let current = std::fs::read_to_string(&stamp_path).ok();

    let stale = match (expected, current.as_deref()) {
        (Some(want), Some(have)) => want != have,
        (Some(_), None) => true,
        (None, _) => false,
    };
    let wants_build = stale || !lib_path.is_file() || source_newer_than(dir, &lib_path);

    // Does the recorded failure still describe what we would build right now?
    let failed_against = std::fs::read_to_string(&fail_path).ok();
    let build_failed_before = wants_build
        && match (expected, failed_against.as_deref()) {
            // Same SDK, and nothing edited since the attempt: it would fail
            // identically, so don't spend the compile finding that out.
            (Some(want), Some(have)) => want == have && !source_newer_than(dir, &fail_path),
            // No SDK, so nothing about the engine can change the outcome: the
            // record stands until the source is edited.
            //
            // This arm is load-bearing rather than tidy. Without it a plugin that
            // does not compile is retried on every launch, and since a pending
            // build is what makes `prebuild::needed()` true and `main` restarts
            // after running it, that is the endless setup window again.
            (None, Some(_)) => !source_newer_than(dir, &fail_path),
            _ => false,
        };

    Layout {
        lib_path,
        stamp_path,
        fail_path,
        needs_build: wants_build && !build_failed_before,
        build_failed_before,
    }
}

fn load_one(
    dir: &Path,
    sdk: Option<&Sdk>,
    expected: Option<&str>,
    // Whether this process is the editor. Decides which scopes are admitted —
    // see the scope check below.
    in_editor: bool,
) -> Result<Outcome, String> {
    if !dir.is_dir() {
        return Ok(Outcome::NotAPlugin);
    }
    let name = name_of(dir);
    let build = dir.join("build");
    let Layout {
        lib_path,
        stamp_path,
        fail_path,
        mut needs_build,
        build_failed_before,
    } = layout(dir, sdk, expected);

    // `src/lib.rs` plus a `dylib` crate-type is what makes a directory a plugin
    // *on a machine that can build one* — `Sdk::compile` derives the rest of the
    // layout from it.
    //
    // A shipped game has no source and no SDK: the export staged the library the
    // editor had already built. So a directory holding a built library and
    // nothing else is a plugin too, and one that can only be loaded. Requiring
    // the source here would mean shipping a plugin author's code inside every
    // game that uses it, to satisfy a marker file nothing would then read.
    if !is_native_source(dir) {
        if !lib_path.is_file() {
            // Neither source nor library: a stray directory, not a plugin.
            return Ok(Outcome::NotAPlugin);
        }
        // Nothing to build FROM, so never mind what `layout` concluded — its
        // staleness test compares source mtimes that do not exist.
        needs_build = false;
    }

    // A build we already know fails. Report it — the author still needs telling
    // that their plugin is not loaded — but do not compile it again: retrying on
    // every launch is what put the editor in a restart loop. See `layout`.
    if build_failed_before {
        return Err(format!(
            "failed to compile last time and has not changed since. Fix it and \
             relaunch, or delete {} to force a retry.",
            fail_path.display()
        ));
    }

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
        let stamp = sdk.compile(dir, &lib_path).map_err(|e| {
            // Record the failure before returning, against the SDK stamp it was
            // attempted with. Without this the next launch retries identically —
            // see `layout` for why that loops rather than merely wasting time.
            if let Some(want) = expected {
                let _ = std::fs::write(&fail_path, want);
            }
            match e {
                // rustc's own diagnostics, written for the plugin author. Passing
                // them through unedited is more useful than any summary.
                BuildError::Compile(out) => format!("failed to compile:\n{out}"),
                // The toolchain states already phrase themselves for a person,
                // naming the compiler and where it would land. Relaying that beats
                // anything this layer could summarise.
                other => other.to_string(),
            }
        })?;
        std::fs::write(&stamp_path, &stamp).map_err(|e| e.to_string())?;
        // It built: drop any record of it having failed before, so a later
        // genuine failure is not mistaken for this one.
        let _ = std::fs::remove_file(&fail_path);
    }

    // Decline a library that is not ours BEFORE mapping it, not after.
    //
    // Doing it after `Library::new` would not do. Opening an image runs its
    // static initializers, and the `Err` arm below can then only leak it —
    // unmapping a half-warmed image is the `FreeLibrary` deadlock. Every library
    // declined that way is an image initialised and held for the life of the
    // process, every launch.
    //
    // A byte search over the file rather than a symbol lookup: an exported name
    // appears verbatim in the export table of a PE, an ELF and a Mach-O alike,
    // so this settles it with no format parsing and nothing mapped.
    if !exports_ctor(&lib_path) {
        return Ok(Outcome::NotAPlugin);
    }

    // SAFETY: loading arbitrary native code, which runs the image's static
    // initializers. That is inherent to the mechanism; the protection is that
    // installation is an explicit act and the source is on disk to read.
    let lib = unsafe { Library::new(&lib_path) }.map_err(|e| e.to_string())?;
    let ctor: Symbol<Ctor> = match unsafe { lib.get(CTOR_SYMBOL) } {
        Ok(f) => f,
        // Not a plugin. Skipped silently: a library that does not export the
        // entry point is simply not ours.
        //
        // Leaked rather than dropped, even here. `Library::new` has already run
        // the image's static initializers — a Rust dylib registers a panic hook
        // and touches std's lazily-initialised globals on the way in — and
        // unmapping it puts a `FreeLibrary` inside the loader lock on a
        // half-warmed image, which has deadlocked here before. The "nothing
        // registered anything yet, so it must be safe" reasoning is exactly what
        // made it look fine then. One skipped library is a few hundred KB held
        // until exit.
        Err(_) => {
            std::mem::forget(lib);
            return Ok(Outcome::NotAPlugin);
        }
    };

    // ── Scope: does this plugin belong in the process that just loaded it? ───
    //
    // Read before the constructor is called, because calling it is what builds
    // the plugin and there is no way to un-add one afterwards.
    //
    // A missing symbol is `Editor`, which is what every native plugin was before
    // scopes existed — so an old plugin keeps its old behaviour instead of
    // appearing in a game it was never written for.
    let scope = match unsafe { lib.get::<ScopeFn>(SCOPE_SYMBOL) } {
        Ok(f) => renzora::NativePluginScope::from_byte(f()),
        Err(_) => renzora::NativePluginScope::Editor,
    };
    // `EditorSession` absent means this is the shipped game — the same check the
    // rest of the engine uses, because a cargo feature cannot answer it (both
    // binaries come out of one `--workspace` build).
    if !in_editor && scope == renzora::NativePluginScope::Editor {
        std::mem::forget(lib);
        // Reported, not silent. A plugin sitting in a game's `plugins/` and
        // doing nothing, with nothing said about it, is indistinguishable from a
        // loader that is broken — and a shipped game has no Settings panel to
        // check, so this line is the only feedback there is.
        return Ok(Outcome::Skipped(
            "is editor-only and does not load in a game. Declare \
             `renzora::plugin!(.., Runtime)` to ship it with one."
                .to_string(),
        ));
    }

    // A panic here would unwind across the library boundary, which is undefined.
    // Catching it turns a broken plugin into one that fails to load rather than
    // one that takes the editor with it. A segfault is still fatal — nothing at
    // this layer can help with that.
    let plugin = std::panic::catch_unwind(std::panic::AssertUnwindSafe(*ctor))
        .map_err(|_| "panicked while constructing".to_string())?;
    Ok(Outcome::Loaded((plugin, lib)))
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

/// The directory holding `plugins/` and the shipped `sdk.tar.zst`.
///
/// NOT simply the executable's parent: inside a Linux AppImage that is a
/// read-only temporary mount with none of this beside it. See
/// [`renzora_plugin_build::install`].
fn exe_dir() -> Option<PathBuf> {
    renzora_plugin_build::install::root()
}

/// The unpacked SDK tree for an install rooted at `root`.
///
/// Not `root.join("sdk")` — on macOS the tree lives under Application Support,
/// because unpacking it into a signed `.app` would break the bundle's seal. See
/// [`renzora_plugin_build::install::sdk_dir`].
fn sdk_dir(root: &Path) -> PathBuf {
    renzora_plugin_build::install::sdk_dir(root)
}

/// Does this library export the native plugin constructor?
///
/// The cheap half of "is this one of ours" — see the call site in [`load_one`]
/// for why it has to be answered before the image is mapped rather than after.
///
/// `CTOR_SYMBOL` carries a trailing NUL for `libloading`; the export table does
/// not, so the search drops it.
fn exports_ctor(path: &Path) -> bool {
    let needle = CTOR_SYMBOL.strip_suffix(b"\0").unwrap_or(CTOR_SYMBOL);
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    bytes.windows(needle.len()).any(|w| w == needle)
}

/// The library a built plugin leaves in its directory: `<dir>/build/<id>.<ext>`.
///
/// `None` when the plugin has never been built, or its build failed.
pub fn artefact_in(dir: &Path) -> Option<PathBuf> {
    let id = dir.file_name()?.to_str()?;
    let lib = dir.join("build").join(format!(
        "{}.{}",
        id.replace('-', "_"),
        std::env::consts::DLL_EXTENSION
    ));
    lib.is_file().then_some(lib)
}

/// Is this directory a plugin's source — one this crate can compile?
///
/// Source alone is not enough, because `plugins/` holds more than source: a
/// prebuilt plugin shipped inside a game arrives as a directory with nothing in
/// it but its `build/`.
///
/// `crate-type = ["dylib"]` is the test because it already IS what makes a
/// directory compilable here, rather than a convention layered on top: the
/// driver hands it `--crate-type dylib`, `--extern bevy` and
/// `-C prefer-dynamic`, and anything shaped differently fails. A failing build
/// is what makes `prebuild::needed()` true, so getting this wrong means the
/// editor shows the setup window, builds nothing, and restarts.
///
/// A directory with no `Cargo.toml` answers false and is handled a step later:
/// if it holds a built library it is a shipped-game plugin, and if it holds
/// neither it is not a plugin at all.
pub fn is_native_source(dir: &Path) -> bool {
    if !dir.join("src").join("lib.rs").is_file() {
        return false;
    }
    let Ok(text) = std::fs::read_to_string(dir.join("Cargo.toml")) else {
        return false;
    };
    text.lines()
        .filter(|l| l.trim_start().starts_with("crate-type"))
        .any(|l| l.contains("\"dylib\""))
}

/// Entries of `dir`, in a stable order.
///
/// Sorted because load order decides plugin-build order in the `App`, and a
/// directory iteration order that varies between machines would make a
/// misbehaving plugin reproduce for one person and not another.
/// Every plugin directory visible to this install, deduplicated by name.
///
/// An install can have two plugin roots — see
/// [`renzora_plugin_build::install::plugin_dirs`]. Inside a macOS `.app` the
/// writable one under Application Support comes first and the bundled one
/// second, so a plugin the user installed shadows a plugin of the same name
/// that shipped with the editor.
///
/// One function rather than the same loop in the loader and in `prebuild`,
/// because those two MUST agree on what exists. They already share `layout` for
/// the same reason: if the pre-boot pass and the loader disagreed about which
/// directories to look in, the loader would compile during `App` assembly —
/// silently undoing the reason the pre-boot pass exists.
pub(crate) fn plugin_entries(root: &Path) -> Vec<PathBuf> {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out = Vec::new();
    for dir in renzora_plugin_build::install::plugin_dirs(root) {
        for entry in read_dir_sorted(&dir) {
            if !entry.is_dir() {
                continue;
            }
            let name = name_of(&entry);
            // Dotfiles are not plugins, and `.DS_Store` is a file macOS drops in
            // every directory a user has opened in the Finder.
            if name.starts_with('.') {
                continue;
            }
            if seen.insert(name) {
                out.push(entry);
            }
        }
    }
    out
}

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

pub(crate) fn name_of(p: &Path) -> String {
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

/// Regression cover for the failed-build record in [`layout`].
///
/// The bug this pins: a plugin that would not compile made `needs_build` true
/// forever. `prebuild::needed()` reads that, `main` runs the setup window when it
/// says yes and then restarts the process — so a single broken plugin looped the
/// editor through setup windows endlessly, none of which could be closed because
/// the next iteration reopened them.
///
/// Only `layout` is exercised. It is the shared predicate both the pre-boot pass
/// and the loader ask, so it is where the loop begins and ends.
#[cfg(test)]
mod tests {
    use super::*;

    /// A plugin directory with a source file, in a unique temp path.
    fn plugin(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "renzora_layout_{tag}_{}_{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).expect("create plugin dir");
        std::fs::write(dir.join("src").join("lib.rs"), "// source").expect("write source");
        dir
    }

    /// Enough of a gap that the next write lands on a later mtime even on a
    /// filesystem with coarse timestamps.
    fn settle() {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    fn record_failure(dir: &Path, stamp: &str) {
        let build = dir.join("build");
        std::fs::create_dir_all(&build).expect("create build dir");
        std::fs::write(build.join("failed.txt"), stamp).expect("write failure record");
    }

    #[test]
    fn fresh_plugin_wants_building() {
        let dir = plugin("fresh");
        let l = layout(&dir, None, Some("stamp-a"));
        assert!(l.needs_build, "a plugin with no artifacts must build");
        assert!(!l.build_failed_before);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The loop, directly: same engine, untouched source, previous failure.
    #[test]
    fn known_failure_is_not_retried() {
        let dir = plugin("known");
        settle();
        record_failure(&dir, "stamp-a");

        let l = layout(&dir, None, Some("stamp-a"));
        assert!(
            !l.needs_build,
            "retrying a build already known to fail is what looped the editor"
        );
        assert!(l.build_failed_before, "the loader needs this to report it instead");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The engine moved: the failure said nothing about *this* SDK.
    #[test]
    fn failure_record_expires_when_the_engine_moves() {
        let dir = plugin("moved");
        settle();
        record_failure(&dir, "stamp-a");

        let l = layout(&dir, None, Some("stamp-b"));
        assert!(l.needs_build, "a new engine build must be retried");
        assert!(!l.build_failed_before);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The author fixed it. This is the loop that matters most — edit, relaunch,
    /// build — and a failure record that outlived an edit would break it.
    #[test]
    fn failure_record_expires_when_the_source_is_edited() {
        let dir = plugin("edited");
        record_failure(&dir, "stamp-a");
        settle();
        std::fs::write(dir.join("src").join("lib.rs"), "// fixed").expect("edit source");

        let l = layout(&dir, None, Some("stamp-a"));
        assert!(l.needs_build, "an edit must be retried");
        assert!(!l.build_failed_before);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Guarding on `crate-type` rather than on the marketplace sidecar because
    /// the manifest is the only thing that knows, and a plugin dropped in by hand
    /// has no sidecar.
    #[test]
    fn a_dylib_plugin_is_native_source() {
        let dir = plugin("native");
        std::fs::write(
            dir.join("Cargo.toml"),
            "[lib]\ncrate-type = [\"dylib\"]\n\n[dependencies]\nbevy = \"0.19\"\n",
        )
        .expect("write manifest");
        assert!(is_native_source(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A shipped game's plugin directory: a built library and no source at all.
    /// Answering false here is what routes it to the load-only path rather than
    /// to a compile it has nothing to compile from.
    #[test]
    fn a_directory_with_no_manifest_is_not_native_source() {
        let dir = plugin("nomanifest");
        assert!(!is_native_source(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `"cdylib"` also ends in `dylib`, so a loose match would claim a directory
    /// the driver cannot compile. A failing build is what makes
    /// `prebuild::needed()` true, so that is the endless setup window.
    #[test]
    fn only_a_dylib_crate_type_is_claimed() {
        let native = plugin("route_native");
        std::fs::write(native.join("Cargo.toml"), "[lib]\ncrate-type = [\"dylib\"]\n").unwrap();
        assert!(is_native_source(&native));

        let other = plugin("route_cdylib");
        std::fs::write(other.join("Cargo.toml"), "[lib]\ncrate-type = [\"cdylib\"]\n").unwrap();
        assert!(!is_native_source(&other));

        let _ = std::fs::remove_dir_all(&native);
        let _ = std::fs::remove_dir_all(&other);
    }

    /// A plugin built somewhere else and dropped in: no source, no manifest,
    /// just the library. Nothing to build, and the loader loads it.
    #[test]
    fn a_prebuilt_plugin_needs_no_builder() {
        let dir = plugin("prebuilt");
        std::fs::remove_file(dir.join("src").join("lib.rs")).unwrap();
        assert!(!is_native_source(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// "Is this one of ours" is a question about the file's exports, and it has
    /// to be answerable without mapping it: see the note at the call site on why
    /// declining after `Library::new` leaks an initialised image.
    #[test]
    fn a_library_without_the_ctor_symbol_is_declined() {
        let dir = plugin("symbols");
        let ours = dir.join("native.bin");
        let theirs = dir.join("other.bin");
        // The name as it appears in an export table: no trailing NUL.
        std::fs::write(&ours, b"\x7fELF...renzora_native_plugin_ctor...").unwrap();
        std::fs::write(&theirs, b"\x7fELF...some_other_entry_point...").unwrap();

        assert!(exports_ctor(&ours));
        assert!(!exports_ctor(&theirs), "a library that is not ours must not be claimed");
        assert!(!exports_ctor(&dir.join("absent.bin")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// With no SDK installed `layout` is asked with `expected: None` — and the
    /// failure record still has to hold, or a plugin that will not compile
    /// reopens the setup window on every launch forever.
    #[test]
    fn an_unstamped_failure_is_not_retried_until_the_source_moves() {
        let dir = plugin("unstamped");
        record_failure(&dir, "");

        let l = layout(&dir, None, None);
        assert!(l.build_failed_before, "the record must still apply");
        assert!(!l.needs_build, "and must stop the retry");

        settle();
        std::fs::write(dir.join("src").join("lib.rs"), "// fixed").expect("edit source");
        let l = layout(&dir, None, None);
        assert!(l.needs_build, "an edit must be retried");
        assert!(!l.build_failed_before);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// With nothing to compare against, only a missing artefact or an edited
    /// source may cost a rebuild. Whatever a previous build wrote beside the
    /// library is provenance, not a key.
    #[test]
    fn an_unstamped_artefact_survives_a_changed_toolchain() {
        let dir = plugin("toolchain_moved");
        let build = dir.join("build");
        std::fs::create_dir_all(&build).unwrap();
        std::fs::write(build.join(format!("{}.{}", name_of(&dir), default_lib_ext())), b"lib")
            .unwrap();
        // Whatever a previous build recorded beside it is provenance, not a key.
        std::fs::write(build.join("stamp.txt"), "rustc 1.90.0").unwrap();

        assert!(!layout(&dir, None, None).needs_build);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
