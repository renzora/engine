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

// The console's `clog_*!` macros, at the crate root where they were exported
// before the console became its own layer (`renzora_log_api`).
pub use renzora_log_api::{clog_error, clog_info, clog_success, clog_warn};

// Subjects that are not here any more, because each is its own crate one layer
// up and an edit to one should rebuild only the crates that use it. Plugins
// still see them at these paths through `renzora_sdk`:
//   `net`, `net_backend`     -> `renzora_net_api`
//   `audio`, `audio_backend` -> `renzora_audio_api`
//   `text_mesh`              -> `renzora_text_mesh`
//   `undo`, `create_menu`, `asset_actions` (and their root re-exports)
//                            -> `renzora_shell_api`
//   `diagnostics`, `profiling`, `probes`, `load_timeline`, `runtime_warnings`,
//   `content_problems`       -> `renzora_diagnostics_api`
//   `version`, `rust_toolchain` (+ `RustToolchain`), `child_process`,
//   `dlss_runtime`, and core's auth, load_work, plugin_inventory,
//   plugin_updates, project_index, resize, streaming -> `renzora_platform_api`
//   the language-backend half of scripting (`script!`, `ScriptCtx`,
//   `ScriptHook`, `script_extension`, `backend`, `command`, `component`,
//   `context`, `get_handler`, and core's script_bridge, input_actions,
//   reflection) -> `renzora_scripting_api`
//   `save`                   -> `renzora_save_api`
//   core's animation, bone_style, sprite_anim -> `renzora_animation_api`
//   `physics`, `vehicle`, `scene_query` -> `renzora_physics_api`
//   `world_environment`, `wind`, `sun`, `spline` -> `renzora_world_api`
//   `gi`, `postprocess`, `post_effects`, `auto_exposure`, `grid`, and core's
//   blockout_grid, environment_suppressed -> `renzora_render_api`
//   `wgsl`, `shader_source`, and core's material_ref, material_nodes, graph
//                            -> `renzora_material_api`

// `terrain_tools` (the terrain service the MCP server calls) moved to
// `renzora_world_api`; plugins still reach `renzora::terrain_tools` through
// `renzora_sdk`.


// ── Language / localization contract ─────────────────────────────────────
// The translation table is its own layer, `renzora_lang_api`; re-exported as a
// module (not globbed) so plugins keep writing the explicit, greppable
// `renzora::lang::t("..")`. Engine crates depend on the layer directly.
pub use renzora_lang_api as lang;

// ── Plugin declaration (legacy) ─────────────────────────────────────────
// Engine crates declare their plugins in their `Cargo.toml` now. `add!` stays
// only so a crate made by the `renzora add` scaffolder, which still writes one,
// compiles; see plugin_meta.rs. `plugin!` and its ABI record are
// `renzora_platform_api`'s.
mod plugin_meta;


// The Inspector's pages, the component categories every editor surface groups
// by. Ungated: Add Component's picker lives in ember, which links this crate
// without the `editor` feature.
pub use renzora_shell_api::inspector_pages;

// ── Editor contract ──────────────────────────────────────────────────────
// The thin editor types shared across the binary/bundle boundary are their own
// layer, `renzora_editor_api`. Re-exported flat under `editor` so plugins keep
// writing `renzora::EditorSelection`, `renzora::FieldDef`, `renzora::float_field!`
// and `renzora::Inspectable`; non-editor builds carry none of it.
#[cfg(feature = "editor")]
pub use renzora_editor_api::*;

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
// The launcher / loading / editor phase and the open-project requests live in
// `renzora_project_api` now; re-exported at the root paths they always had.
pub use renzora_project_api::{
    RecentProjects, RequestOpenProject, RequestOpenProjectPath, RequestSwitchProject,
    SplashEffects, SplashEffectsSetting, SplashState,
};
