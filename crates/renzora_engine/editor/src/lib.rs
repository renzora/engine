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
mod streaming_panel;

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
                    camera::sync_2d_camera_targets,
                    camera::sync_viewport_camera_targets,
                    camera::share_sky_to_secondary_viewports,
                    camera::share_ibl_to_secondary_viewports,
                    camera::update_editor_camera_matrix,
                    // The controller edits the focused 2D camera live; the
                    // per-slot mirror must run after it (and after the one-shot
                    // framing) so it persists the latest framing and drives the
                    // other slots' cameras from their own stored pan/zoom.
                    (
                        camera::editor_2d_camera_controller,
                        camera::frame_2d_default,
                        camera::sync_2d_viewport_cameras,
                    )
                        .chain(),
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

        // Streaming debug panel — snapshot refresh only while the panel is the
        // active tab (hidden panel costs nothing), throttled to 4 Hz.
        {
            use renzora_ember::panel::RegisterPanelContent;
            app.init_resource::<streaming_panel::StreamingDebugSnapshot>()
                .register_panel_content("streaming_debug", true, streaming_panel::build)
                .add_systems(
                    Update,
                    streaming_panel::update_streaming_debug_snapshot
                        .run_if(in_state(renzora::SplashState::Editor))
                        .run_if(renzora_ember::dock::panel_active("streaming_debug"))
                        .run_if(bevy::time::common_conditions::on_timer(
                            std::time::Duration::from_millis(250),
                        )),
                );
        }
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
/// the viewport to 2D view. When it changes to an *affirmatively 3D* entity
/// (mesh, 3D camera, light) while we're in 2D view, fall back to 3D. Ambiguous
/// selections — a freshly dropped `SceneInstance` root, an empty group node, a
/// still-loading UI canvas — carry no renderable markers either way and must
/// leave the view alone: treating "not 2D" as "3D" used to yank a 2D project's
/// viewport into 3D every time a scene or UI was dropped into the hierarchy.
/// Other view transitions (3D ↔ UI) are left to the UI auto-switch system or
/// the user.
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
    let is_3d = world.get::<Mesh3d>(entity).is_some()
        || world.get::<Camera3d>(entity).is_some()
        || world.get::<DirectionalLight>(entity).is_some()
        || world.get::<PointLight>(entity).is_some()
        || world.get::<SpotLight>(entity).is_some();

    let view = world
        .get_resource::<ViewportSettings>()
        .map(|s| s.viewport_view)
        .unwrap_or_default();
    let target = match (is_2d, view) {
        (true, ViewportView::Two) => return,
        (true, _) => ViewportView::Two,
        (false, ViewportView::Two) if is_3d => ViewportView::Three,
        (false, _) => return,
    };
    if let Some(mut settings) = world.get_resource_mut::<ViewportSettings>() {
        settings.viewport_view = target;
    }
}
