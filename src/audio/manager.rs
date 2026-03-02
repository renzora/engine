//! Kira AudioManager wrapper resource
//!
//! Wraps the kira AudioManager and all mixer track handles.
//! Stored as a NonSend resource because cpal::Stream is not Send on all platforms.

use bevy::prelude::*;
use kira::{
    AudioManager, AudioManagerSettings, DefaultBackend,
    effect::panning_control::{PanningControlBuilder, PanningControlHandle},
    effect::reverb::ReverbBuilder,
    effect::delay::DelayBuilder,
    listener::{ListenerHandle, ListenerId},
    sound::static_sound::StaticSoundHandle,
    sound::streaming::StreamingSoundHandle,
    sound::FromFileError,
    sound::PlaybackState,
    sound::SoundData,
    track::{TrackBuilder, TrackHandle, SendTrackBuilder, SendTrackHandle, SpatialTrackBuilder, SpatialTrackHandle, SpatialTrackDistances},
    Decibels, Easing, Mix, Panning, Tween,
    PlaySoundError,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::audio::mixer::MixerState;
use crate::component_system::components::audio_emitter::RolloffType;

/// Convert linear amplitude (0.0+) to decibels for Kira 0.12.
/// Kira treats -60 dB as silence.
pub fn amplitude_to_db(amp: f64) -> f32 {
    if amp <= 0.0 {
        -60.0
    } else {
        (20.0 * amp.log10()) as f32
    }
}

/// Convert Bevy Vec3 (glam 0.30) to mint Vector3 for Kira (glam 0.32).
pub fn vec3_to_mint(v: Vec3) -> mint::Vector3<f32> {
    mint::Vector3 { x: v.x, y: v.y, z: v.z }
}

/// Convert Bevy Quat (glam 0.30) to mint Quaternion for Kira (glam 0.32).
pub fn quat_to_mint(q: Quat) -> mint::Quaternion<f32> {
    mint::Quaternion { v: mint::Vector3 { x: q.x, y: q.y, z: q.z }, s: q.w }
}

/// NonSend resource wrapping the kira AudioManager and all mixer tracks.
pub struct KiraAudioManager {
    pub manager: AudioManager<DefaultBackend>,

    /// Sub-tracks of master
    pub sfx_track: TrackHandle,
    pub music_track: TrackHandle,
    pub ambient_track: TrackHandle,

    /// Per-bus panning effect handles for mixer pan knobs
    pub sfx_pan: PanningControlHandle,
    pub music_pan: PanningControlHandle,
    pub ambient_pan: PanningControlHandle,

    /// User-created mixer bus tracks (parallel to MixerState::custom_buses)
    pub custom_tracks: Vec<(TrackHandle, PanningControlHandle)>,

    /// Per-entity active sound handles (from scripting or autoplay)
    pub active_sounds: HashMap<Entity, Vec<StaticSoundHandle>>,

    /// Current music handle (streaming)
    pub music_handle: Option<StreamingSoundHandle<FromFileError>>,

    /// Kira spatial listener (the "ears" in 3D space)
    pub listener: Option<ListenerHandle>,

    /// Per-entity spatial track handles (positioned emitters)
    pub spatial_tracks: HashMap<Entity, SpatialTrackHandle>,

    /// Global send tracks for reverb and delay effects
    pub reverb_send: SendTrackHandle,
    pub delay_send: SendTrackHandle,

    /// Per-entity sub-tracks with send routing (created when reverb_send/delay_send > 0)
    pub emitter_send_tracks: HashMap<Entity, TrackHandle>,

    /// Global master volume (0.0–1.0, linear amplitude)
    pub master_volume: f64,

    /// Project root path for resolving relative asset paths
    pub project_path: Option<PathBuf>,
}

impl KiraAudioManager {
    pub fn new() -> Self {
        let mut manager = AudioManager::<DefaultBackend>::new(AudioManagerSettings::default())
            .expect("[KiraAudio] Failed to create AudioManager");

        let mut sfx_builder = TrackBuilder::new();
        let sfx_pan = sfx_builder.add_effect(PanningControlBuilder::default());
        let sfx_track = manager
            .add_sub_track(sfx_builder)
            .expect("[KiraAudio] Failed to create sfx track");

        let mut music_builder = TrackBuilder::new();
        let music_pan = music_builder.add_effect(PanningControlBuilder::default());
        let music_track = manager
            .add_sub_track(music_builder)
            .expect("[KiraAudio] Failed to create music track");

        let mut ambient_builder = TrackBuilder::new();
        let ambient_pan = ambient_builder.add_effect(PanningControlBuilder::default());
        let ambient_track = manager
            .add_sub_track(ambient_builder)
            .expect("[KiraAudio] Failed to create ambient track");

        // Global send tracks: wet-only (mix=1.0) reverb and delay
        let reverb_send = manager
            .add_send_track(
                SendTrackBuilder::new()
                    .with_effect(ReverbBuilder::new().feedback(0.85).damping(0.3).stereo_width(1.0).mix(Mix(1.0)))
            )
            .expect("[KiraAudio] Failed to create reverb send track");

        let delay_send = manager
            .add_send_track(
                SendTrackBuilder::new()
                    .with_effect(DelayBuilder::new().delay_time(std::time::Duration::from_millis(375)).feedback(Decibels(-6.0)).mix(Mix(1.0)))
            )
            .expect("[KiraAudio] Failed to create delay send track");

        Self {
            manager,
            sfx_track,
            music_track,
            ambient_track,
            sfx_pan,
            music_pan,
            ambient_pan,
            custom_tracks: Vec::new(),
            active_sounds: HashMap::new(),
            music_handle: None,
            listener: None,
            spatial_tracks: HashMap::new(),
            reverb_send,
            delay_send,
            emitter_send_tracks: HashMap::new(),
            master_volume: 1.0,
            project_path: None,
        }
    }

    /// Play sound data on the named bus, falling back to SFX for unknown names.
    pub fn play_on_bus<D: SoundData>(
        &mut self,
        data: D,
        bus: &str,
        mixer: &MixerState,
    ) -> Result<D::Handle, PlaySoundError<D::Error>> {
        match bus {
            "Master" => self.manager.play(data),
            "Music" => self.music_track.play(data),
            "Ambient" => self.ambient_track.play(data),
            "Sfx" => self.sfx_track.play(data),
            name => {
                if let Some(idx) = mixer.custom_buses.iter().position(|(n, _)| n == name) {
                    if let Some((track, _)) = self.custom_tracks.get_mut(idx) {
                        return track.play(data);
                    }
                }
                self.sfx_track.play(data)
            }
        }
    }

    /// Resolve a relative asset path to an absolute path.
    pub fn resolve_path(&self, relative: &str) -> PathBuf {
        let p = Path::new(relative);
        if p.is_absolute() {
            return p.to_path_buf();
        }
        if let Some(proj) = &self.project_path {
            proj.join(relative)
        } else {
            p.to_path_buf()
        }
    }

    /// Track a sound handle for a given entity.
    pub fn track_sound(&mut self, entity: Entity, handle: StaticSoundHandle) {
        self.active_sounds.entry(entity).or_default().push(handle);
    }

    /// Remove finished (stopped) sound handles and drop empty entries.
    pub fn prune_finished(&mut self) {
        self.active_sounds.retain(|_, handles| {
            handles.retain(|h| h.state() != PlaybackState::Stopped);
            !handles.is_empty()
        });
    }

    /// Lazily create the Kira listener if it doesn't exist yet.
    /// Returns the ListenerId (Copy) for use when creating spatial tracks.
    pub fn ensure_listener(&mut self) -> ListenerId {
        if let Some(ref listener) = self.listener {
            return listener.id();
        }
        let origin = vec3_to_mint(Vec3::ZERO);
        let identity = quat_to_mint(Quat::IDENTITY);
        let handle = self.manager.add_listener(origin, identity)
            .expect("[KiraAudio] Failed to create listener");
        let id = handle.id();
        self.listener = Some(handle);
        id
    }

    /// Get or create a spatial track for the given entity as a child of the appropriate bus track.
    pub fn get_or_create_spatial_track(
        &mut self,
        entity: Entity,
        position: Vec3,
        bus: &str,
        min_dist: f32,
        max_dist: f32,
        rolloff: &RolloffType,
        mixer: &MixerState,
    ) -> Option<&mut SpatialTrackHandle> {
        if self.spatial_tracks.contains_key(&entity) {
            return self.spatial_tracks.get_mut(&entity);
        }

        let listener_id = self.ensure_listener();
        let mint_pos = vec3_to_mint(position);

        let attenuation = match rolloff {
            RolloffType::Logarithmic => Easing::OutPowi(2),
            RolloffType::Linear => Easing::Linear,
        };

        let builder = SpatialTrackBuilder::new()
            .distances(SpatialTrackDistances {
                min_distance: min_dist,
                max_distance: max_dist,
            })
            .attenuation_function(attenuation)
            .persist_until_sounds_finish(true);

        // Create the spatial track as a sub-track of the appropriate bus
        let result = match bus {
            "Music" => self.music_track.add_spatial_sub_track(listener_id, mint_pos, builder),
            "Ambient" => self.ambient_track.add_spatial_sub_track(listener_id, mint_pos, builder),
            "Master" => {
                // For Master, use SFX track as fallback parent
                self.sfx_track.add_spatial_sub_track(listener_id, mint_pos, builder)
            }
            name => {
                if let Some(idx) = mixer.custom_buses.iter().position(|(n, _)| n == name) {
                    if let Some((track, _)) = self.custom_tracks.get_mut(idx) {
                        match track.add_spatial_sub_track(listener_id, mint_pos, builder) {
                            Ok(handle) => Ok(handle),
                            Err(e) => Err(e),
                        }
                    } else {
                        self.sfx_track.add_spatial_sub_track(listener_id, mint_pos, builder)
                    }
                } else {
                    self.sfx_track.add_spatial_sub_track(listener_id, mint_pos, builder)
                }
            }
        };

        match result {
            Ok(handle) => {
                self.spatial_tracks.insert(entity, handle);
                self.spatial_tracks.get_mut(&entity)
            }
            Err(e) => {
                warn!("[KiraAudio] Failed to create spatial track for {:?}: {}", entity, e);
                None
            }
        }
    }

    /// Get or create a per-emitter sub-track with reverb/delay send routing.
    /// Returns the track to play sounds on. If both sends are zero, returns None
    /// (caller should use bus track directly).
    pub fn get_or_create_emitter_send_track(
        &mut self,
        entity: Entity,
        bus: &str,
        reverb_amount: f32,
        delay_amount: f32,
        mixer: &MixerState,
    ) -> Option<&mut TrackHandle> {
        if reverb_amount <= 0.001 && delay_amount <= 0.001 {
            return None;
        }

        if self.emitter_send_tracks.contains_key(&entity) {
            return self.emitter_send_tracks.get_mut(&entity);
        }

        // Build a sub-track of the appropriate bus with send routing
        let mut builder = TrackBuilder::new();
        if reverb_amount > 0.001 {
            let send_db = amplitude_to_db(reverb_amount as f64);
            builder = builder.with_send(&self.reverb_send, send_db);
        }
        if delay_amount > 0.001 {
            let send_db = amplitude_to_db(delay_amount as f64);
            builder = builder.with_send(&self.delay_send, send_db);
        }

        // Create as child of the appropriate bus track
        let result = match bus {
            "Music" => self.music_track.add_sub_track(builder),
            "Ambient" => self.ambient_track.add_sub_track(builder),
            "Sfx" => self.sfx_track.add_sub_track(builder),
            name => {
                if let Some(idx) = mixer.custom_buses.iter().position(|(n, _)| n == name) {
                    if let Some((track, _)) = self.custom_tracks.get_mut(idx) {
                        track.add_sub_track(builder)
                    } else {
                        self.sfx_track.add_sub_track(builder)
                    }
                } else {
                    self.sfx_track.add_sub_track(builder)
                }
            }
        };

        match result {
            Ok(track) => {
                self.emitter_send_tracks.insert(entity, track);
                self.emitter_send_tracks.get_mut(&entity)
            }
            Err(e) => {
                warn!("[KiraAudio] Failed to create emitter send track for {:?}: {}", entity, e);
                None
            }
        }
    }

    /// Stop all sounds for a specific entity and clean up its tracks.
    pub fn stop_entity_sounds(&mut self, entity: Entity) {
        if let Some(handles) = self.active_sounds.remove(&entity) {
            for mut handle in handles {
                let _ = handle.stop(Tween::default());
            }
        }
        self.spatial_tracks.remove(&entity);
        self.emitter_send_tracks.remove(&entity);
    }

    /// Stop all active scripting sounds immediately.
    pub fn stop_all_sounds(&mut self) {
        for (_, handles) in self.active_sounds.drain() {
            for mut handle in handles {
                let _ = handle.stop(Tween::default());
            }
        }
        // Drop all spatial tracks, emitter send tracks, and listener for full cleanup
        self.spatial_tracks.clear();
        self.emitter_send_tracks.clear();
        self.listener = None;
    }

    /// Stop music immediately or with a fade.
    pub fn stop_music(&mut self, fade_secs: f32) {
        if let Some(mut handle) = self.music_handle.take() {
            let tween = if fade_secs > 0.0 {
                Tween {
                    duration: std::time::Duration::from_secs_f32(fade_secs),
                    ..Default::default()
                }
            } else {
                Tween::default()
            };
            let _ = handle.stop(tween);
        }
    }
}
