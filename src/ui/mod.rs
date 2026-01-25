mod panels;
mod style;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiTextureHandle};

use crate::commands::CommandHistory;
use crate::core::{
    AppState, AssetLoadingProgress, ConsoleState, DefaultCameraEntity, EditorEntity, ExportState, KeyBindings, SceneTabId,
    SelectionState, HierarchyState, ViewportState, SceneManagerState, AssetBrowserState, EditorSettings, WindowState,
    OrbitCameraState, PlayModeState, PlayState,
};
use crate::gizmo::GizmoState;
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
    pub orbit: ResMut<'w, OrbitCameraState>,
    pub camera2d_state: Res<'w, Camera2DState>,
    pub keybindings: ResMut<'w, KeyBindings>,
    pub plugin_host: ResMut<'w, PluginHost>,
    pub loading_progress: Res<'w, AssetLoadingProgress>,
    pub default_camera: Res<'w, DefaultCameraEntity>,
    pub play_mode: ResMut<'w, PlayModeState>,
    pub console: ResMut<'w, ConsoleState>,
    pub command_history: ResMut<'w, CommandHistory>,
}
use crate::node_system::{self, NodeRegistry, NodeTypeMarker};
use crate::project::{AppConfig, CurrentProject};
use crate::scripting::{ScriptRegistry, RhaiScriptEngine};
use crate::viewport::{CameraPreviewImage, ViewportImage};
use crate::plugin_core::PluginHost;
use crate::ui_api::renderer::UiRenderer;
use crate::ui_api::UiEvent as InternalUiEvent;
use panels::{
    render_assets, render_export_dialog, render_hierarchy, render_inspector, render_plugin_menus, render_plugin_panels,
    render_plugin_toolbar, render_scene_tabs, render_script_editor, render_settings_window,
    render_splash, render_status_bar, render_title_bar, render_toolbar, render_viewport,
    InspectorQueries, TITLE_BAR_HEIGHT,
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
    entities: Query<(Entity, &EditorEntity, Option<&ChildOf>, Option<&Children>, Option<&SceneTabId>, Option<&node_system::SceneRoot>, Option<&NodeTypeMarker>)>,
    entities_for_inspector: Query<(Entity, &EditorEntity)>,
    mut inspector_queries: InspectorQueries,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    current_project: Option<Res<CurrentProject>>,
    node_registry: Res<NodeRegistry>,
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
        &mut commands,
        &mut meshes,
        &mut materials,
        &editor.plugin_host,
        &mut editor.command_history,
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
        &node_registry,
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut editor.selection,
        &mut editor.hierarchy,
        &mut editor.play_mode,
    );
    all_ui_events.extend(toolbar_events);

    // Check if we're in play mode
    let in_play_mode = editor.play_mode.is_in_play_mode();

    // Use stored panel widths from viewport state (or 0 in play mode)
    let stored_hierarchy_width = if in_play_mode { 0.0 } else { editor.viewport.hierarchy_width };
    let stored_inspector_width = if in_play_mode { 0.0 } else { editor.viewport.inspector_width };
    let stored_assets_height = if in_play_mode { 0.0 } else { editor.viewport.assets_height };

    // In play mode, skip scene tabs and panels
    let (scene_tabs_height, actual_hierarchy_width, actual_inspector_width) = if in_play_mode {
        (0.0, 0.0, 0.0)
    } else {
        // Render scene tabs
        let tabs_height = render_scene_tabs(
            ctx,
            &mut editor.scene_state,
            stored_hierarchy_width,
            stored_inspector_width,
            TITLE_BAR_HEIGHT + toolbar_height,
        );

        // Render bottom panel (assets + console tabs)
        let bottom_events = render_assets(
            ctx,
            current_project.as_deref(),
            &mut editor.viewport,
            &mut editor.assets,
            &mut editor.scene_state,
            &mut editor.console,
            stored_hierarchy_width,
            stored_inspector_width,
            stored_assets_height,
            &editor.plugin_host,
            &mut ui_renderer,
        );
        all_ui_events.extend(bottom_events);

        // Render left panel (hierarchy) - returns actual width after resize
        let content_start_y = TITLE_BAR_HEIGHT + toolbar_height + tabs_height;

        let active_tab = editor.scene_state.active_scene_tab;
        let (hierarchy_events, hierarchy_width, hierarchy_changed) = render_hierarchy(
            ctx,
            &mut editor.selection,
            &mut editor.hierarchy,
            &entities,
            &mut commands,
            &mut meshes,
            &mut materials,
            &node_registry,
            active_tab,
            stored_hierarchy_width,
            &editor.plugin_host,
            &mut editor.assets,
            &editor.default_camera,
            &mut editor.command_history,
            &mut ui_renderer,
        );
        all_ui_events.extend(hierarchy_events);

        // Mark scene as modified if hierarchy made changes
        if hierarchy_changed {
            editor.scene_state.mark_modified();
        }

        // Update stored hierarchy width
        editor.viewport.hierarchy_width = hierarchy_width;

        // Render right panel (inspector) - returns actual width after resize and change flag
        let (inspector_events, inspector_width, inspector_changed) = render_inspector(
            ctx,
            &editor.selection,
            &entities_for_inspector,
            &mut inspector_queries,
            &script_registry,
            &rhai_engine,
            stored_inspector_width,
            camera_preview_texture_id,
            &editor.plugin_host,
            &mut ui_renderer,
        );
        all_ui_events.extend(inspector_events);

        // Mark scene as modified if inspector made changes
        if inspector_changed {
            editor.scene_state.mark_modified();
        }

        // Update stored inspector width
        editor.viewport.inspector_width = inspector_width;

        (tabs_height, hierarchy_width, inspector_width)
    };

    // Calculate available height for central area
    let content_start_y = TITLE_BAR_HEIGHT + toolbar_height + scene_tabs_height;
    let screen_rect = ctx.screen_rect();
    let status_bar_height = if in_play_mode { 0.0 } else { 24.0 };
    let central_height = screen_rect.height() - content_start_y - stored_assets_height - status_bar_height;

    // In play mode, skip script editor and render full viewport
    if !in_play_mode {
        // Render script editor if scripts are open, otherwise render viewport
        let script_editor_shown = render_script_editor(
            ctx,
            &mut editor.scene_state,
            current_project.as_deref(),
            actual_hierarchy_width,
            actual_inspector_width,
            content_start_y,
            central_height,
        );

        if !script_editor_shown {
            // Render central viewport (docked between panels)
            render_viewport(
                ctx,
                &mut editor.viewport,
                &mut editor.assets,
                &mut editor.orbit,
                &editor.camera2d_state,
                actual_hierarchy_width,
                actual_inspector_width,
                content_start_y,
                [1600.0, 900.0],
                central_height,
                viewport_texture_id,
            );
        }
    } else {
        // In play mode, render full-screen viewport (no panels)
        render_viewport(
            ctx,
            &mut editor.viewport,
            &mut editor.assets,
            &mut editor.orbit,
            &editor.camera2d_state,
            0.0, // No left panel
            0.0, // No right panel
            content_start_y,
            [1600.0, 900.0],
            central_height,
            viewport_texture_id,
        );

        // Render play mode overlay with info
        render_play_mode_overlay(ctx, &mut editor.play_mode);
    }

    // Only show settings/export dialogs in edit mode
    if !in_play_mode {
        // Ctrl+, to open settings
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(bevy_egui::egui::Key::Comma)) {
            editor.settings.show_settings_window = !editor.settings.show_settings_window;
        }

        // Render settings window (floating)
        render_settings_window(ctx, &mut editor.settings, &mut editor.keybindings);

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
