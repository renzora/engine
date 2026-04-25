//! Command-based undo/redo.
//!
//! Every user action is represented as an `UndoCommand`. Call sites do NOT
//! mutate directly — they build a command and pass it to
//! `UndoStacks::execute`, which applies it and stores it on the stack.
//! Redo replays `execute`; undo runs the command's `undo`.
//!
//! Shortcuts Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z operate on `UndoStacks::active`.

use std::any::Any;
use std::collections::{HashMap, VecDeque};

use bevy::prelude::*;
use renzora::{MeshColor, MeshPrimitive, ShapeRegistry};
use renzora_editor_framework::{
    EditorLocked, EditorSelection, FieldValue, InspectorRegistry, SpawnRegistry,
};

// ── Public API ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum UndoContext {
    Scene,
    MaterialGraph(String),
    Blueprint(String),
    Lifecycle,
    Other(String),
}

impl Default for UndoContext {
    fn default() -> Self { UndoContext::Scene }
}

/// A single undoable action. `execute` is called on initial push AND on redo.
/// `undo` reverses the action. Both take `&mut self` so the command can
/// refresh captured state (e.g. update an entity id after respawn).
pub trait UndoCommand: Any + Send + Sync {
    fn label(&self) -> &str { "edit" }
    fn execute(&mut self, world: &mut World);
    fn undo(&mut self, world: &mut World);
    fn merge(&mut self, _other: &dyn UndoCommand) -> bool { false }
}

// ── Stacks ─────────────────────────────────────────────────────────────────

#[derive(Resource, Default)]
pub struct UndoStacks {
    stacks: HashMap<UndoContext, ContextStack>,
    pub active: UndoContext,
}

#[derive(Default)]
struct ContextStack {
    undo: VecDeque<Box<dyn UndoCommand>>,
    redo: VecDeque<Box<dyn UndoCommand>>,
}

const MAX_DEPTH: usize = 500;

impl UndoStacks {
    pub fn clear(&mut self, context: &UndoContext) {
        if let Some(s) = self.stacks.get_mut(context) {
            s.undo.clear();
            s.redo.clear();
        }
    }
    pub fn clear_all(&mut self) { self.stacks.clear(); }
    pub fn can_undo(&self, context: &UndoContext) -> bool {
        self.stacks.get(context).map_or(false, |s| !s.undo.is_empty())
    }
    pub fn can_redo(&self, context: &UndoContext) -> bool {
        self.stacks.get(context).map_or(false, |s| !s.redo.is_empty())
    }
    /// Returns `(undo_labels, redo_labels)` for the given context.
    /// `undo` is ordered front=oldest → back=most recent;
    /// `redo` is ordered front=oldest-undone → back=next-to-redo.
    pub fn labels(&self, context: &UndoContext) -> (Vec<String>, Vec<String>) {
        self.stacks
            .get(context)
            .map(|s| {
                (
                    s.undo.iter().map(|c| c.label().to_string()).collect(),
                    s.redo.iter().map(|c| c.label().to_string()).collect(),
                )
            })
            .unwrap_or_default()
    }
}

/// Execute `cmd` and push it onto the active (or supplied) stack.
///
/// Prefer this over mutating the world directly — it's the single entry
/// point that keeps the history in sync with the session.
pub fn execute(world: &mut World, context: UndoContext, mut cmd: Box<dyn UndoCommand>) {
    cmd.execute(world);
    record(world, context, cmd);
}

/// Push `cmd` onto the stack WITHOUT executing it. Use when the mutation
/// has already happened via code that can't easily be expressed as a single
/// command (e.g. complex reparent with sibling index preservation).
pub fn record(world: &mut World, context: UndoContext, cmd: Box<dyn UndoCommand>) {
    world.resource_scope(|_w, mut stacks: Mut<UndoStacks>| {
        let stack = stacks.stacks.entry(context).or_default();
        if let Some(back) = stack.undo.back_mut() {
            if back.merge(cmd.as_ref()) {
                stack.redo.clear();
                return;
            }
        }
        stack.undo.push_back(cmd);
        stack.redo.clear();
        while stack.undo.len() > MAX_DEPTH { stack.undo.pop_front(); }
    });
}

// ── Messages ───────────────────────────────────────────────────────────────

#[derive(Message)]
pub struct RequestUndo;

#[derive(Message)]
pub struct RequestRedo;

#[derive(Message)]
pub struct UndoExhausted;

// ── Plugin ─────────────────────────────────────────────────────────────────

pub struct UndoPlugin;

impl Plugin for UndoPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UndoStacks>()
            .add_message::<RequestUndo>()
            .add_message::<RequestRedo>()
            .add_message::<UndoExhausted>()
            // Ensure the hooks resource exists regardless of plugin order —
            // RenzoraEditorPlugin also initialises it, but we can't rely on
            // that running first.
            .init_resource::<renzora_editor_framework::EditorActionHooks>()
            .add_systems(Update, (shortcut_input, handle_undo, handle_redo).chain());

        // Register undo/redo as late-bound hooks so the editor framework's
        // title bar / menu handlers can invoke them without taking a
        // dependency on this crate (which would create a cycle).
        let mut hooks = app
            .world_mut()
            .resource_mut::<renzora_editor_framework::EditorActionHooks>();
        hooks.undo = Some(undo_once);
        hooks.redo = Some(redo_once);
        hooks.can_undo = Some(can_undo_active);
        hooks.can_redo = Some(can_redo_active);
    }
}

fn can_undo_active(world: &World) -> bool {
    world
        .get_resource::<UndoStacks>()
        .map(|s| s.can_undo(&s.active))
        .unwrap_or(false)
}

fn can_redo_active(world: &World) -> bool {
    world
        .get_resource::<UndoStacks>()
        .map(|s| s.can_redo(&s.active))
        .unwrap_or(false)
}

fn shortcut_input(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Option<Res<renzora::keybindings::KeyBindings>>,
    mut undo_w: MessageWriter<RequestUndo>,
    mut redo_w: MessageWriter<RequestRedo>,
) {
    // Route through KeyBindings so:
    //   - User-rebound Undo/Redo keys are respected
    //   - Command palette dispatches (KeyBindings::dispatch) fire the messages
    let Some(bindings) = bindings else { return };
    use renzora::keybindings::EditorAction;
    if bindings.just_pressed(EditorAction::Undo, &keys) {
        undo_w.write(RequestUndo);
    }
    if bindings.just_pressed(EditorAction::Redo, &keys) {
        redo_w.write(RequestRedo);
    }
}

/// Undo the most recent action on the active stack. Callable from anywhere
/// with `&mut World` — bypasses the message bus so it works from deferred
/// callers (toolbar clicks, menu items, command palette) without frame-timing
/// concerns.
pub fn undo_once(world: &mut World) {
    let active = world.resource::<UndoStacks>().active.clone();
    let cmd = world.resource_mut::<UndoStacks>().stacks
        .get_mut(&active).and_then(|s| s.undo.pop_back());
    let Some(mut cmd) = cmd else { world.write_message(UndoExhausted); return; };
    cmd.undo(world);
    if let Some(s) = world.resource_mut::<UndoStacks>().stacks.get_mut(&active) {
        s.redo.push_back(cmd);
    }
}

/// Redo the most recently undone action on the active stack.
pub fn redo_once(world: &mut World) {
    let active = world.resource::<UndoStacks>().active.clone();
    let cmd = world.resource_mut::<UndoStacks>().stacks
        .get_mut(&active).and_then(|s| s.redo.pop_back());
    let Some(mut cmd) = cmd else { world.write_message(UndoExhausted); return; };
    cmd.execute(world);
    if let Some(s) = world.resource_mut::<UndoStacks>().stacks.get_mut(&active) {
        s.undo.push_back(cmd);
    }
}

fn handle_undo(world: &mut World) {
    let count = world.get_resource::<Messages<RequestUndo>>()
        .map(|m| m.iter_current_update_messages().count()).unwrap_or(0);
    if count == 0 { return; }
    undo_once(world);
}

fn handle_redo(world: &mut World) {
    let count = world.get_resource::<Messages<RequestRedo>>()
        .map(|m| m.iter_current_update_messages().count()).unwrap_or(0);
    if count == 0 { return; }
    redo_once(world);
}

// ──────────────────────────────────────────────────────────────────────────
// Built-in commands for common scene operations.
// Plugins may use these or define their own.
// ──────────────────────────────────────────────────────────────────────────

pub struct SpawnShapeCmd {
    pub entity: Entity,
    pub shape_id: String,
    pub name: String,
    pub position: Vec3,
    pub color: Color,
}

impl UndoCommand for SpawnShapeCmd {
    fn label(&self) -> &str { "Spawn shape" }
    fn execute(&mut self, world: &mut World) {
        let Some(create_mesh) = world
            .resource::<ShapeRegistry>()
            .get(&self.shape_id)
            .map(|e| e.create_mesh) else { return };
        let mesh = create_mesh(&mut world.resource_mut::<Assets<Mesh>>());
        let material = world.resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial {
            base_color: self.color,
            perceptual_roughness: 0.9,
            ..default()
        });
        self.entity = world.spawn((
            Name::new(self.name.clone()),
            Transform::from_translation(self.position),
            Mesh3d(mesh),
            MeshMaterial3d(material),
            MeshPrimitive(self.shape_id.clone()),
            MeshColor(self.color),
        )).id();
        if let Some(sel) = world.get_resource::<EditorSelection>() { sel.set(Some(self.entity)); }
    }
    fn undo(&mut self, world: &mut World) {
        if let Some(sel) = world.get_resource::<EditorSelection>() {
            if sel.get() == Some(self.entity) { sel.clear(); }
        }
        if let Ok(e) = world.get_entity_mut(self.entity) { e.despawn(); }
    }
}

pub struct DeleteShapesCmd {
    pub items: Vec<DeletedShape>,
}

pub struct DeletedShape {
    pub entity: Entity,
    pub shape_id: String,
    pub name: String,
    pub transform: Transform,
    pub color: Color,
}

impl UndoCommand for DeleteShapesCmd {
    fn label(&self) -> &str { "Delete" }
    fn execute(&mut self, world: &mut World) {
        if let Some(sel) = world.get_resource::<EditorSelection>() {
            let selected = sel.get_all();
            if self.items.iter().any(|i| selected.contains(&i.entity)) { sel.clear(); }
        }
        for item in &self.items {
            if let Ok(e) = world.get_entity_mut(item.entity) { e.despawn(); }
        }
    }
    fn undo(&mut self, world: &mut World) {
        for item in self.items.iter_mut() {
            let Some(create_mesh) = world.resource::<ShapeRegistry>()
                .get(&item.shape_id).map(|e| e.create_mesh) else { continue };
            let mesh = create_mesh(&mut world.resource_mut::<Assets<Mesh>>());
            let material = world.resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial {
                base_color: item.color, perceptual_roughness: 0.9, ..default()
            });
            item.entity = world.spawn((
                Name::new(item.name.clone()),
                item.transform,
                Mesh3d(mesh),
                MeshMaterial3d(material),
                MeshPrimitive(item.shape_id.clone()),
                MeshColor(item.color),
            )).id();
        }
    }
}

pub struct TransformCmd {
    pub entity: Entity,
    pub old: Transform,
    pub new: Transform,
}

impl UndoCommand for TransformCmd {
    fn label(&self) -> &str { "Transform" }
    fn execute(&mut self, world: &mut World) {
        if let Ok(mut e) = world.get_entity_mut(self.entity) {
            if let Some(mut t) = e.get_mut::<Transform>() { *t = self.new; }
        }
    }
    fn undo(&mut self, world: &mut World) {
        if let Ok(mut e) = world.get_entity_mut(self.entity) {
            if let Some(mut t) = e.get_mut::<Transform>() { *t = self.old; }
        }
    }
}

pub struct RenameCmd {
    pub entity: Entity,
    pub old: String,
    pub new: String,
}

impl UndoCommand for RenameCmd {
    fn label(&self) -> &str { "Rename" }
    fn execute(&mut self, world: &mut World) {
        if let Some(mut n) = world.get_mut::<Name>(self.entity) { *n = Name::new(self.new.clone()); }
    }
    fn undo(&mut self, world: &mut World) {
        if let Some(mut n) = world.get_mut::<Name>(self.entity) { *n = Name::new(self.old.clone()); }
    }
}

pub struct SetHierarchyOrderCmd {
    pub entity: Entity,
    pub old: Option<u32>,
    pub new: Option<u32>,
}

impl UndoCommand for SetHierarchyOrderCmd {
    fn label(&self) -> &str { "Reorder" }
    fn execute(&mut self, world: &mut World) { apply_order(world, self.entity, self.new); }
    fn undo(&mut self, world: &mut World) { apply_order(world, self.entity, self.old); }
}

fn apply_order(world: &mut World, entity: Entity, order: Option<u32>) {
    let Ok(mut e) = world.get_entity_mut(entity) else { return };
    match order {
        Some(o) => { e.insert(renzora_editor_framework::HierarchyOrder(o)); }
        None => { e.remove::<renzora_editor_framework::HierarchyOrder>(); }
    }
}

pub struct ReparentCmd {
    pub entity: Entity,
    pub old_parent: Option<Entity>,
    pub new_parent: Option<Entity>,
}

impl UndoCommand for ReparentCmd {
    fn label(&self) -> &str { "Reparent" }
    fn execute(&mut self, world: &mut World) { apply_parent(world, self.entity, self.new_parent); }
    fn undo(&mut self, world: &mut World) { apply_parent(world, self.entity, self.old_parent); }
}

fn apply_parent(world: &mut World, entity: Entity, parent: Option<Entity>) {
    let Ok(mut e) = world.get_entity_mut(entity) else { return };
    match parent {
        Some(p) => { e.set_parent_in_place(p); }
        None => { e.remove_parent_in_place(); }
    }
}

pub struct LockToggleCmd {
    pub entity: Entity,
    pub was_locked: bool,
}

impl UndoCommand for LockToggleCmd {
    fn label(&self) -> &str { "Toggle lock" }
    fn execute(&mut self, world: &mut World) { set_locked(world, self.entity, !self.was_locked); }
    fn undo(&mut self, world: &mut World) { set_locked(world, self.entity, self.was_locked); }
}

fn set_locked(world: &mut World, entity: Entity, locked: bool) {
    let Ok(mut e) = world.get_entity_mut(entity) else { return };
    if locked { e.insert(EditorLocked); } else { e.remove::<EditorLocked>(); }
}

pub struct VisibilityToggleCmd {
    pub entity: Entity,
    pub was_visible: bool,
}

impl UndoCommand for VisibilityToggleCmd {
    fn label(&self) -> &str { "Toggle visibility" }
    fn execute(&mut self, world: &mut World) { set_visibility(world, self.entity, !self.was_visible); }
    fn undo(&mut self, world: &mut World) { set_visibility(world, self.entity, self.was_visible); }
}

fn set_visibility(world: &mut World, entity: Entity, visible: bool) {
    if let Some(mut v) = world.get_mut::<Visibility>(entity) {
        *v = if visible { Visibility::Inherited } else { Visibility::Hidden };
    }
}

pub struct FieldChangeCmd {
    pub entity: Entity,
    pub field_name: &'static str,
    pub old: FieldValue,
    pub new: FieldValue,
    pub set_fn: fn(&mut World, Entity, FieldValue),
}

impl UndoCommand for FieldChangeCmd {
    fn label(&self) -> &str { self.field_name }
    fn execute(&mut self, world: &mut World) { (self.set_fn)(world, self.entity, self.new.clone()); }
    fn undo(&mut self, world: &mut World) { (self.set_fn)(world, self.entity, self.old.clone()); }
    fn merge(&mut self, other: &dyn UndoCommand) -> bool {
        let any: &dyn Any = other;
        let Some(o) = any.downcast_ref::<FieldChangeCmd>() else { return false };
        if o.entity != self.entity || o.field_name != self.field_name { return false; }
        self.new = o.new.clone();
        true
    }
}

/// Bundles multiple commands into a single undo entry. `execute` runs each
/// in order; `undo` runs them in reverse. Use for anything that's logically
/// one user action but expands into N mutations (multi-reparent, duplicate,
/// paste, etc.).
pub struct CompoundCmd {
    pub label: String,
    pub cmds: Vec<Box<dyn UndoCommand>>,
}

impl UndoCommand for CompoundCmd {
    fn label(&self) -> &str { &self.label }
    fn execute(&mut self, world: &mut World) {
        for c in self.cmds.iter_mut() { c.execute(world); }
    }
    fn undo(&mut self, world: &mut World) {
        for c in self.cmds.iter_mut().rev() { c.undo(world); }
    }
}

pub struct GroupAsChildrenCmd {
    pub parent: Entity,
    pub group_name: String,
    /// Members + their parent before grouping.
    pub members: Vec<(Entity, Option<Entity>)>,
}

impl UndoCommand for GroupAsChildrenCmd {
    fn label(&self) -> &str { "Group" }
    fn execute(&mut self, world: &mut World) {
        self.parent = world.spawn((
            Name::new(self.group_name.clone()),
            Transform::default(),
            Visibility::default(),
        )).id();
        for (entity, _) in &self.members {
            if let Ok(mut e) = world.get_entity_mut(*entity) {
                e.set_parent_in_place(self.parent);
            }
        }
        if let Some(sel) = world.get_resource::<EditorSelection>() { sel.set(Some(self.parent)); }
    }
    fn undo(&mut self, world: &mut World) {
        for (entity, old_parent) in &self.members {
            if let Ok(mut e) = world.get_entity_mut(*entity) {
                match old_parent {
                    Some(p) => { e.set_parent_in_place(*p); }
                    None => { e.remove_parent_in_place(); }
                }
            }
        }
        if let Ok(e) = world.get_entity_mut(self.parent) { e.despawn(); }
        if let Some(sel) = world.get_resource::<EditorSelection>() {
            if sel.get() == Some(self.parent) { sel.clear(); }
        }
    }
}

pub enum SpawnEntityKind {
    Preset { id: String },
    Component { type_id: String, display_name: String },
}

pub struct SpawnEntityCmd {
    pub entity: Entity,
    pub kind: SpawnEntityKind,
}

impl UndoCommand for SpawnEntityCmd {
    fn label(&self) -> &str { "Spawn" }
    fn execute(&mut self, world: &mut World) {
        match &self.kind {
            SpawnEntityKind::Preset { id } => {
                let spawn_fn = world.get_resource::<SpawnRegistry>()
                    .and_then(|r| r.iter().find(|p| p.id == id).map(|p| p.spawn_fn));
                if let Some(f) = spawn_fn { self.entity = f(world); }
            }
            SpawnEntityKind::Component { type_id, display_name } => {
                let add_fn = world.get_resource::<InspectorRegistry>()
                    .and_then(|r| r.iter().find(|e| e.type_id == type_id.as_str()).and_then(|e| e.add_fn));
                if let Some(f) = add_fn {
                    let e = world.spawn((Name::new(display_name.clone()), Transform::default())).id();
                    f(world, e);
                    self.entity = e;
                }
            }
        }
        if let Some(sel) = world.get_resource::<EditorSelection>() { sel.set(Some(self.entity)); }
    }
    fn undo(&mut self, world: &mut World) {
        if let Some(sel) = world.get_resource::<EditorSelection>() {
            if sel.get() == Some(self.entity) { sel.clear(); }
        }
        if let Ok(e) = world.get_entity_mut(self.entity) { e.despawn(); }
    }
}
