//! Renzora Editor — pluggable editor shell with docking panel system.
//!
//! The UI framework (docking, panels, widgets, theme) lives in `renzora_ui`.
//! This crate adds the Bevy plugin that wires it all together.

pub mod camera;
pub mod commands;
pub mod inspector_registry;
pub mod selection;
pub mod spawn_registry;

// Re-export full UI API so downstream crates can use `renzora_editor::DockTree` etc.
pub use renzora_ui::*;

pub use commands::EditorCommands;
pub use inspector_registry::{
    FieldDef, FieldType, FieldValue, InspectorEntry, InspectorRegistry,
};
pub use selection::EditorSelection;

/// Marker component to hide an entity (and its children) from the hierarchy panel.
#[derive(Component)]
pub struct HideInHierarchy;

/// Marker component — entity is locked from editing in the hierarchy.
#[derive(Component)]
pub struct EditorLocked;

/// Optional label color for an entity row in the hierarchy.
#[derive(Component)]
pub struct EntityLabelColor(pub [u8; 3]);

/// Optional tag string for an entity.
#[derive(Component, Default)]
pub struct EntityTag {
    pub tag: String,
}

pub use spawn_registry::{EntityPreset, SpawnRegistry};

use std::sync::atomic::{AtomicBool, Ordering};

use bevy::prelude::*;
use bevy::ecs::system::SystemState;
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass};
use bevy_egui::egui;
use renzora_theme::ThemeManager;

// Module and type names come through `pub use renzora_ui::*` above.
// Use fully-qualified `renzora_ui::module::fn` for sub-module function calls.

static FONTS_INITIALIZED: AtomicBool = AtomicBool::new(false);

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
        if !app.is_plugin_added::<EguiPlugin>() {
            app.add_plugins(EguiPlugin::default());
        }

        app.init_state::<EditorState>()
            .init_resource::<ThemeManager>()
            .init_resource::<PanelRegistry>()
            .init_resource::<DockingState>()
            .init_resource::<LayoutManager>()
            .init_resource::<DocumentTabState>()
            .init_resource::<EditorSelection>()
            .init_resource::<EditorCommands>()
            .init_resource::<InspectorRegistry>()
            .init_resource::<SpawnRegistry>()
            .add_systems(PostStartup, camera::spawn_editor_camera)
            .add_systems(EguiPrimaryContextPass, editor_ui_system);
    }
}

/// Main exclusive-ish editor UI system.
///
/// Uses `SystemState` to extract the egui context, clones it (Arc-backed, cheap),
/// then renders everything with `&World` access for panels.
fn editor_ui_system(world: &mut World) {
    // 1. Get egui context
    let mut state = SystemState::<EguiContexts>::new(world);
    let mut contexts = state.get_mut(world);
    let ctx = match contexts.ctx_mut() {
        Ok(c) => c.clone(), // Arc clone — cheap
        Err(_) => return,
    };
    state.apply(world);

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

    // 5. Title bar (top) — returns action
    let title_action = renzora_ui::title_bar::render_title_bar(&ctx, &theme, &registry, &layout_manager);

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

    // C) Handle drop — apply tree mutations
    if should_drop {
        if let Some(drag) = world.remove_resource::<DragState>() {
            if let Some(target) = render_result.drop_target {
                if let Some(mut docking) = world.get_resource_mut::<DockingState>() {
                    // 1. Remove panel from source
                    docking.tree.remove_panel(&drag.panel_id);
                    // 2. Apply drop based on zone
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
            // If no valid target, the tab snaps back (remove_panel was not called)
        }
    }

    // D) Handle cancel (Escape key)
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        world.remove_resource::<DragState>();
    }

    // E) Draw floating ghost overlay
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

    // I) Handle title bar layout switch
    if let TitleBarAction::SwitchLayout(i) = title_action {
        switch_layout(world, i);
    }

    // J) Handle document tab actions
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

    // K) Drain and execute deferred editor commands (from inspector, etc.)
    let cmds = world
        .get_resource::<EditorCommands>()
        .map(|ec| ec.drain())
        .unwrap_or_default();
    for cmd in cmds {
        cmd(world);
    }
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
