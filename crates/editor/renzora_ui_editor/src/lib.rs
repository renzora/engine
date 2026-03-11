//! Renzora UI Editor — Figma-like visual editor for bevy_ui game interfaces.
//!
//! Provides three editor panels:
//! - **Widget Palette** (`widget_library`) — drag widget types to create entities
//! - **UI Canvas** (`ui_canvas`) — 2D visual editor to position and resize widgets
//! - **UI Inspector** (`ui_inspector`) — property editor for selected widget
//!
//! Game UI is built with bevy_ui components (`Node`, `Text`, `ImageNode`, etc.)
//! and marked with `UiCanvas` / `UiWidget` components for editor integration.
//! At runtime, bevy_ui renders them automatically — no editor code needed.

pub mod canvas;
pub mod components;
pub mod inspector;
pub mod palette;

use bevy::prelude::*;
use renzora_core::PlayModeState;
use renzora_editor::AppEditorExt;

use canvas::UiCanvasPanel;
use components::{UiCanvas, UiWidget};
use inspector::UiInspectorPanel;
use palette::WidgetPalettePanel;

pub struct UiEditorPlugin;

impl Plugin for UiEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] UiEditorPlugin");

        // Register components for reflection (scene serialization)
        app.register_type::<UiCanvas>();
        app.register_type::<UiWidget>();

        // Register panels
        app.register_panel(WidgetPalettePanel::default());
        app.register_panel(UiCanvasPanel::default());
        app.register_panel(UiInspectorPanel::default());

        // Ensure UI entities have required visibility components + hide during editing
        app.add_systems(Update, (ensure_ui_visibility_components, sync_ui_canvas_visibility, debug_ui_tree).chain());
    }
}

/// Ensures UiCanvas and UiWidget entities have the full visibility component stack
/// that bevy_ui needs (Visibility, InheritedVisibility, ViewVisibility).
/// Scene deserialization may not insert required components automatically.
fn ensure_ui_visibility_components(
    mut commands: Commands,
    canvases_no_iv: Query<Entity, (With<UiCanvas>, Without<InheritedVisibility>)>,
    widgets_no_iv: Query<Entity, (With<UiWidget>, Without<InheritedVisibility>)>,
) {
    for entity in canvases_no_iv.iter().chain(widgets_no_iv.iter()) {
        info!("[ui_editor] Inserting missing visibility components on {:?}", entity);
        commands.entity(entity).insert((
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));
    }
}

/// Hides `UiCanvas` entities while editing (the egui canvas panel shows the preview).
/// Shows them during play mode so bevy_ui renders them on the game camera.
/// Sets `UiTargetCamera` to the game camera (which renders to the viewport texture).
fn sync_ui_canvas_visibility(
    mut commands: Commands,
    play_mode: Res<PlayModeState>,
    mut canvases: Query<(Entity, &mut Visibility, &Name, Option<&bevy::ui::UiTargetCamera>), With<UiCanvas>>,
) {
    let in_play = play_mode.is_in_play_mode();
    let target = if in_play {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };

    let game_camera = play_mode.active_game_camera;

    for (entity, mut vis, name, existing_target_cam) in &mut canvases {
        if *vis != target {
            info!(
                "[ui_editor] Canvas {:?} ({}) visibility: {:?} -> {:?} (play_mode={})",
                entity, name, *vis, target, in_play
            );
            *vis = target;
        }

        if in_play {
            if let Some(cam_entity) = game_camera {
                let needs_insert = match existing_target_cam {
                    Some(tc) => tc.entity() != cam_entity,
                    None => true,
                };
                if needs_insert {
                    info!(
                        "[ui_editor] Setting UiTargetCamera on {:?} ({}) -> game camera {:?}",
                        entity, name, cam_entity
                    );
                    commands.entity(entity).insert(bevy::ui::UiTargetCamera(cam_entity));
                }
            }
        } else if existing_target_cam.is_some() {
            info!("[ui_editor] Removing UiTargetCamera from {:?} ({})", entity, name);
            commands.entity(entity).remove::<bevy::ui::UiTargetCamera>();
        }
    }

    if canvases.is_empty() {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let count = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if count % 300 == 0 {
            warn!("[ui_editor] No UiCanvas entities found in scene (play_mode={})", in_play);
        }
    }
}

/// Diagnostic: log full UI entity tree state when entering play mode.
fn debug_ui_tree(
    play_mode: Res<PlayModeState>,
    canvases: Query<(Entity, &Name, &Node, &Visibility, Option<&InheritedVisibility>, Option<&ViewVisibility>), With<UiCanvas>>,
    widgets: Query<(Entity, &Name, &Node, &Visibility, Option<&InheritedVisibility>, Option<&ViewVisibility>, Option<&ChildOf>), With<UiWidget>>,
    cameras: Query<(Entity, &Camera, Option<&Name>)>,
) {
    // Only log on play mode transitions
    static LAST_PLAY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let in_play = play_mode.is_in_play_mode();
    let was_playing = LAST_PLAY.swap(in_play, std::sync::atomic::Ordering::Relaxed);
    if in_play == was_playing {
        return;
    }

    info!("[ui_editor] === UI TREE DUMP (play_mode={}) ===", in_play);

    for (entity, name, node, vis, inh_vis, view_vis) in &canvases {
        info!(
            "[ui_editor]   CANVAS {:?} name={} vis={:?} inherited={:?} view={:?} w={:?} h={:?} pos={:?}",
            entity, name, vis, inh_vis, view_vis, node.width, node.height, node.position_type,
        );
    }

    for (entity, name, node, vis, inh_vis, view_vis, parent) in &widgets {
        info!(
            "[ui_editor]   WIDGET {:?} name={} parent={:?} vis={:?} inherited={:?} view={:?} w={:?} h={:?}",
            entity, name, parent.map(|p| p.parent()), vis, inh_vis, view_vis, node.width, node.height,
        );
    }

    for (entity, camera, name) in &cameras {
        info!(
            "[ui_editor]   CAMERA {:?} name={:?} active={} order={}",
            entity,
            name.map(|n| n.as_str()),
            camera.is_active,
            camera.order,
        );
    }

    info!("[ui_editor] === END UI TREE DUMP ===");
}
