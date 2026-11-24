//! Point a camera at a different render target, and make Bevy notice.
//!
//! Bevy recomputes a camera's target size (and with it the projection's aspect,
//! the view's resolution and the UI laid out on it) only when a window or image
//! changes, when the camera is added, or when its `Projection` changes
//! (`bevy_render::camera::camera_system`). Swapping the `RenderTarget` itself is
//! none of those. So a camera moved from an offscreen image back to the window
//! kept the image's size: after the render scale went to 0.5 and back to 1, the
//! game drew a half-size frame stretched over the whole window, 3D and UI alike,
//! until a window resize happened to recompute it.
//!
//! Marking the projection changed is the trigger Bevy already honours, so every
//! runtime retarget goes through [`retarget`].

use bevy::camera::RenderTarget;
use bevy::prelude::*;

/// Insert `bundle` (a `RenderTarget`, or one plus whatever goes with it) on the
/// camera `entity` and have Bevy recompute its target size this frame.
pub(crate) fn retarget(commands: &mut Commands, entity: Entity, bundle: impl Bundle) {
    commands.entity(entity).insert(bundle).queue(|mut camera: EntityWorldMut| {
        if let Some(mut projection) = camera.get_mut::<Projection>() {
            projection.set_changed();
        }
    });
}

/// [`retarget`] to the primary window.
pub(crate) fn to_window(commands: &mut Commands, entity: Entity) {
    retarget(commands, entity, RenderTarget::default());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The projection change is what makes Bevy recompute the target size.
    #[test]
    fn retargeting_marks_the_projection_changed() {
        let mut app = App::new();
        let cam = app.world_mut().spawn((RenderTarget::default(), Projection::default())).id();
        app.update();
        app.world_mut().clear_trackers();
        let mut commands_queue = bevy::ecs::world::CommandQueue::default();
        {
            let mut commands = Commands::new(&mut commands_queue, app.world());
            to_window(&mut commands, cam);
        }
        commands_queue.apply(app.world_mut());
        let projection = app.world_mut().entity(cam).get_ref::<Projection>().unwrap();
        assert!(projection.is_changed());
    }
}
