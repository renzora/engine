//! What plugins this process found on disk, and what became of each.
//!
//! `renzora_native_plugin`'s loader reports here as it runs, so the Settings UI
//! can list every installed plugin without re-deriving the loader's rules.
//!
//! That last part is the reason this exists rather than the settings panel
//! doing its own `read_dir`. "Is this a plugin?" is a real question with a
//! non-obvious answer: a plugin is a directory containing `src/lib.rs` whose
//! manifest declares a `dylib`, or one holding nothing but a prebuilt `build/`,
//! and the loader skips entries for reasons — wrong scope, no shared engine
//! image, a build that failed — that a second implementation would silently
//! disagree about. A panel that lists a *different* set from the one the engine
//! loaded is worse than no panel, and the same mistake has been made here before
//! with script extensions.
//!
//! # Enabling and disabling
//!
//! [`load_disabled_plugins`](crate::load_disabled_plugins) is the persisted
//! list, keyed by [`PluginEntry::id`], and the loader consults it before
//! loading anything. Disabling **cannot** take effect until the next launch, and
//! that is structural rather than unfinished: a plugin adds systems, resources
//! and function pointers to the `App` while it is being assembled, and Bevy has
//! no way to withdraw those. Unmapping the image is worse still — a retired
//! system is still *in* the schedule, merely returning early.
//!
//! So the toggle records intent, and the loader acts on it at startup. The UI
//! says so plainly instead of pretending otherwise.
//!
//! Nothing here reads the disk: the loader runs during `App` assembly, before
//! any resource exists, so it reads the preference file directly and reports
//! the result to this inventory afterwards.

use bevy::prelude::*;

/// What happened to one plugin this launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginState {
    /// Installed into the `App`.
    Loaded,
    /// Turned off by the user; the loader did not open the file at all.
    Disabled,
    /// The loader declined it for a reason of its own — wrong scope for this
    /// process, no shared engine image to bind to, never built.
    /// Carries the reason, phrased for a person.
    Skipped(String),
    /// It should have loaded and did not. Carries the error.
    Failed(String),
}

impl PluginState {
    /// Whether the plugin is running right now.
    pub fn is_active(&self) -> bool {
        matches!(self, PluginState::Loaded)
    }
}

/// One plugin found on disk.
#[derive(Debug, Clone)]
pub struct PluginEntry {
    /// The stable key: the plugin's directory name under `plugins/`.
    ///
    /// This is what [`load_disabled_plugins`](crate::load_disabled_plugins)
    /// stores, so it has to be the same string on every platform. A directory
    /// name is, where the built library's filename is not — the same plugin
    /// compiles to `grayscale.dll` on Windows and `libgrayscale.so` on Linux,
    /// and a preference file that survives moving between them must not
    /// disagree about which plugin was turned off.
    pub id: String,
    pub state: PluginState,
}

/// Every plugin this process found, in the order the loaders reached them.
#[derive(Resource, Default)]
pub struct PluginInventory {
    pub entries: Vec<PluginEntry>,
}

impl PluginInventory {
    /// Record one plugin. Replaces any existing entry with the same id and kind,
    /// so a loader that re-scans does not double up.
    pub fn record(&mut self, id: impl Into<String>, state: PluginState) {
        let id = id.into();
        self.entries.retain(|e| e.id != id);
        self.entries.push(PluginEntry { id, state });
    }

    /// Entries sorted for display: by name, case-insensitively.
    pub fn sorted(&self) -> Vec<&PluginEntry> {
        let mut out: Vec<&PluginEntry> = self.entries.iter().collect();
        out.sort_by_key(|e| e.id.to_lowercase());
        out
    }
}

/// The live, editable mirror of the persisted disable list.
///
/// The loaders do **not** read this — they run during `App` assembly, before any
/// resource exists, and read the preference file directly. This is the copy the
/// Settings UI edits and the reactive bindings watch, saved back to disk on every
/// change. Same arrangement as `dev_mode`, which `load_dev_mode` reads off disk
/// for a plugin's benefit while `EditorSettings` carries the editable one.
///
/// Which means the resource and the file can disagree for exactly one session:
/// between toggling a plugin and restarting. That gap *is* the feature — see the
/// module doc on why disabling cannot take effect immediately.
#[derive(Resource, Default)]
pub struct DisabledPlugins(pub Vec<String>);

impl DisabledPlugins {
    pub fn contains(&self, id: &str) -> bool {
        self.0.iter().any(|d| d == id)
    }

    /// Turn a plugin on or off. Returns whether anything changed.
    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> bool {
        // Phrased as "is it already enabled" rather than negating the disabled
        // flag at the comparison: `enabled == !was` is the same test, but clippy
        // rejects it under `-D warnings` and it reads backwards anyway.
        let currently_enabled = !self.contains(id);
        if enabled == currently_enabled {
            return false;
        }
        if enabled {
            self.0.retain(|d| d != id);
        } else {
            self.0.push(id.to_string());
        }
        true
    }
}

/// Record a plugin into the world's inventory, creating it if needed.
///
/// A free function because the loader runs during `App` assembly, where the
/// resource may not exist yet.
pub fn record_plugin(world: &mut World, id: impl Into<String>, state: PluginState) {
    world
        .get_resource_or_insert_with(PluginInventory::default)
        .record(id, state);
}

/// Artwork filenames a plugin may ship, in the order they are tried.
///
/// **Every one of these is a real format in the wild**, which is the whole
/// reason this is a list. The marketplace converted its covers to WebP and every
/// thumbnail silently disappeared from both panels, because this used to name
/// `thumbnail.jpg` and nothing else: 68 of 72 installed plugins shipped
/// `thumbnail.webp`, one shipped `thumbnail.png`, and not one of them shipped
/// the name being looked for. The failure is invisible by construction, since a
/// plugin with no artwork is the ordinary case and draws a glyph.
///
/// WebP first because that is what the store publishes now. The rest are what a
/// plugin author might reasonably have on disk, and all four decode with the
/// features `renzora_ember` already enables.
#[cfg(not(target_arch = "wasm32"))]
const THUMBNAIL_FILES: &[&str] =
    &["thumbnail.webp", "thumbnail.png", "thumbnail.jpg", "thumbnail.jpeg"];

/// Where a plugin's store artwork lives: `<exe>/plugins/<id>/thumbnail.<ext>`.
///
/// Here rather than in either panel because two of them need it — Settings →
/// Plugins and the exporter's plugin picker — and a thumbnail that showed up in
/// one place and not the other would look like a broken image rather than a
/// disagreement about the path.
///
/// `None` when the install directory cannot be determined, which is the same
/// condition under which no plugins would have loaded either.
///
/// Searches every plugin root rather than assuming one beside the executable.
/// A macOS install has two — the plugins sealed inside the signed `.app` and
/// the ones the user installed into Application Support — and looking only at
/// the first would draw a placeholder for exactly the plugins someone chose to
/// install, which reads as a broken image rather than a missing file.
#[cfg(not(target_arch = "wasm32"))]
pub fn plugin_thumbnail_path(id: &str) -> Option<std::path::PathBuf> {
    let root = renzora_native_build::install::root()?;
    thumbnail_in(&renzora_native_build::install::plugin_dirs(&root), id)
}

/// The artwork for `id` under any of `dirs`, or the first candidate when there
/// is none.
///
/// Split from [`plugin_thumbnail_path`] so the search itself can be tested
/// without an install to point at.
///
/// Roots are the outer loop and filenames the inner one, so a plugin the user
/// installed wins over a same-named one sealed in the bundle whatever format
/// each of them chose. Ordering it the other way round would let a stale
/// `thumbnail.jpg` inside the `.app` beat the `thumbnail.webp` beside it.
///
/// Falling back to the first candidate when nothing exists keeps the old
/// behaviour for the caller, which expects a path it can test rather than a
/// `None` meaning "no plugins at all".
#[cfg(not(target_arch = "wasm32"))]
fn thumbnail_in(dirs: &[std::path::PathBuf], id: &str) -> Option<std::path::PathBuf> {
    let mut first = None;
    for dir in dirs {
        for name in THUMBNAIL_FILES {
            let path = dir.join(id).join(name);
            if first.is_none() {
                first = Some(path.clone());
            }
            if path.is_file() {
                return Some(path);
            }
        }
    }
    first
}

/// No install directory to search in a browser tab.
#[cfg(target_arch = "wasm32")]
pub fn plugin_thumbnail_path(_id: &str) -> Option<std::path::PathBuf> {
    None
}

/// The directory a plugin the user installs is written into.
///
/// The *writable* root specifically, which inside a macOS `.app` is not the one
/// plugins are read from — see `install::plugin_dirs`. This is what a "show me
/// the plugins folder" button opens: the place a new plugin goes, and the only
/// place one can be removed from.
///
/// Created if it is not there yet. An install that has never taken a plugin has
/// no such directory, and pointing a file manager at a path that does not exist
/// raises an error dialog rather than showing an empty folder — which reads as
/// the button being broken rather than as the folder being empty.
#[cfg(not(target_arch = "wasm32"))]
pub fn plugins_dir() -> Option<std::path::PathBuf> {
    let root = renzora_native_build::install::root()?;
    let dir = renzora_native_build::install::plugins_write_dir(&root);
    let _ = std::fs::create_dir_all(&dir);
    Some(dir)
}

/// Where `id` actually lives, searching every root the loader reads.
///
/// Nearest first, so a plugin the user installed shadows a bundled one of the
/// same name — the precedence `install::plugin_dirs` documents, and the one
/// [`thumbnail_in`] already follows.
#[cfg(not(target_arch = "wasm32"))]
pub fn plugin_dir(id: &str) -> Option<std::path::PathBuf> {
    let root = renzora_native_build::install::root()?;
    plugin_dir_in(&renzora_native_build::install::plugin_dirs(&root), id)
}

/// [`plugin_dir`] against an explicit set of roots, nearest first.
///
/// Split out for the same reason [`thumbnail_in`] is: the roots come from
/// `current_exe()`, so the search itself is untestable without one.
#[cfg(not(target_arch = "wasm32"))]
fn plugin_dir_in(dirs: &[std::path::PathBuf], id: &str) -> Option<std::path::PathBuf> {
    dirs.iter().map(|d| d.join(id)).find(|d| d.is_dir())
}

/// Whether `id` can be deleted, or ships sealed inside the editor.
///
/// The UI asks this to decide whether to draw a delete button at all, so the
/// button is never offered for something [`delete_plugin`] would refuse. Both
/// answer from the same rule — see [`removable_in`].
#[cfg(not(target_arch = "wasm32"))]
pub fn plugin_is_removable(id: &str) -> bool {
    let Some(root) = renzora_native_build::install::root() else {
        return false;
    };
    removable_in(
        &renzora_native_build::install::plugins_write_dir(&root),
        &renzora_native_build::install::plugin_dirs(&root),
        id,
    )
}

/// Not installed in a browser tab, so nothing to remove.
#[cfg(target_arch = "wasm32")]
pub fn plugin_is_removable(_id: &str) -> bool {
    false
}

/// The bundle-safety rule, as a function of paths alone.
///
/// A plugin is removable when the directory it was actually found in sits under
/// the *writable* root. On every platform but macOS there is only one root and
/// the answer is always yes; inside a `.app` there are two, and the second is
/// sealed by the bundle's signature.
///
/// Stated as "where it was found", not "does a directory of that name exist
/// under the writable root" — those differ for a plugin that exists in both,
/// which is the shadowing case `plugin_dirs` documents. The installed copy wins
/// and is removable; deleting it must not be refused because a bundled one of
/// the same name also exists, and must not delete the bundled one when it does
/// not.
#[cfg(not(target_arch = "wasm32"))]
fn removable_in(writable: &std::path::Path, dirs: &[std::path::PathBuf], id: &str) -> bool {
    plugin_dir_in(dirs, id).is_some_and(|found| found.starts_with(writable))
}

/// Delete an installed plugin from disk.
///
/// # What this deliberately cannot do
///
/// **It removes only from the writable root.** On macOS the editor reads plugins
/// from two places and one of them is sealed inside the signed `.app`. That
/// directory is still *writable*, which is the trap: `remove_dir_all` would
/// succeed, nothing would complain, and the damage would surface later as a
/// bundle that fails `codesign --verify` and is refused outright once
/// re-quarantined. A plugin that ships with the editor is refused by name here
/// instead, with a reason the user can act on.
///
/// **The plugin keeps running until the next launch.** Its code is mapped into
/// this process and Bevy cannot withdraw the systems, resources and function
/// pointers it registered — the same structural reason disabling waits for a
/// restart (see the module docs). Deleting the directory is about what the
/// *next* launch finds, and the UI has to say so rather than implying the plugin
/// is gone.
///
/// **It is a rename, not a `remove_dir_all`.** That mapped library is a file
/// Windows will not let anyone delete, so deleting a plugin that was actually
/// loaded, which is most of them, failed outright with a sharing violation,
/// while the doc-comment above claimed the delete was only about the next
/// launch. `retire_plugin_dir` moves the tree out of `plugins/` instead, which
/// is allowed for a mapped file, and a later launch that does not have it open
/// reclaims the bytes. The same swap is what lets the marketplace replace a
/// plugin it is running; see that module for the whole story.
#[cfg(not(target_arch = "wasm32"))]
pub fn delete_plugin(id: &str) -> Result<(), String> {
    let root = renzora_native_build::install::root()
        .ok_or_else(|| "could not work out where the editor is installed".to_string())?;
    let writable = renzora_native_build::install::plugins_write_dir(&root);
    let dir = removable_plugin_dir(&writable, id)?;
    if !dir.is_dir() {
        // Present, but not in the root that can be written to — on every
        // platform that is only ever the macOS bundle.
        if plugin_dir(id).is_some() {
            return Err(format!("`{id}` ships with the editor and cannot be removed"));
        }
        return Err(format!("`{id}` is not installed"));
    }
    renzora_native_build::install::retire_plugin_dir(&writable, id)
        .map(|_| ())
        .map_err(|e| format!("could not delete `{id}`: {e}"))
}

/// Resolve `<writable>/<id>`, refusing an id that is not a plain directory name.
///
/// Split out so the guard can be tested without an install, and kept separate
/// from the `is_dir` check so a caller can tell "refused" from "not there".
///
/// The guard matters more than it looks. `id` is a directory name the loader
/// read off disk, so in practice it is already safe — but it is the argument to
/// a `remove_dir_all`, and the cost of being wrong once is somebody's home
/// directory. An id carrying a separator or a `..` is refused rather than
/// normalised: nothing legitimate produces one, so there is no correct
/// interpretation to fall back to.
#[cfg(not(target_arch = "wasm32"))]
fn removable_plugin_dir(
    writable: &std::path::Path,
    id: &str,
) -> Result<std::path::PathBuf, String> {
    let one_plain_component = !id.is_empty()
        && !id.contains('/')
        && !id.contains('\\')
        && std::path::Path::new(id).components().count() == 1
        && !matches!(id, "." | "..");
    if !one_plain_component {
        return Err(format!("`{id}` is not a plugin name"));
    }
    Ok(writable.join(id))
}

/// Show `path` in the OS file manager.
///
/// **A folder opens; a file is revealed inside its folder.** The two are not the
/// same gesture, and treating them the same is what made this wrong once: every
/// platform arm reached for the *parent*, so asking to see a folder opened the
/// one above it, every time.
///
/// Selecting a folder inside its parent is technically "revealing" it, but
/// nobody asking to see a folder in their file manager means "show me the folder
/// next to its siblings". They mean open it.
///
/// Lives here rather than in a panel because three of them need it — the asset
/// browser's Reveal and the plugins panel's two — and a set of platform arms
/// this fiddly is exactly what should not be written twice.
#[cfg(not(target_arch = "wasm32"))]
pub fn reveal_in_explorer(path: &std::path::Path) {
    let is_dir = path.is_dir();
    #[cfg(target_os = "windows")]
    {
        if is_dir {
            let _ = std::process::Command::new("explorer").arg(path).spawn();
        } else {
            // No space after the comma, and one argument: `explorer` parses
            // `/select,<path>` as a single token.
            let _ = std::process::Command::new("explorer")
                .arg(format!("/select,{}", path.display()))
                .spawn();
        }
    }
    #[cfg(target_os = "macos")]
    {
        let mut cmd = std::process::Command::new("open");
        if !is_dir {
            cmd.arg("-R");
        }
        let _ = cmd.arg(path).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // `xdg-open` has no "select this file" mode: handed a file it *launches*
        // it in the default app, which is the one thing Reveal must not do. So a
        // file opens its containing folder, without the file selected in it.
        let target = if is_dir { path } else { path.parent().unwrap_or(path) };
        let _ = std::process::Command::new("xdg-open").arg(target).spawn();
    }
}

/// There is no file manager to show it in, and no filesystem to show.
///
/// A no-op rather than a missing function, so a caller reaching for Reveal does
/// not have to `cfg` around it — which is what the asset browser did before this
/// moved here, and what the web editor's build needs it to keep doing. The other
/// helpers in this module are absent on wasm instead, because "delete a plugin"
/// has no sensible empty answer the way "show this to the user" does.
#[cfg(target_arch = "wasm32")]
pub fn reveal_in_explorer(_path: &std::path::Path) {}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod delete_tests {
    use super::*;
    use std::path::PathBuf;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("renzora-del-{}-{tag}", std::process::id()))
            .join("plugins");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The ordinary case: an id names a directory directly under the writable
    /// root, and nothing else.
    #[test]
    fn a_plain_name_resolves_under_the_writable_root() {
        let dir = scratch("plain");
        let got = removable_plugin_dir(&dir, "system_monitor").expect("an ordinary name");
        assert_eq!(got, dir.join("system_monitor"));
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    /// The guard that matters: this path ends in `remove_dir_all`, so an id that
    /// could climb out of the plugins directory is refused rather than
    /// normalised. Nothing legitimate produces one.
    #[test]
    fn an_id_that_could_escape_the_plugins_directory_is_refused() {
        let dir = scratch("escape");
        for bad in ["", ".", "..", "../../etc", "a/b", "a\\b", "/etc", "plug/"] {
            assert!(
                removable_plugin_dir(&dir, bad).is_err(),
                "`{bad}` must be refused"
            );
        }
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    /// The property the guard exists to hold, stated directly: whatever it
    /// accepts resolves to an immediate child of the root it was handed.
    #[test]
    fn an_accepted_id_never_leaves_the_root() {
        let dir = scratch("contained");
        for name in ["a", "system_monitor", "a.b", "a-b_c", "chromatic_aberration"] {
            let got = removable_plugin_dir(&dir, name).expect("a single component");
            assert!(got.starts_with(&dir), "{name} escaped to {got:?}");
            assert_eq!(got.parent(), Some(dir.as_path()));
        }
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    /// The two roots a macOS install has: Application Support, then the sealed
    /// `Contents/MacOS/plugins` inside the `.app`. Returns `(writable, dirs)`
    /// in `plugin_dirs` order — writable first, which is precedence.
    fn two_roots(tag: &str) -> (PathBuf, Vec<PathBuf>) {
        let base = std::env::temp_dir().join(format!("renzora-rm-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let writable = base.join("Application Support").join("renzora").join("plugins");
        let bundled =
            base.join("Renzora Engine.app").join("Contents").join("MacOS").join("plugins");
        std::fs::create_dir_all(&writable).unwrap();
        std::fs::create_dir_all(&bundled).unwrap();
        (writable.clone(), vec![writable, bundled])
    }

    /// A plugin the user installed is removable.
    #[test]
    fn an_installed_plugin_is_removable() {
        let (writable, dirs) = two_roots("installed");
        std::fs::create_dir_all(writable.join("system_monitor")).unwrap();
        assert!(removable_in(&writable, &dirs, "system_monitor"));
        let _ = std::fs::remove_dir_all(writable.ancestors().nth(3).unwrap());
    }

    /// The rule that protects the signature: a plugin that exists ONLY inside
    /// the bundle is not removable. The directory is writable and
    /// `remove_dir_all` would succeed — which is exactly why the refusal has to
    /// be a decision rather than an error from the filesystem.
    #[test]
    fn a_bundled_plugin_is_not_removable() {
        let (writable, dirs) = two_roots("bundled");
        std::fs::create_dir_all(dirs[1].join("console")).unwrap();
        assert!(
            !removable_in(&writable, &dirs, "console"),
            "deleting this would break the .app's signature"
        );
        let _ = std::fs::remove_dir_all(writable.ancestors().nth(3).unwrap());
    }

    /// Shadowing: the same name in both roots. The installed copy wins and is
    /// removable — the presence of a bundled one must not refuse the delete,
    /// and the delete must not reach the bundled one.
    #[test]
    fn an_installed_plugin_shadowing_a_bundled_one_is_still_removable() {
        let (writable, dirs) = two_roots("shadow");
        std::fs::create_dir_all(writable.join("console")).unwrap();
        std::fs::create_dir_all(dirs[1].join("console")).unwrap();
        assert!(removable_in(&writable, &dirs, "console"));
        assert_eq!(
            plugin_dir_in(&dirs, "console"),
            Some(writable.join("console")),
            "the installed copy is the one found"
        );
        let _ = std::fs::remove_dir_all(writable.ancestors().nth(3).unwrap());
    }

    /// A flat install — every platform but a macOS bundle — has one root, so
    /// everything installed is removable.
    #[test]
    fn a_flat_install_can_remove_anything_it_has() {
        let dir = scratch("flat");
        std::fs::create_dir_all(dir.join("system_monitor")).unwrap();
        assert!(removable_in(&dir, std::slice::from_ref(&dir), "system_monitor"));
        assert!(!removable_in(&dir, std::slice::from_ref(&dir), "never_installed"));
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod thumbnail_tests {
    use super::*;
    use std::path::PathBuf;

    /// A scratch plugin root holding `<id>/<file>` for each name given.
    fn root(tag: &str, id: &str, files: &[&str]) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("renzora-thumb-{}-{tag}", std::process::id()))
            .join("plugins");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(id)).unwrap();
        for f in files {
            std::fs::write(dir.join(id).join(f), b"x").unwrap();
        }
        dir
    }

    /// The regression this list exists for. Every one of these is a format the
    /// store or an author has actually shipped, and naming only `thumbnail.jpg`
    /// meant none of them was ever found.
    #[test]
    fn every_shipped_format_is_found() {
        for name in THUMBNAIL_FILES {
            let dir = root(name, "crt", &[name]);
            assert_eq!(
                thumbnail_in(&[dir.clone()], "crt"),
                Some(dir.join("crt").join(name)),
                "{name} was not found"
            );
            let _ = std::fs::remove_dir_all(dir.parent().unwrap());
        }
    }

    /// WebP wins when a plugin carries more than one, because that is what the
    /// store publishes and the others are likely to be a stale leftover.
    #[test]
    fn webp_is_preferred() {
        let dir = root("order", "crt", &["thumbnail.jpg", "thumbnail.png", "thumbnail.webp"]);
        assert_eq!(
            thumbnail_in(&[dir.clone()], "crt"),
            Some(dir.join("crt").join("thumbnail.webp"))
        );
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    /// A root the user installed into beats one sealed in a bundle, whatever
    /// format each chose. Roots are the outer loop for exactly this: ordering it
    /// the other way would let a stale `.jpg` in the `.app` win.
    #[test]
    fn an_earlier_root_wins_over_a_later_one() {
        let user = root("user", "crt", &["thumbnail.jpg"]);
        let bundled = root("bundled", "crt", &["thumbnail.webp"]);
        assert_eq!(
            thumbnail_in(&[user.clone(), bundled.clone()], "crt"),
            Some(user.join("crt").join("thumbnail.jpg")),
            "the first root should win even with a less-preferred format"
        );
        let _ = std::fs::remove_dir_all(user.parent().unwrap());
        let _ = std::fs::remove_dir_all(bundled.parent().unwrap());
    }

    /// A plugin with no artwork is the ordinary case, and the caller wants a
    /// path it can test rather than a `None` that means "no plugins at all".
    #[test]
    fn no_artwork_still_returns_a_testable_path() {
        let dir = root("none", "crt", &[]);
        let got = thumbnail_in(&[dir.clone()], "crt").expect("a candidate path");
        assert!(!got.is_file());
        assert_eq!(got, dir.join("crt").join(THUMBNAIL_FILES[0]));
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }
}
