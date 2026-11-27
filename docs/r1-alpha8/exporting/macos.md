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

> `renzora build macos` derives the image tag, pulls it, and runs the `docker/build-all.sh` step inside the container — that wrapper is the documented build path; you never run cargo natively. See [Installation](/docs/r1-alpha8/getting-started/installation).

## Output layout

Unlike the WASM and mobile lanes (which nest under a `runtime/` subfolder), desktop builds place the binary **directly** in the platform directory:

```
dist/macos-arm64/
├── renzora                       # the engine binary
├── librenzora.dylib              # SDK / contracts (shared by host + plugins)
├── librenzora_editor.dylib       # editor bundle (delete to ship the game)
├── libbevy_dylib-<hash>.dylib    # shared Bevy 0.20 dylib
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

> Game assets are loaded through the VFS: an `.rpak` embedded in the binary, an adjacent `renzora.rpak`, or a loose `assets/` directory next to the binary. See [Asset Packing (rpak)](/docs/r1-alpha8/packaging/asset-packing) for packing assets into a single archive for distribution.

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

### Exporting from the editor (on a Mac)

The editor's **Export** dialog produces a ready-to-run game. A new macOS export preset wraps the output in a `.app` (the **Wrap as a .app bundle** switch; presets you saved before this default keep their old setting), and every macOS export is **code-signed** as its final step. The signing is what lets the game launch at all: a copy-mode export ships the runtime template, whose signature belongs to the editor's own bundle and which Apple Silicon refuses to run anywhere else, so an unsigned export shows "the application can't be opened" the instant you double-click it.

Inside a `.app`, all three packaging modes put the game data (`<game>.rpak`) beside the executable in `Contents/MacOS/`, including **Single executable** and **Lean single binary**. The bundle is already one file to the player, and `codesign` refuses an executable with data appended to it, so a bundle built that way could not be signed. Outside a `.app`, the single-file modes still append the data; the export signs the executable around it, which launches fine on a Mac but cannot be notarized, so distribute the `.app`.

**Zip the `.app` with Finder (right-click -> Compress) or `ditto -c -k --keepParent`**, not a plain `zip`, `tar` or git. The seal on the data files in `Contents/MacOS/` is stored in extended attributes, and tools that drop them leave a bundle macOS reports as damaged.

Which tier you get depends only on what you have configured:

- **No Apple account (the default): ad-hoc signed.** The `.app` runs on your machine and on any Mac it reaches without the download quarantine (a USB stick, AirDrop, itch's app, Steam). A copy a player *downloads* is quarantined and meets Gatekeeper, which they clear once with right-click -> Open (or System Settings -> Privacy & Security -> "Open Anyway"). This is as far as a free account goes, and it is enough for testing and for launchers that do not quarantine.
- **Developer ID + notarization: opens with a clean double-click anywhere.** Set these in the environment before launching the editor, and the export signs with your identity, submits to Apple's notary service, and staples the ticket:

  ```bash
  export RENZORA_CODESIGN_IDENTITY="Developer ID Application: You (TEAMID)"
  export RENZORA_NOTARY_KEY="/path/to/AuthKey_XXXXXXXXXX.p8"
  export RENZORA_NOTARY_KEY_ID="XXXXXXXXXX"
  export RENZORA_NOTARY_ISSUER="your-issuer-uuid"
  ```

  With only `RENZORA_CODESIGN_IDENTITY` set, the export is signed with your Developer ID but not notarized, so a download still warns. Notarization staples its ticket to the `.app`, so keep **Wrap as a .app bundle** on. The identity lives in your login keychain, and the export unlocks it once per signing call; choose **Always Allow** on the keychain prompt (or allow `codesign` in the key's Access Control) and it stops asking. Signing happens only when you export **from a Mac**: `codesign`, `notarytool` and `stapler` do not exist on other hosts, and a macOS game cross-exported from Linux or Windows is left unsigned with a note saying so.

**Include server** writes `server.rpak` and `server.sh` beside the `.app`; the script runs the game binary inside the bundle in server mode.

### `renzora build` (Docker cross-compile)

The Docker lane is a different path, and it produces a **plain, unsigned directory** of a binary plus dynamic libraries: no `.app`, no `Info.plist`, no signature. Bundling and signing are macOS-side steps you run on a Mac, the same `codesign` -> `notarytool submit` -> `stapler staple` the editor's export automates. For a quick local run of an unsigned cross-built binary, clear the quarantine:

```bash
xattr -cr /path/to/renzora
```

## Troubleshooting

| Issue | Cause / fix |
|---|---|
| `WARN: osxcross not found, skipping macOS builds` | The container has no osxcross. Use the official `ghcr.io/renzora/macos` image, which bundles it. |
| `"<app> is damaged and can't be opened"` / `"the application can't be opened"` / `zsh: killed` | An unsigned binary. An export from the editor on a Mac is signed automatically; this is a cross-built (`renzora build`) or hand-assembled binary. Run `xattr -cr <binary>`, or sign and notarize on a Mac. |
| `dyld: Library not loaded: ...dylib` | A required dylib is missing. Ship the whole platform directory — keep `librenzora.dylib`, `libbevy_dylib-*.dylib`, `libstd-*.dylib`, and `plugins/` next to the binary. |
| Editor launches instead of the game | `librenzora_editor.dylib` is still present. Delete it, or run with `--no-editor`. |
| Won't launch on older macOS | The build floor is macOS 11.0 Big Sur. |
| Slow first launch | Metal shaders compile on first run — expected, and cached afterward. |
