# Installation

Welcome! Let's get Renzora running on your computer. There are two ways to get it — pick whichever feels easiest, and you'll be in the editor in a few minutes.

Here's what you're installing — the Renzora editor, where you'll build your games:

![The Renzora editor with a 3D city scene open: a scene list on the left, the viewport with move/rotate gizmos in the middle, a component library and Transform panel on the right, and an asset browser along the bottom.](/assets/previews/interface.png)

The two ways to get Renzora:

- **Download a prebuilt build** — the easiest start. No tools to set up; just download, extract, and run.
- **Build from source with `cargo renzora`** — the way to work on your own projects. Rust and a checkout, nothing else.

> **You don't need Docker to install Renzora.** Building from source is an ordinary `cargo` build of your own platform. Docker enters the picture only when you want to ship — producing export templates for platforms you don't own, like building a macOS or Android bundle from a Windows machine. That's covered under [cross-compiling](#cross-compiling-for-other-platforms).

## System requirements

| | Minimum |
|---|---|
| **Windows** | Windows 10+, 64-bit |
| **macOS** | macOS 12 Monterey or newer |
| **Linux** | Ubuntu 22.04+ / Fedora 38+ |
| **GPU** | A GPU with Vulkan, Metal, or DX12 (Renzora renders through `wgpu`) |
| **RAM** | 4 GB minimum, 8 GB recommended |

> Renzora is a Bevy 0.19 engine and uses WebGPU/`wgpu` for rendering. Very old GPUs without a Vulkan/Metal/DX12 backend are not supported.

## Download a prebuilt build (easiest)

Want the quickest start? Grab a prebuilt engine for your platform from the download page — no Docker, no Cargo, no terminal required:

**[renzora.com/download](/download)**

Each platform ships as a `.zip` archive built from the [GitHub releases](https://github.com/renzora/engine/releases) — download it, extract, and run the engine directly. Windows gets a folder of files, Linux a single `.AppImage`, macOS a `.app` bundle.

There is **one executable**, `renzora`, plus the plugins it loads. It runs as the editor when the editor image (`renzora_editor.dll` / `.so` / `.dylib`) is sitting beside it, and as your shipped game when it isn't — so "remove the editor" is deleting one file, and the binary you edit in is the same one your game runs on.

### Nightly builds

Alongside the numbered releases there are **nightlies** — an automated build of `main`, tagged like `r1-alpha8-nightly-16aug26` and marked as a prerelease. They are the right thing to run if you're developing *against* the engine and want the latest fixes; they are not the right thing to ship a game on. Nightlies are built every night there are new commits, and the last two weeks are kept.

### Windows

Download the Windows `.zip`, extract it anywhere, and double-click `renzora.exe`.

### macOS

Download the macOS `.zip` and extract it, then move the Renzora app to your Applications folder.

> On first launch macOS Gatekeeper may block an unsigned build. Right-click the app, choose **Open**, then confirm in the security dialog.

### Linux

Download the Linux `.zip` and extract it:

```bash
unzip linux-x64.zip
./"Renzora Engine-x86_64.AppImage"
```

## Keeping it up to date

The editor updates itself. **Help ▸ Check for Updates** downloads the new version and installs it in place; when a background check at startup has already found one, that menu item reads **Update to `r1-alpha8`** instead and a **New update available** chip appears in the top bar, so you don't have to go looking.

The dialog shows what you're running and what's available, and gives you one button that walks through Download → Install & Restart. **Release notes** opens the full notes for the selected version in your browser. The download is checksummed, and if anything goes wrong while the files are being replaced your existing install is put back — the worst case is that the update didn't happen.

**Install to** is where the new version lands. It shows the folder the editor is running from, which is what you want unless you're keeping more than one copy around; **Browse…** picks a different one. It's a picker rather than a text box on purpose — this path decides which directory gets *replaced*.

**Skip This Version** stops the top bar and the Help menu mentioning the version currently on offer. It's one version, not a mute button — the next release asks again — and the skipped version stays in the list, so downloading it later is still one click.

**Channel** picks what you get offered:

| | |
|---|---|
| **Auto** (default) | Follows what you're running: a nightly is offered newer nightlies, a release is offered releases. |
| **Stable** | Numbered `r1-alpha*` releases only. |
| **Nightly** | Dated builds of `main` only. Requires Developer Mode. |

> **Nightlies need Dev Mode.** With it off, every channel resolves to Stable, the Nightly chip is hidden, and the top bar stops mentioning nightly builds — a nightly is last night's `main`, and nothing should be nudging you onto one unless you asked. The switch sits right under the channel picker in the update dialog (it's the same flag as Settings ▸ Editor ▸ Dev Mode, so flipping either moves both). Your channel choice is remembered, so turning Dev Mode back on restores it.

> Running from a source checkout? Installing there replaces the `dist/` tree you just built — recoverable by rebuilding, but never something to do on one stray click, so the dialog makes you confirm twice and names the exact directory in between. Pointing **Install to** somewhere else drops the confirmation, since nothing you built is at risk. Usually you want `git pull` and a rebuild instead.

## Build from source (recommended)

Clone the repo and build it. That's the whole thing:

```bash
git clone https://github.com/renzora/engine.git
cd engine
cargo renzora           # build, stage, and launch the editor
```

You need [Rust](https://rustup.rs/) — `rust-toolchain.toml` pins the version, so rustup fetches the right one on first build. Nothing else.

`cargo renzora` compiles the workspace, arranges `dist/<platform>/` the way a shipped build is arranged, and launches it. The first build takes several minutes because Bevy is large; after that it's incremental. `cargo renzora dist` stages without launching.

### Scaffolding a new project

To start a game rather than hack on the engine, the `renzora` CLI clones the engine for you:

```bash
cargo install renzora
renzora new my-game
cd my-game
cargo renzora
```

> The CLI is the published [`renzora` crate](https://crates.io/crates/renzora). The crate *named* `renzora` inside the engine repo is a different thing (the SDK library), so `cargo install renzora` installs the CLI — not that library.

### Why not Docker?

Renzora used to tell you to build in a container, and there was a real reason: the plugin ABI depended on the build environment.

A plugin that shares the engine's compiled Bevy has to import it by an exact filename — `bevy_dylib-<hash>`, where the hash covers the feature set, profile, flags, target and compiler version. Build the engine differently and a downloaded plugin looks for a file that isn't there. "Match everyone else's environment" meant "use the container", and so the container was the install path.

[Standalone plugins](/docs/r1-alpha8/extending/standalone-plugins) don't link Bevy at all. They export one symbol and import nothing — the engine passes its interface *in* — so there's no filename to match and no environment to be canonical about. A plugin built with any Rust version on any machine loads into an engine built with any other.

That removed the last reason to containerise an ordinary build. Docker is still how you cross-compile for platforms you don't own, and still what CI runs, but it isn't how you install the engine.

One caveat if you're contributing: `cargo test` can't link the full workspace natively on Windows (the test harness blows past the PE format's 65,535-symbol export limit). Run the suite with `renzora test`, which uses the container. `cargo check` and `cargo clippy` work natively everywhere.

### Good to know: one binary, editor as a removable bundle

There is exactly one workspace binary: `renzora` (`renzora.exe` on Windows). The editor is **not** a compile-time feature — it ships as a removable bundle (`renzora_editor.dll` / `librenzora_editor.so` / `.dylib`) placed **beside the exe**:

- Bundle present → the binary launches as the **editor**.
- Delete that one file (or pass `--no-editor`) → the same binary is the **shipped game**.

You don't need the deeper details to get started — the cross-compile toolchain and every launch flag are covered in the build reference below and in the Advanced docs.

### Cross-compiling for other platforms

**This is what Docker is for.** Shipping a game means producing builds for machines you don't have — a macOS bundle from Windows, an Android APK from Linux. Each platform needs its own compiler, linker and system libraries, and the toolchain images carry them so you don't have to install six SDKs by hand.

Install [Docker](https://docs.docker.com/get-docker/) and the CLI, then:

```bash
cargo install renzora
renzora init                    # pull the toolchain images (first run is slow)
renzora build windows linux     # export templates land in dist/<platform>/
```

`renzora build [platforms...]` (no args = every platform) accepts `windows`, `linux`, `macos`, `wasm` (Web), `android`, and `ios`. The CLI pulls only the images for the platforms you name.

You do **not** need any of this to build and run Renzora on your own machine — `cargo renzora` already does that.

> The web build is **game-runtime only** — there is no WebAssembly editor. tvOS is **not** a supported target.

## What's next?

- [Core concepts](/docs/r1-alpha8/getting-started/concepts) — how scenes, entities, and scripts fit together
- [Your first project](/docs/r1-alpha8/getting-started/first-project) — build something in the editor
