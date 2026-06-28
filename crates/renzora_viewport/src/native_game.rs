//! Bevy-native (ember) **Game** panel — the constrained play-mode view.
//!
//! Mirrors [`crate::native_viewport`]: it shows an off-screen render target via
//! an `ImageNode` and reports its on-screen rect so [`crate::resolve_game_panel`]
//! can size the target to the panel. The difference is *what* renders into that
//! target — the game camera, switched on only in play mode (see
//! [`crate::play_mode`]). When this panel is docked, pressing Play renders the
//! game here instead of taking over the whole window, so the editor chrome stays
//! up — the Scene viewport keeps its editor view while the Game panel shows the
//! running game (a Scene/Game split).

use std::sync::atomic::Ordering;

use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};
use bevy::window::PrimaryWindow;

use renzora::core::{PlayModeState, SceneCamera};
use renzora_ember::dock::FocusPanelRequest;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, bind_with};

use crate::{GamePanel, GamePanelResize};

/// Marks the game panel's content node so [`report_game_geometry`] can find its
/// on-screen rect (and the play-mode resolver can size the target to it).
#[derive(Component)]
struct NativeGamePanel;

/// Marks the in-panel **Play** button (the placeholder shown before play starts).
#[derive(Component)]
struct GamePlayButton;

pub fn register(app: &mut App) {
    use renzora_editor_framework::SplashState;
    // `scroll = false`: the game image fills the panel.
    app.register_panel_content("game", false, build_game_panel);
    app.add_systems(
        Update,
        (
            report_game_geometry,
            game_play_button_click,
            focus_game_tab_on_play,
        )
            .run_if(in_state(SplashState::Editor)),
    );
}

fn build_game_panel(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let content = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                position_type: PositionType::Relative,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                overflow: Overflow::clip(),
                ..default()
            },
            // Near-black so the panel reads as "off" before the first frame and
            // letterboxes any aspect mismatch unobtrusively.
            BackgroundColor(Color::srgb(0.02, 0.02, 0.03)),
            RelativeCursorPosition::default(),
            Interaction::default(),
            NativeGamePanel,
            Name::new("native-game-panel"),
        ))
        .id();

    let img = commands
        .spawn((
            ImageNode::default(),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            Name::new("native-game-image"),
        ))
        .id();
    bind_with(
        commands,
        img,
        |w| w.get_resource::<GamePanel>().and_then(|g| g.image.clone()),
        |w, e, handle: &Option<Handle<Image>>| {
            if let (Some(handle), Some(mut node)) = (handle, w.get_mut::<ImageNode>(e)) {
                node.image = handle.clone();
            }
        },
    );
    // Only show the live image while playing; otherwise the stale last frame
    // would linger behind the placeholder after Stop.
    bind_display(commands, img, |w| in_play(w));
    commands.entity(content).add_child(img);

    // Placeholder shown while not playing: a clickable Play button + a hint.
    let placeholder = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(12.0),
                ..default()
            },
            Name::new("game-panel-placeholder"),
        ))
        .id();
    bind_display(commands, placeholder, |w| !in_play(w));

    let play_green = (90u8, 200u8, 120u8);
    let btn = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::axes(Val::Px(16.0), Val::Px(9.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.35, 0.78, 0.47, 0.12)),
            BorderColor::all(Color::srgba(0.35, 0.78, 0.47, 0.5)),
            Interaction::default(),
            GamePlayButton,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("game-panel-play-btn"),
        ))
        .id();
    // Glyph + label live with `FocusPolicy::Pass` so the click lands on the
    // parent button (which carries `Interaction`) — same pattern as the top bar.
    let icon = icon_text(commands, &fonts.phosphor, "play", play_green, 18.0);
    commands.entity(icon).insert(bevy::ui::FocusPolicy::Pass);
    let label = commands
        .spawn((
            Text::new("Play"),
            ui_font(&fonts.ui, 14.0),
            TextColor(Color::srgb_u8(play_green.0, play_green.1, play_green.2)),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    commands.entity(btn).add_children(&[icon, label]);

    let hint = commands
        .spawn((
            Text::new("Run the game inside this panel"),
            ui_font(&fonts.ui, 12.0),
            TextColor(Color::srgba(1.0, 1.0, 1.0, 0.3)),
            Name::new("game-panel-hint"),
        ))
        .id();
    commands.entity(placeholder).add_children(&[btn, hint]);
    commands.entity(content).add_child(placeholder);

    content
}

/// Whether the editor is currently in play mode (Playing or Paused).
fn in_play(w: &World) -> bool {
    w.get_resource::<PlayModeState>()
        .map(|p| p.is_in_play_mode())
        .unwrap_or(false)
}

/// Click the in-panel **Play** button → request play (only from Editing, and
/// only if the scene has a camera to play). Mirrors the top-bar Play button.
fn game_play_button_click(
    btns: Query<&Interaction, (Changed<Interaction>, With<GamePlayButton>)>,
    play_mode: Option<ResMut<PlayModeState>>,
    scene_cams: Query<(), With<SceneCamera>>,
) {
    let Some(mut pm) = play_mode else {
        return;
    };
    let has_cam = !scene_cams.is_empty();
    for interaction in &btns {
        if *interaction == Interaction::Pressed && pm.is_editing() && has_cam {
            pm.request_play = true;
        }
    }
}

/// When play *starts*, bring the Game tab to the foreground (if it's docked but
/// hidden behind another tab) so the running game is actually visible — covers
/// every play trigger: the top-bar button, the Play shortcut, and the in-panel
/// button. Edge-triggered via a remembered previous state so it only fires on
/// the Editing→Playing transition, leaving the user free to switch tabs after.
fn focus_game_tab_on_play(
    play_mode: Option<Res<PlayModeState>>,
    game: Option<Res<GamePanel>>,
    mut focus: ResMut<FocusPanelRequest>,
    mut was_playing: Local<bool>,
) {
    let playing = play_mode.is_some_and(|p| p.is_in_play_mode());
    let present = game.is_some_and(|g| g.present);
    if playing && !*was_playing && present {
        focus.0 = Some("game".to_string());
    }
    *was_playing = playing;
}

/// Publish the game panel's on-screen rect + hover into [`GamePanelResize`]
/// (logical px) so [`crate::resolve_game_panel`] sizes the render image to the
/// panel. Same scale-invariant top-left derivation as the viewport's reporter.
fn report_game_geometry(
    panels: Query<(&ComputedNode, &RelativeCursorPosition), With<NativeGamePanel>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    req: Option<Res<GamePanelResize>>,
) {
    let Some(req) = req else {
        return;
    };
    let r = &req.0;
    let cursor = windows.iter().next().and_then(|w| w.cursor_position());
    for (cn, rcp) in &panels {
        let inv = cn.inverse_scale_factor();
        let size = cn.size() * inv; // logical px
        r.width.store(size.x.max(1.0) as u32, Ordering::Relaxed);
        r.height.store(size.y.max(1.0) as u32, Ordering::Relaxed);
        r.hovered.store(rcp.cursor_over, Ordering::Relaxed);
        if let (Some(cursor), Some(norm)) = (cursor, rcp.normalized) {
            let top_left = cursor - (norm + Vec2::splat(0.5)) * size;
            r.screen_x.store(top_left.x.to_bits(), Ordering::Relaxed);
            r.screen_y.store(top_left.y.to_bits(), Ordering::Relaxed);
        }
    }
}
