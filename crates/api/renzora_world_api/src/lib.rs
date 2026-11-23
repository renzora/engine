//! The world's shared state.
//!
//! A layer of the contract hierarchy, beside `renzora` (it needs nothing from
//! it). Plugins reach every item at the path it always had
//! (`renzora::WorldEnvironment`, `renzora::Sun`, `renzora::WindState`, ...)
//! through `renzora_sdk`; the engine's own crates depend on this crate directly.

// `WorldEnvironment`: the unified environment contract type (see its module
// doc).
pub mod world_environment;
pub use world_environment::*;

// One world-global wind, shared by foliage, cloth, the ocean and the cloud deck.
// In the contract because four crates read `WindState` and must all see the
// same `TypeId`.
pub mod wind;
pub use wind::*;

// `Sun` (read by six crates, one of them a plugin) and `NightStarsData`.
// `renzora_lighting` re-exports `Sun`, so existing paths keep resolving.
pub mod sun;
pub use sun::*;

// `SplinePath` and its Catmull-Rom evaluation. `renzora_terrain_editor` builds
// and draws paths while compiled into the binary; the systems are a plugin.
pub mod spline;
pub use spline::*;

// Terrain operations (ground height, sculpt, generate, erode) for crates that
// must not link the terrain crate, which today means the MCP server. The
// terrain editor installs the service. Moved from `renzora::terrain_tools`; not
// glob re-exported, so callers name `terrain_tools::` explicitly.
pub mod terrain_tools;
