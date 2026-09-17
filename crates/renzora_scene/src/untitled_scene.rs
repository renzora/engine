//! What is in the Untitled scene before you have put anything in it.
//!
//! Dismissing the splash overlay lands you in the scratch project (see
//! `renzora_splash::untitled`) rather than in a named one, and an empty void is
//! a bad place to start messing about: a mesh dragged into it is unlit and
//! floating, with no way to tell where the ground is or which way is up. So the
//! scratch scene opens with a ground grid and a sun.
//!
//! **No `WorldEnvironment`.** The starter scene is lit and grounded but has no
//! sky and no image-based lighting, so the first environment in the scene is
//! whichever one you add. A default environment would be an opinion about the
//! look of a scene that has not been started yet, and removing one you did not
//! ask for is worse than adding one you did.
//!
//! # Why this is spawned rather than written into the file
//!
//! The obvious alternative is for `create_project` to write a `main.bsn` holding
//! these two entities. The interim BSN format stores `Entity::to_bits()` and
//! fully-qualified type paths, neither of which a hand-written file can be
//! checked against at compile time: a mistyped path does not fail to build, it
//! fails to *load*, and the first thing a new user would see is a broken scene.
//! Spawning uses the real types, so the compiler checks it.
//!
//! # Why it can run more than once
//!
//! The seed is keyed on the scene file still being the empty stub
//! `create_project` wrote, not on a flag. So it runs on every launch until you
//! save, and stops for good the moment you do: a scratch scene you emptied on
//! purpose and saved stays empty, and one you emptied by accident and quit is
//! back the way it was.

use bevy::prelude::*;

use renzora::{CurrentProject, MeshColor, MeshPrimitive, Sun, UntitledProject};

/// The ground plane's half-extent in metres. The `plane` shape is 2x2, so this
/// is a scale rather than a size: 20 gives a 40m square, which is enough to walk
/// a character around without being so large that the grid reads as a texture.
const GROUND_SCALE: f32 = 20.0;

/// A mid grey, the same the `plane` shape is registered with. Picked so the
/// generated blockout grid stays legible on it, which is the whole reason the
/// ground is here.
const GROUND_COLOR: Color = Color::srgb(0.35, 0.35, 0.35);

/// Spawn the starter contents into an untouched scratch scene.
///
/// Exclusive because it has to read the project, decide from a file on disk and
/// spawn: three things that would otherwise be a resource read, an IO call and a
/// command queue spread across as many systems for no benefit.
pub(crate) fn seed_untitled_scene(world: &mut World) {
    if world.get_resource::<UntitledProject>().is_none() {
        return;
    }
    let Some(project) = world.get_resource::<CurrentProject>() else {
        return;
    };
    if !scene_is_untouched(&project.main_scene_path()) {
        return;
    }

    world.spawn((
        Name::new("Ground"),
        Transform::from_scale(Vec3::new(GROUND_SCALE, 1.0, GROUND_SCALE)),
        MeshPrimitive("plane".to_string()),
        MeshColor(GROUND_COLOR),
    ));

    // `Sun` rather than a bare `DirectionalLight`: it is the serializable form
    // the editor's own sun inspector edits, and `scene_io::rehydrate_suns` syncs
    // the light and the transform from its azimuth and elevation. Spawning the
    // light directly would give a sun that the Sun panel does not drive.
    world.spawn((
        Name::new("Sun"),
        Transform::default(),
        DirectionalLight::default(),
        Sun::default(),
    ));

    renzora::core::console_log::console_info(
        "Scene",
        "Untitled scene opened with a ground grid and a sun",
    );
}

/// Is this scene file still exactly what project creation wrote?
///
/// A byte check rather than a parse: the question is "has anyone saved over
/// this", and an empty scene that someone saved deliberately is a different
/// thing from one that has never been written. Treats an unreadable file as
/// touched, because seeding on top of a scene that exists but could not be read
/// would spawn a second ground into whatever loads later.
fn scene_is_untouched(path: &std::path::Path) -> bool {
    match std::fs::read_to_string(path) {
        Ok(contents) => contents.trim() == "// renzora interim bsn v1",
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stub `create_project` writes has to be recognised, or the starter
    /// scene never appears and the untitled editor opens on a void.
    #[test]
    fn the_stub_written_by_project_creation_reads_as_untouched() {
        let dir = std::env::temp_dir().join(format!(
            "renzora-untitled-seed-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let scene = dir.join("main.bsn");

        // Exactly what `renzora_splash::project::create_project` writes.
        std::fs::write(&scene, "// renzora interim bsn v1\n").expect("write");
        assert!(scene_is_untouched(&scene));

        // One entity block later, and it is the user's scene.
        std::fs::write(&scene, "// renzora interim bsn v1\nentity 4294967295 {\n}\n")
            .expect("write");
        assert!(!scene_is_untouched(&scene));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A scene file that is not there at all is not "untouched": seeding into a
    /// project whose scene failed to write would put a ground in a scene that
    /// never loads.
    #[test]
    fn a_missing_scene_file_is_not_seeded() {
        assert!(!scene_is_untouched(std::path::Path::new(
            "renzora-no-such-scene-anywhere.bsn"
        )));
    }
}
