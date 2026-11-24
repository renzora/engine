//! The game's audio settings, as they live in the runtime settings store.
//!
//! The store is `CurrentProject.config.audio`. It is the one place a bus volume
//! is decided: the editor's mixer panel writes it, a script's
//! `set_master_volume` / `set_bus_volume` write it, a game's own settings menu
//! (a Rust script with `&mut World`) writes it, and `renzora_audio` applies
//! whatever is there to the mixer while the game runs. Nothing else holds a
//! second copy of master volume to drift out of step with it.
//!
//! These helpers live in the contract, not in `renzora_audio`, because the
//! callers that most need them cannot link `renzora_audio`: a Rust script or a
//! native plugin reaches only `renzora_sdk`, where this module is
//! `renzora::audio::settings`.
//!
//! The one subtlety they exist for is the empty section. A project.toml with no
//! `[audio]` section means "the default board", so `buses` is empty, and a
//! caller that searched it for `"Master"` would find nothing and change
//! nothing. Every setter here writes the default board out first, so
//! `set_bus_volume(&mut audio, "Master", 0.5)` works on a project that never
//! opened the mixer.

pub use renzora_project_api::{AudioConfig, BusConfig};

/// Names the built-in buses own. Playback matches these strings literally to
/// pick a track, so they are routing keys, not labels: a custom bus may never
/// take one, and the four built-ins cannot be renamed without breaking every
/// `AudioPlayer` already pointing at them.
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

/// Master's tint. Not from the palette, because Master is not one channel among
/// others and should not look like one.
pub const MASTER_COLOR: [u8; 3] = [200, 200, 205];

/// The loudest a bus may be set: linear 1.5, about +3.5 dB of head-room, the
/// top of the mixer's fader. A script asking for more is clamped to the same
/// ceiling a person dragging the fader hits.
pub const MAX_BUS_VOLUME: f64 = 1.5;

/// The board a project with no `[audio]` section has: the four built-ins at
/// unity, centred, each on its own colour.
pub fn default_config() -> AudioConfig {
    let bus = |key: &str, color: [u8; 3]| BusConfig {
        key: key.to_string(),
        name: String::new(),
        volume: 1.0,
        panning: 0.0,
        muted: false,
        soloed: false,
        color: Some(color),
        output: None,
        reverb_send: 0.0,
        delay_send: 0.0,
    };
    AudioConfig {
        buses: vec![
            bus("Master", MASTER_COLOR),
            bus("Sfx", BUS_COLORS[1]),
            bus("Music", BUS_COLORS[5]),
            bus("Ambient", BUS_COLORS[4]),
        ],
    }
}

/// What `config` means: itself, or the default board when the section is empty.
///
/// Comparing through this is what lets an untouched project stay untouched:
/// the default mixer and an empty section are the same board, so neither side
/// sees a difference to write back.
pub fn effective(config: &AudioConfig) -> AudioConfig {
    if config.buses.is_empty() {
        default_config()
    } else {
        config.clone()
    }
}

/// The bus with routing key `key`. An exact match first, then one ignoring
/// case: keys are unique without regard to case, so the fallback can never
/// pick the wrong bus, and a script that wrote `"music"` gets what it meant.
pub fn bus<'a>(config: &'a AudioConfig, key: &str) -> Option<&'a BusConfig> {
    config.buses.iter().find(|b| b.key == key).or_else(|| {
        config
            .buses
            .iter()
            .find(|b| b.key.eq_ignore_ascii_case(key))
    })
}

/// [`bus`], for writing. Materializes the default board first when the section
/// is empty, so a built-in can be set on a project that never wrote one.
pub fn bus_mut<'a>(config: &'a mut AudioConfig, key: &str) -> Option<&'a mut BusConfig> {
    if config.buses.is_empty() {
        *config = default_config();
    }
    let index = config.buses.iter().position(|b| b.key == key).or_else(|| {
        config
            .buses
            .iter()
            .position(|b| b.key.eq_ignore_ascii_case(key))
    })?;
    config.buses.get_mut(index)
}

/// A bus's linear volume, `None` for an unknown key.
pub fn bus_volume(config: &AudioConfig, key: &str) -> Option<f64> {
    if config.buses.is_empty() {
        return BUILTIN_BUSES
            .iter()
            .any(|b| b.eq_ignore_ascii_case(key))
            .then_some(1.0);
    }
    bus(config, key).map(|b| b.volume)
}

/// Whether a bus is muted, `None` for an unknown key.
pub fn bus_muted(config: &AudioConfig, key: &str) -> Option<bool> {
    if config.buses.is_empty() {
        return BUILTIN_BUSES
            .iter()
            .any(|b| b.eq_ignore_ascii_case(key))
            .then_some(false);
    }
    bus(config, key).map(|b| b.muted)
}

/// Set a bus's linear volume, clamped to `0..=MAX_BUS_VOLUME`. Returns false,
/// changing nothing, when no bus has that key.
///
/// Leaves `config` untouched when the value is already what was asked for. A
/// caller holding `ResMut<CurrentProject>` should still compare with
/// [`bus_volume`] before borrowing mutably, since the borrow alone marks the
/// whole project changed; this check only spares the empty section from being
/// written out for nothing.
pub fn set_bus_volume(config: &mut AudioConfig, key: &str, volume: f64) -> bool {
    let volume = clamp_volume(volume);
    if bus_volume(config, key) == Some(volume) {
        return true;
    }
    match bus_mut(config, key) {
        Some(bus) => {
            bus.volume = volume;
            true
        }
        None => false,
    }
}

/// Mute or unmute a bus. Returns false when no bus has that key.
pub fn set_bus_muted(config: &mut AudioConfig, key: &str, muted: bool) -> bool {
    if bus_muted(config, key) == Some(muted) {
        return true;
    }
    match bus_mut(config, key) {
        Some(bus) => {
            bus.muted = muted;
            true
        }
        None => false,
    }
}

/// The master volume: the Master bus's fader, there is no other.
pub fn master_volume(config: &AudioConfig) -> f64 {
    bus_volume(config, "Master").unwrap_or(1.0)
}

/// Set the master volume. See [`master_volume`].
pub fn set_master_volume(config: &mut AudioConfig, volume: f64) {
    set_bus_volume(config, "Master", volume);
}

/// `volume` within the fader's range. A NaN from a script's arithmetic becomes
/// silence rather than reaching the mixer, where it would poison every sample
/// on the bus.
pub fn clamp_volume(volume: f64) -> f64 {
    if volume.is_nan() {
        0.0
    } else {
        volume.clamp(0.0, MAX_BUS_VOLUME)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole reason the setters exist: an empty section is the default
    /// board, and setting Master on it must work.
    #[test]
    fn a_volume_can_be_set_on_a_project_with_no_audio_section() {
        let mut audio = AudioConfig::default();
        set_master_volume(&mut audio, 0.25);
        assert_eq!(master_volume(&audio), 0.25);
        assert_eq!(audio.buses.len(), BUILTIN_BUSES.len());
        assert_eq!(bus_volume(&audio, "Music"), Some(1.0));
    }

    #[test]
    fn an_unknown_bus_is_refused_and_changes_nothing() {
        let mut audio = default_config();
        assert!(!set_bus_volume(&mut audio, "Nope", 0.1));
        assert_eq!(audio, default_config());
    }

    #[test]
    fn keys_match_without_regard_to_case_and_volumes_clamp() {
        let mut audio = default_config();
        assert!(set_bus_volume(&mut audio, "music", 9.0));
        assert_eq!(bus_volume(&audio, "Music"), Some(MAX_BUS_VOLUME));
        assert!(set_bus_muted(&mut audio, "SFX", true));
        assert_eq!(bus_muted(&audio, "Sfx"), Some(true));
        set_master_volume(&mut audio, f64::NAN);
        assert_eq!(master_volume(&audio), 0.0);
    }

    /// Setting what is already there must not write out an empty section, or a
    /// script calling it every frame would give an untouched project an
    /// `[audio]` table.
    #[test]
    fn setting_the_current_value_leaves_an_empty_section_empty() {
        let mut audio = AudioConfig::default();
        set_master_volume(&mut audio, 1.0);
        set_bus_muted(&mut audio, "Music", false);
        assert!(audio.buses.is_empty());
    }
}
