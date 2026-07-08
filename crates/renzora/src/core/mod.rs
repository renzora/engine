pub mod console_log;
pub mod keybindings;
pub mod reflection;
pub mod viewport_types;

// Sub-areas split out of this file to keep it manageable. Each is re-exported
// flat (`pub use <mod>::*`) so every `renzora::Foo` path is unchanged — the
// dlopen contract only cares that the types resolve to this one shared dylib.
pub mod animation; // .anim clip format + property keyframes
pub mod components; // shared ECS components + entity-tag markers
pub mod project_config; // project.toml model + editor preferences
pub mod sprite_anim; // multi-sheet sprites (SpriteImages) for 2D animation
pub use animation::*;
pub use project_config::*;
pub use components::*;
pub use sprite_anim::*;

use bevy::input::gamepad::{GamepadAxis, GamepadButton};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// ============================================================================
// Asset byte loader
// ============================================================================

/// Pluggable loader that reads raw bytes for a project-relative asset key
/// (e.g. `"particles/fire.particle"`). The host engine installs it once with a
/// virtual-filesystem-aware closure (rpak archive in exported games, loose
/// files on disk in the editor). Crates that load assets *outside* Bevy's
/// AssetServer — audio (Kira) and particle effects — go through this so they
/// work in exported `.rpak` builds, not just on-disk projects.
type AssetByteLoader = dyn Fn(&str) -> Option<Vec<u8>> + Send + Sync;

static ASSET_BYTE_LOADER: std::sync::OnceLock<std::sync::Mutex<Option<Box<AssetByteLoader>>>> =
    std::sync::OnceLock::new();

fn asset_byte_loader_cell() -> &'static std::sync::Mutex<Option<Box<AssetByteLoader>>> {
    ASSET_BYTE_LOADER.get_or_init(|| std::sync::Mutex::new(None))
}

/// Install the project/VFS-aware byte loader. Called by the host engine once
/// the virtual filesystem and project root are known.
pub fn set_asset_byte_loader(f: Box<AssetByteLoader>) {
    if let Ok(mut guard) = asset_byte_loader_cell().lock() {
        *guard = Some(f);
    }
}

/// Load raw bytes for a project-relative asset key via the installed loader.
/// Returns `None` if no loader is installed or the asset can't be found.
pub fn load_asset_bytes(relative: &str) -> Option<Vec<u8>> {
    asset_byte_loader_cell()
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|f| f(relative)))
        .flatten()
}

// ============================================================================
// Shape Registry
// ============================================================================

/// A registered shape that can be spawned and rehydrated.
pub struct ShapeEntry {
    /// Unique identifier (e.g. `"cube"`, `"spiral_stairs"`). Must match `MeshPrimitive` values.
    pub id: &'static str,
    /// Human-readable name shown in UI.
    pub name: &'static str,
    /// Phosphor icon glyph for the shape library panel.
    pub icon: &'static str,
    /// Category string for grouping in UI (e.g. `"Basic"`, `"Level"`).
    pub category: &'static str,
    /// Factory function that creates the mesh.
    pub create_mesh: fn(&mut Assets<Mesh>) -> Handle<Mesh>,
    /// Default base color when spawning.
    pub default_color: Color,
}

/// Global registry of available shapes. Crates register shapes during plugin `build()`.
///
/// Used by the shape library panel (editor) and rehydration (runtime) to look up
/// mesh factories by ID.
#[derive(Resource, Default)]
pub struct ShapeRegistry {
    entries: Vec<ShapeEntry>,
}

impl ShapeRegistry {
    /// Register a new shape. Duplicate IDs are silently ignored.
    pub fn register(&mut self, entry: ShapeEntry) {
        if self.entries.iter().any(|e| e.id == entry.id) {
            return;
        }
        self.entries.push(entry);
    }

    /// Look up a shape by ID.
    pub fn get(&self, id: &str) -> Option<&ShapeEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Look up a shape by ID (mutable).
    pub fn get_mut(&mut self, id: &str) -> Option<&mut ShapeEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    /// Iterate over all registered shapes.
    pub fn iter(&self) -> impl Iterator<Item = &ShapeEntry> {
        self.entries.iter()
    }

    /// Create a mesh for the given shape ID, or `None` if not registered.
    pub fn create_mesh(&self, id: &str, meshes: &mut Assets<Mesh>) -> Option<Handle<Mesh>> {
        self.get(id).map(|entry| (entry.create_mesh)(meshes))
    }
}

/// Base color for an entity's material — serializable companion to `MeshMaterial3d`.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct MeshColor(pub Color);

/// The built-in checkerboard "no texture yet" image, applied as
/// `base_color_texture` on the default material of newly spawned primitives
/// (and their scene-load rehydration) so untextured geometry reads as
/// deliberately-untextured rather than flat plastic. `MeshColor` still tints
/// it via `base_color`. Inserted at startup by the engine; consumers treat
/// it as optional so headless/server builds (no `Assets<Image>`) still work.
///
/// The image comes from [`build_checker_image`] — the same checker the
/// viewport's Textures-off toggle swaps in — so "new untextured shape" and
/// "textures disabled" read identically.
#[derive(Resource, Clone)]
pub struct CheckerTexture(pub Handle<Image>);

/// Encode a linear grey value to the sRGB bytes an `Rgba8UnormSrgb` texture
/// decodes back to that same linear value on sample (so lighting matches the
/// terrain checker, whose colors are specified in linear space).
fn checker_srgb_bytes(linear: f32) -> [u8; 4] {
    let s = Srgba::from(LinearRgba::new(linear, linear, linear, 1.0));
    [
        (s.red * 255.0).round() as u8,
        (s.green * 255.0).round() as u8,
        (s.blue * 255.0).round() as u8,
        255,
    ]
}

/// Bake the terrain-default grey checkerboard into a small point-sampled,
/// repeating texture. Tiles crisply across a mesh's UVs (and wraps for UVs
/// outside 0..1), the closest `StandardMaterial` equivalent of terrain's
/// world-space procedural checker. Single source of truth for every checker
/// consumer: the viewport's Textures-off swap and [`CheckerTexture`].
pub fn build_checker_image() -> Image {
    use bevy::asset::RenderAssetUsages;
    use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
    use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

    // TerrainCheckerboardMaterial::default() colors (linear grey).
    let a = checker_srgb_bytes(0.32);
    let b = checker_srgb_bytes(0.22);
    const CELLS: usize = 2; // checker squares per axis across one UV tile
    const SIZE: usize = CELLS * 2; // 2 px/cell — point-sampled, stays sharp
    let mut data = Vec::with_capacity(SIZE * SIZE * 4);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let on = ((x / 2) + (y / 2)) % 2 == 0;
            data.extend_from_slice(if on { &a } else { &b });
        }
    }
    let mut image = Image::new(
        Extent3d {
            width: SIZE as u32,
            height: SIZE as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Nearest,
        min_filter: ImageFilterMode::Nearest,
        ..Default::default()
    });
    image
}

// ============================================================================
// Editor ↔ Physics decoupling events
// ============================================================================

/// Sent by the editor to request pausing the physics simulation.
#[derive(bevy::prelude::Event)]
pub struct PausePhysics;

/// Sent by the editor to request unpausing the physics simulation.
#[derive(bevy::prelude::Event)]
pub struct UnpausePhysics;

/// Sent by the editor to request resetting all script runtime states.
#[derive(bevy::prelude::Event)]
pub struct ResetScriptStates;

/// Notification that scripts were hot-reloaded. The scripting crate triggers
/// this so the editor can show toast notifications without importing scripting.
#[derive(bevy::prelude::Event)]
pub struct ScriptsReloaded {
    pub names: Vec<String>,
}

/// Outcome of a mid-session plugin hot-load attempt — a `.dll`/`.so`/`.dylib`
/// dropped into the `plugins/` directory while the app is running.
///
/// The dynamic plugin loader builds the plugin into the live `World` via a
/// temporary `App` that borrows the running world, so any plugin that only
/// touches the **main** world (gameplay, components, resources, systems, UI)
/// activates on the next frame. A plugin that also targets the **render** world
/// (post-process effects, custom render-graph nodes) can't be wired into the
/// already-initialized renderer at runtime and needs a restart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HotLoadOutcome {
    /// Built fully into the live world — active next frame.
    Loaded,
    /// Loaded as far as the main world allows, but the plugin also targets the
    /// render world, which can't be hot-wired. Restart to take full effect.
    NeedsReload,
    /// Not loaded (wrong scope for this host, incompatible ABI, or a plugin
    /// with the same name is already loaded — restart to replace it).
    Skipped,
    /// The plugin's `build` panicked or its entry symbol was missing.
    Failed,
}

/// Fired once per hot-load attempt by the dynamic plugin loader. Defined in the
/// shared `renzora` dylib so the binary-side loader that triggers it and the
/// editor-bundle observer that turns it into a toast resolve one `TypeId`
/// across the dlopen boundary (mirrors [`ScriptsReloaded`]). The runtime, which
/// has no toast UI, simply ignores it (the loader also logs every outcome).
#[derive(bevy::prelude::Event, Clone, Debug)]
pub struct HotPluginNotice {
    /// The plugin's file stem (e.g. `my_cool_effect`).
    pub id: String,
    /// What happened.
    pub outcome: HotLoadOutcome,
    /// A human-readable message suitable for a toast.
    pub message: String,
}

/// Sent by the editor to save the current scene before play mode.
#[derive(bevy::prelude::Event)]
pub struct SaveCurrentScene;

/// Snapshot the live scene into an in-memory buffer before entering Simulate
/// mode, so [`RestoreSimulateSnapshot`] can revert every mutation the simulation
/// makes (moved bodies, ragdoll pose, spawned/despawned entities) on Stop.
/// Observed by `renzora_engine` (which owns scene (de)serialization); the editor
/// only fires the event so the dependency direction stays one-way.
#[derive(bevy::prelude::Event)]
pub struct SnapshotSceneForSimulate;

/// Restore the scene captured by [`SnapshotSceneForSimulate`] when leaving
/// Simulate mode. A no-op if no snapshot was taken.
#[derive(bevy::prelude::Event)]
pub struct RestoreSimulateSnapshot;

/// Fired by the editor immediately after a document tab is closed, with
/// the closed tab's id. Lets per-tab caches (asset handles, undo stacks,
/// etc.) drop their entries without coupling the editor to every
/// downstream consumer.
#[derive(bevy::prelude::Event, Debug, Clone, Copy)]
pub struct TabClosed {
    pub tab_id: u64,
}

// ============================================================================
// Character Controller Commands (shared between scripting and physics)
// ============================================================================

/// Queued character controller commands, processed by renzora_physics each frame.
#[derive(bevy::prelude::Resource, Default)]
pub struct CharacterCommandQueue {
    pub commands: Vec<(bevy::ecs::entity::Entity, CharacterCommand)>,
}

/// A character controller command for a specific entity.
#[derive(Debug)]
pub enum CharacterCommand {
    Move(bevy::prelude::Vec2),
    Jump,
    Sprint(bool),
}

// ============================================================================
// Action State (shared between input and physics/scripting)
// ============================================================================

/// Per-action runtime state computed each frame by the input system.
#[derive(Clone, Debug, Default)]
pub struct ActionData {
    pub pressed: bool,
    pub just_pressed: bool,
    pub just_released: bool,
    pub axis_1d: f32,
    pub axis_2d: bevy::prelude::Vec2,
}

/// Computed action states, populated by the input system and read by
/// physics, scripting, and blueprints each frame.
#[derive(bevy::prelude::Resource, Clone, Debug, Default)]
pub struct ActionState {
    pub actions: std::collections::HashMap<String, ActionData>,
}

impl ActionState {
    pub fn pressed(&self, action: &str) -> bool {
        self.actions.get(action).is_some_and(|a| a.pressed)
    }
    pub fn just_pressed(&self, action: &str) -> bool {
        self.actions.get(action).is_some_and(|a| a.just_pressed)
    }
    pub fn just_released(&self, action: &str) -> bool {
        self.actions.get(action).is_some_and(|a| a.just_released)
    }
    pub fn axis_1d(&self, action: &str) -> f32 {
        self.actions.get(action).map_or(0.0, |a| a.axis_1d)
    }
    pub fn axis_2d(&self, action: &str) -> bevy::prelude::Vec2 {
        self.actions
            .get(action)
            .map_or(bevy::prelude::Vec2::ZERO, |a| a.axis_2d)
    }
}

// ============================================================================
// MaterialRef (shared between material and terrain)
// ============================================================================

/// Reference to a material file. Add to any entity with `Mesh3d` to assign a material.
#[derive(
    bevy::prelude::Component,
    serde::Serialize,
    serde::Deserialize,
    bevy::prelude::Reflect,
    Clone,
    Debug,
)]
#[reflect(Component, Serialize, Deserialize)]
pub struct MaterialRef(pub String);


/// Generic script action event. Scripts call `action("name", { args })` and
/// domain crates observe this event to handle actions they recognize.
/// This decouples scripting from domain crates — no ScriptExtension imports needed.
#[derive(bevy::prelude::Event, Debug, Clone)]
pub struct ScriptAction {
    /// The action name (e.g. "apply_force", "play_sound", "gauge_damage").
    pub name: String,
    /// The entity that triggered the action (script's owning entity).
    pub entity: bevy::ecs::entity::Entity,
    /// Optional target entity (by name or ID).
    pub target_entity: Option<String>,
    /// Action arguments as key-value pairs.
    pub args: std::collections::HashMap<String, ScriptActionValue>,
}

/// Value types for script action arguments.
#[derive(Debug, Clone)]
pub enum ScriptActionValue {
    Float(f32),
    Int(i64),
    Bool(bool),
    String(String),
    Vec3([f32; 3]),
}

// ============================================================================
// Play Mode
// ============================================================================

/// Current play-mode state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PlayState {
    /// Normal editing.
    #[default]
    Editing,
    /// Game is running (game camera active, editor overlays hidden).
    Playing,
    /// Game is paused.
    Paused,
    /// Simulating in-editor: scripts + physics + animation run, but the editor
    /// stays fully live — editor camera, gizmos, selection and inspector all
    /// remain active, unlike [`PlayState::Playing`] which swaps to the game
    /// camera and hides the editor chrome. Entering snapshots the scene; Stop
    /// restores it, so a simulation never permanently mutates the scene.
    Simulating,
}

/// Editor signal: a viewport "brush"/paint tool is currently active (e.g. the
/// tilemap paint tool). While set, the 2D pick/drag systems stand down so a
/// click paints instead of re-selecting or dragging the entity out from under
/// the brush. Any editor tool may raise it; it lives in the contract so the
/// gizmo crate can read it without depending on the tool's crate.
#[derive(Resource, Default)]
pub struct ViewportBrushActive(pub bool);

/// Resource that tracks play mode state and pending transitions.
#[derive(Resource, Default)]
pub struct PlayModeState {
    pub state: PlayState,
    /// Entity of the active game camera during play mode.
    pub active_game_camera: Option<bevy::ecs::entity::Entity>,
    /// Set to `true` to request entering play mode next frame.
    pub request_play: bool,
    /// Set to `true` to request entering Simulate mode next frame (run the
    /// simulation while keeping the editor live; see [`PlayState::Simulating`]).
    pub request_simulate: bool,
    /// Set to `true` to request stopping play mode next frame.
    pub request_stop: bool,
    /// Set to `true` to toggle pause.
    pub request_pause: bool,
}

impl PlayModeState {
    pub fn is_playing(&self) -> bool {
        self.state == PlayState::Playing
    }
    pub fn is_paused(&self) -> bool {
        self.state == PlayState::Paused
    }
    pub fn is_editing(&self) -> bool {
        self.state == PlayState::Editing
    }
    /// Returns true while simulating in-editor (editor chrome stays live).
    pub fn is_simulating(&self) -> bool {
        self.state == PlayState::Simulating
    }
    /// Returns true if in Playing or Paused state (full play mode). Deliberately
    /// EXCLUDES `Simulating`: callers use this to hide editor chrome / swap to the
    /// game camera, and Simulate keeps the editor live, so it must read as "not in
    /// play mode" for all that tooling to stay active.
    pub fn is_in_play_mode(&self) -> bool {
        matches!(self.state, PlayState::Playing | PlayState::Paused)
    }
    /// Returns true if scripts (and the physics/animation they drive) should be
    /// executing this frame — true in both full Play and in-editor Simulate.
    pub fn is_scripts_running(&self) -> bool {
        matches!(self.state, PlayState::Playing | PlayState::Simulating)
    }
}

/// Run condition: returns true when NOT in play mode (i.e. editing).
/// Use as `.run_if(not_in_play_mode)` on editor systems that should be disabled during play.
pub fn not_in_play_mode(play_mode: Option<Res<PlayModeState>>) -> bool {
    !play_mode.as_ref().is_some_and(|pm| pm.is_in_play_mode())
}

/// Per-panel metadata for the bevy_ui editor shell, keyed by panel id.
///
/// `renzora_shell` seeds this with each panel's title/icon at startup (its
/// `PANEL_META` table); plugins can add or override entries via
/// [`RenzoraShellExt::register_shell_panel`].
#[derive(Resource, Default)]
pub struct ShellPanelRegistry {
    pub panels: bevy::platform::collections::HashMap<String, ShellPanelInfo>,
}

#[derive(Clone, Default)]
pub struct ShellPanelInfo {
    pub title: String,
    /// Phosphor icon NAME (kebab-case), resolved to a glyph via
    /// `renzora_ember::font::icon_glyph` (empty if none).
    pub icon: String,
    pub category: String,
}

/// One drawn piece of a bevy_ui status-bar item: an optional phosphor icon
/// (name *or* raw glyph) + text + color.
#[derive(Clone)]
pub struct ShellStatusSegment {
    pub icon: String,
    pub text: String,
    pub color: [u8; 3],
}

impl ShellStatusSegment {
    pub fn new(icon: impl Into<String>, text: impl Into<String>, color: [u8; 3]) -> Self {
        Self {
            icon: icon.into(),
            text: text.into(),
            color,
        }
    }
}

/// Which side of the status bar an item sits on.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ShellStatusAlign {
    Left,
    Right,
}

/// A bevy_ui status-bar item contributed by a plugin (the native counterpart of
/// the egui `StatusBarItem`). `render` runs each frame with `&World` and returns
/// the current segments, so live metrics update without re-registering.
pub struct ShellStatusItem {
    pub id: &'static str,
    pub align: ShellStatusAlign,
    pub order: i32,
    pub render: fn(&bevy::prelude::World) -> Vec<ShellStatusSegment>,
}

/// Registry of bevy_ui status-bar items. Any renzora plugin can push to it; the
/// shell renders them (no egui dependency).
#[derive(Resource, Default)]
pub struct ShellStatusRegistry {
    pub items: Vec<ShellStatusItem>,
}

/// Overrides the status bar's left-hand **"Ready"** label. The host owns the
/// status bar, so a plugin can't replace that label by registering a status item
/// (those only *append*). Instead it writes here: `label = Some(text)` swaps the
/// "Ready" text for `text` (in `color`, falling back to the muted default when
/// `None`); `label = None` restores "Ready". This is how the auto-save plugin
/// shows its "Auto save in Ns" countdown in place of "Ready".
#[derive(Resource, Default)]
pub struct ShellReadyStatus {
    pub label: Option<String>,
    pub color: Option<[u8; 3]>,
}

/// The bevy-native editor-extension API. A renzora plugin (full ECS access) uses
/// this to add panels + status-bar items to the bevy_ui shell directly — no
/// egui, no bridge — mirroring how `#[derive]` component macros let plugins add
/// their own data.
pub trait RenzoraShellExt {
    /// Register a panel's metadata (title/icon/category) for the dock + Add-Panel
    /// picker. The panel's *content* is registered separately via
    /// `renzora_ember`'s `register_panel_content`.
    fn register_shell_panel(
        &mut self,
        id: impl Into<String>,
        title: impl Into<String>,
        icon: impl Into<String>,
        category: impl Into<String>,
    ) -> &mut Self;

    /// Register a status-bar item.
    fn register_shell_status_item(&mut self, item: ShellStatusItem) -> &mut Self;
}

impl RenzoraShellExt for bevy::app::App {
    fn register_shell_panel(
        &mut self,
        id: impl Into<String>,
        title: impl Into<String>,
        icon: impl Into<String>,
        category: impl Into<String>,
    ) -> &mut Self {
        self.init_resource::<ShellPanelRegistry>();
        self.world_mut().resource_mut::<ShellPanelRegistry>().panels.insert(
            id.into(),
            ShellPanelInfo {
                title: title.into(),
                icon: icon.into(),
                category: category.into(),
            },
        );
        self
    }

    fn register_shell_status_item(&mut self, item: ShellStatusItem) -> &mut Self {
        self.init_resource::<ShellStatusRegistry>();
        self.world_mut()
            .resource_mut::<ShellStatusRegistry>()
            .items
            .push(item);
        self
    }
}

/// Panel ids that have a **bevy-native** (ember) content renderer — i.e. their
/// own crate builds the panel into the dock leaf and keeps it in sync, instead
/// of the shell's placeholder/`content_dispatch`. The shell skips these ids so
/// the two never fight over the same `content` entity.
#[derive(Resource, Default)]
pub struct NativePanelIds(pub bevy::platform::collections::HashSet<String>);

/// Lets a panel crate declare it owns the bevy_ui rendering for an id.
pub trait NativePanelExt {
    /// Mark `id` as having a native ember content renderer (order-independent).
    fn register_native_panel(&mut self, id: &str) -> &mut Self;
}

impl NativePanelExt for App {
    fn register_native_panel(&mut self, id: &str) -> &mut Self {
        self.init_resource::<NativePanelIds>();
        if let Some(mut ids) = self.world_mut().get_resource_mut::<NativePanelIds>() {
            ids.0.insert(id.to_string());
        }
        self
    }
}

/// Run condition: returns true when the viewport is in 3D view. Use on
/// editor systems whose visuals (transform gizmo arrows, collider wireframes,
/// rotation pies, etc.) only make sense projecting through a 3D camera.
pub fn in_three_view(settings: Option<Res<crate::core::viewport_types::ViewportSettings>>) -> bool {
    use crate::core::viewport_types::ViewportView;
    settings.is_none_or(|s| s.viewport_view == ViewportView::Three)
}

/// Run condition: returns true when the viewport is in 2D view. Use on
/// editor systems that pick/drag/draw 2D entities through the orthographic
/// editor camera.
pub fn in_two_view(settings: Option<Res<crate::core::viewport_types::ViewportSettings>>) -> bool {
    use crate::core::viewport_types::ViewportView;
    settings.is_some_and(|s| s.viewport_view == ViewportView::Two)
}

/// Marker component added to the game camera entity during play mode.
#[derive(Component)]
pub struct PlayModeCamera;

/// Lightweight network status bridge — updated by the network crate,
/// read by blueprint and other crates that need connection info without
/// depending on renzora_network.
#[derive(Resource, Default, Clone, Debug)]
pub struct NetworkBridge {
    /// Whether this instance is running as a server.
    pub is_server: bool,
    /// Whether the client is connected to a server (or server is running).
    pub is_connected: bool,
    /// Number of connected clients (server only).
    pub player_count: i32,
}

/// A single networked RPC delivered to this peer, awaiting dispatch to
/// scripts' `on_rpc(name, args)` hook.
#[derive(Clone, Debug)]
pub struct IncomingRpc {
    /// RPC name the sender used (the first arg to `rpc(name, args)`).
    pub name: String,
    /// Decoded argument table.
    pub args: std::collections::HashMap<String, ScriptActionValue>,
    /// Sender's peer id (0 = server/local).
    pub from: u64,
}

/// Inbox bridge for networked RPCs received from the wire.
///
/// `renzora_network` pushes a [`IncomingRpc`] here for every `GameEvent` it
/// receives; `renzora_scripting` drains it each frame and invokes every
/// script's `on_rpc(name, args)` hook. Lives in core because scripting must
/// not depend on the network crate (same indirection as [`NetworkBridge`]).
#[derive(Resource, Default)]
pub struct ScriptRpcInbox {
    pub pending: Vec<IncomingRpc>,
}

/// A player join/leave event, awaiting dispatch to scripts'
/// `on_player_joined(id)` / `on_player_left(id)` hooks. Server-authoritative:
/// only the server/host observes connections, so only it produces these.
#[derive(Clone, Debug)]
pub struct NetPlayerEvent {
    /// Peer id that joined or left (same id space as [`IncomingRpc::from`]).
    pub id: u64,
    /// `true` = joined, `false` = left.
    pub joined: bool,
}

/// Inbox of player lifecycle events. `renzora_network` (server side) pushes a
/// [`NetPlayerEvent`] when a client connects/disconnects; `renzora_scripting`
/// drains it each frame and invokes every script's `on_player_joined(id)` /
/// `on_player_left(id)` hook. Lives in core for the same reason as
/// [`ScriptRpcInbox`] — scripting must not depend on the network crate.
#[derive(Resource, Default)]
pub struct ScriptNetLifecycleInbox {
    pub pending: Vec<NetPlayerEvent>,
}

/// A UI markup callback awaiting dispatch to scripts' `on_ui(name, args)` hook.
///
/// Produced by `renzora_hui` when a `bevy_hui` template node fires an event
/// (e.g. `on_press="start_game"`) that has no Rust-side `HtmlFunctions`
/// binding — the name then falls through to scripts instead.
#[derive(Clone, Debug)]
pub struct UiCallback {
    /// The markup callback name (the value of `on_press` / `on_change` / …).
    pub name: String,
    /// `tag:`-prefixed markup attributes on the node, decoded as args.
    pub args: std::collections::HashMap<String, ScriptActionValue>,
    /// The UI node entity that fired the event, as raw bits (`Entity::to_bits`).
    /// Scripts receive this so they can target the originating widget.
    pub entity_bits: u64,
}

/// Inbox bridge for UI markup callbacks. `renzora_hui` pushes a [`UiCallback`]
/// when a template event fires; `renzora_scripting` drains it each frame and
/// invokes every script's `on_ui(name, args)` hook (broadcast, same semantics
/// as [`ScriptRpcInbox`]). Lives in core so scripting depends on neither
/// `renzora_hui` nor `bevy_hui`.
#[derive(Resource, Default)]
pub struct ScriptUiInbox {
    pub pending: Vec<UiCallback>,
}

/// An animation event fired when playback crosses a clip marker, awaiting
/// dispatch to scripts' `on_animation_event(name, entity)` hook.
#[derive(Clone, Debug)]
pub struct AnimEvent {
    /// The marker name.
    pub name: String,
    /// The animator entity that fired it (`Entity::to_bits`).
    pub entity_bits: u64,
}

/// Inbox bridge for animation events. The animation runtime pushes an
/// [`AnimEvent`] when playback crosses a clip marker; `renzora_scripting` drains
/// it each frame and invokes every script's `on_animation_event(name, entity)`
/// hook (broadcast, same semantics as [`ScriptUiInbox`]). Lives in core so
/// scripting doesn't depend on `renzora_animation`.
#[derive(Resource, Default)]
pub struct ScriptAnimEventInbox {
    pub pending: Vec<AnimEvent>,
}

/// Marker resource: present when the runtime is running as a dedicated server
/// (`renzora-runtime --server`). Inserted before engine plugins build so they
/// can opt out of client/render-only setup. A dedicated server has no render
/// world (`backends: None`), so GPU-only plugins (e.g. `bevy_hanabi`) must
/// check this and skip their render-side initialization to avoid panicking on
/// the absent `RenderApp`. Networking uses it to skip client-side setup.
#[derive(Resource, Default)]
pub struct DedicatedServer;

/// Marker resource: present when the runtime is running as a host/listen-server
/// (`renzora-runtime --host`). Unlike [`DedicatedServer`] the host renders
/// normally (it has a local player), so it is *not* headless — it runs both the
/// client and server plugin sets in one process. Inserted before engine plugins
/// build so networking can wire host mode (client setup stays, the server plugin
/// owns the protocol/observers so they register exactly once).
#[derive(Resource, Default)]
pub struct HostServer;

/// Whether this process is an EDITOR session (the `renzora_editor` bundle dll
/// is present beside the exe) vs. a shipped game. Inserted by
/// `add_engine_plugins(is_editor)` before the engine plugins build. Lets the
/// dual-mode crates — compiled WITHOUT an `editor` cargo feature — still decide
/// editor-vs-game behaviour at RUNTIME, e.g. `RuntimePlugin` only runs the
/// rpak/project/scene game-startup when this is `false` (the editor's splash
/// drives loading otherwise). Defaults to `false` (a plain game) when absent.
#[derive(Resource, Clone, Copy, Default)]
pub struct EditorSession(pub bool);

impl EditorSession {
    /// True in an editor session (bundle present), false in a shipped game.
    pub fn is_editor(&self) -> bool {
        self.0
    }
}

/// Resource: request a scene load from scripts/blueprints.
/// The runtime system drains this each frame.
#[derive(Resource, Default)]
pub struct PendingSceneLoad {
    /// Scene name or relative path to load.
    pub requests: Vec<String>,
}

/// Marker resource requesting a scene save.
///
/// Insert this resource to trigger the scene save system next frame.
#[derive(Resource)]
pub struct SaveSceneRequested;

/// Request "Save As" — prompts user for a new scene name/path.
#[derive(Resource)]
pub struct SaveAsSceneRequested;

/// Request "New Scene" — clears the world and sets up a blank scene.
#[derive(Resource)]
pub struct NewSceneRequested;

/// Request "Open Scene" — prompts user to pick a scene file.
#[derive(Resource)]
pub struct OpenSceneRequested;

/// Request opening a *specific* scene file in its own document tab (loaded from
/// disk), e.g. double-clicking a `.bsn` in the asset browser or its "Open Scene"
/// context-menu item. Unlike [`OpenSceneRequested`] (which pops a file dialog),
/// this carries the path directly so the scene system can load it without a
/// prompt.
#[derive(Resource)]
pub struct OpenScenePathRequested(pub std::path::PathBuf);

/// When on, click systems log to the editor console what a left click actually
/// hit — cursor position, viewport hover state, every UI node that took
/// `Interaction::Pressed`, and the selection before/after — so a click that
/// "bleeds" between panels can be traced to the exact node/system responsible.
#[derive(Resource)]
pub struct ClickDebug(pub bool);

impl Default for ClickDebug {
    fn default() -> Self {
        // Default ON so click-hit tracing is available without a toggle dance.
        Self(true)
    }
}

/// Request a tab switch — serializes current scene, deserializes target.
#[derive(Resource)]
pub struct TabSwitchRequest {
    pub old_tab_id: u64,
    pub new_tab_id: u64,
}

/// In-memory snapshot of a scene tab's state (entities + camera).
pub struct TabSceneSnapshot {
    pub scene_ron: String,
    pub camera_focus: [f32; 3],
    pub camera_distance: f32,
    pub camera_yaw: f32,
    pub camera_pitch: f32,
}

/// Stores serialized scene data for each tab so switching tabs can serialize/deserialize.
#[derive(Resource, Default)]
pub struct SceneTabBuffers {
    pub buffers: std::collections::HashMap<u64, TabSceneSnapshot>,
}

/// Marker resource requesting the export overlay to open.
///
/// Insert this resource to trigger the export overlay next frame.
#[derive(Resource)]
pub struct ExportRequested;

/// Marker resource requesting the import overlay to open.
///
/// Insert this resource to trigger the import overlay next frame.
#[derive(Resource)]
pub struct ImportRequested;

/// Optional: carries the suggested target directory from the asset browser.
#[derive(Resource)]
pub struct ImportTargetDir(pub String);

/// The asset browser's current folder, project-relative and forward-slashed
/// (`""` = project root; `None` = no browser/project active). The browser
/// republishes it each frame so drag-and-drop imports land in the folder the
/// user is looking at, instead of the importer's default target. Read by the
/// importer's drop handler.
#[derive(Resource, Default)]
pub struct AssetBrowserCwd(pub Option<String>);

/// True while an OS file drag is hovering the editor window — set when a
/// `HoveredFile` event arrives and cleared on the matching drop or cancel. The
/// importer owns it (it already drains the file-drop events); the asset browser
/// reads it to render a "drop to import" highlight over its panel.
#[derive(Resource, Default)]
pub struct FileDragHovering(pub bool);

/// Set `true` by the importer when files are dropped onto the editor, so the
/// asset browser scrolls its grid to the freshly-imported items. The browser
/// resets it once consumed, then pins the grid to the bottom for a short window
/// (long enough for the ~0.5 s rescan to surface the new file and grow the grid).
#[derive(Resource, Default)]
pub struct AssetDropScrollRequest(pub bool);

/// Set true while the pointer is over a panel that owns the `Ctrl/Cmd+A`
/// shortcut for its own selection (currently the asset browser's file grid).
///
/// `Ctrl+A` is bound in several places — the hierarchy's "select all entities"
/// and the asset browser's "select all files" both listen for it. Without a
/// referee they'd fire together. So the panel under the pointer raises this flag
/// and the global entity select-all stands down for that frame, letting the
/// hovered panel handle the key. Absent/false → the entity select-all wins
/// (e.g. `Ctrl+A` over the viewport still selects every entity).
#[derive(Resource, Default)]
pub struct SelectAllClaimed(pub bool);

/// One split static mesh referenced by an assembly `.prefab`: a display name
/// and the project-relative path to its `.glb`. The mesh's world transform is
/// baked into the `.glb` itself, so the assembly entity sits at identity.
#[derive(Clone, Debug)]
pub struct AssemblyMeshEntry {
    pub name: String,
    pub model_path: String,
}

/// A request to write an assembly `.prefab` for a freshly split model.
///
/// The import worker runs off-thread and can't touch the `World`, but writing a
/// prefab in the engine's scene format needs the type registry and the existing
/// `save_prefab_source` serializer. So the worker hands the mesh list to the
/// main thread via [`PendingAssemblyWrites`], where an engine system fulfills it.
#[derive(Clone, Debug)]
pub struct AssemblyWriteRequest {
    /// Absolute path of the `.prefab` to write.
    pub prefab_path: std::path::PathBuf,
    /// The split meshes the assembly references, in source order.
    pub entries: Vec<AssemblyMeshEntry>,
}

/// Queue of assembly `.prefab` files to write, drained by an engine system.
/// See [`AssemblyWriteRequest`].
#[derive(Resource, Default)]
pub struct PendingAssemblyWrites(pub Vec<AssemblyWriteRequest>);

/// Marker resource requesting the tutorial overlay to start.
#[derive(Resource)]
pub struct TutorialRequested;

/// One-shot: request to toggle the settings overlay.
#[derive(Resource)]
pub struct ToggleSettingsRequested;

/// One-shot: request to open the Create Node overlay in the hierarchy panel.
#[derive(Resource)]
pub struct CreateNodeRequested;

/// One-shot: request the code editor to open a file.
///
/// Inserted by the asset browser (or any plugin) so the code editor plugin
/// can observe it without a direct crate dependency.
#[derive(Resource)]
pub struct OpenCodeEditorFile {
    pub path: std::path::PathBuf,
}

/// One-shot: request the command palette to toggle open/closed.
///
/// Inserted by the title-bar search button; consumed by `renzora_command_palette`.
#[derive(Resource)]
pub struct ToggleCommandPaletteRequested;

/// One-shot: request a viewport camera operation from the View menu.
///
/// Consumed by the camera controller in `renzora_camera`.
#[derive(Resource, Clone, Copy, Debug)]
pub enum CameraViewRequest {
    ZoomIn,
    ZoomOut,
    ResetZoom,
    FrameAll,
}

/// Toggle: when active, only the selected entity (and its ancestors/descendants)
/// remain visible in the viewport. Toggled from the View menu.
#[derive(Resource, Default)]
pub struct IsolationMode {
    pub active: bool,
}

/// Tracks whether a UI text input has keyboard focus.
///
/// When `true`, keyboard shortcuts should not fire so typing is not interrupted.
/// Updated each frame by the viewport/editor systems from the bevy_ui (ember)
/// focus state.
#[derive(Resource, Default)]
pub struct InputFocusState {
    /// True when a UI text field (or an editing drag-value) has keyboard focus,
    /// so global editor shortcuts hold off while the user is typing.
    pub ui_wants_keyboard: bool,
    /// True when the pointer is over a floating UI panel/overlay (not the viewport).
    pub pointer_over_ui: bool,
    /// True when a panel is consuming the Delete key (e.g. the animation
    /// timeline with a keyframe selected). The entity-delete shortcut skips
    /// while this is set so Delete removes the keyframe, not the entity.
    pub suppress_entity_delete: bool,
}

/// HUD data for the modal transform overlay (written by gizmo crate, read by viewport).
///
/// When `active` is true the viewport panel draws the scale circle / axis info.
#[derive(Resource, Default)]
pub struct ModalTransformHud {
    /// Whether modal transform is active.
    pub active: bool,
    /// Mode name ("Grab", "Rotate", "Scale").
    pub mode: &'static str,
    /// Whether this is Scale mode (draws circle + line overlay).
    pub is_scale: bool,
    /// Screen-space pivot position (entity center projected).
    pub pivot: Option<[f32; 2]>,
    /// Current cursor screen position.
    pub cursor: [f32; 2],
    /// Axis constraint name ("", "X", "Y", "Z", "YZ", "XZ", "XY").
    pub axis_name: &'static str,
    /// Axis constraint color [r, g, b, a] in 0..=255.
    pub axis_color: [u8; 4],
    /// Numeric input display string.
    pub numeric_display: String,
    /// Scale-mode reference circle radius in screen px (cursor's distance from
    /// the pivot when the gesture started — when the cursor is back on this
    /// circle, the scale factor is exactly 1).
    pub ref_radius: f32,
    /// Scale-mode live scale factor (current cursor distance / start distance),
    /// shown as the readout when the user hasn't typed an explicit value.
    pub scale_factor: f32,
}

/// Holds the optional render target for the game camera.
///
/// - `Some(handle)` — camera renders to this image (editor mode).
/// - `None` — camera renders to the window (standalone mode).
#[derive(Resource, Default)]
pub struct ViewportRenderTarget {
    pub image: Option<Handle<Image>>,
}

/// Open an existing project from project.toml path
pub fn open_project(
    project_toml_path: &Path,
) -> Result<CurrentProject, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(project_toml_path)?;
    let config: ProjectConfig = toml::from_str(&content)?;

    let path = project_toml_path
        .parent()
        .ok_or("Invalid project path")?
        .to_path_buf();

    Ok(CurrentProject { path, config })
}

// ── Auth bridge ──────────────────────────────────────────────────────────────

/// Lightweight auth info resource that the auth plugin keeps in sync.
/// The editor reads this to display sign-in state in the title bar without
/// depending on the full `renzora_auth` crate.
#[derive(Resource, Default, Clone)]
pub struct AuthBridge {
    /// Whether the auth sign-in window is currently open.
    pub window_open: bool,
    /// The signed-in username, if any.
    pub signed_in_username: Option<String>,
}

/// Marker resource inserted for one frame when sign-in succeeds.
/// The editor can consume this to react (e.g. switch to the Hub layout).
#[derive(Resource)]
pub struct AuthJustSignedIn;

/// Event-like resource: requests the auth window to toggle open/closed.
#[derive(Resource)]
pub struct AuthToggleWindowRequest;

/// Event-like resource: requests sign-out.
#[derive(Resource)]
pub struct AuthSignOutRequest;

// ============================================================================
// PropertyValue (shared between scripting and blueprints)
// ============================================================================

/// Value types for property writes and reflection-based get/set.
#[derive(Clone, Debug)]
pub enum PropertyValue {
    Float(f32),
    Int(i64),
    Bool(bool),
    String(String),
    Vec3([f32; 3]),
    Color([f32; 4]),
}

// ============================================================================
// ScriptInput (shared between scripting and blueprints)
// ============================================================================

/// Input state resource collected each frame for scripts and blueprints.
#[derive(Resource, Default, Clone)]
pub struct ScriptInput {
    pub keys_pressed: HashMap<KeyCode, bool>,
    pub keys_just_pressed: HashMap<KeyCode, bool>,
    pub keys_just_released: HashMap<KeyCode, bool>,
    pub mouse_pressed: HashMap<MouseButton, bool>,
    pub mouse_just_pressed: HashMap<MouseButton, bool>,
    pub mouse_position: Vec2,
    pub mouse_delta: Vec2,
    pub scroll_delta: Vec2,
    pub gamepad_axes: HashMap<u32, HashMap<GamepadAxis, f32>>,
    pub gamepad_buttons: HashMap<u32, HashMap<GamepadButton, bool>>,
    pub gamepad_buttons_just_pressed: HashMap<u32, HashMap<GamepadButton, bool>>,
    /// Slot ids of currently connected gamepads, sorted ascending. Slots are
    /// stable across the session: a pad keeps its id until it disconnects, and
    /// a newly connected pad takes the lowest free id — so unplugging pad 0
    /// doesn't shift pad 1 down.
    pub connected_gamepads: Vec<u32>,
}

impl ScriptInput {
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.keys_pressed.get(&key).copied().unwrap_or(false)
    }

    pub fn is_key_just_pressed(&self, key: KeyCode) -> bool {
        self.keys_just_pressed.get(&key).copied().unwrap_or(false)
    }

    pub fn get_movement_vector(&self) -> Vec2 {
        let mut x = 0.0f32;
        let mut y = 0.0f32;
        if self.is_key_pressed(KeyCode::KeyA) || self.is_key_pressed(KeyCode::ArrowLeft) {
            x -= 1.0;
        }
        if self.is_key_pressed(KeyCode::KeyD) || self.is_key_pressed(KeyCode::ArrowRight) {
            x += 1.0;
        }
        if self.is_key_pressed(KeyCode::KeyS) || self.is_key_pressed(KeyCode::ArrowDown) {
            y -= 1.0;
        }
        if self.is_key_pressed(KeyCode::KeyW) || self.is_key_pressed(KeyCode::ArrowUp) {
            y += 1.0;
        }
        let v = Vec2::new(x, y);
        if v.length_squared() > 0.0 {
            v.normalize()
        } else {
            v
        }
    }

    pub fn get_gamepad_left_stick(&self, id: u32) -> Vec2 {
        let axes = match self.gamepad_axes.get(&id) {
            Some(a) => a,
            None => return Vec2::ZERO,
        };
        Vec2::new(
            axes.get(&GamepadAxis::LeftStickX).copied().unwrap_or(0.0),
            axes.get(&GamepadAxis::LeftStickY).copied().unwrap_or(0.0),
        )
    }

    pub fn get_gamepad_right_stick(&self, id: u32) -> Vec2 {
        let axes = match self.gamepad_axes.get(&id) {
            Some(a) => a,
            None => return Vec2::ZERO,
        };
        Vec2::new(
            axes.get(&GamepadAxis::RightStickX).copied().unwrap_or(0.0),
            axes.get(&GamepadAxis::RightStickY).copied().unwrap_or(0.0),
        )
    }

    pub fn get_gamepad_trigger(&self, id: u32, left: bool) -> f32 {
        let axes = match self.gamepad_axes.get(&id) {
            Some(a) => a,
            None => return 0.0,
        };
        let axis = if left {
            GamepadAxis::LeftZ
        } else {
            GamepadAxis::RightZ
        };
        axes.get(&axis).copied().unwrap_or(0.0)
    }

    pub fn is_gamepad_button_pressed(&self, id: u32, button: GamepadButton) -> bool {
        self.gamepad_buttons
            .get(&id)
            .and_then(|b| b.get(&button))
            .copied()
            .unwrap_or(false)
    }

    pub fn is_gamepad_button_just_pressed(&self, id: u32, button: GamepadButton) -> bool {
        self.gamepad_buttons_just_pressed
            .get(&id)
            .and_then(|b| b.get(&button))
            .copied()
            .unwrap_or(false)
    }

    pub fn gamepad_count(&self) -> usize {
        self.connected_gamepads.len()
    }

    pub fn is_gamepad_connected(&self, id: u32) -> bool {
        self.connected_gamepads.contains(&id)
    }

    /// Lowest connected gamepad slot, if any. Used to back the legacy
    /// single-gamepad script globals.
    pub fn first_gamepad(&self) -> Option<u32> {
        self.connected_gamepads.first().copied()
    }
}

// ============================================================================
// Graph types (shared between blueprint and editor crates)
// ============================================================================

/// Node identifier in a visual scripting graph.
pub type NodeId = u64;

/// Pin data types for blueprint nodes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, Reflect)]
pub enum PinType {
    /// Execution flow (white wires) — controls order of operations.
    Exec,
    Float,
    Int,
    Bool,
    String,
    Vec2,
    Vec3,
    Color,
    Entity,
    /// Wildcard — accepts any data type.
    Any,
}

impl PinType {
    /// Can `from` connect to `to`?
    pub fn compatible(from: PinType, to: PinType) -> bool {
        if from == to {
            return true;
        }
        if to == PinType::Any && from != PinType::Exec {
            return true;
        }
        if from == PinType::Any && to != PinType::Exec {
            return true;
        }
        matches!(
            (from, to),
            (PinType::Int, PinType::Float)
                | (
                    PinType::Float,
                    PinType::Vec2 | PinType::Vec3 | PinType::Color
                )
                | (PinType::Vec3, PinType::Color)
                | (PinType::Color, PinType::Vec3)
                | (PinType::Bool, PinType::Int | PinType::Float)
        )
    }
}

/// Pin direction.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize, Reflect)]
pub enum PinDir {
    Input,
    Output,
}

/// Concrete values stored on pins (inline constants, defaults).
#[derive(Clone, Debug, Serialize, Deserialize, Reflect, PartialEq)]
#[derive(Default)]
pub enum PinValue {
    #[default]
    None,
    Float(f32),
    Int(i32),
    Bool(bool),
    String(String),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Color([f32; 4]),
    Entity(String),
}


impl PinValue {
    pub fn as_float(&self) -> f32 {
        match self {
            Self::Float(v) => *v,
            Self::Int(v) => *v as f32,
            Self::Bool(true) => 1.0,
            Self::Bool(false) => 0.0,
            _ => 0.0,
        }
    }

    pub fn as_int(&self) -> i32 {
        match self {
            Self::Int(v) => *v,
            Self::Float(v) => *v as i32,
            Self::Bool(true) => 1,
            Self::Bool(false) => 0,
            _ => 0,
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            Self::Bool(v) => *v,
            Self::Float(v) => *v != 0.0,
            Self::Int(v) => *v != 0,
            _ => false,
        }
    }

    pub fn as_string(&self) -> String {
        match self {
            Self::String(v) => v.clone(),
            Self::Float(v) => format!("{v}"),
            Self::Int(v) => format!("{v}"),
            Self::Bool(v) => format!("{v}"),
            Self::Entity(v) => v.clone(),
            _ => String::new(),
        }
    }

    pub fn as_vec3(&self) -> [f32; 3] {
        match self {
            Self::Vec3(v) => *v,
            Self::Color([r, g, b, _]) => [*r, *g, *b],
            Self::Float(v) => [*v, *v, *v],
            _ => [0.0, 0.0, 0.0],
        }
    }

    pub fn as_vec2(&self) -> [f32; 2] {
        match self {
            Self::Vec2(v) => *v,
            Self::Float(v) => [*v, *v],
            _ => [0.0, 0.0],
        }
    }

    pub fn as_color(&self) -> [f32; 4] {
        match self {
            Self::Color(v) => *v,
            Self::Vec3([r, g, b]) => [*r, *g, *b, 1.0],
            Self::Float(v) => [*v, *v, *v, 1.0],
            _ => [1.0, 1.0, 1.0, 1.0],
        }
    }

    pub fn pin_type(&self) -> PinType {
        match self {
            Self::None => PinType::Any,
            Self::Float(_) => PinType::Float,
            Self::Int(_) => PinType::Int,
            Self::Bool(_) => PinType::Bool,
            Self::String(_) => PinType::String,
            Self::Vec2(_) => PinType::Vec2,
            Self::Vec3(_) => PinType::Vec3,
            Self::Color(_) => PinType::Color,
            Self::Entity(_) => PinType::Entity,
        }
    }
}

/// Describes a pin on a node type (static definition).
#[derive(Clone, Debug)]
pub struct PinTemplate {
    pub name: String,
    pub label: String,
    pub pin_type: PinType,
    pub direction: PinDir,
    pub default_value: PinValue,
}

impl PinTemplate {
    pub fn exec_in(name: &str, label: &str) -> Self {
        Self {
            name: name.to_string(),
            label: label.to_string(),
            pin_type: PinType::Exec,
            direction: PinDir::Input,
            default_value: PinValue::None,
        }
    }

    pub fn exec_out(name: &str, label: &str) -> Self {
        Self {
            name: name.to_string(),
            label: label.to_string(),
            pin_type: PinType::Exec,
            direction: PinDir::Output,
            default_value: PinValue::None,
        }
    }

    pub fn input(name: &str, label: &str, pin_type: PinType) -> Self {
        Self {
            name: name.to_string(),
            label: label.to_string(),
            pin_type,
            direction: PinDir::Input,
            default_value: PinValue::None,
        }
    }

    pub fn output(name: &str, label: &str, pin_type: PinType) -> Self {
        Self {
            name: name.to_string(),
            label: label.to_string(),
            pin_type,
            direction: PinDir::Output,
            default_value: PinValue::None,
        }
    }

    pub fn with_default(mut self, value: PinValue) -> Self {
        self.default_value = value;
        self
    }
}

/// Static definition of a blueprint node type.
pub struct BlueprintNodeDef {
    pub node_type: &'static str,
    pub display_name: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    pub pins: fn() -> Vec<PinTemplate>,
    /// RGB header color for the node in the graph editor.
    pub color: [u8; 3],
}

/// A connection between two pins in a graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Reflect)]
pub struct BlueprintConnection {
    pub from_node: NodeId,
    pub from_pin: String,
    pub to_node: NodeId,
    pub to_pin: String,
}

/// A node instance in a graph.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Reflect)]
pub struct BlueprintNode {
    pub id: NodeId,
    pub node_type: String,
    pub position: [f32; 2],
    /// Override values for input pins (user-set constants).
    pub input_values: HashMap<String, PinValue>,
}

impl BlueprintNode {
    pub fn new(id: NodeId, node_type: &str, position: [f32; 2]) -> Self {
        Self {
            id,
            node_type: node_type.to_string(),
            position,
            input_values: HashMap::new(),
        }
    }

    pub fn get_input_value(&self, pin_name: &str) -> Option<&PinValue> {
        self.input_values.get(pin_name)
    }
}

/// A resizable comment / group box drawn behind the nodes of a graph. Purely
/// visual — no pins, never compiled — but dragging it moves the nodes it
/// encloses, so it doubles as a group. Shared by every graph model (blueprint /
/// material / particle) so the editor view can render, drag, resize and persist
/// them uniformly.
#[derive(Clone, Debug, Serialize, Deserialize, Reflect, PartialEq)]
#[reflect(Default)]
pub struct GraphComment {
    pub id: u64,
    /// `[x, y, w, h]` in canvas px.
    pub rect: [f32; 4],
    pub text: String,
    /// RGB tint of the title bar / border.
    pub color: [u8; 3],
}

impl Default for GraphComment {
    fn default() -> Self {
        Self {
            id: 0,
            rect: [0.0, 0.0, 220.0, 140.0],
            text: "Comment".to_string(),
            color: [88, 110, 150],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // ── ProjectConfig TOML round-trip ──────────────────────────────────────

    #[test]
    fn project_config_default_round_trips_through_toml() {
        // The defaults are what greets a freshly-created project; they have
        // to survive a save/load cycle byte-for-byte (modulo serializer
        // formatting), otherwise a save without edits would silently mutate
        // the project file.
        let original = ProjectConfig::default();
        let serialized = toml::to_string_pretty(&original).expect("serialize");
        let parsed: ProjectConfig = toml::from_str(&serialized).expect("parse");
        assert_eq!(original, parsed);
    }

    #[test]
    fn project_config_round_trips_with_editor_section() {
        let original = ProjectConfig {
            name: "Demo".into(),
            version: "0.2.1".into(),
            main_scene: "scenes/intro.ron".into(),
            editor_last_scene: Some("scenes/wip.ron".into()),
            icon: Some("assets/icon.png".into()),
            ui_font: None,
            autoload: vec!["scenes/loader.ron".into()],
            window: WindowConfig {
                width: 1920,
                height: 1080,
                resizable: false,
                mode: WindowMode::Fullscreen,
            },
            viewport: ViewportConfig::default(),
            rendering_2d: Rendering2dConfig::default(),
            rendering: RenderingConfig::default(),
            console_logging: false,
            network: None,
            editor: Some(crate::core::viewport_types::EditorPrefs::default()),
        };
        let s = toml::to_string_pretty(&original).expect("serialize");
        let parsed: ProjectConfig = toml::from_str(&s).expect("parse");
        assert_eq!(original, parsed);
    }

    #[test]
    fn project_config_skips_none_optional_fields_in_toml() {
        // `editor_last_scene`, `icon`, `network`, `editor` use
        // skip_serializing_if = "Option::is_none". A round-trip with all
        // None should produce TOML that has no mention of those keys —
        // catches a regression where the attribute disappears.
        let cfg = ProjectConfig::default();
        let serialized = toml::to_string_pretty(&cfg).expect("serialize");
        assert!(!serialized.contains("editor_last_scene"));
        assert!(!serialized.contains("icon"));
        assert!(!serialized.contains("[network]"));
        assert!(!serialized.contains("[editor]"));
    }

    #[test]
    fn project_config_parses_minimal_toml() {
        // Hand-rolled TOML that omits everything optional. Defaults must
        // fill in the gaps without erroring.
        let s = r#"
            name = "MyProject"
            version = "1.0.0"
            main_scene = "scenes/main.ron"
        "#;
        let parsed: ProjectConfig = toml::from_str(s).expect("parse minimal");
        assert_eq!(parsed.name, "MyProject");
        assert_eq!(parsed.version, "1.0.0");
        assert_eq!(parsed.main_scene, "scenes/main.ron");
        assert_eq!(parsed.editor_last_scene, None);
        assert_eq!(parsed.icon, None);
        assert_eq!(parsed.network, None);
        assert_eq!(parsed.editor, None);
        // window has its own #[serde(default)] so it should default cleanly.
        assert_eq!(parsed.window, WindowConfig::default());
    }

    // ── WindowConfig / NetworkProjectConfig defaults ──────────────────────

    #[test]
    fn window_config_default_is_720p_resizable() {
        let w = WindowConfig::default();
        assert_eq!(w.width, 1280);
        assert_eq!(w.height, 720);
        assert!(w.resizable);
        assert_eq!(w.mode, WindowMode::Windowed);
    }

    #[test]
    fn network_config_default_uses_loopback_udp() {
        let n = NetworkProjectConfig::default();
        assert_eq!(n.server_addr, "127.0.0.1");
        assert_eq!(n.port, 7636);
        assert_eq!(n.transport, "udp");
        assert_eq!(n.tick_rate, 64);
        assert_eq!(n.max_clients, 32);
    }

    #[test]
    fn network_config_round_trips() {
        let n = NetworkProjectConfig {
            server_addr: "10.0.0.5".into(),
            port: 9000,
            transport: "websocket".into(),
            tick_rate: 30,
            max_clients: 8,
        };
        let s = toml::to_string_pretty(&n).expect("serialize");
        let parsed: NetworkProjectConfig = toml::from_str(&s).expect("parse");
        assert_eq!(n, parsed);
    }

    // ── EntityTag default ─────────────────────────────────────────────────

    #[test]
    fn entity_tag_default_is_empty_string() {
        // The script lookup tables short-circuit on empty tags. If this
        // ever changed to e.g. "Untagged", every empty-tag entity would
        // suddenly start getting indexed.
        let tag = EntityTag::default();
        assert!(tag.tag.is_empty());
    }

    // ── AssetPathChanged::rewrite ─────────────────────────────────────────

    #[test]
    fn rewrite_file_rename_exact_match() {
        let evt = AssetPathChanged {
            old: "models/old.glb".into(),
            new: "models/new.glb".into(),
            is_dir: false,
        };
        assert_eq!(evt.rewrite("models/old.glb"), Some("models/new.glb".into()));
        assert_eq!(evt.rewrite("models/other.glb"), None);
    }

    #[test]
    fn rewrite_dir_rename_rewrites_descendants() {
        // `is_dir: true` rewrites anything under the folder, with a `/`
        // separator check so "modelsX" doesn't accidentally match "models".
        let evt = AssetPathChanged {
            old: "models".into(),
            new: "geometry".into(),
            is_dir: true,
        };
        assert_eq!(
            evt.rewrite("models/car.glb"),
            Some("geometry/car.glb".into())
        );
        assert_eq!(evt.rewrite("models"), Some("geometry".into()));
        // Different folder — must not rewrite.
        assert_eq!(evt.rewrite("modelsX/foo.glb"), None);
        // Unrelated path.
        assert_eq!(evt.rewrite("textures/a.png"), None);
    }

    #[test]
    fn rewrite_file_rename_does_not_match_dir_prefix() {
        // File rename requires exact match — must not rewrite something
        // that starts with the file's name as a string.
        let evt = AssetPathChanged {
            old: "models/old".into(),
            new: "models/new".into(),
            is_dir: false,
        };
        assert_eq!(evt.rewrite("models/old/inner.glb"), None);
    }

    // ── PbrAlphaMode default ──────────────────────────────────────────────

    #[test]
    fn pbr_alpha_mode_default_is_opaque() {
        assert_eq!(PbrAlphaMode::default(), PbrAlphaMode::Opaque);
    }

    // ── CurrentProject path helpers ───────────────────────────────────────

    fn make_project(root: &str) -> CurrentProject {
        CurrentProject {
            path: PathBuf::from(root),
            config: ProjectConfig::default(),
        }
    }

    #[test]
    fn resolve_path_joins_relative() {
        let proj = make_project("/projects/demo");
        let resolved = proj.resolve_path("scenes/main.ron");
        assert_eq!(
            resolved,
            PathBuf::from("/projects/demo").join("scenes/main.ron")
        );
    }

    #[test]
    fn resolve_path_keeps_absolute_input() {
        // Wait, looking at impl: `self.path.join(relative)` — Path::join
        // treats absolute paths as the new full path, so on Unix
        // /etc/passwd would replace the project root entirely. That's
        // already the documented behaviour ("if absolute, ignore root").
        let proj = make_project("/projects/demo");
        let abs = if cfg!(windows) { "C:/etc/x" } else { "/etc/x" };
        let resolved = proj.resolve_path(abs);
        assert_eq!(resolved, PathBuf::from(abs));
    }

    #[test]
    fn make_relative_handles_relative_input() {
        let proj = make_project(".");
        // A path that's already relative is returned with normalized
        // forward slashes regardless of input separator.
        let rel = std::path::Path::new("scenes/main.ron");
        assert_eq!(proj.make_relative(rel), Some("scenes/main.ron".into()));
    }
}
