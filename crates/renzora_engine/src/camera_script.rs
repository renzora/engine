//! Camera scripting: read and write a camera's field of view.
//!
//! FOV lives in Bevy's `Projection`, which is an *enum* — the generic
//! `get("Component.field")` / `set(...)` reflect paths cannot address a field
//! inside an enum variant, so a script had no way to reach it at all. (The old
//! API draft documented a `set_camera_fov` that was never implemented; this is
//! that function, actually built.)
//!
//! The shape follows `renzora_animation`: the mutation is a declared
//! `ScriptAction` handled by an observer here, and the read goes through a
//! small reflected mirror component, because reads resolve as reflected field
//! lookups and there is nothing reflectable to point them at otherwise.
//!
//! Degrees, not radians, on both sides: that is what the inspector's FOV field
//! shows, and a script and the inspector disagreeing about units is a bug
//! waiting to be filed.

use bevy::prelude::*;
use renzora::ScriptAction;
use serde::{Deserialize, Serialize};

/// Same bounds the inspector's FOV field clamps to — past these a perspective
/// projection degenerates.
const MIN_FOV_DEGREES: f32 = 10.0;
const MAX_FOV_DEGREES: f32 = 170.0;

/// Read-only mirror of a camera's lens, for `camera_fov()`. Never saved to
/// scenes — it is rebuilt from `Projection` every frame.
#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct CameraReadState {
    /// Vertical field of view in degrees, or 0 for an orthographic camera.
    pub fov: f32,
}

/// Auto-inserts [`CameraReadState`] on any camera that lacks one.
pub fn auto_init_camera_read_state(
    mut commands: Commands,
    cameras: Query<Entity, (With<Camera>, With<Projection>, Without<CameraReadState>)>,
) {
    for entity in &cameras {
        commands.entity(entity).try_insert(CameraReadState::default());
    }
}

/// Refreshes [`CameraReadState`] from the camera's projection.
pub fn update_camera_read_state(mut cameras: Query<(&Projection, &mut CameraReadState)>) {
    for (projection, mut state) in &mut cameras {
        let fov = match projection {
            Projection::Perspective(p) => p.fov.to_degrees(),
            // Orthographic and custom projections have no FOV. Reporting 0
            // rather than a fake angle lets a script tell the difference.
            _ => 0.0,
        };
        if state.fov != fov {
            state.fov = fov;
        }
    }
}

/// Observer: applies `set_fov` to the acting entity's perspective projection.
pub fn handle_camera_script_actions(
    trigger: On<ScriptAction>,
    mut cameras: Query<&mut Projection>,
) {
    use renzora::ScriptActionValue as V;
    let action = trigger.event();
    if action.name != "set_camera_fov" {
        return;
    }

    let degrees = match action.args.get("degrees") {
        Some(V::Float(v)) => *v,
        Some(V::Int(v)) => *v as f32,
        _ => return,
    };

    // `entity_id` lets a script aim at another camera; without it the action
    // applies to whatever entity the script is attached to.
    let target = match action.args.get("entity_id") {
        Some(V::Int(id)) => Entity::from_bits(*id as u64),
        _ => action.entity,
    };

    let Ok(mut projection) = cameras.get_mut(target) else {
        return;
    };
    if let Projection::Perspective(ref mut perspective) = *projection {
        perspective.fov = degrees.clamp(MIN_FOV_DEGREES, MAX_FOV_DEGREES).to_radians();
    }
}

/// Camera scripting bindings — declared, not written, so every language
/// backend gets them and this crate compiles no interpreter.
#[cfg(feature = "scripting")]
pub struct CameraScriptExtension;

#[cfg(feature = "scripting")]
impl renzora_scripting::extension::ScriptExtension for CameraScriptExtension {
    fn name(&self) -> &str {
        "camera"
    }

    fn bindings(&self) -> Vec<renzora_scripting::extension::Binding> {
        use renzora_scripting::extension::{Bind, ParamKind};
        vec![
            Bind::action("set_fov", "set_camera_fov")
                .arg("degrees", ParamKind::Float)
                .doc("Set this camera's vertical field of view, in degrees (10–170).")
                .build(),
            Bind::read("camera_fov", "CameraReadState", "fov")
                .doc("This camera's vertical field of view in degrees, 0 if orthographic.")
                .build(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_state_mirrors_perspective_fov_in_degrees() {
        // The inspector shows degrees and so does `set_fov`; a mirror reporting
        // radians would make `camera_fov()` silently disagree with both.
        let mut world = World::new();
        let entity = world
            .spawn((
                Projection::Perspective(PerspectiveProjection {
                    fov: std::f32::consts::FRAC_PI_3,
                    ..default()
                }),
                CameraReadState::default(),
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_camera_read_state);
        schedule.run(&mut world);

        let state = world.get::<CameraReadState>(entity).unwrap();
        assert!((state.fov - 60.0).abs() < 1e-3, "got {}", state.fov);
    }

    #[test]
    fn orthographic_cameras_report_zero() {
        let mut world = World::new();
        let entity = world
            .spawn((
                Projection::Orthographic(OrthographicProjection::default_3d()),
                CameraReadState::default(),
            ))
            .id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_camera_read_state);
        schedule.run(&mut world);

        assert_eq!(world.get::<CameraReadState>(entity).unwrap().fov, 0.0);
    }
}
