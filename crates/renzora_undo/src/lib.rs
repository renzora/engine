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
use renzora_editor_framework::{EditorLocked, EditorSelection, FieldValue, InspectorRegistry, SpawnRegistry};

// ── Public API ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[derive(Default)]
pub enum UndoContext {
    #[default]
    Scene,
    MaterialGraph(String),
    Blueprint(String),
    Lifecycle,
    Other(String),
}


/// A single undoable action. `execute` is called on initial push AND on redo.
/// `undo` reverses the action. Both take `&mut self` so the command can
/// refresh captured state (e.g. update an entity id after respawn).
pub trait UndoCommand: Any + Send + Sync {
    fn label(&self) -> &str {
        "edit"
    }
    fn execute(&mut self, world: &mut World);
    fn undo(&mut self, world: &mut World);
    fn merge(&mut self, _other: &dyn UndoCommand) -> bool {
        false
    }
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
    pub fn clear_all(&mut self) {
        self.stacks.clear();
    }
    pub fn can_undo(&self, context: &UndoContext) -> bool {
        self.stacks
            .get(context)
            .is_some_and(|s| !s.undo.is_empty())
    }
    pub fn can_redo(&self, context: &UndoContext) -> bool {
        self.stacks
            .get(context)
            .is_some_and(|s| !s.redo.is_empty())
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
    let is_scene = matches!(context, UndoContext::Scene);
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
        while stack.undo.len() > MAX_DEPTH {
            stack.undo.pop_front();
        }
    });
    if is_scene {
        mark_active_scene_tab_modified(world);
    }
}

/// Flip the active document tab's `is_modified` flag so the Save button
/// enables. The save handlers in `renzora_scene` clear it back to false.
fn mark_active_scene_tab_modified(world: &mut World) {
    if let Some(mut tabs) = world.get_resource_mut::<renzora_ui::DocumentTabState>() {
        let active = tabs.active_tab;
        if let Some(tab) = tabs.tabs.get_mut(active) {
            if !tab.is_modified {
                tab.is_modified = true;
            }
        }
    }
}

// ── Messages ───────────────────────────────────────────────────────────────

#[derive(Message)]
pub struct RequestUndo;

#[derive(Message)]
pub struct RequestRedo;

#[derive(Message)]
pub struct UndoExhausted;

// ── Plugin ─────────────────────────────────────────────────────────────────

#[derive(Default)]
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
    let cmd = world
        .resource_mut::<UndoStacks>()
        .stacks
        .get_mut(&active)
        .and_then(|s| s.undo.pop_back());
    let Some(mut cmd) = cmd else {
        world.write_message(UndoExhausted);
        return;
    };
    cmd.undo(world);
    if let Some(s) = world.resource_mut::<UndoStacks>().stacks.get_mut(&active) {
        s.redo.push_back(cmd);
    }
    if matches!(active, UndoContext::Scene) {
        mark_active_scene_tab_modified(world);
    }
}

/// Redo the most recently undone action on the active stack.
pub fn redo_once(world: &mut World) {
    let active = world.resource::<UndoStacks>().active.clone();
    let cmd = world
        .resource_mut::<UndoStacks>()
        .stacks
        .get_mut(&active)
        .and_then(|s| s.redo.pop_back());
    let Some(mut cmd) = cmd else {
        world.write_message(UndoExhausted);
        return;
    };
    cmd.execute(world);
    if let Some(s) = world.resource_mut::<UndoStacks>().stacks.get_mut(&active) {
        s.undo.push_back(cmd);
    }
    if matches!(active, UndoContext::Scene) {
        mark_active_scene_tab_modified(world);
    }
}

fn handle_undo(world: &mut World) {
    let count = world
        .get_resource::<Messages<RequestUndo>>()
        .map(|m| m.iter_current_update_messages().count())
        .unwrap_or(0);
    if count == 0 {
        return;
    }
    undo_once(world);
}

fn handle_redo(world: &mut World) {
    let count = world
        .get_resource::<Messages<RequestRedo>>()
        .map(|m| m.iter_current_update_messages().count())
        .unwrap_or(0);
    if count == 0 {
        return;
    }
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
    fn label(&self) -> &str {
        "Spawn shape"
    }
    fn execute(&mut self, world: &mut World) {
        let Some(create_mesh) = world
            .resource::<ShapeRegistry>()
            .get(&self.shape_id)
            .map(|e| e.create_mesh)
        else {
            return;
        };
        let mesh = create_mesh(&mut world.resource_mut::<Assets<Mesh>>());
        let material = world
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                base_color: self.color,
                perceptual_roughness: 0.9,
                ..default()
            });
        self.entity = world
            .spawn((
                Name::new(self.name.clone()),
                Transform::from_translation(self.position),
                Mesh3d(mesh),
                MeshMaterial3d(material),
                MeshPrimitive(self.shape_id.clone()),
                MeshColor(self.color),
            ))
            .id();
        if let Some(sel) = world.get_resource::<EditorSelection>() {
            sel.set(Some(self.entity));
        }
    }
    fn undo(&mut self, world: &mut World) {
        if let Some(sel) = world.get_resource::<EditorSelection>() {
            if sel.get() == Some(self.entity) {
                sel.clear();
            }
        }
        if let Ok(e) = world.get_entity_mut(self.entity) {
            e.despawn();
        }
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
    fn label(&self) -> &str {
        "Delete"
    }
    fn execute(&mut self, world: &mut World) {
        if let Some(sel) = world.get_resource::<EditorSelection>() {
            let selected = sel.get_all();
            if self.items.iter().any(|i| selected.contains(&i.entity)) {
                sel.clear();
            }
        }
        for item in &self.items {
            if let Ok(e) = world.get_entity_mut(item.entity) {
                e.despawn();
            }
        }
    }
    fn undo(&mut self, world: &mut World) {
        for item in self.items.iter_mut() {
            let Some(create_mesh) = world
                .resource::<ShapeRegistry>()
                .get(&item.shape_id)
                .map(|e| e.create_mesh)
            else {
                continue;
            };
            let mesh = create_mesh(&mut world.resource_mut::<Assets<Mesh>>());
            let material = world
                .resource_mut::<Assets<StandardMaterial>>()
                .add(StandardMaterial {
                    base_color: item.color,
                    perceptual_roughness: 0.9,
                    ..default()
                });
            item.entity = world
                .spawn((
                    Name::new(item.name.clone()),
                    item.transform,
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    MeshPrimitive(item.shape_id.clone()),
                    MeshColor(item.color),
                ))
                .id();
        }
    }
}

pub struct TransformCmd {
    pub entity: Entity,
    pub old: Transform,
    pub new: Transform,
}

impl UndoCommand for TransformCmd {
    fn label(&self) -> &str {
        "Transform"
    }
    fn execute(&mut self, world: &mut World) {
        if let Ok(mut e) = world.get_entity_mut(self.entity) {
            if let Some(mut t) = e.get_mut::<Transform>() {
                *t = self.new;
            }
        }
    }
    fn undo(&mut self, world: &mut World) {
        if let Ok(mut e) = world.get_entity_mut(self.entity) {
            if let Some(mut t) = e.get_mut::<Transform>() {
                *t = self.old;
            }
        }
    }
}

pub struct RenameCmd {
    pub entity: Entity,
    pub old: String,
    pub new: String,
}

impl UndoCommand for RenameCmd {
    fn label(&self) -> &str {
        "Rename"
    }
    fn execute(&mut self, world: &mut World) {
        if let Some(mut n) = world.get_mut::<Name>(self.entity) {
            *n = Name::new(self.new.clone());
        }
    }
    fn undo(&mut self, world: &mut World) {
        if let Some(mut n) = world.get_mut::<Name>(self.entity) {
            *n = Name::new(self.old.clone());
        }
    }
}

pub struct SetHierarchyOrderCmd {
    pub entity: Entity,
    pub old: Option<u32>,
    pub new: Option<u32>,
}

impl UndoCommand for SetHierarchyOrderCmd {
    fn label(&self) -> &str {
        "Reorder"
    }
    fn execute(&mut self, world: &mut World) {
        apply_order(world, self.entity, self.new);
    }
    fn undo(&mut self, world: &mut World) {
        apply_order(world, self.entity, self.old);
    }
}

fn apply_order(world: &mut World, entity: Entity, order: Option<u32>) {
    let Ok(mut e) = world.get_entity_mut(entity) else {
        return;
    };
    match order {
        Some(o) => {
            e.insert(renzora_editor_framework::HierarchyOrder(o));
        }
        None => {
            e.remove::<renzora_editor_framework::HierarchyOrder>();
        }
    }
}

pub struct ReparentCmd {
    pub entity: Entity,
    pub old_parent: Option<Entity>,
    pub new_parent: Option<Entity>,
}

impl UndoCommand for ReparentCmd {
    fn label(&self) -> &str {
        "Reparent"
    }
    fn execute(&mut self, world: &mut World) {
        apply_parent(world, self.entity, self.new_parent);
    }
    fn undo(&mut self, world: &mut World) {
        apply_parent(world, self.entity, self.old_parent);
    }
}

fn apply_parent(world: &mut World, entity: Entity, parent: Option<Entity>) {
    let Ok(mut e) = world.get_entity_mut(entity) else {
        return;
    };
    match parent {
        Some(p) => {
            e.set_parent_in_place(p);
        }
        None => {
            e.remove_parent_in_place();
        }
    }
}

pub struct LockToggleCmd {
    pub entity: Entity,
    pub was_locked: bool,
}

impl UndoCommand for LockToggleCmd {
    fn label(&self) -> &str {
        "Toggle lock"
    }
    fn execute(&mut self, world: &mut World) {
        set_locked(world, self.entity, !self.was_locked);
    }
    fn undo(&mut self, world: &mut World) {
        set_locked(world, self.entity, self.was_locked);
    }
}

fn set_locked(world: &mut World, entity: Entity, locked: bool) {
    let Ok(mut e) = world.get_entity_mut(entity) else {
        return;
    };
    if locked {
        e.insert(EditorLocked);
    } else {
        e.remove::<EditorLocked>();
    }
}

pub struct VisibilityToggleCmd {
    pub entity: Entity,
    pub was_visible: bool,
}

impl UndoCommand for VisibilityToggleCmd {
    fn label(&self) -> &str {
        "Toggle visibility"
    }
    fn execute(&mut self, world: &mut World) {
        set_visibility(world, self.entity, !self.was_visible);
    }
    fn undo(&mut self, world: &mut World) {
        set_visibility(world, self.entity, self.was_visible);
    }
}

fn set_visibility(world: &mut World, entity: Entity, visible: bool) {
    if let Some(mut v) = world.get_mut::<Visibility>(entity) {
        *v = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
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
    fn label(&self) -> &str {
        self.field_name
    }
    fn execute(&mut self, world: &mut World) {
        (self.set_fn)(world, self.entity, self.new.clone());
    }
    fn undo(&mut self, world: &mut World) {
        (self.set_fn)(world, self.entity, self.old.clone());
    }
    fn merge(&mut self, other: &dyn UndoCommand) -> bool {
        let any: &dyn Any = other;
        let Some(o) = any.downcast_ref::<FieldChangeCmd>() else {
            return false;
        };
        if o.entity != self.entity || o.field_name != self.field_name {
            return false;
        }
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
    fn label(&self) -> &str {
        &self.label
    }
    fn execute(&mut self, world: &mut World) {
        for c in self.cmds.iter_mut() {
            c.execute(world);
        }
    }
    fn undo(&mut self, world: &mut World) {
        for c in self.cmds.iter_mut().rev() {
            c.undo(world);
        }
    }
}

pub struct GroupAsChildrenCmd {
    pub parent: Entity,
    pub group_name: String,
    /// Members + their parent before grouping.
    pub members: Vec<(Entity, Option<Entity>)>,
}

impl UndoCommand for GroupAsChildrenCmd {
    fn label(&self) -> &str {
        "Group"
    }
    fn execute(&mut self, world: &mut World) {
        self.parent = world
            .spawn((
                Name::new(self.group_name.clone()),
                Transform::default(),
                Visibility::default(),
            ))
            .id();
        for (entity, _) in &self.members {
            if let Ok(mut e) = world.get_entity_mut(*entity) {
                e.set_parent_in_place(self.parent);
            }
        }
        if let Some(sel) = world.get_resource::<EditorSelection>() {
            sel.set(Some(self.parent));
        }
    }
    fn undo(&mut self, world: &mut World) {
        for (entity, old_parent) in &self.members {
            if let Ok(mut e) = world.get_entity_mut(*entity) {
                match old_parent {
                    Some(p) => {
                        e.set_parent_in_place(*p);
                    }
                    None => {
                        e.remove_parent_in_place();
                    }
                }
            }
        }
        if let Ok(e) = world.get_entity_mut(self.parent) {
            e.despawn();
        }
        if let Some(sel) = world.get_resource::<EditorSelection>() {
            if sel.get() == Some(self.parent) {
                sel.clear();
            }
        }
    }
}

pub enum SpawnEntityKind {
    Preset {
        id: String,
    },
    Component {
        type_id: String,
        display_name: String,
    },
}

pub struct SpawnEntityCmd {
    pub entity: Entity,
    pub kind: SpawnEntityKind,
}

impl UndoCommand for SpawnEntityCmd {
    fn label(&self) -> &str {
        "Spawn"
    }
    fn execute(&mut self, world: &mut World) {
        match &self.kind {
            SpawnEntityKind::Preset { id } => {
                let spawn_fn = world
                    .get_resource::<SpawnRegistry>()
                    .and_then(|r| r.iter().find(|p| p.id == id).map(|p| p.spawn_fn));
                if let Some(f) = spawn_fn {
                    self.entity = f(world);
                }
            }
            SpawnEntityKind::Component {
                type_id,
                display_name,
            } => {
                let add_fn = world.get_resource::<InspectorRegistry>().and_then(|r| {
                    r.iter()
                        .find(|e| e.type_id == type_id.as_str())
                        .and_then(|e| e.add_fn)
                });
                if let Some(f) = add_fn {
                    let e = world
                        .spawn((Name::new(display_name.clone()), Transform::default()))
                        .id();
                    f(world, e);
                    self.entity = e;
                }
            }
        }
        if let Some(sel) = world.get_resource::<EditorSelection>() {
            sel.set(Some(self.entity));
        }
    }
    fn undo(&mut self, world: &mut World) {
        if let Some(sel) = world.get_resource::<EditorSelection>() {
            if sel.get() == Some(self.entity) {
                sel.clear();
            }
        }
        if let Ok(e) = world.get_entity_mut(self.entity) {
            e.despawn();
        }
    }
}

renzora::add!(UndoPlugin, Editor);

// ──────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    /// Records every execute/undo into a shared log resource so tests can
    /// assert *which* command ran and in *what order*, plus a net counter.
    #[derive(Resource, Default)]
    struct Log {
        events: Vec<String>,
        counter: i32,
    }

    /// Minimal command with no GPU/asset dependencies. `execute` adds `delta`
    /// to the counter and logs `"exec:{id}"`; `undo` subtracts and logs
    /// `"undo:{id}"`.
    struct CounterCmd {
        id: String,
        delta: i32,
        merge_with_same_id: bool,
    }

    impl CounterCmd {
        fn new(id: &str, delta: i32) -> Box<dyn UndoCommand> {
            Box::new(CounterCmd {
                id: id.to_string(),
                delta,
                merge_with_same_id: false,
            })
        }
        fn mergeable(id: &str, delta: i32) -> Box<dyn UndoCommand> {
            Box::new(CounterCmd {
                id: id.to_string(),
                delta,
                merge_with_same_id: true,
            })
        }
    }

    impl UndoCommand for CounterCmd {
        fn label(&self) -> &str {
            &self.id
        }
        fn execute(&mut self, world: &mut World) {
            let mut log = world.resource_mut::<Log>();
            log.counter += self.delta;
            log.events.push(format!("exec:{}", self.id));
        }
        fn undo(&mut self, world: &mut World) {
            let mut log = world.resource_mut::<Log>();
            log.counter -= self.delta;
            log.events.push(format!("undo:{}", self.id));
        }
        fn merge(&mut self, other: &dyn UndoCommand) -> bool {
            if !self.merge_with_same_id {
                return false;
            }
            let any: &dyn Any = other;
            let Some(o) = any.downcast_ref::<CounterCmd>() else {
                return false;
            };
            if o.id != self.id {
                return false;
            }
            // Fold the other delta into ourselves.
            self.delta += o.delta;
            true
        }
    }

    /// Bare World with the resources the stack logic needs. Uses a non-Scene
    /// active context so the `DocumentTabState` branch is skipped entirely.
    fn world() -> World {
        let mut w = World::new();
        w.insert_resource(Log::default());
        w.insert_resource(UndoStacks {
            active: UndoContext::Lifecycle,
            ..default()
        });
        w
    }

    fn ctx() -> UndoContext {
        UndoContext::Lifecycle
    }

    fn counter(w: &World) -> i32 {
        w.resource::<Log>().counter
    }

    fn events(w: &World) -> Vec<String> {
        w.resource::<Log>().events.clone()
    }

    #[test]
    fn execute_applies_command_and_records_it() {
        let mut w = world();
        execute(&mut w, ctx(), CounterCmd::new("a", 5));

        assert_eq!(counter(&w), 5, "execute should run the command");
        assert_eq!(events(&w), vec!["exec:a"]);
        let stacks = w.resource::<UndoStacks>();
        assert!(stacks.can_undo(&ctx()));
        assert!(!stacks.can_redo(&ctx()));
        let (undo, redo) = stacks.labels(&ctx());
        assert_eq!(undo, vec!["a"]);
        assert!(redo.is_empty());
    }

    #[test]
    fn record_pushes_without_executing() {
        let mut w = world();
        record(&mut w, ctx(), CounterCmd::new("a", 5));

        // record must NOT call execute.
        assert_eq!(counter(&w), 0);
        assert!(events(&w).is_empty());
        assert!(w.resource::<UndoStacks>().can_undo(&ctx()));
    }

    #[test]
    fn push_three_undo_twice_yields_exact_state() {
        let mut w = world();
        execute(&mut w, ctx(), CounterCmd::new("a", 1));
        execute(&mut w, ctx(), CounterCmd::new("b", 10));
        execute(&mut w, ctx(), CounterCmd::new("c", 100));
        assert_eq!(counter(&w), 111);

        let active = ctx();
        w.resource_mut::<UndoStacks>().active = active.clone();

        undo_once(&mut w); // undo c
        undo_once(&mut w); // undo b

        assert_eq!(counter(&w), 1, "only 'a' should remain applied");
        // Most recent undone first.
        assert_eq!(
            events(&w),
            vec!["exec:a", "exec:b", "exec:c", "undo:c", "undo:b"]
        );

        let stacks = w.resource::<UndoStacks>();
        let (undo, redo) = stacks.labels(&active);
        assert_eq!(undo, vec!["a"], "one entry left on undo stack");
        // redo deque is front=oldest-undone .. back=next-to-redo. `c` was
        // undone first (front), `b` second and is next to be redone (back).
        assert_eq!(redo, vec!["c", "b"]);
    }

    #[test]
    fn redo_reapplies_in_original_order() {
        let mut w = world();
        execute(&mut w, ctx(), CounterCmd::new("a", 1));
        execute(&mut w, ctx(), CounterCmd::new("b", 10));
        execute(&mut w, ctx(), CounterCmd::new("c", 100));

        undo_once(&mut w); // undo c
        undo_once(&mut w); // undo b
        assert_eq!(counter(&w), 1);

        redo_once(&mut w); // redo b (next-to-redo is back of redo deque)
        assert_eq!(counter(&w), 11);
        redo_once(&mut w); // redo c
        assert_eq!(counter(&w), 111);

        assert_eq!(
            events(&w).iter().filter(|e| e.starts_with("exec")).count(),
            5,
            "3 initial execs + 2 redos"
        );
        let stacks = w.resource::<UndoStacks>();
        assert!(stacks.can_undo(&ctx()));
        assert!(!stacks.can_redo(&ctx()), "redo stack drained");
        let (undo, _redo) = stacks.labels(&ctx());
        assert_eq!(undo, vec!["a", "b", "c"]);
    }

    #[test]
    fn new_action_after_undo_clears_redo_stack() {
        let mut w = world();
        execute(&mut w, ctx(), CounterCmd::new("a", 1));
        execute(&mut w, ctx(), CounterCmd::new("b", 10));

        undo_once(&mut w); // undo b -> redo has [b]
        assert!(w.resource::<UndoStacks>().can_redo(&ctx()));

        // A brand-new action must invalidate the redo branch.
        execute(&mut w, ctx(), CounterCmd::new("c", 100));

        let stacks = w.resource::<UndoStacks>();
        assert!(!stacks.can_redo(&ctx()), "redo invalidated by new action");
        let (undo, redo) = stacks.labels(&ctx());
        assert_eq!(undo, vec!["a", "c"]);
        assert!(redo.is_empty());
        assert_eq!(counter(&w), 101, "a(1) + c(100), b was undone");
    }

    #[test]
    fn undo_on_empty_stack_is_noop_and_emits_exhausted() {
        let mut w = world();
        w.init_resource::<Messages<UndoExhausted>>();

        undo_once(&mut w);

        assert_eq!(counter(&w), 0);
        assert!(events(&w).is_empty());
        let msgs = w.resource::<Messages<UndoExhausted>>();
        assert_eq!(
            msgs.iter_current_update_messages().count(),
            1,
            "undo on empty stack writes UndoExhausted"
        );
    }

    #[test]
    fn redo_on_empty_stack_is_noop_and_emits_exhausted() {
        let mut w = world();
        w.init_resource::<Messages<UndoExhausted>>();

        redo_once(&mut w);

        assert_eq!(counter(&w), 0);
        assert!(events(&w).is_empty());
        let msgs = w.resource::<Messages<UndoExhausted>>();
        assert_eq!(msgs.iter_current_update_messages().count(), 1);
    }

    #[test]
    fn clear_drops_both_stacks_for_context() {
        let mut w = world();
        execute(&mut w, ctx(), CounterCmd::new("a", 1));
        undo_once(&mut w); // populate redo
        {
            let s = w.resource::<UndoStacks>();
            assert!(s.can_redo(&ctx()));
        }

        w.resource_mut::<UndoStacks>().clear(&ctx());

        let s = w.resource::<UndoStacks>();
        assert!(!s.can_undo(&ctx()));
        assert!(!s.can_redo(&ctx()));
        let (undo, redo) = s.labels(&ctx());
        assert!(undo.is_empty() && redo.is_empty());
    }

    #[test]
    fn clear_is_scoped_to_one_context() {
        let mut w = world();
        execute(&mut w, UndoContext::Lifecycle, CounterCmd::new("a", 1));
        execute(
            &mut w,
            UndoContext::Other("x".into()),
            CounterCmd::new("b", 2),
        );

        w.resource_mut::<UndoStacks>().clear(&UndoContext::Lifecycle);

        let s = w.resource::<UndoStacks>();
        assert!(!s.can_undo(&UndoContext::Lifecycle));
        assert!(
            s.can_undo(&UndoContext::Other("x".into())),
            "other context untouched"
        );
    }

    #[test]
    fn clear_all_wipes_every_context() {
        let mut w = world();
        execute(&mut w, UndoContext::Lifecycle, CounterCmd::new("a", 1));
        execute(
            &mut w,
            UndoContext::Other("x".into()),
            CounterCmd::new("b", 2),
        );

        w.resource_mut::<UndoStacks>().clear_all();

        let s = w.resource::<UndoStacks>();
        assert!(!s.can_undo(&UndoContext::Lifecycle));
        assert!(!s.can_undo(&UndoContext::Other("x".into())));
    }

    #[test]
    fn capacity_evicts_oldest_entries() {
        let mut w = world();
        // Push one more than the cap.
        for i in 0..(MAX_DEPTH + 1) {
            record(&mut w, ctx(), CounterCmd::new(&format!("c{i}"), 1));
        }

        let s = w.resource::<UndoStacks>();
        let (undo, _redo) = s.labels(&ctx());
        assert_eq!(undo.len(), MAX_DEPTH, "stack capped at MAX_DEPTH");
        // Oldest ("c0") evicted; newest still present at the back.
        assert_eq!(undo.first().map(String::as_str), Some("c1"));
        assert_eq!(
            undo.last().map(String::as_str),
            Some(format!("c{}", MAX_DEPTH).as_str())
        );
    }

    #[test]
    fn merge_folds_two_pushes_into_one_entry() {
        let mut w = world();
        // Two consecutive mergeable pushes with the same id collapse into a
        // single undo entry, with deltas folded together.
        record(&mut w, ctx(), CounterCmd::mergeable("drag", 1));
        record(&mut w, ctx(), CounterCmd::mergeable("drag", 4));

        let (undo, _redo) = w.resource::<UndoStacks>().labels(&ctx());
        assert_eq!(undo, vec!["drag"], "two merges -> one entry");

        // `record` does NOT execute, so the counter is still 0 here. Undoing
        // the single merged entry reverses the *combined* delta (1 + 4 = 5),
        // taking the counter to -5 — proving the second push folded into the
        // first (delta 5) rather than stacking as two separate entries.
        undo_once(&mut w);
        assert_eq!(counter(&w), -5);
    }

    #[test]
    fn non_merging_push_does_not_collapse() {
        let mut w = world();
        // Distinct ids must NOT merge even when mergeable.
        record(&mut w, ctx(), CounterCmd::mergeable("a", 1));
        record(&mut w, ctx(), CounterCmd::mergeable("b", 1));
        let (undo, _redo) = w.resource::<UndoStacks>().labels(&ctx());
        assert_eq!(undo, vec!["a", "b"]);
    }

    #[test]
    fn merge_clears_redo_branch() {
        let mut w = world();
        // Back is mergeable "drag"; populate the redo branch, then a same-id
        // mergeable arrives and must clear redo (merge path, not push path).
        record(&mut w, ctx(), CounterCmd::mergeable("drag", 1)); // undo=[drag]
        record(&mut w, ctx(), CounterCmd::new("z", 0)); // undo=[drag,z]
        undo_once(&mut w); // undo=[drag], redo=[z]
        assert!(w.resource::<UndoStacks>().can_redo(&ctx()));

        record(&mut w, ctx(), CounterCmd::mergeable("drag", 9)); // merges into back

        let s = w.resource::<UndoStacks>();
        assert!(!s.can_redo(&ctx()), "merge must clear the redo branch");
        let (undo, _redo) = s.labels(&ctx());
        assert_eq!(undo, vec!["drag"]);
    }

    #[test]
    fn can_undo_redo_false_for_unknown_context() {
        let w = world();
        let s = w.resource::<UndoStacks>();
        assert!(!s.can_undo(&UndoContext::Other("never".into())));
        assert!(!s.can_redo(&UndoContext::Other("never".into())));
        let (undo, redo) = s.labels(&UndoContext::Other("never".into()));
        assert!(undo.is_empty() && redo.is_empty());
    }
}
