//! The console, at the path plugins and scripts have always used
//! (`renzora::console_log::console_warn`).
//!
//! It lives in its own contract layer, `renzora_log_api`; this module only
//! re-exports it. Engine crates depend on that layer directly rather than on
//! `renzora`, so editing anything else in the contract does not rebuild the
//! crates that only log.

pub use renzora_log_api::*;
