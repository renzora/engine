//! Shared UI building blocks for editor panels and game UI.

mod buttons;
mod category;
mod colors;
mod empty_state;
mod fader;
mod knob;
mod mixer_strip;
mod property;
mod section;
pub mod node_graph;
pub mod tile_grid;
pub mod search_overlay;
mod drop_zone;
mod toggle;
pub mod tree;
mod utils;
mod vu_meter;

pub use buttons::icon_button;
pub use category::{
    category_colors, collapsible_section, collapsible_section_removable, CategoryHeaderAction,
};
pub use colors::{checkerboard, dim_color};
pub use empty_state::empty_state;
pub use fader::{vertical_fader, FaderConfig};
pub use knob::{rotary_knob, KnobConfig};
pub use mixer_strip::{mixer_channel_strip, MixerStripConfig, MixerStripState};
pub use property::{inline_property, property_row, LABEL_WIDTH, MIN_PANEL_WIDTH};
pub use section::section_header;
pub use tile_grid::{split_label_two_lines, TileGrid, TileLayout, TileState};
pub use toggle::toggle_switch;
pub use tree::{draw_drop_line, tree_drop_zone, tree_row, TreeDropZone, TreeRowConfig, TreeRowResult};
pub use utils::sanitize_f32;
pub use vu_meter::{vu_meter, VuMeterConfig, VuMeterValue};
pub use colors::{get_theme_colors, set_theme_colors, ThemeColors};
pub use node_graph::{
    node_graph, NodeGraphState, NodeGraphConfig, NodeGraphResponse,
    NodeDef, PinDef, PinDirection, PinShape, ConnectionDef, PinId, NodeId,
};
pub use drop_zone::{file_drop_zone, FileDropResult};
pub use search_overlay::{search_overlay, OverlayAction, OverlayEntry};
