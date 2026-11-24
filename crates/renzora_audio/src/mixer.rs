//! The mixer board: bus volumes, panning, mute, solo and colour for the four
//! built-ins and any user-created buses, plus its persistence.
//!
//! Pure state. Pushing it at whatever is making sound is
//! [`crate::runtime::sync_mixer_to_backend`]'s job — this module has no idea a
//! backend exists, which is what lets the same board drive a native mixer, a
//! WebAudio one, or nothing at all.

use bevy::prelude::*;
use renzora_audio_api::settings::{self, AudioConfig};
use renzora_project_api::{CurrentProject, EditorSession};

/// Per-channel strip state
#[derive(Clone, Debug)]
pub struct ChannelStrip {
    /// Linear amplitude 0.0 - 1.5 (1.0 = unity, ~+3.5 dB head-room)
    pub volume: f64,
    /// Pan position -1.0 = hard left, 0.0 = centre, 1.0 = hard right
    pub panning: f64,
    pub muted: bool,
    pub soloed: bool,
    /// Current real-time peak amplitude (0.0 - 1.5) for VU meters
    pub peak_level: f32,
    /// cpal input device name. `Some` ⇒ a live mic capture stream is opened
    /// on this bus; samples mix into the bus track exactly like a played
    /// sound, so volume / pan / mute / solo all apply normally.
    pub input_device: Option<String>,
    /// cpal output device name. Reserved for future per-bus device routing
    /// (currently unused by the audio pipeline; the field is here so the
    /// mixer panel can carry the value while the routing side is built out).
    pub output_device: Option<String>,
    /// Strip tint, RGB 0–255. Purely cosmetic — nothing in the audio path reads
    /// it. It lives on the strip rather than in the panel so the colour survives
    /// the mixer's keyed-list rebuilds (a rename or a reorder respawns the whole
    /// strip) and so any other panel showing a bus can match its colour.
    pub color: [u8; 3],
    /// Routing key of the bus this one feeds; `None` feeds Master. Ignored on
    /// Master itself, which feeds the device.
    pub output: Option<String>,
    /// Post-fader send to the shared reverb, 0..1.
    pub reverb_send: f32,
    /// Post-fader send to the shared delay, 0..1.
    pub delay_send: f32,
}

/// The built-in routing keys and the strip palette. Defined in the contract
/// beside the settings helpers, because the default board they describe is also
/// what a script setting a volume on an empty `[audio]` section writes out, and
/// two copies of it could disagree about Master's colour.
pub use renzora_audio_api::settings::{BUILTIN_BUSES, BUS_COLORS};

impl Default for ChannelStrip {
    fn default() -> Self {
        Self {
            volume: 1.0,
            panning: 0.0,
            muted: false,
            soloed: false,
            peak_level: 0.0,
            input_device: None,
            output_device: None,
            color: [130, 130, 140],
            output: None,
            reverb_send: 0.0,
            delay_send: 0.0,
        }
    }
}

impl ChannelStrip {
    /// Effective amplitude after applying mute / solo logic
    pub fn effective_volume(&self, any_solo: bool) -> f64 {
        if self.muted {
            return 0.0;
        }
        if any_solo && !self.soloed {
            return 0.0;
        }
        self.volume
    }
}

/// One user-created bus: its permanent routing key, its editable display name,
/// and its strip state.
///
/// The key/name split is the whole reason this is a struct rather than the
/// `(String, ChannelStrip)` tuple it used to be — see [`renzora_project_api::BusConfig`] for
/// why a bus needs both. In short: the key is what an `AudioPlayer` stores and
/// what playback matches, so it must never change; the name is what the mixer
/// shows, so it must be free to.
#[derive(Clone, Debug)]
pub struct Bus {
    /// Permanent routing key, unique across every bus including the built-ins.
    pub key: String,
    /// Display name. Starts equal to `key` and diverges when the user renames.
    pub name: String,
    pub strip: ChannelStrip,
}

/// Mixer resource - the single source of truth for all bus parameters
///
/// `Clone` so a panel can snapshot the board to build from: the mixer's
/// Routing page builds its menus from the whole board after the world borrow
/// that read it has ended.
#[derive(Resource, Clone)]
pub struct MixerState {
    pub master: ChannelStrip,
    pub sfx: ChannelStrip,
    pub music: ChannelStrip,
    pub ambient: ChannelStrip,
    /// User-created buses, in mixer order.
    pub custom_buses: Vec<Bus>,
}

/// Hand-written (rather than derived) so the four built-in buses start on
/// distinct colours — colour-coding is only useful if the default board is
/// already colour-coded.
impl Default for MixerState {
    fn default() -> Self {
        let tinted = |color: [u8; 3]| ChannelStrip {
            color,
            ..Default::default()
        };
        Self {
            master: tinted(settings::MASTER_COLOR),
            sfx: tinted(BUS_COLORS[1]),     // amber
            music: tinted(BUS_COLORS[5]),   // sky
            ambient: tinted(BUS_COLORS[4]), // teal
            custom_buses: Vec::new(),
        }
    }
}

impl MixerState {
    /// True when `key` is already spoken for, by a built-in or a custom bus.
    /// Case-insensitive: two keys differing only in case would be ambiguous to
    /// anyone typing one into an `AudioPlayer` or a script.
    pub fn bus_key_taken(&self, key: &str) -> bool {
        let key = key.trim();
        BUILTIN_BUSES.iter().any(|b| b.eq_ignore_ascii_case(key))
            || self
                .custom_buses
                .iter()
                .any(|b| b.key.eq_ignore_ascii_case(key))
    }

    /// The custom bus with this key, if any.
    pub fn bus(&self, key: &str) -> Option<&Bus> {
        self.custom_buses.iter().find(|b| b.key == key)
    }

    /// Index of the custom bus with this key. The audio backend keeps its track
    /// list parallel to `custom_buses`, so a positional lookup is what turns a
    /// key on an `AudioPlayer` into a track to play on.
    pub fn bus_index(&self, key: &str) -> Option<usize> {
        self.custom_buses.iter().position(|b| b.key == key)
    }

    /// Append a custom bus with a fresh key ("Bus 1", "Bus 2", …), the same
    /// display name, and the next palette colour; returns its index.
    ///
    /// Naming happens here rather than in the panel so "add a bus" is a single
    /// click with an immediate, visible result — the name is one double-click
    /// away from being changed, which is a cheaper thing to ask of the user than
    /// a form they must fill in before anything appears.
    pub fn add_bus(&mut self) -> usize {
        self.add_bus_keyed(None)
    }

    /// [`Self::add_bus`] with a caller-chosen key, uniquified if taken.
    ///
    /// The `Some` path is what a runtime caller uses — a script opening a
    /// channel per player wants to choose the key it will later route to, not
    /// discover one. Returns the index either way; read the key back off the
    /// bus, since it may have been uniquified.
    pub fn add_bus_keyed(&mut self, key: Option<&str>) -> usize {
        let key = match key {
            Some(k) => self.unique_key(k.trim()),
            None => {
                let mut n = self.custom_buses.len() + 1;
                loop {
                    let candidate = format!("Bus {n}");
                    if !self.bus_key_taken(&candidate) {
                        break candidate;
                    }
                    n += 1;
                }
            }
        };
        let color = BUS_COLORS[self.custom_buses.len() % BUS_COLORS.len()];
        self.custom_buses.push(Bus {
            name: key.clone(),
            key,
            strip: ChannelStrip {
                color,
                ..Default::default()
            },
        });
        self.custom_buses.len() - 1
    }

    /// `base`, or `base 2` / `base 3` / … if it is taken. Falls back to the
    /// generated form for an empty base so a key is never the empty string —
    /// which would match every `AudioPlayer` whose bus field was left unset.
    fn unique_key(&self, base: &str) -> String {
        let base = if base.is_empty() { "Bus" } else { base };
        if !self.bus_key_taken(base) {
            return base.to_string();
        }
        (2..)
            .map(|n| format!("{base} {n}"))
            .find(|k| !self.bus_key_taken(k))
            .expect("an unbounded range always yields a free key")
    }

    /// Rename the custom bus at `index`. Cheap and always safe, because the name
    /// is not the routing key — nothing outside the mixer panel reads it. This
    /// used to have to walk the world re-pointing every `AudioPlayer`, and could
    /// only fix the scene that happened to be open.
    ///
    /// Returns `false`, changing nothing, when the name is empty or unchanged.
    /// Duplicate display names are allowed: they are labels, and two buses that
    /// look alike in the panel is a cosmetic problem the user can see and fix.
    pub fn rename_bus(&mut self, index: usize, name: &str) -> bool {
        let name = name.trim();
        let Some(bus) = self.custom_buses.get_mut(index) else {
            return false;
        };
        if name.is_empty() || name == bus.name {
            return false;
        }
        bus.name = name.to_string();
        true
    }

    /// Delete the custom bus at `index`, re-pointing every bus that fed it at
    /// Master. Left alone they would feed Master anyway (an unknown output key
    /// does), but a stale key would come back to life if a bus with that key
    /// were ever created again.
    pub fn remove_bus(&mut self, index: usize) -> Option<Bus> {
        if index >= self.custom_buses.len() {
            return None;
        }
        let removed = self.custom_buses.remove(index);
        for strip in self.strips_mut() {
            if strip.output.as_deref() == Some(removed.key.as_str()) {
                strip.output = None;
            }
        }
        Some(removed)
    }

    /// Every strip on the board, Master first.
    pub fn strips_mut(&mut self) -> impl Iterator<Item = &mut ChannelStrip> {
        [&mut self.master, &mut self.sfx, &mut self.music, &mut self.ambient]
            .into_iter()
            .chain(self.custom_buses.iter_mut().map(|b| &mut b.strip))
    }

    /// The strip with routing key `key`, built-in or custom.
    pub fn strip_by_key(&self, key: &str) -> Option<&ChannelStrip> {
        match key {
            "Master" => Some(&self.master),
            "Sfx" => Some(&self.sfx),
            "Music" => Some(&self.music),
            "Ambient" => Some(&self.ambient),
            _ => self.bus(key).map(|b| &b.strip),
        }
    }

    /// Whether routing `key` into `target` would close a loop: true when
    /// `target` is `key` itself or already feeds it, directly or through other
    /// buses. The mixer offers only targets this says are safe; the backend
    /// checks again, because a hand-edited project.toml can say anything.
    pub fn would_loop(&self, key: &str, target: &str) -> bool {
        let mut cur = Some(target.to_string());
        // Bounded by the bus count, so a loop already in the data cannot hang.
        for _ in 0..=self.custom_buses.len() + 4 {
            let Some(k) = cur else { return false };
            if k == key {
                return true;
            }
            if k == "Master" {
                return false;
            }
            cur = self.strip_by_key(&k).and_then(|s| s.output.clone());
        }
        true
    }

    /// Rebuild the board from a project's `[audio]` section.
    ///
    /// Buses whose key names a built-in seed that built-in's strip; everything
    /// else becomes a custom bus in the order written. An empty config leaves
    /// the default board untouched, so a project.toml with no `[audio]` section
    /// behaves exactly as it did before the section existed.
    ///
    /// The session-only parts of a strip (its capture device, output device and
    /// live meter) are carried over by key. They are not in the config, so a
    /// rebuild would otherwise drop them, and now that the board is rebuilt
    /// whenever a script or settings menu changes a volume, that would close a
    /// live microphone every time someone touched a slider.
    pub fn apply_config(&mut self, config: &renzora_project_api::AudioConfig) {
        if config.buses.is_empty() {
            return;
        }
        let session: Vec<(String, Option<String>, Option<String>, f32)> = self
            .keyed_strips()
            .map(|(key, s)| {
                (
                    key.to_string(),
                    s.input_device.clone(),
                    s.output_device.clone(),
                    s.peak_level,
                )
            })
            .collect();
        *self = Self::default();
        self.seed(config);
        for (key, input, output, peak) in session {
            if let Some(strip) = self.strip_by_key_mut(&key) {
                strip.input_device = input;
                strip.output_device = output;
                strip.peak_level = peak;
            }
        }
    }

    /// Every strip with its routing key, Master first.
    fn keyed_strips(&self) -> impl Iterator<Item = (&str, &ChannelStrip)> {
        [
            ("Master", &self.master),
            ("Sfx", &self.sfx),
            ("Music", &self.music),
            ("Ambient", &self.ambient),
        ]
        .into_iter()
        .chain(self.custom_buses.iter().map(|b| (b.key.as_str(), &b.strip)))
    }

    /// [`Self::strip_by_key`], for writing.
    pub fn strip_by_key_mut(&mut self, key: &str) -> Option<&mut ChannelStrip> {
        match key {
            "Master" => Some(&mut self.master),
            "Sfx" => Some(&mut self.sfx),
            "Music" => Some(&mut self.music),
            "Ambient" => Some(&mut self.ambient),
            _ => self
                .custom_buses
                .iter_mut()
                .find(|b| b.key == key)
                .map(|b| &mut b.strip),
        }
    }

    /// The body of [`Self::apply_config`]: the buses written onto a default board.
    fn seed(&mut self, config: &renzora_project_api::AudioConfig) {
        for bus in &config.buses {
            let strip = ChannelStrip {
                volume: bus.volume,
                panning: bus.panning,
                muted: bus.muted,
                soloed: bus.soloed,
                color: bus.color.unwrap_or([130, 130, 140]),
                output: bus.output.clone(),
                reverb_send: bus.reverb_send,
                delay_send: bus.delay_send,
                ..Default::default()
            };
            match bus.key.as_str() {
                "Master" => self.master = strip,
                "Sfx" => self.sfx = strip,
                "Music" => self.music = strip,
                "Ambient" => self.ambient = strip,
                _ => self.custom_buses.push(Bus {
                    key: bus.key.clone(),
                    name: if bus.name.is_empty() {
                        bus.key.clone()
                    } else {
                        bus.name.clone()
                    },
                    strip,
                }),
            }
        }
    }

    /// Snapshot the board for writing back to project.toml. Built-ins first, in
    /// `BUILTIN_BUSES` order, then the custom buses in mixer order.
    pub fn to_config(&self) -> renzora_project_api::AudioConfig {
        let entry = |key: &str, name: &str, s: &ChannelStrip| renzora_project_api::BusConfig {
            key: key.to_string(),
            // A built-in's name is its key, and writing both would put a
            // redundant `name = "Music"` in every project file.
            name: if name == key { String::new() } else { name.to_string() },
            volume: s.volume,
            panning: s.panning,
            muted: s.muted,
            soloed: s.soloed,
            color: Some(s.color),
            output: s.output.clone(),
            reverb_send: s.reverb_send,
            delay_send: s.delay_send,
        };
        let mut buses = vec![
            entry("Master", "Master", &self.master),
            entry("Sfx", "Sfx", &self.sfx),
            entry("Music", "Music", &self.music),
            entry("Ambient", "Ambient", &self.ambient),
        ];
        buses.extend(
            self.custom_buses
                .iter()
                .map(|b| entry(&b.key, &b.name, &b.strip)),
        );
        renzora_project_api::AudioConfig { buses }
    }
}

/// Rename custom bus `index`.
///
/// Kept as a world-level function because the mixer panel calls it from a
/// deferred `World` closure, but it is now a one-liner over
/// [`MixerState::rename_bus`]: renaming touches a display label and nothing
/// else. It used to walk every `AudioPlayer` and timeline track re-pointing them
/// at the new name, because the name *was* the routing key — and even then it
/// could only fix the scene that happened to be open, leaving every closed
/// scene's emitters aimed at a name nothing answered to. The key/name split
/// (see [`Bus`]) removed the problem rather than mitigating it.
pub fn rename_custom_bus(world: &mut World, index: usize, new_name: &str) -> bool {
    world
        .get_resource_mut::<MixerState>()
        .is_some_and(|mut m| m.rename_bus(index, new_name))
}

/// Keep the board and the runtime settings store (`CurrentProject.config.audio`)
/// in agreement, every frame, in the editor and in a shipped game alike.
///
/// The store is the truth; the board is how the backend and the mixer panel see
/// it. Both sides can be written, though, so the question each frame is which
/// one moved. `agreed` holds the value the two last matched on:
///
/// - **The store differs from `agreed`.** Something outside the mixer wrote it:
///   a project opened, a script called `set_bus_volume`, a game's settings menu
///   or a Rust script assigned the config. It wins, and the board is rebuilt
///   from it.
/// - **The store still equals `agreed` but the board does not.** The mixer panel
///   moved a fader. The board is mirrored into the store, so the next reader of
///   `CurrentProject` (a settings menu drawing its slider) sees the fader's
///   value.
///
/// Comparing against `agreed` rather than simply against each other is what
/// stops the two writers fighting. This used to apply the config once per
/// project *path*, because the panel's write-back marks the project changed and
/// re-applying on that would undo the drag in progress; the price was that a
/// change made to the config while the game ran was never heard.
///
/// Both comparisons go through [`settings::effective`], so an untouched project
/// (no `[audio]` section, default board) shows no difference and gets no section
/// written into it.
pub fn sync_mixer_with_settings(
    project: Option<ResMut<CurrentProject>>,
    mixer: Option<ResMut<MixerState>>,
    mut agreed: Local<Option<AudioConfig>>,
) {
    let (Some(mut project), Some(mut mixer)) = (project, mixer) else {
        return;
    };
    // `MixerState` is changed nearly every frame (the meters write peak levels),
    // so this rarely skips; it saves the compare on frames with no backend.
    if agreed.is_some() && !project.is_changed() && !mixer.is_changed() {
        return;
    }
    let live = settings::effective(&project.config.audio);
    if agreed.as_ref() != Some(&live) {
        if mixer.to_config() != live {
            mixer.apply_config(&live);
        }
        *agreed = Some(live);
        return;
    }
    let board = mixer.to_config();
    if board != live {
        // Only now take `DerefMut`: it marks the whole project changed, which
        // every system watching `CurrentProject` reacts to.
        project.config.audio = board.clone();
        *agreed = Some(board);
    }
}

/// Whether this process may write project.toml for an audio change: only an
/// editor session may.
///
/// A shipped game's `CurrentProject.path` is the sentinel `"."` (its assets are
/// in an rpak), so a write would drop a `project.toml` into whatever the
/// working directory happens to be. And a player's volume slider is the
/// player's, not the developer's mix: in a game it lives in memory, in the
/// settings store, for as long as the game runs (saving it across runs is the
/// game's own save data). The editor's Play runs the game as a child process
/// that is not an editor session, so a script there cannot rewrite the mix
/// either.
pub fn may_write_project_file(session: Option<&EditorSession>) -> bool {
    session.is_some_and(EditorSession::is_editor)
}

/// Write the store's `[audio]` section to project.toml when it changes, in an
/// editor session only (see [`may_write_project_file`]).
///
/// Throttled and compare-first: a fader drag changes the store on every frame
/// of the drag, and rewriting project.toml at 60 Hz would be both wasteful and
/// a good way to catch the file mid-write. `on_disk` is what was last loaded or
/// written for this path, so opening a project writes nothing, and only a real
/// change does.
pub fn persist_audio_settings(
    project: Option<Res<CurrentProject>>,
    session: Option<Res<EditorSession>>,
    time: Res<Time>,
    mut on_disk: Local<Option<(std::path::PathBuf, AudioConfig)>>,
    mut last_save: Local<f64>,
) {
    if !may_write_project_file(session.as_deref()) {
        return;
    }
    let Some(project) = project else {
        return;
    };
    let audio = &project.config.audio;
    match on_disk.as_ref() {
        Some((path, saved)) if *path == project.path => {
            if saved == audio {
                return;
            }
        }
        // A project opened (or this is the first frame): what is in memory is
        // what was just read from disk.
        _ => {
            *on_disk = Some((project.path.clone(), audio.clone()));
            return;
        }
    }
    let now = time.elapsed_secs_f64();
    if *last_save != 0.0 && now - *last_save < 0.75 {
        return;
    }
    *last_save = now;
    match project.save_config() {
        Ok(()) => *on_disk = Some((project.path.clone(), audio.clone())),
        // Left unrecorded, so it is retried after the throttle.
        Err(e) => warn!("[Mixer] couldn't save the bus graph to project.toml: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_board_survives_a_round_trip_through_project_toml() {
        let mut original = MixerState::default();
        original.music.volume = 0.4;
        original.music.muted = true;
        original.add_bus();
        original.add_bus();
        original.rename_bus(0, "Kicks");
        original.custom_buses[1].strip.panning = -0.5;

        let mut restored = MixerState::default();
        restored.apply_config(&original.to_config());

        assert_eq!(restored.music.volume, 0.4);
        assert!(restored.music.muted);
        assert_eq!(restored.custom_buses.len(), 2);
        assert_eq!(restored.custom_buses[0].key, "Bus 1");
        assert_eq!(restored.custom_buses[0].name, "Kicks");
        assert_eq!(restored.custom_buses[1].strip.panning, -0.5);
    }

    /// The whole reason the key/name split exists: a rename must not move the
    /// thing an `AudioPlayer` routes by.
    #[test]
    fn renaming_a_bus_leaves_its_routing_key_alone() {
        let mut mixer = MixerState::default();
        let i = mixer.add_bus();
        let key = mixer.custom_buses[i].key.clone();

        assert!(mixer.rename_bus(i, "Footsteps"));

        assert_eq!(mixer.custom_buses[i].key, key);
        assert_eq!(mixer.custom_buses[i].name, "Footsteps");
        assert_eq!(mixer.bus_index(&key), Some(i));
        assert!(mixer.bus("Footsteps").is_none());
    }

    #[test]
    fn a_requested_key_is_uniquified_rather_than_colliding() {
        let mut mixer = MixerState::default();
        let a = mixer.add_bus_keyed(Some("Voice"));
        let b = mixer.add_bus_keyed(Some("Voice"));

        assert_eq!(mixer.custom_buses[a].key, "Voice");
        assert_eq!(mixer.custom_buses[b].key, "Voice 2");
        assert_ne!(a, b);
    }

    /// An empty key would match every `AudioPlayer` whose bus field was never
    /// set, silently capturing sounds meant for the SFX fallback.
    #[test]
    fn a_bus_can_never_take_an_empty_key() {
        let mut mixer = MixerState::default();
        let i = mixer.add_bus_keyed(Some("   "));
        assert_eq!(mixer.custom_buses[i].key, "Bus");
    }

    /// A built-in's key is contractual — a custom bus taking it would shadow the
    /// track `play_on_bus` matches by name.
    #[test]
    fn a_custom_bus_cannot_shadow_a_built_in() {
        let mut mixer = MixerState::default();
        let i = mixer.add_bus_keyed(Some("Music"));
        assert_eq!(mixer.custom_buses[i].key, "Music 2");
        assert!(mixer.bus_key_taken("music"));
    }

    /// The mixer offers only routing targets that cannot loop.
    #[test]
    fn routing_back_into_a_bus_that_feeds_you_is_a_loop() {
        let mut mixer = MixerState::default();
        let g = mixer.add_bus_keyed(Some("Group"));
        mixer.sfx.output = Some("Group".into());
        assert!(mixer.would_loop("Group", "Sfx"));
        assert!(mixer.would_loop("Sfx", "Sfx"));
        assert!(!mixer.would_loop("Sfx", "Music"));
        assert!(!mixer.would_loop("Music", "Group"));

        // Deleting the group points what fed it back at Master.
        mixer.remove_bus(g);
        assert_eq!(mixer.sfx.output, None);
    }

    /// Routing and sends are part of the mix, so they survive project.toml.
    #[test]
    fn routing_and_sends_survive_a_round_trip() {
        let mut original = MixerState::default();
        original.add_bus_keyed(Some("Group"));
        original.sfx.output = Some("Group".into());
        original.music.reverb_send = 0.3;
        let mut restored = MixerState::default();
        restored.apply_config(&original.to_config());
        assert_eq!(restored.sfx.output.as_deref(), Some("Group"));
        assert_eq!(restored.music.reverb_send, 0.3);
    }

    /// A project.toml with no `[audio]` section must behave exactly as it did
    /// before the section existed.
    #[test]
    fn an_empty_config_leaves_the_default_board_untouched() {
        let mut mixer = MixerState::default();
        mixer.add_bus();
        mixer.apply_config(&renzora_project_api::AudioConfig::default());
        assert_eq!(mixer.custom_buses.len(), 1);
    }

    /// The default board and the contract's default config are one board, or an
    /// untouched project would get an `[audio]` section written on first sync.
    #[test]
    fn the_default_board_is_the_default_config() {
        assert_eq!(
            MixerState::default().to_config(),
            settings::default_config()
        );
    }

    /// A capture device is session state, not in the config; rebuilding the
    /// board for a script's volume change must not close the microphone.
    #[test]
    fn a_rebuild_keeps_each_strips_session_devices() {
        let mut mixer = MixerState::default();
        mixer.music.input_device = Some("Mic".into());
        let mut config = mixer.to_config();
        settings::set_bus_volume(&mut config, "Music", 0.2);
        mixer.apply_config(&config);
        assert_eq!(mixer.music.volume, 0.2);
        assert_eq!(mixer.music.input_device.as_deref(), Some("Mic"));
    }

    // ── The settings store, live ────────────────────────────────────────────

    /// A unique, empty directory to stand in for a project folder, so the
    /// "never writes project.toml" check looks somewhere nothing else writes.
    fn scratch_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("renzora_audio_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    /// The two settings systems over a project at `path`, as a game or the
    /// editor runs them. `session` is `None` for a build that never inserted one.
    fn settings_app(path: std::path::PathBuf, session: Option<bool>) -> App {
        let mut app = App::new();
        app.init_resource::<Time>()
            .insert_resource(MixerState::default())
            .insert_resource(CurrentProject {
                path,
                config: Default::default(),
            })
            .add_systems(
                Update,
                (sync_mixer_with_settings, persist_audio_settings).chain(),
            );
        if let Some(is_editor) = session {
            app.insert_resource(EditorSession(is_editor));
        }
        app
    }

    fn project_audio(app: &mut App) -> Mut<'_, CurrentProject> {
        app.world_mut().resource_mut::<CurrentProject>()
    }

    /// The point of the store: a change made to it while the game runs, by a
    /// settings menu or a Rust script, reaches the board.
    #[test]
    fn a_live_change_to_the_settings_store_reaches_the_board() {
        let mut app = settings_app(std::path::PathBuf::from("."), Some(false));
        app.update();

        settings::set_bus_volume(&mut project_audio(&mut app).config.audio, "Music", 0.3);
        settings::set_bus_muted(&mut project_audio(&mut app).config.audio, "Sfx", true);
        app.update();

        let mixer = app.world().resource::<MixerState>();
        assert_eq!(mixer.music.volume, 0.3);
        assert!(mixer.sfx.muted);
    }

    /// A store nobody touched is not applied again: the mixer panel's drag is
    /// mirrored into the store rather than reverted by it.
    #[test]
    fn an_unchanged_store_does_not_overwrite_a_fader_drag() {
        let mut app = settings_app(std::path::PathBuf::from("."), Some(false));
        app.update();

        app.world_mut().resource_mut::<MixerState>().music.volume = 0.6;
        app.update();
        app.update();

        assert_eq!(app.world().resource::<MixerState>().music.volume, 0.6);
        let audio = &app.world().resource::<CurrentProject>().config.audio;
        assert_eq!(settings::bus_volume(audio, "Music"), Some(0.6));
    }

    /// An untouched project gets no `[audio]` section from merely running.
    #[test]
    fn an_untouched_project_keeps_an_empty_audio_section() {
        let mut app = settings_app(std::path::PathBuf::from("."), Some(false));
        app.update();
        app.update();
        let audio = &app.world().resource::<CurrentProject>().config.audio;
        assert!(audio.buses.is_empty());
    }

    /// A shipped game's project path is `"."`; writing would put a project.toml
    /// in the working directory. Neither a game session nor a build with no
    /// session resource may write, however the volume changes.
    #[test]
    fn a_shipped_game_never_writes_project_toml() {
        for (name, session) in [("game", Some(false)), ("nosession", None)] {
            let dir = scratch_dir(name);
            let mut app = settings_app(dir.clone(), session);
            app.update();
            app.world_mut().resource_mut::<MixerState>().master.volume = 0.5;
            app.update();
            settings::set_master_volume(&mut project_audio(&mut app).config.audio, 0.2);
            app.update();
            app.update();

            assert!(
                !dir.join("project.toml").exists(),
                "{name}: a game wrote project.toml"
            );
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    #[test]
    fn only_an_editor_session_may_write_project_toml() {
        assert!(may_write_project_file(Some(&EditorSession(true))));
        assert!(!may_write_project_file(Some(&EditorSession(false))));
        assert!(!may_write_project_file(None));
    }

    /// `SetMasterVolume` (what a script's `set_master_volume` becomes) lands in
    /// the store and on the Master strip, the same frame. It used to be dropped
    /// before reaching this crate, and before that set a multiplier the fader
    /// never showed.
    #[test]
    fn set_master_volume_moves_the_master_fader() {
        use crate::commands::{AudioCommand, AudioCommandQueue};
        use crate::systems::{process_audio_commands, MusicVoice};

        let mut app = settings_app(std::path::PathBuf::from("."), Some(false));
        app.init_resource::<crate::link::AudioLink>()
            .init_resource::<crate::runtime::SoundCache>()
            .init_resource::<crate::runtime::ActiveVoices>()
            .init_resource::<MusicVoice>()
            .init_resource::<AudioCommandQueue>()
            .add_systems(
                Update,
                process_audio_commands.before(sync_mixer_with_settings),
            );
        app.update();

        let mut queue = app.world_mut().resource_mut::<AudioCommandQueue>();
        queue.push(AudioCommand::SetMasterVolume { volume: 0.4 });
        queue.push(AudioCommand::SetBusVolume {
            bus: "music".into(),
            volume: 0.7,
        });
        queue.push(AudioCommand::SetBusMuted {
            bus: "Ambient".into(),
            muted: true,
        });
        app.update();

        let audio = &app.world().resource::<CurrentProject>().config.audio;
        assert_eq!(settings::master_volume(audio), f64::from(0.4f32));
        let mixer = app.world().resource::<MixerState>();
        assert_eq!(mixer.master.volume, f64::from(0.4f32));
        assert_eq!(mixer.music.volume, f64::from(0.7f32));
        assert!(mixer.ambient.muted);
    }

    /// The script route end to end: a `set_bus_volume` action fired by a script
    /// reaches the queue through the observer.
    #[test]
    fn a_script_action_queues_a_bus_volume() {
        use crate::commands::{AudioCommand, AudioCommandQueue};
        use renzora_scripting_api::{ScriptAction, ScriptActionValue};

        let mut app = App::new();
        app.init_resource::<AudioCommandQueue>()
            .init_resource::<crate::script_actions::AudioPlayerRuntime>()
            .add_observer(crate::script_actions::handle_audio_script_actions);
        let entity = app.world_mut().spawn_empty().id();
        let mut args = std::collections::HashMap::new();
        args.insert("bus".to_string(), ScriptActionValue::String("Music".into()));
        args.insert("volume".to_string(), ScriptActionValue::Int(0));
        app.world_mut().trigger(ScriptAction {
            name: "set_bus_volume".into(),
            entity,
            target_entity: None,
            args,
        });

        let queued = app.world_mut().resource_mut::<AudioCommandQueue>().drain();
        assert!(matches!(
            queued.as_slice(),
            [AudioCommand::SetBusVolume { bus, volume }] if bus == "Music" && *volume == 0.0
        ));
    }
}
