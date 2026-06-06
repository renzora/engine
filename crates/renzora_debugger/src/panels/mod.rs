//! Shared diagnostic state resources read by the native (ember) debug panels.
//!
//! These modules hold only the per-frame snapshot `Resource`s (updated by the
//! crate's backend-agnostic `update_*` systems); the native panels in
//! [`crate::native`] render them.

pub mod lumen;
pub mod scripting;
