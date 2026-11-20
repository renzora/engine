# Cargo projects: plan

Status: planned, nothing built yet. A prototype of the loading half existed in September 2026 (`renzora_bevy_project`) and is no longer in the tree; what it taught is folded in below.

## The idea

A Renzora project today is a folder of scenes, assets and scripts. The engine is a prebuilt binary, and your code is scripts and plugins the editor compiles against the SDK. Nobody has to write a `main`, open a window or pick a scene format.

A **cargo project** is the same folder plus one Rust crate that you own. It gives a Bevy programmer what they are used to (their IDE, `cargo run`, `cargo test`, git, ordinary crates) without giving up anything the editor does:

- The whole game runs in the editor's viewport, its own systems and rendering included, because the editor loads the crate into its own `World`.
- Every engine feature (terrain, clouds, physics, particles, materials, UI, Solari) comes from one plugin, `RenzoraPlugin`, and each can be switched off in `Cargo.toml`.
- `cargo run` runs the game outside the editor, and `cargo build --release` produces what a lean export produces.

The two project kinds share the engine, the editor, the scene format and every asset type. They differ only in who holds the code.

| | Renzora project | Cargo project |
|---|---|---|
| Your code | `.rs` scripts and plugins in the project | one crate with `Cargo.toml` |
| Who owns `main` | the engine binary | you |
| Needs Rust installed | only for scripts and plugins | yes |
| Runs outside the editor | the runtime, or an export | `cargo run` |
| Engine features | all, stripped at export | chosen in `Cargo.toml` |
| Scenes, assets, editor, viewport | same | same |

## Why this is mostly done already

Three pieces exist and are reused unchanged in spirit:

1. **The runtime is one call.** `renzora_runtime::add_engine_plugins(app, is_editor)` adds every engine plugin, and it is what the engine's own game binary calls. `RenzoraPlugin` is that, wrapped.
2. **Every subsystem is a Cargo feature.** `renzora_runtime`'s `default` list holds about 45 of them, all on. The lean export turns off the ones a game does not use, worked out by `renzora_export::capabilities` from the scenes' type paths and the scripts' API names.
3. **The lean export already builds a cargo project.** It copies the engine source into `target/export-src/`, writes the project's scripts and plugins into a generated `renzora_static_plugins` crate, and runs cargo. A cargo project is that hidden crate made permanent and handed to the user.

On top of those, the native plugin path already compiles a crate with its own `Cargo.toml` and plain third-party dependencies against the SDK (`renzora_native_build::deps`), and hot-reloads Rust scripts. Loading a whole game crate into the editor is the same operation at a larger size.

## What a new cargo project contains

The splash's New Project gains a choice between a Renzora project and a cargo project. A cargo project is an ordinary Renzora project folder plus a crate, so every existing tool (asset browser, scene loading, export) finds what it expects:

```
my_game/
  project.toml        # as today
  scenes/main.bsn     # as today
  assets/ ...         # as today
  Cargo.toml          # new
  src/lib.rs          # the game: GamePlugin
  src/main.rs         # five lines, for cargo run
```

`src/lib.rs` holds the game and exports it the way a native plugin does:

```rust
use bevy::prelude::*;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, spin);
    }
}

#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct Spin { pub speed: f32 }

fn spin(time: Res<Time>, mut q: Query<(&Spin, &mut Transform)>) {
    for (spin, mut t) in &mut q {
        t.rotate_y(spin.speed * time.delta_secs());
    }
}

renzora::plugin!(GamePlugin);
```

`src/main.rs` is only for running outside the editor:

```rust
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(renzora_game::RenzoraPlugin::default())
        .add_plugins(my_game::GamePlugin)
        .run();
}
```

### Why the game lives in `lib.rs`, not `main`

The prototype loaded a crate whose game was written inside `fn main`. It had to generate a new crate root, rewrite every top-level `mod foo;` into `#[path = "..."] mod foo;` (wrapping the crate instead re-roots `crate::` and breaks every `use crate::...`), lift `main`'s body into a `Plugin::build`, drop `DefaultPlugins` and turn `.run()` into nothing. It worked on a real 1800-line project, but it is a source rewrite that any unusual `main` defeats.

A template that puts the game in a `GamePlugin` needs none of it. The editor compiles `lib.rs` as a dylib, exactly like a native plugin, and `main.rs` is never involved. The rewrite stays useful later, for opening an existing Bevy project that was not made from the template (phase 6).

## How the editor runs it

Opening a cargo project:

1. The editor sees `Cargo.toml` beside `project.toml`.
2. It compiles `src/lib.rs` as a `dylib` against the SDK, through `renzora_plugin_build`, the path native plugins take. Plain dependencies in the crate's `Cargo.toml` are built the way a plugin's are.
3. It loads the library and adds `GamePlugin` to the editor's `App`. The game's systems join the editor's `World`, so the viewport shows the real game, and every `Reflect` component appears in Add Component.

**Loading happens at boot.** A plugin is added in `Plugin::build`, and there is no `&mut App` after `run()`. Opening a cargo project from the splash therefore restarts into it, as the prototype did, the same way installing a plugin does.

**Hot reload on save.** Saving a `.rs` file in the crate rebuilds the library off the main thread and loads the new image. As with scripts, the old image is never unloaded (`FreeLibrary` has deadlocked here before), so each reload leaks one image until a restart.

Hot reload has one real limit, and the editor should say so rather than misbehave. Rust scripts get away with swapping an image because a script only owns a function pointer. A game crate adds systems, components and resources to the `World`, and Bevy cannot remove a system from a schedule. The first version therefore reloads by **restarting the editor into the new build**, with the open scene saved and reopened, and a true in-place reload is later work (see open questions).

**Editing code inside the editor.** The Assets panel already shows the project folder, so `src/` and `Cargo.toml` appear beside the scenes and models; `target/` and `.renzora/` are hidden. A `.rs` file opens in the code editor, compile errors land in the Problems panel with line numbers (as script errors do today), and a save from an outside IDE is noticed by the project watcher the same way. Inspector edits to a game component's fields are data and apply at once; a code edit goes through the rebuild described above.

**Game systems and edit mode.** A game's `Update` systems would run while you are arranging the scene, which is wrong for gameplay (a spawner would spawn) and right for some things (a procedural mesh should show). The same question already has an answer for scripts: they run in Play, or per script with its play toggle. The proposal is that `GamePlugin`'s systems run only in Play by default, and that a system opts into edit mode with a run condition the template documents.

## Where Bevy and Renzora come from

This is the one real design decision.

The engine is built on the in-tree Bevy (Bevy `main`), not a crates.io release, and two Bevys in one process corrupt the `World`. So a cargo project must use the engine's Bevy, never `bevy = "0.x"` from crates.io.

**In the editor** this is automatic: the SDK supplies Bevy and the engine crates, and the plugin build links against them.

**For `cargo run` outside the editor** the crate needs the engine from somewhere cargo understands. Two ways:

1. **From source (first version).** `Cargo.toml` depends on the engine by git, pinned to the exact commit the editor was built from, and gets Bevy through it. It works with plain cargo and nothing extra, but the first build compiles all of Bevy and Renzora: many minutes and several GB in the project's `target/`. A downloaded editor already fetches the matching engine source for lean exports (`renzora_export::download`), so the commit is known.
2. **Against the prebuilt SDK (later).** The crate links the libraries already compiled beside the editor and builds in seconds. Plain cargo does not link prebuilt rlibs, so this needs a driver: either a `renzora run` command that compiles the crate the way the plugin build does, or a `RUSTC_WRAPPER` that substitutes the SDK's artefacts for the engine's crates. A wrapper like that was tried for plugins and removed on 2026-10-24 (it broke plugins that use plain crates and made SDK staging walk cargo's fingerprints); if it comes back it must be narrower than that one.

The editor never needs either: it always builds through the SDK. Only standalone `cargo run` pays for the choice.

### Third-party crates

Ordinary Rust crates (serde, rand, noise) work as normal. A crate built on Bevy (avian, bevy_egui) would bring a second Bevy, so it cannot come from crates.io. It either comes from the engine (avian already does, behind the `avian` feature) or is vendored into the engine first. `renzora_native_build::deps::reject_engine_crates` already catches this for plugins and names the crate, and the same check applies here.

## `RenzoraPlugin`

A thin plugin in a new facade crate, so the user depends on one crate and sees one feature list:

```rust
App::new().add_plugins(renzora_game::RenzoraPlugin::default())
```

It does what the engine binary's startup does today, minus the parts that assume it is that binary:

- opens the window and adds `DefaultPlugins` with the engine's settings;
- calls `add_engine_plugins(app, false)`;
- finds the project: `CARGO_MANIFEST_DIR` under `cargo run`, beside the executable in a release build, overridable in the builder;
- loads the main scene from `project.toml`.

`RuntimePlugin` currently reads its project from beside the executable and sets up the window itself; those assumptions have to become options rather than facts. That refactor is the main engine-side work.

The facade needs a name that is not `renzora`: that crate name is already the contract crate in the workspace and the CLI on crates.io. `renzora_game` is used above as a placeholder.

## Choosing engine features

The facade forwards the runtime's features. A new project lists all of them, so everything works on day one, and removing one is an edit:

```toml
[dependencies]
renzora_game = { git = "...", rev = "...", default-features = false, features = [
    "render_3d", "physics_3d", "audio", "terrain", "clouds",
] }
```

Remove `"clouds"`, rebuild, and clouds are not in the game.

- **In the editor all features stay on.** The editor needs every tool, and it builds through the SDK, which is compiled with everything. Unused features cost nothing per frame.
- **A scene that uses a feature the crate turned off** must be caught before it fails at load. The editor already runs the capability scan for exports; for a cargo project it compares the result with the crate's features and puts a Problem on the scene ("uses Clouds, but the `clouds` feature is off in Cargo.toml"), with a fix that adds the feature back.
- **The editor may offer to keep the list in step**, adding a feature when a scene starts using it, but never removes one on its own: a feature the scenes do not use may still be used from code.

## Running the game with the editor around it

The engine binary already becomes the editor when `renzora_editor.<dll|so|dylib>` sits beside it (`renzora_runtime::editor_image`). `RenzoraPlugin` can make the same check, so `cargo run` with the editor image available (behind a flag or a feature) opens the game inside the full editor, and without it runs the plain game. This is optional and comes late; opening the crate from the editor is the main path.

A release build never contains the editor. It is a separate image that is not there, as for every export today.

## Export

Export of a cargo project builds the user's crate instead of the generated `renzora_static_plugins` crate:

- the same isolated copy in `target/export-src/`, so the user's own `target/` is untouched;
- `--release`, with the crate's own features;
- then everything export already does around the binary: packing assets and scenes, the icon, platform bundles, and cross-compiling through the Docker images.

`cargo build --release` on its own gives the binary; Export gives the shippable package.

## When Renzora updates

A cargo project names an engine version in two places, and they move differently.

**In the editor, the project is always on the editor's version.** The editor never uses the crate's pin: it compiles the crate against its own SDK, every time. After an update the SDK's content stamp changes, the crate's recorded stamp no longer matches, and it rebuilds against the new engine on the next open, which is exactly what installed plugins do today. There is no state in which the editor runs a game built for another version.

If the update changed an API the game uses, that rebuild fails with ordinary compile errors in the user's own files, shown in the Problems panel with line numbers. That is the same experience as moving any Bevy project to a new Bevy, and it is the honest cost of owning the code. Two things keep it small:

- **Pin to releases, not nightlies.** The template pins a release tag (`r1-alpha8`), and the editor that opens it is normally that release. A nightly-to-nightly move is for people who chose nightlies.
- **Say what changed.** The release notes already list every change; the update notice in the editor links the ones that touched the API.

**Outside the editor, `cargo run` uses the crate's pin**, the git revision in `Cargo.toml`. After an editor update that pin is behind, and a standalone build would quietly use the old engine while the editor uses the new one. So on open the editor compares the pin with its own version (the same comparison it makes for `project.toml`'s `created_with`) and, when they differ, offers to move the pin. Accepting is a one-line edit to `Cargo.toml`, shown before it is written, and it is the only edit the editor makes to that file besides features.

**An older editor opening a newer project** (a teammate who has not updated) refuses to build it rather than trying: the pin is ahead of the SDK, and the crate may use APIs that do not exist yet. It says which version the project needs.

**Scenes are already covered.** A scene saved by an older version loads in a newer one: a new component field needs `#[reflect(default)]`, and a renamed type needs an alias, both of which the engine already requires of itself. A project's own components follow the same rules, and the template's comments say so.

## Scripting in a cargo project

All of a cargo project's code is in the crate, so there are no separate script files to drop on an entity. Attaching behaviour still works, in two ways, and both are supported.

**Components and systems (the Bevy way, primary).** A Bevy programmer attaches components, not scripts, and behaviour is a system that queries for them:

```rust
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct Spin { pub speed: f32 }

fn spin(time: Res<Time>, mut q: Query<(&Spin, &mut Transform)>) { /* ... */ }
```

`Spin` appears in Add Component and its fields in the inspector, because the game is loaded into the editor. Dropping a source file such as `src/spin.rs` on an entity adds the components that file defines; the editor maps a type to its file through its module path (`my_game::spin::Spin`). The gesture is the one Renzora projects use, and what it attaches is a component.

**Named script functions (the Renzora way).** The per-entity `fn(&mut ScriptCtx)` that `.rs` scripts use, registered by name from `GamePlugin`:

```rust
app.add_script("door", door::update);
app.add_script_hooks("door", door::hooks);
```

They appear in the Scripts component's picker by name and attach exactly as script files do today, with the same hooks and the same per-script play toggle. A script from a Renzora project moves over by copying its file into `src/` and replacing its `renzora::script!(...)` line with the registration. `ScriptComponent` then refers to a script by name or by file, and dispatch looks the name up in the table the crate filled.

There is no separate scripting API for Rust in either kind of project. A `.rs` script already has the whole `&mut World`, and `ScriptCtx` is a convenience over it rather than a sandbox. In a cargo project, Bevy's own API is the main surface, and Renzora's types (terrain, clouds, physics components, `ScriptCtx`) are used where the engine's features are wanted.

**Not supported:** standalone `.rs` script files beside the crate. Two places for code, built two different ways, would be harder to explain than either one.

Language backends from the marketplace (`.lua` and others) are data the engine loads, not Rust, so they work in a cargo project unchanged.

## Phases

1. **Template and editor loading.** New Project choice, the template, compiling `lib.rs` through the plugin build, loading `GamePlugin` at boot, restart-to-reload on save, game systems gated to Play.
2. **`RenzoraPlugin` and standalone `cargo run`**, with the engine as a pinned git dependency. Includes the `RuntimePlugin` refactor.
3. **Features**: the facade's feature list and the missing-feature Problem.
4. **Export** of a cargo project.
5. **`cargo run` with the editor image**, optional.
6. **Open an existing Bevy project** that was not made from the template, using the prototype's crate-root rewrite.
7. **Fast standalone builds** against the prebuilt SDK, if the from-source build proves too slow in practice.

Docs pages for users (getting started with a cargo project, choosing features) ship with phases 1 to 3.

## Open questions

- **In-place hot reload.** Restarting is safe but slow. An in-place reload needs every system the old image added to stop running (a retired system stays in the schedule, so it must be gated), and every component type to keep its `TypeId` across builds, which holds only if the type's crate identity does not change between builds. Worth measuring once phase 1 exists.
- **Edit-mode systems.** Whether "Play only by default" is right, and what the opt-in looks like.
- **The facade crate's name** and whether it should ever be published. It cannot go on crates.io while the engine is on Bevy `main`.
- **Workspaces.** A user crate inside a cargo workspace with other crates: the editor builds one crate today, and a workspace means building its path dependencies too.
