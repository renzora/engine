//! Renzora Editor — pluggable editor shell with docking panel system.
//!
//! The UI framework (docking, panels, widgets, theme) lives in `renzora_ui`.
//! This crate adds the Bevy plugin that wires it all together.

pub mod bevy_inspectors;
pub mod camera;
pub mod commands;
pub mod material_thumbnail_registry;
pub mod model_thumbnail_registry;
pub mod sdk;
pub mod settings;

// Re-export full UI API so downstream crates can use `renzora_editor_framework::DockTree` etc.
pub use renzora_ui::*;

pub use commands::EditorCommands;
// Editor CONTRACT moved into `renzora` core (Operation Merge fold). Re-exported
// here at the SAME `renzora_editor_framework::*` paths so this crate's own framework code
// and the bundle-side panels keep resolving `renzora_editor_framework::FieldDef` /
// `AppEditorExt` / the field macros etc. The field macros now expand to
// `renzora::FieldDef` (one shared contract — no `renzora_editor_framework` dependency).
pub use renzora::{
    shortcut_dispatch_system, AppEditorExt, ComponentIconEntry, ComponentIconRegistry,
    EditorSelection, EntityPreset, FieldDef, FieldType, FieldValue, HierarchyExpandRequests,
    HierarchyOrder, InspectableComponent, InspectorEntry, InspectorRegistry, NativeInspectorDrawer,
    NativeInspectorRegistry, SceneStarter, SceneStarterRegistry, ShortcutEntry, ShortcutHandler,
    ShortcutRegistry, SpawnRegistry, ToolActivator, ToolEntry, ToolPredicate, ToolSection,
    ToolbarRegistry,
};
pub use renzora::{
    bool_field, color_rgba_field, enum_u32_field, float_field, int_field, string_field,
    tuple_color_field, vec3_color_field,
};
pub use material_thumbnail_registry::{
    material_thumb_path, migrate_legacy_thumbnail_cache, thumbnail_cache_dir,
    MaterialThumbnailRegistry,
};
pub use model_thumbnail_registry::{model_thumb_path, ModelThumbnailRegistry};
pub use renzora_macros::{post_process, Inspectable};

/// Late-bound hooks for actions that live in downstream crates the editor
/// framework can't depend on directly (avoids a cycle with `renzora_undo`).
/// Downstream crates install hooks in their `Plugin::build`; the menu /
/// title bar handlers call them when present.
#[derive(bevy::prelude::Resource, Default, Clone)]
pub struct EditorActionHooks {
    pub undo: Option<fn(&mut bevy::prelude::World)>,
    pub redo: Option<fn(&mut bevy::prelude::World)>,
    pub can_undo: Option<fn(&bevy::prelude::World) -> bool>,
    pub can_redo: Option<fn(&bevy::prelude::World) -> bool>,
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
pub use settings::{
    CustomFonts, EditorSettings, InspectorComponentFilterStyle, InspectorExpandDefault, MonoFont,
    SelectionGranularity, SelectionHighlightMode, SettingsTab, UiFont,
};

// Re-export core marker components so downstream crates can use `renzora_editor_framework::HideInHierarchy` etc.
pub use renzora::SplashState;
pub use renzora::{EditorCamera, EditorLocked, HideInHierarchy};

/// Optional label color for an entity row in the hierarchy.
#[derive(Component)]
pub struct EntityLabelColor(pub [u8; 3]);

// `HierarchyOrder` moved into `renzora` core (re-exported at top of file).

/// One-shot flag set by scene loading code; read and cleared by the
/// hierarchy crate after the tree cache is built. When `true`, the
/// hierarchy will auto-select its top entity (provided the user hasn't
/// already selected something on this scene).
#[derive(Resource, Default)]
pub struct AutoSelectFirstHierarchyEntity(pub bool);

// `HierarchyExpandRequests` moved into `renzora` core (re-exported at top).

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

// (spawn-registry types re-exported from `renzora` at the top of this file)

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

/// Reference frame the transform gizmo operates in — shared so the gizmo and
/// the viewport toolbar toggle agree.
///
/// `World`: handles align to the world axes (X/Y/Z). `Local`: handles align to
/// the selected object's own orientation. Both write back through any parent
/// transform, so the object moves/rotates/scales correctly regardless of how
/// it's nested. Scale is always evaluated along local axes (a non-uniform world
/// scale of a rotated object can't be expressed as a `Transform`), so the
/// toggle only changes the scale gizmo's handle orientation, not its math.
#[derive(bevy::prelude::Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum GizmoSpace {
    #[default]
    World,
    Local,
}

impl GizmoSpace {
    /// World-space orientation for the gizmo's axes, given the selection's world
    /// rotation. World mode ignores it; Local mode aligns to it.
    pub fn basis(self, world_rotation: bevy::prelude::Quat) -> bevy::prelude::Quat {
        match self {
            Self::World => bevy::prelude::Quat::IDENTITY,
            Self::Local => world_rotation,
        }
    }
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
        matches!(
            self,
            Self::TerrainSculpt | Self::TerrainPaint | Self::FoliagePaint
        )
    }
}

use bevy::prelude::*;
use renzora_theme::ThemeManager;

// Module and type names come through `pub use renzora_ui::*` above.
// Use fully-qualified `renzora_ui::module::fn` for sub-module function calls.

/// Whether the editor overlay is active or hidden.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EditorState {
    #[default]
    Active,
    Inactive,
}

/// Apply queued [`EditorCommands`] (panel/inspector actions push closures here).
pub fn drain_editor_commands_native(world: &mut World) {
    let cmds = world
        .get_resource::<EditorCommands>()
        .map(|ec| ec.drain())
        .unwrap_or_default();
    for cmd in cmds {
        cmd(world);
    }
}

/// Plugin that adds the Renzora editor overlay to any Bevy app.
#[derive(Default)]
pub struct RenzoraEditorPlugin;

impl Plugin for RenzoraEditorPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] RenzoraEditorPlugin");

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
                manager
                    .layouts
                    .retain(|l| default_names.contains(l.name.as_str()));
                for def in &defaults.layouts {
                    if !manager.layouts.iter().any(|l| l.name == def.name) {
                        manager.layouts.push(def.clone());
                    }
                }
                // Re-stamp the `hidden` flag from defaults so workspaces saved
                // before asset-mode layouts existed correctly hide them from
                // the title bar.
                for layout in &mut manager.layouts {
                    if let Some(def) = defaults.layouts.iter().find(|d| d.name == layout.name) {
                        layout.hidden = def.hidden;
                    }
                }
                manager.active_index = previous_active_name
                    .and_then(|name| manager.layouts.iter().position(|l| l.name == name))
                    .unwrap_or(0);
                // Initial last-scene = current if it's a scene layout, else 0.
                manager.last_scene_index = manager
                    .layouts
                    .get(manager.active_index)
                    .filter(|l| !l.hidden)
                    .map(|_| manager.active_index)
                    .unwrap_or(0);
                let tree = manager
                    .layouts
                    .get(manager.active_index)
                    .map(|l| l.tree.clone())
                    .unwrap_or_else(renzora_ui::layouts::scene_layout);
                (manager, DockingState { tree })
            }
            None => (LayoutManager::default(), DockingState::default()),
        };

        app.init_state::<EditorState>()
            .init_resource::<ThemeManager>()
            .init_resource::<PanelRegistry>()
            .init_resource::<renzora::ShellPanelRegistry>()
            .insert_resource(initial_docking)
            .insert_resource(initial_manager)
            .init_resource::<FloatingPanels>()
            .init_resource::<DocumentTabState>()
            .init_resource::<renzora_ui::EditorContext>()
            .init_resource::<EditorSelection>()
            .init_resource::<EditorCommands>()
            .init_resource::<InspectorRegistry>()
            .init_resource::<SpawnRegistry>()
            .init_resource::<SceneStarterRegistry>()
            .init_resource::<ComponentIconRegistry>()
            .init_resource::<ToolbarRegistry>()
            .init_resource::<ShortcutRegistry>()
            .init_resource::<EditorActionHooks>()
            .init_resource::<MaterialThumbnailRegistry>()
            .init_resource::<ModelThumbnailRegistry>()
            .init_resource::<renzora::core::IsolationMode>();

        register_builtin_tools(&mut app.world_mut().resource_mut::<ToolbarRegistry>());

        app.add_systems(
            Update,
            shortcut_dispatch_system.run_if(in_state(SplashState::Editor)),
        );

        // Auto-save the dock layout whenever it changes.
        app.init_resource::<PendingLayoutSave>().add_systems(
            Update,
            (mark_layout_dirty, flush_layout_save).run_if(in_state(SplashState::Editor)),
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

        app.init_resource::<EditorSettings>()
            .init_resource::<ActiveTool>()
            .init_resource::<GizmoMode>()
            .init_resource::<CustomFonts>()
            .init_resource::<HierarchyFilter>()
            .init_resource::<AssetBrowserExtensionFilter>()
            .init_resource::<HierarchyExpandRequests>()
            .init_resource::<AutoSelectFirstHierarchyEntity>()
            .init_resource::<renzora_ui::Toasts>()
            .add_plugins(renzora_ui::window_chrome::WindowChromePlugin)
            .add_systems(PostStartup, camera::spawn_ui_camera)
            // Drain queued `EditorCommands` (panel actions — visibility/lock
            // toggles, undo/redo, etc.) under the native (bevy_ui) shell.
            .add_systems(
                Update,
                drain_editor_commands_native.run_if(in_state(SplashState::Editor)),
            )
            .add_observer(show_script_reload_toasts)
            .add_observer(show_hot_plugin_toasts)
            .add_systems(OnEnter(SplashState::Editor), wire_theme_project_path)
            .add_systems(
                Update,
                sync_active_tool_to_gizmo_mode.run_if(in_state(SplashState::Editor)),
            )
            .add_systems(Update, apply_vsync_setting)
            .add_systems(Update, (reset_ui_scale_shortcut, apply_ui_scale_setting).chain())
            .add_systems(
                Update,
                apply_isolation_mode.run_if(in_state(SplashState::Editor)),
            )
            .add_systems(
                Update,
                editor_panel_drop.run_if(in_state(SplashState::Editor)),
            );
    }
}

/// Apply the `ViewportSettings.vsync` toggle to the primary window's
/// `present_mode`. Lives on `ViewportSettings` (not `EditorSettings`) so
/// it persists alongside the rest of the per-project viewport prefs in
/// `project.toml`. Runs every frame but only writes when the mode
/// differs, so it's effectively free.
fn apply_vsync_setting(
    viewport: Res<renzora::core::viewport_types::ViewportSettings>,
    mut windows: Query<&mut bevy::window::Window, With<bevy::window::PrimaryWindow>>,
) {
    let Ok(mut window) = windows.single_mut() else {
        return;
    };
    let desired = if viewport.vsync {
        bevy::window::PresentMode::AutoVsync
    } else {
        bevy::window::PresentMode::AutoNoVsync
    };
    if window.present_mode != desired {
        window.present_mode = desired;
    }
}

/// Apply `EditorSettings.ui_scale` by scaling the primary window's reported
/// DPI factor to `os_scale * ui_scale`. Everything downstream reads that one
/// factor — winit's cursor-position conversion, the UI cameras, egui — so
/// logical px and UI design px remain the same space and no widget math has
/// to know the setting exists.
///
/// Deliberately NOT:
/// - `set_scale_factor_override`: bevy_winit reacts to an override change by
///   resizing the physical window to preserve the logical size (a 1920px
///   window grows to 2880px at 150%) instead of zooming the content in place.
/// - `bevy::ui::UiScale`: global to all UI layout, so it would also scale the
///   UI-canvas tab's render-to-texture game UI (breaking its 1:1 authoring
///   guarantee) — and the canvas systems already own that resource for
///   reference-resolution mapping.
///
/// bevy_winit only writes the base factor back on a real OS DPI change
/// (`react_to_scale_factor_change`), which it announces via
/// `WindowBackendScaleFactorChanged`; we track the true OS factor from those
/// messages (seeded from the window before our first write) and re-apply the
/// multiplier on top. Writing the base factor requests no window resize:
/// bevy_winit's `changed_windows` reconstructs the cached logical size using
/// the *current* base factor, so the physical size round-trips unchanged.
fn apply_ui_scale_setting(
    settings: Res<EditorSettings>,
    mut backend_scale: bevy::ecs::message::MessageReader<
        bevy::window::WindowBackendScaleFactorChanged,
    >,
    mut scale_changed: bevy::ecs::message::MessageWriter<
        bevy::window::WindowScaleFactorChanged,
    >,
    mut os_scale: Local<Option<f32>>,
    mut windows: Query<
        (Entity, &mut bevy::window::Window),
        With<bevy::window::PrimaryWindow>,
    >,
) {
    let Ok((entity, mut window)) = windows.single_mut() else {
        return;
    };
    for msg in backend_scale.read() {
        if msg.window == entity {
            *os_scale = Some(msg.scale_factor as f32);
        }
    }
    let base = *os_scale.get_or_insert_with(|| window.resolution.base_scale_factor());
    let desired = base * settings.ui_scale.clamp(0.5, 3.0);
    if (window.resolution.base_scale_factor() - desired).abs() > 1e-4 {
        window.resolution.set_scale_factor(desired);
        // Writing the factor directly doesn't notify anyone — `camera_system`
        // only recomputes `Camera::computed.target_info` when it sees a
        // window created/resized/scale-changed message, so without this the
        // new scale only shows up after a restart (or a real resize).
        scale_changed.write(bevy::window::WindowScaleFactorChanged {
            window: entity,
            scale_factor: desired as f64,
        });
    }
}

/// `Ctrl+0` (rebindable: [`EditorAction::ResetUiScale`]) snaps the editor UI
/// scale back to 100% — the escape hatch if a scale choice makes the settings
/// panel itself hard to operate.
fn reset_ui_scale_shortcut(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<renzora::core::keybindings::KeyBindings>,
    input_focus: Res<renzora::core::InputFocusState>,
    mut settings: ResMut<EditorSettings>,
) {
    if keybindings.rebinding.is_some() || input_focus.ui_wants_keyboard {
        return;
    }
    if !keybindings.just_pressed(
        renzora::core::keybindings::EditorAction::ResetUiScale,
        &keyboard,
    ) {
        return;
    }
    if settings.ui_scale != 1.0 {
        settings.ui_scale = 1.0;
        let _ = renzora::save_ui_scale(1.0);
    }
}

/// View menu Isolation Mode: when active, hide all mesh entities that aren't
/// the current selection (or its ancestors/descendants). On deactivate,
/// restore the entities we hid back to `Visibility::Inherited`.
fn apply_isolation_mode(
    iso: Res<renzora::core::IsolationMode>,
    selection: Res<EditorSelection>,
    children_q: Query<&Children>,
    parent_q: Query<&ChildOf>,
    mut visibility_q: Query<
        (Entity, &mut Visibility),
        (
            With<Mesh3d>,
            Without<renzora::core::EditorCamera>,
            Without<renzora::core::HideInHierarchy>,
        ),
    >,
    mut hidden_entities: Local<Vec<Entity>>,
    mut last_active: Local<bool>,
) {
    if iso.active == *last_active {
        return;
    }
    *last_active = iso.active;

    if iso.active {
        let mut keep: std::collections::HashSet<Entity> = std::collections::HashSet::new();
        for sel in selection.get_all() {
            keep.insert(sel);
            let mut cur = sel;
            while let Ok(child_of) = parent_q.get(cur) {
                let parent = child_of.parent();
                keep.insert(parent);
                cur = parent;
            }
            collect_descendants(sel, &children_q, &mut keep);
        }
        hidden_entities.clear();
        for (entity, mut vis) in &mut visibility_q {
            if !keep.contains(&entity) && *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
                hidden_entities.push(entity);
            }
        }
    } else {
        let drained: Vec<Entity> = hidden_entities.drain(..).collect();
        for entity in drained {
            if let Ok((_, mut vis)) = visibility_q.get_mut(entity) {
                *vis = Visibility::Inherited;
            }
        }
    }
}

fn collect_descendants(
    entity: Entity,
    children_q: &Query<&Children>,
    out: &mut std::collections::HashSet<Entity>,
) {
    if let Ok(children) = children_q.get(entity) {
        for child in children.iter() {
            if out.insert(child) {
                collect_descendants(child, children_q, out);
            }
        }
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
        1 => {
            toasts.success(format!("Reloaded {}", event.names[0]));
        }
        n => {
            toasts.success(format!("Reloaded {} scripts", n));
        }
    }
}

/// Observer: surface the outcome of a mid-session plugin hot-load as a toast.
/// The loader (in the runtime binary) triggers `HotPluginNotice`; this runs in
/// the editor bundle, so the `renzora.dll`-defined event crosses the boundary.
fn show_hot_plugin_toasts(
    trigger: On<renzora::HotPluginNotice>,
    mut toasts: ResMut<renzora_ui::Toasts>,
) {
    let notice = trigger.event();
    match notice.outcome {
        renzora::HotLoadOutcome::Loaded => toasts.success(notice.message.clone()),
        renzora::HotLoadOutcome::NeedsReload => toasts.warning(notice.message.clone()),
        renzora::HotLoadOutcome::Skipped => toasts.info(notice.message.clone()),
        renzora::HotLoadOutcome::Failed => toasts.error(notice.message.clone()),
    }
}

/// Returns true if an entity with a terrain-data component is currently selected.
/// Used by terrain toolbar entries and the terrain tool panel to show
/// context-sensitive UI only when a terrain is active.
pub fn is_terrain_selected(world: &World) -> bool {
    let Some(sel) = world.get_resource::<EditorSelection>() else {
        return false;
    };
    let Some(entity) = sel.get() else {
        return false;
    };

    fn has_terrain_component(world: &World, entity: Entity) -> bool {
        let Ok(er) = world.get_entity(entity) else {
            return false;
        };
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

/// Register the built-in Transform tools with the [`ToolbarRegistry`].
/// Predicate: transform gizmo tools are hidden in 2D view (the 2D editor uses
/// direct pick/drag, not the 3D translate/rotate/scale gizmos) and in the
/// mesh Edit/Sculpt modes (element editing disengages the entity gizmo, so
/// the buttons would be dead weight next to the modeling section).
fn visible_when_not_2d(w: &World) -> bool {
    use renzora::core::viewport_types::{ViewportMode, ViewportSettings, ViewportView};
    w.get_resource::<ViewportSettings>()
        .map(|s| {
            s.viewport_view != ViewportView::Two
                && !matches!(s.viewport_mode, ViewportMode::Edit | ViewportMode::Sculpt)
        })
        .unwrap_or(true)
}

/// Hide in the mesh Edit/Sculpt modes only — for Select, which unlike the
/// gizmo tools stays useful in the 2D view.
fn visible_outside_mesh_modes(w: &World) -> bool {
    use renzora::core::viewport_types::{ViewportMode, ViewportSettings};
    w.get_resource::<ViewportSettings>()
        .map(|s| !matches!(s.viewport_mode, ViewportMode::Edit | ViewportMode::Sculpt))
        .unwrap_or(true)
}

/// Called once at plugin build time.
fn register_builtin_tools(registry: &mut ToolbarRegistry) {
    // Icons are kebab-case Phosphor names resolved by the native toolbar renderer.

    // Transform section — Select stays in all views; the move/rotate/scale
    // gizmo tools hide in 2D. Everything hides in mesh Edit/Sculpt modes.
    registry.register(
        ToolEntry::new(
            "builtin.select",
            "cursor",
            "Select (Q)",
            ToolSection::Transform,
        )
        .order(0)
        .visible_if(visible_outside_mesh_modes)
        .active_if(|w| {
            w.get_resource::<ActiveTool>()
                .copied() == Some(ActiveTool::Select)
        })
        .on_activate(|w| {
            w.insert_resource(ActiveTool::Select);
        }),
    );
    registry.register(
        ToolEntry::new(
            "builtin.translate",
            "arrows-out-cardinal",
            "Move (W)",
            ToolSection::Transform,
        )
        .order(1)
        .visible_if(visible_when_not_2d)
        .active_if(|w| {
            w.get_resource::<ActiveTool>()
                .copied() == Some(ActiveTool::Translate)
        })
        .on_activate(|w| {
            w.insert_resource(ActiveTool::Translate);
        }),
    );
    registry.register(
        ToolEntry::new(
            "builtin.rotate",
            "arrows-counter-clockwise",
            "Rotate (E)",
            ToolSection::Transform,
        )
        .order(2)
        .visible_if(visible_when_not_2d)
        .active_if(|w| {
            w.get_resource::<ActiveTool>()
                .copied() == Some(ActiveTool::Rotate)
        })
        .on_activate(|w| {
            w.insert_resource(ActiveTool::Rotate);
        }),
    );
    registry.register(
        ToolEntry::new(
            "builtin.scale",
            "arrows-out-simple",
            "Scale (R)",
            ToolSection::Transform,
        )
        .order(3)
        .visible_if(visible_when_not_2d)
        .active_if(|w| {
            w.get_resource::<ActiveTool>()
                .copied() == Some(ActiveTool::Scale)
        })
        .on_activate(|w| {
            w.insert_resource(ActiveTool::Scale);
        }),
    );

    // Terrain tools (builtin.terrain_sculpt / terrain_paint / foliage_paint) are
    // registered by `renzora_terrain_editor::TerrainEditorPlugin` so their
    // activators can reach `TerrainData` and the inspector tab state directly.
}

/// Keep `GizmoMode` in sync with `ActiveTool` so gizmo systems that still read
/// `GizmoMode` continue to work during the migration.
fn sync_active_tool_to_gizmo_mode(
    active_tool: Res<ActiveTool>,
    gizmo_mode: Option<ResMut<GizmoMode>>,
) {
    let Some(mut gizmo_mode) = gizmo_mode else {
        return;
    };
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

/// Sync `EditorContext` to whatever the active document tab represents and
/// route the workspace layout accordingly. Scene tabs restore the user's
/// last chosen scene-mode layout (`last_scene_index`); asset tabs force
/// their hidden asset-mode variant.
///
/// Call this after any change to the active document tab (activate, close,
/// add). Idempotent — safe to call when nothing changed.
fn sync_context_and_layout_for_active_tab(world: &mut World) {
    use renzora_ui::{DocumentTabState, EditorContext};

    let Some(tab) = world
        .get_resource::<DocumentTabState>()
        .and_then(|ts| ts.active_tab().cloned())
    else {
        return;
    };

    let new_ctx = EditorContext::from_tab(&tab);
    world.insert_resource(new_ctx.clone());

    match new_ctx {
        EditorContext::Scene => {
            let restore_idx = world
                .get_resource::<LayoutManager>()
                .map(|lm| lm.last_scene_index)
                .unwrap_or(0);
            // Only switch if we're currently in a different (likely hidden)
            // layout — avoids resetting the user's dock edits unnecessarily.
            let needs_switch = world
                .get_resource::<LayoutManager>()
                .map(|lm| lm.active_index != restore_idx)
                .unwrap_or(false);
            if needs_switch {
                switch_layout(world, restore_idx);
            }
        }
        EditorContext::Asset { kind, .. } => {
            // Prefer the hidden asset-mode layout when the kind has one
            // (panels know how to render from file path). Otherwise fall
            // back to the scene-mode layout for that kind — same as the
            // pre-context-aware behaviour, until those panels are wired.
            if let Some(layout) = kind.asset_layout_name().or_else(|| kind.layout_name()) {
                switch_layout_by_name(world, layout);
            }
        }
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

/// Open (or focus an existing) document tab for the given asset path, then
/// switch to the layout appropriate for its kind. Called by the asset browser
/// when the user double-clicks a file.
pub fn open_asset_tab(world: &mut World, path: &std::path::Path, kind: renzora_ui::DocTabKind) {
    use renzora_ui::{DocTabKind, DocumentTabState};

    let rel = world
        .get_resource::<renzora::core::CurrentProject>()
        .and_then(|p| p.make_relative(path))
        .unwrap_or_else(|| path.to_string_lossy().replace('\\', "/"));

    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Untitled".to_string());

    let existing = world
        .get_resource::<DocumentTabState>()
        .and_then(|ts| ts.find_by_path(&rel, kind));

    let switch_ids: Option<(u64, u64)> = if let Some(idx) = existing {
        world
            .get_resource_mut::<DocumentTabState>()
            .and_then(|mut ts| ts.activate_tab(idx))
    } else {
        let old_id = world
            .get_resource::<DocumentTabState>()
            .and_then(|ts| ts.active_tab_id());
        let new_id = world.get_resource_mut::<DocumentTabState>().map(|mut ts| {
            let idx = ts.add_tab_of_kind(name, Some(rel), kind);
            ts.active_tab = idx;
            ts.touch_scene_mru(idx); // no-op for asset kinds
            ts.tabs[idx].id
        });
        match (old_id, new_id) {
            (Some(o), Some(n)) if o != n => Some((o, n)),
            _ => None,
        }
    };

    if let Some((old_id, new_id)) = switch_ids {
        world.insert_resource(renzora::TabSwitchRequest {
            old_tab_id: old_id,
            new_tab_id: new_id,
        });
    }

    // Sync EditorContext + route to the appropriate (asset-mode or scene-mode) layout.
    sync_context_and_layout_for_active_tab(world);

    if matches!(kind, DocTabKind::Script | DocTabKind::Shader) {
        world.insert_resource(renzora::core::OpenCodeEditorFile {
            path: path.to_path_buf(),
        });
    }
}

// ── Editor drop zones ───────────────────────────────────────────────────────

/// Marks a panel content root as an asset-drop target — dropping a matching
/// asset over it opens that asset in its editor. Attach via [`mark_drop_zone`];
/// handled by [`editor_panel_drop`].
#[derive(Component)]
pub struct EditorDropZone;

/// Make `entity` (a native panel's content root) an asset-drop target: inserts
/// the [`EditorDropZone`] marker plus a `RelativeCursorPosition` so the drop
/// system can hit-test the cursor against it.
pub fn mark_drop_zone(commands: &mut Commands, entity: Entity) {
    commands
        .entity(entity)
        .insert((EditorDropZone, bevy::ui::RelativeCursorPosition::default()));
}

/// Map a file path to the document-tab kind it opens as, or `None` for files
/// that don't correspond to an editor-opening asset type.
pub fn doc_kind_for_path(path: &std::path::Path) -> Option<DocTabKind> {
    let name = path.file_name().and_then(|n| n.to_str()).map(|s| s.to_lowercase())?;
    if name.ends_with(".material_bp") || name.ends_with(".material") {
        return Some(DocTabKind::Material);
    }
    if name.ends_with(".particle") {
        return Some(DocTabKind::Particle);
    }
    if name.ends_with(".blueprint") || name.ends_with(".bp") {
        return Some(DocTabKind::Blueprint);
    }
    let ext = name.rsplit('.').next().unwrap_or("");
    Some(match ext {
        "bsn" | "ron" => DocTabKind::Scene,
        "rhai" | "lua" | "js" | "ts" | "py" | "html" => DocTabKind::Script,
        "wgsl" | "glsl" | "vert" | "frag" => DocTabKind::Shader,
        _ => return None,
    })
}

/// On release of a *detached* asset drag whose cursor is over an
/// [`EditorDropZone`], open the dropped asset in its visual editor
/// (material / particle / blueprint). Script/shader drops are handled by the
/// code editor's own drop target, so they're ignored here.
fn editor_panel_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    payload: Option<Res<AssetDragPayload>>,
    zones: Query<&bevy::ui::RelativeCursorPosition, With<EditorDropZone>>,
    cmds: Option<Res<EditorCommands>>,
    mut commands: Commands,
) {
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let Some(payload) = payload else { return };
    if !payload.is_detached {
        return;
    }
    let Some(kind) = doc_kind_for_path(&payload.path) else { return };
    if !matches!(kind, DocTabKind::Material | DocTabKind::Particle | DocTabKind::Blueprint) {
        return;
    }
    if !zones.iter().any(|r| r.cursor_over) {
        return;
    }
    let Some(cmds) = cmds else { return };
    let path = payload.path.clone();
    cmds.push(move |world: &mut World| open_asset_tab(world, &path, kind));
    commands.remove_resource::<AssetDragPayload>();
}

/// Handle "New Project" — close current project and return to splash screen.
/// Public so the bevy_ui shell can drive File > New/Open Project.
pub fn handle_new_project(world: &mut World) {
    world.remove_resource::<renzora::CurrentProject>();
    world
        .resource_mut::<NextState<SplashState>>()
        .set(SplashState::Splash);
    info!("Returning to splash screen for new project");
}

/// Handle "Open Project" — emit a marker resource the splash plugin
/// consumes (it owns the file dialog + AppConfig + state transition).
/// Keeps `renzora_editor_framework` decoupled from `renzora_splash`.
pub fn handle_open_project(world: &mut World) {
    world.insert_resource(renzora::RequestOpenProject);
}
