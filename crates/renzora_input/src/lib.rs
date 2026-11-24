//! The action map's behaviour: loading `input_map.ron` and turning raw input
//! into `ActionState` every frame.
//!
//! The map's *types* live in the contract (`renzora_scripting_api::input_map`),
//! so a script can read and rebind the controls while the game runs; this
//! crate re-exports them at the paths it always had (`renzora_input::InputMap`
//! and the old `action` / `map` / `default` modules), so no caller had to move.

pub mod state;

pub use renzora_scripting_api::input_map::{
    ActionKind, InputAction, InputBinding, InputMap, RebindError,
};
pub use state::ActionState;

/// The old module paths, kept so `renzora_input::action::InputBinding` and
/// friends still resolve after the types moved to the contract.
pub mod action {
    pub use renzora_scripting_api::input_map::{ActionKind, InputAction, InputBinding};
}
/// See [`action`].
pub mod map {
    pub use renzora_scripting_api::input_map::{InputMap, RebindError};
}
/// See [`action`].
pub mod default {
    pub use renzora_scripting_api::input_map::default_actions;
}

use bevy::prelude::*;

/// Input mapping plugin.
///
/// Registers `InputMap` and `ActionState` resources and updates action state
/// each frame from raw Bevy input. On startup, attempts to load `input_map.ron`
/// from the project directory; falls back to defaults if not found.
///
/// Loading is once, but applying is every frame: `update_action_state` reads
/// the resource each `PreUpdate`, so anything that changes `InputMap` later (a
/// script's controls menu, `set_setting("input.jump.0", ...)`) takes effect on
/// the next frame with nothing else to notify.
#[derive(Default)]
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
    project: Option<Res<renzora_project_api::CurrentProject>>,
    vfs: Option<Res<renzora_project_api::VirtualFileReader>>,
    mut input_map: ResMut<InputMap>,
) {
    // Try VFS first (rpak archive)
    if let Some(ref vfs) = vfs {
        if let Some(content) = vfs.read_string("input_map.ron") {
            match ron::from_str::<InputMap>(&content) {
                Ok(map) => {
                    info!(
                        "[InputPlugin] Loaded input map from VFS ({} actions)",
                        map.actions.len()
                    );
                    *input_map = map;
                    return;
                }
                Err(e) => {
                    warn!(
                        "[InputPlugin] Failed to parse input_map.ron from VFS: {}",
                        e
                    );
                }
            }
        }
    }

    // Try disk (project directory)
    if let Some(ref project) = project {
        let path = project.resolve_path("input_map.ron");
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => match ron::from_str::<InputMap>(&content) {
                    Ok(map) => {
                        info!(
                            "[InputPlugin] Loaded input map from {} ({} actions)",
                            path.display(),
                            map.actions.len()
                        );
                        *input_map = map;
                        return;
                    }
                    Err(e) => {
                        warn!("[InputPlugin] Failed to parse {}: {}", path.display(), e);
                    }
                },
                Err(e) => {
                    warn!("[InputPlugin] Failed to read {}: {}", path.display(), e);
                }
            }
        }
    }

    info!(
        "[InputPlugin] Using default input map ({} actions)",
        input_map.actions.len()
    );
}

/// Save the input map to the project directory.
/// Called by the editor when the input map is modified.
pub fn save_input_map(
    input_map: &InputMap,
    project: &renzora_project_api::CurrentProject,
) -> Result<(), String> {
    let path = project.resolve_path("input_map.ron");
    let content = ron::ser::to_string_pretty(input_map, ron::ser::PrettyConfig::default())
        .map_err(|e| format!("Failed to serialize input map: {}", e))?;
    std::fs::write(&path, content)
        .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;
    info!("[InputPlugin] Saved input map to {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ron_round_trip_preserves_actions() {
        // Maps live on disk as RON files. A round-trip must preserve the
        // action set exactly so users don't lose bindings on save.
        let original = InputMap {
            actions: vec![
                InputAction::button("jump", vec![InputBinding::key(KeyCode::Space)]),
                InputAction::axis_2d(
                    "move",
                    vec![InputBinding::composite_2d(
                        KeyCode::KeyW,
                        KeyCode::KeyS,
                        KeyCode::KeyA,
                        KeyCode::KeyD,
                    )],
                    0.1,
                ),
            ],
        };
        let serialized = ron::to_string(&original).expect("serialize");
        let parsed: InputMap = ron::from_str(&serialized).expect("parse");
        assert_eq!(parsed, original);
    }

    /// A rebind made through the contract type is live: the next pass of
    /// `update_action_state` reads the new key, with no reload and no event.
    /// This is what a script's controls menu relies on.
    #[test]
    fn a_rebind_takes_effect_on_the_next_frame() {
        let mut world = World::new();
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::KeyF);
        world.insert_resource(keys);
        world.init_resource::<ButtonInput<MouseButton>>();
        world.init_resource::<ActionState>();
        world.insert_resource(InputMap {
            actions: vec![InputAction::button(
                "jump",
                vec![InputBinding::key(KeyCode::Space)],
            )],
        });

        let mut schedule = Schedule::default();
        schedule.add_systems(state::update_action_state);

        schedule.run(&mut world);
        assert!(!world.resource::<ActionState>().pressed("jump"));

        world
            .resource_mut::<InputMap>()
            .rebind(
                "jump",
                0,
                InputBinding::from_spec("Key:KeyF"),
            )
            .expect("jump exists");
        schedule.run(&mut world);
        assert!(world.resource::<ActionState>().pressed("jump"));
    }
}
