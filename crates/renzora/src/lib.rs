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
// Solari's settings component and the GPU capability flags. Shared here so the
// host, the Solari plugin and the editor all resolve one `TypeId` across the
// dlopen boundary: the plugin is installed beside the engine, not linked into
// it, so the boundary-crossing types must live in this shared dylib.
pub mod gi;
pub use gi::*;

// `WorldEnvironment` — the unified environment contract type (see its module
// doc). Shared dylib, same boundary reason.
pub mod world_environment;
pub use world_environment::*;

// The cloud deck's authored settings. Here rather than in the `clouds` plugin
// because `renzora_level_presets` builds a sky by inserting `CloudsData` and is
// compiled into the editor binary, while the renderer is a plugin loaded at
// runtime — a binary cannot name a type that lives in a plugin.
pub mod clouds;
pub use clouds::*;

// Auto-exposure settings. Same boundary reason as `clouds`: `level_presets`
// inserts and queries it, and the `debugger` plugin reads it for the live EV
// readout — both compiled into the binary, while the metering is a plugin.
pub mod auto_exposure;
pub use auto_exposure::*;

// Shader-file post-process effects. Here because the scene format, the
// inspector and the scripting API all name the list, from different crates;
// the renderer is `renzora_postprocess`.
pub mod post_effects;
pub use post_effects::*;

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

// The infinite ground grid's two components. The renderer stays in
// `renzora_grid`; only the vocabulary is here, so a plugin can put a ground
// plane under its own preview. Costs no dependencies — `Color`s and `f32`s.
// NOT glob re-exported: `InfiniteGrid` is specific enough to say in full.
#[cfg(feature = "grid")]
pub mod grid;

// The declarative script-binding vocabulary: `ScriptExtension`, `Binding`, and
// the `Bind` builder. Here rather than in `renzora_scripting` so an installed
// plugin can declare script functions at all — a plugin reaches `bevy`,
// `renzora` and `renzora_ember` and nothing else, so while this lived in the
// scripting crate the one mechanism for extending the script API was open only
// to crates compiled into the engine. `renzora_scripting::extension`
// re-exports it, so nothing that already used it had to change.
//
// NOT glob re-exported: `Param`, `Binding` and `Bind` are too generic for the
// crate root.
pub mod script_extension;

// `AudioLink` — the engine side of the audio boundary, and the handle types it
// allocates. The mixer/timeline/emitters are still `renzora_audio`; only the
// link is here, so any plugin can play a sound. NOT glob re-exported —
// `SoundId` and `VoiceId` are too generic for the crate root.
#[cfg(feature = "audio")]
pub mod audio;

// What the debug panels read. Here rather than in `renzora_shader` /
// `renzora_scripting` because those panels are a native plugin now, and a plugin
// reaches only `bevy`, `renzora` and `renzora_ember`. Collection stays with the
// crates doing the work; both re-export their old path. NOT glob re-exported:
// `MaterialPerfStats` and friends are panel vocabulary, not crate-root names.
#[cfg(feature = "diagnostics")]
pub mod diagnostics;

// HTTP request vocabulary + the submission queue. NOT glob re-exported: `Request`
// and `Response` are names generic enough to collide, so callers say
// `renzora::net::Request`. The engine ships no HTTP client — the socket is opened
// by whichever crate implements `net_backend::Backend` — but the queue is
// process-global state and therefore has to be singular. See the module doc.
pub mod net;

// The client contract itself: the `Backend` trait, what it answers with, and the
// events it sends back. Here rather than in `renzora_net` because `net`'s
// process-global queue holds these types, and a boundary type has exactly one
// definition — the rule the whole contract crate exists for.
pub mod net_backend;

// The mixer contract, for the same reason: `audio`'s link holds these types.
pub mod audio_backend;

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

// Ray and sphere casts against the scene's physics colliders, for crates that
// must not link a physics engine (particles collide with the real level
// through it). The physics crate installs the service; a build without one
// simply has no `SceneQueries` resource. Not glob re-exported: `SceneRay` and
// friends read better qualified at the call site.
pub mod scene_query;

// The physics contract: the components scenes save, the runtime state the
// engine and a backend exchange, and the `PhysicsBackend` trait a physics engine
// implements. Here because a backend plugin must name these types without
// linking `renzora_physics`. Not glob re-exported: `PhysicsBackend` and `Caps`
// read better qualified, and `Caps` would collide with audio's.
pub mod physics;

// ── Language / localization contract ─────────────────────────────────────
// The process-global translation table + `t()` lookup every crate calls, plus
// the plugin-facing registration API. Lives here in the shared dylib so the
// runtime binary, the editor image, and every installed plugin all read
// and write ONE table across the boundary. The `renzora_lang` plugin populates
// it (embedded built-ins + external `languages/*.toml` packs); any plugin can
// contribute its own keys. Not glob-re-exported: callers write the explicit
// `renzora::lang::t("…")` so localized call sites are greppable.
pub mod lang;

// Saved games: the process-global working set of named values and the slots it
// is written to. Here for the same reason as `lang`: every language backend and
// every Rust script must share one store. Not glob-re-exported: callers write
// `renzora::save::write("slot1")`, which reads as what it does.
pub mod save;

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
/// The lifecycle events a Rust script can receive — see [`script_hook::ScriptHook`].
pub mod script_hook;
pub use script_hook::ScriptHook;

// ── The language-backend half of scripting ───────────────────────────────
// `renzora_scripting` owns the scripting SYSTEM and still does. These are the
// types that cross its boundary, and they are here for the reason §6 of
// CLAUDE.md gives: a type two crates both need has one definition, in the
// contract crate.
//
// Concretely, the two crates are the engine and an INSTALLED PLUGIN. A language
// backend is meant to be a marketplace plugin, and a plugin compiles against the
// staged SDK, which offers `bevy`, `renzora` and `renzora_ember`. While
// `ScriptBackend` lived in `renzora_scripting` there was no way for a plugin to
// name it, so there could be no third-party language at all.
//
// `get_handler` is the one that could not have been solved by staging another
// crate. It is thread-local state that answers a script's synchronous reads
// mid-hook, so two copies means the backend reads handlers the engine never
// wrote, and every `get` returns nothing with no error anywhere. Here, the
// shared `renzora_dylib` image makes it singular, exactly as it already does for
// the translation table and the console buffer.
//
// NOT glob re-exported: `command`, `context` and `component` are far too generic
// for this crate's root, and `renzora_scripting` re-exports them flat anyway for
// everything that was already written against it.
/// Plugin-contributed entries in the "create a new file" menus.
///
/// Here rather than in either panel for the reason `backend` is: the two crates
/// that need the type are the editor and an installed plugin.
pub mod create_menu;
pub use create_menu::{CreateMenuItem, CreateMenuRegistry, RegisterCreateMenuItem};
/// Plugin-contributed rows in the Assets panel's right-click menu, keyed on
/// file extension. Here for the same reason as `create_menu`.
pub mod asset_actions;
pub use asset_actions::{AssetAction, AssetActionRegistry, AssetActionScope, RegisterAssetAction};

pub mod backend;
pub mod command;
pub mod component;
pub mod context;
pub mod get_handler;
// The names a backend actually writes, lifted to the root the way `ScriptCtx`
// and `ScriptHook` are. A plugin implementing a language should not have to know
// which of five modules each type happens to sit in.
pub use backend::{AppScriptBackendExt, FileReader, ScriptBackend};
pub use command::ScriptCommand;
pub use component::{ScriptValue, ScriptVariableDefinition, ScriptVariables};
pub use context::{GamepadSnapshot, ScriptContext, GAMEPAD_BUTTON_NAMES};
// `add!` is registered at the crate root via `#[macro_export]` in plugin_meta.rs.

// ── Render phases ────────────────────────────────────────────────────────
// `RenderComposition`: the ordered slots (GI, HDR post, LDR post, overlay)
// a render pass registers into, anchored to Bevy's TAA, tonemapping and AA.
// In `renzora.dll` so an installed plugin's pass and an engine pass sort in
// one list across the dlopen boundary. The `renzora_postprocess` crate is
// the shader-file effects that run in one of these phases.
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

// The Inspector's pages, the component categories every editor surface groups
// by. Ungated: Add Component's picker lives in ember, which links this crate
// without the `editor` feature.
pub mod inspector_pages;

// The one WGSL↔naga seam (parse/validate) for self-contained shaders.
pub mod wgsl;

// Shader text in any dialect (plain WGSL, WESL, or the pre-0.20 naga_oil one)
// to a Bevy `Shader`, translating the old dialect so it keeps working.
pub mod shader_source;

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

// The editor derive macro, re-exported from core so consumers write
// `renzora::Inspectable` and what it generates emits `renzora::FieldDef` etc.
// (single shared contract, no `renzora_editor_framework`).
#[cfg(feature = "editor")]
pub use renzora_macros::Inspectable;

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

/// Whether the launcher and loading screen run their decorative effects: the
/// launcher's 3D Light Chamber cinematic and the loading screen's animated haze.
///
/// Both are full-window and both scale with the window's pixel count, so on a
/// very large or ultrawide display they cost the most for the least: a 49"
/// 5120x1440 panel has 3.5x the pixels of 1080p. `Auto` resolves to reduced on
/// such displays and to full elsewhere; the resolved answer is
/// [`SplashEffectsSetting::reduced`].
#[derive(
    serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum SplashEffects {
    #[default]
    Auto,
    Full,
    Reduced,
}

/// The [`SplashEffects`] choice, mirrored from the editor's settings so the
/// splash crate can read it without linking the editor framework.
#[derive(bevy::prelude::Resource, Default, Clone, Copy)]
pub struct SplashEffectsSetting(pub SplashEffects);

impl SplashEffectsSetting {
    /// Resolve against the display the launcher is on. `monitor` is its
    /// physical size, when known.
    ///
    /// "Large" is wider than 21:9 or more pixels than 4K: the displays where a
    /// full-window shader is the most expensive thing on screen.
    pub fn reduced(&self, monitor: Option<(u32, u32)>) -> bool {
        match self.0 {
            SplashEffects::Full => false,
            SplashEffects::Reduced => true,
            SplashEffects::Auto => monitor.is_some_and(|(w, h)| {
                let h = h.max(1);
                w as f32 / h as f32 > 2.4 || (w as u64) * (h as u64) > 3840 * 2160
            }),
        }
    }
}

/// Marker request: open a different project. Inserted by the editor's File
/// menu; consumed by the splash plugin which shows the file dialog,
/// validates, updates recent projects, and transitions state.
#[derive(bevy::prelude::Resource)]
pub struct RequestOpenProject;

/// Request: open the project rooted at this path, with no file dialog.
///
/// [`RequestOpenProject`]'s sibling for a project the user has *already* named
/// — File > Recent Projects picks the path off [`RecentProjects`], so asking
/// them to find the same `project.toml` in an OS dialog would be the whole
/// point of the list undone.
#[derive(bevy::prelude::Resource)]
pub struct RequestOpenProjectPath(pub std::path::PathBuf);

/// The recently-opened project roots, most recent first.
///
/// Mirrored out of the launcher's `AppConfig` by the splash plugin, which owns
/// the on-disk list. It lives here because the File menu is built in
/// `renzora_shell`, and the shell must not depend on the splash crate to read
/// one `Vec<PathBuf>`. Empty in a build with no splash plugin, which is exactly
/// what the menu should show there.
#[derive(bevy::prelude::Resource, Default, Clone)]
pub struct RecentProjects(pub Vec<std::path::PathBuf>);
