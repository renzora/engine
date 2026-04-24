use bevy::prelude::Entity;

/// Persistent UI state for the inspector panel.
#[derive(Default)]
pub struct InspectorState {
    pub show_add_overlay: bool,
    pub add_search: String,
    pub component_filter: String,
    /// When set, the inspector pins to this entity and ignores selection changes.
    pub locked_entity: Option<Entity>,
}
