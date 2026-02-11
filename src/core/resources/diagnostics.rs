//! Diagnostics state resource for performance monitoring panels

use bevy::prelude::*;
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin, EntityCountDiagnosticsPlugin, SystemInformationDiagnosticsPlugin};
use std::collections::{VecDeque, HashMap};

/// Maximum number of samples to keep for graphs
const MAX_SAMPLES: usize = 120;

/// Maximum number of archetypes to display
const MAX_ARCHETYPES_DISPLAY: usize = 20;

/// Information about a single render pass
#[derive(Clone, Default)]
pub struct RenderPassInfo {
    pub name: String,
    pub gpu_time_ms: f32,
    pub cpu_time_ms: f32,
}

/// Render statistics from the GPU
#[derive(Resource)]
pub struct RenderStats {
    /// Whether render diagnostics are enabled
    pub enabled: bool,
    /// Total GPU time in ms
    pub gpu_time_ms: f32,
    /// CPU time spent on render commands in ms
    pub cpu_render_time_ms: f32,
    /// GPU time history for graphing
    pub gpu_time_history: Vec<f32>,
    /// Number of draw calls
    pub draw_calls: u64,
    /// Number of triangles rendered
    pub triangles: u64,
    /// Number of vertices processed
    pub vertices: u64,
    /// Vertex shader invocations
    pub vertex_shader_invocations: u64,
    /// Fragment shader invocations
    pub fragment_shader_invocations: u64,
    /// Compute shader invocations
    pub compute_shader_invocations: u64,
    /// Per-pass timing information
    pub render_passes: Vec<RenderPassInfo>,
    /// Update interval
    pub update_interval: f32,
    /// Time since last update
    pub time_since_update: f32,
}

impl Default for RenderStats {
    fn default() -> Self {
        Self {
            enabled: false, // Will be enabled if RenderDiagnosticsPlugin is added
            gpu_time_ms: 0.0,
            cpu_render_time_ms: 0.0,
            gpu_time_history: Vec::with_capacity(MAX_SAMPLES),
            draw_calls: 0,
            triangles: 0,
            vertices: 0,
            vertex_shader_invocations: 0,
            fragment_shader_invocations: 0,
            compute_shader_invocations: 0,
            render_passes: Vec::new(),
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

/// Cached diagnostics state for UI panels
#[derive(Resource)]
pub struct DiagnosticsState {
    /// Current FPS
    pub fps: f64,
    /// Current frame time in ms
    pub frame_time_ms: f64,
    /// FPS history for graphing
    pub fps_history: VecDeque<f32>,
    /// Frame time history for graphing
    pub frame_time_history: VecDeque<f32>,
    /// Entity count
    pub entity_count: usize,
    /// Entity count history
    pub entity_count_history: VecDeque<f32>,
    /// CPU usage percentage (if available)
    pub cpu_usage: Option<f64>,
    /// Memory usage in bytes (if available)
    pub memory_usage: Option<u64>,
    /// Whether diagnostics are being collected
    pub enabled: bool,
    /// Update interval in seconds
    pub update_interval: f32,
    /// Time since last update
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
            update_interval: 0.1, // Update 10 times per second
            time_since_update: 0.0,
        }
    }
}

impl DiagnosticsState {
    /// Get average FPS over the history
    pub fn avg_fps(&self) -> f32 {
        if self.fps_history.is_empty() {
            return 0.0;
        }
        self.fps_history.iter().sum::<f32>() / self.fps_history.len() as f32
    }

    /// Get min FPS over the history
    pub fn min_fps(&self) -> f32 {
        self.fps_history.iter().copied().fold(f32::MAX, f32::min)
    }

    /// Get max FPS over the history
    pub fn max_fps(&self) -> f32 {
        self.fps_history.iter().copied().fold(0.0, f32::max)
    }

    /// Get average frame time over the history
    pub fn avg_frame_time(&self) -> f32 {
        if self.frame_time_history.is_empty() {
            return 0.0;
        }
        self.frame_time_history.iter().sum::<f32>() / self.frame_time_history.len() as f32
    }

    /// Get 1% low FPS (worst 1% of frames)
    pub fn one_percent_low_fps(&self) -> f32 {
        if self.fps_history.is_empty() {
            return 0.0;
        }
        let mut sorted: Vec<f32> = self.fps_history.iter().copied().collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let count = (sorted.len() as f32 * 0.01).max(1.0) as usize;
        sorted.iter().take(count).sum::<f32>() / count as f32
    }

    fn push_sample<T: Copy>(history: &mut VecDeque<T>, value: T) {
        if history.len() >= MAX_SAMPLES {
            history.pop_front();
        }
        history.push_back(value);
    }
}

/// System to update diagnostics state from Bevy's DiagnosticsStore
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

    // Only update at specified interval to reduce overhead
    if state.time_since_update < state.update_interval {
        return;
    }
    state.time_since_update = 0.0;

    // FPS
    if let Some(fps) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FPS) {
        if let Some(value) = fps.smoothed() {
            state.fps = value;
            DiagnosticsState::push_sample(&mut state.fps_history, value as f32);
        }
    }

    // Frame time
    if let Some(frame_time) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FRAME_TIME) {
        if let Some(value) = frame_time.smoothed() {
            state.frame_time_ms = value * 1000.0; // Convert to milliseconds
            DiagnosticsState::push_sample(&mut state.frame_time_history, (value * 1000.0) as f32);
        }
    }

    // Entity count - use query for accuracy
    let entity_count = entities.iter().count();
    state.entity_count = entity_count;
    DiagnosticsState::push_sample(&mut state.entity_count_history, entity_count as f32);

    // System info (CPU/Memory) - these might not be available on all platforms
    if let Some(cpu) = diagnostics.get(&SystemInformationDiagnosticsPlugin::SYSTEM_CPU_USAGE) {
        state.cpu_usage = cpu.smoothed();
    }

    if let Some(mem) = diagnostics.get(&SystemInformationDiagnosticsPlugin::SYSTEM_MEM_USAGE) {
        state.memory_usage = mem.smoothed().map(|v| v as u64);
    }
}

/// System to update render stats by counting mesh data in the scene
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

    // Count mesh statistics from the scene
    let mut total_vertices: u64 = 0;
    let mut total_triangles: u64 = 0;
    let mut draw_calls: u64 = 0;

    for mesh_handle in mesh_query.iter() {
        draw_calls += 1;

        if let Some(mesh) = meshes.get(&mesh_handle.0) {
            // Count vertices
            if let Some(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
                total_vertices += positions.len() as u64;
            }

            // Count triangles from indices
            if let Some(indices) = mesh.indices() {
                total_triangles += (indices.len() / 3) as u64;
            } else {
                // No indices means each 3 vertices form a triangle
                if let Some(positions) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
                    total_triangles += (positions.len() / 3) as u64;
                }
            }
        }
    }

    stats.draw_calls = draw_calls;
    stats.vertices = total_vertices;
    stats.triangles = total_triangles;

    // Estimate vertex/fragment shader invocations
    // Vertex shaders run once per vertex, fragment shaders run roughly per pixel covered
    stats.vertex_shader_invocations = total_vertices;
    // Fragment invocations is harder to estimate - use triangles * average pixels per triangle
    // This is a rough estimate assuming ~100 pixels per triangle on average
    stats.fragment_shader_invocations = total_triangles * 100;

    // Try to get GPU timing from diagnostics if available
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

    // If no GPU timing found, estimate from frame time
    if !found_gpu_timing {
        if let Some(frame_time) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FRAME_TIME) {
            if let Some(value) = frame_time.smoothed() {
                let gpu_time = (value * 1000.0 * 0.6) as f32;
                stats.gpu_time_ms = gpu_time;
                stats.push_gpu_time(gpu_time);
            }
        }
    }

    stats.enabled = true;
}

// ============================================================================
// ECS Stats State
// ============================================================================

/// Information about an archetype (collection of entities with same components)
#[derive(Clone, Debug)]
pub struct ArchetypeInfo {
    /// Unique archetype ID
    pub id: usize,
    /// Number of entities in this archetype
    pub entity_count: usize,
    /// Component type names in this archetype
    pub components: Vec<String>,
}

/// Component type statistics
#[derive(Clone, Debug, Default)]
pub struct ComponentTypeStats {
    /// Component type name
    pub name: String,
    /// Total instances across all archetypes
    pub instance_count: usize,
    /// Number of archetypes containing this component
    pub archetype_count: usize,
}

/// ECS statistics state for monitoring entity/component/archetype counts
#[derive(Resource)]
pub struct EcsStatsState {
    /// Current entity count
    pub entity_count: usize,
    /// Entity count history for graphing
    pub entity_count_history: VecDeque<f32>,
    /// Current archetype count
    pub archetype_count: usize,
    /// Top archetypes by entity count
    pub top_archetypes: Vec<ArchetypeInfo>,
    /// Component type statistics sorted by instance count
    pub component_stats: Vec<ComponentTypeStats>,
    /// List of resource type names
    pub resources: Vec<String>,
    /// Whether to show archetypes section (collapsible)
    pub show_archetypes: bool,
    /// Whether to show components section (collapsible)
    pub show_components: bool,
    /// Whether to show resources section (collapsible)
    pub show_resources: bool,
    /// Update interval in seconds
    pub update_interval: f32,
    /// Time since last update
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
            update_interval: 0.5, // Update every 0.5 seconds (less frequent due to overhead)
            time_since_update: 0.0,
        }
    }
}

impl EcsStatsState {
    fn push_sample(history: &mut VecDeque<f32>, value: f32) {
        if history.len() >= MAX_SAMPLES {
            history.pop_front();
        }
        history.push_back(value);
    }
}

// ============================================================================
// Memory Profiler State
// ============================================================================

/// Memory trend direction
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MemoryTrend {
    #[default]
    Stable,
    Increasing,
    Decreasing,
}

/// Asset memory breakdown by type
#[derive(Clone, Debug, Default)]
pub struct AssetMemoryStats {
    /// Estimated mesh memory in bytes
    pub meshes_bytes: u64,
    /// Number of loaded meshes
    pub mesh_count: usize,
    /// Estimated texture memory in bytes
    pub textures_bytes: u64,
    /// Number of loaded textures
    pub texture_count: usize,
    /// Estimated material memory in bytes
    pub materials_bytes: u64,
    /// Number of loaded materials
    pub material_count: usize,
}

/// Memory profiler state for tracking memory usage
#[derive(Resource)]
pub struct MemoryProfilerState {
    /// Process memory usage in bytes (from OS)
    pub process_memory: u64,
    /// Peak process memory usage
    pub peak_memory: u64,
    /// Memory usage history for graphing
    pub memory_history: VecDeque<f32>,
    /// Memory trend (increasing/stable/decreasing)
    pub memory_trend: MemoryTrend,
    /// Asset memory breakdown
    pub asset_memory: AssetMemoryStats,
    /// Allocation rate estimate (bytes per second)
    pub allocation_rate: f64,
    /// Previous memory sample for rate calculation
    prev_memory: u64,
    /// Update interval in seconds
    pub update_interval: f32,
    /// Time since last update
    pub time_since_update: f32,
    /// Whether memory profiling is available
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
            prev_memory: 0,
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

    /// Calculate memory trend based on recent history
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

        let threshold = older_avg * 0.05; // 5% threshold

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
// System Timing State
// ============================================================================

/// Schedule timing information
#[derive(Clone, Debug, Default)]
pub struct ScheduleTiming {
    /// Schedule name
    pub name: String,
    /// Estimated time in ms
    pub time_ms: f32,
    /// Percentage of frame time
    pub percentage: f32,
}

/// System timing state (limited without external profiler)
#[derive(Resource)]
pub struct SystemTimingState {
    /// Frame time breakdown by schedule (estimated)
    pub schedule_timings: Vec<ScheduleTiming>,
    /// Update interval
    pub update_interval: f32,
    /// Time since last update
    pub time_since_update: f32,
    /// Note about limitations
    pub limitation_note: String,
}

impl Default for SystemTimingState {
    fn default() -> Self {
        Self {
            schedule_timings: Vec::new(),
            update_interval: 0.5,
            time_since_update: 0.0,
            limitation_note: "Per-system timing requires Tracy integration. \
                See: https://github.com/bevyengine/bevy/blob/main/docs/profiling.md".to_string(),
        }
    }
}

// ============================================================================
// Update Systems
// ============================================================================

/// System to update ECS stats (exclusive system due to World access)
pub fn update_ecs_stats(world: &mut World) {
    // Get time delta first
    let time_delta = {
        let time = world.resource::<Time>();
        time.delta_secs()
    };

    // Check update interval
    {
        let mut state = world.resource_mut::<EcsStatsState>();
        state.time_since_update += time_delta;

        if state.time_since_update < state.update_interval {
            return;
        }
        state.time_since_update = 0.0;
    }

    // Collect data while holding only immutable references
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

    // Sort archetypes
    archetype_infos.sort_by(|a, b| b.entity_count.cmp(&a.entity_count));
    let top_archetypes: Vec<ArchetypeInfo> = archetype_infos.into_iter().take(MAX_ARCHETYPES_DISPLAY).collect();

    // Convert component counts to stats
    let mut stats: Vec<ComponentTypeStats> = component_counts
        .into_iter()
        .map(|(name, (instance_count, archetype_count))| ComponentTypeStats {
            name,
            instance_count,
            archetype_count,
        })
        .collect();
    stats.sort_by(|a, b| b.instance_count.cmp(&a.instance_count));

    // Now update state
    let mut state = world.resource_mut::<EcsStatsState>();
    state.entity_count = entity_count;
    EcsStatsState::push_sample(&mut state.entity_count_history, entity_count as f32);
    state.archetype_count = archetype_count;
    state.top_archetypes = top_archetypes;
    state.component_stats = stats;
}

/// System to update memory profiler stats
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

    // Get process memory
    if let Some(stats) = memory_stats::memory_stats() {
        let prev = state.process_memory;
        state.process_memory = stats.physical_mem as u64;

        // Update peak
        if state.process_memory > state.peak_memory {
            state.peak_memory = state.process_memory;
        }

        // Calculate allocation rate
        if prev > 0 && dt > 0.0 {
            let diff = state.process_memory as i64 - prev as i64;
            state.allocation_rate = diff as f64 / dt as f64;
        }

        // Push to history (in MB for graphing)
        let memory_mb = state.process_memory as f32 / (1024.0 * 1024.0);
        state.push_sample(memory_mb);
        state.calculate_trend();
    }

    // Estimate asset memory
    let mut mesh_bytes: u64 = 0;
    let mesh_count = meshes.len();
    for (_, mesh) in meshes.iter() {
        // Estimate mesh size: vertices * ~32 bytes + indices * 4 bytes
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
        // Estimate texture size from dimensions and format
        let width = image.width() as u64;
        let height = image.height() as u64;
        let bpp = 4u64; // Assume 4 bytes per pixel (RGBA8)
        texture_bytes += width * height * bpp;
    }

    let material_count = materials.len();
    // Rough estimate: ~256 bytes per material for properties + handles
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

/// System to update system timing stats (limited estimation)
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

    // Get frame time
    let frame_time_ms = if let Some(ft) = diagnostics.get(&FrameTimeDiagnosticsPlugin::FRAME_TIME) {
        ft.smoothed().unwrap_or(0.0) as f32 * 1000.0
    } else {
        16.67
    };

    // Estimate schedule breakdown (rough approximations)
    // These are just estimates since we can't get per-system timing without Tracy
    state.schedule_timings = vec![
        ScheduleTiming {
            name: "PreUpdate".to_string(),
            time_ms: frame_time_ms * 0.05,
            percentage: 5.0,
        },
        ScheduleTiming {
            name: "Update".to_string(),
            time_ms: frame_time_ms * 0.35,
            percentage: 35.0,
        },
        ScheduleTiming {
            name: "PostUpdate".to_string(),
            time_ms: frame_time_ms * 0.10,
            percentage: 10.0,
        },
        ScheduleTiming {
            name: "Render".to_string(),
            time_ms: frame_time_ms * 0.50,
            percentage: 50.0,
        },
    ];
}

// ============================================================================
// Plugin
// ============================================================================

/// Plugin to set up diagnostics
pub struct DiagnosticsPlugin;

impl Plugin for DiagnosticsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            FrameTimeDiagnosticsPlugin::default(),
            EntityCountDiagnosticsPlugin::default(),
            SystemInformationDiagnosticsPlugin::default(),
        ))
        .init_resource::<DiagnosticsState>()
        .init_resource::<RenderStats>()
        .init_resource::<EcsStatsState>()
        .init_resource::<MemoryProfilerState>()
        .init_resource::<SystemTimingState>()
        .init_resource::<super::physics_debug::PhysicsDebugState>()
        .init_resource::<super::camera_debug::CameraDebugState>()
        .init_resource::<super::physics_properties::PhysicsPropertiesState>()
        .init_resource::<super::physics_playground::PlaygroundState>()
        .init_resource::<super::physics_forces::PhysicsForcesState>()
        .init_resource::<super::physics_metrics::PhysicsMetricsState>()
        .init_resource::<super::physics_scenarios::PhysicsScenariosState>()
        .init_resource::<super::collision_viz::CollisionVizState>()
        .init_resource::<super::movement_trails::MovementTrailsState>()
        .init_resource::<super::stress_test::StressTestState>()
        .init_resource::<super::state_recorder::StateRecorderState>()
        .init_resource::<super::arena_presets::ArenaPresetsState>()
        .add_systems(Update, (
            update_diagnostics_state,
            update_render_stats,
            update_ecs_stats,
            update_memory_profiler,
            update_system_timing,
        ))
        .add_systems(Update, (
            super::physics_debug::update_physics_debug_state,
            super::camera_debug::update_camera_debug_state,
            super::physics_properties::sync_physics_properties,
            super::physics_playground::process_playground_commands,
            super::physics_forces::update_forces_panel_state,
            super::physics_forces::process_force_commands,
        ))
        .add_systems(Update, (
            super::physics_metrics::update_physics_metrics,
            super::physics_scenarios::process_scenario_commands,
            super::collision_viz::update_collision_viz,
            super::collision_viz::render_collision_viz_gizmos,
            super::movement_trails::update_movement_trails,
            super::movement_trails::render_trail_gizmos,
            super::movement_trails::process_trail_commands,
            super::stress_test::process_stress_test,
            super::state_recorder::process_recorder_commands,
            super::state_recorder::render_recorder_ghosts,
            super::arena_presets::process_arena_commands,
            super::arena_presets::animate_arena_kinematic,
            crate::gizmo::render_physics_debug_gizmos,
        ));
    }
}
