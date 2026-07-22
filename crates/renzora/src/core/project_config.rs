//! Project configuration, editor preferences, and on-disk settings.
//!
//! Split out of `core/mod.rs` to keep it manageable. Holds `ProjectConfig`
//! (the `project.toml` model) and its sub-configs (window / viewport / 2D /
//! rendering / network), the renderer-backend / UI-scale / stats-refresh /
//! dev-mode preference load+save helpers, `CurrentProject`, and the
//! `VirtualFileReader` (disk-vs-rpak read abstraction). Re-exported from `core`
//! (`pub use project_config::*`) so every `renzora::Foo` path is unchanged.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
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
    /// Auto-save: periodically re-save the open scene. On by default.
    #[serde(default = "default_true")]
    autosave_enabled: bool,
    /// Seconds between auto-saves (the countdown shown in the status bar).
    #[serde(default = "default_autosave_interval_secs")]
    autosave_interval_secs: u32,
    /// Active UI language code (e.g. `"en"`, `"fr"`, `"ja"`). Read at startup by
    /// the localization runtime so the user's choice survives restarts; it's a
    /// per-user preference, not a project property, hence it lives here.
    #[serde(default = "default_language")]
    language: String,
    /// Play-button target: `true` = Play launches the game in its own runtime
    /// window (with the project's configured title/resolution/mode), `false` =
    /// Play runs inside the editor viewport panel. Set from the Play button's
    /// target dropdown; per-user because it's a workflow preference, not a
    /// project property.
    #[serde(default)]
    play_runtime_window: bool,
    /// Play launches the scene into a VR headset (external runtime process
    /// with `--vr`). Layered above `play_runtime_window`: when set, the Play
    /// button's target is "VR Headset" regardless of the window preference.
    #[serde(default)]
    play_vr: bool,
    /// Multiplier on panel scrolling (mouse wheel / arrow keys / middle-drag);
    /// defaults to 1.5. Per-user because scroll feel is a property of the
    /// user's mouse and habits, not the project.
    #[serde(default = "default_scroll_speed")]
    scroll_speed: f32,
    /// Max entries the editor console retains. Per-user because it trades memory
    /// / per-frame console-panel cost against scrollback depth — a preference of
    /// the machine, not the project. Defaults small (100) so a chatty log can't
    /// drop frames; users who want deeper history raise it in Settings.
    #[serde(default = "default_console_log_limit")]
    console_log_limit: u32,
}

fn default_language() -> String {
    "en".to_string()
}

fn default_autosave_interval_secs() -> u32 {
    300
}

fn default_ui_scale() -> f32 {
    1.0
}
fn default_scroll_speed() -> f32 {
    1.5
}
fn default_console_log_limit() -> u32 {
    super::console_log::DEFAULT_MAX_LOG_ENTRIES as u32
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
            autosave_enabled: true,
            autosave_interval_secs: default_autosave_interval_secs(),
            language: default_language(),
            play_runtime_window: false,
            play_vr: false,
            scroll_speed: default_scroll_speed(),
            console_log_limit: default_console_log_limit(),
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

/// Load the persisted panel scroll-speed multiplier, defaulting to 1.5 (the
/// editor's default feel) when the file is absent or unreadable.
pub fn load_scroll_speed() -> f32 {
    #[cfg(target_arch = "wasm32")]
    {
        default_scroll_speed()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(path) = editor_pref_path() else {
            return default_scroll_speed();
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return default_scroll_speed();
        };
        toml::from_str::<EditorPrefFile>(&text)
            .map(|f| f.scroll_speed)
            .unwrap_or_else(|_| default_scroll_speed())
            .clamp(0.1, 5.0)
    }
}

/// Persist the panel scroll-speed multiplier (read-modify-write so other prefs
/// in the file survive).
#[cfg(not(target_arch = "wasm32"))]
pub fn save_scroll_speed(scroll_speed: f32) -> std::io::Result<()> {
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
    prefs.scroll_speed = scroll_speed;
    let text = toml::to_string_pretty(&prefs).map_err(std::io::Error::other)?;
    std::fs::write(&path, text)
}

/// Load the persisted console log-entry limit, defaulting to
/// [`console_log::DEFAULT_MAX_LOG_ENTRIES`] when the file is absent or
/// unreadable. Floored at 10 so the console can never be capped to nothing.
pub fn load_console_log_limit() -> usize {
    let default = super::console_log::DEFAULT_MAX_LOG_ENTRIES;
    #[cfg(target_arch = "wasm32")]
    {
        default
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(path) = editor_pref_path() else {
            return default;
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return default;
        };
        toml::from_str::<EditorPrefFile>(&text)
            .map(|f| f.console_log_limit as usize)
            .unwrap_or(default)
            .max(10)
    }
}

/// Persist the console log-entry limit (read-modify-write so other prefs in the
/// file survive).
#[cfg(not(target_arch = "wasm32"))]
pub fn save_console_log_limit(limit: usize) -> std::io::Result<()> {
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
    prefs.console_log_limit = limit as u32;
    let text = toml::to_string_pretty(&prefs).map_err(std::io::Error::other)?;
    std::fs::write(&path, text)
}

/// Load the persisted UI language code, defaulting to `"en"` when the file is
/// absent or unreadable. Called by the localization runtime at startup.
pub fn load_language() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        "en".to_string()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(path) = editor_pref_path() else {
            return default_language();
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return default_language();
        };
        toml::from_str::<EditorPrefFile>(&text)
            .map(|f| f.language)
            .unwrap_or_else(|_| default_language())
    }
}

/// Persist the active UI language code (read-modify-write so other prefs in the
/// file survive).
#[cfg(not(target_arch = "wasm32"))]
pub fn save_language(code: &str) -> std::io::Result<()> {
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
    prefs.language = code.to_string();
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

/// Load the persisted Play-button target (default `false` = in-viewport play).
/// The editor seeds `EditorSettings.external_play_window` from this at startup
/// so the Play dropdown's choice survives restarts.
pub fn load_play_runtime_window() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        false
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        editor_pref_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| toml::from_str::<EditorPrefFile>(&t).ok())
            .map(|f| f.play_runtime_window)
            .unwrap_or(false)
    }
}

/// Load the persisted VR play target (default `false`).
pub fn load_play_vr() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        false
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        editor_pref_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| toml::from_str::<EditorPrefFile>(&t).ok())
            .map(|f| f.play_vr)
            .unwrap_or(false)
    }
}

/// Persist the VR play target (read-modify-write, so other fields survive).
#[cfg(not(target_arch = "wasm32"))]
pub fn save_play_vr(play_vr: bool) -> std::io::Result<()> {
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
    prefs.play_vr = play_vr;
    let text = toml::to_string_pretty(&prefs).map_err(std::io::Error::other)?;
    std::fs::write(&path, text)
}

/// Persist the Play-button target (read-modify-write, so other fields survive).
#[cfg(not(target_arch = "wasm32"))]
pub fn save_play_runtime_window(runtime_window: bool) -> std::io::Result<()> {
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
    prefs.play_runtime_window = runtime_window;
    let text = toml::to_string_pretty(&prefs).map_err(std::io::Error::other)?;
    std::fs::write(&path, text)
}

/// Auto-save preferences, persisted per-user in `~/.renzora/editor.toml`.
///
/// A contract resource (rather than living in `EditorSettings`) so the
/// `renzora_autosave` plugin — which owns the countdown + save trigger — depends
/// only on this dylib, and the settings UI edits it the same way. Off by default;
/// the editor never writes scene files until the user opts in.
#[derive(Resource, Clone, Copy, PartialEq, Debug)]
pub struct AutoSaveSettings {
    pub enabled: bool,
    /// Seconds between auto-saves.
    pub interval_secs: u32,
}

impl Default for AutoSaveSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_secs: default_autosave_interval_secs(),
        }
    }
}

/// Load the persisted auto-save preferences (defaults when the file is absent).
pub fn load_autosave() -> AutoSaveSettings {
    #[cfg(target_arch = "wasm32")]
    {
        AutoSaveSettings::default()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let prefs = editor_pref_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| toml::from_str::<EditorPrefFile>(&t).ok())
            .unwrap_or_default();
        AutoSaveSettings {
            enabled: prefs.autosave_enabled,
            // Clamp to a sane floor so a corrupt/0 value can't busy-save.
            interval_secs: prefs.autosave_interval_secs.clamp(10, 3600),
        }
    }
}

/// Persist the auto-save preferences (read-modify-write, so other fields survive).
#[cfg(not(target_arch = "wasm32"))]
pub fn save_autosave(settings: &AutoSaveSettings) -> std::io::Result<()> {
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
    prefs.autosave_enabled = settings.enabled;
    prefs.autosave_interval_secs = settings.interval_secs;
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

/// One entry in [`ProjectConfig::editor_open_tabs`] — a document tab the
/// editor had open when the project was last used. Lives in the contract
/// crate only as a serialization record; the editor UI crate owns the
/// `DocTabKind` enum and converts to/from the `kind` name.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EditorOpenTab {
    /// Project-relative path of the open document (e.g. `"scenes/main.bsn"`).
    pub path: String,
    /// Persisted tab-kind name (`"scene"`, `"material"`, `"particle"`,
    /// `"blueprint"`, `"script"`, `"shader"`, `"other"`). Kept as a string so
    /// a config written by a newer editor with extra kinds still parses here;
    /// unknown names degrade to a plain tab instead of failing the load.
    #[serde(default)]
    pub kind: String,
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
    /// Every document tab the editor had open when the project was last used
    /// (in display order). Restored on project load so open materials/scripts/
    /// scenes survive a reload; the *active* scene still comes from
    /// `editor_last_scene`. Editor-only — the runtime ignores it and export
    /// strips it from shipped builds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub editor_open_tabs: Vec<EditorOpenTab>,
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
            editor_open_tabs: Vec::new(),
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
