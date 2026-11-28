//! `renzora`, as a plugin sees it: the contract crate and every subject crate
//! layered above it, under one name.
//!
//! The engine's own crates depend on the layers they use and never on this; see
//! the manifest for why. A subject moved out of `renzora` gets one line here,
//! re-exporting it at the path plugins already write, so moving it breaks no
//! plugin.

// The bottom layer, flat: every `renzora::Foo` a plugin writes, the
// `#[macro_export]` macros (`add!`, `plugin!`, the inspector field macros)
// included.
pub use renzora::*;

// `renzora::core`, put back together. The contract crate's `core` holds only
// what stayed in it, and some of what moved out used to live in there too
// (`renzora::core::AuthBridge`, `renzora::core::resize`). Declaring the module
// here shadows the one the glob above brings in, so both halves answer at the
// old path.
pub mod core {
    pub use renzora::core::*;
    pub use renzora_platform_api::*;
    pub use renzora_scripting_api::{input_actions, input_actions::*, reflection, script_bridge, script_bridge::*};
    pub use renzora_animation_api::*;
    pub use renzora_render_api::{blockout_grid, blockout_grid::*, environment_suppressed, environment_suppressed::*};
    pub use renzora_material_api::{graph, graph::*, material_nodes, material_nodes::*, material_ref, material_ref::*};
}

// The language-backend half of scripting: `renzora::ScriptCtx`,
// `renzora::script!`, `renzora::command`, `renzora::component`, the property
// bridge, ... Glob, because its root mirrors what `renzora`'s root and `core`
// used to offer.
pub use renzora_scripting_api::*;
pub use renzora_scripting_api::{script, __script_entry, __script_hook_entry};

// `renzora::GameUiScale`: the player's multiplier on the game UI's fit, at the
// root because a settings-menu script is the thing that writes it. (`InputMap`
// and the rest of the action map arrive at the root through the
// `renzora_scripting_api` glob above.)
pub use renzora::viewport_types::GameUiScale;

// `renzora::save`.
pub use renzora_save_api as save;

// `renzora::bus`: named events and services between plugins.
pub use renzora_bus_api as bus;

// `renzora::physics`, `renzora::vehicle`, `renzora::scene_query`.
pub use renzora_physics_api::{physics, scene_query, vehicle};

// `renzora::particles`: the components that place and drive a particle
// effect, so a script can spawn a hit spark or an explosion itself.
pub use renzora_particles_api as particles;

// `renzora::WorldEnvironment`, `renzora::Sun`, `renzora::WindState`,
// `renzora::SplinePath` and their modules, flat, the way they were.
pub use renzora_world_api::*;

// `renzora::SolariLit`, `renzora::PostEffects`, `renzora::postprocess`,
// `renzora::grid` and the rest of the rendering contract. A glob carries the
// feature-gated modules too, whenever `renzora_render_api` was compiled with
// them, so no `cfg` is needed here.
pub use renzora_render_api::*;

// `renzora::SolariGi`, `renzora::DlssMode`, `renzora::SolariDiagnostics`, ...
pub use renzora_solari_api::*;

// `renzora::gi::...` as it was before Solari's settings left the render layer:
// both halves under the one module. Explicit, so it shadows the `gi` the
// render glob above brings in.
pub mod gi {
    pub use renzora_render_api::gi::*;
    pub use renzora_solari_api::*;
}

// `renzora::MaterialRef`, `renzora::GraphComment`, ...
pub use renzora_material_api::*;

// `renzora::shader_source`, `renzora::wgsl`.
pub use renzora_shader_api::{shader_source, wgsl};

// `renzora::AnimClip`, `renzora::SpriteImages`, `renzora::bone_style`, ...
pub use renzora_animation_api::*;

// `renzora::version`, `renzora::RustToolchain`, `renzora::PluginUpdates`,
// `renzora::child_process` and the rest of the platform layer, flat, the way
// they were.
pub use renzora_platform_api::*;

// `renzora::net` and `renzora::net_backend`: the HTTP client.
pub use renzora_net_api::{net, net_backend};

// `renzora::network`: multiplayer (replication, RPCs, ownership). Not the same
// thing as `net`, which is HTTP.
pub use renzora_network_api as network;

// `renzora::audio` and `renzora::audio_backend`.
pub use renzora_audio_api::{audio, audio_backend};

// `renzora::undo`, `renzora::create_menu`, `renzora::asset_actions`,
// `renzora::entity_actions`, and the registration types
// `renzora::CreateMenuItem` and friends at the root.
pub use renzora_shell_api::{
    asset_actions, create_menu, entity_actions, undo, AssetAction, AssetActionRegistry,
    AssetActionScope, CreateMenuItem, CreateMenuRegistry, EntityAction, EntityActionRegistry,
    RegisterAssetAction, RegisterCreateMenuItem, RegisterEntityAction,
};

// `renzora::profiling`, `renzora::probes`, `renzora::load_timeline`,
// `renzora::runtime_warnings`, `renzora::content_problems` and the debug-panel
// tables at `renzora::diagnostics`.
pub use renzora_diagnostics_api::{content_problems, load_timeline, probes, profiling};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub use renzora_diagnostics_api::runtime_warnings;
#[cfg(feature = "diagnostics")]
pub use renzora_diagnostics_api::diagnostics;

// `renzora::text_mesh`, the way `bevy` writes `pub use bevy_audio as audio;`.
#[cfg(feature = "text_mesh")]
pub use renzora_text_mesh as text_mesh;

// `renzora::ui`: the UI framework (panels, widgets, theme, reactive bindings),
// embedded in `renzora_dylib` so the editor and every native plugin share one
// copy of its theme, stylesheet and registries. Native plugins import it from
// here; engine crates depend on `renzora_ember` directly.
#[cfg(feature = "ui")]
pub use renzora_ember as ui;
