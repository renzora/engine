//! Editor CONTRACT (Operation Merge fold).
//!
//! The thin, boundary-crossing editor types — the inspector / spawn / toolbar /
//! shortcut registries, `EditorSelection`, `AppEditorExt`, the field macros —
//! that BOTH the lean game binary (via the dual-mode crates' editor code, under
//! `--workspace` feature unification) AND the editor bundle reference. Hosting
//! them here in the shared `renzora` dylib gives them ONE `TypeId` across the
//! binary↔bundle boundary; if they lived in an rlib they'd duplicate per side
//! and resources like `EditorSelection` wouldn't unify.
//!
//! Everything UI-heavy (the dock framework, `RenzoraEditorPlugin`, the
//! `bevy_inspectors` data, settings/camera UI) stays in the editor impl crate,
//! which only the bundle links. Gated by the crate `editor` feature so
//! runtime / server / mobile builds carry zero editor surface.

mod ext;
mod gpu_pass_registry;
mod inspector_registry;
mod selection;
mod shortcut_registry;
mod spawn_registry;
mod timeline_bridge;
mod toolbar_registry;
mod types;

// `#[macro_export]` field macros (float_field! etc.) are exported at the
// `renzora` crate root automatically; these re-exports surface the non-macro
// items (FieldDef, AppEditorExt, the registries, …) at the crate root too.
pub use ext::*;
pub use gpu_pass_registry::*;
pub use inspector_registry::*;
pub use selection::*;
pub use shortcut_registry::*;
pub use spawn_registry::*;
pub use timeline_bridge::*;
pub use toolbar_registry::*;
pub use types::*;
