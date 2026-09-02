//! Logging, and recording draw commands from a render callback.
//!
//! A `sys::RenderCtx` deliberately carries no interface pointer — it is an
//! opaque handle to host state, and widening it would tie the render ABI to the
//! system ABI. So the plugin keeps its own copy of the table in [`IFACE`], set
//! once at init, and both the log helpers and [`RenderPass`] read it from there.

use crate::sys;

use super::system::{catch, materialize};

/// The interface, stashed so a render callback can reach it.
///
/// A `sys::RenderCtx` deliberately carries no interface pointer — it is an
/// opaque handle to host state, and widening it would tie the render ABI to the
/// system ABI. So the plugin keeps its own copy, set once at init.
pub(crate) static IFACE: core::sync::atomic::AtomicPtr<sys::Interface> =
    core::sync::atomic::AtomicPtr::new(core::ptr::null_mut());

/// Write a line to the engine log.
///
/// A plugin has no stdout worth using and no `tracing` subscriber of its own, so
/// this is the only way its output reaches the console panel. Silently a no-op
/// before `Plugin::build` has been entered — there is no host to log to yet.
pub fn log(level: sys::LogLevel, msg: &str) {
    let iface = IFACE.load(core::sync::atomic::Ordering::Relaxed);
    if iface.is_null() {
        return;
    }
    // SAFETY: set once at init from the host's `'static` table.
    unsafe {
        ((*iface).log)(
            core::ptr::null_mut(),
            level,
            sys::StrRef {
                ptr: msg.as_ptr(),
                len: msg.len(),
            },
        );
    }
}

pub fn info(msg: &str) {
    log(sys::LogLevel::Info, msg);
}

pub fn warn(msg: &str) {
    log(sys::LogLevel::Warn, msg);
}

pub fn error(msg: &str) {
    log(sys::LogLevel::Error, msg);
}

/// Records draw commands for one view. Mirrors `TrackedRenderPass`.
pub struct RenderPass {
    ctx: sys::RenderCtx,
}

impl RenderPass {
    /// Bind this pass's pipeline and its bind group (view texture at 0, sampler
    /// at 1 — the fullscreen contract).
    pub fn set_pipeline(&mut self) {
        let iface = IFACE.load(core::sync::atomic::Ordering::Relaxed);
        if iface.is_null() {
            return;
        }
        unsafe { ((*iface).render_set_pipeline)(self.ctx, sys::PipelineId(0)) };
    }

    /// Issue a draw. A fullscreen pass is `draw(0..3, 0..1)` — the engine's
    /// fullscreen vertex shader builds a covering triangle from the vertex
    /// index, so there is no vertex buffer.
    pub fn draw(&mut self, vertices: core::ops::Range<u32>, instances: core::ops::Range<u32>) {
        let iface = IFACE.load(core::sync::atomic::Ordering::Relaxed);
        if iface.is_null() {
            return;
        }
        unsafe {
            ((*iface).render_draw)(
                self.ctx,
                vertices.end - vertices.start,
                instances.end - instances.start,
            )
        };
    }
}

pub(crate) unsafe extern "C" fn render_thunk<F>(
    ctx: sys::RenderCtx,
    _p: sys::PipelineId,
) -> sys::SystemStatus
where
    F: Fn(&mut RenderPass) + 'static,
{
    let mut pass = RenderPass { ctx };
    // A panic inside the render graph would take the editor down mid-frame.
    match catch(|| materialize::<F>()(&mut pass)) {
        Ok(()) => sys::SystemStatus::Ok,
        Err(_) => sys::SystemStatus::Panicked,
    }
}
