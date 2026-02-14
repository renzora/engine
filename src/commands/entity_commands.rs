//! Entity-related undoable commands.

#![allow(dead_code)]

use bevy::prelude::*;

use crate::core::{EditorEntity, SceneNode, SceneTabId, SelectionState};

use super::command::{Command, CommandContext, CommandResult};

/// Helper to despawn an entity and all its children recursively
fn despawn_with_children_recursive(world: &mut World, entity: Entity) {
    // First collect all children
    let children: Vec<Entity> = world
        .get::<Children>(entity)
        .map(|c| c.iter().collect())
        .unwrap_or_default();

    // Recursively despawn children
    for child in children {
        despawn_with_children_recursive(world, child);
    }

    // Then despawn the entity itself
    world.despawn(entity);
}

// ============================================================================
// Create Entity Command
// ============================================================================

/// Command to create a new entity in the scene
pub struct CreateEntityCommand {
    /// Name for the new entity
    pub name: String,
    /// Parent entity (if any)
    pub parent: Option<Entity>,
    /// Transform for the entity
    pub transform: Transform,
    /// The created entity (set after execution)
    created_entity: Option<Entity>,
    /// Whether to select the entity after creation
    pub select_after_create: bool,
    /// Previous selection (for undo)
    previous_selection: Option<Entity>,
}

impl CreateEntityCommand {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            parent: None,
            transform: Transform::default(),
            created_entity: None,
            select_after_create: true,
            previous_selection: None,
        }
    }

    pub fn with_parent(mut self, parent: Entity) -> Self {
        self.parent = Some(parent);
        self
    }

    pub fn with_transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }

    pub fn select_after(mut self, select: bool) -> Self {
        self.select_after_create = select;
        self
    }

    /// Get the created entity (only valid after execute)
    pub fn entity(&self) -> Option<Entity> {
        self.created_entity
    }
}

impl Command for CreateEntityCommand {
    fn description(&self) -> String {
        format!("Create '{}'", self.name)
    }

    fn execute(&mut self, ctx: &mut CommandContext) -> CommandResult {
        // Store previous selection
        if self.select_after_create {
            let selection = ctx.world.resource::<SelectionState>();
            self.previous_selection = selection.selected_entity;
        }

        // Create the entity
        let mut entity_commands = ctx.world.spawn((
            self.transform,
            Visibility::Inherited,
            EditorEntity {
                name: self.name.clone(),
                tag: String::new(),
                visible: true,
                locked: false,
            },
            SceneNode,
        ));

        // Add parent relationship
        if let Some(parent) = self.parent {
            entity_commands.insert(ChildOf(parent));
        }

        let entity = entity_commands.id();
        self.created_entity = Some(entity);

        // Select if requested
        if self.select_after_create {
            let mut selection = ctx.world.resource_mut::<SelectionState>();
            selection.select(entity);
        }

        CommandResult::Success
    }

    fn undo(&mut self, ctx: &mut CommandContext) -> CommandResult {
        let Some(entity) = self.created_entity else {
            return CommandResult::Failed("No entity to delete".to_string());
        };

        // Restore previous selection
        if self.select_after_create {
            let mut selection = ctx.world.resource_mut::<SelectionState>();
            if selection.selected_entity == Some(entity) {
                if let Some(prev) = self.previous_selection {
                    selection.select(prev);
                } else {
                    selection.clear();
                }
            }
        }

        // Delete the entity and its children
        despawn_with_children_recursive(ctx.world, entity);

        CommandResult::Success
    }

    fn redo(&mut self, ctx: &mut CommandContext) -> CommandResult {
        // Need to create a new entity on redo since the old one was despawned
        self.created_entity = None;
        self.execute(ctx)
    }
}

// ============================================================================
// Delete Entity Command
// ============================================================================

/// Stored data for a deleted entity
#[derive(Clone)]
struct DeletedEntityData {
    name: String,
    tag: String,
    visible: bool,
    locked: bool,
    transform: Transform,
    parent: Option<Entity>,
    // Children are stored as their own DeletedEntityData recursively
    children: Vec<DeletedEntityData>,
}

/// Command to delete an entity from the scene
pub struct DeleteEntityCommand {
    /// Entity to delete
    pub entity: Entity,
    /// Stored data for undo
    deleted_data: Option<DeletedEntityData>,
    /// Previous selection (for undo)
    previous_selection: Option<Entity>,
    /// Recreated entity (for redo tracking)
    recreated_entity: Option<Entity>,
}

impl DeleteEntityCommand {
    pub fn new(entity: Entity) -> Self {
        Self {
            entity,
            deleted_data: None,
            previous_selection: None,
            recreated_entity: None,
        }
    }

    /// Recursively capture entity data for deletion
    fn capture_entity_data(world: &World, entity: Entity) -> Option<DeletedEntityData> {
        let editor_entity = world.get::<EditorEntity>(entity)?;
        let transform = world.get::<Transform>(entity).copied().unwrap_or_default();
        let parent = world.get::<ChildOf>(entity).map(|c| c.0);

        // Capture children
        let mut children = Vec::new();
        if let Some(entity_children) = world.get::<Children>(entity) {
            for child in entity_children.iter() {
                if let Some(child_data) = Self::capture_entity_data(world, child) {
                    children.push(child_data);
                }
            }
        }

        Some(DeletedEntityData {
            name: editor_entity.name.clone(),
            tag: editor_entity.tag.clone(),
            visible: editor_entity.visible,
            locked: editor_entity.locked,
            transform,
            parent,
            children,
        })
    }

    /// Recursively recreate entity from stored data
    fn recreate_entity(world: &mut World, data: &DeletedEntityData, parent: Option<Entity>) -> Entity {
        let visibility = if data.visible { Visibility::Inherited } else { Visibility::Hidden };

        let entity = world.spawn((
            data.transform,
            visibility,
            EditorEntity {
                name: data.name.clone(),
                tag: data.tag.clone(),
                visible: data.visible,
                locked: data.locked,
            },
            SceneNode,
        )).id();

        // Add parent relationship
        if let Some(p) = parent.or(data.parent) {
            world.entity_mut(entity).insert(ChildOf(p));
        }

        // Recursively recreate children with this entity as parent
        for child_data in &data.children {
            Self::recreate_entity(world, child_data, Some(entity));
        }

        entity
    }
}

impl Command for DeleteEntityCommand {
    fn description(&self) -> String {
        if let Some(ref data) = self.deleted_data {
            format!("Delete '{}'", data.name)
        } else {
            "Delete entity".to_string()
        }
    }

    fn execute(&mut self, ctx: &mut CommandContext) -> CommandResult {
        // Use recreated entity if this is a redo
        let entity = self.recreated_entity.unwrap_or(self.entity);

        // Check if entity exists
        if ctx.world.get_entity(entity).is_err() {
            return CommandResult::Failed("Entity does not exist".to_string());
        }

        // Store selection state
        let selection = ctx.world.resource::<SelectionState>();
        self.previous_selection = selection.selected_entity;

        // Capture entity data before deletion (only on first execute)
        if self.deleted_data.is_none() {
            self.deleted_data = Self::capture_entity_data(ctx.world, entity);
        }

        // Clear selection if this entity is selected
        {
            let mut selection = ctx.world.resource_mut::<SelectionState>();
            if selection.is_selected(entity) {
                selection.clear();
            }
        }

        // Delete the entity and its descendants
        despawn_with_children_recursive(ctx.world, entity);

        CommandResult::Success
    }

    fn undo(&mut self, ctx: &mut CommandContext) -> CommandResult {
        let Some(ref data) = self.deleted_data else {
            return CommandResult::Failed("No data to restore".to_string());
        };

        // Recreate the entity
        let entity = Self::recreate_entity(ctx.world, data, None);
        self.recreated_entity = Some(entity);

        // Restore selection
        if self.previous_selection == Some(self.entity) {
            let mut selection = ctx.world.resource_mut::<SelectionState>();
            selection.select(entity);
        }

        CommandResult::Success
    }

    fn redo(&mut self, ctx: &mut CommandContext) -> CommandResult {
        self.execute(ctx)
    }
}

// ============================================================================
// Rename Entity Command
// ============================================================================

/// Command to rename an entity
pub struct RenameEntityCommand {
    pub entity: Entity,
    pub new_name: String,
    old_name: Option<String>,
}

impl RenameEntityCommand {
    pub fn new(entity: Entity, new_name: impl Into<String>) -> Self {
        Self {
            entity,
            new_name: new_name.into(),
            old_name: None,
        }
    }
}

impl Command for RenameEntityCommand {
    fn description(&self) -> String {
        if let Some(ref old) = self.old_name {
            format!("Rename '{}' to '{}'", old, self.new_name)
        } else {
            format!("Rename to '{}'", self.new_name)
        }
    }

    fn execute(&mut self, ctx: &mut CommandContext) -> CommandResult {
        let Some(mut editor_entity) = ctx.world.get_mut::<EditorEntity>(self.entity) else {
            return CommandResult::Failed("Entity not found".to_string());
        };

        // Store old name for undo
        if self.old_name.is_none() {
            self.old_name = Some(editor_entity.name.clone());
        }

        // Check if name actually changed
        if editor_entity.name == self.new_name {
            return CommandResult::NoOp;
        }

        editor_entity.name = self.new_name.clone();
        CommandResult::Success
    }

    fn undo(&mut self, ctx: &mut CommandContext) -> CommandResult {
        let Some(old_name) = &self.old_name else {
            return CommandResult::Failed("No old name stored".to_string());
        };

        let Some(mut editor_entity) = ctx.world.get_mut::<EditorEntity>(self.entity) else {
            return CommandResult::Failed("Entity not found".to_string());
        };

        editor_entity.name = old_name.clone();
        CommandResult::Success
    }

    fn can_merge(&self, other: &dyn Command) -> bool {
        // Merge successive renames of the same entity
        if let Some(other) = (other as &dyn std::any::Any).downcast_ref::<RenameEntityCommand>() {
            other.entity == self.entity
        } else {
            false
        }
    }

    fn merge(&mut self, other: Box<dyn Command>) {
        if let Ok(other) = other.downcast::<RenameEntityCommand>() {
            self.new_name = other.new_name;
        }
    }
}

// ============================================================================
// Reparent Entity Command
// ============================================================================

/// Command to change an entity's parent
pub struct ReparentEntityCommand {
    pub entity: Entity,
    pub new_parent: Option<Entity>,
    old_parent: Option<Entity>,
}

impl ReparentEntityCommand {
    pub fn new(entity: Entity, new_parent: Option<Entity>) -> Self {
        Self {
            entity,
            new_parent,
            old_parent: None,
        }
    }
}

impl Command for ReparentEntityCommand {
    fn description(&self) -> String {
        "Reparent entity".to_string()
    }

    fn execute(&mut self, ctx: &mut CommandContext) -> CommandResult {
        // Store old parent
        if self.old_parent.is_none() {
            self.old_parent = ctx.world.get::<ChildOf>(self.entity).map(|c| c.0);
        }

        // Check if parent actually changed
        let current_parent = ctx.world.get::<ChildOf>(self.entity).map(|c| c.0);
        if current_parent == self.new_parent {
            return CommandResult::NoOp;
        }

        // Remove from old parent
        let mut entity_mut = ctx.world.entity_mut(self.entity);
        entity_mut.remove::<ChildOf>();

        // Add to new parent
        if let Some(new_parent) = self.new_parent {
            entity_mut.insert(ChildOf(new_parent));
        }

        CommandResult::Success
    }

    fn undo(&mut self, ctx: &mut CommandContext) -> CommandResult {
        // Remove current parent
        let mut entity_mut = ctx.world.entity_mut(self.entity);
        entity_mut.remove::<ChildOf>();

        // Restore old parent
        if let Some(old_parent) = self.old_parent {
            entity_mut.insert(ChildOf(old_parent));
        }

        CommandResult::Success
    }
}

// ============================================================================
// Set Transform Command
// ============================================================================

/// Command to set an entity's transform
pub struct SetTransformCommand {
    pub entity: Entity,
    pub new_transform: Transform,
    /// Transform before the change (for undo)
    pub old_transform: Option<Transform>,
    /// Timestamp for merging (rapid changes should merge)
    pub timestamp: f64,
}

impl SetTransformCommand {
    pub fn new(entity: Entity, new_transform: Transform) -> Self {
        Self {
            entity,
            new_transform,
            old_transform: None,
            timestamp: 0.0,
        }
    }

    pub fn with_timestamp(mut self, timestamp: f64) -> Self {
        self.timestamp = timestamp;
        self
    }
}

impl Command for SetTransformCommand {
    fn description(&self) -> String {
        "Transform".to_string()
    }

    fn execute(&mut self, ctx: &mut CommandContext) -> CommandResult {
        let Some(mut transform) = ctx.world.get_mut::<Transform>(self.entity) else {
            return CommandResult::Failed("Entity not found".to_string());
        };

        // Store old transform for undo
        if self.old_transform.is_none() {
            self.old_transform = Some(*transform);
        }

        *transform = self.new_transform;
        CommandResult::Success
    }

    fn undo(&mut self, ctx: &mut CommandContext) -> CommandResult {
        let Some(old_transform) = self.old_transform else {
            return CommandResult::Failed("No old transform stored".to_string());
        };

        let Some(mut transform) = ctx.world.get_mut::<Transform>(self.entity) else {
            return CommandResult::Failed("Entity not found".to_string());
        };

        *transform = old_transform;
        CommandResult::Success
    }

    fn can_merge(&self, other: &dyn Command) -> bool {
        // Merge transform changes on the same entity within 0.5 seconds
        if let Some(other) = (other as &dyn std::any::Any).downcast_ref::<SetTransformCommand>() {
            other.entity == self.entity && (other.timestamp - self.timestamp).abs() < 0.5
        } else {
            false
        }
    }

    fn merge(&mut self, other: Box<dyn Command>) {
        if let Ok(other) = other.downcast::<SetTransformCommand>() {
            self.new_transform = other.new_transform;
            self.timestamp = other.timestamp;
        }
    }
}

// ============================================================================
// Set Selection Command
// ============================================================================

/// Command to change the selection
pub struct SetSelectionCommand {
    pub new_selection: Option<Entity>,
    old_selection: Option<Entity>,
}

impl SetSelectionCommand {
    pub fn new(new_selection: Option<Entity>) -> Self {
        Self {
            new_selection,
            old_selection: None,
        }
    }
}

impl Command for SetSelectionCommand {
    fn description(&self) -> String {
        match self.new_selection {
            Some(_) => "Select".to_string(),
            None => "Deselect".to_string(),
        }
    }

    fn execute(&mut self, ctx: &mut CommandContext) -> CommandResult {
        let mut selection = ctx.world.resource_mut::<SelectionState>();

        // Store old selection
        if self.old_selection.is_none() {
            self.old_selection = selection.selected_entity;
        }

        // Check if selection changed
        if selection.selected_entity == self.new_selection {
            return CommandResult::NoOp;
        }

        match self.new_selection {
            Some(entity) => selection.select(entity),
            None => selection.clear(),
        }

        CommandResult::Success
    }

    fn undo(&mut self, ctx: &mut CommandContext) -> CommandResult {
        let mut selection = ctx.world.resource_mut::<SelectionState>();

        match self.old_selection {
            Some(entity) => selection.select(entity),
            None => selection.clear(),
        }

        CommandResult::Success
    }
}

// ============================================================================
// Set Visibility Command
// ============================================================================

/// Command to change entity visibility
pub struct SetVisibilityCommand {
    pub entity: Entity,
    pub visible: bool,
    old_visible: Option<bool>,
}

impl SetVisibilityCommand {
    pub fn new(entity: Entity, visible: bool) -> Self {
        Self {
            entity,
            visible,
            old_visible: None,
        }
    }
}

impl Command for SetVisibilityCommand {
    fn description(&self) -> String {
        if self.visible {
            "Show".to_string()
        } else {
            "Hide".to_string()
        }
    }

    fn execute(&mut self, ctx: &mut CommandContext) -> CommandResult {
        let Some(mut editor_entity) = ctx.world.get_mut::<EditorEntity>(self.entity) else {
            return CommandResult::Failed("Entity not found".to_string());
        };

        // Store old state
        if self.old_visible.is_none() {
            self.old_visible = Some(editor_entity.visible);
        }

        if editor_entity.visible == self.visible {
            return CommandResult::NoOp;
        }

        editor_entity.visible = self.visible;

        // Also update Bevy visibility
        if let Some(mut visibility) = ctx.world.get_mut::<Visibility>(self.entity) {
            *visibility = if self.visible {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }

        CommandResult::Success
    }

    fn undo(&mut self, ctx: &mut CommandContext) -> CommandResult {
        let Some(old_visible) = self.old_visible else {
            return CommandResult::Failed("No old state stored".to_string());
        };

        let Some(mut editor_entity) = ctx.world.get_mut::<EditorEntity>(self.entity) else {
            return CommandResult::Failed("Entity not found".to_string());
        };

        editor_entity.visible = old_visible;

        if let Some(mut visibility) = ctx.world.get_mut::<Visibility>(self.entity) {
            *visibility = if old_visible {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }

        CommandResult::Success
    }
}

// ============================================================================
// Set Locked Command
// ============================================================================

/// Command to change entity locked state
pub struct SetLockedCommand {
    pub entity: Entity,
    pub locked: bool,
    old_locked: Option<bool>,
}

impl SetLockedCommand {
    pub fn new(entity: Entity, locked: bool) -> Self {
        Self {
            entity,
            locked,
            old_locked: None,
        }
    }
}

impl Command for SetLockedCommand {
    fn description(&self) -> String {
        if self.locked {
            "Lock".to_string()
        } else {
            "Unlock".to_string()
        }
    }

    fn execute(&mut self, ctx: &mut CommandContext) -> CommandResult {
        let Some(mut editor_entity) = ctx.world.get_mut::<EditorEntity>(self.entity) else {
            return CommandResult::Failed("Entity not found".to_string());
        };

        if self.old_locked.is_none() {
            self.old_locked = Some(editor_entity.locked);
        }

        if editor_entity.locked == self.locked {
            return CommandResult::NoOp;
        }

        editor_entity.locked = self.locked;
        CommandResult::Success
    }

    fn undo(&mut self, ctx: &mut CommandContext) -> CommandResult {
        let Some(old_locked) = self.old_locked else {
            return CommandResult::Failed("No old state stored".to_string());
        };

        let Some(mut editor_entity) = ctx.world.get_mut::<EditorEntity>(self.entity) else {
            return CommandResult::Failed("Entity not found".to_string());
        };

        editor_entity.locked = old_locked;
        CommandResult::Success
    }
}

// ============================================================================
// Spawn Mesh Instance Command (for undo of asset drops)
// ============================================================================

use crate::component_system::{
    MeshInstanceData, SceneInstanceData,
    MeshNodeData, Sprite2DData,
    CameraNodeData, CameraRigData, Camera2DData,
    PointLightData, DirectionalLightData, SpotLightData,
    PhysicsBodyData, CollisionShapeData,
    UIPanelData, UILabelData, UIButtonData, UIImageData,
    WorldEnvironmentMarker, HealthData, ScriptComponent,
    SkyboxData, FogData, AntiAliasingData, AmbientOcclusionData,
    ReflectionsData, BloomData, TonemappingData, DepthOfFieldData, MotionBlurData, AmbientLightData,
};

// ============================================================================
// Duplicate Entity Command
// ============================================================================

/// Command to duplicate an entity and all its children
pub struct DuplicateEntityCommand {
    /// Entity to duplicate
    pub entity: Entity,
    /// The duplicated entity (set after execution)
    duplicated_entity: Option<Entity>,
    /// Previous selection (for undo)
    previous_selection: Option<Entity>,
}

impl DuplicateEntityCommand {
    pub fn new(entity: Entity) -> Self {
        Self {
            entity,
            duplicated_entity: None,
            previous_selection: None,
        }
    }

    /// Recursively duplicate an entity and its children
    fn duplicate_recursive(world: &mut World, source: Entity, new_parent: Option<Entity>) -> Option<Entity> {
        // Get source entity data
        let editor_entity = world.get::<EditorEntity>(source)?.clone();
        let transform = world.get::<Transform>(source).copied().unwrap_or_default();
        let scene_tab_id = world.get::<SceneTabId>(source).copied();

        // Generate new name with "(Copy)" suffix
        let new_name = if editor_entity.name.ends_with(" (Copy)") || editor_entity.name.contains(" (Copy ") {
            // Already a copy, increment the number
            if let Some(base) = editor_entity.name.strip_suffix(" (Copy)") {
                format!("{} (Copy 2)", base)
            } else if let Some(idx) = editor_entity.name.rfind(" (Copy ") {
                let base = &editor_entity.name[..idx];
                let suffix = &editor_entity.name[idx + 7..];
                if let Some(num_str) = suffix.strip_suffix(')') {
                    if let Ok(num) = num_str.parse::<u32>() {
                        format!("{} (Copy {})", base, num + 1)
                    } else {
                        format!("{} (Copy)", editor_entity.name)
                    }
                } else {
                    format!("{} (Copy)", editor_entity.name)
                }
            } else {
                format!("{} (Copy)", editor_entity.name)
            }
        } else {
            format!("{} (Copy)", editor_entity.name)
        };

        // Create the new entity with base components
        let visibility = if editor_entity.visible { Visibility::Inherited } else { Visibility::Hidden };
        let mut new_entity = world.spawn((
            transform,
            visibility,
            EditorEntity {
                name: new_name,
                tag: editor_entity.tag.clone(),
                visible: editor_entity.visible,
                locked: editor_entity.locked,
            },
            SceneNode,
        ));

        // Add scene tab ID if present
        if let Some(tab_id) = scene_tab_id {
            new_entity.insert(tab_id);
        }

        // Add parent relationship
        if let Some(parent) = new_parent {
            new_entity.insert(ChildOf(parent));
        }

        let new_id = new_entity.id();

        // Copy optional components - we need to clone them before inserting
        // to avoid borrowing issues

        // Mesh components
        if let Some(data) = world.get::<MeshNodeData>(source).cloned() {
            world.entity_mut(new_id).insert(data);
        }
        if let Some(data) = world.get::<MeshInstanceData>(source).cloned() {
            world.entity_mut(new_id).insert(data);
        }
        if let Some(data) = world.get::<SceneInstanceData>(source).cloned() {
            world.entity_mut(new_id).insert(data);
        }

        // 2D rendering
        if let Some(data) = world.get::<Sprite2DData>(source).cloned() {
            world.entity_mut(new_id).insert(data);
        }

        // Camera components
        if let Some(data) = world.get::<CameraNodeData>(source).cloned() {
            // Don't copy is_default_camera flag - only one camera can be default
            let mut cam_data = data;
            cam_data.is_default_camera = false;
            world.entity_mut(new_id).insert(cam_data);
        }
        if let Some(data) = world.get::<CameraRigData>(source).cloned() {
            let mut cam_data = data;
            cam_data.is_default_camera = false;
            world.entity_mut(new_id).insert(cam_data);
        }
        if let Some(data) = world.get::<Camera2DData>(source).cloned() {
            let mut cam_data = data;
            cam_data.is_default_camera = false;
            world.entity_mut(new_id).insert(cam_data);
        }

        // Light components
        if let Some(data) = world.get::<PointLightData>(source).cloned() {
            world.entity_mut(new_id).insert(data);
        }
        if let Some(data) = world.get::<DirectionalLightData>(source).cloned() {
            world.entity_mut(new_id).insert(data);
        }
        if let Some(data) = world.get::<SpotLightData>(source).cloned() {
            world.entity_mut(new_id).insert(data);
        }

        // Physics components
        if let Some(data) = world.get::<PhysicsBodyData>(source).cloned() {
            world.entity_mut(new_id).insert(data);
        }
        if let Some(data) = world.get::<CollisionShapeData>(source).cloned() {
            world.entity_mut(new_id).insert(data);
        }

        // UI components
        if let Some(data) = world.get::<UIPanelData>(source).cloned() {
            world.entity_mut(new_id).insert(data);
        }
        if let Some(data) = world.get::<UILabelData>(source).cloned() {
            world.entity_mut(new_id).insert(data);
        }
        if let Some(data) = world.get::<UIButtonData>(source).cloned() {
            world.entity_mut(new_id).insert(data);
        }
        if let Some(data) = world.get::<UIImageData>(source).cloned() {
            world.entity_mut(new_id).insert(data);
        }

        // Environment
        if let Some(data) = world.get::<WorldEnvironmentMarker>(source).cloned() {
            world.entity_mut(new_id).insert(data);
        }

        // Post-processing components
        if let Some(data) = world.get::<SkyboxData>(source).cloned() {
            world.entity_mut(new_id).insert(data);
        }
        if let Some(data) = world.get::<FogData>(source).cloned() {
            world.entity_mut(new_id).insert(data);
        }
        if let Some(data) = world.get::<AntiAliasingData>(source).cloned() {
            world.entity_mut(new_id).insert(data);
        }
        if let Some(data) = world.get::<AmbientOcclusionData>(source).cloned() {
            world.entity_mut(new_id).insert(data);
        }
        if let Some(data) = world.get::<ReflectionsData>(source).cloned() {
            world.entity_mut(new_id).insert(data);
        }
        if let Some(data) = world.get::<BloomData>(source).cloned() {
            world.entity_mut(new_id).insert(data);
        }
        if let Some(data) = world.get::<TonemappingData>(source).cloned() {
            world.entity_mut(new_id).insert(data);
        }
        if let Some(data) = world.get::<DepthOfFieldData>(source).cloned() {
            world.entity_mut(new_id).insert(data);
        }
        if let Some(data) = world.get::<MotionBlurData>(source).cloned() {
            world.entity_mut(new_id).insert(data);
        }
        if let Some(data) = world.get::<AmbientLightData>(source).cloned() {
            world.entity_mut(new_id).insert(data);
        }

        // Gameplay
        if let Some(data) = world.get::<HealthData>(source).cloned() {
            world.entity_mut(new_id).insert(data);
        }

        // Scripting - reset runtime state
        if let Some(data) = world.get::<ScriptComponent>(source).cloned() {
            let mut script = data;
            script.runtime_state = Default::default();
            world.entity_mut(new_id).insert(script);
        }

        // Recursively duplicate children
        let children: Vec<Entity> = world
            .get::<Children>(source)
            .map(|c| c.iter().collect())
            .unwrap_or_default();

        for child in children {
            // Only duplicate editor entities (not internal Bevy components)
            if world.get::<EditorEntity>(child).is_some() {
                Self::duplicate_recursive(world, child, Some(new_id));
            }
        }

        Some(new_id)
    }
}

impl Command for DuplicateEntityCommand {
    fn description(&self) -> String {
        "Duplicate".to_string()
    }

    fn execute(&mut self, ctx: &mut CommandContext) -> CommandResult {
        // Check if source entity exists
        if ctx.world.get_entity(self.entity).is_err() {
            return CommandResult::Failed("Entity does not exist".to_string());
        }

        // Store previous selection
        let selection = ctx.world.resource::<SelectionState>();
        self.previous_selection = selection.selected_entity;

        // Get the parent of the source entity (duplicate will be a sibling)
        let parent = ctx.world.get::<ChildOf>(self.entity).map(|c| c.0);

        // Duplicate the entity tree
        let Some(new_entity) = Self::duplicate_recursive(ctx.world, self.entity, parent) else {
            return CommandResult::Failed("Failed to duplicate entity".to_string());
        };

        self.duplicated_entity = Some(new_entity);

        // Offset the duplicate slightly so it's visible
        if let Some(mut transform) = ctx.world.get_mut::<Transform>(new_entity) {
            transform.translation += Vec3::new(0.5, 0.0, 0.5);
        }

        // Select the duplicated entity
        let mut selection = ctx.world.resource_mut::<SelectionState>();
        selection.select(new_entity);

        CommandResult::Success
    }

    fn undo(&mut self, ctx: &mut CommandContext) -> CommandResult {
        let Some(entity) = self.duplicated_entity else {
            return CommandResult::Failed("No duplicated entity to delete".to_string());
        };

        // Check if entity still exists
        if ctx.world.get_entity(entity).is_err() {
            return CommandResult::Failed("Duplicated entity no longer exists".to_string());
        }

        // Restore previous selection
        {
            let mut selection = ctx.world.resource_mut::<SelectionState>();
            if selection.selected_entity == Some(entity) {
                if let Some(prev) = self.previous_selection {
                    selection.select(prev);
                } else {
                    selection.clear();
                }
            }
        }

        // Delete the duplicated entity and its children
        despawn_with_children_recursive(ctx.world, entity);
        self.duplicated_entity = None;

        CommandResult::Success
    }

    fn redo(&mut self, ctx: &mut CommandContext) -> CommandResult {
        self.execute(ctx)
    }
}

/// Command to track a spawned mesh instance (for undo support)
/// This is created AFTER the entity is spawned, so execute() is a no-op.
pub struct SpawnMeshInstanceCommand {
    /// The spawned entity
    pub entity: Entity,
    /// Entity name
    pub name: String,
    /// Transform of the entity
    pub transform: Transform,
    /// Model path for respawning
    pub model_path: Option<String>,
    /// Parent entity (if any)
    pub parent: Option<Entity>,
    /// Whether the entity still exists (false after undo)
    entity_exists: bool,
}

impl SpawnMeshInstanceCommand {
    pub fn new(
        entity: Entity,
        name: String,
        transform: Transform,
        model_path: Option<String>,
        parent: Option<Entity>,
    ) -> Self {
        Self {
            entity,
            name,
            transform,
            model_path,
            parent,
            entity_exists: true,
        }
    }
}

impl Command for SpawnMeshInstanceCommand {
    fn description(&self) -> String {
        format!("Spawn {}", self.name)
    }

    fn execute(&mut self, _ctx: &mut CommandContext) -> CommandResult {
        // Entity was already spawned before this command was created
        // Just mark that it exists
        self.entity_exists = true;
        CommandResult::Success
    }

    fn undo(&mut self, ctx: &mut CommandContext) -> CommandResult {
        if !self.entity_exists {
            return CommandResult::NoOp;
        }

        // Check if entity still exists
        if ctx.world.get_entity(self.entity).is_err() {
            return CommandResult::Failed("Entity no longer exists".to_string());
        }

        // Clear selection if this entity was selected
        {
            let mut selection = ctx.world.resource_mut::<SelectionState>();
            if selection.selected_entity == Some(self.entity) {
                selection.selected_entity = None;
            }
        }

        // Despawn the entity and all children
        despawn_with_children_recursive(ctx.world, self.entity);
        self.entity_exists = false;

        CommandResult::Success
    }

    fn redo(&mut self, ctx: &mut CommandContext) -> CommandResult {
        if self.entity_exists {
            return CommandResult::NoOp;
        }

        // Check if parent exists before spawning
        let parent_exists = self.parent
            .map(|p| ctx.world.get_entity(p).is_ok())
            .unwrap_or(false);

        // Respawn the MeshInstance entity
        let new_entity = ctx.world.spawn((
            self.transform,
            Visibility::default(),
            EditorEntity {
                name: self.name.clone(),
                tag: String::new(),
                visible: true,
                locked: false,
            },
            SceneNode,
            MeshInstanceData {
                model_path: self.model_path.clone(),
            },
        )).id();

        // Parent to scene root if needed
        if let Some(parent) = self.parent {
            if parent_exists {
                ctx.world.entity_mut(new_entity).insert(ChildOf(parent));
            }
        }

        // Update our entity reference
        self.entity = new_entity;
        self.entity_exists = true;

        // Select the respawned entity
        ctx.world.resource_mut::<SelectionState>().select(self.entity);

        // Note: The model will be reloaded by check_mesh_instance_models system
        // since the entity has MeshInstanceData but no MeshInstanceModelLoading marker

        CommandResult::Success
    }
}

// ============================================================================
// Group Entities Command (Create Parent)
// ============================================================================

/// Command to group selected entities under a new parent entity
pub struct GroupEntitiesCommand {
    /// Entities to group
    pub entities: Vec<Entity>,
    /// The created parent entity (set after execution)
    created_parent: Option<Entity>,
    /// Original parents of each entity (for undo)
    original_parents: Vec<(Entity, Option<Entity>)>,
    /// Previous selection (for undo)
    previous_selection: Option<Entity>,
}

impl GroupEntitiesCommand {
    pub fn new(entities: Vec<Entity>) -> Self {
        Self {
            entities,
            created_parent: None,
            original_parents: Vec::new(),
            previous_selection: None,
        }
    }
}

impl Command for GroupEntitiesCommand {
    fn description(&self) -> String {
        format!("Group {} entities", self.entities.len())
    }

    fn execute(&mut self, ctx: &mut CommandContext) -> CommandResult {
        if self.entities.is_empty() {
            return CommandResult::Failed("No entities to group".to_string());
        }

        // Store previous selection
        let selection = ctx.world.resource::<SelectionState>();
        self.previous_selection = selection.selected_entity;

        // Store original parents
        self.original_parents.clear();
        for &entity in &self.entities {
            let parent = ctx.world.get::<ChildOf>(entity).map(|c| c.0);
            self.original_parents.push((entity, parent));
        }

        // Find the common parent (if all share the same parent, use that; otherwise root)
        let first_parent = self.original_parents.first().and_then(|(_, p)| *p);
        let common_parent = if self.original_parents.iter().all(|(_, p)| *p == first_parent) {
            first_parent
        } else {
            None
        };

        // Compute center position of all entities
        let mut center = Vec3::ZERO;
        let mut count = 0;
        for &entity in &self.entities {
            if let Some(transform) = ctx.world.get::<Transform>(entity) {
                center += transform.translation;
                count += 1;
            }
        }
        if count > 0 {
            center /= count as f32;
        }

        // Create the parent entity
        let mut parent_commands = ctx.world.spawn((
            Transform::from_translation(center),
            Visibility::Inherited,
            EditorEntity {
                name: "Group".to_string(),
                tag: String::new(),
                visible: true,
                locked: false,
            },
            SceneNode,
        ));

        if let Some(common) = common_parent {
            parent_commands.insert(ChildOf(common));
        }

        let parent_entity = parent_commands.id();
        self.created_parent = Some(parent_entity);

        // Reparent all entities under the new parent, adjusting transforms to be relative
        for &entity in &self.entities {
            // Offset child transform so world position stays the same
            if let Some(mut transform) = ctx.world.get_mut::<Transform>(entity) {
                transform.translation -= center;
            }
            let mut entity_mut = ctx.world.entity_mut(entity);
            entity_mut.remove::<ChildOf>();
            entity_mut.insert(ChildOf(parent_entity));
        }

        // Select the new parent
        let mut selection = ctx.world.resource_mut::<SelectionState>();
        selection.select(parent_entity);

        CommandResult::Success
    }

    fn undo(&mut self, ctx: &mut CommandContext) -> CommandResult {
        let Some(parent_entity) = self.created_parent else {
            return CommandResult::Failed("No parent entity to undo".to_string());
        };

        // Get the parent's translation for restoring child positions
        let parent_translation = ctx.world.get::<Transform>(parent_entity)
            .map(|t| t.translation)
            .unwrap_or(Vec3::ZERO);

        // Restore original parents and positions
        for (entity, original_parent) in &self.original_parents {
            if ctx.world.get_entity(*entity).is_err() {
                continue;
            }
            // Restore world position
            if let Some(mut transform) = ctx.world.get_mut::<Transform>(*entity) {
                transform.translation += parent_translation;
            }
            let mut entity_mut = ctx.world.entity_mut(*entity);
            entity_mut.remove::<ChildOf>();
            if let Some(parent) = original_parent {
                entity_mut.insert(ChildOf(*parent));
            }
        }

        // Restore previous selection
        let mut selection = ctx.world.resource_mut::<SelectionState>();
        if let Some(prev) = self.previous_selection {
            selection.select(prev);
        } else {
            selection.clear();
        }

        // Despawn the parent entity
        ctx.world.despawn(parent_entity);
        self.created_parent = None;

        CommandResult::Success
    }

    fn redo(&mut self, ctx: &mut CommandContext) -> CommandResult {
        self.created_parent = None;
        self.execute(ctx)
    }
}

// ============================================================================
// Downcast helper - uses Any trait bound
// ============================================================================

impl dyn Command {
    /// Attempt to downcast to a concrete command type
    pub fn downcast<T: Command + 'static>(self: Box<Self>) -> Result<Box<T>, Box<Self>> {
        if std::any::Any::type_id(&*self) == std::any::TypeId::of::<T>() {
            unsafe {
                let raw = Box::into_raw(self);
                Ok(Box::from_raw(raw as *mut T))
            }
        } else {
            Err(self)
        }
    }
}
