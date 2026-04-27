#![allow(unused_mut, dead_code, unused_variables)]

//! Packing: create `.rpak` archives from a project directory.

use std::collections::{BTreeMap, HashSet, VecDeque};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

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

    /// Serialize and zstd-compress the archive, returning the compressed bytes.
    pub fn finish(self, compression_level: i32) -> io::Result<Vec<u8>> {
        // Build uncompressed blob: header + data
        let mut raw = Vec::new();

        // Entry count
        let count = self.entries.len() as u32;
        raw.extend_from_slice(&count.to_le_bytes());

        // First pass: compute offsets (relative to start of data section)
        let mut offset: u64 = 0;
        let mut index_entries: Vec<(String, u64, u64)> = Vec::with_capacity(self.entries.len());
        for (path, data) in &self.entries {
            let size = data.len() as u64;
            index_entries.push((path.clone(), offset, size));
            offset += size;
        }

        // Write index
        for (path, offset, size) in &index_entries {
            let path_bytes = path.as_bytes();
            raw.extend_from_slice(&(path_bytes.len() as u32).to_le_bytes());
            raw.extend_from_slice(path_bytes);
            raw.extend_from_slice(&offset.to_le_bytes());
            raw.extend_from_slice(&size.to_le_bytes());
        }

        // Write file data
        for (_path, data) in &self.entries {
            raw.extend_from_slice(data);
        }

        // Compress
        let compressed = zstd::encode_all(raw.as_slice(), compression_level)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        Ok(compressed)
    }

    /// Write the archive to a standalone `.rpak` file.
    pub fn write_to_file(self, path: &Path, compression_level: i32) -> io::Result<()> {
        let compressed = self.finish(compression_level)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &compressed)?;
        Ok(())
    }

    /// Append the archive to a binary, creating a self-contained executable.
    pub fn append_to_binary(
        self,
        binary_path: &Path,
        output_path: &Path,
        compression_level: i32,
    ) -> io::Result<()> {
        let compressed = self.finish(compression_level)?;
        let binary_data = std::fs::read(binary_path)?;

        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut out = std::fs::File::create(output_path)?;
        out.write_all(&binary_data)?;
        out.write_all(&compressed)?;
        // Footer: size of compressed data + magic
        out.write_all(&(compressed.len() as u64).to_le_bytes())?;
        out.write_all(crate::RPAK_MAGIC)?;
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
                try_pack(&scene, &mut packer, &mut visited, &mut queue, &mut on_packed);
            }
            if let Some(icon) = extract_toml_value(&text, "icon") {
                try_pack(&icon, &mut packer, &mut visited, &mut queue, &mut on_packed);
            }
        }
    }

    // BFS: for each queued file, scan for asset path references and pack them
    while let Some(file_key) = queue.pop_front() {
        let Some(data) = packer.get(&file_key) else { continue };
        let Ok(text) = std::str::from_utf8(data) else { continue };
        let refs = extract_quoted_asset_paths(text);

        for reference in refs {
            let norm = reference.replace('\\', "/");
            let stripped = norm.trim_start_matches("./").to_string();
            for candidate in [
                norm.clone(),
                stripped.clone(),
            ] {
                if !visited.contains(&candidate) {
                    try_pack(&candidate, &mut packer, &mut visited, &mut queue, &mut on_packed);
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
    "ron",        // scenes
    "lua",        // scripts
    "rhai",       // scripts
    "blueprint",  // visual scripting
    "toml",       // project config
    "json",       // data files
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
    pub fn optimize_meshes_with_progress<F, P>(
        &mut self,
        optimize_fn: F,
        mut on_progress: P,
    ) where
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
        self.entries.retain(|path, _| !path.ends_with(".camera.ron"));

        let ron_keys: Vec<String> = self.entries
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
        self.entries.retain(|path, _| !path.ends_with(".camera.ron"));

        // Strip visual components from scene .ron files
        let ron_keys: Vec<String> = self.entries
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
            if bytes[i] == b'\\' { i += 2; continue; }
            if bytes[i] == b'"' { break; }
            i += 1;
        }
        if i >= len { break; }
        let key = &input[quote_start + 1..i];
        i += 1; // past closing quote

        // Is this a type-path map key? (contains "::" and followed by ":")
        if !key.contains("::") {
            continue;
        }
        let mut j = i;
        while j < len && (bytes[j] == b' ' || bytes[j] == b'\t') { j += 1; }
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
        while entry_start > 0 && (bytes[entry_start - 1] == b' ' || bytes[entry_start - 1] == b'\t') {
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
        while i < len && bytes[i].is_ascii_whitespace() { i += 1; }

        // Skip the value
        i = skip_ron_value(bytes, i);

        // Skip trailing comma
        while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') { i += 1; }
        if i < len && bytes[i] == b',' { i += 1; }

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
            if bytes[i] == b'\\' { i += 2; continue; }
            if bytes[i] == b'"' { break; }
            i += 1;
        }
        if i >= len { break; }
        let key = &input[quote_start + 1..i];
        i += 1;

        if !key.contains("::") {
            continue;
        }
        let mut j = i;
        while j < len && (bytes[j] == b' ' || bytes[j] == b'\t') { j += 1; }
        if j >= len || bytes[j] != b':' {
            continue;
        }

        // Keep unless it matches a strip prefix
        if !strip_prefixes.iter().any(|p| key.starts_with(p)) {
            continue;
        }

        // Strip this entry
        let mut entry_start = quote_start;
        while entry_start > 0 && (bytes[entry_start - 1] == b' ' || bytes[entry_start - 1] == b'\t') {
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
        while i < len && bytes[i].is_ascii_whitespace() { i += 1; }
        i = skip_ron_value(bytes, i);
        while i < len && (bytes[i] == b' ' || bytes[i] == b'\t') { i += 1; }
        if i < len && bytes[i] == b',' { i += 1; }

        copy_from = i;
    }

    out.push_str(&input[copy_from..]);
    out
}

/// Skip a RON value starting at `start`, returning the index just past it.
fn skip_ron_value(bytes: &[u8], start: usize) -> usize {
    let len = bytes.len();
    let mut i = start;
    if i >= len { return i; }

    match bytes[i] {
        // Delimited: balanced skip over (), {}, []
        b'(' | b'{' | b'[' => {
            let mut depth = 1u32;
            i += 1;
            while i < len && depth > 0 {
                match bytes[i] {
                    b'"' => { i = skip_ron_string(bytes, i); continue; }
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
            while i < len && (bytes[i].is_ascii_alphanumeric()
                || bytes[i] == b'_' || bytes[i] == b'.'
                || bytes[i] == b'-' || bytes[i] == b'+')
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
        if bytes[i] == b'\\' { i += 2; continue; }
        if bytes[i] == b'"' { return i + 1; }
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
    ".png", ".jpg", ".jpeg", ".bmp", ".tga", ".hdr", ".exr",
    // 3D models
    ".glb", ".gltf", ".bin",
    // Audio
    ".ogg", ".wav", ".mp3", ".flac",
    // Scenes
    ".ron",
    // Materials / shaders
    ".material", ".shader", ".wgsl",
    // Scripts
    ".lua", ".rhai",
    // Data
    ".blueprint", ".json",
    // Fonts
    ".ttf", ".otf",
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
                if bytes[i] == b'\\' { i += 2; continue; }
                if bytes[i] == b'"' { break; }
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
