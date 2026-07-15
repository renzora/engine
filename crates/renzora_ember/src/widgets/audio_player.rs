//! Audio player — a reusable, backend-agnostic playback control surface.
//!
//! This widget is *only* the visuals: a play/pause button, a scrub track, a
//! time readout, and the [`waveform`] widget. It deliberately owns no audio
//! backend. All state lives in the [`AudioPlayer`] component, which the
//! **consumer** drives: it reads `playing` / `seek_to` (the user's intent this
//! widget writes) and writes `position` / `duration` / `amps` (the truth its
//! backend reports). Keeping playback in the consumer is what lets ember stay
//! free of any audio dependency while different consumers (the marketplace
//! preview, a future asset browser) wire the same widget to different backends.

use bevy::prelude::*;
use bevy::ui::{FocusPolicy, RelativeCursorPosition};
use bevy::window::SystemCursorIcon;

use crate::cursor_icon::HoverCursor;
use crate::font::{icon_glyph, icon_text, ui_font, EmberFonts};
use crate::theme::*;

// The waveform widget lives in a sibling module; reuse its `WaveData` holder so
// this widget can re-feed amplitudes into the same GPU-painted envelope.
use super::waveform::{waveform, WaveData};

/// The visual + interaction state of an audio player. The consumer owns the
/// meaning of every field:
/// - `playing` / `seek_to` are *requests* this widget raises from user input
///   (the consumer applies them to its backend and clears `seek_to`).
/// - `position` / `duration` (seconds) and `amps` (0..1 envelope) are *truth*
///   the consumer pushes in each frame; the widget only renders them.
#[derive(Component, Default)]
pub struct AudioPlayer {
    pub playing: bool,
    pub position: f32,
    pub duration: f32,
    pub seek_to: Option<f32>,
    pub amps: Vec<f32>,
}

/// Child-entity refs the apply system writes to, plus the last amps we pushed
/// into the waveform (so we only re-feed it when they actually change — the
/// consumer typically sets `amps` once but `position` every frame).
#[derive(Component)]
pub(crate) struct AudioPlayerRefs {
    play_icon: Entity,
    fill: Entity,
    time_label: Entity,
    wave_root: Entity,
    synced_amps: Vec<f32>,
    /// Last progress fraction pushed into the waveform, so we only re-bake its
    /// material when the playhead actually moves (not every idle frame).
    synced_progress: f32,
}

/// The play/pause button, carrying the player root it toggles (so the click
/// system doesn't have to walk the hierarchy).
#[derive(Component)]
pub(crate) struct AudioPlayBtn(Entity);

/// The scrub track, carrying the player root it seeks.
#[derive(Component)]
pub(crate) struct AudioScrubTrack(Entity);

/// Build an audio player and return its root (which carries [`AudioPlayer`]).
///
/// Layout is a small column: a controls row (play button, a flex-grow scrub
/// track, a `m:ss / m:ss` time label) above a full-width waveform. The waveform
/// starts empty and only draws once the consumer supplies `amps`.
pub fn audio_player(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // Reserve the root id up-front so the button/track can point back at it.
    let root = commands.spawn_empty().id();

    // Play/pause button (glyph swaps in the apply system).
    let play_icon = icon_text(commands, &fonts.phosphor, "play", on_accent(), 12.0);
    commands.entity(play_icon).insert(FocusPolicy::Pass);
    let play_btn = commands
        .spawn((
            Node {
                width: Val::Px(28.0),
                height: Val::Px(28.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(14.0)),
                ..default()
            },
            BackgroundColor(rgb(accent())),
            Interaction::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            AudioPlayBtn(root),
            Name::new("audio-player-play"),
        ))
        .id();
    commands.entity(play_btn).add_child(play_icon);

    // Scrub track: a thin bar with a fill sub-node sized to position/duration.
    let fill = commands
        .spawn((
            Node {
                width: Val::Percent(0.0),
                height: Val::Percent(100.0),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(accent())),
            FocusPolicy::Pass,
            Name::new("audio-player-fill"),
        ))
        .id();
    let track = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                height: Val::Px(6.0),
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            Interaction::default(),
            RelativeCursorPosition::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            AudioScrubTrack(root),
            Name::new("audio-player-track"),
        ))
        .id();
    commands.entity(track).add_child(fill);

    // Time readout ("m:ss / m:ss").
    let time_label = commands
        .spawn((
            Text::new("0:00 / 0:00"),
            ui_font(&fonts.ui, 10.5),
            TextColor(rgb(text_muted())),
            Name::new("audio-player-time"),
        ))
        .id();

    let controls = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    commands.entity(controls).add_children(&[play_btn, track, time_label]);

    // Waveform (empty until the consumer feeds amps). Re-style to full width /
    // shorter height so it sits under the controls rather than at the widget's
    // default fixed size.
    let wave = waveform(commands, &[]);
    commands.entity(wave).insert(Node {
        width: Val::Percent(100.0),
        height: Val::Px(48.0),
        position_type: PositionType::Relative,
        overflow: Overflow::clip(),
        border: UiRect::all(Val::Px(1.0)),
        border_radius: BorderRadius::all(Val::Px(6.0)),
        ..default()
    });

    commands.entity(root).insert((
        Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            ..default()
        },
        AudioPlayer::default(),
        AudioPlayerRefs {
            play_icon,
            fill,
            time_label,
            wave_root: wave,
            synced_amps: Vec::new(),
            synced_progress: -1.0,
        },
        Name::new("audio-player"),
    ));
    commands.entity(root).add_children(&[controls, wave]);
    root
}

/// Play button click → toggle `AudioPlayer.playing` (a request the consumer acts on).
pub(crate) fn audio_player_play_click(
    q: Query<(&Interaction, &AudioPlayBtn), Changed<Interaction>>,
    mut players: Query<&mut AudioPlayer>,
) {
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed {
            if let Ok(mut p) = players.get_mut(btn.0) {
                p.playing = !p.playing;
            }
        }
    }
}

/// Click/drag the scrub track → set `seek_to` from the cursor's x over the track.
/// No `Changed` filter so a held drag keeps seeking, like the slider widget.
pub(crate) fn audio_player_scrub(
    tracks: Query<(&Interaction, &RelativeCursorPosition, &AudioScrubTrack)>,
    mut players: Query<&mut AudioPlayer>,
) {
    for (interaction, rcp, track) in &tracks {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(n) = rcp.normalized else {
            continue;
        };
        // `normalized` is centered (-0.5..0.5); shift to a 0..1 fraction.
        let frac = (n.x + 0.5).clamp(0.0, 1.0);
        if let Ok(mut p) = players.get_mut(track.0) {
            let dur = p.duration;
            if dur > 0.0 {
                p.seek_to = Some(frac * dur);
                // Reflect the scrub immediately so the fill doesn't lag a frame
                // behind the backend reporting the new position.
                p.position = frac * dur;
            }
        }
    }
}

/// Each frame: mirror the consumer-owned state onto the visuals — fill width,
/// play/pause glyph, time label, and (only when they change) the waveform amps.
pub(crate) fn audio_player_apply(
    mut players: Query<(&AudioPlayer, &mut AudioPlayerRefs)>,
    mut nodes: Query<&mut Node>,
    mut texts: Query<&mut Text>,
    children: Query<&Children>,
    mut waves: Query<&mut WaveData>,
) {
    for (player, mut refs) in &mut players {
        let frac = if player.duration > 0.0 {
            (player.position / player.duration).clamp(0.0, 1.0)
        } else {
            0.0
        };
        if let Ok(mut n) = nodes.get_mut(refs.fill) {
            let w = Val::Percent(frac * 100.0);
            if n.width != w {
                n.width = w;
            }
        }

        if let Ok(mut t) = texts.get_mut(refs.play_icon) {
            if let Some(ch) = icon_glyph(if player.playing { "pause" } else { "play" }) {
                let s = ch.to_string();
                if t.0 != s {
                    t.0 = s;
                }
            }
        }

        if let Ok(mut t) = texts.get_mut(refs.time_label) {
            let label = format!("{} / {}", fmt_time(player.position), fmt_time(player.duration));
            if t.0 != label {
                t.0 = label;
            }
        }

        // Feed the waveform: amplitudes only when they change (once per clip),
        // but progress whenever the playhead moves so the played/unplayed sweep
        // animates during playback. Both go through `WaveData`, whose `Changed`
        // re-bakes the material.
        let need_amps = refs.synced_amps != player.amps;
        let need_prog = (refs.synced_progress - frac).abs() > 0.0005;
        if need_amps || need_prog {
            let ds = if need_amps {
                downsample(&player.amps, 32)
            } else {
                Vec::new()
            };
            if let Ok(kids) = children.get(refs.wave_root) {
                for &k in kids {
                    if let Ok(mut wd) = waves.get_mut(k) {
                        if need_amps {
                            wd.set_amps(ds.clone());
                        }
                        if need_prog {
                            wd.set_progress(frac);
                        }
                    }
                }
            }
            if need_amps {
                refs.synced_amps = player.amps.clone();
            }
            if need_prog {
                refs.synced_progress = frac;
            }
        }
    }
}

/// Format seconds as `m:ss`.
fn fmt_time(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Downsample an amplitude envelope to at most `buckets` samples (peak |value|
/// per bucket, clamped to 0..1). The waveform material caps at 32 samples, so
/// the consumer's raw peaks must be reduced before they're handed over.
fn downsample(amps: &[f32], buckets: usize) -> Vec<f32> {
    if amps.is_empty() || buckets == 0 {
        return Vec::new();
    }
    if amps.len() <= buckets {
        return amps.iter().map(|a| a.abs().clamp(0.0, 1.0)).collect();
    }
    let per = amps.len() as f32 / buckets as f32;
    (0..buckets)
        .map(|b| {
            let lo = (b as f32 * per) as usize;
            let hi = (((b + 1) as f32 * per) as usize).min(amps.len()).max(lo + 1);
            amps[lo..hi]
                .iter()
                .fold(0.0f32, |m, &v| m.max(v.abs()))
                .clamp(0.0, 1.0)
        })
        .collect()
}
