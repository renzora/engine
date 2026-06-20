//! Protocol module.
//!
//! With the from-scratch UDP transport, "the protocol" is just the bincode
//! `Packet` enum in [`crate::transport`] plus the serde [`crate::messages`]
//! types. There is no channel/component registration step to perform (that was
//! a lightyear concept), so this module is intentionally empty — kept as a
//! stable home for any future wire-format helpers.
