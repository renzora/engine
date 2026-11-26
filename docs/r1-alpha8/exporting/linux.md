# Export: Linux

Build and package your Renzora game for 64-bit Linux desktop (`x86_64`).

## How Linux builds are produced

Renzora is **one binary** (`renzora`) that is either the editor or the shipped game depending on whether the removable editor bundle sits beside it. There is no per-platform "runtime build" toggle and no separate game executable — exporting for Linux means producing that binary plus its shared libraries and your packed assets.

Linux builds inside `ghcr.io/renzora/linux` (`docker/linux/Dockerfile`, `FROM base`). The image bundles both Linux GNU toolchains (so one build emits **x86_64 and arm64**), the `mold`/`lld` linkers, the cross-gcc for the non-host arch, and `appimagetool`, so the host needs only Docker + Git (Rust just to install the CLI):

```bash
renzora build linux
```

`renzora build [platforms...]` writes the Linux artifacts to **`dist/linux-x64/`** (it runs the `docker/build-all.sh` step inside the container).

### Why the build runs in Docker

Builds go through the `renzora` CLI even on a Linux host: a native `cargo`/`apt` build produces a mismatched engine build hash and breaks the dynamic-plugin ABI. So there is no `sudo apt install …`/`rustup` setup — the host needs only Docker + Git:

```bash
renzora build       # editor build: binary + editor bundle + shared bevy_dylib, into dist/linux-x64/
renzora run         # build, then launch the editor
```

## The output folder

After `renzora build linux`, the desktop lane builds the **editor** and wraps it into an AppDir (and an AppImage if `appimagetool` is available):

```
dist/linux-x64/
├── Renzora Engine.AppDir/
│   ├── AppRun                    # launcher; sets LD_LIBRARY_PATH
│   ├── renzora                   # the engine binary (no extension)
│   ├── librenzora.so             # SDK / contracts shared library
│   ├── librenzora_editor.so      # editor bundle — delete this to ship the game
│   ├── libbevy_dylib-<hash>.so   # shared Bevy
│   ├── libstd-<hash>.so          # matching Rust std
│   ├── renzora-engine.desktop
│   └── plugins/                  # distribution-plugin cdylibs (*.so)
└── Renzora Engine-x86_64.AppImage
```

The binary links its libraries by name, so the `.so` files must travel **beside** it (or on `LD_LIBRARY_PATH`). The generated `AppRun` does exactly that, then checks the system before starting the binary (see [Which systems a game runs on](#which-systems-a-game-runs-on)):

```bash
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
export LD_LIBRARY_PATH="$HERE:$HERE/plugins:${LD_LIBRARY_PATH:-}"
APP_NAME="My Game"
BIN="mygame"
# ... the Linux support check (crates/renzora_export/src/apprun_preflight.sh) ...
exec "$HERE/$BIN" "$@"
```

> `chmod +x` is applied by the build, but if you copy the binary around manually, re-run `chmod +x renzora`.

## Editor vs. shipped game

Renzora uses the "one binary, removable editor" model. The same `renzora` binary is the editor or the game depending on one file:

| State | Result |
|---|---|
| `librenzora_editor.so` present beside `renzora` | Launches as the **editor** |
| `librenzora_editor.so` deleted (or pass `--no-editor`) | The same binary is the **shipped game** |

So to turn the build above into a distributable game, delete `librenzora_editor.so` and keep the binary, `librenzora.so`, `libbevy_dylib-*.so`, `libstd-*.so`, and the `plugins/` folder.

## Packaging assets

Game assets ship as a single Renzora archive (`.rpak`, a Zstd-compressed v2 archive). At launch the binary looks for them in this order:

1. `--rpak <path>` command-line override
2. An `.rpak` **embedded** in the executable (appended with a footer)
3. An adjacent `<exe-stem>.rpak` — for a binary named `renzora`, that's `renzora.rpak`
4. The raw filesystem (an `assets/` folder next to the binary)

The adjacent-archive rule matches the **binary stem**: if you rename the binary to `mygame`, the archive must be `mygame.rpak`. A minimal Linux game folder therefore looks like:

```
mygame/
├── renzora                   # (rename to taste; rpak stem must match)
├── renzora.rpak              # your packed assets
├── librenzora.so
├── libbevy_dylib-<hash>.so
├── libstd-<hash>.so
└── plugins/
```

## AppImage

When `appimagetool` is on `PATH`, the Linux build wraps the output into a portable `Renzora Engine-x86_64.AppImage` from the AppDir shown above. The AppDir carries an `AppRun` (sets `LD_LIBRARY_PATH`), a `renzora-engine.desktop` entry, and `.DirIcon`/`renzora-engine.png` if an `icon.png` is present at build time.

```bash
# Reproduce the wrap manually from an existing AppDir:
ARCH=x86_64 appimagetool "Renzora Engine.AppDir" "Renzora Engine-x86_64.AppImage"
chmod +x "Renzora Engine-x86_64.AppImage"
./"Renzora Engine-x86_64.AppImage"
```

> The build's AppImage wrapping currently targets the **editor**. To produce a game AppImage, delete `librenzora_editor.so` from the AppDir first, add your `.rpak`, then run `appimagetool` as above. If `appimagetool` is missing, the build leaves the unwrapped `.AppDir` in place.

## Runtime requirements

A Linux machine running your game needs:

| Requirement | Notes |
|---|---|
| **GPU + Vulkan** | Renzora renders through `wgpu`; install Vulkan drivers (`mesa-vulkan-drivers` or your vendor's driver). |
| **X11 or Wayland** | `winit` auto-selects whichever is available; `libxkbcommon` / `libwayland` provide the client libs. |
| **ALSA / PulseAudio** | Needed by the audio backend; without them (or in a build made with `renzora_runtime`'s `audio` feature off) the game runs silently. |

## Which systems a game runs on

A Linux program needs at least the glibc of the machine that compiled it, so where your game was built decides which distros can start it.

| How you export | Built on | Runs on |
|---|---|---|
| **Binary + .rpak** or **Single executable** | The editor's prebuilt runtime, compiled on AlmaLinux 8 | glibc 2.28 and newer: Ubuntu 20.04+, Mint 20+, Debian 10+, Fedora 29+, RHEL 8+, Steam Deck, and current rolling distros |
| **Lean single binary** | Your machine (or the Docker toolchain, when exporting from another OS) | That machine's glibc and newer |

After a lean Linux build the export log says which glibc the game needs, and warns when that is newer than 2.28, naming the file that raised it. To reach older systems with a lean build, export from an older distro, for example AlmaLinux 8 or Ubuntu 20.04 in [Distrobox](https://distrobox.it).

Every exported AppImage checks the player's system before the game starts, against the glibc its own files actually need. A player on a system that is too old, a musl distro, NixOS without `nix-ld`, or one missing a library gets a dialog saying what is wrong instead of a game that silently does not start. The game binary repeats the checks it can do itself (display server, Vulkan, windowing libraries) when it is started without the AppImage. Point players at [Installing on Linux](/docs/r1-alpha8/getting-started/linux) for per-distro fixes. `RENZORA_SKIP_PREFLIGHT=1` turns the check off.

## Architecture

Linux builds for **x86_64** (`dist/linux-x64/`) and **ARM64** (`dist/linux-arm64/`). `renzora build linux` produces both from one container, compiling the other architecture with the image's cross-gcc; `linux-x64` or `linux-arm64` picks just one.

## Troubleshooting

| Issue | Fix |
|---|---|
| `error while loading shared libraries: librenzora.so` | The `.so` files must sit beside the binary or be on `LD_LIBRARY_PATH`. The AppImage/`AppRun` handles this automatically. |
| `Permission denied` running the binary | `chmod +x renzora` (or `chmod +x` the `.AppImage`). |
| No window / GPU not detected | Install Vulkan drivers (`mesa-vulkan-drivers` or proprietary). Renzora has no software-rendering fallback. |
| No audio | Install ALSA or PulseAudio runtime libraries. |
| Game can't find assets | Ensure `<binary-stem>.rpak` sits beside the binary, or pass `--rpak /path/to/game.rpak`. |
| Wayland glitches | Unset `WAYLAND_DISPLAY` to fall back to X11. |

## See also

- [Exporting overview](/docs/r1-alpha8/exporting/overview) — the shared build model across platforms
- [Installation](/docs/r1-alpha8/getting-started/installation) — building the engine from source and the cargo aliases
- [Installing on Linux](/docs/r1-alpha8/getting-started/linux): the per-distro setup to send your players to
- [Installing Docker](/docs/r1-alpha8/setup/docker): for exporting to Linux from another OS
