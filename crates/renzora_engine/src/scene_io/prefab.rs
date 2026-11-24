//! Prefabs: turning an entity into a reusable file, and keeping the copies of
//! one in step.
//!
//! A prefab is a scene file whose contents are instanced under an entity with a
//! `SceneInstance`. That part is `instances.rs`; this module is the workflow on
//! top of it:
//!
//! - [`make_prefab`]: write an entity and everything under it to
//!   `prefabs/<name>.bsn`, and put an instance of that file where it was.
//! - [`apply_instance`]: write one instance's contents to its file, then
//!   rebuild every other instance of that file from it.
//! - [`reload_instance`]: throw an instance's contents away and rebuild them
//!   from its file (Revert).
//!
//! # Why the entity goes *inside* the instance
//!
//! An instance root carries the placement (its transform, its parent) and the
//! file carries the contents. So the entity's own components (its mesh, its
//! script) must be in the file, which means the entity becomes the file's root
//! under a new instance root that takes over its transform. That is also the
//! shape a networked avatar has: the networked entity is the instance root, and
//! `renzora::network::networked_root` walks up to it.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use renzora_log_api::*;
use renzora_project_api::CurrentProject;
use renzora_scene_api::SceneInstance;

use super::instances::{expand_scene_instances, save_prefab_source};

/// The folder new prefabs go in, relative to the project root.
pub const PREFAB_DIR: &str = "prefabs";

/// A free `prefabs/<name>.bsn` for `name`, relative to the project root:
/// `name` made file-safe, with `_2`, `_3`... added until nothing is there.
pub fn unique_prefab_path(project_root: &Path, name: &str) -> String {
    let stem: String = name
        .trim()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c.to_ascii_lowercase() } else { '_' })
        .collect();
    let stem = stem.trim_matches('_');
    let stem = if stem.is_empty() { "prefab" } else { stem };
    let mut candidate = format!("{PREFAB_DIR}/{stem}.bsn");
    let mut n = 2;
    while project_root.join(&candidate).exists() {
        candidate = format!("{PREFAB_DIR}/{stem}_{n}.bsn");
        n += 1;
    }
    candidate
}

/// What [`make_prefab`] did, so it can be undone.
#[derive(Clone, Debug)]
pub struct MadePrefab {
    /// The new instance root, now standing where the entity was.
    pub instance: Entity,
    /// The prefab file, relative to the project root.
    pub path: String,
}

/// Turn `entity` into a prefab at `rel_path` (relative to the project root),
/// and replace it in the scene with an instance of it.
///
/// The entity keeps its identity: it moves under the new instance root with an
/// identity transform, and the root takes its transform, parent and hierarchy
/// position, so nothing in the scene moves.
pub fn make_prefab(world: &mut World, entity: Entity, rel_path: &str) -> Result<MadePrefab, String> {
    let project = world
        .get_resource::<CurrentProject>()
        .map(|p| p.path.clone())
        .ok_or("no project is open")?;
    if world.get_entity(entity).is_err() {
        return Err("the entity no longer exists".into());
    }
    if world.get::<SceneInstance>(entity).is_some() {
        return Err("it is already a prefab instance".into());
    }
    let name = world
        .get::<Name>(entity)
        .map(|n| n.as_str().to_string())
        .unwrap_or_else(|| "Prefab".into());
    let transform = world.get::<Transform>(entity).copied().unwrap_or_default();
    let parent = world.get::<ChildOf>(entity).map(|c| c.parent());
    let order = world.get::<renzora_scene_api::HierarchyOrder>(entity).copied();

    let mut root = world.spawn((
        Name::new(name),
        transform,
        Visibility::default(),
        SceneInstance { source: rel_path.to_string(), ..Default::default() },
    ));
    if let Some(order) = order {
        root.insert(order);
    }
    if let Some(p) = parent {
        root.insert(ChildOf(p));
    }
    let instance = root.id();
    world.entity_mut(entity).insert((Transform::IDENTITY, ChildOf(instance)));

    let abs = project.join(rel_path);
    if let Err(e) = save_prefab_source(world, instance, &abs) {
        // Put things back: a half-made prefab is worse than none.
        unmake_prefab(world, entity, instance, transform, parent);
        return Err(format!("could not write {rel_path}: {e}"));
    }
    console_success("Prefab", format!("Created prefab {rel_path}"));
    Ok(MadePrefab { instance, path: rel_path.to_string() })
}

/// Reverse [`make_prefab`] in the scene. The file stays: deleting a file the
/// user may already have used elsewhere is not an undo's job.
pub fn unmake_prefab(world: &mut World, entity: Entity, instance: Entity, transform: Transform, parent: Option<Entity>) {
    if let Ok(mut e) = world.get_entity_mut(entity) {
        e.insert(transform);
        match parent {
            Some(p) => {
                e.insert(ChildOf(p));
            }
            None => {
                e.remove::<ChildOf>();
            }
        }
    }
    if let Ok(e) = world.get_entity_mut(instance) {
        e.despawn();
    }
}

/// Every instance of `source` (project-relative) in the world.
pub fn instances_of(world: &mut World, source: &str) -> Vec<Entity> {
    let mut q = world.query::<(Entity, &SceneInstance)>();
    q.iter(world).filter(|(_, i)| same_source(&i.source, source)).map(|(e, _)| e).collect()
}

/// Compare project-relative paths the way a user means them: `prefabs\a.bsn`
/// and `prefabs/a.bsn` are one file.
fn same_source(a: &str, b: &str) -> bool {
    a.replace('\\', "/").trim_start_matches("./") == b.replace('\\', "/").trim_start_matches("./")
}

/// Rebuild `instance`'s contents from its file, discarding what is there.
pub fn reload_instance(world: &mut World, instance: Entity) {
    if world.get::<SceneInstance>(instance).is_none() {
        return;
    }
    if let Ok(mut e) = world.get_entity_mut(instance) {
        e.despawn_related::<Children>();
    }
    // Expands every instance with nothing under it, which is now this one
    // (and any other that was empty anyway, which is what it would want).
    expand_scene_instances(world);
}

/// Write `instance`'s contents to its prefab file, then rebuild every other
/// instance of that file from it. Returns how many other instances changed.
///
/// This is the explicit form of what saving the scene does for a prefab used
/// once. Saving skips a prefab used more than once, because it cannot tell
/// which copy's edits to keep; Apply says which.
pub fn apply_instance(world: &mut World, instance: Entity) -> Result<usize, String> {
    let source = world
        .get::<SceneInstance>(instance)
        .map(|i| i.source.clone())
        .ok_or("not a prefab instance")?;
    let abs: PathBuf = world
        .get_resource::<CurrentProject>()
        .map(|p| p.path.join(&source))
        .ok_or("no project is open")?;
    save_prefab_source(world, instance, &abs).map_err(|e| e.to_string())?;
    let others: Vec<Entity> = instances_of(world, &source).into_iter().filter(|e| *e != instance).collect();
    for other in &others {
        if let Ok(mut e) = world.get_entity_mut(*other) {
            e.despawn_related::<Children>();
        }
    }
    if !others.is_empty() {
        expand_scene_instances(world);
    }
    console_success(
        "Prefab",
        format!("Applied to {source}, {} other instance(s) updated", others.len()),
    );
    Ok(others.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefab_paths_are_file_safe_and_unique() {
        let dir = std::env::temp_dir().join(format!("renzora_prefab_test_{}", std::process::id()));
        std::fs::create_dir_all(dir.join(PREFAB_DIR)).unwrap();
        assert_eq!(unique_prefab_path(&dir, "Red Barrel!"), "prefabs/red_barrel.bsn");
        std::fs::write(dir.join("prefabs/red_barrel.bsn"), "").unwrap();
        assert_eq!(unique_prefab_path(&dir, "Red Barrel!"), "prefabs/red_barrel_2.bsn");
        assert_eq!(unique_prefab_path(&dir, "  ???  "), "prefabs/prefab.bsn");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A world that can save and load scene files from a temporary project.
    fn project_world(tag: &str) -> (World, PathBuf) {
        let dir = std::env::temp_dir().join(format!("renzora_prefab_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut world = World::new();
        let registry = AppTypeRegistry::default();
        {
            let mut r = registry.write();
            r.register::<Name>();
            r.register::<Transform>();
            r.register::<ChildOf>();
            r.register::<Children>();
            r.register::<Visibility>();
            r.register::<SceneInstance>();
        }
        world.insert_resource(registry);
        world.insert_resource(CurrentProject { path: dir.clone(), config: Default::default() });
        (world, dir)
    }

    fn child_names(world: &World, e: Entity) -> Vec<String> {
        world
            .get::<Children>(e)
            .map(|c| c.iter().filter_map(|k| world.get::<Name>(k).map(|n| n.to_string())).collect())
            .unwrap_or_default()
    }

    #[test]
    fn make_apply_and_revert_round_trip_through_the_file() {
        let (mut world, dir) = project_world("roundtrip");
        let barrel = world.spawn((Name::new("barrel"), Transform::from_xyz(5.0, 0.0, 0.0))).id();
        world.spawn((Name::new("lid"), Transform::default(), ChildOf(barrel)));

        // Make: the barrel moves inside a new instance that took its place.
        let made = make_prefab(&mut world, barrel, "prefabs/barrel.bsn").unwrap();
        assert!(dir.join("prefabs/barrel.bsn").is_file());
        assert_eq!(world.get::<Transform>(made.instance).unwrap().translation.x, 5.0);
        assert_eq!(world.get::<Transform>(barrel).unwrap().translation, Vec3::ZERO);
        assert_eq!(world.get::<ChildOf>(barrel).map(|c| c.parent()), Some(made.instance));

        // A second copy, filled from the file, together with a third: two
        // instances of one file filled in one pass, which used to trip the
        // recursion guard on the second.
        let third = world
            .spawn((
                Name::new("barrel third"),
                Transform::default(),
                SceneInstance { source: "prefabs/barrel.bsn".into(), ..Default::default() },
            ))
            .id();
        let copy = world
            .spawn((
                Name::new("barrel copy"),
                Transform::default(),
                SceneInstance { source: "prefabs/barrel.bsn".into(), ..Default::default() },
            ))
            .id();
        expand_scene_instances(&mut world);
        assert_eq!(child_names(&world, copy), vec!["barrel"]);
        assert_eq!(child_names(&world, third), vec!["barrel"]);

        // Edit the first copy and apply: the second copy follows. The editor
        // has suffixed the name for uniqueness (`_1`) and kept the real one in
        // `BindName`; the file must get the real one.
        world
            .entity_mut(barrel)
            .insert((Name::new("barrel_dented_1"), renzora_scene_api::BindName("barrel_dented".into())));
        assert_eq!(apply_instance(&mut world, made.instance).unwrap(), 2);
        assert_eq!(child_names(&world, third), vec!["barrel_dented"]);
        assert_eq!(child_names(&world, copy), vec!["barrel_dented"]);

        // Edit the second copy and revert: the file wins.
        let inner = world.get::<Children>(copy).unwrap()[0];
        world.entity_mut(inner).insert(Name::new("scribbled"));
        reload_instance(&mut world, copy);
        assert_eq!(child_names(&world, copy), vec!["barrel_dented"]);

        // Unmake puts the original back where it was.
        unmake_prefab(&mut world, barrel, made.instance, Transform::from_xyz(5.0, 0.0, 0.0), None);
        assert!(world.get_entity(made.instance).is_err());
        assert_eq!(world.get::<Transform>(barrel).unwrap().translation.x, 5.0);
        assert!(world.get::<ChildOf>(barrel).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sources_compare_by_meaning() {
        assert!(same_source("prefabs\\coin.bsn", "./prefabs/coin.bsn"));
        assert!(!same_source("prefabs/coin.bsn", "prefabs/coin2.bsn"));
    }
}
