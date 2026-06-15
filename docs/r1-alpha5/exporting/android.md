# Export: Android

Build your game's runtime for Android phones, tablets, and Fire TV as a native `libmain.so` and package it into an APK.

> ⚠️ **Android exports the game runtime only — never the editor.** The editor ships as a desktop `renzora_editor` dlopen bundle and has no Android equivalent. The Android lane is also **best-effort** in the container build: if it fails it logs a warning and does not fail the rest of the build.

## How an Android build is put together

There are three moving parts, and it helps to keep them straight:

1. **`renzora_android`** (package `renzora-android`) is a tiny `cdylib` crate whose library name is `main`, so it compiles to **`libmain.so`**. Its entire source is the runtime entry point:

   ```rust
   // crates/renzora_android/src/lib.rs
   use bevy::prelude::*;

   #[bevy_main]
   fn main() {
       let mut app = renzora_runtime::build_runtime_app();
       app.run();
   }
   ```

2. **The Gradle template** in `templates/android/` wraps that `.so` in an APK. The Android `GameActivity` (`com.google.androidgamesdk.GameActivity`) loads the library named `main` at launch:

   ```xml
   <!-- templates/android/app/src/main/AndroidManifest.xml -->
   <meta-data android:name="android.app.lib_name" android:value="main" />
   ```

3. **Your game content** (scenes, scripts, textures, materials) is delivered as a Renzora **`.rpak`** archive bundled into the APK's assets. At startup the engine's VFS finds the rpak among the APK assets and runs your project from it — there is no loose `assets/` folder on the device.

## What you need

You can build either inside the engine's Docker image (no Android SDK on your machine) or locally with Android Studio.

### Container route (recommended)

The `ghcr.io/renzora/engine` image (`docker/Dockerfile`, `FROM rust:1.93.0-bookworm`) already bundles everything: the `aarch64-linux-android` and `x86_64-linux-android` rustup targets and **Android NDK r27c**. Your host only needs Docker. This route produces the native `libmain.so`; it does **not** assemble the APK (Gradle/Android SDK live outside the container).

### Local route (Android Studio + cargo-ndk)

To assemble an actual APK on your own machine, install once:

- **Android Studio** (provides the Android SDK, NDK, and a bundled JDK/JBR — Android Gradle Plugin 8.7.0 needs JDK 17+).
- **cargo-ndk**: `cargo install cargo-ndk`
- The Rust Android targets (the template uses the nightly toolchain):

  ```bash
  rustup target add aarch64-linux-android --toolchain nightly
  rustup target add x86_64-linux-android --toolchain nightly
  ```

The template auto-detects `JAVA_HOME`, `ANDROID_HOME`, and the newest installed NDK under `$ANDROID_HOME/ndk`; set those env vars only if detection fails.

> `renzora build android` (the CLI) orchestrates these steps inside the container — that is the documented build path.

## Building the native library

### In the container

`renzora build` accepts the platform tokens `android` (both arches), `android-arm64`, and `android-x86` (which builds the **x86_64** ABI, for emulators):

```bash
# Build both Android architectures inside the engine image
renzora build android
```

It copies the resulting libraries into arch-suffixed, `runtime/`-nested directories:

| Platform token | Rust target | Android ABI | Output |
|---|---|---|---|
| `android-arm64` | `aarch64-linux-android` | `arm64-v8a` | `dist/android-arm64/runtime/libmain.so` |
| `android-x86` | `x86_64-linux-android` | `x86_64` | `dist/android-x86/runtime/libmain.so` |

Under the hood each arch is just:

```bash
cargo build --profile dist -p renzora-android \
    --target aarch64-linux-android --target-dir target/android
```

### Locally

`renzora build android` runs the cross-compile inside the container, which owns the NDK linker setup. For a hand-assembled APK outside the container, use **cargo-ndk**, which the template invokes for you (see below) — it sets up the correct NDK toolchain and `--platform` (min SDK) for each ABI.

## Packaging into an APK

`templates/android/build-template.sh` does the full job: build the `.so` with `cargo-ndk`, drop it into `jniLibs/<abi>/` alongside the NDK's `libc++_shared.so`, then run Gradle.

```bash
# from the engine repo root
./templates/android/build-template.sh             # arm64-v8a (default)
./templates/android/build-template.sh --x86_64    # x86_64 (emulator)
./templates/android/build-template.sh --firetv    # Fire TV / Android TV (arm64)
./templates/android/build-template.sh --all        # all of the above
```

Each run emits an **unsigned** release APK and copies it to `target/templates/` (and to `~/.config/renzora/templates/`, or `%APPDATA%/renzora/templates` on Windows):

- `renzora-runtime-android-arm64.apk`
- `renzora-runtime-android-x86_64.apk`
- `renzora-runtime-firetv-arm64.apk`

These are **runtime shell APKs**. The editor's export step is what bundles your project's `.rpak` into one and signs it, producing an installable game APK.

### App configuration

The Gradle config (`templates/android/app/build.gradle.kts`) ships these defaults — edit them for your own title before packaging:

| Setting | Default | Notes |
|---|---|---|
| `applicationId` / `namespace` | `com.renzora.runtime` | Your unique package id |
| `compileSdk` | `34` | Android 14 |
| `minSdk` | `30` | Android 11 — oldest supported |
| `targetSdk` | `34` | Android 14 |
| `versionCode` | `1` | Integer; increment each store upload |
| `versionName` | `0.2.0` | User-facing version string |
| `abiFilters` | `arm64-v8a`, `x86_64` | ABIs packed into the APK |
| Activity | `com.google.androidgamesdk.GameActivity` | from `androidx.games:games-activity` |

### Build flavors

The template defines two product flavors (dimension `device`):

| Flavor | Target | What differs |
|---|---|---|
| `standard` | Phones & tablets | Standard `LAUNCHER` intent |
| `firetv` | Amazon Fire TV / Android TV | `LEANBACK_LAUNCHER`, forced `landscape`, touchscreen marked optional, `applicationIdSuffix = .firetv` |

## Signing

The Gradle template produces `app-<flavor>-release-unsigned.apk`. The editor's export signs it for you. To sign manually, use the standard Android toolchain (this is plain Android, not an engine feature):

```bash
# create a release keystore once — keep it safe, you can't update the app without it
keytool -genkey -v -keystore release.keystore -alias mykey \
    -keyalg RSA -keysize 2048 -validity 10000

# sign and align
apksigner sign --ks release.keystore --out my_game.apk \
    app-standard-release-unsigned.apk
```

## Rendering

Renzora renders on Android through **wgpu's Vulkan backend**. The manifest declares OpenGL ES 3.0 as the minimum device feature, but the engine itself uses Vulkan; devices without a working Vulkan driver will not run the runtime.

## Scripting on Android

Android is a **native** target, so **both** scripting backends are available — `.lua` (mlua / Lua 5.4) and `.rhai` (Rhai). This is unlike the web export, where Lua is dropped and only Rhai runs. Remember that Rhai is a **subset** of the Lua surface (only the `props`, `on_ready`, and `on_update` hooks, ~45 of the ~70 globals). See the [Scripting API](../api/scripting) for the full list.

## Input

The engine consumes Android touch and gamepad events through Bevy/winit. The scripting **context globals** are the mouse/keyboard/gamepad set (`mouse_x`, `mouse_left`, `input_x`/`input_y`, `gamepad_*`, the `is_key_*` helpers) — there is no separate per-finger touch API exposed to scripts today. For on-screen buttons and virtual sticks, author a HUD with the markup UI (`renzora_ember` `.html` templates) and wire its `on_press`/`on_change` events to your scripts.

## Troubleshooting

| Symptom | Cause / fix |
|---|---|
| `cargo-ndk not found` | `cargo install cargo-ndk` (local route only). |
| `No NDK found` / linker errors | Install the NDK via Android Studio's SDK Manager, or set `ANDROID_NDK_HOME`. The container already has NDK r27c. |
| `libmain.so` missing after a container build | The Android lane is best-effort — scroll up for its `WARN: Android … build failed` line and the real cargo error. |
| App installs but crashes/black-screens immediately | Almost always no usable Vulkan driver, or a missing `libc++_shared.so` — the template copies it next to `libmain.so`; a hand-rolled APK must include it. |
| Game launches but has no content | The project `.rpak` wasn't bundled into the APK assets. Use the editor export (or pack the rpak) rather than installing a bare template shell. |
