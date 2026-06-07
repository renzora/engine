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
use bevy::ecs::world::CommandQueue;
use std::sync::Arc;

use renzora_editor_framework::{EditorCommands, ToolSection, ToolbarRegistry};
use renzora_ember::font::{icon_glyph, icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::bind_2way;
use renzora_ember::widgets::{checkbox, drag_value, drag_value_flat, DragRange};
use renzora_ember::theme::{
    border, hover_bg, panel_bg, popup_bg, rgb, tab_active, text_muted, text_primary, value_text,
};
use renzora_hui::cursor_icon::HoverCursor;
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
    Play,
    Scripts,
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

/// Build the header row (child 0 of the primary viewport panel).
pub(crate) fn build_header(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
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
    let gap2 = gap(commands, 6.0);
    let play = action_btn(commands, fonts, HeaderAction::Play, "play");
    let scripts = action_btn(commands, fonts, HeaderAction::Scripts, "code");

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

    let gap3 = gap(commands, 8.0);
    let view_dd = dropdown(commands, fonts, DropKind::View, 56.0);
    let mode_dd = dropdown(commands, fonts, DropKind::Mode, 80.0);

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

    let spacer = commands
        .spawn((Node { flex_grow: 1.0, ..default() }, Name::new("vp-hdr-spacer")))
        .id();
    let display_dd = build_display_dropdown(commands, fonts);
    let snap_dd = build_snap_dropdown(commands, fonts);
    let camera_dd = build_camera_dropdown(commands, fonts);
    let gap4 = gap(commands, 3.0);
    let maximize = action_btn(commands, fonts, HeaderAction::Maximize, "arrows-out");

    // Left: session actions (+ tools, later). Right: view/mode, inline snapping,
    // camera speed, then the Display / Snap / Camera dropdowns + maximize.
    commands.entity(row).add_children(&[
        undo, redo, gap1, save, gap2, play, scripts, tools_gap, tools, spacer, view_dd, mode_dd,
        gap5, translate, rotate, scale, gap6, cam_speed, gap3, display_dd, snap_dd, camera_dd,
        gap4, maximize,
    ]);
    row
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
        )
            .run_if(in_state(SplashState::Editor)),
    );
    // Exclusive (need `&World` for the registry predicates).
    app.add_systems(
        Update,
        (populate_tools, update_tool_buttons).run_if(in_state(SplashState::Editor)),
    );
}

// ── View / Mode dropdowns (A2) ───────────────────────────────────────────────

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum DropKind {
    View,
    Mode,
}

impl DropKind {
    /// Option labels for this dropdown, in `ALL` order.
    fn labels(self) -> Vec<&'static str> {
        match self {
            DropKind::View => ViewportView::ALL.iter().map(|v| v.label()).collect(),
            DropKind::Mode => ViewportMode::ALL.iter().map(|m| m.label()).collect(),
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
                Text::new(*label),
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
            Text::new(labels.first().copied().unwrap_or("")),
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
                DropKind::View => settings.viewport_view.label(),
                DropKind::Mode => settings.viewport_mode.label(),
            };
            if text.0 != label {
                text.0 = label.to_string();
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
    has_scene_camera: bool,
    is_active: bool,   // in-editor play OR external runtime alive
    is_editing: bool,  // editing and no runtime
    is_scripts: bool,  // scripts-only running
    maximized: bool,
    primary: Color,
    muted: Color,
    play: Color,
    scripts: Color,
    stop: Color,
    accent: Color,
    hovered_bg: Color,
}

fn update_header_visuals(
    actions: Query<(&HeaderAction, &HeaderIcon, &Interaction, &mut BackgroundColor)>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
    undo: Option<Res<renzora_undo::UndoStacks>>,
    tabs: Option<Res<renzora_ui::DocumentTabState>>,
    pm: Option<Res<renzora::core::PlayModeState>>,
    runtime: Option<Res<crate::external_runtime::ExternalRuntime>>,
    maximized: Option<Res<renzora_ui::ViewportMaximized>>,
    theme: Option<Res<ThemeManager>>,
    scene_cams: Query<(), With<renzora::core::SceneCamera>>,
) {
    let Some(theme) = theme else { return };
    let t = &theme.active_theme;

    let (can_undo, can_redo) = undo
        .map(|s| (s.can_undo(&s.active), s.can_redo(&s.active)))
        .unwrap_or((false, false));
    let can_save = tabs
        .and_then(|tabs| tabs.tabs.get(tabs.active_tab).map(|t| t.is_modified))
        .unwrap_or(false);
    let is_playing = pm.as_ref().map(|p| p.is_in_play_mode()).unwrap_or(false);
    let is_scripts = pm.as_ref().map(|p| p.is_scripts_only()).unwrap_or(false);
    let is_editing_raw = pm.as_ref().map(|p| p.is_editing()).unwrap_or(true);
    let runtime_alive = runtime.map(|r| r.is_alive()).unwrap_or(false);

    let model = HeaderModel {
        can_undo,
        can_redo,
        can_save,
        has_scene_camera: !scene_cams.is_empty(),
        is_active: is_playing || runtime_alive,
        is_editing: is_editing_raw && !runtime_alive,
        is_scripts,
        maximized: maximized.is_some_and(|m| m.0),
        primary: col(t.text.primary),
        muted: col(t.text.muted),
        play: col(t.semantic.success),
        scripts: col(t.semantic.accent),
        stop: col(t.semantic.error),
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
        HeaderAction::Play => {
            if m.is_active {
                ("stop", m.stop, true)
            } else if !m.has_scene_camera {
                ("play", m.muted, false)
            } else {
                ("play", m.play, true)
            }
        }
        HeaderAction::Scripts => {
            if m.is_scripts {
                ("stop", m.scripts, true)
            } else if m.is_active {
                ("code", m.muted, false)
            } else {
                ("code", m.scripts, m.is_editing)
            }
        }
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
            HeaderAction::Play => cmds.push(|w: &mut World| {
                use renzora::core::{PlayModeState, SceneCamera};
                let runtime_alive = w
                    .get_resource::<crate::external_runtime::ExternalRuntime>()
                    .map(|r| r.is_alive())
                    .unwrap_or(false);
                let mut cam_q = w.query_filtered::<(), With<SceneCamera>>();
                let has_cam = cam_q.iter(w).next().is_some();
                if let Some(mut pm) = w.get_resource_mut::<PlayModeState>() {
                    if runtime_alive || pm.is_in_play_mode() {
                        pm.request_stop = true;
                    } else if pm.is_scripts_only() {
                        pm.request_stop = true;
                        pm.request_play = true;
                    } else if pm.is_editing() && has_cam {
                        pm.request_play = true;
                    }
                }
            }),
            HeaderAction::Scripts => cmds.push(|w: &mut World| {
                use renzora::core::PlayModeState;
                if let Some(mut pm) = w.get_resource_mut::<PlayModeState>() {
                    if pm.is_scripts_only() {
                        pm.request_stop = true;
                    } else if pm.is_editing() {
                        pm.request_scripts_only = true;
                    }
                }
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

fn build_display_dropdown(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let mut kids: Vec<Entity> = Vec::new();

    kids.push(section_label(commands, fonts, "Visualization"));
    for (i, m) in VisualizationMode::ALL.iter().enumerate() {
        kids.push(option_row(commands, fonts, DisplayOption::Viz(i), m.label()));
    }

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, "Render"));
    kids.push(toggle_row!(commands, fonts, "Mesh", render_toggles.mesh));
    kids.push(toggle_row!(commands, fonts, "Textures", render_toggles.textures));
    kids.push(toggle_row!(commands, fonts, "Wireframe", render_toggles.wireframe));
    kids.push(toggle_row!(commands, fonts, "Lighting", render_toggles.lighting));
    kids.push(toggle_row!(commands, fonts, "Shadows", render_toggles.shadows));

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, "Overlays"));
    kids.push(toggle_row!(commands, fonts, "Grid", show_grid));
    kids.push(toggle_row!(commands, fonts, "Sub-grid", show_subgrid));
    kids.push(toggle_row!(commands, fonts, "Axis Gizmo", show_axis_gizmo));
    kids.push(toggle_row!(commands, fonts, "Scene Icons", show_scene_icons));

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, "Collision Gizmos"));
    kids.push(option_row(
        commands,
        fonts,
        DisplayOption::Collision(true),
        "Selected Only",
    ));
    kids.push(option_row(
        commands,
        fonts,
        DisplayOption::Collision(false),
        "Always",
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

/// A label + two-way checkbox row, bound to a `ViewportSettings` field.
fn check_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    get: impl Fn(&World) -> bool + Send + Sync + 'static,
    set: impl Fn(&mut World, bool) + Send + Sync + 'static,
) -> Entity {
    let cb = checkbox(commands, false);
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
    kids.push(section_label(commands, fonts, "Projection"));
    kids.push(proj_row(commands, fonts, ProjectionMode::Perspective, "Perspective"));
    kids.push(proj_row(commands, fonts, ProjectionMode::Orthographic, "Orthographic"));

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, "View Angles"));
    for (label, sc, yaw, pitch) in VIEW_ANGLES {
        kids.push(click_row(
            commands,
            fonts,
            &format!("{label}  ({sc})"),
            HeaderClick::ViewAngle {
                yaw: *yaw,
                pitch: *pitch,
            },
        ));
    }

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, "Sensitivities"));
    kids.push(drag_row!(commands, fonts, "Look", 0.05, 2.0, 0.05, camera.look_sensitivity));
    kids.push(drag_row!(commands, fonts, "Orbit", 0.05, 2.0, 0.05, camera.orbit_sensitivity));
    kids.push(drag_row!(commands, fonts, "Pan", 0.1, 5.0, 0.1, camera.pan_sensitivity));
    kids.push(drag_row!(commands, fonts, "Zoom", 0.1, 5.0, 0.1, camera.zoom_sensitivity));

    kids.push(separator_row(commands));
    kids.push(toggle_row!(commands, fonts, "Invert Y Axis", camera.invert_y));
    kids.push(toggle_row!(
        commands,
        fonts,
        "Distance Relative Speed",
        camera.distance_relative_speed
    ));
    kids.push(click_row(commands, fonts, "Reset to Defaults", HeaderClick::CamReset));

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
    kids.push(section_label(commands, fonts, "Object Snapping"));
    kids.push(snap_dist_row(
        commands,
        fonts,
        "Objects",
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
        "Floor",
        SnapBtnKind::Floor,
        HeaderClick::ToggleFloorSnap,
        -1000.0,
        1000.0,
        0.1,
        |w| snap_val(w, |s| s.floor_y),
        |w, v| set_snap(w, |s| &mut s.floor_y, v),
    ));

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, "Transform Aids"));
    kids.push(toggle_row!(commands, fonts, "Edge Snap", snap.translate_edge_snap));
    kids.push(toggle_row!(
        commands,
        fonts,
        "Scale from Bottom",
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
                font: fonts.phosphor.clone(),
                font_size: 15.0,
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
