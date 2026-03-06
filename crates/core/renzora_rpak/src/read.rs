//! Reading `.rpak` archives — both standalone files and embedded in binaries.

use std::collections::HashMap;
use std::io;
use std::path::Path;

/// An in-memory rpak archive. All file contents are held in memory after loading.
pub struct RpakArchive {
    /// archive-relative path (forward slashes) -> file contents
    files: HashMap<String, Vec<u8>>,
}

impl RpakArchive {
    /// Load from a standalone `.rpak` file.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_file(path: &Path) -> io::Result<Self> {
        let compressed = std::fs::read(path)?;
        Self::from_compressed(&compressed)
    }

    /// Try to load an rpak archive embedded at the end of the current executable.
    ///
    /// Returns `None` if the executable doesn't have an appended rpak.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_current_exe() -> io::Result<Option<Self>> {
        let exe_path = std::env::current_exe()?;
        Self::from_binary(&exe_path)
    }

    /// Load from raw compressed bytes (useful for WASM or in-memory data).
    pub fn from_bytes(compressed: &[u8]) -> io::Result<Self> {
        Self::from_compressed(compressed)
    }

    /// Try to load an rpak archive embedded at the end of a binary.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_binary(binary_path: &Path) -> io::Result<Option<Self>> {
        let data = std::fs::read(binary_path)?;

        // Check for magic footer
        if data.len() < 12 {
            return Ok(None);
        }

        let magic_start = data.len() - 4;
        if &data[magic_start..] != crate::RPAK_MAGIC {
            return Ok(None);
        }

        // Read compressed data size
        let size_start = magic_start - 8;
        let size_bytes: [u8; 8] = data[size_start..magic_start]
            .try_into()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid rpak footer"))?;
        let compressed_size = u64::from_le_bytes(size_bytes) as usize;

        if size_start < compressed_size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "rpak size exceeds binary",
            ));
        }

        let compressed_start = size_start - compressed_size;
        let compressed = &data[compressed_start..size_start];

        Ok(Some(Self::from_compressed(compressed)?))
    }

    /// Decompress and parse an rpak blob.
    fn from_compressed(compressed: &[u8]) -> io::Result<Self> {
        let raw = zstd::decode_all(compressed)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        Self::parse(&raw)
    }

    fn parse(raw: &[u8]) -> io::Result<Self> {
        let mut pos = 0;

        let read_u32 = |pos: &mut usize| -> io::Result<u32> {
            if *pos + 4 > raw.len() {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "truncated rpak"));
            }
            let bytes: [u8; 4] = raw[*pos..*pos + 4].try_into().unwrap();
            *pos += 4;
            Ok(u32::from_le_bytes(bytes))
        };

        let read_u64 = |pos: &mut usize| -> io::Result<u64> {
            if *pos + 8 > raw.len() {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "truncated rpak"));
            }
            let bytes: [u8; 8] = raw[*pos..*pos + 8].try_into().unwrap();
            *pos += 8;
            Ok(u64::from_le_bytes(bytes))
        };

        let entry_count = read_u32(&mut pos)? as usize;

        // Read index
        let mut index: Vec<(String, u64, u64)> = Vec::with_capacity(entry_count);
        for _ in 0..entry_count {
            let path_len = read_u32(&mut pos)? as usize;
            if pos + path_len > raw.len() {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "truncated rpak path"));
            }
            let path = std::str::from_utf8(&raw[pos..pos + path_len])
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
                .to_string();
            pos += path_len;

            let offset = read_u64(&mut pos)?;
            let size = read_u64(&mut pos)?;
            index.push((path, offset, size));
        }

        // Data section starts at current pos
        let data_start = pos;

        let mut files = HashMap::with_capacity(entry_count);
        for (path, offset, size) in index {
            let start = data_start + offset as usize;
            let end = start + size as usize;
            if end > raw.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("rpak entry '{}' extends past end of data", path),
                ));
            }
            files.insert(path, raw[start..end].to_vec());
        }

        Ok(Self { files })
    }

    /// Get file contents by archive-relative path.
    pub fn get(&self, path: &str) -> Option<&[u8]> {
        let normalized = path.replace('\\', "/");
        self.files.get(&normalized).map(|v| v.as_slice())
    }

    /// Check if a file exists in the archive.
    pub fn contains(&self, path: &str) -> bool {
        let normalized = path.replace('\\', "/");
        self.files.contains_key(&normalized)
    }

    /// Iterate over all file paths in the archive.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(|s| s.as_str())
    }

    /// Number of files in the archive.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Extract all files to a directory on disk.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn extract_to(&self, output_dir: &Path) -> io::Result<()> {
        for (path, data) in &self.files {
            let out_path = output_dir.join(path);
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&out_path, data)?;
        }
        Ok(())
    }
}
