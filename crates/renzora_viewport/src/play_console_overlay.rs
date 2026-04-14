//! Console log overlay rendered over the game view during play mode.
//!
//! Uses `bevy_ui` (not egui) because the editor's egui UI camera is disabled
//! during play mode. Text nodes target the game camera directly. Toggle with
//! the backtick (`) key.

use bevy::prelude::*;

use renzora::core::{PlayModeCamera, PlayModeState, console_log::LogLevel};
use renzora_console::state::ConsoleState;

const VISIBLE_ENTRIES: usize = 14;

#[derive(Resource)]
pub struct PlayConsoleOverlayState {
    pub visible: bool,
}

impl Default for PlayConsoleOverlayState {
    fn default() -> Self { Self { visible: true } }
}

/// Marker on the root overlay node so we can despawn/rebuild it cleanly.
#[derive(Component)]
pub struct PlayConsoleRoot;

/// Marker on the text child so we can update its contents each frame.
#[derive(Component)]
pub struct PlayConsoleText;

pub fn toggle_play_console_overlay(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<PlayConsoleOverlayState>,
    play_mode: Option<Res<PlayModeState>>,
) {
    let in_play = play_mode.as_ref().map_or(false, |pm| pm.is_in_play_mode());
    if !in_play { return; }
    if keyboard.just_pressed(KeyCode::Backquote) {
        state.visible = !state.visible;
    }
}

/// Spawn/despawn the overlay root based on play mode + visible toggle.
pub fn manage_play_console_overlay(
    mut commands: Commands,
    state: Res<PlayConsoleOverlayState>,
    play_mode: Option<Res<PlayModeState>>,
    mut existing: Query<(Entity, &mut Visibility), With<PlayConsoleRoot>>,
    play_cam: Query<Entity, With<PlayModeCamera>>,
) {
    let in_play = play_mode.as_ref().map_or(false, |pm| pm.is_in_play_mode());
    let should_show = in_play && state.visible;

    // If we have an existing overlay, just toggle its visibility — spawning
    // and despawning every play/stop cycle caused flicker.
    if let Ok((_, mut vis)) = existing.single_mut() {
        *vis = if should_show { Visibility::Inherited } else { Visibility::Hidden };
        return;
    }

    // No overlay yet: only spawn once a play-mode camera exists. Otherwise
    // the UiTargetCamera would dangle and the UI never renders.
    if !should_show { return; }
    let Ok(cam_entity) = play_cam.single() else { return };

    commands
        .spawn((
            Name::new("PlayConsoleOverlay"),
            PlayConsoleRoot,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(8.0),
                left: Val::Px(8.0),
                width: Val::Px(520.0),
                max_height: Val::Px(260.0),
                padding: UiRect::all(Val::Px(8.0)),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            GlobalZIndex(1000),
            bevy::ui::UiTargetCamera(cam_entity),
        ))
        .with_children(|parent| {
            parent.spawn((
                PlayConsoleText,
                Text::new(""),
                TextFont { font_size: 12.0, ..default() },
                TextColor(Color::srgb(0.85, 0.85, 0.85)),
            ));
        });
}

/// Refresh the text contents from the global log buffer.
pub fn update_play_console_text(
    state: Res<PlayConsoleOverlayState>,
    play_mode: Option<Res<PlayModeState>>,
    console: Option<Res<ConsoleState>>,
    mut text_q: Query<&mut Text, With<PlayConsoleText>>,
) {
    let in_play = play_mode.as_ref().map_or(false, |pm| pm.is_in_play_mode());
    if !in_play || !state.visible { return; }

    // Use the editor's `ConsoleState` as the source — it owns the persistent
    // log history. The raw `GLOBAL_LOG_BUFFER` is drained into ConsoleState
    // every frame, so reading from it directly gives only transient entries.
    let Some(console) = console else { return };

    let entries: Vec<String> = console.entries
        .iter()
        .rev()
        .take(VISIBLE_ENTRIES)
        .rev()
        .map(|e| {
            let prefix = match e.level {
                LogLevel::Info => "[info]",
                LogLevel::Success => "[ok]",
                LogLevel::Warning => "[warn]",
                LogLevel::Error => "[err]",
            };
            format!("{} {}: {}", prefix, e.category, e.message)
        })
        .collect();

    let joined = entries.join("\n");
    for mut text in &mut text_q {
        if text.0 != joined {
            text.0 = joined.clone();
        }
    }
}
