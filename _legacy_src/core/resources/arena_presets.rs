//! Arena environment presets — standalone resource for spawning arena environments

use bevy::prelude::*;
use crate::core::{EditorEntity, SceneNode};
use crate::spawn::EditorSceneRoot;

/// Marker for arena entities
#[derive(Component)]
pub struct ArenaEntity;

/// Arena environment type
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ArenaType {
    #[default]
    Walled,
    StairFall,
    MovingPlatforms,
    WindyDay,
    HillSlide,
    Pinball,
    GolfCourse,
    Funnel,
}

impl ArenaType {
    pub fn label(&self) -> &'static str {
        match self {
            ArenaType::Walled => "Walled Arena",
            ArenaType::StairFall => "Stair Fall",
            ArenaType::MovingPlatforms => "Moving Platforms",
            ArenaType::WindyDay => "Windy Day",
            ArenaType::HillSlide => "Hill Slide",
            ArenaType::Pinball => "Pinball",
            ArenaType::GolfCourse => "Golf Course",
            ArenaType::Funnel => "Funnel",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ArenaType::Walled => "Flat floor with walls, ramps, and pillars",
            ArenaType::StairFall => "Giant staircase — objects tumble down steps",
            ArenaType::MovingPlatforms => "Kinematic platforms that move back and forth",
            ArenaType::WindyDay => "Flat arena with constant lateral wind force",
            ArenaType::HillSlide => "Steep angled slope — objects slide and roll to the bottom",
            ArenaType::Pinball => "Vertical board with bumpers, flippers, and gutters",
            ArenaType::GolfCourse => "Rolling hills with holes — objects settle into valleys",
            ArenaType::Funnel => "Large funnel that channels objects down into a tight space",
        }
    }

    pub const ALL: &'static [ArenaType] = &[
        ArenaType::Walled,
        ArenaType::StairFall,
        ArenaType::MovingPlatforms,
        ArenaType::WindyDay,
        ArenaType::HillSlide,
        ArenaType::Pinball,
        ArenaType::GolfCourse,
        ArenaType::Funnel,
    ];
}

/// Commands for the arena presets system
#[derive(Clone, Debug)]
pub enum ArenaCommand {
    Spawn,
    Clear,
}

/// State resource for arena presets
#[derive(Resource)]
pub struct ArenaPresetsState {
    pub arena_type: ArenaType,
    pub scale: f32,
    pub arena_entity_count: usize,
    pub commands: Vec<ArenaCommand>,
    pub has_active_arena: bool,
}

impl Default for ArenaPresetsState {
    fn default() -> Self {
        Self {
            arena_type: ArenaType::default(),
            scale: 1.0,
            arena_entity_count: 0,
            commands: Vec::new(),
            has_active_arena: false,
        }
    }
}

/// System that processes arena commands: spawns/clears arena environments
pub fn process_arena_commands(
    mut state: ResMut<ArenaPresetsState>,
    mut commands: Commands,
    arena_entities: Query<Entity, With<ArenaEntity>>,
    scene_roots: Query<Entity, With<EditorSceneRoot>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let cmds: Vec<ArenaCommand> = state.commands.drain(..).collect();
    if cmds.is_empty() {
        return;
    }

    let scene_root = scene_roots.iter().next();

    for cmd in cmds {
        match cmd {
            ArenaCommand::Spawn => {
                // Despawn all existing arena entities
                for entity in arena_entities.iter() {
                    commands.entity(entity).despawn();
                }

                let scale = state.scale;
                let arena_type = state.arena_type;

                match arena_type {
                    ArenaType::Walled => spawn_walled_arena(&mut commands, &mut meshes, &mut materials, scene_root, scale),
                    ArenaType::StairFall => spawn_stair_fall(&mut commands, &mut meshes, &mut materials, scene_root, scale),
                    ArenaType::MovingPlatforms => spawn_moving_platforms(&mut commands, &mut meshes, &mut materials, scene_root, scale),
                    ArenaType::WindyDay => spawn_windy_day(&mut commands, &mut meshes, &mut materials, scene_root, scale),
                    ArenaType::HillSlide => spawn_hill_slide(&mut commands, &mut meshes, &mut materials, scene_root, scale),
                    ArenaType::Pinball => spawn_pinball(&mut commands, &mut meshes, &mut materials, scene_root, scale),
                    ArenaType::GolfCourse => spawn_golf_course(&mut commands, &mut meshes, &mut materials, scene_root, scale),
                    ArenaType::Funnel => spawn_funnel(&mut commands, &mut meshes, &mut materials, scene_root, scale),
                }

                state.has_active_arena = true;
                state.arena_entity_count = arena_entities.iter().count();
            }
            ArenaCommand::Clear => {
                for entity in arena_entities.iter() {
                    commands.entity(entity).despawn();
                }
                state.has_active_arena = false;
                state.arena_entity_count = 0;
            }
        }
    }
}

/// System that animates kinematic platforms for the MovingPlatforms arena
pub fn animate_arena_kinematic(
    state: Res<ArenaPresetsState>,
    time: Res<Time>,
    mut kinematic_transforms: Query<
        (&mut Transform, &avian3d::prelude::RigidBody),
        (With<ArenaEntity>, Without<avian3d::prelude::LinearVelocity>),
    >,
) {
    if !state.has_active_arena || state.arena_type != ArenaType::MovingPlatforms {
        return;
    }

    let t = time.elapsed_secs();
    let mut platform_idx = 0u32;
    for (mut transform, body) in kinematic_transforms.iter_mut() {
        if *body == avian3d::prelude::RigidBody::Kinematic {
            let phase = platform_idx as f32 * 1.5;
            let offset = (t * 0.8 + phase).sin() * 4.0;
            transform.translation.x = offset;
            platform_idx += 1;
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

pub fn spawn_static(
    commands: &mut Commands,
    name: &str,
    mesh: Handle<Mesh>,
    mat: Handle<StandardMaterial>,
    transform: Transform,
    collider: avian3d::prelude::Collider,
    scene_root: Option<Entity>,
) {
    let mut ec = commands.spawn((
        ArenaEntity,
        EditorEntity { name: name.to_string(), tag: "arena".to_string(), visible: true, locked: false },
        SceneNode,
        Mesh3d(mesh),
        MeshMaterial3d(mat),
        transform,
        Visibility::default(),
        avian3d::prelude::RigidBody::Static,
        collider,
        avian3d::prelude::Friction::new(0.6),
    ));
    if let Some(root) = scene_root {
        ec.insert(ChildOf(root));
    }
}

pub fn spawn_kinematic(
    commands: &mut Commands,
    name: &str,
    mesh: Handle<Mesh>,
    mat: Handle<StandardMaterial>,
    transform: Transform,
    collider: avian3d::prelude::Collider,
    scene_root: Option<Entity>,
) {
    let mut ec = commands.spawn((
        ArenaEntity,
        EditorEntity { name: name.to_string(), tag: "arena".to_string(), visible: true, locked: false },
        SceneNode,
        Mesh3d(mesh),
        MeshMaterial3d(mat),
        transform,
        Visibility::default(),
        avian3d::prelude::RigidBody::Kinematic,
        collider,
        avian3d::prelude::Friction::new(0.5),
    ));
    if let Some(root) = scene_root {
        ec.insert(ChildOf(root));
    }
}

// ============================================================================
// Arena environments
// ============================================================================

/// Walled arena: flat floor, 4 walls, ramps, pillars
pub fn spawn_walled_arena(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    scene_root: Option<Entity>,
    scale: f32,
) {
    let arena_size = 20.0 * scale;
    let wall_mat = materials.add(StandardMaterial { base_color: Color::srgba(0.3, 0.3, 0.35, 0.8), ..default() });
    let ramp_mat = materials.add(StandardMaterial { base_color: Color::srgba(0.4, 0.35, 0.25, 0.9), ..default() });
    let obstacle_mat = materials.add(StandardMaterial { base_color: Color::srgba(0.5, 0.25, 0.25, 0.9), ..default() });

    // Floor
    spawn_static(commands, "Floor",
        meshes.add(Cuboid::new(arena_size * 2.0, 0.5, arena_size * 2.0)), wall_mat.clone(),
        Transform::from_translation(Vec3::new(0.0, -0.25, 0.0)),
        avian3d::prelude::Collider::cuboid(arena_size * 2.0, 0.5, arena_size * 2.0), scene_root);

    // Walls
    let wh = 4.0 * scale;
    for (pos, size) in [
        (Vec3::new(0.0, wh * 0.5, arena_size), Vec3::new(arena_size * 2.0, wh, 0.5)),
        (Vec3::new(0.0, wh * 0.5, -arena_size), Vec3::new(arena_size * 2.0, wh, 0.5)),
        (Vec3::new(arena_size, wh * 0.5, 0.0), Vec3::new(0.5, wh, arena_size * 2.0)),
        (Vec3::new(-arena_size, wh * 0.5, 0.0), Vec3::new(0.5, wh, arena_size * 2.0)),
    ] {
        spawn_static(commands, "Wall",
            meshes.add(Cuboid::new(size.x, size.y, size.z)), wall_mat.clone(),
            Transform::from_translation(pos),
            avian3d::prelude::Collider::cuboid(size.x, size.y, size.z), scene_root);
    }

    // Ramps
    let rl = arena_size * 0.6;
    let rw = arena_size * 0.4;
    for (pos, rot) in [
        (Vec3::new(arena_size * 0.4, 1.5 * scale, 0.0), Quat::from_rotation_z(-0.25)),
        (Vec3::new(-arena_size * 0.4, 1.5 * scale, 0.0), Quat::from_rotation_z(0.25)),
    ] {
        spawn_static(commands, "Ramp",
            meshes.add(Cuboid::new(rl, 0.3, rw)), ramp_mat.clone(),
            Transform::from_translation(pos).with_rotation(rot),
            avian3d::prelude::Collider::cuboid(rl, 0.3, rw), scene_root);
    }

    // Pillars
    let pc = 8usize;
    for i in 0..pc {
        let angle = (i as f32 / pc as f32) * std::f32::consts::TAU;
        let dist = arena_size * 0.3;
        let pos = Vec3::new(angle.cos() * dist, 1.5 * scale, angle.sin() * dist);
        if i % 2 == 0 {
            spawn_static(commands, "Pillar",
                meshes.add(Cylinder::new(0.8 * scale, 3.0 * scale)), obstacle_mat.clone(),
                Transform::from_translation(pos),
                avian3d::prelude::Collider::cylinder(0.8 * scale, 3.0 * scale), scene_root);
        } else {
            spawn_static(commands, "Obstacle",
                meshes.add(Cuboid::new(1.5 * scale, 2.0 * scale, 1.5 * scale)), obstacle_mat.clone(),
                Transform::from_translation(pos),
                avian3d::prelude::Collider::cuboid(1.5 * scale, 2.0 * scale, 1.5 * scale), scene_root);
        }
    }
}

/// Stair Fall: giant staircase, objects tumble down
pub fn spawn_stair_fall(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    scene_root: Option<Entity>,
    scale: f32,
) {
    let step_count = 20;
    let step_w = 20.0 * scale;
    let step_depth = 2.0 * scale;
    let step_height = 0.6 * scale;
    let stair_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.35, 0.35, 0.4), ..default() });
    let rail_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.5, 0.3, 0.2), ..default() });

    for s in 0..step_count {
        let y = s as f32 * step_height;
        let z = s as f32 * step_depth;
        spawn_static(commands, "Step",
            meshes.add(Cuboid::new(step_w, step_height, step_depth)), stair_mat.clone(),
            Transform::from_translation(Vec3::new(0.0, y + step_height * 0.5, z)),
            avian3d::prelude::Collider::cuboid(step_w, step_height, step_depth), scene_root);
    }

    // Side rails
    let total_len = step_count as f32 * step_depth;
    let total_h = step_count as f32 * step_height;
    let rail_angle = (total_h / total_len).atan();
    let rail_len = (total_len * total_len + total_h * total_h).sqrt();
    for side in [-1.0f32, 1.0] {
        spawn_static(commands, "Rail",
            meshes.add(Cuboid::new(0.3, 2.0 * scale, rail_len)), rail_mat.clone(),
            Transform::from_translation(Vec3::new(
                side * step_w * 0.5,
                total_h * 0.5 + 1.0 * scale,
                total_len * 0.5,
            )).with_rotation(Quat::from_rotation_x(-rail_angle)),
            avian3d::prelude::Collider::cuboid(0.3, 2.0 * scale, rail_len), scene_root);
    }

    // Landing pad at bottom
    spawn_static(commands, "Landing",
        meshes.add(Cuboid::new(step_w * 1.5, 0.5, step_w)), stair_mat.clone(),
        Transform::from_translation(Vec3::new(0.0, -0.25, -step_w * 0.3)),
        avian3d::prelude::Collider::cuboid(step_w * 1.5, 0.5, step_w), scene_root);
}

/// Moving Platforms: kinematic platforms that oscillate
pub fn spawn_moving_platforms(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    scene_root: Option<Entity>,
    scale: f32,
) {
    let platform_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.2, 0.5, 0.6), ..default() });
    let floor_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.3, 0.3, 0.35), ..default() });
    let arena = 20.0 * scale;

    // Ground floor
    spawn_static(commands, "Floor",
        meshes.add(Cuboid::new(arena * 2.0, 0.5, arena * 2.0)), floor_mat.clone(),
        Transform::from_translation(Vec3::new(0.0, -0.25, 0.0)),
        avian3d::prelude::Collider::cuboid(arena * 2.0, 0.5, arena * 2.0), scene_root);

    // Kinematic platforms at different heights
    let platform_count = 6;
    let pw = arena * 0.4;
    let pd = arena * 0.3;
    for i in 0..platform_count {
        let y = 2.0 * scale + i as f32 * 2.5 * scale;
        let z = (i as f32 - platform_count as f32 * 0.5) * pd * 0.8;
        spawn_kinematic(commands, "Platform",
            meshes.add(Cuboid::new(pw, 0.4, pd)), platform_mat.clone(),
            Transform::from_translation(Vec3::new(0.0, y, z)),
            avian3d::prelude::Collider::cuboid(pw, 0.4, pd), scene_root);
    }

    // Walls to contain
    let wh = 15.0 * scale;
    for (pos, size) in [
        (Vec3::new(0.0, wh * 0.5, arena), Vec3::new(arena * 2.0, wh, 0.5)),
        (Vec3::new(0.0, wh * 0.5, -arena), Vec3::new(arena * 2.0, wh, 0.5)),
        (Vec3::new(arena, wh * 0.5, 0.0), Vec3::new(0.5, wh, arena * 2.0)),
        (Vec3::new(-arena, wh * 0.5, 0.0), Vec3::new(0.5, wh, arena * 2.0)),
    ] {
        spawn_static(commands, "Wall",
            meshes.add(Cuboid::new(size.x, size.y, size.z)), floor_mat.clone(),
            Transform::from_translation(pos),
            avian3d::prelude::Collider::cuboid(size.x, size.y, size.z), scene_root);
    }
}

/// Windy Day: flat open arena, wind force applied each frame
pub fn spawn_windy_day(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    scene_root: Option<Entity>,
    scale: f32,
) {
    let arena = 20.0 * scale;
    let floor_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.25, 0.4, 0.25), ..default() });
    let wall_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.3, 0.3, 0.35), ..default() });

    // Floor (grass-colored)
    spawn_static(commands, "Ground",
        meshes.add(Cuboid::new(arena * 2.0, 0.5, arena * 2.0)), floor_mat,
        Transform::from_translation(Vec3::new(0.0, -0.25, 0.0)),
        avian3d::prelude::Collider::cuboid(arena * 2.0, 0.5, arena * 2.0), scene_root);

    // Tall walls to catch wind-blown objects
    let wh = 8.0 * scale;
    for (pos, size) in [
        (Vec3::new(0.0, wh * 0.5, arena), Vec3::new(arena * 2.0, wh, 0.5)),
        (Vec3::new(0.0, wh * 0.5, -arena), Vec3::new(arena * 2.0, wh, 0.5)),
        (Vec3::new(arena, wh * 0.5, 0.0), Vec3::new(0.5, wh, arena * 2.0)),
        (Vec3::new(-arena, wh * 0.5, 0.0), Vec3::new(0.5, wh, arena * 2.0)),
    ] {
        spawn_static(commands, "Wall",
            meshes.add(Cuboid::new(size.x, size.y, size.z)), wall_mat.clone(),
            Transform::from_translation(pos),
            avian3d::prelude::Collider::cuboid(size.x, size.y, size.z), scene_root);
    }

    // Scattered windbreak obstacles
    let obs_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.4, 0.3, 0.2), ..default() });
    let obs_count = 8;
    for i in 0..obs_count {
        let angle = (i as f32 / obs_count as f32) * std::f32::consts::TAU;
        let dist = arena * 0.4;
        let pos = Vec3::new(angle.cos() * dist, 1.0 * scale, angle.sin() * dist);
        spawn_static(commands, "Windbreak",
            meshes.add(Cuboid::new(2.0 * scale, 2.0 * scale, 0.5)), obs_mat.clone(),
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(angle)),
            avian3d::prelude::Collider::cuboid(2.0 * scale, 2.0 * scale, 0.5), scene_root);
    }
}

/// Hill Slide: steep slope with bumps
pub fn spawn_hill_slide(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    scene_root: Option<Entity>,
    scale: f32,
) {
    let hill_len = 20.0 * scale;
    let hill_w = 20.0 * scale;
    let slope_angle: f32 = 0.35; // ~20 degrees
    let hill_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.35, 0.5, 0.25), ..default() });
    let bump_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.45, 0.35, 0.25), ..default() });
    let wall_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.3, 0.3, 0.35), ..default() });

    // Main slope
    let slope_len = hill_len * 1.5;
    let slope_rise = slope_len * slope_angle.sin();
    spawn_static(commands, "Slope",
        meshes.add(Cuboid::new(hill_w, 0.5, slope_len)), hill_mat.clone(),
        Transform::from_translation(Vec3::new(0.0, slope_rise * 0.5, 0.0))
            .with_rotation(Quat::from_rotation_x(-slope_angle)),
        avian3d::prelude::Collider::cuboid(hill_w, 0.5, slope_len), scene_root);

    // Flat landing area at bottom
    let landing_z = slope_len * slope_angle.cos() * 0.5 + hill_w * 0.5;
    spawn_static(commands, "Landing",
        meshes.add(Cuboid::new(hill_w * 1.5, 0.5, hill_w)), hill_mat.clone(),
        Transform::from_translation(Vec3::new(0.0, -0.25, landing_z)),
        avian3d::prelude::Collider::cuboid(hill_w * 1.5, 0.5, hill_w), scene_root);

    // Bumps on the slope
    let bump_count = (hill_len as usize / 3).clamp(3, 12);
    for i in 0..bump_count {
        let t = (i as f32 + 0.5) / bump_count as f32;
        let z = (t - 0.5) * slope_len * slope_angle.cos();
        let y = (t - 0.5) * slope_len * slope_angle.sin() + slope_rise * 0.5 + 0.5;
        let x = ((i as f32 * 3.7).sin()) * hill_w * 0.3;
        spawn_static(commands, "Bump",
            meshes.add(Sphere::new(0.8 * scale)), bump_mat.clone(),
            Transform::from_translation(Vec3::new(x, y, z)),
            avian3d::prelude::Collider::sphere(0.8 * scale), scene_root);
    }

    // Side rails on slope
    for side in [-1.0f32, 1.0] {
        spawn_static(commands, "Rail",
            meshes.add(Cuboid::new(0.3, 1.5 * scale, slope_len)), wall_mat.clone(),
            Transform::from_translation(Vec3::new(side * hill_w * 0.5, slope_rise * 0.5 + 0.75 * scale, 0.0))
                .with_rotation(Quat::from_rotation_x(-slope_angle)),
            avian3d::prelude::Collider::cuboid(0.3, 1.5 * scale, slope_len), scene_root);
    }
}

/// Pinball: vertical-ish board with bumpers
pub fn spawn_pinball(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    scene_root: Option<Entity>,
    scale: f32,
) {
    let board_w = 20.0 * scale;
    let board_h = board_w * 2.0;
    let tilt: f32 = 0.15; // slight tilt from vertical
    let board_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.15, 0.2, 0.15), ..default() });
    let bumper_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.8, 0.2, 0.2), ..default() });
    let wall_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.4, 0.4, 0.45), ..default() });

    // Back board (slightly tilted from vertical)
    spawn_static(commands, "Board",
        meshes.add(Cuboid::new(board_w, board_h, 0.3)), board_mat,
        Transform::from_translation(Vec3::new(0.0, board_h * 0.5, -1.0 * scale))
            .with_rotation(Quat::from_rotation_x(tilt)),
        avian3d::prelude::Collider::cuboid(board_w, board_h, 0.3), scene_root);

    // Front glass (keeps balls on the board)
    let glass_mat = materials.add(StandardMaterial { base_color: Color::srgba(0.5, 0.5, 0.6, 0.2), ..default() });
    spawn_static(commands, "Glass",
        meshes.add(Cuboid::new(board_w, board_h, 0.1)), glass_mat,
        Transform::from_translation(Vec3::new(0.0, board_h * 0.5, 0.8 * scale))
            .with_rotation(Quat::from_rotation_x(tilt)),
        avian3d::prelude::Collider::cuboid(board_w, board_h, 0.1), scene_root);

    // Side walls
    for side in [-1.0f32, 1.0] {
        spawn_static(commands, "Side",
            meshes.add(Cuboid::new(0.3, board_h, 2.0 * scale)), wall_mat.clone(),
            Transform::from_translation(Vec3::new(side * board_w * 0.5, board_h * 0.5, 0.0))
                .with_rotation(Quat::from_rotation_x(tilt)),
            avian3d::prelude::Collider::cuboid(0.3, board_h, 2.0 * scale), scene_root);
    }

    // Bottom catcher
    spawn_static(commands, "Bottom",
        meshes.add(Cuboid::new(board_w, 0.5, 2.0 * scale)), wall_mat.clone(),
        Transform::from_translation(Vec3::new(0.0, 0.25, 0.0)),
        avian3d::prelude::Collider::cuboid(board_w, 0.5, 2.0 * scale), scene_root);

    // Round bumpers scattered on the board
    let bumper_count = 10;
    for i in 0..bumper_count {
        let bx = ((i as f32 * 2.399).cos()) * board_w * 0.35;
        let by = 2.0 * scale + (i as f32 / bumper_count as f32) * (board_h - 4.0 * scale);
        spawn_static(commands, "Bumper",
            meshes.add(Cylinder::new(0.6 * scale, 1.5 * scale)), bumper_mat.clone(),
            Transform::from_translation(Vec3::new(bx, by, 0.0))
                .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            avian3d::prelude::Collider::cylinder(0.6 * scale, 1.5 * scale), scene_root);
    }

    // Angled flippers / deflectors
    let deflector_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.6, 0.5, 0.1), ..default() });
    for (pos, angle) in [
        (Vec3::new(-board_w * 0.25, board_h * 0.2, 0.0), 0.4f32),
        (Vec3::new(board_w * 0.25, board_h * 0.2, 0.0), -0.4),
        (Vec3::new(-board_w * 0.15, board_h * 0.55, 0.0), -0.3),
        (Vec3::new(board_w * 0.15, board_h * 0.55, 0.0), 0.3),
    ] {
        spawn_static(commands, "Deflector",
            meshes.add(Cuboid::new(board_w * 0.2, 0.2, 1.2 * scale)), deflector_mat.clone(),
            Transform::from_translation(pos)
                .with_rotation(Quat::from_rotation_z(angle) * Quat::from_rotation_x(tilt)),
            avian3d::prelude::Collider::cuboid(board_w * 0.2, 0.2, 1.2 * scale), scene_root);
    }
}

/// Golf Course: rolling hills with valley holes
pub fn spawn_golf_course(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    scene_root: Option<Entity>,
    scale: f32,
) {
    let course_len = 20.0 * scale * 2.0;
    let course_w = 20.0 * scale;
    let green_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.2, 0.55, 0.2), ..default() });
    let sand_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.75, 0.65, 0.45), ..default() });
    let wall_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.3, 0.3, 0.35), ..default() });

    // Main fairway (flat base)
    spawn_static(commands, "Fairway",
        meshes.add(Cuboid::new(course_w, 0.5, course_len)), green_mat.clone(),
        Transform::from_translation(Vec3::new(0.0, -0.25, 0.0)),
        avian3d::prelude::Collider::cuboid(course_w, 0.5, course_len), scene_root);

    // Rolling hills (raised sections)
    let hill_count = (course_len as usize / 8).clamp(3, 10);
    for i in 0..hill_count {
        let z = (i as f32 / hill_count as f32 - 0.5) * course_len * 0.8;
        let x = ((i as f32 * 2.1).sin()) * course_w * 0.2;
        let hill_r = (2.0 + (i as f32 * 0.7).sin().abs() * 2.0) * scale;
        let hill_h = (0.8 + (i as f32 * 1.3).sin().abs() * 1.5) * scale;
        // Hemisphere-ish bump using a squished sphere collider
        spawn_static(commands, "Hill",
            meshes.add(Sphere::new(hill_r)), green_mat.clone(),
            Transform::from_translation(Vec3::new(x, hill_h * 0.3, z))
                .with_scale(Vec3::new(1.0, hill_h / hill_r, 1.0)),
            avian3d::prelude::Collider::sphere(hill_r), scene_root);
    }

    // Sand traps (low friction areas -- just visual, static boxes slightly recessed)
    for i in 0..3 {
        let z = (i as f32 - 1.0) * course_len * 0.25;
        let x = ((i as f32 * 3.5 + 1.0).sin()) * course_w * 0.25;
        spawn_static(commands, "Sand Trap",
            meshes.add(Cuboid::new(3.0 * scale, 0.1, 3.0 * scale)), sand_mat.clone(),
            Transform::from_translation(Vec3::new(x, 0.05, z)),
            avian3d::prelude::Collider::cuboid(3.0 * scale, 0.1, 3.0 * scale), scene_root);
    }

    // Side walls
    for side in [-1.0f32, 1.0] {
        spawn_static(commands, "Fence",
            meshes.add(Cuboid::new(0.3, 1.5 * scale, course_len)), wall_mat.clone(),
            Transform::from_translation(Vec3::new(side * course_w * 0.5, 0.75 * scale, 0.0)),
            avian3d::prelude::Collider::cuboid(0.3, 1.5 * scale, course_len), scene_root);
    }

    // End walls
    for side in [-1.0f32, 1.0] {
        spawn_static(commands, "End Wall",
            meshes.add(Cuboid::new(course_w, 1.5 * scale, 0.3)), wall_mat.clone(),
            Transform::from_translation(Vec3::new(0.0, 0.75 * scale, side * course_len * 0.5)),
            avian3d::prelude::Collider::cuboid(course_w, 1.5 * scale, 0.3), scene_root);
    }
}

/// Funnel: large cone that channels objects into a tight space
pub fn spawn_funnel(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    scene_root: Option<Entity>,
    scale: f32,
) {
    let funnel_r = 20.0 * scale;
    let funnel_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.35, 0.3, 0.4), ..default() });
    let floor_mat = materials.add(StandardMaterial { base_color: Color::srgb(0.3, 0.3, 0.35), ..default() });

    // Build funnel from angled ring segments
    let segments = 16;
    let funnel_height = funnel_r * 0.8;
    let inner_r = 2.0 * scale; // narrow opening at bottom
    let segment_angle = std::f32::consts::TAU / segments as f32;
    let segment_w = funnel_r * segment_angle * 1.1; // slightly overlapping
    let slope_len = ((funnel_r - inner_r).powi(2) + funnel_height.powi(2)).sqrt();
    let slope_angle = (funnel_height / (funnel_r - inner_r)).atan();

    for i in 0..segments {
        let angle = i as f32 * segment_angle;
        let mid_r = (funnel_r + inner_r) * 0.5;
        let cx = angle.cos() * mid_r;
        let cz = angle.sin() * mid_r;
        let cy = funnel_height * 0.5;

        spawn_static(commands, "Funnel Segment",
            meshes.add(Cuboid::new(segment_w, 0.2, slope_len)), funnel_mat.clone(),
            Transform::from_translation(Vec3::new(cx, cy, cz))
                .with_rotation(
                    Quat::from_rotation_y(-angle)
                    * Quat::from_rotation_x(slope_angle)
                ),
            avian3d::prelude::Collider::cuboid(segment_w, 0.2, slope_len), scene_root);
    }

    // Collection floor at the bottom
    spawn_static(commands, "Collection Floor",
        meshes.add(Cuboid::new(inner_r * 4.0, 0.5, inner_r * 4.0)), floor_mat.clone(),
        Transform::from_translation(Vec3::new(0.0, -0.25, 0.0)),
        avian3d::prelude::Collider::cuboid(inner_r * 4.0, 0.5, inner_r * 4.0), scene_root);

    // Outer containment ring at funnel rim
    for i in 0..segments {
        let angle = i as f32 * segment_angle;
        let cx = angle.cos() * (funnel_r + 0.5);
        let cz = angle.sin() * (funnel_r + 0.5);
        spawn_static(commands, "Rim",
            meshes.add(Cuboid::new(segment_w, 3.0 * scale, 0.5)), funnel_mat.clone(),
            Transform::from_translation(Vec3::new(cx, funnel_height + 1.5 * scale, cz))
                .with_rotation(Quat::from_rotation_y(-angle)),
            avian3d::prelude::Collider::cuboid(segment_w, 3.0 * scale, 0.5), scene_root);
    }
}
