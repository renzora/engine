//! `PakBackend` — abstracts where rpak bytes come from.
//!
//! The trait sits between [`crate::RpakArchive`] and the OS, so the same
//! reader code works whether the bytes live in:
//!
//! - a `Vec<u8>` ([`BytesBackend`]) — WASM, in-memory test fixtures, or rpaks
//!   handed to us as raw bytes by another layer
//! - a memory-mapped file ([`MmapBackend`]) — desktops, iOS, anywhere mmap is
//!   cheap and the OS page cache should hold the working set instead of our
//!   heap
//! - a file handle with positional reads ([`FileBackend`]) — fallback for
//!   filesystems where mmap fails (some network shares, some sandbox configs)
//!
//! A future Android-specific backend can wrap `AAssetManager` and serve
//! reads directly from the APK without copying into a `Vec<u8>` first.
//!
//! ## Lifetimes
//!
//! `read_slice` returns a `Cow<'_, [u8]>` so backends that can hand back a
//! borrowed slice (mmap, in-memory bytes) avoid the per-read allocation;
//! backends that have to issue a real syscall (file pread) return `Owned`.
//! The borrow lifetime is tied to `&self`, so the slice stays valid as long
//! as the archive does.

use std::borrow::Cow;
use std::io;

#[cfg(not(target_arch = "wasm32"))]
use std::fs::File;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

/// Abstracts byte access to the underlying rpak storage.
///
/// All offsets are absolute within the backend. Callers (notably
/// [`crate::RpakArchive`]) are responsible for translating
/// rpak-relative offsets into backend-absolute offsets when an rpak is
/// embedded inside a larger file (i.e. appended to a binary).
pub trait PakBackend: Send + Sync {
    /// Total addressable size in bytes.
    fn len(&self) -> u64;

    /// Read `size` bytes starting at `offset`.
    ///
    /// Returns a borrowed slice when the backend can serve one cheaply
    /// (mmap, in-memory) and an owned `Vec` otherwise (file pread).
    fn read_slice<'a>(&'a self, offset: u64, size: u64) -> io::Result<Cow<'a, [u8]>>;
}

// ────────────────────────────────────────────────────────────────────────
// BytesBackend — Vec<u8> in RAM
// ────────────────────────────────────────────────────────────────────────

/// Backend over an owned `Vec<u8>`. Used for WASM (where the rpak is
/// handed in by JavaScript) and for `RpakArchive::from_bytes`.
pub struct BytesBackend {
    bytes: Vec<u8>,
}

impl BytesBackend {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }
}

impl PakBackend for BytesBackend {
    fn len(&self) -> u64 {
        self.bytes.len() as u64
    }

    fn read_slice<'a>(&'a self, offset: u64, size: u64) -> io::Result<Cow<'a, [u8]>> {
        let start = offset as usize;
        let end = start
            .checked_add(size as usize)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "offset+size overflow"))?;
        if end > self.bytes.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "read past end of bytes",
            ));
        }
        Ok(Cow::Borrowed(&self.bytes[start..end]))
    }
}

// ────────────────────────────────────────────────────────────────────────
// MmapBackend — memory-mapped file
// ────────────────────────────────────────────────────────────────────────

/// Backend over a memory-mapped file. Native default on desktop and iOS:
/// the OS pages bytes in on demand from disk, so a multi-GB rpak uses
/// effectively no RSS until actually accessed.
///
/// `unsafe` block: `Mmap::map` is unsafe because the OS does not protect us
/// from another process modifying the underlying file while it's mapped.
/// In our case the rpak is read-only and shipped as part of the install,
/// so the standard "treat-as-immutable" assumption holds. (If a player
/// edits their game files mid-session, the worst case is a bad read; we
/// don't claim memory safety beyond what every other game engine assumes
/// of its data files.)
#[cfg(not(target_arch = "wasm32"))]
pub struct MmapBackend {
    mmap: memmap2::Mmap,
}

#[cfg(not(target_arch = "wasm32"))]
impl MmapBackend {
    /// Mmap an entire file. Returns the original `io::Error` from `Mmap::map`
    /// on failure so the caller can decide whether to fall back to a
    /// `FileBackend`.
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        // Reject zero-byte files up-front; mmap of empty file is a portability
        // landmine (some platforms reject, some return an empty mapping).
        let len = file.metadata()?.len();
        if len == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "cannot mmap empty file",
            ));
        }
        let mmap = unsafe { memmap2::Mmap::map(&file)? };
        Ok(Self { mmap })
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl PakBackend for MmapBackend {
    fn len(&self) -> u64 {
        self.mmap.len() as u64
    }

    fn read_slice<'a>(&'a self, offset: u64, size: u64) -> io::Result<Cow<'a, [u8]>> {
        let start = offset as usize;
        let end = start
            .checked_add(size as usize)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "offset+size overflow"))?;
        if end > self.mmap.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "read past end of mmap",
            ));
        }
        Ok(Cow::Borrowed(&self.mmap[start..end]))
    }
}

// ────────────────────────────────────────────────────────────────────────
// FileBackend — File handle + positional reads
// ────────────────────────────────────────────────────────────────────────

/// Backend over a `File` handle using positional reads (`pread64` on Unix,
/// `ReadFile`+`OVERLAPPED` on Windows via `seek_read`).
///
/// Used as a fallback when [`MmapBackend::open`] fails — for example on
/// network shares, sandbox configs that disable mmap, or filesystems that
/// don't support it. Slower than mmap (every `read_slice` is a syscall and
/// allocates) but the contract is identical.
#[cfg(not(target_arch = "wasm32"))]
pub struct FileBackend {
    file: File,
    len: u64,
}

#[cfg(not(target_arch = "wasm32"))]
impl FileBackend {
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        let len = file.metadata()?.len();
        Ok(Self { file, len })
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl PakBackend for FileBackend {
    fn len(&self) -> u64 {
        self.len
    }

    fn read_slice<'a>(&'a self, offset: u64, size: u64) -> io::Result<Cow<'a, [u8]>> {
        let mut buf = vec![0u8; size as usize];
        read_exact_at(&self.file, &mut buf, offset)?;
        Ok(Cow::Owned(buf))
    }
}

/// Cross-platform positional read. Loops until the buffer is filled or
/// errors — `seek_read` on Windows can return short reads.
#[cfg(unix)]
fn read_exact_at(file: &File, buf: &mut [u8], offset: u64) -> io::Result<()> {
    use std::os::unix::fs::FileExt;
    file.read_exact_at(buf, offset)
}

#[cfg(windows)]
fn read_exact_at(file: &File, buf: &mut [u8], offset: u64) -> io::Result<()> {
    use std::os::windows::fs::FileExt;
    let mut off = offset;
    let mut remaining: &mut [u8] = buf;
    while !remaining.is_empty() {
        let n = file.seek_read(remaining, off)?;
        if n == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "short read at offset",
            ));
        }
        off += n as u64;
        remaining = &mut remaining[n..];
    }
    Ok(())
}

#[cfg(not(any(unix, windows, target_arch = "wasm32")))]
fn read_exact_at(_file: &File, _buf: &mut [u8], _offset: u64) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "FileBackend not supported on this platform",
    ))
}
