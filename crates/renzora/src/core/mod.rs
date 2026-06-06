pub mod console_log;
pub mod keybindings;
pub mod reflection;
pub mod viewport_types;

use bevy::input::gamepad::{GamepadAxis, GamepadButton};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Generic file reader resource that abstracts filesystem vs. archive (rpak) reads.
///
/// By default reads from disk. The runtime replaces this with a Vfs-backed
/// reader so materials (and other systems) transparently read from rpak archives.
#[derive(Resource, Clone)]
pub struct VirtualFileReader {
    reader: Arc<dyn Fn(&str) -> Option<String> + Send + Sync>,
}

impl Default for VirtualFileReader {
    fn default() -> Self {
        Self {
            reader: Arc::new(|path| std::fs::read_to_string(path).ok()),
        }
    }
}

impl VirtualFileReader {
    /// Create a reader backed by a custom function.
    pub fn new(f: impl Fn(&str) -> Option<String> + Send + Sync + 'static) -> Self {
        Self {
            reader: Arc::new(f),
        }
    }

    /// Read a file to string. Tries the backing store (archive or disk).
    pub fn read_string(&self, path: &str) -> Option<String> {
        (self.reader)(path)
    }
}

/// Window display mode for exported games.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WindowMode {
    #[default]
    Windowed,
    Fullscreen,
    /// Borderless decorations, sized to the monitor. No exclusive mode.
    Borderless,
}

/// Window configuration for exported/runtime games
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WindowConfig {
    pub width: u32,
    pub height: u32,
    #[serde(default = "default_resizable")]
    pub resizable: bool,
    #[serde(default)]
    pub mode: WindowMode,
}

fn default_resizable() -> bool {
    true
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            resizable: true,
            mode: WindowMode::Windowed,
        }
    }
}

/// Graphics backend the editor and runtime request from wgpu at startup.
///
/// wgpu selects the backend when the render plugin initializes and cannot
/// switch it while the app runs, so this preference is persisted to disk and
/// read *before* the render plugin is built (see
/// `renzora_runtime::platform_wgpu_settings`). Changing it therefore only
/// takes effect after restarting the editor/runtime.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RendererBackend {
    /// Use the engine's standard backend for the current OS: Vulkan on
    /// Windows / Linux / BSD, Metal on Apple platforms. Recommended.
    #[default]
    Auto,
    /// Direct3D 12 — Windows only.
    Dx12,
    /// Vulkan — Windows, Linux, Android.
    Vulkan,
    /// Metal — macOS / iOS only.
    Metal,
    /// OpenGL — broad-compatibility fallback (no wireframe; fewer features).
    Gl,
}

impl RendererBackend {
    /// Human-readable label for settings UIs.
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Automatic",
            Self::Dx12 => "DirectX 12",
            Self::Vulkan => "Vulkan",
            Self::Metal => "Metal",
            Self::Gl => "OpenGL",
        }
    }

    /// Resolve `Auto` to the concrete backend the engine actually requests on
    /// this platform. Explicit choices pass through unchanged. Mirrors the
    /// per-OS default in `renzora_runtime::platform_wgpu_settings`; useful for
    /// displaying the active backend (e.g. in the status bar).
    pub fn resolved(self) -> RendererBackend {
        match self {
            Self::Auto => {
                #[cfg(any(target_os = "macos", target_os = "ios"))]
                {
                    Self::Metal
                }
                #[cfg(not(any(target_os = "macos", target_os = "ios")))]
                {
                    Self::Vulkan
                }
            }
            other => other,
        }
    }

    /// Backends worth offering on the current platform. `Auto` is always
    /// first; the rest are only backends wgpu can actually create here, so a
    /// settings UI never lets the user pick e.g. DX12 on Linux — which would
    /// leave wgpu unable to find an adapter and panic at startup.
    pub fn available() -> &'static [RendererBackend] {
        #[cfg(target_os = "windows")]
        {
            &[Self::Auto, Self::Dx12, Self::Vulkan]
        }
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        {
            &[Self::Auto, Self::Metal]
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios")))]
        {
            &[Self::Auto, Self::Vulkan, Self::Gl]
        }
    }
}

/// On-disk wrapper so the preference file stays forward-compatible
/// (`backend = "dx12"`, with room to grow).
#[derive(Serialize, Deserialize, Default)]
struct RendererPrefFile {
    backend: RendererBackend,
}

/// Path to the persisted renderer preference: `~/.renzora/renderer.toml`.
/// Mirrors the crash-report directory convention. Resolves the home dir via
/// env vars (`HOME`, falling back to Windows' `USERPROFILE`) so `renzora`
/// core keeps its dep list to bevy + serialization (no `dirs`).
#[cfg(not(target_arch = "wasm32"))]
fn renderer_pref_path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)?;
    Some(home.join(".renzora").join("renderer.toml"))
}

/// Load the persisted renderer backend preference, defaulting to
/// [`RendererBackend::Auto`] when the file is absent or unreadable.
pub fn load_renderer_backend() -> RendererBackend {
    #[cfg(target_arch = "wasm32")]
    {
        RendererBackend::Auto
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(path) = renderer_pref_path() else {
            return RendererBackend::Auto;
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return RendererBackend::Auto;
        };
        toml::from_str::<RendererPrefFile>(&text)
            .map(|f| f.backend)
            .unwrap_or_default()
    }
}

/// Persist the renderer backend preference. Takes effect on the next launch.
/// No-op error on wasm (no writable home dir).
#[cfg(not(target_arch = "wasm32"))]
pub fn save_renderer_backend(backend: RendererBackend) -> std::io::Result<()> {
    let Some(path) = renderer_pref_path() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "could not resolve home directory for renderer preference",
        ));
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(&RendererPrefFile { backend }).map_err(std::io::Error::other)?;
    std::fs::write(&path, text)
}

/// How the game's render viewport scales to fill the OS window.
///
/// Mirrors Godot's stretch modes — the *render resolution* (what the
/// camera shoots) and the *window size* (what the OS displays) are
/// independent concerns. Pixel-art games typically render at a small
/// fixed resolution (320×180, 480×270, etc.) and let the GPU upscale
/// to whatever window the player has, with nearest-neighbor sampling
/// preserving crisp pixels.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum StretchMode {
    /// Camera renders directly to the window. `viewport.width/height` is
    /// ignored; the visible world matches the OS window pixel-for-pixel.
    /// This is the default — same behaviour as before viewport mode existed.
    #[default]
    Disabled,
    /// Camera renders to an offscreen image at `viewport.width/height`,
    /// then the GPU upscales that image to fill the OS window with
    /// nearest-neighbour sampling. Letterbox/pillarbox depending on
    /// `aspect_mode` when the window aspect doesn't match the viewport.
    Viewport,
}

/// How the viewport image fills the OS window when their aspect ratios
/// differ. Only meaningful when [`StretchMode::Viewport`] is in use.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AspectMode {
    /// Preserve viewport aspect — black bars (letterbox / pillarbox) fill
    /// the gap. Pixel-perfect; what most retro games ship with.
    #[default]
    Keep,
    /// Stretch the viewport non-uniformly to fill the window. Distorts
    /// pixels — almost never what you want, but matches some legacy ports.
    Expand,
    /// Pin width to the window; viewport may letterbox top/bottom if
    /// the window is taller than the viewport's aspect.
    KeepWidth,
    /// Pin height to the window; viewport may pillarbox left/right if
    /// the window is wider than the viewport's aspect.
    KeepHeight,
}

/// Texture sampling filter — affects how loaded images look when
/// rendered at a different size than their native resolution.
///
/// `Nearest` preserves pixel-art crispness (each source pixel maps
/// to a discrete block of screen pixels with no smoothing).
/// `Linear` blends neighbouring pixels for smooth scaling, which
/// reads as blurry on pixel art but is right for HD textures.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextureFilter {
    /// Nearest-neighbour sampling — no blending, crisp pixel art.
    /// Good default for sprite-based / retro games.
    #[default]
    Nearest,
    /// Bilinear sampling — smooths between neighbouring pixels.
    /// Right for high-resolution art and smooth scaling.
    Linear,
}

/// 2D rendering config. Currently just the default image filter for
/// sprites; future fields (canvas blend modes, default tonemap, etc.)
/// land here.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[derive(Default)]
pub struct Rendering2dConfig {
    /// Sampler used when loading sprite textures. Defaults to
    /// `Nearest` so pixel-art assets render crisp out of the box.
    /// Per-sprite overrides can come later.
    #[serde(default)]
    pub image_filter: TextureFilter,
}


/// Game render-resolution config. Sits next to [`WindowConfig`]; the
/// window is the OS-managed surface, the viewport is the resolution the
/// camera shoots at.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ViewportConfig {
    /// Game render width in pixels. Only honoured when `stretch_mode`
    /// is `Viewport`. For pixel art, a low value (e.g. 320) gives
    /// chunky pixels when upscaled to a 1080p window.
    pub width: u32,
    /// Game render height in pixels.
    pub height: u32,
    /// How to scale the rendered image to fit the window.
    #[serde(default)]
    pub stretch_mode: StretchMode,
    /// How to handle aspect mismatch between viewport and window.
    #[serde(default)]
    pub aspect_mode: AspectMode,
}

impl Default for ViewportConfig {
    fn default() -> Self {
        // Defaults match `WindowConfig` so a fresh project with
        // `stretch_mode: Disabled` (the default) acts identically
        // to projects authored before this field existed.
        Self {
            width: 1280,
            height: 720,
            stretch_mode: StretchMode::default(),
            aspect_mode: AspectMode::default(),
        }
    }
}

/// Network configuration stored in `[network]` section of project.toml.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct NetworkProjectConfig {
    /// Server address (IP or hostname).
    #[serde(default = "default_server_addr")]
    pub server_addr: String,
    /// Port for the server to listen on / client to connect to.
    #[serde(default = "default_port")]
    pub port: u16,
    /// Transport protocol: "udp", "webtransport", "websocket".
    #[serde(default = "default_transport")]
    pub transport: String,
    /// Server tick rate in Hz.
    #[serde(default = "default_tick_rate")]
    pub tick_rate: u16,
    /// Maximum number of connected clients.
    #[serde(default = "default_max_clients")]
    pub max_clients: u16,
}

fn default_server_addr() -> String {
    "127.0.0.1".to_string()
}
fn default_port() -> u16 {
    7636
}
fn default_transport() -> String {
    "udp".to_string()
}
fn default_tick_rate() -> u16 {
    64
}
fn default_max_clients() -> u16 {
    32
}

impl Default for NetworkProjectConfig {
    fn default() -> Self {
        Self {
            server_addr: default_server_addr(),
            port: default_port(),
            transport: default_transport(),
            tick_rate: default_tick_rate(),
            max_clients: default_max_clients(),
        }
    }
}

/// Which rendering path the engine should use. See `crates/renzora_engine`
/// `RenderingModePlugin` for what each value does.
///
/// `Auto` picks per platform — desktop builds get `Deferred` (G-buffer +
/// SSR + proper albedo), mobile gets `Forward` (TBDR-friendly, lighter
/// memory bandwidth). Most projects should leave this on `Auto`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum RenderingMode {
    /// Detect from platform: Deferred on desktop, Forward on mobile / web.
    #[default]
    Auto,
    /// Forward + prepass. Lighting computed inline per mesh. Cheaper on
    /// memory bandwidth, mobile-GPU friendly, MSAA easy. No SSR.
    Forward,
    /// Deferred shading via Bevy's G-buffer. Decoupled lighting,
    /// many-lights efficient, unlocks SSR + free albedo prepass.
    /// Higher memory cost; transparency needs a separate forward pass.
    Deferred,
}

impl RenderingMode {
    /// Resolve `Auto` to a concrete mode based on the build target.
    /// Returns `self` unchanged for explicit `Forward` / `Deferred`.
    ///
    /// Currently `Auto` always resolves to `Forward` — the Deferred
    /// path works in Bevy 0.18 but enabling it surfaces breakage in
    /// custom material extensions (Lumen / SSGI normals corrupted,
    /// custom forward shaders without deferred output, etc.). Users
    /// opt into Deferred explicitly via `project.toml`:
    /// `[rendering] mode = "deferred"`.
    ///
    /// Once Phase 10b/10c land deferred-compatible versions of every
    /// material extension, this will flip to: Deferred on desktop,
    /// Forward on mobile / web (TBDR-friendly).
    pub fn resolve(self) -> Self {
        match self {
            Self::Auto => Self::Forward,
            other => other,
        }
    }
}

/// Renderer-level settings stored in project.toml.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct RenderingConfig {
    /// Forward vs. Deferred shading path. See [`RenderingMode`].
    #[serde(default)]
    pub mode: RenderingMode,
}

/// Resolved rendering mode for this run. Inserted as a resource at
/// engine init from the project config's [`RenderingConfig::mode`]
/// (with `Auto` resolved via [`RenderingMode::resolve`]). Plugins and
/// camera-spawn code read this to decide whether to attach
/// `DeferredPrepass`, route SSR, sample G-buffer for albedo, etc.
///
/// Never contains `Auto` — by the time it's inserted, the abstract
/// preference has been resolved to a concrete `Forward` or `Deferred`.
#[derive(bevy::prelude::Resource, Clone, Copy, Debug)]
pub struct ResolvedRenderingMode(pub RenderingMode);

impl Default for ResolvedRenderingMode {
    fn default() -> Self {
        Self(RenderingMode::default().resolve())
    }
}

impl ResolvedRenderingMode {
    pub fn is_deferred(&self) -> bool {
        matches!(self.0, RenderingMode::Deferred)
    }
    pub fn is_forward(&self) -> bool {
        matches!(self.0, RenderingMode::Forward)
    }
}

/// Project configuration stored in project.toml
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ProjectConfig {
    pub name: String,
    pub version: String,
    pub main_scene: String,
    /// The scene the editor had open when the project was last closed. Editor
    /// reopens this on project load, falling back to `main_scene` if absent.
    /// Runtime / exported builds always use `main_scene` (this field is
    /// editor-only and ignored by the runtime).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor_last_scene: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Scenes that load before `main_scene` and persist across every
    /// subsequent `load_scene()` call. Use for the loading overlay,
    /// global audio, save state — anything that needs to stay alive while
    /// the active scene swaps. Paths are project-relative (e.g.
    /// `"scenes/loader.ron"`). Empty by default; nothing happens if unset.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub autoload: Vec<String>,
    #[serde(default)]
    pub window: WindowConfig,
    /// Game render-resolution config. Independent of `window` — the
    /// camera renders at `viewport.width × viewport.height`, then the
    /// `stretch_mode` controls how that image fills the window. Default
    /// `Disabled` ignores the viewport resolution and renders straight
    /// to the window, so existing projects don't change behaviour.
    #[serde(default)]
    pub viewport: ViewportConfig,
    /// 2D rendering settings (sprite image filter, etc.).
    #[serde(default)]
    pub rendering_2d: Rendering2dConfig,
    /// 3D rendering pipeline settings (forward vs. deferred). See
    /// [`RenderingConfig`].
    #[serde(default)]
    pub rendering: RenderingConfig,
    /// Whether the runtime should attach a console (Windows) for `println!` /
    /// `log::*` output. No effect on Linux/macOS where stdout is always live.
    #[serde(default, skip_serializing_if = "is_false")]
    pub console_logging: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<NetworkProjectConfig>,
    /// Editor-only preferences (viewport toggles, camera speed, snap, etc.).
    /// The runtime ignores this section; export strips it from shipped builds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor: Option<crate::core::viewport_types::EditorPrefs>,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            name: "New Project".to_string(),
            version: "0.1.0".to_string(),
            main_scene: "scenes/main.ron".to_string(),
            editor_last_scene: None,
            icon: None,
            autoload: Vec::new(),
            window: WindowConfig::default(),
            viewport: ViewportConfig::default(),
            rendering_2d: Rendering2dConfig::default(),
            rendering: RenderingConfig::default(),
            console_logging: false,
            network: None,
            editor: None,
        }
    }
}

/// Runtime resource holding the currently open project
#[derive(Resource, Clone, Debug)]
pub struct CurrentProject {
    pub path: PathBuf,
    pub config: ProjectConfig,
}

impl CurrentProject {
    pub fn resolve_path(&self, relative: &str) -> PathBuf {
        self.path.join(relative)
    }

    pub fn main_scene_path(&self) -> PathBuf {
        self.resolve_path(&self.config.main_scene)
    }

    /// Save the project config back to project.toml.
    pub fn save_config(&self) -> Result<(), Box<dyn std::error::Error>> {
        let toml_path = self.path.join("project.toml");
        let content = toml::to_string_pretty(&self.config)?;
        std::fs::write(&toml_path, content)?;
        Ok(())
    }

    /// Convert an absolute path to a project-relative path (e.g. `assets/textures/foo.png`).
    pub fn make_relative(&self, path: &Path) -> Option<String> {
        if path.is_relative() {
            return Some(path.to_string_lossy().replace('\\', "/"));
        }

        let canonical_project = self.path.canonicalize().ok();
        let canonical_path = path.canonicalize().ok();

        if let (Some(proj), Some(p)) = (&canonical_project, &canonical_path) {
            if let Ok(rel) = p.strip_prefix(proj) {
                return Some(rel.to_string_lossy().replace('\\', "/"));
            }
        }

        if let Ok(rel) = path.strip_prefix(&self.path) {
            return Some(rel.to_string_lossy().replace('\\', "/"));
        }

        None
    }

    /// Convert an absolute path to an asset-relative path for `AssetServer::load()`.
    ///
    /// Strips the project root prefix so the resulting path
    /// (e.g. `textures/foo.png`) is portable across machines and works with the
    /// asset reader in both editor and standalone runtime builds.
    pub fn make_asset_relative(&self, path: &Path) -> String {
        // Try direct strip first
        if let Ok(rel) = path.strip_prefix(&self.path) {
            return rel.to_string_lossy().replace('\\', "/");
        }

        // Try canonicalized paths
        if let (Ok(canon_proj), Ok(canon_path)) = (self.path.canonicalize(), path.canonicalize()) {
            if let Ok(rel) = canon_path.strip_prefix(&canon_proj) {
                return rel.to_string_lossy().replace('\\', "/");
            }
        }

        // Fallback: return the path as-is with normalized slashes
        path.to_string_lossy().replace('\\', "/")
    }
}

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

/// Unique tag for identifying an entity from scripts and other systems.
///
/// Unlike `Name` (which is a display label and can be duplicated), a tag
/// is intended to be a unique identifier for lookup via `get_on()` etc.
#[derive(Component, Default, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct EntityTag {
    pub tag: String,
}

/// Marker component for the editor's scene-navigation camera.
///
/// This camera is used for orbit/pan/zoom during editing and renders to the
/// viewport texture. It is hidden from the hierarchy and cannot be deleted.
/// User-created scene cameras are separate entities.
#[derive(Component)]
pub struct EditorCamera;

/// Marker component for the editor's 2D scene-navigation camera.
///
/// Sibling of [`EditorCamera`]: orthographic, attached to the same viewport
/// render target, but only active when `ViewportSettings.viewport_view` is
/// [`ViewportView::Two`]. Pan with middle-mouse, zoom with scroll.
#[derive(Component)]
pub struct EditorCamera2d;

/// Identifies which of the multi-viewport slots a 3D editor camera belongs to.
///
/// There are [`viewport_types::VIEWPORT_COUNT`] of these cameras, one per
/// viewport panel (`viewport`, `viewport-2`, …). Each renders the same scene
/// from its own angle into its own render-target image. The *focused* slot's
/// camera additionally carries the [`EditorCamera`] marker so the existing
/// single-camera gizmo / picking / overlay systems all operate on whichever
/// viewport the user is interacting with — see `Viewports` in
/// [`viewport_types`].
#[derive(Component, Clone, Copy, Debug)]
pub struct ViewportCamera(pub usize);

/// Marker for viewport slot 0's camera specifically. Unlike [`EditorCamera`]
/// (which follows focus), this never moves off slot 0 — used as the stable
/// "default focus" view.
#[derive(Component, Clone, Copy, Debug)]
pub struct PrimaryViewportCamera;

/// Marker for the single hidden camera that bakes the procedural sky into a
/// cubemap + prefilters it for IBL. Every visible viewport (and preview camera)
/// shares that one bake's results — they carry only a `Skybox` + an
/// `EnvironmentMapLight` referencing the shared textures, never their own
/// `Atmosphere` pass. This is what makes all the views render an identical
/// environment from a single bake.
#[derive(Component, Clone, Copy, Debug)]
pub struct EnvironmentBakeCamera;

/// Marker component tagging an entity as a 2D scene node.
///
/// Currently semantically equivalent to a plain `Transform` parent, but
/// distinguished so the editor can: (a) auto-switch the viewport to 2D
/// view when one is selected, and (b) show a 2D-specific hierarchy icon
/// instead of the generic folder/circle.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct Node2d;

/// Asset-relative path of the image bound to a `Sprite`.
///
/// Mirror of `UiImagePath` for sprites. Bevy's `Sprite.image` holds a
/// `Handle<Image>`, which doesn't survive scene save/load — handle IDs
/// are runtime-only and don't remap. This component stores the path so
/// a rehydration system can re-load the image and assign the handle on
/// scene load (or whenever the path changes via the inspector / a
/// drag-drop).
#[derive(Component, Reflect, Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SpriteImagePath(pub String);

/// Marker component to hide an entity (and its children) from the hierarchy panel.
#[derive(Component)]
pub struct HideInHierarchy;

/// Marker component — entity persists across scene loads (e.g. loader UI root).
/// `process_pending_scene_loads` and similar despawn-the-world logic must skip these.
///
/// Auto-applied to every entity spawned from an autoload scene (see
/// `renzora_engine::autoload`). The component is also reflected so users can
/// hand-tag arbitrary entities from the inspector if they ever need to.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component)]
pub struct Persistent;

/// Marker component — entity is locked from editing in the hierarchy.
#[derive(Component)]
pub struct EditorLocked;

/// Marker component — viewport picking stops at this entity instead of walking
/// past it to a higher-up named ancestor. Apply to compound entities (terrains,
/// prefab roots, etc.) that own many named children but should be selectable
/// as a unit.
#[derive(Component, Default, Clone, Copy, Debug)]
pub struct SelectionStop;

/// Marker component — camera should be excluded from scene-wide effects (skybox, post-processing).
#[derive(Component)]
pub struct IsolatedCamera;

/// Marks an entity as the root of a nested-scene instance.
///
/// The `source` field is an asset-relative path to the `.ron` scene file that
/// provides the instance's contents. In the host scene file, only this root
/// entity (with its transform + any host-level overrides) is serialized; the
/// instance's child entity tree lives in the referenced source file and is
/// expanded on load.
///
/// Edits to entities *inside* an instance tree autosave back to the source
/// file. Edits to the instance root's transform persist in the host scene as
/// per-instance placement overrides.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SceneInstance {
    /// Asset-relative path to the source `.ron` scene file.
    pub source: String,
}

/// Serializable marker for a scene camera entity.
///
/// Stored alongside `Camera3d` so the camera can be recreated on scene load
/// (since `Camera3d` itself is not serializable).
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct SceneCamera;

/// Marks a camera as the default game camera for preview and play mode.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct DefaultCamera;

/// Live scene EV-100, written each frame by `renzora_auto_exposure`'s
/// GPU luminance readback system. `0.0` until the first readback completes
/// (or if auto-exposure isn't enabled). Read by scripting / debug HUDs.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct CameraExposureState {
    pub ev100: f32,
}

/// Maps each rendering camera to its effect source entities.
///
/// Each route is `(target_camera, [source_entities])`. For a given Settings
/// type the **first** source entity that has it wins.
///
/// Updated each frame by the routing system (editor: viewport crate,
/// runtime: renzora_engine). Read by per-crate sync systems.
#[derive(Resource, Default, Debug)]
pub struct EffectRouting {
    pub routes: Vec<(Entity, Vec<Entity>)>,
}

impl EffectRouting {
    /// Iterate all routes.
    pub fn iter(&self) -> impl Iterator<Item = &(Entity, Vec<Entity>)> {
        self.routes.iter()
    }
}

/// Serializable shape ID — stored alongside `Mesh3d` so the shape can be recreated on scene load.
///
/// The string must match a shape registered in the [`ShapeRegistry`].
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize)]
pub struct MeshPrimitive(pub String);

/// Event fired when a model importer has pulled PBR material data out of a
/// source file and needs somewhere to persist it as a `.material` graph.
/// Importers (the import dialog and the viewport drop pipeline) trigger this
/// per extracted material; an observer in `renzora_shader::material` writes
/// a node-graph `.material` file. Both sides communicate only through this
/// type — no sibling crate deps.
#[derive(Event, Debug, Clone)]
pub struct PbrMaterialExtracted {
    /// Human-friendly name for the material; becomes the `.material` filename.
    pub name: String,
    /// Absolute path of the directory to write the `.material` file into.
    pub output_dir: std::path::PathBuf,
    /// Absolute path of the project root. Subscribers compute the
    /// project-relative `wgsl_path` link saved into the `.material` from
    /// this; left empty when there's no project context.
    pub project_root: std::path::PathBuf,
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    /// glTF emissive factor (RGB linear). Multiplied with `emissive_texture`
    /// when present; used as a constant when not.
    pub emissive: [f32; 3],
    /// Asset-relative URIs to the corresponding textures, e.g.
    /// `"models/car/textures/body_albedo.png"`. `None` if absent.
    pub base_color_texture: Option<String>,
    pub normal_texture: Option<String>,
    /// glTF metallic-roughness map. Channels: G = roughness, B = metallic.
    pub metallic_roughness_texture: Option<String>,
    pub emissive_texture: Option<String>,
    /// Ambient occlusion map (R channel only).
    pub occlusion_texture: Option<String>,
    /// glTF spec-gloss `specularGlossinessTexture` (RGB = specular color,
    /// A = glossiness). The material observer routes its inverted alpha
    /// channel into the `roughness` pin so per-pixel glossiness survives
    /// the spec-gloss → metal-rough conversion. `None` for metal-rough
    /// materials.
    pub specular_glossiness_texture: Option<String>,
    /// glTF alpha behavior. The graph resolver maps this onto Bevy's
    /// `AlphaMode` so transparency renders correctly.
    pub alpha_mode: PbrAlphaMode,
    /// Alpha discard threshold for `Mask` mode. Ignored otherwise.
    pub alpha_cutoff: f32,
    /// `doubleSided` flag — render both faces (glass, foliage, fabric).
    pub double_sided: bool,
}

/// Mirrors glTF 2.0 `alphaMode`. Lives in core so the import event and the
/// material graph use a single shared enum without crate-cycle gymnastics.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[derive(Default)]
pub enum PbrAlphaMode {
    #[default]
    Opaque,
    Mask,
    Blend,
}


/// Event fired when a file or folder is renamed/moved inside the project's
/// asset tree. Subscribers should patch any stored asset-relative references
/// from `old` to `new` (and, when `old` is a folder, any paths prefixed by it).
/// Paths are asset-relative (no leading project root, forward slashes).
#[derive(Event, Debug, Clone)]
pub struct AssetPathChanged {
    pub old: String,
    pub new: String,
    /// `true` when the moved item was a directory — consumers should perform
    /// prefix matching on stored paths. `false` matches the exact path.
    pub is_dir: bool,
}

impl AssetPathChanged {
    /// If `path` references the moved asset (or something under it when
    /// `is_dir`), return the rewritten path. Otherwise `None`.
    pub fn rewrite(&self, path: &str) -> Option<String> {
        if self.is_dir {
            if let Some(rest) = path.strip_prefix(&self.old) {
                let sep = rest.starts_with('/') || rest.is_empty();
                if sep {
                    return Some(format!("{}{}", self.new, rest));
                }
            }
            None
        } else if path == self.old {
            Some(self.new.clone())
        } else {
            None
        }
    }
}

/// Serializable marker for an imported 3D model (GLTF/GLB).
///
/// Stored on the parent entity; the actual `SceneRoot` is a child.
/// On scene load, the runtime rehydrates by re-loading the model from `model_path`.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct MeshInstanceData {
    /// Asset-relative path to the GLB/GLTF file (e.g. `models/chair.glb`).
    pub model_path: Option<String>,
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

/// Sent by the editor to save the current scene before play mode.
#[derive(bevy::prelude::Event)]
pub struct SaveCurrentScene;

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

// ============================================================================
// Animation clip format (shared between animation and import)
// ============================================================================

/// One animation clip, serialized to a `.anim` file (RON format).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnimClip {
    pub name: String,
    pub duration: f32,
    pub tracks: Vec<BoneTrack>,
}

/// Animation curves for a single bone/target.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BoneTrack {
    pub bone_name: String,
    pub translations: Vec<(f32, [f32; 3])>,
    pub rotations: Vec<(f32, [f32; 4])>,
    pub scales: Vec<(f32, [f32; 3])>,
}

// ============================================================================
// TransformWrite (deferred transform mutations from scripts/blueprints)
// ============================================================================

/// Deferred transform write — batched and applied by the scripting command processor.
#[derive(Debug)]
pub struct TransformWrite {
    pub entity: bevy::ecs::entity::Entity,
    pub new_position: Option<bevy::prelude::Vec3>,
    pub new_rotation: Option<bevy::prelude::Vec3>,
    pub translation: Option<bevy::prelude::Vec3>,
    pub rotation_delta: Option<bevy::prelude::Vec3>,
    pub new_scale: Option<bevy::prelude::Vec3>,
    pub look_at: Option<bevy::prelude::Vec3>,
}

/// Queue for batched transform writes.
#[derive(bevy::prelude::Resource, Default)]
pub struct TransformWriteQueue {
    pub writes: Vec<TransformWrite>,
}

/// Write an AnimClip to a `.anim` file (RON format).
pub fn write_anim_file(clip: &AnimClip, path: &std::path::Path) -> Result<(), String> {
    let ron_str = ron::ser::to_string_pretty(clip, ron::ser::PrettyConfig::default())
        .map_err(|e| format!("RON serialization error: {}", e))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }
    std::fs::write(path, ron_str).map_err(|e| format!("Failed to write file: {}", e))?;
    Ok(())
}

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
    /// Scripts running inside the editor (editor UI visible, no camera switch).
    ScriptsOnly,
    /// Scripts paused inside the editor.
    ScriptsPaused,
}

/// Resource that tracks play mode state and pending transitions.
#[derive(Resource, Default)]
pub struct PlayModeState {
    pub state: PlayState,
    /// Entity of the active game camera during play mode.
    pub active_game_camera: Option<bevy::ecs::entity::Entity>,
    /// Set to `true` to request entering play mode next frame.
    pub request_play: bool,
    /// Set to `true` to request stopping play mode next frame.
    pub request_stop: bool,
    /// Set to `true` to toggle pause.
    pub request_pause: bool,
    /// Set to `true` to request scripts-only mode next frame.
    pub request_scripts_only: bool,
}

impl PlayModeState {
    pub fn is_playing(&self) -> bool {
        self.state == PlayState::Playing
    }
    pub fn is_paused(&self) -> bool {
        matches!(self.state, PlayState::Paused | PlayState::ScriptsPaused)
    }
    pub fn is_editing(&self) -> bool {
        self.state == PlayState::Editing
    }
    /// Returns true if in Playing or Paused state (full play mode).
    pub fn is_in_play_mode(&self) -> bool {
        matches!(self.state, PlayState::Playing | PlayState::Paused)
    }
    /// Returns true if scripts are in scripts-only mode (running or paused).
    pub fn is_scripts_only(&self) -> bool {
        matches!(
            self.state,
            PlayState::ScriptsOnly | PlayState::ScriptsPaused
        )
    }
    /// Returns true if scripts should be executing this frame.
    pub fn is_scripts_running(&self) -> bool {
        matches!(self.state, PlayState::Playing | PlayState::ScriptsOnly)
    }
}

/// Run condition: returns true when NOT in play mode (editing or scripts-only).
/// Use as `.run_if(not_in_play_mode)` on editor systems that should be disabled during play.
pub fn not_in_play_mode(play_mode: Option<Res<PlayModeState>>) -> bool {
    !play_mode.as_ref().is_some_and(|pm| pm.is_in_play_mode())
}

/// Which UI backend the editor renders with.
///
/// Transitional state for the egui → bevy_ui/HUI migration. `Egui` is the
/// legacy immediate-mode editor; `BevyUi` is the new `renzora_shell` bevy_ui
/// host. The editor is now bevy_ui-only (egui has been removed), so this always
/// resolves to `BevyUi`; the `Egui` variant is retained only so the historical
/// run-conditions still type-check and renders nothing.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum EditorUiBackend {
    Egui,
    #[default]
    BevyUi,
}

impl EditorUiBackend {
    pub fn is_egui(&self) -> bool {
        matches!(self, EditorUiBackend::Egui)
    }
    pub fn is_bevy_ui(&self) -> bool {
        matches!(self, EditorUiBackend::BevyUi)
    }
}

/// Run condition: true when the legacy egui editor should render. egui has been
/// removed, so this is effectively always false (kept so historical
/// `.run_if(editor_backend_is_egui)` call sites still compile).
pub fn editor_backend_is_egui(backend: Option<Res<EditorUiBackend>>) -> bool {
    backend.as_ref().map(|b| b.is_egui()).unwrap_or(false)
}

/// Run condition: true when the bevy_ui (ember) editor should render — the only
/// backend now. Absent resource defaults to `true`.
pub fn editor_backend_is_bevy_ui(backend: Option<Res<EditorUiBackend>>) -> bool {
    backend.as_ref().map(|b| b.is_bevy_ui()).unwrap_or(true)
}

/// Per-panel metadata for the bevy_ui editor shell, keyed by panel id.
///
/// Transitional bridge for the egui → bevy_ui migration: `renzora_editor`
/// populates this from its egui `PanelRegistry` (so the shell gets each panel's
/// real title/icon without linking egui). Once panels register a bevy-native
/// renderer directly, this becomes their primary registration.
#[derive(Resource, Default)]
pub struct ShellPanelRegistry {
    pub panels: bevy::platform::collections::HashMap<String, ShellPanelInfo>,
}

#[derive(Clone, Default)]
pub struct ShellPanelInfo {
    pub title: String,
    /// Phosphor glyph string for the panel's icon (empty if none). The Phosphor
    /// font shares codepoints with egui-phosphor, so the glyph renders directly.
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
/// Updated each frame by the viewport/editor systems from egui state.
#[derive(Resource, Default)]
pub struct InputFocusState {
    pub egui_wants_keyboard: bool,
    /// True when the pointer is over an egui panel (not the viewport).
    pub egui_has_pointer: bool,
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
#[derive(Clone, Debug, Serialize, Deserialize, Reflect)]
pub struct BlueprintConnection {
    pub from_node: NodeId,
    pub from_pin: String,
    pub to_node: NodeId,
    pub to_pin: String,
}

/// A node instance in a graph.
#[derive(Clone, Debug, Serialize, Deserialize, Reflect)]
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

#[cfg(test)]
mod tests {
    use super::*;

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
