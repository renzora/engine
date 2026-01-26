//! UI Abstraction Layer (UIDL)
//!
//! This module provides a stable UI abstraction that plugins can use to define
//! their user interfaces without depending on egui directly. The editor's
//! internal renderer translates these abstract widgets to egui.

pub mod events;
pub mod layout;
pub mod renderer;
pub mod types;
pub mod widgets;

pub use events::UiEvent;
pub use types::*;
pub use widgets::*;
