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
pub mod keybindings;
pub mod reflection;
pub mod resize;
pub mod viewport_types;

pub mod animation; // .anim clip format + property keyframes
pub mod asset_bytes; // the project/VFS byte loader for non-AssetServer assets
pub mod auth; // sign-in state mirrored for the title bar
pub mod blockout_grid; // the generated "no material yet" grid textures
pub mod components; // shared ECS components + entity-tag markers
pub mod editor_events; // one-way events the editor fires at other crates
pub mod entity_id; // canonical unique snake_case entity ids (Name)
pub mod graph; // node-graph vocabulary (blueprint / material / particle)
pub mod input_actions; // named actions + the character controller queue
pub mod material_ref; // pointing an entity at a .material file
pub mod play_mode; // Play / Simulate / Edit state and its run conditions
pub mod plugin_inventory; // what plugins were found on disk, and their state
pub mod project_config; // project.toml model + editor preferences
pub mod script_bridge; // the inboxes scripting drains each frame
pub mod session; // process kind + the editor's one-shot requests
pub mod shapes; // built-in shape registry (mesh factories by id)
pub mod shell; // panels, status items and top-bar buttons a plugin adds
pub mod sprite_anim; // multi-sheet sprites (SpriteImages) for 2D animation
pub mod streaming; // world-streaming gate + camera-position helpers

pub use animation::*;
pub use asset_bytes::*;
pub use auth::*;
pub use blockout_grid::*;
pub use components::*;
pub use editor_events::*;
pub use entity_id::*;
pub use graph::*;
pub use input_actions::*;
pub use material_ref::*;
pub use play_mode::*;
pub use plugin_inventory::*;
pub use project_config::*;
pub use script_bridge::*;
pub use session::*;
pub use shapes::*;
pub use shell::*;
pub use sprite_anim::*;
pub use streaming::*;
