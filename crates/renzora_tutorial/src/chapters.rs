//! The onboarding content: every [`Chapter`] and its [`Step`]s.
//!
//! Split from [`crate::steps`] (which owns the *types*) because this is pure
//! data and grows with every editor feature we teach, while the vocabulary
//! underneath it changes rarely.
//!
//! Two rules when adding to this file:
//!
//! 1. **Say only what the editor actually does.** These strings are the first
//!    thing a new user reads, so a stale path here is worse than no tutorial —
//!    it sends them hunting through menus. Issue #83 was exactly that (a step
//!    still pointing at Settings → Viewport → Camera after the settings sidebar
//!    was regrouped).
//! 2. **`Info` is for what we genuinely can't observe**, not for steps that are
//!    merely fiddly to detect. A marketplace purchase or a node dropped in the
//!    material graph lives behind another crate's private state; camera moves,
//!    dock changes and file writes do not.

use renzora::core::keybindings::EditorAction;

use crate::steps::{Chapter, Hint, HintAnim, Step, StepKind, NO_HINT};

/// Every chapter, in picker order. Index 0 auto-launches on a project's first run.
pub const CHAPTERS: &[Chapter] = &[
    GETTING_STARTED,
    SCENE_BUILDING,
    SCRIPTING,
    MATERIALS,
    WORKSPACE,
    MARKETPLACE,
    PLAY_MODE,
];

// ── 1. Getting Started ──────────────────────────────────────────────────────

const GETTING_STARTED: Chapter = Chapter {
    id: "getting-started",
    icon: "graduation-cap",
    title: "Getting Started",
    summary: "Move the camera, select and move objects, find your way around",
    outro: "That's the basics — orbiting, zooming, flying, selecting and moving objects. \
            Pick another chapter from the list any time, or dive in and start building.",
    steps: GETTING_STARTED_STEPS,
};

/// Camera basics first (so the user learns to look at the glowing target), then
/// select it, then move it, then a tour of the shell.
const GETTING_STARTED_STEPS: &[Step] = &[
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
        kind: StepKind::Shortcut(EditorAction::ResetCamera),
        badge: "house",
        title: "Find your way home",
        body: "Flown off into the void? Press HOME to snap the camera back to where it started. It's the fastest way out of being lost.",
        hint: Hint { icons: &["keyboard"], keys: &["Home"], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::Select,
        badge: "cursor-click",
        title: "Select the cube",
        body: "Left-click the glowing cube to select it. Selected objects show a transform gizmo and appear in the *Inspector*.",
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
        body: "Click the highlighted + in a panel's tab bar and choose \"*Demo Panel*\" from the list to dock it into the editor.",
        hint: Hint { icons: &["mouse-left-click", "plus"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::ReorderPanel,
        badge: "arrows-out-cardinal",
        title: "Rearrange it",
        body: "Drag the highlighted \"*Demo Panel*\" tab and drop it over another panel (or its edge) to re-dock it somewhere new.",
        hint: Hint { icons: &["mouse-left-click"], keys: &["drag"], anim: HintAnim::Drag },
    },
    Step {
        kind: StepKind::CameraSpeed,
        badge: "sliders",
        // The Camera page moved under the sidebar's EDITOR group (it used to sit
        // under VIEWPORT) — see `CATS` in `renzora_settings::native`. Keep this
        // wording in step with that table; a wrong path sends users hunting.
        title: "Tune your fly speed",
        body: "Too fast or too slow? The viewport toolbar has a *Move Speed* control right there — or set it permanently in Settings → Editor → Camera. Either one changes how fast right-click + WASD flies.",
        hint: Hint { icons: &["sliders"], keys: &["Ctrl", ","], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::AddEnvironment,
        badge: "globe",
        title: "Add an environment",
        body: "Open the *Hierarchy* panel, click its highlighted \"*Add Entity*\" button, then choose \"*World Environment*\" from the list — that drops in a sky, sun, atmosphere and fog in one go.",
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
        body: "Drag a .glb / .gltf / .fbx / .obj onto the viewport (or use the *Asset browser*'s Import button) to bring a model into your project.",
        hint: Hint { icons: &["cube", "check-circle"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::CreateScript,
        badge: "file-plus",
        title: "Create a script",
        body: "Open the code editor on a script: add a script to a selected entity, or double-click a .lua file in the *Asset browser*.",
        hint: Hint { icons: &["file-plus", "code"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::CreateUi,
        badge: "frame-corners",
        title: "Author some UI",
        body: "Switch the viewport to the UI view (the UI tab above the viewport, or add a *UI Canvas* from the Add menu) to start building game interface.",
        hint: Hint { icons: &["frame-corners", "cursor-click"], keys: &[], anim: HintAnim::Pulse },
    },
];

// ── 2. Building a Scene ─────────────────────────────────────────────────────

const SCENE_BUILDING: Chapter = Chapter {
    id: "scene-building",
    icon: "cube",
    title: "Building a Scene",
    summary: "Shapes, lights, duplicating, deleting, and editing components",
    outro: "You can now fill a scene: add shapes and lights, duplicate and delete them, \
            and tune any component from the Inspector.",
    steps: SCENE_BUILDING_STEPS,
};

const SCENE_BUILDING_STEPS: &[Step] = &[
    Step {
        kind: StepKind::AddShape,
        badge: "cube",
        title: "Add a shape",
        body: "In the *Hierarchy* panel, click the highlighted \"*Add Entity*\" button and pick something from the *Shapes* group — a Cube, Sphere, Stairs, a Torus. It drops in at the world origin.",
        hint: Hint { icons: &["mouse-left-click", "cube"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::SelectAny,
        badge: "cursor-click",
        title: "Select it",
        body: "Click your new shape in the viewport, or click its row in the *Hierarchy* panel. Either way the *Inspector* fills with its components.",
        hint: Hint { icons: &["cursor-click"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::Scale,
        badge: "resize",
        title: "Edit a component",
        body: "In the *Inspector*, find the *Transform* component and type a new *Scale* — or press R for the *Scale* tool and drag a gizmo handle. Every component works this way: the *Inspector* edits it live.",
        hint: Hint { icons: &["keyboard"], keys: &["R", "drag"], anim: HintAnim::Drag },
    },
    Step {
        kind: StepKind::ToggleSnap,
        badge: "grid-nine",
        title: "Turn on snapping",
        body: "Open the *Snaps* group in the viewport toolbar and switch on Move snapping. Now dragging the gizmo steps in fixed increments instead of sliding freely — the fastest way to line things up. Rotate and *Scale* have their own switches.",
        hint: Hint { icons: &["mouse-left-click", "grid-nine"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::Duplicate,
        badge: "copy",
        title: "Duplicate it",
        body: "With the shape still selected, press Ctrl+D. You get an independent copy in the same spot — drag it aside to see both.",
        hint: Hint { icons: &["copy"], keys: &["Ctrl", "D"], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::AddLight,
        badge: "lightbulb",
        title: "Light it",
        body: "*Add Entity* → a Point Light or Spot Light. Move it above your shapes and watch the shading change as you drag.",
        hint: Hint { icons: &["mouse-left-click", "lightbulb"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::Delete,
        badge: "trash",
        title: "Delete something",
        body: "Select an entity you don't want and press Delete. Changed your mind? Ctrl+Z brings it back — the whole editor is undoable.",
        hint: Hint { icons: &["trash"], keys: &["Del"], anim: HintAnim::Pulse },
    },
];

// ── 3. Scripting ────────────────────────────────────────────────────────────

const SCRIPTING: Chapter = Chapter {
    id: "scripting",
    icon: "code",
    title: "Scripting",
    summary: "Write Lua, attach it to an entity, and watch it run",
    outro: "You've written a script, attached it, and run it. Everything else in the \
            scripting API — input, physics, UI, networking — hangs off the same hooks.",
    steps: SCRIPTING_STEPS,
};

const SCRIPTING_STEPS: &[Step] = &[
    Step {
        kind: StepKind::Info,
        badge: "code",
        title: "How scripts work",
        body: "A script is a .lua file in your project's scripts/ folder, attached to an entity through its *Script* component. The engine calls your hooks: on_ready() once, on_update() every frame.",
        hint: NO_HINT,
    },
    Step {
        kind: StepKind::ScriptFile,
        badge: "file-plus",
        title: "Create a script",
        body: "Select an entity, find the *Script* section in the *Inspector*, and add a new script. That creates the .lua file and opens it in the code editor.",
        hint: Hint { icons: &["file-plus"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::Info,
        badge: "arrows-clockwise",
        title: "Make it spin",
        body: "Replace the file's contents with:\n\nfunction on_update()\n    rotate(0, delta * 90, 0)\nend\n\nrotate() takes Euler DEGREES, and delta is the seconds since last frame — so that's 90° per second, at any frame rate. Save with Ctrl+S.",
        hint: Hint { icons: &["floppy-disk"], keys: &["Ctrl", "S"], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::Simulate,
        badge: "flask",
        title: "Run it",
        body: "Open the play-target menu (the caret next to *Play*), choose *Simulate*, then press *Play*. *Simulate* runs your scripts while the editor stays fully live — camera, gizmos and *Inspector* all keep working.",
        hint: Hint { icons: &["mouse-left-click", "flask"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::ScriptedMotion,
        badge: "arrows-clockwise",
        title: "Watch it turn",
        body: "Your entity should be spinning. Tweak the number in the script and save — the change takes effect without leaving *Simulate*.",
        hint: Hint { icons: &["eye"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::Info,
        badge: "graph",
        title: "Reach other entities",
        body: "A script isn't limited to its own entity. set_on(\"Lamp\", \"PointLight.intensity\", 5000) writes a field on any named entity, and get_on() reads one back — that's how one script drives a whole scene.",
        hint: NO_HINT,
    },
    Step {
        kind: StepKind::StopPlay,
        badge: "stop",
        title: "Stop the simulation",
        body: "Press *Stop*. *Simulate* snapshots the scene on entry and restores it on exit, so your spinning entity snaps back to where it started — nothing you simulate is permanent.",
        hint: Hint { icons: &["stop"], keys: &[], anim: HintAnim::Pulse },
    },
];

// ── 4. Materials ────────────────────────────────────────────────────────────

const MATERIALS: Chapter = Chapter {
    id: "materials",
    icon: "paint-brush",
    title: "Materials",
    summary: "Build a material in the node graph and put it on a mesh",
    outro: "Materials are node graphs that compile to WGSL. The same graph editor \
            drives post-process effects and decals.",
    steps: MATERIALS_STEPS,
};

const MATERIALS_STEPS: &[Step] = &[
    Step {
        kind: StepKind::SelectAny,
        badge: "cursor-click",
        title: "Pick a mesh",
        body: "Select any object with a mesh — the material editor follows your selection, and shows one tab per material in the selected object's subtree.",
        hint: Hint { icons: &["cursor-click"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::OpenPanel("material_graph"),
        badge: "graph",
        title: "Open the node graph",
        body: "Add the *Material Graph* panel from a tab bar's + picker. This is where a material is actually built: nodes in, one Output node at the end.",
        hint: Hint { icons: &["mouse-left-click", "plus"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::Info,
        badge: "plus-circle",
        title: "Add a node",
        body: "Press SPACE (or right-click the canvas) for the node palette, and drop in a Color or Texture node. Drag from its output socket to the Output node's Base Color to wire it up.",
        hint: Hint { icons: &["keyboard", "cursor-click"], keys: &["Space"], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::OpenPanel("material_preview"),
        badge: "sphere",
        title: "Preview it",
        body: "Add the *Material Preview* panel to see the compiled result on a lit sphere against an HDRI, without hunting for the object in your scene.",
        hint: Hint { icons: &["mouse-left-click", "sphere"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::Info,
        badge: "floppy-disk",
        title: "Save the material",
        body: "Save the graph to a .material file in your project. Anything else in the scene can then point at it, and edits flow through to every user at once.",
        hint: NO_HINT,
    },
];

// ── 5. Your Workspace ───────────────────────────────────────────────────────

const WORKSPACE: Chapter = Chapter {
    id: "workspace",
    icon: "layout",
    title: "Your Workspace",
    summary: "Bend the editor's layout, toolbars and shortcuts around how you work",
    outro: "The layout is yours: panels, workspaces, toolbar order and key bindings \
            all persist with the project.",
    steps: WORKSPACE_STEPS,
};

const WORKSPACE_STEPS: &[Step] = &[
    Step {
        kind: StepKind::SwitchLayout,
        badge: "squares-four",
        title: "Workspaces",
        body: "Each tab in the highlighted bar is a whole saved layout. Scene, Blueprints, Scripting… switch between them and the panels rearrange for that kind of work.",
        hint: Hint { icons: &["mouse-left-click", "squares-four"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::OpenPanel("problems"),
        badge: "warning",
        title: "Dock a panel",
        body: "Every panel lives in the + picker on a tab bar. Add the *Problems* panel — it collects script and asset errors as they happen.",
        hint: Hint { icons: &["mouse-left-click", "plus"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::ReorderToolbar,
        badge: "dots-six-vertical",
        title: "Rearrange the toolbar",
        body: "The viewport's toolbar groups are draggable. Grab one — the tools, the snap controls — and drop it somewhere else in the strip. The order is saved with your project.",
        hint: Hint { icons: &["mouse-left-click"], keys: &["drag"], anim: HintAnim::Drag },
    },
    Step {
        kind: StepKind::Info,
        badge: "keyboard",
        title: "Rebind anything",
        body: "Settings → Shortcuts lists every editor action with its key. Click one and press a new combination to rebind it — including Home, which you used to reset the camera.",
        hint: Hint { icons: &["keyboard"], keys: &["Ctrl", ","], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::ChangeTheme,
        badge: "palette",
        title: "Re-skin it",
        body: "The theme menu in the status bar re-skins the whole editor instantly. Themes are folders you can edit — or install more from the marketplace.",
        hint: Hint { icons: &["palette"], keys: &[], anim: HintAnim::Pulse },
    },
];

// ── 6. The Marketplace ──────────────────────────────────────────────────────

const MARKETPLACE: Chapter = Chapter {
    id: "marketplace",
    icon: "storefront",
    title: "The Marketplace",
    summary: "Find, install and use assets from renzora.com without leaving the editor",
    outro: "Anything you install lands in your project like a file you'd imported \
            yourself — and you can publish your own work back the same way.",
    steps: MARKETPLACE_STEPS,
};

const MARKETPLACE_STEPS: &[Step] = &[
    Step {
        kind: StepKind::OpenPanel("hub_store"),
        badge: "storefront",
        title: "Open the Store",
        body: "Add the *Store* panel from a tab bar's + picker. It's the renzora.com marketplace, in a dock panel — models, materials, themes and plugins.",
        hint: Hint { icons: &["mouse-left-click", "storefront"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::Info,
        badge: "user-circle",
        title: "Sign in (optional)",
        body: "Browsing is open to everyone. Sign in from the account row at the top of the ☰ menu if you want your purchases, wallet and library to follow you between machines.",
        hint: NO_HINT,
    },
    Step {
        kind: StepKind::InstallAsset,
        badge: "download-simple",
        title: "Install something free",
        body: "Find a free asset and click *Get*. Pick a destination folder in your project when it asks, and the download lands there.",
        hint: Hint { icons: &["download-simple"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::OpenPanel("hub_library"),
        badge: "books",
        title: "Your library",
        body: "The *Library* panel lists everything you own, so you can re-install it into any project later without paying twice.",
        hint: Hint { icons: &["mouse-left-click", "books"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::Info,
        badge: "upload-simple",
        title: "Publish your own",
        body: "The *Publish* panel uploads your own models, materials and plugins to the marketplace — the same wizard as the website, without leaving the editor.",
        hint: NO_HINT,
    },
];

// ── 7. Play Mode ────────────────────────────────────────────────────────────

const PLAY_MODE: Chapter = Chapter {
    id: "play-mode",
    icon: "play",
    title: "Play Mode",
    summary: "Test your game in the viewport, in its own window, or in a headset",
    outro: "Play, Simulate and Stop are the whole test loop. When you're ready to ship, \
            Export builds the same thing as a standalone binary.",
    steps: PLAY_MODE_STEPS,
};

const PLAY_MODE_STEPS: &[Step] = &[
    Step {
        kind: StepKind::Info,
        badge: "play",
        title: "Play vs Simulate",
        body: "*Play* runs your game for real: the game camera takes over and the editor chrome hides. *Simulate* ticks scripts and physics while the editor stays live, so you can keep selecting and inspecting things as they move.",
        hint: NO_HINT,
    },
    Step {
        kind: StepKind::Play,
        badge: "play",
        title: "Press Play",
        body: "Hit *Play* in the top bar. You need a camera in the scene to play through — add one from *Add Entity* if the button looks dimmed.",
        hint: Hint { icons: &["mouse-left-click", "play"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::StopPlay,
        badge: "stop",
        title: "And Stop",
        body: "*Stop* returns you to editing and restores your layout. Nothing that happened during play is kept — the scene goes back exactly as it was.",
        hint: Hint { icons: &["stop"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::Info,
        badge: "monitor",
        title: "Where it plays",
        body: "The caret next to *Play* chooses the target: Viewport (in the panel), Window (its own OS window, using your project's window settings), VR (an OpenXR headset), or *Simulate*.",
        hint: Hint { icons: &["monitor"], keys: &[], anim: HintAnim::Pulse },
    },
    Step {
        kind: StepKind::Info,
        badge: "package",
        title: "Shipping it",
        body: "When the game is ready, *Export* builds a standalone binary with only the engine features your project actually uses — the editor never ships with it.",
        hint: NO_HINT,
    },
];
