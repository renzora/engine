//! Panel geometry: the tree/grid splitter, the two width breakpoints that
//! collapse the browser as it narrows, the chrome band colours, and the
//! grid-vs-list container retune.

use bevy::prelude::*;

use renzora_ember::font::EmberFonts;
use renzora_ember::reactive::Rx;
use renzora_ember::widgets::icon_label_button_collapsing;

use crate::state::{AssetGrid, AssetRoot, NativeAssets, Splitter, ViewToggleBtn};

/// Drag the tree/content divider to resize the tree pane. The drag persists via
/// `divider_drag` (captured on press) so it keeps tracking even when the cursor
/// moves off the thin splitter — mirrors the dock's divider.
pub(crate) fn splitter_drag(
    splitter: Query<&Interaction, With<Splitter>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    mut state: ResMut<NativeAssets>,
) {
    if mouse.just_released(MouseButton::Left) {
        state.divider_drag = None;
    }
    let Some(cx) = windows.iter().find_map(|w| w.cursor_position()).map(|p| p.x) else {
        return;
    };
    if state.divider_drag.is_none()
        && mouse.just_pressed(MouseButton::Left)
        && splitter.iter().any(|i| *i == Interaction::Pressed)
    {
        state.divider_drag = Some((cx, state.tree_width));
    }
    if let Some((start_x, start_w)) = state.divider_drag {
        state.tree_width = (start_w + (cx - start_x)).clamp(120.0, 420.0);
    }
}

/// Show the ew-resize cursor whenever the divider is hovered or being dragged.
pub(crate) fn divider_cursor(
    splitter: Query<&Interaction, With<Splitter>>,
    state: Res<NativeAssets>,
    windows: Query<Entity, With<bevy::window::PrimaryWindow>>,
    mut commands: Commands,
    mut forcing: Local<bool>,
) {
    let want = state.divider_drag.is_some()
        || splitter.iter().any(|i| matches!(i, Interaction::Hovered | Interaction::Pressed));
    let Ok(win) = windows.single() else {
        return;
    };
    if want {
        commands
            .entity(win)
            .insert(bevy::window::CursorIcon::System(bevy::window::SystemCursorIcon::EwResize));
        *forcing = true;
    } else if *forcing {
        *forcing = false;
        commands
            .entity(win)
            .insert(bevy::window::CursorIcon::System(bevy::window::SystemCursorIcon::Default));
    }
}

/// Two width breakpoints, both measured on the panel root:
/// - below `TREE_ONLY_WIDTH`, collapse to a tree-only file browser (no room for
///   a usable grid beside the tree — mirrors the egui browser's behaviour);
/// - below `COMPACT_WIDTH`, keep the grid but drop the toolbar's action labels.
///   The full row (Add + Import + New Folder + Sort + view + search + zoom) needs
///   roughly this much before it starts wrapping; under it, icon-only buttons
///   buy back ~150px and keep everything reachable on one line.
pub(crate) fn responsive_layout(
    root: Query<&bevy::ui::ComputedNode, With<AssetRoot>>,
    mut state: ResMut<NativeAssets>,
) {
    const TREE_ONLY_WIDTH: f32 = 310.0;
    const COMPACT_WIDTH: f32 = 820.0;
    let Ok(cn) = root.single() else {
        return;
    };
    let width = cn.size().x * cn.inverse_scale_factor();
    if width <= 0.0 {
        return;
    }
    let narrow = width < TREE_ONLY_WIDTH;
    if state.narrow != narrow {
        state.narrow = narrow;
    }
    let compact = width < COMPACT_WIDTH;
    if state.compact != compact {
        state.compact = compact;
    }
}

/// The band behind the action toolbar — a step lighter than the plain panel
/// header.
pub(crate) fn header_surface() -> Color {
    renzora_ember::theme::mix(
        renzora_ember::theme::header_bg(),
        renzora_ember::theme::section_bg(),
        0.25,
    )
}

/// The band behind the breadcrumb row — a further step lighter again, so the
/// path reads as its own strip rather than as more toolbar.
///
/// Shade alone wasn't enough: at one step off the header the two rows looked
/// like a single flat block, which is why the row also carries hairline rules
/// top and bottom. Both bands are mixed *toward* `section_bg` rather than
/// lightened by a fixed amount, so they land between two colours the theme
/// author picked and can't invert on a light palette.
pub(crate) fn crumb_surface() -> Color {
    renzora_ember::theme::mix(
        renzora_ember::theme::header_bg(),
        renzora_ember::theme::section_bg(),
        0.7,
    )
}

/// True once the panel is too tight for the toolbar's full action labels.
pub(crate) fn is_compact(w: &Rx) -> bool {
    w.get_resource::<NativeAssets>().is_some_and(|s| s.compact)
}

/// A toolbar action button that collapses to icon-only once the panel is too
/// tight for its label (see [`is_compact`]). Dropping the four action labels
/// frees ~150px, which is what keeps the breadcrumb readable and the whole row
/// on one line.
pub(crate) fn toolbar_action(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
) -> Entity {
    icon_label_button_collapsing(commands, fonts, icon, label, is_compact)
}

/// Toggle grid/list view.
pub(crate) fn view_toggle_click(
    q: Query<&Interaction, (With<ViewToggleBtn>, Changed<Interaction>)>,
    mut state: ResMut<NativeAssets>,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        state.list_view = !state.list_view;
    }
}

/// Retune the grid container's gaps/padding when the view mode changes (tight
/// rows for the list, roomy cells for the grid).
pub(crate) fn update_grid_layout(
    state: Res<NativeAssets>,
    mut last: Local<Option<bool>>,
    mut q: Query<&mut Node, With<AssetGrid>>,
) {
    if *last == Some(state.list_view) {
        return;
    }
    *last = Some(state.list_view);
    for mut n in &mut q {
        if state.list_view {
            n.row_gap = Val::Px(1.0);
            n.column_gap = Val::Px(0.0);
            n.padding = UiRect::axes(Val::Px(4.0), Val::Px(4.0));
        } else {
            n.row_gap = Val::Px(12.0);
            n.column_gap = Val::Px(10.0);
            n.padding = UiRect::all(Val::Px(10.0));
        }
    }
}
