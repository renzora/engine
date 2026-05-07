#![allow(unused_mut, dead_code, unused_variables)]

//! Packing: create `.rpak` archives from a project directory.

use std::collections::{BTreeMap, HashSet, VecDeque};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::format::{
    encode_footer, encode_index, Compression, Header, PakEntry, FOOTER_LEN,
    FORMAT_VERSION, HEADER_FLAG_INDEX_COMPRESSED, HEADER_LEN,
};

/// Collects files and writes them into an `.rpak` archive.
pub struct RpakPacker {
    /// path (relative, forward-slash) -> file contents
    entries: BTreeMap<String, Vec<u8>>,
}

impl RpakPacker {
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Add a file with the given archive-relative path.
    pub fn add_file(&mut self, archive_path: &str, data: Vec<u8>) {
        let normalized = archive_path.replace('\\', "/");
        self.entries.insert(normalized, data);
    }

    /// Add a file from disk, computing the archive path relative to `base_dir`.
    pub fn add_from_disk(&mut self, base_dir: &Path, file_path: &Path) -> io::Result<()> {
        let relative = file_path
            .strip_prefix(base_dir)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let archive_path = relative.to_string_lossy().replace('\\', "/");
        let data = std::fs::read(file_path)?;
        self.add_file(&archive_path, data);
        Ok(())
    }

    /// Total number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Read the bytes of an entry by archive path.
    pub fn get(&self, archive_path: &str) -> Option<&[u8]> {
        self.entries.get(archive_path).map(|v| v.as_slice())
    }

    /// Serialize the archive into v2 bytes: header + per-entry-compressed
    /// data + (optionally compressed) index.
    ///
    /// Returns the standalone rpak content with no footer — append the
    /// 16-byte footer from [`crate::format::encode_footer`] when embedding
    /// in a host binary.
    pub fn finish(self, compression_level: i32) -> io::Result<Vec<u8>> {
        // Reserve header space; we'll backfill once the index offset is known.
        let mut out: Vec<u8> = vec![0u8; HEADER_LEN as usize];

        // ── Pass 1: write each entry's payload (zstd-compress when it shrinks).
        let mut entries: Vec<PakEntry> = Vec::with_capacity(self.entries.len());
        for (path, data) in &self.entries {
            let uncompressed_size = data.len() as u64;
            let entry_offset = out.len() as u64;

            // zstd-compress the payload, but fall back to Stored if the
            // compressor ends up making it larger (common for tiny files,
            // already-compressed formats like .png/.ogg/.glb-with-jpeg).
            let compressed = zstd::encode_all(data.as_slice(), compression_level)
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            let (compression, payload) = if (compressed.len() as u64) < uncompressed_size {
                (Compression::Zstd, compressed)
            } else {
                (Compression::Stored, data.clone())
            };

            let compressed_size = payload.len() as u64;
            out.extend_from_slice(&payload);

            entries.push(PakEntry {
                path: path.clone(),
                offset: entry_offset,
                compressed_size,
                uncompressed_size,
                compression,
                flags: 0,
                crc32: 0, // reserved for a later step
            });
        }

        // ── Pass 2: encode the index and (optionally) zstd it.
        let raw_index = encode_index(&entries);
        let index_uncompressed_len = raw_index.len();
        let compressed_index = zstd::encode_all(raw_index.as_slice(), compression_level)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        let (index_compress, stored_index): (bool, Vec<u8>) =
            if compressed_index.len() < index_uncompressed_len {
                (true, compressed_index)
            } else {
                (false, raw_index)
            };

        let index_offset = out.len() as u64;
        let index_compressed_len = stored_index.len();
        out.extend_from_slice(&stored_index);

        // ── Pass 3: backfill the header.
        let mut flags: u32 = 0;
        if index_compress {
            flags |= HEADER_FLAG_INDEX_COMPRESSED;
        }
        let header = Header {
            version: FORMAT_VERSION,
            flags,
            index_offset,
            index_compressed: index_compressed_len as u32,
            index_uncompressed: index_uncompressed_len as u32,
        };
        let mut header_bytes = Vec::with_capacity(HEADER_LEN as usize);
        header.write_into(&mut header_bytes);
        // Pad header to fixed length.
        while header_bytes.len() < HEADER_LEN as usize {
            header_bytes.push(0);
        }
        out[..HEADER_LEN as usize].copy_from_slice(&header_bytes);

        Ok(out)
    }

    /// Write the archive to a standalone `.rpak` file.
    pub fn write_to_file(self, path: &Path, compression_level: i32) -> io::Result<()> {
        let bytes = self.finish(compression_level)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &bytes)?;
        Ok(())
    }

    /// Append the archive to a binary, creating a self-contained executable.
    /// The 16-byte footer carrying `rpak_total_size + RPAK_MAGIC` is written
    /// after the rpak content so the loader can locate the rpak start.
    pub fn append_to_binary(
        self,
        binary_path: &Path,
        output_path: &Path,
        compression_level: i32,
    ) -> io::Result<()> {
        let rpak_bytes = self.finish(compression_level)?;
        let binary_data = std::fs::read(binary_path)?;

        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut out = std::fs::File::create(output_path)?;
        out.write_all(&binary_data)?;
        out.write_all(&rpak_bytes)?;
        let footer = encode_footer(rpak_bytes.len() as u64);
        debug_assert_eq!(footer.len(), FOOTER_LEN as usize);
        out.write_all(&footer)?;
        out.flush()?;

        Ok(())
    }
}

/// Pack only files that are transitively referenced from project entry points.
///
/// Reads `project.toml` to find the main scene, then performs a BFS over
/// scene/script/config files, scanning quoted strings for asset paths.
/// Only files actually referenced end up in the archive.
///
/// If `allowed_extensions` is `Some`, only files whose extension (lowercase)
/// appears in the list will be considered during the walk.
pub fn pack_project(
    project_dir: &Path,
    allowed_extensions: Option<&[&str]>,
) -> io::Result<RpakPacker> {
    pack_project_with_progress(project_dir, allowed_extensions, |_| {})
}

/// Like [`pack_project`] but calls `on_packed(archive_key)` each time a
/// file is added to the archive, enabling real-time progress reporting.
pub fn pack_project_with_progress<P>(
    project_dir: &Path,
    allowed_extensions: Option<&[&str]>,
    mut on_packed: P,
) -> io::Result<RpakPacker>
where
    P: FnMut(&str),
{
    let mut packer = RpakPacker::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();

    // Helper: try to find a file on disk given an archive-relative path.
    let resolve = |archive_key: &str| -> Option<PathBuf> {
        let path = project_dir.join(archive_key);
        if path.is_file() {
            return Some(path);
        }
        None
    };

    // Compute the archive key for a disk path (project-root-relative, forward slashes).
    let archive_key_for = |disk_path: &Path| -> Option<String> {
        disk_path
            .strip_prefix(project_dir)
            .ok()
            .map(|rel| rel.to_string_lossy().replace('\\', "/"))
    };

    // Try to pack a file by archive key if it passes the extension filter.
    let mut try_pack = |key: &str,
                        packer: &mut RpakPacker,
                        visited: &mut HashSet<String>,
                        queue: &mut VecDeque<String>,
                        on_packed: &mut P| {
        if visited.contains(key) {
            return;
        }
        if let Some(exts) = allowed_extensions {
            let ext = Path::new(key)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase());
            if let Some(ref ext) = ext {
                if !exts.iter().any(|a| a.eq_ignore_ascii_case(ext)) {
                    return;
                }
            }
        }
        if let Some(disk_path) = resolve(key) {
            if let Some(actual_key) = archive_key_for(&disk_path) {
                if visited.insert(actual_key.clone()) {
                    let _ = packer.add_from_disk(project_dir, &disk_path);
                    on_packed(&actual_key);
                    // Compiled material `.wgsl` files travel with a
                    // `<path>.meta` sidecar that holds texture bindings,
                    // parameters, alpha mode, etc. The .wgsl itself contains
                    // no quoted asset paths, so the BFS would otherwise
                    // never reach the meta. Pack it here.
                    if actual_key.ends_with(".wgsl") {
                        let meta_key = format!("{}.meta", actual_key);
                        if !visited.contains(&meta_key) {
                            if let Some(meta_disk) = resolve(&meta_key) {
                                if let Some(meta_actual) = archive_key_for(&meta_disk) {
                                    if visited.insert(meta_actual.clone()) {
                                        let _ =
                                            packer.add_from_disk(project_dir, &meta_disk);
                                        on_packed(&meta_actual);
                                        queue.push_back(meta_actual);
                                    }
                                }
                            }
                        }
                    }
                    queue.push_back(actual_key);
                }
            }
        }
    };

    // Always include project config files.
    // For project.toml, strip the editor-only `[editor]` section before
    // packing so exported builds don't carry editor preferences.
    for name in &["project.toml", "project.ron"] {
        let path = project_dir.join(name);
        if path.is_file() {
            if visited.insert(name.to_string()) {
                if *name == "project.toml" {
                    let text = std::fs::read_to_string(&path)?;
                    let stripped = strip_editor_section(&text);
                    packer.add_file(name, stripped.into_bytes());
                } else {
                    packer.add_from_disk(project_dir, &path)?;
                }
                on_packed(name);
                queue.push_back(name.to_string());
            }
        }
    }

    // Read main_scene and icon from project.toml
    let project_toml_path = project_dir.join("project.toml");
    if project_toml_path.is_file() {
        if let Ok(text) = std::fs::read_to_string(&project_toml_path) {
            if let Some(scene) = extract_toml_value(&text, "main_scene") {
                try_pack(
                    &scene,
                    &mut packer,
                    &mut visited,
                    &mut queue,
                    &mut on_packed,
                );
            }
            if let Some(icon) = extract_toml_value(&text, "icon") {
                try_pack(&icon, &mut packer, &mut visited, &mut queue, &mut on_packed);
            }
        }
    }

    // BFS: for each queued file, scan for asset path references and pack them
    while let Some(file_key) = queue.pop_front() {
        let refs: Vec<String> = {
            let Some(data) = packer.get(&file_key) else {
                continue;
            };
            // Binary GLBs aren't UTF-8 — pull out the JSON chunk so we can
            // discover externally-referenced textures and .bin files.
            let text: &str = if file_key.ends_with(".glb") {
                let Some(json) = extract_glb_json(data) else {
                    continue;
                };
                json
            } else if let Ok(t) = std::str::from_utf8(data) {
                t
            } else {
                continue;
            };
            extract_quoted_asset_paths(text)
        };

        // Parent dir of the referrer (forward-slashed). Used to resolve
        // references that are relative to the file containing them — e.g.
        // a GLB at models/foo/scene.glb pointing to "textures/img.png".
        let parent_dir: String = match Path::new(&file_key).parent() {
            Some(p) if !p.as_os_str().is_empty() => p.to_string_lossy().replace('\\', "/"),
            _ => String::new(),
        };

        for reference in refs {
            let norm = reference.replace('\\', "/");
            let stripped = norm.trim_start_matches("./").to_string();
            let mut candidates: Vec<String> = vec![norm.clone(), stripped.clone()];
            if !parent_dir.is_empty() {
                candidates.push(format!("{}/{}", parent_dir, stripped));
            }
            for candidate in candidates {
                if !visited.contains(&candidate) {
                    try_pack(
                        &candidate,
                        &mut packer,
                        &mut visited,
                        &mut queue,
                        &mut on_packed,
                    );
                }
            }
        }
    }

    Ok(packer)
}

/// Pack only files matching `allowed_extensions` that are transitively
/// referenced from project entry points.
pub fn pack_project_filtered(
    project_dir: &Path,
    allowed_extensions: &[&str],
) -> io::Result<RpakPacker> {
    pack_project(project_dir, Some(allowed_extensions))
}

/// Extensions to include when packing a server rpak (no rendering assets).
pub const SERVER_EXTENSIONS: &[&str] = &[
    "ron",       // scenes
    "lua",       // scripts
    "rhai",      // scripts
    "blueprint", // visual scripting
    "toml",      // project config
    "json",      // data files
];

// ============================================================================
// Mesh optimization
// ============================================================================

impl RpakPacker {
    /// Run an optimization function on every `.glb` entry in the archive.
    ///
    /// The closure receives the raw GLB bytes and should return optimized bytes.
    /// Entries that fail optimization are left unchanged.
    pub fn optimize_meshes<F>(&mut self, optimize_fn: F)
    where
        F: Fn(&[u8]) -> Result<Vec<u8>, String>,
    {
        self.optimize_meshes_with_progress(optimize_fn, |_, _, _| {});
    }

    /// Like [`optimize_meshes`] but calls `on_progress(current, total, filename)`
    /// before processing each `.glb` entry.
    pub fn optimize_meshes_with_progress<F, P>(&mut self, optimize_fn: F, mut on_progress: P)
    where
        F: Fn(&[u8]) -> Result<Vec<u8>, String>,
        P: FnMut(usize, usize, &str),
    {
        let glb_keys: Vec<String> = self
            .entries
            .keys()
            .filter(|k| k.ends_with(".glb"))
            .cloned()
            .collect();

        let total = glb_keys.len();
        for (i, key) in glb_keys.iter().enumerate() {
            on_progress(i + 1, total, key);
            if let Some(data) = self.entries.get(key).cloned() {
                match optimize_fn(&data) {
                    Ok(optimized) => {
                        self.entries.insert(key.clone(), optimized);
                    }
                    Err(_e) => {
                        // Leave entry unchanged on failure
                    }
                }
            }
        }
    }

    /// Generate LOD variants for every `.glb` entry.
    ///
    /// The closure receives `(glb_bytes, simplify_ratio)` and returns the
    /// simplified GLB bytes. LOD entries are added as `name_lod1.glb`, etc.
    pub fn generate_mesh_lods<F>(&mut self, lod_count: u32, lod_fn: F)
    where
        F: Fn(&[u8], f32) -> Result<Vec<u8>, String>,
    {
        self.generate_mesh_lods_with_progress(lod_count, lod_fn, |_, _, _| {});
    }

    /// Like [`generate_mesh_lods`] but calls `on_progress(current, total, filename)`
    /// before processing each `.glb` × LOD level combination.
    pub fn generate_mesh_lods_with_progress<F, P>(
        &mut self,
        lod_count: u32,
        lod_fn: F,
        mut on_progress: P,
    ) where
        F: Fn(&[u8], f32) -> Result<Vec<u8>, String>,
        P: FnMut(usize, usize, &str),
    {
        let glb_keys: Vec<String> = self
            .entries
            .keys()
            .filter(|k| k.ends_with(".glb"))
            .cloned()
            .collect();

        let total = glb_keys.len() * lod_count as usize;
        let mut current = 0;

        for key in &glb_keys {
            let Some(data) = self.entries.get(key).cloned() else {
                current += lod_count as usize;
                continue;
            };
            for lod in 1..=lod_count {
                current += 1;
                on_progress(current, total, key);
                let ratio = 1.0 / (2.0_f32.powi(lod as i32));
                match lod_fn(&data, ratio) {
                    Ok(lod_bytes) => {
                        let lod_key = key.replace(".glb", &format!("_lod{lod}.glb"));
                        self.entries.insert(lod_key, lod_bytes);
                    }
                    Err(_e) => {
                        // Skip this LOD level on failure
                    }
                }
            }
        }
    }
}

// ============================================================================
// Scene component stripping
// ============================================================================

/// Component type-path prefixes that the server needs. Everything else is stripped.
const SERVER_KEEP_PREFIXES: &[&str] = &[
    "bevy_ecs::",
    "bevy_transform::",
    "renzora::",
    "renzora_scripting::",
    "renzora_physics::",
    "renzora_blueprint::",
    "renzora_terrain::",
    "renzora_network::",
    "renzora_engine::",
];

/// Components from server-kept prefixes that still reference visual assets
/// and should be stripped from headless server scenes.
const SERVER_STRIP_EXACT: &[&str] = &[
    "renzora::MeshInstanceData",
    "renzora::MeshColor",
    "renzora::MeshPrimitive",
];

/// Editor-only crate prefixes — stripped from runtime (client) exports so
/// the runtime binary can deserialize scenes without editor types.
const EDITOR_ONLY_PREFIXES: &[&str] = &[
    "renzora_camera::",
    "renzora_editor::",
    "renzora_gizmo::",
    "renzora_grid::",
    "renzora_hierarchy::",
    "renzora_inspector::",
    "renzora_debugger::",
    "renzora_console::",
    "renzora_viewport::",
    "renzora_keybindings::",
    "renzora_settings::",
];

impl RpakPacker {
    /// Strip editor-only components from all `.ron` scene files.
    ///
    /// Also removes `.camera.ron` sidecar files (editor camera state).
    /// Call this after packing and before writing for **client runtime** exports.
    pub fn strip_for_runtime(&mut self) {
        // Remove camera sidecar files
        self.entries
            .retain(|path, _| !path.ends_with(".camera.ron"));

        let ron_keys: Vec<String> = self
            .entries
            .keys()
            .filter(|k| k.ends_with(".ron"))
            .cloned()
            .collect();

        for key in ron_keys {
            if let Some(data) = self.entries.get(&key) {
                if let Ok(input) = std::str::from_utf8(data) {
                    let stripped = strip_components_by_prefix(input, EDITOR_ONLY_PREFIXES);
                    self.entries.insert(key, stripped.into_bytes());
                }
            }
        }
    }

    /// Strip visual-only components from all `.ron` scene files in the archive.
    ///
    /// Also removes `.camera.ron` files entirely (editor-only camera state).
    /// Call this after packing and before writing for server exports.
    pub fn strip_for_server(&mut self) {
        // Remove camera sidecar files
        self.entries
            .retain(|path, _| !path.ends_with(".camera.ron"));

        // Strip visual components from scene .ron files
        let ron_keys: Vec<String> = self
            .entries
            .keys()
            .filter(|k| k.ends_with(".ron"))
            .cloned()
            .collect();

        for key in ron_keys {
            if let Some(data) = self.entries.get(&key) {
                if let Ok(input) = std::str::from_utf8(data) {
                    let stripped = strip_visual_components(input);
                    self.entries.insert(key, stripped.into_bytes());
                }
            }
        }
    }
}

/// Remove component entries from a Bevy scene RON whose type paths
/// don't match any `SERVER_KEEP_PREFIXES`.
///
/// Operates at the text level to avoid RON deserialization issues with
/// Bevy-specific enum syntax (e.g. `Srgba((...))`, `Window(Primary)`).
fn strip_visual_components(input: &str) -> String {
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(len);
    let mut i = 0;
    let mut copy_from = 0;

    while i < len {
        if bytes[i] != b'"' {
            i += 1;
            continue;
        }

        // Found opening quote — extract the key
        let quote_start = i;
        i += 1;
        while i < len {
            if bytes[i] == b'\\' {
                i += 2;
                continue;
            }
            if bytes[i] == b'"' {
                break;
            }
            i += 1;
        }
        if i >= len {
            break;
        }
        let key = &input[quote_start + 1..i];
        i += 1; // past closing quote

        // Is this a type-path map key? (contains "::" and followed by ":")
        if !key.contains("::") {
            continue;
        }
        let mut j = i;
        while j < len && (bytes[j] == b' ' || bytes[j] == b'\t') {
            j += 1;
        }
        if j >= len || bytes[j] != b':' {
            continue;
        }

        // It's a component entry — check the keep list
        let kept_by_prefix = SERVER_KEEP_PREFIXES.iter().any(|p| key.starts_with(p));
        let force_stripped = SERVER_STRIP_EXACT.iter().any(|e| key == *e);
        if kept_by_prefix && !force_stripped {
            continue; // keep it
        }

        // Strip: find the start of this entry line (back up past indentation)
        let mut entry_start = quote_start;
        while entry_start > 0 && (bytes[entry_start - 1] == b' ' || bytes[entry_start - 1] == b'\t')
        {
            entry_start -= 1;
        }
        if entry_start > 0 && bytes[entry_start - 1] == b'\n' {
            entry_start -= 1;
        }
        if entry_start > 0 && bytes[entry_start - 1] == b'\r' {
            entry_start -= 1;
        }

        // Copy everything before this entry
        out.push_str(&input[copy_from..entry_start]);

        // Skip past ':' and whitespace to the value
        i = j + 1;
        while i < len && bytes[i].is_ascii_whitespace() {
            i += 1;
        }

        // Skip the value
        i = skip_ron_value(bytes, i);

        // Skip trailing comma
        while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }
        if i < len && bytes[i] == b',' {
            i += 1;
        }

        copy_from = i;
    }

    out.push_str(&input[copy_from..]);
    out
}

/// Remove component entries whose type paths start with any of the given prefixes.
///
/// Used by `strip_for_runtime` to remove editor-only components.
fn strip_components_by_prefix(input: &str, strip_prefixes: &[&str]) -> String {
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(len);
    let mut i = 0;
    let mut copy_from = 0;

    while i < len {
        if bytes[i] != b'"' {
            i += 1;
            continue;
        }

        let quote_start = i;
        i += 1;
        while i < len {
            if bytes[i] == b'\\' {
                i += 2;
                continue;
            }
            if bytes[i] == b'"' {
                break;
            }
            i += 1;
        }
        if i >= len {
            break;
        }
        let key = &input[quote_start + 1..i];
        i += 1;

        if !key.contains("::") {
            continue;
        }
        let mut j = i;
        while j < len && (bytes[j] == b' ' || bytes[j] == b'\t') {
            j += 1;
        }
        if j >= len || bytes[j] != b':' {
            continue;
        }

        // Keep unless it matches a strip prefix
        if !strip_prefixes.iter().any(|p| key.starts_with(p)) {
            continue;
        }

        // Strip this entry
        let mut entry_start = quote_start;
        while entry_start > 0 && (bytes[entry_start - 1] == b' ' || bytes[entry_start - 1] == b'\t')
        {
            entry_start -= 1;
        }
        if entry_start > 0 && bytes[entry_start - 1] == b'\n' {
            entry_start -= 1;
        }
        if entry_start > 0 && bytes[entry_start - 1] == b'\r' {
            entry_start -= 1;
        }

        out.push_str(&input[copy_from..entry_start]);

        i = j + 1;
        while i < len && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        i = skip_ron_value(bytes, i);
        while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }
        if i < len && bytes[i] == b',' {
            i += 1;
        }

        copy_from = i;
    }

    out.push_str(&input[copy_from..]);
    out
}

/// Skip a RON value starting at `start`, returning the index just past it.
fn skip_ron_value(bytes: &[u8], start: usize) -> usize {
    let len = bytes.len();
    let mut i = start;
    if i >= len {
        return i;
    }

    match bytes[i] {
        // Delimited: balanced skip over (), {}, []
        b'(' | b'{' | b'[' => {
            let mut depth = 1u32;
            i += 1;
            while i < len && depth > 0 {
                match bytes[i] {
                    b'"' => {
                        i = skip_ron_string(bytes, i);
                        continue;
                    }
                    b'(' | b'{' | b'[' => depth += 1,
                    b')' | b'}' | b']' => depth -= 1,
                    _ => {}
                }
                i += 1;
            }
            i
        }
        // String value
        b'"' => skip_ron_string(bytes, i),
        // Atom: identifier, number, bool, enum variant (may be followed by "(..)")
        _ => {
            while i < len
                && (bytes[i].is_ascii_alphanumeric()
                    || bytes[i] == b'_'
                    || bytes[i] == b'.'
                    || bytes[i] == b'-'
                    || bytes[i] == b'+')
            {
                i += 1;
            }
            // Enum variant with data: Variant(..)
            if i < len && bytes[i] == b'(' {
                i = skip_ron_value(bytes, i);
            }
            i
        }
    }
}

/// Skip a quoted string starting at the opening `"`, returning the index just past the closing `"`.
fn skip_ron_string(bytes: &[u8], start: usize) -> usize {
    let len = bytes.len();
    let mut i = start + 1; // past opening quote
    while i < len {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == b'"' {
            return i + 1;
        }
        i += 1;
    }
    i
}

// ============================================================================
// Asset path scanning (used by pack_project)
// ============================================================================

/// Known asset file extensions for reference scanning.
const ASSET_EXTENSIONS: &[&str] = &[
    // Images
    ".png",
    ".jpg",
    ".jpeg",
    ".bmp",
    ".tga",
    ".hdr",
    ".exr",
    // 3D models
    ".glb",
    ".gltf",
    ".bin",
    // Audio
    ".ogg",
    ".wav",
    ".mp3",
    ".flac",
    // Scenes
    ".ron",
    // Materials / shaders
    ".material",
    ".shader",
    ".wgsl",
    // Scripts
    ".lua",
    ".rhai",
    // Data
    ".blueprint",
    ".json",
    // Fonts
    ".ttf",
    ".otf",
];

/// Extract a simple `key = "value"` from TOML text.
fn extract_toml_value(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(key) {
            let rest = rest.trim();
            if let Some(rest) = rest.strip_prefix('=') {
                let rest = rest.trim();
                if rest.starts_with('"') && rest.len() >= 2 {
                    if let Some(end) = rest[1..].find('"') {
                        return Some(rest[1..1 + end].to_string());
                    }
                }
            }
        }
    }
    None
}

/// Scan text for quoted strings that look like asset paths.
fn extract_quoted_asset_paths(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut refs = Vec::new();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'"' {
            let start = i + 1;
            i += 1;
            while i < len {
                if bytes[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if bytes[i] == b'"' {
                    break;
                }
                i += 1;
            }
            if i < len {
                let s = &text[start..i];
                if looks_like_asset_path(s) {
                    refs.push(s.to_string());
                }
            }
            i += 1;
        } else {
            i += 1;
        }
    }

    refs
}

/// Extract the JSON chunk from a binary glTF (GLB) file.
///
/// GLB layout: 12-byte header (`glTF` magic + version + total length), then
/// a sequence of chunks. The first chunk must be JSON. We need its bytes so
/// the BFS can discover external `uri` references (textures, `.bin` buffers)
/// inside the glTF JSON without choking on the binary BIN chunk.
fn extract_glb_json(data: &[u8]) -> Option<&str> {
    if data.len() < 20 || &data[0..4] != b"glTF" {
        return None;
    }
    let chunk_len = u32::from_le_bytes(data[12..16].try_into().ok()?) as usize;
    if &data[16..20] != b"JSON" {
        return None;
    }
    let json_end = 20usize.checked_add(chunk_len)?;
    if json_end > data.len() {
        return None;
    }
    std::str::from_utf8(&data[20..json_end]).ok()
}

/// Check if a quoted string looks like an asset file path.
fn looks_like_asset_path(s: &str) -> bool {
    if s.is_empty() || s.len() > 512 {
        return false;
    }
    // Rust type paths like "bevy_ecs::name::Name" are not asset paths
    if s.contains("::") {
        return false;
    }
    let lower = s.to_ascii_lowercase();
    ASSET_EXTENSIONS.iter().any(|ext| lower.ends_with(ext))
}

/// Strip the editor-only `[editor]` section (and any `[editor.*]` subtables)
/// from a project.toml string. Returns the original text on parse failure.
fn strip_editor_section(text: &str) -> String {
    let Ok(mut value) = text.parse::<toml::Value>() else {
        return text.to_string();
    };
    if let Some(table) = value.as_table_mut() {
        table.remove("editor");
    }
    toml::to_string_pretty(&value).unwrap_or_else(|_| text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read::RpakArchive;

    #[test]
    fn new_packer_is_empty() {
        let p = RpakPacker::new();
        assert!(p.is_empty());
        assert_eq!(p.len(), 0);
    }

    #[test]
    fn add_file_normalizes_backslashes() {
        let mut p = RpakPacker::new();
        p.add_file("models\\car.glb", vec![1, 2, 3]);
        // Lookups go through normalize, so both spellings hit the same entry.
        assert_eq!(p.get("models/car.glb"), Some(&[1u8, 2, 3][..]));
        assert_eq!(p.len(), 1);
    }

    #[test]
    fn add_file_overwrites_duplicate_path() {
        // BTreeMap semantics — last write wins. Documented contract;
        // pinning it so we don't accidentally start collecting duplicates.
        let mut p = RpakPacker::new();
        p.add_file("scenes/main.ron", vec![1]);
        p.add_file("scenes/main.ron", vec![2, 3]);
        assert_eq!(p.len(), 1);
        assert_eq!(p.get("scenes/main.ron"), Some(&[2u8, 3][..]));
    }

    #[test]
    fn empty_archive_round_trips() {
        let p = RpakPacker::new();
        let bytes = p.finish(3).expect("finish");
        let archive = RpakArchive::from_bytes(&bytes).expect("read");
        assert!(archive.is_empty());
        assert_eq!(archive.len(), 0);
    }

    #[test]
    fn pack_then_read_preserves_files() {
        let mut p = RpakPacker::new();
        let scene = b"(\"main scene\")".to_vec();
        let model: Vec<u8> = (0u8..=200).cycle().take(2048).collect();
        p.add_file("scenes/main.ron", scene.clone());
        p.add_file("models/car.glb", model.clone());
        p.add_file("textures/wall.png", vec![0xff, 0xee, 0xdd]);

        let bytes = p.finish(3).expect("finish");
        let archive = RpakArchive::from_bytes(&bytes).expect("read");

        assert_eq!(archive.len(), 3);
        assert_eq!(archive.get("scenes/main.ron").as_deref(), Some(scene.as_slice()));
        assert_eq!(archive.get("models/car.glb").as_deref(), Some(model.as_slice()));
        assert_eq!(
            archive.get("textures/wall.png").as_deref(),
            Some(&[0xff, 0xee, 0xdd][..])
        );
    }

    #[test]
    fn read_normalizes_backslash_queries() {
        // Archive paths are stored with forward slashes. Asking with
        // backslashes (Windows callers) should find the file anyway.
        let mut p = RpakPacker::new();
        p.add_file("models/car.glb", vec![1, 2, 3]);
        let bytes = p.finish(3).expect("finish");
        let archive = RpakArchive::from_bytes(&bytes).expect("read");
        assert_eq!(archive.get("models\\car.glb").as_deref(), Some(&[1u8, 2, 3][..]));
        assert!(archive.contains("models\\car.glb"));
    }

    #[test]
    fn read_paths_iter_lists_all_entries() {
        let mut p = RpakPacker::new();
        p.add_file("a.txt", vec![1]);
        p.add_file("b.txt", vec![2]);
        p.add_file("nested/c.txt", vec![3]);
        let bytes = p.finish(3).expect("finish");
        let archive = RpakArchive::from_bytes(&bytes).expect("read");
        let mut paths: Vec<&str> = archive.paths().collect();
        paths.sort();
        assert_eq!(paths, vec!["a.txt", "b.txt", "nested/c.txt"]);
    }

    #[test]
    fn missing_file_returns_none() {
        let mut p = RpakPacker::new();
        p.add_file("only.txt", vec![42]);
        let bytes = p.finish(3).expect("finish");
        let archive = RpakArchive::from_bytes(&bytes).expect("read");
        assert!(archive.get("nope.txt").is_none());
        assert!(!archive.contains("nope.txt"));
    }

    #[test]
    fn corrupted_bytes_return_error() {
        // A garbage buffer doesn't have a valid header → archive load fails.
        // Earlier versions silently produced an empty archive on garbage; this
        // pins the error path so we can't regress.
        let result = RpakArchive::from_bytes(&[0u8, 1, 2, 3]);
        assert!(result.is_err());
    }

    #[test]
    fn large_file_round_trips() {
        // Sanity-check the offset/size arithmetic with a file bigger than
        // the header so any off-by-one in pos accounting shows up.
        let mut p = RpakPacker::new();
        let big: Vec<u8> = (0..10_000).map(|i| (i % 251) as u8).collect();
        p.add_file("big.bin", big.clone());
        let bytes = p.finish(3).expect("finish");
        let archive = RpakArchive::from_bytes(&bytes).expect("read");
        assert_eq!(archive.get("big.bin").as_deref(), Some(big.as_slice()));
    }

    #[test]
    fn already_compressed_files_use_stored_compression() {
        // Random bytes don't compress — the packer should fall through to
        // Stored so we don't waste CPU decompressing a buffer that grew.
        let mut p = RpakPacker::new();
        let mut rnd = 0xdeadbeefu64;
        let random: Vec<u8> = (0..4096)
            .map(|_| {
                rnd = rnd.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                (rnd >> 33) as u8
            })
            .collect();
        p.add_file("noise.bin", random.clone());
        let bytes = p.finish(3).expect("finish");
        let archive = RpakArchive::from_bytes(&bytes).expect("read");

        let entry = archive.entry("noise.bin").expect("entry");
        assert_eq!(entry.compression, crate::format::Compression::Stored);
        assert_eq!(entry.compressed_size, entry.uncompressed_size);
        assert_eq!(archive.get("noise.bin").as_deref(), Some(random.as_slice()));
    }

    #[test]
    fn appended_to_binary_round_trips() {
        // Build a fake host-binary file, append a packed rpak with footer,
        // and verify `from_bytes` (which auto-detects the footer) extracts
        // the rpak content correctly.
        let mut p = RpakPacker::new();
        p.add_file("hello.txt", b"world".to_vec());
        let rpak = p.finish(3).expect("finish");

        let mut combined = b"PRETEND_THIS_IS_AN_EXE".to_vec();
        combined.extend_from_slice(&rpak);
        combined.extend_from_slice(&crate::format::encode_footer(rpak.len() as u64));

        let archive = RpakArchive::from_bytes(&combined).expect("read");
        assert_eq!(archive.get("hello.txt").as_deref(), Some(b"world".as_slice()));
    }

    #[test]
    fn from_file_uses_mmap_backend() {
        // Write a packed rpak to a temp file and read it back through
        // `from_file`, which should pick the `MmapBackend` path. Verifies
        // the backend abstraction round-trips end-to-end on real disk I/O.
        let mut p = RpakPacker::new();
        let payload: Vec<u8> = (0u8..=255).cycle().take(8192).collect();
        p.add_file("blob.bin", payload.clone());
        p.add_file("text/readme.txt", b"hello from disk".to_vec());

        let dir = std::env::temp_dir().join("renzora_rpak_test_mmap");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test.rpak");
        p.write_to_file(&path, 3).expect("write");

        let archive = RpakArchive::from_file(&path).expect("read");
        assert_eq!(archive.get("blob.bin").as_deref(), Some(payload.as_slice()));
        assert_eq!(
            archive.get("text/readme.txt").as_deref(),
            Some(b"hello from disk".as_slice())
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn from_binary_finds_appended_rpak_via_file() {
        // Same end-to-end check, but for the appended-to-binary case —
        // exercises footer detection through the mmap/file backend.
        let mut p = RpakPacker::new();
        p.add_file("config.toml", b"name = \"test\"".to_vec());

        let dir = std::env::temp_dir().join("renzora_rpak_test_appended");
        let _ = std::fs::create_dir_all(&dir);
        let host_path = dir.join("fake_exe.bin");
        let combined_path = dir.join("fake_exe_with_rpak.bin");

        std::fs::write(&host_path, b"FAKE_HOST_BINARY_BYTES").unwrap();
        p.append_to_binary(&host_path, &combined_path, 3)
            .expect("append");

        let archive = RpakArchive::from_binary(&combined_path)
            .expect("read")
            .expect("found");
        assert_eq!(
            archive.get("config.toml").as_deref(),
            Some(b"name = \"test\"".as_slice())
        );

        let _ = std::fs::remove_file(&host_path);
        let _ = std::fs::remove_file(&combined_path);
    }

    /// Build a minimal GLB byte sequence wrapping `json` as the JSON chunk,
    /// followed by a binary BIN chunk of garbage bytes (so the whole file
    /// is not valid UTF-8 — that's the case the BFS used to choke on).
    fn synth_glb(json: &str) -> Vec<u8> {
        let json_bytes = json.as_bytes();
        // GLB JSON chunks must be 4-byte aligned, padded with 0x20.
        let json_pad = (4 - (json_bytes.len() % 4)) % 4;
        let json_chunk_len = json_bytes.len() + json_pad;
        let bin: [u8; 8] = [0xff, 0x00, 0xfe, 0x80, 0x00, 0xc0, 0xff, 0xee];
        let total =
            12 + 8 + json_chunk_len + 8 + bin.len();

        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(json_chunk_len as u32).to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(json_bytes);
        for _ in 0..json_pad {
            out.push(0x20);
        }
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&bin);
        out
    }

    #[test]
    fn extract_glb_json_returns_json_chunk() {
        let json = r#"{"images":[{"uri":"textures/img.png"}]}"#;
        let glb = synth_glb(json);
        // Whole file is not UTF-8 because of the BIN chunk bytes.
        assert!(std::str::from_utf8(&glb).is_err());
        let extracted = extract_glb_json(&glb).expect("json chunk");
        // Padding bytes are part of the chunk; trim before comparison.
        assert_eq!(extracted.trim_end_matches(' '), json);
    }

    #[test]
    fn extract_glb_json_rejects_non_glb() {
        assert!(extract_glb_json(b"").is_none());
        assert!(extract_glb_json(b"not a glb at all").is_none());
        assert!(extract_glb_json(&[0u8; 24]).is_none());
    }
}
