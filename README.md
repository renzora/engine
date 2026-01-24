# Renzora Engine

A game engine built on [Bevy](https://bevyengine.org/) with a visual editor for creating 3D games.

## Prerequisites

1. **Install Rust** from [rustup.rs](https://rustup.rs/) (this gives you `rustup`, `cargo`, and `rustc`)
2. Windows 10/11, Linux, or macOS

### Optional (Faster Linking)

For faster compile times, add the LLD linker:

**Windows:**
```bash
rustup component add llvm-tools-preview
```

**Linux:**
```bash
sudo apt install lld clang  # Ubuntu/Debian
```

Then add `.cargo/config.toml` to the project (see Configuration section).

## Building

### Editor (Development)

The editor uses dynamic linking for fast iteration:

```bash
cargo run --features editor
```

### Runtime (Release)

The runtime must be statically linked for distribution. Use a separate target directory to avoid cargo reusing the editor's dynamic-linked Bevy:

**Windows (PowerShell):**
```powershell
$env:CARGO_TARGET_DIR="target-runtime"; cargo build --release --features runtime --bin renzora_runtime
cp target-runtime/release/renzora_runtime.exe runtimes/windows/
```

**Windows (Command Prompt):**
```cmd
set CARGO_TARGET_DIR=target-runtime && cargo build --release --features runtime --bin renzora_runtime
copy target-runtime\release\renzora_runtime.exe runtimes\windows\
```

**Linux/macOS:**
```bash
CARGO_TARGET_DIR=target-runtime cargo build --release --features runtime --bin renzora_runtime
cp target-runtime/release/renzora_runtime runtimes/linux/  # or runtimes/macos/
```

**Why a separate target directory?** The editor uses `bevy/dynamic_linking` for fast builds. Cargo caches this and would reuse it for the runtime, resulting in a tiny (~1.5MB) binary that crashes. The separate directory forces a clean static build (~50MB).

## Project Structure

```
renzora/
├── src/
│   ├── main.rs              # Editor entry point
│   ├── runtime/
│   │   └── main.rs          # Runtime entry point
│   ├── export/              # Game export/packaging
│   ├── core/                # Core editor resources
│   ├── ui/                  # Editor UI panels
│   └── shared/              # Shared code (editor + runtime)
├── runtimes/
│   └── windows/
│       └── renzora_runtime.exe  # Pre-built runtime for exports
├── assets/                  # Editor assets
└── Cargo.toml
```

## Features

| Feature | Description |
|---------|-------------|
| `editor` | Full editor with UI, asset browser, scene editing (default) |
| `runtime` | Minimal runtime for exported games |

## Export System

Renzora packages games into a single executable:

- **Pack Format (RPCK v2):** Custom binary format with file table and zstd compression
- **Asset Discovery:** Automatically finds all assets referenced by your scenes
- **Compression:** zstd level 3, skips already-compressed formats (PNG, JPG, MP3, GLB)
- **Single File:** Runtime + assets appended into one executable

### Pack Format Structure

```
HEADER (28 bytes):
  Magic: "RPCK" (4 bytes)
  Version: u32 (4 bytes)
  Header Size: u32 (4 bytes)
  Flags: u32 (4 bytes)
  File Count: u32 (4 bytes)
  Data Offset: u64 (8 bytes)

FILE TABLE (per file):
  Path Length: u32
  Path: UTF-8 string
  Offset: u64
  Size: u64 (original)
  Compressed Size: u64
  Flags: u32 (bit 0 = compressed)

DATA SECTION:
  Compressed/raw file contents

FOOTER (12 bytes):
  Pack Start Offset: u64
  Magic: "RPCK" (4 bytes)
```

## Configuration

### Cargo.toml Profiles

The project uses Bevy's recommended optimization settings:

```toml
[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 3

[profile.release]
lto = "thin"
```

### Faster Linking (Optional)

Create `.cargo/config.toml`:

```toml
# Windows
[target.x86_64-pc-windows-msvc]
linker = "rust-lld.exe"

# Linux
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=lld"]
```

## Troubleshooting

### Runtime crashes immediately

Run from terminal to see error messages:
```bash
cd export_folder
./YourGame.exe
```

The runtime will show crash details and wait for Enter before closing.

### Small runtime binary (~1.5MB)

This means Bevy was compiled with dynamic linking. Use a separate target directory:
```powershell
$env:CARGO_TARGET_DIR="target-runtime"; cargo build --release --features runtime --bin renzora_runtime
```

The correct size should be ~50MB (statically linked Bevy).

### Export shows "Runtime not found"

Ensure the runtime binary exists at:
```
runtimes/windows/renzora_runtime.exe
```

Build it with the commands in the "Runtime (Release)" section above.

## License

Apache License (see LICENSE.md)
Version 2.0, January 2004
http://www.apache.org/licenses/