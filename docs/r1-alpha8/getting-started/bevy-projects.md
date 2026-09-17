# Opening a Bevy Project

You do not have to build your game in the editor to use the editor. Point
Renzora at a crate you already wrote against [Bevy](https://bevyengine.org) and
it compiles it, loads it, and shows you the world your code builds: hierarchy,
inspector, gizmos and viewport, over entities that came out of your own
`Startup` systems.

Your project does not change. It keeps its `main.rs`, it keeps `cargo run`, it
keeps its `assets/` folder, and nothing is written into it except a hidden
`.renzora/` build directory.

## Starting one

On the dashboard, **New Bevy Project**. Pick an empty folder and it writes a
cargo crate you can play immediately: a camera, a light, a ground plane, a cube
you drive with WASD and a few spinning pickups.

```
my-game/
├── Cargo.toml     bevy 0.19, and the dev-profile split that makes it run
├── src/main.rs    the whole game, one file, no modules
├── assets/
└── .gitignore     /target and /.renzora
```

It is a normal Bevy project with no dependency on Renzora, so `cargo run` in
that folder plays it with no editor involved. Its components derive `Reflect`,
which is not required but means the inspector can edit them the moment you open
it, and shows you the pattern to copy for your own.

An existing crate is never written over: a folder that already holds a
`Cargo.toml` is refused, and you want **Import Bevy Project** for that one.

## Importing one

On the dashboard, **Import Bevy Project**. In the editor, **File > Import Bevy
Project...**. Both ask for the folder that holds your `Cargo.toml`, and both
restart the editor into it. See [Opening a different
project](#opening-a-different-project) for why that is a restart and not a
transition.

**Open Project** works too. If the folder you pick turns out to be a Bevy crate
it is routed to the same place rather than refused.

From a terminal, the same thing:

```sh
renzora --project path/to/my-game
```

> **Requires Bevy 0.19.** The engine and your crate share one compiled Bevy, and
> that is the entire reason your `Transform` and the editor's are the same type.
> A different minor version cannot be made to work.

## Editing in your own IDE

The editor watches your crate. Save a `.rs` file or `Cargo.toml` anywhere under
the project root, in whatever editor you like, and it rebuilds in the
background: the editor keeps running at full speed while `rustc` works.

What happens next depends on the result.

- **It did not compile.** The errors land in the Console and the Problems panel
  within a second or two, while you are still looking at the code that caused
  them. This is most of the value, because most saves produce an error rather
  than a build you want to look at.
- **It compiled.** The viewport is swapped over to the new code. The editor is
  not restarted: your panel layout, viewport camera, open tabs and selection all
  stay where they were.

A build that fails is not retried until you edit again, so one mistake reports
once rather than scrolling. `target/`, `.renzora/` and dot-directories are not
watched, since those are output rather than source.

### What a reload does

It rebuilds your world rather than patching it:

1. the new library is loaded and its `Startup` runs, building the new world;
2. **then** the previous generation's entities are despawned.

That order matters on a project with real assets. Despawning first would drop the
last handle to a mesh or texture, Bevy would free it, and the new startup would
read all of it off disk again. Building first means both generations hold those
handles for a moment, so every load is a cache hit and nothing is re-read.

Clearing the old entities is correctness, not tidiness: add a field to a
component and the new code reads it with a layout the old entities do not have.

### The cost

Each reload loads a library that can never be unloaded, because Bevy keeps a
`drop` function pointer for every component type and never unregisters one.

That library is compiled **code**, not content, so it tracks the size of your
project rather than your assets: about 537 KB for the scaffold, and 4 MB for a
5,500-line project carrying 93 MB of models. A session's worth is tens of
megabytes, reclaimed when you restart the editor, which also prunes the files
left behind in `.renzora/`.

**File ▸ Reload Project Code** forces one by hand if you need it.

## The viewport does not run your game

Your `Startup` runs, so the world your code builds is there to look at and edit.
Your `Update` systems do not.

That is deliberate. A viewport that runs your game is a viewport where pressing
Play changes the thing you are editing, and pressing Stop leaves the debris
behind. Holding `Update` back means the world in front of you only changes when
you change it.

To actually play, press Play.

This applies to every system your project registers, including ones a plugin
registers from `Plugin::finish` rather than `Plugin::build`.

**Your plugins are yours.** They are added to a shell app holding the editor's
world, so the editor never records them as its own. Two things follow: you may
add a plugin the editor already has (`FrameTimeDiagnosticsPlugin` is the usual
one) without the duplicate-plugin panic Bevy would otherwise raise, and the
editor's own `is_plugin_added` checks are not answered by your project. That
second one is why the FPS readout stopped reading zero when a project brought
its own diagnostics.

## What makes a folder a Bevy project

A `Cargo.toml` with a `bevy` dependency, and no `project.toml`. That is the whole
test. A folder with a `project.toml` is a normal Renzora project and is opened
the way it always was, even if it also has a `Cargo.toml`.

Nothing is written to your repository. The editor synthesizes the project config
in memory: the name comes from your package, assets root at `assets/`, and there
is no main scene because there is no scene.

## What the editor does to your crate

It generates a crate root under `.renzora/bevy/` that *is* your crate: your
module declarations pointed at your files, your `use` statements, your
crate-level items, and your `fn main` reshaped into a Bevy `Plugin`. Then it
compiles that with the same compiler and the same SDK a native plugin uses, and
loads it.

`fn main` is read, not guessed at. Given the ordinary shape:

```rust
fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin { /* ... */ }))
        .add_plugins((CorePlugin, PlayerPlugin, HudPlugin))
        .insert_resource(Score(0))
        .run();
}
```

everything survives except two things:

- **`App::new()` becomes the editor's `App`.** Every builder call you wrote runs
  against it, in the order you wrote them.
- **`DefaultPlugins` is dropped**, along with anything configured on it. The
  editor is already a Bevy app with a window, a renderer and an asset server;
  adding a second `WindowPlugin` panics. Your window title, resolution and
  `ImagePlugin` settings are not applied: the editor's window is the editor's.

A `let mut app = App::new();` binding works the same way, and so does a `main`
returning `AppExit`.

## Assets

Bevy's `AssetPlugin` roots at `assets/`, so `asset_server.load("models/x.glb")`
means `<project>/assets/models/x.glb`. The editor uses that root for a Bevy
project, and the project root for a Renzora one. To point somewhere else:

```toml
[package.metadata.renzora]
asset_root = "content"
```

## What you place in the editor is written back as Rust

Save (Ctrl+S) writes `<project>/src/renzora_authored.rs`: an ordinary Bevy
plugin that spawns everything you created in the editor.

```rust
// src/renzora_authored.rs: generated by Renzora. Do not edit.
use bevy::prelude::*;
use bevy::world_serialization::WorldAssetRoot;

pub struct AuthoredScenePlugin;

impl Plugin for AuthoredScenePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn);
    }
}

fn spawn(mut commands: Commands, assets: Res<AssetServer>) {
    commands.spawn((
        Name::new("Rock"),
        Transform::from_xyz(4.0, 0.0, -2.0),
        WorldAssetRoot(assets.load("models/nature/rock_largeA.glb#Scene0")),
    ));
}
```

It's plain Bevy with no engine types in it, so it compiles in a checkout that
has never seen Renzora. It's in your `src/`, in your git history, and diffs
like any other file. The editor declares and adds the plugin itself; to get the
same content from `cargo run`, add the two lines to your own `main.rs`:

```rust
mod renzora_authored;
// ...
.add_plugins(renzora_authored::AuthoredScenePlugin)
```

To take ownership of something in there, move it into one of your own files and
delete it in the editor. A line in both places spawns the object twice.

### What is not written back

**Entities your own code spawned.** They appear in the hierarchy, they're live,
you can move them and see it, and the move is not saved.

That isn't caution, it's the shape of the problem. A Bevy project's world is
*computed*. Move a tree that `scatter()` placed from a seeded PCG inside a loop
and there is no line of source that says where it is; there's an iteration index
and a random number. Writing that edit back would mean rewriting the generator,
which moves every other tree too. Only what the editor created has a line of its
own to own.

## Entities get named

The editor gives every entity a `Name`, derived from its own components, because
an entity without one is invisible to the hierarchy. It prefers a component *you*
defined over one Bevy did, so a `spawn((Player, Transform::default(), ...))`
shows up as **Player** and a bare camera as **Camera3d**. Repeats are numbered.

Your component names come from a table the generated crate root builds. Bevy
keeps component type names behind a feature the engine does not ship, so they
cannot be read back out of a running world. The table covers every
`#[derive(Component)]` type in your crate that is `pub` or `pub(crate)`; one
private to its module cannot be named from the crate root, so it is skipped and
the entity falls back to a Bevy component or to **Entity**. If the table itself
does not compile (a `#[cfg]`-gated component, a `Component` derived through a
macro) it is dropped and your project is built without it rather than failing
to open.

An entity that already has a `Name` keeps it, so naming things yourself is still
worth doing: it is the difference between **Collectible 14** and **Star (tower
top)**.

## Inspecting your components

Derive `Reflect` on a component and it gets its own inspector section, with a
row per field that edits the live value and a button in the header that opens
the file declaring it, at its declaration:

```rust
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct Player {
    pub yaw: f32,
    pub velocity: Vec3,
    pub grounded: bool,
}
```

There is no `app.register_type::<Player>()` to remember: the derive is the whole
opt-in. Renzora catches your crate's types up on Bevy's type registry when your
code loads, which Bevy itself cannot do for you because the registry is filled
before your library is in the process.

Only the components *your crate* declares are shown this way. The engine's own
reflected components keep their curated sections, because a generated section
for engine state that a system rewrites every frame would accept an edit and
silently revert it.

If your component has a `Default`, reflect that too (`#[reflect(Component,
Default)]`). It is what lets a field be reset, and what fills in a field added
after a scene was saved.

Field types have to be reflectable as well, which usually means adding
`#[derive(Reflect)]` to the enums and structs your components hold. The derive
registers them along with the component, so one derive on each type is enough.

### What does not get a section

A **marker** with no fields (`struct Dead;`) has nothing to edit, and a component
with no `Reflect` cannot be read at all. Those appear in a **Project Components**
list instead, which holds exactly what has no section of its own: the name, and
a click that opens its declaration.

## Your resources, too

The same derive works on a `Resource`, and they show up in the **Resources**
panel rather than the inspector, because a resource belongs to the world and not
to an entity:

```rust
#[derive(Resource, Reflect, Default)]
#[reflect(Resource, Default)]
pub struct PlayerStatus {
    pub wanted: u8,
    pub cash: u32,
}
```

Anything registered arrives there, so game state you would otherwise have to
`println!` at is live and editable while the game runs in the viewport.

## Play runs the game in its own process

Press Play and the editor launches the engine runtime pointed at your project,
as a separate process with its own window. It loads the same library the editor
built from your code, so Play costs a few seconds and usually nothing at all,
because the build from your last save is already there.

This is the shipped-game path, not a special editor mode: the same binary and the
same loading your game gets when you export it.

Stop kills the process. That is the point of doing it this way: everything the
game spawned and every value it changed lived in that process, so none of it can
leak back into the world you are editing. There is no snapshot to restore and
nothing to get subtly wrong.

Because the game reads your project from disk, the editor saves before it
launches, so anything you placed is written back as Rust first (see [What you
place in the editor is written back as
Rust](#what-you-place-in-the-editor-is-written-back-as-rust)).

## Cameras

Your `Camera3d` is adopted as the scene camera, which is what Play mode looks
for. In the editor it is deactivated and the editor's viewport camera is driven
onto its pose instead, so the game renders through the editor's pipeline; in a
shipped game it is left alone. The first camera found also becomes the default.

## When the editor cannot read your `main`

Some projects build their `App` somewhere the generator cannot follow: a helper
function returning an `App`, or a crate that is only a library. Name the plugins
to add and it stops guessing:

```toml
[package.metadata.renzora]
plugins = ["game::GamePlugin"]
```

Paths are resolved from your crate root, exactly as they would be in `main.rs`.

## Opening a different project

A Bevy plugin is installed while the `App` is being built, and there is no
`&mut App` once the editor is running, so code cannot be loaded into a session
that has already started. Importing therefore restarts the editor, carrying the
path across. If you have unsaved documents open you get the usual prompt first.

The same applies to your own code: **editing a `.rs` file and saving does not
hot-reload.** A plugin's systems are function pointers baked into Bevy's
schedules and there is no way to take them back out, so a code change means
restarting. Compiling the project is only a few seconds; the editor's own
startup is the rest.

## Limits worth knowing before you start

- **No dependency may pull in Bevy.** `bevy_rapier3d`, `avian3d`, `bevy_egui`
  and anything else that depends on Bevy would be compiled against a *second*
  Bevy, whose types are not the engine's. The build refuses and names the crates
  it found, rather than producing something that loads and corrupts the world.
  Ordinary crates (`serde`, `rand`, `noise`) are fine and are built by cargo as
  usual.
- **Build scripts do not run.** The plugin compiler invokes `rustc` directly, so
  anything `build.rs` generates will be missing. You are told when a crate has
  one.
- **Inspector edits are live only.** There is no scene file to write them back
  to: the truth is in your `level.rs`. Move something and it moves; restart and
  it is where your code puts it.
- **The game runs from the moment it loads.** `Startup` runs at load, and the
  plugin cannot be uninstalled, so there is no Stop that unloads it.
- **A workspace needs pointing at the game** when more than one member depends on
  Bevy:

  ```toml
  [workspace.metadata.renzora]
  member = "crates/game"
  ```

- **Raw `bevy_ui` draws over the editor.** A HUD spawned as `Node`s is in window
  space and the editor's chrome is too.

## Diagnosing a project that will not load

```sh
cargo run -p renzora_bevy_project --example stage_project -- <path>
```

prints what was read out of the manifest, writes the generated crate root, and
says what it skipped. Add `--sdk <dir>` to compile it as well. The generated root
is at `.renzora/bevy/src/lib.rs` and is worth reading when something is missing:
it is your crate, and what is not in it is what the editor could not see.

Add `.renzora/` to your `.gitignore`.
