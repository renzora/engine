//! Editor-side (bevy_ui / ember) panels for `renzora_game_ui`.
//!
//! Currently: a WORK-IN-PROGRESS native port of the egui `UiCanvasPanel`
//! (panel id "ui_canvas") — the WYSIWYG UI canvas editor — built in split files.
//!
//! Why a separate crate: the native panels need `renzora_ember`, but
//! `ember -> renzora_hui -> renzora_game_ui`, so `game_ui` can't depend on ember
//! (cycle). This crate sits above ember and depends on both, reading game_ui's
//! editor types (`UiCanvas`, `canvas_render::UiCanvasRender`).
//!
//! Architecture: the game UI is *real bevy_ui* rendered to an offscreen image by
//! `renzora_game_ui::canvas_render` (`UiCanvasRender.image_handle`). The native
//! panel displays that rendered image (an `ImageNode`) and overlays the editing
//! chrome — selection box, resize/rotate handles, marquee — as bevy_ui nodes, so
//! it never has to reimplement the egui `paint_*` widget-preview functions.
//!
//! Files:
//! - `lib.rs`     — plugin, shared `NativeCanvasState`, active-canvas sync, root build
//! - `viewport.rs`— the rendered-canvas image + zoomed design frame
//! - `toolbar.rs` — zoom / grid / snap controls
//!
//! NOT YET REGISTERED: the plugin sets up state + the toolbar, but does not call
//! `register_panel_content("ui_canvas", ...)` until the selection + interaction
//! layer (`overlay.rs` / `interaction.rs`: click/marquee select, drag-move,
//! resize, rotate, align/distribute) lands — so the egui panel stays active and
//! direct-manipulation editing isn't lost mid-port. Flip it on in `build_panel`.

#![allow(dead_code)]

use bevy::prelude::*;

use renzora_editor::SplashState;
use renzora_ember::font::EmberFonts;
use renzora_game_ui::UiCanvas;

mod toolbar;
mod viewport;

/// Persistent native-canvas editor state (mirrors the egui `CanvasState`'s
/// non-interaction fields). Interaction state (drag/resize/rotate/marquee) will
/// live alongside the interaction systems in follow-up files.
#[derive(Resource)]
pub(crate) struct NativeCanvasState {
    pub zoom: f32,
    pub pan: Vec2,
    pub grid_size: f32,
    pub show_grid: bool,
    pub snap_enabled: bool,
    pub active_canvas: Option<Entity>,
    pub canvas_width: f32,
    pub canvas_height: f32,
}

impl Default for NativeCanvasState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
            grid_size: 10.0,
            show_grid: true,
            snap_enabled: true,
            active_canvas: None,
            canvas_width: 1280.0,
            canvas_height: 720.0,
        }
    }
}

#[derive(Default)]
pub struct GameUiEditorPlugin;

impl Plugin for GameUiEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] GameUiEditorPlugin");
        app.init_resource::<NativeCanvasState>();
        app.add_systems(Update, sync_active_canvas.run_if(in_state(SplashState::Editor)));
        toolbar::register(app);

        // Not registered yet — flip on once selection/interaction lands:
        // app.register_panel_content("ui_canvas", false, build_panel);
        let _ = build_panel as fn(&mut Commands, &EmberFonts) -> Entity;
    }
}

renzora::add!(GameUiEditorPlugin, Editor);

fn build_panel(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() },
            Name::new("native-ui-canvas"),
        ))
        .id();
    let toolbar = toolbar::build(commands, fonts);
    let viewport = viewport::build(commands, fonts);
    commands.entity(root).add_children(&[toolbar, viewport]);
    root
}

/// Track the active canvas + mirror its reference resolution, like the egui
/// panel did at the top of `ui()`.
fn sync_active_canvas(mut state: ResMut<NativeCanvasState>, canvases: Query<(Entity, &UiCanvas)>) {
    let still_valid = state.active_canvas.is_some_and(|a| canvases.get(a).is_ok());
    if !still_valid {
        state.active_canvas = canvases.iter().next().map(|(e, _)| e);
    }
    if let Some(active) = state.active_canvas {
        if let Ok((_, canvas)) = canvases.get(active) {
            state.canvas_width = canvas.reference_width;
            state.canvas_height = canvas.reference_height;
        }
    }
}
