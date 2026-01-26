//! Pack file format for bundling assets
//!
//! Format (Renzora Pack v2 - inspired by Godot's PCK):
//!
//! HEADER (28 bytes):
//!   Magic: "RPCK" (4 bytes)
//!   Version: u32 (4 bytes) - currently 2
//!   Header size: u32 (4 bytes) - for forward compatibility
//!   Flags: u32 (4 bytes) - pack-level flags (reserved)
//!   File count: u32 (4 bytes)

#![allow(dead_code)]
//!   Data offset: u64 (8 bytes) - offset from pack start to data section
//!
//! FILE TABLE (repeated for each file):
//!   Path length: u32
//!   Path: UTF-8 string (variable length)
//!   Offset: u64 (from start of data section)
//!   Size: u64 (original uncompressed size)
//!   Compressed size: u64 (size in pack, same as size if not compressed)
//!   Flags: u32 (bit 0 = compressed with zstd)
//!
//! DATA SECTION:
//!   File contents (compressed or raw) concatenated
//!
//! FOOTER (12 bytes, at very end of file):
//!   Pack start offset: u64 (offset from start of file to HEADER)
//!   Magic: "RPCK" (4 bytes)
//!
//! When appended to an exe, the runtime reads the last 12 bytes to find
//! the footer, then seeks to the pack start offset to read the header.

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::core::ExportLogger;

const PACK_MAGIC: &[u8; 4] = b"RPCK";
const PACK_VERSION: u32 = 2;
const HEADER_SIZE: u32 = 28;

// File entry flags
const FLAG_COMPRESSED: u32 = 1 << 0;

// Extensions that are already compressed (skip zstd)
const SKIP_COMPRESSION: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "ktx2",  // images
    "mp3", "ogg", "flac", "aac",           // audio
    "glb",                                  // binary gltf (contains compressed data)
    "zip", "gz", "zst", "br",              // archives
];

/// Entry in the pack file table
#[derive(Debug, Clone)]
pub struct PackEntry {
    pub path: String,
    pub offset: u64,
    pub size: u64,
    pub compressed_size: u64,
    pub flags: u32,
}

impl PackEntry {
    pub fn is_compressed(&self) -> bool {
        self.flags & FLAG_COMPRESSED != 0
    }
}

/// Check if a file extension should skip compression
fn should_skip_compression(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| SKIP_COMPRESSION.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Compress data with zstd, returns None if compression doesn't help
fn compress_data(data: &[u8]) -> Option<Vec<u8>> {
    // Use compression level 3 (good balance of speed/ratio)
    let compressed = zstd::bulk::compress(data, 3).ok()?;

    // Only use compression if it actually saves space (>5% reduction)
    if compressed.len() < data.len() * 95 / 100 {
        Some(compressed)
    } else {
        None
    }
}

/// Create a pack file from a directory
pub fn create_pack(source_dir: &Path, output_path: &Path, logger: &ExportLogger) -> Result<(), String> {
    let mut files: Vec<(String, PathBuf)> = Vec::new();

    // Collect all files recursively
    collect_files(source_dir, source_dir, &mut files)?;

    if files.is_empty() {
        return Err("No files to pack".to_string());
    }

    logger.info(format!("Packing {} files with zstd compression...", files.len()));

    // First pass: read and compress all files, build entries
    let mut file_data: Vec<Vec<u8>> = Vec::new();
    let mut entries: Vec<PackEntry> = Vec::new();
    let mut current_offset: u64 = 0;
    let mut total_original: u64 = 0;
    let mut total_compressed: u64 = 0;

    for (rel_path, full_path) in &files {
        let raw_data = fs::read(full_path)
            .map_err(|e| format!("Failed to read {:?}: {}", full_path, e))?;

        let original_size = raw_data.len() as u64;
        total_original += original_size;

        // Decide whether to compress
        let (data, compressed_size, flags) = if should_skip_compression(full_path) {
            // Already compressed format, store as-is
            let size = raw_data.len() as u64;
            (raw_data, size, 0)
        } else if let Some(compressed) = compress_data(&raw_data) {
            // Compression helped
            let size = compressed.len() as u64;
            (compressed, size, FLAG_COMPRESSED)
        } else {
            // Compression didn't help, store as-is
            let size = raw_data.len() as u64;
            (raw_data, size, 0)
        };

        total_compressed += compressed_size;

        entries.push(PackEntry {
            path: rel_path.clone(),
            offset: current_offset,
            size: original_size,
            compressed_size,
            flags,
        });

        current_offset += compressed_size;
        file_data.push(data);
    }

    // Calculate file table size
    let mut file_table_size: u64 = 0;
    for entry in &entries {
        file_table_size += 4; // path length
        file_table_size += entry.path.len() as u64; // path
        file_table_size += 8; // offset
        file_table_size += 8; // size
        file_table_size += 8; // compressed_size
        file_table_size += 4; // flags
    }

    // Data offset = header size + file table size
    let data_offset = HEADER_SIZE as u64 + file_table_size;

    // Create the pack file
    let file = File::create(output_path)
        .map_err(|e| format!("Failed to create pack file: {}", e))?;
    let mut writer = BufWriter::new(file);

    // Write header (28 bytes)
    writer
        .write_all(PACK_MAGIC)
        .map_err(|e| format!("Failed to write magic: {}", e))?;
    writer
        .write_all(&PACK_VERSION.to_le_bytes())
        .map_err(|e| format!("Failed to write version: {}", e))?;
    writer
        .write_all(&HEADER_SIZE.to_le_bytes())
        .map_err(|e| format!("Failed to write header size: {}", e))?;
    writer
        .write_all(&0u32.to_le_bytes()) // flags (reserved)
        .map_err(|e| format!("Failed to write flags: {}", e))?;
    writer
        .write_all(&(files.len() as u32).to_le_bytes())
        .map_err(|e| format!("Failed to write file count: {}", e))?;
    writer
        .write_all(&data_offset.to_le_bytes())
        .map_err(|e| format!("Failed to write data offset: {}", e))?;

    // Write file table
    for entry in &entries {
        let path_bytes = entry.path.as_bytes();
        writer
            .write_all(&(path_bytes.len() as u32).to_le_bytes())
            .map_err(|e| format!("Failed to write path length: {}", e))?;
        writer
            .write_all(path_bytes)
            .map_err(|e| format!("Failed to write path: {}", e))?;
        writer
            .write_all(&entry.offset.to_le_bytes())
            .map_err(|e| format!("Failed to write offset: {}", e))?;
        writer
            .write_all(&entry.size.to_le_bytes())
            .map_err(|e| format!("Failed to write size: {}", e))?;
        writer
            .write_all(&entry.compressed_size.to_le_bytes())
            .map_err(|e| format!("Failed to write compressed size: {}", e))?;
        writer
            .write_all(&entry.flags.to_le_bytes())
            .map_err(|e| format!("Failed to write flags: {}", e))?;
    }

    // Write data section
    for data in &file_data {
        writer
            .write_all(data)
            .map_err(|e| format!("Failed to write file data: {}", e))?;
    }

    // Write footer (pack_start = 0 for standalone pack file)
    let pack_start: u64 = 0;
    writer
        .write_all(&pack_start.to_le_bytes())
        .map_err(|e| format!("Failed to write pack offset: {}", e))?;
    writer
        .write_all(PACK_MAGIC)
        .map_err(|e| format!("Failed to write footer magic: {}", e))?;

    writer
        .flush()
        .map_err(|e| format!("Failed to flush: {}", e))?;

    // Log compression stats
    let ratio = if total_original > 0 {
        (total_compressed as f64 / total_original as f64) * 100.0
    } else {
        100.0
    };

    // Format sizes nicely
    let format_size = |bytes: u64| -> String {
        if bytes >= 1024 * 1024 {
            format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
        } else if bytes >= 1024 {
            format!("{:.1} KB", bytes as f64 / 1024.0)
        } else {
            format!("{} bytes", bytes)
        }
    };

    logger.success(format!(
        "Pack complete: {} → {} ({:.1}% of original)",
        format_size(total_original),
        format_size(total_compressed),
        ratio
    ));

    Ok(())
}

/// Append a pack file to an executable, creating a self-contained binary
pub fn append_pack_to_exe(
    exe_path: &Path,
    pack_path: &Path,
    output_path: &Path,
) -> Result<(), String> {
    // Read the original exe
    let exe_data = fs::read(exe_path)
        .map_err(|e| format!("Failed to read exe: {}", e))?;

    // Read the pack file (without its footer, we'll write a new one)
    let pack_data = fs::read(pack_path)
        .map_err(|e| format!("Failed to read pack: {}", e))?;

    // The pack data without its footer (last 12 bytes)
    let pack_content = &pack_data[..pack_data.len() - 12];

    // Create output file
    let mut output = File::create(output_path)
        .map_err(|e| format!("Failed to create output: {}", e))?;

    // Write exe
    output.write_all(&exe_data)
        .map_err(|e| format!("Failed to write exe: {}", e))?;

    // Record where the pack starts (right after exe)
    let pack_start = exe_data.len() as u64;

    // Write pack content (without old footer)
    output.write_all(pack_content)
        .map_err(|e| format!("Failed to write pack: {}", e))?;

    // Write new footer with correct offset
    output.write_all(&pack_start.to_le_bytes())
        .map_err(|e| format!("Failed to write pack offset: {}", e))?;
    output.write_all(PACK_MAGIC)
        .map_err(|e| format!("Failed to write footer magic: {}", e))?;

    output.flush()
        .map_err(|e| format!("Failed to flush: {}", e))?;

    Ok(())
}

/// Collect files recursively from a directory
fn collect_files(
    base_dir: &Path,
    current_dir: &Path,
    files: &mut Vec<(String, PathBuf)>,
) -> Result<(), String> {
    if !current_dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(current_dir)
        .map_err(|e| format!("Failed to read dir {:?}: {}", current_dir, e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();

        if path.is_dir() {
            collect_files(base_dir, &path, files)?;
        } else {
            // Get relative path using forward slashes (cross-platform)
            let rel_path = path.strip_prefix(base_dir)
                .map_err(|_| "Failed to get relative path".to_string())?
                .to_string_lossy()
                .replace('\\', "/");

            files.push((rel_path, path));
        }
    }

    Ok(())
}

/// Create a pack from export data and append to exe
pub fn create_packed_exe(
    runtime_exe: &Path,
    export_dir: &Path,
    output_exe: &Path,
    logger: &ExportLogger,
) -> Result<(), String> {
    // Create a temporary pack file
    let temp_pack = export_dir.join("_temp.pak");

    // Create pack from the export directory
    create_pack(export_dir, &temp_pack, logger)?;

    // Append to exe
    logger.info("Appending pack to runtime executable...");
    append_pack_to_exe(runtime_exe, &temp_pack, output_exe)?;

    // Log final exe size
    if let Ok(meta) = fs::metadata(output_exe) {
        let size = meta.len();
        let format_size = |bytes: u64| -> String {
            if bytes >= 1024 * 1024 {
                format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
            } else if bytes >= 1024 {
                format!("{:.1} KB", bytes as f64 / 1024.0)
            } else {
                format!("{} bytes", bytes)
            }
        };
        logger.info(format!("Final executable size: {}", format_size(size)));
    }

    // Clean up temp pack
    let _ = fs::remove_file(&temp_pack);

    Ok(())
}
