//! Kira-based audio system for Renzora Engine
//!
//! Provides full control over audio playback via the kira crate:
//! - Mixer tracks (sfx, music, ambient)
//! - Per-instance handle control (pause, resume, volume, pitch tweens)
//! - Editor audio preview (plays outside of play mode)
//! - Scripting integration (AudioCommandQueue → KiraAudioManager)

pub mod manager;
pub mod mixer;
pub mod preview;
pub mod systems;

pub use manager::KiraAudioManager;
pub use mixer::MixerState;
pub use preview::AudioPreviewState;

use bevy::prelude::*;
use crate::scripting::ScriptingSet;

pub struct KiraPlugin;

impl Plugin for KiraPlugin {
    fn build(&self, app: &mut App) {
        app.insert_non_send_resource(KiraAudioManager::new())
            .insert_resource(AudioPreviewState::default())
            .insert_resource(MixerState::default())
            // Sync project path into audio manager
            .add_systems(Update, sync_project_path)
            // Command processing (replaces Bevy AudioPlayer systems)
            .add_systems(
                Update,
                (
                    systems::process_kira_commands,
                    systems::sync_spatial_audio.after(systems::process_kira_commands),
                )
                    .in_set(ScriptingSet::CommandProcessing),
            )
            // Sync mixer UI state to Kira track handles
            .add_systems(Update, mixer::sync_mixer_to_kira)
            // Preview runs unconditionally (outside play mode too)
            .add_systems(Update, systems::preview_audio_system)
            // Prune finished sound handles
            .add_systems(
                Update,
                systems::prune_finished_sounds.in_set(ScriptingSet::Cleanup),
            )
            // VU meter peak level estimation
            .add_systems(Update, systems::update_vu_meters)
            // Autoplay emitters on play mode enter
            .add_systems(
                Update,
                systems::autoplay_audio_emitters.in_set(ScriptingSet::PreScript),
            )
            // Cleanup on play mode exit
            .add_systems(
                Update,
                systems::cleanup_kira_on_stop.in_set(ScriptingSet::Cleanup),
            )
            // Stop sounds when emitter component is removed or entity despawned
            .add_systems(
                Update,
                systems::cleanup_removed_emitters.in_set(ScriptingSet::Cleanup),
            )
            // Load persisted mixer volumes on startup
            .add_systems(Startup, load_mixer_volumes)
            // Persist mixer volumes when changed
            .add_systems(Update, persist_mixer_volumes);
    }
}

/// Load a persisted ChannelStripConfig into a runtime ChannelStrip
fn load_strip(strip: &mut mixer::ChannelStrip, cfg: &crate::project::ChannelStripConfig) {
    strip.volume = cfg.volume;
    strip.panning = cfg.panning;
    strip.muted = cfg.muted;
    strip.soloed = cfg.soloed;
}

/// Save a runtime ChannelStrip into a ChannelStripConfig
fn save_strip(strip: &mixer::ChannelStrip) -> crate::project::ChannelStripConfig {
    crate::project::ChannelStripConfig {
        volume: strip.volume,
        panning: strip.panning,
        muted: strip.muted,
        soloed: strip.soloed,
    }
}

/// Check if a runtime ChannelStrip differs from its persisted config
fn strip_changed(strip: &mixer::ChannelStrip, cfg: &crate::project::ChannelStripConfig) -> bool {
    (strip.volume - cfg.volume).abs() > 0.001
        || (strip.panning - cfg.panning).abs() > 0.001
        || strip.muted != cfg.muted
        || strip.soloed != cfg.soloed
}

/// Load persisted mixer state from AppConfig on startup
fn load_mixer_volumes(
    config: Option<Res<crate::project::AppConfig>>,
    mut mixer: ResMut<MixerState>,
) {
    let Some(config) = config else { return };
    let vols = &config.mixer_volumes;

    // Load full strip state (new format)
    load_strip(&mut mixer.master, &vols.master_strip);
    load_strip(&mut mixer.sfx, &vols.sfx_strip);
    load_strip(&mut mixer.music, &vols.music_strip);
    load_strip(&mut mixer.ambient, &vols.ambient_strip);

    // If strip volumes are default but legacy volumes differ, use legacy (migration)
    if (vols.master_strip.volume - 1.0).abs() < 0.001 && (vols.master - 1.0).abs() > 0.001 {
        mixer.master.volume = vols.master;
    }
    if (vols.sfx_strip.volume - 1.0).abs() < 0.001 && (vols.sfx - 1.0).abs() > 0.001 {
        mixer.sfx.volume = vols.sfx;
    }
    if (vols.music_strip.volume - 1.0).abs() < 0.001 && (vols.music - 1.0).abs() > 0.001 {
        mixer.music.volume = vols.music;
    }
    if (vols.ambient_strip.volume - 1.0).abs() < 0.001 && (vols.ambient - 1.0).abs() > 0.001 {
        mixer.ambient.volume = vols.ambient;
    }

    // Load custom buses
    for (name, cfg) in &vols.custom_buses {
        let mut strip = mixer::ChannelStrip::default();
        load_strip(&mut strip, cfg);
        mixer.custom_buses.push((name.clone(), strip));
    }
}

/// Persist mixer state to AppConfig when changed
fn persist_mixer_volumes(
    mixer: Res<MixerState>,
    mut config: Option<ResMut<crate::project::AppConfig>>,
) {
    if !mixer.is_changed() { return; }
    let Some(ref mut config) = config else { return };
    let vols = &config.mixer_volumes;

    let needs_save = strip_changed(&mixer.master, &vols.master_strip)
        || strip_changed(&mixer.sfx, &vols.sfx_strip)
        || strip_changed(&mixer.music, &vols.music_strip)
        || strip_changed(&mixer.ambient, &vols.ambient_strip)
        || mixer.custom_buses.len() != vols.custom_buses.len()
        || mixer.custom_buses.iter().zip(vols.custom_buses.iter()).any(
            |((name, strip), (saved_name, saved_cfg))| name != saved_name || strip_changed(strip, saved_cfg)
        );

    if needs_save {
        let vols = &mut config.mixer_volumes;
        // Keep legacy fields in sync for backwards compatibility
        vols.master = mixer.master.volume;
        vols.sfx = mixer.sfx.volume;
        vols.music = mixer.music.volume;
        vols.ambient = mixer.ambient.volume;
        // Save full strip state
        vols.master_strip = save_strip(&mixer.master);
        vols.sfx_strip = save_strip(&mixer.sfx);
        vols.music_strip = save_strip(&mixer.music);
        vols.ambient_strip = save_strip(&mixer.ambient);
        // Save custom buses
        vols.custom_buses = mixer.custom_buses.iter()
            .map(|(name, strip)| (name.clone(), save_strip(strip)))
            .collect();
        let _ = config.save();
    }
}

/// Keep KiraAudioManager.project_path in sync with the current project
fn sync_project_path(
    project: Option<Res<crate::project::CurrentProject>>,
    audio: Option<NonSendMut<KiraAudioManager>>,
) {
    let Some(mut audio) = audio else { return };
    let new_path = project.as_ref().map(|p| p.path.clone());
    if audio.project_path != new_path {
        audio.project_path = new_path;
    }
}
