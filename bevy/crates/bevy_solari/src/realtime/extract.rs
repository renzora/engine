use super::{
    debug::{SolariDebug, SolariDebugResources},
    prepare::SolariLightingResources,
    SolariLighting,
};
use bevy_camera::Camera;
use bevy_ecs::system::{Commands, ResMut};
use bevy_pbr::deferred::SkipDeferredLighting;
use bevy_render::{sync_world::RenderEntity, MainWorld};

pub fn extract_solari_lighting(mut main_world: ResMut<MainWorld>, mut commands: Commands) {
    let mut cameras_3d = main_world.query::<(
        RenderEntity,
        &Camera,
        Option<&mut SolariLighting>,
        Option<&mut SolariDebug>,
    )>();

    for (entity, camera, solari_lighting, solari_debug) in cameras_3d.iter_mut(&mut main_world) {
        let mut entity_commands = commands
            .get_entity(entity)
            .expect("Camera entity wasn't synced.");
        if let Some(mut solari_lighting) = solari_lighting
            && camera.is_active
        {
            entity_commands.insert((solari_lighting.clone(), SkipDeferredLighting));
            solari_lighting.reset = false;
            // RENZORA PATCH: the diagnostics (debug.rs), extracted and removed
            // here rather than by a generic extract, which never removes.
            match solari_debug {
                Some(mut solari_debug) => {
                    entity_commands.insert(*solari_debug);
                    if solari_debug.reset_reference {
                        solari_debug.reset_reference = false;
                    }
                }
                None => {
                    entity_commands.remove::<(SolariDebug, SolariDebugResources)>();
                }
            }
        } else {
            entity_commands.remove::<(
                SolariLighting,
                SolariLightingResources,
                SkipDeferredLighting,
                SolariDebug,
                SolariDebugResources,
            )>();
        }
    }
}
