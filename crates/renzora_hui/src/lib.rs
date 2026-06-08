//! `renzora_hui` — transitional facade.
//!
//! The HUI markup runtime and the icon/cursor-icon helpers were folded into
//! [`renzora_ember`] (ember is now the single UI crate). This crate re-exports
//! them so existing `renzora_hui::...` paths keep compiling while callers
//! migrate to `renzora_ember::...`. The markup runtime plugin is registered by
//! ember (`renzora::add!(renzora_ember::markup::MarkupPlugin)`); this facade
//! installs nothing itself.

// Shared, vello-free helpers (live at the ember crate root).
pub use renzora_ember::{cursor_icon, icons, phosphor_map};

// Markup runtime modules.
pub use renzora_ember::markup::{
    binding, cursor, decor, dnd, drag, foreach, input_field, interactions, loader, lua_bridge,
    provenance, template, transitions, vector, widgets, writeback,
};

// Convenience re-exports that lived at the old crate root.
pub use renzora_ember::markup::{HtmlTemplatePath, MarkupSource};
