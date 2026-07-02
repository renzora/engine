//! Bevy-native (ember) viewport header bar — the chrome above the 3D image.
//!
//! Mirrors `header.rs` (the egui version) but built from bevy_ui nodes so it
//! runs in the native editor. Increment A1 covers the left "session actions"
//! strip (undo / redo / save / play / scripts) and the maximize toggle; the
//! view/mode dropdowns, Display/Camera/Snap dropdowns, inline snapping, and the
//! registry-driven tool buttons land in later increments (see the
//! `native-viewport-migration` plan).

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use renzora::core::viewport_types::{
    CameraSettingsState, CollisionGizmoVisibility, ProjectionMode, SnapSettings, ViewAngleCommand,
    ViewportMode, ViewportSettings, ViewportView, VisualizationMode,
};
use renzora::core::ShapeRegistry;
use renzora_undo::{execute, SpawnShapeCmd, UndoContext};
use bevy::ecs::world::CommandQueue;
use std::sync::Arc;

use renzora_editor_framework::{EditorCommands, GizmoSpace, ToolSection, ToolbarRegistry};
use renzora_ember::font::{icon_glyph, icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::bind_2way;
use renzora_ember::widgets::{
    drag_value, drag_value_flat, scroll_area, toggle_switch, DragRange, OverlaySurface,
};
use renzora_ember::theme::{
    border, hover_bg, panel_bg, popup_bg, rgb, tab_active, text_muted, text_primary, value_text,
};
use renzora_ember::cursor_icon::HoverCursor;
use renzora_theme::ThemeManager;

/// Height of the viewport header bar (matches the egui `HEADER_HEIGHT`).
pub const HEADER_HEIGHT: f32 = 28.0;

const BTN_W: f32 = 26.0;
const BTN_H: f32 = 22.0;

/// One of the fixed "session action" buttons in the header's left strip.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum HeaderAction {
    Undo,
    Redo,
    Save,
    Maximize,
}

/// Points a button at its child glyph `Text` entity so the visuals system can
/// re-glyph / re-color it without walking children.
#[derive(Component)]
struct HeaderIcon(Entity);

fn col(c: renzora_theme::ThemeColor) -> Color {
    let [r, g, b, _a] = c.to_array();
    Color::srgb_u8(r, g, b)
}

/// Slugify an option label for the shared `opt.<slug>` translation namespace:
/// lowercase, each run of non-alphanumerics → one `_`, trimmed.
fn opt_slug(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut pending_us = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            if pending_us && !out.is_empty() {
                out.push('_');
            }
            pending_us = false;
            out.push(c.to_ascii_lowercase());
        } else {
            pending_us = true;
        }
    }
    out
}

/// Localize a dropdown OPTION label via the shared `opt.<slug>` namespace,
/// reusing a few `common.*` keys where the value already has one. The enum's
/// `label()` identity (used for index/state matching) is unchanged — only the
/// displayed string is translated.
fn loc_opt(s: &str) -> String {
    match s {
        "None" => renzora::lang::t_or("common.none", s),
        "Disabled" => renzora::lang::t_or("common.disabled", s),
        "Default" => renzora::lang::t_or("common.default", s),
        "Always" => renzora::lang::t_or("common.always", s),
        _ => renzora::lang::t_or(&format!("opt.{}", opt_slug(s)), s),
    }
}

/// Build the header row.
///
/// Historically this was child 0 of the primary viewport panel; it is now
/// lifted to the editor shell and rendered as a full-width strip directly
/// below the document-tab bar (see `renzora_shell::spawn_shell`). The driver
/// systems in [`register`] locate it by component, so it works regardless of
/// where it is parented.
pub fn build_header(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = commands
        .spawn((
            Node {
                // Content-sized: the shared toolbar host centers the strip, so
                // the header sizes to its widgets and the host does the centering.
                height: Val::Px(HEADER_HEIGHT),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(8.0)),
                column_gap: Val::Px(2.0),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            HeaderBg,
            Name::new("viewport-header"),
        ))
        .id();

    let undo = action_btn(commands, fonts, HeaderAction::Undo, "arrow-u-up-left");
    let redo = action_btn(commands, fonts, HeaderAction::Redo, "arrow-u-up-right");
    let gap1 = gap(commands, 6.0);
    let save = action_btn(commands, fonts, HeaderAction::Save, "floppy-disk");

    // Registry-driven tool buttons (Select/Translate/... + terrain + plugins).
    // Populated from ToolbarRegistry by a deferred system (predicates need World).
    let tools_gap = gap(commands, 8.0);
    let tools = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(1.0),
                height: Val::Percent(100.0),
                ..default()
            },
            ToolContainer,
            Name::new("vp-tools"),
        ))
        .id();

    // "Add shape" menu — a dropdown listing every registered shape (the same
    // ShapeRegistry the shape-library panel reads), grouped by category.
    let shapes_gap = gap(commands, 8.0);
    let shapes_dd = build_shapes_dropdown(commands, fonts);
    // The shape menu is 3D-only (it spawns 3D primitives). The tool CONTAINER
    // stays visible in 2D — its 3D transform tools (translate/rotate/scale) hide
    // themselves via a per-tool predicate, but 2D-relevant registry tools (e.g.
    // the tilemap Paint tool) must still show.
    for e in [shapes_gap, shapes_dd] {
        commands.entity(e).insert(ThreeDOnly);
    }

    // World/Local space toggle for the transform gizmo (3D only).
    let space_gap = gap(commands, 8.0);
    let space_btn = space_toggle(commands, fonts);
    commands.entity(space_btn).insert(ThreeDOnly);

    let gap3 = gap(commands, 8.0);
    let view_dd = dropdown(commands, fonts, DropKind::View, 56.0);
    let mode_dd = dropdown(commands, fonts, DropKind::Mode, 80.0);

    // 2D-only zoom readout (orthographic scale → percentage), updated by
    // `update_zoom_readout`. Hidden in 3D/UI via `TwoDOnly`.
    let zoom_readout = commands
        .spawn((
            Text::new("100%"),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
            Node {
                align_self: AlignSelf::Center,
                margin: UiRect::horizontal(Val::Px(4.0)),
                ..default()
            },
            TwoDOnly,
            ZoomReadout,
            Name::new("vp-2d-zoom"),
        ))
        .id();

    // 2D-only switches — the faint behind-sprites grid mesh
    // (`ViewportSettings.show_grid_2d`, off by default; separate from the
    // Display dropdown's 3D "Grid" toggle, which drives the 3D floor grid and
    // is hidden in 2D along with the rest of the dropdown) and the ruler bars
    // (`show_rulers_2d`, on by default).
    let grid_gap = gap(commands, 6.0);
    let grid_switch = switch_2d(
        commands,
        fonts,
        "viewport.grid",
        "vp-2d-grid-switch",
        |s| s.show_grid_2d,
        |s, v| s.show_grid_2d = v,
    );
    let ruler_gap = gap(commands, 6.0);
    let ruler_switch = switch_2d(
        commands,
        fonts,
        "viewport.rulers",
        "vp-2d-ruler-switch",
        |s| s.show_rulers_2d,
        |s, v| s.show_rulers_2d = v,
    );
    for e in [grid_gap, grid_switch, ruler_gap, ruler_switch] {
        commands.entity(e).insert(TwoDOnly);
    }

    // Inline snapping: translate doubles as the 2D grid snap; rotate / scale and
    // the camera-speed widget are 3D-only (hidden in 2D — `ThreeDOnly`).
    let gap5 = gap(commands, 6.0);
    let translate = snap_pair(
        commands,
        fonts,
        SnapToggle::Translate,
        "arrows-out-cardinal",
        0.01,
        100.0,
        0.02,
        |w| snap_val(w, |s| s.translate_snap),
        |w, v| set_snap(w, |s| &mut s.translate_snap, v),
    );
    let rotate = snap_pair(
        commands,
        fonts,
        SnapToggle::Rotate,
        "arrow-clockwise",
        1.0,
        90.0,
        0.2,
        |w| snap_val(w, |s| s.rotate_snap),
        |w, v| set_snap(w, |s| &mut s.rotate_snap, v),
    );
    let scale = snap_pair(
        commands,
        fonts,
        SnapToggle::Scale,
        "arrows-out",
        0.01,
        10.0,
        0.01,
        |w| snap_val(w, |s| s.scale_snap),
        |w, v| set_snap(w, |s| &mut s.scale_snap, v),
    );
    commands.entity(rotate).insert(ThreeDOnly);
    commands.entity(scale).insert(ThreeDOnly);
    let gap6 = gap(commands, 6.0);
    let cam_speed = cam_speed_widget(commands, fonts);
    commands.entity(cam_speed).insert(ThreeDOnly);

    // Fixed gap (not a flex spacer) so the whole toolbar reads as one centered
    // cluster rather than splitting to the left/right edges.
    let center_gap = gap(commands, 12.0);
    let display_dd = build_display_dropdown(commands, fonts);
    let snap_dd = build_snap_dropdown(commands, fonts);
    let camera_dd = build_camera_dropdown(commands, fonts);
    // The Display / Snap / Camera dropdowns are all 3D controls — hide in 2D.
    for e in [display_dd, snap_dd, camera_dd] {
        commands.entity(e).insert(ThreeDOnly);
    }
    let gap4 = gap(commands, 3.0);
    let maximize = action_btn(commands, fonts, HeaderAction::Maximize, "arrows-out");

    // One centered cluster: session actions (+ tools, shapes), then view/mode,
    // inline snapping, camera speed, the Display / Snap / Camera dropdowns, and
    // maximize — all grouped in the middle via the row's `justify-content`.
    commands.entity(row).add_children(&[
        undo, redo, gap1, save, tools_gap, tools, shapes_gap, shapes_dd, space_gap, space_btn,
        center_gap, view_dd, mode_dd, zoom_readout, grid_gap, grid_switch, ruler_gap, ruler_switch,
        gap5, translate, rotate, scale, gap6, cam_speed, gap3, display_dd, snap_dd, camera_dd,
        gap4, maximize,
    ]);
    row
}

/// 2D-only label + ember switch, two-way bound to a `ViewportSettings` bool
/// (the grid / ruler toggles). Lives inline in the toolbar (not a dropdown)
/// so flipping chrome while tile-aligning is one click.
fn switch_2d(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label_key: &str,
    name: &'static str,
    get: impl Fn(&ViewportSettings) -> bool + Copy + Send + Sync + 'static,
    set: impl Fn(&mut ViewportSettings, bool) + Send + Sync + 'static,
) -> Entity {
    let sw = toggle_switch(commands, false);
    bind_2way(
        commands,
        sw,
        move |w| {
            w.get_resource::<ViewportSettings>()
                .map(get)
                .unwrap_or(false)
        },
        move |w, v: &bool| {
            if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                set(&mut s, *v);
            }
        },
    );
    let lbl = commands
        .spawn((
            Text::new(renzora::lang::t(label_key)),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let row = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                height: Val::Percent(100.0),
                margin: UiRect::horizontal(Val::Px(2.0)),
                ..default()
            },
            Name::new(name),
        ))
        .id();
    commands.entity(row).add_children(&[lbl, sw]);
    row
}

// ── World / Local gizmo-space toggle ─────────────────────────────────────────

#[derive(Component)]
struct SpaceToggleBtn;

#[derive(Component)]
struct SpaceToggleText;

/// A pill that flips the transform gizmo between World and Local space.
fn space_toggle(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let label = commands
        .spawn((
            Text::new(renzora::lang::t("viewport.gizmo.world")),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
            SpaceToggleText,
            bevy::picking::Pickable::IGNORE,
        ))
        .id();
    let btn = commands
        .spawn((
            Node {
                height: Val::Px(BTN_H),
                padding: UiRect::horizontal(Val::Px(8.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(tab_active())),
            Interaction::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            SpaceToggleBtn,
            Name::new("vp-space-toggle"),
        ))
        .id();
    commands.entity(btn).add_child(label);
    btn
}

/// Click flips the space; cycles World ↔ Local.
fn space_toggle_click(
    q: Query<&Interaction, (Changed<Interaction>, With<SpaceToggleBtn>)>,
    mut space: ResMut<GizmoSpace>,
) {
    for i in &q {
        if *i == Interaction::Pressed {
            *space = match *space {
                GizmoSpace::World => GizmoSpace::Local,
                GizmoSpace::Local => GizmoSpace::World,
            };
        }
    }
}

/// Keep the pill's label in sync with the active space.
fn update_space_toggle(space: Res<GizmoSpace>, mut texts: Query<&mut Text, With<SpaceToggleText>>) {
    if !space.is_changed() {
        return;
    }
    let label = match *space {
        GizmoSpace::World => renzora::lang::t("viewport.gizmo.world"),
        GizmoSpace::Local => renzora::lang::t("viewport.gizmo.local"),
    };
    for mut t in &mut texts {
        t.0 = label.clone();
    }
}

fn gap(commands: &mut Commands, w: f32) -> Entity {
    commands
        .spawn((Node { width: Val::Px(w), ..default() }, Name::new("gap")))
        .id()
}

fn action_btn(
    commands: &mut Commands,
    fonts: &EmberFonts,
    action: HeaderAction,
    icon: &str,
) -> Entity {
    let glyph = icon_text(commands, &fonts.phosphor, icon, text_primary(), 14.0);
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(BTN_W),
                height: Val::Px(BTN_H),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            action,
            HeaderIcon(glyph),
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("vp-hdr-action"),
        ))
        .id();
    commands.entity(btn).add_child(glyph);
    btn
}

pub(crate) fn register(app: &mut App) {
    use renzora_editor_framework::SplashState;
    use renzora_ember::toolbar::PanelToolbarExt;

    // The viewport header is the viewport's toolbar: it sits in the shared strip
    // below the document tabs (centered by the host) and shows while ANY of the
    // 4 viewport slots is the active dock tab. It still owns the 3D/2D/UI
    // selector. Built once by the shell's toolbar host.
    app.register_panel_toolbar_multi(
        &["viewport", "viewport-2", "viewport-3", "viewport-4"],
        |commands, fonts| build_header(commands, fonts),
    );

    app.add_systems(
        Update,
        (
            update_header_visuals,
            header_action_click,
            dropdown_toggle,
            dropdown_option_click,
            dropdown_dismiss,
            viewport_maximize_dock,
            update_dropdown_visuals,
            panel_toggle,
            display_option_click,
            update_display_visuals,
            snap_toggle_click,
            update_snap_toggles,
            update_three_d_only,
            update_header_chrome,
            header_click,
            update_click_rows,
            update_panel_buttons,
            update_camera_snap_triggers,
            tool_button_click,
            // Nested tuple: keeps the top-level system count within Bevy's
            // 20-element tuple limit for `add_systems`.
            (
                shape_spawn_click,
                update_shape_menu,
                space_toggle_click,
                update_space_toggle,
                panel_toggle_dismiss,
                update_two_d_only,
                update_zoom_readout,
            ),
        )
            .run_if(in_state(SplashState::Editor)),
    );
    // Exclusive (need `&World` for the registry predicates / shape registry).
    app.add_systems(
        Update,
        (populate_tools, update_tool_buttons, populate_shapes)
            .run_if(in_state(SplashState::Editor)),
    );
}

// ── View / Mode dropdowns (A2) ───────────────────────────────────────────────

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum DropKind {
    View,
    Mode,
}

impl DropKind {
    /// Localized option labels for this dropdown, in `ALL` order (the enum's
    /// `label()` identity stays English for index matching; only the displayed
    /// string is translated via the shared `opt.<slug>` namespace).
    fn labels(self) -> Vec<String> {
        match self {
            DropKind::View => ViewportView::ALL.iter().map(|v| loc_opt(v.label())).collect(),
            DropKind::Mode => ViewportMode::ALL.iter().map(|m| loc_opt(m.label())).collect(),
        }
    }
}

/// The dropdown trigger button; owns its popup panel + label text entity.
#[derive(Component)]
struct DropTrigger {
    kind: DropKind,
    panel: Entity,
    label: Entity,
    open: bool,
}

/// A selectable row inside a dropdown popup (`index` into `kind.labels()`).
#[derive(Component)]
struct DropOption {
    kind: DropKind,
    index: usize,
}

fn dropdown(commands: &mut Commands, fonts: &EmberFonts, kind: DropKind, width: f32) -> Entity {
    let labels = kind.labels();

    // Popup panel (hidden until the trigger is clicked).
    let mut rows = Vec::with_capacity(labels.len());
    for (i, label) in labels.iter().enumerate() {
        let txt = commands
            .spawn((
                Text::new(label.clone()),
                ui_font(&fonts.ui, 12.0),
                TextColor(rgb(text_primary())),
            ))
            .id();
        let row = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(BTN_H),
                    align_items: AlignItems::Center,
                    padding: UiRect::left(Val::Px(8.0)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Interaction::default(),
                DropOption { kind, index: i },
                HoverCursor(SystemCursorIcon::Pointer),
                Name::new("vp-drop-option"),
            ))
            .id();
        commands.entity(row).add_child(txt);
        rows.push(row);
    }
    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(100.0),
                left: Val::Px(0.0),
                margin: UiRect::top(Val::Px(4.0)),
                min_width: Val::Px(120.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                padding: UiRect::all(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            GlobalZIndex(600),
            bevy::ui::RelativeCursorPosition::default(),
            Name::new("vp-drop-panel"),
        ))
        .id();
    commands.entity(panel).add_children(&rows);

    // Trigger button: current label + caret.
    let label_e = commands
        .spawn((
            Text::new(labels.first().cloned().unwrap_or_default()),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let caret = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 10.0);
    let trigger = commands
        .spawn((
            Node {
                width: Val::Px(width),
                height: Val::Px(BTN_H),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::horizontal(Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(tab_active())),
            Interaction::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            DropTrigger {
                kind,
                panel,
                label: label_e,
                open: false,
            },
            Name::new("vp-dropdown"),
        ))
        .id();
    commands.entity(trigger).add_children(&[label_e, caret]);

    let wrap = commands
        .spawn((
            Node {
                position_type: PositionType::Relative,
                ..default()
            },
            Name::new("vp-dropdown-wrap"),
        ))
        .id();
    commands.entity(wrap).add_children(&[trigger, panel]);
    wrap
}

fn dropdown_toggle(
    mut triggers: Query<(&Interaction, &mut DropTrigger), Changed<Interaction>>,
    mut nodes: Query<&mut Node>,
) {
    for (interaction, mut t) in &mut triggers {
        if *interaction != Interaction::Pressed {
            continue;
        }
        t.open = !t.open;
        if let Ok(mut n) = nodes.get_mut(t.panel) {
            n.display = if t.open { Display::Flex } else { Display::None };
        }
    }
}

fn dropdown_option_click(
    options: Query<(&Interaction, &DropOption), Changed<Interaction>>,
    mut triggers: Query<&mut DropTrigger>,
    mut nodes: Query<&mut Node>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, opt) in &options {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let idx = opt.index;
        match opt.kind {
            DropKind::View => cmds.push(move |w: &mut World| {
                if let (Some(mut s), Some(v)) = (
                    w.get_resource_mut::<ViewportSettings>(),
                    ViewportView::ALL.get(idx).copied(),
                ) {
                    s.viewport_view = v;
                }
            }),
            DropKind::Mode => cmds.push(move |w: &mut World| {
                if let (Some(mut s), Some(m)) = (
                    w.get_resource_mut::<ViewportSettings>(),
                    ViewportMode::ALL.get(idx).copied(),
                ) {
                    s.viewport_mode = m;
                }
            }),
        }
        // Close the dropdown this option belongs to.
        for mut t in &mut triggers {
            if t.kind == opt.kind {
                t.open = false;
                if let Ok(mut n) = nodes.get_mut(t.panel) {
                    n.display = Display::None;
                }
            }
        }
    }
}

/// Honor the viewport "maximize" toggle on the bevy_ui shell's ember dock: swap
/// the dock to a viewport-only leaf while maximized and restore the saved tree
/// when un-maximized (the egui dock handles this itself in renzora_editor_framework).
fn viewport_maximize_dock(
    max: Option<Res<renzora_ui::ViewportMaximized>>,
    dock: Option<ResMut<renzora_ember::dock::Dock>>,
    dirty: Option<ResMut<renzora_ember::dock::DockDirty>>,
    mut saved: Local<Option<renzora_ember::dock::DockTree>>,
    mut last: Local<bool>,
) {
    let (Some(mut dock), Some(mut dirty)) = (dock, dirty) else {
        return;
    };
    let maximized = max.is_some_and(|m| m.0);
    if maximized == *last {
        return;
    }
    *last = maximized;
    if maximized {
        *saved = Some(dock.tree.clone());
        dock.tree = renzora_ember::dock::DockTree::leaf("viewport");
    } else if let Some(tree) = saved.take() {
        dock.tree = tree;
    }
    dirty.0 = true;
}

/// Close any open dropdown when the pointer presses outside its trigger + panel.
fn dropdown_dismiss(
    mouse: Res<ButtonInput<MouseButton>>,
    mut triggers: Query<(&mut DropTrigger, &Interaction)>,
    panels: Query<&bevy::ui::RelativeCursorPosition>,
    mut nodes: Query<&mut Node>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    for (mut t, trig_inter) in &mut triggers {
        if !t.open {
            continue;
        }
        let over_trigger = !matches!(trig_inter, Interaction::None);
        let over_panel = panels.get(t.panel).is_ok_and(|r| r.cursor_over);
        if !over_trigger && !over_panel {
            t.open = false;
            if let Ok(mut n) = nodes.get_mut(t.panel) {
                n.display = Display::None;
            }
        }
    }
}

fn update_dropdown_visuals(
    settings: Option<Res<ViewportSettings>>,
    theme: Option<Res<ThemeManager>>,
    triggers: Query<(&DropTrigger, &Interaction, &mut BackgroundColor)>,
    options: Query<
        (&DropOption, &Interaction, &mut BackgroundColor),
        Without<DropTrigger>,
    >,
    mut texts: Query<&mut Text>,
) {
    let (Some(settings), Some(theme)) = (settings, theme) else {
        return;
    };
    let t = &theme.active_theme;
    let accent = col(t.semantic.accent);
    let inactive = col(t.widgets.inactive_bg);
    let hovered = col(t.widgets.hovered_bg);

    let current_view = ViewportView::ALL
        .iter()
        .position(|v| *v == settings.viewport_view);
    let current_mode = ViewportMode::ALL
        .iter()
        .position(|m| *m == settings.viewport_mode);
    let current = |k: DropKind| match k {
        DropKind::View => current_view,
        DropKind::Mode => current_mode,
    };

    for (trig, interaction, mut bg) in triggers {
        // Trigger label tracks the live setting.
        if let Ok(mut text) = texts.get_mut(trig.label) {
            let label = match trig.kind {
                DropKind::View => loc_opt(settings.viewport_view.label()),
                DropKind::Mode => loc_opt(settings.viewport_mode.label()),
            };
            if text.0 != label {
                text.0 = label;
            }
        }
        let want = if trig.open || *interaction == Interaction::Hovered {
            hovered
        } else {
            inactive
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }

    for (opt, interaction, mut bg) in options {
        let is_current = current(opt.kind) == Some(opt.index);
        let want = if is_current {
            accent
        } else if *interaction == Interaction::Hovered {
            hovered
        } else {
            Color::NONE
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

/// Resolved header palette + the booleans that drive each button's glyph,
/// color, and hover/active background.
struct HeaderModel {
    can_undo: bool,
    can_redo: bool,
    can_save: bool,
    maximized: bool,
    primary: Color,
    muted: Color,
    accent: Color,
    hovered_bg: Color,
}

fn update_header_visuals(
    actions: Query<(&HeaderAction, &HeaderIcon, &Interaction, &mut BackgroundColor)>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
    undo: Option<Res<renzora_undo::UndoStacks>>,
    tabs: Option<Res<renzora_ui::DocumentTabState>>,
    maximized: Option<Res<renzora_ui::ViewportMaximized>>,
    theme: Option<Res<ThemeManager>>,
) {
    let Some(theme) = theme else { return };
    let t = &theme.active_theme;

    let (can_undo, can_redo) = undo
        .map(|s| (s.can_undo(&s.active), s.can_redo(&s.active)))
        .unwrap_or((false, false));
    let can_save = tabs
        .and_then(|tabs| tabs.tabs.get(tabs.active_tab).map(|t| t.is_modified))
        .unwrap_or(false);

    let model = HeaderModel {
        can_undo,
        can_redo,
        can_save,
        maximized: maximized.is_some_and(|m| m.0),
        primary: col(t.text.primary),
        muted: col(t.text.muted),
        accent: col(t.semantic.accent),
        hovered_bg: col(t.widgets.hovered_bg),
    };

    for (action, icon, interaction, mut bg) in actions {
        let (glyph_name, color, enabled) = action_appearance(action, &model);

        if let Ok((mut text, mut tc)) = texts.get_mut(icon.0) {
            if let Some(ch) = icon_glyph(glyph_name) {
                let s = ch.to_string();
                if text.0 != s {
                    text.0 = s;
                }
            }
            if tc.0 != color {
                tc.0 = color;
            }
        }

        // Background: maximize shows the accent while active; the rest just
        // light up on hover. Disabled buttons never show a hover fill.
        let want = if *action == HeaderAction::Maximize && model.maximized {
            model.accent
        } else if enabled && *interaction == Interaction::Hovered {
            model.hovered_bg
        } else {
            Color::NONE
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

/// Glyph name, color, and whether the button is clickable for the given action.
fn action_appearance(action: &HeaderAction, m: &HeaderModel) -> (&'static str, Color, bool) {
    match action {
        HeaderAction::Undo => (
            "arrow-u-up-left",
            if m.can_undo { m.primary } else { m.muted },
            m.can_undo,
        ),
        HeaderAction::Redo => (
            "arrow-u-up-right",
            if m.can_redo { m.primary } else { m.muted },
            m.can_redo,
        ),
        HeaderAction::Save => (
            "floppy-disk",
            if m.can_save { m.primary } else { m.muted },
            m.can_save,
        ),
        HeaderAction::Maximize => (
            if m.maximized { "arrows-in" } else { "arrows-out" },
            if m.maximized { m.primary } else { m.muted },
            true,
        ),
    }
}

fn header_action_click(
    q: Query<(&Interaction, &HeaderAction), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, action) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            HeaderAction::Undo => cmds.push(|w: &mut World| renzora_undo::undo_once(w)),
            HeaderAction::Redo => cmds.push(|w: &mut World| renzora_undo::redo_once(w)),
            HeaderAction::Save => cmds.push(|w: &mut World| {
                w.insert_resource(renzora::core::SaveSceneRequested);
            }),
            HeaderAction::Maximize => cmds.push(|w: &mut World| {
                let mut m = w.get_resource_or_insert_with(renzora_ui::ViewportMaximized::default);
                m.0 = !m.0;
            }),
        }
    }
}

// ── Display dropdown (A3): visualization + render flags + overlays + collision ─

/// Generic "click the trigger to show/hide its popup panel" for icon dropdowns.
#[derive(Component)]
struct PanelToggle {
    panel: Entity,
    open: bool,
}

/// Marks the Display dropdown's icon trigger (for hover / open background).
#[derive(Component)]
struct DisplayTrigger;

/// A click-to-select option inside the Display popup.
#[derive(Component, Clone, Copy)]
enum DisplayOption {
    /// Visualization mode by index into `VisualizationMode::ALL`.
    Viz(usize),
    /// Collision gizmo visibility — `true` = Selected Only, `false` = Always.
    Collision(bool),
}

/// Builds a [`check_row`] bound to `ViewportSettings.<field-path>`. Defined
/// before its first use (macro_rules is textually scoped).
macro_rules! toggle_row {
    ($c:expr, $f:expr, $label:expr, $($field:tt)+) => {
        check_row(
            $c,
            $f,
            $label,
            |w: &World| {
                w.get_resource::<ViewportSettings>()
                    .map(|s| s.$($field)+)
                    .unwrap_or(false)
            },
            |w: &mut World, v: bool| {
                if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                    s.$($field)+ = v;
                }
            },
        )
    };
}

fn panel_toggle(
    mut q: Query<(&Interaction, &mut PanelToggle), Changed<Interaction>>,
    mut nodes: Query<&mut Node>,
) {
    for (interaction, mut t) in &mut q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        t.open = !t.open;
        if let Ok(mut n) = nodes.get_mut(t.panel) {
            n.display = if t.open { Display::Flex } else { Display::None };
        }
    }
}

/// Close any open icon dropdown (the Display / Snap / Camera / Shapes
/// `PanelToggle` popups) when the pointer presses outside both its trigger and
/// its panel. The hand-rolled `PanelToggle` (like ember's `popover`) only
/// toggled on a trigger press, so these popups never closed on an outside click;
/// this mirrors `dropdown_dismiss` for the View/Mode dropdowns. A press inside
/// the panel (e.g. flipping a render-flag switch) leaves it open.
fn panel_toggle_dismiss(
    mouse: Res<ButtonInput<MouseButton>>,
    mut triggers: Query<(&Interaction, &mut PanelToggle)>,
    panels: Query<&bevy::ui::RelativeCursorPosition>,
    mut nodes: Query<&mut Node>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    for (trig_inter, mut t) in &mut triggers {
        if !t.open {
            continue;
        }
        let over_trigger = !matches!(trig_inter, Interaction::None);
        let over_panel = panels.get(t.panel).is_ok_and(|r| r.cursor_over);
        if !over_trigger && !over_panel {
            t.open = false;
            if let Ok(mut n) = nodes.get_mut(t.panel) {
                n.display = Display::None;
            }
        }
    }
}

fn build_display_dropdown(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let mut kids: Vec<Entity> = Vec::new();

    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.display.visualization")));
    for (i, m) in VisualizationMode::ALL.iter().enumerate() {
        kids.push(option_row(commands, fonts, DisplayOption::Viz(i), &loc_opt(m.label())));
    }

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.display.render")));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.display.mesh"), render_toggles.mesh));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.display.textures"), render_toggles.textures));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.shading.wireframe"), render_toggles.wireframe));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.display.lighting"), render_toggles.lighting));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.display.shadows"), render_toggles.shadows));

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.display.overlays")));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.grid"), show_grid));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.display.subgrid"), show_subgrid));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.display.axis_gizmo"), show_axis_gizmo));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.display.scene_icons"), show_scene_icons));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.display.labels"), show_labels));

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.display.collision_gizmos")));
    kids.push(option_row(
        commands,
        fonts,
        DisplayOption::Collision(true),
        &renzora::lang::t("viewport.display.selected_only"),
    ));
    kids.push(option_row(
        commands,
        fonts,
        DisplayOption::Collision(false),
        &renzora::lang::t("common.always"),
    ));

    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(100.0),
                right: Val::Px(0.0),
                margin: UiRect::top(Val::Px(4.0)),
                min_width: Val::Px(200.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            GlobalZIndex(600),
            // Lets `panel_toggle_dismiss` tell an inside-panel click (toggling a
            // render flag) from an outside click that should close the dropdown.
            bevy::ui::RelativeCursorPosition::default(),
            Name::new("vp-display-panel"),
        ))
        .id();
    commands.entity(panel).add_children(&kids);

    let eye = icon_text(commands, &fonts.phosphor, "eye", text_muted(), 15.0);
    let caret = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 8.0);
    let trigger = commands
        .spawn((
            Node {
                width: Val::Px(40.0),
                height: Val::Px(BTN_H),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::horizontal(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(tab_active())),
            Interaction::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            PanelToggle { panel, open: false },
            DisplayTrigger,
            Name::new("vp-display-dropdown"),
        ))
        .id();
    commands.entity(trigger).add_children(&[eye, caret]);

    let wrap = commands
        .spawn((
            Node {
                position_type: PositionType::Relative,
                ..default()
            },
            Name::new("vp-display-wrap"),
        ))
        .id();
    commands.entity(wrap).add_children(&[trigger, panel]);
    wrap
}

fn section_label(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
            Name::new("vp-section-label"),
        ))
        .id()
}

fn separator_row(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(1.0),
                margin: UiRect::vertical(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(rgb(border())),
            Name::new("vp-separator"),
        ))
        .id()
}

/// A label + click-to-select row (for the visualization / collision pickers).
fn option_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    opt: DisplayOption,
    label: &str,
) -> Entity {
    let txt = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(BTN_H),
                align_items: AlignItems::Center,
                padding: UiRect::left(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            opt,
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("vp-display-option"),
        ))
        .id();
    commands.entity(row).add_child(txt);
    row
}

/// A label + two-way switch row, bound to a `ViewportSettings` field.
fn check_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    get: impl Fn(&World) -> bool + Send + Sync + 'static,
    set: impl Fn(&mut World, bool) + Send + Sync + 'static,
) -> Entity {
    let cb = toggle_switch(commands, false);
    bind_2way(commands, cb, get, move |w, v: &bool| set(w, *v));
    let lbl = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(value_text())),
        ))
        .id();
    let spacer = commands
        .spawn(Node {
            flex_grow: 1.0,
            ..default()
        })
        .id();
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
            Name::new("vp-check-row"),
        ))
        .id();
    commands.entity(row).add_children(&[lbl, spacer, cb]);
    row
}

fn display_option_click(
    options: Query<(&Interaction, &DisplayOption), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, opt) in &options {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *opt {
            DisplayOption::Viz(i) => cmds.push(move |w: &mut World| {
                if let (Some(mut s), Some(m)) = (
                    w.get_resource_mut::<ViewportSettings>(),
                    VisualizationMode::ALL.get(i).copied(),
                ) {
                    s.visualization_mode = m;
                }
            }),
            DisplayOption::Collision(selected_only) => cmds.push(move |w: &mut World| {
                if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                    s.collision_gizmo_visibility = if selected_only {
                        CollisionGizmoVisibility::SelectedOnly
                    } else {
                        CollisionGizmoVisibility::Always
                    };
                }
            }),
        }
    }
}

fn update_display_visuals(
    settings: Option<Res<ViewportSettings>>,
    theme: Option<Res<ThemeManager>>,
    triggers: Query<(&Interaction, &PanelToggle, &mut BackgroundColor), With<DisplayTrigger>>,
    options: Query<(&DisplayOption, &Interaction, &mut BackgroundColor), Without<DisplayTrigger>>,
) {
    let (Some(settings), Some(theme)) = (settings, theme) else {
        return;
    };
    let t = &theme.active_theme;
    let accent = col(t.semantic.accent);
    let inactive = col(t.widgets.inactive_bg);
    let hovered = col(t.widgets.hovered_bg);

    for (interaction, toggle, mut bg) in triggers {
        let want = if toggle.open || *interaction == Interaction::Hovered {
            hovered
        } else {
            inactive
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }

    let viz_idx = VisualizationMode::ALL
        .iter()
        .position(|m| *m == settings.visualization_mode);
    let collision_selected =
        settings.collision_gizmo_visibility == CollisionGizmoVisibility::SelectedOnly;

    for (opt, interaction, mut bg) in options {
        let is_current = match *opt {
            DisplayOption::Viz(i) => viz_idx == Some(i),
            DisplayOption::Collision(sel) => sel == collision_selected,
        };
        let want = if is_current {
            accent
        } else if *interaction == Interaction::Hovered {
            hovered
        } else {
            Color::NONE
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

// ── Inline snapping + camera speed (A5) ──────────────────────────────────────

/// Which snap the icon toggle in a snap-pair enables/disables.
#[derive(Component, Clone, Copy)]
enum SnapToggle {
    Translate,
    Rotate,
    Scale,
}

/// The snap-pair pill, tagged with which snap it represents so its *whole*
/// background fills accent when that snap is enabled.
#[derive(Component, Clone, Copy)]
struct SnapPillOf(SnapToggle);

/// Tags header widgets hidden in 2D view (rotate/scale snap, camera speed).
#[derive(Component)]
struct ThreeDOnly;

/// Tags header widgets shown ONLY in 2D view (the zoom readout).
#[derive(Component)]
struct TwoDOnly;

/// Marker on the 2D zoom-percentage text.
#[derive(Component)]
struct ZoomReadout;

/// The header bar background (driven from `theme.surfaces.panel`).
#[derive(Component)]
struct HeaderBg;

/// A pill background behind a header widget (driven from `theme.widgets.inactive_bg`).
#[derive(Component)]
struct WidgetBg;

/// Keep the header bar + widget pills matched to the active egui theme so the
/// native toolbar reads identically to the egui one.
fn update_header_chrome(
    theme: Option<Res<ThemeManager>>,
    mut panels: Query<&mut BackgroundColor, (With<HeaderBg>, Without<WidgetBg>)>,
    mut widgets: Query<&mut BackgroundColor, (With<WidgetBg>, Without<HeaderBg>)>,
) {
    let Some(theme) = theme else { return };
    let t = &theme.active_theme;
    let panel = col(t.surfaces.panel);
    let widget = col(t.widgets.inactive_bg);
    for mut bg in &mut panels {
        if bg.0 != panel {
            bg.0 = panel;
        }
    }
    for mut bg in &mut widgets {
        if bg.0 != widget {
            bg.0 = widget;
        }
    }
}

fn snap_val(w: &World, f: impl Fn(&SnapSettings) -> f32) -> f32 {
    w.get_resource::<ViewportSettings>()
        .map(|s| f(&s.snap))
        .unwrap_or(0.0)
}

fn set_snap(w: &mut World, f: impl Fn(&mut SnapSettings) -> &mut f32, v: f32) {
    if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
        *f(&mut s.snap) = v;
    }
}

/// An icon toggle (enable/disable) + a scrubbable snap amount, in a pill.
#[allow(clippy::too_many_arguments)]
fn snap_pair(
    commands: &mut Commands,
    fonts: &EmberFonts,
    which: SnapToggle,
    icon: &str,
    min: f32,
    max: f32,
    step: f32,
    get: impl Fn(&World) -> f32 + Send + Sync + 'static,
    set: impl Fn(&mut World, f32) + Send + Sync + 'static,
) -> Entity {
    let glyph = icon_text(commands, &fonts.phosphor, icon, value_text(), 13.0);
    let toggle = commands
        .spawn((
            Node {
                width: Val::Px(22.0),
                height: Val::Px(BTN_H),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            which,
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("vp-snap-toggle"),
        ))
        .id();
    commands.entity(toggle).add_child(glyph);

    let dv = drag_value_flat(commands, &fonts.ui, "", value_text(), min, step);
    commands.entity(dv).insert(DragRange { min, max });
    bind_2way(commands, dv, get, move |w, v: &f32| set(w, *v));

    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(2.0),
                padding: UiRect::horizontal(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(hover_bg())),
            SnapPillOf(which),
            Name::new("vp-snap-pair"),
        ))
        .id();
    commands.entity(row).add_children(&[toggle, dv]);
    row
}

/// A camera icon + scrubbable move-speed (3D fly-cam).
fn cam_speed_widget(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let glyph = icon_text(commands, &fonts.phosphor, "video-camera", text_primary(), 13.0);
    let iconbox = commands
        .spawn((
            Node {
                width: Val::Px(20.0),
                height: Val::Px(BTN_H),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            Name::new("vp-cam-icon"),
        ))
        .id();
    commands.entity(iconbox).add_child(glyph);

    let dv = drag_value_flat(commands, &fonts.ui, "", value_text(), 1.0, 0.5);
    commands.entity(dv).insert(DragRange {
        min: 0.1,
        max: 100.0,
    });
    bind_2way(
        commands,
        dv,
        |w| {
            w.get_resource::<ViewportSettings>()
                .map(|s| s.camera.move_speed)
                .unwrap_or(1.0)
        },
        |w, v: &f32| {
            if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                s.camera.move_speed = *v;
            }
        },
    );

    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(2.0),
                padding: UiRect::horizontal(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(hover_bg())),
            WidgetBg,
            Name::new("vp-cam-speed"),
        ))
        .id();
    commands.entity(row).add_children(&[iconbox, dv]);
    row
}

fn snap_toggle_click(
    q: Query<(&Interaction, &SnapToggle), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, which) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let which = *which;
        cmds.push(move |w: &mut World| {
            if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                let flag = match which {
                    SnapToggle::Translate => &mut s.snap.translate_enabled,
                    SnapToggle::Rotate => &mut s.snap.rotate_enabled,
                    SnapToggle::Scale => &mut s.snap.scale_enabled,
                };
                *flag = !*flag;
            }
        });
    }
}

fn update_snap_toggles(
    settings: Option<Res<ViewportSettings>>,
    theme: Option<Res<ThemeManager>>,
    mut pills: Query<(&SnapPillOf, &mut BackgroundColor)>,
) {
    let (Some(settings), Some(theme)) = (settings, theme) else {
        return;
    };
    let t = &theme.active_theme;
    let accent = col(t.semantic.accent);
    let inactive = col(t.widgets.inactive_bg);

    for (pill, mut bg) in &mut pills {
        let enabled = match pill.0 {
            SnapToggle::Translate => settings.snap.translate_enabled,
            SnapToggle::Rotate => settings.snap.rotate_enabled,
            SnapToggle::Scale => settings.snap.scale_enabled,
        };
        // The whole pill fills accent when the snap is on, so the widget reads as
        // one cohesive filled background (not a bright box beside the number).
        let want = if enabled { accent } else { inactive };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

fn update_three_d_only(
    settings: Option<Res<ViewportSettings>>,
    mut q: Query<&mut Node, With<ThreeDOnly>>,
) {
    let Some(settings) = settings else { return };
    let show = settings.viewport_view != ViewportView::Two;
    for mut n in &mut q {
        let want = if show { Display::Flex } else { Display::None };
        if n.display != want {
            n.display = want;
        }
    }
}

/// Sibling of [`update_three_d_only`]: shows `TwoDOnly` widgets only in 2D view.
fn update_two_d_only(
    settings: Option<Res<ViewportSettings>>,
    mut q: Query<&mut Node, With<TwoDOnly>>,
) {
    let Some(settings) = settings else { return };
    let show = settings.viewport_view == ViewportView::Two;
    for mut n in &mut q {
        let want = if show { Display::Flex } else { Display::None };
        if n.display != want {
            n.display = want;
        }
    }
}

/// Update the 2D zoom readout from the editor 2D camera's orthographic scale.
/// Scale 1.0 reads as 100%; zooming in (smaller scale) grows the percentage.
fn update_zoom_readout(
    cameras_2d: Query<&Projection, With<renzora::core::EditorCamera2d>>,
    mut texts: Query<&mut Text, With<ZoomReadout>>,
) {
    let Ok(Projection::Orthographic(o)) = cameras_2d.single() else {
        return;
    };
    if o.scale <= 0.0 {
        return;
    }
    let pct = (100.0 / o.scale).round() as i32;
    for mut t in &mut texts {
        let s = format!("{pct}%");
        if t.0 != s {
            t.0 = s;
        }
    }
}

// ── Camera + Snap dropdowns (A4) ─────────────────────────────────────────────

/// A discrete one-shot click action inside a header dropdown.
#[derive(Component, Clone, Copy)]
enum HeaderClick {
    Projection(ProjectionMode),
    ViewAngle { yaw: f32, pitch: f32 },
    CamReset,
    ToggleObjectSnap,
    ToggleFloorSnap,
}

/// Tags a projection row so it highlights when that projection is current.
#[derive(Component, Clone, Copy)]
struct ProjOption(ProjectionMode);

/// Object/Floor snap toggle buttons (accent fill when enabled).
#[derive(Component, Clone, Copy)]
enum SnapBtnKind {
    Object,
    Floor,
}

/// The Camera dropdown's icon trigger.
#[derive(Component)]
struct CameraTrigger;

/// The Snap dropdown's icon trigger (magnet — accent when any snap is active).
#[derive(Component)]
struct SnapTrigger;

/// View-angle presets: (label, shortcut, yaw, pitch). Mirrors egui `ViewAngle`.
const VIEW_ANGLES: &[(&str, &str, f32, f32)] = {
    use std::f32::consts::{FRAC_PI_2, PI};
    &[
        ("Front", "Num1", 0.0, 0.0),
        ("Back", "Ctrl+Num1", PI, 0.0),
        ("Left", "Ctrl+Num3", -FRAC_PI_2, 0.0),
        ("Right", "Num3", FRAC_PI_2, 0.0),
        ("Top", "Num7", 0.0, FRAC_PI_2),
        ("Bottom", "Ctrl+Num7", 0.0, -FRAC_PI_2),
    ]
};

/// Builds a label + boxed [`drag_value`] row bound to `ViewportSettings.<path>`.
macro_rules! drag_row {
    ($c:expr, $f:expr, $label:expr, $min:expr, $max:expr, $step:expr, $($field:tt)+) => {
        drag_row_build(
            $c, $f, $label, $min, $max, $step,
            |w: &World| {
                w.get_resource::<ViewportSettings>()
                    .map(|s| s.$($field)+)
                    .unwrap_or($min)
            },
            |w: &mut World, v: f32| {
                if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                    s.$($field)+ = v;
                }
            },
        )
    };
}

/// The shared right-anchored popup panel for an icon dropdown.
fn dropdown_panel(commands: &mut Commands, kids: &[Entity]) -> Entity {
    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(100.0),
                right: Val::Px(0.0),
                margin: UiRect::top(Val::Px(4.0)),
                min_width: Val::Px(220.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            GlobalZIndex(600),
            bevy::ui::RelativeCursorPosition::default(),
            Name::new("vp-drop-panel"),
        ))
        .id();
    commands.entity(panel).add_children(kids);
    panel
}

/// A 40px icon + caret dropdown trigger (the caller adds its own marker bundle).
fn icon_trigger_node(commands: &mut Commands, fonts: &EmberFonts, icon: &str) -> Entity {
    let glyph = icon_text(commands, &fonts.phosphor, icon, text_muted(), 15.0);
    let caret = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 8.0);
    let trigger = commands
        .spawn((
            Node {
                width: Val::Px(40.0),
                height: Val::Px(BTN_H),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::horizontal(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(hover_bg())),
            Interaction::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("vp-icon-dropdown"),
        ))
        .id();
    commands.entity(trigger).add_children(&[glyph, caret]);
    trigger
}

fn dropdown_wrap(commands: &mut Commands, trigger: Entity, panel: Entity) -> Entity {
    let wrap = commands
        .spawn((
            Node {
                position_type: PositionType::Relative,
                ..default()
            },
            Name::new("vp-dropdown-wrap"),
        ))
        .id();
    commands.entity(wrap).add_children(&[trigger, panel]);
    wrap
}

/// A label + click-to-fire row (view angles, reset).
fn click_row(commands: &mut Commands, fonts: &EmberFonts, label: &str, click: HeaderClick) -> Entity {
    let txt = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(BTN_H),
                align_items: AlignItems::Center,
                padding: UiRect::left(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            click,
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("vp-click-row"),
        ))
        .id();
    commands.entity(row).add_child(txt);
    row
}

/// A projection-mode row (highlights when current).
fn proj_row(commands: &mut Commands, fonts: &EmberFonts, mode: ProjectionMode, label: &str) -> Entity {
    let row = click_row(commands, fonts, label, HeaderClick::Projection(mode));
    commands.entity(row).insert(ProjOption(mode));
    row
}

/// A toggle button (Objects / Floor) that fills accent when its snap is on.
fn snap_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    kind: SnapBtnKind,
    click: HeaderClick,
) -> Entity {
    let txt = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let btn = commands
        .spawn((
            Node {
                min_width: Val::Px(70.0),
                height: Val::Px(20.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(hover_bg())),
            Interaction::default(),
            kind,
            click,
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("vp-snap-button"),
        ))
        .id();
    commands.entity(btn).add_child(txt);
    btn
}

/// A label + (flex spacer) + boxed drag_value row, bound two-way.
fn drag_row_build(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    min: f32,
    max: f32,
    step: f32,
    get: impl Fn(&World) -> f32 + Send + Sync + 'static,
    set: impl Fn(&mut World, f32) + Send + Sync + 'static,
) -> Entity {
    let lbl = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(value_text())),
        ))
        .id();
    let spacer = commands
        .spawn(Node {
            flex_grow: 1.0,
            ..default()
        })
        .id();
    let dv = drag_value(commands, &fonts.ui, "", value_text(), min, step);
    commands.entity(dv).insert(DragRange { min, max });
    bind_2way(commands, dv, get, move |w, v: &f32| set(w, *v));
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
            Name::new("vp-drag-row"),
        ))
        .id();
    commands.entity(row).add_children(&[lbl, spacer, dv]);
    row
}

#[allow(clippy::vec_init_then_push)]
fn build_camera_dropdown(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let mut kids: Vec<Entity> = Vec::new();
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.camera.projection")));
    kids.push(proj_row(commands, fonts, ProjectionMode::Perspective, &renzora::lang::t("viewport.camera.perspective")));
    kids.push(proj_row(commands, fonts, ProjectionMode::Orthographic, &renzora::lang::t("viewport.camera.orthographic")));

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.camera.view_angles")));
    for (label, sc, yaw, pitch) in VIEW_ANGLES {
        let lbl = match *label {
            "Front" => renzora::lang::t("viewport.camera.front"),
            "Back" => renzora::lang::t("viewport.camera.back"),
            "Left" => renzora::lang::t("viewport.camera.left"),
            "Right" => renzora::lang::t("viewport.camera.right"),
            "Top" => renzora::lang::t("viewport.camera.top"),
            "Bottom" => renzora::lang::t("viewport.camera.bottom"),
            other => other.to_string(),
        };
        kids.push(click_row(
            commands,
            fonts,
            &format!("{lbl}  ({sc})"),
            HeaderClick::ViewAngle {
                yaw: *yaw,
                pitch: *pitch,
            },
        ));
    }

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.camera.sensitivities")));
    kids.push(drag_row!(commands, fonts, &renzora::lang::t("viewport.camera.look"), 0.05, 2.0, 0.05, camera.look_sensitivity));
    kids.push(drag_row!(commands, fonts, &renzora::lang::t("viewport.camera.orbit"), 0.05, 2.0, 0.05, camera.orbit_sensitivity));
    kids.push(drag_row!(commands, fonts, &renzora::lang::t("viewport.camera.pan"), 0.1, 5.0, 0.1, camera.pan_sensitivity));
    kids.push(drag_row!(commands, fonts, &renzora::lang::t("viewport.camera.zoom"), 0.1, 5.0, 0.1, camera.zoom_sensitivity));

    kids.push(separator_row(commands));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.camera.invert_y"), camera.invert_y));
    kids.push(toggle_row!(
        commands,
        fonts,
        &renzora::lang::t("viewport.camera.distance_relative_speed"),
        camera.distance_relative_speed
    ));
    kids.push(click_row(commands, fonts, &renzora::lang::t("inspector.component.reset"), HeaderClick::CamReset));

    let panel = dropdown_panel(commands, &kids);
    let trigger = icon_trigger_node(commands, fonts, "cube");
    commands.entity(trigger).insert((
        PanelToggle { panel, open: false },
        CameraTrigger,
    ));
    dropdown_wrap(commands, trigger, panel)
}

#[allow(clippy::vec_init_then_push)]
fn build_snap_dropdown(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let mut kids: Vec<Entity> = Vec::new();
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.snap.object_snapping")));
    kids.push(snap_dist_row(
        commands,
        fonts,
        &renzora::lang::t("viewport.snap.objects"),
        SnapBtnKind::Object,
        HeaderClick::ToggleObjectSnap,
        0.1,
        10.0,
        0.1,
        |w| snap_val(w, |s| s.object_snap_distance),
        |w, v| set_snap(w, |s| &mut s.object_snap_distance, v),
    ));
    kids.push(snap_dist_row(
        commands,
        fonts,
        &renzora::lang::t("viewport.snap.floor"),
        SnapBtnKind::Floor,
        HeaderClick::ToggleFloorSnap,
        -1000.0,
        1000.0,
        0.1,
        |w| snap_val(w, |s| s.floor_y),
        |w, v| set_snap(w, |s| &mut s.floor_y, v),
    ));

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.snap.transform_aids")));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.snap.edge_snap"), snap.translate_edge_snap));
    kids.push(toggle_row!(
        commands,
        fonts,
        &renzora::lang::t("viewport.snap.scale_from_bottom"),
        snap.scale_bottom_anchor
    ));

    let panel = dropdown_panel(commands, &kids);
    let trigger = icon_trigger_node(commands, fonts, "magnet");
    commands
        .entity(trigger)
        .insert((PanelToggle { panel, open: false }, SnapTrigger));
    dropdown_wrap(commands, trigger, panel)
}

/// A snap toggle button + its bound distance/offset drag value, in one row.
#[allow(clippy::too_many_arguments)]
fn snap_dist_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    kind: SnapBtnKind,
    click: HeaderClick,
    min: f32,
    max: f32,
    step: f32,
    get: impl Fn(&World) -> f32 + Send + Sync + 'static,
    set: impl Fn(&mut World, f32) + Send + Sync + 'static,
) -> Entity {
    let btn = snap_button(commands, fonts, label, kind, click);
    let dv = drag_value(commands, &fonts.ui, "", value_text(), min, step);
    commands.entity(dv).insert(DragRange { min, max });
    bind_2way(commands, dv, get, move |w, v: &f32| set(w, *v));
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
            Name::new("vp-snap-dist-row"),
        ))
        .id();
    commands.entity(row).add_children(&[btn, dv]);
    row
}

fn header_click(
    q: Query<(&Interaction, &HeaderClick), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, click) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *click {
            HeaderClick::Projection(mode) => cmds.push(move |w: &mut World| {
                if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                    s.projection_mode = mode;
                }
            }),
            HeaderClick::ViewAngle { yaw, pitch } => cmds.push(move |w: &mut World| {
                if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                    s.pending_view_angle = Some(ViewAngleCommand { yaw, pitch });
                }
            }),
            HeaderClick::CamReset => cmds.push(|w: &mut World| {
                if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                    s.camera = CameraSettingsState::default();
                }
            }),
            HeaderClick::ToggleObjectSnap => cmds.push(|w: &mut World| {
                if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                    s.snap.object_snap_enabled = !s.snap.object_snap_enabled;
                }
            }),
            HeaderClick::ToggleFloorSnap => cmds.push(|w: &mut World| {
                if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                    s.snap.floor_snap_enabled = !s.snap.floor_snap_enabled;
                }
            }),
        }
    }
}

/// Hover highlight for plain click rows (view angles, reset) — projection rows
/// and snap buttons are handled by [`update_panel_buttons`].
fn update_click_rows(
    theme: Option<Res<ThemeManager>>,
    mut q: Query<
        (&Interaction, &mut BackgroundColor),
        (With<HeaderClick>, Without<ProjOption>, Without<SnapBtnKind>),
    >,
) {
    let Some(theme) = theme else { return };
    let hovered = col(theme.active_theme.widgets.hovered_bg);
    for (interaction, mut bg) in &mut q {
        let want = if *interaction == Interaction::Hovered {
            hovered
        } else {
            Color::NONE
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

fn update_panel_buttons(
    settings: Option<Res<ViewportSettings>>,
    theme: Option<Res<ThemeManager>>,
    mut proj: Query<(&ProjOption, &Interaction, &mut BackgroundColor), Without<SnapBtnKind>>,
    mut snapbtns: Query<(&SnapBtnKind, &Interaction, &mut BackgroundColor), Without<ProjOption>>,
) {
    let (Some(settings), Some(theme)) = (settings, theme) else {
        return;
    };
    let t = &theme.active_theme;
    let accent = col(t.semantic.accent);
    let inactive = col(t.widgets.inactive_bg);
    let hovered = col(t.widgets.hovered_bg);

    for (opt, interaction, mut bg) in &mut proj {
        let want = if settings.projection_mode == opt.0 {
            accent
        } else if *interaction == Interaction::Hovered {
            hovered
        } else {
            Color::NONE
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
    for (kind, interaction, mut bg) in &mut snapbtns {
        let on = match kind {
            SnapBtnKind::Object => settings.snap.object_snap_enabled,
            SnapBtnKind::Floor => settings.snap.floor_snap_enabled,
        };
        let want = if on {
            accent
        } else if *interaction == Interaction::Hovered {
            hovered
        } else {
            inactive
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

fn update_camera_snap_triggers(
    settings: Option<Res<ViewportSettings>>,
    theme: Option<Res<ThemeManager>>,
    mut cam: Query<
        (&Interaction, &PanelToggle, &mut BackgroundColor),
        (With<CameraTrigger>, Without<SnapTrigger>),
    >,
    mut snap: Query<
        (&Interaction, &PanelToggle, &mut BackgroundColor),
        (With<SnapTrigger>, Without<CameraTrigger>),
    >,
) {
    let (Some(settings), Some(theme)) = (settings, theme) else {
        return;
    };
    let t = &theme.active_theme;
    let accent = col(t.semantic.accent);
    let inactive = col(t.widgets.inactive_bg);
    let hovered = col(t.widgets.hovered_bg);

    for (interaction, toggle, mut bg) in &mut cam {
        let want = if toggle.open || *interaction == Interaction::Hovered {
            hovered
        } else {
            inactive
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
    let s = &settings.snap;
    let any_snap = s.object_snap_enabled
        || s.floor_snap_enabled
        || s.translate_edge_snap
        || s.scale_bottom_anchor;
    for (interaction, toggle, mut bg) in &mut snap {
        let want = if any_snap {
            accent
        } else if toggle.open || *interaction == Interaction::Hovered {
            hovered
        } else {
            inactive
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

// ── Registry-driven tool buttons (A6) ────────────────────────────────────────

/// The header's tool-button strip; filled from `ToolbarRegistry` once it exists.
#[derive(Component)]
struct ToolContainer;

/// Marks a container that's already been populated (so we don't refill it, but a
/// freshly re-created panel still gets filled).
#[derive(Component)]
struct ToolsPopulated;

/// A registry-backed tool button: carries the predicates/activator so the
/// per-frame systems can highlight, show/hide, and fire it.
#[derive(Component, Clone)]
struct ToolButton {
    glyph: Entity,
    visible: Arc<dyn Fn(&World) -> bool + Send + Sync>,
    is_active: Arc<dyn Fn(&World) -> bool + Send + Sync>,
    activate: Arc<dyn Fn(&mut World) + Send + Sync>,
}

/// Fill an empty `ToolContainer` from the registry (Transform / Terrain / custom
/// sections with separators). Exclusive because the visibility/active predicates
/// take `&World`; runs until the registry is populated and the container exists.
fn populate_tools(world: &mut World) {
    let Some(registry) = world.get_resource::<ToolbarRegistry>().cloned() else {
        return;
    };
    if registry.entries().is_empty() {
        return; // tools not registered yet
    }
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    let mut cq = world.query_filtered::<Entity, (With<ToolContainer>, Without<ToolsPopulated>)>();
    let Some(container) = cq.iter(world).next() else {
        return;
    };

    // Build the ordered section list: Transform, Terrain, then custom sections.
    let mut sections: Vec<Vec<renzora_editor_framework::ToolEntry>> = Vec::new();
    let by_section = |sec| {
        let mut v: Vec<_> = registry
            .entries()
            .iter()
            .filter(|e| e.section == sec)
            .cloned()
            .collect();
        v.sort_by_key(|e| e.order);
        v
    };
    let transform = by_section(ToolSection::Transform);
    if !transform.is_empty() {
        sections.push(transform);
    }
    let terrain = by_section(ToolSection::Terrain);
    if !terrain.is_empty() {
        sections.push(terrain);
    }
    for id in registry.custom_sections() {
        let v = by_section(ToolSection::Custom(id));
        if !v.is_empty() {
            sections.push(v);
        }
    }

    let mut queue = CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        let mut children: Vec<Entity> = Vec::new();
        for (si, section) in sections.iter().enumerate() {
            if si > 0 {
                children.push(tool_separator(&mut commands));
            }
            for entry in section {
                children.push(tool_button(&mut commands, &fonts, entry));
            }
        }
        commands.entity(container).add_children(&children);
        commands.entity(container).insert(ToolsPopulated);
    }
    queue.apply(world);
}

fn tool_separator(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Px(1.0),
                height: Val::Px(16.0),
                margin: UiRect::horizontal(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(border())),
            Name::new("vp-tool-sep"),
        ))
        .id()
}

fn tool_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    entry: &renzora_editor_framework::ToolEntry,
) -> Entity {
    // `entry.icon` is either a kebab-case Phosphor name (resolved via `icon_glyph`)
    // or, for entries that still pass a raw glyph constant, the glyph char itself —
    // fall back to rendering it verbatim when it isn't a known name.
    let glyph_str = icon_glyph(entry.icon)
        .map(|c| c.to_string())
        .unwrap_or_else(|| entry.icon.to_string());
    let glyph = commands
        .spawn((
            Text::new(glyph_str),
            TextFont {
                // 0.19 Parley: font -> FontSource, font_size -> FontSize.
                font: bevy::text::FontSource::Handle(fonts.phosphor.clone()),
                font_size: bevy::text::FontSize::Px(15.0),
                ..default()
            },
            TextColor(rgb(text_primary())),
        ))
        .id();
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(26.0),
                height: Val::Px(BTN_H),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            ToolButton {
                glyph,
                visible: entry.visible.clone(),
                is_active: entry.is_active.clone(),
                activate: entry.activate.clone(),
            },
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new(format!("vp-tool:{}", entry.id)),
        ))
        .id();
    commands.entity(btn).add_child(glyph);
    btn
}

/// Per-frame: evaluate each tool's `visible` (show/hide) and `is_active`
/// (accent highlight). Exclusive because the predicates take `&World`.
fn update_tool_buttons(world: &mut World) {
    let mut q = world.query::<(Entity, &ToolButton, &Interaction)>();
    let collected: Vec<(Entity, ToolButton, Interaction)> = q
        .iter(world)
        .map(|(e, b, i)| (e, b.clone(), *i))
        .collect();
    if collected.is_empty() {
        return;
    }
    let (accent, hovered, icon_active, icon_inactive) = {
        let Some(tm) = world.get_resource::<ThemeManager>() else {
            return;
        };
        (
            col(tm.active_theme.semantic.accent),
            col(tm.active_theme.widgets.hovered_bg),
            // White-ish on the accent fill when active; a clear neutral otherwise
            // (so tool icons stay legible on light themes).
            col(tm.active_theme.widgets.active_fg),
            col(tm.active_theme.text.secondary),
        )
    };
    let results: Vec<(Entity, bool, Color, Entity, Color)> = collected
        .iter()
        .map(|(e, b, inter)| {
            let visible = (b.visible)(world);
            let active = (b.is_active)(world);
            let bg = if active {
                accent
            } else if *inter == Interaction::Hovered {
                hovered
            } else {
                Color::NONE
            };
            let icol = if active { icon_active } else { icon_inactive };
            (*e, visible, bg, b.glyph, icol)
        })
        .collect();
    for (e, visible, bg, glyph, icol) in results {
        if let Some(mut node) = world.get_mut::<Node>(e) {
            let want = if visible { Display::Flex } else { Display::None };
            if node.display != want {
                node.display = want;
            }
        }
        if let Some(mut bgc) = world.get_mut::<BackgroundColor>(e) {
            if bgc.0 != bg {
                bgc.0 = bg;
            }
        }
        if let Some(mut tc) = world.get_mut::<TextColor>(glyph) {
            if tc.0 != icol {
                tc.0 = icol;
            }
        }
    }
}

fn tool_button_click(
    q: Query<(&Interaction, &ToolButton), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let activate = btn.activate.clone();
        cmds.push(move |w: &mut World| (activate)(w));
    }
}

// ── Add-shape dropdown ───────────────────────────────────────────────────────
//
// A dropdown that spawns any registered shape from the toolbar. It reads the
// same `ShapeRegistry` the shape-library panel uses (so the two never drift),
// grouped by category. Population is deferred to an exclusive system because the
// registry is filled by the shape crate's plugin at startup, after the header is
// built; the list is filled once the registry is non-empty.

/// The shapes-menu trigger button (for hover / open background tinting).
#[derive(Component)]
struct ShapeMenuTrigger;

/// The (initially empty) column inside the shapes popup that `populate_shapes`
/// fills from the registry.
#[derive(Component)]
struct ShapeMenuContainer;

/// Marks a shapes list that's already been filled, so it isn't refilled.
#[derive(Component)]
struct ShapesPopulated;

/// A selectable shape row — carries the registry id so the click handler can
/// look the rest up (name + default color) at spawn time.
#[derive(Component, Clone)]
struct ShapeSpawn {
    id: String,
}

fn build_shapes_dropdown(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // Empty column that `populate_shapes` fills; wrapped in a capped scroll area
    // since the registry holds ~30 shapes across several categories.
    let container = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            ShapeMenuContainer,
            Name::new("vp-shapes-list"),
        ))
        .id();
    let scroll = scroll_area(commands, container, 360.0);

    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(100.0),
                left: Val::Px(0.0),
                margin: UiRect::top(Val::Px(4.0)),
                width: Val::Px(200.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            GlobalZIndex(600),
            // Floating popup: swallow scroll/click so the wheel scrolls the list
            // instead of bleeding to the dock behind it (see ember `popup`).
            OverlaySurface,
            bevy::ui::RelativeCursorPosition::default(),
            Name::new("vp-shapes-panel"),
        ))
        .id();
    commands.entity(panel).add_child(scroll);

    let glyph = icon_text(commands, &fonts.phosphor, "shapes", text_muted(), 15.0);
    let caret = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 8.0);
    let trigger = commands
        .spawn((
            Node {
                width: Val::Px(40.0),
                height: Val::Px(BTN_H),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::horizontal(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(tab_active())),
            Interaction::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            PanelToggle { panel, open: false },
            ShapeMenuTrigger,
            Name::new("vp-shapes-dropdown"),
        ))
        .id();
    commands.entity(trigger).add_children(&[glyph, caret]);

    dropdown_wrap(commands, trigger, panel)
}

/// A label + icon row that spawns shape `id` when clicked.
fn shape_row(commands: &mut Commands, fonts: &EmberFonts, icon: &str, name: &str, id: &str) -> Entity {
    let glyph = icon_text(commands, &fonts.phosphor, icon, text_primary(), 14.0);
    let label = commands
        .spawn((
            Text::new(name),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(BTN_H),
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::horizontal(Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            ShapeSpawn { id: id.to_string() },
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("vp-shape-row"),
        ))
        .id();
    commands.entity(row).add_children(&[glyph, label]);
    row
}

/// Fill an empty `ShapeMenuContainer` from `ShapeRegistry`, grouped by category
/// (a section label whenever the category changes, separators between groups).
/// Exclusive so it can spawn rows from the registry's `&World` data; runs until
/// the registry is populated and the container exists.
fn populate_shapes(world: &mut World) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    // Snapshot (icon, name, id, category) so the borrow of the registry ends
    // before we open a `Commands` over the world.
    let shapes: Vec<(String, String, String, String)> = {
        let Some(reg) = world.get_resource::<ShapeRegistry>() else {
            return;
        };
        reg.iter()
            .map(|e| {
                (
                    e.icon.to_string(),
                    e.name.to_string(),
                    e.id.to_string(),
                    e.category.to_string(),
                )
            })
            .collect()
    };
    if shapes.is_empty() {
        return; // shapes not registered yet
    }
    let mut cq = world.query_filtered::<Entity, (With<ShapeMenuContainer>, Without<ShapesPopulated>)>();
    let Some(container) = cq.iter(world).next() else {
        return;
    };

    let mut queue = CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        let mut children: Vec<Entity> = Vec::new();
        let mut last_cat: Option<&str> = None;
        for (icon, name, id, category) in &shapes {
            if last_cat != Some(category.as_str()) {
                if last_cat.is_some() {
                    children.push(separator_row(&mut commands));
                }
                children.push(section_label(&mut commands, &fonts, category));
                last_cat = Some(category.as_str());
            }
            children.push(shape_row(&mut commands, &fonts, icon, name, id));
        }
        commands.entity(container).add_children(&children);
        commands.entity(container).insert(ShapesPopulated);
    }
    queue.apply(world);
}

/// Spawn the clicked shape at the origin (matching the hierarchy "Add Entity"
/// menu) through the undo system, then leave the menu open so several shapes can
/// be added in a row.
fn shape_spawn_click(
    q: Query<(&Interaction, &ShapeSpawn), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, shape) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let id = shape.id.clone();
        cmds.push(move |w: &mut World| {
            let Some((shape_id, name, color)) = w
                .get_resource::<ShapeRegistry>()
                .and_then(|r| r.get(&id))
                .map(|e| (e.id.to_string(), e.name.to_string(), e.default_color))
            else {
                warn!("Shape '{id}' not found in registry");
                return;
            };
            execute(
                w,
                UndoContext::Scene,
                Box::new(SpawnShapeCmd {
                    entity: Entity::PLACEHOLDER,
                    shape_id,
                    name,
                    position: Vec3::ZERO,
                    color,
                }),
            );
        });
    }
}

/// Hover/open tinting for the shapes trigger + hover highlight for its rows.
fn update_shape_menu(
    theme: Option<Res<ThemeManager>>,
    mut trigger: Query<
        (&Interaction, &PanelToggle, &mut BackgroundColor),
        (With<ShapeMenuTrigger>, Without<ShapeSpawn>),
    >,
    mut rows: Query<(&Interaction, &mut BackgroundColor), With<ShapeSpawn>>,
) {
    let Some(theme) = theme else { return };
    let t = &theme.active_theme;
    let inactive = col(t.widgets.inactive_bg);
    let hovered = col(t.widgets.hovered_bg);

    for (interaction, toggle, mut bg) in &mut trigger {
        let want = if toggle.open || *interaction == Interaction::Hovered {
            hovered
        } else {
            inactive
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
    for (interaction, mut bg) in &mut rows {
        let want = if *interaction == Interaction::Hovered {
            hovered
        } else {
            Color::NONE
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}
