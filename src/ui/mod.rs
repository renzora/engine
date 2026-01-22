mod panels;
mod style;

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiTextureHandle};

use crate::core::{
    AppState, EditorEntity, KeyBindings, SceneTabId,
    SelectionState, HierarchyState, ViewportState, SceneManagerState, AssetBrowserState, EditorSettings, WindowState,
    OrbitCameraState,
};
use crate::gizmo::GizmoState;

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
    pub orbit: Res<'w, OrbitCameraState>,
    pub keybindings: ResMut<'w, KeyBindings>,
}
use crate::node_system::NodeRegistry;
use crate::project::{AppConfig, CurrentProject};
use crate::scripting::{ScriptRegistry, RhaiScriptEngine};
use crate::viewport::{CameraPreviewImage, ViewportImage};
use panels::{
    render_assets, render_hierarchy, render_inspector, render_scene_tabs, render_script_editor,
    render_settings_window, render_splash, render_title_bar, render_toolbar, render_viewport,
    InspectorQueries, TITLE_BAR_HEIGHT,
};
pub use panels::handle_window_actions;
use style::{apply_editor_style, init_fonts};

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
    entities: Query<(Entity, &EditorEntity, Option<&ChildOf>, Option<&Children>, Option<&SceneTabId>)>,
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

    // Render custom title bar (includes menu)
    render_title_bar(
        ctx,
        &mut editor.window_state,
        &mut editor.selection,
        &mut editor.scene_state,
        &mut editor.settings,
        &mut commands,
        &mut meshes,
        &mut materials,
    );

    let toolbar_height = 36.0;

    render_toolbar(
        ctx,
        &mut editor.gizmo,
        &mut editor.settings,
        TITLE_BAR_HEIGHT,
        toolbar_height,
        1600.0, // Default width, will be constrained by panel
    );

    // Render scene tabs
    let left_panel_width = 260.0;
    let right_panel_width = 320.0;
    let scene_tabs_height = render_scene_tabs(
        ctx,
        &mut editor.scene_state,
        left_panel_width,
        right_panel_width,
        TITLE_BAR_HEIGHT + toolbar_height,
    );

    // Render bottom panel (assets)
    let bottom_panel_height = 200.0;
    render_assets(
        ctx,
        current_project.as_deref(),
        &mut editor.viewport,
        &mut editor.assets,
        &mut editor.scene_state,
        260.0,
        320.0,
        bottom_panel_height,
    );

    // Render left panel (hierarchy)
    let content_start_y = TITLE_BAR_HEIGHT + toolbar_height + scene_tabs_height;
    let viewport_height = 500.0; // Will be calculated by panels

    let active_tab = editor.scene_state.active_scene_tab;
    render_hierarchy(
        ctx,
        &mut editor.selection,
        &mut editor.hierarchy,
        &entities,
        &mut commands,
        &mut meshes,
        &mut materials,
        &node_registry,
        active_tab,
        left_panel_width,
        content_start_y,
        viewport_height,
    );

    // Render right panel (inspector)
    render_inspector(
        ctx,
        &editor.selection,
        &entities_for_inspector,
        &mut inspector_queries,
        &script_registry,
        &rhai_engine,
        right_panel_width,
        camera_preview_texture_id,
    );

    // Calculate available height for central area
    let screen_rect = ctx.screen_rect();
    let central_height = screen_rect.height() - content_start_y - editor.viewport.assets_height;

    // Render script editor if scripts are open, otherwise render viewport
    let script_editor_shown = render_script_editor(
        ctx,
        &mut editor.scene_state,
        left_panel_width,
        right_panel_width,
        content_start_y,
        central_height,
    );

    if !script_editor_shown {
        // Render central viewport
        render_viewport(
            ctx,
            &mut editor.viewport,
            &mut editor.assets,
            &editor.orbit,
            left_panel_width,
            right_panel_width,
            content_start_y,
            [1600.0, 900.0],
            viewport_height,
            viewport_texture_id,
        );
    }

    // Ctrl+, to open settings
    if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(bevy_egui::egui::Key::Comma)) {
        editor.settings.show_settings_window = !editor.settings.show_settings_window;
    }

    // Render settings window (floating)
    render_settings_window(ctx, &mut editor.settings, &mut editor.keybindings);
}
