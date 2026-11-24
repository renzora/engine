//! Spawning a prefab from code.
//!
//! A prefab instance is an entity with a [`SceneInstance`]; the engine fills in
//! its contents from the file the next frame, in a running game as in a scene
//! load. So spawning one is spawning that entity, and that is all this does.
//! It lives in the contract so a Rust script can call it as
//! `renzora::core::prefab::spawn_prefab`.

use bevy::prelude::*;

use crate::SceneInstance;

/// What a prefab instance is made of: a name from the file, a placement, and
/// the reference to the file.
pub fn prefab_bundle(path: impl Into<String>, transform: Transform) -> impl Bundle {
    let source: String = path.into();
    let name = std::path::Path::new(&source)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Prefab".into());
    (
        Name::new(name),
        transform,
        Visibility::default(),
        SceneInstance { source, ..Default::default() },
    )
}

/// Spawn an instance of `path` (project-relative, e.g. `prefabs/coin.bsn`).
///
/// Its contents arrive on the next frame. In the editor they do not arrive at
/// all: there, instances are expanded by whoever adds them (a drop, a menu),
/// and an editor script that wants one should do the same.
pub fn spawn_prefab(world: &mut World, path: impl Into<String>, transform: Transform) -> Entity {
    world.spawn(prefab_bundle(path, transform)).id()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spawned_prefab_is_named_after_its_file() {
        let mut world = World::new();
        let e = spawn_prefab(&mut world, "prefabs/red_barrel.bsn", Transform::from_xyz(1.0, 0.0, 0.0));
        assert_eq!(world.get::<Name>(e).map(|n| n.as_str()), Some("red_barrel"));
        assert_eq!(world.get::<SceneInstance>(e).map(|i| i.source.as_str()), Some("prefabs/red_barrel.bsn"));
        assert_eq!(world.get::<Transform>(e).map(|t| t.translation.x), Some(1.0));
    }
}
