# Releases & Nightlies

How the engine gets from `main` to something someone can download.

Everything here is the **Build Engine** workflow, `.github/workflows/build-engine.yml`, plus the packaging script it calls, `scripts/package-release.sh`.

## The version is one string

`renzora::version::ENGINE_VERSION` in `crates/renzora/src/version.rs` is the engine's version — `r1-alpha7` today. It is what:

- the About dialog and the splash show,
- the docs directory is named after (`docs/r1-alpha7/`),
- the release workflow builds its tag from,
- the export downloader asks GitHub for.

Bumping a version means editing that constant and creating `docs/<new-version>/` in the same change. Nothing else hardcodes it; four places used to, and they disagreed.

A published binary also carries two values CI stamps in at compile time, read with `option_env!` when the contract crate compiles:

| Variable | Value | Absent means |
|---|---|---|
| `RENZORA_RELEASE_TAG` | `r1-alpha7` or `r1-alpha7-nightly-16aug26` | built from source (a *dev* build) |
| `RENZORA_BUILD_COMMIT` | the commit the release was cut from | — |

`option_env!` is baked in at compile time, so only a **cold** build picks these up. CI builds cold every run; a warm local tree ignores them, which is correct because a local tree is a dev build anyway.

## Three ways a build starts

| Trigger | Tag | Kind |
|---|---|---|
| `schedule`, 02:00 UTC daily | `r1-alpha7-nightly-16aug26` | prerelease |
| `push` of an `r1-alpha*` tag | the tag itself | full release |
| `workflow_dispatch` | your choice of `none` / `nightly` / `release` | — |

The nightly date is `%d%b%y` lowercased — `16aug26`. Tags sort readably per version because the version comes first.

**Nightlies skip a quiet day.** If nothing landed on `main` in the last 24 hours the run stops at the `setup` job rather than publishing an identical release and burning ~6 runner-hours. **A same-day re-run replaces its nightly** rather than inventing a suffix nobody could predict. The 14 most recent nightlies for a version are kept; older ones are deleted with their tags. `r1-alpha*` releases are never pruned.

## What gets built where

| Platform | Where | Notes |
|---|---|---|
| linux x64 + arm64 | `ghcr.io/renzora/linux` container | one container cross-builds both |
| macos x64 + arm64 | `ghcr.io/renzora/macos` container | osxcross builds both slices |
| windows x64 | `ghcr.io/renzora/windows` container | via xwin |
| windows arm64 | native `windows-11-arm` runner | see below |
| wasm32 | `ghcr.io/renzora/wasm` container | runtime + editor bundles |

**Windows ARM64 is the one target the Docker toolchain cannot produce.** The only MSVC pieces Microsoft allows to be redistributed (so xwin can bake them into a public image) are the CRT and SDK, which leaves clang as the C compiler — and clang can't stand in for MSVC on ARM64, because it emits MSVC NEON intrinsics as undefined externals no redistributable library provides. So that slice builds natively with the real MSVC toolchain on a GitHub-hosted arm64 runner.

That job has **no build cache** and takes hours, which makes it the long pole on every publishing run. The `publish` job therefore treats it as best-effort: whatever slices arrived get published, and a missing one is simply absent from `manifest.json`, which the editor reads as "no template for that platform".

## What a release contains

Two assets per desktop platform, from `scripts/package-release.sh`:

| Asset | What it is |
|---|---|
| `<platform>.zip` | the **engine** — `renzora-editor` and `renzora` together. Linux ships the `.AppImage`; macOS the `.app`. |
| `renzora-runtime-<platform>.zip` | the **export template** — the game runtime and its `plugins/`, no editor. |

Plus `manifest.json` (every asset with size and SHA-256, keyed by platform) and `SHA256SUMS`.

The engine asset keeps the bare `<platform>.zip` name the earlier releases used, so links to it don't rot. The template name is derived from `Platform::dist_dir_name()` in code and from the platform directory name in the script, so the two halves of the contract cannot drift — they did once, and the result was a download feature that could never have succeeded.

### Executable bits

`actions/upload-artifact` does **not** preserve unix file modes, so every binary reaches the publish job as `0644`. `package-release.sh` restores the bit on `renzora`, `renzora-editor`, `AppRun` and `*.AppImage` **before** zipping, because `zip` stores whatever mode a file has at the moment it is archived. Without that pass, Linux and macOS releases ship an engine nobody can launch.

## The C-ABI plugins

`plugins/*` are separate cargo projects, not workspace members — deliberately, since as members they would inherit the engine's feature unification and link Bevy, destroying the property that lets a plugin built by any rustc load into any engine. The cost is that `cargo build --workspace` never sees them.

`docker/build-all.sh`'s `build_plugins` builds each one for the target triple and stages it into `dist/<platform>/plugins/`, before the AppImage/`.app` wrap moves that directory inside the bundle.

It is **best-effort per plugin**: these have real third-party dependencies (mlua compiles C, the HTTP plugin pulls the rustls/ring stack), and one that won't cross-compile for a given target is not a reason to sink the engine build. A failed plugin is named in the lane summary, with the tail of its build log, and is simply absent from the artifact. The summary matters: before this pass existed, CI artifacts shipped an *empty* `plugins/` — no Lua scripting, no HTTP, none of the ~50 post-process effects — and looked exactly like a successful build.

## Binary size

The engine is large — `.text` alone was 134 MB of the runtime's 187 MB — and essentially all of it is code, not data. Symbols are already stripped (`strip = "symbols"`; there are no `.debug*` sections and no PDB path embedded in a release binary), so there is nothing to sweep out. What there is, is monomorphized generics: a release `.rdata` carries ~12,000 distinct `bevy_ecs::` type-name strings, one per instantiated system-param combination.

Measured on `windows-x64`, both changes stacked:

| | As shipped before | + size-opt profile | + UPX | Total |
|---|---|---|---|---|
| `renzora.exe` | 187.0 MB | 138.2 MB | **24.9 MB** | −86.7% |
| `renzora-editor.exe` | 265.6 MB | 194.3 MB | **35.1 MB** | −86.8% |

The whole installed tree goes from ~470 MB to ~77 MB (the plugins stay unpacked at ~15 MB). The profile change alone accounts for a 26% cut and is the more durable half: it is less code, so it is less to page in, less to decompress and a smaller working set. UPX's 83% is a disk-and-download number that costs RAM and startup time back — see below.

Three things act on that, in order of where they apply:

**1. `[profile.dist]` is size-optimised** — `opt-level = "s"` with `lto = "thin"`. Neither works alone here, and the profile used to carry a note that thin LTO made both binaries *bigger*. That finding is real, but it was measured at `opt-level = 2`, where thin LTO's dominant effect is cross-crate inlining, which is not size-constrained. Under `opt-level = "s"` the inliner is size-aware and LTO's dead-stripping dominates instead. This trades frame time for size, deliberately, in the editor as well as the game — **if the viewport regresses, this is the first thing to put back.**

It also means every local `cargo renzora` now pays thin-LTO link time. If that becomes intolerable while iterating, override it per-invocation rather than editing the profile:

```sh
CARGO_PROFILE_DIST_LTO=false CARGO_PROFILE_DIST_OPT_LEVEL=2 cargo renzora
```

Two knobs are deliberately *not* set:

- **`codegen-units = 1`** — measured to help only fat LTO, and to cost a lot of build time for little size under thin.
- **`panic = "abort"`** — `renzora_plugin` guards every call across the C-ABI boundary with `catch_unwind` (audio/net/script backends, `ecs.rs`, `host/mod.rs`). Under `abort` those become no-ops and a panicking third-party plugin takes the editor down instead of being contained. It would save the ~6 MB of `.pdata` unwind tables; it is not worth it.

The `plugins/*` and `tools/updater` builds are unaffected — each is its own workspace with its own `[profile.dist]`.

**2. UPX packs the executables**, in `compress_binaries` (`docker/build-all.sh`), with `--best --lzma`. Measured on the `dist` runtime: **187.3 MB → 31.7 MB, an 83% saving**, and the packed binary boots through full plugin and scripting startup. `--brute` was measured against it and produces a **byte-for-byte identical** file on this input (33,363,456 bytes) while taking 1529 s instead of ~100 s — `--lzma` already selects UPX's strongest compressor, and the extra combinations `--brute` tries have nothing better to find on an amd64 PE. Do not "upgrade" the lanes to `--brute`.

Two things are deliberately not packed: **`renzora-update`**, because it is what repairs a broken install and should be the *last* binary with extra machinery between the loader and `main`; and **`plugins/*`**, 68 libraries totalling ~15 MB against 450 MB of executables.

**UPX is not free at runtime.** A normally-linked executable is demand-paged: the OS maps it and faults in only the pages actually touched, so a 138 MB binary with ~40 MB of hot code costs ~40 MB of working set. A packed executable cannot do that — the whole image is decompressed into private committed memory before `main` runs. Packing therefore trades disk for **RAM and a startup pause, on every launch**, for the editor as much as for an exported game. If that becomes the wrong trade for the editor specifically, `compress_binaries` takes an explicit list of binaries and dropping `renzora-editor` from it is a one-line change.

**Ordering matters on macOS.** `compress_binaries` runs *before* `fixup_macos`, because packing rewrites the file and invalidates any signature it carries — and arm64 macOS refuses a binary whose signature does not verify. `rcodesign` must sign the packed file, not the other way round.

On Linux, packing before the AppImage wrap is also the right order: LZMA beats the AppImage's own squashfs compression, so the resulting `.AppImage` lands near the UPX size rather than the squashfs one.

**3. Bevy's feature set is deliberately maximal** and has *not* been trimmed. The justification recorded in `Cargo.toml` — that the shared `bevy_dylib`'s feature set was the plugin API surface and an input to the ABI hash — no longer applies, since nothing links Bevy but the engine itself. Trimming it (`bevy_solari`, `meshlet`) is therefore now possible and would be a real cut, but it removes engine capability rather than build overhead, so it is a product decision rather than a build one.

## The updater

The editor updates itself from these releases: **Help ▸ Check for Updates**, or the same item labelled **Update to `<tag>`** when the background check at startup has already found one. It downloads the `<platform>.zip` for the host, verifies it against the SHA-256 GitHub publishes for the asset, and replaces the install.

`crates/renzora_update` does the checking, downloading and staging. The replacement itself is `tools/updater` — a separate ~220 KB binary, `renzora-update`, that ships beside the editor:

1. The editor stages the new engine under `~/.renzora/updates/<tag>/staged/`.
2. It copies the sidecar to a temp directory, spawns it with the staged path, the install path, its own PID and a relaunch path, and calls `exit`.
3. The sidecar waits for that PID to disappear, moves the current install aside to a **sibling** `*.renzora-backup` (same volume, so the rename is atomic), installs the staged one, deletes the backup, and relaunches.

Any failure after the rename puts the backup back before reporting. The worst case is "the update didn't happen", never "the engine is gone".

Three details are load-bearing and easy to undo by accident:

- **The sidecar runs from a temp copy, not from the install folder.** Launched in place it would hold an open handle inside the very directory it is about to rename — which Windows refuses outright.
- **It must not inherit `prefer-dynamic`.** That would make it import a `std-<hash>.dll` that lives in the directory it is deleting. `tools/updater/.cargo/config.toml` switches it off with an explicit `=no`, exactly as `plugins/.cargo/config.toml` does, and for a worse failure mode: a plugin that won't load is skipped; an updater that won't load leaves a half-replaced engine and no process to repair it.
- **It is built by two separate paths**, because it is its own cargo workspace and `--workspace` never sees it: `build_updater` in `docker/build-all.sh` (containers) and `build_updater` in `xtask/src/main.rs` (`cargo renzora`). Both are non-fatal — a missing sidecar costs the in-place update and nothing else, and the editor says so rather than failing silently.

What "the install" is depends on the platform, and the sidecar picks its behaviour from what it is given rather than from a platform flag: a **directory** (a Windows install folder, or a macOS `.app`) is replaced wholesale; a **file** (a Linux `.AppImage`) is replaced on its own. `renzora_update::install::detect_layout` works out which, reading `$APPIMAGE` on Linux and walking up to the `.app` on macOS.

The sidecar is deliberately **excluded from export templates** — `renzora-runtime-<platform>.zip` carries the game runtime and its plugins, and an exported game has no business shipping an engine updater.

### Channels

`auto` (the default) follows the build: a nightly is offered newer nightlies, a release is offered releases, and a build from source tracks nightlies. `stable` and `nightly` override it, stored in `~/.renzora/editor.toml` as `update_channel`. It is stored as `auto` rather than resolved once, because the answer changes when you update — taking a nightly user to a release should move them to the stable channel, which a resolved value would not do.

The ordering that makes this work is in `crates/renzora_update/src/version.rs`: at every level, absent sorts above present, so `r1` > `r1-alpha7` > `r1-alpha7-nightly-16aug26`. The day a version ships, everyone on its nightlies is offered it.

### Running from a source checkout

The editor then lives in `<checkout>/dist/<platform>/`, and installing a release over that would overwrite build output. The updater detects the checkout (a `Cargo.toml` beside `crates/` and `src/main.rs`, walking up) and offers the check and the release notes but no install button. To test the updater end to end, copy `dist/<platform>/` somewhere outside the checkout and run it from there.

## Cutting a real release

1. Bump `ENGINE_VERSION` in `crates/renzora/src/version.rs` and create `docs/<version>/`.
2. Land it on `main`.
3. Tag it: `git tag r1-alphaN && git push origin r1-alphaN`.

The tag push triggers the workflow, which builds every platform and publishes the release. Nothing is uploaded by hand.

## Running it manually

**Actions → Build Engine → Run workflow.** Pick a platform (or `all`) and a publish mode. `publish: none` builds and uploads GitHub *artifacts* (7-day retention) without creating a release — the right choice for checking that a platform still builds.

## See also

- [Building Export Templates](/docs/r1-alpha7/packaging/export-templates) — how the editor resolves, downloads and uses a template.
- [Cross-Compilation](/docs/r1-alpha7/packaging/cross-compilation) — the toolchain images the build jobs run in.
- [Building from source](/docs/r1-alpha7/setup/building-from-source) — `cargo renzora` for local work.
