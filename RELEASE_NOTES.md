# Renzora Engine `r1-alpha8`

## Unreleased
- fix(scripting): the editor no longer stutters twice a second on a project with a lot of files, which the Rust script watcher was walking on the main thread
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
