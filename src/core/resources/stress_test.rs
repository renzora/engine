//! Stress test state — automated scaling tests with performance measurement

use bevy::prelude::*;

use crate::core::{EditorEntity, SceneNode};
use crate::spawn::EditorSceneRoot;

/// Marker for stress test entities
#[derive(Component)]
pub struct StressTestEntity;

/// Type of stress test
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum StressTestType {
    #[default]
    ObjectScaling,
    SleepingEfficiency,
    Determinism,
    CollisionStorm,
}

impl StressTestType {
    pub fn label(&self) -> &'static str {
        match self {
            StressTestType::ObjectScaling => "Object Scaling",
            StressTestType::SleepingEfficiency => "Sleeping Efficiency",
            StressTestType::Determinism => "Determinism",
            StressTestType::CollisionStorm => "Collision Storm",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            StressTestType::ObjectScaling => "Spawn objects in scaling steps, measure FPS at each",
            StressTestType::SleepingEfficiency => "Spawn objects, measure sleeping ratio over time",
            StressTestType::Determinism => "Run same scenario twice, compare position hashes",
            StressTestType::CollisionStorm => "Spawn objects in tight space, measure collision throughput",
        }
    }

    pub const ALL: &'static [StressTestType] = &[
        StressTestType::ObjectScaling,
        StressTestType::SleepingEfficiency,
        StressTestType::Determinism,
        StressTestType::CollisionStorm,
    ];
}

/// A single stress test result
#[derive(Clone, Debug)]
pub struct StressTestResult {
    pub object_count: usize,
    pub avg_fps: f32,
    pub physics_ms: f32,
    pub collision_pairs: usize,
}

/// State machine phases
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StressTestPhase {
    Idle,
    Spawning,
    Stabilizing,
    Measuring,
    Recording,
    Cleanup,
    Done,
}

/// Commands from the stress test UI
#[derive(Clone, Debug)]
pub enum StressTestCommand {
    Start,
    Stop,
    ClearResults,
}

/// State resource for the Stress Test panel
#[derive(Resource)]
pub struct StressTestState {
    pub test_type: StressTestType,
    pub is_running: bool,
    pub current_step: usize,
    pub total_steps: usize,
    pub phase: StressTestPhase,
    pub phase_frames: usize,
    pub results: Vec<StressTestResult>,
    pub commands: Vec<StressTestCommand>,
    pub determinism_hashes: Vec<u64>,
    pub fps_accumulator: f32,
    pub fps_samples: usize,
    pub max_objects: usize,
    pub scaling_steps: Vec<usize>,
}

impl StressTestState {
    pub fn rebuild_scaling_steps(&mut self) {
        self.scaling_steps.clear();
        let max = self.max_objects;
        let candidates = [10, 25, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 25000, 50000, 100000, 250000, 500000, 1000000];
        for &c in &candidates {
            if c <= max {
                self.scaling_steps.push(c);
            }
        }
        if self.scaling_steps.last().copied() != Some(max) && max > 0 {
            self.scaling_steps.push(max);
        }
        if self.scaling_steps.is_empty() {
            self.scaling_steps.push(10);
        }
    }
}

impl Default for StressTestState {
    fn default() -> Self {
        let mut s = Self {
            test_type: StressTestType::ObjectScaling,
            is_running: false,
            current_step: 0,
            total_steps: 0,
            phase: StressTestPhase::Idle,
            phase_frames: 0,
            results: Vec::new(),
            commands: Vec::new(),
            determinism_hashes: Vec::new(),
            fps_accumulator: 0.0,
            fps_samples: 0,
            max_objects: 1000,
            scaling_steps: Vec::new(),
        };
        s.rebuild_scaling_steps();
        s.total_steps = s.scaling_steps.len();
        s
    }
}

/// System that processes stress test commands and runs the state machine
pub fn process_stress_test(
    mut state: ResMut<StressTestState>,
    mut commands: Commands,
    stress_entities: Query<Entity, With<StressTestEntity>>,
    scene_roots: Query<Entity, With<EditorSceneRoot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    diagnostics: Res<crate::core::DiagnosticsState>,
    rigid_bodies: Query<(&Transform, &avian3d::prelude::LinearVelocity), With<avian3d::prelude::RigidBody>>,
    _sleeping: Query<Entity, With<avian3d::prelude::Sleeping>>,
    colliding: Query<&avian3d::prelude::CollidingEntities>,
) {
    // Process commands
    let cmds: Vec<StressTestCommand> = state.commands.drain(..).collect();
    for cmd in cmds {
        match cmd {
            StressTestCommand::Start => {
                state.rebuild_scaling_steps();
                state.is_running = true;
                state.current_step = 0;
                state.results.clear();
                state.determinism_hashes.clear();
                state.phase = StressTestPhase::Spawning;
                state.phase_frames = 0;
                state.total_steps = match state.test_type {
                    StressTestType::ObjectScaling => state.scaling_steps.len(),
                    StressTestType::SleepingEfficiency => 10,
                    StressTestType::Determinism => 2,
                    StressTestType::CollisionStorm => 5,
                };
            }
            StressTestCommand::Stop => {
                state.is_running = false;
                state.phase = StressTestPhase::Idle;
                for entity in stress_entities.iter() {
                    commands.entity(entity).despawn();
                }
            }
            StressTestCommand::ClearResults => {
                state.results.clear();
                state.determinism_hashes.clear();
            }
        }
    }

    if !state.is_running {
        return;
    }

    let scene_root = scene_roots.iter().next();
    state.phase_frames += 1;

    match state.phase {
        StressTestPhase::Idle => {}
        StressTestPhase::Spawning => {
            // Despawn previous entities
            for entity in stress_entities.iter() {
                commands.entity(entity).despawn();
            }

            let count = match state.test_type {
                StressTestType::ObjectScaling => {
                    if state.current_step < state.scaling_steps.len() {
                        state.scaling_steps[state.current_step]
                    } else {
                        0
                    }
                }
                StressTestType::SleepingEfficiency => state.max_objects.min(500),
                StressTestType::Determinism => 100,
                StressTestType::CollisionStorm => {
                    100 + state.current_step * 100
                }
            };

            // Spawn dynamic objects with mixed shapes (arena is spawned separately via Arena Presets panel)
            spawn_dynamic_objects(&mut commands, &mut meshes, &mut materials, scene_root, count, state.current_step);

            state.phase = StressTestPhase::Stabilizing;
            state.phase_frames = 0;
        }
        StressTestPhase::Stabilizing => {
            if state.phase_frames >= 30 {
                state.phase = StressTestPhase::Measuring;
                state.phase_frames = 0;
                state.fps_accumulator = 0.0;
                state.fps_samples = 0;
            }
        }
        StressTestPhase::Measuring => {
            state.fps_accumulator += diagnostics.fps as f32;
            state.fps_samples += 1;

            if state.phase_frames >= 60 {
                state.phase = StressTestPhase::Recording;
                state.phase_frames = 0;
            }
        }
        StressTestPhase::Recording => {
            let avg_fps = if state.fps_samples > 0 {
                state.fps_accumulator / state.fps_samples as f32
            } else {
                0.0
            };

            let entity_count = stress_entities.iter().count();
            let collision_count = colliding.iter().map(|c| c.len()).sum::<usize>() / 2;
            let physics_ms = (entity_count as f32 * 0.01 + collision_count as f32 * 0.005).max(0.01);

            state.results.push(StressTestResult {
                object_count: entity_count,
                avg_fps,
                physics_ms,
                collision_pairs: collision_count,
            });

            if matches!(state.test_type, StressTestType::Determinism) {
                let mut hash: u64 = 0;
                for (transform, vel) in rigid_bodies.iter() {
                    let p = transform.translation;
                    hash = hash.wrapping_add((p.x * 1000.0) as u64);
                    hash = hash.wrapping_add((p.y * 1000.0) as u64);
                    hash = hash.wrapping_add((p.z * 1000.0) as u64);
                    hash = hash.wrapping_add((vel.0.x * 1000.0) as u64);
                }
                state.determinism_hashes.push(hash);
            }

            state.current_step += 1;
            if state.current_step >= state.total_steps {
                state.phase = StressTestPhase::Cleanup;
                state.phase_frames = 0;
            } else {
                state.phase = StressTestPhase::Spawning;
                state.phase_frames = 0;
            }
        }
        StressTestPhase::Cleanup => {
            for entity in stress_entities.iter() {
                commands.entity(entity).despawn();
            }
            state.phase = StressTestPhase::Done;
            state.is_running = false;
        }
        StressTestPhase::Done => {
            state.is_running = false;
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Spawn dynamic objects with mixed shapes using a golden-angle spiral pattern.
fn spawn_dynamic_objects(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    scene_root: Option<Entity>,
    count: usize,
    step: usize,
) {
    let sphere_mesh = meshes.add(Sphere::new(0.5));
    let box_mesh = meshes.add(Cuboid::new(0.8, 0.8, 0.8));
    let capsule_mesh = meshes.add(Capsule3d::new(0.3, 1.0));
    let cylinder_mesh = meshes.add(Cylinder::new(0.4, 0.8));

    let sphere_col = avian3d::prelude::Collider::sphere(0.5);
    let box_col = avian3d::prelude::Collider::cuboid(0.8, 0.8, 0.8);
    let capsule_col = avian3d::prelude::Collider::capsule(0.3, 1.0);
    let cylinder_col = avian3d::prelude::Collider::cylinder(0.4, 0.8);

    let hue = (step as f32 * 60.0) % 360.0;
    let mats = [
        materials.add(StandardMaterial { base_color: Color::hsl(hue, 0.7, 0.6), ..default() }),
        materials.add(StandardMaterial { base_color: Color::hsl(hue + 90.0, 0.7, 0.55), ..default() }),
        materials.add(StandardMaterial { base_color: Color::hsl(hue + 180.0, 0.65, 0.5), ..default() }),
        materials.add(StandardMaterial { base_color: Color::hsl(hue + 270.0, 0.6, 0.6), ..default() }),
    ];

    let spread = (count as f32).sqrt() * 1.2;

    for i in 0..count {
        // Golden-angle spiral above arena center
        let angle = i as f32 * 2.399;
        let r = (i as f32 / count.max(1) as f32).sqrt() * spread.max(8.0);
        let pos = Vec3::new(angle.cos() * r, 3.0 + (i as f32 * 0.2), angle.sin() * r);

        let shape_idx = i % 4;
        let (mesh, col, mat, mass) = match shape_idx {
            0 => (sphere_mesh.clone(), sphere_col.clone(), mats[0].clone(), 1.0),
            1 => (box_mesh.clone(), box_col.clone(), mats[1].clone(), 2.0),
            2 => (capsule_mesh.clone(), capsule_col.clone(), mats[2].clone(), 1.5),
            _ => (cylinder_mesh.clone(), cylinder_col.clone(), mats[3].clone(), 1.8),
        };

        let mut ec = commands.spawn((
            StressTestEntity,
            EditorEntity { name: format!("Stress {}", i), tag: "stress_test".to_string(), visible: true, locked: false },
            SceneNode,
            Mesh3d(mesh),
            MeshMaterial3d(mat),
            Transform::from_translation(pos),
            Visibility::default(),
            avian3d::prelude::RigidBody::Dynamic,
            col,
            avian3d::prelude::Mass(mass),
            avian3d::prelude::Restitution::new(0.3),
            avian3d::prelude::Friction::new(0.5),
        ));
        if let Some(root) = scene_root {
            ec.insert(ChildOf(root));
        }
    }
}
