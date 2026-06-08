//! The tutorial state machine: launch, per-step completion detection, advance +
//! celebrate, and teardown.
//!
//! Detection is delta-based off a per-step [`Baseline`] captured when the step
//! begins, so a step completes only on a *new* action by the user (e.g. the
//! camera angle changed *from where it was when this step started*), never
//! because some condition happened to already be true. `EditorSelection` and
//! `OrbitCameraState` expose no change events, so every signal here is polled.

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use renzora::core::viewport_types::{ViewportSettings, ViewportView};
use renzora::core::{CurrentProject, TutorialRequested};
use renzora_camera::OrbitCameraState;
use renzora_code_editor::CodeEditorState;
use renzora_editor_framework::EditorSelection;
use renzora_ember::dock::{Dock, DockDirty, DockTree};
use renzora_ember::font::EmberFonts;
use renzora_theme::ThemeManager;

use crate::demo_panel::DEMO_PANEL_ID;
use crate::overlay_ui::{self, TutorialFinishButton, TutorialProgressFill, TutorialSkipButton};
use crate::steps::{StepKind, STEPS};
use crate::{confetti, demo, persistence};

// Completion thresholds — small enough to feel responsive, large enough to
// ignore sub-pixel jitter.
const ANGLE_EPS: f32 = 0.06; // ~3.4° of orbit/look
const MOVE_EPS: f32 = 0.08; // world units the target mesh slid

/// The tutorial's whole runtime state. `current == STEPS.len()` is the final
/// "complete" card.
#[derive(Resource, Default)]
pub struct TutorialState {
    pub active: bool,
    pub want_start: bool,
    pub current: usize,
    pub root: Option<Entity>,
    pub body: Option<Entity>,
    pub fill: Option<Entity>,
    pub demo_cube: Option<Entity>,
    pub confetti_root: Option<Entity>,
    pub highlight_box: Option<Entity>,
    pub baseline: Baseline,
    pub needs_body_rebuild: bool,
    pub fire_confetti: bool,
}

/// Snapshot of the world taken when a step begins, so detection measures a delta.
#[derive(Default, Clone)]
pub struct Baseline {
    pub orbit: OrbitCameraState,
    pub selection_len: usize,
    pub cube_pos: Vec3,
    // Editor-shell steps (set when each step begins; only the current step's
    // field is consulted — see `detect_and_advance`).
    pub panel_set: Vec<String>,                       // SwitchLayout (panels in the dock tree)
    pub demo_neighbors: Option<Vec<String>>,          // ReorderPanel (tabs of the leaf holding the demo panel)
    pub move_speed: f32,                               // CameraSpeed
    pub env_count: usize,                              // AddEnvironment ("World Environment" entity count)
    pub theme_name: String,                           // ChangeTheme
    pub asset_glb_count: usize,                        // ImportModel
    pub open_files_len: usize,                         // CreateScript
    pub in_ui_view: bool,                             // CreateUi
}

/// Per-frame camera-gesture probe. The editor camera's scroll-zoom moves
/// `focus` (not `distance`) unless pivot-lock is on, and `focus` is also moved by
/// panning and flying — so the *resulting* orbit fields can't tell the gestures
/// apart. We instead read the raw input the way the camera controller does
/// (`renzora_camera/src/lib.rs:733-799`): any wheel notch = zoom, RMB + WASD =
/// fly. Refreshed every frame by [`probe_cam_input`].
#[derive(Resource, Default)]
pub struct CamInput {
    pub zoomed: bool,
    pub flew: bool,
}

/// Drain this frame's wheel events / movement keys into [`CamInput`]. Runs every
/// frame (its own `MessageReader` cursor, independent of the camera's).
pub fn probe_cam_input(
    mut cam: ResMut<CamInput>,
    mut wheel: MessageReader<MouseWheel>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    cam.zoomed = wheel.read().any(|e| e.y.abs() > 0.0);
    let wasd = keys.pressed(KeyCode::KeyW)
        || keys.pressed(KeyCode::KeyA)
        || keys.pressed(KeyCode::KeyS)
        || keys.pressed(KeyCode::KeyD);
    cam.flew = mouse.pressed(MouseButton::Right) && wasd;
}

/// Launch the tutorial from either trigger: the Help-menu / command-palette
/// `TutorialRequested` marker (manual, any time) or an auto first-run on a
/// project that has never completed it. Waits until ember fonts exist before
/// building UI.
#[allow(clippy::too_many_arguments)]
pub fn trigger(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    req: Option<Res<TutorialRequested>>,
    project: Option<Res<CurrentProject>>,
    orbit: Option<Res<OrbitCameraState>>,
    selection: Option<Res<EditorSelection>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut state: ResMut<TutorialState>,
    mut autostart_checked: Local<bool>,
) {
    // Manual trigger (Help → Getting Started Tutorial). Consume the marker.
    if req.is_some() {
        commands.remove_resource::<TutorialRequested>();
        if !state.active {
            state.want_start = true;
        }
    }

    // Auto first-run, evaluated once a project is loaded.
    if !*autostart_checked {
        if let Some(p) = project.as_ref() {
            *autostart_checked = true;
            if persistence::is_first_run(p) {
                state.want_start = true;
            }
        }
    }

    if !state.want_start || state.active {
        return;
    }
    let Some(fonts) = fonts.as_ref() else {
        return; // UI not ready yet — retry next frame.
    };

    state.want_start = false;
    state.active = true;
    state.current = 0;

    let cube = demo::spawn_demo_cube(&mut commands, &mut meshes, &mut materials);
    state.demo_cube = Some(cube);

    let ov = overlay_ui::build_overlay(&mut commands, fonts);
    state.root = Some(ov.root);
    state.body = Some(ov.body);
    state.fill = Some(ov.fill);

    state.confetti_root = Some(confetti::spawn_root(&mut commands));
    state.highlight_box = Some(crate::highlight::spawn_box(&mut commands));

    // Step 0 is Orbit, which only consults `orbit`; the editor-shell baselines are
    // captured fresh when those later steps begin (the recapture block below).
    state.baseline = Baseline {
        orbit: orbit.as_deref().cloned().unwrap_or_default(),
        selection_len: selection.as_ref().map(|s| s.get_all().len()).unwrap_or(0),
        cube_pos: demo::DEMO_CUBE_POS,
        ..default()
    };
    state.needs_body_rebuild = true; // build step 0's body this frame
}

/// Poll the current step's completion signal; on success, celebrate (confetti)
/// and advance to the next step, recapturing the baseline.
#[allow(clippy::too_many_arguments)]
pub fn detect_and_advance(
    mut state: ResMut<TutorialState>,
    orbit: Option<Res<OrbitCameraState>>,
    selection: Option<Res<EditorSelection>>,
    cam: Res<CamInput>,
    dock: Option<Res<Dock>>,
    viewport: Option<Res<ViewportSettings>>,
    theme: Option<Res<ThemeManager>>,
    code: Option<Res<CodeEditorState>>,
    project: Option<Res<CurrentProject>>,
    transforms: Query<&Transform>,
    names: Query<&Name>,
) {
    if !state.active || state.current >= STEPS.len() {
        return;
    }
    let Some(orbit) = orbit.as_deref() else {
        return;
    };

    let done = {
        let b = &state.baseline;
        match STEPS[state.current].kind {
            StepKind::Orbit => {
                ang_delta(orbit.yaw, b.orbit.yaw) > ANGLE_EPS
                    || ang_delta(orbit.pitch, b.orbit.pitch) > ANGLE_EPS
            }
            // Input-driven (see `CamInput`): the orbit fields can't disambiguate
            // zoom/pan/fly, which all move `focus`.
            StepKind::Zoom => cam.zoomed,
            StepKind::Fly => cam.flew,
            StepKind::Select => state
                .demo_cube
                .map(|c| selection.as_ref().is_some_and(|s| s.is_selected(c)))
                .unwrap_or(false),
            StepKind::Move => state
                .demo_cube
                .and_then(|c| transforms.get(c).ok())
                .map(|t| t.translation.distance(b.cube_pos) > MOVE_EPS)
                .unwrap_or(false),
            // The set of panels in the dock tree changed — i.e. a different
            // workspace layout loaded. (A plain tab switch keeps the same set, so
            // it won't satisfy this; that's the AddPanel/ReorderPanel steps' job.)
            StepKind::SwitchLayout => dock
                .as_ref()
                .map(|d| panel_set(&d.tree) != b.panel_set)
                .unwrap_or(false),
            // The Demo panel now exists somewhere in the dock (the user added it).
            StepKind::AddPanel => dock
                .as_ref()
                .map(|d| tree_contains(&d.tree, DEMO_PANEL_ID))
                .unwrap_or(false),
            // The Demo panel's leaf neighbours changed — it was dragged to a new
            // leaf or reordered. (Excludes mere tab-switching, which leaves the
            // leaf's tab list untouched.)
            StepKind::ReorderPanel => dock
                .as_ref()
                .map(|d| leaf_tabs_of(&d.tree, DEMO_PANEL_ID) != b.demo_neighbors)
                .unwrap_or(false),
            // The fly move-speed slider moved.
            StepKind::CameraSpeed => viewport
                .as_ref()
                .map(|vp| (vp.camera.move_speed - b.move_speed).abs() > 0.5)
                .unwrap_or(false),
            // One more "World Environment" entity exists than at step start (the
            // user added one — robust even if the scene already had one).
            StepKind::AddEnvironment => {
                names.iter().filter(|n| n.as_str() == "World Environment").count() > b.env_count
            }
            // The active theme name changed.
            StepKind::ChangeTheme => theme
                .as_ref()
                .map(|t| t.active_theme_name != b.theme_name)
                .unwrap_or(false),
            // A new model file landed under <project>/assets.
            StepKind::ImportModel => project
                .as_ref()
                .map(|p| count_models(p) > b.asset_glb_count)
                .unwrap_or(false),
            // A new script tab opened in the code editor.
            StepKind::CreateScript => code
                .as_ref()
                .map(|c| c.open_files.len() > b.open_files_len && active_is_script(c))
                .unwrap_or(false),
            // The viewport switched into the UI authoring view.
            StepKind::CreateUi => viewport
                .as_ref()
                .map(|vp| matches!(vp.viewport_view, ViewportView::Ui) && !b.in_ui_view)
                .unwrap_or(false),
        }
    };
    if !done {
        return;
    }

    state.fire_confetti = true;
    state.current += 1;
    state.needs_body_rebuild = true;

    // Reset the baseline to "right now" so the next step measures a fresh delta.
    if state.current < STEPS.len() {
        let cube_pos = state
            .demo_cube
            .and_then(|c| transforms.get(c).ok())
            .map(|t| t.translation)
            .unwrap_or(state.baseline.cube_pos);
        state.baseline.orbit = orbit.clone();
        state.baseline.selection_len = selection.as_ref().map(|s| s.get_all().len()).unwrap_or(0);
        state.baseline.cube_pos = cube_pos;
        state.baseline.panel_set = dock.as_ref().map(|d| panel_set(&d.tree)).unwrap_or_default();
        state.baseline.demo_neighbors =
            dock.as_ref().and_then(|d| leaf_tabs_of(&d.tree, DEMO_PANEL_ID));
        if let Some(vp) = viewport.as_ref() {
            state.baseline.move_speed = vp.camera.move_speed;
            state.baseline.in_ui_view = matches!(vp.viewport_view, ViewportView::Ui);
        }
        state.baseline.env_count =
            names.iter().filter(|n| n.as_str() == "World Environment").count();
        state.baseline.theme_name =
            theme.as_ref().map(|t| t.active_theme_name.clone()).unwrap_or_default();
        state.baseline.asset_glb_count = project.as_ref().map(|p| count_models(p)).unwrap_or(0);
        state.baseline.open_files_len = code.as_ref().map(|c| c.open_files.len()).unwrap_or(0);
    }
}

/// Rebuild the card body whenever the step changed, and resize the progress bar.
pub fn rebuild_body(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    mut state: ResMut<TutorialState>,
    children: Query<&Children>,
    mut fills: Query<&mut Node, With<TutorialProgressFill>>,
) {
    if !state.needs_body_rebuild {
        return;
    }
    let Some(fonts) = fonts.as_ref() else {
        return;
    };
    let Some(body) = state.body else {
        return;
    };
    state.needs_body_rebuild = false;

    if let Ok(kids) = children.get(body) {
        for c in kids.iter() {
            commands.entity(c).despawn();
        }
    }
    overlay_ui::build_step_body(&mut commands, fonts, body, state.current, STEPS.len());

    if let Some(fill) = state.fill {
        if let Ok(mut node) = fills.get_mut(fill) {
            let frac = (state.current as f32 / STEPS.len() as f32).clamp(0.0, 1.0);
            node.width = Val::Percent(frac * 100.0);
        }
    }
}

/// Spawn a confetti burst when a step is completed.
pub fn fire_confetti(
    mut commands: Commands,
    time: Res<Time>,
    mut state: ResMut<TutorialState>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    if !state.fire_confetti {
        return;
    }
    state.fire_confetti = false;
    let Some(root) = state.confetti_root else {
        return;
    };
    let (w, h) = windows
        .single()
        .map(|win| (win.width(), win.height()))
        .unwrap_or((1280.0, 720.0));
    // Originate just above the bottom-right card.
    let origin = Vec2::new(w - 200.0, h - 230.0);
    let seed = time.elapsed_secs().to_bits() ^ (state.current as u32).wrapping_mul(0x9E37_79B1);
    confetti::burst(&mut commands, root, origin, 36, seed);
}

/// Skip (any time) or Finish (on the completion card) ends the tutorial: persist
/// completion so it won't auto-launch again, then tear everything down.
#[allow(clippy::too_many_arguments)]
pub fn handle_buttons(
    mut commands: Commands,
    mut state: ResMut<TutorialState>,
    project: Option<ResMut<CurrentProject>>,
    dock: Option<ResMut<Dock>>,
    dock_dirty: Option<ResMut<DockDirty>>,
    skip: Query<&Interaction, (Changed<Interaction>, With<TutorialSkipButton>)>,
    finish: Query<&Interaction, (Changed<Interaction>, With<TutorialFinishButton>)>,
) {
    if !state.active {
        return;
    }
    let pressed = skip
        .iter()
        .chain(finish.iter())
        .any(|i| *i == Interaction::Pressed);
    if !pressed {
        return;
    }

    if let Some(mut p) = project {
        persistence::mark_completed(&mut p);
    }
    // Drop the throwaway demo panel so it doesn't linger in the (persisted) layout.
    if let Some(mut dock) = dock {
        if dock.tree.remove_panel(DEMO_PANEL_ID) {
            if let Some(mut dirty) = dock_dirty {
                dirty.0 = true;
            }
        }
    }
    for e in [
        state.root,
        state.demo_cube,
        state.confetti_root,
        state.highlight_box,
    ]
    .into_iter()
    .flatten()
    {
        commands.entity(e).despawn();
    }
    *state = TutorialState::default();
}

/// The sorted, de-duped set of panel ids present anywhere in a dock tree. Used to
/// tell a *workspace switch* (the whole panel set changes) apart from a mere tab
/// switch or re-dock (same set).
fn panel_set(tree: &DockTree) -> Vec<String> {
    fn collect(t: &DockTree, out: &mut Vec<String>) {
        match t {
            DockTree::Split { first, second, .. } => {
                collect(first, out);
                collect(second, out);
            }
            DockTree::Leaf { tabs, .. } => out.extend(tabs.iter().cloned()),
            DockTree::Empty => {}
        }
    }
    let mut v = Vec::new();
    collect(tree, &mut v);
    v.sort();
    v.dedup();
    v
}

/// Is `id` a tab anywhere in the dock tree?
fn tree_contains(tree: &DockTree, id: &str) -> bool {
    match tree {
        DockTree::Split { first, second, .. } => {
            tree_contains(first, id) || tree_contains(second, id)
        }
        DockTree::Leaf { tabs, .. } => tabs.iter().any(|t| t == id),
        DockTree::Empty => false,
    }
}

/// The ordered tab list of the leaf that contains `id` (its "neighbours"), or
/// `None` if `id` isn't docked. Changes when the panel is moved to a different
/// leaf or reordered within its leaf, but not on a plain tab switch.
fn leaf_tabs_of(tree: &DockTree, id: &str) -> Option<Vec<String>> {
    match tree {
        DockTree::Split { first, second, .. } => {
            leaf_tabs_of(first, id).or_else(|| leaf_tabs_of(second, id))
        }
        DockTree::Leaf { tabs, .. } => tabs.iter().any(|t| t == id).then(|| tabs.clone()),
        DockTree::Empty => None,
    }
}

/// Shortest absolute angular distance between two angles (radians).
fn ang_delta(a: f32, b: f32) -> f32 {
    let d = (a - b).abs() % std::f32::consts::TAU;
    d.min(std::f32::consts::TAU - d)
}

/// Whether the code editor's active tab is a Lua/Rhai script.
fn active_is_script(c: &CodeEditorState) -> bool {
    c.active_tab
        .and_then(|i| c.open_files.get(i))
        .and_then(|f| f.path.extension())
        .and_then(|e| e.to_str())
        .map(|e| {
            let e = e.to_ascii_lowercase();
            e == "lua" || e == "rhai"
        })
        .unwrap_or(false)
}

/// Count model files under `<project>/assets` (depth-capped). Lets the import
/// step detect "a new model landed on disk" without reaching the import UI's
/// private progress state — and it also catches the silent drag-drop import path.
fn count_models(project: &CurrentProject) -> usize {
    // Same model extensions the asset browser recognises (renzora_asset_registry's
    // `AssetKind::Model`). Scanned from the project root (not just `assets/`) so an
    // import is caught wherever it lands; heavy build/vcs dirs are skipped.
    const MODEL_EXTS: &[&str] = &[
        "glb", "gltf", "obj", "fbx", "usd", "usda", "usdc", "usdz", "abc", "dae", "blend",
    ];
    const SKIP_DIRS: &[&str] = &[".git", "target", "node_modules", ".import", "cache"];
    fn walk(dir: &std::path::Path, depth: u8, n: &mut usize) {
        if depth == 0 {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let skip = path
                    .file_name()
                    .and_then(|f| f.to_str())
                    .is_some_and(|f| SKIP_DIRS.contains(&f));
                if !skip {
                    walk(&path, depth - 1, n);
                }
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if MODEL_EXTS.contains(&ext.to_ascii_lowercase().as_str()) {
                    *n += 1;
                }
            }
        }
    }
    let mut n = 0;
    walk(&project.path, 5, &mut n);
    n
}
