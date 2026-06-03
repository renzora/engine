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
use bevy_egui::egui::Color32;

use renzora::core::viewport_types::{
    CollisionGizmoVisibility, ViewportMode, ViewportSettings, ViewportView, VisualizationMode,
};
use renzora_editor::EditorCommands;
use renzora_ember::font::{icon_glyph, icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::bind_2way;
use renzora_ember::widgets::checkbox;
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

fn col(c: Color32) -> Color {
    Color::srgb_u8(c.r(), c.g(), c.b())
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
            BackgroundColor(Color::srgb_u8(38, 38, 46)),
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
    let gap3 = gap(commands, 8.0);
    let view_dd = dropdown(commands, fonts, DropKind::View, 56.0);
    let mode_dd = dropdown(commands, fonts, DropKind::Mode, 80.0);
    let spacer = commands
        .spawn((Node { flex_grow: 1.0, ..default() }, Name::new("vp-hdr-spacer")))
        .id();
    let display_dd = build_display_dropdown(commands, fonts);
    let gap4 = gap(commands, 3.0);
    let maximize = action_btn(commands, fonts, HeaderAction::Maximize, "arrows-out");

    commands.entity(row).add_children(&[
        undo, redo, gap1, save, gap2, play, scripts, gap3, view_dd, mode_dd, spacer, display_dd,
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
    let glyph = icon_text(commands, &fonts.phosphor, icon, (230, 230, 240), 14.0);
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
    use renzora_editor::SplashState;
    app.add_systems(
        Update,
        (
            update_header_visuals,
            header_action_click,
            dropdown_toggle,
            dropdown_option_click,
            update_dropdown_visuals,
            panel_toggle,
            display_option_click,
            update_display_visuals,
        )
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
                TextColor(Color::srgb_u8(230, 230, 240)),
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
            BackgroundColor(Color::srgb_u8(30, 30, 38)),
            BorderColor::all(Color::srgb_u8(60, 60, 74)),
            GlobalZIndex(600),
            Name::new("vp-drop-panel"),
        ))
        .id();
    commands.entity(panel).add_children(&rows);

    // Trigger button: current label + caret.
    let label_e = commands
        .spawn((
            Text::new(labels.first().copied().unwrap_or("")),
            ui_font(&fonts.ui, 12.0),
            TextColor(Color::srgb_u8(230, 230, 240)),
        ))
        .id();
    let caret = icon_text(commands, &fonts.phosphor, "caret-down", (200, 200, 210), 10.0);
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
            BackgroundColor(Color::srgb_u8(50, 50, 62)),
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
    let accent = col(t.semantic.accent.to_color32());
    let inactive = col(t.widgets.inactive_bg.to_color32());
    let hovered = col(t.widgets.hovered_bg.to_color32());

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
        primary: col(t.text.primary.to_color32()),
        muted: col(t.text.muted.to_color32()),
        play: col(t.semantic.success.to_color32()),
        scripts: col(t.semantic.accent.to_color32()),
        stop: col(t.semantic.error.to_color32()),
        accent: col(t.semantic.accent.to_color32()),
        hovered_bg: col(t.widgets.hovered_bg.to_color32()),
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
            BackgroundColor(Color::srgb_u8(30, 30, 38)),
            BorderColor::all(Color::srgb_u8(60, 60, 74)),
            GlobalZIndex(600),
            Name::new("vp-display-panel"),
        ))
        .id();
    commands.entity(panel).add_children(&kids);

    let eye = icon_text(commands, &fonts.phosphor, "eye", (190, 190, 200), 15.0);
    let caret = icon_text(commands, &fonts.phosphor, "caret-down", (160, 160, 170), 8.0);
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
            BackgroundColor(Color::srgb_u8(50, 50, 62)),
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
            TextColor(Color::srgb_u8(140, 140, 152)),
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
            BackgroundColor(Color::srgb_u8(60, 60, 74)),
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
            TextColor(Color::srgb_u8(230, 230, 240)),
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
            TextColor(Color::srgb_u8(220, 220, 230)),
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
    let accent = col(t.semantic.accent.to_color32());
    let inactive = col(t.widgets.inactive_bg.to_color32());
    let hovered = col(t.widgets.hovered_bg.to_color32());

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
