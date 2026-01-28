//! Docking system for the editor
//!
//! This module provides a flexible split-based docking system similar to VS Code or Unity.
//! Panels can be dragged, split, and tabbed to create custom layouts.
//!
//! # Architecture
//!
//! The docking system uses a binary tree structure (`DockTree`) where:
//! - `Split` nodes divide space between two children (horizontal or vertical)
//! - `Leaf` nodes contain one or more tabbed panels
//!
//! # Usage
//!
//! ```ignore
//! // Create a simple layout
//! let tree = DockTree::horizontal(
//!     DockTree::leaf(PanelId::Hierarchy),
//!     DockTree::leaf(PanelId::Viewport),
//!     0.2, // 20% for hierarchy
//! );
//! ```

mod dock_tree;
pub mod drag_drop;
mod layouts;
mod panel_content;
mod panel_registry;
mod renderer;

pub use dock_tree::{DockTree, DropZone, PanelId, SplitDirection};
#[allow(unused_imports)]
pub use drag_drop::{DragState, DropTarget};
pub use layouts::{
    builtin_layouts, default_layout,
    DockingLayoutConfig, WorkspaceLayout,
};
#[allow(unused_imports)]
pub use layouts::{animation_layout, debug_layout, scripting_layout};
pub use panel_content::{
    render_panel_frame,
    DockedPanelContext,
};
#[allow(unused_imports)]
pub use panel_content::{render_placeholder_content, get_panel_min_size, DockablePanel};
pub use panel_registry::PanelAvailability;
#[allow(unused_imports)]
pub use panel_registry::{all_builtin_panels, get_panel_constraints, PanelConstraints};
pub use renderer::{
    calculate_panel_rects, get_legacy_layout_values,
    render_dock_tree,
};
#[allow(unused_imports)]
pub use renderer::{DockRenderResult, get_panel_content_rect, PanelRenderContext, TAB_BAR_HEIGHT, RESIZE_HANDLE_SIZE};
