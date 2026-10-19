//! The top bar's Play control: one button that starts the game, and stops it.
//!
//! There used to be a caret beside it choosing where Play ran (viewport,
//! window, VR headset, or a "Scripts" simulate mode). Four answers to "where
//! does my game run" was three too many, and the in-editor ones let a slow
//! game slow the editor. The game now always runs as its own process; see
//! `renzora_viewport::play_mode`.

use bevy::prelude::*;

use renzora_ember::font::{glyph, ui_font};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::theme::{play_green, rgb};
use renzora_ember::UiInteraction;

/// The Play button in the top bar's left zone. A left margin sets it off from
/// the session actions before it. It sits in the top bar rather than the
/// viewport's tool strip because running the game is not a viewport action,
/// and this bar is on screen in every workspace.
pub(crate) fn build_play_group(commands: &mut Commands, font: &bevy::text::FontSource) -> Entity {
    let play = build_play_button(commands, font);
    let group = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                margin: UiRect::left(Val::Px(8.0)),
                ..default()
            },
            Name::new("play-group"),
        ))
        .id();
    commands.entity(group).add_child(play);
    group
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
fn build_play_button(commands: &mut Commands, font: &bevy::text::FontSource) -> Entity {
    let btn = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            TopBarPlayBtn,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("top-bar-play"),
        ))
        .id();
    let icon = glyph(commands, "play", play_green(), 13.0);
    commands
        .entity(icon)
        .insert((TopBarPlayIcon, PASS_THROUGH));
    let label = commands
        .spawn((
            Text::new(renzora::lang::t("common.play")),
            ui_font(font, 11.0),
            TextColor(rgb(play_green())),
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
    mut fills: Query<(&mut BackgroundColor, &UiInteraction), With<TopBarPlayBtn>>,
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
    for (mut text, mut tcolor) in &mut labels {
        if text.0 != label_text {
            text.0 = label_text.clone();
        }
        if tcolor.0 != color {
            tcolor.0 = color;
        }
    }
    // A tinted fill so the control reads as a *button*, not as green text on the
    // toolbar. Derived from the same state colour the icon and label use, and
    // dimmed along with them when there's no scene camera to play through.
    for (mut bg, interaction) in &mut fills {
        let alpha = match interaction {
            UiInteraction::Pressed => 0.34,
            UiInteraction::Hovered => 0.26,
            UiInteraction::None => 0.16,
        };
        let want = color.with_alpha(alpha);
        if bg.0 != want {
            bg.0 = want;
        }
    }
}
