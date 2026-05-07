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
//! Currently runtime-only (not editor playmode). The editor's splash
//! pipeline doesn't trigger this; if you need autoloads in the editor
//! while testing, enter playmode after manually loading them or run an
//! exported build.

use std::collections::HashSet;

use bevy::prelude::*;

use crate::scene_io;
use renzora::{CurrentProject, Persistent};

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

    for path in resolved {
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
        for entity in new_entities {
            if let Ok(mut ent) = world.get_entity_mut(entity) {
                ent.insert(Persistent);
            }
        }

        info!(
            "[autoload] {}: {} entities tagged Persistent",
            path.display(),
            count
        );
    }
}
