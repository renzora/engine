//! Debug state resources and update systems

use bevy::prelude::*;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin, SystemInformationDiagnosticsPlugin};
use std::collections::{VecDeque, HashMap};

const MAX_SAMPLES: usize = 120;

// ============================================================================
// Diagnostics State
// ============================================================================

#[derive(Resource, Clone)]
pub struct DiagnosticsState {
    pub fps: f64,
    pub frame_time_ms: f64,
    pub fps_history: VecDeque<f32>,
    pub frame_time_history: VecDeque<f32>,
    pub entity_count: usize,
    pub entity_count_history: VecDeque<f32>,
    pub cpu_usage: Option<f64>,
    pub memory_usage: Option<u64>,
    pub enabled: bool,
    pub update_interval: f32,
    pub time_since_update: f32,
}

impl Default for DiagnosticsState {
    fn default() -> Self {
        Self {
            fps: 0.0,
            frame_time_ms: 0.0,
            fps_history: VecDeque::with_capacity(MAX_SAMPLES),
            frame_time_history: VecDeque::with_capacity(MAX_SAMPLES),
            entity_count: 0,
            entity_count_history: VecDeque::with_capacity(MAX_SAMPLES),
            cpu_usage: None,
            memory_usage: None,
            enabled: true,
            update_interval: 0.1,
            time_since_update: 0.0,
        }
    }
}

impl DiagnosticsState {
    pub fn avg_fps(&self) -> f32 {
        if self.fps_history.is_empty() { return 0.0; }
        self.fps_history.iter().sum::<f32>() / self.fps_history.len() as f32
    }

    pub fn min_fps(&self) -> f32 {
        self.fps_history.iter().copied().fold(f32::MAX, f32::min)
    }

    pub fn max_fps(&self) -> f32 {
        self.fps_history.iter().copied().fold(0.0, f32::max)
    }

    pub fn avg_frame_time(&self) -> f32 {
        if self.frame_time_history.is_empty() { return 0.0; }
        self.frame_time_history.iter().sum::<f32>() / self.frame_time_history.len() as f32
    }

    pub fn one_percent_low_fps(&self) -> f32 {
        if self.fps_history.is_empty() { return 0.0; }
        let mut sorted: Vec<f32> = self.fps_history.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let count = (sorted.len() as f32 * 0.01).max(1.0) as usize;
        sorted.iter().take(count).sum::<f32>() / count as f32
    }

    fn push_sample(history: &mut VecDeque<f32>, value: f32) {
        if history.len() >= MAX_SAMPLES {
            history.pop_front();
        }
        history.push_back(value);
    }
}

// ============================================================================
// Render Stats
// ============================================================================

#[derive(Resource, Clone)]
pub struct RenderStats {
    pub enabled: bool,
    pub gpu_time_ms: f32,
    pub gpu_time_history: Vec<f32>,
    pub draw_calls: u64,
    pub triangles: u64,
    pub vertices: u64,
    pub update_interval: f32,
    pub time_since_update: f32,
}

impl Default for RenderStats {
    fn default() -> Self {
        Self {
            enabled: false,
            gpu_time_ms: 0.0,
            gpu_time_history: Vec::with_capacity(MAX_SAMPLES),
            draw_calls: 0,
            triangles: 0,
            vertices: 0,
            update_interval: 0.1,
            time_since_update: 0.0,
        }
    }
}

impl RenderStats {
    fn push_gpu_time(&mut self, value: f32) {
        if self.gpu_time_history.len() >= MAX_SAMPLES {
            self.gpu_time_history.remove(0);
        }
        self.gpu_time_history.push(value);
    }
}

// ============================================================================
// Schedule Timing State
// ============================================================================

#[derive(Clone, Debug, Default)]
pub struct ScheduleTiming {
    pub name: String,
    pub time_ms: f32,
    pub percentage: f32,
}

#[derive(Resource, Clone)]
pub struct SystemTimingState {
    pub schedule_timings: Vec<ScheduleTiming>,
    pub update_interval: f32,
    pub time_since_update: f32,
    pub limitation_note: String,
}

impl Default for SystemTimingState {
    fn default() -> Self {
        Self {
            schedule_timings: Vec::new(),
            update_interval: 0.5,
            time_since_update: 0.0,
            limitation_note: "Per-system timing requires Tracy integration. \
                Run with --features tracy for detailed profiling.".to_string(),
        }
    }
}

// ============================================================================
// Memory Profiler State
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MemoryTrend {
    #[default]
    Stable,
    Increasing,
    Decreasing,
}

#[derive(Clone, Debug, Default)]
pub struct AssetMemoryStats {
    pub meshes_bytes: u64,
    pub mesh_count: usize,
    pub textures_bytes: u64,
    pub texture_count: usize,
    pub materials_bytes: u64,
    pub material_count: usize,
}

#[derive(Resource, Clone)]
pub struct MemoryProfilerState {
    pub process_memory: u64,
    pub peak_memory: u64,
    pub memory_history: VecDeque<f32>,
    pub memory_trend: MemoryTrend,
    pub asset_memory: AssetMemoryStats,
    pub allocation_rate: f64,
    _prev_memory: u64,
    pub update_interval: f32,
    pub time_since_update: f32,
    pub available: bool,
}

impl Default for MemoryProfilerState {
    fn default() -> Self {
        Self {
            process_memory: 0,
            peak_memory: 0,
            memory_history: VecDeque::with_capacity(MAX_SAMPLES),
            memory_trend: MemoryTrend::Stable,
            asset_memory: AssetMemoryStats::default(),
            allocation_rate: 0.0,
            _prev_memory: 0,
            update_interval: 0.5,
            time_since_update: 0.0,
            available: true,
        }
    }
}

impl MemoryProfilerState {
    fn push_sample(&mut self, value: f32) {
        if self.memory_history.len() >= MAX_SAMPLES {
            self.memory_history.pop_front();
        }
        self.memory_history.push_back(value);
    }

    fn calculate_trend(&mut self) {
        if self.memory_history.len() < 10 {
            self.memory_trend = MemoryTrend::Stable;
            return;
        }

        let recent: Vec<f32> = self.memory_history.iter().rev().take(10).copied().collect();
        let older: Vec<f32> = self.memory_history.iter().rev().skip(10).take(10).copied().collect();

        if older.is_empty() {
            self.memory_trend = MemoryTrend::Stable;
            return;
        }

        let recent_avg: f32 = recent.iter().sum::<f32>() / recent.len() as f32;
        let older_avg: f32 = older.iter().sum::<f32>() / older.len() as f32;
        let threshold = older_avg * 0.05;

        if recent_avg > older_avg + threshold {
            self.memory_trend = MemoryTrend::Increasing;
        } else if recent_avg < older_avg - threshold {
            self.memory_trend = MemoryTrend::Decreasing;
        } else {
            self.memory_trend = MemoryTrend::Stable;
        }
    }
}

// ============================================================================
// Camera Debug State
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CameraProjectionType {
    #[default]
    Perspective,
    Orthographic,
}

impl std::fmt::Display for CameraProjectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CameraProjectionType::Perspective => write!(f, "Perspective"),
            CameraProjectionType::Orthographic => write!(f, "Orthographic"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct CameraInfo {
    pub entity: Entity,
    pub name: String,
    pub is_active: bool,
    pub order: isize,
    pub projection_type: CameraProjectionType,
    pub fov_degrees: Option<f32>,
    pub near: f32,
    pub far: f32,
    pub aspect_ratio: f32,
    pub ortho_scale: Option<f32>,
    pub position: Vec3,
    pub rotation_degrees: Vec3,
    pub forward: Vec3,
    pub clear_color: Option<Color>,
    pub viewport: Option<[f32; 4]>,
}

impl Default for CameraInfo {
    fn default() -> Self {
        Self {
            entity: Entity::PLACEHOLDER,
            name: "Camera".to_string(),
            is_active: true,
            order: 0,
            projection_type: CameraProjectionType::Perspective,
            fov_degrees: Some(45.0),
            near: 0.1,
            far: 1000.0,
            aspect_ratio: 16.0 / 9.0,
            ortho_scale: None,
            position: Vec3::ZERO,
            rotation_degrees: Vec3::ZERO,
            forward: -Vec3::Z,
            clear_color: None,
            viewport: None,
        }
    }
}

#[derive(Resource, Clone)]
pub struct CameraDebugState {
    pub cameras: Vec<CameraInfo>,
    pub selected_camera: Option<Entity>,
    pub show_frustum_gizmos: bool,
    pub show_camera_axes: bool,
    pub show_all_frustums: bool,
    pub frustum_color: Color,
    pub update_interval: f32,
    pub time_since_update: f32,
}

impl Default for CameraDebugState {
    fn default() -> Self {
        Self {
            cameras: Vec::new(),
            selected_camera: None,
            show_frustum_gizmos: false,
            show_camera_axes: false,
            show_all_frustums: false,
            frustum_color: Color::srgba(1.0, 1.0, 0.0, 0.5),
            update_interval: 0.2,
            time_since_update: 0.0,
        }
    }
}

impl CameraDebugState {
    pub fn selected_camera_info(&self) -> Option<&CameraInfo> {
        self.selected_camera.and_then(|entity| {
            self.cameras.iter().find(|c| c.entity == entity)
        })
    }

    pub fn scene_camera_count(&self) -> usize {
        self.cameras.len()
    }
}

// ============================================================================
// Physics Debug State
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ColliderShapeType {
    Sphere,
    Box,
    Capsule,
    Cylinder,
    Cone,
    ConvexHull,
    TriMesh,
    HeightField,
    Compound,
    Unknown,
}

impl std::fmt::Display for ColliderShapeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ColliderShapeType::Sphere => write!(f, "Sphere"),
            ColliderShapeType::Box => write!(f, "Box"),
            ColliderShapeType::Capsule => write!(f, "Capsule"),
            ColliderShapeType::Cylinder => write!(f, "Cylinder"),
            ColliderShapeType::Cone => write!(f, "Cone"),
            ColliderShapeType::ConvexHull => write!(f, "Convex Hull"),
            ColliderShapeType::TriMesh => write!(f, "Trimesh"),
            ColliderShapeType::HeightField => write!(f, "Heightfield"),
            ColliderShapeType::Compound => write!(f, "Compound"),
            ColliderShapeType::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PhysicsDebugToggles {
    pub show_colliders: bool,
    pub show_contacts: bool,
    pub show_aabbs: bool,
    pub show_velocities: bool,
    pub show_center_of_mass: bool,
    pub show_joints: bool,
}

#[derive(Clone, Debug)]
pub struct CollisionPairInfo {
    pub entity_a: Entity,
    pub entity_b: Entity,
    pub contact_count: usize,
}

#[derive(Resource, Clone)]
pub struct PhysicsDebugState {
    pub simulation_running: bool,
    pub dynamic_body_count: usize,
    pub kinematic_body_count: usize,
    pub static_body_count: usize,
    pub collider_count: usize,
    pub colliders_by_type: HashMap<ColliderShapeType, usize>,
    pub collision_pair_count: usize,
    pub collision_pairs: Vec<CollisionPairInfo>,
    pub step_time_history: VecDeque<f32>,
    pub step_time_ms: f32,
    pub avg_step_time_ms: f32,
    pub debug_toggles: PhysicsDebugToggles,
    pub show_collision_pairs: bool,
    pub update_interval: f32,
    pub time_since_update: f32,
    pub physics_available: bool,
}

impl Default for PhysicsDebugState {
    fn default() -> Self {
        Self {
            simulation_running: false,
            dynamic_body_count: 0,
            kinematic_body_count: 0,
            static_body_count: 0,
            collider_count: 0,
            colliders_by_type: HashMap::new(),
            collision_pair_count: 0,
            collision_pairs: Vec::new(),
            step_time_history: VecDeque::with_capacity(MAX_SAMPLES),
            step_time_ms: 0.0,
            avg_step_time_ms: 0.0,
            debug_toggles: PhysicsDebugToggles::default(),
            show_collision_pairs: false,
            update_interval: 0.1,
            time_since_update: 0.0,
            physics_available: false,
        }
    }
}

impl PhysicsDebugState {
    pub fn push_step_time(&mut self, time_ms: f32) {
        if self.step_time_history.len() >= MAX_SAMPLES {
            self.step_time_history.pop_front();
        }
        self.step_time_history.push_back(time_ms);

        if !self.step_time_history.is_empty() {
            self.avg_step_time_ms = self.step_time_history.iter().sum::<f32>()
                / self.step_time_history.len() as f32;
        }
    }

    pub fn total_body_count(&self) -> usize {
        self.dynamic_body_count + self.kinematic_body_count + self.static_body_count
    }
}

// ============================================================================
// Culling Debug State
// ============================================================================

#[derive(Resource, Clone)]
pub struct CullingDebugState {
    pub enabled: bool,
    pub max_distance: f32,
    pub fade_start_fraction: f32,
    pub total_entities: u32,
    pub frustum_visible: u32,
    pub frustum_culled: u32,
    pub distance_culled: u32,
    pub distance_faded: u32,
    pub distance_buckets: [u32; 5],
    pub update_interval: f32,
    pub time_since_update: f32,
}

impl Default for CullingDebugState {
    fn default() -> Self {
        Self {
            enabled: false,
            max_distance: 500.0,
            fade_start_fraction: 0.8,
            total_entities: 0,
            frustum_visible: 0,
            frustum_culled: 0,
            distance_culled: 0,
            distance_faded: 0,
            distance_buckets: [0; 5],
            update_interval: 0.2,
            time_since_update: 0.0,
        }
    }
}

// ============================================================================
// Update Systems
// ============================================================================

pub fn update_diagnostics_state(
    diagnostics: Res<DiagnosticsStore>,
    mut state: ResMut<DiagnosticsState>,
    time: Res<Time>,
    entities: Query<Entity>,
) {
    if !state.enabled { return; }

    state.time_since_update += time.delta_secs();
    if state.time_since_update < state.update_interval { return; }
    state.time_since_update = 0.0;

    if let Some(fps) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS) {
        if let Some(value) = fps.smoothed() {
            state.fps = value;
            DiagnosticsState::push_sample(&mut state.fps_history, value as f32);
        }
    }

    if let Some(frame_time) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FRAME_TIME) {
        if let Some(value) = frame_time.smoothed() {
            state.frame_time_ms = value;
            DiagnosticsState::push_sample(&mut state.frame_time_history, value as f32);
        }
    }

    let entity_count = entities.iter().count();
    state.entity_count = entity_count;
    DiagnosticsState::push_sample(&mut state.entity_count_history, entity_count as f32);

    if let Some(cpu) = diagnostics.get(&SystemInformationDiagnosticsPlugin::SYSTEM_CPU_USAGE) {
        state.cpu_usage = cpu.smoothed();
    }

    if let Some(mem) = diagnostics.get(&SystemInformationDiagnosticsPlugin::SYSTEM_MEM_USAGE) {
        state.memory_usage = mem.smoothed().map(|v| v as u64);
    }
}

pub fn update_render_stats(
    diagnostics: Res<DiagnosticsStore>,
    mut stats: ResMut<RenderStats>,
    time: Res<Time>,
    mesh_query: Query<&Mesh3d>,
    meshes: Res<Assets<Mesh>>,
) {
    stats.time_since_update += time.delta_secs();
    if stats.time_since_update < stats.update_interval { return; }
    stats.time_since_update = 0.0;

    let mut total_vertices: u64 = 0;
    let mut total_triangles: u64 = 0;
    let mut draw_calls: u64 = 0;

    for mesh_handle in mesh_query.iter() {
        draw_calls += 1;
        if let Some(mesh) = meshes.get(&mesh_handle.0) {
            if let Some(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
                total_vertices += positions.len() as u64;
            }
            if let Some(indices) = mesh.indices() {
                total_triangles += (indices.len() / 3) as u64;
            } else if let Some(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
                total_triangles += (positions.len() / 3) as u64;
            }
        }
    }

    stats.draw_calls = draw_calls;
    stats.vertices = total_vertices;
    stats.triangles = total_triangles;

    let mut found_gpu_timing = false;
    for diagnostic in diagnostics.iter() {
        let path = diagnostic.path().as_str();
        if path.contains("gpu_time") || path.contains("elapsed") {
            if let Some(value) = diagnostic.smoothed() {
                let gpu_time = (value * 1000.0) as f32;
                stats.gpu_time_ms = gpu_time;
                stats.push_gpu_time(gpu_time);
                found_gpu_timing = true;
            }
        }
    }

    if !found_gpu_timing {
        if let Some(frame_time) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FRAME_TIME) {
            if let Some(value) = frame_time.smoothed() {
                let gpu_time = (value * 0.6) as f32;
                stats.gpu_time_ms = gpu_time;
                stats.push_gpu_time(gpu_time);
            }
        }
    }

    stats.enabled = true;
}

pub fn update_memory_profiler(
    mut state: ResMut<MemoryProfilerState>,
    time: Res<Time>,
    meshes: Res<Assets<Mesh>>,
    images: Res<Assets<Image>>,
    materials: Res<Assets<StandardMaterial>>,
) {
    state.time_since_update += time.delta_secs();
    if state.time_since_update < state.update_interval { return; }
    let dt = state.time_since_update;
    state.time_since_update = 0.0;

    if let Some(stats) = memory_stats::memory_stats() {
        let prev = state.process_memory;
        state.process_memory = stats.physical_mem as u64;

        if state.process_memory > state.peak_memory {
            state.peak_memory = state.process_memory;
        }

        if prev > 0 && dt > 0.0 {
            let diff = state.process_memory as i64 - prev as i64;
            state.allocation_rate = diff as f64 / dt as f64;
        }

        let memory_mb = state.process_memory as f32 / (1024.0 * 1024.0);
        state.push_sample(memory_mb);
        state.calculate_trend();
    }

    let mut mesh_bytes: u64 = 0;
    let mesh_count = meshes.len();
    for (_, mesh) in meshes.iter() {
        if let Some(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            mesh_bytes += (positions.len() * 32) as u64;
        }
        if let Some(indices) = mesh.indices() {
            mesh_bytes += (indices.len() * 4) as u64;
        }
    }

    let mut texture_bytes: u64 = 0;
    let texture_count = images.len();
    for (_, image) in images.iter() {
        texture_bytes += image.width() as u64 * image.height() as u64 * 4;
    }

    let material_count = materials.len();
    let material_bytes = (material_count * 256) as u64;

    state.asset_memory = AssetMemoryStats {
        meshes_bytes: mesh_bytes,
        mesh_count,
        textures_bytes: texture_bytes,
        texture_count,
        materials_bytes: material_bytes,
        material_count,
    };
}

pub fn update_system_timing(
    mut state: ResMut<SystemTimingState>,
    time: Res<Time>,
    diagnostics: Res<DiagnosticsStore>,
) {
    state.time_since_update += time.delta_secs();
    if state.time_since_update < state.update_interval { return; }
    state.time_since_update = 0.0;

    let frame_time_ms = if let Some(ft) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FRAME_TIME) {
        ft.smoothed().unwrap_or(0.0) as f32
    } else {
        16.67
    };

    state.schedule_timings = vec![
        ScheduleTiming { name: "PreUpdate".to_string(), time_ms: frame_time_ms * 0.05, percentage: 5.0 },
        ScheduleTiming { name: "Update".to_string(), time_ms: frame_time_ms * 0.35, percentage: 35.0 },
        ScheduleTiming { name: "PostUpdate".to_string(), time_ms: frame_time_ms * 0.10, percentage: 10.0 },
        ScheduleTiming { name: "Render".to_string(), time_ms: frame_time_ms * 0.50, percentage: 50.0 },
    ];
}

pub fn update_camera_debug_state(
    mut state: ResMut<CameraDebugState>,
    time: Res<Time>,
    cameras_3d: Query<(
        Entity,
        &Camera,
        &GlobalTransform,
        Option<&Name>,
        Option<&Projection>,
    ), With<Camera3d>>,
    cameras_2d: Query<(
        Entity,
        &Camera,
        &GlobalTransform,
        Option<&Name>,
    ), (With<Camera2d>, Without<Camera3d>)>,
) {
    state.time_since_update += time.delta_secs();
    if state.time_since_update < state.update_interval { return; }
    state.time_since_update = 0.0;

    state.cameras.clear();

    for (entity, camera, transform, name, projection) in cameras_3d.iter() {
        let (projection_type, fov, near, far, ortho_scale) = if let Some(proj) = projection {
            match proj {
                Projection::Perspective(p) => (
                    CameraProjectionType::Perspective,
                    Some(p.fov.to_degrees()),
                    p.near,
                    p.far,
                    None,
                ),
                Projection::Orthographic(o) => (
                    CameraProjectionType::Orthographic,
                    None,
                    o.near,
                    o.far,
                    Some(o.scale),
                ),
                _ => (CameraProjectionType::Perspective, Some(45.0), 0.1, 1000.0, None),
            }
        } else {
            (CameraProjectionType::Perspective, Some(45.0), 0.1, 1000.0, None)
        };

        let (_, rotation, translation) = transform.to_scale_rotation_translation();
        let euler = rotation.to_euler(EulerRot::YXZ);
        let rotation_degrees = Vec3::new(
            euler.1.to_degrees(),
            euler.0.to_degrees(),
            euler.2.to_degrees(),
        );

        let clear_color = match &camera.clear_color {
            ClearColorConfig::Default => None,
            ClearColorConfig::Custom(color) => Some(color.clone()),
            ClearColorConfig::None => None,
        };

        let viewport = camera.viewport.as_ref().map(|v| {
            [
                v.physical_position.x as f32,
                v.physical_position.y as f32,
                v.physical_size.x as f32,
                v.physical_size.y as f32,
            ]
        });

        state.cameras.push(CameraInfo {
            entity,
            name: name.map(|n| n.as_str().to_string()).unwrap_or_else(|| format!("Camera {:?}", entity)),
            is_active: camera.is_active,
            order: camera.order,
            projection_type,
            fov_degrees: fov,
            near,
            far,
            aspect_ratio: 16.0 / 9.0,
            ortho_scale,
            position: translation,
            rotation_degrees,
            forward: transform.forward().as_vec3(),
            clear_color,
            viewport,
        });
    }

    for (entity, camera, transform, name) in cameras_2d.iter() {
        let (_, rotation, translation) = transform.to_scale_rotation_translation();
        let euler = rotation.to_euler(EulerRot::YXZ);
        let rotation_degrees = Vec3::new(
            euler.1.to_degrees(),
            euler.0.to_degrees(),
            euler.2.to_degrees(),
        );

        let clear_color = match &camera.clear_color {
            ClearColorConfig::Default => None,
            ClearColorConfig::Custom(color) => Some(color.clone()),
            ClearColorConfig::None => None,
        };

        state.cameras.push(CameraInfo {
            entity,
            name: name.map(|n| n.as_str().to_string()).unwrap_or_else(|| format!("Camera2D {:?}", entity)),
            is_active: camera.is_active,
            order: camera.order,
            projection_type: CameraProjectionType::Orthographic,
            fov_degrees: None,
            near: -1000.0,
            far: 1000.0,
            aspect_ratio: 16.0 / 9.0,
            ortho_scale: Some(1.0),
            position: translation,
            rotation_degrees,
            forward: transform.forward().as_vec3(),
            clear_color,
            viewport: None,
        });
    }

    state.cameras.sort_by(|a, b| a.order.cmp(&b.order));

    if let Some(selected) = state.selected_camera {
        if !state.cameras.iter().any(|c| c.entity == selected) {
            state.selected_camera = None;
        }
    }
}

pub fn update_culling_debug_state(
    mut state: ResMut<CullingDebugState>,
    time: Res<Time>,
    mesh_entities: Query<(&GlobalTransform, &ViewVisibility), With<Mesh3d>>,
    camera_q: Query<&GlobalTransform, With<Camera3d>>,
) {
    state.time_since_update += time.delta_secs();
    if state.time_since_update < state.update_interval { return; }
    state.time_since_update = 0.0;

    let camera_pos = camera_q.iter().next().map(|t| t.translation()).unwrap_or(Vec3::ZERO);

    let mut total = 0u32;
    let mut frustum_visible = 0u32;
    let mut frustum_culled = 0u32;
    let mut distance_culled_count = 0u32;
    let mut distance_faded_count = 0u32;
    let mut buckets = [0u32; 5];

    let max_dist = state.max_distance;
    let fade_start = max_dist * state.fade_start_fraction;

    for (transform, view_vis) in mesh_entities.iter() {
        total += 1;
        let dist = transform.translation().distance(camera_pos);

        let bucket_idx = if dist < 50.0 { 0 }
            else if dist < 100.0 { 1 }
            else if dist < 200.0 { 2 }
            else if dist < 500.0 { 3 }
            else { 4 };
        buckets[bucket_idx] += 1;

        if view_vis.get() {
            frustum_visible += 1;
            if state.enabled {
                if dist > max_dist {
                    distance_culled_count += 1;
                } else if dist > fade_start {
                    distance_faded_count += 1;
                }
            }
        } else {
            frustum_culled += 1;
        }
    }

    state.total_entities = total;
    state.frustum_visible = frustum_visible;
    state.frustum_culled = frustum_culled;
    state.distance_culled = distance_culled_count;
    state.distance_faded = distance_faded_count;
    state.distance_buckets = buckets;
}

// ============================================================================
// ECS Stats State
// ============================================================================

#[derive(Clone, Debug)]
pub struct ArchetypeInfo {
    pub id: usize,
    pub entity_count: usize,
    pub components: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ComponentTypeStats {
    pub name: String,
    pub instance_count: usize,
    pub archetype_count: usize,
}

#[derive(Resource, Clone)]
pub struct EcsStatsState {
    pub entity_count: usize,
    pub entity_count_history: VecDeque<f32>,
    pub archetype_count: usize,
    pub top_archetypes: Vec<ArchetypeInfo>,
    pub component_stats: Vec<ComponentTypeStats>,
    pub resources: Vec<String>,
    pub show_archetypes: bool,
    pub show_components: bool,
    pub show_resources: bool,
    pub update_interval: f32,
    pub time_since_update: f32,
}

impl Default for EcsStatsState {
    fn default() -> Self {
        Self {
            entity_count: 0,
            entity_count_history: VecDeque::with_capacity(MAX_SAMPLES),
            archetype_count: 0,
            top_archetypes: Vec::new(),
            component_stats: Vec::new(),
            resources: Vec::new(),
            show_archetypes: false,
            show_components: false,
            show_resources: false,
            update_interval: 0.5,
            time_since_update: 0.0,
        }
    }
}

pub fn update_ecs_stats(world: &mut World) {
    let time_delta = {
        let time = world.resource::<Time>();
        time.delta_secs()
    };

    {
        let mut state = world.resource_mut::<EcsStatsState>();
        state.time_since_update += time_delta;
        if state.time_since_update < state.update_interval { return; }
        state.time_since_update = 0.0;
    }

    let entity_count = world.entities().len() as usize;

    let mut archetype_infos: Vec<ArchetypeInfo> = Vec::new();
    let mut component_counts: HashMap<String, (usize, usize)> = HashMap::new();

    for archetype in world.archetypes().iter() {
        let arch_entity_count = archetype.len() as usize;
        if arch_entity_count == 0 { continue; }

        let mut components: Vec<String> = Vec::new();
        for component_id in archetype.components() {
            if let Some(info) = world.components().get_info(*component_id) {
                let name = info.name().to_string();
                let entry = component_counts.entry(name.clone()).or_insert((0, 0));
                entry.0 += arch_entity_count;
                entry.1 += 1;
                components.push(name);
            }
        }

        archetype_infos.push(ArchetypeInfo {
            id: archetype.id().index(),
            entity_count: arch_entity_count,
            components,
        });
    }

    let archetype_count = archetype_infos.len();
    archetype_infos.sort_by(|a, b| b.entity_count.cmp(&a.entity_count));
    let top_archetypes: Vec<ArchetypeInfo> = archetype_infos.into_iter().take(20).collect();

    let mut stats: Vec<ComponentTypeStats> = component_counts
        .into_iter()
        .map(|(name, (instance_count, archetype_count))| ComponentTypeStats { name, instance_count, archetype_count })
        .collect();
    stats.sort_by(|a, b| b.instance_count.cmp(&a.instance_count));

    let mut state = world.resource_mut::<EcsStatsState>();
    state.entity_count = entity_count;
    if state.entity_count_history.len() >= MAX_SAMPLES { state.entity_count_history.pop_front(); }
    state.entity_count_history.push_back(entity_count as f32);
    state.archetype_count = archetype_count;
    state.top_archetypes = top_archetypes;
    state.component_stats = stats;
}

// ============================================================================
// Render Pipeline Graph Data
// ============================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RenderNodeCategory {
    CameraSetup,
    Shadow,
    Geometry,
    Transparency,
    PostProcess,
    Upscale,
    Custom,
    Other,
}

impl RenderNodeCategory {
    pub fn color(&self) -> [u8; 3] {
        match self {
            RenderNodeCategory::CameraSetup => [80, 140, 220],
            RenderNodeCategory::Shadow => [160, 90, 200],
            RenderNodeCategory::Geometry => [80, 190, 120],
            RenderNodeCategory::Transparency => [80, 200, 200],
            RenderNodeCategory::PostProcess => [220, 160, 60],
            RenderNodeCategory::Upscale => [200, 80, 180],
            RenderNodeCategory::Custom => [200, 70, 70],
            RenderNodeCategory::Other => [140, 140, 150],
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            RenderNodeCategory::CameraSetup => "Camera Setup",
            RenderNodeCategory::Shadow => "Shadow",
            RenderNodeCategory::Geometry => "Geometry",
            RenderNodeCategory::Transparency => "Transparency",
            RenderNodeCategory::PostProcess => "Post Process",
            RenderNodeCategory::Upscale => "Upscale",
            RenderNodeCategory::Custom => "Custom",
            RenderNodeCategory::Other => "Other",
        }
    }
}

#[derive(Clone, Debug)]
pub struct RenderGraphNode {
    pub id: usize,
    pub display_name: String,
    pub type_name: String,
    pub sub_graph: String,
    pub category: RenderNodeCategory,
    pub position: [f32; 2],
    pub layer: usize,
    pub gpu_time_ms: f32,
}

#[derive(Clone, Debug)]
pub struct RenderGraphEdge {
    pub from: usize,
    pub to: usize,
}

#[derive(Clone, Debug)]
pub struct RenderPipelineCanvasState {
    pub offset: [f32; 2],
    pub zoom: f32,
}

impl Default for RenderPipelineCanvasState {
    fn default() -> Self {
        Self { offset: [0.0, 0.0], zoom: 1.0 }
    }
}

impl RenderPipelineCanvasState {
    pub fn zoom_at(&mut self, screen_pos: [f32; 2], canvas_center: [f32; 2], delta: f32) {
        let old_zoom = self.zoom;
        self.zoom = (self.zoom * (1.0 + delta * 0.003)).clamp(0.15, 5.0);
        if (self.zoom - old_zoom).abs() > 0.0001 {
            let rel = [screen_pos[0] - canvas_center[0], screen_pos[1] - canvas_center[1]];
            let canvas_before = [rel[0] / old_zoom - self.offset[0], rel[1] / old_zoom - self.offset[1]];
            let canvas_after = [rel[0] / self.zoom - self.offset[0], rel[1] / self.zoom - self.offset[1]];
            self.offset[0] += canvas_after[0] - canvas_before[0];
            self.offset[1] += canvas_after[1] - canvas_before[1];
        }
    }
}

#[derive(Resource, Clone)]
pub struct RenderPipelineGraphData {
    pub nodes: Vec<RenderGraphNode>,
    pub edges: Vec<RenderGraphEdge>,
    pub sub_graphs: Vec<String>,
    pub canvas: RenderPipelineCanvasState,
    pub show_timing: bool,
    pub show_sub_graphs: bool,
    pub node_index: HashMap<usize, usize>,
    pub hovered_node: Option<usize>,
    pub dragged_node: Option<usize>,
    pub drag_offset: [f32; 2],
    pub vertical: bool,
    pub initialized: bool,
}

impl Default for RenderPipelineGraphData {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            sub_graphs: Vec::new(),
            canvas: RenderPipelineCanvasState::default(),
            show_timing: true,
            show_sub_graphs: true,
            node_index: HashMap::new(),
            hovered_node: None,
            dragged_node: None,
            drag_offset: [0.0, 0.0],
            vertical: false,
            initialized: false,
        }
    }
}

impl RenderPipelineGraphData {
    pub fn get_node(&self, id: usize) -> Option<&RenderGraphNode> {
        self.node_index.get(&id).and_then(|&idx| self.nodes.get(idx))
    }

    pub fn rebuild_index(&mut self) {
        self.node_index.clear();
        for (idx, node) in self.nodes.iter().enumerate() {
            self.node_index.insert(node.id, idx);
        }
    }
}

pub fn extract_render_graph(app: &mut App) {
    use bevy::render::RenderApp;
    use bevy::render::render_graph::RenderGraph;

    let mut data = app.world_mut().resource_mut::<RenderPipelineGraphData>();
    data.nodes.clear();
    data.edges.clear();
    data.sub_graphs.clear();
    drop(data);

    let mut all_nodes: Vec<RenderGraphNode> = Vec::new();
    let mut all_edges: Vec<RenderGraphEdge> = Vec::new();
    let mut sub_graph_names: Vec<String> = Vec::new();
    let mut next_id: usize = 0;

    if let Some(render_app) = app.get_sub_app(RenderApp) {
        let render_graph = render_app.world().resource::<RenderGraph>();

        let label_to_id = extract_graph_nodes(render_graph, "Main", &mut all_nodes, &mut all_edges, &mut next_id);
        if !label_to_id.is_empty() { sub_graph_names.push("Main".to_string()); }

        for (sub_label, sub_graph) in render_graph.iter_sub_graphs() {
            let sg_name = format!("{:?}", sub_label);
            extract_graph_nodes(sub_graph, &sg_name, &mut all_nodes, &mut all_edges, &mut next_id);
            sub_graph_names.push(sg_name);
        }
    }

    let mut data = app.world_mut().resource_mut::<RenderPipelineGraphData>();
    data.nodes = all_nodes;
    data.edges = all_edges;
    data.sub_graphs = sub_graph_names;
    data.rebuild_index();
    auto_layout(&mut data);
    data.initialized = true;
}

fn extract_graph_nodes(
    graph: &bevy::render::render_graph::RenderGraph,
    sub_graph_name: &str,
    all_nodes: &mut Vec<RenderGraphNode>,
    all_edges: &mut Vec<RenderGraphEdge>,
    next_id: &mut usize,
) -> HashMap<String, usize> {
    let mut label_to_id: HashMap<String, usize> = HashMap::new();

    for node_state in graph.iter_nodes() {
        let label_str = format!("{:?}", node_state.label);
        let type_name = node_state.type_name.to_string();
        let display_name = clean_type_name(&type_name, &label_str);
        let category = categorize_node(&type_name, &label_str);

        let id = *next_id;
        *next_id += 1;
        label_to_id.insert(label_str, id);

        all_nodes.push(RenderGraphNode {
            id, display_name, type_name,
            sub_graph: sub_graph_name.to_string(),
            category, position: [0.0, 0.0], layer: 0, gpu_time_ms: 0.0,
        });
    }

    for node_state in graph.iter_nodes() {
        let from_label = format!("{:?}", node_state.label);
        let from_id = match label_to_id.get(&from_label) { Some(&id) => id, None => continue };

        for edge in node_state.edges.output_edges() {
            let to_label = format!("{:?}", edge.get_input_node());
            if let Some(&to_id) = label_to_id.get(&to_label) {
                if !all_edges.iter().any(|e| e.from == from_id && e.to == to_id) {
                    all_edges.push(RenderGraphEdge { from: from_id, to: to_id });
                }
            }
        }
    }

    label_to_id
}

fn clean_type_name(type_name: &str, label_str: &str) -> String {
    let label_clean = label_str.trim_matches(|c: char| !c.is_alphanumeric() && c != ' ' && c != '_');
    let struct_name = type_name.rsplit("::").next().unwrap_or(type_name);
    let name = struct_name.trim_end_matches("Node").trim_end_matches("Pass").trim_end_matches("Driver");

    if name.is_empty() { return label_clean.replace('_', " "); }

    let mut result = String::new();
    for (i, ch) in name.chars().enumerate() {
        if i > 0 && ch.is_uppercase() {
            let prev = name.chars().nth(i - 1).unwrap_or(' ');
            if prev.is_lowercase() || (prev.is_uppercase() && name.chars().nth(i + 1).map_or(false, |n| n.is_lowercase())) {
                result.push(' ');
            }
        }
        result.push(ch);
    }

    if result.trim().is_empty() { label_clean.replace('_', " ") } else { result }
}

fn categorize_node(type_name: &str, label_str: &str) -> RenderNodeCategory {
    let lower = type_name.to_lowercase();
    let label_lower = label_str.to_lowercase();

    if lower.contains("camera") || lower.contains("driver") { return RenderNodeCategory::CameraSetup; }
    if lower.contains("shadow") { return RenderNodeCategory::Shadow; }
    if lower.contains("opaque") || lower.contains("prepass") || lower.contains("deferred")
        || lower.contains("main_pass") || label_lower.contains("opaque") { return RenderNodeCategory::Geometry; }
    if lower.contains("transparent") || lower.contains("transmissive") || lower.contains("alpha")
        || label_lower.contains("transparent") { return RenderNodeCategory::Transparency; }
    if lower.contains("bloom") || lower.contains("tonemap") || lower.contains("fxaa")
        || lower.contains("smaa") || lower.contains("taa") || lower.contains("sharpen")
        || lower.contains("auto_exposure") || lower.contains("msaa") || lower.contains("skybox")
        || lower.contains("dof") || lower.contains("motion_blur") || lower.contains("chromatic")
        || lower.contains("post_process") || lower.contains("copy_deferred") { return RenderNodeCategory::PostProcess; }
    if lower.contains("upscal") || label_lower.contains("upscal") { return RenderNodeCategory::Upscale; }
    if lower.contains("outline") || lower.contains("renzora") || lower.contains("custom") { return RenderNodeCategory::Custom; }

    RenderNodeCategory::Other
}

pub fn auto_layout(data: &mut RenderPipelineGraphData) {
    if data.nodes.is_empty() { return; }

    let node_width: f32 = 180.0;
    let node_height: f32 = 60.0;
    let h_gap: f32 = 100.0;
    let v_gap: f32 = 30.0;

    let mut predecessors: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut successors: HashMap<usize, Vec<usize>> = HashMap::new();
    for node in &data.nodes { predecessors.insert(node.id, Vec::new()); successors.insert(node.id, Vec::new()); }
    for edge in &data.edges { predecessors.entry(edge.to).or_default().push(edge.from); successors.entry(edge.from).or_default().push(edge.to); }

    let mut layer_of: HashMap<usize, usize> = HashMap::new();
    let mut in_degree: HashMap<usize, usize> = HashMap::new();
    for node in &data.nodes { in_degree.insert(node.id, 0); }
    for edge in &data.edges { *in_degree.entry(edge.to).or_default() += 1; }

    let mut queue: Vec<usize> = Vec::new();
    for node in &data.nodes {
        if in_degree[&node.id] == 0 { queue.push(node.id); layer_of.insert(node.id, 0); }
    }

    while let Some(nid) = queue.first().copied() {
        queue.remove(0);
        if let Some(succs) = successors.get(&nid) {
            for &s in succs {
                let current_layer = layer_of[&nid];
                let entry = layer_of.entry(s).or_insert(0);
                *entry = (*entry).max(current_layer + 1);
                let deg = in_degree.get_mut(&s).unwrap();
                *deg -= 1;
                if *deg == 0 { queue.push(s); }
            }
        }
    }

    for node in &data.nodes { layer_of.entry(node.id).or_insert(0); }
    let max_layer = layer_of.values().copied().max().unwrap_or(0);

    let mut layers: Vec<Vec<usize>> = vec![Vec::new(); max_layer + 1];
    for node in &data.nodes { layers[layer_of[&node.id]].push(node.id); }

    for layer_idx in 1..=max_layer {
        let mut barycenters: Vec<(usize, f32)> = Vec::new();
        for &nid in &layers[layer_idx] {
            let preds = &predecessors[&nid];
            if preds.is_empty() { barycenters.push((nid, 0.0)); } else {
                let mut sum = 0.0f32;
                let mut count = 0;
                for &p in preds {
                    let p_layer = layer_of[&p];
                    if let Some(pos) = layers[p_layer].iter().position(|&x| x == p) { sum += pos as f32; count += 1; }
                }
                barycenters.push((nid, if count > 0 { sum / count as f32 } else { 0.0 }));
            }
        }
        barycenters.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        layers[layer_idx] = barycenters.into_iter().map(|(nid, _)| nid).collect();
    }

    let vertical = data.vertical;
    let max_nodes_in_layer = layers.iter().map(|l| l.len()).max().unwrap_or(1);

    if vertical {
        let total_width = max_nodes_in_layer as f32 * (node_width + h_gap) - h_gap;
        for (layer_idx, layer_nodes) in layers.iter().enumerate() {
            let layer_width = layer_nodes.len() as f32 * (node_width + h_gap) - h_gap;
            let x_offset = (total_width - layer_width) / 2.0;
            for (node_idx, &nid) in layer_nodes.iter().enumerate() {
                if let Some(&idx) = data.node_index.get(&nid) {
                    data.nodes[idx].position = [x_offset + node_idx as f32 * (node_width + h_gap), layer_idx as f32 * (node_height + v_gap + 40.0)];
                    data.nodes[idx].layer = layer_idx;
                }
            }
        }
    } else {
        let total_height = max_nodes_in_layer as f32 * (node_height + v_gap) - v_gap;
        for (layer_idx, layer_nodes) in layers.iter().enumerate() {
            let layer_height = layer_nodes.len() as f32 * (node_height + v_gap) - v_gap;
            let y_offset = (total_height - layer_height) / 2.0;
            for (node_idx, &nid) in layer_nodes.iter().enumerate() {
                if let Some(&idx) = data.node_index.get(&nid) {
                    data.nodes[idx].position = [layer_idx as f32 * (node_width + h_gap), y_offset + node_idx as f32 * (node_height + v_gap)];
                    data.nodes[idx].layer = layer_idx;
                }
            }
        }
    }
}

pub fn update_render_pipeline_timing(
    mut graph_data: ResMut<RenderPipelineGraphData>,
    render_stats: Res<RenderStats>,
) {
    if !graph_data.initialized { return; }

    if render_stats.gpu_time_ms > 0.0 {
        let total_weight: f32 = graph_data.nodes.iter().map(|n| category_weight(n.category)).sum();
        if total_weight > 0.0 {
            for node in &mut graph_data.nodes {
                let weight = category_weight(node.category);
                node.gpu_time_ms = render_stats.gpu_time_ms * weight / total_weight;
            }
        }
    }
}

fn category_weight(cat: RenderNodeCategory) -> f32 {
    match cat {
        RenderNodeCategory::Geometry => 3.0,
        RenderNodeCategory::Shadow => 2.0,
        RenderNodeCategory::Transparency => 2.0,
        RenderNodeCategory::PostProcess => 1.5,
        RenderNodeCategory::Upscale => 1.0,
        RenderNodeCategory::CameraSetup => 0.5,
        RenderNodeCategory::Custom => 1.0,
        RenderNodeCategory::Other => 0.5,
    }
}
