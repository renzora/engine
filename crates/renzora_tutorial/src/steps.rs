//! Step + chapter types — the vocabulary the onboarding content is written in.
//!
//! A [`Chapter`] is one guided track (Getting Started, Scripting, Materials…);
//! the content of every chapter lives in [`crate::chapters`]. Each [`Step`] is a
//! hands-on task whose completion signal is polled in
//! [`crate::state::detect_step_done`], and whose animated mouse/key [`Hint`] is
//! drawn by [`crate::hints`].
//!
//! **Steps no longer auto-advance.** Detection only *arms* the step: the card
//! then shows a green Continue button and the user moves on when they're ready
//! (see `state::handle_continue`). That's what makes [`StepKind::Info`] viable —
//! a step with nothing to detect, which is how we teach the parts of the editor
//! that expose no observable state (a marketplace purchase, say) without either
//! lying about detecting them or reaching into another crate's private state.

use renzora::core::keybindings::EditorAction;

/// What action a step asks for — drives both the hint art and the detection
/// predicate. Order within a chapter's slice is the order the tutorial walks.
///
/// Payload-carrying variants keep the *content* in [`crate::chapters`] and the
/// *mechanism* here, so teaching a new panel is a one-line data change rather
/// than a new enum variant plus a new match arm.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    /// Nothing to detect — the user reads it and clicks Continue. Use when the
    /// action genuinely isn't observable from this crate, not as a shortcut
    /// around writing a predicate.
    Info,

    // ── Camera ───────────────────────────────────────────────────────────────
    /// Orbit/look: the camera's yaw or pitch changed.
    Orbit,
    /// Zoom: the camera's orbit distance changed.
    Zoom,
    /// Fly/move: the camera's focus point moved (RMB + WASD).
    Fly,

    // ── Selection & transform ────────────────────────────────────────────────
    /// Select: the demo target mesh became the selected entity.
    Select,
    /// Transform: the demo target mesh was moved with the gizmo.
    Move,
    /// The demo mesh was scaled (the Inspector's Transform fields, or `R`).
    Scale,
    /// Any entity is selected (not necessarily the demo mesh).
    SelectAny,

    // ── Scene contents ───────────────────────────────────────────────────────
    /// A new mesh entity appeared — the shape library, or a dropped model.
    AddShape,
    /// Added a World Environment via the hierarchy's Add Entity menu.
    AddEnvironment,
    /// A new light entity appeared.
    AddLight,
    /// The scene's entity count grew (duplicate) …
    Duplicate,
    /// … or shrank (delete).
    Delete,
    /// Imported a 3D model (a new model file landed in the project's assets).
    ImportModel,
    /// Any new asset file landed under the project — the marketplace install
    /// path, which writes into the project like any other download.
    InstallAsset,

    // ── Editor shell ─────────────────────────────────────────────────────────
    /// Switched the active workspace/layout (title-bar tabs).
    SwitchLayout,
    /// Added the tutorial's Demo panel to the dock via a tab bar's + picker.
    AddPanel,
    /// Re-docked / reordered the Demo panel by dragging its tab.
    ReorderPanel,
    /// Docked the named panel (by its `register_panel_content` id).
    OpenPanel(&'static str),
    /// Dragged the viewport toolbar's groups into a new arrangement.
    ReorderToolbar,
    /// Toggled any of the gizmo snapping switches (translate/rotate/scale).
    ToggleSnap,
    /// Changed the fly camera's move speed in Settings.
    CameraSpeed,
    /// Switched to a different editor theme in Settings → Theme.
    ChangeTheme,
    /// Pressed the key bound to an editor action (respects rebinding).
    Shortcut(EditorAction),

    // ── Scripting ────────────────────────────────────────────────────────────
    /// Opened/created a script in the code editor.
    CreateScript,
    /// A `.lua` script file exists under the project's `scripts/`.
    ScriptFile,
    /// Something in the scene turned *while the simulation ran* — i.e. a script
    /// actually drove it. Scene-wide on purpose: the user attaches their script
    /// to an entity of their own, not to the tutorial's demo mesh.
    ScriptedMotion,

    // ── Play ─────────────────────────────────────────────────────────────────
    /// Entered in-editor Simulate.
    Simulate,
    /// Entered full Play mode.
    Play,
    /// Returned to editing from Play/Simulate.
    StopPlay,

    // ── Authoring ────────────────────────────────────────────────────────────
    /// Entered the UI authoring view.
    CreateUi,
}

/// How a hint's glyphs animate.
#[derive(Clone, Copy)]
pub enum HintAnim {
    /// Gentle alpha + scale breathing — "press / click this".
    Pulse,
    /// Glyph slides back and forth — "drag".
    Drag,
}

/// The animated input hint shown under a step's instructions: one or more
/// Phosphor glyphs (mouse buttons, keyboard) plus optional key "chips".
pub struct Hint {
    /// Phosphor icon names (kebab-case) drawn left-to-right, e.g.
    /// `["mouse-middle-click"]`. Verified to exist in `phosphor_map`.
    pub icons: &'static [&'static str],
    /// Keyboard chips drawn after the icons, e.g. `["W", "A", "S", "D"]`.
    pub keys: &'static [&'static str],
    pub anim: HintAnim,
}

/// A hint that draws nothing — for [`StepKind::Info`] steps, which have no
/// gesture to illustrate.
pub const NO_HINT: Hint = Hint {
    icons: &[],
    keys: &[],
    anim: HintAnim::Pulse,
};

/// One onboarding task.
pub struct Step {
    pub kind: StepKind,
    /// Phosphor glyph shown in the step's header badge.
    pub badge: &'static str,
    pub title: &'static str,
    pub body: &'static str,
    pub hint: Hint,
}

/// One guided track. `id` is the persistence key (recorded in
/// `~/.renzora/editor.toml` once finished) and must stay stable across releases
/// — renaming one re-runs the chapter for everyone who'd already done it.
pub struct Chapter {
    pub id: &'static str,
    /// Phosphor glyph for the picker row and the card header.
    pub icon: &'static str,
    pub title: &'static str,
    /// One line under the title in the picker.
    pub summary: &'static str,
    /// Shown on the chapter's completion card.
    pub outro: &'static str,
    pub steps: &'static [Step],
}

/// Every chapter, in the order the picker lists them. Index 0 is the one that
/// auto-launches on the user's first run.
pub const CHAPTERS: &[Chapter] = crate::chapters::CHAPTERS;

/// The chrome elements (by bevy_ui `Name`) the animated highlight box + arrow
/// should frame for a step, **most specific first**. Empty for steps whose
/// target is the viewport or a floating overlay we can't reliably locate.
///
/// It's a list because these steps are two-stage: click a button, *then* pick a
/// row out of the overlay it opened. The highlight takes the first candidate
/// that's actually on screen, so it follows the user from the button to the row
/// without the step needing to know which stage they're in.
///
/// `search-row:<label>` targets a row in the shared ember search overlay (Add
/// Entity, the dock's + panel picker). It matches the row's **displayed** label,
/// which is localized — so in a non-English editor these fall back to
/// highlighting the button that opens the overlay, which is what they did
/// before rows were nameable at all.
pub fn highlight_for(kind: StepKind) -> &'static [&'static str] {
    match kind {
        // `ribbon-strip` — the workspace tab bar. NOT `"ribbon"`: that's the key
        // passed to `overflow_strip`, which names its nodes `<key>-strip` /
        // `-items` / `-overflow`. Targeting the bare key silently matched
        // nothing, so this step simply never highlighted.
        StepKind::SwitchLayout => &["ribbon-strip"],
        // The tab bar's "+" button (small + precise, unlike the whole dock area),
        // then the Demo Panel row in the picker it opens.
        // Three stages, in order of what's on screen: the row itself if it's
        // visible, else the overlay's search box (the list is long — typing is
        // how you actually reach a row that's scrolled away), else the + button
        // that opens the whole thing. Without the middle candidate, a scrolled
        // list falls all the way back to a button now hidden behind the overlay.
        StepKind::AddPanel => &["search-row:Demo Panel", "search-bar", "dock-add-panel"],
        StepKind::OpenPanel(_) => &["search-bar", "dock-add-panel"],
        // The Demo panel's own tab (exists once it's been added).
        StepKind::ReorderPanel => &["tab:tutorial_demo_panel"],
        // The hierarchy's "Add Entity" button, then the row being asked for.
        StepKind::AddEnvironment => &["search-row:World Environment", "search-bar", "add-entity"],
        StepKind::AddShape | StepKind::AddLight => &["search-bar", "add-entity"],
        // The viewport toolbar's camera-speed scrubber. The step also mentions
        // Settings → Editor → Camera, but that's behind a modal we can't point
        // into — the toolbar control is the one that's on screen right now.
        StepKind::CameraSpeed => &["vp-cam-speed"],
        StepKind::ChangeTheme => &["theme-menu"],
        _ => &[],
    }
}
