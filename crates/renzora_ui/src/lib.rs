//! Renzora UI — shared editor data types.
//!
//! Historically this crate housed the egui docking-panel framework and widget
//! library. After the native (bevy_ui / `renzora_ember` + `renzora_shell`)
//! migration, all egui rendering was removed; what remains are the pure,
//! runtime-agnostic data types still consumed across the editor: document tabs,
//! the dock tree model, layout/workspace persistence, window-chrome actions,
//! drag payloads, the toast queue, and the floating/panel registries.

pub mod asset_drag;
pub mod dock_tree;
pub mod document_tabs;
pub mod floating;
pub mod layouts;
pub mod panel;
pub mod shape_drag;
pub mod toast;
pub mod tree;
pub mod window_chrome;

// Re-export key types at crate root
pub use asset_drag::AssetDragPayload;
pub use dock_tree::{
    delete_saved_workspace, load_saved_workspace, save_workspace, DockTree, DockingState, DropZone,
    SplitDirection, ViewportMaximized,
};
pub use document_tabs::{DocTabAction, DocTabKind, DocumentTab, DocumentTabState, EditorContext};
pub use floating::{FloatingPanel, FloatingPanels};
pub use layouts::{LayoutManager, WorkspaceLayout};
pub use panel::{EditorPanel, PanelLocation, PanelRegistry};
pub use shape_drag::{PendingShapeDrop, ShapeDragPreview, ShapeDragPreviewState, ShapeDragState};
pub use toast::Toasts;
pub use tree::TreeDropZone;
