//! Renzora UI — shared editor data types.
//!
//! Historically this crate housed the egui docking-panel framework and widget
//! library. After the native (bevy_ui / `renzora_ember` + `renzora_shell`)
//! migration, all egui rendering was removed; what remains are the pure,
//! runtime-agnostic data types still consumed across the editor: document tabs,
//! the dock tree model, layout/workspace persistence, window-chrome actions,
//! drag payloads, the toast queue, and the floating/panel registries.

pub mod asset_drag;
pub mod document_tabs;
pub mod floating;
pub mod panel;
pub mod shape_drag;
pub mod toast;
pub mod tree;
/// The one survivor of the deleted `dock_tree` module.
pub mod viewport_maximize;
pub mod window_chrome;

// Re-export key types at crate root
pub use asset_drag::AssetDragPayload;
pub use document_tabs::{DocTabAction, DocTabKind, DocumentTab, DocumentTabState, EditorContext};
pub use floating::{FloatingPanel, FloatingPanels};
pub use panel::{EditorPanel, PanelLocation, PanelRegistry};
pub use shape_drag::{PendingShapeDrop, ShapeDragPreview, ShapeDragPreviewState, ShapeDragState};
pub use toast::Toasts;
pub use tree::TreeDropZone;
pub use viewport_maximize::ViewportMaximized;
