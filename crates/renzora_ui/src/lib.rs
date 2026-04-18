//! Renzora UI — standalone docking panel framework and widget library for Bevy + egui.
//!
//! Includes the dock tree layout engine, panel trait, drag-drop, document tabs,
//! title bar, status bar, theme application, and all reusable widgets.

pub mod asset_drag;
pub mod shape_drag;
pub mod dock_renderer;
pub mod dock_tree;
pub mod document_tabs;
pub mod drag_drop;
pub mod floating;
pub mod layouts;
pub mod panel;
pub mod status_bar;
pub mod theme;
pub mod title_bar;
pub mod toast;
pub mod widgets;
pub mod window_chrome;

// Re-export key types at crate root
pub use dock_tree::{
    delete_saved_workspace, load_saved_workspace, save_workspace, DockTree, DockingState,
    DropZone, SplitDirection,
};
pub use document_tabs::{DocTabAction, DocumentTab, DocumentTabState};
pub use asset_drag::{AssetDragPayload, AssetDropResult, asset_drop_target, draw_asset_drag_ghost};
pub use shape_drag::{ShapeDragState, ShapeDragPreview, ShapeDragPreviewState, PendingShapeDrop};
pub use drag_drop::{DragState, DropTarget};
pub use floating::{FloatingPanel, FloatingPanels, FloatingRenderResult};
pub use layouts::{LayoutManager, WorkspaceLayout};
pub use panel::{EditorPanel, PanelLocation, PanelRegistry};
pub use status_bar::{StatusBarAlignment, StatusBarItem, StatusBarRegistry};
pub use title_bar::TitleBarAction;
pub use toast::Toasts;

// Re-export all widgets at crate root
pub use widgets::*;
