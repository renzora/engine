//! Physics playground state for stress-test spawning

use bevy::prelude::*;

use crate::core::{EditorEntity, SceneNode};
use crate::spawn::EditorSceneRoot;

/// Marker component for entities spawned by the playground
#[derive(Component)]
pub struct PlaygroundEntity;

/// Shape to spawn
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PlaygroundShape {
    #[default]
    Sphere,
    Box,
    Capsule,
    Cylinder,
}

impl PlaygroundShape {
    pub fn label(&self) -> &'static str {
        match self {
            PlaygroundShape::Sphere => "Sphere",
            PlaygroundShape::Box => "Box",
            PlaygroundShape::Capsule => "Capsule",
            PlaygroundShape::Cylinder => "Cylinder",
        }
    }

    pub const ALL: &'static [PlaygroundShape] = &[
        PlaygroundShape::Sphere,
        PlaygroundShape::Box,
        PlaygroundShape::Capsule,
        PlaygroundShape::Cylinder,
    ];
}

/// Spawn pattern
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SpawnPattern {
    #[default]
    Single,
    Stack,
    Wall,
    Rain,
    Pyramid,
    Explosion,
}

impl SpawnPattern {
    pub fn label(&self) -> &'static str {
        match self {
            SpawnPattern::Single => "Single",
            SpawnPattern::Stack => "Stack",
            SpawnPattern::Wall => "Wall",
            SpawnPattern::Rain => "Rain",
            SpawnPattern::Pyramid => "Pyramid",
            SpawnPattern::Explosion => "Explosion",
        }
    }

    pub const ALL: &'static [SpawnPattern] = &[
        SpawnPattern::Single,
        SpawnPattern::Stack,
        SpawnPattern::Wall,
        SpawnPattern::Rain,
        SpawnPattern::Pyramid,
        SpawnPattern::Explosion,
    ];
}

/// Commands from the playground UI
#[derive(Clone, Debug)]
pub enum PlaygroundCommand {
    Spawn,
    ClearAll,
}

/// State resource for the Physics Playground panel
#[derive(Resource)]
pub struct PlaygroundState {
    /// Shape to spawn
    pub shape: PlaygroundShape,
    /// Spawn pattern
    pub pattern: SpawnPattern,
    /// Number of objects to spawn (for multi-patterns)
    pub count: u32,
    /// Mass of spawned objects
    pub mass: f32,
    /// Restitution (bounciness)
    pub restitution: f32,
    /// Friction
    pub friction: f32,
    /// Spawn height offset
    pub spawn_height: f32,
    /// Total playground entities alive
    pub alive_count: usize,
    /// Pending commands
    pub commands: Vec<PlaygroundCommand>,
}

impl Default for PlaygroundState {
    fn default() -> Self {
        Self {
            shape: PlaygroundShape::Sphere,
            pattern: SpawnPattern::Single,
            count: 10,
            mass: 1.0,
            restitution: 0.3,
            friction: 0.5,
            spawn_height: 10.0,
            alive_count: 0,
            commands: Vec::new(),
        }
    }
}

/// Generate spawn positions for the given pattern
fn generate_positions(pattern: SpawnPattern, count: u32, height: f32) -> Vec<Vec3> {
    let count = count.max(1) as usize;
    match pattern {
        SpawnPattern::Single => {
            vec![Vec3::new(0.0, height, 0.0)]
        }
        SpawnPattern::Stack => {
            (0..count)
                .map(|i| Vec3::new(0.0, height + i as f32 * 1.2, 0.0))
                .collect()
        }
        SpawnPattern::Wall => {
            let cols = (count as f32).sqrt().ceil() as usize;
            let rows = (count + cols - 1) / cols;
            let mut positions = Vec::with_capacity(count);
            for row in 0..rows {
                for col in 0..cols {
                    if positions.len() >= count {
                        break;
                    }
                    positions.push(Vec3::new(
                        (col as f32 - cols as f32 / 2.0) * 1.2,
                        height + row as f32 * 1.2,
                        0.0,
                    ));
                }
            }
            positions
        }
        SpawnPattern::Rain => {
            let spread = (count as f32).sqrt() * 2.0;
            (0..count)
                .map(|i| {
                    // Deterministic pseudo-random spread using index
                    let angle = i as f32 * 2.399; // golden angle
                    let radius = (i as f32 / count as f32).sqrt() * spread;
                    Vec3::new(
                        angle.cos() * radius,
                        height + (i as f32 * 0.3),
                        angle.sin() * radius,
                    )
                })
                .collect()
        }
        SpawnPattern::Pyramid => {
            let mut positions = Vec::new();
            let layers = (count as f32).cbrt().ceil() as usize;
            let mut remaining = count;
            for layer in 0..layers {
                let size = layers - layer;
                for x in 0..size {
                    for z in 0..size {
                        if remaining == 0 {
                            break;
                        }
                        remaining -= 1;
                        let offset = -(size as f32 - 1.0) / 2.0;
                        positions.push(Vec3::new(
                            (x as f32 + offset) * 1.2,
                            height + layer as f32 * 1.2,
                            (z as f32 + offset) * 1.2,
                        ));
                    }
                }
            }
            positions
        }
        SpawnPattern::Explosion => {
            (0..count)
                .map(|i| {
                    let phi = (i as f32 / count as f32) * std::f32::consts::PI * 2.0;
                    let theta = ((i as f32 * 2.399) % std::f32::consts::PI);
                    let r = 0.5;
                    Vec3::new(
                        r * theta.sin() * phi.cos(),
                        height + r * theta.cos(),
                        r * theta.sin() * phi.sin(),
                    )
                })
                .collect()
        }
    }
}

/// System that processes playground commands — spawns and despawns entities
pub fn process_playground_commands(
    mut state: ResMut<PlaygroundState>,
    mut commands: Commands,
    playground_entities: Query<Entity, With<PlaygroundEntity>>,
    scene_roots: Query<Entity, With<EditorSceneRoot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Update alive count
    state.alive_count = playground_entities.iter().count();

    let cmds: Vec<PlaygroundCommand> = state.commands.drain(..).collect();
    if cmds.is_empty() {
        return;
    }

    // Find the scene root to parent spawned entities under
    let scene_root = scene_roots.iter().next();

    for cmd in cmds {
        match cmd {
            PlaygroundCommand::Spawn => {
                let positions = generate_positions(state.pattern, state.count, state.spawn_height);

                let mesh_handle = match state.shape {
                    PlaygroundShape::Sphere => meshes.add(Sphere::new(0.5)),
                    PlaygroundShape::Box => meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
                    PlaygroundShape::Capsule => meshes.add(Capsule3d::new(0.3, 1.0)),
                    PlaygroundShape::Cylinder => meshes.add(Cylinder::new(0.4, 1.0)),
                };

                let collider = match state.shape {
                    PlaygroundShape::Sphere => avian3d::prelude::Collider::sphere(0.5),
                    PlaygroundShape::Box => avian3d::prelude::Collider::cuboid(1.0, 1.0, 1.0),
                    PlaygroundShape::Capsule => avian3d::prelude::Collider::capsule(0.3, 1.0),
                    PlaygroundShape::Cylinder => avian3d::prelude::Collider::cylinder(0.4, 1.0),
                };

                // Deterministic color from count
                let color = Color::hsl(
                    (state.alive_count as f32 * 37.0) % 360.0,
                    0.7,
                    0.6,
                );
                let material_handle = materials.add(StandardMaterial {
                    base_color: color,
                    ..default()
                });

                let shape_label = state.shape.label();
                for (i, pos) in positions.iter().enumerate() {
                    let name = format!("Playground {} {}", shape_label, state.alive_count + i);
                    let mut entity_commands = commands.spawn((
                        PlaygroundEntity,
                        EditorEntity {
                            name,
                            tag: "playground".to_string(),
                            visible: true,
                            locked: false,
                        },
                        SceneNode,
                        Mesh3d(mesh_handle.clone()),
                        MeshMaterial3d(material_handle.clone()),
                        Transform::from_translation(*pos),
                        Visibility::default(),
                        avian3d::prelude::RigidBody::Dynamic,
                        collider.clone(),
                        avian3d::prelude::Mass(state.mass),
                        avian3d::prelude::Restitution::new(state.restitution),
                        avian3d::prelude::Friction::new(state.friction),
                    ));

                    // Parent under scene root so they appear in the hierarchy
                    if let Some(root) = scene_root {
                        entity_commands.insert(ChildOf(root));
                    }
                }
            }
            PlaygroundCommand::ClearAll => {
                for entity in playground_entities.iter() {
                    commands.entity(entity).despawn();
                }
            }
        }
    }
}
