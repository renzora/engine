//! Custom asset reader for packed executables
//!
//! Reads assets directly from the embedded pack data without extracting to disk.
//! Uses lazy loading - only reads file data when actually requested.
//! Supports zstd decompression for compressed entries.

use bevy::asset::io::{AssetReader, AssetReaderError, PathStream, Reader, VecReader};
use bevy::prelude::*;
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const PACK_MAGIC: &[u8; 4] = b"RPCK";
const FLAG_COMPRESSED: u32 = 1 << 0;

/// Pack entry metadata (without the actual data)
#[derive(Clone)]
struct PackEntry {
    offset: u64,
    size: u64,
    compressed_size: u64,
    flags: u32,
}

impl PackEntry {
    fn is_compressed(&self) -> bool {
        self.flags & FLAG_COMPRESSED != 0
    }
}

/// Pack index - just stores file locations, not the actual data
#[derive(Resource, Clone)]
pub struct PackIndex {
    /// Path to the executable containing the pack
    exe_path: PathBuf,
    /// Where the data section starts in the file
    data_start: u64,
    /// File entries indexed by path
    entries: Arc<HashMap<String, PackEntry>>,
}

impl PackIndex {
    /// Load pack index from the current executable (doesn't load file data)
    pub fn from_current_exe() -> Option<Self> {
        let exe_path = std::env::current_exe().ok()?;
        let file = File::open(&exe_path).ok()?;
        let mut reader = std::io::BufReader::new(file);

        // Read footer (last 12 bytes)
        reader.seek(SeekFrom::End(-12)).ok()?;

        let mut footer = [0u8; 12];
        reader.read_exact(&mut footer).ok()?;

        // Check magic at end of footer
        if &footer[8..12] != PACK_MAGIC {
            return None; // Not a packed file
        }

        // Get pack start offset
        let pack_start = u64::from_le_bytes(footer[0..8].try_into().ok()?);

        // Seek to pack header
        reader.seek(SeekFrom::Start(pack_start)).ok()?;

        // Read and verify header magic
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic).ok()?;
        if &magic != PACK_MAGIC {
            return None;
        }

        // Read version
        let mut version_bytes = [0u8; 4];
        reader.read_exact(&mut version_bytes).ok()?;
        let version = u32::from_le_bytes(version_bytes);

        // Handle v1 vs v2 format
        let (file_count, data_start) = if version >= 2 {
            // v2: header_size + flags + file_count + data_offset
            let mut header_size_bytes = [0u8; 4];
            reader.read_exact(&mut header_size_bytes).ok()?;
            let _header_size = u32::from_le_bytes(header_size_bytes);

            let mut flags_bytes = [0u8; 4];
            reader.read_exact(&mut flags_bytes).ok()?;
            let _flags = u32::from_le_bytes(flags_bytes);

            let mut count_bytes = [0u8; 4];
            reader.read_exact(&mut count_bytes).ok()?;
            let file_count = u32::from_le_bytes(count_bytes);

            let mut data_offset_bytes = [0u8; 8];
            reader.read_exact(&mut data_offset_bytes).ok()?;
            let data_offset = u64::from_le_bytes(data_offset_bytes);

            (file_count, pack_start + data_offset)
        } else {
            // v1: flags + file_count (no header_size or data_offset)
            let mut flags_bytes = [0u8; 4];
            reader.read_exact(&mut flags_bytes).ok()?;

            let mut count_bytes = [0u8; 4];
            reader.read_exact(&mut count_bytes).ok()?;
            let file_count = u32::from_le_bytes(count_bytes);

            // For v1, we'll calculate data_start after reading entries
            (file_count, 0) // placeholder, will be set after reading entries
        };

        // Read file table
        let mut entries = HashMap::new();

        for _ in 0..file_count {
            // Path length
            let mut len_bytes = [0u8; 4];
            reader.read_exact(&mut len_bytes).ok()?;
            let path_len = u32::from_le_bytes(len_bytes) as usize;

            // Path
            let mut path_bytes = vec![0u8; path_len];
            reader.read_exact(&mut path_bytes).ok()?;
            let path = String::from_utf8(path_bytes).ok()?;

            // Offset
            let mut offset_bytes = [0u8; 8];
            reader.read_exact(&mut offset_bytes).ok()?;
            let offset = u64::from_le_bytes(offset_bytes);

            // Size (original/uncompressed)
            let mut size_bytes = [0u8; 8];
            reader.read_exact(&mut size_bytes).ok()?;
            let size = u64::from_le_bytes(size_bytes);

            // v2 has compressed_size and flags
            let (compressed_size, flags) = if version >= 2 {
                let mut compressed_size_bytes = [0u8; 8];
                reader.read_exact(&mut compressed_size_bytes).ok()?;
                let compressed_size = u64::from_le_bytes(compressed_size_bytes);

                let mut flags_bytes = [0u8; 4];
                reader.read_exact(&mut flags_bytes).ok()?;
                let flags = u32::from_le_bytes(flags_bytes);

                (compressed_size, flags)
            } else {
                // v1: no compression
                (size, 0)
            };

            // Normalize path (use forward slashes)
            let normalized_path = path.replace('\\', "/");
            entries.insert(
                normalized_path,
                PackEntry {
                    offset,
                    size,
                    compressed_size,
                    flags,
                },
            );
        }

        // For v1, data starts right after the file table
        let final_data_start = if version >= 2 {
            data_start
        } else {
            reader.stream_position().ok()?
        };

        println!(
            "Pack v{} loaded with {} files:",
            version,
            entries.len()
        );
        for (path, entry) in entries.iter() {
            if entry.is_compressed() {
                println!(
                    "  - {} ({} -> {} bytes, compressed)",
                    path, entry.size, entry.compressed_size
                );
            } else {
                println!("  - {} ({} bytes)", path, entry.size);
            }
        }

        Some(Self {
            exe_path,
            data_start: final_data_start,
            entries: Arc::new(entries),
        })
    }

    /// Read file data on-demand from the pack (handles decompression)
    pub fn read_file(&self, path: &str) -> Option<Vec<u8>> {
        let normalized = path.replace('\\', "/");
        let entry = self.entries.get(&normalized)?;

        // Open the exe file fresh (this is thread-safe)
        let mut file = File::open(&self.exe_path).ok()?;

        // Seek to file data
        let file_offset = self.data_start + entry.offset;
        file.seek(SeekFrom::Start(file_offset)).ok()?;

        // Read the stored data (might be compressed)
        let mut stored_data = vec![0u8; entry.compressed_size as usize];
        file.read_exact(&mut stored_data).ok()?;

        // Decompress if needed
        if entry.is_compressed() {
            zstd::bulk::decompress(&stored_data, entry.size as usize).ok()
        } else {
            Some(stored_data)
        }
    }

    /// Read file as string
    pub fn read_string(&self, path: &str) -> Option<String> {
        let data = self.read_file(path)?;
        String::from_utf8(data).ok()
    }

    /// Check if file exists in pack
    pub fn contains(&self, path: &str) -> bool {
        let normalized = path.replace('\\', "/");
        self.entries.contains_key(&normalized)
    }
}

/// Custom asset reader that reads from pack data on-demand
pub struct PackAssetReader {
    pack_index: PackIndex,
}

impl PackAssetReader {
    pub fn new(pack_index: PackIndex) -> Self {
        Self { pack_index }
    }

    fn normalize_asset_path(path: &Path) -> String {
        let path_str = path.to_string_lossy();

        // Try with assets/ prefix for pack lookup
        let with_assets = if path_str.starts_with("assets/") || path_str.starts_with("assets\\") {
            path_str.to_string()
        } else {
            format!("assets/{}", path_str)
        };

        with_assets.replace('\\', "/")
    }
}

impl AssetReader for PackAssetReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        let pack_path = Self::normalize_asset_path(path);

        if let Some(data) = self.pack_index.read_file(&pack_path) {
            println!("PackAssetReader: Loaded '{}' ({} bytes)", pack_path, data.len());
            Ok(VecReader::new(data))
        } else {
            println!("PackAssetReader: '{}' not found in pack", pack_path);
            Err(AssetReaderError::NotFound(path.to_path_buf()))
        }
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<impl Reader + 'a, AssetReaderError> {
        // Meta files are optional, return not found
        Err::<VecReader, _>(AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        // We don't support directory listing from pack
        Err(AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn is_directory<'a>(&'a self, _path: &'a Path) -> Result<bool, AssetReaderError> {
        Ok(false)
    }
}
