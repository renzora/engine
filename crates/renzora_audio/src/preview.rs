//! In-editor audio preview — auditioning a clip outside play mode.

use bevy::prelude::*;

use renzora_plugin::audio::{PlayRequest, StopRequest, StopTarget};

use crate::link::{AudioLink, VoiceId};
use crate::runtime::{ActiveVoices, SoundCache};

/// The clip currently being auditioned, if any.
#[derive(Resource, Default)]
pub struct AudioPreviewState {
    /// The voice, so it can be stopped and so its end can be noticed.
    pub voice: Option<VoiceId>,
    pub previewing_entity: Option<Entity>,
    pub previewing_path: Option<String>,
    pub previewing_bus: Option<String>,
}

impl AudioPreviewState {
    /// Start auditioning `path`, replacing whatever was playing.
    ///
    /// The preview is a voice like any other, which is what lets the mixer meter,
    /// mute and solo it exactly as it does game audio. Auditioning through a
    /// separate path is how you end up shipping a mix that only sounded right in
    /// the editor.
    #[allow(clippy::too_many_arguments)]
    pub fn play(
        &mut self,
        link: &mut AudioLink,
        cache: &mut SoundCache,
        voices: &mut ActiveVoices,
        project: Option<&renzora::core::CurrentProject>,
        path: &str,
        bus: &str,
        entity: Entity,
    ) {
        self.stop(link);

        let Some(sound) = cache.get_or_load(link, project, path) else {
            return;
        };
        let voice = link.next_voice();
        let request = PlayRequest {
            voice: voice.0,
            clip: sound.0,
            bus: bus.to_string(),
            gain: 1.0,
            pan: 0.0,
            pitch: 1.0,
            looping: None,
            fade_in: 0.0,
            start: 0.0,
            emitter: None,
            reverb_send: 0.0,
            delay_send: 0.0,
        };
        if let Err(e) = link.play(&request) {
            warn!("[audio] preview of `{path}` failed: {e}");
            return;
        }
        // Tracked against the previewed entity, so a despawn cleans it up like
        // anything else.
        voices.insert(entity, voice);
        self.voice = Some(voice);
        self.previewing_entity = Some(entity);
        self.previewing_path = Some(path.to_string());
        self.previewing_bus = Some(bus.to_string());
    }

    /// Stop the preview. A short fade rather than a cut, because a stop
    /// mid-waveform is a click.
    pub fn stop(&mut self, link: &mut AudioLink) {
        if let Some(voice) = self.voice.take() {
            link.stop(&StopRequest {
                target: StopTarget::Voice(voice.0),
                fade: 0.02,
            });
        }
        self.clear();
    }

    /// Forget what was being previewed without stopping anything — for the case
    /// where the voice has already ended on its own.
    pub fn clear(&mut self) {
        self.voice = None;
        self.previewing_entity = None;
        self.previewing_path = None;
        self.previewing_bus = None;
    }

    pub fn is_playing_entity(&self, entity: Entity) -> bool {
        self.previewing_entity == Some(entity)
    }

    pub fn is_playing(&self) -> bool {
        self.voice.is_some()
    }
}
