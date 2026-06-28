#![allow(unused_variables, unused_assignments, dead_code)]

//! Renzora Game UI — bevy_ui game interface components.
//!
//! **Runtime** (this crate, always lean):
//! - `UiCanvas`, `UiWidget`, `UiWidgetType` — serializable marker components
//! - Widget data components (`ProgressBarData`, `SliderData`, etc.)
//! - Runtime systems that drive widget behavior
//! - `GameUiPlugin` — registers types for reflection + runtime systems
//!
//! All editor-only code (inspector registrations, the WYSIWYG canvas, the
//! render target, presets, view auto-switching, debug logging) lives in the
//! separate `renzora_game_ui_editor` crate, which depends on this one.

pub mod components;
pub mod script_extension;
pub mod shapes;
pub mod spawn;
pub mod systems;

use bevy::prelude::*;

pub use components::{
    HtmlTemplatePath, HuiBuildOnSelf, UiCanvas, UiTheme, UiThemed, UiWidget, UiWidgetType,
};

#[derive(Default)]
pub struct GameUiPlugin;

impl Plugin for GameUiPlugin {
    fn build(&self, app: &mut App) {
        // ── Reflection registration ─────────────────────────────────────
        app.register_type::<components::UiCanvas>();
        app.register_type::<components::UiWidget>();
        app.register_type::<components::HtmlTemplatePath>();
        app.register_type::<components::HuiBuildOnSelf>();
        app.register_type::<components::UiWidgetPart>();
        // Single-entity primitive (replaces ProgressBar / HealthBar / LoadingScreen)
        app.register_type::<components::UiBarFill>();
        app.register_type::<components::ProgressDirection>();
        // Form inputs
        app.register_type::<components::SliderData>();
        app.register_type::<components::CheckboxData>();
        app.register_type::<components::ToggleData>();
        app.register_type::<components::RadioButtonData>();
        app.register_type::<components::DropdownData>();
        app.register_type::<components::TextInputData>();
        app.register_type::<components::NumberInputData>();
        // Layout / overlay primitives
        app.register_type::<components::ScrollViewData>();
        app.register_type::<components::TooltipData>();
        app.register_type::<components::ModalData>();
        app.register_type::<components::DraggableWindowData>();
        app.register_type::<components::SeparatorData>();
        app.register_type::<components::SeparatorDirection>();
        app.register_type::<components::ScrollbarData>();
        app.register_type::<components::ScrollbarOrientation>();
        app.register_type::<components::UiImagePath>();
        // Settings UI rows (used by editor settings panel)
        app.register_type::<components::KeybindRowData>();
        app.register_type::<components::SettingsRowData>();
        app.register_type::<components::SettingsControlType>();
        // Widget style components
        app.register_type::<components::UiFill>();
        app.register_type::<components::UiStroke>();
        app.register_type::<components::UiBorderRadius>();
        app.register_type::<components::UiBoxShadow>();
        app.register_type::<components::UiOpacity>();
        app.register_type::<components::UiClipContent>();
        app.register_type::<components::UiCursor>();
        app.register_type::<components::UiTextStyle>();
        app.register_type::<components::UiPadding>();
        // Interaction & animation
        app.register_type::<components::UiInteractionStyle>();
        app.register_type::<components::UiTransition>();
        app.register_type::<components::UiTween>();
        // Theming
        app.register_type::<components::UiTheme>();
        app.register_type::<components::UiThemed>();

        // ── Default theme resource ────────────────────────────────────
        app.init_resource::<components::UiTheme>();

        // ── Script actions (decoupled — observes ScriptAction events) ──
        app.add_observer(script_extension::handle_ui_script_actions);

        // ── Auto-layout on reparent ────────────────────────────────────
        // When a UI widget is dragged to a new parent in the hierarchy,
        // re-apply parent-aware positioning: Container parent → Relative
        // (flex flow), Canvas parent → Absolute (free placement). The
        // Changed-filtered system covers runtime drag-reparents; the
        // Insert observer covers the scene-load case (reflection inserts
        // bypass change detection).
        app.add_systems(Update, on_widget_reparented);
        app.add_observer(on_childof_inserted);

        // Visibility-mode binding: same dual-path setup as the reparent
        // logic. The Changed system handles inspector edits to the
        // mode dropdown; the observer applies the saved mode on scene
        // load when reflection inserts skip change-tick propagation.
        app.add_observer(on_canvas_inserted);

        // ── Shape primitives ────────────────────────────────────────────
        app.add_plugins(shapes::ShapesPlugin);

        // ── Canvas scaler & visibility-mode ──────────────────────────────
        //
        // `update_ui_scale` adjusts the global `UiScale` to fit the 3D
        // viewport's render target. Useful in the shipped game (UI scales with
        // the window), but in the editor it would also scale the UI rendered to
        // our fixed 1280×720 editor render target — making a Node with
        // `width: Px(100)` show up as some other pixel count depending on the
        // editor window size. So we skip it in an editor session; UiScale stays
        // at the default 1.0 and the canvas tab renders 1:1 with what the user
        // authors.
        //
        // Runtime-gated on `EditorSession` (NOT `#[cfg]`): under the single
        // `--workspace` editor build the old `#[cfg(not(editor))]` compiled this
        // OUT of the shipped game too, so exported games never scaled their UI.
        let is_editor = app
            .world()
            .get_resource::<renzora::EditorSession>()
            .map(|s| s.0)
            .unwrap_or(false);
        if !is_editor {
            app.add_systems(Update, update_ui_scale);
        }
        app.add_systems(
            Update,
            (
                rehydrate_ui_images,
                sync_ui_zindex,
                apply_canvas_visibility_mode,
            ),
        );

        // ── Runtime widget systems ──────────────────────────────────────
        app.add_systems(
            Update,
            (
                systems::apply_bar_fill,
                systems::slider_system,
                systems::checkbox_system,
                systems::toggle_system,
                systems::radio_button_system,
                systems::tooltip_system,
                systems::dropdown_system,
                systems::dropdown_option_system,
                systems::modal_system,
                systems::draggable_window_system,
                systems::separator_system,
                systems::number_input_system,
                systems::scrollbar_system,
                systems::keybind_row_system,
                systems::settings_row_system,
                systems::interaction_style_system,
                systems::ui_theme_system,
                systems::ui_tween_system,
                systems::ensure_style_components,
                systems::apply_widget_style_system,
            ),
        );

        info!("[runtime] GameUiPlugin");
    }
}

// ── Canvas visibility_mode → Visibility ──────────────────────────────────
//
// `UiCanvas.visibility_mode` is the user-facing dropdown ("always",
// "play_only", "editor_only"). Until now it was a hint nothing read.
// This system writes the actual Bevy `Visibility` component from it
// whenever the canvas is freshly added or the dropdown changes.
//
// Runs in both editor and runtime — `PlayModeState` is optional, so in
// runtime builds (no PlayModeState resource) `in_play` defaults to true,
// making "play_only" canvases visible at runtime, "editor_only" hidden,
// and "always" always visible. Scripts can still override via
// `ui_show` / `ui_hide` afterward; the system only fires when the
// canvas component itself changes (`Changed<UiCanvas>`), not every frame.

fn apply_canvas_visibility_mode(
    play_mode: Option<Res<renzora::PlayModeState>>,
    mut canvases: Query<(&UiCanvas, &mut Visibility), Changed<UiCanvas>>,
) {
    let in_play = play_mode.is_none_or(|p| p.is_in_play_mode());
    for (canvas, mut vis) in &mut canvases {
        apply_canvas_visibility_to(in_play, canvas, &mut vis);
    }
}

fn apply_canvas_visibility_to(in_play: bool, canvas: &UiCanvas, vis: &mut Visibility) {
    let visible = match canvas.visibility_mode.as_str() {
        "always" => true,
        "play_only" => in_play,
        "editor_only" => !in_play,
        _ => true,
    };
    let target = if visible {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    if *vis != target {
        *vis = target;
    }
}

/// Lifecycle observer covering the scene-load case the `Changed`-filtered
/// system above misses. Reflection inserts (used by `DynamicScene::write_to_world`)
/// don't propagate Bevy's change ticks, so a saved canvas with
/// `visibility_mode: "play_only"` would render incorrectly in the editor
/// until the user touched it. The observer fires on insert and applies
/// the same logic.
fn on_canvas_inserted(
    trigger: On<Insert, UiCanvas>,
    play_mode: Option<Res<renzora::PlayModeState>>,
    mut canvases: Query<(&UiCanvas, &mut Visibility)>,
) {
    let entity = trigger.entity;
    let in_play = play_mode.is_none_or(|p| p.is_in_play_mode());
    if let Ok((canvas, mut vis)) = canvases.get_mut(entity) {
        apply_canvas_visibility_to(in_play, canvas, &mut vis);
    }
}

// ── Reparent system ────────────────────────────────────────────────────────
//
// Fires when a `ChildOf` is inserted *or* replaced on a UI widget entity
// (drag in hierarchy → Replace; spawn → Insert; both surface as
// `Changed<ChildOf>`). Re-runs the parent-aware layout logic so the moved
// widget switches between Absolute (canvas root) and Relative (Container)
// automatically.
//
// Originally written as an `On<Insert, ChildOf>` observer, which missed
// the drag-in-hierarchy case because that fires `Replace` not `Insert`.
// `Changed` filter catches both.

fn on_widget_reparented(
    mut commands: Commands,
    changed: Query<Entity, (With<UiWidget>, Changed<ChildOf>)>,
) {
    for entity in &changed {
        commands.queue(move |world: &mut World| {
            crate::game_ui::spawn::reapply_layout_from_parent(world, entity);
        });
    }
}

/// Lifecycle observer covering the scene-load case the `Changed`-filtered
/// system above misses. `DynamicScene::write_to_world` inserts `ChildOf`
/// via reflection without propagating change ticks, so widgets loaded
/// from a saved scene wouldn't have their parent-aware layout applied
/// (Container parent → Relative, Canvas root → Absolute) until the user
/// touched them.
fn on_childof_inserted(
    trigger: On<Insert, ChildOf>,
    mut commands: Commands,
    widgets: Query<(), With<UiWidget>>,
) {
    let entity = trigger.entity;
    if widgets.get(entity).is_err() {
        return;
    }
    commands.queue(move |world: &mut World| {
        crate::game_ui::spawn::reapply_layout_from_parent(world, entity);
    });
}

// ── Canvas scaler ───────────────────────────────────────────────────────────

/// Scales `Val::Px` values (text size, padding, border-radius) uniformly so
/// they stay proportional to the viewport.
fn update_ui_scale(
    canvases: Query<&UiCanvas>,
    render_target: Option<Res<renzora::ViewportRenderTarget>>,
    images: Res<Assets<Image>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut ui_scale: ResMut<bevy::ui::UiScale>,
) {
    let (ref_w, ref_h) = canvases
        .iter()
        .next()
        .map(|c| (c.reference_width, c.reference_height))
        .unwrap_or((1280.0, 720.0));

    if ref_w <= 0.0 || ref_h <= 0.0 {
        return;
    }

    let actual = render_target
        .as_ref()
        .and_then(|rt| rt.image.as_ref())
        .and_then(|h| images.get(h))
        .map(|img| {
            let s = img.size();
            (s.x as f32, s.y as f32)
        });

    let (actual_w, actual_h) = match actual {
        Some(size) => size,
        None => {
            if let Ok(window) = windows.single() {
                (window.width(), window.height())
            } else {
                return;
            }
        }
    };

    if actual_w <= 0.0 || actual_h <= 0.0 {
        return;
    }

    let scale = (actual_w / ref_w).min(actual_h / ref_h);
    ui_scale.0 = scale;
}

// ── Image rehydration ───────────────────────────────────────────────────────

/// Rehydrates `ImageNode` for UI image widgets after scene deserialization.
///
/// `ImageNode` contains a `Handle<Image>` which fails serialization and gets
/// stripped on save. `UiImagePath` stores the asset-relative path and survives.
/// This system re-loads the image and inserts `ImageNode` on any entity that
/// has `UiImagePath` but no `ImageNode`.
fn rehydrate_ui_images(
    mut commands: Commands,
    query: Query<
        (Entity, &components::UiImagePath),
        (Without<ImageNode>, Added<components::UiImagePath>),
    >,
    asset_server: Res<AssetServer>,
) {
    for (entity, img_path) in &query {
        let path = img_path.path.clone();
        let handle: Handle<Image> = asset_server.load(path);
        commands.entity(entity).try_insert(ImageNode::new(handle));
    }
}

// ── Z-index sync ────────────────────────────────────────────────────────────

/// Syncs `ZIndex` on UI canvas and widget entities so that items higher in the
/// hierarchy (top of the list) render on top — matching the layer order convention
/// used by most editors (Photoshop, Unity, etc.).
fn sync_ui_zindex(
    canvas_entities: Query<Entity, With<UiCanvas>>,
    canvas_data: Query<(&UiCanvas, Option<&GlobalZIndex>)>,
    widgets: Query<Entity, With<UiWidget>>,
    zindex_query: Query<Option<&ZIndex>>,
    children_query: Query<&Children>,
    child_of_query: Query<&ChildOf>,
    mut commands: Commands,
) {
    let mut processed_parents = std::collections::HashSet::new();

    for entity in canvas_entities.iter().chain(widgets.iter()) {
        let parent = match child_of_query.get(entity) {
            Ok(c) => c.parent(),
            Err(_) => continue,
        };

        if !processed_parents.insert(parent) {
            continue;
        }

        let Ok(children) = children_query.get(parent) else {
            continue;
        };

        // Count only UI entities among siblings for correct reverse indexing.
        let ui_count = children
            .iter()
            .filter(|c| canvas_entities.contains(*c) || widgets.contains(*c))
            .count() as i32;

        let mut ui_idx = 0i32;
        for child in children.iter() {
            if canvas_entities.contains(child) || widgets.contains(child) {
                // First child (top of hierarchy) gets highest ZIndex → renders on top.
                let desired = ZIndex(ui_count - 1 - ui_idx);
                let current = zindex_query.get(child).ok().flatten().copied();
                if current != Some(desired) {
                    commands.entity(child).try_insert(desired);
                }
                ui_idx += 1;
            }
        }
    }

    // Root-level canvases (no parent) use GlobalZIndex from sort_order.
    for entity in &canvas_entities {
        if child_of_query.contains(entity) {
            continue;
        }
        if let Ok((canvas, current_gz)) = canvas_data.get(entity) {
            let desired = GlobalZIndex(canvas.sort_order);
            if current_gz.copied() != Some(desired) {
                commands.entity(entity).try_insert(desired);
            }
        }
    }
}
renzora::add!(GameUiPlugin);
