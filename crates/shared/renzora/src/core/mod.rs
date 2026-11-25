//! The shared vocabulary every `renzora_*` crate agrees on.
//!
//! Nothing here does any work; it is all types, events and registries that two
//! or more crates need one definition of. Anything that crosses a crate — or the
//! dlopen boundary between the binary, the editor bundle and a plugin — lives
//! here so all of them resolve one `TypeId` out of the one shared dylib.
//!
//! Every submodule is re-exported **flat** (`pub use <mod>::*`), so a `use
//! renzora::Foo` path never names the module it lives in and moving a type
//! between modules breaks nothing.

pub mod console_log;
// The viewport layer, re-exported at its old module path.
pub use renzora_viewport_api as viewport_types;

// The crash layer, re-exported at its old module path.
pub use renzora_crash_api as recoverable;
// The shell layer, re-exported as modules (and flattened below, as before).
pub use renzora_shell_api::{editor_events, keybindings, shell};

// The scene layer, re-exported as modules the same way (flattened below).
pub use renzora_scene_api::{asset_bytes, components, entity_id, prefab, shapes};

// The project layer, re-exported as modules so every `renzora::core::<mod>::X`
// path keeps resolving (and flattened below, as before).
pub use renzora_project_api::{
    image_formats, model_file, play_mode, project_config, project_files, session, settings_file,
    settings_path,
};
// The two play-mode run conditions that read the editor viewport, kept beside
// `ViewportSettings`. Flattened here so `renzora::core::in_three_view` still works.
pub use viewport_types::{in_three_view, in_two_view};

// Moved one layer up, so an edit to one rebuilds only its users; plugins still
// see their items at `renzora::...` through `renzora_sdk`:
//   image_formats, play_mode, project_config, project_files, session,
//   settings_file, GraphicsQuality                  -> `renzora_project_api`
//   asset_bytes, components, entity_id, shapes,
//   RenderResolution                                -> `renzora_scene_api`
//   companion                                       -> `renzora_platform_api`
//   editor_events, keybindings, shell, and the root's
//   inspector_pages                                 -> `renzora_shell_api`
//   auth, load_work, plugin_inventory, plugin_updates, project_index, resize,
//   streaming                                       -> `renzora_platform_api`
//   script_bridge, input_actions, reflection        -> `renzora_scripting_api`
//   animation, bone_style, sprite_anim              -> `renzora_animation_api`
//   blockout_grid, environment_suppressed           -> `renzora_render_api`
//   material_ref, material_nodes, graph             -> `renzora_material_api`

pub use asset_bytes::*;
pub use components::*;
pub use editor_events::*;
pub use entity_id::*;
pub use image_formats::*;
pub use model_file::*;
pub use play_mode::*;
pub use project_config::*;
pub use project_files::*;
pub use session::*;
pub use shapes::*;
pub use shell::*;
