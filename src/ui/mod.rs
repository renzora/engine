mod panels;
mod style;
pub mod docking;
pub mod inspectors;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiTextureHandle};
use egui_phosphor::regular::{PLAY, PAUSE, STOP};

use crate::commands::CommandHistory;
use crate::core::{
    AppState, AssetLoadingProgress, ConsoleState, DefaultCameraEntity, DiagnosticsState, DockingState,
    EditorEntity, GamepadDebugState, ImagePreviewTextures, InputFocusState, InspectorPanelRenderState,
    KeyBindings, RenderStats, SelectionState, HierarchyState, ViewportState,
    SceneManagerState, AssetBrowserState, EditorSettings, WindowState, OrbitCameraState,
    PlayModeState, PlayState, ThumbnailCache, ResizeEdge,
    EcsStatsState, MemoryProfilerState, PhysicsDebugState, CameraDebugState, CullingDebugState, SystemTimingState,
    AnimationTimelineState, TabKind, RenderPipelineGraphData,
    PhysicsPropertiesState, PlaygroundState, PhysicsForcesState,
    PhysicsMetricsState, PhysicsScenariosState, CollisionVizState,
    MovementTrailsState, StressTestState, StateRecorderState, ArenaPresetsState,
};
use crate::gizmo::{GizmoState, ModalTransformState};
use crate::viewport::{Camera2DState, ModelPreviewCache, ParticlePreviewImage};
use crate::shader_thumbnail::ShaderThumbnailCache;
use crate::brushes::{BrushSettings, BrushState, BlockEditState};
use crate::terrain::{TerrainSettings, TerrainSculptState};
use crate::surface_painting::{SurfacePaintSettings, SurfacePaintState};
use crate::update::{UpdateState, UpdateDialogState};
use crate::component_system::{ComponentRegistry, AddComponentPopupState};
use panels::HierarchyQueries;
use crate::project::{AppConfig, CurrentProject};
use crate::scripting::{ScriptRegistry, RhaiScriptEngine};
use crate::viewport::{CameraPreviewImage, ViewportImage};
use crate::plugin_core::PluginHost;
use crate::blueprint::{BlueprintEditorState, BlueprintCanvasState, MaterialPreviewState, MaterialPreviewImage, nodes::NodeRegistry as BlueprintNodeRegistry, serialization::BlueprintFile};
use crate::ui_api::renderer::UiRenderer;
use crate::ui_api::UiEvent as InternalUiEvent;
use docking::{
    get_legacy_layout_values, render_dock_tree, render_panel_frame,
    calculate_panel_rects_with_adjustments, DockedPanelContext, PanelId, DropZone, SplitDirection,
};
use bevy_egui::egui::{Rect, Pos2};
use panels::{
    render_plugin_panels,
    render_document_tabs, render_code_editor_content, render_image_preview_content,
    render_splash, render_status_bar, render_title_bar, render_viewport,
    TITLE_BAR_HEIGHT,
    render_hierarchy_content, render_assets_content, render_assets_dialogs,
    render_console_content, render_history_content, render_gamepad_content, render_performance_content,
    render_render_stats_content, render_ecs_stats_content, render_memory_profiler_content,
    render_physics_debug_content, render_camera_debug_content, render_system_profiler_content,
    render_level_tools_content, render_animation_content, AnimationPanelState,
    render_particle_editor_content,
    render_particle_preview_content,
    render_shader_preview_content,
    render_physics_playground_content,
    render_physics_properties_content,
    render_physics_forces_content,
    render_physics_metrics_content,
    render_physics_scenarios_content,
    render_collision_viz_content,
    render_movement_trails_content,
    render_stress_test_content,
    render_state_recorder_content,
    render_arena_presets_content,
    render_shape_library_content,
    render_culling_debug_content,
};
use crate::particles::ParticleEditorState;
use crate::shader_preview::{ShaderPreviewState, ShaderPreviewRender, ShaderType};
#[allow(unused_imports)]
pub use panels::{handle_window_actions, property_row, inline_property, LABEL_WIDTH, get_inspector_theme, InspectorThemeColors, load_effect_from_file, save_effect_to_file, ShapeLibraryState};
use style::{apply_editor_style_with_theme, init_fonts, set_ui_font};
use renzora_theme::ThemeManager;

/// Bundled editor state resources for system parameter limits
#[derive(SystemParam)]
pub struct EditorResources<'w> {
    pub selection: ResMut<'w, SelectionState>,
    pub hierarchy: ResMut<'w, HierarchyState>,
    pub viewport: ResMut<'w, ViewportState>,
    pub scene_state: ResMut<'w, SceneManagerState>,
    pub assets: ResMut<'w, AssetBrowserState>,
    pub settings: ResMut<'w, EditorSettings>,
    pub window_state: ResMut<'w, WindowState>,
    pub gizmo: ResMut<'w, GizmoState>,
    pub modal_transform: Res<'w, ModalTransformState>,
    pub orbit: ResMut<'w, OrbitCameraState>,
    pub camera2d_state: Res<'w, Camera2DState>,
    pub keybindings: ResMut<'w, KeyBindings>,
    pub plugin_host: ResMut<'w, PluginHost>,
    pub loading_progress: Res<'w, AssetLoadingProgress>,
    pub default_camera: Res<'w, DefaultCameraEntity>,
    pub play_mode: ResMut<'w, PlayModeState>,
    pub console: ResMut<'w, ConsoleState>,
    pub command_history: ResMut<'w, CommandHistory>,
    pub thumbnail_cache: ResMut<'w, ThumbnailCache>,
    pub model_preview_cache: ResMut<'w, ModelPreviewCache>,
    pub image_preview_textures: ResMut<'w, ImagePreviewTextures>,
    pub component_registry: Res<'w, ComponentRegistry>,
    pub add_component_popup: ResMut<'w, AddComponentPopupState>,
    pub keyboard: Res<'w, ButtonInput<KeyCode>>,
    pub docking: ResMut<'w, DockingState>,
    pub blueprint_editor: ResMut<'w, BlueprintEditorState>,
    pub blueprint_canvas: ResMut<'w, BlueprintCanvasState>,
    pub blueprint_nodes: Res<'w, BlueprintNodeRegistry>,
    pub material_preview: ResMut<'w, MaterialPreviewState>,
    pub theme_manager: ResMut<'w, ThemeManager>,
    pub input_focus: ResMut<'w, InputFocusState>,
    pub gamepad_debug: Res<'w, GamepadDebugState>,
    pub diagnostics: Res<'w, DiagnosticsState>,
    pub render_stats: Res<'w, RenderStats>,
    pub ecs_stats: ResMut<'w, EcsStatsState>,
    pub memory_profiler: Res<'w, MemoryProfilerState>,
    pub physics_debug: ResMut<'w, PhysicsDebugState>,
    pub camera_debug: ResMut<'w, CameraDebugState>,
    pub system_timing: Res<'w, SystemTimingState>,
    pub brush_settings: ResMut<'w, BrushSettings>,
    #[allow(dead_code)]
    pub brush_state: ResMut<'w, BrushState>,
    pub block_edit: Res<'w, BlockEditState>,
    pub terrain_settings: ResMut<'w, TerrainSettings>,
    pub terrain_sculpt_state: Res<'w, TerrainSculptState>,
    pub surface_paint_settings: ResMut<'w, SurfacePaintSettings>,
    pub surface_paint_state: ResMut<'w, SurfacePaintState>,
    pub update_state: ResMut<'w, UpdateState>,
    pub update_dialog: ResMut<'w, UpdateDialogState>,
    pub app_config: ResMut<'w, AppConfig>,
    pub animation_timeline: ResMut<'w, AnimationTimelineState>,
    pub particle_preview_image: Option<Res<'w, ParticlePreviewImage>>,
    pub particle_editor_state: ResMut<'w, ParticleEditorState>,
    pub inspector_render_state: ResMut<'w, InspectorPanelRenderState>,
    pub shader_preview: ResMut<'w, ShaderPreviewState>,
    pub shader_preview_render: Res<'w, ShaderPreviewRender>,
    pub shader_thumbnail_cache: ResMut<'w, ShaderThumbnailCache>,
    pub physics_properties: ResMut<'w, PhysicsPropertiesState>,
    pub physics_playground: ResMut<'w, PlaygroundState>,
    pub physics_forces: ResMut<'w, PhysicsForcesState>,
    pub physics_metrics: ResMut<'w, PhysicsMetricsState>,
    pub physics_scenarios: ResMut<'w, PhysicsScenariosState>,
    pub collision_viz: ResMut<'w, CollisionVizState>,
    pub movement_trails: ResMut<'w, MovementTrailsState>,
    pub stress_test: ResMut<'w, StressTestState>,
    pub state_recorder: ResMut<'w, StateRecorderState>,
    pub arena_presets: ResMut<'w, ArenaPresetsState>,
    pub render_pipeline_data: ResMut<'w, RenderPipelineGraphData>,
    pub shape_library: ResMut<'w, ShapeLibraryState>,
    pub culling_debug: ResMut<'w, CullingDebugState>,
}

/// Convert internal UiEvent to plugin API UiEvent
fn convert_ui_event_to_api(event: InternalUiEvent) -> editor_plugin_api::events::UiEvent {
    use editor_plugin_api::events::UiEvent as ApiUiEvent;
    use editor_plugin_api::ui::UiId as ApiUiId;

    match event {
        InternalUiEvent::ButtonClicked(id) => ApiUiEvent::ButtonClicked(ApiUiId(id.0)),
        InternalUiEvent::CheckboxToggled { id, checked } => ApiUiEvent::CheckboxToggled { id: ApiUiId(id.0), checked },
        InternalUiEvent::SliderChanged { id, value } => ApiUiEvent::SliderChanged { id: ApiUiId(id.0), value },
        InternalUiEvent::SliderIntChanged { id, value } => ApiUiEvent::SliderIntChanged { id: ApiUiId(id.0), value },
        InternalUiEvent::TextInputChanged { id, value } => ApiUiEvent::TextInputChanged { id: ApiUiId(id.0), value },
        InternalUiEvent::TextInputSubmitted { id, value } => ApiUiEvent::TextInputSubmitted { id: ApiUiId(id.0), value },
        InternalUiEvent::DropdownSelected { id, index } => ApiUiEvent::DropdownSelected { id: ApiUiId(id.0), index },
        InternalUiEvent::ColorChanged { id, color } => ApiUiEvent::ColorChanged { id: ApiUiId(id.0), color },
        InternalUiEvent::TreeNodeToggled { id, expanded } => ApiUiEvent::TreeNodeToggled { id: ApiUiId(id.0), expanded },
        InternalUiEvent::TreeNodeSelected(id) => ApiUiEvent::TreeNodeSelected(ApiUiId(id.0)),
        InternalUiEvent::TabSelected { id, index } => ApiUiEvent::TabSelected { id: ApiUiId(id.0), index },
        InternalUiEvent::TabClosed { id, index } => ApiUiEvent::TabClosed { id: ApiUiId(id.0), index },
        InternalUiEvent::TableRowSelected { id, row } => ApiUiEvent::TableRowSelected { id: ApiUiId(id.0), row },
        InternalUiEvent::TableSortChanged { id, column, ascending } => ApiUiEvent::TableSortChanged { id: ApiUiId(id.0), column, ascending },
        InternalUiEvent::CustomEvent { type_id, data } => ApiUiEvent::CustomEvent { type_id, data },
        // Events that don't have direct mapping - convert to custom event
        InternalUiEvent::TableRowActivated { id, row } => ApiUiEvent::CustomEvent {
            type_id: "TableRowActivated".to_string(),
            data: format!("{}:{}", id.0, row).into_bytes(),
        },
        InternalUiEvent::ContextMenuRequested { id, position } => ApiUiEvent::CustomEvent {
            type_id: "ContextMenuRequested".to_string(),
            data: format!("{}:{}:{}", id.0, position[0], position[1]).into_bytes(),
        },
        InternalUiEvent::DragStarted { id } => ApiUiEvent::CustomEvent {
            type_id: "DragStarted".to_string(),
            data: format!("{}", id.0).into_bytes(),
        },
        InternalUiEvent::DragEnded { id, target } => ApiUiEvent::CustomEvent {
            type_id: "DragEnded".to_string(),
            data: format!("{}:{}", id.0, target.map(|t| t.0).unwrap_or(u64::MAX)).into_bytes(),
        },
        InternalUiEvent::ItemDropped { target_id, source_id } => ApiUiEvent::CustomEvent {
            type_id: "ItemDropped".to_string(),
            data: format!("{}:{}", target_id.0, source_id.0).into_bytes(),
        },
        InternalUiEvent::PanelTabSelected { location, tab_id } => ApiUiEvent::CustomEvent {
            type_id: "PanelTabSelected".to_string(),
            data: format!("{}:{}", location, tab_id).into_bytes(),
        },
    }
}

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, _app: &mut App) {
        // Systems are registered in main.rs to maintain proper ordering
    }
}

/// Splash screen UI system (runs in Splash state)
pub fn splash_ui(
    mut contexts: EguiContexts,
    mut window_state: ResMut<WindowState>,
    mut settings: ResMut<EditorSettings>,
    mut app_config: ResMut<AppConfig>,
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
    mut fonts_initialized: Local<bool>,
    theme_manager: Res<ThemeManager>,
) {
    let Ok(ctx) = contexts.ctx_mut() else { return };

    // Initialize fonts once
    if !*fonts_initialized {
        init_fonts(ctx);
        *fonts_initialized = true;
    }

    render_splash(
        ctx,
        &mut window_state,
        &mut settings,
        &mut app_config,
        &mut commands,
        &mut next_state,
        &theme_manager.active_theme,
    );
}

/// Editor UI system (runs in Editor state)
#[allow(clippy::too_many_arguments)]
pub fn editor_ui(
    mut contexts: EguiContexts,
    mut editor: EditorResources,
    mut commands: Commands,
    mut hierarchy_queries: HierarchyQueries,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    current_project: Option<Res<CurrentProject>>,
    _script_registry: Res<ScriptRegistry>,
    rhai_engine: Res<RhaiScriptEngine>,
    app_state: Res<State<AppState>>,
    viewport_image: Option<Res<ViewportImage>>,
    camera_preview_image: Option<Res<CameraPreviewImage>>,
    material_preview_image: Option<Res<MaterialPreviewImage>>,
    time: Res<Time>,
    mut local_state: Local<(UiRenderer, AnimationPanelState, panels::NodeExplorerState)>,
    mut fonts_applied: Local<bool>,
) {
    // Only run in Editor state (run_if doesn't work with EguiPrimaryContextPass)
    if *app_state.get() != AppState::Editor {
        return;
    }

    // Reset inspector render state for this frame
    editor.inspector_render_state.reset();

    // Register and get viewport texture ID from egui
    let viewport_texture_id = viewport_image.as_ref().map(|img| {
        // Try to get existing ID, or add it if not yet registered
        contexts.image_id(&img.0).unwrap_or_else(|| {
            contexts.add_image(EguiTextureHandle::Weak(img.0.id()))
        })
    });

    // Register and get camera preview texture ID from egui
    let camera_preview_texture_id = camera_preview_image.as_ref().map(|img| {
        contexts.image_id(&img.0).unwrap_or_else(|| {
            contexts.add_image(EguiTextureHandle::Weak(img.0.id()))
        })
    });

    // Register and get material preview texture ID from egui
    let material_preview_texture_id = material_preview_image.as_ref().map(|img| {
        contexts.image_id(&img.0).unwrap_or_else(|| {
            contexts.add_image(EguiTextureHandle::Weak(img.0.id()))
        })
    });

    // Register and get particle preview texture ID from egui
    let particle_preview_texture_id = editor.particle_preview_image.as_ref().and_then(|img| {
        img.texture_id.or_else(|| {
            if img.handle != bevy::prelude::Handle::default() {
                Some(contexts.add_image(EguiTextureHandle::Weak(img.handle.id())))
            } else {
                None
            }
        })
    });
    let particle_preview_size = editor.particle_preview_image.as_ref()
        .map(|img| img.size)
        .unwrap_or((256, 256));

    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    // Update input focus state - this lets other systems know if egui has keyboard focus
    editor.input_focus.egui_wants_keyboard = ctx.wants_keyboard_input();

    // Force font atlas rebuild on first editor frame. init_fonts() is called during splash inside
    // EguiPrimaryContextPass, but bevy_egui calls begin_pass() before user systems run, so the
    // first frame renders with stale/default fonts. Re-applying here guarantees the atlas is
    // rebuilt at the correct DPI and with the user's saved font preference.
    if !*fonts_applied {
        set_ui_font(ctx, editor.settings.ui_font);
        *fonts_applied = true;
    }

    apply_editor_style_with_theme(ctx, &editor.theme_manager.active_theme, editor.settings.font_size);

    // Check play mode state early
    let in_play_mode_early = editor.play_mode.is_in_play_mode();

    // Render status bar at bottom (must be rendered early to reserve space) - skip in play mode
    if !in_play_mode_early {
        render_status_bar(
            ctx,
            &editor.plugin_host,
            &editor.loading_progress,
            &mut editor.theme_manager,
            &editor.update_state,
            &mut editor.update_dialog,
        );
    }

    // Collect all UI events to forward to plugins
    let mut all_ui_events = Vec::new();

    // Check if we're in fullscreen play mode (not scripts-only)
    let in_fullscreen_play = matches!(editor.play_mode.state, PlayState::Playing | PlayState::Paused);

    // In fullscreen play mode, skip title bar entirely
    let content_start_y;

    if !in_fullscreen_play {
        // Render custom title bar (includes menu)
        let title_bar_events = render_title_bar(
            ctx,
            &mut editor.window_state,
            &mut editor.selection,
            &mut editor.scene_state,
            &mut editor.settings,
            &mut editor.assets,
            &editor.plugin_host,
            &mut editor.command_history,
            &mut editor.docking,
            &mut editor.viewport,
            &mut editor.gizmo,
            &editor.theme_manager.active_theme,
        );
        all_ui_events.extend(title_bar_events);

        content_start_y = TITLE_BAR_HEIGHT;
    } else {
        content_start_y = 0.0;
    }

    let screen_rect = ctx.content_rect();
    let status_bar_height = if in_fullscreen_play { 0.0 } else { 24.0 };

    // Suppress unused import warning
    let _ = get_legacy_layout_values;

    if in_fullscreen_play {
        // Full-screen viewport in play mode
        let central_height = (screen_rect.height() - content_start_y - status_bar_height).max(100.0);
        render_viewport(
            ctx,
            &mut editor.viewport,
            &mut editor.assets,
            &mut editor.orbit,
            &editor.camera2d_state,
            &mut editor.gizmo,
            &editor.modal_transform,
            &mut editor.settings,
            &editor.terrain_sculpt_state,
            &mut editor.terrain_settings,
            0.0,
            0.0,
            content_start_y,
            [1600.0, 900.0],
            central_height,
            viewport_texture_id,
            &editor.theme_manager.active_theme,
            &editor.selection,
            &mut editor.hierarchy,
            &mut editor.command_history,
            true, // in_play_mode
            &editor.shape_library,
            &mut editor.play_mode,
        );

        // Render play mode overlay with info
        render_play_mode_overlay(ctx, &mut editor.play_mode);

        // Render on-screen console logs over the fullscreen viewport
        let fullscreen_vp_rect = bevy_egui::egui::Rect::from_min_size(
            bevy_egui::egui::Pos2::new(0.0, content_start_y),
            bevy_egui::egui::Vec2::new(screen_rect.width(), central_height),
        );
        render_viewport_logs(ctx, &editor.console, time.elapsed_secs_f64(), fullscreen_vp_rect);
    } else {
        // DOCKING SYSTEM - Render dock tree with draggable panels

        // Render document tabs bar (for scene/script/blueprint tabs with + button)
        let document_tabs_height = render_document_tabs(
            ctx,
            &mut editor.scene_state,
            &mut editor.blueprint_editor,
            &mut editor.docking,
            0.0,  // No left margin - spans full width
            0.0,  // No right margin
            content_start_y,
            &editor.theme_manager.active_theme,
        );

        // Calculate dock area (below scene tabs, above status bar)
        let dock_start_y = content_start_y + document_tabs_height;
        // Ensure dock rect has non-negative dimensions (can happen with very small windows)
        let dock_bottom = (screen_rect.height() - status_bar_height).max(dock_start_y + 1.0);
        let dock_right = screen_rect.width().max(1.0);
        let dock_rect = Rect::from_min_max(
            Pos2::new(0.0, dock_start_y),
            Pos2::new(dock_right, dock_bottom),
        );

        // Clone drag state for rendering (avoid borrow issues)
        let drag_state = editor.docking.drag_state.clone();

        // Render dock tree structure (tabs, resize handles, backgrounds)
        // Use Order::Background so panel content at Order::Middle can receive input
        let dock_result = bevy_egui::egui::Area::new(bevy_egui::egui::Id::new("dock_tree_area"))
            .fixed_pos(dock_rect.min)
            .order(bevy_egui::egui::Order::Background)
            .show(ctx, |ui| {
                ui.set_clip_rect(dock_rect);
                ui.set_min_size(dock_rect.size());
                ui.set_max_size(dock_rect.size());
                render_dock_tree(
                    ui,
                    &editor.docking.dock_tree,
                    dock_rect,
                    drag_state.as_ref(),
                    bevy_egui::egui::Id::new("dock_tree"),
                    &editor.theme_manager.active_theme,
                )
            }).inner;

        // Update viewport state with resize handle interaction
        editor.viewport.resize_handle_active = dock_result.resize_handle_active;

        // Update drag animation progress - reset if target changed
        if let Some(ref mut drag) = editor.docking.drag_state {
            // Check if target changed and reset animation if so
            let current_target = dock_result.drop_completed.as_ref().map(|t| (t.target_panel.clone(), t.zone));
            if let Some((panel, zone)) = current_target {
                drag.update_target(Some(panel), Some(zone));
            } else {
                drag.update_target(None, None);
            }
            // Animate to 1.0 over ~10 frames for smooth transition
            drag.animation_progress = (drag.animation_progress + 0.12).min(1.0);
        }

        // Use panel rects from dock tree render result - these include animation adjustments
        // for drag/drop previews. The leaf_rects from dock_result have animated positions.
        let panel_rects = calculate_panel_rects_with_adjustments(
            &editor.docking.dock_tree,
            dock_rect,
            &dock_result.leaf_rects,
        );

        // Process dock tree events
        // Handle drag start
        if let Some(panel) = dock_result.drag_started.clone() {
            if let Some(pos) = ctx.pointer_hover_pos() {
                // Find the rect of the panel being dragged
                let panel_rect = panel_rects.iter()
                    .find(|(id, _, _)| id == &panel)
                    .map(|(_, rect, _)| *rect)
                    .unwrap_or(bevy_egui::egui::Rect::from_min_size(pos, bevy_egui::egui::Vec2::new(300.0, 200.0)));
                editor.docking.start_drag(panel, pos, panel_rect);
            }
        }

        // Handle tab close
        if let Some(panel) = dock_result.panel_to_close {
            editor.docking.close_panel(&panel);
        }

        // Handle panel add (from "+" button in tab bar)
        if let Some((leaf_panel, new_panel)) = dock_result.panel_to_add {
            editor.docking.panel_availability.open_panel(&new_panel);
            editor.docking.dock_tree.add_tab(&leaf_panel, new_panel);
        }

        // Handle active tab change
        if let Some((_, new_active)) = dock_result.new_active_tab {
            editor.docking.dock_tree.set_active_tab(&new_active);
        }

        // Handle ratio update (resize) - applied AFTER panel_rects is calculated
        // so it takes effect next frame
        if let Some((path, new_ratio)) = dock_result.ratio_update {
            editor.docking.dock_tree.update_ratio(&path, new_ratio);
            editor.docking.mark_modified();
        }

        // Handle drop completion
        if ctx.input(|i| i.pointer.any_released()) && editor.docking.drag_state.is_some() {
            let drag_panel = editor.docking.drag_state.as_ref().map(|d| d.panel.clone());
            if let Some(panel) = drag_panel {
                if let Some(edge_zone) = dock_result.edge_drop {
                    // Edge drop: wrap the entire tree with the panel on the dock border
                    editor.docking.dock_tree.remove_panel(&panel);
                    editor.docking.dock_tree.wrap_with_panel(panel, edge_zone);
                    editor.docking.mark_modified();
                } else if let Some(drop_target) = &dock_result.drop_completed {
                    // Per-panel drop: split or tab within a specific leaf
                    if panel != drop_target.target_panel {
                        editor.docking.dock_tree.remove_panel(&panel);
                        match drop_target.zone {
                            DropZone::Tab => {
                                editor.docking.dock_tree.add_tab(&drop_target.target_panel, panel);
                            }
                            DropZone::Left => {
                                editor.docking.dock_tree.split_at(&drop_target.target_panel, panel, SplitDirection::Horizontal, true);
                            }
                            DropZone::Right => {
                                editor.docking.dock_tree.split_at(&drop_target.target_panel, panel, SplitDirection::Horizontal, false);
                            }
                            DropZone::Top => {
                                editor.docking.dock_tree.split_at(&drop_target.target_panel, panel, SplitDirection::Vertical, true);
                            }
                            DropZone::Bottom => {
                                editor.docking.dock_tree.split_at(&drop_target.target_panel, panel, SplitDirection::Vertical, false);
                            }
                            // Edge zones are handled above, not here
                            DropZone::EdgeLeft | DropZone::EdgeRight | DropZone::EdgeTop | DropZone::EdgeBottom => {}
                        }
                        editor.docking.mark_modified();
                    }
                }
            }
            editor.docking.end_drag();
        }

        // Sync plugin panels into dock tree
        {
            let api = editor.plugin_host.api();
            for (panel_def, _plugin_id) in &api.panels {
                let dock_id = PanelId::Plugin(panel_def.id.clone());
                if !editor.docking.dock_tree.contains_panel(&dock_id) {
                    // Add plugin panel to dock tree based on its preferred location
                    let target = match panel_def.default_location {
                        crate::plugin_core::PanelLocation::Left => Some(PanelId::Hierarchy),
                        crate::plugin_core::PanelLocation::Right => Some(PanelId::Inspector),
                        crate::plugin_core::PanelLocation::Bottom => Some(PanelId::Assets),
                        _ => None,
                    };
                    if let Some(target_panel) = target {
                        editor.docking.dock_tree.add_tab(&target_panel, dock_id);
                    } else {
                        // Floating preference - add as tab to inspector as fallback
                        editor.docking.dock_tree.add_tab(&PanelId::Inspector, dock_id);
                    }
                    editor.docking.mark_modified();
                }
            }
        }

        // Capture drag state before panel loop — the assets panel's safety reset
        // clears dragging_asset when pointer is released, which would race with the
        // inspector panel's copy if assets renders first in the docking order.
        editor.inspector_render_state.dragging_asset_path = editor.assets.dragging_asset.clone();

        let dragging_panel = editor.docking.drag_state.as_ref().map(|d| d.panel.clone());

        for (panel_id, panel_rect, is_active) in panel_rects {
            if !is_active {
                continue; // Only render active tabs
            }

            // When dragging a panel, mute its content area and skip rendering
            if dragging_panel.as_ref() == Some(&panel_id) {
                let content_rect = bevy_egui::egui::Rect::from_min_max(
                    bevy_egui::egui::Pos2::new(panel_rect.min.x, panel_rect.min.y + 28.0),
                    panel_rect.max,
                );
                let painter = ctx.layer_painter(bevy_egui::egui::LayerId::new(
                    bevy_egui::egui::Order::Middle,
                    bevy_egui::egui::Id::new(("panel_drag_mute", panel_id.clone())),
                ));
                painter.rect_filled(content_rect, 0.0, bevy_egui::egui::Color32::from_rgba_unmultiplied(18, 20, 24, 220));
                continue;
            }

            let panel_ctx = DockedPanelContext::new(panel_rect, panel_id.clone(), is_active);

            match panel_id {
                PanelId::Hierarchy => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        // Check if a scene file is being dragged
                        let dragging_scene = editor.assets.dragging_asset.as_ref()
                            .map(|p| p.to_string_lossy().to_lowercase().ends_with(".ron"))
                            .unwrap_or(false);

                        // Check if a script/blueprint file is being dragged
                        let dragging_script = editor.assets.dragging_asset.as_ref()
                            .map(|p| {
                                let s = p.to_string_lossy().to_lowercase();
                                s.ends_with(".rhai") || s.ends_with(".blueprint")
                            })
                            .unwrap_or(false);

                        let active_tab = editor.scene_state.active_scene_tab;
                        let (events, changed) = render_hierarchy_content(
                            ui,
                            ctx,
                            &mut editor.selection,
                            &mut editor.hierarchy,
                            &hierarchy_queries,
                            &mut commands,
                            &mut meshes,
                            &mut materials,
                            &editor.component_registry,
                            active_tab,
                            &editor.plugin_host,
                            &mut editor.assets,
                            dragging_scene,
                            dragging_script,
                            &editor.default_camera,
                            &mut editor.command_history,
                            &editor.theme_manager.active_theme,
                        );
                        all_ui_events.extend(events);
                        if changed {
                            editor.scene_state.mark_modified();
                        }
                    });
                }

                PanelId::Inspector => {
                    // Render panel frame but defer content to exclusive system
                    // This allows the inspector to use registry-based component iteration
                    // with &mut World access
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |_ui| {
                        // Content will be rendered by inspector_content_exclusive system
                    });

                    // Store render state for exclusive system
                    editor.inspector_render_state.request_render(
                        panel_ctx.content_rect,
                        camera_preview_texture_id,
                        &editor.theme_manager.active_theme,
                    );
                }

                PanelId::Assets => {
                    // Store panel bounds for global file drop detection
                    let content_rect = panel_ctx.content_rect;
                    editor.assets.panel_bounds = [content_rect.min.x, content_rect.min.y, content_rect.width(), content_rect.height()];

                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_assets_content(
                            ui,
                            current_project.as_deref(),
                            &mut editor.assets,
                            &mut editor.scene_state,
                            &mut editor.thumbnail_cache,
                            &mut editor.model_preview_cache,
                            &mut editor.shader_thumbnail_cache,
                            &editor.theme_manager.active_theme,
                        );
                    });
                }

                PanelId::Console => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_console_content(ui, &mut editor.console, &editor.theme_manager.active_theme);
                    });
                }

                PanelId::History => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_history_content(ui, &mut editor.command_history);
                    });
                }

                PanelId::Viewport => {
                    // Viewport uses render_panel_frame for consistent background
                    let content_rect = panel_ctx.content_rect;
                    editor.viewport.position = [content_rect.min.x, content_rect.min.y];
                    editor.viewport.size = [content_rect.width(), content_rect.height()];

                    // Draw background to prevent any visual leaking
                    let bg_area = bevy_egui::egui::Area::new(bevy_egui::egui::Id::new("viewport_bg"))
                        .fixed_pos(content_rect.min)
                        .order(bevy_egui::egui::Order::Background)
                        .show(ctx, |ui| {
                            ui.painter().rect_filled(
                                content_rect,
                                0.0,
                                editor.theme_manager.active_theme.surfaces.panel.to_color32(),
                            );
                        });
                    let _ = bg_area;

                    render_viewport(
                        ctx,
                        &mut editor.viewport,
                        &mut editor.assets,
                        &mut editor.orbit,
                        &editor.camera2d_state,
                        &mut editor.gizmo,
                        &editor.modal_transform,
                        &mut editor.settings,
                        &editor.terrain_sculpt_state,
                        &mut editor.terrain_settings,
                        content_rect.min.x,
                        screen_rect.width() - content_rect.max.x,
                        content_rect.min.y,
                        [content_rect.width(), content_rect.height()],
                        content_rect.height(),
                        viewport_texture_id,
                        &editor.theme_manager.active_theme,
                        &editor.selection,
                        &mut editor.hierarchy,
                        &mut editor.command_history,
                        false, // not in fullscreen play mode when docked
                        &editor.shape_library,
                        &mut editor.play_mode,
                    );

                    // Render on-screen console logs in viewport (only during play mode)
                    if editor.play_mode.is_in_play_mode() {
                        let vp_rect = bevy_egui::egui::Rect::from_min_size(
                            bevy_egui::egui::Pos2::new(editor.viewport.position[0], editor.viewport.position[1]),
                            bevy_egui::egui::Vec2::new(editor.viewport.size[0], editor.viewport.size[1]),
                        );
                        render_viewport_logs(ctx, &editor.console, time.elapsed_secs_f64(), vp_rect);
                    }
                }

                PanelId::Animation => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_animation_content(
                            ui,
                            &editor.selection,
                            &mut hierarchy_queries.components.gltf_animations,
                            &hierarchy_queries.components.animations,
                            &mut local_state.1,
                            &editor.theme_manager.active_theme,
                        );
                    });
                }

                PanelId::Timeline => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        panels::render_timeline_content(
                            ui,
                            &mut editor.animation_timeline,
                            &editor.selection,
                            &hierarchy_queries.components.gltf_animations,
                            &hierarchy_queries.components.animations,
                            &editor.theme_manager.active_theme,
                        );
                    });
                }

                PanelId::CodeEditor => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_code_editor_content(
                            ui,
                            ctx,
                            &mut editor.scene_state,
                            current_project.as_deref(),
                        );
                    });
                }

                PanelId::ShaderPreview => {
                    let preview_tex = match editor.shader_preview.shader_type {
                        ShaderType::Compute => editor.shader_preview_render.compute_texture_id,
                        ShaderType::Fragment | ShaderType::PbrFragment => editor.shader_preview_render.texture_id,
                    };
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_shader_preview_content(
                            ui,
                            &mut editor.shader_preview,
                            &editor.scene_state,
                            &editor.theme_manager.active_theme,
                            preview_tex,
                        );
                    });
                }

                PanelId::Blueprint => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        panels::render_blueprint_panel(
                            ui,
                            ctx,
                            &mut editor.blueprint_editor,
                            &mut editor.blueprint_canvas,
                            &editor.blueprint_nodes,
                            current_project.as_deref(),
                            &mut editor.assets,
                            &mut editor.thumbnail_cache,
                            &mut editor.scene_state,
                        );
                    });
                }

                PanelId::NodeLibrary => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        panels::render_node_library_panel(
                            ui,
                            &mut editor.blueprint_editor,
                            &editor.blueprint_canvas,
                            &editor.blueprint_nodes,
                        );
                    });
                }

                PanelId::MaterialPreview => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        panels::render_material_preview_content(
                            ui,
                            ctx,
                            &mut editor.material_preview,
                            &editor.blueprint_editor,
                            material_preview_texture_id,
                        );
                    });
                }

                PanelId::Settings => {
                    // Clone theme to avoid borrow conflict with theme_manager mutation
                    let theme_for_frame = editor.theme_manager.active_theme.clone();
                    render_panel_frame(ctx, &panel_ctx, &theme_for_frame, |ui| {
                        panels::render_settings_content(
                            ui,
                            ctx,
                            &mut editor.settings,
                            &mut editor.keybindings,
                            &mut editor.theme_manager,
                            &mut editor.app_config,
                            &mut editor.update_state,
                            &mut editor.update_dialog,
                            &mut editor.scene_state,
                            &mut editor.plugin_host,
                        );
                    });
                }

                PanelId::Gamepad => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_gamepad_content(ui, &editor.gamepad_debug, &editor.theme_manager.active_theme);
                    });
                }

                PanelId::Performance => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_performance_content(ui, &editor.diagnostics, &editor.theme_manager.active_theme);
                    });
                }

                PanelId::RenderStats => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_render_stats_content(ui, &editor.render_stats, &editor.theme_manager.active_theme);
                    });
                }

                PanelId::RenderPipeline => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        panels::render_render_pipeline_content(
                            ui,
                            &mut editor.render_pipeline_data,
                            &editor.render_stats,
                            &editor.theme_manager.active_theme,
                        );
                    });
                }

                PanelId::ShapeLibrary => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_shape_library_content(
                            ui,
                            &mut editor.shape_library,
                            &editor.theme_manager.active_theme,
                        );
                    });
                }

                PanelId::EcsStats => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_ecs_stats_content(ui, &mut editor.ecs_stats, &editor.theme_manager.active_theme);
                    });
                }

                PanelId::MemoryProfiler => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_memory_profiler_content(ui, &editor.memory_profiler, &editor.theme_manager.active_theme);
                    });
                }

                PanelId::PhysicsDebug => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_physics_debug_content(ui, &mut editor.physics_debug, &editor.theme_manager.active_theme);
                    });
                }

                PanelId::PhysicsPlayground => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_physics_playground_content(ui, &mut editor.physics_playground, &editor.theme_manager.active_theme);
                    });
                }

                PanelId::PhysicsProperties => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_physics_properties_content(ui, &mut editor.physics_properties, &editor.theme_manager.active_theme);
                    });
                }

                PanelId::PhysicsForces => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_physics_forces_content(ui, &mut editor.physics_forces, &editor.theme_manager.active_theme);
                    });
                }

                PanelId::PhysicsMetrics => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_physics_metrics_content(ui, &mut editor.physics_metrics, &editor.theme_manager.active_theme);
                    });
                }

                PanelId::PhysicsScenarios => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_physics_scenarios_content(ui, &mut editor.physics_scenarios, &editor.theme_manager.active_theme);
                    });
                }

                PanelId::CollisionViz => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_collision_viz_content(ui, &mut editor.collision_viz, &editor.theme_manager.active_theme);
                    });
                }

                PanelId::MovementTrails => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_movement_trails_content(ui, &mut editor.movement_trails, &editor.theme_manager.active_theme);
                    });
                }

                PanelId::StressTest => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_stress_test_content(ui, &mut editor.stress_test, &editor.theme_manager.active_theme);
                    });
                }

                PanelId::StateRecorder => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_state_recorder_content(ui, &mut editor.state_recorder, &editor.theme_manager.active_theme);
                    });
                }

                PanelId::ArenaPresets => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_arena_presets_content(ui, &mut editor.arena_presets, &editor.theme_manager.active_theme);
                    });
                }

                PanelId::CameraDebug => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_camera_debug_content(ui, &mut editor.camera_debug, &editor.theme_manager.active_theme);
                    });
                }

                PanelId::CullingDebug => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_culling_debug_content(ui, &mut editor.culling_debug, &editor.theme_manager.active_theme);
                    });
                }

                PanelId::SystemProfiler => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_system_profiler_content(ui, &editor.diagnostics, &editor.system_timing, &editor.theme_manager.active_theme);
                    });
                }

                PanelId::LevelTools => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_level_tools_content(
                            ui,
                            &mut editor.gizmo,
                            &mut editor.brush_settings,
                            &editor.block_edit,
                            &mut editor.terrain_settings,
                            &mut editor.surface_paint_settings,
                            &mut editor.surface_paint_state,
                            &mut editor.assets,
                            &editor.theme_manager.active_theme,
                        );
                    });
                }

                PanelId::StudioPreview => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        if let Some(texture_id) = editor.viewport.studio_preview_texture_id {
                            panels::render_studio_preview_content(
                                ui,
                                texture_id,
                                editor.viewport.studio_preview_size,
                                &editor.theme_manager.active_theme,
                            );
                        } else {
                            ui.centered_and_justified(|ui| {
                                ui.label("Studio Preview\n\nInitializing...");
                            });
                        }
                    });
                }

                PanelId::NodeExplorer => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        // Collect node info for the selected entity
                        let node_infos = if let Some(selected) = editor.selection.selected_entity {
                            panels::collect_node_infos(
                                selected,
                                &hierarchy_queries.components.names,
                                &hierarchy_queries.components.global_transforms,
                                &hierarchy_queries.components.mesh3d_components,
                                &hierarchy_queries.components.skinned_meshes,
                                &hierarchy_queries.components.children,
                            )
                        } else {
                            std::collections::HashMap::new()
                        };

                        panels::render_node_explorer_content(
                            ui,
                            editor.selection.selected_entity,
                            &mut local_state.2,
                            &node_infos,
                            &editor.theme_manager.active_theme,
                        );
                    });
                }

                PanelId::ImagePreview => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        panels::render_image_preview_content(
                            ui,
                            ctx,
                            &mut editor.scene_state,
                            &editor.theme_manager.active_theme,
                            &mut editor.image_preview_textures,
                        );
                    });
                }

                PanelId::VideoEditor => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        ui.centered_and_justified(|ui| {
                            ui.label(bevy_egui::egui::RichText::new("Video Editor")
                                .size(18.0)
                                .color(editor.theme_manager.active_theme.text.muted.to_color32()));
                        });
                    });
                }

                PanelId::DAW => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        ui.centered_and_justified(|ui| {
                            ui.label(bevy_egui::egui::RichText::new("Digital Audio Workstation")
                                .size(18.0)
                                .color(editor.theme_manager.active_theme.text.muted.to_color32()));
                        });
                    });
                }

                PanelId::ParticleEditor => {
                    // Temporarily take the particle editor state to avoid borrow conflicts
                    let mut particle_state = std::mem::take(&mut *editor.particle_editor_state);
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_particle_editor_content(
                            ui,
                            &mut particle_state,
                            &mut editor.scene_state,
                            &mut editor.assets,
                            &editor.theme_manager.active_theme,
                        );
                    });
                    // Put the state back
                    *editor.particle_editor_state = particle_state;
                }

                PanelId::ParticlePreview => {
                    // Use local copies to avoid borrow issues in closure
                    let tex_id = particle_preview_texture_id;
                    let tex_size = particle_preview_size;
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        render_particle_preview_content(ui, tex_id, tex_size);
                    });
                }

                PanelId::TextureEditor => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        ui.centered_and_justified(|ui| {
                            ui.label(bevy_egui::egui::RichText::new("Texture Editor")
                                .size(18.0)
                                .color(editor.theme_manager.active_theme.text.muted.to_color32()));
                        });
                    });
                }

                PanelId::ScriptVariables => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        panels::render_script_variables_content(
                            ui,
                            &editor.scene_state,
                            &rhai_engine,
                            current_project.as_deref(),
                        );
                    });
                }

                PanelId::Plugin(name) => {
                    render_panel_frame(ctx, &panel_ctx, &editor.theme_manager.active_theme, |ui| {
                        let api = editor.plugin_host.api();
                        if let Some(widgets) = api.panel_contents.get(name.as_str()) {
                            for widget in widgets {
                                local_state.0.render(ui, widget);
                            }
                        } else {
                            ui.centered_and_justified(|ui| {
                                ui.label(bevy_egui::egui::RichText::new(format!("Plugin: {}", name))
                                    .color(bevy_egui::egui::Color32::GRAY));
                            });
                        }
                    });
                }
            }
        }

        // Draw cursor tooltip when dragging a tab (panel stays in place, tooltip follows cursor)
        if let Some(ref drag) = editor.docking.drag_state {
            if let Some(cursor_pos) = ctx.pointer_hover_pos() {
                let painter = ctx.layer_painter(bevy_egui::egui::LayerId::new(
                    bevy_egui::egui::Order::Tooltip,
                    bevy_egui::egui::Id::new("drag_preview_tooltip"),
                ));
                let theme = &editor.theme_manager.active_theme;
                let text = format!("{} {}", drag.panel.icon(), drag.panel.title());
                let rect = bevy_egui::egui::Rect::from_min_size(
                    cursor_pos + bevy_egui::egui::vec2(12.0, 12.0),
                    bevy_egui::egui::vec2(120.0, 28.0),
                );
                let popup_bg = theme.surfaces.popup.to_color32();
                let [r, g, b, _] = popup_bg.to_array();
                painter.rect_filled(rect, 4.0, bevy_egui::egui::Color32::from_rgba_unmultiplied(r, g, b, 230));
                painter.rect_stroke(rect, 4.0, bevy_egui::egui::Stroke::new(1.0, theme.semantic.accent.to_color32()), bevy_egui::egui::StrokeKind::Outside);
                painter.text(rect.center(), bevy_egui::egui::Align2::CENTER_CENTER, text, bevy_egui::egui::FontId::proportional(13.0), bevy_egui::egui::Color32::WHITE);
            }
        }

        // Draw drop zone overlay in foreground (on top of panel content, no panel movement)
        if let Some(overlay_rect) = dock_result.drop_overlay {
            let accent = editor.theme_manager.active_theme.semantic.accent.to_color32();
            let painter = ctx.layer_painter(bevy_egui::egui::LayerId::new(
                bevy_egui::egui::Order::Foreground,
                bevy_egui::egui::Id::new("drop_zone_overlay"),
            ));

            // Subtle fill
            painter.rect_filled(
                overlay_rect,
                4.0,
                bevy_egui::egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 45),
            );

            // Animated glow border
            let time = ctx.input(|i| i.time) as f32;
            let pulse = (time * 5.0).sin() * 0.5 + 0.5; // 0.0–1.0, ~1.6 Hz

            // Wide outer glow
            painter.rect_stroke(
                overlay_rect,
                4.0,
                bevy_egui::egui::Stroke::new(
                    8.0 + 4.0 * pulse,
                    bevy_egui::egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), (35.0 + 55.0 * pulse) as u8),
                ),
                bevy_egui::egui::StrokeKind::Outside,
            );

            // Crisp inner border
            painter.rect_stroke(
                overlay_rect,
                4.0,
                bevy_egui::egui::Stroke::new(
                    2.0,
                    bevy_egui::egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), (160.0 + 95.0 * pulse) as u8),
                ),
                bevy_egui::egui::StrokeKind::Inside,
            );

            ctx.request_repaint(); // Keep the glow animating
        }

        // Clear any remaining drag if pointer was released and no panel handled it
        if editor.assets.dragging_asset.is_some() && ctx.input(|i| i.pointer.any_released()) {
            editor.assets.dragging_asset = None;
        }

        // Handle shape library drag-to-viewport
        if editor.shape_library.dragging_shape.is_some() && ctx.input(|i| i.pointer.any_released()) {
            let pointer_pos = ctx.pointer_hover_pos();
            let vp_rect = bevy_egui::egui::Rect::from_min_size(
                bevy_egui::egui::Pos2::new(editor.viewport.position[0], editor.viewport.position[1]),
                bevy_egui::egui::Vec2::new(editor.viewport.size[0], editor.viewport.size[1]),
            );
            let in_viewport = pointer_pos.map_or(false, |p| vp_rect.contains(p));
            if in_viewport {
                if let Some(mesh_type) = editor.shape_library.dragging_shape.take() {
                    // Use surface hit position if available, otherwise ground plane
                    let drop_pos = editor.assets.drag_surface_position
                        .or(editor.assets.drag_ground_position)
                        .unwrap_or(Vec3::ZERO);
                    editor.assets.pending_shape_drop = Some((mesh_type, drop_pos));
                    editor.assets.pending_shape_drop_normal = editor.assets.drag_surface_normal;
                }
            } else {
                editor.shape_library.dragging_shape = None;
            }
        }
    }

    // Only show settings/export dialogs in edit mode
    if !editor.play_mode.is_in_play_mode() {
        // Ctrl+, to toggle settings panel
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(bevy_egui::egui::Key::Comma)) {
            let settings_panel = PanelId::Settings;
            if editor.docking.is_panel_visible(&settings_panel) {
                editor.docking.close_panel(&settings_panel);
            } else {
                editor.docking.open_panel(settings_panel);
            }
        }

        // Layout switching shortcuts (Ctrl+1/2/3/4)
        ctx.input(|i| {
            if i.modifiers.ctrl && !i.modifiers.shift && !i.modifiers.alt {
                let layout_name = if i.key_pressed(bevy_egui::egui::Key::Num1) {
                    Some("Default")
                } else if i.key_pressed(bevy_egui::egui::Key::Num2) {
                    Some("Scripting")
                } else if i.key_pressed(bevy_egui::egui::Key::Num3) {
                    Some("Animation")
                } else if i.key_pressed(bevy_egui::egui::Key::Num4) {
                    Some("Debug")
                } else {
                    None
                };

                if let Some(name) = layout_name {
                    if editor.docking.switch_layout(name) {
                        // Sync viewport state with layout
                        match name {
                            "Default" => {
                                editor.viewport.hierarchy_width = 260.0;
                                editor.viewport.inspector_width = 320.0;
                                editor.viewport.assets_height = 200.0;
                            }
                            "Scripting" => {
                                editor.viewport.hierarchy_width = 220.0;
                                editor.viewport.inspector_width = 300.0;
                                editor.viewport.assets_height = 180.0;
                            }
                            "Animation" => {
                                editor.viewport.hierarchy_width = 260.0;
                                editor.viewport.inspector_width = 320.0;
                                editor.viewport.assets_height = 250.0;
                            }
                            "Debug" => {
                                editor.viewport.hierarchy_width = 300.0;
                                editor.viewport.inspector_width = 280.0;
                                editor.viewport.assets_height = 200.0;
                            }
                            _ => {}
                        }
                    }
                }
            }
        });

        // Handle toggle bottom panel shortcut (only if not rebinding)
        if editor.keybindings.rebinding.is_none() {
            use crate::core::EditorAction;
            if editor.keybindings.just_pressed(EditorAction::ToggleBottomPanel, &editor.keyboard) {
                if editor.viewport.bottom_panel_minimized {
                    // Restore
                    editor.viewport.bottom_panel_minimized = false;
                    editor.viewport.assets_height = editor.viewport.bottom_panel_prev_height;
                } else {
                    // Minimize
                    editor.viewport.bottom_panel_prev_height = editor.viewport.assets_height;
                    editor.viewport.bottom_panel_minimized = true;
                }
            }
        }

        // Render assets dialogs (create script, create folder, import)
        render_assets_dialogs(ctx, &mut editor.assets, &editor.theme_manager.active_theme);

        // Render export project dialog
        render_export_dialog(ctx, &mut editor.scene_state, current_project.as_deref(), &editor.theme_manager.active_theme);

        // Process layout switch requests from assets panel
        if let Some(layout_name) = editor.assets.requested_layout.take() {
            if editor.docking.switch_layout(&layout_name) {
                // Sync viewport state with layout
                match layout_name.as_str() {
                    "Scripting" => {
                        editor.viewport.hierarchy_width = 220.0;
                        editor.viewport.inspector_width = 300.0;
                        editor.viewport.assets_height = 180.0;
                    }
                    "Blueprints" => {
                        editor.viewport.hierarchy_width = 260.0;
                        editor.viewport.inspector_width = 320.0;
                        editor.viewport.assets_height = 200.0;
                    }
                    _ => {}
                }
            }
        }

        // Process pending blueprint open requests from assets panel
        if let Some(path) = editor.assets.pending_blueprint_open.take() {
            let path_str = path.to_string_lossy().to_string();

            // Check if already open
            if editor.blueprint_editor.open_blueprints.contains_key(&path_str) {
                // Just activate it
                editor.scene_state.set_active_document(TabKind::Blueprint(path_str.clone()));
                editor.blueprint_editor.active_blueprint = Some(path_str);
            } else {
                // Load and open the blueprint
                match BlueprintFile::load(&path) {
                    Ok(file) => {
                        editor.blueprint_editor.open_blueprints.insert(path_str.clone(), file.graph);
                        // Add to tab order
                        editor.scene_state.tab_order.push(TabKind::Blueprint(path_str.clone()));
                        // Activate blueprint
                        editor.scene_state.set_active_document(TabKind::Blueprint(path_str.clone()));
                        editor.blueprint_editor.active_blueprint = Some(path_str);
                    }
                    Err(e) => {
                        bevy::log::error!("Failed to load blueprint {:?}: {}", path, e);
                    }
                }
            }
        }

        // Render plugin-registered panels (skip those already docked in the tree)
        let docked_plugin_ids: std::collections::HashSet<String> = {
            let api = editor.plugin_host.api();
            api.panels.iter()
                .filter(|(panel_def, _)| {
                    editor.docking.dock_tree.contains_panel(&PanelId::Plugin(panel_def.id.clone()))
                })
                .map(|(panel_def, _)| panel_def.id.clone())
                .collect()
        };
        let plugin_events = render_plugin_panels(ctx, &editor.plugin_host, &mut local_state.0, &docked_plugin_ids);
        all_ui_events.extend(plugin_events);
    }

    // Window resize edges (only in windowed mode, not maximized)
    if !editor.window_state.is_maximized {
        render_window_resize_edges(ctx, &mut editor.window_state);
    }

    // Handle file drops globally - import to assets panel
    let project_assets_folder = current_project.as_ref().map(|p| p.path.join("assets"));
    handle_global_file_drops(ctx, &mut editor.assets, &editor.viewport, project_assets_folder);

    // Forward all UI events to plugins (convert from internal to API type)
    for event in all_ui_events {
        // Handle PanelTabSelected locally to switch active tabs
        if let InternalUiEvent::PanelTabSelected { location, ref tab_id } = event {
            use crate::plugin_core::TabLocation;
            let loc = match location {
                0 => TabLocation::Left,
                1 => TabLocation::Right,
                2 => TabLocation::Bottom,
                _ => continue,
            };
            if tab_id.is_empty() {
                editor.plugin_host.api_mut().clear_active_tab(loc);
            } else {
                editor.plugin_host.api_mut().set_active_tab(loc, tab_id.clone());
            }
        }
        editor.plugin_host.api_mut().push_ui_event(convert_ui_event_to_api(event));
    }
}

/// Render the export project settings dialog
fn render_export_dialog(
    ctx: &bevy_egui::egui::Context,
    scene_state: &mut SceneManagerState,
    current_project: Option<&CurrentProject>,
    theme: &renzora_theme::Theme,
) {
    if !scene_state.show_export_dialog {
        return;
    }

    let Some(project) = current_project else {
        scene_state.show_export_dialog = false;
        return;
    };

    // Initialize dialog state from project config on first open
    if scene_state.export_dialog.binary_name.is_empty() {
        scene_state.export_dialog.binary_name = project.config.name.clone();
        scene_state.export_dialog.icon_path = project.config.icon.clone().unwrap_or_default();
        scene_state.export_dialog.fullscreen = project.config.window.fullscreen;
        scene_state.export_dialog.width = project.config.window.width;
        scene_state.export_dialog.height = project.config.window.height;
        scene_state.export_dialog.resizable = project.config.window.resizable;
    }

    let text_primary = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let accent = theme.semantic.accent.to_color32();
    let panel_bg = theme.surfaces.popup.to_color32();
    let row_even = theme.panels.inspector_row_even.to_color32();
    let row_odd = theme.panels.inspector_row_odd.to_color32();
    let item_bg = theme.panels.item_bg.to_color32();
    let border = theme.widgets.border.to_color32();

    let mut open = true;
    bevy_egui::egui::Window::new(format!("{} Export Project", egui_phosphor::regular::EXPORT))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(bevy_egui::egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .min_width(380.0)
        .frame(bevy_egui::egui::Frame::window(&ctx.style()).fill(panel_bg))
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            let label_width = 100.0;

            // Binary Name
            export_row(ui, 0, "Binary Name", label_width, row_even, row_odd, |ui| {
                ui.add_sized(
                    [ui.available_width(), 20.0],
                    bevy_egui::egui::TextEdit::singleline(&mut scene_state.export_dialog.binary_name)
                        .hint_text("MyGame"),
                );
            });

            // Icon
            export_row(ui, 1, "Icon", label_width, row_even, row_odd, |ui| {
                let icon_text = if scene_state.export_dialog.icon_path.is_empty() {
                    "None".to_string()
                } else {
                    scene_state.export_dialog.icon_path.clone()
                };

                let truncated = if icon_text.len() > 30 {
                    format!("...{}", &icon_text[icon_text.len()-27..])
                } else {
                    icon_text
                };

                ui.label(bevy_egui::egui::RichText::new(&truncated).size(12.0).color(text_muted));

                ui.with_layout(bevy_egui::egui::Layout::right_to_left(bevy_egui::egui::Align::Center), |ui| {
                    if !scene_state.export_dialog.icon_path.is_empty() {
                        if ui.add(bevy_egui::egui::Button::new(
                            bevy_egui::egui::RichText::new(egui_phosphor::regular::X).size(12.0).color(text_muted)
                        ).fill(item_bg).corner_radius(bevy_egui::egui::CornerRadius::same(3))).clicked() {
                            scene_state.export_dialog.icon_path.clear();
                        }
                    }

                    if ui.add(bevy_egui::egui::Button::new(
                        bevy_egui::egui::RichText::new("Browse...").size(12.0).color(text_primary)
                    ).fill(item_bg).stroke(bevy_egui::egui::Stroke::new(1.0, border))
                      .corner_radius(bevy_egui::egui::CornerRadius::same(3))).clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Icon", &["ico", "png"])
                            .set_directory(&project.path)
                            .pick_file()
                        {
                            // Try to make path relative to project
                            if let Some(rel) = project.make_relative(&path) {
                                scene_state.export_dialog.icon_path = rel;
                            } else {
                                scene_state.export_dialog.icon_path = path.to_string_lossy().to_string();
                            }
                        }
                    }
                });
            });

            // Fullscreen
            export_row(ui, 2, "Fullscreen", label_width, row_even, row_odd, |ui| {
                ui.checkbox(&mut scene_state.export_dialog.fullscreen, "Start in fullscreen");
            });

            // Resolution
            export_row(ui, 3, "Resolution", label_width, row_even, row_odd, |ui| {
                ui.horizontal(|ui| {
                    ui.add(bevy_egui::egui::DragValue::new(&mut scene_state.export_dialog.width)
                        .range(320..=7680)
                        .speed(1));
                    ui.label(bevy_egui::egui::RichText::new("x").size(12.0).color(text_muted));
                    ui.add(bevy_egui::egui::DragValue::new(&mut scene_state.export_dialog.height)
                        .range(240..=4320)
                        .speed(1));
                });
            });

            // Resizable
            export_row(ui, 4, "Resizable", label_width, row_even, row_odd, |ui| {
                ui.checkbox(&mut scene_state.export_dialog.resizable, "Allow window resize");
            });

            ui.add_space(12.0);

            // Export / Cancel buttons
            ui.horizontal(|ui| {
                let export_btn = bevy_egui::egui::Button::new(
                    bevy_egui::egui::RichText::new(format!("{} Export", egui_phosphor::regular::EXPORT)).color(bevy_egui::egui::Color32::WHITE).size(13.0)
                )
                    .fill(accent)
                    .corner_radius(bevy_egui::egui::CornerRadius::same(4))
                    .min_size(bevy_egui::egui::Vec2::new(100.0, 28.0));

                if ui.add(export_btn).clicked() {
                    scene_state.export_project_requested = true;
                    scene_state.show_export_dialog = false;
                }

                let cancel_btn = bevy_egui::egui::Button::new(
                    bevy_egui::egui::RichText::new("Cancel").color(text_primary).size(13.0)
                )
                    .fill(item_bg)
                    .stroke(bevy_egui::egui::Stroke::new(1.0, border))
                    .corner_radius(bevy_egui::egui::CornerRadius::same(4))
                    .min_size(bevy_egui::egui::Vec2::new(80.0, 28.0));

                if ui.add(cancel_btn).clicked() {
                    scene_state.show_export_dialog = false;
                    scene_state.export_dialog = Default::default();
                }
            });
        });

    if !open {
        scene_state.show_export_dialog = false;
        scene_state.export_dialog = Default::default();
    }
}

/// Helper for export dialog rows with alternating backgrounds
fn export_row(
    ui: &mut bevy_egui::egui::Ui,
    row_index: usize,
    label: &str,
    label_width: f32,
    row_even: bevy_egui::egui::Color32,
    row_odd: bevy_egui::egui::Color32,
    add_widget: impl FnOnce(&mut bevy_egui::egui::Ui),
) {
    let bg = if row_index % 2 == 0 { row_even } else { row_odd };
    let available_width = ui.available_width();

    bevy_egui::egui::Frame::new()
        .fill(bg)
        .inner_margin(bevy_egui::egui::Margin::symmetric(6, 4))
        .show(ui, |ui| {
            ui.set_min_width(available_width - 12.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.add_sized(
                    [label_width, 18.0],
                    bevy_egui::egui::Label::new(bevy_egui::egui::RichText::new(label).size(12.0)).truncate()
                );
                add_widget(ui);
            });
        });
}

/// Render play mode overlay with status and stop hint
fn render_play_mode_overlay(ctx: &bevy_egui::egui::Context, play_mode: &mut PlayModeState) {
    use bevy_egui::egui::{self, Color32, Align2, RichText, CornerRadius, Vec2};

    // Top-center status indicator
    egui::Area::new(egui::Id::new("play_mode_status"))
        .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 16.0))
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(Color32::from_rgba_unmultiplied(0, 0, 0, 180))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(16, 8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Status icon and text
                        let (icon, color, text) = match play_mode.state {
                            PlayState::Playing => (PLAY, Color32::from_rgb(64, 200, 100), "Playing"),
                            PlayState::Paused => (PAUSE, Color32::from_rgb(255, 200, 80), "Paused"),
                            PlayState::ScriptsOnly => (PLAY, Color32::from_rgb(66, 150, 250), "Scripts Running"),
                            PlayState::ScriptsPaused => (PAUSE, Color32::from_rgb(255, 200, 80), "Scripts Paused"),
                            PlayState::Editing => (STOP, Color32::WHITE, "Editing"),
                        };

                        ui.label(RichText::new(icon).color(color).size(14.0));
                        ui.label(RichText::new(text).color(Color32::WHITE).size(14.0));
                        ui.add_space(16.0);
                        ui.label(RichText::new("Press ESC to stop").color(Color32::from_rgb(150, 150, 160)).size(12.0));
                    });
                });
        });

    // Bottom-right stop button
    egui::Area::new(egui::Id::new("play_mode_controls"))
        .anchor(Align2::RIGHT_BOTTOM, Vec2::new(-20.0, -20.0))
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(Color32::from_rgba_unmultiplied(40, 40, 50, 220))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Pause/Resume button
                        if play_mode.state == PlayState::Playing {
                            if ui.add(egui::Button::new(RichText::new("\u{f04c}").size(16.0))
                                .fill(Color32::from_rgb(60, 60, 80))
                                .min_size(Vec2::new(32.0, 32.0))).clicked() {
                                play_mode.state = PlayState::Paused;
                            }
                        } else if play_mode.state == PlayState::Paused {
                            if ui.add(egui::Button::new(RichText::new("\u{f04b}").size(16.0))
                                .fill(Color32::from_rgb(50, 120, 70))
                                .min_size(Vec2::new(32.0, 32.0))).clicked() {
                                play_mode.state = PlayState::Playing;
                            }
                        }

                        // Stop button
                        if ui.add(egui::Button::new(RichText::new("\u{f04d}").size(16.0))
                            .fill(Color32::from_rgb(180, 60, 60))
                            .min_size(Vec2::new(32.0, 32.0))).clicked() {
                            play_mode.request_stop = true;
                        }
                    });
                });
        });
}

/// Render on-screen console logs in the viewport during play mode (like Unreal Engine)
fn render_viewport_logs(
    ctx: &bevy_egui::egui::Context,
    console: &ConsoleState,
    current_time: f64,
    viewport_rect: bevy_egui::egui::Rect,
) {
    use bevy_egui::egui::{self, Color32, FontId, Pos2, Vec2};

    const MAX_VISIBLE: usize = 12;
    const DISPLAY_DURATION: f64 = 5.0;
    const FADE_DURATION: f64 = 1.0;
    const LINE_HEIGHT: f32 = 14.0;
    const MARGIN_X: f32 = 12.0;
    const MARGIN_Y: f32 = 12.0;

    // Collect recent entries within the display window
    let recent: Vec<_> = console.entries.iter().rev()
        .filter(|e| {
            let age = current_time - e.timestamp;
            age >= 0.0 && age < DISPLAY_DURATION
        })
        .take(MAX_VISIBLE)
        .collect();

    if recent.is_empty() {
        return;
    }

    // Paint directly with a clipped painter to stay within viewport bounds
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("viewport_console_logs")))
        .with_clip_rect(viewport_rect);

    let origin = Pos2::new(viewport_rect.min.x + MARGIN_X, viewport_rect.min.y + MARGIN_Y);

    // Render oldest at top, newest at bottom
    for (i, entry) in recent.iter().rev().enumerate() {
        let age = current_time - entry.timestamp;
        let alpha = if age > DISPLAY_DURATION - FADE_DURATION {
            ((DISPLAY_DURATION - age) / FADE_DURATION) as f32
        } else {
            1.0
        };

        let [r, g, b] = entry.level.color();
        let text_color = Color32::from_rgba_unmultiplied(r, g, b, (alpha * 255.0) as u8);
        let shadow_color = Color32::from_rgba_unmultiplied(0, 0, 0, (alpha * 200.0) as u8);

        let y = i as f32 * LINE_HEIGHT;
        let pos = Pos2::new(origin.x, origin.y + y);

        let text = if entry.category.is_empty() {
            entry.message.clone()
        } else {
            format!("[{}] {}", entry.category, entry.message)
        };

        // Drop shadow for readability
        painter.text(
            Pos2::new(pos.x + 1.0, pos.y + 1.0),
            egui::Align2::LEFT_TOP,
            &text,
            FontId::new(10.0, egui::FontFamily::Monospace),
            shadow_color,
        );

        // Main text
        painter.text(
            pos,
            egui::Align2::LEFT_TOP,
            &text,
            FontId::new(10.0, egui::FontFamily::Monospace),
            text_color,
        );
    }
}

/// Maximum number of thumbnails to load concurrently
const MAX_CONCURRENT_THUMBNAIL_LOADS: usize = 8;

/// System to load and register asset thumbnails
pub fn thumbnail_loading_system(
    mut contexts: EguiContexts,
    mut thumbnail_cache: ResMut<ThumbnailCache>,
    asset_server: Res<AssetServer>,
    images: Res<Assets<Image>>,
) {
    use bevy::asset::LoadState;
    use std::path::PathBuf;

    // Get the egui context early - if unavailable, skip this frame
    let Ok(_ctx) = contexts.ctx_mut() else {
        return;
    };

    // Count how many are currently loading (have handles but no texture_id yet)
    let currently_loading = thumbnail_cache
        .image_handles
        .keys()
        .filter(|path| !thumbnail_cache.texture_ids.contains_key(*path))
        .count();

    // Collect paths that need handles created (limit by max concurrent)
    let paths_needing_handles: Vec<PathBuf> = thumbnail_cache
        .loading
        .iter()
        .filter(|path| !thumbnail_cache.image_handles.contains_key(*path))
        .take(MAX_CONCURRENT_THUMBNAIL_LOADS.saturating_sub(currently_loading))
        .cloned()
        .collect();

    // Start loading new images
    for path in paths_needing_handles {
        let handle: Handle<Image> = asset_server.load(path.clone());
        thumbnail_cache.image_handles.insert(path, handle);
    }

    // Check loading status and register textures
    let handles_to_check: Vec<(PathBuf, Handle<Image>)> = thumbnail_cache
        .image_handles
        .iter()
        .filter(|(path, _)| !thumbnail_cache.texture_ids.contains_key(*path))
        .map(|(path, handle)| (path.clone(), handle.clone()))
        .collect();

    for (path, handle) in handles_to_check {
        match asset_server.get_load_state(&handle) {
            Some(LoadState::Loaded) => {
                // Image is loaded, verify it exists and has a compatible format
                if let Some(image) = images.get(&handle) {
                    // Check if the texture format is compatible with egui
                    // egui requires filterable textures - reject Uint/Sint formats
                    use bevy::render::render_resource::TextureFormat;
                    let format = image.texture_descriptor.format;

                    // Blacklist formats known to be incompatible with egui
                    // (Uint and Sint formats are not filterable)
                    let is_incompatible = matches!(
                        format,
                        TextureFormat::R8Uint | TextureFormat::R8Sint
                            | TextureFormat::R16Uint | TextureFormat::R16Sint
                            | TextureFormat::R32Uint | TextureFormat::R32Sint
                            | TextureFormat::Rg8Uint | TextureFormat::Rg8Sint
                            | TextureFormat::Rg16Uint | TextureFormat::Rg16Sint
                            | TextureFormat::Rg32Uint | TextureFormat::Rg32Sint
                            | TextureFormat::Rgba8Uint | TextureFormat::Rgba8Sint
                            | TextureFormat::Rgba16Uint | TextureFormat::Rgba16Sint
                            | TextureFormat::Rgba32Uint | TextureFormat::Rgba32Sint
                            | TextureFormat::Rgb10a2Uint
                    );

                    if !is_incompatible {
                        // Register with egui
                        let texture_id = contexts.add_image(EguiTextureHandle::Weak(handle.id()));
                        thumbnail_cache.texture_ids.insert(path.clone(), texture_id);
                        thumbnail_cache.loading.remove(&path);
                    } else {
                        // Format not compatible with egui - mark as failed
                        warn!("Thumbnail format {:?} not compatible with egui: {}", format, path.display());
                        thumbnail_cache.loading.remove(&path);
                        thumbnail_cache.image_handles.remove(&path);
                        thumbnail_cache.failed.insert(path);
                    }
                }
            }
            Some(LoadState::Failed(_)) => {
                // Failed to load - remove from loading and mark as failed
                thumbnail_cache.loading.remove(&path);
                thumbnail_cache.image_handles.remove(&path);
                thumbnail_cache.failed.insert(path);
            }
            _ => {
                // Still loading, do nothing
            }
        }
    }
}

/// Handle file drops globally - route to assets panel for import
fn handle_global_file_drops(
    ctx: &bevy_egui::egui::Context,
    assets: &mut AssetBrowserState,
    viewport: &ViewportState,
    project_assets_folder: Option<std::path::PathBuf>,
) {
    use bevy_egui::egui::{Pos2, Rect};

    // Check if there are any dropped files
    let dropped_files: Vec<std::path::PathBuf> = ctx.input(|i| {
        i.raw.dropped_files.iter()
            .filter_map(|f| f.path.clone())
            .collect()
    });

    if dropped_files.is_empty() {
        return;
    }

    // Get cursor position when files were dropped
    let hover_pos = ctx.input(|i| i.pointer.hover_pos());
    let Some(pos) = hover_pos else {
        return;
    };

    // Build assets panel rect from stored bounds
    let [px, py, pw, ph] = assets.panel_bounds;
    let assets_rect = Rect::from_min_size(Pos2::new(px, py), bevy_egui::egui::Vec2::new(pw, ph));

    // Build viewport rect
    let viewport_rect = Rect::from_min_size(
        Pos2::new(viewport.position[0], viewport.position[1]),
        bevy_egui::egui::Vec2::new(viewport.size[0], viewport.size[1]),
    );

    // Determine target folder: use current folder, or fall back to project assets folder
    let target_folder = assets.current_folder.clone().or(project_assets_folder);

    // Only import if we have a valid target folder
    let Some(folder) = target_folder else {
        return;
    };

    for path in dropped_files {
        let is_hdr = path.extension()
            .and_then(|e| e.to_str())
            .map(|ext| matches!(ext.to_lowercase().as_str(), "hdr" | "exr"))
            .unwrap_or(false);

        if is_hdr && viewport_rect.contains(pos) {
            // HDR/EXR on viewport → create/update skybox + import to assets
            assets.pending_skybox_drop = Some(path.clone());
            assets.pending_file_imports.push(path);
        } else if assets_rect.contains(pos) || viewport_rect.contains(pos) {
            // Dropped in assets panel or viewport - queue for import (copy to folder)
            info!("File dropped, importing to assets: {:?}", path);
            assets.pending_file_imports.push(path);
        }
        // If dropped elsewhere, ignore

        // Store the target folder for import if current_folder is not set
        if assets.current_folder.is_none() {
            assets.current_folder = Some(folder.clone());
        }
    }
}

/// Render invisible resize edges around the window for custom window resizing
fn render_window_resize_edges(ctx: &bevy_egui::egui::Context, window_state: &mut WindowState) {
    use bevy_egui::egui::{self, CursorIcon, Sense, Vec2, Id, Pos2};

    let screen_rect = ctx.content_rect();
    let edge_size = 5.0;
    let corner_size = 10.0;

    // Helper to create resize area
    let mut create_resize_area = |id: &str, rect: egui::Rect, edge: ResizeEdge, cursor: CursorIcon| {
        egui::Area::new(Id::new(id))
            .fixed_pos(rect.min)
            .order(egui::Order::Foreground)
            .interactable(true)
            .show(ctx, |ui| {
                let (_, response) = ui.allocate_exact_size(rect.size(), Sense::drag());

                if response.hovered() || response.dragged() {
                    ctx.set_cursor_icon(cursor);
                }

                if response.drag_started() {
                    window_state.resize_edge = edge;
                    window_state.is_resizing = true;
                }

                if response.drag_stopped() {
                    window_state.resize_edge = ResizeEdge::None;
                    window_state.is_resizing = false;
                    window_state.resize_start_rect = None;
                    window_state.resize_start_cursor = None;
                }
            });
    };

    // Bottom edge
    create_resize_area(
        "resize_bottom",
        egui::Rect::from_min_size(
            Pos2::new(corner_size, screen_rect.height() - edge_size),
            Vec2::new(screen_rect.width() - corner_size * 2.0, edge_size),
        ),
        ResizeEdge::Bottom,
        CursorIcon::ResizeVertical,
    );

    // Right edge
    create_resize_area(
        "resize_right",
        egui::Rect::from_min_size(
            Pos2::new(screen_rect.width() - edge_size, corner_size),
            Vec2::new(edge_size, screen_rect.height() - corner_size * 2.0),
        ),
        ResizeEdge::Right,
        CursorIcon::ResizeHorizontal,
    );

    // Left edge
    create_resize_area(
        "resize_left",
        egui::Rect::from_min_size(
            Pos2::new(0.0, corner_size),
            Vec2::new(edge_size, screen_rect.height() - corner_size * 2.0),
        ),
        ResizeEdge::Left,
        CursorIcon::ResizeHorizontal,
    );

    // Top edge (below title bar area)
    create_resize_area(
        "resize_top",
        egui::Rect::from_min_size(
            Pos2::new(corner_size, 0.0),
            Vec2::new(screen_rect.width() - corner_size * 2.0, edge_size),
        ),
        ResizeEdge::Top,
        CursorIcon::ResizeVertical,
    );

    // Corner: Bottom-Right
    create_resize_area(
        "resize_bottom_right",
        egui::Rect::from_min_size(
            Pos2::new(screen_rect.width() - corner_size, screen_rect.height() - corner_size),
            Vec2::new(corner_size, corner_size),
        ),
        ResizeEdge::BottomRight,
        CursorIcon::ResizeNwSe,
    );

    // Corner: Bottom-Left
    create_resize_area(
        "resize_bottom_left",
        egui::Rect::from_min_size(
            Pos2::new(0.0, screen_rect.height() - corner_size),
            Vec2::new(corner_size, corner_size),
        ),
        ResizeEdge::BottomLeft,
        CursorIcon::ResizeNeSw,
    );

    // Corner: Top-Right
    create_resize_area(
        "resize_top_right",
        egui::Rect::from_min_size(
            Pos2::new(screen_rect.width() - corner_size, 0.0),
            Vec2::new(corner_size, corner_size),
        ),
        ResizeEdge::TopRight,
        CursorIcon::ResizeNeSw,
    );

    // Corner: Top-Left
    create_resize_area(
        "resize_top_left",
        egui::Rect::from_min_size(
            Pos2::new(0.0, 0.0),
            Vec2::new(corner_size, corner_size),
        ),
        ResizeEdge::TopLeft,
        CursorIcon::ResizeNwSe,
    );
}

/// Exclusive system for rendering inspector panel content with World access
///
/// This runs after `editor_ui` and renders the actual Inspector content using
/// registry-based component iteration, which requires `&mut World`.
pub fn inspector_content_exclusive(world: &mut World) {
    use bevy_egui::{egui, EguiContext, PrimaryEguiContext};
    use crate::core::{InspectorPanelRenderState, SelectionState, AssetBrowserState, ThumbnailCache, AppState};
    use crate::component_system::AddComponentPopupState;
    use crate::plugin_core::PluginHost;
    use crate::ui_api::renderer::UiRenderer;

    // Check if we should run (only in Editor state)
    let should_run = world
        .get_resource::<State<AppState>>()
        .map(|s| *s.get() == AppState::Editor)
        .unwrap_or(false);

    if !should_run {
        return;
    }

    // Check if inspector panel was requested for render
    let render_state = {
        let Some(state) = world.get_resource::<InspectorPanelRenderState>() else {
            return;
        };
        if !state.should_render || state.content_rect.is_none() || state.theme.is_none() {
            return;
        }
        // Clone the state so we can release the borrow
        (
            state.content_rect.unwrap(),
            state.camera_preview_texture_id,
            state.theme.clone().unwrap(),
        )
    };

    let (content_rect, _camera_preview_texture, theme) = render_state;

    // Use nested resource_scope to properly manage borrows
    // This extracts resources, runs the closure, and puts them back
    world.resource_scope(|world, mut add_component_popup: Mut<AddComponentPopupState>| {
        world.resource_scope(|world, mut assets: Mut<AssetBrowserState>| {
            world.resource_scope(|world, mut thumbnail_cache: Mut<ThumbnailCache>| {
                // Clone the egui context
                let ctx = {
                    let mut query = world.query_filtered::<&mut EguiContext, With<PrimaryEguiContext>>();
                    let Ok(mut egui_ctx) = query.single_mut(world) else {
                        return;
                    };
                    egui_ctx.get_mut().clone()
                };

                // Create a temporary UiRenderer
                let mut ui_renderer = UiRenderer::default();

                // Render inspector content using Area at the stored position
                // Note: We pass None for plugin_host to avoid borrow conflicts.
                // Plugin-registered inspector sections will be skipped.
                let (events, scene_changed) = egui::Area::new(egui::Id::new("inspector_content_area"))
                    .fixed_pos(content_rect.min)
                    .order(egui::Order::Middle)
                    .show(&ctx, |ui| {
                        ui.set_clip_rect(content_rect);
                        ui.set_min_size(content_rect.size());
                        ui.set_max_size(content_rect.size());

                        // Clone selection inside a scope to end the borrow before mutable access
                        let selection = {
                            let s = world.resource::<SelectionState>();
                            SelectionState {
                                selected_entity: s.selected_entity,
                                multi_selection: s.multi_selection.clone(),
                                context_menu_entity: s.context_menu_entity,
                                selection_anchor: s.selection_anchor,
                            }
                        };

                        // Call the world-based inspector renderer
                        panels::render_inspector_content_world(
                            ui,
                            world,
                            &selection,
                            None, // Plugin sections skipped to avoid borrow conflicts
                            &mut ui_renderer,
                            &mut add_component_popup,
                            &mut assets,
                            &mut thumbnail_cache,
                            &theme,
                        )
                    }).inner;

                // Store scene_changed in render state
                world.resource_mut::<InspectorPanelRenderState>().scene_changed = scene_changed;

                // Forward events to plugin host
                let mut plugin_host = world.resource_mut::<PluginHost>();
                for event in events {
                    plugin_host.api_mut().push_ui_event(convert_ui_event_to_api(event));
                }

                // Mark scene as modified if changed
                if scene_changed {
                    world.resource_mut::<crate::core::SceneManagerState>().mark_modified();
                }
            });
        });
    });

    // Process drag results now that AssetBrowserState is back in the world
    let render_state = world.resource::<InspectorPanelRenderState>();
    let drag_accepted = render_state.drag_accepted;
    let pending_import = render_state.pending_file_import.clone();

    if drag_accepted {
        world.resource_mut::<AssetBrowserState>().dragging_asset = None;
    }
    if let Some(import_path) = pending_import {
        world.resource_mut::<AssetBrowserState>().pending_file_imports.push(import_path);
    }
    world.resource_mut::<InspectorPanelRenderState>().drag_accepted = false;
    world.resource_mut::<InspectorPanelRenderState>().pending_file_import = None;
}
