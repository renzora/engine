//! Physics scenario presets — one-click spawning of classic physics test scenes

use bevy::prelude::*;

use crate::core::{EditorEntity, SceneNode};
use crate::spawn::EditorSceneRoot;

/// Marker component for entities spawned by a scenario
#[derive(Component)]
pub struct ScenarioEntity;

/// Available scenario types
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ScenarioType {
    #[default]
    NewtonsCradle,
    DominoChain,
    WreckingBall,
    StackTest,
    BilliardBreak,
    InclinedPlane,
    Pendulum,
    ProjectileLaunch,
    Gauntlet,
    Avalanche,
}

impl ScenarioType {
    pub fn label(&self) -> &'static str {
        match self {
            ScenarioType::NewtonsCradle => "Newton's Cradle",
            ScenarioType::DominoChain => "Domino Chain",
            ScenarioType::WreckingBall => "Wrecking Ball",
            ScenarioType::StackTest => "Stack Test",
            ScenarioType::BilliardBreak => "Billiard Break",
            ScenarioType::InclinedPlane => "Inclined Plane",
            ScenarioType::Pendulum => "Pendulum",
            ScenarioType::ProjectileLaunch => "Projectile",
            ScenarioType::Gauntlet => "Gauntlet",
            ScenarioType::Avalanche => "Avalanche",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ScenarioType::NewtonsCradle => "5 spheres on joints — demonstrates conservation of momentum",
            ScenarioType::DominoChain => "20 dominoes in a curved line — chain reaction test",
            ScenarioType::WreckingBall => "Heavy sphere on a joint + wall of boxes",
            ScenarioType::StackTest => "Tower of boxes — stability and solver accuracy test",
            ScenarioType::BilliardBreak => "Triangle of spheres + cue ball with initial velocity",
            ScenarioType::InclinedPlane => "Static ramp with objects sliding down",
            ScenarioType::Pendulum => "Sphere on a distance joint — simple harmonic motion",
            ScenarioType::ProjectileLaunch => "Projectile launched at angle — ballistic trajectory",
            ScenarioType::Gauntlet => "Obstacle course: ramps, pendulums, rotating platforms",
            ScenarioType::Avalanche => "Steep slope with pile of mixed shapes",
        }
    }

    pub const ALL: &'static [ScenarioType] = &[
        ScenarioType::NewtonsCradle,
        ScenarioType::DominoChain,
        ScenarioType::WreckingBall,
        ScenarioType::StackTest,
        ScenarioType::BilliardBreak,
        ScenarioType::InclinedPlane,
        ScenarioType::Pendulum,
        ScenarioType::ProjectileLaunch,
        ScenarioType::Gauntlet,
        ScenarioType::Avalanche,
    ];
}

/// Commands from the scenarios UI
#[derive(Clone, Debug)]
pub enum ScenarioCommand {
    Spawn,
    ClearScenario,
}

/// State resource for the Scenario Presets panel
#[derive(Resource)]
pub struct PhysicsScenariosState {
    /// Currently selected scenario
    pub selected_scenario: ScenarioType,
    /// Scale multiplier for spawned scenarios
    pub scale: f32,
    /// Pending commands
    pub commands: Vec<ScenarioCommand>,
    /// Number of scenario entities alive
    pub alive_count: usize,
}

impl Default for PhysicsScenariosState {
    fn default() -> Self {
        Self {
            selected_scenario: ScenarioType::NewtonsCradle,
            scale: 1.0,
            commands: Vec::new(),
            alive_count: 0,
        }
    }
}

/// System that processes scenario commands
pub fn process_scenario_commands(
    mut state: ResMut<PhysicsScenariosState>,
    mut commands: Commands,
    scenario_entities: Query<Entity, With<ScenarioEntity>>,
    scene_roots: Query<Entity, With<EditorSceneRoot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    state.alive_count = scenario_entities.iter().count();

    let cmds: Vec<ScenarioCommand> = state.commands.drain(..).collect();
    if cmds.is_empty() {
        return;
    }

    let scene_root = scene_roots.iter().next();
    let scale = state.scale;

    for cmd in cmds {
        match cmd {
            ScenarioCommand::Spawn => {
                match state.selected_scenario {
                    ScenarioType::NewtonsCradle => spawn_newtons_cradle(&mut commands, scene_root, &mut meshes, &mut materials, scale),
                    ScenarioType::DominoChain => spawn_domino_chain(&mut commands, scene_root, &mut meshes, &mut materials, scale),
                    ScenarioType::WreckingBall => spawn_wrecking_ball(&mut commands, scene_root, &mut meshes, &mut materials, scale),
                    ScenarioType::StackTest => spawn_stack_test(&mut commands, scene_root, &mut meshes, &mut materials, scale),
                    ScenarioType::BilliardBreak => spawn_billiard_break(&mut commands, scene_root, &mut meshes, &mut materials, scale),
                    ScenarioType::InclinedPlane => spawn_inclined_plane(&mut commands, scene_root, &mut meshes, &mut materials, scale),
                    ScenarioType::Pendulum => spawn_pendulum(&mut commands, scene_root, &mut meshes, &mut materials, scale),
                    ScenarioType::ProjectileLaunch => spawn_projectile_launch(&mut commands, scene_root, &mut meshes, &mut materials, scale),
                    ScenarioType::Gauntlet => spawn_gauntlet(&mut commands, scene_root, &mut meshes, &mut materials, scale),
                    ScenarioType::Avalanche => spawn_avalanche(&mut commands, scene_root, &mut meshes, &mut materials, scale),
                }
            }
            ScenarioCommand::ClearScenario => {
                for entity in scenario_entities.iter() {
                    commands.entity(entity).despawn();
                }
            }
        }
    }
}

// ============================================================================
// Helper: spawn a scenario entity with standard components
// ============================================================================

fn spawn_entity(
    commands: &mut Commands,
    scene_root: Option<Entity>,
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
    name: String,
    transform: Transform,
    body_type: avian3d::prelude::RigidBody,
    collider: avian3d::prelude::Collider,
    mass: f32,
    restitution: f32,
    friction: f32,
) -> Entity {
    let mut ec = commands.spawn((
        ScenarioEntity,
        EditorEntity {
            name,
            tag: "scenario".to_string(),
            visible: true,
            locked: false,
        },
        SceneNode,
        Mesh3d(mesh),
        MeshMaterial3d(material),
        transform,
        Visibility::default(),
        body_type,
        collider,
        avian3d::prelude::Mass(mass),
        avian3d::prelude::Restitution::new(restitution),
        avian3d::prelude::Friction::new(friction),
    ));
    if let Some(root) = scene_root {
        ec.insert(ChildOf(root));
    }
    ec.id()
}

fn make_material(materials: &mut Assets<StandardMaterial>, color: Color) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: color,
        ..default()
    })
}

// ============================================================================
// Scenario implementations
// ============================================================================

fn spawn_newtons_cradle(
    commands: &mut Commands,
    scene_root: Option<Entity>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    s: f32,
) {
    let sphere_mesh = meshes.add(Sphere::new(0.5 * s));
    let mat = make_material(materials, Color::hsl(200.0, 0.7, 0.6));
    let r = 0.5 * s;
    let spacing = r * 2.05;

    // 5 hanging spheres
    for i in 0..5 {
        let x = (i as f32 - 2.0) * spacing;
        let y = 5.0 * s;
        let pulled_back = i == 0; // first ball pulled back
        let px = if pulled_back { x - 3.0 * s } else { x };
        let py = if pulled_back { y + 1.0 * s } else { y };

        spawn_entity(
            commands, scene_root,
            sphere_mesh.clone(), mat.clone(),
            format!("Cradle Ball {}", i),
            Transform::from_translation(Vec3::new(px, py, 0.0)),
            avian3d::prelude::RigidBody::Dynamic,
            avian3d::prelude::Collider::sphere(r),
            1.0, 1.0, 0.0,
        );
    }

    // Anchor bar (static)
    let bar_mesh = meshes.add(Cuboid::new(spacing * 6.0, 0.2 * s, 0.2 * s));
    let bar_mat = make_material(materials, Color::srgb(0.5, 0.5, 0.5));
    spawn_entity(
        commands, scene_root,
        bar_mesh, bar_mat,
        "Cradle Anchor".to_string(),
        Transform::from_translation(Vec3::new(0.0, 8.0 * s, 0.0)),
        avian3d::prelude::RigidBody::Static,
        avian3d::prelude::Collider::cuboid(spacing * 6.0, 0.2 * s, 0.2 * s),
        1.0, 0.5, 0.5,
    );
}

fn spawn_domino_chain(
    commands: &mut Commands,
    scene_root: Option<Entity>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    s: f32,
) {
    let domino_mesh = meshes.add(Cuboid::new(0.1 * s, 1.0 * s, 0.5 * s));
    let mat = make_material(materials, Color::hsl(30.0, 0.8, 0.5));

    for i in 0..20 {
        let angle = i as f32 * 0.15;
        let radius = 3.0 * s + i as f32 * 0.3 * s;
        let x = angle.cos() * radius;
        let z = angle.sin() * radius;
        let facing = angle + std::f32::consts::FRAC_PI_2;

        // First domino is tilted slightly to start the chain
        let tilt = if i == 0 {
            Quat::from_rotation_z(0.15) * Quat::from_rotation_y(facing)
        } else {
            Quat::from_rotation_y(facing)
        };

        spawn_entity(
            commands, scene_root,
            domino_mesh.clone(), mat.clone(),
            format!("Domino {}", i),
            Transform::from_translation(Vec3::new(x, 0.5 * s, z)).with_rotation(tilt),
            avian3d::prelude::RigidBody::Dynamic,
            avian3d::prelude::Collider::cuboid(0.1 * s, 1.0 * s, 0.5 * s),
            0.5, 0.3, 0.6,
        );
    }

    // Ground plane
    spawn_ground(commands, scene_root, meshes, materials, s);
}

fn spawn_wrecking_ball(
    commands: &mut Commands,
    scene_root: Option<Entity>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    s: f32,
) {
    // Wall of boxes
    let box_mesh = meshes.add(Cuboid::new(1.0 * s, 1.0 * s, 1.0 * s));
    let wall_mat = make_material(materials, Color::hsl(0.0, 0.6, 0.6));

    for row in 0..5 {
        for col in 0..4 {
            let x = (col as f32 - 1.5) * 1.1 * s;
            let y = 0.5 * s + row as f32 * 1.1 * s;
            spawn_entity(
                commands, scene_root,
                box_mesh.clone(), wall_mat.clone(),
                format!("Wall Block {},{}", row, col),
                Transform::from_translation(Vec3::new(x, y, 0.0)),
                avian3d::prelude::RigidBody::Dynamic,
                avian3d::prelude::Collider::cuboid(1.0 * s, 1.0 * s, 1.0 * s),
                1.0, 0.3, 0.5,
            );
        }
    }

    // Wrecking ball (heavy sphere offset to the side)
    let ball_mesh = meshes.add(Sphere::new(1.5 * s));
    let ball_mat = make_material(materials, Color::srgb(0.3, 0.3, 0.3));
    spawn_entity(
        commands, scene_root,
        ball_mesh, ball_mat,
        "Wrecking Ball".to_string(),
        Transform::from_translation(Vec3::new(-8.0 * s, 6.0 * s, 0.0)),
        avian3d::prelude::RigidBody::Dynamic,
        avian3d::prelude::Collider::sphere(1.5 * s),
        50.0, 0.2, 0.3,
    );

    spawn_ground(commands, scene_root, meshes, materials, s);
}

fn spawn_stack_test(
    commands: &mut Commands,
    scene_root: Option<Entity>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    s: f32,
) {
    let box_mesh = meshes.add(Cuboid::new(1.0 * s, 1.0 * s, 1.0 * s));

    for i in 0..15 {
        let hue = (i as f32 / 15.0) * 360.0;
        let mat = make_material(materials, Color::hsl(hue, 0.7, 0.6));
        spawn_entity(
            commands, scene_root,
            box_mesh.clone(), mat,
            format!("Stack Box {}", i),
            Transform::from_translation(Vec3::new(0.0, 0.5 * s + i as f32 * 1.05 * s, 0.0)),
            avian3d::prelude::RigidBody::Dynamic,
            avian3d::prelude::Collider::cuboid(1.0 * s, 1.0 * s, 1.0 * s),
            1.0, 0.2, 0.7,
        );
    }

    spawn_ground(commands, scene_root, meshes, materials, s);
}

fn spawn_billiard_break(
    commands: &mut Commands,
    scene_root: Option<Entity>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    s: f32,
) {
    let sphere_mesh = meshes.add(Sphere::new(0.3 * s));
    let r = 0.3 * s;
    let spacing = r * 2.1;

    // Triangle formation
    let ball_mat = make_material(materials, Color::hsl(45.0, 0.8, 0.5));
    let mut idx = 0;
    for row in 0..5 {
        for col in 0..=row {
            let x = (col as f32 - row as f32 / 2.0) * spacing;
            let z = row as f32 * spacing * 0.866;
            spawn_entity(
                commands, scene_root,
                sphere_mesh.clone(), ball_mat.clone(),
                format!("Billiard Ball {}", idx),
                Transform::from_translation(Vec3::new(x, r, z)),
                avian3d::prelude::RigidBody::Dynamic,
                avian3d::prelude::Collider::sphere(r),
                1.0, 0.95, 0.1,
            );
            idx += 1;
        }
    }

    // Cue ball with initial velocity
    let cue_mat = make_material(materials, Color::WHITE);
    let cue_id = spawn_entity(
        commands, scene_root,
        sphere_mesh, cue_mat,
        "Cue Ball".to_string(),
        Transform::from_translation(Vec3::new(0.0, r, -5.0 * s)),
        avian3d::prelude::RigidBody::Dynamic,
        avian3d::prelude::Collider::sphere(r),
        1.0, 0.95, 0.1,
    );
    commands.entity(cue_id).insert(avian3d::prelude::LinearVelocity(Vec3::new(0.0, 0.0, 15.0 * s)));

    spawn_ground(commands, scene_root, meshes, materials, s);
}

fn spawn_inclined_plane(
    commands: &mut Commands,
    scene_root: Option<Entity>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    s: f32,
) {
    // Static ramp
    let ramp_mesh = meshes.add(Cuboid::new(4.0 * s, 0.2 * s, 6.0 * s));
    let ramp_mat = make_material(materials, Color::srgb(0.6, 0.6, 0.6));
    let ramp_angle = 0.4; // ~23 degrees
    spawn_entity(
        commands, scene_root,
        ramp_mesh, ramp_mat,
        "Ramp".to_string(),
        Transform::from_translation(Vec3::new(0.0, 3.0 * s, 0.0))
            .with_rotation(Quat::from_rotation_x(ramp_angle)),
        avian3d::prelude::RigidBody::Static,
        avian3d::prelude::Collider::cuboid(4.0 * s, 0.2 * s, 6.0 * s),
        1.0, 0.3, 0.3,
    );

    // Objects on the ramp
    let sphere_mesh = meshes.add(Sphere::new(0.4 * s));
    let box_mesh = meshes.add(Cuboid::new(0.8 * s, 0.8 * s, 0.8 * s));
    let mat1 = make_material(materials, Color::hsl(120.0, 0.7, 0.5));
    let mat2 = make_material(materials, Color::hsl(240.0, 0.7, 0.5));

    spawn_entity(
        commands, scene_root,
        sphere_mesh, mat1,
        "Ramp Sphere".to_string(),
        Transform::from_translation(Vec3::new(-0.5 * s, 5.0 * s, -1.0 * s)),
        avian3d::prelude::RigidBody::Dynamic,
        avian3d::prelude::Collider::sphere(0.4 * s),
        1.0, 0.3, 0.3,
    );

    spawn_entity(
        commands, scene_root,
        box_mesh, mat2,
        "Ramp Box".to_string(),
        Transform::from_translation(Vec3::new(0.5 * s, 5.5 * s, -1.5 * s)),
        avian3d::prelude::RigidBody::Dynamic,
        avian3d::prelude::Collider::cuboid(0.8 * s, 0.8 * s, 0.8 * s),
        2.0, 0.2, 0.4,
    );

    spawn_ground(commands, scene_root, meshes, materials, s);
}

fn spawn_pendulum(
    commands: &mut Commands,
    scene_root: Option<Entity>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    s: f32,
) {
    // Anchor point (static)
    let anchor_mesh = meshes.add(Cuboid::new(0.3 * s, 0.3 * s, 0.3 * s));
    let anchor_mat = make_material(materials, Color::srgb(0.5, 0.5, 0.5));
    spawn_entity(
        commands, scene_root,
        anchor_mesh, anchor_mat,
        "Pendulum Anchor".to_string(),
        Transform::from_translation(Vec3::new(0.0, 8.0 * s, 0.0)),
        avian3d::prelude::RigidBody::Static,
        avian3d::prelude::Collider::cuboid(0.3 * s, 0.3 * s, 0.3 * s),
        1.0, 0.5, 0.5,
    );

    // Pendulum bob (pulled to the side)
    let bob_mesh = meshes.add(Sphere::new(0.6 * s));
    let bob_mat = make_material(materials, Color::hsl(280.0, 0.7, 0.6));
    spawn_entity(
        commands, scene_root,
        bob_mesh, bob_mat,
        "Pendulum Bob".to_string(),
        Transform::from_translation(Vec3::new(4.0 * s, 5.0 * s, 0.0)),
        avian3d::prelude::RigidBody::Dynamic,
        avian3d::prelude::Collider::sphere(0.6 * s),
        2.0, 0.8, 0.1,
    );

    spawn_ground(commands, scene_root, meshes, materials, s);
}

fn spawn_projectile_launch(
    commands: &mut Commands,
    scene_root: Option<Entity>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    s: f32,
) {
    // Launch pad (static box)
    let pad_mesh = meshes.add(Cuboid::new(2.0 * s, 0.5 * s, 2.0 * s));
    let pad_mat = make_material(materials, Color::srgb(0.4, 0.4, 0.4));
    spawn_entity(
        commands, scene_root,
        pad_mesh, pad_mat,
        "Launch Pad".to_string(),
        Transform::from_translation(Vec3::new(-5.0 * s, 0.25 * s, 0.0)),
        avian3d::prelude::RigidBody::Static,
        avian3d::prelude::Collider::cuboid(2.0 * s, 0.5 * s, 2.0 * s),
        1.0, 0.5, 0.5,
    );

    // Projectile with initial velocity at 45 degrees
    let proj_mesh = meshes.add(Sphere::new(0.3 * s));
    let proj_mat = make_material(materials, Color::hsl(15.0, 0.9, 0.5));
    let speed = 15.0 * s;
    let angle = std::f32::consts::FRAC_PI_4; // 45 degrees
    let vel = Vec3::new(speed * angle.cos(), speed * angle.sin(), 0.0);

    let proj_id = spawn_entity(
        commands, scene_root,
        proj_mesh, proj_mat,
        "Projectile".to_string(),
        Transform::from_translation(Vec3::new(-5.0 * s, 1.0 * s, 0.0)),
        avian3d::prelude::RigidBody::Dynamic,
        avian3d::prelude::Collider::sphere(0.3 * s),
        1.0, 0.5, 0.3,
    );
    commands.entity(proj_id).insert(avian3d::prelude::LinearVelocity(vel));

    // Target wall
    let wall_mesh = meshes.add(Cuboid::new(0.5 * s, 4.0 * s, 4.0 * s));
    let wall_mat = make_material(materials, Color::hsl(0.0, 0.5, 0.5));
    spawn_entity(
        commands, scene_root,
        wall_mesh, wall_mat,
        "Target Wall".to_string(),
        Transform::from_translation(Vec3::new(10.0 * s, 2.0 * s, 0.0)),
        avian3d::prelude::RigidBody::Static,
        avian3d::prelude::Collider::cuboid(0.5 * s, 4.0 * s, 4.0 * s),
        1.0, 0.3, 0.5,
    );

    spawn_ground(commands, scene_root, meshes, materials, s);
}

fn spawn_gauntlet(
    commands: &mut Commands,
    scene_root: Option<Entity>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    s: f32,
) {
    // Series of ramps and platforms
    let platform_mesh = meshes.add(Cuboid::new(6.0 * s, 0.3 * s, 3.0 * s));
    let plat_mat = make_material(materials, Color::srgb(0.5, 0.5, 0.5));

    // Ramp 1 - downhill start
    spawn_entity(
        commands, scene_root,
        platform_mesh.clone(), plat_mat.clone(),
        "Gauntlet Ramp 1".to_string(),
        Transform::from_translation(Vec3::new(-6.0 * s, 4.0 * s, 0.0))
            .with_rotation(Quat::from_rotation_z(-0.3)),
        avian3d::prelude::RigidBody::Static,
        avian3d::prelude::Collider::cuboid(6.0 * s, 0.3 * s, 3.0 * s),
        1.0, 0.3, 0.4,
    );

    // Flat platform
    spawn_entity(
        commands, scene_root,
        platform_mesh.clone(), plat_mat.clone(),
        "Gauntlet Platform".to_string(),
        Transform::from_translation(Vec3::new(2.0 * s, 2.0 * s, 0.0)),
        avian3d::prelude::RigidBody::Static,
        avian3d::prelude::Collider::cuboid(6.0 * s, 0.3 * s, 3.0 * s),
        1.0, 0.3, 0.4,
    );

    // Ramp 2 - to the end
    spawn_entity(
        commands, scene_root,
        platform_mesh, plat_mat,
        "Gauntlet Ramp 2".to_string(),
        Transform::from_translation(Vec3::new(10.0 * s, 1.0 * s, 0.0))
            .with_rotation(Quat::from_rotation_z(-0.2)),
        avian3d::prelude::RigidBody::Static,
        avian3d::prelude::Collider::cuboid(6.0 * s, 0.3 * s, 3.0 * s),
        1.0, 0.3, 0.4,
    );

    // Obstacles - swinging pendulum bobs
    let obstacle_mesh = meshes.add(Sphere::new(0.8 * s));
    let obs_mat = make_material(materials, Color::hsl(0.0, 0.8, 0.5));
    for i in 0..3 {
        let x = -2.0 * s + i as f32 * 4.0 * s;
        spawn_entity(
            commands, scene_root,
            obstacle_mesh.clone(), obs_mat.clone(),
            format!("Gauntlet Obstacle {}", i),
            Transform::from_translation(Vec3::new(x, 6.0 * s, 0.0)),
            avian3d::prelude::RigidBody::Dynamic,
            avian3d::prelude::Collider::sphere(0.8 * s),
            5.0, 0.5, 0.3,
        );
    }

    // Test ball
    let ball_mesh = meshes.add(Sphere::new(0.5 * s));
    let ball_mat = make_material(materials, Color::hsl(180.0, 0.8, 0.5));
    spawn_entity(
        commands, scene_root,
        ball_mesh, ball_mat,
        "Gauntlet Ball".to_string(),
        Transform::from_translation(Vec3::new(-8.0 * s, 6.0 * s, 0.0)),
        avian3d::prelude::RigidBody::Dynamic,
        avian3d::prelude::Collider::sphere(0.5 * s),
        1.0, 0.5, 0.3,
    );

    spawn_ground(commands, scene_root, meshes, materials, s);
}

fn spawn_avalanche(
    commands: &mut Commands,
    scene_root: Option<Entity>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    s: f32,
) {
    // Steep slope
    let slope_mesh = meshes.add(Cuboid::new(8.0 * s, 0.3 * s, 8.0 * s));
    let slope_mat = make_material(materials, Color::srgb(0.45, 0.4, 0.35));
    spawn_entity(
        commands, scene_root,
        slope_mesh, slope_mat,
        "Avalanche Slope".to_string(),
        Transform::from_translation(Vec3::new(0.0, 5.0 * s, 0.0))
            .with_rotation(Quat::from_rotation_x(0.6)),
        avian3d::prelude::RigidBody::Static,
        avian3d::prelude::Collider::cuboid(8.0 * s, 0.3 * s, 8.0 * s),
        1.0, 0.2, 0.3,
    );

    // Mixed shapes piled on the slope
    let sphere_mesh = meshes.add(Sphere::new(0.4 * s));
    let box_mesh = meshes.add(Cuboid::new(0.7 * s, 0.7 * s, 0.7 * s));

    for i in 0..30 {
        let angle = i as f32 * 2.399; // golden angle
        let radius = (i as f32 / 30.0).sqrt() * 3.0 * s;
        let x = angle.cos() * radius;
        let z = angle.sin() * radius;
        let y = 8.0 * s + (i as f32 * 0.2) * s;
        let hue = (i as f32 * 17.0) % 360.0;
        let mat = make_material(materials, Color::hsl(hue, 0.7, 0.6));

        if i % 2 == 0 {
            spawn_entity(
                commands, scene_root,
                sphere_mesh.clone(), mat,
                format!("Avalanche Sphere {}", i),
                Transform::from_translation(Vec3::new(x, y, z)),
                avian3d::prelude::RigidBody::Dynamic,
                avian3d::prelude::Collider::sphere(0.4 * s),
                1.0, 0.3, 0.4,
            );
        } else {
            spawn_entity(
                commands, scene_root,
                box_mesh.clone(), mat,
                format!("Avalanche Box {}", i),
                Transform::from_translation(Vec3::new(x, y, z)),
                avian3d::prelude::RigidBody::Dynamic,
                avian3d::prelude::Collider::cuboid(0.7 * s, 0.7 * s, 0.7 * s),
                1.5, 0.2, 0.5,
            );
        }
    }

    spawn_ground(commands, scene_root, meshes, materials, s);
}

/// Spawn a static ground plane (shared by many scenarios)
fn spawn_ground(
    commands: &mut Commands,
    scene_root: Option<Entity>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    s: f32,
) {
    let ground_mesh = meshes.add(Cuboid::new(50.0 * s, 0.1, 50.0 * s));
    let ground_mat = make_material(materials, Color::srgb(0.35, 0.35, 0.35));
    spawn_entity(
        commands, scene_root,
        ground_mesh, ground_mat,
        "Scenario Ground".to_string(),
        Transform::from_translation(Vec3::new(0.0, -0.05, 0.0)),
        avian3d::prelude::RigidBody::Static,
        avian3d::prelude::Collider::cuboid(50.0 * s, 0.1, 50.0 * s),
        1.0, 0.3, 0.5,
    );
}
