//! The top bar's Play control: one button that starts the game, and stops it.
//!
//! There used to be a caret beside it choosing where Play ran (viewport,
//! window, VR headset, or a "Scripts" simulate mode). Four answers to "where
//! does my game run" was three too many, and the in-editor ones let a slow
//! game slow the editor. The game now always runs as its own process; see
//! `renzora_viewport::play_mode`.

use bevy::prelude::*;
use bevy::ui::BackgroundGradient;

use renzora_ember::font::{glyph, ui_font};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::theme::{play_green, rgb, text_muted, text_primary};
use renzora_ember::widgets::{chip_border, chip_gradient, CHIP_RADIUS};
use renzora_ember::UiInteraction;

/// The Play button, in the top bar's right zone just ahead of the Marketplace.
/// It sits in the top bar rather than the viewport's tool strip because running
/// the game is not a viewport action, and this bar is on screen in every
/// workspace. It led the left zone until the scene switcher took that spot:
/// the left end says what you are editing, the right end runs and ships it.
pub(crate) fn build_play_group(commands: &mut Commands, font: &bevy::text::FontSource) -> Entity {
    let keep = build_keep_button(commands, font);
    let play = build_play_button(commands, font);
    let group = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                margin: UiRect::right(Val::Px(6.0)),
                ..default()
            },
            Name::new("play-group"),
        ))
        .id();
    commands.entity(group).add_children(&[keep, play]);
    group
}

/// The Keep button: adopt what the running game has changed, so Stop leaves it
/// in the scene instead of putting it back. See `renzora_live_link_editor`.
#[derive(Component)]
pub(crate) struct TopBarKeepBtn;

/// Shaped like the Play pill beside it, neutral where Play is coloured: it is
/// a secondary action, and only exists while a game is connected.
fn build_keep_button(commands: &mut Commands, font: &bevy::text::FontSource) -> Entity {
    let btn = commands
        .spawn((
            Node {
                display: Display::None,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(CHIP_RADIUS)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            TopBarKeepBtn,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("top-bar-keep"),
        ))
        .id();
    let icon = glyph(commands, "push-pin", text_primary(), 14.0);
    commands.entity(icon).insert(PASS_THROUGH);
    let label = commands
        .spawn((
            Text::new(renzora::lang::t("play.keep")),
            ui_font(font, 11.0),
            TextColor(rgb(text_primary())),
            PASS_THROUGH,
        ))
        .id();
    commands.entity(btn).add_children(&[icon, label]);
    btn
}

/// Keep: show while the editor mirrors a running game, and ask the live link
/// to adopt the game's changes when clicked.
pub(crate) fn keep_btn(
    live: Option<Res<renzora::LivePlaySession>>,
    mut btns: Query<(&mut Node, &mut BackgroundColor, &UiInteraction), With<TopBarKeepBtn>>,
    mut commands: Commands,
) {
    for (mut node, mut fill, interaction) in &mut btns {
        let display = if live.is_some() { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
        let alpha = match interaction {
            UiInteraction::Pressed => 0.28,
            UiInteraction::Hovered => 0.18,
            _ => 0.10,
        };
        let want = Color::srgba(1.0, 1.0, 1.0, alpha);
        if fill.0 != want {
            fill.0 = want;
        }
        if live.is_some() && *interaction == UiInteraction::Pressed {
            commands.insert_resource(renzora::KeepLiveChangesRequested);
        }
    }
}

/// The Play / Stop button (icon + text).
#[derive(Component)]
pub(crate) struct TopBarPlayBtn;
/// The play button's phosphor glyph (swaps play ↔ stop with state).
#[derive(Component)]
pub(crate) struct TopBarPlayIcon;
/// The play button's "Play" / "Stop" text label.
#[derive(Component)]
pub(crate) struct TopBarPlayLabel;

/// Build the top-bar Play button: a phosphor glyph + a "Play" label in one
/// clickable pill. The glyph + label live as `PASS_THROUGH` children so the
/// hover/click lands on the parent (where `UiInteraction` lives).
///
/// Shaped exactly like the Marketplace chip beside it (`top_bar`'s
/// `shell_action_button`): the same padding, rounding, glyph size, gradient
/// and a primary-coloured label, with only the hue telling them apart. Side by
/// side, a square green chip with green text next to a round violet one with
/// white text read as two unrelated controls.
fn build_play_button(commands: &mut Commands, font: &bevy::text::FontSource) -> Entity {
    let btn = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(CHIP_RADIUS)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BackgroundGradient::default(),
            BorderColor::all(Color::NONE),
            UiInteraction::default(),
            TopBarPlayBtn,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("top-bar-play"),
        ))
        .id();
    let icon = glyph(commands, "play", play_green(), 14.0);
    commands
        .entity(icon)
        .insert((TopBarPlayIcon, PASS_THROUGH));
    let label = commands
        .spawn((
            Text::new(renzora::lang::t("common.play")),
            ui_font(font, 11.0),
            TextColor(rgb(text_primary())),
            TopBarPlayLabel,
            PASS_THROUGH,
        ))
        .id();
    commands.entity(btn).add_children(&[icon, label]);
    btn
}

/// Whether any global (autoload) scene supplies a camera.
///
/// The Play gate asks "is there a scene camera", and until now asked it of the
/// *live world*. Global scenes don't load until Play, so a project whose only
/// camera lives in one could never start: the camera isn't there to open the
/// gate, and the gate is what would load it.
///
/// Answered from the scene files rather than the world, since that is the only
/// place the information exists while editing.
#[derive(Resource, Default)]
pub(crate) struct GlobalSceneHasCamera(bool);

/// Recompute [`GlobalSceneHasCamera`] when the autoload list changes.
///
/// A substring test for the component's type name, not a parse: the answer only
/// gates a button, both scene formats spell the type the same way, and a wrong
/// answer degrades safely: a false positive lets Play start, and the game
/// reports the missing camera itself.
pub(crate) fn track_global_scene_cameras(
    project: Option<Res<renzora::CurrentProject>>,
    mut state: ResMut<GlobalSceneHasCamera>,
    mut last: Local<Option<Vec<String>>>,
) {
    let Some(project) = project else { return };
    if last.as_ref() == Some(&project.config.autoload) {
        return;
    }
    *last = Some(project.config.autoload.clone());
    state.0 = project.config.autoload.iter().any(|rel| {
        std::fs::read_to_string(project.resolve_path(rel))
            .map(|text| text.contains("SceneCamera"))
            .unwrap_or(false)
    });
}

/// Click the top-bar Play button: start the game when there is a scene camera
/// to play through, or stop it while it runs.
pub(crate) fn play_btn_click(
    btns: Query<&UiInteraction, (Changed<UiInteraction>, With<TopBarPlayBtn>)>,
    play_mode: Option<ResMut<renzora::core::PlayModeState>>,
    runtime: Option<Res<renzora_viewport::external_runtime::ExternalRuntime>>,
    scene_cams: Query<(), With<renzora::core::SceneCamera>>,
    global_cam: Option<Res<GlobalSceneHasCamera>>,
) {
    let Some(mut pm) = play_mode else { return };
    let running = runtime.is_some_and(|r| r.is_alive());
    // A camera in a global scene counts even though it isn't loaded yet.
    let has_cam = !scene_cams.is_empty() || global_cam.is_some_and(|g| g.0);
    for interaction in &btns {
        if *interaction != UiInteraction::Pressed {
            continue;
        }
        if running {
            pm.request_stop = true;
        } else if has_cam {
            pm.request_play = true;
        }
    }
}

/// Drive the Play button's glyph, label and colour from whether the game is
/// running: green "Play" when it isn't (muted if there's no scene camera), red
/// "Stop" while it is.
pub(crate) fn update_play_button(
    runtime: Option<Res<renzora_viewport::external_runtime::ExternalRuntime>>,
    theme: Option<Res<renzora_theme::ThemeManager>>,
    scene_cams: Query<(), With<renzora::core::SceneCamera>>,
    global_cam: Option<Res<GlobalSceneHasCamera>>,
    mut icons: Query<&mut renzora_ember::icons::Icon, With<TopBarPlayIcon>>,
    mut labels: Query<(&mut Text, &mut TextColor), With<TopBarPlayLabel>>,
    mut fills: Query<(&mut BackgroundGradient, &mut BorderColor, &UiInteraction), With<TopBarPlayBtn>>,
) {
    let Some(theme) = theme else { return };
    let t = &theme.active_theme;
    let tc = |c: renzora_theme::ThemeColor| {
        let [r, g, b, _] = c.to_array();
        Color::srgb_u8(r, g, b)
    };
    let running = runtime.is_some_and(|r| r.is_alive());
    // Matches `play_btn_click`: a global scene's camera counts, so the button
    // doesn't read as disabled while the click handler would accept it.
    let has_cam = !scene_cams.is_empty() || global_cam.is_some_and(|g| g.0);

    // `icon_name` is a phosphor glyph name (not localized); the label IS localized.
    let (icon_name, color, label_text) = if running {
        ("stop", tc(t.semantic.error), renzora::lang::t("common.stop"))
    } else if has_cam {
        ("play", tc(t.semantic.success), renzora::lang::t("common.play"))
    } else {
        ("play", tc(t.text.muted), renzora::lang::t("common.play"))
    };

    for mut icon in &mut icons {
        if icon.name != icon_name {
            icon.name = icon_name.to_string();
            icon.resolved = false; // force `apply_icons` to re-render the glyph
        }
        if icon.color != Some(color) {
            icon.color = Some(color);
            icon.resolved = false;
        }
    }
    // The label stays the primary text colour, like the Marketplace pill's; the
    // state colour is carried by the glyph and the fill. Muted only when there
    // is no scene camera, so a Play that would do nothing reads as unavailable.
    let label_color = if running || has_cam {
        rgb(text_primary())
    } else {
        rgb(text_muted())
    };
    for (mut text, mut tcolor) in &mut labels {
        if text.0 != label_text {
            text.0 = label_text.clone();
        }
        if tcolor.0 != label_color {
            tcolor.0 = label_color;
        }
    }
    // A tinted gradient so the control reads as a *button*. Derived from the
    // same state colour the icon uses, like the Marketplace chip, and dimmed
    // along with it when there's no scene camera to play through.
    for (mut bg, mut edge, interaction) in &mut fills {
        let hot = *interaction != UiInteraction::None;
        let want = chip_gradient(color, hot);
        if *bg != want {
            *bg = want;
        }
        let want = chip_border(color, hot);
        if *edge != want {
            *edge = want;
        }
    }
}
