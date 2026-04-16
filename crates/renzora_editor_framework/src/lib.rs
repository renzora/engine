//! Renzora Editor — pluggable editor shell with docking panel system.
//!
//! The UI framework (docking, panels, widgets, theme) lives in `renzora_ui`.
//! This crate adds the Bevy plugin that wires it all together.

pub mod bevy_inspectors;
pub mod camera;
pub mod commands;
pub mod sdk;
pub mod ext;
pub mod inspector_registry;
pub mod selection;
pub mod settings;
pub mod shortcut_registry;
pub mod spawn_registry;
pub mod mode_options_registry;
pub mod tool_options_registry;
pub mod toolbar_registry;
pub mod viewport_overlay;

// Re-export full UI API so downstream crates can use `renzora_editor_framework::DockTree` etc.
pub use renzora_ui::*;

pub use commands::EditorCommands;
pub use inspector_registry::{
    FieldDef, FieldType, FieldValue, InspectorEntry, InspectorRegistry,
};
pub use ext::{AppEditorExt, InspectableComponent};
pub use renzora_macros::{Inspectable, post_process};
pub use selection::EditorSelection;
pub use shortcut_registry::{ShortcutEntry, ShortcutRegistry};
pub use toolbar_registry::{ToolEntry, ToolSection, ToolbarRegistry};

/// Late-bound hooks for actions that live in downstream crates the editor
/// framework can't depend on directly (avoids a cycle with `renzora_undo`).
/// Downstream crates install hooks in their `Plugin::build`; the menu /
/// title bar handlers call them when present.
#[derive(bevy::prelude::Resource, Default, Clone)]
pub struct EditorActionHooks {
    pub undo: Option<fn(&mut bevy::prelude::World)>,
    pub redo: Option<fn(&mut bevy::prelude::World)>,
}

/// Debounce state for the auto-save layout system — tracks "dirty since N
/// frames ago". We delay the write a few frames after the last change so
/// mid-drag updates don't hammer the disk.
#[derive(bevy::prelude::Resource, Default)]
pub struct PendingLayoutSave {
    pub dirty: bool,
    pub frames_stable: u32,
}

/// Mirror `DockingState` into the active layout's slot + mark dirty for
/// the debounced save. Runs every frame — when nothing changed, we just
/// bump the stable-frame counter toward the flush threshold.
fn mark_layout_dirty(
    docking: bevy::prelude::Res<renzora_ui::DockingState>,
    mut manager: bevy::prelude::ResMut<renzora_ui::LayoutManager>,
    mut pending: bevy::prelude::ResMut<PendingLayoutSave>,
) {
    if docking.is_changed() {
        // Keep the active layout slot in sync with the live dock so
        // switching away and back preserves edits.
        let idx = manager.active_index;
        if let Some(slot) = manager.layouts.get_mut(idx) {
            slot.tree = docking.tree.clone();
        }
        pending.dirty = true;
        pending.frames_stable = 0;
    } else if manager.is_changed() {
        // active_index changed (layout switch) — persist that too.
        pending.dirty = true;
        pending.frames_stable = 0;
    } else if pending.dirty {
        pending.frames_stable = pending.frames_stable.saturating_add(1);
    }
}

/// Write the workspace (all layouts + active index) to disk once it's
/// been stable for a few frames — avoids fsync churn during drag gestures.
fn flush_layout_save(
    manager: bevy::prelude::Res<renzora_ui::LayoutManager>,
    mut pending: bevy::prelude::ResMut<PendingLayoutSave>,
) {
    const SAVE_DELAY_FRAMES: u32 = 8;
    if pending.dirty && pending.frames_stable >= SAVE_DELAY_FRAMES {
        renzora_ui::save_workspace(&manager);
        pending.dirty = false;
        pending.frames_stable = 0;
    }
}

/// Reset only the currently-active layout to its factory default. Other
/// layouts keep their customisations. The saved workspace file is also
/// deleted so the next launch starts clean.
pub fn reset_layout(world: &mut bevy::prelude::World) {
    world.resource_scope::<renzora_ui::LayoutManager, _>(|world, mut manager| {
        if let Some(mut docking) = world.get_resource_mut::<renzora_ui::DockingState>() {
            manager.reset_active(&mut docking);
        }
    });
    if let Some(mut pending) = world.get_resource_mut::<PendingLayoutSave>() {
        pending.dirty = false;
        pending.frames_stable = 0;
    }
    renzora_ui::delete_saved_workspace();
}

/// Insert this as a resource to request the context-menu plugin open its
/// "Add Component" overlay at a given screen position. Consumed (removed)
/// by the plugin once opened. Lives in the SDK so any panel can fire the
/// request without depending on the plugin crate directly.
#[derive(bevy::prelude::Resource, Clone, Copy, Debug)]
pub struct OpenAddComponentMenuRequest {
    pub screen_pos: bevy::prelude::Vec2,
}
pub use mode_options_registry::{ModeOptionsDrawer, ViewportModeOptionsRegistry};
pub use tool_options_registry::{ToolOptionsDrawer, ToolOptionsRegistry};
pub use viewport_overlay::{ViewportOverlayDrawer, ViewportOverlayRegistry};
pub use settings::{CustomFonts, EditorSettings, MonoFont, SelectionHighlightMode, SettingsTab, UiFont};

// Re-export core marker components so downstream crates can use `renzora_editor_framework::HideInHierarchy` etc.
pub use renzora::{HideInHierarchy, EditorLocked, EditorCamera};
pub use renzora_splash::SplashState;

/// Optional label color for an entity row in the hierarchy.
#[derive(Component)]
pub struct EntityLabelColor(pub [u8; 3]);

/// Sort order for root-level entities in the hierarchy panel.
/// Lower values appear first. Entities without this component sort last.
#[derive(Component, Clone, Copy)]
pub struct HierarchyOrder(pub u32);

/// Pending entities to expand in the hierarchy panel next time it renders.
/// Systems that spawn entities as children can push the parent entity here so
/// the panel reveals the newly spawned child even if the user hasn't toggled
/// expansion manually.
#[derive(Resource, Default)]
pub struct HierarchyExpandRequests {
    entries: std::sync::RwLock<Vec<Entity>>,
}

impl HierarchyExpandRequests {
    pub fn push(&self, entity: Entity) {
        self.entries.write().unwrap().push(entity);
    }
    pub fn drain(&self) -> Vec<Entity> {
        std::mem::take(&mut *self.entries.write().unwrap())
    }
}

pub use renzora::EntityTag;

/// Optional extension filter for the asset browser. When set, only files with
/// these extensions (and folders) appear in the grid/list/tree. Mirrors the
/// `HierarchyFilter` pattern: other crates set this when their workspace
/// becomes active and unset it on leave.
#[derive(Resource, Default, Clone, PartialEq, Eq, Debug)]
pub struct AssetBrowserExtensionFilter(pub Option<Vec<String>>);

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
    /// Hide entities that (or whose ancestors) have any of the listed component type names.
    ExcludeDescendantsOf(Vec<&'static str>),
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
    /// A plugin tool is driving viewport input. Built-in picking + box
    /// selection skip themselves when they see this.
    None,
}

/// Unified active tool — replaces scattered `GizmoMode`, `TerrainToolState`, `FoliageToolState`.
///
/// Only one tool is active at a time. The viewport toolbar sets this directly.
/// Downstream crates read this to decide whether their systems should run.
#[derive(bevy::prelude::Resource, Default, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum ActiveTool {
    #[default]
    Select,
    Translate,
    Rotate,
    Scale,
    TerrainSculpt,
    TerrainPaint,
    FoliagePaint,
    /// No built-in tool active. Plugins that own their own input mode (mesh
    /// draw, brush tools, etc.) set this so the gizmo + select-click systems
    /// disengage while the plugin is driving.
    None,
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

        // Restore the user's saved workspace (all layouts + active index)
        // if one exists, otherwise use the factory defaults. The active
        // layout's tree seeds `DockingState`.
        //
        // Reconcile against the current factory defaults: drop any saved
        // layouts whose names no longer exist (e.g. removed workspaces like
        // the old "Lifecycle" one), and append any new defaults that weren't
        // in the save file. Preserves user customisations on surviving names.
        let saved_manager = renzora_ui::load_saved_workspace();
        let (initial_manager, initial_docking) = match saved_manager {
            Some(mut manager) => {
                let defaults = LayoutManager::default();
                let default_names: std::collections::HashSet<&str> =
                    defaults.layouts.iter().map(|l| l.name.as_str()).collect();
                let previous_active_name = manager
                    .layouts
                    .get(manager.active_index)
                    .map(|l| l.name.clone());
                manager.layouts.retain(|l| default_names.contains(l.name.as_str()));
                for def in &defaults.layouts {
                    if !manager.layouts.iter().any(|l| l.name == def.name) {
                        manager.layouts.push(def.clone());
                    }
                }
                manager.active_index = previous_active_name
                    .and_then(|name| manager.layouts.iter().position(|l| l.name == name))
                    .unwrap_or(0);
                let tree = manager
                    .layouts
                    .get(manager.active_index)
                    .map(|l| l.tree.clone())
                    .unwrap_or_else(|| renzora_ui::layouts::scene_layout());
                (manager, DockingState { tree })
            }
            None => (LayoutManager::default(), DockingState::default()),
        };

        app.init_state::<EditorState>()
            .init_resource::<ThemeManager>()
            .init_resource::<PanelRegistry>()
            .insert_resource(initial_docking)
            .insert_resource(initial_manager)
            .init_resource::<FloatingPanels>()
            .init_resource::<DocumentTabState>()
            .init_resource::<EditorSelection>()
            .init_resource::<EditorCommands>()
            .init_resource::<InspectorRegistry>()
            .init_resource::<SpawnRegistry>()
            .init_resource::<ComponentIconRegistry>()
            .init_resource::<ViewportOverlayRegistry>()
            .init_resource::<ToolbarRegistry>()
            .init_resource::<ToolOptionsRegistry>()
            .init_resource::<ViewportModeOptionsRegistry>()
            .init_resource::<ShortcutRegistry>()
            .init_resource::<EditorActionHooks>();

        register_builtin_tools(
            &mut app.world_mut().resource_mut::<ToolbarRegistry>(),
        );

        app.add_systems(
            Update,
            shortcut_registry::shortcut_dispatch_system
                .run_if(in_state(SplashState::Editor)),
        );

        // Auto-save the dock layout whenever it changes.
        app.init_resource::<PendingLayoutSave>()
            .add_systems(
                Update,
                (mark_layout_dirty, flush_layout_save)
                    .run_if(in_state(SplashState::Editor)),
            );

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
            .init_resource::<AssetBrowserExtensionFilter>()
            .init_resource::<HierarchyExpandRequests>()
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
    trigger: On<renzora::ScriptsReloaded>,
    mut toasts: ResMut<renzora_ui::Toasts>,
) {
    let event = trigger.event();
    match event.names.len() {
        0 => {}
        1 => { toasts.success(format!("Reloaded {}", event.names[0])); }
        n => { toasts.success(format!("Reloaded {} scripts", n)); }
    }
}

/// Returns true if an entity with a terrain-data component is currently selected.
/// Used by terrain toolbar entries and the terrain tool panel to show
/// context-sensitive UI only when a terrain is active.
pub fn is_terrain_selected(world: &World) -> bool {
    let Some(sel) = world.get_resource::<EditorSelection>() else { return false };
    let Some(entity) = sel.get() else { return false };

    fn has_terrain_component(world: &World, entity: Entity) -> bool {
        let Ok(er) = world.get_entity(entity) else { return false };
        let archetype = er.archetype();
        for &component_id in archetype.components() {
            if let Some(info) = world.components().get_info(component_id) {
                let name = info.name();
                if name.contains("TerrainData") || name.contains("TerrainChunkData") {
                    return true;
                }
            }
        }
        false
    }

    if has_terrain_component(world, entity) {
        return true;
    }
    if let Some(parent) = world.get::<ChildOf>(entity) {
        if has_terrain_component(world, parent.0) {
            return true;
        }
    }
    false
}

/// Register the built-in Transform + Terrain/Foliage tools with the
/// [`ToolbarRegistry`]. Called once at plugin build time.
fn register_builtin_tools(registry: &mut ToolbarRegistry) {
    use egui_phosphor::regular::*;

    // Transform section — always visible
    registry.register(
        ToolEntry::new("builtin.select", CURSOR, "Select (Q)", ToolSection::Transform)
            .order(0)
            .active_if(|w| {
                w.get_resource::<ActiveTool>()
                    .copied()
                    .map_or(false, |t| t == ActiveTool::Select)
            })
            .on_activate(|w| { w.insert_resource(ActiveTool::Select); }),
    );
    registry.register(
        ToolEntry::new("builtin.translate", ARROWS_OUT_CARDINAL, "Move (W)", ToolSection::Transform)
            .order(1)
            .active_if(|w| {
                w.get_resource::<ActiveTool>()
                    .copied()
                    .map_or(false, |t| t == ActiveTool::Translate)
            })
            .on_activate(|w| { w.insert_resource(ActiveTool::Translate); }),
    );
    registry.register(
        ToolEntry::new("builtin.rotate", ARROWS_COUNTER_CLOCKWISE, "Rotate (E)", ToolSection::Transform)
            .order(2)
            .active_if(|w| {
                w.get_resource::<ActiveTool>()
                    .copied()
                    .map_or(false, |t| t == ActiveTool::Rotate)
            })
            .on_activate(|w| { w.insert_resource(ActiveTool::Rotate); }),
    );
    registry.register(
        ToolEntry::new("builtin.scale", ARROWS_OUT_SIMPLE, "Scale (R)", ToolSection::Transform)
            .order(3)
            .active_if(|w| {
                w.get_resource::<ActiveTool>()
                    .copied()
                    .map_or(false, |t| t == ActiveTool::Scale)
            })
            .on_activate(|w| { w.insert_resource(ActiveTool::Scale); }),
    );

    // Terrain section — visible only when a terrain is selected. Clicking the
    // active button a second time deactivates (reverts to Select).
    registry.register(
        ToolEntry::new("builtin.terrain_sculpt", MOUNTAINS, "Sculpt Terrain", ToolSection::Terrain)
            .order(0)
            .visible_if(is_terrain_selected)
            .active_if(|w| {
                w.get_resource::<ActiveTool>()
                    .copied()
                    .map_or(false, |t| t == ActiveTool::TerrainSculpt)
            })
            .on_activate(|w| {
                let cur = w.get_resource::<ActiveTool>().copied().unwrap_or_default();
                let new = if cur == ActiveTool::TerrainSculpt {
                    ActiveTool::Select
                } else {
                    ActiveTool::TerrainSculpt
                };
                w.insert_resource(new);
            }),
    );
    registry.register(
        ToolEntry::new("builtin.terrain_paint", PAINT_BRUSH, "Paint Terrain Layers", ToolSection::Terrain)
            .order(1)
            .visible_if(is_terrain_selected)
            .active_if(|w| {
                w.get_resource::<ActiveTool>()
                    .copied()
                    .map_or(false, |t| t == ActiveTool::TerrainPaint)
            })
            .on_activate(|w| {
                let cur = w.get_resource::<ActiveTool>().copied().unwrap_or_default();
                let new = if cur == ActiveTool::TerrainPaint {
                    ActiveTool::Select
                } else {
                    ActiveTool::TerrainPaint
                };
                w.insert_resource(new);
            }),
    );
    registry.register(
        ToolEntry::new("builtin.foliage_paint", TREE, "Paint Foliage", ToolSection::Terrain)
            .order(2)
            .visible_if(is_terrain_selected)
            .active_if(|w| {
                w.get_resource::<ActiveTool>()
                    .copied()
                    .map_or(false, |t| t == ActiveTool::FoliagePaint)
            })
            .on_activate(|w| {
                let cur = w.get_resource::<ActiveTool>().copied().unwrap_or_default();
                let new = if cur == ActiveTool::FoliagePaint {
                    ActiveTool::Select
                } else {
                    ActiveTool::FoliagePaint
                };
                w.insert_resource(new);
            }),
    );
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
        // TerrainSculpt / TerrainPaint / FoliagePaint / None — plugin tool is
        // driving; disengage gizmo + pick + box-select.
        if *gizmo_mode != GizmoMode::None {
            *gizmo_mode = GizmoMode::None;
        }
    }
}

/// When the editor state is entered, tell the ThemeManager about the project
/// directory so it can discover custom `.toml` themes in `<project>/themes/`.
fn wire_theme_project_path(
    project: Option<Res<renzora::CurrentProject>>,
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
    let fonts_dir = match world.get_resource::<renzora::CurrentProject>() {
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
        .get_resource::<renzora::PlayModeState>()
        .map(|pm| pm.state)
        .unwrap_or(renzora::PlayState::Editing);

    if matches!(play_state, renzora::PlayState::Playing | renzora::PlayState::Paused) {
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
        .get_resource::<renzora::PlayModeState>()
        .map(|pm| renzora_ui::title_bar::PlayModeInfo {
            is_playing: pm.is_playing(),
            is_paused: pm.is_paused(),
            is_scripts_only: pm.is_scripts_only(),
        })
        .unwrap_or_default();
    // Auth state comes from the AuthBridge resource (synced by AuthPlugin).
    let auth_bridge = world
        .get_resource::<renzora::AuthBridge>()
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
    renzora_ui::status_bar::render_status_bar(&ctx, &theme, world);

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
        TitleBarAction::SwitchLayout(i) => {
            // Clear selection first so auto-switch-on-selection systems don't
            // immediately override the user's manual layout choice.
            if let Some(sel) = world.get_resource::<EditorSelection>() {
                sel.set(None);
            }
            switch_layout(world, i);
        }
        TitleBarAction::NewProject => handle_new_project(world),
        TitleBarAction::OpenProject => handle_open_project(world),
        TitleBarAction::NewScene => {
            world.insert_resource(renzora::NewSceneRequested);
        }
        TitleBarAction::OpenScene => {
            world.insert_resource(renzora::OpenSceneRequested);
        }
        TitleBarAction::Save => {
            world.insert_resource(renzora::SaveSceneRequested);
        }
        TitleBarAction::SaveAs => {
            world.insert_resource(renzora::SaveAsSceneRequested);
        }
        TitleBarAction::Export => {
            world.insert_resource(renzora::ExportRequested);
        }
        TitleBarAction::ToggleSettings => {
            if let Some(mut settings) = world.get_resource_mut::<EditorSettings>() {
                settings.show_settings = !settings.show_settings;
            }
        }
        TitleBarAction::ToggleSignIn => {
            world.insert_resource(renzora::AuthToggleWindowRequest);
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
            world.insert_resource(renzora::AuthSignOutRequest);
        }
        TitleBarAction::Play => {
            if let Some(mut pm) = world.get_resource_mut::<renzora::PlayModeState>() {
                pm.request_play = true;
            }
        }
        TitleBarAction::Stop => {
            if let Some(mut pm) = world.get_resource_mut::<renzora::PlayModeState>() {
                pm.request_stop = true;
            }
        }
        TitleBarAction::Pause => {
            if let Some(mut pm) = world.get_resource_mut::<renzora::PlayModeState>() {
                pm.request_pause = true;
            }
        }
        TitleBarAction::ScriptsOnly => {
            if let Some(mut pm) = world.get_resource_mut::<renzora::PlayModeState>() {
                pm.request_scripts_only = true;
            }
        }
        TitleBarAction::StartTutorial => {
            world.insert_resource(renzora::TutorialRequested);
        }
        TitleBarAction::Undo => {
            if let Some(hook) = world.get_resource::<EditorActionHooks>().and_then(|h| h.undo) {
                hook(world);
            }
        }
        TitleBarAction::Redo => {
            if let Some(hook) = world.get_resource::<EditorActionHooks>().and_then(|h| h.redo) {
                hook(world);
            }
        }
        TitleBarAction::ResetLayout => reset_layout(world),
        TitleBarAction::None => {}
    }

    // L) Handle document tab actions
    match doc_tab_action {
        DocTabAction::Activate(idx) => {
            let ids = world
                .get_resource_mut::<DocumentTabState>()
                .and_then(|mut ts| ts.activate_tab(idx));
            if let Some((old_id, new_id)) = ids {
                world.insert_resource(renzora::TabSwitchRequest { old_tab_id: old_id, new_tab_id: new_id });
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
                world.insert_resource(renzora::TabSwitchRequest { old_tab_id: old_id, new_tab_id: new_id });
            }

            // Close the tab and clean up buffer
            let closed_id = world
                .get_resource_mut::<DocumentTabState>()
                .and_then(|mut ts| ts.close_tab(idx));
            if let Some(id) = closed_id {
                if let Some(mut buffers) = world.get_resource_mut::<renzora::SceneTabBuffers>() {
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
                world.insert_resource(renzora::TabSwitchRequest { old_tab_id: old_id, new_tab_id: new_id });
            }
        }
        DocTabAction::None => {}
    }


    // Auth window rendering is handled by AuthPlugin (renzora_auth).
    // React to successful sign-in by switching to the Hub layout.
    if world.remove_resource::<renzora::AuthJustSignedIn>().is_some() {
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
    if world.remove_resource::<renzora::ToggleSettingsRequested>().is_some() {
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
pub fn switch_layout_by_name(world: &mut World, name: &str) {
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
    world.remove_resource::<renzora::CurrentProject>();
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

        let project = match renzora::open_project(&file) {
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
    use renzora::PlayState;

    // Only handle ScriptsOnly transitions here.
    // Playing/Paused/Stop are handled by renzora_viewport::play_mode::handle_play_mode_transitions
    // which also does the camera switching.
    // Track what physics action to take after releasing the borrow
    enum PhysicsAction { None, Unpause, Pause }

    let (needs_reset, physics_action) = {
        let Some(mut pm) = world.get_resource_mut::<renzora::PlayModeState>() else {
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
        PhysicsAction::Unpause => { world.trigger(renzora::UnpausePhysics); }
        PhysicsAction::Pause => { world.trigger(renzora::PausePhysics); }
        PhysicsAction::None => {}
    }

    if needs_reset {
        world.trigger(renzora::ResetScriptStates);
    }
}
