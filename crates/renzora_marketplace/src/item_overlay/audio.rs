//! The audio preview: the track selector and the one ember audio player, plus
//! the native Kira bridge that actually makes sound.
//!
//! Everything below the widgets is `#[cfg(not(wasm32))]`: the Kira stack doesn't
//! compile for the browser, so on wasm the player renders and stays silent.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use crate::auth::marketplace::MediaItem;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::KeyedSnapshot;
use renzora_ember::reactive::Rx;
use renzora_ember::reactive::tracked::{bind_display, keyed_list};
use renzora_ember::theme::*;
use renzora_ember::widgets::{audio_player, tint};

use super::gallery::media_by_type;
use super::{empty_snapshot, section_label, AudioTrackBtn, HubAudioPlayer, ItemOverlay, HUE_STORE};

#[cfg(not(target_arch = "wasm32"))]
use crossbeam_channel::{unbounded, TryRecvError};
#[cfg(not(target_arch = "wasm32"))]
use renzora_audio::{decode::DecodedAudio, AudioLink};
#[cfg(not(target_arch = "wasm32"))]
use renzora_ember::widgets::AudioPlayer as EmberAudioPlayer;

#[cfg(not(target_arch = "wasm32"))]
use super::{AudioPlayback, EQ_BANDS, EQ_COLUMNS, PREVIEW_SECS};

/// The audio section: a header, a track selector (only for multi-track assets),
/// and a single ember [`audio_player`] the hub drives via Kira. The whole section
/// hides when the asset has no audio media.
pub(super) fn build_audio(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let wrap = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        })
        .id();
    bind_display(commands, wrap, |w| !media_by_type(w, "audio").is_empty());

    let label = section_label(commands, fonts, "Audio preview");

    // Track selector (rebuilt on selection — cheap; it's just labels).
    let selector = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            ..default()
        })
        .id();
    keyed_list(commands, selector, audio_selector_snapshot);

    // The single player, in its own keyed slot keyed only on "audio exists" so a
    // selection change never rebuilds (and tears down) the live player.
    let player_slot = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            ..default()
        })
        .id();
    keyed_list(commands, player_slot, audio_player_snapshot);

    commands
        .entity(wrap)
        .add_children(&[label, selector, player_slot]);
    wrap
}

/// A friendly label for an audio track: its file name, else `Track N`.
fn track_label(m: &MediaItem, index: usize) -> String {
    m.url
        .rsplit('/')
        .next()
        .map(|s| s.split('?').next().unwrap_or(s))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("Track {}", index + 1))
}

/// Keyed snapshot of the audio track selector — one row per track, highlighted
/// when selected. Empty (hidden) unless there's more than one track.
fn audio_selector_snapshot(world: &Rx) -> KeyedSnapshot {
    let tracks = media_by_type(world, "audio");
    if tracks.len() <= 1 {
        return empty_snapshot();
    }
    let sel = world
        .get_resource::<ItemOverlay>()
        .map(|s| s.audio_selected)
        .unwrap_or(0);
    use std::hash::{Hash, Hasher};
    let names: Vec<String> = tracks
        .iter()
        .enumerate()
        .map(|(i, m)| track_label(m, i))
        .collect();
    let items: Vec<(u64, u64)> = names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            i.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (n, i == sel).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| audio_track_row(c, f, &names[i], i, i == sel)),
    }
}

/// One clickable track-selector row.
fn audio_track_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    name: &str,
    index: usize,
    selected: bool,
) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(if selected {
                tint(HUE_STORE, 26)
            } else {
                Color::NONE
            }),
            Interaction::default(),
            AudioTrackBtn(index),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(name.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(if selected { text_primary() } else { text_muted() })),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(row).add_child(text);
    row
}

/// Keyed snapshot of the one audio player. A constant key/hash means it's built
/// exactly once when audio first appears and never rebuilt thereafter, so the
/// live Kira binding survives image/track selection.
fn audio_player_snapshot(world: &Rx) -> KeyedSnapshot {
    if media_by_type(world, "audio").is_empty() {
        return empty_snapshot();
    }
    KeyedSnapshot {
        items: vec![(0, 0)],
        build: Box::new(|c, f, _i| {
            let p = audio_player(c, f);
            c.entity(p).insert(HubAudioPlayer);
            p
        }),
    }
}

// ── Playback (native, driving the ember AudioPlayer via Kira) ────────────────

/// The audio-track URLs (the `/media` audio subset), in order.
#[cfg(not(target_arch = "wasm32"))]
fn audio_urls(state: &ItemOverlay) -> Vec<String> {
    state
        .media
        .iter()
        .filter(|m| m.media_type == "audio")
        .map(|m| m.url.clone())
        .collect()
}

/// Stop the live clip and clear the playback state. Explicit `stop()` is
/// required — dropping a Kira handle doesn't halt the sound.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn stop_audio_inner(audio: &mut AudioPlayback, link: &mut AudioLink) {
    if let Some(voice) = audio.voice.take() {
        link.stop(&renzora_audio::StopRequest {
            target: renzora_audio::StopTarget::Voice(voice.0),
            fade: 0.02,
        });
    }
    audio.position = 0.0;
    audio.track = None;
    audio.rx = None;
    audio.loading = false;
    audio.duration = 0.0;
    audio.spectrum.clear();
    audio.levels.clear();
}

/// A preview play request: the whole clip on Master, starting `at` seconds in.
#[cfg(not(target_arch = "wasm32"))]
fn play_request(
    voice: renzora_audio::VoiceId,
    sound: renzora_audio::SoundId,
    at: f64,
) -> renzora_audio::PlayRequest {
    renzora_audio::PlayRequest {
        voice: voice.0,
        clip: sound.0,
        // Master rather than a preview bus of its own: an audition should be
        // heard through the same board the game is, mute and solo included.
        bus: String::from("Master"),
        gain: 1.0,
        pan: 0.0,
        pitch: 1.0,
        looping: None,
        fade_in: 0.0,
        start: at,
        emitter: None,
        reverb_send: 0.0,
        delay_send: 0.0,
    }
}

/// The file extension of a URL, as a decoding hint.
#[cfg(not(target_arch = "wasm32"))]
fn extension_of(url: &str) -> String {
    url.rsplit('/')
        .next()
        .and_then(|name| name.rsplit_once('.'))
        .map(|(_, ext)| {
            // Trailing query strings are common on signed download URLs.
            ext.split(['?', '#']).next().unwrap_or(ext).to_ascii_lowercase()
        })
        .unwrap_or_default()
}

/// Kick off a background download of the clip bytes for `url`.
#[cfg(not(target_arch = "wasm32"))]
fn spawn_audio_download(audio: &mut AudioPlayback, url: &str) {
    let (tx, rx) = unbounded();
    audio.rx = Some(rx);
    audio.loading = true;
    let url = url.to_string();
    std::thread::spawn(move || {
        let _ = tx.send(crate::auth::marketplace::download_file(&url));
    });
}

/// Precompute a spectrogram over the first [`PREVIEW_SECS`] of a clip: for each
/// of [`EQ_COLUMNS`] time slices, the energy in [`EQ_BANDS`] log-spaced frequency
/// bands (via a Goertzel single-frequency filter per band — cheap, no FFT crate).
/// The live EQ reads the column under the playhead so the bars bounce with the
/// music instead of showing one static envelope.
#[cfg(not(target_arch = "wasm32"))]
fn compute_spectrogram(data: &DecodedAudio) -> Vec<Vec<f32>> {
    use std::f32::consts::PI;
    let total_frames = data.frames();
    let sr = data.sample_rate as f32;
    if total_frames == 0 || sr <= 0.0 {
        return Vec::new();
    }
    let cap = (((PREVIEW_SECS * sr) as usize).min(total_frames)).max(1);
    // Log-spaced band centers from 60 Hz up to just under Nyquist.
    let fmin = 60.0f32;
    let fmax = (sr * 0.45).clamp(fmin * 2.0, 14000.0);
    let centers: Vec<f32> = (0..EQ_BANDS)
        .map(|b| fmin * (fmax / fmin).powf(b as f32 / (EQ_BANDS.max(2) - 1) as f32))
        .collect();
    let win = 512usize.min(cap);
    let mut cols: Vec<Vec<f32>> = Vec::with_capacity(EQ_COLUMNS);
    for c in 0..EQ_COLUMNS {
        let center = ((c as f32 + 0.5) / EQ_COLUMNS as f32 * cap as f32) as usize;
        let start = center.saturating_sub(win / 2).min(cap.saturating_sub(win));
        let end = (start + win).min(cap);
        let mut col = vec![0.0f32; EQ_BANDS];
        for (bi, &f) in centers.iter().enumerate() {
            let coeff = 2.0 * (2.0 * PI * (f / sr)).cos();
            let (mut s1, mut s2) = (0.0f32, 0.0f32);
            for i in start..end {
                let [left, right] = data.frame(i);
                let x = (left + right) * 0.5;
                let s0 = x + coeff * s1 - s2;
                s2 = s1;
                s1 = s0;
            }
            col[bi] = (s1 * s1 + s2 * s2 - coeff * s1 * s2).max(0.0).sqrt();
        }
        cols.push(col);
    }
    // Global-normalize, then sqrt-lift so quiet bands stay visible.
    let maxv = cols.iter().flatten().copied().fold(0.0f32, f32::max);
    if maxv > 1e-6 {
        for col in &mut cols {
            for v in col {
                *v = (*v / maxv).sqrt().clamp(0.0, 1.0);
            }
        }
    }
    cols
}

/// Ease the EQ bar levels toward the spectrogram column under the playhead each
/// frame (fast attack, slower release), so the bars animate with the audio and
/// fall to zero when paused/stopped.
#[cfg(not(target_arch = "wasm32"))]
fn update_eq(audio: &mut AudioPlayback, playing: bool, position: f32) {
    if audio.levels.len() != EQ_BANDS {
        audio.levels = vec![0.0; EQ_BANDS];
    }
    let target: Vec<f32> = if playing && !audio.spectrum.is_empty() && audio.duration > 0.0 {
        let frac = (position / audio.duration).clamp(0.0, 1.0);
        let col = ((frac * audio.spectrum.len() as f32) as usize).min(audio.spectrum.len() - 1);
        audio.spectrum[col].clone()
    } else {
        vec![0.0; EQ_BANDS]
    };
    for (lvl, &t) in audio.levels.iter_mut().zip(target.iter()) {
        let rate = if t > *lvl { 0.6 } else { 0.18 };
        *lvl += (t - *lvl) * rate;
        // Snap tiny values to zero so idle (paused) frames stop re-baking.
        if lvl.abs() < 0.001 {
            *lvl = 0.0;
        }
    }
}

/// Bridge the on-screen ember [`AudioPlayer`](EmberAudioPlayer) to the engine's
/// Kira manager: read the widget's `playing` / `seek_to` intent, drive the one
/// live clip, and push back `position` / `duration` / `amps`. Only one clip plays
/// at a time.
#[cfg(not(target_arch = "wasm32"))]
pub(super) fn sync_audio(
    mut state: ResMut<ItemOverlay>,
    link: Option<ResMut<AudioLink>>,
    time: Res<Time>,
    mut players: Query<&mut EmberAudioPlayer, With<HubAudioPlayer>>,
) {
    let Ok(mut ap) = players.single_mut() else {
        return; // no audio player on screen
    };
    let Some(mut link) = link else { return };
    if !link.is_active() {
        return; // no audio backend loaded
    }
    let urls = audio_urls(&state);
    if urls.is_empty() {
        return;
    }
    let sel = state.audio_selected.min(urls.len() - 1);
    let cur_url = urls[sel].clone();

    // Selection moved away from the loaded track → stop it and reset the widget.
    if state.audio.track.is_some() && state.audio.track != Some(sel) {
        stop_audio_inner(&mut state.audio, &mut link);
        ap.playing = false;
        ap.position = 0.0;
        ap.duration = 0.0;
        ap.amps.clear();
        ap.seek_to = None;
    }

    // A finished download → decode, publish peaks/duration, and play it.
    if let Some(rx) = state.audio.rx.take() {
        match rx.try_recv() {
            Ok(Ok(bytes)) => {
                state.audio.loading = false;
                // Decoded twice, on purpose: once here to draw the spectrogram,
                // and once by the backend to play it. The alternative is shipping
                // whole decoded files back across the plugin boundary so the
                // editor can look at them.
                let extension = extension_of(&cur_url);
                match renzora_audio::decode::decode(bytes.clone(), &extension) {
                    Ok(data) => {
                        // Cap the shown/scrubbable duration at the preview length.
                        state.audio.duration = (data.duration() as f32).min(PREVIEW_SECS);
                        state.audio.spectrum = compute_spectrogram(&data);
                        state.audio.levels = vec![0.0; EQ_BANDS];
                        state.audio.position = 0.0;
                        match link.load_bytes(&extension, &bytes) {
                            Some(sound) => {
                                state.audio.sound = sound.0;
                                let voice = link.next_voice();
                                let request = play_request(voice, sound, 0.0);
                                if let Err(e) = link.play(&request) {
                                    state.error = Some(format!("Audio play failed: {e}"));
                                } else {
                                    state.audio.voice = Some(voice);
                                    state.audio.track = Some(sel);
                                    // Honour a pause requested while it loaded.
                                    if !ap.playing {
                                        link.set_paused(voice, true);
                                    }
                                }
                            }
                            None => state.error = Some(String::from("Audio decode failed")),
                        }
                    }
                    Err(e) => state.error = Some(format!("Audio decode failed: {e}")),
                }
            }
            Ok(Err(e)) => {
                state.audio.loading = false;
                state.error = Some(e);
            }
            Err(TryRecvError::Empty) => state.audio.rx = Some(rx),
            Err(TryRecvError::Disconnected) => state.audio.loading = false,
        }
    }

    // Play requested but nothing loaded/loading → start fetching the clip bytes.
    if ap.playing && state.audio.voice.is_none() && !state.audio.loading {
        spawn_audio_download(&mut state.audio, &cur_url);
    }

    // Apply intent to the live voice, and advance the playhead from wall time.
    let mut finished = false;
    let cap_dur = state.audio.duration;
    if let Some(voice) = state.audio.voice {
        // A seek is a restart at an offset: the boundary has no seek op, and
        // adding one to move a 30-second preview scrubber would be a poor trade
        // against replaying a clip the backend has already decoded and cached.
        if let Some(t) = ap.seek_to.take() {
            link.stop(&renzora_audio::StopRequest {
                target: renzora_audio::StopTarget::Voice(voice.0),
                fade: 0.0,
            });
            let fresh = link.next_voice();
            let sound = renzora_audio::SoundId(state.audio.sound);
            if link.play(&play_request(fresh, sound, t as f64)).is_ok() {
                state.audio.voice = Some(fresh);
                state.audio.position = t;
                if !ap.playing {
                    link.set_paused(fresh, true);
                }
            }
        } else if ap.playing != state.audio.was_playing {
            link.set_paused(voice, !ap.playing);
        }

        if ap.playing {
            state.audio.position += time.delta_secs();
        }
        ap.position = state.audio.position;
        // Enforce the preview cap: stop at the limit rather than letting the
        // whole track play.
        if cap_dur > 0.0 && ap.position >= cap_dur {
            link.stop(&renzora_audio::StopRequest {
                target: renzora_audio::StopTarget::Voice(voice.0),
                fade: 0.0,
            });
            finished = true;
        }
    }
    state.audio.was_playing = ap.playing;
    if finished {
        // Ran to the end (or hit the cap): back to paused-at-zero.
        state.audio.voice = None;
        state.audio.track = None;
        state.audio.position = 0.0;
        ap.playing = false;
        ap.position = 0.0;
    }

    // Publish backend truth to the widget (only when it actually changed).
    let d = state.audio.duration;
    if (ap.duration - d).abs() > f32::EPSILON {
        ap.duration = d;
    }
    // Live EQ: ease the bar levels toward the spectrum column under the playhead
    // and push them to the waveform each frame (they fall to zero when paused).
    update_eq(&mut state.audio, ap.playing, ap.position);
    if ap.amps != state.audio.levels {
        ap.amps = state.audio.levels.clone();
    }
}
