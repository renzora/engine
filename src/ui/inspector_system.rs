//! Exclusive system for full editor UI with World access
//!
//! This module provides an exclusive system wrapper for the editor UI that
//! allows the inspector panel to use registry-based component iteration.

use bevy::ecs::system::SystemState;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiTextureHandle};

use crate::component_system::{AddComponentPopupState, ComponentRegistry};
use crate::core::{
    AppState, AssetBrowserState, EditorEntity, SelectionState, ThumbnailCache,
    HierarchyState, ViewportState, SceneManagerState, EditorSettings, WindowState,
    OrbitCameraState, PlayModeState, ConsoleState, DockingState, InputFocusState,
    DiagnosticsState, RenderStats, EcsStatsState, MemoryProfilerState, PhysicsDebugState,
    CameraDebugState, SystemTimingState, AnimationTimelineState, KeyBindings,
    AssetLoadingProgress, DefaultCameraEntity, ImagePreviewTextures, TabKind,
};
use crate::gizmo::{GizmoState, ModalTransformState};
use crate::viewport::{Camera2DState, ModelPreviewCache, CameraPreviewImage, ViewportImage};
use crate::brushes::{BrushSettings, BrushState, BlockEditState};
use crate::terrain::{TerrainSettings, TerrainSculptState};
use crate::update::{UpdateState, UpdateDialogState};
use crate::commands::CommandHistory;
use crate::plugin_core::PluginHost;
use crate::theming::ThemeManager;
use crate::project::{AppConfig, CurrentProject};
use crate::scripting::{ScriptRegistry, RhaiScriptEngine};
use crate::blueprint::{BlueprintEditorState, BlueprintCanvasState, MaterialPreviewState, MaterialPreviewImage, nodes::NodeRegistry as BlueprintNodeRegistry};
use crate::particles::ParticleEditorState;
use crate::ui_api::renderer::UiRenderer;

use super::panels::{
    render_inspector_content_world, render_hierarchy_content, render_assets_content,
    render_console_content, render_history_content, HierarchyQueries, AnimationPanelState,
    NodeExplorerState,
};
use super::docking::{render_dock_tree, render_panel_frame, calculate_panel_rects_with_adjustments, DockedPanelContext, PanelId, DropZone, SplitDirection};

/// Type alias for the complex SystemState needed by editor UI
type EditorUiParams<'w, 's> = (
    EguiContexts<'w, 's>,
    ResMut<'w, SelectionState>,
    ResMut<'w, HierarchyState>,
    ResMut<'w, ViewportState>,
    ResMut<'w, SceneManagerState>,
    ResMut<'w, AssetBrowserState>,
    ResMut<'w, EditorSettings>,
    ResMut<'w, WindowState>,
    ResMut<'w, GizmoState>,
    Res<'w, ModalTransformState>,
    ResMut<'w, OrbitCameraState>,
    Res<'w, Camera2DState>,
    ResMut<'w, KeyBindings>,
    ResMut<'w, PluginHost>,
    Res<'w, AssetLoadingProgress>,
    Res<'w, DefaultCameraEntity>,
    ResMut<'w, PlayModeState>,
    ResMut<'w, ConsoleState>,
    ResMut<'w, CommandHistory>,
    ResMut<'w, ThumbnailCache>,
    ResMut<'w, ModelPreviewCache>,
    ResMut<'w, ImagePreviewTextures>,
    Res<'w, ComponentRegistry>,
    ResMut<'w, AddComponentPopupState>,
    Res<'w, ButtonInput<KeyCode>>,
    ResMut<'w, DockingState>,
    ResMut<'w, BlueprintEditorState>,
    ResMut<'w, BlueprintCanvasState>,
    Res<'w, BlueprintNodeRegistry>,
    ResMut<'w, MaterialPreviewState>,
    ResMut<'w, ThemeManager>,
    ResMut<'w, InputFocusState>,
    Res<'w, DiagnosticsState>,
    Res<'w, RenderStats>,
    ResMut<'w, EcsStatsState>,
    Res<'w, MemoryProfilerState>,
    ResMut<'w, PhysicsDebugState>,
    ResMut<'w, CameraDebugState>,
    Res<'w, SystemTimingState>,
    ResMut<'w, BrushSettings>,
    ResMut<'w, BrushState>,
    Res<'w, BlockEditState>,
    ResMut<'w, TerrainSettings>,
    Res<'w, TerrainSculptState>,
    ResMut<'w, UpdateState>,
    ResMut<'w, UpdateDialogState>,
    ResMut<'w, AppConfig>,
    ResMut<'w, AnimationTimelineState>,
    ResMut<'w, ParticleEditorState>,
    Option<Res<'w, CurrentProject>>,
    Res<'w, ScriptRegistry>,
    Res<'w, RhaiScriptEngine>,
    Res<'w, State<AppState>>,
    Option<Res<'w, ViewportImage>>,
    Option<Res<'w, CameraPreviewImage>>,
    Option<Res<'w, MaterialPreviewImage>>,
);

/// Cached SystemState for editor UI - stored as a resource to persist across frames
#[derive(Resource)]
struct EditorUiSystemState {
    state: SystemState<EditorUiParams<'static, 'static>>,
    local_state: (UiRenderer, AnimationPanelState, NodeExplorerState),
}

impl FromWorld for EditorUiSystemState {
    fn from_world(world: &mut World) -> Self {
        Self {
            state: SystemState::new(world),
            local_state: Default::default(),
        }
    }
}

/// Initialize the editor UI system state
pub fn init_editor_ui_state(world: &mut World) {
    if !world.contains_resource::<EditorUiSystemState>() {
        let state = EditorUiSystemState::from_world(world);
        world.insert_resource(state);
    }
}

/// Check if we should run the editor UI (only in Editor state)
fn should_run_editor_ui(world: &World) -> bool {
    world
        .get_resource::<State<AppState>>()
        .map(|s| *s.get() == AppState::Editor)
        .unwrap_or(false)
}

/// Exclusive system that renders the editor UI with full World access
/// This allows the inspector to use registry-based component iteration
pub fn editor_ui_exclusive(world: &mut World) {
    // Check if we should run
    if !should_run_editor_ui(world) {
        return;
    }

    // Initialize state if needed
    init_editor_ui_state(world);

    // Get the system state - we need to remove and re-insert to satisfy borrow checker
    let mut system_state = world
        .remove_resource::<EditorUiSystemState>()
        .expect("EditorUiSystemState should be initialized");

    // Run the actual UI rendering
    render_editor_ui_with_world(world, &mut system_state);

    // Put the state back
    world.insert_resource(system_state);
}

/// The actual editor UI rendering logic with World access
fn render_editor_ui_with_world(world: &mut World, system_state: &mut EditorUiSystemState) {
    // Extract typed access using SystemState
    let (
        mut contexts,
        mut selection,
        mut hierarchy,
        mut viewport,
        mut scene_state,
        mut assets,
        mut settings,
        mut window_state,
        mut gizmo,
        modal_transform,
        mut orbit,
        camera2d_state,
        mut keybindings,
        mut plugin_host,
        loading_progress,
        default_camera,
        mut play_mode,
        mut console,
        mut command_history,
        mut thumbnail_cache,
        mut model_preview_cache,
        mut image_preview_textures,
        component_registry,
        mut add_component_popup,
        keyboard,
        mut docking,
        mut blueprint_editor,
        mut blueprint_canvas,
        blueprint_nodes,
        mut material_preview,
        mut theme_manager,
        mut input_focus,
        diagnostics,
        render_stats,
        mut ecs_stats,
        memory_profiler,
        mut physics_debug,
        mut camera_debug,
        system_timing,
        mut brush_settings,
        mut brush_state,
        block_edit,
        mut terrain_settings,
        terrain_sculpt_state,
        mut update_state,
        mut update_dialog,
        mut app_config,
        mut animation_timeline,
        mut particle_editor_state,
        current_project,
        script_registry,
        rhai_engine,
        _app_state,
        viewport_image,
        camera_preview_image,
        material_preview_image,
    ) = system_state.state.get_mut(world);

    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    // Register texture IDs
    let viewport_texture_id = viewport_image.as_ref().map(|img| {
        contexts.image_id(&img.0).unwrap_or_else(|| {
            contexts.add_image(EguiTextureHandle::Weak(img.0.id()))
        })
    });

    let camera_preview_texture_id = camera_preview_image.as_ref().map(|img| {
        contexts.image_id(&img.0).unwrap_or_else(|| {
            contexts.add_image(EguiTextureHandle::Weak(img.0.id()))
        })
    });

    let _material_preview_texture_id = material_preview_image.as_ref().map(|img| {
        contexts.image_id(&img.0).unwrap_or_else(|| {
            contexts.add_image(EguiTextureHandle::Weak(img.0.id()))
        })
    });

    // Update input focus
    input_focus.egui_wants_keyboard = ctx.wants_keyboard_input();

    // Apply theme
    super::style::apply_editor_style_with_theme(ctx, &theme_manager.active_theme, settings.font_size);

    // ... (rest of the UI rendering would go here)
    // For now, this is a skeleton - the full implementation would mirror editor_ui

    // The key difference: For the inspector panel, we can now pass &mut world
    // to render_inspector_content_world

    let _ = (
        selection, hierarchy, viewport, scene_state, assets, settings,
        window_state, gizmo, modal_transform, orbit, camera2d_state,
        keybindings, plugin_host, loading_progress, default_camera,
        play_mode, console, command_history, thumbnail_cache,
        model_preview_cache, image_preview_textures, component_registry,
        add_component_popup, keyboard, docking, blueprint_editor,
        blueprint_canvas, blueprint_nodes, material_preview, theme_manager,
        diagnostics, render_stats, ecs_stats, memory_profiler,
        physics_debug, camera_debug, system_timing, brush_settings,
        brush_state, block_edit, terrain_settings, terrain_sculpt_state,
        update_state, update_dialog, app_config, animation_timeline,
        particle_editor_state, current_project, script_registry,
        rhai_engine, viewport_texture_id, camera_preview_texture_id,
    );
}
