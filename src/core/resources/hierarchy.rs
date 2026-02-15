#![allow(dead_code)]

use bevy::prelude::*;
use std::collections::HashSet;

/// State for the hierarchy panel
#[derive(Resource, Default)]
pub struct HierarchyState {
    /// Entities that should be expanded in the hierarchy tree
    pub expanded_entities: HashSet<Entity>,
    /// Entity being dragged in the hierarchy (primary drag entity)
    pub drag_entity: Option<Entity>,
    /// All entities being dragged (for multi-selection drag)
    pub drag_entities: Vec<Entity>,
    /// Current drop target for hierarchy drag
    pub drop_target: Option<HierarchyDropTarget>,
    /// Entity currently being renamed (inline editing)
    pub renaming_entity: Option<Entity>,
    /// Buffer for the rename text input
    pub rename_buffer: String,
    /// Whether we've already requested focus for the rename text edit
    pub rename_focus_set: bool,
    /// Pending request to make a camera the default game camera
    pub pending_make_default_camera: Option<Entity>,
    /// Pending request to snap a camera entity to the current viewport position
    pub pending_snap_to_viewport: Option<Entity>,
    /// Visible entities in order (for Shift+click range selection)
    /// This is the order from the PREVIOUS frame, used for click handling
    pub visible_entity_order: Vec<Entity>,
    /// New visible entity order being built during current frame
    pub building_entity_order: Vec<Entity>,
    /// Search filter for hierarchy
    pub search: String,
    /// Whether the "Add Entity" popup is visible
    pub show_add_entity_popup: bool,
    /// Search text within the "Add Entity" popup
    pub add_entity_search: String,
    /// Parent entity for the new entity (None = scene root)
    pub add_entity_parent: Option<Entity>,
    /// Request focus on the search box next frame
    pub add_entity_focus_search: bool,
    /// Entity that a script/blueprint asset drag is hovering over in the hierarchy
    pub script_drop_target: Option<Entity>,
}

impl HierarchyState {
    /// Toggle expansion state of an entity
    pub fn toggle_expanded(&mut self, entity: Entity) {
        if self.expanded_entities.contains(&entity) {
            self.expanded_entities.remove(&entity);
        } else {
            self.expanded_entities.insert(entity);
        }
    }

    /// Check if an entity is expanded
    pub fn is_expanded(&self, entity: Entity) -> bool {
        self.expanded_entities.contains(&entity)
    }

    /// Expand an entity
    pub fn expand(&mut self, entity: Entity) {
        self.expanded_entities.insert(entity);
    }

    /// Collapse an entity
    pub fn collapse(&mut self, entity: Entity) {
        self.expanded_entities.remove(&entity);
    }

    /// Start dragging entities (supports multi-selection)
    pub fn start_drag(&mut self, entities: Vec<Entity>) {
        if let Some(&first) = entities.first() {
            self.drag_entity = Some(first);
            self.drag_entities = entities;
        }
    }

    /// Clear drag state
    pub fn clear_drag(&mut self) {
        self.drag_entity = None;
        self.drag_entities.clear();
        self.drop_target = None;
    }

    /// Check if an entity is being dragged
    pub fn is_being_dragged(&self, entity: Entity) -> bool {
        self.drag_entities.contains(&entity)
    }
}

/// Where to drop a dragged hierarchy node
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HierarchyDropPosition {
    /// Insert before this entity (as sibling)
    Before,
    /// Insert after this entity (as sibling)
    After,
    /// Insert as child of this entity
    AsChild,
}

/// Drop target for hierarchy drag and drop
#[derive(Clone, Copy, Debug)]
pub struct HierarchyDropTarget {
    pub entity: Entity,
    pub position: HierarchyDropPosition,
}
