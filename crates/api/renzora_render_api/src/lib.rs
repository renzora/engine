//! The rendering contract.
//!
//! A layer of the contract hierarchy, above `renzora` (it names
//! `RenderPhase`). Plugins reach every item and module at the path it always had
//! (`renzora::SolariLit`, `renzora::postprocess`, `renzora::PostEffects`, ...)
//! through `renzora_sdk`; the engine's own crates depend on this crate directly.
//! Solari's settings and diagnostics are a layer of their own,
//! `renzora_solari_api`.

// Global illumination: the GPU capability flags, `SolariLit` and the
// stand-in contracts with the ray tracer. Shared so the host, the Solari
// plugin and the editor all resolve one `TypeId`.
pub mod gi;
pub use gi::*;

// Auto-exposure settings. In the contract because `level_presets` inserts and
// queries it, and the `debugger` plugin reads it for the live EV readout.
pub mod auto_exposure;
pub use auto_exposure::*;

// Shader-file post-process effects. In the contract because the scene format,
// the inspector and the scripting API all name the list, from different
// crates; the renderer is `renzora_postprocess`.
pub mod post_effects;
pub use post_effects::*;

// `RenderComposition`: the ordered slots (GI, HDR post, LDR post, overlay) a
// render pass registers into, anchored to Bevy's TAA, tonemapping and AA. In the
// shared image so an installed plugin's pass and an engine pass sort in one
// list. Gated so non-rendering targets (mobile staticlib, wasm, headless
// server) do not pull the render-graph surface in.
#[cfg(feature = "postprocess")]
pub mod postprocess;

// The infinite ground grid's two components. The renderer stays in
// `renzora_grid`; only the vocabulary is here. `InfiniteGrid` is specific
// enough to say in full, so callers name the module.
#[cfg(feature = "grid")]
pub mod grid;

pub mod blockout_grid; // the generated "no material yet" grid textures
pub mod environment_suppressed; // shading modes that show no sky
pub use blockout_grid::*;
pub use environment_suppressed::*;
