//! Pack file reader for runtime
//!
//! Reads assets from embedded pack data appended to the executable.
//! Supports v1 (uncompressed) and v2 (zstd compressed) formats.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

const PACK_MAGIC: &[u8; 4] = b"RPCK";
const FLAG_COMPRESSED: u32 = 1 << 0;

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

/// Pack reader for loading embedded assets
pub struct PackReader {
    /// File handle to the executable
    file: BufReader<File>,
    /// Offset where the pack data starts in the file
    #[allow(dead_code)]
    pack_start: u64,
    /// Offset where the data section starts (after header + file table)
    data_start: u64,
    /// File entries indexed by path
    entries: HashMap<String, PackEntry>,
}

impl PackReader {
    /// Try to open a pack from the current executable
    pub fn from_current_exe() -> Option<Self> {
        let exe_path = std::env::current_exe().ok()?;
        Self::open(&exe_path)
    }

    /// Open a pack file or packed executable
    pub fn open(path: &Path) -> Option<Self> {
        let file = File::open(path).ok()?;
        let mut reader = BufReader::new(file);

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

            let mut flags_bytes = [0u8; 4];
            reader.read_exact(&mut flags_bytes).ok()?;

            let mut count_bytes = [0u8; 4];
            reader.read_exact(&mut count_bytes).ok()?;
            let file_count = u32::from_le_bytes(count_bytes);

            let mut data_offset_bytes = [0u8; 8];
            reader.read_exact(&mut data_offset_bytes).ok()?;
            let data_offset = u64::from_le_bytes(data_offset_bytes);

            (file_count, pack_start + data_offset)
        } else {
            // v1: flags + file_count
            let mut flags_bytes = [0u8; 4];
            reader.read_exact(&mut flags_bytes).ok()?;

            let mut count_bytes = [0u8; 4];
            reader.read_exact(&mut count_bytes).ok()?;
            let file_count = u32::from_le_bytes(count_bytes);

            (file_count, 0) // placeholder
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

            // Size
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
                (size, 0)
            };

            entries.insert(
                path.clone(),
                PackEntry {
                    path,
                    offset,
                    size,
                    compressed_size,
                    flags,
                },
            );
        }

        // For v1, data starts after file table
        let final_data_start = if version >= 2 {
            data_start
        } else {
            reader.stream_position().ok()?
        };

        Some(Self {
            file: reader,
            pack_start,
            data_start: final_data_start,
            entries,
        })
    }

    /// Check if a file exists in the pack
    pub fn contains(&self, path: &str) -> bool {
        let normalized = path.replace('\\', "/");
        self.entries.contains_key(&normalized)
    }

    /// Read a file from the pack (handles decompression)
    pub fn read(&mut self, path: &str) -> Option<Vec<u8>> {
        let normalized = path.replace('\\', "/");
        let entry = self.entries.get(&normalized)?.clone();

        // Seek to file data
        let file_offset = self.data_start + entry.offset;
        self.file.seek(SeekFrom::Start(file_offset)).ok()?;

        // Read stored data (might be compressed)
        let mut stored_data = vec![0u8; entry.compressed_size as usize];
        self.file.read_exact(&mut stored_data).ok()?;

        // Decompress if needed
        if entry.is_compressed() {
            zstd::bulk::decompress(&stored_data, entry.size as usize).ok()
        } else {
            Some(stored_data)
        }
    }

    /// Read a file as string
    pub fn read_string(&mut self, path: &str) -> Option<String> {
        let data = self.read(path)?;
        String::from_utf8(data).ok()
    }

    /// List all files in the pack
    pub fn list_files(&self) -> Vec<&str> {
        self.entries.keys().map(|s| s.as_str()).collect()
    }

    /// Get the original (uncompressed) size of a file
    pub fn file_size(&self, path: &str) -> Option<u64> {
        let normalized = path.replace('\\', "/");
        self.entries.get(&normalized).map(|e| e.size)
    }

    /// Get the stored (possibly compressed) size of a file
    pub fn file_stored_size(&self, path: &str) -> Option<u64> {
        let normalized = path.replace('\\', "/");
        self.entries.get(&normalized).map(|e| e.compressed_size)
    }

    /// Check if a file is compressed
    pub fn is_compressed(&self, path: &str) -> Option<bool> {
        let normalized = path.replace('\\', "/");
        self.entries.get(&normalized).map(|e| e.is_compressed())
    }
}
