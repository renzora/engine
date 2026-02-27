mod app_state;
mod components;
mod keybindings;
pub mod resources;

pub use app_state::{AppState, AssetLoadingProgress, RuntimeConfig, format_bytes};
pub use components::{AudioListenerMarker, DisabledComponents, EditorEntity, MainCamera, NodeIcon, SceneNode, SceneTabId, ViewportCamera, WorldEnvironmentMarker};
pub use keybindings::{EditorAction, KeyBinding, KeyBindings, bindable_keys};

// Re-export all resources
pub use resources::{
    AnimationTimelineState, TimelinePlayState,
    AssetBrowserState, AssetViewMode, BottomPanelTab, BuildError, BuildState, ColliderImportType, ExportDialogState,
    CollisionGizmoVisibility, ConsoleState, ConvertAxes, DefaultCameraEntity, DiagnosticsPlugin, DiagnosticsState, ImportFileResult, ImportStatus,
    DockingState, EditorSettings, MonoFont, RenderStats, CameraSettings, UiFont,
    GamepadDebugState, GamepadInfo, GamepadButtonState, update_gamepad_debug_state,
    HierarchyDropPosition, HierarchyDropTarget, HierarchyState, InputFocusState, InspectorPanelRenderState, LogEntry, LogLevel, MeshHandling,
    ModelImportSettings, NormalImportMethod, OpenImage, OpenScript, PendingImageDrop, PendingMaterialDrop,
    OrbitCameraState, PlayModeCamera, PlayModeState, PlayState, ProjectionMode, RenderToggles, RightPanelTab, SceneManagerState,
    SceneTab, ScriptError, SelectionHighlightMode, SelectionState, SettingsTab, TabCameraState, TabKind, TangentImportMethod,
    ThumbnailCache, ImagePreviewTextures, supports_thumbnail, supports_model_preview, supports_shader_thumbnail,
    ViewportMode, ViewportState, VisualizationMode, WindowState, ResizeEdge,
    // New debug/profiler resources
    EcsStatsState, MemoryProfilerState, MemoryTrend,
    SystemTimingState,
    PhysicsDebugState, ColliderShapeType,
    CameraDebugState, CameraProjectionType,
    PhysicsPropertiesState, PlaygroundState, PlaygroundEntity, PhysicsForcesState,
    PhysicsMetricsState, PhysicsScenariosState, CollisionVizState,
    MovementTrailsState, StressTestState, StateRecorderState, ArenaPresetsState, RenderPipelineGraphData,
    CullingDebugState, DistanceCulled, update_culling_debug_state, distance_culling_system,
    // Document types for various editors
    OpenVideo, OpenAudio, OpenAnimation, OpenTexture, OpenParticleFX, OpenLevel, OpenTerrain,
};

// Re-export gizmo types from the gizmo module (they were moved there)
pub use crate::gizmo::GizmoState;

use bevy::prelude::*;
use crate::console_info;

pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        // Initialize split resources
        app.init_resource::<SelectionState>()
            .init_resource::<ViewportState>()
            .init_resource::<OrbitCameraState>()
            .init_resource::<HierarchyState>()
            .init_resource::<WindowState>()
            .init_resource::<SceneManagerState>()
            .init_resource::<AssetBrowserState>()
            .init_resource::<EditorSettings>()
            .init_resource::<KeyBindings>()
            .init_resource::<AssetLoadingProgress>()
            .init_resource::<DefaultCameraEntity>()
            .init_resource::<PlayModeState>()
            .init_resource::<ConsoleState>()
            .init_resource::<ThumbnailCache>()
            .init_resource::<ImagePreviewTextures>()
            .init_resource::<DockingState>()
            .init_resource::<InputFocusState>()
            // ShaderPreviewState is registered by ShaderPreviewPlugin
            .init_resource::<GamepadDebugState>()
            .init_resource::<renzora_theme::ThemeManager>()
            .init_resource::<crate::ui::ShapeLibraryState>()
            .init_resource::<resources::InspectorPanelRenderState>()
            .init_resource::<CullingDebugState>()
            .insert_resource(AnimationTimelineState::new())
            .add_systems(Update, (
                track_asset_loading,
                drain_console_buffer,
                update_gamepad_debug_state,
                update_culling_debug_state,
                distance_culling_system,
            ).run_if(in_state(AppState::Editor)));
    }
}

/// System to drain the global console buffer into the ConsoleState resource
fn drain_console_buffer(
    mut console: ResMut<ConsoleState>,
    time: Res<Time>,
) {
    console.drain_shared_buffer(time.elapsed_secs_f64());
}

/// System that tracks asset loading progress via AssetServer
fn track_asset_loading(
    asset_server: Res<AssetServer>,
    mut loading_progress: ResMut<AssetLoadingProgress>,
) {
    use bevy::asset::LoadState;

    if loading_progress.tracking.is_empty() {
        loading_progress.loading = false;
        return;
    }

    // Find assets that have finished loading this frame
    let mut finished_ids = Vec::new();
    let mut newly_loaded_bytes = 0u64;

    for (id, info) in loading_progress.tracking.iter() {
        match asset_server.get_load_state(*id) {
            Some(LoadState::Loaded) => {
                newly_loaded_bytes += info.size_bytes;
                finished_ids.push(*id);
            }
            Some(LoadState::Failed(_)) => {
                // Count failed as "loaded" for progress purposes
                newly_loaded_bytes += info.size_bytes;
                finished_ids.push(*id);
            }
            _ => {
                // Still loading or not loaded
            }
        }
    }

    // Update loaded counts with newly finished assets
    loading_progress.loaded += finished_ids.len();
    loading_progress.loaded_bytes += newly_loaded_bytes;

    // Remove finished assets from tracking
    for id in finished_ids {
        loading_progress.tracking.remove(&id);
    }

    // Update loading state
    loading_progress.loading = !loading_progress.tracking.is_empty();

    // Reset when done loading
    if !loading_progress.loading {
        loading_progress.loaded_bytes = 0;
        loading_progress.total_bytes = 0;
        loading_progress.loaded = 0;
        loading_progress.total = 0;
    }
}

