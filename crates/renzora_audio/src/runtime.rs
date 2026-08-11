//! What the engine tracks on behalf of the backend: loaded sounds, live voices,
//! and the per-frame conversation.
//!
//! The backend knows about handles and samples. It does not know about entities,
//! asset paths, transforms or the mixer panel — those are Bevy concepts and this
//! is where they are turned into the handles it does understand.

use std::collections::HashMap;

use bevy::prelude::*;

use renzora_plugin::audio::{BusState, ListenerState, UpdateRequest};
use renzora_plugin::host::PluginAudioBackend;

use crate::link::{AudioLink, SoundId, VoiceId};
use crate::mixer::MixerState;

/// Sounds the backend has decoded, by the path that produced them.
///
/// Cached because a footstep played forty times must be decoded once. The
/// backend holds the samples; this holds only the handle and what the engine
/// needs to know about it without asking.
#[derive(Resource, Default)]
pub struct SoundCache {
    by_path: HashMap<String, Loaded>,
    /// Paths that failed to load, so a missing asset is reported once rather
    /// than every frame something tries to play it. A scene with a broken
    /// `AudioPlayer` in an `on_update` would otherwise fill the log at 60 Hz.
    failed: HashMap<String, ()>,
}

#[derive(Clone, Copy)]
struct Loaded {
    sound: SoundId,
    duration: f64,
}

impl SoundCache {
    /// The handle for `path`, decoding it if this is the first time.
    ///
    /// Returns `None` when there is no backend, when the bytes could not be
    /// found, or when they would not decode — three different problems that a
    /// caller can do exactly one thing about, which is not play the sound.
    pub fn get_or_load(
        &mut self,
        link: &mut AudioLink,
        project: Option<&renzora::core::CurrentProject>,
        path: &str,
    ) -> Option<SoundId> {
        if let Some(loaded) = self.by_path.get(path) {
            return Some(loaded.sound);
        }
        if self.failed.contains_key(path) {
            return None;
        }
        if !link.is_active() {
            // Not recorded as a failure: the backend may load later, and a path
            // blacklisted while the plugin was missing would stay silent for the
            // rest of the session.
            return None;
        }

        let Some(bytes) = read_asset(project, path) else {
            warn!("[audio] could not read `{path}`");
            self.failed.insert(path.to_string(), ());
            return None;
        };
        let extension = std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();

        let sound = link.next_sound();
        match link.load_clip(sound, extension, &bytes) {
            Ok(Some(info)) => {
                self.by_path.insert(
                    path.to_string(),
                    Loaded {
                        sound,
                        duration: info.duration,
                    },
                );
                Some(sound)
            }
            Ok(None) => None,
            Err(e) => {
                warn!("[audio] `{path}`: {e}");
                self.failed.insert(path.to_string(), ());
                None
            }
        }
    }

    /// How long a loaded sound is, in seconds. `None` if it was never loaded —
    /// the timeline uses this to lay out clips and would rather draw nothing
    /// than guess a length.
    pub fn duration(&self, path: &str) -> Option<f64> {
        self.by_path.get(path).map(|l| l.duration)
    }

    /// Forget everything. For a project switch: the handles belong to a backend
    /// that is about to be told to drop them, and paths are project-relative so
    /// they mean something different now.
    pub fn clear(&mut self, link: &mut AudioLink) {
        for loaded in self.by_path.values() {
            link.unload_clip(loaded.sound);
        }
        self.by_path.clear();
        self.failed.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.by_path.is_empty()
    }
}

/// Read an asset's bytes the way the rest of the engine does.
///
/// The VFS loader first, so `.rpak`-bundled assets work in an exported game,
/// then a plain filesystem read for the editor and loose files. This is the
/// reason the backend never opens a path itself — only the engine knows which
/// of these two a given build is using.
fn read_asset(project: Option<&renzora::core::CurrentProject>, path: &str) -> Option<Vec<u8>> {
    if let Some(bytes) = renzora::core::load_asset_bytes(path) {
        return Some(bytes);
    }
    let resolved = match project {
        Some(p) => p.resolve_path(path),
        None => std::path::PathBuf::from(path),
    };
    std::fs::read(resolved).ok()
}

/// Voices that are currently sounding, and what they belong to.
///
/// Two maps rather than one because both directions are needed every frame: an
/// entity's voices when it is told to stop, and a voice's entity when the
/// backend reports it finished on its own.
#[derive(Resource, Default)]
pub struct ActiveVoices {
    by_entity: HashMap<Entity, Vec<VoiceId>>,
    owner: HashMap<VoiceId, Entity>,
}

impl ActiveVoices {
    pub fn insert(&mut self, entity: Entity, voice: VoiceId) {
        self.by_entity.entry(entity).or_default().push(voice);
        self.owner.insert(voice, entity);
    }

    /// Voices belonging to `entity`.
    pub fn of(&self, entity: Entity) -> &[VoiceId] {
        self.by_entity
            .get(&entity)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Drop one voice from the bookkeeping. Called when the backend says it
    /// finished, and when it is stopped explicitly.
    pub fn remove(&mut self, voice: VoiceId) {
        if let Some(entity) = self.owner.remove(&voice) {
            if let Some(list) = self.by_entity.get_mut(&entity) {
                list.retain(|v| *v != voice);
                // Empty entries would accumulate one per entity that ever played
                // a sound, which for a busy scene is a slow leak of exactly the
                // kind nobody notices.
                if list.is_empty() {
                    self.by_entity.remove(&entity);
                }
            }
        }
    }

    /// Forget an entity's voices without stopping them. For a despawn, where the
    /// stop has already been sent.
    pub fn forget(&mut self, entity: Entity) -> Vec<VoiceId> {
        let voices = self.by_entity.remove(&entity).unwrap_or_default();
        for voice in &voices {
            self.owner.remove(voice);
        }
        voices
    }

    pub fn is_empty(&self) -> bool {
        self.by_entity.is_empty()
    }

    pub fn len(&self) -> usize {
        self.owner.len()
    }

    /// Is this voice still sounding?
    ///
    /// The backend reports finishes by removing them from here, so "tracked" and
    /// "still playing" are the same question.
    pub fn contains(&self, voice: VoiceId) -> bool {
        self.owner.contains_key(&voice)
    }

    /// Every voice, for a global pause or stop.
    pub fn all(&self) -> Vec<VoiceId> {
        self.owner.keys().copied().collect()
    }

    /// Entities that own at least one voice.
    pub fn entities(&self) -> impl Iterator<Item = Entity> + '_ {
        self.by_entity.keys().copied()
    }

    /// `(entity, voices)` pairs, for the per-frame position sync.
    pub fn iter(&self) -> impl Iterator<Item = (Entity, &[VoiceId])> + '_ {
        self.by_entity.iter().map(|(e, v)| (*e, v.as_slice()))
    }
}

/// Mark entities that are currently sounding, so other panels can show it.
///
/// A component rather than a resource other crates read, because the hierarchy
/// asks the question per row and a marker is what an ECS query is for — and
/// because it keeps `renzora::AudioEmitting` the only thing they need to know
/// about audio at all.
pub fn mark_emitting_entities(
    voices: Res<ActiveVoices>,
    marked: Query<Entity, With<renzora::AudioEmitting>>,
    mut commands: Commands,
) {
    for entity in voices.entities() {
        if marked.get(entity).is_err() {
            // `try_insert`: a voice can outlive its entity by a frame, and
            // inserting onto a despawned entity is an error rather than a no-op.
            commands.entity(entity).try_insert(renzora::AudioEmitting);
        }
    }
    for entity in &marked {
        if voices.of(entity).is_empty() {
            commands.entity(entity).try_remove::<renzora::AudioEmitting>();
        }
    }
}

/// Take a backend the plugin host registered, and open its device.
///
/// Separate from the loader because registration and readiness are different
/// moments: the host records the descriptor during plugin init, when there is no
/// good place to open a sound card and nothing to report a failure to.
pub fn adopt_backend(
    registered: Option<Res<PluginAudioBackend>>,
    mut link: ResMut<AudioLink>,
    mut mixer: ResMut<MixerState>,
) {
    let Some(registered) = registered else { return };

    match (&registered.0, link.is_active()) {
        // A backend appeared.
        (Some(entry), false) => {
            link.adopt(entry.name.clone(), entry.state, entry.entry);
            match link.init() {
                Ok(Some(info)) => {
                    info!(
                        "[audio] backend `{}` on `{}` at {} Hz",
                        entry.name, info.device, info.sample_rate
                    );
                    // Touch the mixer so the board is pushed to the fresh
                    // backend on the same frame, rather than whenever someone
                    // next moves a fader.
                    mixer.set_changed();
                }
                Ok(None) => warn!("[audio] backend `{}` did not answer init", entry.name),
                Err(e) => {
                    error!("[audio] {e}");
                    link.release();
                }
            }
        }
        // Its plugin was unloaded — `entry` and `state` point into an image that
        // is about to be unmapped, so this is not merely tidy.
        (None, true) => {
            info!("[audio] backend released");
            link.release();
        }
        _ => {}
    }
}

/// Push the mixer's board to the backend when it actually differs.
///
/// Compared rather than gated on `is_changed()`, because the meters are written
/// into `MixerState` every frame and that marks it changed every frame — see
/// [`audio_update`]. Peak levels are not part of the board, so an equal snapshot
/// means there is nothing to send.
pub fn sync_mixer_to_backend(
    mixer: Res<MixerState>,
    mut link: ResMut<AudioLink>,
    mut sent: Local<Vec<BusState>>,
) {
    if !link.is_active() {
        return;
    }
    let current = board(&mixer);
    if *sent == current {
        return;
    }
    if let Err(e) = link.set_buses(&current) {
        warn!("[audio] could not send the bus graph: {e}");
        return;
    }
    *sent = current;
}

/// The mixer as the backend sees it: built-ins in their contractual order, then
/// the custom buses in mixer order.
///
/// Master first because the backend's own master is index 0 and the two lists
/// have to line up for meters to land on the right strip.
pub fn board(mixer: &MixerState) -> Vec<BusState> {
    let entry = |key: &str, strip: &crate::mixer::ChannelStrip| BusState {
        key: key.to_string(),
        gain: strip.volume as f32,
        pan: strip.panning as f32,
        muted: strip.muted,
        soloed: strip.soloed,
    };
    let mut out = vec![
        entry("Master", &mixer.master),
        entry("Sfx", &mixer.sfx),
        entry("Music", &mixer.music),
        entry("Ambient", &mixer.ambient),
    ];
    out.extend(
        mixer
            .custom_buses
            .iter()
            .map(|b| entry(&b.key, &b.strip)),
    );
    out
}

/// Which scene camera the ears default to: the one marked `DefaultCamera`, else
/// the first.
///
/// Deliberately the same rule `enter_play_mode` uses to choose the camera it
/// renders through, so "the listener" and "the viewpoint" are the same entity
/// without either side having to publish its choice. Scene cameras only — a UI
/// or render-target camera has no business being the ears, and picking by render
/// order would hand them to one.
fn pick_game_camera<'a>(
    cameras: impl Iterator<Item = (&'a GlobalTransform, bool)>,
) -> Option<&'a GlobalTransform> {
    let mut first = None;
    for (transform, is_default) in cameras {
        if is_default {
            return Some(transform);
        }
        if first.is_none() {
            first = Some(transform);
        }
    }
    first
}

/// The per-frame conversation: tell the backend what moved, and take back the
/// meters and the voices that ended.
///
/// One call rather than one per emitter. The boundary crossing is the expensive
/// part, and a scene with two hundred positioned sounds would otherwise make two
/// hundred of them a frame to move things a few centimetres.
pub fn audio_update(
    mut link: ResMut<AudioLink>,
    mut mixer: ResMut<MixerState>,
    mut voices: ResMut<ActiveVoices>,
    listener: Query<(&GlobalTransform, &crate::systems::AudioListener)>,
    editor_camera: Query<&GlobalTransform, With<renzora::core::EditorCamera>>,
    game_camera: Query<
        (&GlobalTransform, Option<&renzora::core::DefaultCamera>),
        With<renzora::core::SceneCamera>,
    >,
    play_mode: Option<Res<renzora::PlayModeState>>,
) {
    if !link.is_active() {
        return;
    }

    // Whose ears? While editing, the viewpoint you are moving is the *editor*
    // camera, so that is where sound has to be heard from — otherwise spatial
    // audio can only be auditioned by entering play mode, and flying around an
    // emitter in the viewport does nothing at all. That was reported as spatial
    // audio being broken, and it was the right complaint: an `AudioListener` on a
    // scene camera sits still while you fly past it.
    //
    // Otherwise: an explicit `AudioListener` if the scene has one, and the game
    // camera if it does not. The camera default is what makes the component
    // optional — a first-person or 2D game never needs one, and the state where
    // nothing has ears (positioned sounds panning by their distance from the
    // world origin, deaf to the camera, for the whole session) cannot happen.
    // The override still matters: in third person the ears belong on the
    // character, not four metres behind them, and a pulled-back strategy camera
    // would attenuate the whole scene to silence.
    let editing = play_mode
        .as_ref()
        .is_some_and(|pm| !pm.is_in_play_mode());
    let ears = |transform: &GlobalTransform| {
        let t = transform.compute_transform();
        ListenerState {
            position: t.translation.to_array(),
            // The right vector, not forward: the pan calculation only needs to
            // know which side a source is on, and deriving that from forward
            // means rebuilding it for every emitter.
            right: (t.rotation * Vec3::X).to_array(),
        }
    };
    let from_component = || {
        listener
            .iter()
            .find(|(_, l)| l.active)
            .map(|(transform, _)| ears(transform))
    };
    let from_game_camera =
        || pick_game_camera(game_camera.iter().map(|(t, d)| (t, d.is_some()))).map(ears);
    let listener_state = if editing {
        editor_camera
            .iter()
            .next()
            .map(ears)
            .or_else(from_component)
            .or_else(from_game_camera)
    } else {
        from_component().or_else(from_game_camera)
    };

    let request = UpdateRequest {
        listener: listener_state,
        // Emitter moves, gains, pitches and pause flags are pushed by the
        // systems that own the queries knowing about them; this call carries the
        // listener and collects the answers.
        ..Default::default()
    };

    let reply = match link.update(&request) {
        Ok(reply) => reply,
        Err(e) => {
            warn!("[audio] {e}");
            return;
        }
    };

    for voice in reply.finished {
        voices.remove(VoiceId(voice));
    }

    // Meters come back in the order `board` sent them, so the offsets are fixed.
    //
    // Written through normal change detection *on purpose*, even though levels
    // move every frame. The mixer panel's VU is a reactive binding that only
    // recomputes on frames where `MixerState` changed, so writing these behind
    // `bypass_change_detection` — which is what this did first, to avoid
    // re-syncing the board sixty times a second — froze every meter after the
    // first frame. `sync_mixer_to_backend` compares before it sends instead, so
    // the cost this was avoiding is gone.
    let peaks = reply.peaks;
    let mixer = mixer.as_mut();
    let set = |index: usize, strip: &mut crate::mixer::ChannelStrip| {
        strip.peak_level = peaks.get(index).copied().unwrap_or(0.0);
    };
    set(0, &mut mixer.master);
    set(1, &mut mixer.sfx);
    set(2, &mut mixer.music);
    set(3, &mut mixer.ambient);
    for (i, bus) in mixer.custom_buses.iter_mut().enumerate() {
        bus.strip.peak_level = peaks.get(4 + i).copied().unwrap_or(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mixer::ChannelStrip;

    /// The ears default to the same camera play mode renders through, so the
    /// two can never disagree about which viewpoint the scene is heard from.
    #[test]
    fn the_default_camera_is_preferred_over_the_first_one() {
        let first = GlobalTransform::from_xyz(1.0, 0.0, 0.0);
        let marked = GlobalTransform::from_xyz(2.0, 0.0, 0.0);
        let picked = pick_game_camera([(&first, false), (&marked, true)].into_iter());
        assert_eq!(picked.map(|t| t.translation().x), Some(2.0));
    }

    /// A scene whose cameras are all unmarked still has ears.
    #[test]
    fn with_no_default_camera_the_first_one_hears() {
        let a = GlobalTransform::from_xyz(1.0, 0.0, 0.0);
        let b = GlobalTransform::from_xyz(2.0, 0.0, 0.0);
        let picked = pick_game_camera([(&a, false), (&b, false)].into_iter());
        assert_eq!(picked.map(|t| t.translation().x), Some(1.0));
    }

    #[test]
    fn no_cameras_at_all_is_not_a_panic() {
        assert!(pick_game_camera(core::iter::empty()).is_none());
    }

    #[test]
    fn the_board_puts_master_first_and_custom_buses_last() {
        let mut mixer = MixerState::default();
        mixer.add_bus();
        mixer.rename_bus(0, "Footsteps");

        let board = board(&mixer);
        assert_eq!(board.len(), 5);
        assert_eq!(board[0].key, "Master");
        assert_eq!(board[1].key, "Sfx");
        assert_eq!(board[2].key, "Music");
        assert_eq!(board[3].key, "Ambient");
        // The *key*, not the display name — renaming must not move routing.
        assert_eq!(board[4].key, "Bus 1");
    }

    #[test]
    fn the_board_carries_strip_state() {
        let mut mixer = MixerState::default();
        mixer.music = ChannelStrip {
            volume: 0.25,
            panning: -0.5,
            muted: true,
            soloed: true,
            ..Default::default()
        };
        let board = board(&mixer);
        assert_eq!(board[2].gain, 0.25);
        assert_eq!(board[2].pan, -0.5);
        assert!(board[2].muted);
        assert!(board[2].soloed);
    }

    #[test]
    fn voices_are_reachable_from_both_directions() {
        let mut voices = ActiveVoices::default();
        let e = Entity::from_raw_u32(1).unwrap();
        voices.insert(e, VoiceId(10));
        voices.insert(e, VoiceId(11));

        assert_eq!(voices.of(e), [VoiceId(10), VoiceId(11)]);
        assert_eq!(voices.len(), 2);

        voices.remove(VoiceId(10));
        assert_eq!(voices.of(e), [VoiceId(11)]);
    }

    /// Empty entries would accumulate one per entity that ever played a sound.
    #[test]
    fn an_entity_with_no_voices_left_is_dropped_entirely() {
        let mut voices = ActiveVoices::default();
        let e = Entity::from_raw_u32(1).unwrap();
        voices.insert(e, VoiceId(1));
        voices.remove(VoiceId(1));
        assert!(voices.is_empty());
        assert_eq!(voices.of(e), []);
    }

    #[test]
    fn forgetting_an_entity_returns_its_voices_and_clears_both_maps() {
        let mut voices = ActiveVoices::default();
        let e = Entity::from_raw_u32(1).unwrap();
        voices.insert(e, VoiceId(1));
        voices.insert(e, VoiceId(2));

        assert_eq!(voices.forget(e), [VoiceId(1), VoiceId(2)]);
        assert!(voices.is_empty());
        assert_eq!(voices.len(), 0);
    }

    #[test]
    fn removing_a_voice_that_was_never_tracked_is_harmless() {
        let mut voices = ActiveVoices::default();
        voices.remove(VoiceId(99));
        assert!(voices.is_empty());
    }

    /// With no backend, loading must not blacklist the path — the plugin may
    /// arrive later, and a path poisoned meanwhile would stay silent forever.
    #[test]
    fn a_missing_backend_does_not_blacklist_a_path() {
        let mut cache = SoundCache::default();
        let mut link = AudioLink::default();
        assert_eq!(cache.get_or_load(&mut link, None, "audio/x.ogg"), None);
        assert!(cache.failed.is_empty());
        assert!(cache.is_empty());
    }

    #[test]
    fn a_duration_is_only_known_for_a_loaded_sound() {
        let cache = SoundCache::default();
        assert_eq!(cache.duration("audio/x.ogg"), None);
    }
}
