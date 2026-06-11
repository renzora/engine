# Export: Linux

Build and package your Renzora game for 64-bit Linux desktop (`x86_64`).

## How Linux builds are produced

Renzora is **one binary** (`renzora`) that is either the editor or the shipped game depending on whether the removable editor bundle sits beside it. There is no per-platform "runtime build" toggle and no separate game executable — exporting for Linux means producing that binary plus its shared libraries and your packed assets.

Every cross-platform target is built inside the engine's Docker image, `ghcr.io/renzora/engine` (`docker/Dockerfile`, `FROM rust:1.93.0-bookworm`). The image bundles the `x86_64-unknown-linux-gnu` toolchain, the `mold`/`lld` linkers, and `appimagetool`, so the host only needs Docker:

```bash
docker/build-all.sh dist linux
```

`build-all.sh <output-dir> [platforms...]` writes the Linux artifacts to **`dist/linux-x64/`**. Because the build host *is* Linux, this lane compiles natively (no cross toolchain), so on a Linux machine with a Rust toolchain you can also build straight from `cargo` (see below).

> `renzora build linux` (the CLI) runs exactly this `docker/build-all.sh` step inside the container. From a checkout you can also call `docker/build-all.sh` or the `.cargo/config.toml` aliases directly — and since the host is Linux, build natively with `cargo`.

### Building from source on a Linux host

A native Linux build needs the usual Bevy system libraries:

```bash
sudo apt install build-essential pkg-config libasound2-dev libudev-dev libxkbcommon-dev libwayland-dev
```

Then use the cargo aliases shipped in `.cargo/config.toml`:

```bash
cargo build-all       # editor build: binary + editor bundle + shared bevy_dylib
cargo build-runtime   # lean game binary only (--bin renzora, no editor bundle)
```

`cargo build-all` and `cargo build-runtime` write to `target/dist/`. `build-runtime` produces just the game-shaped binary; `build-all` additionally produces the editor bundle.

## The output folder

After `docker/build-all.sh dist linux`, the desktop lane builds the **editor** and wraps it into an AppDir (and an AppImage if `appimagetool` is available):

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

The binary links its libraries by name, so the `.so` files must travel **beside** it (or on `LD_LIBRARY_PATH`). The generated `AppRun` does exactly that:

```bash
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
export LD_LIBRARY_PATH="$HERE:$HERE/plugins:${LD_LIBRARY_PATH:-}"
exec "$HERE/renzora" "$@"
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
| **ALSA / PulseAudio** | Audio (Kira) is native-only; without ALSA/PulseAudio the game runs silently. |

## Architecture

The official pipeline targets **`x86_64` only**. There is no `aarch64` Linux lane in `build-all.sh` and no ARM Linux toolchain in the Docker image, so 64-bit ARM Linux (Raspberry Pi, ARM servers) is **not a supported export target** today. The `linux` platform token always maps to `dist/linux-x64/`.

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

- [Exporting overview](/docs/r1-alpha5/exporting/overview) — the shared build model across platforms
- [Installation](/docs/r1-alpha5/getting-started/installation) — building the engine from source and the cargo aliases
