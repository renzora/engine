//! Recognising a hand-written Bevy crate as something the editor can open.
//!
//! A Renzora project is a folder with a `project.toml` in it. A Bevy project is
//! a folder with a `Cargo.toml` that depends on `bevy`, and it has no
//! `project.toml` at all, which is the point. Somebody who wants an editor for
//! the game they already wrote is not going to accept a file being dropped into
//! their repository as the price of looking at it, so nothing here writes
//! anything: [`config_for`] synthesizes the [`ProjectConfig`] in memory and the
//! folder is left exactly as it was found.
//!
//! # Why the parsing is here and not in `renzora_bevy_project`
//!
//! Three unrelated callers need the same answer to "is this a Bevy project":
//! [`crate::open_project`], which has to produce a config for a folder with no
//! config in it; the launcher, which decides whether a recent-projects row is
//! still openable; and `renzora_bevy_project`, which compiles the thing. A
//! second copy of the rule in any of them is a folder that opens in one place
//! and is rejected in another.
//!
//! # What it deliberately does not do
//!
//! It does not read a line of Rust. Everything here comes out of `Cargo.toml`:
//! which file is the crate root, which edition it is written in, what it
//! depends on. Finding the modules and the `App` construction means scanning
//! source, that is a much less certain business, and it belongs next to the
//! code that generates from it rather than in the crate everything links.

use std::path::{Path, PathBuf};

use bevy::ecs::component::ComponentId;
use bevy::platform::collections::HashMap;
use bevy::prelude::Resource;

use crate::core::project_config::{ProjectConfig, ProjectKind};

// No `Eq`: `toml::Value` holds floats, so its map is `PartialEq` only.
#[derive(Debug, Clone, PartialEq)]
pub struct BevyCrate {
    /// The directory holding the `Cargo.toml` this was read from. For a
    /// workspace this is the member's directory, not the workspace root.
    pub dir: PathBuf,
    /// `[package] name`, verbatim (so it may contain `-`).
    pub package: String,
    /// `[package] edition`, defaulted to `2015` the way cargo does when the key
    /// is absent. Passed to `rustc`: the plugin compiler used to hardcode
    /// `2021`, which silently mis-compiles a 2024 crate's `unsafe` attributes
    /// and `gen` identifiers.
    pub edition: String,
    /// The crate root `rustc` should be pointed at: `[lib] path`, else
    /// `src/lib.rs`, else the first `[[bin]] path`, else `src/main.rs`.
    ///
    /// A lib root is preferred over a bin root when both exist, because
    /// `crate::` inside the library's modules has to keep meaning the library.
    pub root: PathBuf,
    /// Is [`Self::root`] a `lib.rs`-shaped root rather than a `main.rs`-shaped
    /// one? Decides where the `App` construction is looked for.
    pub root_is_lib: bool,
    /// `src/main.rs` (or the first `[[bin]] path`) when there is one, whether or
    /// not it is [`Self::root`]. This is where `fn main` lives.
    pub bin: Option<PathBuf>,
    /// The `bevy` dependency's version requirement as written, for the version
    /// check. `None` when `bevy` is a path or git dependency with no `version`
    /// key, which is a shape worth allowing and not worth guessing about.
    pub bevy_req: Option<String>,
    /// Does this crate have a `build.rs`? A build script's output never reaches
    /// the plugin compiler (it invokes `rustc` directly and runs no build
    /// scripts at all), so a crate with one compiles without whatever it
    /// generates, usually as a missing `include!` and never as a message about
    /// build scripts.
    pub has_build_script: bool,
    /// Every `[dependencies]` entry verbatim, as a TOML fragment per crate, so
    /// the staging manifest can forward them to the dependency builder.
    /// `bevy` and `renzora*` are NOT filtered here; `renzora_native_build::deps`
    /// owns that rule and owning it twice is how the two drift.
    pub dependencies: Vec<(String, String)>,
    /// `[package.metadata.renzora]`, verbatim, for the escape hatches: naming
    /// the plugins by hand when the `App` cannot be read out of `fn main`, and
    /// picking a workspace member.
    pub metadata: toml::Table,
}

impl BevyCrate {
    /// The crate name `rustc` is given: the package name with `-` replaced,
    /// which is what cargo would have called it.
    pub fn crate_name(&self) -> String {
        self.package.replace('-', "_")
    }

    /// A `[package.metadata.renzora]` key, as a list of strings.
    pub fn metadata_list(&self, key: &str) -> Vec<String> {
        self.metadata
            .get(key)
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
            .unwrap_or_default()
    }

    /// A `[package.metadata.renzora]` key, as one string.
    pub fn metadata_str(&self, key: &str) -> Option<String> {
        self.metadata.get(key).and_then(|v| v.as_str()).map(str::to_string)
    }
}

/// Is `dir` a folder the Bevy loader should try to open?
///
/// Cheap enough for the launcher to call per row: one `read_to_string` of a file
/// that is a few hundred bytes, and no directory walk.
///
/// # The `project.toml` is the answer when there is one
///
/// This used to read "a `project.toml` means it is **not** a Bevy project",
/// which was right for exactly as long as opening one did not create that file.
/// It does: the editor saves window size, graphics quality and the mixer bus
/// graph per project, and the first save writes the synthesized config to disk.
/// So the rule said "Bevy project" on the first import and "not a Bevy project"
/// on every one after it, and the second answer was silent: the loader simply
/// did nothing, which is the worst possible way to be wrong.
///
/// The kind it records is the answer now, and the `Cargo.toml` sniff is only the
/// fallback for a folder that has never been opened. That is also stable in the
/// direction that matters: a Renzora project may legitimately carry a
/// `Cargo.toml` (an `xtask`, a native plugin's source) and it keeps saying
/// `kind = "authored"`, so it can never be mistaken for a game crate.
pub fn is_bevy_project(dir: &Path) -> bool {
    if let Some(kind) = recorded_kind(dir) {
        return kind == ProjectKind::Bevy;
    }
    inspect(dir).is_some()
}

/// The `kind` a folder's `project.toml` records, if it has one.
///
/// Parsed as a bare table rather than as a whole [`ProjectConfig`], because a
/// config this build cannot fully deserialize (one written by a newer editor,
/// say) still has a perfectly readable `kind`, and refusing to read it would
/// send the project down the wrong path entirely.
fn recorded_kind(dir: &Path) -> Option<ProjectKind> {
    let text = std::fs::read_to_string(dir.join("project.toml")).ok()?;
    let table: toml::Table = toml::from_str(&text).ok()?;
    match table.get("kind").and_then(|k| k.as_str()) {
        Some("bevy") => Some(ProjectKind::Bevy),
        // Present and not `bevy`, or absent entirely: either way this folder
        // has been opened as an ordinary project and is one.
        _ => Some(ProjectKind::Authored),
    }
}

/// Read `dir`'s `Cargo.toml` and describe the crate, or `None` if it is not a
/// Bevy crate.
///
/// Follows a virtual workspace down to a member exactly once: a root manifest
/// with `[workspace]` and no `[package]` is not itself a crate, and the member
/// that is the game has to be found before anything can be said about it. Only
/// one level, because a workspace inside a workspace is not a shape cargo
/// allows and chasing it would only add a way to loop.
pub fn inspect(dir: &Path) -> Option<BevyCrate> {
    let manifest = dir.join("Cargo.toml");
    let table: toml::Table = toml::from_str(&std::fs::read_to_string(&manifest).ok()?).ok()?;

    if table.get("package").is_none() {
        return workspace_member(dir, &table);
    }
    read_package(dir, &table)
}

/// Pick the game out of a virtual workspace.
///
/// `[workspace.metadata.renzora] member = "..."` decides it when the workspace
/// root has that section. Otherwise the members that depend on Bevy are
/// considered, and the choice is only made when there is exactly one. A
/// workspace with a game and a headless server in it is a real shape, and
/// picking one of them by directory order would be picking at random.
fn workspace_member(root: &Path, table: &toml::Table) -> Option<BevyCrate> {
    let ws = table.get("workspace")?.as_table()?;

    if let Some(named) = table
        .get("workspace")
        .and_then(|w| w.get("metadata"))
        .and_then(|m| m.get("renzora"))
        .and_then(|r| r.get("member"))
        .and_then(|v| v.as_str())
    {
        return inspect(&root.join(named));
    }

    // Globs are not expanded. `members = ["crates/*"]` is common and a real
    // glob crate is a dependency this crate will not take, so the directory is
    // listed instead and every entry that is a crate is considered. It reaches
    // the same set for the patterns anyone actually writes.
    let mut candidates: Vec<PathBuf> = Vec::new();
    for entry in ws.get("members").and_then(|m| m.as_array()).into_iter().flatten() {
        let Some(pattern) = entry.as_str() else { continue };
        match pattern.strip_suffix("/*").or_else(|| pattern.strip_suffix("\\*")) {
            Some(parent) => {
                if let Ok(entries) = std::fs::read_dir(root.join(parent)) {
                    candidates.extend(entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()));
                }
            }
            None => candidates.push(root.join(pattern)),
        }
    }

    let mut found: Vec<BevyCrate> = candidates.iter().filter_map(|p| inspect(p)).collect();
    if found.len() == 1 {
        return found.pop();
    }
    None
}

/// Describe a `[package]` manifest, or `None` if it does not depend on Bevy.
fn read_package(dir: &Path, table: &toml::Table) -> Option<BevyCrate> {
    let package = table.get("package")?.as_table()?;
    let name = package.get("name")?.as_str()?.to_string();

    let deps: Vec<(String, String)> = table
        .get("dependencies")
        .and_then(|d| d.as_table())
        .map(|t| t.iter().map(|(k, v)| (k.clone(), v.to_string())).collect())
        .unwrap_or_default();

    // The whole gate. No `bevy` entry, not a Bevy project, and the folder falls
    // through to whatever else could open it.
    let bevy = table.get("dependencies").and_then(|d| d.get("bevy"))?;
    let bevy_req = match bevy {
        toml::Value::String(s) => Some(s.clone()),
        toml::Value::Table(t) => t.get("version").and_then(|v| v.as_str()).map(str::to_string),
        _ => None,
    };

    // Cargo's own default when the key is absent, which is 2015 and not 2021.
    // Worth being exact about: a 2015 crate needs `extern crate` and resolves
    // `use` paths differently, so guessing 2021 for it produces errors in code
    // that compiles perfectly well under cargo.
    let edition = package
        .get("edition")
        .and_then(|v| v.as_str())
        .unwrap_or("2015")
        .to_string();

    let metadata = package
        .get("metadata")
        .and_then(|m| m.get("renzora"))
        .and_then(|r| r.as_table())
        .cloned()
        .unwrap_or_default();

    // `[lib] path`, then the conventional file.
    let lib = table
        .get("lib")
        .and_then(|l| l.get("path"))
        .and_then(|p| p.as_str())
        .map(|p| dir.join(p))
        .filter(|p| p.is_file())
        .or_else(|| Some(dir.join("src").join("lib.rs")).filter(|p| p.is_file()));

    // The first `[[bin]]` with a path, then the conventional file. First rather
    // than "the one matching the package name", because a single-binary crate
    // that renames its target is far more common than a multi-binary one, and
    // when there are several the user has to say which anyway.
    let bin = table
        .get("bin")
        .and_then(|b| b.as_array())
        .and_then(|a| a.iter().find_map(|b| b.get("path")).and_then(|p| p.as_str()))
        .map(|p| dir.join(p))
        .filter(|p| p.is_file())
        .or_else(|| Some(dir.join("src").join("main.rs")).filter(|p| p.is_file()));

    // Lib wins. Its modules say `crate::`, and `crate::` has to keep resolving
    // to the library, which it only does if the library's root is the root of
    // what gets compiled.
    let (root, root_is_lib) = match (&lib, &bin) {
        (Some(l), _) => (l.clone(), true),
        (None, Some(b)) => (b.clone(), false),
        (None, None) => return None,
    };

    Some(BevyCrate {
        dir: dir.to_path_buf(),
        package: name,
        edition,
        root,
        root_is_lib,
        bin,
        bevy_req,
        has_build_script: dir.join("build.rs").is_file()
            || package.get("build").and_then(|b| b.as_str()).is_some(),
        dependencies: deps,
        metadata,
    })
}

/// The in-memory [`ProjectConfig`] for a Bevy project folder.
///
/// Nothing is written. A Bevy project keeps exactly the files its author put
/// there, and everything derived (the staged crate root, the built library)
/// goes under `.renzora/`, which is one line in a `.gitignore` and not this
/// function's business either.
pub fn config_for(dir: &Path) -> Option<ProjectConfig> {
    let krate = inspect(dir)?;
    Some(ProjectConfig {
        name: krate.package.clone(),
        kind: ProjectKind::Bevy,
        // Bevy's `AssetPlugin` default. Overridable in `[package.metadata.renzora]`
        // for a project that already sets `file_path` to something else: that
        // setting lives in the `App` construction this never reads, so it has to
        // be declared rather than discovered.
        asset_root: krate.metadata_str("asset_root").unwrap_or_else(|| "assets".to_string()),
        // Empty rather than `scenes/main.bsn`. There is no scene, and a path to
        // one that does not exist is how the editor ends up reporting a missing
        // file for a project that correctly has none.
        main_scene: String::new(),
        ..Default::default()
    })
}

/// One of the project's own components, as the editor knows it.
///
/// Name, and where it was written. The second half is the point: in a Renzora
/// project the relationship is *entity → attached script*, and in a Bevy project
/// it is *entity → component → the file that declares it*. That edge exists, it
/// is just not navigable, and recording the path is what turns a row in the
/// inspector into a way into the code.
#[derive(Debug, Clone)]
pub struct ProjectComponent {
    /// The type's short name, as the author wrote it (`Vehicle`).
    pub name: String,
    /// Absolute path to the file declaring it.
    pub file: PathBuf,
    /// 1-based line of the declaration.
    pub line: u32,
}

/// What a code-first project's own components are called, and where they live.
///
/// Populated by the generated crate root, which registers each of the project's
/// `#[derive(Component)]` types and records the id it was given. Read by the
/// hierarchy's auto-naming (`renzora_engine::named_entities`) to label an entity
/// `Player` rather than `Entity 214`, and by the inspector's **Project
/// Components** section to list what an entity holds and open its source.
///
/// # Why a table and not `ComponentInfo::name()`
///
/// Bevy stores component type names behind its `debug` feature, and this engine
/// has that feature deliberately **off**: it is listed with `bevy_ui_debug` and
/// the `glam_assert`s in the root manifest as one of the things a shipped binary
/// does not carry. With it off, `ComponentInfo::name()` returns the same
/// placeholder string for every component in the process, so reading names back
/// out of the world names the whole hierarchy after the placeholder.
///
/// Generating the table costs nothing at runtime and nothing in a build that has
/// no Bevy project open: the resource stays empty and the labeller falls back to
/// the handful of Bevy component types it knows by name.
#[derive(Resource, Default)]
pub struct ProjectComponentLabels(pub HashMap<ComponentId, ProjectComponent>);

impl ProjectComponentLabels {
    /// Record a component the project declared.
    ///
    /// Called from generated code, which is why it takes the id rather than a
    /// type parameter: the generated root registers the type (so it has a real
    /// `ComponentId` even before any entity carries one) and passes the result
    /// here.
    pub fn insert(&mut self, id: ComponentId, name: &str, file: &str, line: u32) {
        self.0.insert(
            id,
            ProjectComponent {
                name: name.to_string(),
                file: PathBuf::from(file),
                line,
            },
        );
    }

    /// The short name recorded for `id`.
    pub fn get(&self, id: ComponentId) -> Option<&str> {
        self.0.get(&id).map(|c| c.name.as_str())
    }

    /// Everything recorded for `id`, including where it was written.
    pub fn entry(&self, id: ComponentId) -> Option<&ProjectComponent> {
        self.0.get(&id)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// This entity came from the project's own Rust, not from the editor.
///
/// Inserted alongside the generated `Name` by `renzora_engine::named_entities`,
/// which is the one place every code-spawned entity passes through. The point
/// of the distinction is **write-back**: the editor generates Rust for the
/// entities *it* created, and it must not generate Rust for the ones your code
/// already spawns, or every run would build the grove twice.
///
/// # The rule, and where it is wrong
///
/// "Arrived without a `Name`" is the test, because Bevy code does not name
/// entities and every editor spawn path does. A project that names its own
/// entities (`Name::new("Hero")` in a `spawn`) therefore looks editor-authored
/// and would be written into the generated module, spawning a second Hero on
/// the next run.
///
/// It is left as a known edge rather than solved, because solving it means
/// tagging at each of the editor's spawn sites instead of at one place, and the
/// shape of a Bevy project is overwhelmingly to name nothing. [`AuthoredByEditor`]
/// is what the write-back actually keys on, so a better rule can replace this
/// without the writer changing.
#[derive(bevy::prelude::Component, Clone, Copy, Debug, Default)]
pub struct FromProjectCode;

/// This entity was created in the editor and is written back as Rust.
///
/// The positive half of [`FromProjectCode`], and what the code writer queries.
/// Held as its own component rather than inferred at write time so that the
/// classification happens once, at the moment an entity appears, when there is
/// still evidence for it.
#[derive(bevy::prelude::Component, Clone, Copy, Debug, Default)]
pub struct AuthoredByEditor;

/// The Bevy project **this process was launched for**, if any.
///
/// Inserted by `renzora_bevy_project` while the `App` is being built, from the
/// `--project` argument. Read by the launcher to decide whether opening a
/// project means a state transition or a restart: a Bevy project's code can only
/// be installed during `App` assembly, so opening a *different* one from a
/// running editor has to go through [`restart_into`].
///
/// # Why "launched for" and not "successfully loaded"
///
/// It records the intent, not the outcome, and that distinction is load-bearing.
/// If this held only projects whose code compiled, then a project with a build
/// error would look unloaded to the launcher, which would restart into it, where
/// it would fail to build again and look unloaded again. An infinite restart
/// loop, triggered by a syntax error.
#[derive(bevy::prelude::Resource, Debug, Clone, Default)]
pub struct LaunchedBevyProject(pub Option<PathBuf>);

impl LaunchedBevyProject {
    /// Is `root` the project this process is already running for?
    pub fn is(&self, root: &Path) -> bool {
        self.0.as_deref() == Some(root)
    }
}

/// Restart the editor with `project` open.
///
/// A Bevy project's code is installed while the `App` is being built, and there
/// is no `&mut App` once the editor is running, so importing one is a restart
/// rather than a state transition.
///
/// Lives in the contract crate because two unrelated callers need it and neither
/// should have to know how the other spells it: the launcher's **Import Bevy
/// Project** button, and the loader, which offers the same restart when it finds
/// a Bevy project open whose code it never loaded.
///
/// # Why an argument and not an environment variable
///
/// The first version set `RENZORA_BEVY_PROJECT` and let the child inherit it,
/// because [`crate::restart_process`] forwards *this* process's arguments and the
/// project being imported is by definition not among them. It was the wrong
/// trade twice over. An inherited variable is invisible: when the successor came
/// up showing the dashboard instead of the project, there was nothing in the log,
/// the command line or the process list to say what it had been told. And the
/// splash already honours `--project`, so passing one means the successor opens
/// the project by itself rather than landing on the launcher and waiting to be
/// asked a second time.
///
/// So this re-implements the spawn rather than calling `restart_process`: same
/// detached spawn and the same `$APPIMAGE` rule (see that function for why the
/// variable wins over `current_exe`), with the argument list rewritten.
#[cfg(not(target_arch = "wasm32"))]
pub fn restart_into(project: &Path) -> ! {
    let exe = std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .or_else(|| std::env::current_exe().ok());
    if let Some(exe) = exe {
        // Any `--project` this process was given is dropped along with its
        // value: two of them would leave the successor opening whichever one the
        // argument scan happened to see first.
        let mut args: Vec<std::ffi::OsString> = Vec::new();
        let mut rest = std::env::args_os().skip(1);
        while let Some(arg) = rest.next() {
            if arg == "--project" {
                let _ = rest.next();
                continue;
            }
            args.push(arg);
        }
        args.push("--project".into());
        args.push(project.as_os_str().to_os_string());
        let _ = std::process::Command::new(exe).args(args).spawn();
    }
    crate::core::shell::exit_now(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "renzora-bevy-detect-{name}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        dir
    }

    #[test]
    fn a_plain_bin_crate_is_recognised_and_rooted_at_main() {
        let dir = scratch("bin");
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"my-game\"\nedition = \"2024\"\n\n[dependencies]\nbevy = \"0.19.1\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("src").join("main.rs"), "fn main() {}").unwrap();

        let k = inspect(&dir).expect("a bevy bin crate");
        assert_eq!(k.crate_name(), "my_game");
        assert_eq!(k.edition, "2024");
        assert!(!k.root_is_lib);
        assert_eq!(k.root, dir.join("src").join("main.rs"));
        assert_eq!(k.bevy_req.as_deref(), Some("0.19.1"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A lib+bin package roots at the library, because its modules say `crate::`
    /// and that has to keep meaning the library. The binary is still recorded,
    /// since `fn main` is where the `App` is.
    #[test]
    fn a_lib_and_bin_crate_roots_at_the_lib_but_remembers_the_bin() {
        let dir = scratch("libbin");
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"g\"\n\n[dependencies]\nbevy = { version = \"0.19\" }\n",
        )
        .unwrap();
        std::fs::write(dir.join("src").join("lib.rs"), "").unwrap();
        std::fs::write(dir.join("src").join("main.rs"), "fn main() {}").unwrap();

        let k = inspect(&dir).expect("a bevy crate");
        assert!(k.root_is_lib);
        assert_eq!(k.root, dir.join("src").join("lib.rs"));
        assert_eq!(k.bin, Some(dir.join("src").join("main.rs")));
        // No `edition` key means 2015, which is cargo's rule and not 2021.
        assert_eq!(k.edition, "2015");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_crate_without_bevy_is_not_a_bevy_project() {
        let dir = scratch("nobevy");
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"tool\"\n\n[dependencies]\nserde = \"1\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("src").join("main.rs"), "fn main() {}").unwrap();
        assert!(inspect(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The precedence that keeps an existing Renzora project behaving the way it
    /// always has, even though it has a `Cargo.toml` for its own reasons: an
    /// `xtask`, a native plugin's source.
    #[test]
    fn a_project_toml_beats_a_cargo_toml() {
        let dir = scratch("both");
        std::fs::write(dir.join("project.toml"), "name = \"x\"\nversion = \"0.1.0\"\nmain_scene = \"scenes/main.bsn\"\n").unwrap();
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"x\"\n\n[dependencies]\nbevy = \"0.19\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("src").join("main.rs"), "fn main() {}").unwrap();
        assert!(!is_bevy_project(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The regression this rule was rewritten for. Opening a Bevy project writes
    /// a `project.toml` (the editor saves window size, graphics quality and the
    /// mixer bus graph per project) and the previous rule read *any*
    /// `project.toml` as "not a Bevy project". So the first import worked, the
    /// file appeared, and every import after it silently did nothing.
    #[test]
    fn a_saved_bevy_project_is_still_a_bevy_project() {
        let dir = scratch("saved");
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"g\"\n\n[dependencies]\nbevy = \"0.19\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("src").join("main.rs"), "fn main() {}").unwrap();
        assert!(is_bevy_project(&dir), "before the editor has ever saved it");

        // Exactly what the editor writes back: the synthesized config, round
        // tripped through `save_config`.
        let config = config_for(&dir).expect("a config");
        std::fs::write(dir.join("project.toml"), toml::to_string_pretty(&config).unwrap()).unwrap();
        assert!(is_bevy_project(&dir), "after the editor has saved it");

        // And the round trip has to preserve the two fields the rest of the
        // editor keys off, or the guard and the asset root revert on reopen.
        let reread: ProjectConfig =
            toml::from_str(&std::fs::read_to_string(dir.join("project.toml")).unwrap()).unwrap();
        assert_eq!(reread.kind, ProjectKind::Bevy);
        assert_eq!(reread.asset_root, "assets");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// One Bevy member in a virtual workspace is unambiguous, so it is taken.
    #[test]
    fn a_workspace_with_one_bevy_member_resolves_to_it() {
        let dir = scratch("ws");
        std::fs::write(dir.join("Cargo.toml"), "[workspace]\nmembers = [\"game\", \"tool\"]\n")
            .unwrap();
        for (member, dep) in [("game", "bevy = \"0.19\""), ("tool", "serde = \"1\"")] {
            std::fs::create_dir_all(dir.join(member).join("src")).unwrap();
            std::fs::write(
                dir.join(member).join("Cargo.toml"),
                format!("[package]\nname = \"{member}\"\n\n[dependencies]\n{dep}\n"),
            )
            .unwrap();
            std::fs::write(dir.join(member).join("src").join("main.rs"), "fn main() {}").unwrap();
        }
        let k = inspect(&dir).expect("the one bevy member");
        assert_eq!(k.package, "game");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Two of them is a choice this cannot make, and making it by directory
    /// order would be making it at random.
    #[test]
    fn a_workspace_with_two_bevy_members_is_ambiguous() {
        let dir = scratch("ws2");
        std::fs::write(dir.join("Cargo.toml"), "[workspace]\nmembers = [\"a\", \"b\"]\n").unwrap();
        for member in ["a", "b"] {
            std::fs::create_dir_all(dir.join(member).join("src")).unwrap();
            std::fs::write(
                dir.join(member).join("Cargo.toml"),
                format!("[package]\nname = \"{member}\"\n\n[dependencies]\nbevy = \"0.19\"\n"),
            )
            .unwrap();
            std::fs::write(dir.join(member).join("src").join("main.rs"), "fn main() {}").unwrap();
        }
        assert!(inspect(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_synthesized_config_roots_assets_at_bevys_default_and_has_no_scene() {
        let dir = scratch("cfg");
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"grove\"\n\n[dependencies]\nbevy = \"0.19\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("src").join("main.rs"), "fn main() {}").unwrap();
        let cfg = config_for(&dir).expect("a config");
        assert_eq!(cfg.kind, ProjectKind::Bevy);
        assert_eq!(cfg.asset_root, "assets");
        assert!(cfg.main_scene.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
