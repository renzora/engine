//! Editor-viewport mouse -> world-UI pointer ray publisher.
//!
//! World panels (`WorldUiPanel`, renzora_ember game_ui) consume rays from the
//! `renzora::WorldUiPointers` contract; in VR the wands publish ids 0/1, and
//! this module publishes the mouse as id 2 so in-world UI is hoverable and
//! clickable from the flat editor viewport too. The shipped-game equivalent
//! lives with the panel system itself and gates off in editor sessions.

use bevy::prelude::*;

/// Editor-viewport mouse → world-UI pointer ray (id 2, shared with the
/// shipped-game publisher which gates itself off in editor sessions). Lives
/// here beside the other viewport cursor→ray math. Publishing runs in edit
/// AND play mode, so world panels are hover/clickable while authoring too;
/// the editor picker still selects the panel entity on click — both firing
/// is intentional (select it AND press the button under the cursor).
pub(crate) fn publish_viewport_mouse_ray(
    settings: Option<Res<renzora::core::viewport_types::ViewportSettings>>,
    viewport: Option<Res<crate::ViewportState>>,
    windows: Query<&bevy::window::Window, With<bevy::window::PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<renzora::core::EditorCamera>>,
    mouse: Res<ButtonInput<MouseButton>>,
    pointers: Option<ResMut<renzora::WorldUiPointers>>,
) {
    use renzora::core::viewport_types::ViewportView;

    let Some(mut pointers) = pointers else { return };
    pointers.0.retain(|r| r.id != 2);

    // 3D view only — panels are 3D content.
    if settings.is_some_and(|s| s.viewport_view == ViewportView::Two) {
        return;
    }
    let Some(viewport) = viewport else { return };
    if !viewport.hovered {
        return;
    }
    let Ok(window) = windows.single() else { return };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    let Ok((camera, camera_tf)) = cameras.single() else {
        return;
    };
    // Window-logical cursor → render-image pixels (the viewport image and the
    // panel can differ in size — same scaling as every other cursor→ray site).
    let image_size = viewport.current_size.as_vec2();
    if image_size.x <= 0.0 || viewport.screen_size.x <= 0.0 {
        return;
    }
    let render_pos = (cursor - viewport.screen_position) / viewport.screen_size * image_size;
    let Ok(ray) = camera.viewport_to_world(camera_tf, render_pos) else {
        return;
    };
    pointers.0.push(renzora::core::WorldUiPointerRay {
        id: 2,
        ray,
        trigger: if mouse.pressed(MouseButton::Left) { 1.0 } else { 0.0 },
    });
}
