//! In-editor audio preview state

use bevy::prelude::*;
use kira::sound::static_sound::StaticSoundData;
use kira::Tween;

use crate::audio::manager::KiraAudioManager;
use crate::audio::mixer::MixerState;

/// Resource tracking the current editor audio preview (plays outside of play mode)
#[derive(Resource, Default)]
pub struct AudioPreviewState {
    pub handle: Option<kira::sound::static_sound::StaticSoundHandle>,
    pub previewing_entity: Option<Entity>,
    pub previewing_path: Option<String>,
    pub previewing_bus: Option<String>,
}

impl AudioPreviewState {
    pub fn play(
        &mut self,
        manager: &mut KiraAudioManager,
        path: &str,
        bus: &str,
        mixer: &MixerState,
        entity: Entity,
    ) {
        // Stop previous preview
        self.stop();

        let full_path = manager.resolve_path(path);
        if !full_path.exists() {
            warn!("[AudioPreview] File not found: {}", full_path.display());
            return;
        }

        match StaticSoundData::from_file(&full_path) {
            Ok(data) => {
                match manager.play_on_bus(data, bus, mixer) {
                    Ok(handle) => {
                        self.handle = Some(handle);
                        self.previewing_entity = Some(entity);
                        self.previewing_path = Some(path.to_string());
                        self.previewing_bus = Some(bus.to_string());
                        info!("[AudioPreview] Playing: {}", path);
                    }
                    Err(e) => {
                        warn!("[AudioPreview] Failed to play {}: {}", path, e);
                    }
                }
            }
            Err(e) => {
                warn!("[AudioPreview] Failed to load {}: {}", path, e);
            }
        }
    }

    pub fn stop(&mut self) {
        if let Some(mut handle) = self.handle.take() {
            let _ = handle.stop(Tween::default());
        }
        self.previewing_entity = None;
        self.previewing_path = None;
        self.previewing_bus = None;
    }

    pub fn is_playing_entity(&self, entity: Entity) -> bool {
        self.previewing_entity == Some(entity)
    }

    pub fn is_playing(&self) -> bool {
        self.handle.is_some()
    }
}
