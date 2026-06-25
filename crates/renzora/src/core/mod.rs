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

/// On-disk wrapper for per-user editor preferences (`~/.renzora/editor.toml`).
/// These are machine-local — UI scale depends on the user's monitor, not the
/// project — so they live next to the renderer preference rather than in
/// `project.toml`.
#[derive(Serialize, Deserialize)]
struct EditorPrefFile {
    #[serde(default = "default_ui_scale")]
    ui_scale: f32,
    #[serde(default = "default_system_monitor_ms")]
    stats_system_monitor_ms: u32,
    #[serde(default = "default_render_stats_ms")]
    stats_render_stats_ms: u32,
    #[serde(default = "default_ecs_stats_ms")]
    stats_ecs_stats_ms: u32,
    #[serde(default = "default_true")]
    status_show_fps: bool,
    #[serde(default = "default_true")]
    status_show_ram: bool,
    #[serde(default = "default_true")]
    status_show_gpu: bool,
    #[serde(default = "default_true")]
    status_show_rendering_mode: bool,
    #[serde(default = "default_true")]
    status_show_gpu_name: bool,
    /// Developer mode — unlocks dev/profiling tooling hidden from a normal
    /// editing session. Persisted here so a distribution plugin can read the
    /// host's dev-mode state via [`load_dev_mode`] at startup (the gated
    /// `renzora_tracy` profiler bridge does exactly this). Generic host flag.
    #[serde(default)]
    dev_mode: bool,
}

fn default_ui_scale() -> f32 {
    1.0
}
fn default_system_monitor_ms() -> u32 {
    200
}
fn default_render_stats_ms() -> u32 {
    100
}
fn default_ecs_stats_ms() -> u32 {
    250
}
fn default_true() -> bool {
    true
}

impl Default for EditorPrefFile {
    fn default() -> Self {
        Self {
            ui_scale: 1.0,
            stats_system_monitor_ms: default_system_monitor_ms(),
            stats_render_stats_ms: default_render_stats_ms(),
            stats_ecs_stats_ms: default_ecs_stats_ms(),
            status_show_fps: true,
            status_show_ram: true,
            status_show_gpu: true,
            status_show_rendering_mode: true,
            status_show_gpu_name: true,
            dev_mode: false,
        }
    }
}

/// Path to the persisted editor preferences: `~/.renzora/editor.toml`.
#[cfg(not(target_arch = "wasm32"))]
fn editor_pref_path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)?;
    Some(home.join(".renzora").join("editor.toml"))
}

/// Load the persisted editor UI scale multiplier (1.0 = system DPI),
/// defaulting to 1.0 when the file is absent or unreadable.
pub fn load_ui_scale() -> f32 {
    #[cfg(target_arch = "wasm32")]
    {
        1.0
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(path) = editor_pref_path() else {
            return 1.0;
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return 1.0;
        };
        toml::from_str::<EditorPrefFile>(&text)
            .map(|f| f.ui_scale)
            .unwrap_or(1.0)
            .clamp(0.5, 3.0)
    }
}

/// Persist the editor UI scale multiplier.
#[cfg(not(target_arch = "wasm32"))]
pub fn save_ui_scale(ui_scale: f32) -> std::io::Result<()> {
    let Some(path) = editor_pref_path() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "could not resolve home directory for editor preferences",
        ));
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Read-modify-write so future fields in the file survive a scale edit.
    let mut prefs = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| toml::from_str::<EditorPrefFile>(&t).ok())
        .unwrap_or_default();
    prefs.ui_scale = ui_scale;
    let text = toml::to_string_pretty(&prefs).map_err(std::io::Error::other)?;
    std::fs::write(&path, text)
}

/// Per-user refresh intervals (ms) for the editor's live stat readouts. Higher
/// numbers = fewer updates = cheaper. Edited from Settings → Plugins → "Stats
/// Refresh" and persisted in `~/.renzora/editor.toml`. The throttled stat
/// systems read this live (see [`stat_refresh_throttle`]).
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct StatsRefreshSettings {
    /// Status-bar FPS / RAM / GPU poll interval.
    pub system_monitor_ms: u32,
    /// Render Stats panel refresh interval.
    pub render_stats_ms: u32,
    /// ECS Stats panel refresh interval (its archetype scan is the heaviest).
    pub ecs_stats_ms: u32,
    /// Status-bar segment visibility (which readouts the status bar shows).
    pub show_fps: bool,
    pub show_ram: bool,
    pub show_gpu: bool,
    pub show_rendering_mode: bool,
    pub show_gpu_name: bool,
}

impl Default for StatsRefreshSettings {
    fn default() -> Self {
        Self {
            system_monitor_ms: default_system_monitor_ms(),
            render_stats_ms: default_render_stats_ms(),
            ecs_stats_ms: default_ecs_stats_ms(),
            show_fps: true,
            show_ram: true,
            show_gpu: true,
            show_rendering_mode: true,
            show_gpu_name: true,
        }
    }
}

/// Load the persisted stat-refresh intervals, clamped to sane bounds.
pub fn load_stats_refresh() -> StatsRefreshSettings {
    #[cfg(target_arch = "wasm32")]
    {
        StatsRefreshSettings::default()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let prefs = editor_pref_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| toml::from_str::<EditorPrefFile>(&t).ok())
            .unwrap_or_default();
        StatsRefreshSettings {
            system_monitor_ms: prefs.stats_system_monitor_ms.clamp(16, 10_000),
            render_stats_ms: prefs.stats_render_stats_ms.clamp(16, 10_000),
            ecs_stats_ms: prefs.stats_ecs_stats_ms.clamp(16, 10_000),
            show_fps: prefs.status_show_fps,
            show_ram: prefs.status_show_ram,
            show_gpu: prefs.status_show_gpu,
            show_rendering_mode: prefs.status_show_rendering_mode,
            show_gpu_name: prefs.status_show_gpu_name,
        }
    }
}

/// Persist the stat-refresh intervals (read-modify-write, so `ui_scale` and any
/// future fields in the file survive).
#[cfg(not(target_arch = "wasm32"))]
pub fn save_stats_refresh(settings: &StatsRefreshSettings) -> std::io::Result<()> {
    let Some(path) = editor_pref_path() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "could not resolve home directory for editor preferences",
        ));
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut prefs = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| toml::from_str::<EditorPrefFile>(&t).ok())
        .unwrap_or_default();
    prefs.stats_system_monitor_ms = settings.system_monitor_ms;
    prefs.stats_render_stats_ms = settings.render_stats_ms;
    prefs.stats_ecs_stats_ms = settings.ecs_stats_ms;
    prefs.status_show_fps = settings.show_fps;
    prefs.status_show_ram = settings.show_ram;
    prefs.status_show_gpu = settings.show_gpu;
    prefs.status_show_rendering_mode = settings.show_rendering_mode;
    prefs.status_show_gpu_name = settings.show_gpu_name;
    let text = toml::to_string_pretty(&prefs).map_err(std::io::Error::other)?;
    std::fs::write(&path, text)
}

/// Load the persisted developer-mode flag (default `false`). The editor seeds
/// `EditorSettings.dev_mode` from this at startup, and a distribution plugin can
/// read it directly (e.g. `renzora_tracy` gates its profiler bridge on it).
pub fn load_dev_mode() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        false
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        editor_pref_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| toml::from_str::<EditorPrefFile>(&t).ok())
            .map(|f| f.dev_mode)
            .unwrap_or(false)
    }
}

/// Persist the developer-mode flag (read-modify-write, so other fields survive).
#[cfg(not(target_arch = "wasm32"))]
pub fn save_dev_mode(dev_mode: bool) -> std::io::Result<()> {
    let Some(path) = editor_pref_path() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "could not resolve home directory for editor preferences",
        ));
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut prefs = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| toml::from_str::<EditorPrefFile>(&t).ok())
        .unwrap_or_default();
    prefs.dev_mode = dev_mode;
    let text = toml::to_string_pretty(&prefs).map_err(std::io::Error::other)?;
    std::fs::write(&path, text)
}

/// Build a run condition that fires at most once per the interval returned by
/// `interval_ms`, read **live** from [`StatsRefreshSettings`] so a settings edit
/// takes effect immediately. Falls back to 250 ms when the resource is absent;
/// an interval of 0 means "every frame". Each `.run_if(stat_refresh_throttle(…))`
/// gets its own accumulator.
pub fn stat_refresh_throttle(
    interval_ms: fn(&StatsRefreshSettings) -> u32,
) -> impl FnMut(Res<Time>, Option<Res<StatsRefreshSettings>>) -> bool + Clone {
    let mut acc_ms = 0.0f32;
    move |time: Res<Time>, settings: Option<Res<StatsRefreshSettings>>| {
        let interval = settings.as_deref().map(interval_ms).unwrap_or(250);
        if interval == 0 {
            return true;
        }
        acc_ms += time.delta_secs() * 1000.0;
        if acc_ms >= interval as f32 {
            // Carry the remainder (capped) so we don't drift slow on long frames.
            acc_ms = (acc_ms - interval as f32).min(interval as f32);
            true
        } else {
            false
        }
    }
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
    /// Default UI font for the shipped game — a name resolved by the font
    /// registry, a project `fonts/` path (e.g. `"fonts/Inter.ttf"`), or a system
    /// family. Applied at runtime startup (the game's ember UI uses it); `None`
    /// keeps the embedded default. Shipped (not the editor-stripped section).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ui_font: Option<String>,
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
            main_scene: "scenes/main.bsn".to_string(),
            editor_last_scene: None,
            icon: None,
            autoload: Vec::new(),
            window: WindowConfig::default(),
            viewport: ViewportConfig::default(),
            rendering_2d: Rendering2dConfig::default(),
            rendering: RenderingConfig::default(),
            console_logging: false,
            ui_font: None,
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

/// A reflection probe's authored environment source — the *persistent* side of a
/// parallax-corrected cubemap probe.
///
/// Bevy's `GeneratedEnvironmentMapLight` is the runtime GPU side: its filter
/// **runs the moment that component exists** and demands a **power-of-two cube**
/// texture, so attaching it with an unset (1×1 default) or equirectangular
/// handle spams GPU validation errors. To avoid that, a probe carries *this*
/// component instead, and `renzora_environment_map` only inserts
/// `GeneratedEnvironmentMapLight` **once a valid cube is ready** — loading the
/// `path`, reprojecting an equirect `.exr`/`.hdr` into a POT cube (or using a
/// `.ktx2`/`.dds` cube directly), and applying `intensity`. Only this component
/// persists in the scene; the cube is regenerated on load.
#[derive(Component, Reflect, Clone, Debug, serde::Serialize, serde::Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct ReflectionProbeSource {
    /// Project-relative path to the source image (equirect HDR or cube container).
    pub path: String,
    /// Strength multiplier applied to the probe's reflections (cd/m²).
    pub intensity: f32,
}

impl Default for ReflectionProbeSource {
    fn default() -> Self {
        Self { path: String::new(), intensity: 1.0 }
    }
}

/// Marker component to hide an entity (and its children) from the hierarchy panel.
#[derive(Component)]
pub struct HideInHierarchy;

/// Canonical render-pass ordering phases for the Bevy 0.19 `Core3d` schedule —
/// the centralized "render composition" pipeline (see `docs/render-composition.md`
/// and `renzora::postprocess`). Bevy deleted the render graph in 0.19 and moved
/// to system ordering; this enum is the single shared vocabulary so renzora's
/// many view-target passes (GI, reflections, post-process, …) slot into a known
/// order instead of each hardcoding `.before(some_other_system)`.
///
/// Phases are interleaved with bevy's own post-process systems, which act as
/// fixed anchors (the render-composition framework places these phases around
/// them in ONE place):
///
/// ```text
/// MainPass ─ Gi ─ [bevy TAA] ─ HdrPost ─ [bevy tonemapping] ─ LdrPost ─ [fxaa/smaa] ─ Overlay
/// ```
///
/// A render pass joins a phase with `.in_set(renzora::RenderPhase::Gi)` (for a
/// system pass) or by registering a handler in that phase (data-driven, for the
/// future node-graph pipeline editor) — and never references another pass.
#[derive(
    bevy::ecs::schedule::SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub enum RenderPhase {
    /// HDR/linear, after the main 3D pass and BEFORE temporal AA: global
    /// illumination composite, screen-space reflections. Running before TAA is
    /// what puts GI in the temporal history (otherwise: SSGI flicker / SDF grey).
    Gi,
    /// HDR, after temporal AA: bloom, depth-of-field, motion blur.
    HdrPost,
    /// LDR, after tonemapping: color grading, vignette, and the rest of the
    /// unified post-process effects.
    LdrPost,
    /// Final overlays (debug visualizations, gizmo composites) — after AA.
    Overlay,
}

/// Editor viewport gate: this scene entity was force-hidden because no
/// viewport panel is visible, and the stored value is its *authored*
/// `Visibility` (the slot-0 editor camera must stay active for the
/// atmosphere/IBL probe, so hiding the scene is how a viewport-less workspace
/// stops paying for shadow maps, GI and mesh extraction — see
/// `renzora_viewport::gate_scene_visibility`).
///
/// Deliberately NOT `Reflect`: it must never serialize into a scene file.
/// Scene saves restore the stored value before extracting so the authored
/// visibility is what lands on disk (see `renzora_engine::scene_io`); the
/// hierarchy panel's eye icon reads it for the same reason.
#[derive(Component, Clone, Copy)]
pub struct ViewportGateHidden(pub Visibility);

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

/// Per-camera render-resolution scale (Full / Half / Quarter).
///
/// Sizes this camera's render target at a fraction of the display size and
/// upscales it. In the editor, the viewport reflects the resolution of the
/// relevant scene camera (selected → default → first); in play mode the active
/// camera's resolution drives the game render target. Absent ⇒ Full.
#[derive(Component, Clone, Copy, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct CameraRenderResolution(pub viewport_types::RenderResolution);

/// One named camera angle — a captured world-space pose.
///
/// Stored in a [`CameraPresets`] list on a camera entity so the angle persists
/// in the scene RON and can be jumped to from scripting (`goto_camera_preset`)
/// or the inspector's "Camera Presets" section.
#[derive(Clone, Debug, Reflect, Serialize, Deserialize, PartialEq)]
pub struct CameraPreset {
    /// Lookup key used by scripting (`goto_camera_preset("name")`) and shown in
    /// the inspector list. Not required to be unique, but `goto` matches the
    /// first by name.
    pub name: String,
    /// World-space translation of the camera at capture time.
    pub translation: Vec3,
    /// World-space orientation of the camera at capture time.
    pub rotation: Quat,
}

impl CameraPreset {
    /// Build a preset from a name and a world-space transform.
    pub fn from_transform(name: impl Into<String>, transform: &Transform) -> Self {
        Self {
            name: name.into(),
            translation: transform.translation,
            rotation: transform.rotation,
        }
    }

    /// The pose as a [`Transform`] (scale left at one — camera scale is ignored).
    pub fn to_transform(&self) -> Transform {
        Transform {
            translation: self.translation,
            rotation: self.rotation,
            scale: Vec3::ONE,
        }
    }
}

/// A list of named camera angles attached to a camera entity.
///
/// Authored in the inspector ("Camera Presets" section → *Capture current
/// view*) and serialized into the scene. A script on the same entity can jump
/// the camera to any preset by name with `goto_camera_preset("name")`, or query
/// the list with `camera_preset_count()` / `camera_preset_name(i)`.
#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct CameraPresets {
    pub presets: Vec<CameraPreset>,
}

impl CameraPresets {
    /// Find a preset by name (first match).
    pub fn get(&self, name: &str) -> Option<&CameraPreset> {
        self.presets.iter().find(|p| p.name == name)
    }
}

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
    /// Standalone roughness map (`r` → roughness) for sources that don't pack
    /// metallic-roughness into one image (OBJ `map_Pr`, USD).
    pub roughness_texture: Option<String>,
    /// Standalone metallic map (`r` → metallic).
    pub metallic_texture: Option<String>,
    pub emissive_texture: Option<String>,
    /// Ambient occlusion map (R channel only).
    pub occlusion_texture: Option<String>,
    /// glTF spec-gloss `specularGlossinessTexture` (RGB = specular color,
    /// A = glossiness). The material observer routes its inverted alpha
    /// channel into the `roughness` pin so per-pixel glossiness survives
    /// the spec-gloss → metal-rough conversion. `None` for metal-rough
    /// materials.
    pub specular_glossiness_texture: Option<String>,
    /// Standalone opacity/alpha map with no glTF metal-rough equivalent
    /// (legacy FBX `TransparentColor` / `TransparencyFactor`). The material
    /// observer samples its `r` channel into the `alpha` pin so cloud shells
    /// and cutouts that drive transparency through a dedicated grayscale mask
    /// punch through.
    pub opacity_texture: Option<String>,
    /// Standalone specular/reflectivity mask (legacy FBX `SpecularColor` /
    /// `ReflectionColor`). Routed into `metallic` (and its inverse into
    /// `roughness`) to approximate a pre-PBR specular map.
    pub specular_texture: Option<String>,
    /// Extended PBR channels (clearcoat, transmission, anisotropy, ior, …)
    /// from glTF `KHR_materials_*` / modern FBX / USD. Default for sources that
    /// only author base metallic-roughness.
    pub advanced: PbrAdvanced,
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

/// Extended/advanced PBR channels beyond the base metallic-roughness model —
/// the union of what Bevy's `StandardMaterial` can shade and what glTF
/// `KHR_materials_*` extensions (and modern FBX/USD) author. Importers fill in
/// whatever the source provides; the graph builder seeds the matching
/// `output/surface` pins and samples any textures into them.
///
/// Defaults mirror the glTF 2.0 spec so a material that omits a channel renders
/// identically to one that never had it (e.g. `ior = 1.5`, no clearcoat).
#[derive(Clone, Debug)]
pub struct PbrAdvanced {
    /// `KHR_materials_clearcoat` clearcoatFactor — strength of the lacquer layer.
    pub clearcoat: f32,
    pub clearcoat_roughness: f32,
    pub clearcoat_texture: Option<String>,
    pub clearcoat_roughness_texture: Option<String>,
    pub clearcoat_normal_texture: Option<String>,
    /// `KHR_materials_transmission` transmissionFactor → `specular_transmission`.
    pub specular_transmission: f32,
    pub transmission_texture: Option<String>,
    /// Bevy diffuse transmission (translucent thin surfaces — leaves, paper).
    pub diffuse_transmission: f32,
    /// `KHR_materials_volume` — thickness of the refractive volume + textures.
    pub thickness: f32,
    pub thickness_texture: Option<String>,
    /// `KHR_materials_ior` — index of refraction (glass ≈ 1.5, water ≈ 1.33).
    pub ior: f32,
    /// `KHR_materials_volume` attenuation: how far light travels before being
    /// tinted by `attenuation_color`.
    pub attenuation_distance: f32,
    pub attenuation_color: [f32; 3],
    /// `KHR_materials_anisotropy` — brushed-metal directional highlight.
    pub anisotropy_strength: f32,
    pub anisotropy_rotation: f32,
    pub anisotropy_texture: Option<String>,
    /// `KHR_materials_specular` specularFactor → dielectric `reflectance`.
    pub reflectance: f32,
    /// `KHR_materials_unlit` — bypass lighting entirely (emissive-style flat
    /// shading). The graph builder switches to the unlit output when set.
    pub unlit: bool,
}

impl Default for PbrAdvanced {
    fn default() -> Self {
        Self {
            clearcoat: 0.0,
            clearcoat_roughness: 0.0,
            clearcoat_texture: None,
            clearcoat_roughness_texture: None,
            clearcoat_normal_texture: None,
            specular_transmission: 0.0,
            transmission_texture: None,
            diffuse_transmission: 0.0,
            thickness: 0.0,
            thickness_texture: None,
            ior: 1.5,
            // Large finite sentinel rather than f32::INFINITY: the graph
            // serializes to JSON, which has no infinity literal and would
            // emit `null` — corrupting the value on reload. 1e37 is
            // effectively "no attenuation" for any real scene.
            attenuation_distance: 1.0e37,
            attenuation_color: [1.0, 1.0, 1.0],
            anisotropy_strength: 0.0,
            anisotropy_rotation: 0.0,
            anisotropy_texture: None,
            reflectance: 0.5,
            unlit: false,
        }
    }
}

impl PbrAdvanced {
    /// Returns `true` when no extended channel deviates from its default, so
    /// callers can skip emitting advanced nodes for plain metal-rough materials.
    pub fn is_default(&self) -> bool {
        self.clearcoat == 0.0
            && self.clearcoat_texture.is_none()
            && self.clearcoat_roughness_texture.is_none()
            && self.clearcoat_normal_texture.is_none()
            && self.specular_transmission == 0.0
            && self.transmission_texture.is_none()
            && self.diffuse_transmission == 0.0
            && self.thickness == 0.0
            && self.thickness_texture.is_none()
            && self.ior == 1.5
            && self.anisotropy_strength == 0.0
            && self.anisotropy_texture.is_none()
            && self.reflectance == 0.5
            && !self.unlit
    }

    /// Produce a copy with every texture path mapped through `f` — used by the
    /// import bridges to rewrite model-relative URIs to project-relative ones.
    pub fn rewrite_textures(&self, f: impl Fn(&Option<String>) -> Option<String>) -> Self {
        Self {
            clearcoat_texture: f(&self.clearcoat_texture),
            clearcoat_roughness_texture: f(&self.clearcoat_roughness_texture),
            clearcoat_normal_texture: f(&self.clearcoat_normal_texture),
            transmission_texture: f(&self.transmission_texture),
            thickness_texture: f(&self.thickness_texture),
            anisotropy_texture: f(&self.anisotropy_texture),
            attenuation_color: self.attenuation_color,
            ..self.clone()
        }
    }
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
    /// Property-animation tracks: keyframes bound to arbitrary component fields
    /// (Transform translation/rotation/scale, or any reflected field). Distinct
    /// from skeletal `tracks` (bone curves) — these are sampled by a custom
    /// sampler, not Bevy's `AnimationPlayer`. `#[serde(default)]` keeps legacy
    /// `.anim` files (which have no `property_tracks` field) loadable.
    #[serde(default)]
    pub property_tracks: Vec<PropertyTrack>,
    /// Named event markers — when playback crosses one, scripts' `on_animation_event`
    /// hook fires with the marker name.
    #[serde(default)]
    pub markers: Vec<AnimMarker>,
}

/// A named event marker on an animation clip's timeline.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnimMarker {
    pub time: f32,
    pub name: String,
}

/// Animation curves for a single bone/target.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BoneTrack {
    pub bone_name: String,
    pub translations: Vec<(f32, [f32; 3])>,
    pub rotations: Vec<(f32, [f32; 4])>,
    pub scales: Vec<(f32, [f32; 3])>,
}

// ----------------------------------------------------------------------------
// Property animation (keyframes bound to component fields)
// ----------------------------------------------------------------------------

/// Editor/runtime cache of the **Euler angles** (degrees, XYZ order) last dialed
/// into a rotation — by the inspector or a rotation animation. **Keyed per
/// component** so several rotation fields on one entity (e.g. `Transform` *and*
/// `EnvironmentMapLight`) each keep their own slot instead of fighting over one.
///
/// A quaternion stores only an orientation, so converting it back to Euler angles
/// is lossy: the middle axis wraps at ±90° and full turns (360°, 720°) collapse
/// onto the same value. This keeps the *typed* angles intact so the inspector
/// shows what you entered and a 0→360 rotation key pair animates a real spin.
/// Each slot's `quat` is a staleness fingerprint: when the live rotation no
/// longer matches it, something else moved the entity and the angles are
/// re-derived from the quaternion (the lossy fallback).
///
/// Mirrors Godot's `Node3D::euler_rotation` + dirty-flag design. Not reflected,
/// so it's transient editor state and never serialized into a scene.
#[derive(Component, Clone, Debug, Default)]
pub struct EditorEulerCache {
    slots: HashMap<String, EulerSlot>,
}

#[derive(Clone, Copy, Debug)]
struct EulerSlot {
    deg: Vec3,
    quat: Quat,
}

impl EditorEulerCache {
    /// Cached degrees under `key` if they still describe `rotation` (`q` and `-q`
    /// are the same orientation, so both signs count as a match).
    pub fn degrees_for(&self, key: &str, rotation: Quat) -> Option<Vec3> {
        self.slots.get(key).and_then(|s| {
            (s.quat.abs_diff_eq(rotation, 1e-4) || s.quat.abs_diff_eq(-rotation, 1e-4))
                .then_some(s.deg)
        })
    }

    /// Store typed Euler `deg` under `key`; returns the quaternion they produce.
    pub fn store(&mut self, key: &str, deg: Vec3) -> Quat {
        let quat = euler_deg_to_quat(deg);
        self.slots.insert(key.to_string(), EulerSlot { deg, quat });
        quat
    }
}

/// Quaternion from Euler **degrees** (XYZ order).
pub fn euler_deg_to_quat(deg: Vec3) -> Quat {
    Quat::from_euler(
        EulerRot::XYZ,
        deg.x.to_radians(),
        deg.y.to_radians(),
        deg.z.to_radians(),
    )
}

/// The Euler degrees (XYZ) to display/record for `rotation` under `key`,
/// preferring the cache when it still matches, else deriving from the quaternion
/// (which wraps the middle axis to ±90° — the unavoidable lossy fallback).
pub fn rotation_euler_deg(rotation: Quat, cache: Option<&EditorEulerCache>, key: &str) -> Vec3 {
    if let Some(deg) = cache.and_then(|c| c.degrees_for(key, rotation)) {
        return deg;
    }
    let (x, y, z) = rotation.to_euler(EulerRot::XYZ);
    Vec3::new(x.to_degrees(), y.to_degrees(), z.to_degrees())
}

/// Store `deg` under `key` in the entity's [`EditorEulerCache`] (inserting the
/// component if absent) and return the quaternion to apply to the rotation.
pub fn cache_euler_deg(world: &mut World, entity: Entity, key: &str, deg: Vec3) -> Quat {
    if world.get::<EditorEulerCache>(entity).is_none() {
        world.entity_mut(entity).insert(EditorEulerCache::default());
    }
    world
        .get_mut::<EditorEulerCache>(entity)
        .map(|mut c| c.store(key, deg))
        .unwrap_or_else(|| euler_deg_to_quat(deg))
}

/// A keyframe track bound to one field of one component on a target entity.
///
/// The track is resolved relative to the entity that owns the `AnimatorComponent`:
/// `target` is `""`/`"self"` for that entity, otherwise the `Name` of a descendant.
/// `component` is the reflected short type-name (case-insensitive, e.g. `"transform"`,
/// `"directional_light"`) and `field` is a dotted reflection path (e.g. `"translation"`,
/// `"rotation"`, `"scale"`, `"illuminance"`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PropertyTrack {
    #[serde(default)]
    pub target: String,
    pub component: String,
    pub field: String,
    #[serde(default)]
    pub keys: Vec<PropertyKey>,
}

/// One keyframe: a value at a time, plus how to interpolate toward the next key.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PropertyKey {
    pub time: f32,
    pub value: TrackValue,
    #[serde(default)]
    pub interp: Interp,
}

/// The animatable value kinds a property keyframe can hold. Mirrors the subset of
/// component-field types that can be sampled/interpolated. `Transform::rotation`
/// now records `Vec3` **Euler degrees** (component-lerped, so a 0→360 key pair
/// animates a full spin); `Quat` is retained for backward-compatibility with
/// older clips (slerp — shortest path, can't express a spin).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TrackValue {
    Float(f32),
    Vec3([f32; 3]),
    Quat([f32; 4]),
    Color([f32; 4]),
    Bool(bool),
}

/// How a keyframe interpolates toward the next keyframe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Interp {
    /// Linear blend (component lerp; `slerp` for quaternions).
    #[default]
    Linear,
    /// Hold this key's value until the next key (constant / step).
    Stepped,
}

impl TrackValue {
    /// Convert to a [`PropertyValue`] for the reflection write path. `Quat` has no
    /// `PropertyValue` equivalent (it's handled only by the Transform fast-path),
    /// so it returns `None`.
    pub fn to_property_value(&self) -> Option<PropertyValue> {
        match self {
            TrackValue::Float(v) => Some(PropertyValue::Float(*v)),
            TrackValue::Vec3(v) => Some(PropertyValue::Vec3(*v)),
            TrackValue::Color(v) => Some(PropertyValue::Color(*v)),
            TrackValue::Bool(v) => Some(PropertyValue::Bool(*v)),
            TrackValue::Quat(_) => None,
        }
    }

    /// Build a `TrackValue` from a reflected [`PropertyValue`]. `Int` widens to
    /// `Float`; `String` has no animatable representation.
    pub fn from_property_value(pv: &PropertyValue) -> Option<TrackValue> {
        match pv {
            PropertyValue::Float(v) => Some(TrackValue::Float(*v)),
            PropertyValue::Int(v) => Some(TrackValue::Float(*v as f32)),
            PropertyValue::Bool(v) => Some(TrackValue::Bool(*v)),
            PropertyValue::Vec3(v) => Some(TrackValue::Vec3(*v)),
            PropertyValue::Color(v) => Some(TrackValue::Color(*v)),
            PropertyValue::String(_) => None,
        }
    }

    /// Linear blend between two values of the same kind (`t` in `0..=1`). Returns
    /// `a` if the kinds differ. Quaternions use `slerp`; bools snap at the midpoint.
    pub fn lerp(a: &TrackValue, b: &TrackValue, t: f32) -> TrackValue {
        match (a, b) {
            (TrackValue::Float(x), TrackValue::Float(y)) => TrackValue::Float(x + (y - x) * t),
            (TrackValue::Vec3(x), TrackValue::Vec3(y)) => TrackValue::Vec3([
                x[0] + (y[0] - x[0]) * t,
                x[1] + (y[1] - x[1]) * t,
                x[2] + (y[2] - x[2]) * t,
            ]),
            (TrackValue::Color(x), TrackValue::Color(y)) => TrackValue::Color([
                x[0] + (y[0] - x[0]) * t,
                x[1] + (y[1] - x[1]) * t,
                x[2] + (y[2] - x[2]) * t,
                x[3] + (y[3] - x[3]) * t,
            ]),
            (TrackValue::Quat(x), TrackValue::Quat(y)) => {
                let qa = bevy::prelude::Quat::from_array(*x);
                let qb = bevy::prelude::Quat::from_array(*y);
                TrackValue::Quat(qa.slerp(qb, t).to_array())
            }
            (TrackValue::Bool(x), TrackValue::Bool(y)) => {
                TrackValue::Bool(if t < 0.5 { *x } else { *y })
            }
            _ => *a,
        }
    }
}

/// Sample a property track at time `t` (seconds). Returns `None` for an empty
/// track. Clamps to the first/last key outside the keyed range. `Stepped` keys
/// hold their value until the next key; `Linear` keys blend toward the next.
pub fn sample_property_track(track: &PropertyTrack, t: f32) -> Option<TrackValue> {
    let keys = &track.keys;
    if keys.is_empty() {
        return None;
    }
    if t <= keys[0].time {
        return Some(keys[0].value);
    }
    let last = &keys[keys.len() - 1];
    if t >= last.time {
        return Some(last.value);
    }
    // Find the bracketing pair [i, i+1] with keys[i].time <= t < keys[i+1].time.
    // Keys are kept sorted by time on edit.
    let mut i = 0;
    while i + 1 < keys.len() && keys[i + 1].time <= t {
        i += 1;
    }
    let k0 = &keys[i];
    let k1 = &keys[i + 1];
    if matches!(k0.interp, Interp::Stepped) {
        return Some(k0.value);
    }
    let span = (k1.time - k0.time).max(f32::EPSILON);
    let frac = ((t - k0.time) / span).clamp(0.0, 1.0);
    Some(TrackValue::lerp(&k0.value, &k1.value, frac))
}

#[cfg(test)]
mod property_anim_tests {
    use super::*;

    fn ftrack(keys: Vec<PropertyKey>) -> PropertyTrack {
        PropertyTrack { target: String::new(), component: "x".into(), field: "y".into(), keys }
    }

    #[test]
    fn legacy_anim_without_property_tracks_loads() {
        // A `.anim` written before property tracks existed must still parse.
        let ron = r#"(name:"walk",duration:1.5,tracks:[])"#;
        let clip: AnimClip = ron::from_str(ron).unwrap();
        assert_eq!(clip.duration, 1.5);
        assert!(clip.property_tracks.is_empty());
    }

    #[test]
    fn anim_with_property_tracks_round_trips() {
        let clip = AnimClip {
            name: "sun".into(),
            duration: 2.0,
            tracks: vec![],
            property_tracks: vec![ftrack(vec![
                PropertyKey { time: 0.0, value: TrackValue::Quat([0.0, 0.0, 0.0, 1.0]), interp: Interp::Linear },
                PropertyKey { time: 2.0, value: TrackValue::Float(3.0), interp: Interp::Stepped },
            ])],
        };
        let s = ron::ser::to_string(&clip).unwrap();
        let back: AnimClip = ron::from_str(&s).unwrap();
        assert_eq!(back.property_tracks.len(), 1);
        assert_eq!(back.property_tracks[0].keys.len(), 2);
        assert_eq!(back.property_tracks[0].keys[1].interp, Interp::Stepped);
    }

    #[test]
    fn sample_linear_midpoint() {
        let track = ftrack(vec![
            PropertyKey { time: 0.0, value: TrackValue::Vec3([0.0, 0.0, 0.0]), interp: Interp::Linear },
            PropertyKey { time: 2.0, value: TrackValue::Vec3([2.0, 4.0, 0.0]), interp: Interp::Linear },
        ]);
        match sample_property_track(&track, 1.0).unwrap() {
            TrackValue::Vec3(v) => {
                assert!((v[0] - 1.0).abs() < 1e-5);
                assert!((v[1] - 2.0).abs() < 1e-5);
            }
            other => panic!("expected Vec3, got {other:?}"),
        }
    }

    #[test]
    fn sample_stepped_holds_previous() {
        let track = ftrack(vec![
            PropertyKey { time: 0.0, value: TrackValue::Float(0.0), interp: Interp::Stepped },
            PropertyKey { time: 2.0, value: TrackValue::Float(10.0), interp: Interp::Linear },
        ]);
        assert!(matches!(sample_property_track(&track, 1.9), Some(TrackValue::Float(v)) if v == 0.0));
    }

    #[test]
    fn sample_clamps_outside_range() {
        let track = ftrack(vec![PropertyKey {
            time: 1.0,
            value: TrackValue::Float(5.0),
            interp: Interp::Linear,
        }]);
        assert!(matches!(sample_property_track(&track, 0.0), Some(TrackValue::Float(v)) if v == 5.0));
        assert!(matches!(sample_property_track(&track, 9.0), Some(TrackValue::Float(v)) if v == 5.0));
        assert!(sample_property_track(&ftrack(vec![]), 0.0).is_none());
    }

    #[test]
    fn quat_slerp_endpoints() {
        let a = TrackValue::Quat([0.0, 0.0, 0.0, 1.0]);
        let b = TrackValue::Quat([0.0, 0.0, 1.0, 0.0]);
        assert_eq!(TrackValue::lerp(&a, &b, 0.0), a);
        match TrackValue::lerp(&a, &b, 1.0) {
            TrackValue::Quat(q) => {
                // slerp to b (allowing sign flip — q and -q are the same rotation).
                let dot = q[2] * 1.0 + q[3] * 0.0;
                assert!(dot.abs() > 0.99);
            }
            other => panic!("expected Quat, got {other:?}"),
        }
    }

    #[test]
    fn track_value_conversions() {
        assert!(matches!(TrackValue::Float(1.0).to_property_value(), Some(PropertyValue::Float(_))));
        assert!(TrackValue::Quat([0.0, 0.0, 0.0, 1.0]).to_property_value().is_none());
        assert!(matches!(
            TrackValue::from_property_value(&PropertyValue::Vec3([1.0, 2.0, 3.0])),
            Some(TrackValue::Vec3(_))
        ));
        assert!(matches!(
            TrackValue::from_property_value(&PropertyValue::Int(4)),
            Some(TrackValue::Float(v)) if v == 4.0
        ));
    }
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
    /// Returns true if in Playing or Paused state (full play mode).
    pub fn is_in_play_mode(&self) -> bool {
        matches!(self.state, PlayState::Playing | PlayState::Paused)
    }
    /// Returns true if scripts should be executing this frame.
    pub fn is_scripts_running(&self) -> bool {
        self.state == PlayState::Playing
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
/// Updated each frame by the viewport/editor systems from egui state.
#[derive(Resource, Default)]
pub struct InputFocusState {
    pub egui_wants_keyboard: bool,
    /// True when the pointer is over an egui panel (not the viewport).
    pub egui_has_pointer: bool,
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
