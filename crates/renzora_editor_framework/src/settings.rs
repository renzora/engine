use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Currently selected settings tab. A tab is a *page group*, not a sidebar row:
/// the settings sidebar splits several of these into finer categories (see
/// `CATS` in `renzora_settings`), and one category may stack several sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SettingsTab {
    #[default]
    Project,
    Interface,
    Editor,
    Viewport,
    Scripting,
    Input,
    Shortcuts,
    Theme,
    Plugins,
}

/// What a viewport click resolves to when the raycast hits a mesh inside a
/// larger imported hierarchy. The picker walks up from the hit mesh toward the
/// scene root; this decides where it stops.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum InspectorExpandDefault {
    /// Only the most-edited components (Name, Transform, Scripts) start open;
    /// everything else starts collapsed so long inspectors stay scannable.
    ///
    /// Was the default, for measured reasons that still stand — see [`Self::AllOpen`].
    /// Pick this one back up from Settings if a long component list starts
    /// costing frames.
    Essentials,
    /// Every component starts open.
    ///
    /// **The default**: hiding fields behind a click costs more than the extra
    /// scrolling, and an inspector you have to unfold before you can read it
    /// doesn't answer "what is this entity" at a glance.
    ///
    /// Know what it costs, because a collapsed section is not merely hidden —
    /// `cull_offscreen_sections` despawns its rows entirely and reserves the
    /// height with a placeholder, so what collapsing saves is real nodes rather
    /// than hidden ones. Measured on a scene with a world environment, terrain
    /// and camera: selecting an entity added **~1,082 bevy_ui nodes**, and
    /// bevy_ui charges for every node in the tree every frame whether or not
    /// anything about it changed — ~3 ms/frame, taking the editor from ~72 fps
    /// to ~59. [`Self::Essentials`] is the setting to reach for if that shows up.
    #[default]
    AllOpen,
    /// Every component starts collapsed.
    AllClosed,
}

impl InspectorExpandDefault {
    /// Default first — this drives the settings dropdown, and the option the
    /// editor actually ships with should be the one at the top of the list.
    pub const ALL: &'static [InspectorExpandDefault] =
        &[Self::AllOpen, Self::Essentials, Self::AllClosed];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Essentials => "Essentials Only",
            Self::AllOpen => "All Open",
            Self::AllClosed => "All Closed",
        }
    }
}

/// Available proportional (UI) font families.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
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
#[derive(Resource, Clone, PartialEq, Serialize, Deserialize)]
// Every field defaults, so a settings file written by an older build — or one
// missing a key because the user hand-edited it — loads with the shipped value
// for whatever is absent rather than failing the whole section.
#[serde(default)]
pub struct EditorSettings {
    /// Currently selected settings tab. Session-only: which tab you had open
    /// is not a preference, and restoring it puts you somewhere you did not ask
    /// to be on the next launch.
    #[serde(skip)]
    pub settings_tab: SettingsTab,
    /// What a viewport click selects within an imported model hierarchy
    pub selection_granularity: SelectionGranularity,
    /// Render the selection boundary on top of all geometry
    pub selection_boundary_on_top: bool,
    /// Base font size in points
    pub font_size: f32,
    /// Editor UI scale multiplier applied on top of the OS DPI scale
    /// (1.0 follows the system). Per-user rather than per-project because it is
    /// a property of the user's display, not of the game.
    pub ui_scale: f32,
    /// Panel scroll-speed multiplier (mouse wheel / arrow keys / middle-drag);
    /// 1.0 = default feel. Pushed into ember's `ScrollConfig` by the settings
    /// panel.
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
    /// Settings → Scripting).
    pub external_play_window: bool,
    /// The Play button launches Simulate (scripts + physics with the editor
    /// live) instead of full play. Chosen in the Play dropdown; session-only —
    /// deliberately NOT persisted, so a fresh editor always Plays. When false,
    /// `external_play_window` (which IS persisted) picks viewport vs window.
    pub play_launch_simulate: bool,
    /// The Play button launches the scene into a VR headset: the external
    /// runtime process with `--vr` (OpenXR stereo). Sits above
    /// `external_play_window` in precedence and is persisted per-user like it.
    pub play_launch_vr: bool,
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
    /// New scripts and UI templates are created with commented boilerplate
    /// showing the shape of the thing — hooks for a script, a laid-out panel
    /// for a template — rather than the bare minimum that parses.
    ///
    /// "Off" is *minimal*, not empty: a `.rs` script without
    /// `renzora::script!` exports no entry point and a `.html` without a
    /// `<template>` root does not parse, so the skeleton those need is always
    /// written. What the switch controls is whether there is anything inside it.
    pub new_file_boilerplate: bool,
    /// Pin expanded ancestor rows to the top of the hierarchy as you scroll.
    pub hierarchy_parent_stacking: bool,
    /// A hierarchy row click expands/collapses its subtree as well as selecting
    /// it. Off leaves the caret (and the Left/Right arrow keys) as the only way
    /// to fold a branch, so clicking through a deep model doesn't unfold every
    /// row you touch.
    pub hierarchy_toggle_on_click: bool,
    /// Which component sections start expanded when the inspector is built for a
    /// newly selected entity.
    pub inspector_expand_default: InspectorExpandDefault,
    /// Whether the settings overlay is open. Session-only, obviously: an editor
    /// that reopened the settings window every launch would be a bug.
    #[serde(skip)]
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

    /// Where the open-document tabs live: `false` (default) is the strip under
    /// the top bar, `true` folds them into a dropdown in the top bar beside
    /// Play, giving the row back to the dock. Per-user rather than per-project:
    /// how much vertical room the tabs are worth is a property of the screen
    /// you are on, not of the game.
    pub doc_tabs_dropdown: bool,

    /// Max entries the editor console retains before dropping the oldest. Small
    /// by default (100) because the console panel spawns a UI row per entry, so
    /// a long backlog costs frames. Pushed into the shared log buffer's cap by
    /// the settings panel.
    pub console_log_limit: usize,
}

impl EditorSettings {
    /// What the editor boots with: the settings as last saved, with the shipped
    /// values for anything absent.
    ///
    /// **This, not `default()`, is the startup path.** The two were one function
    /// once, which made `EditorSettings::default()` mean "the shipped values,
    /// except for some that quietly come back off disk" — a trap for anything
    /// trying to *reset* to defaults, because the fields it silently failed to
    /// reset were exactly the ones that survive a restart, and therefore the
    /// only ones anybody would notice. `Default` is now honestly the shipped
    /// values and the disk read lives here.
    ///
    /// **The whole struct round-trips.** It used to seed exactly nine fields
    /// from `~/.renzora/editor.toml`, each through its own `load_*` helper, and
    /// the other twenty-four simply did not survive a restart — you could turn
    /// on word wrap or the minimap, watch it stick for the session, and find it
    /// gone next launch. One `#[serde(default)]` section of
    /// `~/.renzora/settings.toml` replaces the nine helpers and covers all
    /// thirty-three.
    ///
    /// The two session-only fields (`settings_tab`, `show_settings`) are
    /// `#[serde(skip)]` and so take their `Default` here regardless.
    pub fn from_disk() -> Self {
        renzora::core::settings_file::load_section("editor").unwrap_or_default()
    }

    /// Write the whole section. Cheap enough to call on change: it is a
    /// read-modify-write of one small file, debounced by
    /// [`persist_editor_settings`] so a slider drag writes once rather than once
    /// per frame.
    pub fn save(&self) {
        if let Err(e) = renzora::core::settings_file::save_section("editor", self) {
            bevy::log::warn!("[settings] could not save editor settings: {e}");
        }
    }
}

impl Default for EditorSettings {
    fn default() -> Self {
        Self {
            settings_tab: SettingsTab::default(),
            selection_granularity: SelectionGranularity::default(),
            selection_boundary_on_top: false,
            font_size: 17.0,
            ui_scale: 1.0,
            scroll_speed: 1.5,
            ui_font: UiFont::default(),
            mono_font: MonoFont::default(),
            dev_mode: false,
            script_rerun_on_ready_on_reload: true,
            hide_cursor_in_play_mode: true,
            external_play_window: true,
            play_launch_simulate: false,
            play_launch_vr: false,
            maximize_viewport_on_play: true,
            auto_import_on_drop: true,
            drag_value_rail_sweep: true,
            renderer_backend: renzora::RendererBackend::default(),
            // Off: the backdrop needs a viewport panel on screen to have
            // anything to show — an undocked viewport slot renders at 64×64 —
            // and the UI workspace ships without one.
            ui_preview_by_default: false,
            new_file_boilerplate: true,
            hierarchy_parent_stacking: true,
            hierarchy_toggle_on_click: true,
            inspector_expand_default: InspectorExpandDefault::default(),
            show_settings: false,
            plugins_dir: "plugins".to_string(),
            code_auto_close_pairs: true,
            code_trim_trailing_whitespace_on_save: true,
            code_show_minimap: true,
            code_show_whitespace: false,
            code_word_wrap: false,
            code_open_switch_layout: false,
            doc_tabs_dropdown: false,
            console_log_limit: renzora::core::console_log::DEFAULT_MAX_LOG_ENTRIES,
        }
    }
}
