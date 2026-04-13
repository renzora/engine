//! Debugging panels and profiling support for the Renzora editor.
//!
//! Panels: System Profiler, Memory Profiler, Camera Debug, Physics Debug, Culling Debug.
//! Enable the `tracy` feature for Tracy profiler integration.

pub mod panels;
pub mod state;

use std::sync::{Arc, Mutex, RwLock};

use bevy::prelude::*;
use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, EntityCountDiagnosticsPlugin, SystemInformationDiagnosticsPlugin};
use bevy_egui::egui;

use renzora_editor_framework::{AppEditorExt, EditorPanel, PanelLocation};
use renzora_theme::ThemeManager;

use state::*;

// ============================================================================
// Bridge for panels that need mutable state
// ============================================================================

#[derive(Default, Clone)]
struct DebugBridgeInner {
    camera: Option<CameraDebugState>,
    culling: Option<CullingDebugState>,
}

#[derive(Resource, Clone)]
struct DebugBridge {
    pending: Arc<Mutex<DebugBridgeInner>>,
}

impl Default for DebugBridge {
    fn default() -> Self {
        Self { pending: Arc::new(Mutex::new(DebugBridgeInner::default())) }
    }
}

// ============================================================================
// System Profiler Panel
// ============================================================================

struct SystemProfilerPanel;

impl EditorPanel for SystemProfilerPanel {
    fn id(&self) -> &str { "system_profiler" }
    fn title(&self) -> &str { "System Profiler" }
    fn icon(&self) -> Option<&str> { Some(egui_phosphor::regular::CHART_LINE_UP) }
    fn default_location(&self) -> PanelLocation { PanelLocation::Bottom }
    fn min_size(&self) -> [f32; 2] { [200.0, 150.0] }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let diagnostics = world.get_resource::<DiagnosticsState>().cloned().unwrap_or_default();
        let timing = world.get_resource::<SystemTimingState>().cloned().unwrap_or_default();
        let render = world.get_resource::<RenderStats>().cloned().unwrap_or_default();
        let theme = world.get_resource::<ThemeManager>().map(|tm| tm.active_theme.clone()).unwrap_or_default();
        panels::system_profiler::render_system_profiler_content(ui, &diagnostics, &timing, &render, &theme);
    }
}

// ============================================================================
// Memory Profiler Panel
// ============================================================================

struct MemoryProfilerPanel;

impl EditorPanel for MemoryProfilerPanel {
    fn id(&self) -> &str { "memory_profiler" }
    fn title(&self) -> &str { "Memory Profiler" }
    fn icon(&self) -> Option<&str> { Some(egui_phosphor::regular::MEMORY) }
    fn default_location(&self) -> PanelLocation { PanelLocation::Bottom }
    fn min_size(&self) -> [f32; 2] { [200.0, 150.0] }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let state = world.get_resource::<MemoryProfilerState>().cloned().unwrap_or_default();
        let theme = world.get_resource::<ThemeManager>().map(|tm| tm.active_theme.clone()).unwrap_or_default();
        panels::memory::render_memory_profiler_content(ui, &state, &theme);
    }
}

// ============================================================================
// Camera Debug Panel
// ============================================================================

struct CameraDebugPanel {
    bridge: Arc<Mutex<DebugBridgeInner>>,
    local: RwLock<CameraDebugState>,
}

impl CameraDebugPanel {
    fn new(bridge: Arc<Mutex<DebugBridgeInner>>) -> Self {
        Self { bridge, local: RwLock::new(CameraDebugState::default()) }
    }
}

impl EditorPanel for CameraDebugPanel {
    fn id(&self) -> &str { "camera_debug" }
    fn title(&self) -> &str { "Camera Debug" }
    fn icon(&self) -> Option<&str> { Some(egui_phosphor::regular::VIDEO_CAMERA) }
    fn default_location(&self) -> PanelLocation { PanelLocation::Right }
    fn min_size(&self) -> [f32; 2] { [200.0, 150.0] }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        if let Some(state) = world.get_resource::<CameraDebugState>() {
            if let Ok(mut local) = self.local.write() {
                local.cameras = state.cameras.clone();
                // Preserve local UI state (selected_camera, toggles) but update data
                if local.selected_camera.is_some() {
                    if !local.cameras.iter().any(|c| c.entity == local.selected_camera.unwrap()) {
                        local.selected_camera = None;
                    }
                }
            }
        }

        let theme = world.get_resource::<ThemeManager>().map(|tm| tm.active_theme.clone()).unwrap_or_default();

        if let Ok(mut local) = self.local.write() {
            panels::camera::render_camera_debug_content(ui, &mut local, &theme);
        }

        if let Ok(mut pending) = self.bridge.lock() {
            if let Ok(local) = self.local.read() {
                pending.camera = Some(CameraDebugState {
                    cameras: local.cameras.clone(),
                    selected_camera: local.selected_camera,
                    show_frustum_gizmos: local.show_frustum_gizmos,
                    show_camera_axes: local.show_camera_axes,
                    show_all_frustums: local.show_all_frustums,
                    frustum_color: local.frustum_color,
                    update_interval: local.update_interval,
                    time_since_update: local.time_since_update,
                });
            }
        }
    }
}

// ============================================================================
// Culling Debug Panel
// ============================================================================

struct CullingDebugPanel {
    bridge: Arc<Mutex<DebugBridgeInner>>,
    local: RwLock<CullingDebugState>,
}

impl CullingDebugPanel {
    fn new(bridge: Arc<Mutex<DebugBridgeInner>>) -> Self {
        Self { bridge, local: RwLock::new(CullingDebugState::default()) }
    }
}

impl EditorPanel for CullingDebugPanel {
    fn id(&self) -> &str { "culling_debug" }
    fn title(&self) -> &str { "Culling Debug" }
    fn icon(&self) -> Option<&str> { Some(egui_phosphor::regular::EYE_SLASH) }
    fn default_location(&self) -> PanelLocation { PanelLocation::Right }
    fn min_size(&self) -> [f32; 2] { [200.0, 150.0] }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        if let Some(state) = world.get_resource::<CullingDebugState>() {
            if let Ok(mut local) = self.local.write() {
                // Update stats from world, preserve settings
                local.total_entities = state.total_entities;
                local.frustum_visible = state.frustum_visible;
                local.frustum_culled = state.frustum_culled;
                local.distance_culled = state.distance_culled;
                local.distance_faded = state.distance_faded;
                local.distance_buckets = state.distance_buckets;
            }
        }

        let theme = world.get_resource::<ThemeManager>().map(|tm| tm.active_theme.clone()).unwrap_or_default();

        if let Ok(mut local) = self.local.write() {
            panels::culling::render_culling_debug_content(ui, &mut local, &theme);
        }

        if let Ok(mut pending) = self.bridge.lock() {
            if let Ok(local) = self.local.read() {
                pending.culling = Some(CullingDebugState {
                    enabled: local.enabled,
                    max_distance: local.max_distance,
                    fade_start_fraction: local.fade_start_fraction,
                    total_entities: local.total_entities,
                    frustum_visible: local.frustum_visible,
                    frustum_culled: local.frustum_culled,
                    distance_culled: local.distance_culled,
                    distance_faded: local.distance_faded,
                    distance_buckets: local.distance_buckets,
                    update_interval: local.update_interval,
                    time_since_update: local.time_since_update,
                });
            }
        }
    }
}

// ============================================================================
// Performance Panel
// ============================================================================

struct PerformancePanel;

impl EditorPanel for PerformancePanel {
    fn id(&self) -> &str { "performance" }
    fn title(&self) -> &str { "Performance" }
    fn icon(&self) -> Option<&str> { Some(egui_phosphor::regular::SPEEDOMETER) }
    fn default_location(&self) -> PanelLocation { PanelLocation::Bottom }
    fn min_size(&self) -> [f32; 2] { [200.0, 150.0] }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let diagnostics = world.get_resource::<DiagnosticsState>().cloned().unwrap_or_default();
        let theme = world.get_resource::<ThemeManager>().map(|tm| tm.active_theme.clone()).unwrap_or_default();
        panels::performance::render_performance_content(ui, &diagnostics, &theme);
    }
}

// ============================================================================
// Render Stats Panel
// ============================================================================

struct RenderStatsPanel;

impl EditorPanel for RenderStatsPanel {
    fn id(&self) -> &str { "render_stats" }
    fn title(&self) -> &str { "Render Stats" }
    fn icon(&self) -> Option<&str> { Some(egui_phosphor::regular::MONITOR) }
    fn default_location(&self) -> PanelLocation { PanelLocation::Bottom }
    fn min_size(&self) -> [f32; 2] { [200.0, 150.0] }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let render = world.get_resource::<RenderStats>().cloned().unwrap_or_default();
        let theme = world.get_resource::<ThemeManager>().map(|tm| tm.active_theme.clone()).unwrap_or_default();
        panels::render_stats::render_render_stats_content(ui, &render, &theme);
    }
}

// ============================================================================
// Render Pipeline Panel
// ============================================================================

struct RenderPipelinePanel {
    local_graph: RwLock<RenderPipelineGraphData>,
}

impl RenderPipelinePanel {
    fn new() -> Self {
        Self { local_graph: RwLock::new(RenderPipelineGraphData::default()) }
    }
}

impl EditorPanel for RenderPipelinePanel {
    fn id(&self) -> &str { "render_pipeline" }
    fn title(&self) -> &str { "Render Pipeline" }
    fn icon(&self) -> Option<&str> { Some(egui_phosphor::regular::FLOW_ARROW) }
    fn default_location(&self) -> PanelLocation { PanelLocation::Bottom }
    fn min_size(&self) -> [f32; 2] { [400.0, 300.0] }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        if let Some(world_graph) = world.get_resource::<RenderPipelineGraphData>() {
            if let Ok(mut local) = self.local_graph.write() {
                // Sync nodes/edges/timing from world, preserve canvas/interaction state
                if world_graph.initialized && !local.initialized {
                    *local = world_graph.clone();
                } else if world_graph.initialized {
                    // Update timing data
                    for world_node in &world_graph.nodes {
                        if let Some(&idx) = local.node_index.get(&world_node.id) {
                            local.nodes[idx].gpu_time_ms = world_node.gpu_time_ms;
                        }
                    }
                }
            }
        }

        let render = world.get_resource::<RenderStats>().cloned().unwrap_or_default();
        let theme = world.get_resource::<ThemeManager>().map(|tm| tm.active_theme.clone()).unwrap_or_default();

        if let Ok(mut local) = self.local_graph.write() {
            panels::render_pipeline::render_render_pipeline_content(ui, &mut local, &render, &theme);
        }
    }
}

// ============================================================================
// ECS Stats Panel
// ============================================================================

struct EcsStatsPanel {
    local: RwLock<EcsStatsState>,
}

impl EcsStatsPanel {
    fn new() -> Self {
        Self { local: RwLock::new(EcsStatsState::default()) }
    }
}

impl EditorPanel for EcsStatsPanel {
    fn id(&self) -> &str { "ecs_stats" }
    fn title(&self) -> &str { "ECS Stats" }
    fn icon(&self) -> Option<&str> { Some(egui_phosphor::regular::DATABASE) }
    fn default_location(&self) -> PanelLocation { PanelLocation::Bottom }
    fn min_size(&self) -> [f32; 2] { [200.0, 150.0] }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        if let Some(state) = world.get_resource::<EcsStatsState>() {
            if let Ok(mut local) = self.local.write() {
                local.entity_count = state.entity_count;
                local.entity_count_history = state.entity_count_history.clone();
                local.archetype_count = state.archetype_count;
                local.top_archetypes = state.top_archetypes.clone();
                local.component_stats = state.component_stats.clone();
                local.resources = state.resources.clone();
            }
        }

        let theme = world.get_resource::<ThemeManager>().map(|tm| tm.active_theme.clone()).unwrap_or_default();

        if let Ok(mut local) = self.local.write() {
            panels::ecs_stats::render_ecs_stats_content(ui, &mut local, &theme);
        }
    }
}

// ============================================================================
// Sync system: apply panel mutations back to world resources
// ============================================================================

fn sync_debug_bridge(
    bridge: Res<DebugBridge>,
    mut camera: ResMut<CameraDebugState>,
    mut culling: ResMut<CullingDebugState>,
) {
    if let Ok(mut pending) = bridge.pending.lock() {
        if let Some(cam) = pending.camera.take() {
            camera.selected_camera = cam.selected_camera;
            camera.show_frustum_gizmos = cam.show_frustum_gizmos;
            camera.show_camera_axes = cam.show_camera_axes;
            camera.show_all_frustums = cam.show_all_frustums;
            camera.frustum_color = cam.frustum_color;
        }
        if let Some(cull) = pending.culling.take() {
            culling.enabled = cull.enabled;
            culling.max_distance = cull.max_distance;
            culling.fade_start_fraction = cull.fade_start_fraction;
        }
    }
}

// ============================================================================
// Plugin
// ============================================================================

#[derive(Default)]
pub struct DebuggerPlugin;

impl Plugin for DebuggerPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] DebuggerPlugin");
        // Add Bevy diagnostic plugins
        app.add_plugins((
            FrameTimeDiagnosticsPlugin::default(),
            EntityCountDiagnosticsPlugin::default(),
            SystemInformationDiagnosticsPlugin::default(),
        ));

        // Init resources
        app.init_resource::<DiagnosticsState>()
            .init_resource::<RenderStats>()
            .init_resource::<SystemTimingState>()
            .init_resource::<MemoryProfilerState>()
            .init_resource::<CameraDebugState>()
            .init_resource::<CullingDebugState>()
            .init_resource::<EcsStatsState>()
            .init_resource::<RenderPipelineGraphData>();

        // Bridge
        let bridge = DebugBridge::default();
        let arc = bridge.pending.clone();
        app.insert_resource(bridge);

        // Update systems
        use renzora_editor_framework::SplashState;
        app.add_systems(Update, (
            update_diagnostics_state,
            update_render_stats,
            update_memory_profiler,
            update_system_timing,
            update_camera_debug_state,
            update_culling_debug_state,
            update_render_pipeline_timing,
            sync_debug_bridge,
        ).run_if(in_state(SplashState::Editor)));
        app.add_systems(Update, update_ecs_stats.run_if(in_state(SplashState::Editor)));

        app.register_panel(SystemProfilerPanel);
        app.register_panel(MemoryProfilerPanel);
        app.register_panel(PerformancePanel);
        app.register_panel(RenderStatsPanel);
        app.register_panel(RenderPipelinePanel::new());
        app.register_panel(EcsStatsPanel::new());
        app.register_panel(CameraDebugPanel::new(arc.clone()));
        app.register_panel(CullingDebugPanel::new(arc));
    }

    fn finish(&self, app: &mut App) {
        extract_render_graph(app);
    }
}

renzora::add!(DebuggerPlugin, Editor);
