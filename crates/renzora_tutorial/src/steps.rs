//! The onboarding step catalog.
//!
//! Each [`Step`] is a hands-on task the user completes by **actually performing
//! the action** — there is no "Next" button to click through. The matching
//! completion signal for every [`StepKind`] is polled in
//! [`crate::state::detect_and_advance`]; the animated mouse/key [`Hint`] is drawn
//! by [`crate::hints`].

/// What action a step asks for — drives both the hint art and the detection
/// predicate. Order in [`STEPS`] is the order the tutorial walks.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StepKind {
    /// Orbit/look: the camera's yaw or pitch changed.
    Orbit,
    /// Zoom: the camera's orbit distance changed.
    Zoom,
    /// Fly/move: the camera's focus point moved (RMB + WASD).
    Fly,
    /// Select: the demo target mesh became the selected entity.
    Select,
    /// Transform: the demo target mesh was moved with the gizmo.
    Move,
    /// Switched the active workspace/layout (title-bar tabs).
    SwitchLayout,
    /// Added the tutorial's Demo panel to the dock via a tab bar's + picker.
    AddPanel,
    /// Re-docked / reordered the Demo panel by dragging its tab.
    ReorderPanel,
    /// Changed the fly camera's move speed in Settings.
    CameraSpeed,
    /// Added a World Environment via the hierarchy's Add Entity menu.
    AddEnvironment,
    /// Switched to a different editor theme in Settings → Theme.
    ChangeTheme,
    /// Imported a 3D model (a new model file landed in the project's assets).
    ImportModel,
    /// Opened/created a script in the code editor.
    CreateScript,
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

/// One onboarding task.
pub struct Step {
    pub kind: StepKind,
    /// Phosphor glyph shown in the step's header badge.
    pub badge: &'static str,
    pub title: &'static str,
    pub body: &'static str,
    pub hint: Hint,
}

/// The full guided sequence. Camera basics first (so the user learns to look at
/// the glowing target), then select it, then move it.
pub const STEPS: &[Step] = &[
    Step {
        kind: StepKind::Orbit,
        badge: "arrows-clockwise",
        title: "Orbit the view",
        body: "Hold the MIDDLE mouse button and drag to orbit the camera around the scene. Try circling the glowing cube.",
        hint: Hint { icons: &["mouse-middle-click"], keys: &["drag"], anim: HintAnim::Drag },
    },
    Step {
        kind: StepKind::Zoom,
        badge: "magnifying-glass-plus",
        title: "Zoom in and out",
        body: "Scroll the mouse wheel to dolly the camera closer and further away.",
        hint: Hint { icons: &["mouse-scroll"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::Fly,
        badge: "arrows-out-cardinal",
        title: "Fly around",
        body: "Hold the RIGHT mouse button and use W A S D to fly through the scene, like a first-person camera.",
        hint: Hint { icons: &["mouse-right-click"], keys: &["W", "A", "S", "D"], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::Select,
        badge: "cursor-click",
        title: "Select the cube",
        body: "Left-click the glowing cube to select it. Selected objects show a transform gizmo and appear in the Inspector.",
        hint: Hint { icons: &["cursor-click"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::Move,
        badge: "arrows-out-cardinal",
        title: "Move the cube",
        body: "Press W for the Move tool, then drag one of the colored gizmo arrows to slide the cube to a new spot.",
        hint: Hint { icons: &["keyboard"], keys: &["W", "drag"], anim: HintAnim::Drag },
    },
    Step {
        kind: StepKind::SwitchLayout,
        badge: "squares-four",
        title: "Switch workspace",
        body: "Click a workspace tab in the highlighted bar (Scene, Blueprints, Scripting…) to re-arrange the whole editor for a different kind of work.",
        hint: Hint { icons: &["mouse-left-click", "squares-four"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::AddPanel,
        badge: "stack",
        title: "Add a panel",
        body: "Click the highlighted + in a panel's tab bar and choose \"Demo Panel\" from the list to dock it into the editor.",
        hint: Hint { icons: &["mouse-left-click", "plus"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::ReorderPanel,
        badge: "arrows-out-cardinal",
        title: "Rearrange it",
        body: "Drag the highlighted \"Demo Panel\" tab and drop it over another panel (or its edge) to re-dock it somewhere new.",
        hint: Hint { icons: &["mouse-left-click"], keys: &["drag"], anim: HintAnim::Drag },
    },
    Step {
        kind: StepKind::CameraSpeed,
        badge: "sliders",
        title: "Tune your fly speed",
        body: "Open Settings, go to Viewport → Camera and drag the Move Speed slider to change how fast the right-click + WASD fly moves.",
        hint: Hint { icons: &["sliders"], keys: &["Ctrl", ","], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::AddEnvironment,
        badge: "globe",
        title: "Add an environment",
        body: "Click the highlighted \"Add Entity\" button and choose \"World Environment\" to drop in a sky, sun, atmosphere and fog.",
        hint: Hint { icons: &["mouse-left-click", "globe"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::ChangeTheme,
        badge: "palette",
        title: "Pick a theme",
        body: "Open the highlighted theme menu in the status bar (or Settings → Theme) and choose a different theme — the entire editor re-skins instantly.",
        hint: Hint { icons: &["palette"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::ImportModel,
        badge: "cube",
        title: "Import a 3D model",
        body: "Drag a .glb / .gltf / .fbx / .obj onto the viewport (or use the Asset browser's Import button) to bring a model into your project.",
        hint: Hint { icons: &["cube", "check-circle"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::CreateScript,
        badge: "file-plus",
        title: "Create a script",
        body: "Open the code editor on a script: add a script to a selected entity, or double-click a .lua / .rhai file in the Asset browser.",
        hint: Hint { icons: &["file-plus", "code"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::CreateUi,
        badge: "frame-corners",
        title: "Author some UI",
        body: "Switch the viewport to the UI view (the UI tab above the viewport, or add a UI Canvas from the Add menu) to start building game interface.",
        hint: Hint { icons: &["frame-corners", "cursor-click"], keys: &[], anim: HintAnim::Pulse },
    },
];

/// The chrome element (by bevy_ui `Name`) the animated highlight box should frame
/// for a step, or `None` for steps whose target is the viewport or a floating
/// overlay we can't reliably locate. The names are the shell's stable node names
/// (`renzora_shell`): `"ribbon"` (workspace tabs), `"dock-area"` (panels),
/// `"theme-menu"` (status-bar theme switcher).
pub fn highlight_for(kind: StepKind) -> Option<&'static str> {
    match kind {
        StepKind::SwitchLayout => Some("ribbon"),
        // The tab bar's "+" button (small + precise, unlike the whole dock area).
        StepKind::AddPanel => Some("dock-add-panel"),
        // The Demo panel's own tab (exists once it's been added).
        StepKind::ReorderPanel => Some("tab:tutorial_demo_panel"),
        // The hierarchy's "Add Entity" button.
        StepKind::AddEnvironment => Some("add-entity"),
        StepKind::ChangeTheme => Some("theme-menu"),
        _ => None,
    }
}
