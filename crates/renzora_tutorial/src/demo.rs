//! The glowing demo target the tutorial spawns so the "select a mesh" and "move
//! it with the gizmo" steps always have something to act on — no matter what
//! (if anything) the open scene contains.

use bevy::prelude::*;

/// Where the target spawns: sitting on the grid at the world origin, centered in
/// the default editor camera's view.
pub const DEMO_CUBE_POS: Vec3 = Vec3::new(0.0, 0.5, 0.0);

/// Marks the tutorial's throwaway target so we can despawn it on finish/skip.
#[derive(Component)]
pub struct TutorialDemoCube;

/// Spawn the target cube. It is a normal, named scene mesh (NOT `HideInHierarchy`)
/// so the editor's mesh picker treats it as selectable — that is the whole point
/// of the "click the cube" step.
pub fn spawn_demo_cube(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> Entity {
    let mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let material = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.62, 0.22),
        // Emissive > 1 so it visibly glows and reads as "the thing to look at".
        emissive: LinearRgba::rgb(1.4, 0.7, 0.16),
        perceptual_roughness: 0.45,
        ..default()
    });
    commands
        .spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(DEMO_CUBE_POS),
            Name::new("Tutorial Cube"),
            TutorialDemoCube,
        ))
        .id()
}
