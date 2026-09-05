# Releases & Nightlies

How the engine gets from `main` to something someone can download.

Everything here is the **Build Engine** workflow, `.github/workflows/build-engine.yml`, plus the packaging script it calls, `scripts/package-release.sh`.

## The version is one string

`renzora::version::ENGINE_VERSION` in `crates/renzora/src/version.rs` is the engine's version — `r1-alpha8` today. It is what:

- the About dialog and the splash show,
- the docs directory is named after (`docs/r1-alpha8/`),
- the release workflow builds its tag from,
- the export downloader asks GitHub for.

Bumping a version means editing that constant and creating `docs/<new-version>/` in the same change. Nothing else hardcodes it; four places used to, and they disagreed.

A published binary also carries two values CI stamps in at compile time, read with `option_env!` when the contract crate compiles:

| Variable | Value | Absent means |
|---|---|---|
| `RENZORA_RELEASE_TAG` | `r1-alpha8` or `r1-alpha8-nightly-16aug26` | built from source (a *dev* build) |
| `RENZORA_BUILD_COMMIT` | the commit the release was cut from | — |

`option_env!` is baked in at compile time, so only a **cold** build picks these up. CI builds cold every run; a warm local tree ignores them, which is correct because a local tree is a dev build anyway.

## Three ways a build starts

| Trigger | Tag | Kind |
|---|---|---|
| `schedule`, 02:00 UTC daily | `r1-alpha8-nightly-16aug26` | prerelease |
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
| `<platform>.zip` | the **engine** — `renzora` with the editor image (`renzora_editor.*`) beside it, plus `plugins/` and `sdk.tar.zst`. Windows is that tree flat; Linux ships the `.AppImage`; macOS the `.app`. |
| `renzora-runtime-<platform>.zip` | the **export template** — the game runtime and its `plugins/`, no editor. |

Plus `manifest.json` (every asset with size and SHA-256, keyed by platform) and `SHA256SUMS`.

The engine asset keeps the bare `<platform>.zip` name the earlier releases used, so links to it don't rot. The template name is derived from `Platform::dist_dir_name()` in code and from the platform directory name in the script, so the two halves of the contract cannot drift — they did once, and the result was a download feature that could never have succeeded.

### Executable bits

`actions/upload-artifact` does **not** preserve unix file modes, so every binary reaches the publish job as `0644`. `package-release.sh` restores the bit on `renzora`, `renzora-update`, `AppRun` and `*.AppImage` **before** zipping, because `zip` stores whatever mode a file has at the moment it is archived. Without that pass, Linux and macOS releases ship an engine nobody can launch.

Only things that are *launched*. The editor image and everything under `plugins/` are `dlopen`'d rather than executed, so they stay `0644`.

## The plugins in a release

**The first-party plugins are no longer in this repository.** The marketplace
ships them, and everything here that existed only to build them went with them:
`xtask/src/native_plugin.rs`, the C-ABI cargo loop, `cargo renzora plugin
<name>`, both plugin-staging passes, the `--plugins` coverage scope, CI's two
plugin jobs, and `build_plugins` in the container script.

What still lands in `dist/<platform>/plugins/` is the workspace's own **cdylib
distribution plugins** — crates that declare themselves with `renzora::add!` and
build as part of `--workspace`. `stage_dist` in `docker/build-all.sh` sweeps
every cdylib the build produced into that directory, minus an explicit skip list:
the SDK dylibs (`renzora_dylib`, `renzora_ember_dylib`) and the two shared engine
images ship *beside* the executable, never in `plugins/`, because swept in they
would be tens of MB of duplicate weight that the C-ABI loader would then `dlopen`
looking for an entry point they do not export.

`stage()` no longer writes to that directory at all. It used to sweep every `.so`
in `dist/<platform>/plugins/` on the assumption that xtask had put them there —
which stopped being true once the marketplace installed into it and users could
drop a plugin in by hand, so the sweep could only delete things it had not
created.

## Binary size

The engine is large — `.text` alone was 134 MB of the runtime's 187 MB — and essentially all of it is code, not data. Symbols are already stripped (`strip = "symbols"`; there are no `.debug*` sections and no PDB path embedded in a release binary), so there is nothing to sweep out. What there is, is monomorphized generics: a release `.rdata` carries ~12,000 distinct `bevy_ecs::` type-name strings, one per instantiated system-param combination.

Measured on `windows-x64`, both changes stacked:

| | As shipped before | + size-opt profile | + UPX | Total |
|---|---|---|---|---|
| `renzora.exe` | 187.0 MB | 138.2 MB | **24.9 MB** | −86.7% |
| `renzora-editor.exe` | 265.6 MB | 194.3 MB | **35.1 MB** | −86.8% |

These numbers are from when the editor was a **second executable**. It is a loadable image now (`renzora_editor.dll` beside `renzora.exe`), so the second row no longer names a file that ships — the measurement stands, the filename is history. UPX packs executables only, so the image is not packed and the ratios above no longer describe the whole download.

The whole installed tree goes from ~470 MB to ~77 MB (the plugins stay unpacked at ~15 MB). The profile change alone accounts for a 26% cut and is the more durable half: it is less code, so it is less to page in, less to decompress and a smaller working set. UPX's 83% is a disk-and-download number that costs RAM and startup time back — see below.

Three things act on that, in order of where they apply:

**1. There are two release profiles**, because shipping and iterating want opposite things — the smallest possible binary regardless of link time, versus a fast link regardless of size.

| Profile | Used by | Settings |
|---|---|---|
| `dist` | `cargo renzora`, `cargo dist`, every `--profile dist` command in the docs and CI | `opt-level = 2`, `lto = false` |
| `release` | `docker/build-all.sh`, i.e. every GitHub build | `opt-level = "s"`, `lto = "thin"` |

`dist` is what a contributor uses all day and it is deliberately unchanged — thin LTO on this graph is minutes of link time on every build, and at `opt-level = 2` it does not even shrink anything (measured: it made both binaries *bigger*, 170 → 174 MB and 238 → 243 MB, because at that opt-level its dominant effect is cross-crate inlining, which is not size-constrained).

`release` pairs size-opt with thin LTO, which is the combination that works: `opt-level = "s"` makes the inliner size-aware so LTO's cross-crate dead-stripping dominates. It is selected by `PROFILE` in `build-all.sh` (override with `RENZORA_PROFILE=dist` to reproduce a lane quickly), and it is reproducible by hand — `cargo build --profile release --workspace` — which is why it is a named profile rather than a pile of env overrides.

Two exceptions to know about:

- **`tools/updater` stays on its own `dist`.** It is a separate cargo workspace with its own tuned profile, and its output is kilobytes.
- **`windows-arm64` builds through xtask**, so it gets `dist`. That lane sets `CARGO_PROFILE_DIST_OPT_LEVEL` / `CARGO_PROFILE_DIST_LTO` in the workflow to match `release`; keep them in step if the profile changes.

This trades frame time for size in shipped builds, deliberately, in the editor as much as the game — **if the viewport regresses in a release but not locally, this is why.**

Two knobs are deliberately *not* set:

- **`codegen-units = 1`** — measured to help only fat LTO, and to cost a lot of build time for little size under thin.
- **`panic = "abort"`** — `renzora_plugin` guards every call across the C-ABI boundary with `catch_unwind` (audio/net/script backends, `ecs.rs`, `host/mod.rs`). Under `abort` those become no-ops and a panicking third-party plugin takes the editor down instead of being contained. It would save the ~6 MB of `.pdata` unwind tables; it is not worth it.

The `tools/updater` build is unaffected — it is its own workspace with its own `[profile.dist]`.

**2. UPX packs the executables**, in `compress_binaries` (`docker/build-all.sh`), with `--best --lzma`. Measured on the `dist` runtime: **187.3 MB → 31.7 MB, an 83% saving**, and the packed binary boots through full plugin and scripting startup. `--brute` was measured against it and produces a **byte-for-byte identical** file on this input (33,363,456 bytes) while taking 1529 s instead of ~100 s — `--lzma` already selects UPX's strongest compressor, and the extra combinations `--brute` tries have nothing better to find on an amd64 PE. Do not "upgrade" the lanes to `--brute`.

Two things are deliberately not packed: **`renzora-update`**, because it is what repairs a broken install and should be the *last* binary with extra machinery between the loader and `main`; and the **staged plugin cdylibs**, tens of MB against 450 MB of executables.

**UPX is not free at runtime.** A normally-linked executable is demand-paged: the OS maps it and faults in only the pages actually touched, so a 138 MB binary with ~40 MB of hot code costs ~40 MB of working set. A packed executable cannot do that — the whole image is decompressed into private committed memory before `main` runs. Packing therefore trades disk for **RAM and a startup pause, on every launch**, for the editor as much as for an exported game. If that becomes the wrong trade, `compress_binaries` takes an explicit list of binaries and dropping `renzora` from it is a one-line change — though note that is now the *only* packed engine binary, since the editor is a `dlopen`'d image and UPX packs executables only.

**Ordering matters on macOS.** `compress_binaries` runs *before* `fixup_macos`, because packing rewrites the file and invalidates any signature it carries — and arm64 macOS refuses a binary whose signature does not verify. `rcodesign` must sign the packed file, not the other way round.

On Linux, packing before the AppImage wrap is also the right order: LZMA beats the AppImage's own squashfs compression, so the resulting `.AppImage` lands near the UPX size rather than the squashfs one.

**3. Bevy's feature set is deliberately maximal** and has *not* been trimmed. The justification recorded in `Cargo.toml` — that the shared `bevy_dylib`'s feature set was the plugin API surface and an input to the ABI hash — no longer applies, since nothing links Bevy but the engine itself. Trimming it (`bevy_solari`, `meshlet`) is therefore now possible and would be a real cut, but it removes engine capability rather than build overhead, so it is a product decision rather than a build one.

## The updater

The editor updates itself from these releases: **Help ▸ Check for Updates**, or the same item labelled **Update to `<tag>`** when the background check at startup has already found one. When there is one, an **Update to `<tag>`** chip also appears on the right of the top bar, next to the window controls; it opens the same overlay.

The dialog lists **every** version the channel offers, newest first — releases marked with a seal, nightlies with a moon, the running build marked *current*, and any release with no build for your platform greyed out rather than hidden. Picking a row selects it, **including an older one**: rolling back is a download like any other, and the check already fetched the whole list, so offering only the newest would be throwing information away. Selecting a different version discards anything already staged, since that download belongs to a different tag. It downloads the `<platform>.zip` for the host, verifies it against the SHA-256 GitHub publishes for the asset, and replaces the install.

`crates/renzora_update` does the checking, downloading and staging. The replacement itself is `tools/updater` — a separate ~220 KB binary, `renzora-update`, that ships beside the editor:

1. The editor stages the new engine under `~/.renzora/updates/<tag>/staged/`.
2. It copies the sidecar to a temp directory, spawns it with the staged path, the install path, its own PID and a relaunch path, and calls `exit`.
3. The sidecar waits for that PID to disappear, moves the current install aside to a **sibling** `*.renzora-backup` (same volume, so the rename is atomic), installs the staged one, deletes the backup, and relaunches.

Any failure after the rename puts the backup back before reporting. The worst case is "the update didn't happen", never "the engine is gone".

Three details are load-bearing and easy to undo by accident:

- **The sidecar runs from a temp copy, not from the install folder.** Launched in place it would hold an open handle inside the very directory it is about to rename — which Windows refuses outright.
- **It must not inherit `prefer-dynamic`.** That would make it import a `std-<hash>.dll` that lives in the directory it is deleting. `tools/updater/.cargo/config.toml` switches it off with an explicit `=no`, and for a worse failure mode: a plugin that won't load is skipped; an updater that won't load leaves a half-replaced engine and no process to repair it.
- **It is built by two separate paths**, because it is its own cargo workspace and `--workspace` never sees it: `build_updater` in `docker/build-all.sh` (containers) and `build_updater` in `xtask/src/main.rs` (`cargo renzora`). Both are non-fatal — a missing sidecar costs the in-place update and nothing else, and the editor says so rather than failing silently.

What "the install" is depends on the platform, and the sidecar picks its behaviour from what it is given rather than from a platform flag: a **directory** (a Windows install folder, or a macOS `.app`) is replaced wholesale; a **file** (a Linux `.AppImage`) is replaced on its own. `renzora_update::install::detect_layout` works out which, reading `$APPIMAGE` on Linux and walking up to the `.app` on macOS.

The sidecar is deliberately **excluded from export templates** — `renzora-runtime-<platform>.zip` carries the game runtime and its plugins, and an exported game has no business shipping an engine updater.

### Channels

`auto` (the default) follows the build: a nightly is offered newer nightlies, a release is offered releases, and a build from source tracks nightlies. `stable` and `nightly` override it, stored in `~/.renzora/editor.toml` as `update_channel`. It is stored as `auto` rather than resolved once, because the answer changes when you update — taking a nightly user to a release should move them to the stable channel, which a resolved value would not do.

The ordering that makes this work is in `crates/renzora_update/src/version.rs`. Release and pre-release sort with absent above present (`r1` > `r1-alpha8`), and within one version a three-state `Stage` orders `Dev < Nightly(date) < Final`.

That third state earns its place. It started as a two-state "is this a nightly?", which could say that `r1-alpha8` outranks `r1-alpha8-nightly-16aug26` — correct — but had no way to place a build from source, which has no tag at all. Such a build reported the bare version, parsed as the *finished* release, and so outranked every nightly of its own version: the dialog said "Renzora is up to date" while displaying the release notes of the nightly it was declining to offer. A source build is the *least* finished build of a version, not the most.

### Running from a source checkout

The editor then lives in `<checkout>/dist/<platform>/`, so installing a release replaces the tree `cargo renzora` stages into. That is recoverable — rebuild and it comes back — but it is not something to do on one stray click, and the next `cargo renzora` would overwrite the downloaded engine again.

The updater detects the checkout (a `Cargo.toml` beside `crates/` and `src/main.rs`, walking up) and makes you say it twice. The action button reads **Overwrite dist/…** in amber; the first click arms it, a line appears naming the exact directory about to be replaced, and the button turns red and reads **Confirm — Overwrite & Restart**. Any re-check or channel switch disarms it.

Downloading is never gated: it writes to `~/.renzora/updates/<tag>/`, not to the install. Only the install itself asks.

## Cutting a real release

1. `ENGINE_VERSION` in `crates/renzora/src/version.rs` must already be the version you are releasing, with `docs/<version>/` beside it. **Do not bump it to the next version in this commit** — the workflow reads that constant as `version` and compares it against the tag to decide what kind of build this is, so a constant that runs ahead of the tag makes the release classify itself as a *nightly*.
2. Write `RELEASE_NOTES.md` at the repo root for this version. Its first line must name the version.
3. Record the release in `releases.json`: the version, the commit it is cut from, the ABI hash, and `"status": "released"`.
4. Land that on `main`.
5. Tag it: `git tag r1-alphaN && git push origin r1-alphaN`.

The tag push triggers the workflow, which builds every platform and publishes the release. Nothing is uploaded by hand.

Opening the *next* version — bumping `ENGINE_VERSION`, forking `docs/<next>/`, pointing `docs/_versions.json` and CLAUDE.md §4 at it — is a separate change that lands **after** the tag.

### The notes

`RELEASE_NOTES.md` at the repo root is the body of a GitHub release, and it is written as work lands rather than reconstructed at the end. Every feature and every fix adds its line under the file's top **`## Unreleased`** section in the same change that ships it.

**A nightly publishes that section.** The publish job lifts whatever sits under `## Unreleased` into the nightly's release page under *Since the last nightly*, above the standard asset descriptions. That is the whole reason the section is written in advance and named `Unreleased` rather than dated: a nightly's tag is only known at build time — the schedule skips a day nothing landed on — so naming it ahead would mean guessing a date, and a wrong guess publishes the wrong list under the wrong tag.

After a nightly goes out, rename its section to the tag that shipped it and open a fresh `## Unreleased` above. Nothing breaks if that is late: the job keys on the `## Unreleased` heading alone and ignores every heading below it. An empty or missing section is not an error either — the nightly falls back to the asset boilerplate by itself.

**A full release publishes the whole file.** Cutting `r1-alphaN` replaces `RELEASE_NOTES.md` with curated notes for that version — prose organised by theme, not the running list, which has by then served its purpose of making the prose writable. The job `cat`s the file in ahead of the asset boilerplate, so the notes and the "what's in the download" text are one page.

Because the file is written by hand, its failure mode is not a missing file but a **stale** one — the previous version's notes shipping under this version's tag, which nothing downstream could detect. So `setup` refuses to start a release whose `RELEASE_NOTES.md` does not name the version in its first line. That check lives in `setup` rather than in `publish` because `setup` costs seconds and `publish` is ~6 runner-hours downstream: a mismatch should stop before anything is built.

### The ABI hash

`releases.json` at the repo root is the canonical record of what each release froze — version, commit, ABI hash. The value is `RENZORA_BUILD_HASH`, the FNV-1a in `build.rs` over `<crate version>-<rustc version>-bevy<minor>`. Read it from a build rather than deriving it by hand: it is emitted into `target/<profile>/build/renzora_app-*/output`, and because it is keyed on the *toolchain* and not on engine source, two releases built with the same pinned rustc and Bevy legitimately share a hash.

## Running it manually

**Actions → Build Engine → Run workflow.** Pick a platform (or `all`) and a publish mode. `publish: none` builds and uploads GitHub *artifacts* (7-day retention) without creating a release — the right choice for checking that a platform still builds.

## See also

- [Building Export Templates](/docs/r1-alpha8/packaging/export-templates) — how the editor resolves, downloads and uses a template.
- [Cross-Compilation](/docs/r1-alpha8/packaging/cross-compilation) — the toolchain images the build jobs run in.
- [Building from source](/docs/r1-alpha8/setup/building-from-source) — `cargo renzora` for local work.
