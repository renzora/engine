use bevy::prelude::*;
use renzora::{ActionState, CharacterCommand, CharacterCommandQueue};

use crate::character_controller::*;
use crate::data::*;

/// Auto-initialize character controller entities.
///
/// Entities with `CharacterControllerData` but no `CharacterControllerState` get:
/// - `CharacterControllerState` (runtime state)
/// - `CharacterControllerInput` (input buffer)
/// - `PhysicsBodyData` (kinematic) if missing
/// - `CollisionShapeData` (capsule) if missing
pub fn auto_init_character_controller(
    mut commands: Commands,
    new_controllers: Query<
        (Entity, &CharacterControllerData, Option<&Name>, Option<&PhysicsBodyData>, Option<&CollisionShapeData>),
        Without<CharacterControllerState>,
    >,
) {
    for (entity, _cc, name, body, shape) in &new_controllers {
        let label = name.map(|n| n.as_str()).unwrap_or("unnamed");
        info!("[CharacterController] Initialized on '{}' {:?}", label, entity);

        commands.entity(entity).insert((
            CharacterControllerState::default(),
            CharacterControllerInput::default(),
        ));

        // Auto-add kinematic body if not present
        if body.is_none() {
            commands.entity(entity).insert(PhysicsBodyData {
                body_type: PhysicsBodyType::KinematicBody,
                mass: 1.0,
                gravity_scale: 0.0, // Controller manages its own gravity
                linear_damping: 0.0,
                angular_damping: 0.0,
                lock_rotation_x: true,
                lock_rotation_y: true,
                lock_rotation_z: true,
                lock_translation_x: false,
                lock_translation_y: false,
                lock_translation_z: false,
            });
        }

        // Auto-add capsule collider if not present
        if shape.is_none() {
            commands.entity(entity).insert(CollisionShapeData::capsule(0.3, 0.5));
        }
    }
}

/// Process character commands from blueprints/scripts into CharacterControllerInput.
pub fn process_character_commands(
    mut queue: ResMut<CharacterCommandQueue>,
    mut inputs: Query<&mut CharacterControllerInput>,
) {
    for (entity, cmd) in queue.commands.drain(..) {
        let Ok(mut input) = inputs.get_mut(entity) else { continue };
        match cmd {
            CharacterCommand::Move(dir) => {
                input.movement = dir;
            }
            CharacterCommand::Jump => {
                input.jump = true;
            }
            CharacterCommand::Sprint(sprinting) => {
                input.sprint = sprinting;
            }
        }
    }
}

/// Auto-input system: reads ActionState and writes CharacterControllerInput
/// for entities with `auto_input: true`.
///
/// Runs after clear but before blueprints, so blueprints can still override.
pub fn auto_input_from_actions(
    action_state: Res<ActionState>,
    mut controllers: Query<(&CharacterControllerData, &mut CharacterControllerInput)>,
) {
    for (cc_data, mut input) in &mut controllers {
        if !cc_data.auto_input {
            continue;
        }
        input.movement = action_state.axis_2d(&cc_data.move_action);
        input.jump |= action_state.just_pressed(&cc_data.jump_action);
        input.sprint = action_state.pressed(&cc_data.sprint_action);
    }
}

/// Clear per-frame input flags.
///
/// Runs at the start of the frame so blueprint/script systems can write fresh input.
pub fn clear_character_input(
    mut inputs: Query<&mut CharacterControllerInput>,
) {
    for mut input in &mut inputs {
        input.movement = Vec2::ZERO;
        input.jump = false;
        input.sprint = false;
    }
}
