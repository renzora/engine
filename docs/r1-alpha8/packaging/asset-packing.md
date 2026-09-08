# Asset Packing (rpak)

`.rpak` is Renzora's own indexed, per-entry-compressed archive format for shipping a project's assets as a single file.

## What `.rpak` is

An `.rpak` (Renzora Pack) bundles every asset a build needs — scenes, scripts, models, textures, audio, UI templates — into one indexed archive that replaces the loose `assets/` tree at runtime. It is produced by the `renzora_rpak` crate (`crates/renzora_rpak`) and read transparently by the engine's virtual filesystem.

Two properties define the format:

- **Per-entry compression.** Each file is compressed (or stored) independently — the archive is *not* one big zstd blob. Reading one file decompresses only that file, so a multi-GB archive costs almost no memory until something is actually accessed.
- **Two shapes.** An archive is either a **standalone `game.rpak`** file or **appended to the engine binary** to make a single self-contained executable.

> Only two compression schemes exist: **Stored** (raw) and **Zstd**. There is no LZ4, no encryption, no key files. The crate's only dependency for codecs is `zstd 0.13`.

## The v2 file layout

```text
┌──────────────────────────────────────────────┐
│ Header                32 bytes, fixed         │  magic, version, pointer to index
├──────────────────────────────────────────────┤
│ Data section          starts at offset 32     │  per-entry payloads,
│   entry 0 payload  (Stored or Zstd)           │  each independently compressed
│   entry 1 payload  (Stored or Zstd)           │
│   …                                           │
├──────────────────────────────────────────────┤
│ Index (tail)          at header.index_offset  │  enumerates every entry,
│                       optionally zstd'd        │  located at the END of the file
├──────────────────────────────────────────────┤
│ Footer                16 bytes, OPTIONAL       │  present only when appended
│                                                │  to a host binary
└──────────────────────────────────────────────┘
```

The index lives at the **tail**, after the data — not after the header. The header only points at it.

### Header (32 bytes, fixed)

All multi-byte fields are little-endian. The magic appears at the start of the rpak in every file.

| Offset | Field | Type | Notes |
|---|---|---|---|
| 0 | magic | `[u8; 4]` | `"RPAK"` |
| 4 | version | `u32` | `2` (`FORMAT_VERSION`) |
| 8 | flags | `u32` | bit 0 (`0x01`) = index is zstd-compressed |
| 12 | index_offset | `u64` | start of the index, relative to start-of-rpak |
| 20 | index_compressed | `u32` | index size as stored on disk |
| 24 | index_uncompressed | `u32` | index size after decompression |
| 28 | — | 4 bytes | reserved (zero) |

### Data section

Begins at offset `32` (`HEADER_LEN`). It is the concatenation of every entry's payload in index order. Each payload is independently either `Stored` (raw bytes, `compressed_size == uncompressed_size`) or `Zstd`.

### Index (tail)

At `index_offset`, optionally zstd-compressed (per the header flag). When uncompressed it is:

```text
count               u32
per entry:
  path_len          u32
  path              utf8 (path_len bytes, forward-slashed)
  offset            u64   (relative to start-of-rpak)
  compressed_size   u64
  uncompressed_size u64
  compression       u8    (0 = Stored, 1 = Zstd)
  entry_flags       u8    (reserved)
  padding           u16   (= 0)
  crc32             u32   (0 = not computed)
```

Entry `offset`s are relative to the start of the rpak (offset 0 = the `RPAK` magic). For an appended-to-binary archive the reader rebases each offset by the rpak's start position before reading.

### Footer (16 bytes, appended-to-binary only)

When an archive is appended to a host binary, a 16-byte footer is written **after** the rpak so the loader can find where the rpak begins inside the larger file:

| Offset | Field | Type | Notes |
|---|---|---|---|
| 0 | rpak_total_size | `u64` | size of `[Header .. end-of-Index]` |
| 8 | magic | `[u8; 4]` | `"RPAK"` |
| 12 | — | `u32` | reserved (zero) |

Note the magic sits at offset **8** in the footer (not 0) — that is how detection distinguishes a tail footer from a standalone header.

## Detection order

`RpakArchive` figures out what it is looking at by reading the ends of the file (`locate_rpak_start` in `read.rs`):

1. **Last 16 bytes.** If `bytes[8..12] == "RPAK"`, the file is a binary with an appended rpak. Compute `rpak_start = file_size - 16 - rpak_total_size` and read the header there.
2. **First 8 bytes.** If `[0..4] == "RPAK"` and the version `u32` is `2`, it is a standalone `.rpak` with `rpak_start = 0`.
3. **Otherwise** it is not an rpak.

## Compression

Compression is decided per file when packing (`pack.rs`):

- **Already-compressed formats are stored verbatim** — `.png`, `.jpg`/`.jpeg`, `.webp`, `.gif`, `.ogg`, `.mp3`, `.flac`, `.rmip`, `.ktx2`, `.basis`, `.dds`, `.zst`, `.zip`. Re-zstd-ing them burns CPU for no win.
- **Everything else is zstd-compressed.** If the zstd output is not actually smaller than the input, the entry falls back to `Stored`.
- The **index** itself is also zstd'd if that shrinks it (recorded in the header flag).

The single knob is the **zstd compression level** (`i32`), passed to `finish`/`write_to_file`/`append_to_binary`. The editor's export overlay defaults to level **3**. Packing runs in parallel across cores on native targets (via `rayon`); the wasm runtime only ever reads archives, never packs.

## Producing an archive

There is **no `renzora pack` / `renzora unpack` / `renzora keygen` CLI** — those commands do not exist. Archives are produced two ways: through the editor's **Export** overlay, or programmatically through the `renzora_rpak` library.

### From the editor (Export overlay)

`renzora_export` walks the open project, packs the referenced assets, and writes the archive. The **Packaging** section of the Export panel offers two modes:

| Mode | Result |
|---|---|
| **Binary + .rpak** (`SeparateFiles`, default) | A standalone `<binary>.rpak` written next to the runtime binary. The runtime loads it as an adjacent archive. |
| **Single executable** (`SingleBinary`) | The rpak is appended to the runtime binary (plus the 16-byte footer), producing one self-contained file. |

Internally the export pipeline:

1. Runs `pack_project_with_progress` — a BFS that starts from `project.toml`'s `main_scene` and `icon`, then follows quoted asset paths it finds inside scenes, scripts, materials, and GLB JSON chunks. Only transitively-referenced files are packed. (`project.toml`/`project.ron` are always included; the editor-only `[editor]` section is stripped from the packed `project.toml`.)
2. Calls `strip_for_runtime()` to drop editor-only components and `.camera.ron` sidecars from scene RON.
3. Optionally runs `optimize_meshes` and `generate_mesh_lods` over the `.glb` entries.
4. Writes the result via `write_to_file` (separate) or `append_to_binary` (single).

### Server archives

A dedicated-server export produces a leaner, visuals-free archive:

- `pack_project_filtered(project_dir, SERVER_EXTENSIONS)` — where `SERVER_EXTENSIONS` is `ron`, `lua`, `rhai`, `blueprint`, `toml`, `json`.
- `strip_for_server()` removes mesh/visual components from scene RON.
- The result is always a standalone **`server.rpak`**. The generated launcher runs the same engine binary against it: `renzora --server --rpak server.rpak`.

## Reading archives at runtime

At startup the engine builds a `Vfs` (`renzora_engine/vfs.rs`) by probing for an archive in this order:

1. `--rpak <path>` command-line override (used by the server launcher).
2. An rpak **embedded** in the running executable (footer detection on `current_exe`).
3. An **adjacent** `<exe-stem>.rpak` next to the executable.
4. Platform bundles: Android APK `game.rpak`, iOS bundle `game.rpak`, or WASM bytes injected from JavaScript (`set_wasm_rpak`).
5. **Raw filesystem** — development mode, no archive.

Once an archive is loaded, the custom Bevy asset reader (`asset_reader.rs`) resolves each `AssetServer::load` path in this order:

1. Absolute paths — pass through to disk.
2. The rpak archive.
3. The project-local `assets/` override (when a project is open).
4. The exe-adjacent `assets/` directory (exported builds).
5. The current-working-directory `assets/` directory (dev fallback).

So application code never changes — `asset_server.load("textures/brick.png")` reads from the archive when one is present and from disk otherwise:

```rust
use bevy::prelude::*;

fn load_things(asset_server: Res<AssetServer>) {
    // Served from game.rpak if loaded, else from the filesystem.
    let texture: Handle<Image> = asset_server.load("textures/brick.png");
}
```

## The `renzora_rpak` library API

The crate exposes a writer (`RpakPacker`) and a reader (`RpakArchive`).

### Packing

```rust
use renzora_rpak::RpakPacker;
use std::path::Path;

let mut packer = RpakPacker::new();
packer.add_file("scenes/main.ron", scene_bytes);            // bytes you already have
packer.add_from_disk(Path::new("project"), Path::new("project/models/car.glb"))?;

// Serialize to bytes (zstd level 3) …
let bytes: Vec<u8> = packer.finish(3)?;

// … or write a standalone archive …
packer.write_to_file(Path::new("dist/game.rpak"), 3)?;
```

```rust
// Append an archive to a host binary to make a single self-contained executable.
// finish() returns the rpak WITHOUT a footer; append_to_binary adds the 16-byte footer.
packer.append_to_binary(
    Path::new("dist/renzora"),       // host binary
    Path::new("dist/MyGame"),        // output
    3,                               // zstd level
)?;
```

Archive paths are normalized to forward slashes, and duplicate paths follow last-write-wins. Use `pack_project` / `pack_project_filtered` for the reference-following BFS described above instead of adding files by hand.

### Reading

```rust
use renzora_rpak::RpakArchive;
use std::path::Path;

// Pick the backend automatically: mmap on desktop/iOS, falling back to a file handle.
let archive = RpakArchive::from_file(Path::new("dist/game.rpak"))?;

// Or detect an rpak appended to the running executable (returns None if absent):
let embedded = RpakArchive::from_current_exe()?;

// Read + decompress one entry on demand.
if let Some(bytes) = archive.get("scenes/main.ron") {
    // use bytes
}

println!("{} entries", archive.len());
for path in archive.paths() {
    println!("  {path}");
}
```

| Method | Purpose |
|---|---|
| `from_file(path)` | Standalone `.rpak`, mmap with file-handle fallback |
| `from_current_exe()` / `from_binary(path)` | Detect an appended rpak via the footer |
| `from_bytes(bytes)` | In-memory / WASM (`BytesBackend`) |
| `get(path)` | Read + decompress one entry → `Vec<u8>` |
| `entry(path)` | Metadata only (`PakEntry`), no payload read |
| `contains(path)` / `paths()` / `len()` | Index queries |
| `total_compressed_bytes()` / `total_uncompressed_bytes()` | Size accounting |
| `extract_to(dir)` | Unpack every entry to disk |

Reads go through a `PakBackend`: `MmapBackend` (desktop/iOS default — the OS page cache holds the working set), `FileBackend` (positional-read fallback for filesystems where mmap fails), and `BytesBackend` (WASM and in-memory).

## Inspecting an archive

The crate ships an `inspect_pack` example that runs the project BFS and prints what it would pack:

```bash
renzora shell -- cargo run -p renzora_rpak --example inspect_pack -- path/to/project
```

It lists every entry the packer collected and the asset-like quoted paths found in `scenes/main.ron`, which is handy for diagnosing assets that fail to make it into an export.
