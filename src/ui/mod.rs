mod panels;
mod style;
pub mod docking;
pub mod inspectors;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiTextureHandle};

use crate::commands::CommandHistory;
use crate::core::{
    AppState, AssetLoadingProgress, ConsoleState, DefaultCameraEntity, DockingState,
    EditorEntity, ExportState, KeyBindings, SelectionState, HierarchyState, ViewportState,
    SceneManagerState, AssetBrowserState, EditorSettings, WindowState, OrbitCameraState,
    PlayModeState, PlayState, ThumbnailCache, ResizeEdge,
};
use crate::gizmo::{GizmoState, ModalTransformState};
use crate::viewport::Camera2DState;

/// Bundled editor state resources for system parameter limits
#[derive(SystemParam)]
pub struct EditorResources<'w> {
    pub selection: ResMut<'w, SelectionState>,
    pub hierarchy: ResMut<'w, HierarchyState>,
    pub viewport: ResMut<'w, ViewportState>,
    pub scene_state: ResMut<'w, SceneManagerState>,
    pub assets: ResMut<'w, AssetBrowserState>,
    pub settings: ResMut<'w, EditorSettings>,
    pub export_state: ResMut<'w, ExportState>,
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
    pub component_registry: Res<'w, ComponentRegistry>,
    pub add_component_popup: ResMut<'w, AddComponentPopupState>,
    pub keyboard: Res<'w, ButtonInput<KeyCode>>,
    pub docking: ResMut<'w, DockingState>,
    pub blueprint_editor: ResMut<'w, BlueprintEditorState>,
    pub blueprint_canvas: ResMut<'w, BlueprintCanvasState>,
    pub blueprint_nodes: Res<'w, BlueprintNodeRegistry>,
}
use crate::component_system::{ComponentRegistry, AddComponentPopupState};
use panels::HierarchyQueries;
use crate::project::{AppConfig, CurrentProject};
use crate::scripting::{ScriptRegistry, RhaiScriptEngine};
use crate::viewport::{CameraPreviewImage, ViewportImage};
use crate::plugin_core::PluginHost;
use crate::blueprint::{BlueprintEditorState, BlueprintCanvasState, nodes::NodeRegistry as BlueprintNodeRegistry};
use crate::ui_api::renderer::UiRenderer;
use crate::ui_api::UiEvent as InternalUiEvent;
use docking::{
    get_legacy_layout_values, render_dock_tree, render_panel_frame,
    calculate_panel_rects, DockedPanelContext, PanelId, DropZone, SplitDirection,
};
use bevy_egui::egui::{Rect, Pos2};
use panels::{
    render_export_dialog, render_plugin_panels,
    render_document_tabs, render_script_editor, render_script_editor_content, render_settings_window,
    render_splash, render_status_bar, render_title_bar, render_toolbar, render_viewport,
    InspectorQueries, TITLE_BAR_HEIGHT,
    render_hierarchy_content, render_inspector_content, render_assets_content, render_assets_dialogs,
    render_console_content, render_history_content,
};
pub use panels::{handle_window_actions, property_row};
use style::{apply_editor_style, init_fonts};

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
    );
}

/// Editor UI system (runs in Editor state)
pub fn editor_ui(
    mut contexts: EguiContexts,
    mut editor: EditorResources,
    mut commands: Commands,
    hierarchy_queries: HierarchyQueries,
    entities_for_inspector: Query<(Entity, &EditorEntity)>,
    mut inspector_queries: InspectorQueries,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    current_project: Option<Res<CurrentProject>>,
    script_registry: Res<ScriptRegistry>,
    rhai_engine: Res<RhaiScriptEngine>,
    app_state: Res<State<AppState>>,
    viewport_image: Option<Res<ViewportImage>>,
    camera_preview_image: Option<Res<CameraPreviewImage>>,
    mut ui_renderer: Local<UiRenderer>,
) {
    // Only run in Editor state (run_if doesn't work with EguiPrimaryContextPass)
    if *app_state.get() != AppState::Editor {
        return;
    }

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

    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    apply_editor_style(ctx);

    // Check play mode state early
    let in_play_mode_early = editor.play_mode.is_in_play_mode();

    // Render status bar at bottom (must be rendered early to reserve space) - skip in play mode
    if !in_play_mode_early {
        render_status_bar(ctx, &editor.plugin_host, &editor.loading_progress);
    }

    // Collect all UI events to forward to plugins
    let mut all_ui_events = Vec::new();

    // Render custom title bar (includes menu)
    let title_bar_events = render_title_bar(
        ctx,
        &mut editor.window_state,
        &mut editor.selection,
        &mut editor.scene_state,
        &mut editor.settings,
        &mut editor.export_state,
        &mut editor.assets,
        &mut commands,
        &mut meshes,
        &mut materials,
        &editor.plugin_host,
        &mut editor.command_history,
        &mut editor.docking,
        &mut editor.viewport,
    );
    all_ui_events.extend(title_bar_events);

    let toolbar_height = 36.0;

    let toolbar_events = render_toolbar(
        ctx,
        &mut editor.gizmo,
        &mut editor.settings,
        TITLE_BAR_HEIGHT,
        toolbar_height,
        1600.0, // Default width, will be constrained by panel
        &editor.plugin_host,
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut editor.selection,
        &mut editor.hierarchy,
        &mut editor.play_mode,
        &mut editor.docking,
    );
    all_ui_events.extend(toolbar_events);

    // Check if we're in play mode
    let in_play_mode = editor.play_mode.is_in_play_mode();

    let screen_rect = ctx.screen_rect();
    let status_bar_height = if in_play_mode { 0.0 } else { 24.0 };

    // Suppress unused import warning
    let _ = get_legacy_layout_values;

    // In play mode, skip docking and render full viewport
    let content_start_y = TITLE_BAR_HEIGHT + toolbar_height;

    if in_play_mode {
        // Full-screen viewport in play mode
        let central_height = (screen_rect.height() - content_start_y - status_bar_height).max(100.0);
        render_viewport(
            ctx,
            &mut editor.viewport,
            &mut editor.assets,
            &mut editor.orbit,
            &editor.camera2d_state,
            &editor.gizmo,
            &editor.modal_transform,
            0.0,
            0.0,
            content_start_y,
            [1600.0, 900.0],
            central_height,
            viewport_texture_id,
        );

        // Render play mode overlay with info
        render_play_mode_overlay(ctx, &mut editor.play_mode);
    } else {
        // DOCKING SYSTEM - Render dock tree with draggable panels

        // Render document tabs bar (for scene/script tabs with + button)
        let document_tabs_height = render_document_tabs(
            ctx,
            &mut editor.scene_state,
            &mut editor.docking,
            0.0,  // No left margin - spans full width
            0.0,  // No right margin
            content_start_y,
        );

        // Calculate dock area (below scene tabs, above status bar)
        let dock_start_y = content_start_y + document_tabs_height;
        let dock_rect = Rect::from_min_max(
            Pos2::new(0.0, dock_start_y),
            Pos2::new(screen_rect.width(), screen_rect.height() - status_bar_height),
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
                )
            }).inner;

        // Calculate panel rects BEFORE processing events that modify the tree
        // This ensures panel content renders at the same positions as the dock tree structure
        let panel_rects = calculate_panel_rects(&editor.docking.dock_tree, dock_rect);

        // Process dock tree events
        // Handle drag start
        if let Some(panel) = dock_result.drag_started {
            if let Some(pos) = ctx.pointer_hover_pos() {
                editor.docking.start_drag(panel, pos);
            }
        }

        // Handle tab close
        if let Some(panel) = dock_result.panel_to_close {
            editor.docking.close_panel(&panel);
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
            if let Some(drop_target) = &dock_result.drop_completed {
                let drag_panel = editor.docking.drag_state.as_ref().map(|d| d.panel.clone());
                if let Some(panel) = drag_panel {
                    // Don't drop on self
                    if panel != drop_target.target_panel {
                        // First remove from current location
                        editor.docking.dock_tree.remove_panel(&panel);

                        // Then add to new location based on drop zone
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
                        }
                        editor.docking.mark_modified();
                    }
                }
            }
            editor.docking.end_drag();
        }

        // Draw drag preview if dragging
        if let Some(drag) = &editor.docking.drag_state {
            if let Some(pos) = ctx.pointer_hover_pos() {
                bevy_egui::egui::Area::new(bevy_egui::egui::Id::new("drag_preview_area"))
                    .fixed_pos(pos + bevy_egui::egui::Vec2::new(10.0, 10.0))
                    .order(bevy_egui::egui::Order::Tooltip)
                    .interactable(false)
                    .show(ctx, |ui| {
                        bevy_egui::egui::Frame::popup(ui.style())
                            .fill(bevy_egui::egui::Color32::from_rgba_unmultiplied(50, 55, 65, 230))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(bevy_egui::egui::RichText::new(drag.panel.icon()).size(14.0));
                                    ui.label(bevy_egui::egui::RichText::new(drag.panel.title()).size(13.0));
                                });
                            });
                    });
            }
        }

        for (panel_id, panel_rect, is_active) in panel_rects {
            if !is_active {
                continue; // Only render active tabs
            }

            let panel_ctx = DockedPanelContext::new(panel_rect, panel_id.clone(), is_active);

            match panel_id {
                PanelId::Hierarchy => {
                    render_panel_frame(ctx, &panel_ctx, |ui| {
                        // Check if a scene file is being dragged
                        let dragging_scene = editor.assets.dragging_asset.as_ref()
                            .map(|p| p.to_string_lossy().to_lowercase().ends_with(".ron"))
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
                            &editor.default_camera,
                            &mut editor.command_history,
                        );
                        all_ui_events.extend(events);
                        if changed {
                            editor.scene_state.mark_modified();
                        }
                    });
                }

                PanelId::Inspector => {
                    render_panel_frame(ctx, &panel_ctx, |ui| {
                        let (events, changed) = render_inspector_content(
                            ui,
                            &editor.selection,
                            &entities_for_inspector,
                            &mut inspector_queries,
                            &script_registry,
                            &rhai_engine,
                            camera_preview_texture_id,
                            &editor.plugin_host,
                            &mut ui_renderer,
                            &editor.component_registry,
                            &mut editor.add_component_popup,
                            &mut commands,
                            &mut meshes,
                            &mut materials,
                            &mut editor.gizmo,
                        );
                        all_ui_events.extend(events);
                        if changed {
                            editor.scene_state.mark_modified();
                        }
                    });
                }

                PanelId::Assets => {
                    render_panel_frame(ctx, &panel_ctx, |ui| {
                        render_assets_content(
                            ui,
                            current_project.as_deref(),
                            &mut editor.assets,
                            &mut editor.scene_state,
                            &mut editor.thumbnail_cache,
                        );
                    });
                }

                PanelId::Console => {
                    render_panel_frame(ctx, &panel_ctx, |ui| {
                        render_console_content(ui, &mut editor.console);
                    });
                }

                PanelId::History => {
                    render_panel_frame(ctx, &panel_ctx, |ui| {
                        render_history_content(ui, &mut editor.command_history);
                    });
                }

                PanelId::Viewport => {
                    // Viewport needs special handling - render into the content rect
                    let content_rect = panel_ctx.content_rect;
                    editor.viewport.position = [content_rect.min.x, content_rect.min.y];
                    editor.viewport.size = [content_rect.width(), content_rect.height()];

                    // Check if script editor should be shown instead
                    let script_editor_shown = render_script_editor(
                        ctx,
                        &mut editor.scene_state,
                        current_project.as_deref(),
                        content_rect.min.x,
                        screen_rect.width() - content_rect.max.x,
                        content_rect.min.y,
                        content_rect.height(),
                    );

                    if !script_editor_shown {
                        render_viewport(
                            ctx,
                            &mut editor.viewport,
                            &mut editor.assets,
                            &mut editor.orbit,
                            &editor.camera2d_state,
                            &editor.gizmo,
                            &editor.modal_transform,
                            content_rect.min.x,
                            screen_rect.width() - content_rect.max.x,
                            content_rect.min.y,
                            [content_rect.width(), content_rect.height()],
                            content_rect.height(),
                            viewport_texture_id,
                        );
                    }
                }

                PanelId::Animation => {
                    render_panel_frame(ctx, &panel_ctx, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.add_space(20.0);
                            ui.label(bevy_egui::egui::RichText::new("\u{f008}").size(32.0).color(bevy_egui::egui::Color32::from_gray(80)));
                            ui.add_space(8.0);
                            ui.label(bevy_egui::egui::RichText::new("Animation").size(14.0).color(bevy_egui::egui::Color32::from_gray(100)));
                            ui.label(bevy_egui::egui::RichText::new("Coming soon").size(12.0).weak());
                        });
                    });
                }

                PanelId::ScriptEditor => {
                    render_panel_frame(ctx, &panel_ctx, |ui| {
                        render_script_editor_content(
                            ui,
                            ctx,
                            &mut editor.scene_state,
                            current_project.as_deref(),
                        );
                    });
                }

                PanelId::Blueprint => {
                    render_panel_frame(ctx, &panel_ctx, |ui| {
                        panels::render_blueprint_panel(
                            ui,
                            ctx,
                            &mut editor.blueprint_editor,
                            &mut editor.blueprint_canvas,
                            &editor.blueprint_nodes,
                            current_project.as_deref(),
                        );
                    });
                }

                PanelId::NodeLibrary => {
                    render_panel_frame(ctx, &panel_ctx, |ui| {
                        panels::render_node_library_panel(
                            ui,
                            &mut editor.blueprint_editor,
                            &editor.blueprint_canvas,
                            &editor.blueprint_nodes,
                        );
                    });
                }

                PanelId::Plugin(name) => {
                    render_panel_frame(ctx, &panel_ctx, |ui| {
                        ui.label(format!("Plugin: {}", name));
                    });
                }
            }
        }
    }

    // Only show settings/export dialogs in edit mode
    if !in_play_mode {
        // Ctrl+, to open settings
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(bevy_egui::egui::Key::Comma)) {
            editor.settings.show_settings_window = !editor.settings.show_settings_window;
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

        // Render settings window (floating)
        render_settings_window(ctx, &mut editor.settings, &mut editor.keybindings);

        // Render assets dialogs (create script, create folder, import)
        render_assets_dialogs(ctx, &mut editor.assets);

        // Render export dialog (floating)
        render_export_dialog(
            ctx,
            &mut editor.export_state,
            &editor.scene_state,
            current_project.as_deref(),
        );

        // Render plugin-registered panels (floating windows only for now)
        let plugin_events = render_plugin_panels(ctx, &editor.plugin_host, &mut ui_renderer);
        all_ui_events.extend(plugin_events);
    }

    // Window resize edges (only in windowed mode, not maximized)
    if !editor.window_state.is_maximized {
        render_window_resize_edges(ctx, &mut editor.window_state);
    }

    // Handle file drops globally - route to assets panel (import) or viewport (spawn)
    handle_global_file_drops(ctx, &mut editor.assets, &editor.viewport);

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

/// Render play mode overlay with status and stop hint
fn render_play_mode_overlay(ctx: &bevy_egui::egui::Context, play_mode: &mut PlayModeState) {
    use bevy_egui::egui::{self, Color32, Align2, RichText, CornerRadius, Vec2};

    // Top-center status indicator
    egui::Area::new(egui::Id::new("play_mode_status"))
        .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 80.0))
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(Color32::from_rgba_unmultiplied(0, 0, 0, 180))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(16, 8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Status icon and text
                        let (icon, color, text) = match play_mode.state {
                            PlayState::Playing => ("\u{f04b}", Color32::from_rgb(64, 200, 100), "Playing"),
                            PlayState::Paused => ("\u{f04c}", Color32::from_rgb(255, 200, 80), "Paused"),
                            PlayState::Editing => ("\u{f04d}", Color32::WHITE, "Editing"),
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
                // Image is loaded, verify it exists in the assets collection
                if images.contains(&handle) {
                    // Register with egui
                    let texture_id = contexts.add_image(EguiTextureHandle::Weak(handle.id()));
                    thumbnail_cache.texture_ids.insert(path.clone(), texture_id);
                    thumbnail_cache.loading.remove(&path);
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

/// Handle file drops globally - route to assets panel (import) or viewport (spawn)
fn handle_global_file_drops(
    ctx: &bevy_egui::egui::Context,
    assets: &mut AssetBrowserState,
    viewport: &ViewportState,
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

    for path in dropped_files {
        if assets_rect.contains(pos) && assets.current_folder.is_some() {
            // Dropped in assets panel - queue for import (copy to folder)
            info!("File dropped in assets panel, importing: {:?}", path);
            assets.pending_file_imports.push(path);
        } else if viewport_rect.contains(pos) {
            // Dropped in viewport - queue for spawning
            info!("File dropped in viewport, spawning: {:?}", path);
            assets.files_to_spawn.push(path);
        }
        // If dropped elsewhere, ignore
    }
}

/// Render invisible resize edges around the window for custom window resizing
fn render_window_resize_edges(ctx: &bevy_egui::egui::Context, window_state: &mut WindowState) {
    use bevy_egui::egui::{self, CursorIcon, Sense, Vec2, Id, Pos2};

    let screen_rect = ctx.screen_rect();
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
