# Renzora Engine - Developer Notes

## Project Overview

Renzora is a game engine built on Bevy with a visual editor for creating 3D games. It consists of two main binaries:
- **Editor** (`renzora_editor`) - Full editor with UI, asset browser, scene editing
- **Runtime** (`renzora_runtime`) - Minimal runtime for exported games

## Architecture

### Shared Code Pattern

The editor and runtime share spawning logic to ensure features work consistently in both:

```
src/shared/
├── mod.rs           # Re-exports shared items
├── components.rs    # Shared component types (CameraNodeData, MeshNodeData, etc.)
├── scene_format.rs  # Scene serialization format (NodeData, SceneData)
└── spawner.rs       # Core spawning logic used by both editor and runtime
```

**Key function:** `spawn_node_components()` in `src/shared/spawner.rs`
- Handles all node types: cameras, meshes, lights, physics bodies, collision shapes
- Called by both editor and runtime when spawning entities from scene data
- When adding new node types, add them here so they work in both editor and exported games

### Runtime Loader

The runtime loader (`src/runtime/loader.rs`) uses the shared spawner:
```rust
fn spawn_node_recursive(...) {
    // Create base entity
    let mut entity_commands = commands.spawn((transform, Visibility::default(), Name::new(...)));

    // Use shared spawner for type-specific components
    spawn_node_components(&mut entity_commands, node, meshes, materials, Some(&asset_server), &config);

    // Recursively spawn children
    for child in &node.children {
        spawn_node_recursive(..., child, Some(entity));
    }
}
```

### Export System

Games are packaged into a single executable using the RPCK v2 format:

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
  Compressed/raw file contents (zstd level 3)

FOOTER (12 bytes):
  Pack Start Offset: u64
  Magic: "RPCK" (4 bytes)
```

Files that skip compression: PNG, JPG, JPEG, MP3, OGG, GLB, GLTF (already compressed)

### Build Commands

**Editor (development):**
```bash
cargo run --features editor
```

**Runtime (for export):**
```powershell
# Windows PowerShell - use separate target dir to force static linking
$env:CARGO_TARGET_DIR="target-runtime"; cargo build --release --features runtime --bin renzora_runtime
cp target-runtime/release/renzora_runtime.exe runtimes/windows/
```

The separate target directory is required because the editor uses `bevy/dynamic_linking` for fast iteration. Without it, cargo reuses cached dynamic artifacts, producing a tiny (~1.5MB) binary that crashes. The correct runtime should be ~50MB (statically linked Bevy).

## Key Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Editor entry point |
| `src/runtime/main.rs` | Runtime entry point |
| `src/shared/spawner.rs` | Shared node spawning (add new node types here) |
| `src/shared/components.rs` | Shared component definitions |
| `src/shared/scene_format.rs` | Scene file format |
| `src/export/build.rs` | Export orchestration |
| `src/export/assets.rs` | Asset discovery |
| `src/export/pack.rs` | Pack file creation |
| `src/runtime/loader.rs` | Scene loading in runtime |
| `src/runtime/pack_asset_reader.rs` | Bevy AssetReader for packed files |

## Adding New Node Types

1. Add component type to `src/shared/components.rs` if needed
2. Add spawn logic to `src/shared/spawner.rs` in the `spawn_node_components` match statement
3. Add editor UI in appropriate panel (e.g., `src/ui/panels/inspector.rs`)
4. Both editor and runtime will automatically support the new type
