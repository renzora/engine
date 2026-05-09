//! Reading `.rpak` v2 archives — both standalone files and embedded in binaries.
//!
//! The reader does not own the backing bytes itself; it goes through a
//! [`PakBackend`] for every read. Pick the backend that fits the platform:
//! `MmapBackend` for desktops/iOS, `BytesBackend` for WASM and in-memory
//! buffers, `FileBackend` as a portable fallback.
//!
//! For the common cases (open a file, parse `&[u8]`, find an rpak appended
//! to the running executable), the constructors below pick a sensible
//! backend automatically.

use std::collections::HashMap;
use std::io;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

use crate::backend::{BytesBackend, PakBackend};
#[cfg(not(target_arch = "wasm32"))]
use crate::backend::{FileBackend, MmapBackend};
use crate::format::{
    decode_index, detect_appended_footer, Compression, Header, PakEntry, FOOTER_LEN,
    FORMAT_VERSION, HEADER_LEN,
};

/// An rpak archive backed by a [`PakBackend`].
///
/// Header and index are decoded once at construction. Entry payloads are
/// fetched (and decompressed) on demand via [`Self::get`].
pub struct RpakArchive {
    backend: Box<dyn PakBackend>,
    /// Offset of the rpak's header magic within the backend. Zero for
    /// standalone `.rpak` files; non-zero when the rpak is appended to a
    /// host binary and we mmap the whole binary.
    rpak_start: u64,
    index: Vec<PakEntry>,
    by_path: HashMap<String, usize>,
}

impl RpakArchive {
    // ────────────────────────────────────────────────────────────────
    // Constructors
    // ────────────────────────────────────────────────────────────────

    /// Build from a backend. The backend's bytes must contain a valid v2
    /// rpak somewhere — either standalone (rpak at offset 0) or appended
    /// to a host binary (footer at the end points back at the rpak start).
    pub fn from_backend(backend: Box<dyn PakBackend>) -> io::Result<Self> {
        let rpak_start = locate_rpak_start(&*backend)?;
        Self::from_backend_at(backend, rpak_start)
    }

    /// Build from a backend with the rpak's start offset already known —
    /// used by `from_binary` to skip the footer-detection step it just did.
    fn from_backend_at(backend: Box<dyn PakBackend>, rpak_start: u64) -> io::Result<Self> {
        let total_len = backend.len();
        if rpak_start + HEADER_LEN > total_len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "rpak start past backend size",
            ));
        }

        // Decode the header.
        let header_bytes = backend.read_slice(rpak_start, HEADER_LEN)?;
        let header = Header::parse(&header_bytes)?;
        if header.version != FORMAT_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported rpak version: {}", header.version),
            ));
        }
        // Drop the header borrow before issuing the next backend read so we
        // don't hold two slices into the backend simultaneously (some
        // backends, like `BytesBackend`, are fine with that, but the trait
        // doesn't promise it).
        drop(header_bytes);

        // Decode the index.
        let idx_offset_abs = rpak_start + header.index_offset;
        let idx_size = header.index_compressed as u64;
        if idx_offset_abs + idx_size > total_len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "index range exceeds backend size",
            ));
        }
        let idx_raw = backend.read_slice(idx_offset_abs, idx_size)?;
        let idx_decoded: Vec<u8> = if header.index_is_compressed() {
            zstd::decode_all(idx_raw.as_ref())
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
        } else {
            idx_raw.to_vec()
        };
        drop(idx_raw);
        if idx_decoded.len() != header.index_uncompressed as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "index uncompressed size mismatch: header={} actual={}",
                    header.index_uncompressed,
                    idx_decoded.len()
                ),
            ));
        }
        let index = decode_index(&idx_decoded)?;

        // Sanity check: every entry must lie inside the data section.
        for entry in &index {
            let start = entry.offset;
            let end = start
                .checked_add(entry.compressed_size)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "entry size overflow"))?;
            if start < HEADER_LEN || end > header.index_offset {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "entry '{}' [{}..{}] outside data section [{}..{}]",
                        entry.path, start, end, HEADER_LEN, header.index_offset
                    ),
                ));
            }
        }

        let by_path = index
            .iter()
            .enumerate()
            .map(|(i, e)| (e.path.clone(), i))
            .collect();

        Ok(Self {
            backend,
            rpak_start,
            index,
            by_path,
        })
    }

    /// Open a standalone `.rpak` file from disk, preferring memory-mapped
    /// I/O. Falls back to a `FileBackend` if the platform refuses mmap
    /// (network shares, hardened sandboxes, ...).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_file(path: &Path) -> io::Result<Self> {
        match MmapBackend::open(path) {
            Ok(b) => Self::from_backend(Box::new(b)),
            Err(_) => {
                let b = FileBackend::open(path)?;
                Self::from_backend(Box::new(b))
            }
        }
    }

    /// Try to load an rpak archive embedded at the end of the current executable.
    ///
    /// Returns `Ok(None)` when the executable has no appended rpak.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_current_exe() -> io::Result<Option<Self>> {
        let exe_path = std::env::current_exe()?;
        Self::from_binary(&exe_path)
    }

    /// Try to load an rpak archive embedded at the end of a binary file.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_binary(binary_path: &Path) -> io::Result<Option<Self>> {
        let backend: Box<dyn PakBackend> = match MmapBackend::open(binary_path) {
            Ok(b) => Box::new(b),
            Err(_) => Box::new(FileBackend::open(binary_path)?),
        };
        let total = backend.len();
        if total < FOOTER_LEN {
            return Ok(None);
        }
        let tail = backend.read_slice(total - FOOTER_LEN, FOOTER_LEN)?;
        let tail_arr: [u8; 16] = tail.as_ref().try_into().map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "footer slice was wrong length")
        })?;
        let Some((rpak_start, _rpak_total)) = detect_appended_footer(total, &tail_arr) else {
            return Ok(None);
        };
        drop(tail);
        Ok(Some(Self::from_backend_at(backend, rpak_start)?))
    }

    /// Load from raw rpak bytes. Used for WASM (rpak handed in by JS) and
    /// for tests; production native paths should prefer [`Self::from_file`]
    /// so the OS page cache, not our heap, holds the working set.
    pub fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        let backend = BytesBackend::new(bytes.to_vec());
        Self::from_backend(Box::new(backend))
    }

    // ────────────────────────────────────────────────────────────────
    // Read API
    // ────────────────────────────────────────────────────────────────

    /// Read and decompress an entry by archive-relative path.
    ///
    /// Backslashes in the input are normalized to forward slashes so Windows
    /// callers don't need to convert separately.
    pub fn get(&self, path: &str) -> Option<Vec<u8>> {
        let key = normalize(path);
        let idx = *self.by_path.get(&key)?;
        let entry = &self.index[idx];
        self.read_entry(entry).ok()
    }

    /// Look up metadata for an entry without reading the payload.
    pub fn entry(&self, path: &str) -> Option<&PakEntry> {
        let key = normalize(path);
        let idx = *self.by_path.get(&key)?;
        Some(&self.index[idx])
    }

    /// True if the archive contains the named entry.
    pub fn contains(&self, path: &str) -> bool {
        self.by_path.contains_key(&normalize(path))
    }

    /// Iterate over every entry's archive-relative path.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.index.iter().map(|e| e.path.as_str())
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.index.len()
    }

    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// Sum of every entry's stored bytes (compressed where applicable).
    pub fn total_compressed_bytes(&self) -> u64 {
        self.index.iter().map(|e| e.compressed_size).sum()
    }

    /// Sum of every entry's uncompressed bytes — the working-set ceiling
    /// if every entry were resident at once.
    pub fn total_uncompressed_bytes(&self) -> u64 {
        self.index.iter().map(|e| e.uncompressed_size).sum()
    }

    /// Extract every entry to a directory on disk.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn extract_to(&self, output_dir: &Path) -> io::Result<()> {
        for entry in &self.index {
            let data = self.read_entry(entry)?;
            let out_path = output_dir.join(&entry.path);
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&out_path, data)?;
        }
        Ok(())
    }

    fn read_entry(&self, entry: &PakEntry) -> io::Result<Vec<u8>> {
        let raw = self
            .backend
            .read_slice(self.rpak_start + entry.offset, entry.compressed_size)?;
        match entry.compression {
            Compression::Stored => {
                if raw.len() != entry.uncompressed_size as usize {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "stored entry '{}' size mismatch ({} != {})",
                            entry.path,
                            raw.len(),
                            entry.uncompressed_size
                        ),
                    ));
                }
                Ok(raw.into_owned())
            }
            Compression::Zstd => {
                let out = zstd::decode_all(raw.as_ref())
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                if out.len() != entry.uncompressed_size as usize {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "zstd entry '{}' decompressed to {} bytes, expected {}",
                            entry.path,
                            out.len(),
                            entry.uncompressed_size
                        ),
                    ));
                }
                Ok(out)
            }
        }
    }
}

/// Find where the rpak header starts in a backend.
///
/// 1. Last 16 bytes contain the appended-to-binary footer? Use that.
/// 2. Otherwise assume standalone — rpak starts at offset 0.
fn locate_rpak_start(backend: &dyn PakBackend) -> io::Result<u64> {
    let total = backend.len();
    if total >= FOOTER_LEN {
        let tail = backend.read_slice(total - FOOTER_LEN, FOOTER_LEN)?;
        let tail_arr: [u8; 16] = tail.as_ref().try_into().map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "footer slice was wrong length")
        })?;
        if let Some((start, _total)) = detect_appended_footer(total, &tail_arr) {
            return Ok(start);
        }
    }
    Ok(0)
}

fn normalize(path: &str) -> String {
    path.replace('\\', "/")
}
