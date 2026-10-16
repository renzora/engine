//! Panics the engine catches on purpose, and tells the crash reporter about.
//!
//! A Rust panic hook runs BEFORE unwinding starts, so it cannot see whether a
//! `catch_unwind` further up the stack is about to recover. The crash reporter's
//! hook therefore treated every panic as fatal: a Rust script that indexed out of
//! bounds wrote `last_crash.txt` and put up the modal "The application has
//! crashed" dialog, although the dispatcher caught the panic and the editor was
//! still running underneath it. A script that panics every frame did that every
//! frame, and the only way out looked like killing the editor.
//!
//! [`catch`] is `catch_unwind` that first marks the current thread as inside a
//! recoverable call. The hook asks [`panic_will_be_caught`], and for a caught
//! panic it records where it happened ([`record_location`]) instead of reporting
//! a crash. [`catch`] then hands that location back with the message, so the
//! Console line can say what failed and where instead of only that it did.
//!
//! A thread-local depth rather than a flag, because recoverable calls nest: a
//! script can make an audio call, and the audio link guards its own backend.
//! It lives in the contract crate because the hook (in `renzora_engine`) and the
//! catchers (scripts, plugins, audio, net) must read ONE copy of it, and this
//! crate is the single shared image they all link.

use std::cell::{Cell, RefCell};
use std::panic::{catch_unwind, AssertUnwindSafe};

thread_local! {
    static DEPTH: Cell<u32> = const { Cell::new(0) };
    static LOCATION: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// A panic that [`catch`] recovered from.
#[derive(Clone, Debug)]
pub struct CaughtPanic {
    /// The panic message, or `"unknown panic"` for a payload that is not text.
    pub message: String,
    /// `file:line:column`, when the panic hook saw one.
    pub location: Option<String>,
}

impl std::fmt::Display for CaughtPanic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.location {
            Some(at) => write!(f, "{} (at {at})", self.message),
            None => f.write_str(&self.message),
        }
    }
}

/// Undoes the depth increment however [`catch`] leaves, so a depth can never be
/// left raised and make a later, genuinely fatal panic look recoverable.
struct DepthGuard;

impl Drop for DepthGuard {
    fn drop(&mut self) {
        DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
    }
}

/// Run `f`, catching a panic, and tell the crash reporter that it will be caught.
///
/// Use this instead of `std::panic::catch_unwind` wherever the engine recovers
/// from a panic deliberately (a script, a plugin constructor, a backend link).
/// Plain `catch_unwind` still recovers, but the crash dialog fires first.
pub fn catch<R>(f: impl FnOnce() -> R) -> Result<R, CaughtPanic> {
    DEPTH.with(|d| d.set(d.get() + 1));
    let guard = DepthGuard;
    // Cleared first, so a location left over from an earlier caught panic is
    // never attributed to this one.
    LOCATION.with(|l| l.borrow_mut().take());
    let result = catch_unwind(AssertUnwindSafe(f));
    drop(guard);
    result.map_err(|payload| CaughtPanic {
        message: payload
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic".to_string()),
        location: LOCATION.with(|l| l.borrow_mut().take()),
    })
}

/// True while this thread is inside [`catch`]. For the panic hook.
pub fn panic_will_be_caught() -> bool {
    DEPTH.with(|d| d.get() > 0)
}

/// Where a caught panic happened. The panic hook calls this; [`catch`] reads it.
pub fn record_location(location: String) {
    LOCATION.with(|l| *l.borrow_mut() = Some(location));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_only_inside_and_recovers_the_message() {
        assert!(!panic_will_be_caught());
        let inside = catch(panic_will_be_caught).unwrap();
        assert!(inside);

        let nested = catch(|| catch(panic_will_be_caught).unwrap()).unwrap();
        assert!(nested);

        let caught = catch(|| -> () { panic!("index {} out of range", 3) }).unwrap_err();
        assert_eq!(caught.message, "index 3 out of range");
        // The depth must come back down after a panic, or every later panic
        // on this thread would be mistaken for a caught one.
        assert!(!panic_will_be_caught());
    }
}
