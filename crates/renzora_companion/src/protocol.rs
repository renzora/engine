//! What the editor and its companion say to each other.
//!
//! Defined in `renzora_companion_proto`, which links no Bevy, so the
//! standalone companion speaks exactly the protocol the editor does.
//! Re-exported here so `renzora_companion::protocol` still resolves.

pub use renzora_companion_proto::protocol::*;
