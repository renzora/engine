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

use renzora_editor::EditorCommands;
use renzora_ember::font::{icon_glyph, icon_text, EmberFonts};
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
    let spacer = commands
        .spawn((Node { flex_grow: 1.0, ..default() }, Name::new("vp-hdr-spacer")))
        .id();
    let maximize = action_btn(commands, fonts, HeaderAction::Maximize, "arrows-out");

    commands
        .entity(row)
        .add_children(&[undo, redo, gap1, save, gap2, play, scripts, spacer, maximize]);
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
        (update_header_visuals, header_action_click).run_if(in_state(SplashState::Editor)),
    );
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
