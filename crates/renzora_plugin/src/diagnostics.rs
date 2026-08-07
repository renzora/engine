//! Read the host's measurements: frame time, FPS, entity count, per-render-pass
//! GPU and CPU times, process CPU and memory.
//!
//! ```ignore
//! use renzora_plugin::prelude::*;
//! use renzora_plugin::diagnostics::Diagnostics;
//!
//! fn report(diags: Diagnostics) {
//!     if let Some(fps) = diags.get("fps") {
//!         info!("{:.0} fps", fps.smoothed);
//!     }
//! }
//! ```
//!
//! ## Why this is a system param and not an `Interface` function
//!
//! Reading the store needs the world, and [`SystemCall::host`](crate::sys::SystemCall::host)
//! is null while a system runs — the same constraint that made
//! [`Meshes`](crate::ecs::Meshes) and [`Http`](crate::http::Http) params. So the
//! host builds a [`DiagnosticSource`](crate::sys::DiagnosticSource) for the call
//! and this borrows it.
//!
//! ## Why it is not behind a feature
//!
//! The domain modules (`anim`, `physics`, `http`) are opt-in because each is a
//! *vocabulary* riding the generic service channel — bytes `sys` never reads —
//! so adding one moves this crate's semver rather than the ABI. Diagnostics are
//! not that shape: the source pointer is a field of `SystemCall` in every build,
//! the ABI version already records it, and gating the reader would only mean a
//! plugin has to name a feature to use a field that is there regardless.
//!
//! ## What the host does not promise
//!
//! **Which measurements exist.** The set depends on what the host assembled: an
//! editor carries all of them, a shipped game typically carries none, and a
//! backend without GPU timestamp queries has the `render/*/elapsed_cpu` paths but
//! not `elapsed_gpu`. Every reader must handle absence — [`get`](Diagnostics::get)
//! returns `Option` for that reason, and treating a missing diagnostic as zero is
//! how a profiler ends up drawing a flat line and calling it data.
//!
//! **That a present measurement has a value.** A diagnostic registers before its
//! first sample, so `value` is `NaN` for the first frames. [`Diagnostic::is_valid`]
//! is the check; plotting `NaN` is what makes a graph vanish.

use crate::sys;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// One measurement, owned — the borrowed [`sys::DiagnosticEntry`] copied out.
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    /// e.g. `"fps"`, `"entity_count"`,
    /// `"render/main_opaque_pass_3d/elapsed_gpu"`.
    pub path: String,
    /// The most recent sample, or `NaN` before the first one is taken.
    pub value: f64,
    /// The diagnostic's own smoothed average, or the same as `value` for one
    /// that keeps no history. Prefer this for anything a human reads — raw
    /// frame time is far too noisy to display.
    pub smoothed: f64,
}

impl Diagnostic {
    /// Whether this carries a real sample rather than a not-yet-measured `NaN`.
    ///
    /// Worth calling even when you are about to do arithmetic that "handles"
    /// `NaN`, because it does not: `NaN` propagates silently through a sum and
    /// poisons an average without any comparison ever being false.
    pub fn is_valid(&self) -> bool {
        self.value.is_finite()
    }
}

/// The host's diagnostic store, for the duration of one system call.
///
/// Cheap to construct and cheap to ignore: with no host diagnostics the source
/// is null and every method returns empty, so a plugin need not special-case a
/// build that has none.
pub struct Diagnostics<'a> {
    src: *mut sys::DiagnosticSource,
    _p: core::marker::PhantomData<&'a ()>,
}

impl Diagnostics<'_> {
    /// How many measurements the host holds. 0 if it keeps none.
    pub fn len(&self) -> usize {
        if self.src.is_null() {
            return 0;
        }
        // SAFETY: `src` came from the live `SystemCall` and a null `out` is
        // explicitly allowed when `cap` is 0 — that is the documented probe.
        unsafe { ((*self.src).read)(self.src, core::ptr::null_mut(), 0) as usize }
    }

    /// Whether the host holds no measurements at all.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Every measurement this frame.
    ///
    /// Two passes — probe for the count, then fill — because the host cannot
    /// allocate with the plugin's allocator. The count is re-read after the fill
    /// and the result truncated to what was actually written, so a diagnostic
    /// registered between the two passes cannot make this return uninitialised
    /// entries.
    pub fn iter(&self) -> Vec<Diagnostic> {
        let count = self.len();
        if count == 0 {
            return Vec::new();
        }
        let mut raw = Vec::with_capacity(count);
        // SAFETY: `raw` has room for `count` entries; the host writes at most
        // `cap` and returns how many it wrote (capped below in case the store
        // grew between the probe and here).
        let written = unsafe {
            let n = ((*self.src).read)(self.src, raw.as_mut_ptr(), count as u32) as usize;
            let n = n.min(count);
            raw.set_len(n);
            n
        };
        let mut out = Vec::with_capacity(written);
        for e in &raw {
            // SAFETY: the host's paths are valid UTF-8 for the duration of the
            // `read` call, which has returned — so this copy has to happen now,
            // and does. Keeping `e.path` past this loop would dangle.
            let path = unsafe { e.path.as_str() }.to_string();
            out.push(Diagnostic {
                path,
                value: e.value,
                smoothed: e.smoothed,
            });
        }
        out
    }

    /// One measurement by exact path, or `None` if the host has no such
    /// diagnostic.
    ///
    /// Linear over the whole set, which is fine for the occasional lookup and
    /// wrong for reading twenty paths every frame — call [`iter`](Self::iter)
    /// once and match against that instead.
    pub fn get(&self, path: &str) -> Option<Diagnostic> {
        self.iter().into_iter().find(|d| d.path == path)
    }
}

// SAFETY: `declare` pushes nothing because this parameter reads no components
// and no resources *of the plugin's* — the host's diagnostic store is not
// something the plugin can name, and the host declares its own access on the
// dispatcher system.
unsafe impl crate::ecs::SystemParam for Diagnostics<'_> {
    fn declare(_: &mut crate::ecs::InitCtx, _: &mut crate::ecs::SystemBuilder) {}
    unsafe fn fetch(call: *const sys::SystemCall, _: &mut usize) -> Self {
        Diagnostics {
            src: (*call).diagnostics,
            _p: core::marker::PhantomData,
        }
    }
}
