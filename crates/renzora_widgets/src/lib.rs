//! Renzora Widgets — shared UI building blocks for editor panels and game UI.
//!
//! All widgets accept a `&Theme` for consistent styling. Functions are designed
//! to be called directly — no wrapper structs or extension traits needed.
//!
//! # Quick start
//! ```ignore
//! use renzora_widgets::*;
//!
//! // Alternating-row property
//! inline_property(ui, 0, "Speed", &theme, |ui| {
//!     ui.add(egui::DragValue::new(&mut speed).speed(0.1));
//! });
//!
//! // Collapsible category section
//! collapsible_section(ui, "⚙", "Transform", "transform", &theme, "xform", true, |ui| {
//!     // section contents ...
//! });
//! ```

mod buttons;
mod category;
mod colors;
mod empty_state;
mod property;
mod section;
pub mod tile_grid;
mod toggle;
pub mod tree;
mod utils;

pub use buttons::icon_button;
pub use category::{
    collapsible_section, collapsible_section_removable, CategoryHeaderAction,
};
pub use colors::{checkerboard, dim_color};
pub use empty_state::empty_state;
pub use property::{inline_property, property_row, LABEL_WIDTH, MIN_PANEL_WIDTH};
pub use section::section_header;
pub use toggle::toggle_switch;
pub use utils::sanitize_f32;

/// Cached theme colors for fast per-frame access (stored in egui context data).
///
/// Call [`set_theme_colors`] once per frame, then [`get_theme_colors`] from any widget.
pub use colors::{get_theme_colors, set_theme_colors, ThemeColors};
