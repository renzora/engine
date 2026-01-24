//! Pack file reader for runtime
//!
//! Reads assets from embedded pack data appended to the executable.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

const PACK_MAGIC: &[u8; 4] = b"RPCK";

/// Entry in the pack file table
#[derive(Debug, Clone)]
pub struct PackEntry {
    pub path: String,
    pub offset: u64,
    pub size: u64,
}

/// Pack reader for loading embedded assets
pub struct PackReader {
    /// File handle to the executable
    file: BufReader<File>,
    /// Offset where the pack data starts in the file
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
        let _version = u32::from_le_bytes(version_bytes);

        // Read flags (reserved)
        let mut flags_bytes = [0u8; 4];
        reader.read_exact(&mut flags_bytes).ok()?;
        let _flags = u32::from_le_bytes(flags_bytes);

        // Read file count
        let mut count_bytes = [0u8; 4];
        reader.read_exact(&mut count_bytes).ok()?;
        let file_count = u32::from_le_bytes(count_bytes);

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

            entries.insert(path.clone(), PackEntry { path, offset, size });
        }

        // Record where data section starts
        let data_start = reader.stream_position().ok()?;

        Some(Self {
            file: reader,
            pack_start,
            data_start,
            entries,
        })
    }

    /// Check if a file exists in the pack
    pub fn contains(&self, path: &str) -> bool {
        let normalized = path.replace('\\', "/");
        self.entries.contains_key(&normalized)
    }

    /// Read a file from the pack
    pub fn read(&mut self, path: &str) -> Option<Vec<u8>> {
        let normalized = path.replace('\\', "/");
        let entry = self.entries.get(&normalized)?.clone();

        // Seek to file data
        let file_offset = self.data_start + entry.offset;
        self.file.seek(SeekFrom::Start(file_offset)).ok()?;

        // Read file data
        let mut data = vec![0u8; entry.size as usize];
        self.file.read_exact(&mut data).ok()?;

        Some(data)
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

    /// Get the size of a file
    pub fn file_size(&self, path: &str) -> Option<u64> {
        let normalized = path.replace('\\', "/");
        self.entries.get(&normalized).map(|e| e.size)
    }
}
