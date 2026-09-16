# Renzora Engine `r1-alpha8`

## 2026-09-16 11:32
- fix(assets): a Bevy project's textures and models load in the asset browser and show thumbnails, instead of reporting every file as not found

## 2026-09-16 11:22
- feat(splash): New Bevy Project writes a small playable Bevy game into a folder and opens it
- fix(editor): a Bevy project no longer rebuilds itself forever, from the generated crate root its own build writes

## 2026-09-16 10:59
- feat(editor): a Bevy project's `Reflect` types all reach the type registry when its code loads, so its resources appear in the Resources panel
- feat(inspector): a project component's section header opens the file that declares it, at its declaration
- change(inspector): the Project Components list now holds only what has no section of its own, instead of repeating every component above it

## 2026-09-16 09:58
- feat(rendering): GPU occlusion culling skips meshes hidden behind other geometry, on by default and switchable in Settings
- perf(editor): primitives of the same kind share one mesh and primitives of the same colour share one material, so identical shapes draw as one batch

## 2026-09-16 09:31
- feat(editor): Tab accepts the number you typed into a field and moves to the next one, so a position can be typed X, Tab, Y, Tab, Z

## 2026-09-16 09:23
- fix(editor): a model spawned by a Bevy project's own code no longer shows two empty folders above its mesh in the hierarchy

## 2026-09-16 04:20
- feat(inspector): a Bevy project's entities show a Project Components section listing the components the project's own code declared
- feat(inspector): clicking one opens the file that declares it, at the line it is declared on
- feat(editor): a request to open a file in the code editor can name a line to jump to
- feat(inspector): a Bevy project's component that derives `Reflect` gets its own section with editable fields

## 2026-09-16 03:45
- fix(editor): clicking a panel in a Bevy project no longer freezes the cursor, when the project grabs it for mouse-look
- fix(editor): a Bevy project's camera is adopted before Bevy can turn it into an atmosphere probe, rather than racing it in the same schedule

## 2026-09-16 03:20
- fix(editor): a Bevy project whose camera carries an atmosphere environment probe no longer crashes the editor on startup

## 2026-09-16 03:05
- fix(editor): a Bevy project that builds its App across several statements now loads, instead of failing to compile with "borrow of moved value"

## 2026-09-16 02:45
- perf(editor): opening an unchanged Bevy project reuses the last build instead of recompiling it, which took about ten seconds every launch
- fix(editor): a Bevy project no longer relights the whole editor with its own ambient light

## 2026-09-16 02:15
- fix(editor): opening a Bevy project from the dashboard or File ▸ Recent loads its code, instead of opening an empty world

## 2026-09-16 01:40
- feat(editor): saving a Bevy project's Rust rebuilds it in the background, with compile errors in the Console and the Problems panel
- feat(editor): the status bar says whether a Bevy project is rebuilding, failed to build, or is waiting for a restart to load

## 2026-09-16 01:10
- feat(sdk): `cargo run -p renzora_plugin_build --example plugin_interop` builds two plugins that share a type, as an executable check that a plugin can be linked as a library by another

## 2026-09-16 00:35
- feat(editor): saving a Bevy project writes what you placed in the editor to `src/renzora_authored.rs` as ordinary Bevy code
- feat(editor): an entity in a Bevy project records whether it came from your code or from the editor, and only the editor's are written back

## 2026-09-16 00:05
- fix(editor): opening a Bevy project no longer switches off the camera the editor's own UI renders on, which left a black window
- fix(editor): the editor's UI camera is marked as chrome, so passes that look for scene content skip it

## 2026-09-15 23:52
- fix(assets): a `.glb` or `.gltf` loads as a model again; the gaussian-splatting loader had claimed both extensions and took over every glTF load in the engine

## 2026-09-15 23:35
- fix(editor): a Bevy project's models load, instead of every asset it requests at startup being reported as not found
- fix(editor): a Bevy project no longer paints the whole editor its own sky colour
- fix(editor): a Bevy project's camera is adopted whatever frame it is spawned on, and no longer draws over the editor
- fix(editor): opening a code-first project no longer reports a missing scene file it never had

## 2026-09-15 23:02
- fix(editor): importing a Bevy project works a second time, instead of silently doing nothing once its project.toml exists
- fix(editor): importing a Bevy project opens it, instead of restarting to the dashboard
- fix(editor): the Bevy project loader says why it loaded nothing rather than returning in silence

## 2026-09-15 22:58
- feat(editor): Import Bevy Project on the dashboard and in the File menu opens a hand-written Bevy crate
- feat(editor): Open Project accepts a folder, and routes a Bevy crate to the importer instead of refusing it

## 2026-09-15 21:43
- feat(editor): a hand-written Bevy crate can be opened as a project with `--project`, and the editor shows the world its code builds
- feat(editor): entities spawned by a Bevy project are named from their own components instead of being despawned as unnamed
- feat(plugin): a plugin or project is compiled with the Rust edition its own `Cargo.toml` asks for
- fix(editor): Open Project picks a folder instead of a `project.toml`, so a project without one can be opened

## 2026-09-15 13:52
- fix(linux): the editor window carries an app id, so a dock can show its icon and pinning it works

## 2026-09-15 13:38
- feat(editor): asset context menus can copy project-relative paths

## 2026-09-15 13:16
- fix(plugins): deleting a plugin that is currently loaded works instead of failing on Windows, and the folder is reclaimed at the next start

## 2026-09-15 12:37
- fix(marketplace): updating an installed plugin works while the editor is open, instead of failing with "the directory is not empty"

## 2026-09-15 12:14
- feat(marketplace): an Updates view lists the installed plugins with a newer version published, and updates them
- feat(marketplace): a card says what you already have: grey Installed with a tick, or an amber Update when there is a newer version
- feat(update): the software update dialog lists plugin updates under the engine's own versions
- feat(settings): Settings ▸ Editor ▸ Plugins marks plugins with an update and counts them above the grid
- feat(settings): a Plugin Update Reminders switch decides whether the editor volunteers that a plugin is out of date
- feat(export): the exporter's Plugins tab marks any plugin that would ship out of date
- fix(marketplace): a category's card grid fills the width instead of leaving most of a card's worth of gap at the end of every row

## 2026-09-15 11:46
- feat(marketplace): a plugin states the engine each release was built for, so you are offered the newest version your editor can actually run instead of one that will not load
- feat(marketplace): the plugin store hides listings with no release for your engine, and shows the version you would get rather than the newest one published
- feat(marketplace): the update check now says when a newer version exists but needs a newer editor, instead of showing nothing
- feat(plugins): a plugin can add its own entry to the Assets and Attach create menus
- fix(assets): the Lua Script entry no longer appears when no Lua interpreter is installed
- fix(scripting): a plugin can implement a scripting language again, which fixes Lua
- fix(plugins): a plugin's artwork shows up in Settings and the exporter again, whatever image format it ships
- build(ci): every nightly now checks that the published plugins still compile against the engine it just built, on all six desktop platforms
- build: the macOS and Linux build artifacts no longer carry the application twice, halving them from 1.16 GB to about 660 MB
- feat(sdk): a plugin or Rust script that fails to build now writes the compiler's diagnostic to a `build.log` in its own build directory, so it can be read after the window that showed it has closed
- feat(inspector): a component the graphics quality tier has switched off now shows an amber warning in its header saying so, instead of looking broken
- fix(marketplace): installing a free asset without signing in now counts towards the creator's downloads, which it never did
- fix(marketplace): installing a multi-file asset counts one download instead of two
- feat(marketplace): opening an asset in the editor's store now counts a view, as opening its page on the website already did
- fix(particles): removing a particle effect, or pointing it at a different file, now works instead of leaving the old effect drawing or frozen
- fix(particles): clearing the effect file leaves the emitter empty rather than swapping in a default effect that keeps emitting
- fix(particles): editing a `.particle`'s spawn rate, burst count or timing now takes effect, which only its colours did before
- fix(particles): an effect's light is removed when its effect no longer has one
- fix(scene): a particle emitter no longer saves its live spawner timing into the scene file
- feat(scene): choose what happens when a scene is edited outside the editor while you have unsaved changes: ask, reload, or keep yours (Settings → Editor → Scenes)
- feat(scene): deleting a file the open scene still uses now warns and names the entities using it, instead of the mesh quietly vanishing
- feat(particles): a `.particle` edited outside the editor updates every entity using it
- feat(ui): a `.html` template edited outside the editor rebuilds the canvases using it
- fix(assets): a file saved by an editor that writes to a temporary name and renames it now hot-reloads, which covers most editors and previously reloaded nothing
- feat(engine): the editor watches the whole project, so a file edited outside it is picked up as soon as it is saved
- fix(engine): a file being saved is no longer reported while it is still half-written under a temporary name
- fix(assets): a texture, model or sound edited outside the editor reloads on save, which it never did before
- perf(scripting): a Rust script rebuilds when you save it rather than up to half a second later, and an idle project costs nothing to watch
- fix(scripting): opening a project no longer compiles every Rust script twice
- perf(editor): the Assets panel, the script picker and the material picker no longer re-read the project on a timer to notice files changed outside the editor
- perf(editor): the script and material pickers update from the file that changed instead of re-reading the project, so their cost no longer grows with project size
- perf(editor): the Scenes panel no longer reads the scenes folder off disk on every frame it is open
- perf(editor): the editor no longer stats the active theme's shader files every frame to notice a theme edit
- perf(editor): project fonts and language packs are picked up when they change rather than by re-reading their folders on a timer
- perf(editor): a folder tile in the Assets panel walks its contents once instead of every five seconds for as long as it is on screen
- feat(scene): a scene edited outside the editor reloads in the viewport, and says so instead of reloading when you have unsaved changes
- fix(scripting): the editor no longer stutters twice a second on a project with a lot of files, which the Rust script watcher was walking on the main thread
- fix(editor): the VR on/off line at startup is printed again, having been written before the logger existed and silently dropped
- feat(editor): every editor launch logs whether VR is on or off, and whether pipelined rendering is on
- feat(editor): the console names each viewport camera as it switches on or off, and says why
- fix(editor): the editor no longer loses a third of its frame rate just because an OpenXR runtime is installed; VR editing is opt-in with `--xr`
- feat(plugin): a plugin can use the engine's physics types, so physics plugins no longer carry their own copy
- feat(plugin): a plugin can declare script functions, which only engine crates could do before
- refactor(plugin): Solari, lens distortion, wind and water ship as installed plugins rather than in the binary
- refactor(plugin): there is one kind of installable plugin, and it is an ordinary Bevy plugin shipped as source
- refactor(scripting): a language backend is a plain Rust trait a plugin registers, rather than a C-ABI boundary
- feat(settings): Reset to Defaults no longer offers to clear plugin settings, which plugins now keep as ordinary resources
- refactor(net): the HTTP client is a plain Rust trait rather than a C-ABI boundary
- refactor(audio): the mixer is a plain Rust trait rather than a C-ABI boundary
- fix(net): a backend that panics now fails the requests already handed to it, instead of leaving them parked until their own timeouts
- perf(plugin): plugins build several at a time instead of one after another
- perf(plugin): the plugins that take longest to build start first, so one no longer trails after the rest have finished
- fix(plugin): a plugin's cargo build tree is deleted once it has been built, which was most of what an install kept on disk
- fix(plugin): the setup progress bar follows plugins finishing rather than starting, so it no longer jumps ahead and then stalls
- fix(plugin): a disabled plugin that has never been built now appears in Settings, so it can be turned back on
- fix(ci): the Windows build no longer fails its own MSVC runtime audit when there is nothing to report
- feat(ci): a manual build can target a single arch, such as `windows-x64`, instead of a whole operating system
- fix(windows): the engine, its plugins and exported games link the Visual C++ runtime in, so they start on a machine that has never installed the redistributable
- fix(macos): the editor no longer opens as "damaged" — the `.app` ships signed again
- feat(ci): macOS releases can be Developer ID signed and notarized when the signing secrets are set
