use super::{prepare::DlssRenderContext, Dlss, DlssFeature};
use bevy_camera::{Camera, Hdr, MainPassResolutionOverride, Projection};
use bevy_ecs::{
    query::{Has, With},
    system::{Commands, Query, ResMut},
};
use bevy_render::{sync_world::RenderEntity, MainWorld};

pub fn extract_dlss<F: DlssFeature>(
    mut commands: Commands,
    mut main_world: ResMut<MainWorld>,
    cleanup_query: Query<Has<Dlss<F>>>,
) {
    let mut cameras_3d = main_world
        .query_filtered::<(RenderEntity, &Camera, &Projection, Option<&mut Dlss<F>>), With<Hdr>>();

    for (entity, camera, camera_projection, mut dlss) in cameras_3d.iter_mut(&mut main_world) {
        let mut entity_commands = commands
            .get_entity(entity)
            .expect("Camera entity wasn't synced.");
        // Renzora patch: keep the context while inactive, since rebuilding it on
        // reactivation is slow. `reset` discards the stale history on waking.
        if !camera.is_active && dlss.is_some() {
            if let Some(dlss) = dlss.as_mut().filter(|d| !d.reset) {
                dlss.reset = true;
            }
            continue;
        }
        if dlss.is_some() && camera.is_active && camera_projection.is_perspective() {
            entity_commands.insert(dlss.as_deref().unwrap().clone());
            dlss.as_mut().unwrap().reset = false;
        } else if cleanup_query.get(entity) == Ok(true) {
            entity_commands.remove::<(Dlss<F>, DlssRenderContext<F>, MainPassResolutionOverride)>();
        }
    }
}
