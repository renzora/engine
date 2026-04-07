use bevy::prelude::*;

/// Currently selected settings tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    General,
    Viewport,
    Shortcuts,
    Theme,
    Input,
    Plugins,
}

/// Selection highlight mode when using the Select tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionHighlightMode {
    #[default]
    Outline,
    Gizmo,
}

/// Available proportional (UI) font families.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum UiFont {
    Roboto,
    OpenSans,
    #[default]
    NotoSans,
    /// A custom `.ttf`/`.otf` from the project's `fonts/` directory.
    Custom(String),
}

impl UiFont {
    pub fn label(&self) -> &str {
        match self {
            Self::Roboto => "Roboto",
            Self::OpenSans => "Open Sans",
            Self::NotoSans => "Noto Sans",
            Self::Custom(name) => name,
        }
    }

    pub const BUILTIN: &'static [UiFont] = &[Self::Roboto, Self::OpenSans, Self::NotoSans];

    pub fn font_key(&self) -> &str {
        match self {
            Self::Roboto => "roboto",
            Self::OpenSans => "open-sans",
            Self::NotoSans => "noto-sans",
            Self::Custom(name) => name,
        }
    }
}

/// Available monospace (code) font families.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum MonoFont {
    #[default]
    JetBrainsMono,
    FiraCode,
    SourceCodePro,
    /// A custom `.ttf`/`.otf` from the project's `fonts/` directory.
    Custom(String),
}

impl MonoFont {
    pub fn label(&self) -> &str {
        match self {
            Self::JetBrainsMono => "JetBrains Mono",
            Self::FiraCode => "Fira Code",
            Self::SourceCodePro => "Source Code Pro",
            Self::Custom(name) => name,
        }
    }

    pub const BUILTIN: &'static [MonoFont] = &[
        Self::JetBrainsMono,
        Self::FiraCode,
        Self::SourceCodePro,
    ];

    pub fn font_key(&self) -> &str {
        match self {
            Self::JetBrainsMono => "jetbrains-mono",
            Self::FiraCode => "fira-code",
            Self::SourceCodePro => "source-code-pro",
            Self::Custom(name) => name,
        }
    }
}

/// Custom fonts discovered in the project's `fonts/` directory.
///
/// Each entry is a font key (filename stem) that has been loaded into egui.
#[derive(Resource, Default, Clone, Debug)]
pub struct CustomFonts {
    pub names: Vec<String>,
}

/// General editor settings and preferences.
///
/// Cross-cutting settings that don't belong to any specific editor plugin.
/// Viewport, camera, grid, and keybinding settings live in their own crates.
#[derive(Resource, Clone, PartialEq)]
pub struct EditorSettings {
    /// Currently selected settings tab
    pub settings_tab: SettingsTab,
    /// Selection highlight mode (outline or gizmo)
    pub selection_highlight_mode: SelectionHighlightMode,
    /// Render the selection boundary on top of all geometry
    pub selection_boundary_on_top: bool,
    /// Base font size in points
    pub font_size: f32,
    /// Selected UI (proportional) font family
    pub ui_font: UiFont,
    /// Selected monospace (code) font family
    pub mono_font: MonoFont,
    /// Developer mode — enables plugin development tools
    pub dev_mode: bool,
    /// Re-run on_ready when a script is hot-reloaded
    pub script_rerun_on_ready_on_reload: bool,
    /// Use game camera when running scripts (ScriptsOnly mode)
    pub scripts_use_game_camera: bool,
    /// Hide and lock the cursor when entering play mode
    pub hide_cursor_in_play_mode: bool,
    /// Whether the settings overlay is open
    pub show_settings: bool,
    /// Directory to load dynamic plugins from
    pub plugins_dir: String,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            settings_tab: SettingsTab::default(),
            selection_highlight_mode: SelectionHighlightMode::default(),
            selection_boundary_on_top: false,
            font_size: 14.0,
            ui_font: UiFont::default(),
            mono_font: MonoFont::default(),
            dev_mode: false,
            script_rerun_on_ready_on_reload: true,
            scripts_use_game_camera: true,
            hide_cursor_in_play_mode: true,
            show_settings: false,
            plugins_dir: "plugins".to_string(),
        }
    }
}
