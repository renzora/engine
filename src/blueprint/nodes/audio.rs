//! Audio nodes
//!
//! Nodes for playing sounds, music, and spatial audio.

use super::{NodeTypeDefinition, Pin, PinType, PinValue};

// =============================================================================
// SOUND PLAYBACK
// =============================================================================

/// Play sound
pub static PLAY_SOUND: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "audio/play_sound",
    display_name: "Play Sound",
    category: "Audio",
    description: "Play a sound effect (one-shot)",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("sound", "Sound", PinType::Asset),
        Pin::input("volume", "Volume", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("pitch", "Pitch", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("handle", "Handle", PinType::AudioHandle),
    ],
    color: [180, 100, 220],
    is_event: false,
    is_comment: false,
};

/// Play sound at position (3D spatial)
pub static PLAY_SOUND_AT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "audio/play_sound_at",
    display_name: "Play Sound At",
    category: "Audio",
    description: "Play a sound at a 3D position (spatial audio)",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("sound", "Sound", PinType::Asset),
        Pin::input("position", "Position", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::input("volume", "Volume", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("pitch", "Pitch", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("radius", "Radius", PinType::Float).with_default(PinValue::Float(10.0)),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("handle", "Handle", PinType::AudioHandle),
    ],
    color: [180, 100, 220],
    is_event: false,
    is_comment: false,
};

/// Play sound attached to entity
pub static PLAY_SOUND_ATTACHED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "audio/play_sound_attached",
    display_name: "Play Sound Attached",
    category: "Audio",
    description: "Play a sound attached to an entity (follows entity)",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("sound", "Sound", PinType::Asset),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("volume", "Volume", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("pitch", "Pitch", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("handle", "Handle", PinType::AudioHandle),
    ],
    color: [180, 100, 220],
    is_event: false,
    is_comment: false,
};

/// Stop sound
pub static STOP_SOUND: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "audio/stop_sound",
    display_name: "Stop Sound",
    category: "Audio",
    description: "Stop a playing sound",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("handle", "Handle", PinType::AudioHandle),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 100, 220],
    is_event: false,
    is_comment: false,
};

/// Pause sound
pub static PAUSE_SOUND: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "audio/pause_sound",
    display_name: "Pause Sound",
    category: "Audio",
    description: "Pause a playing sound",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("handle", "Handle", PinType::AudioHandle),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 100, 220],
    is_event: false,
    is_comment: false,
};

/// Resume sound
pub static RESUME_SOUND: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "audio/resume_sound",
    display_name: "Resume Sound",
    category: "Audio",
    description: "Resume a paused sound",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("handle", "Handle", PinType::AudioHandle),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 100, 220],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// MUSIC
// =============================================================================

/// Play music (looping background)
pub static PLAY_MUSIC: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "audio/play_music",
    display_name: "Play Music",
    category: "Audio",
    description: "Play looping background music",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("music", "Music", PinType::Asset),
        Pin::input("volume", "Volume", PinType::Float).with_default(PinValue::Float(0.5)),
        Pin::input("fade_in", "Fade In", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("handle", "Handle", PinType::AudioHandle),
    ],
    color: [180, 100, 220],
    is_event: false,
    is_comment: false,
};

/// Stop music
pub static STOP_MUSIC: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "audio/stop_music",
    display_name: "Stop Music",
    category: "Audio",
    description: "Stop background music",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("fade_out", "Fade Out", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 100, 220],
    is_event: false,
    is_comment: false,
};

/// Crossfade music
pub static CROSSFADE_MUSIC: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "audio/crossfade_music",
    display_name: "Crossfade Music",
    category: "Audio",
    description: "Crossfade to new background music",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("music", "New Music", PinType::Asset),
        Pin::input("duration", "Duration", PinType::Float).with_default(PinValue::Float(2.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 100, 220],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// AUDIO PROPERTIES
// =============================================================================

/// Set volume
pub static SET_VOLUME: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "audio/set_volume",
    display_name: "Set Volume",
    category: "Audio",
    description: "Set the volume of a playing sound",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("handle", "Handle", PinType::AudioHandle),
        Pin::input("volume", "Volume", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 100, 220],
    is_event: false,
    is_comment: false,
};

/// Set pitch
pub static SET_PITCH: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "audio/set_pitch",
    display_name: "Set Pitch",
    category: "Audio",
    description: "Set the pitch/playback speed of a sound",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("handle", "Handle", PinType::AudioHandle),
        Pin::input("pitch", "Pitch", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 100, 220],
    is_event: false,
    is_comment: false,
};

/// Set panning
pub static SET_PANNING: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "audio/set_panning",
    display_name: "Set Panning",
    category: "Audio",
    description: "Set stereo panning (-1 left, 0 center, 1 right)",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("handle", "Handle", PinType::AudioHandle),
        Pin::input("pan", "Pan", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 100, 220],
    is_event: false,
    is_comment: false,
};

/// Set master volume
pub static SET_MASTER_VOLUME: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "audio/set_master_volume",
    display_name: "Set Master Volume",
    category: "Audio",
    description: "Set the master/global volume",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("volume", "Volume", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 100, 220],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// AUDIO QUERIES
// =============================================================================

/// Is playing
pub static IS_PLAYING: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "audio/is_playing",
    display_name: "Is Playing",
    category: "Audio",
    description: "Check if a sound is currently playing",
    create_pins: || vec![
        Pin::input("handle", "Handle", PinType::AudioHandle),
        Pin::output("playing", "Is Playing", PinType::Bool),
    ],
    color: [180, 100, 220],
    is_event: false,
    is_comment: false,
};

/// Get playback position
pub static GET_PLAYBACK_POSITION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "audio/get_position",
    display_name: "Get Playback Position",
    category: "Audio",
    description: "Get the current playback position in seconds",
    create_pins: || vec![
        Pin::input("handle", "Handle", PinType::AudioHandle),
        Pin::output("position", "Position", PinType::Float),
        Pin::output("duration", "Duration", PinType::Float),
        Pin::output("progress", "Progress", PinType::Float),
    ],
    color: [180, 100, 220],
    is_event: false,
    is_comment: false,
};

/// Set playback position
pub static SET_PLAYBACK_POSITION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "audio/set_position",
    display_name: "Set Playback Position",
    category: "Audio",
    description: "Seek to a position in the audio",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("handle", "Handle", PinType::AudioHandle),
        Pin::input("position", "Position", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 100, 220],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// AUDIO EVENTS
// =============================================================================

/// On sound finished
pub static ON_SOUND_FINISHED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "audio/on_finished",
    display_name: "On Sound Finished",
    category: "Audio Events",
    description: "Triggered when a sound finishes playing",
    create_pins: || vec![
        Pin::input("handle", "Handle", PinType::AudioHandle),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 100, 220],
    is_event: true,
    is_comment: false,
};

// =============================================================================
// SPATIAL AUDIO
// =============================================================================

/// Set audio listener
pub static SET_AUDIO_LISTENER: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "audio/set_listener",
    display_name: "Set Audio Listener",
    category: "Audio",
    description: "Set the entity to use as the audio listener",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 100, 220],
    is_event: false,
    is_comment: false,
};

/// Set spatial audio properties
pub static SET_SPATIAL_PROPERTIES: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "audio/set_spatial",
    display_name: "Set Spatial Properties",
    category: "Audio",
    description: "Configure spatial audio properties for a sound",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("handle", "Handle", PinType::AudioHandle),
        Pin::input("min_distance", "Min Distance", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("max_distance", "Max Distance", PinType::Float).with_default(PinValue::Float(100.0)),
        Pin::input("rolloff", "Rolloff Factor", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [180, 100, 220],
    is_event: false,
    is_comment: false,
};
