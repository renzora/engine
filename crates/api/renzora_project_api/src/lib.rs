//! The open project and the process around it: `project.toml` and the editor
//! preferences beside it, what changed on disk under the project, which kind
//! of process this is and the one-shot requests the editor's chrome sends, the
//! Play / Simulate / Edit state, and the project's graphics-quality tier.
//!
//! A layer of the contract hierarchy, moved out of `renzora::core`. Plugins
//! reach every item at the path it always had (`renzora::ProjectConfig`,
//! `renzora::core::project_config::..`, `renzora::PlayModeState`, ...) through
//! the re-exports; engine crates depend on this crate directly.
//!
//! Modules are flattened into the root the same way `renzora::core` flattens
//! them, except `settings_file`, which callers always name explicitly.

pub mod image_formats; // which extensions load as an Image (raster + SVG)
pub mod model_file; // GLB to `.rvg`, for code that cannot link the builder
pub mod play_mode; // Play / Simulate / Edit state and its run conditions
pub mod project_config;
pub mod project_files; // what changed on disk under the open project
pub mod session; // process kind + the editor's one-shot requests
pub mod settings_file; // project.toml model + editor preferences
pub mod settings_path; // one setting by dotted path, for script get/set_setting
mod quality; // the graphics-quality tier, a project setting
mod app_state; // launcher/loading/editor phase and the open-project requests

pub use app_state::*;
pub use image_formats::*;
pub use model_file::{build_model_file, install_model_file_builder, ModelFileBuilder};
pub use play_mode::*;
pub use project_config::*;
pub use project_files::*;
pub use quality::*;
pub use session::*;
// Only the mirror component at the root: `get_setting` / `set_setting` are too
// generic for it, and callers name the module.
pub use settings_path::{SettingError, SettingsReadState};
