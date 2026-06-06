//! Tree-row drag-and-drop data types shared with the native hierarchy.

/// Where a dragged tree row will be dropped, relative to the target row.
///
/// The row height is split into thirds:
/// - `Before` — top third (insert above)
/// - `AsChild` — middle third (reparent under the target)
/// - `After` — bottom third (insert below)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeDropZone {
    Before,
    AsChild,
    After,
}
