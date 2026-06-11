# iOS

Ship your game to iPhone and iPad by cross-compiling the Renzora runtime to a static library and linking it into a small Xcode app.

## How iOS export works

iOS is not built like the desktop targets. There is no single executable: instead the `renzora_ios` crate (package `renzora-ios`) compiles to a **static library**, `librenzora_ios.a`, which is linked into a thin UIKit app shell. That shell launches Bevy, which drives a Metal surface through `winit`/`wgpu`.

The static library exports exactly one C symbol, defined in `crates/renzora_ios/src/lib.rs`:

```rust
#[unsafe(no_mangle)]
pub extern "C" fn renzora_main() {
    let mut app = renzora_runtime::build_runtime_app();
    app.run();
}
```

The bundled Swift `AppDelegate` calls it once UIKit is ready:

```swift
import UIKit

@_silgen_name("renzora_main")
func renzora_main()

@main
class AppDelegate: UIResponder, UIApplicationDelegate {
    var window: UIWindow?

    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
    ) -> Bool {
        // Bevy/winit creates its own UIWindow via the Metal backend.
        renzora_main()
        return true
    }
}
```

| Piece | What it is |
|---|---|
| `renzora-ios` crate | `crate-type = ["staticlib"]`, depends on `renzora_runtime` + `bevy` |
| `librenzora_ios.a` | the compiled static library you link into Xcode |
| `renzora_main()` | the single `extern "C"` entry point the app shell calls |
| `templates/ios/` | the Xcode app shell (`RenzoraRuntime.xcodeproj`, `AppDelegate.swift`, `Info.plist`, `LaunchScreen.storyboard`) |
| Renderer | `wgpu` → **Metal**, on a UIKit `UIWindow` managed by `winit` |

> An iOS build is **runtime only** — it links `renzora_runtime`, never the editor bundle. The editor is a desktop-only removable cdylib and does not run on iOS.

> **tvOS / Apple TV is not supported.** See [tvOS is not supported](#tvos-is-not-supported) below before you plan around it.

## Prerequisites

- **A Mac with Xcode** — required to assemble and code-sign the final `.app`. The Linux build container can produce the static library, but only macOS + Xcode can build and sign the app bundle.
- **Apple Developer account** — needed to run on a physical device and to publish to the App Store.
- **Rust iOS targets** — install them with `rustup` if you build the library on a Mac:

```bash
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
```

`aarch64-apple-ios` is for real devices; `aarch64-apple-ios-sim` is the Apple Silicon simulator. These are the only two iOS targets the project supports.

## Building the static library

There are two ways to produce `librenzora_ios.a`.

### In the build container (recommended)

The Docker image (`ghcr.io/renzora/engine`) bundles the iOS SDK and toolchain, so a single command cross-compiles the library from any host:

```bash
docker/build-all.sh dist ios
```

This runs the iOS lane and writes the result to:

```
dist/ios-arm64/runtime/librenzora_ios.a
```

> The iOS lane is **best-effort**: in `build-all.sh` it is marked `optional`, so if the iOS cross-compile fails it logs a warning and the rest of the build still succeeds. The container builds the **device** library (`aarch64-apple-ios`) only — it cannot produce a signed `.app`, and there is no simulator lane.

### Directly with Cargo (on a Mac)

The repo ships cargo aliases in `.cargo/config.toml`:

```bash
cargo build-ios        # build --profile dist --target aarch64-apple-ios -p renzora-ios
cargo build-ios-sim    # build --profile dist --target aarch64-apple-ios-sim -p renzora-ios
```

The artifact lands under `target/<triple>/dist/librenzora_ios.a`.

## Assembling the app

The Xcode shell lives in `templates/ios/`. On a Mac, `templates/ios/build-template.sh` cross-compiles the library, links it into the Xcode project, and packages the resulting `RenzoraRuntime.app` as a `.zip` template:

```bash
# iOS device (arm64)
templates/ios/build-template.sh

# iOS simulator (arm64, Apple Silicon)
templates/ios/build-template.sh --simulator
```

The script requires macOS with the Xcode command-line tools (`xcrun`, `xcodebuild`) and the matching Rust target installed. It targets **iOS 16.0** by default (`IPHONEOS_DEPLOYMENT_TARGET=16.0`) and produces an **unsigned** bundle (`CODE_SIGNING_ALLOWED=NO`) — you sign it afterward in Xcode for device testing or distribution.

To turn the template into your actual game, export from the editor: the export step injects your packed game assets into the bundle (and is where you configure signing for distribution).

## Game assets in the bundle

At startup the engine's VFS (`renzora_engine/vfs.rs`) looks for a packed archive named `game.rpak` in the app bundle's resource directory (resolved via `CFBundleCopyResourceURL`). Place your exported `game.rpak` in the app's `Resources` so the shipped game can read its scenes, scripts, and assets. See [Asset Packing (rpak)](../packaging/asset-packing.md) for how the archive is produced.

## Bundle configuration

The template's `Info.plist` declares the device requirements and orientation support. The relevant keys:

| Key | Value | Notes |
|---|---|---|
| `CFBundleIdentifier` | `$(PRODUCT_BUNDLE_IDENTIFIER)` | set your reverse-DNS bundle ID in Xcode |
| `CFBundleDisplayName` | `Renzora Runtime` | the name shown on the home screen — rename for your game |
| `UIRequiredDeviceCapabilities` | `arm64`, `metal` | the engine needs a 64-bit Metal-capable device |
| `UISupportedInterfaceOrientations` | portrait + landscape (left/right) | iPad adds upside-down portrait |
| `UILaunchStoryboardName` | `LaunchScreen` | the bundled launch storyboard |
| `UIStatusBarHidden` / `UIRequiresFullScreen` | `true` | runs full-screen with no status bar |

## Signing and distribution

The template is unsigned, so signing happens in Xcode:

1. Open `RenzoraRuntime.xcodeproj` (or your exported project) in Xcode.
2. Under **Signing & Capabilities**, pick your team and set the bundle identifier. Automatic signing is simplest for development.
3. Select a connected device or a simulator and build (Cmd+B).
4. For the App Store, choose **Product → Archive**, then **Distribute App → App Store Connect**.

> On a physical device you may see "Untrusted Developer" the first time. Trust the profile under **Settings → General → VPN & Device Management** on the device.

## Input on iOS

Touch and motion arrive through Bevy's standard `winit` input events. Build your on-screen controls (virtual joysticks, buttons) with the [Game UI](../scripting/game-ui.md) system, which renders `.html` markup as real `bevy_ui` widgets and routes presses to your scripts via the `on_ui` hook. MFi / Bluetooth game controllers are delivered as ordinary Bevy gamepad input.

## Performance notes

- Apple GPUs are strong; most post-process effects run well. Target **60 FPS** (120 FPS on ProMotion devices is a bonus).
- Metal is the only backend — `wgpu` targets Metal on Apple platforms automatically; there is no Vulkan/DX path here.
- Watch **thermal throttling**: sustained GPU load makes the device down-clock. Test on the oldest hardware you intend to support.

## Troubleshooting

| Issue | Solution |
|---|---|
| Linker can't find `librenzora_ios.a` | Confirm `LIBRARY_SEARCH_PATHS` points at the directory holding the `.a`, and that it matches the build SDK (device vs simulator) |
| "Untrusted Developer" on device | Trust the profile under Settings → General → VPN & Device Management |
| Signing errors | Make sure the provisioning profile matches your bundle ID and registered device |
| Simulator won't launch | Build for `aarch64-apple-ios-sim` on an Apple Silicon Mac; the device `.a` won't run in the simulator |
| Black screen on launch | Check the device is Metal-capable and meets the deployment target |
| Game has no content | Ensure `game.rpak` is bundled in the app's `Resources` directory |

## tvOS is not supported

Despite some leftover references in the repo, **tvOS / Apple TV cannot be built today** and is not a supported export target:

- The build container (`docker/Dockerfile`) installs only `aarch64-apple-ios` and `aarch64-apple-ios-sim`. **No tvOS rustup target is installed.**
- `docker/build-all.sh` has **no tvOS lane** — only `ios`.
- The orphaned `cargo build-tvos` / `cargo build-tvos-sim` aliases and the `--tvos` flags in `templates/ios/build-template.sh` target `aarch64-apple-tvos`, which the toolchain cannot compile.

These are aspirational placeholders. Treat iOS (iPhone/iPad) as the only Apple mobile target until tvOS toolchain support actually lands.
