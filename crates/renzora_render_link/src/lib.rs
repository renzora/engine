//! The shared-memory link between the editor and its separate render process.
//!
//! # Why a separate process at all
//!
//! The editor UI and the 3D viewport used to share one renderer and one frame,
//! so a scene that took 150 ms to render held every panel to 6 FPS. With the
//! viewport rendered by a child process (`renzora --render-host <file>`), the
//! editor's own frames only draw the UI, and they never wait on the scene.
//!
//! # The layout
//!
//! Both processes map the same file. The editor creates it; the render host
//! opens it. It holds a [`Header`], then two frame buffers of
//! [`MAX_WIDTH`] x [`MAX_HEIGHT`] BGRA8 pixels each.
//!
//! - **Editor to host**: a [`ViewRequest`] (camera pose, projection, size),
//!   written under a seqlock, so the host never reads half an update.
//! - **Host to editor**: finished frames, alternating between the two buffers.
//!   Each buffer has its own seqlock, so the editor can tell when the host
//!   started overwriting the buffer it was copying and drop that copy.
//!
//! A file in the temp folder rather than a named OS mapping: `memmap2` is
//! already in the build, works on every desktop platform, and the OS keeps a
//! small, hot file in the page cache, so both mappings hit the same memory.
//!
//! # Lifetime
//!
//! The host also watches its stdin, which the editor holds open: when the
//! editor exits, or crashes, the pipe closes and the host exits too. That lives
//! in the host, not here; this crate is only the layout.

mod messages;
pub use messages::{EditorEntity, SceneMessage};

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::Link;

#[cfg(not(target_arch = "wasm32"))]
use std::fs::OpenOptions;
use std::io;
use std::path::Path;
use std::sync::atomic::{fence, AtomicU32, AtomicU64, Ordering};

#[cfg(not(target_arch = "wasm32"))]
use memmap2::MmapMut;

/// Identifies the file as a render link ("RNDL").
pub const MAGIC: u32 = 0x4C44_4E52;
/// Bumped whenever the layout changes; both sides must agree.
pub const VERSION: u32 = 1;
/// The largest frame the link carries. A bigger viewport is rendered at this
/// size and scaled up by the panel, which keeps the file a fixed size.
pub const MAX_WIDTH: u32 = 3840;
pub const MAX_HEIGHT: u32 = 2160;
/// Room reserved for the header. Far more than it needs, so the frame buffers
/// start page-aligned and the header can grow without moving them.
const HEADER_BYTES: usize = 4096;
const FRAME_BYTES: usize = MAX_WIDTH as usize * MAX_HEIGHT as usize * 4;
/// The whole file: header, then two frame buffers.
pub const FILE_BYTES: usize = HEADER_BYTES + 2 * FRAME_BYTES;

/// What the editor wants rendered. Plain data, copied whole under a seqlock.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewRequest {
    /// The camera's world translation.
    pub translation: [f32; 3],
    /// The camera's world rotation, as a quaternion (x, y, z, w).
    pub rotation: [f32; 4],
    /// `1` for a perspective projection, `0` for orthographic.
    pub perspective: u32,
    /// Vertical field of view, radians. Perspective only.
    pub fov_y: f32,
    pub near: f32,
    pub far: f32,
    /// World units per pixel scale. Orthographic only.
    pub ortho_scale: f32,
    /// Frame size, in pixels. Clamped to [`MAX_WIDTH`] x [`MAX_HEIGHT`].
    pub width: u32,
    pub height: u32,
    /// Bumped by the editor whenever the scene file on disk should be
    /// reloaded (it was saved). The host reloads when it sees a new value.
    pub scene_generation: u32,
}

/// One frame buffer's bookkeeping.
#[repr(C)]
struct FrameSlot {
    /// Seqlock: odd while the host is writing this buffer.
    seq: AtomicU64,
    width: AtomicU32,
    height: AtomicU32,
}

/// The fixed header at the start of the file.
#[repr(C)]
struct Header {
    magic: AtomicU32,
    version: AtomicU32,
    /// Seqlock over `view`: odd while the editor is writing it.
    view_seq: AtomicU64,
    view: ViewRequest,
    /// Number of the newest complete frame, starting at 1. The frame lives in
    /// buffer `number % 2`. Zero until the host has published anything.
    latest: AtomicU64,
    frames: [FrameSlot; 2],
}

// The header must fit its reservation, or the frame buffers would overlap it.
const _: () = assert!(std::mem::size_of::<Header>() <= HEADER_BYTES);

/// One side's mapping of the link file.
#[cfg(not(target_arch = "wasm32"))]
pub struct Link {
    map: MmapMut,
}

#[cfg(not(target_arch = "wasm32"))]
impl Link {
    /// Create the link file (the editor's side), replacing any left over from a
    /// previous run, and stamp the header.
    pub fn create(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        file.set_len(FILE_BYTES as u64)?;
        // SAFETY: the file is ours and sized above. The other process only ever
        // touches it through the same atomics and seqlocks this type uses.
        let map = unsafe { MmapMut::map_mut(&file)? };
        let link = Self { map };
        let header = link.header();
        header.version.store(VERSION, Ordering::Relaxed);
        header.magic.store(MAGIC, Ordering::Release);
        Ok(link)
    }

    /// Open a link file the editor created (the render host's side).
    pub fn open(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        if file.metadata()?.len() < FILE_BYTES as u64 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "render link file is too small"));
        }
        // SAFETY: as in `create`.
        let map = unsafe { MmapMut::map_mut(&file)? };
        let link = Self { map };
        let header = link.header();
        if header.magic.load(Ordering::Acquire) != MAGIC {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "not a render link file"));
        }
        let version = header.version.load(Ordering::Relaxed);
        if version != VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("render link version {version}, this build speaks {VERSION}"),
            ));
        }
        Ok(link)
    }

    fn header(&self) -> &Header {
        // SAFETY: the map is page-aligned and at least `HEADER_BYTES` long, and
        // every field the other process writes is an atomic or seqlock-guarded.
        unsafe { &*(self.map.as_ptr() as *const Header) }
    }

    fn view_ptr(&self) -> *mut ViewRequest {
        let header = self.map.as_ptr() as *mut Header;
        // SAFETY: points inside the mapping; see `header`.
        unsafe { std::ptr::addr_of_mut!((*header).view) }
    }

    fn frame_ptr(&self, slot: usize) -> *mut u8 {
        // SAFETY: both buffers lie inside the mapping; `FILE_BYTES` covers them.
        unsafe { (self.map.as_ptr() as *mut u8).add(HEADER_BYTES + slot * FRAME_BYTES) }
    }

    /// Editor: publish what to render next.
    pub fn write_view(&self, view: &ViewRequest) {
        let mut view = *view;
        view.width = view.width.clamp(1, MAX_WIDTH);
        view.height = view.height.clamp(1, MAX_HEIGHT);
        let seq = &self.header().view_seq;
        seq.fetch_add(1, Ordering::AcqRel);
        fence(Ordering::Release);
        // SAFETY: the host only reads this under the seqlock, and drops a read
        // that overlapped this write.
        unsafe { std::ptr::write_volatile(self.view_ptr(), view) };
        fence(Ordering::Release);
        seq.fetch_add(1, Ordering::AcqRel);
    }

    /// Host: the latest view request, or `None` if the editor has not written
    /// one yet (or was mid-write; the next frame will see it).
    pub fn read_view(&self) -> Option<ViewRequest> {
        let seq = &self.header().view_seq;
        let before = seq.load(Ordering::Acquire);
        if before == 0 || before % 2 == 1 {
            return None;
        }
        fence(Ordering::Acquire);
        // SAFETY: a torn copy is detected below and discarded.
        let view = unsafe { std::ptr::read_volatile(self.view_ptr()) };
        fence(Ordering::Acquire);
        (seq.load(Ordering::Acquire) == before).then_some(view)
    }

    /// Host: publish a frame. `fill` writes `width * height * 4` tightly
    /// packed BGRA8 bytes into the slice it is handed.
    pub fn publish_frame(&self, width: u32, height: u32, fill: impl FnOnce(&mut [u8])) {
        let width = width.clamp(1, MAX_WIDTH);
        let height = height.clamp(1, MAX_HEIGHT);
        let header = self.header();
        let number = header.latest.load(Ordering::Acquire) + 1;
        let slot_index = (number % 2) as usize;
        let slot = &header.frames[slot_index];
        slot.seq.fetch_add(1, Ordering::AcqRel);
        fence(Ordering::Release);
        slot.width.store(width, Ordering::Relaxed);
        slot.height.store(height, Ordering::Relaxed);
        let len = width as usize * height as usize * 4;
        // SAFETY: inside this slot's buffer; `len` is at most `FRAME_BYTES`.
        let bytes = unsafe { std::slice::from_raw_parts_mut(self.frame_ptr(slot_index), len) };
        fill(bytes);
        fence(Ordering::Release);
        slot.seq.fetch_add(1, Ordering::AcqRel);
        header.latest.store(number, Ordering::Release);
    }

    /// Editor: copy the newest frame into `out` if it is newer than `seen`.
    /// Returns its number and size, or `None` when there is nothing new or the
    /// host overwrote it mid-copy.
    pub fn read_frame(&self, seen: u64, out: &mut Vec<u8>) -> Option<(u64, u32, u32)> {
        let header = self.header();
        let number = header.latest.load(Ordering::Acquire);
        if number == 0 || number == seen {
            return None;
        }
        let slot_index = (number % 2) as usize;
        let slot = &header.frames[slot_index];
        let before = slot.seq.load(Ordering::Acquire);
        if before % 2 == 1 {
            return None;
        }
        fence(Ordering::Acquire);
        let width = slot.width.load(Ordering::Relaxed);
        let height = slot.height.load(Ordering::Relaxed);
        let len = width as usize * height as usize * 4;
        if len == 0 || len > FRAME_BYTES {
            return None;
        }
        out.clear();
        // SAFETY: inside this slot's buffer; a torn copy is discarded below.
        out.extend_from_slice(unsafe { std::slice::from_raw_parts(self.frame_ptr(slot_index), len) });
        fence(Ordering::Acquire);
        (slot.seq.load(Ordering::Acquire) == before).then_some((number, width, height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("renzora-link-test-{}-{name}", std::process::id()))
    }

    #[test]
    fn view_round_trips_between_two_mappings() {
        let path = temp_path("view");
        let editor = Link::create(&path).unwrap();
        let host = Link::open(&path).unwrap();
        assert_eq!(host.read_view(), None);
        let view = ViewRequest {
            translation: [1.0, 2.0, 3.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            perspective: 1,
            fov_y: 0.8,
            near: 0.1,
            far: 1000.0,
            ortho_scale: 1.0,
            width: 640,
            height: 360,
            scene_generation: 4,
        };
        editor.write_view(&view);
        assert_eq!(host.read_view(), Some(view));
        drop((editor, host));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn frames_alternate_and_are_read_once() {
        let path = temp_path("frame");
        let editor = Link::create(&path).unwrap();
        let host = Link::open(&path).unwrap();
        let mut out = Vec::new();
        assert!(editor.read_frame(0, &mut out).is_none());

        host.publish_frame(4, 2, |b| b.fill(7));
        let (n, w, h) = editor.read_frame(0, &mut out).unwrap();
        assert_eq!((n, w, h), (1, 4, 2));
        assert!(out.iter().all(|&b| b == 7));
        assert!(editor.read_frame(n, &mut out).is_none(), "the same frame is not new twice");

        host.publish_frame(4, 2, |b| b.fill(9));
        let (n2, ..) = editor.read_frame(n, &mut out).unwrap();
        assert_eq!(n2, 2);
        assert!(out.iter().all(|&b| b == 9));
        drop((editor, host));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_mismatched_file_is_refused() {
        let path = temp_path("bad");
        std::fs::write(&path, vec![0u8; FILE_BYTES]).unwrap();
        assert!(Link::open(&path).is_err());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn oversized_requests_are_clamped() {
        let path = temp_path("clamp");
        let editor = Link::create(&path).unwrap();
        let host = Link::open(&path).unwrap();
        editor.write_view(&ViewRequest { width: 10_000, height: 10_000, ..Default::default() });
        let view = host.read_view().unwrap();
        assert_eq!((view.width, view.height), (MAX_WIDTH, MAX_HEIGHT));
        drop((editor, host));
        let _ = std::fs::remove_file(path);
    }
}
