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
    /// Visible entities in order (for Shift+click range selection)
    /// This is the order from the PREVIOUS frame, used for click handling
    pub visible_entity_order: Vec<Entity>,
    /// New visible entity order being built during current frame
    pub building_entity_order: Vec<Entity>,
    /// Search filter for hierarchy
    pub search: String,
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
