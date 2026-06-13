//! Debug state resources and update systems

use bevy::diagnostic::{
    DiagnosticsStore, FrameTimeDiagnosticsPlugin, SystemInformationDiagnosticsPlugin,
};
use bevy::prelude::*;
use std::collections::{HashMap, VecDeque};

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
        if self.fps_history.is_empty() {
            return 0.0;
        }
        self.fps_history.iter().sum::<f32>() / self.fps_history.len() as f32
    }

    pub fn min_fps(&self) -> f32 {
        self.fps_history.iter().copied().fold(f32::MAX, f32::min)
    }

    pub fn max_fps(&self) -> f32 {
        self.fps_history.iter().copied().fold(0.0, f32::max)
    }

    pub fn avg_frame_time(&self) -> f32 {
        if self.frame_time_history.is_empty() {
            return 0.0;
        }
        self.frame_time_history.iter().sum::<f32>() / self.frame_time_history.len() as f32
    }

    pub fn one_percent_low_fps(&self) -> f32 {
        if self.fps_history.is_empty() {
            return 0.0;
        }
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
    /// Whether `gpu_time_ms` is a real measurement. False when the GPU backend
    /// doesn't support timestamp queries (e.g. OpenGL) — the panel then shows
    /// "n/a" instead of a fabricated value.
    pub gpu_timing_available: bool,
    pub gpu_time_history: Vec<f32>,
    /// Instance counts / loaded geometry totals — derived from the scene, not
    /// from the render pipeline (no culling/batching/instancing awareness).
    pub mesh_instances: u64,
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
            gpu_timing_available: false,
            gpu_time_history: Vec::with_capacity(MAX_SAMPLES),
            mesh_instances: 0,
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
            limitation_note: "Per-system timing is not collected in-process. \
                For detailed profiling, build with `cargo run --features \
                bevy/trace_chrome` and open the capture in chrome://tracing."
                .to_string(),
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
        let older: Vec<f32> = self
            .memory_history
            .iter()
            .rev()
            .skip(10)
            .take(10)
            .copied()
            .collect();

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
    /// Entities the UI wants to flip `Camera::is_active` on. Populated
    /// by the panel when the toggle button is clicked; drained by
    /// `apply_camera_toggles` in the same frame.
    pub pending_toggles: Vec<Entity>,
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
            pending_toggles: Vec::new(),
        }
    }
}

impl CameraDebugState {
    pub fn selected_camera_info(&self) -> Option<&CameraInfo> {
        self.selected_camera
            .and_then(|entity| self.cameras.iter().find(|c| c.entity == entity))
    }

    pub fn scene_camera_count(&self) -> usize {
        self.cameras.len()
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
    if !state.enabled {
        return;
    }

    state.time_since_update += time.delta_secs();
    if state.time_since_update < state.update_interval {
        return;
    }
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

/// Top-level (shallowest) GPU render-pass spans recorded by
/// `RenderDiagnosticsPlugin`, as `(pass_name, elapsed_ms)`.
///
/// The plugin emits one `render/<pass…>/elapsed_gpu` diagnostic per span, already
/// in milliseconds. Spans nest — a parent pass's time includes its children — so
/// summing every span would double-count. We keep only the shallowest depth (the
/// outermost passes); every deeper span is an descendant of one of those, so the
/// shallowest set both sums to the true frame total and gives a clean breakdown.
///
/// Returns an empty vec when no GPU timestamps exist (timestamp queries
/// unsupported), which callers treat as "GPU timing unavailable".
pub(crate) fn top_level_gpu_spans(diagnostics: &DiagnosticsStore) -> Vec<(String, f32)> {
    let mut spans: Vec<(usize, String, f32)> = Vec::new();
    let mut min_depth = usize::MAX;
    for diagnostic in diagnostics.iter() {
        let path = diagnostic.path().as_str();
        let Some(stripped) = path.strip_suffix("/elapsed_gpu") else {
            continue;
        };
        let Some(value) = diagnostic.smoothed() else {
            continue;
        };
        let depth = stripped.split('/').count();
        min_depth = min_depth.min(depth);
        let name = stripped.rsplit('/').next().unwrap_or(stripped).to_string();
        spans.push((depth, name, value as f32));
    }
    spans
        .into_iter()
        .filter(|(depth, _, _)| *depth == min_depth)
        .map(|(_, name, ms)| (name, ms))
        .collect()
}

pub fn update_render_stats(
    diagnostics: Res<DiagnosticsStore>,
    mut stats: ResMut<RenderStats>,
    time: Res<Time>,
    mesh_query: Query<&Mesh3d>,
    meshes: Res<Assets<Mesh>>,
) {
    stats.time_since_update += time.delta_secs();
    if stats.time_since_update < stats.update_interval {
        return;
    }
    stats.time_since_update = 0.0;

    let mut total_vertices: u64 = 0;
    let mut total_triangles: u64 = 0;
    let mut mesh_instances: u64 = 0;

    for mesh_handle in mesh_query.iter() {
        mesh_instances += 1;
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

    stats.mesh_instances = mesh_instances;
    stats.vertices = total_vertices;
    stats.triangles = total_triangles;

    // Real GPU time: sum of the top-level render-pass GPU spans (already in ms).
    // No fabricated fallback — if timestamp queries aren't available the panel
    // reports "n/a" rather than a frame-time-derived guess.
    let gpu_spans = top_level_gpu_spans(&diagnostics);
    if gpu_spans.is_empty() {
        stats.gpu_timing_available = false;
    } else {
        let total: f32 = gpu_spans.iter().map(|(_, ms)| *ms).sum();
        stats.gpu_timing_available = true;
        stats.gpu_time_ms = total;
        stats.push_gpu_time(total);
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
    if state.time_since_update < state.update_interval {
        return;
    }
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
        // Real CPU-side buffer sizes computed from the mesh's actual vertex
        // layout and index buffer (not a fixed bytes-per-vertex guess).
        mesh_bytes += mesh.get_vertex_buffer_size() as u64;
        if let Some(indices) = mesh.get_index_buffer_bytes() {
            mesh_bytes += indices.len() as u64;
        }
    }

    let mut texture_bytes: u64 = 0;
    let texture_count = images.len();
    for (_, image) in images.iter() {
        // Real allocated size when the pixel data is resident on the CPU;
        // dimensional estimate only as a fallback for GPU-only textures that
        // expose no CPU-side data.
        texture_bytes += match image.data.as_ref() {
            Some(data) => data.len() as u64,
            None => {
                let s = image.texture_descriptor.size;
                s.width as u64 * s.height as u64 * s.depth_or_array_layers as u64 * 4
            }
        };
    }

    // StandardMaterial's GPU uniform is a fixed-size struct; 256 bytes is a
    // reasonable per-material estimate for it. Referenced textures are counted
    // above under texture memory, not here.
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
    if state.time_since_update < state.update_interval {
        return;
    }
    state.time_since_update = 0.0;

    // Real per-pass GPU timings from `RenderDiagnosticsPlugin` (top-level spans,
    // in ms). Replaces the old fabricated PreUpdate/Update/PostUpdate/Render split
    // that was just fixed fractions of frame time. Empty when GPU timestamps are
    // unavailable, in which case the panel shows "No timing data available".
    let mut spans = top_level_gpu_spans(&diagnostics);
    spans.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let total: f32 = spans.iter().map(|(_, ms)| *ms).sum();

    state.schedule_timings = spans
        .into_iter()
        .map(|(name, time_ms)| ScheduleTiming {
            name,
            time_ms,
            percentage: if total > 0.0 { time_ms / total * 100.0 } else { 0.0 },
        })
        .collect();
}

pub fn update_camera_debug_state(
    mut state: ResMut<CameraDebugState>,
    time: Res<Time>,
    cameras_3d: Query<
        (
            Entity,
            &Camera,
            &GlobalTransform,
            Option<&Name>,
            Option<&Projection>,
        ),
        With<Camera3d>,
    >,
    cameras_2d: Query<
        (
            Entity,
            &Camera,
            &GlobalTransform,
            Option<&Name>,
            Option<&Projection>,
        ),
        (With<Camera2d>, Without<Camera3d>),
    >,
) {
    state.time_since_update += time.delta_secs();
    if state.time_since_update < state.update_interval {
        return;
    }
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
                _ => (
                    CameraProjectionType::Perspective,
                    Some(45.0),
                    0.1,
                    1000.0,
                    None,
                ),
            }
        } else {
            (
                CameraProjectionType::Perspective,
                Some(45.0),
                0.1,
                1000.0,
                None,
            )
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
            ClearColorConfig::Custom(color) => Some(*color),
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

        // Real aspect ratio from the camera's render target / viewport; falls
        // back to 16:9 only when the target size isn't known yet (e.g. before the
        // first frame), rather than always reporting a hardcoded 16:9.
        let aspect_ratio = camera
            .logical_viewport_size()
            .filter(|s| s.y > 0.0)
            .map(|s| s.x / s.y)
            .unwrap_or(16.0 / 9.0);

        state.cameras.push(CameraInfo {
            entity,
            name: name
                .map(|n| n.as_str().to_string())
                .unwrap_or_else(|| format!("Camera {:?}", entity)),
            is_active: camera.is_active,
            order: camera.order,
            projection_type,
            fov_degrees: fov,
            near,
            far,
            aspect_ratio,
            ortho_scale,
            position: translation,
            rotation_degrees,
            forward: transform.forward().as_vec3(),
            clear_color,
            viewport,
        });
    }

    for (entity, camera, transform, name, projection) in cameras_2d.iter() {
        let (_, rotation, translation) = transform.to_scale_rotation_translation();
        let euler = rotation.to_euler(EulerRot::YXZ);
        let rotation_degrees = Vec3::new(
            euler.1.to_degrees(),
            euler.0.to_degrees(),
            euler.2.to_degrees(),
        );

        let clear_color = match &camera.clear_color {
            ClearColorConfig::Default => None,
            ClearColorConfig::Custom(color) => Some(*color),
            ClearColorConfig::None => None,
        };

        // Read the real orthographic projection instead of assuming defaults.
        let (near, far, ortho_scale) = match projection {
            Some(Projection::Orthographic(o)) => (o.near, o.far, Some(o.scale)),
            _ => (-1000.0, 1000.0, None),
        };
        let aspect_ratio = camera
            .logical_viewport_size()
            .filter(|s| s.y > 0.0)
            .map(|s| s.x / s.y)
            .unwrap_or(16.0 / 9.0);

        state.cameras.push(CameraInfo {
            entity,
            name: name
                .map(|n| n.as_str().to_string())
                .unwrap_or_else(|| format!("Camera2D {:?}", entity)),
            is_active: camera.is_active,
            order: camera.order,
            projection_type: CameraProjectionType::Orthographic,
            fov_degrees: None,
            near,
            far,
            aspect_ratio,
            ortho_scale,
            position: translation,
            rotation_degrees,
            forward: transform.forward().as_vec3(),
            clear_color,
            viewport: None,
        });
    }

    state.cameras.sort_by_key(|c| c.order);

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
    if state.time_since_update < state.update_interval {
        return;
    }
    state.time_since_update = 0.0;

    let camera_pos = camera_q
        .iter()
        .next()
        .map(|t| t.translation())
        .unwrap_or(Vec3::ZERO);

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

        let bucket_idx = if dist < 50.0 {
            0
        } else if dist < 100.0 {
            1
        } else if dist < 200.0 {
            2
        } else if dist < 500.0 {
            3
        } else {
            4
        };
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
        if state.time_since_update < state.update_interval {
            return;
        }
        state.time_since_update = 0.0;
    }

    let entity_count = world.entities().len() as usize;

    let mut archetype_infos: Vec<ArchetypeInfo> = Vec::new();
    let mut component_counts: HashMap<String, (usize, usize)> = HashMap::new();

    for archetype in world.archetypes().iter() {
        let arch_entity_count = archetype.len() as usize;
        if arch_entity_count == 0 {
            continue;
        }

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
    archetype_infos.sort_by_key(|a| std::cmp::Reverse(a.entity_count));
    let top_archetypes: Vec<ArchetypeInfo> = archetype_infos.into_iter().take(20).collect();

    let mut stats: Vec<ComponentTypeStats> = component_counts
        .into_iter()
        .map(
            |(name, (instance_count, archetype_count))| ComponentTypeStats {
                name,
                instance_count,
                archetype_count,
            },
        )
        .collect();
    stats.sort_by_key(|s| std::cmp::Reverse(s.instance_count));

    let mut state = world.resource_mut::<EcsStatsState>();
    state.entity_count = entity_count;
    if state.entity_count_history.len() >= MAX_SAMPLES {
        state.entity_count_history.pop_front();
    }
    state.entity_count_history.push_back(entity_count as f32);
    state.archetype_count = archetype_count;
    state.top_archetypes = top_archetypes;
    state.component_stats = stats;
}
