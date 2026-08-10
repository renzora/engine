//! Autoload scenes — scenes listed in `project.toml` that are loaded
//! before `main_scene` and persist across every subsequent
//! `load_scene()` call.
//!
//! Used for engine-wide UI overlays (loading bar), audio managers,
//! save-state holders, settings — anything that must outlive the active
//! scene. Equivalent to Godot's autoloads or Unity's
//! `DontDestroyOnLoad`-on-spawn.
//!
//! # Mechanism
//!
//! Each entry in `project.config.autoload` is loaded via
//! [`scene_io::load_scene`]. The diff between the entity set before and
//! after the load is the set of entities the autoload spawned; every one
//! of them gets a [`Persistent`] component inserted. From then on,
//! `process_pending_scene_loads`'s `Without<Persistent>` filter skips
//! them automatically.
//!
//! # Editor play sessions
//!
//! A game build calls [`load_autoloads`] once on `Startup`. The editor has no
//! equivalent moment — it boots into an editing session, not a game — so
//! pressing Play fires `renzora::LoadAutoloadScenes` and Stop fires
//! `UnloadAutoloadScenes`, handled by the observers at the bottom of this
//! module. That is what makes a global HUD / music / networking scene testable
//! without an export.
//!
//! Teardown despawns the entities [`AutoloadedEntities`] recorded, **not**
//! every `Persistent` entity in the world: the marker is reflected and a user
//! can apply it by hand from the inspector, so a blanket sweep would delete
//! authored scene content.

use std::collections::HashSet;

use bevy::prelude::*;

use crate::scene_io;
use renzora::{CurrentProject, Persistent};

/// What the autoload pass spawned, so an editor Stop can despawn exactly what
/// Play added. Empty in a shipped game, where autoloads live for the whole
/// process.
#[derive(Resource, Default)]
pub struct AutoloadedEntities {
    pub entities: Vec<Entity>,
    /// Which autoload scenes are currently resident, so a second pass doesn't
    /// spawn a duplicate set. Tracked by path because that is the only identity
    /// a scene has once loaded — the entities themselves carry no record of the
    /// file they came from.
    pub paths: Vec<std::path::PathBuf>,
}

impl AutoloadedEntities {
    pub fn is_resident(&self, path: &std::path::Path) -> bool {
        self.paths.iter().any(|p| scene_io::paths_equal(p, path))
    }
}

/// Load every autoload scene listed in `project.config.autoload`, and
/// tag each spawned entity with [`Persistent`] so subsequent scene
/// changes don't despawn them.
///
/// Runs on `Startup`, before [`scene_io::load_current_scene`].
pub fn load_autoloads(world: &mut World) {
    let Some(project) = world.get_resource::<CurrentProject>() else {
        return;
    };
    if project.config.autoload.is_empty() {
        return;
    }

    // Resolve every relative path up front before any borrowing tangles.
    let resolved: Vec<std::path::PathBuf> = project
        .config
        .autoload
        .iter()
        .map(|s| project.resolve_path(s))
        .collect();

    // The scene already sitting in the world. In the editor that's the scene
    // you have open, and it is entirely reasonable for it to also be listed as
    // global — you author a HUD scene, then tick it on. Loading it again would
    // spawn a second copy of every entity, which the id allocator resolves by
    // suffixing (`camera` → `camera_1`), so the symptom is a scene that appears
    // to duplicate itself the moment you press Play.
    let open_scene: Option<std::path::PathBuf> = world
        .get_resource::<scene_io::SceneLoadState>()
        .and_then(|s| s.current_path.clone())
        .map(std::path::PathBuf::from);

    for path in resolved {
        if let Some(open) = &open_scene {
            if scene_io::paths_equal(open, &path) {
                info!(
                    "[autoload] skipping {} — already open as the current scene",
                    path.display()
                );
                continue;
            }
        }
        // Idempotence per scene, not just per pass: re-entering Play, or a
        // second autoload entry naming the same file, must not double-spawn.
        if world
            .get_resource::<AutoloadedEntities>()
            .is_some_and(|a| a.is_resident(&path))
        {
            info!("[autoload] skipping {} — already loaded", path.display());
            continue;
        }

        info!("[autoload] loading {}", path.display());

        // Snapshot the entity set before the load so we can identify what
        // the load spawned. Anything in `before` already existed and
        // belongs to the editor / earlier autoloads / plugin bootstrap;
        // we don't want to retag those.
        let before: HashSet<Entity> = {
            let mut q = world.query::<Entity>();
            q.iter(world).collect()
        };

        scene_io::load_scene(world, &path);

        // Diff against `before` to find this autoload's entities, then
        // tag each one. Children that get spawned later (e.g. by
        // rehydrate systems on subsequent frames) won't be in this
        // snapshot — but the typical autoload payload is UI canvases and
        // singleton script entities, which are all spawned synchronously
        // by `load_scene`, so this captures everything that matters.
        let mut new_entities: Vec<Entity> = Vec::new();
        {
            let mut q = world.query::<Entity>();
            for e in q.iter(world) {
                if !before.contains(&e) {
                    new_entities.push(e);
                }
            }
        }

        let count = new_entities.len();
        for entity in &new_entities {
            if let Ok(mut ent) = world.get_entity_mut(*entity) {
                ent.insert(Persistent);
            }
        }
        {
            let mut tracked = world.get_resource_or_insert_with(AutoloadedEntities::default);
            tracked.entities.extend(new_entities);
            tracked.paths.push(path.clone());
        }

        info!(
            "[autoload] {}: {} entities tagged Persistent",
            path.display(),
            count
        );
    }
}

/// Carry `Persistent` onto children that appear *after* the autoload pass.
///
/// [`load_autoloads`] tags what `load_scene` spawned synchronously, but a
/// rehydrate system can attach a subtree frames later — a glTF root gaining its
/// mesh children is the common case. Those arrivals missed the diff, so without
/// this the next scene load despawns them out from under a persistent parent,
/// leaving a root with nothing under it.
///
/// Keyed on `Added<ChildOf>` so this costs a lookup per newly-parented entity,
/// not a walk of the hierarchy every frame.
pub fn propagate_persistent_to_children(
    mut commands: Commands,
    added: Query<(Entity, &ChildOf), Added<ChildOf>>,
    persistent: Query<(), With<Persistent>>,
) {
    for (entity, parent) in added.iter() {
        // Re-inserting on an entity that already has the marker would be
        // harmless but pointless, and `Added<ChildOf>` fires once per
        // newly-parented entity, so the skip stays cheap.
        if persistent.get(entity).is_err() && persistent.get(parent.parent()).is_ok() {
            commands.entity(entity).insert(Persistent);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn residency_is_path_keyed_and_clearable() {
        let mut tracked = AutoloadedEntities::default();
        let p = PathBuf::from("/proj/scenes/ui.ron");
        assert!(!tracked.is_resident(&p));
        tracked.paths.push(p.clone());
        assert!(tracked.is_resident(&p));
        assert!(!tracked.is_resident(&PathBuf::from("/proj/scenes/other.ron")));
        // Stop clears residency, or the next Play would skip every global scene
        // and they would never come back for the rest of the session.
        tracked.paths.clear();
        assert!(!tracked.is_resident(&p));
    }
}

/// Editor Play: load the global scenes a shipped game gets at `Startup`.
///
/// Idempotent — re-entering Play without a Stop would otherwise spawn a second
/// copy of every global scene.
pub fn on_load_autoload_scenes(
    _trigger: On<renzora::LoadAutoloadScenes>,
    mut commands: Commands,
) {
    commands.queue(|world: &mut World| {
        // `load_autoloads` is idempotent per scene path, so re-entering Play
        // without a Stop adds nothing rather than duplicating.
        load_autoloads(world);
    });
}

/// Editor Stop: despawn what Play loaded.
pub fn on_unload_autoload_scenes(
    _trigger: On<renzora::UnloadAutoloadScenes>,
    mut commands: Commands,
) {
    commands.queue(|world: &mut World| {
        let Some(mut tracked) = world.get_resource_mut::<AutoloadedEntities>() else {
            return;
        };
        // Clear the residency record with the entities. Leaving paths behind
        // would make the next Play believe the scenes were still loaded and
        // skip them, so Stop would permanently disable global scenes.
        tracked.paths.clear();
        let entities = std::mem::take(&mut tracked.entities);
        let mut despawned = 0usize;
        for entity in entities {
            // Children of an already-despawned root are gone with it, and a
            // script may have despawned something itself, so a missing id is
            // routine rather than an error.
            if world.get_entity(entity).is_ok() {
                world.despawn(entity);
                despawned += 1;
            }
        }
        if despawned > 0 {
            info!("[autoload] unloaded {despawned} global-scene entities");
        }
    });
}
