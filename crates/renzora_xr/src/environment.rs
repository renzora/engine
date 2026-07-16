//! Environment sync: the engine's sky/atmosphere systems decorate the
//! scene's authored cameras, but the XR eye cameras (and the desktop mirror)
//! are spawned by the XR stack and never pass through that authoring path —
//! so mirror the `Skybox` + clear color from the scene's camera onto every
//! XR-side view each frame.

use bevy::core_pipeline::Skybox;
use bevy::prelude::*;
use bevy_mod_xr::camera::XrCamera;

use crate::rig::VrMirrorCamera;

pub(crate) fn register(app: &mut App) {
    app.add_systems(Update, sync_environment_to_xr_cameras);
}

/// Query split, not `&mut Camera` everywhere: the read-only source query and
/// the mutable target query stay provably disjoint (`Without<XrCamera>` vs
/// `With<XrCamera>`, likewise the mirror marker) so the system can't trip
/// Bevy's access checker.
#[allow(clippy::type_complexity)]
fn sync_environment_to_xr_cameras(
    mut commands: Commands,
    source_cameras: Query<
        (Option<&Skybox>, &Camera),
        (With<Camera3d>, Without<XrCamera>, Without<VrMirrorCamera>),
    >,
    xr_cameras: Query<(Entity, Option<&Skybox>), With<XrCamera>>,
    mirror_cameras: Query<(Entity, Option<&Skybox>), (With<VrMirrorCamera>, Without<XrCamera>)>,
    mut target_settings: ParamSet<(
        Query<&mut Camera, With<XrCamera>>,
        Query<&mut Camera, With<VrMirrorCamera>>,
    )>,
) {
    // The scene's authored camera is deactivated in VR (see rig.rs) but its
    // components remain the authoring source of truth.
    let Some((source_skybox, source_camera)) = source_cameras.iter().next() else {
        return;
    };
    let clear = source_camera.clear_color;

    let sync_skybox = |commands: &mut Commands, entity: Entity, existing: Option<&Skybox>| {
        match (source_skybox, existing) {
            (Some(sky), Some(current)) => {
                if current.image != sky.image || current.brightness != sky.brightness {
                    commands.entity(entity).insert(sky.clone());
                }
            }
            (Some(sky), None) => {
                commands.entity(entity).insert(sky.clone());
            }
            (None, Some(_)) => {
                commands.entity(entity).remove::<Skybox>();
            }
            (None, None) => {}
        }
    };

    for (entity, existing) in xr_cameras.iter() {
        sync_skybox(&mut commands, entity, existing);
    }
    for (entity, existing) in mirror_cameras.iter() {
        sync_skybox(&mut commands, entity, existing);
    }
    // `ClearColorConfig` has no `PartialEq`, so compare structurally before
    // writing — unconditional writes would dirty `Camera` change detection
    // every frame.
    for mut camera in target_settings.p0().iter_mut() {
        if !clear_color_eq(&camera.clear_color, &clear) {
            camera.clear_color = clear;
        }
    }
    for mut camera in target_settings.p1().iter_mut() {
        if !clear_color_eq(&camera.clear_color, &clear) {
            camera.clear_color = clear;
        }
    }
}

fn clear_color_eq(
    a: &bevy::camera::ClearColorConfig,
    b: &bevy::camera::ClearColorConfig,
) -> bool {
    use bevy::camera::ClearColorConfig as C;
    match (a, b) {
        (C::Default, C::Default) | (C::None, C::None) => true,
        (C::Custom(x), C::Custom(y)) => x == y,
        _ => false,
    }
}
