//! Packing: create `.rpak` archives from a project directory.

use std::collections::BTreeMap;
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

/// Recursively pack an entire directory into an `.rpak` archive.
///
/// Returns the packer so you can call `write_to_file` or `append_to_binary`.
pub fn pack_directory(project_dir: &Path) -> io::Result<RpakPacker> {
    let mut packer = RpakPacker::new();

    fn walk(packer: &mut RpakPacker, base: &Path, dir: &Path) -> io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                // Skip hidden dirs and common non-asset dirs
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with('.') || name_str == "target" {
                    continue;
                }
                walk(packer, base, &path)?;
            } else {
                packer.add_from_disk(base, &path)?;
            }
        }
        Ok(())
    }

    walk(&mut packer, project_dir, project_dir)?;
    Ok(packer)
}
