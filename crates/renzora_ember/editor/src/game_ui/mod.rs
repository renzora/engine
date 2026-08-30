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
//! `crate::game_ui::canvas_render` (`UiCanvasRender.image_handle`). The native
//! panel displays that rendered image (an `ImageNode`) and overlays the editing
//! chrome — selection box, resize/rotate handles, marquee — as bevy_ui nodes, so
//! it never has to reimplement the egui `paint_*` widget-preview functions.
//!
//! Files:
//! - `lib.rs`     — plugin, shared `NativeCanvasState`, active-canvas sync, root build
//! - `viewport.rs`— the rendered-canvas image + zoomed design frame
//! - `toolbar.rs` — zoom / grid / snap controls
//!
//! # Its own panel
//!
//! Registered as the `ui_canvas` dock panel. It spent a while mounted *inside*
//! the viewport panel instead, revealed by `ViewportView::Ui` — a stopgap from
//! before `overlay.rs` / `interaction.rs` existed, when the egui panel was still
//! the real editor and this could not yet stand alone.
//!
//! The cost of that arrangement was that opening a UI took the 3D view away:
//! one surface, two jobs, and whichever you were not doing vanished. As a panel
//! it docks anywhere, sits beside the viewport if you want both, and the
//! viewport goes back to being 3D-or-2D and nothing else.
//!
//! It still shows the scene behind the canvas — `viewport.rs` reads the shared
//! `renzora::ViewportRenderTarget` image, which is contract state any panel may
//! sample. Borrowing the picture never required living inside the panel that
//! draws it.

#![allow(dead_code)]

use bevy::prelude::*;

use renzora::SplashState;
use renzora_ember::font::EmberFonts;
use renzora_ember::game_ui::UiCanvas;

mod align;
pub mod canvas;
pub mod canvas_render;
mod geometry;
mod inspectors;
mod interaction;
mod nav;
mod overlay;
mod register;
mod ruler;
pub mod spawn_ext;
mod toolbar;
mod ui_inspector;
mod viewport;

use overlay::CanvasHitLayer;

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
    /// Per-frame design-space geometry of every widget under the active canvas.
    pub widgets: Vec<geometry::WidgetGeom>,
    /// Active marquee (rubber-band) selection rectangle in design space, as
    /// `(press, current)` corners. `None` when not box-selecting. The overlay
    /// draws it; release selects the widgets it fully encloses.
    pub marquee: Option<(Vec2, Vec2)>,
    /// Where a flow drag would drop, recomputed each frame while one is in
    /// progress. The overlay outlines `parent` and draws the insertion line;
    /// releasing applies it. `None` whenever no flow drag is running.
    pub drop: Option<geometry::DropTarget>,
    /// The widget under the cursor, recomputed every frame. Only needed for the
    /// hover name badge — the click path does its own hit-test at press time,
    /// because what a press hits depends on the current selection.
    pub hovered: Option<Entity>,
    /// Outline the node under the cursor, without needing a click.
    pub hover_outline: bool,
    /// Also outline the container the cursor is inside — the node's parent.
    /// Reading which group you are in is most of what is hard about a nested
    /// template, and it is the same box the drop target draws during a drag.
    pub hover_group: bool,
    /// Put a node's name on the boxes above. Off leaves the outlines, which are
    /// still useful once you know the tree.
    pub show_names: bool,
    /// Design-space rulers along the top and left of the canvas.
    pub show_rulers: bool,
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
            widgets: Vec::new(),
            marquee: None,
            drop: None,
            hovered: None,
            hover_outline: true,
            hover_group: true,
            show_names: true,
            show_rulers: true,
        }
    }
}

#[derive(Default)]
pub struct GameUiEditorPlugin;

impl Plugin for GameUiEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] GameUiEditorPlugin");
        // Editor inspector entries, hierarchy icons, presets, UI render target,
        // and editor-only sync/debug systems — relocated verbatim from
        // `renzora_game_ui`'s old `#[cfg(feature = "editor")]` block.
        register::register_game_ui_editor(app);
        app.init_resource::<NativeCanvasState>();
        // panel-systems-ungated: game-UI runtime, not editor chrome — not tied to a panel's visibility
        app.add_systems(Update, sync_active_canvas.run_if(in_state(SplashState::Editor)));
        // The canvas geometry snapshot only runs while the panel is actually
        // mounted (the hit layer exists), so it costs nothing when closed/unwired.
        // panel-systems-ungated: game-UI runtime, not editor chrome — not tied to a panel's visibility
        app.add_systems(
            Update,
            // Chained: `track_hover` hit-tests the snapshot, so it has to read
            // the one taken this frame rather than last frame's.
            (geometry::snapshot_widgets, geometry::track_hover).chain().run_if(in_state(SplashState::Editor)).run_if(any_with_component::<CanvasHitLayer>),
        );
        toolbar::register(app);
        overlay::register(app);
        interaction::register(app);
        nav::register(app);
        ruler::register(app);
        inspectors::register(app);
        app.add_systems(
            Update,
            viewport::create_canvas_click.run_if(in_state(SplashState::Editor)),
        );

        // The panel. `false` — it owns its own scrolling (the canvas area pans
        // and zooms rather than scrolls).
        //
        // Registered here rather than by the shell because the shell's static
        // panel table is for panels the shell itself knows how to draw; this one
        // belongs to the crate that draws it, which is also what lets an editor
        // built without this plugin simply not offer it.
        use renzora::core::RenzoraShellExt;
        use renzora_ember::panel::RegisterPanelContent;
        // "UI Editor", not "UI Canvas": the panel is the place you edit a UI.
        // The *canvas* is the entity in the scene that mounts a template, and
        // two things with the same name would be one thing in the user's head.
        // The id stays `ui_canvas` — it is in saved layouts.
        app.register_shell_panel("ui_canvas", "UI Editor", "browser", "Scene");
        app.register_panel_content("ui_canvas", false, |commands, fonts| {
            build_ui_canvas(commands, fonts)
        });
    }
}

/// Is the UI canvas panel on screen?
///
/// `renzora_viewport` asks, so it can run the offscreen UI render camera only
/// while something is showing its output. That used to be
/// `view == ViewportView::Ui`, which is exactly the coupling the panel exists to
/// undo.
pub fn canvas_panel_visible(world: &World) -> bool {
    renzora_ember::dock::panel_visible_anywhere(
        "ui_canvas",
        world.get_resource::<renzora_ember::dock::Dock>(),
        world.get_resource::<renzora_ember::dock::FixedDock>(),
        world.get_resource::<renzora_ember::dock::DockWindows>(),
    )
}

renzora::add!(GameUiEditorPlugin, Editor);

/// Build the full UI canvas editor content (toolbar + scene-backdrop + rendered
/// UI image + selection overlay). Mounted by the viewport in UI mode.
pub fn build_ui_canvas(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node { width: Val::Percent(100.0), flex_grow: 1.0, min_height: Val::Px(0.0), flex_direction: FlexDirection::Column, ..default() },
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
