//! Renzora Game UI — bevy_ui game interface components and (optionally) editor panels.
//!
//! **Runtime** (always available):
//! - `UiCanvas`, `UiWidget`, `UiWidgetType` — serializable marker components
//! - `GameUiPlugin` — registers types for reflection + runtime systems
//!
//! **Editor** (behind `editor` feature):
//! - Widget Palette, UI Canvas, and UI Inspector panels
//! - Play-mode visibility sync, debug tree logging

pub mod components;

#[cfg(feature = "editor")]
pub mod canvas;
#[cfg(feature = "editor")]
pub mod inspector;
#[cfg(feature = "editor")]
pub mod palette;

use bevy::prelude::*;

pub use components::{UiCanvas, UiWidget, UiWidgetType};

pub struct GameUiPlugin;

impl Plugin for GameUiPlugin {
    fn build(&self, app: &mut App) {
        // Register components for reflection (scene serialization) — always needed
        app.register_type::<UiCanvas>();
        app.register_type::<UiWidget>();

        #[cfg(feature = "editor")]
        {
            use renzora_editor::AppEditorExt;
            info!("[editor] GameUiPlugin (editor panels)");

            app.register_panel(palette::WidgetPalettePanel::default());
            app.register_panel(canvas::UiCanvasPanel::default());
            app.register_panel(inspector::UiInspectorPanel::default());

            app.add_systems(
                Update,
                (
                    ensure_ui_visibility_components,
                    sync_ui_canvas_visibility,
                    debug_ui_tree,
                )
                    .chain(),
            );
        }

        #[cfg(not(feature = "editor"))]
        {
            info!("[runtime] GameUiPlugin");
        }
    }
}

// ── Editor-only systems ─────────────────────────────────────────────────────

#[cfg(feature = "editor")]
fn ensure_ui_visibility_components(
    mut commands: Commands,
    canvases_no_iv: Query<Entity, (With<UiCanvas>, Without<InheritedVisibility>)>,
    widgets_no_iv: Query<Entity, (With<UiWidget>, Without<InheritedVisibility>)>,
) {
    for entity in canvases_no_iv.iter().chain(widgets_no_iv.iter()) {
        info!(
            "[ui_editor] Inserting missing visibility components on {:?}",
            entity
        );
        commands.entity(entity).insert((
            InheritedVisibility::default(),
            ViewVisibility::default(),
        ));
    }
}

#[cfg(feature = "editor")]
fn sync_ui_canvas_visibility(
    mut commands: Commands,
    play_mode: Res<renzora_core::PlayModeState>,
    mut canvases: Query<
        (
            Entity,
            &mut Visibility,
            &Name,
            Option<&bevy::ui::UiTargetCamera>,
        ),
        With<UiCanvas>,
    >,
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
                    commands
                        .entity(entity)
                        .insert(bevy::ui::UiTargetCamera(cam_entity));
                }
            }
        } else if existing_target_cam.is_some() {
            info!(
                "[ui_editor] Removing UiTargetCamera from {:?} ({})",
                entity, name
            );
            commands
                .entity(entity)
                .remove::<bevy::ui::UiTargetCamera>();
        }
    }

    if canvases.is_empty() {
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let count = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if count % 300 == 0 {
            warn!(
                "[ui_editor] No UiCanvas entities found in scene (play_mode={})",
                in_play
            );
        }
    }
}

#[cfg(feature = "editor")]
fn debug_ui_tree(
    play_mode: Res<renzora_core::PlayModeState>,
    canvases: Query<
        (
            Entity,
            &Name,
            &Node,
            &Visibility,
            Option<&InheritedVisibility>,
            Option<&ViewVisibility>,
        ),
        With<UiCanvas>,
    >,
    widgets: Query<
        (
            Entity,
            &Name,
            &Node,
            &Visibility,
            Option<&InheritedVisibility>,
            Option<&ViewVisibility>,
            Option<&ChildOf>,
        ),
        With<UiWidget>,
    >,
    cameras: Query<(Entity, &Camera, Option<&Name>)>,
) {
    static LAST_PLAY: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);
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
            entity,
            name,
            parent.map(|p| p.parent()),
            vis,
            inh_vis,
            view_vis,
            node.width,
            node.height,
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
