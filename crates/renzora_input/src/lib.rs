pub mod action;
pub mod default;
pub mod map;
pub mod state;

pub use action::{ActionKind, InputAction, InputBinding};
pub use map::InputMap;
pub use state::ActionState;

use bevy::prelude::*;

/// Input mapping plugin.
///
/// Registers `InputMap` and `ActionState` resources and updates action state
/// each frame from raw Bevy input. On startup, attempts to load `input_map.ron`
/// from the project directory; falls back to defaults if not found.
pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] InputPlugin");

        app.register_type::<InputAction>()
            .register_type::<ActionKind>()
            .register_type::<InputBinding>()
            .register_type::<InputMap>();

        app.init_resource::<InputMap>()
            .init_resource::<ActionState>()
            .add_systems(PreUpdate, state::update_action_state)
            .add_systems(Startup, load_input_map_on_startup);
    }
}

/// Try to load `input_map.ron` from the project directory on startup.
fn load_input_map_on_startup(
    project: Option<Res<renzora::CurrentProject>>,
    vfs: Option<Res<renzora::VirtualFileReader>>,
    mut input_map: ResMut<InputMap>,
) {
    // Try VFS first (rpak archive)
    if let Some(ref vfs) = vfs {
        if let Some(content) = vfs.read_string("input_map.ron") {
            match ron::from_str::<InputMap>(&content) {
                Ok(map) => {
                    info!("[InputPlugin] Loaded input map from VFS ({} actions)", map.actions.len());
                    *input_map = map;
                    return;
                }
                Err(e) => {
                    warn!("[InputPlugin] Failed to parse input_map.ron from VFS: {}", e);
                }
            }
        }
    }

    // Try disk (project directory)
    if let Some(ref project) = project {
        let path = project.resolve_path("input_map.ron");
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    match ron::from_str::<InputMap>(&content) {
                        Ok(map) => {
                            info!("[InputPlugin] Loaded input map from {} ({} actions)", path.display(), map.actions.len());
                            *input_map = map;
                            return;
                        }
                        Err(e) => {
                            warn!("[InputPlugin] Failed to parse {}: {}", path.display(), e);
                        }
                    }
                }
                Err(e) => {
                    warn!("[InputPlugin] Failed to read {}: {}", path.display(), e);
                }
            }
        }
    }

    info!("[InputPlugin] Using default input map ({} actions)", input_map.actions.len());
}

/// Save the input map to the project directory.
/// Called by the editor when the input map is modified.
pub fn save_input_map(input_map: &InputMap, project: &renzora::CurrentProject) -> Result<(), String> {
    let path = project.resolve_path("input_map.ron");
    let content = ron::ser::to_string_pretty(input_map, ron::ser::PrettyConfig::default())
        .map_err(|e| format!("Failed to serialize input map: {}", e))?;
    std::fs::write(&path, content)
        .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;
    info!("[InputPlugin] Saved input map to {}", path.display());
    Ok(())
}
