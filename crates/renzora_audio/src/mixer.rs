//! The mixer board: bus volumes, panning, mute, solo and colour for the four
//! built-ins and any user-created buses, plus its persistence.
//!
//! Pure state. Pushing it at whatever is making sound is
//! [`crate::runtime::sync_mixer_to_backend`]'s job — this module has no idea a
//! backend exists, which is what lets the same board drive a native mixer, a
//! WebAudio one, or nothing at all.

use bevy::prelude::*;

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
}

/// Names the built-in buses own. `play_on_bus` (and its spatial twin) match
/// these strings *literally* to pick a track, so they are routing keys, not
/// labels — a custom bus may never take one, and the four built-ins can't be
/// renamed without breaking every `AudioPlayer` already pointing at them.
pub const BUILTIN_BUSES: [&str; 4] = ["Master", "Sfx", "Music", "Ambient"];

/// The colour palette the mixer offers in its swatch grid, and which new custom
/// buses cycle through so a fresh bus is distinguishable from its neighbours
/// without the user having to pick a colour first.
pub const BUS_COLORS: [[u8; 3]; 12] = [
    [220, 70, 70],   // red
    [228, 132, 52],  // amber
    [205, 192, 52],  // ochre
    [120, 200, 80],  // green
    [48, 196, 140],  // teal
    [75, 162, 220],  // sky
    [90, 110, 225],  // blue
    [135, 90, 228],  // violet
    [200, 80, 190],  // magenta
    [220, 80, 130],  // rose
    [160, 110, 75],  // brown
    [130, 130, 140], // gray
];

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
/// `(String, ChannelStrip)` tuple it used to be — see [`renzora::BusConfig`] for
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
#[derive(Resource)]
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
            master: tinted([200, 200, 205]),
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

    /// Rebuild the board from a project's `[audio]` section.
    ///
    /// Buses whose key names a built-in seed that built-in's strip; everything
    /// else becomes a custom bus in the order written. An empty config leaves
    /// the default board untouched, so a project.toml with no `[audio]` section
    /// behaves exactly as it did before the section existed.
    pub fn apply_config(&mut self, config: &renzora::AudioConfig) {
        if config.buses.is_empty() {
            return;
        }
        *self = Self::default();
        for bus in &config.buses {
            let strip = ChannelStrip {
                volume: bus.volume,
                panning: bus.panning,
                muted: bus.muted,
                soloed: bus.soloed,
                color: bus.color.unwrap_or([130, 130, 140]),
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
    pub fn to_config(&self) -> renzora::AudioConfig {
        let entry = |key: &str, name: &str, s: &ChannelStrip| renzora::BusConfig {
            key: key.to_string(),
            // A built-in's name is its key, and writing both would put a
            // redundant `name = "Music"` in every project file.
            name: if name == key { String::new() } else { name.to_string() },
            volume: s.volume,
            panning: s.panning,
            muted: s.muted,
            soloed: s.soloed,
            color: Some(s.color),
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
        renzora::AudioConfig { buses }
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

/// Seed the board from the project's `[audio]` section when a project opens.
///
/// Keyed on the project *path*, not on `CurrentProject::is_changed()`: the
/// write-back below edits the config and so marks the resource changed, and
/// re-applying on that would overwrite whatever the user just did with the value
/// that had only this moment been saved.
pub fn load_mixer_config(
    project: Option<Res<renzora::core::CurrentProject>>,
    mixer: Option<ResMut<MixerState>>,
    mut loaded: Local<Option<std::path::PathBuf>>,
) {
    let (Some(project), Some(mut mixer)) = (project, mixer) else {
        return;
    };
    if loaded.as_deref() == Some(project.path.as_path()) {
        return;
    }
    *loaded = Some(project.path.clone());
    mixer.apply_config(&project.config.audio);
}

/// Write the board back to project.toml when it changes.
///
/// Throttled and compare-first, following `renzora_viewport::persistence`: a
/// fader drag marks `MixerState` changed on every frame of the drag, and
/// rewriting project.toml at 60 Hz would be both wasteful and a good way to
/// catch the file mid-write. The read-only compare matters for a second reason —
/// taking `DerefMut` on `CurrentProject` marks it changed, which cascades into
/// every system watching the project.
pub fn save_mixer_config(
    mut project: Option<ResMut<renzora::core::CurrentProject>>,
    mixer: Option<Res<MixerState>>,
    time: Res<Time>,
    mut last_save: Local<f64>,
) {
    let (Some(project), Some(mixer)) = (project.as_mut(), mixer) else {
        return;
    };
    if !mixer.is_changed() {
        return;
    }
    let now = time.elapsed_secs_f64();
    if *last_save != 0.0 && now - *last_save < 0.75 {
        return;
    }
    let snapshot = mixer.to_config();
    if project.as_ref().config.audio == snapshot {
        return;
    }
    *last_save = now;
    project.config.audio = snapshot;
    if let Err(e) = project.save_config() {
        warn!("[Mixer] couldn't save the bus graph to project.toml: {e}");
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

    /// A project.toml with no `[audio]` section must behave exactly as it did
    /// before the section existed.
    #[test]
    fn an_empty_config_leaves_the_default_board_untouched() {
        let mut mixer = MixerState::default();
        mixer.add_bus();
        mixer.apply_config(&renzora::AudioConfig::default());
        assert_eq!(mixer.custom_buses.len(), 1);
    }
}
