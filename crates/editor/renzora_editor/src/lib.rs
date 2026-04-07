//! Renzora Editor — pluggable editor shell with docking panel system.
//!
//! The UI framework (docking, panels, widgets, theme) lives in `renzora_ui`.
//! This crate adds the Bevy plugin that wires it all together.

pub mod bevy_inspectors;
pub mod camera;
pub mod commands;
pub mod ext;
pub mod inspector_registry;
pub mod selection;
pub mod settings;
pub mod spawn_registry;

// Re-export full UI API so downstream crates can use `renzora_editor_framework::DockTree` etc.
pub use renzora_ui::*;

pub use commands::EditorCommands;
pub use inspector_registry::{
    FieldDef, FieldType, FieldValue, InspectorEntry, InspectorRegistry,
};
pub use ext::{AppEditorExt, InspectableComponent};
pub use renzora_macros::{Inspectable, post_process};
pub use selection::EditorSelection;
pub use settings::{CustomFonts, EditorSettings, MonoFont, SelectionHighlightMode, SettingsTab, UiFont};

// Re-export core marker components so downstream crates can use `renzora_editor_framework::HideInHierarchy` etc.
pub use renzora_core::{HideInHierarchy, EditorLocked, EditorCamera};
pub use renzora_splash::SplashState;

/// Optional label color for an entity row in the hierarchy.
#[derive(Component)]
pub struct EntityLabelColor(pub [u8; 3]);

/// Sort order for root-level entities in the hierarchy panel.
/// Lower values appear first. Entities without this component sort last.
#[derive(Component, Clone, Copy)]
pub struct HierarchyOrder(pub u32);

pub use renzora_core::EntityTag;

/// Filter mode for the hierarchy panel. Other panels can set this to restrict
/// which entities are shown (e.g. UI workspace only shows cameras + canvases).
#[derive(Resource, Default, Clone, PartialEq, Eq, Debug)]
pub enum HierarchyFilter {
    /// Show all entities (default).
    #[default]
    All,
    /// Show only entities that have at least one of the listed component type names.
    /// Component names are matched via Bevy's `AppTypeRegistry`.
    OnlyWithComponents(Vec<&'static str>),
}

pub use spawn_registry::{ComponentIconEntry, ComponentIconRegistry, EntityPreset, SpawnRegistry};

/// Gizmo transform mode — shared so both the gizmo and viewport toolbar can access it.
#[derive(bevy::prelude::Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum GizmoMode {
    /// Select mode — click to select, drag for box/marquee selection.
    #[default]
    Select,
    Translate,
    Rotate,
    Scale,
}

/// Unified active tool — replaces scattered `GizmoMode`, `TerrainToolState`, `FoliageToolState`.
///
/// Only one tool is active at a time. The viewport toolbar sets this directly.
/// Downstream crates read this to decide whether their systems should run.
#[derive(bevy::prelude::Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActiveTool {
    #[default]
    Select,
    Translate,
    Rotate,
    Scale,
    TerrainSculpt,
    TerrainPaint,
    FoliagePaint,
}

impl ActiveTool {
    /// Returns the equivalent `GizmoMode` if this is a gizmo tool, `None` for terrain/foliage tools.
    pub fn gizmo_mode(&self) -> Option<GizmoMode> {
        match self {
            Self::Select => Some(GizmoMode::Select),
            Self::Translate => Some(GizmoMode::Translate),
            Self::Rotate => Some(GizmoMode::Rotate),
            Self::Scale => Some(GizmoMode::Scale),
            _ => None,
        }
    }

    pub fn is_terrain(&self) -> bool {
        matches!(self, Self::TerrainSculpt | Self::TerrainPaint)
    }

    pub fn is_terrain_or_foliage(&self) -> bool {
        matches!(self, Self::TerrainSculpt | Self::TerrainPaint | Self::FoliagePaint)
    }
}

use std::sync::atomic::{AtomicBool, Ordering};

use bevy::prelude::*;
use bevy::ecs::system::SystemState;
use bevy_egui::{EguiContexts, EguiGlobalSettings, EguiPlugin, EguiPrimaryContextPass};
use bevy_egui::egui;
use renzora_theme::ThemeManager;

// Module and type names come through `pub use renzora_ui::*` above.
// Use fully-qualified `renzora_ui::module::fn` for sub-module function calls.

static FONTS_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Tracks the previously applied font selections so we only reload fonts when they change.
#[derive(Resource, Default, Clone)]
struct PreviousFontSettings {
    ui_font: UiFont,
    mono_font: MonoFont,
}

/// Throttle timer for rescanning project custom fonts and themes directories.
#[derive(Resource)]
struct ProjectScanTimer(f64);

/// Cached SystemState for the editor exclusive system (avoids per-frame allocation).
#[derive(Resource)]
struct EditorEguiState(SystemState<EguiContexts<'static, 'static>>);

/// Whether the editor overlay is active or hidden.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EditorState {
    #[default]
    Active,
    Inactive,
}

/// Plugin that adds the Renzora editor overlay to any Bevy app.
pub struct RenzoraEditorPlugin;

impl Plugin for RenzoraEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] RenzoraEditorPlugin");
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin::default());
        }

        // Disable auto-creation of the primary Egui context on the first camera.
        // We explicitly attach PrimaryEguiContext to the UI camera so egui renders
        // to the window, not to the editor's offscreen 3D camera.
        app.world_mut()
            .resource_mut::<EguiGlobalSettings>()
            .auto_create_primary_context = false;

        app.init_state::<EditorState>()
            .init_resource::<ThemeManager>()
            .init_resource::<PanelRegistry>()
            .init_resource::<DockingState>()
            .init_resource::<FloatingPanels>()
            .init_resource::<LayoutManager>()
            .init_resource::<DocumentTabState>()
            .init_resource::<EditorSelection>()
            .init_resource::<EditorCommands>()
            .init_resource::<InspectorRegistry>()
            .init_resource::<SpawnRegistry>()
            .init_resource::<ComponentIconRegistry>();

        // Register inspector entries and icons for core Bevy components
        bevy_inspectors::register_bevy_inspectors(
            &mut app.world_mut().resource_mut::<InspectorRegistry>(),
        );
        bevy_inspectors::register_bevy_icons(
            &mut app.world_mut().resource_mut::<ComponentIconRegistry>(),
        );
        bevy_inspectors::register_bevy_presets(
            &mut app.world_mut().resource_mut::<SpawnRegistry>(),
        );

        app
            .init_resource::<EditorSettings>()
            .init_resource::<ActiveTool>()
            .init_resource::<GizmoMode>()
            .init_resource::<CustomFonts>()
            .init_resource::<HierarchyFilter>()
            .init_resource::<renzora_ui::Toasts>()
            .add_systems(PostStartup, camera::spawn_ui_camera)
            .add_systems(
                EguiPrimaryContextPass,
                editor_ui_system.run_if(in_state(SplashState::Editor)),
            )
            .add_observer(show_script_reload_toasts)
            .add_systems(OnEnter(SplashState::Editor), wire_theme_project_path)
            .add_systems(
                Update,
                sync_active_tool_to_gizmo_mode.run_if(in_state(SplashState::Editor)),
            )
            ;
    }
}

/// Observer: show toast notifications when scripts are hot-reloaded.
fn show_script_reload_toasts(
    trigger: On<renzora_core::ScriptsReloaded>,
    mut toasts: ResMut<renzora_ui::Toasts>,
) {
    let event = trigger.event();
    match event.names.len() {
        0 => {}
        1 => { toasts.success(format!("Reloaded {}", event.names[0])); }
        n => { toasts.success(format!("Reloaded {} scripts", n)); }
    }
}

/// Keep `GizmoMode` in sync with `ActiveTool` so gizmo systems that still read
/// `GizmoMode` continue to work during the migration.
fn sync_active_tool_to_gizmo_mode(
    active_tool: Res<ActiveTool>,
    gizmo_mode: Option<ResMut<GizmoMode>>,
) {
    let Some(mut gizmo_mode) = gizmo_mode else { return };
    if !active_tool.is_changed() {
        return;
    }
    if let Some(mode) = active_tool.gizmo_mode() {
        if *gizmo_mode != mode {
            *gizmo_mode = mode;
        }
    } else {
        if *gizmo_mode != GizmoMode::Select {
            *gizmo_mode = GizmoMode::Select;
        }
    }
}

/// When the editor state is entered, tell the ThemeManager about the project
/// directory so it can discover custom `.toml` themes in `<project>/themes/`.
fn wire_theme_project_path(
    project: Option<Res<renzora_core::CurrentProject>>,
    mut theme_manager: ResMut<ThemeManager>,
) {
    if let Some(project) = project {
        theme_manager.set_project_path(&project.path);
    }
}

/// How often (in seconds) to rescan the project `fonts/` and `themes/` directories.
const PROJECT_SCAN_INTERVAL: f64 = 2.0;

/// Scan the project's `fonts/` directory for custom `.ttf`/`.otf` files,
/// load any new ones into egui's font data, and update the `CustomFonts` resource.
fn load_project_custom_fonts(world: &mut World, ctx: &egui::Context) {
    let fonts_dir = match world.get_resource::<renzora_core::CurrentProject>() {
        Some(project) => project.resolve_path("fonts"),
        None => return,
    };
    let entries = match std::fs::read_dir(&fonts_dir) {
        Ok(entries) => entries,
        Err(_) => return, // No fonts/ directory — that's fine
    };

    let existing = world
        .get_resource::<CustomFonts>()
        .cloned()
        .unwrap_or_default();

    let mut fonts = ctx.fonts(|f| f.definitions().clone());
    let mut new_names = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext != "ttf" && ext != "otf" {
            continue;
        }
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_owned(),
            None => continue,
        };
        // Already loaded (built-in or previously discovered custom)
        if fonts.font_data.contains_key(&stem) {
            continue;
        }
        let data = match std::fs::read(&path) {
            Ok(d) => d,
            Err(e) => {
                bevy::log::warn!("Failed to read custom font {}: {}", path.display(), e);
                continue;
            }
        };
        fonts
            .font_data
            .insert(stem.clone().into(), egui::FontData::from_owned(data).into());
        new_names.push(stem);
    }

    if !new_names.is_empty() {
        bevy::log::info!(
            "[editor] Loaded {} custom font(s) from {}",
            new_names.len(),
            fonts_dir.display()
        );
        ctx.set_fonts(fonts);

        let mut all_names = existing.names;
        all_names.extend(new_names);
        all_names.sort();
        world.insert_resource(CustomFonts { names: all_names });
    } else if !world.contains_resource::<CustomFonts>() {
        world.insert_resource(CustomFonts::default());
    }
}

/// Main exclusive-ish editor UI system.
///
/// Uses `SystemState` to extract the egui context, clones it (Arc-backed, cheap),
/// then renders everything with `&World` access for panels.
pub fn editor_ui_system(world: &mut World) {
    // 1. Get egui context (cached to avoid per-frame allocation)
    if !world.contains_resource::<EditorEguiState>() {
        let s = EditorEguiState(SystemState::new(world));
        world.insert_resource(s);
    }
    let mut cached = world.remove_resource::<EditorEguiState>().unwrap();
    let mut contexts = cached.0.get_mut(world);
    let ctx = match contexts.ctx_mut() {
        Ok(c) => c.clone(), // Arc clone — cheap
        Err(_) => {
            world.insert_resource(cached);
            return;
        }
    };
    cached.0.apply(world);
    world.insert_resource(cached);

    // 2. Init fonts once, then apply theme + font settings
    let settings = world
        .get_resource::<EditorSettings>()
        .cloned()
        .unwrap_or_default();
    if !FONTS_INITIALIZED.load(Ordering::Relaxed) {
        renzora_ui::theme::init_fonts(
            &ctx,
            settings.ui_font.font_key(),
            settings.mono_font.font_key(),
        );
        // Scan project fonts/ directory and load custom fonts
        load_project_custom_fonts(world, &ctx);
        world.insert_resource(PreviousFontSettings {
            ui_font: settings.ui_font,
            mono_font: settings.mono_font,
        });
        FONTS_INITIALIZED.store(true, Ordering::Relaxed);
    } else {
        // React to font family changes from the settings panel
        let prev = world
            .get_resource::<PreviousFontSettings>()
            .cloned()
            .unwrap_or_default();
        if prev.ui_font != settings.ui_font {
            renzora_ui::theme::set_ui_font(&ctx, settings.ui_font.font_key());
        }
        if prev.mono_font != settings.mono_font {
            renzora_ui::theme::set_mono_font(&ctx, settings.mono_font.font_key());
        }
        if prev.ui_font != settings.ui_font || prev.mono_font != settings.mono_font {
            world.insert_resource(PreviousFontSettings {
                ui_font: settings.ui_font.clone(),
                mono_font: settings.mono_font.clone(),
            });
        }

        // Periodically rescan project fonts/ and themes/ directories
        let now = world
            .get_resource::<Time>()
            .map(|t| t.elapsed_secs_f64())
            .unwrap_or(0.0);
        let last_scan = world
            .get_resource::<ProjectScanTimer>()
            .map(|t| t.0)
            .unwrap_or(0.0);
        if now - last_scan >= PROJECT_SCAN_INTERVAL {
            load_project_custom_fonts(world, &ctx);
            if let Some(mut tm) = world.get_resource_mut::<ThemeManager>() {
                tm.scan_themes();
            }
            world.insert_resource(ProjectScanTimer(now));
        }
    }
    let theme = world
        .get_resource::<ThemeManager>()
        .map(|tm| tm.active_theme.clone())
        .unwrap_or_default();
    renzora_ui::theme::apply_theme(&ctx, &theme, settings.font_size);

    // 3. Temporarily remove PanelRegistry so we can pass &World to panels
    let registry = world
        .remove_resource::<PanelRegistry>()
        .unwrap_or_default();

    // 4. Get layout manager for title bar
    let layout_manager = world
        .get_resource::<LayoutManager>()
        .cloned()
        .unwrap_or_default();

    // 4.5. Check play mode — in Playing mode, skip editor UI and show play overlay
    let play_state = world
        .get_resource::<renzora_core::PlayModeState>()
        .map(|pm| pm.state)
        .unwrap_or(renzora_core::PlayState::Editing);

    if matches!(play_state, renzora_core::PlayState::Playing | renzora_core::PlayState::Paused) {
        world.insert_resource(registry);

        // Game camera renders directly to window, UI camera is disabled.
        // Escape handling is done in handle_play_shortcuts (Bevy input system).

        // Process play mode requests (scripts-only transitions)
        process_play_mode_requests(world);

        // Drain editor commands
        let cmds = world
            .get_resource::<EditorCommands>()
            .map(|ec| ec.drain())
            .unwrap_or_default();
        for cmd in cmds {
            cmd(world);
        }

        return;
    }

    // 5. Title bar (top) — returns action
    let play_info = world
        .get_resource::<renzora_core::PlayModeState>()
        .map(|pm| renzora_ui::title_bar::PlayModeInfo {
            is_playing: pm.is_playing(),
            is_paused: pm.is_paused(),
            is_scripts_only: pm.is_scripts_only(),
        })
        .unwrap_or_default();
    // Auth state comes from the AuthBridge resource (synced by AuthPlugin).
    let auth_bridge = world
        .get_resource::<renzora_core::AuthBridge>()
        .cloned()
        .unwrap_or_default();
    let sign_in_open = auth_bridge.window_open;
    let signed_in_username = auth_bridge.signed_in_username;
    let title_action = renzora_ui::title_bar::render_title_bar(&ctx, &theme, &registry, &layout_manager, &play_info, sign_in_open, signed_in_username.as_deref());

    // 6. Document tabs (below title bar)
    let doc_tab_state = world
        .get_resource::<DocumentTabState>()
        .cloned()
        .unwrap_or_default();
    let doc_tab_action = renzora_ui::document_tabs::render_document_tabs(&ctx, &doc_tab_state, &theme);

    // 7. Status bar (bottom)
    render_plugin_status_bar(&ctx, &theme, world);

    // 8. Get current drag state (read-only snapshot for rendering)
    let drag_snapshot = world.get_resource::<DragState>().map(|d| DragState {
        panel_id: d.panel_id.clone(),
        origin: d.origin,
        is_detached: d.is_detached,
    });

    // 9. Central panel with dock tree
    let render_result = egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(theme.surfaces.extreme.to_color32()))
        .show(&ctx, |ui| {
            let rect = ui.available_rect_before_wrap();
            let base_id = egui::Id::new("dock_tree");

            let docking = world.get_resource::<DockingState>();
            let tree = match docking {
                Some(ds) => &ds.tree,
                None => return renzora_ui::dock_renderer::DockRenderResult::default(),
            };

            renzora_ui::dock_renderer::render_dock_tree(
                ui,
                tree,
                rect,
                &registry,
                world,
                base_id,
                &theme,
                drag_snapshot.as_ref(),
            )
        })
        .inner;

    // 9b. Render floating panels
    let floating_result = {
        let mut floating = world
            .remove_resource::<FloatingPanels>()
            .unwrap_or_default();
        let fr = renzora_ui::floating::render_floating_panels(
            &ctx,
            &mut floating,
            &registry,
            world,
            &theme,
        );
        world.insert_resource(floating);
        fr
    };

    // 10. Re-insert the registry
    world.insert_resource(registry);

    // 11. Apply deferred mutations from rendering

    // A) Handle drag start
    if let Some(ref panel_id) = render_result.drag_started {
        if world.get_resource::<DragState>().is_none() {
            world.insert_resource(DragState {
                panel_id: panel_id.clone(),
                origin: ctx.pointer_latest_pos().unwrap_or_default(),
                is_detached: false,
            });
        }
    }

    // B) Handle drag in progress — update detach state
    let mut should_drop = false;
    if let Some(mut drag) = world.get_resource_mut::<DragState>() {
        if let Some(pos) = ctx.pointer_latest_pos() {
            if !drag.is_detached && pos.distance(drag.origin) > 5.0 {
                drag.is_detached = true;
            }
        }
        // Check if pointer released while detached
        if drag.is_detached && !ctx.input(|i| i.pointer.any_down()) {
            should_drop = true;
        }
    }

    // C) Handle drop — apply tree mutations (re-dock from floating or rearrange docked)
    if should_drop {
        if let Some(drag) = world.remove_resource::<DragState>() {
            if let Some(target) = render_result.drop_target {
                // Remove from floating if re-docking
                if let Some(mut floating) = world.get_resource_mut::<FloatingPanels>() {
                    floating.remove(&drag.panel_id);
                }
                // Apply dock tree mutations
                if let Some(mut docking) = world.get_resource_mut::<DockingState>() {
                    docking.tree.remove_panel(&drag.panel_id);
                    match target.zone {
                        renzora_ui::DropZone::Tab(idx) => {
                            docking.tree.add_tab_at(&target.panel_id, drag.panel_id, idx);
                        }
                        renzora_ui::DropZone::Center => {
                            docking.tree.add_tab(&target.panel_id, drag.panel_id);
                        }
                        renzora_ui::DropZone::Left | renzora_ui::DropZone::Right | renzora_ui::DropZone::Top | renzora_ui::DropZone::Bottom => {
                            docking.tree.split_at(&target.panel_id, drag.panel_id, target.zone);
                        }
                    }
                }
            }
            // No target → snap back (no-op)
        }
    }

    // C2) Handle Ctrl+drag undock — immediately float the panel
    if let Some(ref panel_id) = render_result.ctrl_drag_undock {
        let drop_pos = ctx.pointer_latest_pos().unwrap_or_default();
        undock_panel_to_floating(world, panel_id, drop_pos);
    }

    // C3) Handle right-click "Undock" context menu action
    if let Some(ref panel_id) = render_result.context_menu_undock {
        let drop_pos = ctx.pointer_latest_pos().unwrap_or_default();
        undock_panel_to_floating(world, panel_id, drop_pos);
    }

    // D) Handle cancel (Escape key)
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        world.remove_resource::<DragState>();
    }

    // D2) Handle asset drag in progress — update detach state, cancel, release
    {
        let mut asset_should_cancel = false;
        if let Some(mut asset_drag) = world.get_resource_mut::<AssetDragPayload>() {
            if let Some(pos) = ctx.pointer_latest_pos() {
                if !asset_drag.is_detached && pos.distance(asset_drag.origin) > 5.0 {
                    asset_drag.is_detached = true;
                }
            }
            // Cancel on Escape
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                asset_should_cancel = true;
            }
            // Release — drop targets consume this in their own rendering;
            // if pointer released with no target hit, just cancel.
            if asset_drag.is_detached && !ctx.input(|i| i.pointer.any_down()) {
                asset_should_cancel = true;
            }
        }
        if asset_should_cancel {
            world.remove_resource::<AssetDragPayload>();
        }
    }

    // E) Draw floating ghost overlays
    // Asset drag ghost
    if let Some(asset_drag) = world.get_resource::<AssetDragPayload>() {
        if asset_drag.is_detached {
            if let Some(pos) = ctx.pointer_latest_pos() {
                renzora_ui::asset_drag::draw_asset_drag_ghost(&ctx, asset_drag, pos, &theme);
            }
        }
    }
    // Panel drag ghost
    if let Some(drag) = world.get_resource::<DragState>() {
        if drag.is_detached {
            if let Some(pos) = ctx.pointer_latest_pos() {
                let registry = world.get_resource::<PanelRegistry>();
                let title = registry
                    .and_then(|r| r.get(&drag.panel_id).map(|p| p.title().to_string()))
                    .unwrap_or_else(|| drag.panel_id.clone());
                renzora_ui::drag_drop::draw_drag_ghost(&ctx, &title, pos, &theme);
            }
        }
    }

    // E2) Handle floating panel close
    if let Some(ref panel_id) = floating_result.panel_to_close {
        if let Some(mut floating) = world.get_resource_mut::<FloatingPanels>() {
            floating.remove(panel_id);
        }
    }

    // E3) Handle "Dock" button or right-click "Dock" on a floating panel
    if let Some(ref panel_id) = floating_result.panel_to_dock {
        dock_panel_to_default(world, panel_id);
    }

    // E4) Handle grip drag from floating panel — start a DragState for dock-drop
    if let Some(ref panel_id) = floating_result.redock_drag_started {
        if world.get_resource::<DragState>().is_none() {
            world.insert_resource(DragState {
                panel_id: panel_id.clone(),
                origin: ctx.pointer_latest_pos().unwrap_or_default(),
                is_detached: true,
            });
        }
    }

    // F) Handle panel close
    if let Some(ref panel_id) = render_result.panel_to_close {
        if let Some(mut docking) = world.get_resource_mut::<DockingState>() {
            docking.tree.remove_panel(panel_id);
        }
    }

    // G) Handle tab switch
    if let Some((_, ref new_active)) = render_result.new_active_tab {
        if let Some(mut docking) = world.get_resource_mut::<DockingState>() {
            docking.tree.set_active_tab(new_active);
        }
    }

    // H) Handle resize
    if let Some((ref path, new_ratio)) = render_result.ratio_update {
        if let Some(mut docking) = world.get_resource_mut::<DockingState>() {
            docking.tree.update_ratio(path, new_ratio);
        }
    }

    // I) Handle add panel
    if let Some((ref sibling, ref new_panel)) = render_result.panel_to_add {
        if let Some(mut docking) = world.get_resource_mut::<DockingState>() {
            docking.tree.add_tab(sibling, new_panel.clone());
        }
    }

    // K) Handle title bar actions
    match title_action {
        TitleBarAction::SwitchLayout(i) => switch_layout(world, i),
        TitleBarAction::NewProject => handle_new_project(world),
        TitleBarAction::OpenProject => handle_open_project(world),
        TitleBarAction::NewScene => {
            world.insert_resource(renzora_core::NewSceneRequested);
        }
        TitleBarAction::OpenScene => {
            world.insert_resource(renzora_core::OpenSceneRequested);
        }
        TitleBarAction::Save => {
            world.insert_resource(renzora_core::SaveSceneRequested);
        }
        TitleBarAction::SaveAs => {
            world.insert_resource(renzora_core::SaveAsSceneRequested);
        }
        TitleBarAction::Export => {
            world.insert_resource(renzora_core::ExportRequested);
        }
        TitleBarAction::ToggleSettings => {
            if let Some(mut settings) = world.get_resource_mut::<EditorSettings>() {
                settings.show_settings = !settings.show_settings;
            }
        }
        TitleBarAction::ToggleSignIn => {
            world.insert_resource(renzora_core::AuthToggleWindowRequest);
        }
        TitleBarAction::OpenUserSettings => {
            if let Some(mut settings) = world.get_resource_mut::<EditorSettings>() {
                settings.show_settings = !settings.show_settings;
            }
        }
        TitleBarAction::OpenUserLibrary => {
            // Switch to Hub layout
            switch_layout_by_name(world, "Hub");
        }
        TitleBarAction::SignOut => {
            world.insert_resource(renzora_core::AuthSignOutRequest);
        }
        TitleBarAction::Play => {
            if let Some(mut pm) = world.get_resource_mut::<renzora_core::PlayModeState>() {
                pm.request_play = true;
            }
        }
        TitleBarAction::Stop => {
            if let Some(mut pm) = world.get_resource_mut::<renzora_core::PlayModeState>() {
                pm.request_stop = true;
            }
        }
        TitleBarAction::Pause => {
            if let Some(mut pm) = world.get_resource_mut::<renzora_core::PlayModeState>() {
                pm.request_pause = true;
            }
        }
        TitleBarAction::ScriptsOnly => {
            if let Some(mut pm) = world.get_resource_mut::<renzora_core::PlayModeState>() {
                pm.request_scripts_only = true;
            }
        }
        TitleBarAction::StartTutorial => {
            world.insert_resource(renzora_core::TutorialRequested);
        }
        TitleBarAction::None => {}
    }

    // L) Handle document tab actions
    match doc_tab_action {
        DocTabAction::Activate(idx) => {
            let ids = world
                .get_resource_mut::<DocumentTabState>()
                .and_then(|mut ts| ts.activate_tab(idx));
            if let Some((old_id, new_id)) = ids {
                world.insert_resource(renzora_core::TabSwitchRequest { old_tab_id: old_id, new_tab_id: new_id });
            }
        }
        DocTabAction::Close(idx) => {
            // If closing the active tab, switch to adjacent first
            let switch_and_close = {
                if let Some(ts) = world.get_resource::<DocumentTabState>() {
                    if idx == ts.active_tab && ts.tabs.len() > 1 {
                        let new_active = if idx + 1 < ts.tabs.len() { idx + 1 } else { idx.saturating_sub(1) };
                        let old_id = ts.tabs[idx].id;
                        let new_id = ts.tabs[new_active].id;
                        Some((old_id, new_id, new_active))
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            if let Some((old_id, new_id, new_active)) = switch_and_close {
                // Activate adjacent tab first
                if let Some(mut ts) = world.get_resource_mut::<DocumentTabState>() {
                    ts.active_tab = new_active;
                }
                world.insert_resource(renzora_core::TabSwitchRequest { old_tab_id: old_id, new_tab_id: new_id });
            }

            // Close the tab and clean up buffer
            let closed_id = world
                .get_resource_mut::<DocumentTabState>()
                .and_then(|mut ts| ts.close_tab(idx));
            if let Some(id) = closed_id {
                if let Some(mut buffers) = world.get_resource_mut::<renzora_core::SceneTabBuffers>() {
                    buffers.buffers.remove(&id);
                }
            }
        }
        DocTabAction::Reorder(from, to) => {
            if let Some(mut ts) = world.get_resource_mut::<DocumentTabState>() {
                ts.reorder(from, to);
            }
        }
        DocTabAction::AddNew => {
            let (old_id, new_id) = {
                if let Some(mut ts) = world.get_resource_mut::<DocumentTabState>() {
                    let old_id = ts.active_tab_id().unwrap_or(0);
                    let idx = ts.add_tab("Untitled Scene".into(), None);
                    ts.active_tab = idx;
                    let new_id = ts.tabs[idx].id;
                    (old_id, new_id)
                } else {
                    (0, 0)
                }
            };
            if old_id != 0 {
                world.insert_resource(renzora_core::TabSwitchRequest { old_tab_id: old_id, new_tab_id: new_id });
            }
        }
        DocTabAction::None => {}
    }


    // Auth window rendering is handled by AuthPlugin (renzora_auth).
    // React to successful sign-in by switching to the Hub layout.
    if world.remove_resource::<renzora_core::AuthJustSignedIn>().is_some() {
        switch_layout_by_name(world, "Hub");
    }

    // Toast notifications
    {
        let current_time = world.resource::<bevy::prelude::Time>().elapsed_secs_f64();
        if let Some(mut toasts) = world.get_resource_mut::<renzora_ui::Toasts>() {
            toasts.show(&ctx, current_time);
        }
    }

    // J2) Handle one-shot shortcut resources
    if world.remove_resource::<renzora_core::ToggleSettingsRequested>().is_some() {
        if let Some(mut settings) = world.get_resource_mut::<EditorSettings>() {
            settings.show_settings = !settings.show_settings;
        }
    }

    // K) Drain and execute deferred editor commands (from inspector, etc.)
    let cmds = world
        .get_resource::<EditorCommands>()
        .map(|ec| ec.drain())
        .unwrap_or_default();
    for cmd in cmds {
        cmd(world);
    }

    // L2) Process play mode state transitions
    process_play_mode_requests(world);
}

/// Switch to a layout by index.
fn switch_layout(world: &mut World, index: usize) {
    let new_tree = world
        .get_resource::<LayoutManager>()
        .and_then(|lm| lm.layouts.get(index).map(|l| l.tree.clone()));
    if let Some(tree) = new_tree {
        if let Some(mut docking) = world.get_resource_mut::<DockingState>() {
            docking.tree = tree;
        }
        if let Some(mut layout_mgr) = world.get_resource_mut::<LayoutManager>() {
            layout_mgr.active_index = index;
        }
        // Clear any floating panels when switching layouts
        if let Some(mut floating) = world.get_resource_mut::<FloatingPanels>() {
            floating.panels.clear();
        }
    }
}

/// Switch to a layout by name.
fn switch_layout_by_name(world: &mut World, name: &str) {
    let index = world
        .get_resource::<LayoutManager>()
        .and_then(|lm| lm.layouts.iter().position(|l| l.name == name));
    if let Some(i) = index {
        switch_layout(world, i);
    }
}

/// Re-dock a floating panel back into the dock tree at a reasonable location.
fn dock_panel_to_default(world: &mut World, panel_id: &str) {
    // Remove from floating
    if let Some(mut floating) = world.get_resource_mut::<FloatingPanels>() {
        floating.remove(panel_id);
    }

    // Re-add to the dock tree
    if let Some(mut docking) = world.get_resource_mut::<DockingState>() {
        let all_panels = docking.tree.all_panels();
        if let Some(first) = all_panels.first() {
            // Add as a tab next to the first panel in the tree
            docking.tree.add_tab(first, panel_id.to_string());
        } else {
            // Tree is empty — set as root leaf
            docking.tree = DockTree::leaf(panel_id);
        }
    }
}

/// Remove a panel from the dock tree and add it as a floating window.
fn undock_panel_to_floating(world: &mut World, panel_id: &str, pos: egui::Pos2) {
    let size = world
        .get_resource::<PanelRegistry>()
        .and_then(|r| r.get(panel_id))
        .map(|p| {
            let min = p.min_size();
            egui::Vec2::new(min[0].max(400.0), min[1].max(300.0))
        })
        .unwrap_or(egui::Vec2::new(400.0, 300.0));
    let win_pos = egui::Pos2::new(pos.x - size.x / 2.0, pos.y - 14.0);

    if let Some(mut docking) = world.get_resource_mut::<DockingState>() {
        docking.tree.remove_panel(panel_id);
    }
    if let Some(mut floating) = world.get_resource_mut::<FloatingPanels>() {
        floating.add(panel_id.to_string(), win_pos, size);
    }
}

/// Handle "New Project" — close current project and return to splash screen.
fn handle_new_project(world: &mut World) {
    world.remove_resource::<renzora_core::CurrentProject>();
    world
        .resource_mut::<NextState<SplashState>>()
        .set(SplashState::Splash);
    info!("Returning to splash screen for new project");
}

/// Handle "Open Project" — file dialog, validate, then reopen via splash.
fn handle_open_project(world: &mut World) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let file = rfd::FileDialog::new()
            .set_title("Open Project")
            .add_filter("Project File", &["toml"])
            .pick_file();

        let Some(file) = file else { return };

        let project = match renzora_core::open_project(&file) {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to open project: {}", e);
                rfd::MessageDialog::new()
                    .set_title("Invalid Project")
                    .set_description(&format!("Failed to open project: {}", e))
                    .set_buttons(rfd::MessageButtons::Ok)
                    .show();
                return;
            }
        };

        // Update recent projects
        if let Some(mut app_config) = world.get_resource_mut::<renzora_splash::AppConfig>() {
            app_config.add_recent_project(project.path.clone());
            let _ = app_config.save();
        }

        world.insert_resource(project);
        world
            .resource_mut::<NextState<SplashState>>()
            .set(SplashState::Splash);
        world.insert_resource(renzora_splash::PendingProjectReopen);
        info!("Opening project...");
    }

    #[cfg(target_arch = "wasm32")]
    {
        let _ = world;
        warn!("Open Project is not available in the browser");
    }
}

/// Process play mode request flags and apply state transitions.
fn process_play_mode_requests(world: &mut World) {
    use renzora_core::PlayState;

    // Only handle ScriptsOnly transitions here.
    // Playing/Paused/Stop are handled by renzora_viewport::play_mode::handle_play_mode_transitions
    // which also does the camera switching.
    // Track what physics action to take after releasing the borrow
    enum PhysicsAction { None, Unpause, Pause }

    let (needs_reset, physics_action) = {
        let Some(mut pm) = world.get_resource_mut::<renzora_core::PlayModeState>() else {
            return;
        };

        if pm.request_scripts_only {
            pm.request_scripts_only = false;
            match pm.state {
                PlayState::Editing => {
                    pm.state = PlayState::ScriptsOnly;
                    info!("[PlayMode] Scripts Only — entering scripts mode");
                    (true, PhysicsAction::Unpause)
                }
                PlayState::ScriptsPaused => {
                    pm.state = PlayState::ScriptsOnly;
                    info!("[PlayMode] Scripts Resumed");
                    (false, PhysicsAction::Unpause)
                }
                _ => (false, PhysicsAction::None),
            }
        } else if pm.is_scripts_only() || matches!(pm.state, PlayState::ScriptsPaused) {
            // Handle stop/pause for scripts-only mode
            if pm.request_stop {
                pm.request_stop = false;
                pm.state = PlayState::Editing;
                info!("[PlayMode] Scripts Stopped");
                (false, PhysicsAction::Pause)
            } else if pm.request_pause {
                pm.request_pause = false;
                match pm.state {
                    PlayState::ScriptsOnly => {
                        pm.state = PlayState::ScriptsPaused;
                        info!("[PlayMode] Scripts Paused");
                        (false, PhysicsAction::Pause)
                    }
                    PlayState::ScriptsPaused => {
                        pm.state = PlayState::ScriptsOnly;
                        info!("[PlayMode] Scripts Resumed");
                        (false, PhysicsAction::Unpause)
                    }
                    _ => (false, PhysicsAction::None),
                }
            } else {
                (false, PhysicsAction::None)
            }
        } else {
            (false, PhysicsAction::None)
        }
    };

    // Trigger physics events (decoupled — renzora_physics observes these)
    match physics_action {
        PhysicsAction::Unpause => { world.trigger(renzora_core::UnpausePhysics); }
        PhysicsAction::Pause => { world.trigger(renzora_core::PausePhysics); }
        PhysicsAction::None => {}
    }

    if needs_reset {
        world.trigger(renzora_core::ResetScriptStates);
    }
}


/// Render the viewport texture fullscreen during play mode.

fn render_plugin_status_bar(ctx: &egui::Context, theme: &renzora_theme::Theme, _world: &World) {
    let text_color = theme.text.secondary.to_color32();
    let border_color = theme.widgets.border.to_color32();
    let panel_fill = theme.surfaces.panel.to_color32();

    egui::TopBottomPanel::bottom("renzora_status_bar")
        .exact_height(22.0)
        .frame(
            egui::Frame::NONE
                .fill(panel_fill)
                .stroke(egui::Stroke::new(1.0, border_color)),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                ui.label(
                    egui::RichText::new("Ready").size(11.0).color(text_color),
                );
            });
        });
}

