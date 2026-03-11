//! Renzora Editor — pluggable editor shell with docking panel system.
//!
//! The UI framework (docking, panels, widgets, theme) lives in `renzora_ui`.
//! This crate adds the Bevy plugin that wires it all together.

pub mod camera;
pub mod commands;
pub mod ext;
pub mod inspector_registry;
pub mod selection;
pub mod settings;
pub mod spawn_registry;

// Re-export full UI API so downstream crates can use `renzora_editor::DockTree` etc.
pub use renzora_ui::*;

pub use commands::EditorCommands;
pub use inspector_registry::{
    FieldDef, FieldType, FieldValue, InspectorEntry, InspectorRegistry,
};
pub use ext::{AppEditorExt, InspectableComponent};
pub use renzora_macros::{Inspectable, post_process};
pub use selection::EditorSelection;
pub use settings::{EditorSettings, MonoFont, SelectionHighlightMode, SettingsTab, UiFont};

// Re-export core marker components so downstream crates can use `renzora_editor::HideInHierarchy` etc.
pub use renzora_core::{HideInHierarchy, EditorLocked, EditorCamera};
pub use renzora_splash::SplashState;

/// Optional label color for an entity row in the hierarchy.
#[derive(Component)]
pub struct EntityLabelColor(pub [u8; 3]);

pub use renzora_core::EntityTag;

pub use spawn_registry::{EntityPreset, SpawnRegistry};

/// Gizmo transform mode — shared so both the gizmo and viewport toolbar can access it.
#[derive(bevy::prelude::Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum GizmoMode {
    #[default]
    Translate,
    Rotate,
    Scale,
}

use std::sync::atomic::{AtomicBool, Ordering};

use bevy::prelude::*;
use bevy::ecs::system::SystemState;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use bevy_egui::egui;
use renzora_theme::ThemeManager;

// Module and type names come through `pub use renzora_ui::*` above.
// Use fully-qualified `renzora_ui::module::fn` for sub-module function calls.

static FONTS_INITIALIZED: AtomicBool = AtomicBool::new(false);

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
            .init_resource::<EditorSettings>()
            .init_resource::<renzora_ui::Toasts>()
            .add_systems(PostStartup, camera::spawn_ui_camera)
            .add_systems(
                EguiPrimaryContextPass,
                editor_ui_system.run_if(in_state(SplashState::Editor)),
            )
            .add_systems(
                Update,
                show_script_reload_toasts.run_if(in_state(SplashState::Editor)),
            );
    }
}

/// Picks up script hot-reload events and shows toast notifications.
fn show_script_reload_toasts(
    reload_events: Option<Res<renzora_scripting::ScriptReloadEvents>>,
    mut toasts: ResMut<renzora_ui::Toasts>,
) {
    let Some(events) = reload_events else { return };
    for name in &events.reloaded {
        toasts.success(format!("Reloaded {}", name));
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

    // 2. Init fonts once, then apply theme
    if !FONTS_INITIALIZED.load(Ordering::Relaxed) {
        renzora_ui::theme::init_fonts(&ctx);
        FONTS_INITIALIZED.store(true, Ordering::Relaxed);
    }
    let theme = world
        .get_resource::<ThemeManager>()
        .map(|tm| tm.active_theme.clone())
        .unwrap_or_default();
    renzora_ui::theme::apply_theme(&ctx, &theme);

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

        // Render viewport texture fullscreen
        render_play_mode_viewport(&ctx, world);
        render_play_mode_overlay(&ctx, &theme, play_state);

        // Escape to stop
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if let Some(mut pm) = world.get_resource_mut::<renzora_core::PlayModeState>() {
                pm.request_stop = true;
            }
        }

        // Toasts
        {
            let current_time = world.resource::<bevy::prelude::Time>().elapsed_secs_f64();
            if let Some(mut toasts) = world.get_resource_mut::<renzora_ui::Toasts>() {
                toasts.show(&ctx, current_time);
            }
        }

        // Process play mode requests
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
    let title_action = renzora_ui::title_bar::render_title_bar(&ctx, &theme, &registry, &layout_manager, &play_info);

    // 6. Document tabs (below title bar)
    let doc_tab_state = world
        .get_resource::<DocumentTabState>()
        .cloned()
        .unwrap_or_default();
    let doc_tab_action = renzora_ui::document_tabs::render_document_tabs(&ctx, &doc_tab_state, &theme);

    // 7. Status bar (bottom)
    renzora_ui::status_bar::render_status_bar(&ctx, &theme);

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
        DocTabAction::Activate(idx, _layout_name) => {
            let layout_name = world
                .get_resource_mut::<DocumentTabState>()
                .and_then(|mut ts| ts.activate_tab(idx).map(|s| s.to_string()));
            if let Some(name) = layout_name {
                switch_layout_by_name(world, &name);
            }
        }
        DocTabAction::Close(idx) => {
            let layout_name = world
                .get_resource_mut::<DocumentTabState>()
                .and_then(|mut ts| ts.close_tab(idx).map(|s| s.to_string()));
            if let Some(name) = layout_name {
                switch_layout_by_name(world, &name);
            }
        }
        DocTabAction::Reorder(from, to) => {
            if let Some(mut ts) = world.get_resource_mut::<DocumentTabState>() {
                ts.reorder(from, to);
            }
        }
        DocTabAction::AddNew(kind) => {
            let layout_name = kind.preferred_layout().to_string();
            let label = kind.label().to_string();
            if let Some(mut ts) = world.get_resource_mut::<DocumentTabState>() {
                let idx = ts.add_tab(format!("Untitled {}", label), kind);
                ts.active_tab = idx;
            }
            switch_layout_by_name(world, &layout_name);
        }
        DocTabAction::None => {}
    }

    // Toast notifications
    {
        let current_time = world.resource::<bevy::prelude::Time>().elapsed_secs_f64();
        if let Some(mut toasts) = world.get_resource_mut::<renzora_ui::Toasts>() {
            toasts.show(&ctx, current_time);
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

    // Apply physics state change (borrow on PlayModeState is released)
    match physics_action {
        PhysicsAction::Unpause => renzora_physics::unpause(world),
        PhysicsAction::Pause => renzora_physics::pause(world),
        PhysicsAction::None => {}
    }

    if needs_reset {
        reset_script_states(world);
    }
}

/// Reset all script runtime states so on_ready fires again.
fn reset_script_states(world: &mut World) {
    use renzora_scripting::ScriptComponent;
    let mut query = world.query::<&mut ScriptComponent>();
    for mut sc in query.iter_mut(world) {
        for entry in sc.scripts.iter_mut() {
            entry.runtime_state.initialized = false;
            entry.runtime_state.has_error = false;
        }
    }
}

/// Render the viewport texture fullscreen during play mode.
fn render_play_mode_viewport(ctx: &egui::Context, world: &mut World) {
    use bevy_egui::EguiUserTextures;
    use renzora_core::ViewportRenderTarget;

    // Get the image handle
    let image_handle = world
        .get_resource::<ViewportRenderTarget>()
        .and_then(|vrt| vrt.image.clone());

    // Get the egui texture ID
    let texture_id = image_handle.as_ref().and_then(|handle| {
        world
            .get_resource::<EguiUserTextures>()
            .and_then(|ut| ut.image_id(handle.id()))
    });

    // Resize the render texture to match the full window
    let screen = ctx.screen_rect();
    let w = (screen.width() * ctx.pixels_per_point()).max(1.0) as u32;
    let h = (screen.height() * ctx.pixels_per_point()).max(1.0) as u32;

    if let Some(ref handle) = image_handle {
        if let Some(mut images) = world.get_resource_mut::<Assets<Image>>() {
            if let Some(image) = images.get_mut(handle) {
                let current = image.texture_descriptor.size;
                if current.width != w || current.height != h {
                    image.texture_descriptor.size.width = w;
                    image.texture_descriptor.size.height = h;
                    image.data = Some(vec![0u8; (w * h * 4) as usize]);
                }
            }
        }
    }

    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(egui::Color32::BLACK))
        .show(ctx, |ui| {
            if let Some(tex_id) = texture_id {
                let rect = ui.available_rect_before_wrap();
                let size = egui::vec2(rect.width(), rect.height());
                ui.put(
                    rect,
                    egui::Image::new(egui::load::SizedTexture::new(tex_id, size)),
                );
            }
        });
}

/// Render a minimal overlay during full play mode (Playing/Paused).
fn render_play_mode_overlay(
    ctx: &egui::Context,
    theme: &renzora_theme::Theme,
    state: renzora_core::PlayState,
) {
    use egui::{Align2, Color32, FontId, Vec2};

    let (icon, color, label) = match state {
        renzora_core::PlayState::Playing => (
            egui_phosphor::regular::PLAY,
            Color32::from_rgb(64, 200, 100),
            "Playing",
        ),
        renzora_core::PlayState::Paused => (
            egui_phosphor::regular::PAUSE,
            Color32::from_rgb(255, 200, 80),
            "Paused",
        ),
        _ => return,
    };

    // Top center status indicator
    egui::Area::new(egui::Id::new("play_mode_status"))
        .anchor(Align2::CENTER_TOP, Vec2::new(0.0, 8.0))
        .order(egui::Order::Foreground)
        .interactable(false)
        .show(ctx, |ui| {
            let frame = egui::Frame::NONE
                .fill(Color32::from_rgba_unmultiplied(0, 0, 0, 180))
                .inner_margin(egui::Margin::symmetric(16, 6))
                .corner_radius(egui::CornerRadius::same(6));
            frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    ui.label(
                        egui::RichText::new(icon)
                            .font(FontId::proportional(14.0))
                            .color(color),
                    );
                    ui.label(
                        egui::RichText::new(label)
                            .font(FontId::proportional(13.0))
                            .color(Color32::WHITE),
                    );
                    ui.label(
                        egui::RichText::new("Press ESC to stop")
                            .font(FontId::proportional(11.0))
                            .color(Color32::from_rgb(140, 140, 150)),
                    );
                });
            });
        });
}
