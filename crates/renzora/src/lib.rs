//! Renzora — the contracts crate. Types, events, components, resources.
//!
//! `renzora` is the foundation every other crate in the engine depends on.
//! It has zero dependencies of its own beyond Bevy + serde, so it cannot
//! introduce circular dependencies and any crate is free to swap any other
//! crate as long as both honor the contracts defined here.
//!
//! Plugins that want extra functionality (post-process effects, editor
//! framework, theming, etc.) depend on those crates explicitly:
//!
//! ```toml
//! [dependencies]
//! bevy = { workspace = true }
//! renzora = { path = "..." }                 # types + events
//! renzora_editor_framework = { path = "..." } # editor panels, inspector
//! renzora_postprocess = { path = "..." }      # post-process effect derive
//! ```

// Re-export bevy so plugin authors can write `use renzora::bevy::prelude::*;`
// to skip a separate workspace dep if they want.
pub use bevy;

// ── Core types ───────────────────────────────────────────────────────────
// Everything that used to live in `renzora_core`. Re-exported at the crate
// root so callers write `renzora::Foo` instead of `renzora::core::Foo`.
pub mod core;
pub use core::*;

// ── Global illumination contract ─────────────────────────────────────────
// GI settings components (`RtLighting`, `LumenLighting`) + the Lumen
// diagnostics snapshot. Shared here so the GI distribution plugin
// (`renzora_lumen`), the editor inspectors, `renzora_level_presets`, and the
// debugger's Lumen panel all resolve one `TypeId` across the dlopen boundary —
// the plugin can't be statically linked by those consumers (it's a cdylib), so
// the boundary-crossing types must live in this shared dylib instead.
pub mod gi;
pub use gi::*;

// `WorldEnvironment` — the unified environment contract type (see its module
// doc + docs/world-environment-spec.md). Shared dylib, same boundary reason.
pub mod world_environment;
pub use world_environment::*;

// The cloud deck's authored settings. Here rather than in the `clouds` plugin
// because `renzora_level_presets` builds a sky by inserting `CloudsData` and is
// compiled into the editor binary, while the renderer is a plugin loaded at
// runtime — a binary cannot name a type that lives in a plugin.
pub mod clouds;
pub use clouds::*;

// Auto-exposure settings. Same boundary reason as `clouds`: `level_presets`
// inserts and queries it, and `renzora_debugger` reads it for the live EV
// readout — both compiled into the binary, while the metering is a plugin.
pub mod auto_exposure;
pub use auto_exposure::*;

// `Sun` (read by six crates, one of them now a plugin) and `NightStarsData`.
// `renzora_lighting` re-exports `Sun`, so existing paths keep resolving.
pub mod sun;
pub use sun::*;

// `SplinePath` + its Catmull-Rom evaluation. `renzora_terrain_editor` builds and
// draws paths while compiled into the binary; the systems are a plugin.
pub mod spline;
pub use spline::*;

// Coverage→SDF glyph packing and the `SdfTextMaterial`. NOT glob re-exported —
// `build_text_mesh` and `SPREAD` are too generic for the crate root, so callers
// say `renzora::text_mesh::build_text_mesh`. Shared between `renzora_ember`'s
// world-space UI emitter and the `text3d` plugin, which is exactly why it has to
// be one definition. Feature-gated because it is the only part of this crate
// that needs `bevy::text`, which a UI-stripped lean export does not build. See
// the module doc.
#[cfg(feature = "text_mesh")]
pub mod text_mesh;

// HTTP request vocabulary + the submission queue. NOT glob re-exported: `Request`
// and `Response` are names generic enough to collide, so callers say
// `renzora::net::Request`. The engine ships no HTTP client — the socket is opened
// by `plugins/http` behind the C-ABI boundary — but the queue is process-global
// state and therefore has to be singular. See the module doc.
pub mod net;

// Undo/redo core. NOT glob re-exported: `execute` and `record` are names far too
// generic for the crate root, so callers say `renzora::undo::execute`. An
// editing tool is the obvious thing to ship as a plugin, and the one thing it
// must do is make its edits undoable — see the module doc.
pub mod undo;

// One world-global wind, shared by foliage, cloth, the ocean and the cloud
// deck. Here rather than in `renzora_wind` for the usual reason: four crates
// read `WindState` and must all see the same `TypeId`.
pub mod wind;
pub use wind::*;

// ── Language / localization contract ─────────────────────────────────────
// The process-global translation table + `t()` lookup every crate calls, plus
// the plugin-facing registration API. Lives here in the shared dylib so the
// runtime binary, the editor bundle, and dlopen'd distribution plugins all read
// and write ONE table across the boundary. The `renzora_lang` plugin populates
// it (embedded built-ins + external `languages/*.toml` packs); any plugin can
// contribute its own keys. Not glob-re-exported: callers write the explicit
// `renzora::lang::t("…")` so localized call sites are greppable.
pub mod lang;

// ── Engine version / release channel ─────────────────────────────────────
// `ENGINE_VERSION` (`r1-alphaN`) plus the release-tag resolution the export
// downloader needs to ask GitHub for *its own* version's runtime templates
// instead of whatever `releases/latest` happens to be. Here in the contract
// crate because the shell, the splash, the command palette and the exporter all
// need the same answer — they used to carry four different ones. Not
// glob-re-exported: call sites read `renzora::version::ENGINE_VERSION`.
pub mod version;

// ── Plugin declaration ──────────────────────────────────────────────────
// `renzora::add!(MyPlugin)` declares a plugin to `cargo renzora sync`, which
// generates the dependency edge that links it and the list that installs it.
// The macro itself only type-checks — see the module docs.
mod plugin_meta;
/// Where a native plugin may load — see [`plugin!`]. Re-exported because both
/// the macro's expansion and the loader that reads the symbol name it.
pub use plugin_meta::NativePluginScope;
/// What a Rust script is handed — see [`script_ctx::ScriptCtx`].
pub mod script_ctx;
pub use script_ctx::ScriptCtx;
// `add!` is registered at the crate root via `#[macro_export]` in plugin_meta.rs.

// ── Post-process framework ───────────────────────────────────────────────
// `PostProcessPlugin<T>`, `PostProcessEffect`, the unified render-graph node
// and the shared `PostProcessRegistry`. Folded in from the old standalone
// `renzora_postprocess` dylib so its symbols ship inside `renzora.dll`
// instead of a separate file — the ~50 effect plugins still resolve one
// shared copy (and one `PostProcessRegistry` TypeId) across the dlopen
// boundary, just from `renzora` now. The `renzora_postprocess` crate
// remains as a thin rlib re-export shim so existing `renzora_postprocess::…`
// paths (incl. those emitted by the `post_process` macro) keep compiling.
//
// Gated so non-rendering targets (mobile staticlib, wasm, headless server)
// don't pull the render-graph surface into the lean base crate.
#[cfg(feature = "postprocess")]
pub mod postprocess;

// ── Runtime warning capture ──────────────────────────────────────────────
// The `LogPlugin::custom_layer` factory + global ring buffer behind the
// editor's Scene Diagnostics "Recent Runtime Warnings" feed. Hosted here in
// the shared `renzora` dylib (not the editor-only `renzora_scene` rlib) so
// the capture layer installed by the lean runtime binary and the panel that
// reads it from the editor bundle touch ONE buffer across the dylib boundary.
// Gated off mobile (no editor there, and the bevy_log tracing-subscriber
// surface isn't guaranteed) — desktop + wasm-editor keep it.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod runtime_warnings;

// Per-file problems with authored content, behind the editor's Problems panel.
// Here rather than in a plugin crate because the producer (the material
// resolver) and the reader (the code editor) must agree on one definition.
pub mod content_problems;

// The one WGSL↔naga seam (parse/validate) for self-contained shaders.
pub mod wgsl;

// ── Editor contract (Operation Merge fold) ───────────────────────────────
// The thin editor types shared across the binary↔bundle boundary live here in
// the one shared `renzora` dylib (so `EditorSelection` et al. unify to one
// `TypeId`). Gated by `editor` so non-editor builds carry no editor surface.
// The `#[macro_export]` field macros land at the crate root automatically; the
// `pub use` surfaces the non-macro items (FieldDef, AppEditorExt, registries).
#[cfg(feature = "editor")]
mod editor_contract;
#[cfg(feature = "editor")]
pub use editor_contract::*;

// Editor derive/attribute macros, re-exported from core so consumers write
// `renzora::Inspectable` / `renzora::post_process` and the macros they generate
// emit `renzora::FieldDef` etc. (single shared contract, no `renzora_editor_framework`).
#[cfg(feature = "editor")]
pub use renzora_macros::{post_process, Inspectable};

/// The engine's `serde`, re-exported so a plugin derives against **this** copy.
///
/// A native plugin's own crates.io dependencies are resolved separately from the
/// engine's, so a plugin that writes `serde = "1"` in its manifest gets a
/// *different crate* from the one Bevy was compiled against. Deriving
/// `Serialize` on any struct holding a Bevy type then fails, because `Vec3`
/// implements the engine's `Serialize` and not the plugin's:
///
/// ```text
/// error[E0277]: the trait bound `bevy::prelude::Vec3: serde::Deserialize<'de>`
///               is not satisfied
/// ```
///
/// That is the duplicate-crate hazard working as intended — a compile error
/// rather than two incompatible types meeting at runtime — but serde is not a
/// crate anyone should be duplicating: it is half of how Bevy's own types
/// round-trip, so a plugin authoring a component needs the engine's copy the
/// same way it needs the engine's `Transform`. Hence the same rule the rest of
/// this crate follows: if two sides must agree on it, it lives here.
///
/// Use it instead of depending on `serde` directly, and point the derive at this
/// path so the generated code resolves:
///
/// ```ignore
/// use renzora::serde::{Deserialize, Serialize};
///
/// #[derive(Component, Reflect, Serialize, Deserialize)]
/// #[serde(crate = "renzora::serde")]
/// #[reflect(Component, Serialize, Deserialize)]
/// pub struct MySettings { pub tint: Vec3 }
/// ```
///
/// The `#[serde(crate = ...)]` line is what a re-exported serde requires: the
/// derive emits absolute paths, and without it they point at a `serde` the
/// plugin does not have.
pub use serde;

/// The engine's `serde_json`, re-exported for the same reason as [`serde`].
///
/// Beyond the duplicate-crate hazard, this is what makes `Response::json::<T>()`
/// usable from a plugin at all: that bound is `T: serde::de::DeserializeOwned`
/// against *this* crate's serde, so a `T` derived against a privately resolved
/// one does not satisfy it.
///
/// It also means a plugin whose only third-party needs were serde and
/// serde_json declares **no** dependencies, so cargo is never invoked for it and
/// the build stays offline and about a second long.
pub use serde_json;

// ── App lifecycle state ──────────────────────────────────────────────────
//
// Coordination contract used by both the splash screen UI and the editor
// framework. Lives in the SDK so neither side has to depend on the other's
// implementation crate.

/// Top-level app phase. The splash UI runs while `Splash`, a loading
/// overlay during `Loading`, and the full editor while `Editor`.
#[derive(bevy::prelude::States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SplashState {
    #[default]
    Splash,
    Loading,
    Editor,
}

/// Marker request: open a different project. Inserted by the editor's File
/// menu; consumed by the splash plugin which shows the file dialog,
/// validates, updates recent projects, and transitions state.
#[derive(bevy::prelude::Resource)]
pub struct RequestOpenProject;
