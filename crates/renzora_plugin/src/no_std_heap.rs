//! The allocator a `#![no_std]` plugin runs on, and the abort its panic handler
//! calls. Both are wired up by [`crate::no_std_runtime!`]; this module exists so
//! that macro stays three lines and the `unsafe` lives somewhere it can be read.
//!
//! ## Why the host's `malloc` rather than a bundled allocator
//!
//! A plugin is `dlopen`'d into a running engine, so the process already has an
//! initialised C runtime mapped — using it means one heap for the whole process
//! instead of a second one sitting beside it. That is not merely tidier: buffers
//! move across the boundary in both directions, and a plugin freeing memory on a
//! heap the host never allocated from is the classic way to corrupt one.
//!
//! Bundling something like `dlmalloc` would also undo much of the point. The
//! reason to drop `std` is size (~112 KB → ~18 KB per plugin), and an embedded
//! allocator puts a chunk of that straight back.

use core::alloc::{GlobalAlloc, Layout};
use core::ptr;

// Name the C library on Apple targets so the link actually finds these.
//
// A `no_std` crate graph pulls in no `std`, and `std` is the only thing that
// carries the `#[link]` for libc — so rustc drives the linker with
// `-nodefaultlibs` and nothing supplies `malloc`, `free`, `abort`, `memcpy` or
// `dyld_stub_binder`. On Linux that goes unnoticed at build time, because ld
// permits undefined symbols in a shared object and leaves them for the loader
// to resolve out of the host process (which has libc mapped already). ld64 does
// not: a dylib must resolve everything up front, so an unadorned `no_std`
// plugin fails to *link* on macOS with a list of undefined C runtime symbols
// rather than failing later at `dlopen`.
//
// `System` is the whole C runtime on Apple platforms, so one entry covers the
// allocator below, `abort`, and the `memcpy`/stub-binder references the
// codegen backend leaves behind. It is the same library the host is already
// running on — this asks the linker to write down a dependency the process has
// taken regardless, not to add one.
#[cfg_attr(target_vendor = "apple", link(name = "System"))]
extern "C" {
    fn malloc(size: usize) -> *mut u8;
    fn realloc(ptr: *mut u8, size: usize) -> *mut u8;
    fn free(ptr: *mut u8);
    // Imported under a different Rust name than its symbol, so the public
    // wrapper below can keep the obvious name without shadowing this one.
    #[link_name = "abort"]
    fn c_abort() -> !;
}

/// Abort the process. Used by the panic handler, where there is nothing else to
/// do — see [`crate::no_std_runtime!`].
pub fn abort() -> ! {
    // SAFETY: `abort` is a C runtime function that does not return and touches
    // no memory we own.
    unsafe { c_abort() }
}

/// Alignment `malloc` is guaranteed to give us on every platform Renzora
/// targets. Anything at or below this takes the fast path; anything above needs
/// the manual alignment dance in [`alloc_aligned`].
const MALLOC_ALIGN: usize = 16;

/// The global allocator for a `no_std` plugin: the host process's own heap.
pub struct HostHeap;

// SAFETY: every method forwards to the C runtime allocator, which is
// thread-safe and already initialised by the host that loaded this library.
// Requests above `MALLOC_ALIGN` go through the over-allocate path, so a pointer
// returned from `alloc` always satisfies the requested alignment, and `dealloc`
// reverses whichever path `alloc` took by testing the same `layout.align()` —
// the allocator contract guarantees the same layout comes back.
unsafe impl GlobalAlloc for HostHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.align() <= MALLOC_ALIGN {
            malloc(layout.size())
        } else {
            alloc_aligned(layout)
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if layout.align() <= MALLOC_ALIGN {
            free(ptr)
        } else {
            free(base_of(ptr))
        }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if layout.align() <= MALLOC_ALIGN {
            return realloc(ptr, new_size);
        }
        // `realloc` cannot preserve an alignment it does not know about: it may
        // move the block to an address that suits `malloc` and no longer suits
        // us. So over-aligned blocks are grown by hand — allocate, copy the
        // smaller of the two sizes, free.
        let fresh = alloc_aligned(Layout::from_size_align_unchecked(new_size, layout.align()));
        if !fresh.is_null() {
            ptr::copy_nonoverlapping(ptr, fresh, core::cmp::min(layout.size(), new_size));
            free(base_of(ptr));
        }
        fresh
    }
}

/// Satisfy an over-aligned request out of plain `malloc`.
///
/// Asks for `size + align` bytes so an aligned address is guaranteed to exist
/// inside the block, then stores the original `malloc` pointer in the word
/// immediately below the address it returns, where [`base_of`] reads it back.
/// The extra `size_of::<*mut u8>()` reserves room for that word, and the `+ 1`
/// alignment step guarantees at least one byte of slack below the aligned
/// address even when `malloc` happens to return a perfectly aligned pointer.
///
/// This is the portable form on purpose. `_aligned_malloc` (Windows) and
/// `posix_memalign` (POSIX) would each do the job, but they need their own
/// matching free function, so using them means a `cfg` fork through all three
/// methods above for no benefit a plugin can measure.
unsafe fn alloc_aligned(layout: Layout) -> *mut u8 {
    let header = core::mem::size_of::<*mut u8>();
    let Some(total) = layout
        .size()
        .checked_add(layout.align())
        .and_then(|n| n.checked_add(header))
    else {
        return ptr::null_mut();
    };

    let base = malloc(total);
    if base.is_null() {
        return ptr::null_mut();
    }

    // First address at or above `base + header` that satisfies the alignment.
    let raw = base as usize + header;
    let aligned = (raw + layout.align() - 1) & !(layout.align() - 1);
    let out = aligned as *mut u8;

    // Stash the pointer `free` will need. Written unaligned because `out` is
    // aligned to the *request*, which says nothing about where the word below
    // it lands.
    (out.cast::<*mut u8>()).sub(1).write_unaligned(base);
    out
}

/// Recover the `malloc` pointer stashed by [`alloc_aligned`].
///
/// # Safety
///
/// `ptr` must have come from [`alloc_aligned`] — i.e. the caller must already
/// have checked that the layout's alignment exceeds [`MALLOC_ALIGN`].
unsafe fn base_of(ptr: *mut u8) -> *mut u8 {
    (ptr.cast::<*mut u8>()).sub(1).read_unaligned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exercises `HostHeap` directly rather than installing it as the global
    /// allocator — the test binary already has one, and a second would not link.
    /// Calling the trait methods is the same code path a `no_std` plugin takes.
    #[test]
    fn round_trips_every_alignment() {
        // Spans both branches: 1..=16 take `malloc` straight, 32..=256 go
        // through the over-allocate path where the bugs would live.
        for align in [1usize, 2, 4, 8, 16, 32, 64, 128, 256] {
            for size in [1usize, 7, 64, 1000] {
                let layout = Layout::from_size_align(size, align).unwrap();
                unsafe {
                    let p = HostHeap.alloc(layout);
                    assert!(!p.is_null(), "alloc failed at align {align} size {size}");
                    assert_eq!(
                        p as usize % align,
                        0,
                        "misaligned: align {align} size {size}"
                    );
                    // Write the whole block so a too-small allocation trips ASAN
                    // / the heap guard rather than passing silently.
                    ptr::write_bytes(p, 0xAB, size);
                    assert_eq!(*p, 0xAB);
                    assert_eq!(*p.add(size - 1), 0xAB);
                    HostHeap.dealloc(p, layout);
                }
            }
        }
    }

    #[test]
    fn realloc_preserves_contents_and_alignment() {
        for align in [8usize, 16, 64, 128] {
            let layout = Layout::from_size_align(32, align).unwrap();
            unsafe {
                let p = HostHeap.alloc(layout);
                assert!(!p.is_null());
                for i in 0..32 {
                    *p.add(i) = i as u8;
                }

                let grown = HostHeap.realloc(p, layout, 512);
                assert!(!grown.is_null());
                assert_eq!(grown as usize % align, 0, "realloc lost alignment {align}");
                // The original 32 bytes must survive the move.
                for i in 0..32 {
                    assert_eq!(*grown.add(i), i as u8, "realloc corrupted byte {i}");
                }

                HostHeap.dealloc(grown, Layout::from_size_align(512, align).unwrap());
            }
        }
    }

    /// An over-aligned request `malloc` cannot satisfy must come back null.
    ///
    /// This is the reachable half of `alloc_aligned`'s failure handling. The
    /// other half — the `checked_add` chain — turns out to be unreachable from
    /// safe code on 64-bit: `Layout` already refuses any size that does not fit
    /// in `isize::MAX` once rounded up to its alignment, so `size + align +
    /// header` cannot wrap. It stays because `Layout::from_size_align_unchecked`
    /// can still hand us one, and silently returning a tiny block for a huge
    /// request is the kind of bug that surfaces as corruption elsewhere.
    #[test]
    fn unsatisfiable_request_returns_null() {
        let layout = Layout::from_size_align(isize::MAX as usize - 63, 64).unwrap();
        unsafe {
            assert!(HostHeap.alloc(layout).is_null());
        }
    }
}
