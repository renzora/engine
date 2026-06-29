# Export: macOS

Cross-compile your game to macOS (Intel and Apple Silicon) from the Renzora Docker image using osxcross — no Mac required to build.

## How macOS builds work

macOS targets are built by `renzora build` inside `ghcr.io/renzora/macos` (`docker/macos/Dockerfile`, `FROM base`). The image ships [osxcross](https://github.com/tpoechtrager/osxcross) with the macOS SDK and the two Rust Apple targets, so the build runs on a Linux host — you do **not** need a Mac to produce macOS binaries.

There are two macOS architectures, each built and emitted separately:

| Token | Rust target | Output directory |
|---|---|---|
| `macos` | both of the below | `dist/macos-x64/`, `dist/macos-arm64/` |
| `macos-x64` | `x86_64-apple-darwin` | `dist/macos-x64/` |
| `macos-arm64` | `aarch64-apple-darwin` | `dist/macos-arm64/` |

> macOS lanes run **only if osxcross is present** in the image. If it is missing, `build-all.sh` prints `WARN: osxcross not found, skipping macOS builds` and continues with the other platforms — it does not fail the build.

### Build command

From the repository root, with Docker installed:

```bash
# Both architectures
renzora build macos

# A single architecture
renzora build macos-arm64
renzora build macos-x64
```

Every argument is a platform token. Pass no tokens to build every platform the image supports; output lands in `dist/<platform>/`.

> `renzora build macos` derives the image tag, pulls it, and runs the `docker/build-all.sh` step inside the container — that wrapper is the documented build path; you never run cargo natively. See [Installation](/docs/r1-alpha5/getting-started/installation).

## Output layout

Unlike the WASM and mobile lanes (which nest under a `runtime/` subfolder), desktop builds place the binary **directly** in the platform directory:

```
dist/macos-arm64/
├── renzora                       # the engine binary
├── librenzora.dylib              # SDK / contracts (shared by host + plugins)
├── librenzora_editor.dylib       # editor bundle (delete to ship the game)
├── libbevy_dylib-<hash>.dylib    # shared Bevy 0.19 dylib
├── libstd-<hash>.dylib           # matching Rust std
└── plugins/
    └── lib<plugin>.dylib         # distribution-plugin cdylibs
```

`dist/macos-x64/` has the identical layout for the Intel target. macOS dynamic libraries use the `lib` prefix and the `.dylib` extension.

## Editor vs. shipped game

Renzora is **one binary** whose role is decided at runtime by what sits beside it — the same model as every other platform:

- `librenzora_editor.dylib` present → the binary launches as the **editor**.
- Delete that one file (or launch with `--no-editor`) → the same binary is the **shipped game**.

So to turn a macOS build into a distributable game, remove the editor bundle and ship the rest of the directory:

```bash
rm dist/macos-arm64/librenzora_editor.dylib
```

The remaining `renzora` binary, the SDK/Bevy/std dylibs, and the `plugins/` folder are what your players run. Keep the dylibs next to the binary — they are resolved relative to the executable's directory.

> Game assets are loaded through the VFS: an `.rpak` embedded in the binary, an adjacent `renzora.rpak`, or a loose `assets/` directory next to the binary. See [Asset Packing (rpak)](/docs/r1-alpha5/packaging/asset-packing) for packing assets into a single archive for distribution.

## Toolchain and compatibility

| | Detail |
|---|---|
| **Cross toolchain** | osxcross (clang) with the macOS SDK baked into the Docker image |
| **Rust targets** | `x86_64-apple-darwin` (Intel), `aarch64-apple-darwin` (Apple Silicon) |
| **Minimum macOS** | `11.0` Big Sur (`MACOSX_DEPLOYMENT_TARGET` in the image) |
| **Rendering** | Metal, via `wgpu` |
| **Universal binary** | Not produced — each architecture is a separate directory |

`build-all.sh` emits two per-architecture binaries; it does **not** `lipo` them into a single universal (fat) binary. If you want one, combine them on a Mac:

```bash
lipo -create -output renzora-universal \
    dist/macos-x64/renzora \
    dist/macos-arm64/renzora
```

## Distributing to other Macs

The macOS lane produces a **plain directory** of a binary plus dynamic libraries — it does not generate an `.app` bundle, an `Info.plist`, a code signature, or a `.dmg`. (Only the Linux editor lane wraps its output, into an AppImage.) Bundling, signing, notarizing, and disk-image packaging are macOS-side steps you perform yourself, on a Mac, if you distribute beyond your own machine.

Because the cross-compiled binaries are **unsigned**, Gatekeeper blocks them on other Macs until you either sign/notarize them or clear the quarantine attribute:

```bash
# Quick local workaround for an unsigned build
xattr -cr /path/to/renzora
```

If you have an Apple Developer account, the standard distribution flow on a Mac is `codesign` → `notarytool submit` → `stapler staple`. These are Apple tools, not part of the Renzora build; see Apple's developer documentation for the current commands.

## Troubleshooting

| Issue | Cause / fix |
|---|---|
| `WARN: osxcross not found, skipping macOS builds` | The container has no osxcross. Use the official `ghcr.io/renzora/macos` image, which bundles it. |
| `"<app> is damaged and can't be opened"` | Unsigned cross-built binary flagged by Gatekeeper. Run `xattr -cr <binary>`, or sign and notarize on a Mac. |
| `dyld: Library not loaded: ...dylib` | A required dylib is missing. Ship the whole platform directory — keep `librenzora.dylib`, `libbevy_dylib-*.dylib`, `libstd-*.dylib`, and `plugins/` next to the binary. |
| Editor launches instead of the game | `librenzora_editor.dylib` is still present. Delete it, or run with `--no-editor`. |
| Won't launch on older macOS | The build floor is macOS 11.0 Big Sur. |
| Slow first launch | Metal shaders compile on first run — expected, and cached afterward. |
