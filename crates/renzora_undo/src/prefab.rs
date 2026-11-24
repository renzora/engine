//! The editor's prefab actions, as the hierarchy menu and the instance's
//! inspector card both call them: Create Prefab and Revert go on the undo
//! stack, Apply tells the user what it changed.
//!
//! The scene-side work is `renzora_engine::scene_io::prefab`; this is only the
//! undo and the feedback around it.

use bevy::prelude::*;
use renzora_editor_framework::EditorSelection;
use renzora_engine::scene_io::prefab;

use crate::{active_context, execute, seal, UndoCommand};

fn toast(world: &mut World, ok: bool, message: String) {
    if let Some(mut toasts) = world.get_resource_mut::<renzora_ui::Toasts>() {
        if ok {
            toasts.success(message);
        } else {
            toasts.warning(message);
        }
    }
}

/// Turn `entity` into a prefab in the project's `prefabs/` folder and put an
/// instance of it where it was. Undo puts the entity back; the file stays.
pub fn create_prefab_with_undo(world: &mut World, entity: Entity) {
    let Some(root) = world
        .get_resource::<renzora_project_api::CurrentProject>()
        .map(|p| p.path.clone())
    else {
        return;
    };
    let name = world.get::<Name>(entity).map(|n| n.as_str().to_string()).unwrap_or_default();
    let path = prefab::unique_prefab_path(&root, &name);
    let ctx = active_context(world);
    execute(
        world,
        ctx.clone(),
        Box::new(CreatePrefabCmd {
            entity,
            path,
            instance: None,
            transform: world.get::<Transform>(entity).copied().unwrap_or_default(),
            parent: world.get::<ChildOf>(entity).map(|c| c.parent()),
        }),
    );
    seal(world, &ctx);
}

struct CreatePrefabCmd {
    entity: Entity,
    path: String,
    instance: Option<Entity>,
    transform: Transform,
    parent: Option<Entity>,
}

impl UndoCommand for CreatePrefabCmd {
    fn label(&self) -> &str {
        "Create Prefab"
    }

    fn execute(&mut self, world: &mut World) {
        match prefab::make_prefab(world, self.entity, &self.path) {
            Ok(made) => {
                self.instance = Some(made.instance);
                if let Some(sel) = world.get_resource::<EditorSelection>() {
                    sel.set(Some(made.instance));
                }
                toast(world, true, format!("Created prefab {}", made.path));
            }
            Err(e) => toast(world, false, format!("Could not create a prefab: {e}")),
        }
    }

    fn undo(&mut self, world: &mut World) {
        if let Some(instance) = self.instance.take() {
            prefab::unmake_prefab(world, self.entity, instance, self.transform, self.parent);
            if let Some(sel) = world.get_resource::<EditorSelection>() {
                sel.set(Some(self.entity));
            }
        }
    }
}

/// Write this instance's contents to its prefab and rebuild every other
/// instance from it. Not undoable: it writes a file other scenes may use.
pub fn apply_prefab(world: &mut World, instance: Entity) {
    match prefab::apply_instance(world, instance) {
        Ok(0) => toast(world, true, "Applied to the prefab".into()),
        Ok(n) => toast(world, true, format!("Applied to the prefab and {n} other instance(s)")),
        Err(e) => toast(world, false, format!("Could not apply: {e}")),
    }
}

/// Discard this instance's edits and rebuild it from its prefab. Undo brings
/// the edits back.
pub fn revert_prefab_with_undo(world: &mut World, instance: Entity) {
    let Some(snapshot) = renzora_engine::scene_io::snapshot_entity_subtrees(world, &[instance]) else {
        prefab::reload_instance(world, instance);
        return;
    };
    let ctx = active_context(world);
    execute(
        world,
        ctx.clone(),
        Box::new(RevertPrefabCmd { snapshot, original: instance, live: instance }),
    );
    seal(world, &ctx);
}

/// Revert, undone by restoring the whole instance from a snapshot taken
/// before it. The restored instance is a new entity, which `live` tracks so a
/// redo reverts the right one.
struct RevertPrefabCmd {
    snapshot: String,
    original: Entity,
    live: Entity,
}

impl UndoCommand for RevertPrefabCmd {
    fn label(&self) -> &str {
        "Revert to Prefab"
    }

    fn execute(&mut self, world: &mut World) {
        prefab::reload_instance(world, self.live);
    }

    fn undo(&mut self, world: &mut World) {
        if let Ok(e) = world.get_entity_mut(self.live) {
            e.despawn();
        }
        let map = renzora_engine::scene_io::spawn_entities_from_snapshot(world, &self.snapshot);
        if let Some(&restored) = map.get(&self.original) {
            self.live = restored;
            if let Some(sel) = world.get_resource::<EditorSelection>() {
                sel.set(Some(restored));
            }
        }
    }
}
