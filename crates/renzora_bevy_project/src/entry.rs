//! Turning a Bevy crate into something the plugin compiler can build.
//!
//! The output is a **generated crate root** under `<project>/.renzora/bevy/`,
//! and everything about the design follows from one fact: the project's modules
//! say `crate::`, and `crate::` has to keep meaning the project.
//!
//! That rules out the obvious approach. Wrapping the crate (`#[path =
//! "…/src/main.rs"] mod game;` from a root of our own) re-roots it, so every
//! `use crate::common::Aabb` in the project now resolves to *our* root and fails.
//! Compiling the project's own root directly does not work either: `fn main` is
//! there, the plugin types are `pub` inside *private* modules and so unreachable
//! from outside, and there is nowhere to put the exported constructor.
//!
//! So the generated file **is** the crate root. It reproduces the project's root
//! (its `use` statements, its consts, its module list) with every `mod foo;`
//! rewritten to `#[path = "<absolute>"] mod foo;` so the modules are read from
//! where the author keeps them, and `fn main` lifted out and reshaped into
//! `Plugin::build`. `crate::` resolves to exactly the same set of modules it
//! always did, because it is the same set of modules.
//!
//! Nested modules need no help: `#[path]` is only on the top level, and a
//! `mod bar;` inside `src/foo.rs` resolves relative to the path `rustc` loaded
//! `foo` from, which is the original file in the original directory.
//!
//! ```text
//! <project>/
//!   Cargo.toml                 the author's
//!   src/main.rs                the author's
//!   src/level.rs               the author's
//!   .renzora/bevy/
//!     Cargo.toml               generated: the author's deps, minus Bevy
//!     src/lib.rs               generated: the root above
//!     game-<gen>.dll           what rustc produced
//!     stamp.txt                what it was built against
//! ```

use std::path::{Path, PathBuf};

use renzora::core::bevy_project::BevyCrate;

use crate::scan::{self, Masked};

/// A generated root, ready to compile.
#[derive(Debug)]
pub struct Entry {
    /// The staging directory, shaped the way `Sdk::compile` expects
    /// (`<dir>/src/lib.rs` plus a `Cargo.toml`).
    pub dir: PathBuf,
    /// What the user should be told about what was skipped or guessed. Not
    /// errors: a dropped `DefaultPlugins` is correct and still worth saying,
    /// because the alternative is someone wondering for an hour why their window
    /// title did not apply.
    pub notes: Vec<String>,
}

/// Everything that can stop a crate becoming a plugin, with the remedy attached.
///
/// Each of these is a message a user reads, so each one says what to do. A
/// failure here is nearly always a shape this cannot read rather than a mistake
/// the user made, and phrasing it as their mistake would be both rude and wrong.
#[derive(Debug)]
pub enum Error {
    Read(PathBuf, std::io::Error),
    Write(PathBuf, std::io::Error),
    /// No `fn main`, no `[package.metadata.renzora] plugins`, and nothing else
    /// to go on.
    NoEntryPoint { found_plugins: Vec<String> },
    /// `fn main` exists but builds its `App` somewhere this cannot follow.
    IndirectApp,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Read(p, e) => write!(f, "could not read {}: {e}", p.display()),
            Error::Write(p, e) => write!(f, "could not write {}: {e}", p.display()),
            Error::NoEntryPoint { found_plugins } => {
                write!(
                    f,
                    "this crate has no `fn main`, so there is nothing to read the `App` out of. \
                     Name the plugins to add in `Cargo.toml`:\n\
                     \n    [package.metadata.renzora]\n    plugins = [\"my_module::MyPlugin\"]\n"
                )?;
                if !found_plugins.is_empty() {
                    write!(f, "\nPlugins found in this crate: {}", found_plugins.join(", "))?;
                }
                Ok(())
            }
            Error::IndirectApp => write!(
                f,
                "`fn main` does not call `App::new()` directly, so there is no `App` here to \
                 build into. It is probably assembled by a helper function; name the plugins \
                 to add in `Cargo.toml` instead:\n\
                 \n    [package.metadata.renzora]\n    plugins = [\"my_module::MyPlugin\"]\n"
            ),
        }
    }
}

impl std::error::Error for Error {}

/// Where a project's generated crate and its build output live.
///
/// Inside the project, because it is derived from the project and moves with it,
/// and hidden because nobody should be asked to look at it or commit it. Not in
/// the engine's own directory: two projects open in two editors would then be
/// writing the same file.
pub fn staging_dir(project: &Path) -> PathBuf {
    project.join(".renzora").join("bevy")
}

/// Generate the crate root and the manifest beside it.
pub fn stage(krate: &BevyCrate, project: &Path) -> Result<Entry, Error> {
    stage_with(krate, project, Labels::Generate)
}

/// Whether to generate the component-label table.
///
/// It is the one part of the generated root that names the project's *types*
/// rather than copying its text, so it is the one part that can fail to compile
/// on a crate that is otherwise fine: a `#[cfg]`-gated component, a `Component`
/// derived through a macro, a type alias. It buys nothing but nicer names in the
/// hierarchy, so a build that trips over it is retried without it (see
/// [`crate::load`]) rather than reported as a project that will not open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Labels {
    Generate,
    /// Second attempt: names the entities after their Bevy components instead.
    Skip,
}

/// [`stage`], with control over the optional half.
pub fn stage_with(krate: &BevyCrate, project: &Path, labels: Labels) -> Result<Entry, Error> {
    let dir = staging_dir(project);
    std::fs::create_dir_all(dir.join("src")).map_err(|e| Error::Write(dir.clone(), e))?;

    let root_src = read(&krate.root)?;
    let root_mask = Masked::new(&root_src);
    let mut notes = Vec::new();

    // ── The body of `Plugin::build` ─────────────────────────────────────────
    //
    // An explicit list wins over anything read out of source. It is the escape
    // hatch for every shape this cannot parse, and a user who has written one
    // has already been told the parse did not work.
    let declared = krate.metadata_list("plugins");
    let (build_body, main_span) = if !declared.is_empty() {
        notes.push(format!(
            "using the plugin list from Cargo.toml: {}",
            declared.join(", ")
        ));
        (declared_body(&declared), None)
    } else {
        from_main(krate, &root_src, &root_mask, &mut notes)?
    };

    // ── The root itself ─────────────────────────────────────────────────────
    let mut out = root_src.clone();
    let mut edits: Vec<(usize, usize, String)> = Vec::new();

    for decl in scan::modules(&root_mask) {
        let Some(file) = scan::module_file(&krate.root, &decl.name) else {
            // A module with no file is `#[cfg]`-ed out, or the crate is broken.
            // Left exactly as written so `rustc` gives the real diagnostic
            // instead of this guessing at one.
            continue;
        };
        edits.push((
            decl.span.0,
            decl.span.1,
            format!("#[path = \"{}\"] mod {};", rustc_path(&file), decl.name),
        ));
    }

    // `fn main` is removed only when it is in this file. For a lib+bin crate it
    // is in the binary, which is not part of what gets compiled at all.
    if let Some(span) = main_span {
        edits.push((span.0, span.1, String::new()));
    }

    // The write-back module, if the project has one and has not declared it
    // itself. It holds what the user placed in the editor, so the editor has to
    // compile it or a placement would vanish on the next launch, and a project
    // that has added `mod renzora_authored;` to its own root already declares
    // it, so declaring it twice would be a compile error.
    let authored = crate::writeback::module_path(&krate.root);
    let already_declared = scan::modules(&root_mask)
        .iter()
        .any(|m| m.name == crate::writeback::MODULE);
    let authored_module = (authored.is_file() && !already_declared).then(|| {
        format!(
            "
#[path = \"{}\"] mod {};
",
            rustc_path(&authored),
            crate::writeback::MODULE
        )
    });
    let authored_plugin = authored
        .is_file()
        .then(|| format!("{}::{}", crate::writeback::MODULE, crate::writeback::PLUGIN));

    for span in bin_only_attributes(&root_src, &root_mask) {
        // `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` is
        // the near-universal first line of a Bevy `main.rs`, and it is an error
        // on a crate that is not a binary.
        edits.push((span.0, span.1, String::new()));
    }

    edits.sort_by_key(|(start, _, _)| *start);
    for (start, end, replacement) in edits.into_iter().rev() {
        out.replace_range(start..end, &replacement);
    }

    // Collected from the whole module tree, not just the root: a project keeps
    // its components next to the systems that use them, which is anywhere.
    let names = match labels {
        Labels::Generate => collect_components(&krate.root, &root_mask),
        Labels::Skip => Vec::new(),
    };
    // Appended after the rewrite, not before: every edit above is an offset into
    // the original text, and growing the string first would still be safe (the
    // edits all precede the end) but reads as though it might not be.
    if let Some(declaration) = authored_module {
        out.push_str(&declaration);
    }
    let mut build_body = build_body;
    if let Some(plugin) = &authored_plugin {
        // Added last, so the editor's placements land on top of whatever the
        // project's own `Startup` built.
        build_body.push_str(&format!("
        __renzora_app.add_plugins({plugin});
"));
        notes.push(format!(
            "what you place in the editor is written to src/{}.rs and spawned by {plugin}",
            crate::writeback::MODULE
        ));
    }
    out.push_str(&tail(&build_body, &names));
    write(&dir.join("src").join("lib.rs"), &out)?;
    write(&dir.join("Cargo.toml"), &manifest(krate))?;

    if krate.has_build_script {
        notes.push(
            "this crate has a build script, and the plugin compiler runs none, so anything \
             `build.rs` generates will be missing"
                .to_string(),
        );
    }

    Ok(Entry { dir, notes })
}

/// Read the `App` out of `fn main`, wherever `fn main` lives.
///
/// Returns the rewritten body and, when `fn main` is in the crate root itself,
/// the span to cut out of it.
#[allow(clippy::type_complexity)]
fn from_main(
    krate: &BevyCrate,
    root_src: &str,
    root_mask: &Masked,
    notes: &mut Vec<String>,
) -> Result<(String, Option<(usize, usize)>), Error> {
    // The common case: one `main.rs` that is both the crate root and the home of
    // `fn main`.
    if !krate.root_is_lib {
        let Some(main) = scan::main_fn(root_mask) else {
            return Err(Error::NoEntryPoint {
                found_plugins: scan::plugin_impls(root_mask),
            });
        };
        let app = scan::app_construction(root_src, main.body).ok_or(Error::IndirectApp)?;
        note_dropped(&app.dropped, notes);
        return Ok((app.body, Some(main.item)));
    }

    // A lib+bin crate. The root is the library, so `fn main` is in a file that is
    // not being compiled: its body is lifted across, and the paths in it are
    // rewritten, because from inside the library `my_game::GamePlugin` is
    // `crate::GamePlugin`.
    let Some(bin) = krate.bin.as_ref() else {
        return Err(Error::NoEntryPoint {
            found_plugins: scan::plugin_impls(root_mask),
        });
    };
    let bin_src = read(bin)?;
    let bin_mask = Masked::new(&bin_src);
    let Some(main) = scan::main_fn(&bin_mask) else {
        return Err(Error::NoEntryPoint {
            found_plugins: scan::plugin_impls(root_mask),
        });
    };
    let app = scan::app_construction(&bin_src, main.body).ok_or(Error::IndirectApp)?;
    note_dropped(&app.dropped, notes);
    notes.push(format!(
        "this crate is a library and a binary; the `App` was read from {} and paths through \
         `{}::` rewritten to `crate::`",
        bin.file_name().unwrap_or_default().to_string_lossy(),
        krate.crate_name()
    ));
    let body = app.body.replace(&format!("{}::", krate.crate_name()), "crate::");
    Ok((body, None))
}

fn note_dropped(dropped: &[String], notes: &mut Vec<String>) {
    for group in dropped {
        notes.push(format!(
            "{group} was not added: the editor already provides the window, the renderer and \
             the asset server. Anything configured on it with `.set(...)` is not applied"
        ));
    }
}

/// `Plugin::build`'s body for an explicitly declared plugin list.
fn declared_body(plugins: &[String]) -> String {
    // One `add_plugins` per entry rather than one tuple, because
    // `add_plugins` on a tuple is capped at 20 elements and a list of 21 would
    // fail with a trait error that says nothing about the limit.
    plugins
        .iter()
        .map(|p| format!("        __renzora_app.add_plugins({p});\n"))
        .collect()
}

/// Every publicly reachable component in the crate, as a `module::path::Type`.
///
/// Walks the module tree the same way `rustc` will, so a component declared
/// three modules deep is found and named with the path that actually resolves
/// from the crate root. Depth-limited and visited-set guarded, because a `#[path]`
/// pointing back up the tree is a shape Rust allows and an infinite walk is not
/// a failure anyone would enjoy diagnosing.
fn collect_components(root: &Path, root_mask: &Masked) -> Vec<ComponentSite> {
    fn walk(
        file: &Path,
        prefix: &str,
        seen: &mut Vec<PathBuf>,
        out: &mut Vec<ComponentSite>,
        depth: u32,
    ) {
        if depth > 16 || seen.contains(&file.to_path_buf()) {
            return;
        }
        seen.push(file.to_path_buf());
        let Ok(src) = std::fs::read_to_string(file) else {
            return;
        };
        let mask = Masked::new(&src);
        for decl in scan::components(&mask) {
            out.push(ComponentSite {
                path: format!("{prefix}{}", decl.name),
                name: decl.name,
                file: file.to_path_buf(),
                line: decl.line,
                reflect: decl.reflect,
            });
        }
        for decl in scan::modules(&mask) {
            let Some(child) = scan::module_file(file, &decl.name) else {
                continue;
            };
            walk(&child, &format!("{prefix}{}::", decl.name), seen, out, depth + 1);
        }
    }

    let mut out = Vec::new();
    let mut seen = vec![root.to_path_buf()];
    // The root's own components first, then each module's.
    for decl in scan::components(root_mask) {
        out.push(ComponentSite {
            path: decl.name.clone(),
            name: decl.name,
            file: root.to_path_buf(),
            line: decl.line,
            reflect: decl.reflect,
        });
    }
    for decl in scan::modules(root_mask) {
        let Some(child) = scan::module_file(root, &decl.name) else {
            continue;
        };
        walk(&child, &format!("{}::", decl.name), &mut seen, &mut out, 1);
    }
    out
}

/// One of the project's components, as the generated table will record it.
///
/// `path` is how the generated root *names* the type; `file` and `line` are how
/// the inspector *finds* it. The two differ because a module path is not a file
/// path (`game::vehicle::Vehicle` lives in `src/game/vehicle.rs`), and the
/// scanner is the only thing that ever knows both at once.
struct ComponentSite {
    /// `module::path::Type`, resolvable from the crate root.
    path: String,
    /// The type's short name, for display.
    name: String,
    /// Absolute path to the declaring file.
    file: PathBuf,
    /// 1-based line of the declaration.
    line: u32,
    /// Does it derive `Reflect`? Only these are registered with the type
    /// registry, which is what lets the inspector read their fields.
    reflect: bool,
}

/// The generated half of the root, appended after the project's own code.
fn tail(build_body: &str, labels: &[ComponentSite]) -> String {
    // One `register_component` per type, whose id is recorded against the name
    // the author wrote. `try_insert`-shaped on purpose: a component the project
    // declares but never uses still registers cleanly, and registering one twice
    // returns the same id.
    let label_body: String = labels
        .iter()
        .map(|site| {
            let path = &site.path;
            let name = &site.name;
            // The declaring file, as a Rust string literal. Forward slashes for
            // the same reason `#[path]` uses them: a Windows path in a string
            // literal would need its backslashes escaped, and one layer of
            // quoting too many is a path that silently points elsewhere.
            let file = site.file.to_string_lossy().replace('\\', "/");
            let line = site.line;
            // Only a `Reflect` type goes into the type registry, and that is the
            // difference between the inspector showing a row that says `Player`
            // and one the author can actually edit. Emitting it for a type that
            // does not derive `Reflect` is a compile error in code the author
            // cannot see, so the derive list decides, not an assumption.
            let register_type = if site.reflect {
                format!("        app.register_type::<{path}>();\n")
            } else {
                String::new()
            };
            // Two separate borrows of the world rather than one: registering the
            // component and holding the resource at the same time is two
            // mutable borrows of the same `World`.
            format!(
                "    {{\n{register_type}        let id = app.world_mut().register_component::<{path}>();\n        app.world_mut().resource_mut::<__RenzoraLabels>().insert(id, \"{name}\", \"{file}\", {line});\n    }}\n"
            )
        })
        .collect();

    format!(
        r#"

// ── Generated by Renzora ─────────────────────────────────────────────────────
//
// Everything above is the project's own crate root, with its `mod` declarations
// pointed at the files the author keeps them in and `fn main` lifted out. This
// is the part that makes it loadable: one plugin whose `build` is what `main`
// used to do, and the exported constructor the loader looks up by name.
//
// Do not edit. Regenerated from the project every time it is built.

#[doc(hidden)]
trait __RenzoraNoRun {{
    /// Stands in for `App::run()`.
    ///
    /// The editor is already running; the game's `App` calls are being replayed
    /// into it. A no-op rather than a deletion because a `main` returning
    /// `AppExit` ends in `.run()` as a tail expression, and deleting that leaves
    /// an expression where a statement belongs.
    fn __renzora_no_run(&mut self) -> &mut Self {{
        self
    }}

    /// Stands in for an `add_plugins` call that only added `DefaultPlugins`.
    ///
    /// A no-op rather than a deletion, because the call may be a whole
    /// statement: deleting `app.add_plugins(DefaultPlugins);` leaves `app;`,
    /// which moves the `&mut App` and breaks every line after it.
    fn __renzora_no_plugins(&mut self) -> &mut Self {{
        self
    }}
}}

#[doc(hidden)]
impl __RenzoraNoRun for ::bevy::app::App {{}}

#[doc(hidden)]
type __RenzoraLabels = ::renzora::core::bevy_project::ProjectComponentLabels;

/// Tell the editor what this project's components are called.
///
/// Bevy keeps component type names behind its `debug` feature and the engine
/// builds without it, so `ComponentInfo::name()` is the same placeholder for
/// everything. Registering each type here records a real `ComponentId` against
/// the name the author wrote, which is what lets the hierarchy say `Player`.
///
/// It also catches the project up on the type registry.
///
/// `App::new()` already calls `AppTypeRegistry::new_with_derived_types()`, which
/// registers every `#[derive(Reflect)]` type linked into the process. That ran
/// before this library was loaded, so none of the project's types were in the
/// image yet and none of them are in the registry. Registering the derived types
/// a second time picks up everything this image brought with it: components,
/// **resources**, events and reflected assets, including the ones no scanner
/// could name because they are `#[cfg]`-gated, macro-generated or private.
///
/// That is what puts a project's `Reflect` types in the inspector and its
/// resources in the Resources panel, from one derive and no registration call.
/// The explicit `register_type` calls below stay for now as the guaranteed path
/// for components: auto-registration collects through `inventory`, whose
/// behaviour across a `dlopen`ed image is the one thing here that has not been
/// verified on this platform, and a redundant registration is a no-op.
///
/// Cosmetic, and treated as such: if this does not compile the project is
/// rebuilt without it rather than failing to open.
#[doc(hidden)]
fn __renzora_component_labels(app: &mut ::bevy::app::App) {{
    if let Some(registry) = app
        .world()
        .get_resource::<::bevy::ecs::reflect::AppTypeRegistry>()
    {{
        registry.write().register_derived_types();
    }}
    app.init_resource::<__RenzoraLabels>();
{label_body}}}

#[doc(hidden)]
pub struct __RenzoraGamePlugin;

impl ::bevy::app::Plugin for __RenzoraGamePlugin {{
    fn build(&self, __renzora_app: &mut ::bevy::app::App) {{
        __renzora_component_labels(__renzora_app);
{build_body}
    }}
}}

::renzora::plugin!(__RenzoraGamePlugin, Runtime);
"#
    )
}

/// The manifest the generated crate compiles against.
///
/// Two jobs, and only the second is obvious. Bevy's derive macros read the
/// manifest at `CARGO_MANIFEST_DIR` to decide whether to emit `bevy::ecs` or
/// `bevy_ecs` paths, so a crate without one fails inside `#[derive(Component)]`
/// with a message about a missing file. And the `[dependencies]` table is what
/// `renzora_native_build::deps` builds with cargo, so the project's real
/// third-party crates are forwarded here verbatim and resolved exactly as cargo
/// would have resolved them.
///
/// `bevy` is left in deliberately: `deps` strips it, and stripping it here as
/// well would put the rule in two places. What it must NOT be is absent, or
/// `BevyManifest` resolves to the subcrates and every derive emits paths the
/// facade does not have.
fn manifest(krate: &BevyCrate) -> String {
    let mut out = String::new();
    out.push_str(
        "# Generated by Renzora from the project's own Cargo.toml. Do not edit.\n\
         #\n\
         # Nothing runs cargo against this crate: it is compiled by `rustc` directly,\n\
         # against the SDK staged beside the editor. The manifest is here so Bevy's\n\
         # derive macros can resolve their own crate paths, and so the project's\n\
         # third-party dependencies can be built (see `renzora_native_build::deps`,\n\
         # which strips `bevy` and `renzora*` before handing this to cargo).\n\n",
    );
    out.push_str("[package]\n");
    out.push_str(&format!("name = \"{}\"\n", krate.package));
    out.push_str("version = \"0.1.0\"\n");
    out.push_str(&format!("edition = \"{}\"\n", krate.edition));
    out.push_str("\n[lib]\ncrate-type = [\"dylib\"]\n");
    out.push_str("\n[dependencies]\n");
    for (name, value) in &krate.dependencies {
        out.push_str(&format!("{name} = {value}\n"));
    }
    // The contract crate is never in the project's manifest, because the project
    // has never heard of it, but the generated tail calls `renzora::plugin!`.
    if !krate.dependencies.iter().any(|(n, _)| n == "renzora") {
        out.push_str("renzora = \"*\"\n");
    }
    out
}

/// Inner attributes that are errors on anything but a binary.
///
/// `windows_subsystem` is the one that matters, because it is on the first line
/// of almost every Bevy `main.rs` that has ever shipped: without it a release
/// build opens a console window behind the game. On a `dylib` it is
/// `error: `windows_subsystem` attribute is only valid on a binary`, which would
/// be the first thing a user saw and would tell them nothing about why.
fn bin_only_attributes(src: &str, mask: &Masked) -> Vec<(usize, usize)> {
    const BIN_ONLY: &[&str] = &["windows_subsystem", "no_main"];
    let text = mask.as_str();
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(rel) = text[from..].find("#!") {
        let at = from + rel;
        from = at + 2;
        if mask.depth_at(at) != 0 {
            continue;
        }
        let Some(open) = text[at..].find('[').map(|o| at + o) else {
            continue;
        };
        let Some(end) = mask.match_delim(open) else { continue };
        if BIN_ONLY.iter().any(|a| src[open..end].contains(a)) {
            out.push((at, end));
        }
        from = end;
    }
    out
}

/// A path as `#[path = "…"]` should spell it.
///
/// Forward slashes even on Windows. `rustc` accepts them there, and the
/// alternative is escaping backslashes inside a string literal that is itself
/// being written into generated source: one layer of quoting too many, and the
/// failure is a path that silently points somewhere else.
fn rustc_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn read(path: &Path) -> Result<String, Error> {
    std::fs::read_to_string(path).map_err(|e| Error::Read(path.to_path_buf(), e))
}

fn write(path: &Path, contents: &str) -> Result<(), Error> {
    std::fs::write(path, contents).map_err(|e| Error::Write(path.to_path_buf(), e))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "renzora-bevy-entry-{name}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        dir
    }

    fn write_project(dir: &Path, main: &str, modules: &[(&str, &str)]) {
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"demo-game\"\nedition = \"2024\"\n\n[dependencies]\nbevy = \"0.19.1\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("src").join("main.rs"), main).unwrap();
        for (name, body) in modules {
            std::fs::write(dir.join("src").join(format!("{name}.rs")), body).unwrap();
        }
    }

    /// The shape of the project this was built against: bin crate, seven
    /// modules, one chain in `main`.
    #[test]
    fn a_plain_bin_crate_stages_a_root_that_keeps_its_modules() {
        let dir = scratch("bin");
        write_project(
            &dir,
            "#![cfg_attr(not(debug_assertions), windows_subsystem = \"windows\")]\n\
             mod level;\n\
             use bevy::prelude::*;\n\
             fn main() {\n    App::new().add_plugins(DefaultPlugins).add_plugins(level::LevelPlugin).run();\n}\n",
            &[("level", "pub struct LevelPlugin;")],
        );
        let krate = renzora::core::bevy_project::inspect(&dir).expect("a bevy crate");
        let entry = stage(&krate, &dir).expect("staged");

        let root = std::fs::read_to_string(entry.dir.join("src").join("lib.rs")).unwrap();
        // The module is read from where the author keeps it.
        assert!(root.contains("#[path = \""));
        assert!(root.contains("/src/level.rs\"] mod level;"));
        // `fn main` is gone, its body is in the plugin.
        assert!(!root.contains("fn main()"));
        assert!(root.contains("__renzora_app"));
        assert!(root.contains("level::LevelPlugin"));
        // The binary-only attribute would be a hard error on a dylib.
        assert!(!root.contains("windows_subsystem"));
        // The window and renderer belong to the editor.
        //
        // Asserted against **masked** source, because the generated tail's own
        // doc comments quote `app.add_plugins(DefaultPlugins);` while explaining
        // why that call is neutered, and a test that reads prose as code is a
        // test that fails for the wrong reason. `Masked` blanks comments, which
        // is the whole point of it.
        let code = Masked::new(&root);
        assert!(!code.as_str().contains("DefaultPlugins"));
        assert!(code.as_str().contains("__renzora_no_plugins()"));
        assert!(root.contains("renzora_native_plugin_ctor") || root.contains("plugin!"));

        // The manifest carries the project's edition, or a 2024 crate is
        // compiled as 2021 and fails on syntax the author is entitled to use.
        let manifest = std::fs::read_to_string(entry.dir.join("Cargo.toml")).unwrap();
        assert!(manifest.contains("edition = \"2024\""));
        assert!(manifest.contains("bevy = \"0.19.1\""));
        assert!(manifest.contains("renzora = \"*\""));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An explicit list is the escape hatch, so it has to win over whatever
    /// `fn main` happens to say.
    #[test]
    fn a_declared_plugin_list_overrides_the_parse() {
        let dir = scratch("declared");
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"g\"\nedition = \"2021\"\n\n\
             [package.metadata.renzora]\nplugins = [\"level::LevelPlugin\"]\n\n\
             [dependencies]\nbevy = \"0.19\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("src").join("main.rs"), "mod level;\nfn main() { build().run(); }\n")
            .unwrap();
        std::fs::write(dir.join("src").join("level.rs"), "pub struct LevelPlugin;").unwrap();

        let krate = renzora::core::bevy_project::inspect(&dir).expect("a bevy crate");
        let entry = stage(&krate, &dir).expect("staged");
        let root = std::fs::read_to_string(entry.dir.join("src").join("lib.rs")).unwrap();
        assert!(root.contains("__renzora_app.add_plugins(level::LevelPlugin);"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The failure that must not be silent: no `App::new()` to redirect, so the
    /// rewrite would have configured a throwaway `App`.
    #[test]
    fn an_indirect_app_is_refused_with_the_escape_hatch_in_the_message() {
        let dir = scratch("indirect");
        write_project(&dir, "fn main() { build_app().run(); }\n", &[]);
        let krate = renzora::core::bevy_project::inspect(&dir).expect("a bevy crate");
        let err = stage(&krate, &dir).expect_err("cannot read this App");
        assert!(matches!(err, Error::IndirectApp));
        assert!(err.to_string().contains("[package.metadata.renzora]"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A `mod` whose file does not exist is left alone rather than rewritten to
    /// a path that does not exist either: `rustc`'s own diagnostic is better
    /// than anything invented here.
    #[test]
    fn a_module_with_no_file_is_left_for_rustc_to_report() {
        let dir = scratch("nofile");
        write_project(&dir, "mod missing;\nfn main() { App::new().run(); }\n", &[]);
        let krate = renzora::core::bevy_project::inspect(&dir).expect("a bevy crate");
        let entry = stage(&krate, &dir).expect("staged");
        let root = std::fs::read_to_string(entry.dir.join("src").join("lib.rs")).unwrap();
        assert!(root.contains("mod missing;"));
        assert!(!root.contains("#[path"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
