use bevy::prelude::*;

/// Currently selected settings tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsTab {
    #[default]
    Project,
    Interface,
    Editor,
    Viewport,
    Scripting,
    Assets,
    Input,
    Shortcuts,
    Theme,
    Plugins,
}

/// Selection highlight mode when using the Select tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionHighlightMode {
    Outline,
    #[default]
    Gizmo,
}

/// What a viewport click resolves to when the raycast hits a mesh inside a
/// larger imported hierarchy. The picker walks up from the hit mesh toward the
/// scene root; this decides where it stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionGranularity {
    /// The exact leaf mesh the ray hit — never bubbles up to a parent.
    Mesh,
    /// The root of the clicked mesh's own sub-tree: the topmost named ancestor
    /// still *below* the model boundary (`SelectionStop`). For a flat model
    /// whose meshes sit directly under the root this is the mesh itself; for a
    /// nested scene it's the top-level sub-object (e.g. a whole building).
    #[default]
    MeshRoot,
    /// The entire imported model as one unit — bubbles all the way up to the
    /// model root (the `SelectionStop` bearer).
    EntireRoot,
}

impl SelectionGranularity {
    pub const ALL: &'static [SelectionGranularity] =
        &[Self::Mesh, Self::MeshRoot, Self::EntireRoot];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Mesh => "Mesh",
            Self::MeshRoot => "Mesh Root",
            Self::EntireRoot => "Entire Model",
        }
    }
}

/// Which inspector component sections start expanded when the inspector is
/// (re)built for a freshly selected entity.
///
/// The inspector rebuilds its section list on every selection / component-set
/// change, so this is the *initial* open state each time — the user can still
/// collapse/expand any section by hand, and the expand/collapse-all button
/// overrides it for the current view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InspectorExpandDefault {
    /// Only the most-edited components (Name, Transform, Scripts) start open;
    /// everything else starts collapsed so long inspectors stay scannable.
    /// The default.
    #[default]
    Essentials,
    /// Every component starts open.
    AllOpen,
    /// Every component starts collapsed.
    AllClosed,
}

impl InspectorExpandDefault {
    pub const ALL: &'static [InspectorExpandDefault] =
        &[Self::Essentials, Self::AllOpen, Self::AllClosed];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Essentials => "Essentials Only",
            Self::AllOpen => "All Open",
            Self::AllClosed => "All Closed",
        }
    }
}

/// How the inspector presents its per-component filter: a vertical icon menu down
/// the left edge, or a compact dropdown in the top bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InspectorComponentFilterStyle {
    /// A single dropdown in the top bar.
    #[default]
    Dropdown,
    /// One icon button per component down the left rail (plus an "All" entry).
    VerticalMenu,
}

impl InspectorComponentFilterStyle {
    pub const ALL: &'static [InspectorComponentFilterStyle] =
        &[Self::Dropdown, Self::VerticalMenu];

    pub fn label(&self) -> &'static str {
        match self {
            Self::VerticalMenu => "Vertical Menu",
            Self::Dropdown => "Dropdown",
        }
    }
}

/// Available proportional (UI) font families.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum UiFont {
    /// The operating system's default UI font (Segoe UI on Windows, San
    /// Francisco on macOS, …), resolved via Parley's system-font discovery.
    /// Always available, so it's the reliable "change something" option.
    System,
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
            Self::System => "System UI",
            Self::Roboto => "Roboto",
            Self::OpenSans => "Open Sans",
            Self::NotoSans => "Noto Sans",
            Self::Custom(name) => name,
        }
    }

    pub const BUILTIN: &'static [UiFont] =
        &[Self::System, Self::Roboto, Self::OpenSans, Self::NotoSans];

    pub fn font_key(&self) -> &str {
        match self {
            Self::System => "system",
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

    pub const BUILTIN: &'static [MonoFont] =
        &[Self::JetBrainsMono, Self::FiraCode, Self::SourceCodePro];

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
    /// What a viewport click selects within an imported model hierarchy
    pub selection_granularity: SelectionGranularity,
    /// Render the selection boundary on top of all geometry
    pub selection_boundary_on_top: bool,
    /// Base font size in points
    pub font_size: f32,
    /// Editor UI scale multiplier applied on top of the OS DPI scale
    /// (1.0 follows the system). Persisted per-user in `~/.renzora/editor.toml`
    /// because it's a property of the user's display, not the project.
    pub ui_scale: f32,
    /// Panel scroll-speed multiplier (mouse wheel / arrow keys / middle-drag);
    /// 1.0 = default feel. Pushed into ember's `ScrollConfig` by the settings
    /// panel and persisted per-user in `~/.renzora/editor.toml`.
    pub scroll_speed: f32,
    /// Selected UI (proportional) font family
    pub ui_font: UiFont,
    /// Selected monospace (code) font family
    pub mono_font: MonoFont,
    /// Developer mode — enables plugin development tools
    pub dev_mode: bool,
    /// Re-run on_ready when a script is hot-reloaded
    pub script_rerun_on_ready_on_reload: bool,
    /// Hide and lock the cursor when entering play mode
    pub hide_cursor_in_play_mode: bool,
    /// Spawn the runtime as a child process when entering play mode, instead
    /// of doing the in-editor camera switch. Gives a "real exported game"
    /// experience — its own window with the project's configured title /
    /// resolution / window mode / icon, and full insulation from editor state.
    /// Uses the packaged `renzora-runtime` sibling when one exists, otherwise
    /// relaunches this same binary with `--no-editor` (the engine is one
    /// binary either way). Chosen from the Play button's target dropdown (or
    /// Settings → Scripting) and persisted per-user in `~/.renzora/editor.toml`.
    pub external_play_window: bool,
    /// The Play button launches Simulate (scripts + physics with the editor
    /// live) instead of full play. Chosen in the Play dropdown; session-only —
    /// deliberately NOT persisted, so a fresh editor always Plays. When false,
    /// `external_play_window` (which IS persisted) picks viewport vs window.
    pub play_launch_simulate: bool,
    /// When entering play mode, maximize the viewport (collapse the rest of the
    /// dock to a single viewport leaf) for a clean game view; restored on Stop.
    pub maximize_viewport_on_play: bool,
    /// Auto-import dropped assets with default settings instead of showing the import overlay
    pub auto_import_on_drop: bool,
    /// Numeric drag fields: a press on the bottom slider rail sets the value
    /// absolutely (a fast min→max sweep) instead of the fine relative scrub.
    pub drag_value_rail_sweep: bool,
    /// Graphics backend wgpu requests at startup. Persisted to disk (not held
    /// only in this resource) because the renderer is created before this
    /// resource exists; changing it requires an editor restart to take effect.
    pub renderer_backend: renzora::RendererBackend,
    /// Enable game viewport preview behind the UI canvas by default when entering the UI workspace.
    pub ui_preview_by_default: bool,
    /// Pin expanded ancestor rows to the top of the hierarchy as you scroll.
    pub hierarchy_parent_stacking: bool,
    /// Which component sections start expanded when the inspector is built for a
    /// newly selected entity.
    pub inspector_expand_default: InspectorExpandDefault,
    /// How the inspector presents its per-component filter (left rail vs dropdown).
    pub inspector_component_filter_style: InspectorComponentFilterStyle,
    /// Whether the settings overlay is open
    pub show_settings: bool,
    /// Directory to load dynamic plugins from
    pub plugins_dir: String,

    // ── Code editor preferences ──
    /// Type `(` `[` `{` `"` `'` to insert the closing pair too.
    pub code_auto_close_pairs: bool,
    /// Strip trailing spaces/tabs from each line on save.
    pub code_trim_trailing_whitespace_on_save: bool,
    /// Show the minimap sidebar in the code editor.
    pub code_show_minimap: bool,
    /// Show whitespace markers in the code editor.
    pub code_show_whitespace: bool,
    /// Soft-wrap long lines in the code editor instead of scrolling horizontally.
    pub code_word_wrap: bool,
    /// "Open in Code Editor" behaviour: `false` adds a Code Editor panel to the
    /// current dock layout; `true` switches to the dedicated "Scripting" layout.
    pub code_open_switch_layout: bool,
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            settings_tab: SettingsTab::default(),
            selection_highlight_mode: SelectionHighlightMode::default(),
            selection_granularity: SelectionGranularity::default(),
            selection_boundary_on_top: false,
            font_size: 17.0,
            ui_scale: renzora::load_ui_scale(),
            scroll_speed: renzora::load_scroll_speed(),
            ui_font: UiFont::default(),
            mono_font: MonoFont::default(),
            // Seeded from the persisted contract flag so dev mode (and anything
            // gated on it, e.g. the `renzora_tracy` profiler) survives restarts.
            dev_mode: renzora::load_dev_mode(),
            script_rerun_on_ready_on_reload: true,
            hide_cursor_in_play_mode: true,
            // Seeded from the persisted per-user pref (the Play dropdown's
            // choice); defaults to in-viewport play.
            external_play_window: renzora::load_play_runtime_window(),
            play_launch_simulate: false,
            maximize_viewport_on_play: true,
            auto_import_on_drop: true,
            drag_value_rail_sweep: true,
            // Seed the UI's working copy from the persisted preference so the
            // settings panel shows what the renderer actually booted with.
            renderer_backend: renzora::load_renderer_backend(),
            ui_preview_by_default: true,
            hierarchy_parent_stacking: true,
            inspector_expand_default: InspectorExpandDefault::default(),
            inspector_component_filter_style: InspectorComponentFilterStyle::default(),
            show_settings: false,
            plugins_dir: "plugins".to_string(),
            code_auto_close_pairs: true,
            code_trim_trailing_whitespace_on_save: true,
            code_show_minimap: true,
            code_show_whitespace: false,
            code_word_wrap: false,
            code_open_switch_layout: false,
        }
    }
}
