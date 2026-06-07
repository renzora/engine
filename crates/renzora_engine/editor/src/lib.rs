//! Editor-only half of `renzora_engine` (the dual-mode crate split).
//!
//! `renzora_engine` compiles lean — no `editor` cargo feature — so its
//! game-startup code is always present and branches at RUNTIME on
//! `renzora::EditorSession`. The editor-only pieces that used to live behind
//! `#[cfg(feature = "editor")]` in `renzora_engine` moved here: the editor
//! camera lifecycle, the save-scene observer, the 2D selection auto-view-switch,
//! and the native (ember) crash-report overlay.
//!
//! [`EngineEditorPlugin`] registers via `renzora::add!(.., Editor)`. The editor
//! bundle (`renzora_editor`) links this crate as an rlib and replays its
//! Editor-scope registration at dlopen; the lean runtime never links it.

use bevy::prelude::*;

mod crash_overlay;

/// Editor-scope companion to `renzora_engine::RuntimePlugin`. Adds the editor
/// camera lifecycle, the save-scene observer, the 2D selection auto-view-switch,
/// and the crash-report overlay.
#[derive(Default)]
pub struct EngineEditorPlugin;

impl Plugin for EngineEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] EngineEditorPlugin");
        use renzora_engine::camera;

        // Editor camera spawn is deferred until the loading screen hands off to
        // the editor view (`OnEnter(SplashState::Editor)`). By then a project is
        // loaded and `ResolvedRenderingMode` reflects its choice, so the camera
        // attaches the right prepass at spawn time. Bevy 0.18 specializes the
        // prepass pipeline at first render, so the attachments must be correct
        // *at spawn* — they can't be retrofitted. `OnEnter(Editor)` is the only
        // place both conditions hold (the long-form rationale lived in
        // renzora_engine before the split).
        app.init_resource::<renzora::viewport_types::EditorCameraMatrix>()
            .init_resource::<LastSelectionForView2dSwitch>()
            .add_systems(
                OnEnter(renzora::SplashState::Editor),
                (
                    // MUST run before camera spawn — updates ResolvedRenderingMode
                    // from the loaded project so the camera attaches the right
                    // prepass attachments at spawn time.
                    renzora_engine::sync_rendering_mode_from_project,
                    camera::spawn_editor_camera,
                    camera::spawn_editor_2d_camera,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    camera::sync_camera_render_target,
                    camera::sync_viewport_camera_targets,
                    camera::share_sky_to_secondary_viewports,
                    camera::share_ibl_to_secondary_viewports,
                    camera::update_editor_camera_matrix,
                    camera::editor_2d_camera_controller,
                    auto_switch_view_on_2d_selection,
                ),
            );

        // Save-scene event from the editor.
        app.add_observer(on_save_current_scene);

        // Native (ember) crash-report overlay — surfaces the previous session's
        // crash while in the editor. The runtime-side `CrashReportPlugin` (in
        // renzora_engine) owns the panic hook and the `CrashReportWindowState`
        // this reads.
        app.add_systems(
            Update,
            (
                crash_overlay::manage_crash_overlay,
                crash_overlay::crash_overlay_buttons,
            )
                .run_if(in_state(renzora::SplashState::Editor)),
        );
    }
}

renzora::add!(EngineEditorPlugin, Editor);

/// Listen for the editor's save-scene event and persist the current scene.
fn on_save_current_scene(_trigger: On<renzora::SaveCurrentScene>, mut commands: Commands) {
    commands.queue(|world: &mut World| {
        renzora_engine::scene_io::save_current_scene(world);
    });
}

/// Tracks the last selection processed for auto-view-switching, so the 2D-flip
/// fires on selection *change* only — same pattern the UI auto-switch uses, but
/// kept independent so the two systems don't fight over a shared tracker.
#[derive(Resource, Default)]
pub struct LastSelectionForView2dSwitch(pub Option<Entity>);

/// When the selection changes to a 2D entity (Sprite, Camera2d or Node2d), flip
/// the viewport to 2D view. When it changes to a non-2D entity *and* we're
/// currently in 2D view, fall back to 3D. Other view transitions (3D ↔ UI) are
/// left to the UI auto-switch system or the user.
pub fn auto_switch_view_on_2d_selection(world: &mut World) {
    use renzora::core::viewport_types::{ViewportSettings, ViewportView};

    let current_sel = world
        .get_resource::<renzora::EditorSelection>()
        .and_then(|s| s.get());
    let last_sel = world
        .get_resource::<LastSelectionForView2dSwitch>()
        .map(|l| l.0)
        .unwrap_or(None);
    if current_sel == last_sel {
        return;
    }
    if let Some(mut last) = world.get_resource_mut::<LastSelectionForView2dSwitch>() {
        last.0 = current_sel;
    }
    let Some(entity) = current_sel else { return };

    let is_2d = world.get::<bevy::sprite::Sprite>(entity).is_some()
        || world.get::<Camera2d>(entity).is_some()
        || world.get::<renzora::core::Node2d>(entity).is_some();

    let view = world
        .get_resource::<ViewportSettings>()
        .map(|s| s.viewport_view)
        .unwrap_or_default();
    let target = match (is_2d, view) {
        (true, ViewportView::Two) => return,
        (true, _) => ViewportView::Two,
        (false, ViewportView::Two) => ViewportView::Three,
        (false, _) => return,
    };
    if let Some(mut settings) = world.get_resource_mut::<ViewportSettings>() {
        settings.viewport_view = target;
    }
}
