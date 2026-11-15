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
        // Renzora patch: an inactive camera keeps its DLSS context. Removing it
        // here meant every camera that idled and woke again (a viewport hidden
        // and shown, render-on-change) rebuilt the context, 35 to 40 ms each
        // time. While inactive the camera has no `ExtractedView`, so nothing
        // prepares or runs DLSS for it; on waking, `reset` drops the history
        // it built before it went idle.
        if !camera.is_active {
            if let Some(dlss) = dlss.as_mut().filter(|d| !d.reset) {
                dlss.reset = true;
            }
            continue;
        }
        if dlss.is_some() && camera_projection.is_perspective() {
            entity_commands.insert(dlss.as_deref().unwrap().clone());
            dlss.as_mut().unwrap().reset = false;
        } else if cleanup_query.get(entity) == Ok(true) {
            entity_commands.remove::<(Dlss<F>, DlssRenderContext<F>, MainPassResolutionOverride)>();
        }
    }
}
