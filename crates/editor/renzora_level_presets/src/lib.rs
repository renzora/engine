//! Level Presets panel for the Renzora editor.
//!
//! Provides a single panel that spawns barebones template levels
//! (meshes + lights + camera) similar to Unreal Engine's level templates.

pub mod panels;
pub mod state;

use std::sync::{Arc, Mutex, RwLock};

use bevy::prelude::*;
use bevy_egui::egui;

use renzora_core::{MeshColor, MeshPrimitive, SceneCamera};
use renzora_editor::{AppEditorExt, EditorPanel, PanelLocation};
use renzora_lighting::Sun;
use renzora_theme::ThemeManager;

use state::*;

// ============================================================================
// Bridge for mutable panel state
// ============================================================================

#[derive(Default)]
struct BridgeInner {
    state: Option<LevelPresetsState>,
}

#[derive(Resource)]
struct LevelPresetsBridge {
    pending: Arc<Mutex<BridgeInner>>,
}

impl Default for LevelPresetsBridge {
    fn default() -> Self {
        Self {
            pending: Arc::new(Mutex::new(BridgeInner::default())),
        }
    }
}

fn get_theme(world: &World) -> renzora_theme::Theme {
    world
        .get_resource::<ThemeManager>()
        .map(|tm| tm.active_theme.clone())
        .unwrap_or_default()
}

// ============================================================================
// Panel
// ============================================================================

struct LevelPresetsPanel {
    bridge: Arc<Mutex<BridgeInner>>,
    local: RwLock<LevelPresetsState>,
}

impl LevelPresetsPanel {
    fn new(bridge: Arc<Mutex<BridgeInner>>) -> Self {
        Self {
            bridge,
            local: RwLock::new(LevelPresetsState::default()),
        }
    }
}

impl EditorPanel for LevelPresetsPanel {
    fn id(&self) -> &str { "level_presets" }
    fn title(&self) -> &str { "Level Presets" }
    fn icon(&self) -> Option<&str> { Some(egui_phosphor::regular::MAP_TRIFOLD) }
    fn default_location(&self) -> PanelLocation { PanelLocation::Right }
    fn min_size(&self) -> [f32; 2] { [250.0, 300.0] }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        if let Some(state) = world.get_resource::<LevelPresetsState>() {
            if let Ok(mut local) = self.local.write() {
                local.entity_count = state.entity_count;
                local.has_active_level = state.has_active_level;
            }
        }
        let theme = get_theme(world);
        if let Ok(mut local) = self.local.write() {
            panels::presets::render_level_presets_content(ui, &mut local, &theme);
        }
        if let Ok(mut pending) = self.bridge.lock() {
            if let Ok(mut local) = self.local.write() {
                pending.state = Some(local.clone());
                local.commands.clear();
            }
        }
    }
}

// ============================================================================
// Sync bridge → world
// ============================================================================

fn sync_bridge(bridge: Res<LevelPresetsBridge>, mut state: ResMut<LevelPresetsState>) {
    if let Ok(mut pending) = bridge.pending.lock() {
        if let Some(s) = pending.state.take() {
            state.selected = s.selected;
            state.scale = s.scale;
            state.commands = s.commands;
        }
    }
}

// ============================================================================
// Command processing
// ============================================================================

fn process_level_commands(
    mut state: ResMut<LevelPresetsState>,
    mut commands: Commands,
    level_entities: Query<Entity, With<LevelPresetEntity>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    state.entity_count = level_entities.iter().count();

    let cmds: Vec<LevelCommand> = state.commands.drain(..).collect();
    if cmds.is_empty() {
        return;
    }

    let scale = state.scale;
    let preset = state.selected;

    for cmd in cmds {
        match cmd {
            LevelCommand::Spawn => {
                for entity in level_entities.iter() {
                    commands.entity(entity).despawn();
                }
                spawn_level(&mut commands, &mut meshes, &mut materials, preset, scale);
                state.has_active_level = true;
            }
            LevelCommand::Clear => {
                for entity in level_entities.iter() {
                    commands.entity(entity).despawn();
                }
                state.has_active_level = false;
                state.entity_count = 0;
            }
        }
    }
}

// ============================================================================
// Plugin
// ============================================================================

pub struct LevelPresetsPlugin;

impl Plugin for LevelPresetsPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] LevelPresetsPlugin");
        app.init_resource::<LevelPresetsState>();

        let bridge = LevelPresetsBridge::default();
        let arc = bridge.pending.clone();
        app.insert_resource(bridge);

        use renzora_editor::SplashState;
        app.add_systems(
            Update,
            (sync_bridge, process_level_commands).run_if(in_state(SplashState::Editor)),
        );

        app.register_panel(LevelPresetsPanel::new(arc));
    }
}

// ============================================================================
// Spawn helpers
// ============================================================================

/// Spawn a mesh entity using a unit mesh + Transform::scale so that the
/// `MeshPrimitive` shape ID rehydrates correctly on scene reload.
///
/// `size` maps directly to `Transform::scale`:
///   - cube  (unit 1×1×1):  size = (width, height, depth)
///   - cylinder (unit ⌀1×h1): size = (diameter, height, diameter)
///   - sphere (unit ⌀1):     size = (diameter, diameter, diameter)
fn spawn_level_mesh(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    name: &str,
    shape_id: &str,
    color: Color,
    pos: Vec3,
    size: Vec3,
    rotation: Quat,
) {
    let handle = match shape_id {
        "cube" => meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        "sphere" => meshes.add(Sphere::new(0.5)),
        "cylinder" => meshes.add(Cylinder::new(0.5, 1.0)),
        _ => return,
    };
    let mat = materials.add(StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.9,
        ..default()
    });
    commands.spawn((
        LevelPresetEntity,
        Name::new(name.to_string()),
        Mesh3d(handle),
        MeshMaterial3d(mat),
        MeshPrimitive(shape_id.to_string()),
        MeshColor(color),
        Transform {
            translation: pos,
            rotation,
            scale: size,
        },
        Visibility::default(),
    ));
}

/// Shorthand for spawning axis-aligned cubes (no rotation).
fn cube(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    name: &str,
    color: Color,
    pos: Vec3,
    size: Vec3,
) {
    spawn_level_mesh(commands, meshes, materials, name, "cube", color, pos, size, Quat::IDENTITY);
}

/// Shorthand for spawning a rotated cube.
fn cube_rot(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    name: &str,
    color: Color,
    pos: Vec3,
    size: Vec3,
    rotation: Quat,
) {
    spawn_level_mesh(commands, meshes, materials, name, "cube", color, pos, size, rotation);
}

/// Shorthand for spawning a cylinder. `diameter` and `height` map to scale.
fn cyl(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    name: &str,
    color: Color,
    pos: Vec3,
    diameter: f32,
    height: f32,
) {
    spawn_level_mesh(
        commands, meshes, materials, name, "cylinder", color,
        pos, Vec3::new(diameter, height, diameter), Quat::IDENTITY,
    );
}

/// Shorthand for spawning a sphere. `diameter` maps to uniform scale.
fn sphere(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    name: &str,
    color: Color,
    pos: Vec3,
    diameter: f32,
) {
    spawn_level_mesh(
        commands, meshes, materials, name, "sphere", color,
        pos, Vec3::splat(diameter), Quat::IDENTITY,
    );
}

/// Spawn a sun/directional light.
fn spawn_level_sun(commands: &mut Commands, azimuth: f32, elevation: f32, illuminance: f32) {
    let sun = Sun {
        azimuth,
        elevation,
        ..default()
    };
    let direction = sun.direction();
    commands.spawn((
        LevelPresetEntity,
        Name::new("Sun"),
        Transform::from_rotation(Quat::from_rotation_arc(Vec3::NEG_Z, direction)),
        DirectionalLight {
            color: Color::srgb(1.0, 0.98, 0.95),
            illuminance,
            shadows_enabled: true,
            ..default()
        },
        sun,
    ));
}

/// Spawn a scene camera (serializable via SceneCamera marker).
fn spawn_level_camera(commands: &mut Commands, transform: Transform) {
    commands.spawn((
        LevelPresetEntity,
        Name::new("Camera"),
        SceneCamera,
        Camera3d::default(),
        Camera {
            is_active: false,
            ..default()
        },
        transform,
    ));
}

/// Spawn a point light.
fn spawn_level_point_light(commands: &mut Commands, pos: Vec3, color: Color, intensity: f32) {
    commands.spawn((
        LevelPresetEntity,
        Name::new("Point Light"),
        Transform::from_translation(pos),
        PointLight {
            color,
            intensity,
            shadows_enabled: true,
            ..default()
        },
    ));
}

// ============================================================================
// Level dispatching
// ============================================================================

fn spawn_level(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    preset: LevelPreset,
    s: f32,
) {
    match preset {
        LevelPreset::FPS => spawn_fps(commands, meshes, materials, s),
        LevelPreset::ThirdPerson => spawn_third_person(commands, meshes, materials, s),
        LevelPreset::Platformer => spawn_platformer(commands, meshes, materials, s),
        LevelPreset::TopDown => spawn_top_down(commands, meshes, materials, s),
        LevelPreset::Racing => spawn_racing(commands, meshes, materials, s),
        LevelPreset::Sandbox => spawn_sandbox(commands, meshes, materials, s),
        LevelPreset::Corridor => spawn_corridor(commands, meshes, materials, s),
        LevelPreset::Arena => spawn_arena(commands, meshes, materials, s),
        LevelPreset::Showcase => spawn_showcase(commands, meshes, materials, s),
        LevelPreset::Terrain => spawn_terrain(commands, meshes, materials, s),
    }
}

// ============================================================================
// Colors
// ============================================================================

const FLOOR_COLOR: Color = Color::srgb(0.35, 0.35, 0.38);
const WALL_COLOR: Color = Color::srgb(0.45, 0.45, 0.48);
const COVER_COLOR: Color = Color::srgb(0.55, 0.40, 0.30);
const RAMP_COLOR: Color = Color::srgb(0.40, 0.42, 0.38);
const ACCENT_COLOR: Color = Color::srgb(0.30, 0.50, 0.60);
const PILLAR_COLOR: Color = Color::srgb(0.50, 0.48, 0.45);
const PLATFORM_COLOR: Color = Color::srgb(0.42, 0.42, 0.45);

// ============================================================================
// FPS — enclosed room with cover, ramp, balcony
// ============================================================================

fn spawn_fps(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    s: f32,
) {
    let room = 20.0 * s;
    let wall_h = 6.0 * s;
    let wall_t = 0.3 * s;

    // Floor
    cube(commands, meshes, materials, "Floor", FLOOR_COLOR,
        Vec3::new(0.0, -wall_t * 0.5, 0.0),
        Vec3::new(room * 2.0, wall_t, room * 2.0));

    // 4 walls
    for (pos, size) in [
        (Vec3::new(0.0, wall_h * 0.5, room), Vec3::new(room * 2.0, wall_h, wall_t)),
        (Vec3::new(0.0, wall_h * 0.5, -room), Vec3::new(room * 2.0, wall_h, wall_t)),
        (Vec3::new(room, wall_h * 0.5, 0.0), Vec3::new(wall_t, wall_h, room * 2.0)),
        (Vec3::new(-room, wall_h * 0.5, 0.0), Vec3::new(wall_t, wall_h, room * 2.0)),
    ] {
        cube(commands, meshes, materials, "Wall", WALL_COLOR, pos, size);
    }

    // Balcony platform (second floor, one side)
    let bal_w = room * 0.8;
    let bal_d = 6.0 * s;
    let bal_y = 3.0 * s;
    cube(commands, meshes, materials, "Balcony", PLATFORM_COLOR,
        Vec3::new(0.0, bal_y, room - bal_d * 0.5),
        Vec3::new(bal_w, wall_t, bal_d));

    // Ramp up to balcony
    let ramp_len = 8.0 * s;
    let ramp_w = 3.0 * s;
    let angle = (bal_y / ramp_len).asin();
    cube_rot(commands, meshes, materials, "Ramp", RAMP_COLOR,
        Vec3::new(-room * 0.6, bal_y * 0.5, room - bal_d - ramp_len * 0.4),
        Vec3::new(ramp_w, wall_t, ramp_len),
        Quat::from_rotation_x(-angle));

    // Cover boxes scattered around
    for (i, (x, z)) in [
        (-5.0, -4.0), (3.0, -8.0), (-8.0, 4.0), (6.0, 6.0),
        (0.0, 2.0), (-3.0, -10.0), (9.0, -2.0),
    ]
    .iter()
    .enumerate()
    {
        let h = if i % 2 == 0 { 1.5 * s } else { 2.5 * s };
        let w = if i % 3 == 0 { 2.0 * s } else { 1.5 * s };
        cube(commands, meshes, materials, "Cover", COVER_COLOR,
            Vec3::new(*x * s, h * 0.5, *z * s), Vec3::new(w, h, w));
    }

    // Pillar near center (cylinder: diameter = 1.2*s, height = wall_h)
    cyl(commands, meshes, materials, "Pillar", PILLAR_COLOR,
        Vec3::new(4.0 * s, wall_h * 0.5, -2.0 * s), 1.2 * s, wall_h);

    spawn_level_sun(commands, 135.0, 45.0, 10000.0);
    spawn_level_camera(
        commands,
        Transform::from_xyz(-room * 0.7, 2.0 * s, -room * 0.7).looking_at(Vec3::ZERO, Vec3::Y),
    );
}

// ============================================================================
// Third Person — open courtyard
// ============================================================================

fn spawn_third_person(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    s: f32,
) {
    let size = 30.0 * s;

    // Ground
    cube(commands, meshes, materials, "Ground", FLOOR_COLOR,
        Vec3::new(0.0, -0.15 * s, 0.0), Vec3::new(size * 2.0, 0.3 * s, size * 2.0));

    // Perimeter low walls
    let wh = 2.0 * s;
    let wt = 0.4 * s;
    for (pos, sz) in [
        (Vec3::new(0.0, wh * 0.5, size), Vec3::new(size * 2.0, wh, wt)),
        (Vec3::new(0.0, wh * 0.5, -size), Vec3::new(size * 2.0, wh, wt)),
        (Vec3::new(size, wh * 0.5, 0.0), Vec3::new(wt, wh, size * 2.0)),
        (Vec3::new(-size, wh * 0.5, 0.0), Vec3::new(wt, wh, size * 2.0)),
    ] {
        cube(commands, meshes, materials, "Wall", WALL_COLOR, pos, sz);
    }

    // Pillars in courtyard (cylinder: diameter = 1.0*s, height = 4.0*s)
    for i in 0..6 {
        let angle = i as f32 * std::f32::consts::TAU / 6.0;
        let r = 12.0 * s;
        let x = angle.cos() * r;
        let z = angle.sin() * r;
        cyl(commands, meshes, materials, "Pillar", PILLAR_COLOR,
            Vec3::new(x, 2.0 * s, z), 1.0 * s, 4.0 * s);
    }

    // Stairs (3 steps)
    for step in 0..3u32 {
        let y = step as f32 * 0.5 * s;
        let d = 3.0 * s - step as f32 * 0.3 * s;
        cube(commands, meshes, materials, "Step", RAMP_COLOR,
            Vec3::new(0.0, y + 0.25 * s, -size * 0.6),
            Vec3::new(4.0 * s, 0.5 * s, d));
    }

    // Elevated walkway
    let walkway_y = 2.5 * s;
    cube(commands, meshes, materials, "Walkway", PLATFORM_COLOR,
        Vec3::new(0.0, walkway_y, size * 0.7),
        Vec3::new(size * 1.5, 0.3 * s, 3.0 * s));

    // Walkway supports
    for x_off in [-size * 0.5, 0.0, size * 0.5] {
        cube(commands, meshes, materials, "Support", PILLAR_COLOR,
            Vec3::new(x_off, walkway_y * 0.5, size * 0.7),
            Vec3::new(0.5 * s, walkway_y, 0.5 * s));
    }

    spawn_level_sun(commands, 200.0, 50.0, 12000.0);
    spawn_level_camera(
        commands,
        Transform::from_xyz(-15.0 * s, 10.0 * s, -15.0 * s).looking_at(Vec3::ZERO, Vec3::Y),
    );
}

// ============================================================================
// Platformer — floating platforms
// ============================================================================

fn spawn_platformer(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    s: f32,
) {
    // Starting platform
    cube(commands, meshes, materials, "Start Platform", FLOOR_COLOR,
        Vec3::new(0.0, 0.0, 0.0), Vec3::new(6.0 * s, 0.5 * s, 6.0 * s));

    // Series of floating platforms
    let platforms: [(f32, f32, f32, f32, f32); 8] = [
        (5.0, 1.5, 3.0, 3.0, 3.0),
        (10.0, 3.0, -2.0, 2.5, 2.5),
        (14.0, 5.0, 1.0, 3.0, 2.0),
        (19.0, 4.0, -3.0, 2.0, 2.0),
        (23.0, 6.5, 0.0, 3.5, 3.5),
        (28.0, 8.0, 2.0, 2.0, 2.0),
        (32.0, 7.0, -1.0, 2.5, 2.5),
        (37.0, 9.0, 0.0, 5.0, 5.0),
    ];

    for (i, (x, y, z, w, d)) in platforms.iter().enumerate() {
        let name = if i == platforms.len() - 1 { "End Platform" } else { "Platform" };
        let color = if i == platforms.len() - 1 { ACCENT_COLOR } else { PLATFORM_COLOR };
        cube(commands, meshes, materials, name, color,
            Vec3::new(*x * s, *y * s, *z * s),
            Vec3::new(*w * s, 0.5 * s, *d * s));
    }

    // Thin pillar obstacles (cylinder: diameter = 0.6*s, height = 2.0*s)
    for (x, y, z) in [(12.0, 4.0, 0.0), (21.0, 5.0, -1.5), (30.0, 7.5, 1.0)] {
        cyl(commands, meshes, materials, "Obstacle", COVER_COLOR,
            Vec3::new(x * s, y * s, z * s), 0.6 * s, 2.0 * s);
    }

    spawn_level_sun(commands, 90.0, 60.0, 14000.0);
    spawn_level_camera(
        commands,
        Transform::from_xyz(-5.0 * s, 8.0 * s, 12.0 * s)
            .looking_at(Vec3::new(18.0 * s, 4.0 * s, 0.0), Vec3::Y),
    );
}

// ============================================================================
// Top Down — grid rooms with low walls
// ============================================================================

fn spawn_top_down(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    s: f32,
) {
    let grid = 24.0 * s;

    // Floor
    cube(commands, meshes, materials, "Floor", FLOOR_COLOR,
        Vec3::new(0.0, -0.15 * s, 0.0), Vec3::new(grid * 2.0, 0.3 * s, grid * 2.0));

    let wall_h = 1.5 * s;
    let wall_t = 0.3 * s;

    // Outer walls
    for (pos, sz) in [
        (Vec3::new(0.0, wall_h * 0.5, grid), Vec3::new(grid * 2.0, wall_h, wall_t)),
        (Vec3::new(0.0, wall_h * 0.5, -grid), Vec3::new(grid * 2.0, wall_h, wall_t)),
        (Vec3::new(grid, wall_h * 0.5, 0.0), Vec3::new(wall_t, wall_h, grid * 2.0)),
        (Vec3::new(-grid, wall_h * 0.5, 0.0), Vec3::new(wall_t, wall_h, grid * 2.0)),
    ] {
        cube(commands, meshes, materials, "Outer Wall", WALL_COLOR, pos, sz);
    }

    // Internal room dividers (grid pattern with gaps for doorways)
    let cell = 8.0 * s;
    let segment = 5.0 * s;
    // Horizontal walls
    for row in [-1, 0, 1] {
        let z = row as f32 * cell;
        for col in [-1, 1] {
            let x = col as f32 * cell * 0.7;
            cube(commands, meshes, materials, "Divider", WALL_COLOR,
                Vec3::new(x, wall_h * 0.5, z), Vec3::new(segment, wall_h, wall_t));
        }
    }
    // Vertical walls
    for col in [-1, 0, 1] {
        let x = col as f32 * cell;
        for row in [-1, 1] {
            let z = row as f32 * cell * 0.7;
            cube(commands, meshes, materials, "Divider", WALL_COLOR,
                Vec3::new(x, wall_h * 0.5, z), Vec3::new(wall_t, wall_h, segment));
        }
    }

    spawn_level_sun(commands, 180.0, 70.0, 10000.0);
    spawn_level_camera(
        commands,
        Transform::from_xyz(0.0, 30.0 * s, 0.1).looking_at(Vec3::ZERO, Vec3::Y),
    );
}

// ============================================================================
// Racing — oval track
// ============================================================================

fn spawn_racing(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    s: f32,
) {
    let track_w = 8.0 * s;
    let straight = 30.0 * s;
    let radius = 15.0 * s;
    let barrier_h = 1.0 * s;
    let barrier_t = 0.4 * s;

    // Ground plane (large)
    cube(commands, meshes, materials, "Ground", Color::srgb(0.28, 0.35, 0.25),
        Vec3::new(0.0, -0.1 * s, 0.0), Vec3::new(100.0 * s, 0.2 * s, 80.0 * s));

    // Track surface (two straights)
    cube(commands, meshes, materials, "Track", FLOOR_COLOR,
        Vec3::new(0.0, 0.0, -radius), Vec3::new(straight, 0.15 * s, track_w));
    cube(commands, meshes, materials, "Track", FLOOR_COLOR,
        Vec3::new(0.0, 0.0, radius), Vec3::new(straight, 0.15 * s, track_w));

    // Curved ends (approximated with angled segments)
    let segments = 8u32;
    for end in [-1.0_f32, 1.0] {
        let cx = end * straight * 0.5;
        for i in 0..segments {
            let a0 = std::f32::consts::PI * (0.5 - end * 0.5)
                + i as f32 * std::f32::consts::PI / segments as f32;
            let a1 = a0 + std::f32::consts::PI / segments as f32;
            let mid_a = (a0 + a1) * 0.5;
            let seg_len = 2.0 * radius * (std::f32::consts::PI / segments as f32 / 2.0).sin();
            let mx = cx + mid_a.cos() * radius;
            let mz = mid_a.sin() * radius;
            cube_rot(commands, meshes, materials, "Curve", FLOOR_COLOR,
                Vec3::new(mx, 0.0, mz),
                Vec3::new(seg_len + 0.5 * s, 0.15 * s, track_w),
                Quat::from_rotation_y(-mid_a + std::f32::consts::FRAC_PI_2));
        }
    }

    // Inner + outer barriers along straights
    for z_sign in [-1.0_f32, 1.0] {
        let z_inner = z_sign * (radius - track_w * 0.5);
        let z_outer = z_sign * (radius + track_w * 0.5);
        cube(commands, meshes, materials, "Barrier", ACCENT_COLOR,
            Vec3::new(0.0, barrier_h * 0.5, z_inner),
            Vec3::new(straight, barrier_h, barrier_t));
        cube(commands, meshes, materials, "Barrier", ACCENT_COLOR,
            Vec3::new(0.0, barrier_h * 0.5, z_outer),
            Vec3::new(straight, barrier_h, barrier_t));
    }

    // Start line marker
    cube(commands, meshes, materials, "Start Line", Color::WHITE,
        Vec3::new(-straight * 0.3, 0.08 * s, -radius),
        Vec3::new(0.3 * s, 0.05 * s, track_w));

    spawn_level_sun(commands, 160.0, 55.0, 14000.0);
    spawn_level_camera(
        commands,
        Transform::from_xyz(0.0, 40.0 * s, 0.0).looking_at(Vec3::ZERO, Vec3::Z),
    );
}

// ============================================================================
// Sandbox — blank canvas
// ============================================================================

fn spawn_sandbox(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    s: f32,
) {
    cube(commands, meshes, materials, "Ground", FLOOR_COLOR,
        Vec3::new(0.0, -0.15 * s, 0.0), Vec3::new(100.0 * s, 0.3 * s, 100.0 * s));

    spawn_level_sun(commands, 150.0, 45.0, 10000.0);
    spawn_level_camera(
        commands,
        Transform::from_xyz(10.0 * s, 8.0 * s, 10.0 * s).looking_at(Vec3::ZERO, Vec3::Y),
    );
}

// ============================================================================
// Corridor — L-shaped hallway
// ============================================================================

fn spawn_corridor(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    s: f32,
) {
    let hall_w = 5.0 * s;
    let wall_h = 4.0 * s;
    let wall_t = 0.3 * s;
    let leg1 = 25.0 * s;
    let leg2 = 20.0 * s;

    // Floors
    cube(commands, meshes, materials, "Floor", FLOOR_COLOR,
        Vec3::new(-leg1 * 0.5, -wall_t * 0.5, 0.0), Vec3::new(leg1, wall_t, hall_w));
    cube(commands, meshes, materials, "Floor", FLOOR_COLOR,
        Vec3::new(0.0, -wall_t * 0.5, 0.0), Vec3::new(hall_w, wall_t, hall_w));
    cube(commands, meshes, materials, "Floor", FLOOR_COLOR,
        Vec3::new(0.0, -wall_t * 0.5, leg2 * 0.5), Vec3::new(hall_w, wall_t, leg2));

    // Walls for first leg
    for z_sign in [-1.0_f32, 1.0] {
        cube(commands, meshes, materials, "Wall", WALL_COLOR,
            Vec3::new(-leg1 * 0.5, wall_h * 0.5, z_sign * hall_w * 0.5),
            Vec3::new(leg1, wall_h, wall_t));
    }

    // Walls for second leg
    for x_sign in [-1.0_f32, 1.0] {
        cube(commands, meshes, materials, "Wall", WALL_COLOR,
            Vec3::new(x_sign * hall_w * 0.5, wall_h * 0.5, leg2 * 0.5),
            Vec3::new(wall_t, wall_h, leg2));
    }

    // End walls
    cube(commands, meshes, materials, "End Wall", WALL_COLOR,
        Vec3::new(-leg1, wall_h * 0.5, 0.0), Vec3::new(hall_w + wall_t, wall_h, wall_t));
    cube(commands, meshes, materials, "End Wall", WALL_COLOR,
        Vec3::new(0.0, wall_h * 0.5, leg2), Vec3::new(hall_w + wall_t, wall_h, wall_t));

    // Corner connector wall
    cube(commands, meshes, materials, "Corner Wall", WALL_COLOR,
        Vec3::new(-hall_w * 0.5, wall_h * 0.5, hall_w * 0.5),
        Vec3::new(wall_t, wall_h, hall_w));

    // Crate obstacles along corridors
    for i in 0..4u32 {
        let x = -leg1 + (i as f32 + 1.0) * leg1 * 0.2;
        let side = if i % 2 == 0 { 1.5 } else { -1.5 };
        cube(commands, meshes, materials, "Crate", COVER_COLOR,
            Vec3::new(x, 0.6 * s, side * s),
            Vec3::new(1.2 * s, 1.2 * s, 1.2 * s));
    }
    for i in 0..3u32 {
        let z = 3.0 * s + (i as f32 + 1.0) * leg2 * 0.2;
        let side = if i % 2 == 0 { 1.5 } else { -1.5 };
        cube(commands, meshes, materials, "Crate", COVER_COLOR,
            Vec3::new(side * s, 0.6 * s, z),
            Vec3::new(1.2 * s, 1.2 * s, 1.2 * s));
    }

    // Ceiling lights
    for x in [-leg1 * 0.7, -leg1 * 0.3] {
        spawn_level_point_light(commands, Vec3::new(x, wall_h * 0.8, 0.0), Color::srgb(1.0, 0.95, 0.85), 5000.0);
    }
    for z in [leg2 * 0.25, leg2 * 0.65] {
        spawn_level_point_light(commands, Vec3::new(0.0, wall_h * 0.8, z), Color::srgb(1.0, 0.95, 0.85), 5000.0);
    }

    spawn_level_sun(commands, 90.0, 30.0, 5000.0);
    spawn_level_camera(
        commands,
        Transform::from_xyz(-leg1 * 0.8, 2.0 * s, 0.0)
            .looking_at(Vec3::new(0.0, 1.5 * s, 0.0), Vec3::Y),
    );
}

// ============================================================================
// Arena — circular walled arena with central pillar
// ============================================================================

fn spawn_arena(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    s: f32,
) {
    let radius = 18.0 * s;
    let wall_h = 3.5 * s;

    // Floor
    cube(commands, meshes, materials, "Floor", FLOOR_COLOR,
        Vec3::new(0.0, -0.15 * s, 0.0), Vec3::new(radius * 2.2, 0.3 * s, radius * 2.2));

    // Circular wall segments
    let segments = 16u32;
    let seg_angle = std::f32::consts::TAU / segments as f32;
    let seg_len = 2.0 * radius * (seg_angle / 2.0).sin() + 0.5 * s;
    for i in 0..segments {
        let angle = i as f32 * seg_angle;
        let x = angle.cos() * radius;
        let z = angle.sin() * radius;
        cube_rot(commands, meshes, materials, "Arena Wall", WALL_COLOR,
            Vec3::new(x, wall_h * 0.5, z),
            Vec3::new(seg_len, wall_h, 0.4 * s),
            Quat::from_rotation_y(-angle + std::f32::consts::FRAC_PI_2));
    }

    // Central pillar (cylinder: diameter = 2.4*s, height = wall_h*1.2)
    cyl(commands, meshes, materials, "Central Pillar", PILLAR_COLOR,
        Vec3::new(0.0, wall_h * 0.6, 0.0), 2.4 * s, wall_h * 1.2);

    // Symmetrical cover (4 boxes at 90-degree intervals)
    let cover_r = radius * 0.5;
    for i in 0..4u32 {
        let angle = i as f32 * std::f32::consts::FRAC_PI_2 + std::f32::consts::FRAC_PI_4;
        let x = angle.cos() * cover_r;
        let z = angle.sin() * cover_r;
        cube_rot(commands, meshes, materials, "Cover", COVER_COLOR,
            Vec3::new(x, 0.9 * s, z),
            Vec3::new(2.5 * s, 1.8 * s, 2.5 * s),
            Quat::from_rotation_y(angle));
    }

    spawn_level_sun(commands, 120.0, 50.0, 12000.0);
    spawn_level_camera(
        commands,
        Transform::from_xyz(radius * 0.8, 12.0 * s, radius * 0.8).looking_at(Vec3::ZERO, Vec3::Y),
    );
}

// ============================================================================
// Showcase — display pedestal with lighting
// ============================================================================

fn spawn_showcase(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    s: f32,
) {
    // Base platform (cylinder: diameter = 16.0*s, height = 0.5*s)
    cyl(commands, meshes, materials, "Pedestal", Color::srgb(0.25, 0.25, 0.28),
        Vec3::new(0.0, 0.25 * s, 0.0), 16.0 * s, 0.5 * s);

    // Inner raised platform (cylinder: diameter = 6.0*s, height = 0.3*s)
    cyl(commands, meshes, materials, "Stage", Color::srgb(0.18, 0.18, 0.22),
        Vec3::new(0.0, 0.65 * s, 0.0), 6.0 * s, 0.3 * s);

    // Sample object on stage
    cube_rot(commands, meshes, materials, "Display Object", ACCENT_COLOR,
        Vec3::new(0.0, 1.55 * s, 0.0),
        Vec3::new(1.5 * s, 1.5 * s, 1.5 * s),
        Quat::from_rotation_y(0.4));

    // Backdrop wall
    cube(commands, meshes, materials, "Backdrop", Color::srgb(0.15, 0.15, 0.18),
        Vec3::new(0.0, 4.0 * s, -6.0 * s),
        Vec3::new(20.0 * s, 8.0 * s, 0.3 * s));

    // Rim lights (3-point lighting)
    spawn_level_point_light(commands,
        Vec3::new(-5.0 * s, 5.0 * s, 4.0 * s), Color::srgb(1.0, 0.95, 0.9), 15000.0);
    spawn_level_point_light(commands,
        Vec3::new(5.0 * s, 5.0 * s, 4.0 * s), Color::srgb(0.9, 0.95, 1.0), 12000.0);
    spawn_level_point_light(commands,
        Vec3::new(0.0, 6.0 * s, -4.0 * s), Color::srgb(0.8, 0.85, 1.0), 8000.0);

    spawn_level_sun(commands, 180.0, 35.0, 4000.0);
    spawn_level_camera(
        commands,
        Transform::from_xyz(0.0, 3.0 * s, 10.0 * s)
            .looking_at(Vec3::new(0.0, 1.5 * s, 0.0), Vec3::Y),
    );
}

// ============================================================================
// Terrain — hilly landscape with stepped elevations
// ============================================================================

fn spawn_terrain(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    s: f32,
) {
    // Base ground
    cube(commands, meshes, materials, "Ground", Color::srgb(0.30, 0.38, 0.25),
        Vec3::new(0.0, -0.15 * s, 0.0), Vec3::new(60.0 * s, 0.3 * s, 60.0 * s));

    // Hills
    let hills: [(f32, f32, f32, f32, f32); 6] = [
        (-12.0, -10.0, 10.0, 2.0, 8.0),
        (-8.0, 8.0, 8.0, 3.0, 6.0),
        (10.0, -6.0, 12.0, 1.5, 10.0),
        (15.0, 10.0, 6.0, 4.0, 6.0),
        (0.0, -18.0, 14.0, 2.5, 8.0),
        (-20.0, 0.0, 8.0, 1.0, 12.0),
    ];

    for (i, (x, z, w, h, d)) in hills.iter().enumerate() {
        // Base layer
        cube(commands, meshes, materials, "Hill", Color::srgb(0.35, 0.42, 0.30),
            Vec3::new(*x * s, *h * s * 0.5, *z * s),
            Vec3::new(*w * s, *h * s, *d * s));

        // Smaller top layer for a more natural look
        if *h > 1.5 {
            let shrink = 0.6;
            cube(commands, meshes, materials, "Hill Top", Color::srgb(0.38, 0.45, 0.32),
                Vec3::new(*x * s, *h * s + *h * 0.2 * s, *z * s),
                Vec3::new(*w * shrink * s, *h * 0.4 * s, *d * shrink * s));
        }

        // Trees on some hills
        if i % 2 == 0 {
            for t in 0..3u32 {
                let tx = *x + (t as f32 - 1.0) * 2.0;
                let tz = *z + (t as f32 - 1.0) * 1.5;
                // Trunk (cylinder: diameter = 0.3*s, height = 2.0*s)
                cyl(commands, meshes, materials, "Trunk", Color::srgb(0.45, 0.30, 0.20),
                    Vec3::new(tx * s, *h * s + 1.0 * s, tz * s), 0.3 * s, 2.0 * s);
                // Canopy (sphere: diameter = 2.0*s)
                sphere(commands, meshes, materials, "Canopy", Color::srgb(0.20, 0.45, 0.20),
                    Vec3::new(tx * s, *h * s + 2.5 * s, tz * s), 2.0 * s);
            }
        }
    }

    // Valley
    cube(commands, meshes, materials, "Valley Floor", Color::srgb(0.25, 0.32, 0.22),
        Vec3::new(2.0 * s, -0.2 * s, 0.0), Vec3::new(8.0 * s, 0.1 * s, 12.0 * s));

    spawn_level_sun(commands, 170.0, 40.0, 12000.0);
    spawn_level_camera(
        commands,
        Transform::from_xyz(25.0 * s, 15.0 * s, 25.0 * s).looking_at(Vec3::ZERO, Vec3::Y),
    );
}
