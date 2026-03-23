//! Audio mixer state and Kira synchronization
//!
//! MixerState is a regular Bevy Resource that stores bus volumes, panning,
//! mute, and solo for the Master, SFX, Music, Ambient, and any user-created
//! custom buses.  A dedicated system syncs the state to Kira's TrackHandles
//! every frame when the resource changes.

use bevy::prelude::*;
use kira::{Panning, Tween};
use kira::effect::panning_control::PanningControlBuilder;
use super::manager::{KiraAudioManager, amplitude_to_db};

/// Per-channel strip state
#[derive(Clone, Debug)]
pub struct ChannelStrip {
    /// Linear amplitude 0.0 – 1.5  (1.0 = unity, ≈ +3.5 dB head-room)
    pub volume: f64,
    /// Pan position  -1.0 = hard left, 0.0 = centre, 1.0 = hard right
    pub panning: f64,
    pub muted: bool,
    pub soloed: bool,
    /// Current real-time peak amplitude (0.0 - 1.5) for VU meters
    pub peak_level: f32,
}

impl Default for ChannelStrip {
    fn default() -> Self {
        Self { volume: 1.0, panning: 0.0, muted: false, soloed: false, peak_level: 0.0 }
    }
}

impl ChannelStrip {
    /// Effective amplitude after applying mute / solo logic
    pub fn effective_volume(&self, any_solo: bool) -> f64 {
        if self.muted { return 0.0; }
        if any_solo && !self.soloed { return 0.0; }
        self.volume
    }
}

/// Mixer resource – the single source of truth for all bus parameters
#[derive(Resource)]
pub struct MixerState {
    pub master: ChannelStrip,
    pub sfx: ChannelStrip,
    pub music: ChannelStrip,
    pub ambient: ChannelStrip,
    /// User-created buses: (display name, strip state)
    pub custom_buses: Vec<(String, ChannelStrip)>,
    // ── UI transient state ────────────────────────────────────────────────
    pub adding_bus: bool,
    pub new_bus_name: String,
    /// Index of custom bus currently being renamed (None = not renaming)
    pub renaming_bus: Option<usize>,
    /// Buffer for the rename text input
    pub rename_buf: String,
    /// Drag-reorder state: source custom bus index being dragged
    pub dragging_bus: Option<usize>,
}

impl Default for MixerState {
    fn default() -> Self {
        Self {
            master: ChannelStrip::default(),
            sfx: ChannelStrip::default(),
            music: ChannelStrip::default(),
            ambient: ChannelStrip::default(),
            custom_buses: Vec::new(),
            adding_bus: false,
            new_bus_name: String::new(),
            renaming_bus: None,
            rename_buf: String::new(),
            dragging_bus: None,
        }
    }
}

/// System: sync MixerState ↔ Kira TrackHandles every frame
pub fn sync_mixer_to_kira(
    mixer: Res<MixerState>,
    audio: Option<NonSendMut<KiraAudioManager>>,
) {
    let Some(mut audio) = audio else { return };

    // ── Push state (Apply UI changes to Kira) ───────────────────────────
    if !mixer.is_changed() { return; }

    let any_solo = mixer.sfx.soloed
        || mixer.music.soloed
        || mixer.ambient.soloed
        || mixer.custom_buses.iter().any(|(_, s)| s.soloed);

    // ── Master via Kira main track ────────────────────────────────────────
    let master_amp = if mixer.master.muted { 0.0 } else { mixer.master.volume };
    let _ = audio.manager
        .main_track()
        .set_volume(amplitude_to_db(master_amp), Tween::default());
    audio.master_volume = master_amp;

    // ── Built-in sub-tracks ───────────────────────────────────────────────
    let sfx_eff = mixer.sfx.effective_volume(any_solo);
    let music_eff = mixer.music.effective_volume(any_solo);
    let ambient_eff = mixer.ambient.effective_volume(any_solo);

    let _ = audio.sfx_track.set_volume(amplitude_to_db(sfx_eff), Tween::default());
    audio.sfx_pan.set_panning(Panning(mixer.sfx.panning as f32), Tween::default());
    let _ = audio.music_track.set_volume(amplitude_to_db(music_eff), Tween::default());
    audio.music_pan.set_panning(Panning(mixer.music.panning as f32), Tween::default());
    let _ = audio.ambient_track.set_volume(amplitude_to_db(ambient_eff), Tween::default());
    audio.ambient_pan.set_panning(Panning(mixer.ambient.panning as f32), Tween::default());

    // ── Custom buses: create new tracks if needed ─────────────────────────
    {
        use kira::track::TrackBuilder;
        while audio.custom_tracks.len() < mixer.custom_buses.len() {
            let mut track_builder = TrackBuilder::new();
            let pan_handle = track_builder.add_effect(PanningControlBuilder::default());
            match audio.manager.add_sub_track(track_builder) {
                Ok(track) => {
                    audio.custom_tracks.push((track, pan_handle));
                },
                Err(e) => {
                    warn!("[Mixer] Failed to create custom track: {e}");
                    break;
                }
            }
        }
    }

    for (i, (_, strip)) in mixer.custom_buses.iter().enumerate() {
        if let Some((track, pan_handle)) = audio.custom_tracks.get_mut(i) {
            let eff = strip.effective_volume(any_solo);
            let _ = track.set_volume(amplitude_to_db(eff), Tween::default());
            pan_handle.set_panning(Panning(strip.panning as f32), Tween::default());
        }
    }
}
