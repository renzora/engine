//! Audio command routing for Rhai scripting
//!
//! Audio commands from scripts are queued into AudioCommandQueue.
//! The actual Kira playback is handled by src/audio/systems.rs.
//! This module exists for routing completeness — see scripting/runtime.rs
//! for where RhaiCommand::PlaySound etc. are mapped to AudioCommand.
