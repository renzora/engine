//! Rendering: full-screen passes and parameterised post-process effects.
//!
//! Handle-based, like `wgpu-native`'s C API and Godot's RID system: every GPU
//! object is an opaque integer the host maps back to the real thing. That is what
//! lets a plugin drive the GPU without linking wgpu — and it means an invalid
//! handle is a checked lookup failure rather than a wild pointer.
//!
//! TWO THINGS TO KNOW BEFORE EXTENDING THIS:
//!
//! 1. Any wgpu enum that crosses here (`TextureFormat`, `BufferUsages`, …) becomes
//!    OUR frozen ABI. Bevy upgrades wgpu regularly, so each one added is a
//!    permanent mapping-table maintenance cost. Mirror them explicitly; never
//!    re-export wgpu's.
//! 2. Resources a plugin creates are owned by the host on its behalf and must be
//!    freed when the plugin unloads, or a reloaded plugin leaks VRAM every cycle.

use core::ffi::c_void;

use super::{ComponentId, StrRef, SystemStatus};

/// A render pipeline the host built for a plugin.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PipelineId(pub u32);

impl PipelineId {
    pub const INVALID: PipelineId = PipelineId(u32::MAX);
    pub const fn is_valid(self) -> bool {
        self.0 != u32::MAX
    }
}

/// Opaque handle to an in-progress render pass. **Only valid inside the
/// [`RenderCallback`] that received it** — it borrows host render state that is
/// gone the moment the callback returns.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct RenderCtx(pub *mut c_void);

/// Where in the frame a plugin's pass runs. Mirrors the engine's own phase
/// ordering; `#[repr(u32)]` and append-only.
/// Newtype rather than an `enum`, and that is a soundness requirement rather
/// than a style choice.
///
/// The **plugin writes this value and the host reads it** — out of plugin memory
/// for the ones that live in structs, and straight off the FFI boundary for the
/// ones passed by value. Materialising an out-of-range discriminant into a Rust
/// enum is undefined behaviour, and not the harmless kind: rustc attaches
/// `!range` metadata to the load, so LLVM may legally assume the impossible and
/// a `match` can take an arbitrary arm.
///
/// That is exactly what a MINOR bump would cause. A plugin built against a newer
/// ABI writes a discriminant the older host has no variant for. The version
/// handshake is supposed to refuse that plugin — but then the soundness of every
/// appended variant rests on the handshake being bug-free, forever. A newtype
/// removes the question: any `u32` is a valid value, unknown ones fall to the
/// `_` arm, and "appending a variant is a MINOR change" is true rather than
/// merely usually true.
///
/// The constants below keep the variant names, so this is a source-compatible
/// change at every call site.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderPhase(pub u32);

#[allow(non_upper_case_globals)]
impl RenderPhase {
    /// HDR, after the main 3D pass, before temporal AA — GI, reflections.
    pub const Gi: Self = Self(0);
    /// HDR, after temporal AA — bloom, depth of field, motion blur.
    pub const HdrPost: Self = Self(1);
    /// LDR, after tonemapping — colour grading, vignette.
    pub const LdrPost: Self = Self(2);
    /// Final overlays, after AA.
    pub const Overlay: Self = Self(3);

    /// Whether this is a value this build knows. Anything else came from a
    /// plugin built against a newer ABI.
    pub const fn is_known(self) -> bool {
        self.0 < 4
    }

    /// The variant name, or `"?"` for a value from a newer ABI.
    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "Gi",
            1 => "HdrPost",
            2 => "LdrPost",
            3 => "Overlay",
            _ => "?",
        }
    }
}

impl core::fmt::Debug for RenderPhase {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_known() {
            f.write_str(self.name())
        } else {
            write!(f, "RenderPhase({})", self.0)
        }
    }
}

/// Records draw commands for one view. Runs inside the host's render graph.
pub type RenderCallback =
    unsafe extern "C" fn(ctx: RenderCtx, pipeline: PipelineId) -> SystemStatus;

/// A full-screen pass a plugin contributes to the frame.
///
/// The host compiles `fragment_wgsl` and builds the pipeline, because pipeline
/// creation needs the `RenderDevice` and `PipelineCache`, which live in the
/// render world. The plugin gets a [`PipelineId`] back when its callback runs.
///
/// The fragment shader is paired with the engine's fullscreen vertex shader and
/// gets the current view texture at binding 0 and a sampler at binding 1.
#[repr(C)]
pub struct RenderPassDesc {
    /// Stable id, e.g. `"my_plugin.tint"`. Shown in the editor's render-pass
    /// list and used to reorder passes.
    pub id: StrRef,
    pub fragment_wgsl: StrRef,
    pub phase: RenderPhase,
    /// Sort key within the phase — lower runs first.
    pub order: f32,
    pub callback: RenderCallback,
}

/// A parameterised full-screen effect.
///
/// The difference from [`RenderPassDesc`] is `settings`: a plugin component
/// whose bytes are uploaded to a uniform buffer each frame and bound at
/// `@group(0) @binding(2)`, so the shader can be *controlled* rather than fixed.
/// That is the whole gap between "a plugin can draw" and "a plugin can ship an
/// effect" — a pass with no parameters can only ever be a constant.
///
/// The host does extraction, the uniform buffer, the bind group and the draw.
/// A plugin describing an effect this way writes no render code at all; use
/// [`RenderPassDesc`] when you need to record commands yourself.
///
/// The settings component must be `#[repr(C)]` and laid out for std140 —
/// `vec3` fields need padding to 16 bytes, same as any Bevy uniform.
#[repr(C)]
pub struct PostProcessDesc {
    /// Stable id, e.g. `"my_plugin.bloom"`. Shown in the editor's render-pass
    /// list and used to reorder effects.
    pub id: StrRef,
    pub fragment_wgsl: StrRef,
    /// Plugin component carrying the uniform payload. Registered normally, so
    /// its field schema also drives the inspector.
    pub settings: ComponentId,
    /// Size of one settings instance, for the bind group layout.
    pub settings_size: u64,
    pub phase: RenderPhase,
    /// Sort key within the phase — lower runs first.
    pub order: f32,
}
