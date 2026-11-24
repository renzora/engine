//! The two lighting jobs that write onto 3D cameras: temporal shadow filtering
//! for soft shadows, and the Sun's contact-shadow settings.
//!
//! Compiled only with `render_3d`. `ContactShadows` is bevy_pbr's, and the
//! soft-shadow fields on Bevy's lights exist only with its PCSS feature; a lean
//! export with no 3D content strips both, and with them nothing here has
//! anything to write to.

use bevy::light::ShadowFilteringMethod;
use bevy::pbr::ContactShadows;
use bevy::prelude::*;
use renzora_world_api::sun::CONTACT_SHADOW_STEPS;

use crate::Sun;

pub(crate) fn register(app: &mut App) {
    app.add_systems(Update, (match_shadow_filtering, sync_contact_shadow_settings));
}

/// Switch every 3D camera to temporal shadow filtering while any light casts
/// soft shadows, and back to Bevy's default once none does.
///
/// Soft shadows sample the shadow map with a sparse, rotating pattern, so a
/// single frame of them is stippled. Only `ShadowFilteringMethod::Temporal`
/// varies that pattern from frame to frame, which is what lets TAA average it
/// into a smooth penumbra; under the default filter the stipple is fixed and
/// TAA has nothing to smooth. Leaving this to the user meant soft shadows
/// looked broken the moment they were switched on.
///
/// Nothing else in the engine sets `ShadowFilteringMethod`, so this owns it.
/// It writes only when the wanted value differs, so a steady scene costs a
/// scan of its lights and no change detection churn.
fn match_shadow_filtering(
    mut commands: Commands,
    directional: Query<&DirectionalLight>,
    points: Query<&PointLight>,
    spots: Query<&SpotLight>,
    mut cameras: Query<(Entity, Option<&mut ShadowFilteringMethod>), With<Camera3d>>,
) {
    let soft = directional
        .iter()
        .any(|l| l.shadow_maps_enabled && l.soft_shadow_size.is_some())
        || points
            .iter()
            .any(|l| l.shadow_maps_enabled && l.soft_shadows_enabled)
        || spots
            .iter()
            .any(|l| l.shadow_maps_enabled && l.soft_shadows_enabled);
    let wanted = if soft {
        ShadowFilteringMethod::Temporal
    } else {
        ShadowFilteringMethod::default()
    };
    for (entity, method) in &mut cameras {
        match method {
            Some(mut method) if *method != wanted => *method = wanted,
            Some(_) => {}
            // A camera without the component renders with the default, so it
            // only needs one when soft shadows want something else.
            None if wanted != ShadowFilteringMethod::default() => {
                commands.entity(entity).try_insert(wanted);
            }
            None => {}
        }
    }
}

/// The `ContactShadows` a camera should carry for `sun`: its reach, thickness
/// and steps, with the steps held to [`CONTACT_SHADOW_STEPS`] and the lengths
/// kept positive (a zero-length ray divides by zero in the march).
fn contact_shadows_for(sun: &Sun) -> ContactShadows {
    ContactShadows {
        linear_steps: sun
            .contact_shadow_steps
            .clamp(*CONTACT_SHADOW_STEPS.start(), *CONTACT_SHADOW_STEPS.end()),
        thickness: sun.contact_shadow_thickness.max(0.001),
        length: sun.contact_shadow_length.max(0.01),
    }
}

/// Copy the Sun's contact-shadow settings onto every camera that draws them.
///
/// Bevy keeps these on the camera, but they are about the light, and the
/// switch that turns them on is on the Sun; a camera holding its own copy
/// would mean setting them once per camera, and the editor's viewport cameras
/// cannot be selected at all. `renzora_engine` decides which cameras carry
/// `ContactShadows`; this only fills in its values, and writes a camera only
/// when they differ so a still scene does not re-specialize anything.
fn sync_contact_shadow_settings(
    suns: Query<&Sun>,
    mut cameras: Query<&mut ContactShadows>,
) {
    let Some(sun) = suns.iter().next() else {
        return;
    };
    let want = contact_shadows_for(sun);
    for mut current in &mut cameras {
        let same = current.linear_steps == want.linear_steps
            && current.thickness == want.thickness
            && current.length == want.length;
        if !same {
            *current = want;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Sun's values reach every camera that draws contact shadows, and a
    /// camera without them is left alone.
    #[test]
    fn contact_shadow_settings_reach_the_cameras() {
        let mut app = App::new();
        app.add_systems(Update, sync_contact_shadow_settings);
        app.world_mut().spawn(Sun {
            contact_shadow_length: 1.5,
            contact_shadow_thickness: 0.25,
            contact_shadow_steps: 32,
            ..default()
        });
        let camera = app.world_mut().spawn(ContactShadows::default()).id();
        let other = app.world_mut().spawn(Transform::default()).id();
        app.update();
        let cs = app.world().get::<ContactShadows>(camera).unwrap();
        assert_eq!((cs.length, cs.thickness, cs.linear_steps), (1.5, 0.25, 32));
        assert!(app.world().get::<ContactShadows>(other).is_none());
    }

    #[test]
    fn contact_shadow_settings_are_clamped() {
        let wild = Sun {
            contact_shadow_length: 0.0,
            contact_shadow_thickness: -1.0,
            contact_shadow_steps: 100_000,
            ..default()
        };
        let cs = contact_shadows_for(&wild);
        assert!(cs.length > 0.0 && cs.thickness > 0.0);
        assert_eq!(cs.linear_steps, *CONTACT_SHADOW_STEPS.end());
    }

    fn filtering_app() -> (App, Entity) {
        let mut app = App::new();
        app.add_systems(Update, match_shadow_filtering);
        let camera = app.world_mut().spawn(Camera3d::default()).id();
        (app, camera)
    }

    fn filtering(app: &App, camera: Entity) -> ShadowFilteringMethod {
        app.world()
            .get::<ShadowFilteringMethod>(camera)
            .copied()
            .unwrap_or_default()
    }

    #[test]
    fn soft_shadows_switch_cameras_to_temporal_filtering_and_back() {
        let (mut app, camera) = filtering_app();
        let light = app
            .world_mut()
            .spawn(DirectionalLight {
                shadow_maps_enabled: true,
                ..default()
            })
            .id();
        app.update();
        assert_eq!(filtering(&app, camera), ShadowFilteringMethod::default());

        app.world_mut()
            .get_mut::<DirectionalLight>(light)
            .unwrap()
            .soft_shadow_size = Some(0.5);
        app.update();
        assert_eq!(filtering(&app, camera), ShadowFilteringMethod::Temporal);

        app.world_mut().despawn(light);
        app.update();
        assert_eq!(filtering(&app, camera), ShadowFilteringMethod::default());
    }

    /// A light with its shadow maps off casts no shadow at all, soft or not.
    #[test]
    fn a_light_without_shadow_maps_does_not_count() {
        let (mut app, camera) = filtering_app();
        app.world_mut().spawn(PointLight {
            shadow_maps_enabled: false,
            soft_shadows_enabled: true,
            ..default()
        });
        app.update();
        assert_eq!(filtering(&app, camera), ShadowFilteringMethod::default());

        app.world_mut().spawn(SpotLight {
            shadow_maps_enabled: true,
            soft_shadows_enabled: true,
            ..default()
        });
        app.update();
        assert_eq!(filtering(&app, camera), ShadowFilteringMethod::Temporal);
    }
}
