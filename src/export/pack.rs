//! Pack file format for bundling assets
//!
//! Format (Renzora Pack - inspired by Godot's PCK):
//!
//! HEADER:
//!   Magic: "RPCK" (4 bytes)
//!   Version: u32 (4 bytes)
//!   Flags: u32 (4 bytes) - reserved for compression/encryption
//!   File count: u32 (4 bytes)
//!
//! FILE TABLE (repeated for each file):
//!   Path length: u32
//!   Path: UTF-8 string (variable length)
//!   Offset: u64 (from start of data section)
//!   Size: u64
//!
//! DATA SECTION:
//!   Raw file contents concatenated
//!
//! FOOTER (at very end of file):
//!   Pack start offset: u64 (offset from start of file to HEADER)
//!   Magic: "RPCK" (4 bytes)
//!
//! When appended to an exe, the runtime reads the last 12 bytes to find
//! the footer, then seeks to the pack start offset to read the header.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const PACK_MAGIC: &[u8; 4] = b"RPCK";
const PACK_VERSION: u32 = 1;

/// Entry in the pack file table
#[derive(Debug, Clone)]
pub struct PackEntry {
    pub path: String,
    pub offset: u64,
    pub size: u64,
}

/// Create a pack file from a directory
pub fn create_pack(
    source_dir: &Path,
    output_path: &Path,
) -> Result<(), String> {
    let mut files: Vec<(String, PathBuf)> = Vec::new();

    // Collect all files recursively
    collect_files(source_dir, source_dir, &mut files)?;

    if files.is_empty() {
        return Err("No files to pack".to_string());
    }

    // Create the pack file
    let file = File::create(output_path)
        .map_err(|e| format!("Failed to create pack file: {}", e))?;
    let mut writer = BufWriter::new(file);

    // Write header
    writer.write_all(PACK_MAGIC)
        .map_err(|e| format!("Failed to write magic: {}", e))?;
    writer.write_all(&PACK_VERSION.to_le_bytes())
        .map_err(|e| format!("Failed to write version: {}", e))?;
    writer.write_all(&0u32.to_le_bytes()) // flags (reserved)
        .map_err(|e| format!("Failed to write flags: {}", e))?;
    writer.write_all(&(files.len() as u32).to_le_bytes())
        .map_err(|e| format!("Failed to write file count: {}", e))?;

    // Calculate file table size to determine data section offset
    let mut file_table_size: u64 = 0;
    for (rel_path, _) in &files {
        file_table_size += 4; // path length
        file_table_size += rel_path.len() as u64; // path
        file_table_size += 8; // offset
        file_table_size += 8; // size
    }

    // Build file entries with offsets
    let mut entries: Vec<PackEntry> = Vec::new();
    let mut current_offset: u64 = 0;

    for (rel_path, full_path) in &files {
        let size = fs::metadata(full_path)
            .map_err(|e| format!("Failed to get file size for {:?}: {}", full_path, e))?
            .len();

        entries.push(PackEntry {
            path: rel_path.clone(),
            offset: current_offset,
            size,
        });

        current_offset += size;
    }

    // Write file table
    for entry in &entries {
        let path_bytes = entry.path.as_bytes();
        writer.write_all(&(path_bytes.len() as u32).to_le_bytes())
            .map_err(|e| format!("Failed to write path length: {}", e))?;
        writer.write_all(path_bytes)
            .map_err(|e| format!("Failed to write path: {}", e))?;
        writer.write_all(&entry.offset.to_le_bytes())
            .map_err(|e| format!("Failed to write offset: {}", e))?;
        writer.write_all(&entry.size.to_le_bytes())
            .map_err(|e| format!("Failed to write size: {}", e))?;
    }

    // Write data section
    for (_, full_path) in &files {
        let mut file = File::open(full_path)
            .map_err(|e| format!("Failed to open {:?}: {}", full_path, e))?;
        io::copy(&mut file, &mut writer)
            .map_err(|e| format!("Failed to copy {:?}: {}", full_path, e))?;
    }

    // Get current position (this is where we need to write the footer offset)
    let pack_end = writer.stream_position()
        .map_err(|e| format!("Failed to get position: {}", e))?;

    // The pack started at offset 0 in this file
    // When appended to exe, we need to record where it starts
    let pack_start: u64 = 0;

    // Write footer
    writer.write_all(&pack_start.to_le_bytes())
        .map_err(|e| format!("Failed to write pack offset: {}", e))?;
    writer.write_all(PACK_MAGIC)
        .map_err(|e| format!("Failed to write footer magic: {}", e))?;

    writer.flush()
        .map_err(|e| format!("Failed to flush: {}", e))?;

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
) -> Result<(), String> {
    // Create a temporary pack file
    let temp_pack = export_dir.join("_temp.pak");

    // Create pack from the export directory
    create_pack(export_dir, &temp_pack)?;

    // Append to exe
    append_pack_to_exe(runtime_exe, &temp_pack, output_exe)?;

    // Clean up temp pack
    let _ = fs::remove_file(&temp_pack);

    Ok(())
}
