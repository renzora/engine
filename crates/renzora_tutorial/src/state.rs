//! The tutorial state machine: chapter selection, launch, per-step completion
//! detection, advance + celebrate, and teardown.
//!
//! Detection is delta-based off a per-step [`Baseline`] captured when the step
//! begins, so a step completes only on a *new* action by the user (e.g. the
//! camera angle changed *from where it was when this step started*), never
//! because some condition happened to already be true. `EditorSelection` and
//! `OrbitCameraState` expose no change events, so every signal here is polled.
//!
//! **Detection arms a step; it does not advance it.** [`detect_step_done`] sets
//! `step_done` and fires the confetti, then the card shows a green Continue
//! button and [`handle_continue`] does the actual advance. Auto-advancing used
//! to yank the card out from under people mid-read the instant they completed
//! the gesture — and it left no room for [`StepKind::Info`] steps, which have
//! nothing to detect at all.

use bevy::ecs::system::SystemParam;
use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::PrimaryWindow;

use renzora::core::keybindings::{EditorAction, KeyBindings};
use renzora::core::viewport_types::ViewportSettings;
use renzora::core::{CurrentProject, PlayModeState, TutorialRequested};
use renzora::WorldEnvironment;
use renzora_camera::OrbitCameraState;
use renzora_code_editor::CodeEditorState;
use renzora_editor_framework::EditorSelection;
use renzora_ember::dock::{Dock, DockDirty, DockTree};
use renzora_ember::font::EmberFonts;
use renzora_theme::ThemeManager;

use crate::demo_panel::DEMO_PANEL_ID;
use crate::overlay_ui::{
    self, TutorialCardTitle, TutorialCloseButton, TutorialContinueButton, TutorialChapterButton,
    TutorialFinishButton, TutorialProgressFill, TutorialSkipButton,
};
use crate::steps::{StepKind, CHAPTERS};
use crate::{confetti, demo, persistence};

// Completion thresholds — small enough to feel responsive, large enough to
// ignore sub-pixel jitter.
const ANGLE_EPS: f32 = 0.06; // ~3.4° of orbit/look
const MOVE_EPS: f32 = 0.08; // world units the target mesh slid
const SCALE_EPS: f32 = 0.05; // relative change in the mesh's scale
const SPIN_EPS: f32 = 0.12; // radians the mesh turned under a script

/// The tutorial's whole runtime state. `current == steps().len()` is the final
/// "chapter complete" card; `show_picker` replaces the body with the chapter list.
#[derive(Resource, Default)]
pub struct TutorialState {
    pub active: bool,
    pub want_start: bool,
    /// Showing the chapter list instead of a step.
    pub show_picker: bool,
    /// Index into [`CHAPTERS`].
    pub chapter: usize,
    pub current: usize,
    /// The current step's action has been performed — Continue is now offered.
    pub step_done: bool,
    pub root: Option<Entity>,
    pub body: Option<Entity>,
    pub fill: Option<Entity>,
    pub demo_cube: Option<Entity>,
    pub confetti_root: Option<Entity>,
    pub highlight_box: Option<Entity>,
    pub highlight_arrow: Option<Entity>,
    pub baseline: Baseline,
    pub needs_body_rebuild: bool,
    pub fire_confetti: bool,
}

impl TutorialState {
    /// The running chapter's steps (empty if the picker is up).
    pub fn steps(&self) -> &'static [crate::steps::Step] {
        CHAPTERS.get(self.chapter).map(|c| c.steps).unwrap_or(&[])
    }

    /// The step being shown, or `None` on the picker / completion card.
    pub fn step(&self) -> Option<&'static crate::steps::Step> {
        self.steps().get(self.current)
    }
}

/// Snapshot of the world taken when a step begins, so detection measures a delta.
#[derive(Default, Clone)]
pub struct Baseline {
    pub orbit: OrbitCameraState,
    pub cube_pos: Vec3,
    pub cube_scale: Vec3,
    /// Order-independent sum of every entity's rotation components. The
    /// "a script moved something" signal can't key off the demo cube — the user
    /// attaches their script to an entity of their own — so this is deliberately
    /// scene-wide: if anything at all turned while the simulation ran, the sum
    /// moved.
    pub rot_sum: f32,
    // Editor-shell baselines (only the current step's field is consulted — see
    // `detect_step_done`).
    pub panel_set: Vec<String>,              // SwitchLayout (panels in the dock tree)
    pub demo_neighbors: Option<Vec<String>>, // ReorderPanel (tabs of the leaf holding the demo panel)
    pub move_speed: f32,                     // CameraSpeed
    pub toolbar_order: Vec<String>,          // ReorderToolbar
    /// (translate, rotate, scale) snap switches — ToggleSnap.
    pub snap: (bool, bool, bool),
    pub env_count: usize,                    // AddEnvironment (entities carrying `WorldEnvironment`)
    pub mesh_count: usize,                   // AddShape
    pub light_count: usize,                  // AddLight
    pub entity_count: usize,                 // Duplicate / Delete
    pub theme_name: String,                  // ChangeTheme
    pub asset_model_count: usize,            // ImportModel
    pub asset_file_count: usize,             // InstallAsset
    pub script_count: usize,                 // ScriptFile
    pub open_files_len: usize,               // CreateScript
    pub ui_panel_open: bool,                 // CreateUi
    pub was_playing: bool,                   // Play / Simulate / StopPlay
}

/// Any light kind, for the "you added a light" signal — a named filter because
/// the inline `Or<(..)>` trips clippy's `type_complexity`, which CI denies.
type AnyLight = Or<(With<PointLight>, With<SpotLight>, With<DirectionalLight>)>;

/// Everything [`detect_step_done`] polls, bundled so the system stays under
/// Bevy's parameter cap — there are more signals than a plain argument list can
/// carry, and they're all read-only.
#[derive(SystemParam)]
pub struct Signals<'w, 's> {
    pub orbit: Option<Res<'w, OrbitCameraState>>,
    pub selection: Option<Res<'w, EditorSelection>>,
    pub cam: Res<'w, CamInput>,
    pub dock: Option<Res<'w, Dock>>,
    pub viewport: Option<Res<'w, ViewportSettings>>,
    pub theme: Option<Res<'w, ThemeManager>>,
    pub code: Option<Res<'w, CodeEditorState>>,
    pub project: Option<Res<'w, CurrentProject>>,
    pub play: Option<Res<'w, PlayModeState>>,
    pub keyboard: Res<'w, ButtonInput<KeyCode>>,
    pub bindings: Option<Res<'w, KeyBindings>>,
    pub transforms: Query<'w, 's, &'static Transform>,
    pub envs: Query<'w, 's, (), With<WorldEnvironment>>,
    pub named_all: Query<'w, 's, &'static Name>,
    pub meshes: Query<'w, 's, (), With<Mesh3d>>,
    pub lights: Query<'w, 's, (), AnyLight>,
    pub named: Query<'w, 's, (), With<Name>>,
}

impl Signals<'_, '_> {
    /// How many world-environment entities exist.
    ///
    /// Counts the [`WorldEnvironment`] component **or** the preset's entity id,
    /// because neither alone has proved reliable. The display name definitely
    /// isn't it — `SpawnEntityCmd` overwrites the spawn_fn's "World Environment"
    /// with `unique_entity_name(world, preset.id, ..)`, i.e. `world_environment`
    /// (issue #83). The component should be enough, since the preset attaches it
    /// — but it's the one signal here we can't verify from this crate, and a
    /// tutorial step that can never complete is a dead end for the user, so the
    /// id check backs it up. `_1`, `_2`, … suffixes make it a prefix match.
    fn env_count(&self) -> usize {
        let by_component = self.envs.iter().count();
        let by_id = self
            .named_all
            .iter()
            .filter(|n| {
                let n = n.as_str();
                n == "world_environment" || n.starts_with("world_environment_")
            })
            .count();
        by_component.max(by_id)
    }

    /// Capture "the world as it is right now" for the step about to start.
    fn snapshot(&self, demo_cube: Option<Entity>, prev: &Baseline) -> Baseline {
        let cube = demo_cube.and_then(|c| self.transforms.get(c).ok());
        Baseline {
            orbit: self.orbit.as_deref().cloned().unwrap_or_default(),
            cube_pos: cube.map(|t| t.translation).unwrap_or(prev.cube_pos),
            cube_scale: cube.map(|t| t.scale).unwrap_or(Vec3::ONE),
            rot_sum: rot_sum(&self.transforms),
            panel_set: self.dock.as_ref().map(|d| panel_set(&d.tree)).unwrap_or_default(),
            demo_neighbors: self
                .dock
                .as_ref()
                .and_then(|d| leaf_tabs_of(&d.tree, DEMO_PANEL_ID)),
            move_speed: self.viewport.as_ref().map(|v| v.camera.move_speed).unwrap_or(0.0),
            toolbar_order: self
                .viewport
                .as_ref()
                .map(|v| v.toolbar_order.clone())
                .unwrap_or_default(),
            snap: self
                .viewport
                .as_ref()
                .map(|v| {
                    (
                        v.snap.translate_enabled,
                        v.snap.rotate_enabled,
                        v.snap.scale_enabled,
                    )
                })
                .unwrap_or_default(),
            env_count: self.env_count(),
            mesh_count: self.meshes.iter().count(),
            light_count: self.lights.iter().count(),
            entity_count: self.named.iter().count(),
            theme_name: self
                .theme
                .as_ref()
                .map(|t| t.active_theme_name.clone())
                .unwrap_or_default(),
            asset_model_count: self.project.as_ref().map(|p| count_models(p)).unwrap_or(0),
            asset_file_count: self.project.as_ref().map(|p| count_assets(p)).unwrap_or(0),
            script_count: self.project.as_ref().map(|p| count_scripts(p)).unwrap_or(0),
            open_files_len: self.code.as_ref().map(|c| c.open_files.len()).unwrap_or(0),
            // The UI editor used to be a *view* of the viewport, so this asked
            // the viewport what it was showing. It is the `ui_canvas` panel now,
            // so the question is whether that panel is in the dock.
            ui_panel_open: self
                .dock
                .as_ref()
                .is_some_and(|d| tree_contains(&d.tree, "ui_canvas")),
            was_playing: self
                .play
                .as_ref()
                .is_some_and(|p| p.is_in_play_mode() || p.is_simulating()),
        }
    }
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
/// `TutorialRequested` marker (manual, any time) or an auto first run for a user
/// who has never seen it — once per install, not once per project. Waits until
/// ember fonts exist before building UI.
///
/// The two triggers land in different places: a first run drops straight into
/// Getting Started (a brand-new user shouldn't have to choose a chapter before
/// they've seen the editor), while a manual re-run opens the chapter picker.
#[allow(clippy::too_many_arguments)]
pub fn trigger(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    req: Option<Res<TutorialRequested>>,
    project: Option<Res<CurrentProject>>,
    signals: Signals,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut state: ResMut<TutorialState>,
    mut autostart_checked: Local<bool>,
    mut from_menu: Local<bool>,
) {
    // Manual trigger (Help → Getting Started Tutorial). Consume the marker.
    if req.is_some() {
        commands.remove_resource::<TutorialRequested>();
        if !state.active {
            state.want_start = true;
            *from_menu = true;
        }
    }

    // Auto first-run. Still waits for a project to load — the tutorial's steps
    // act on an open project, and it's also our only chance to migrate a legacy
    // per-project completion flag before answering.
    if !*autostart_checked {
        if let Some(p) = project.as_ref() {
            *autostart_checked = true;
            if persistence::is_first_run(p) {
                state.want_start = true;
                *from_menu = false;
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
    state.chapter = 0;
    state.current = 0;
    state.step_done = false;
    state.show_picker = *from_menu;

    let cube = demo::spawn_demo_cube(&mut commands, &mut meshes, &mut materials);
    state.demo_cube = Some(cube);

    let ov = overlay_ui::build_overlay(&mut commands, fonts);
    state.root = Some(ov.root);
    state.body = Some(ov.body);
    state.fill = Some(ov.fill);

    state.confetti_root = Some(confetti::spawn_root(&mut commands));
    state.highlight_box = Some(crate::highlight::spawn_box(&mut commands));
    state.highlight_arrow = Some(crate::highlight::spawn_arrow(&mut commands));

    state.baseline = signals.snapshot(Some(cube), &Baseline {
        cube_pos: demo::DEMO_CUBE_POS,
        ..default()
    });
    state.needs_body_rebuild = true; // build the picker / step 0 this frame
}

/// Poll the current step's completion signal; on success, celebrate (confetti)
/// and arm the Continue button. Does **not** advance — see the module docs.
pub fn detect_step_done(mut state: ResMut<TutorialState>, signals: Signals) {
    if !state.active || state.show_picker || state.step_done {
        return;
    }
    let Some(step) = state.step() else {
        return; // on the completion card
    };
    let b = &state.baseline;
    let cube = state.demo_cube.and_then(|c| signals.transforms.get(c).ok());
    let playing = signals
        .play
        .as_ref()
        .is_some_and(|p| p.is_in_play_mode() || p.is_simulating());

    let done = match step.kind {
        // Nothing to detect: Continue is offered as soon as the step appears.
        StepKind::Info => true,

        // Scoped to this arm rather than an early return for the whole system:
        // an `Info` step has nothing to do with the camera and must still arm if
        // the orbit resource somehow isn't there.
        StepKind::Orbit => signals.orbit.as_deref().is_some_and(|orbit| {
            ang_delta(orbit.yaw, b.orbit.yaw) > ANGLE_EPS
                || ang_delta(orbit.pitch, b.orbit.pitch) > ANGLE_EPS
        }),
        // Input-driven (see `CamInput`): the orbit fields can't disambiguate
        // zoom/pan/fly, which all move `focus`.
        StepKind::Zoom => signals.cam.zoomed,
        StepKind::Fly => signals.cam.flew,

        StepKind::Select => state
            .demo_cube
            .map(|c| signals.selection.as_ref().is_some_and(|s| s.is_selected(c)))
            .unwrap_or(false),
        StepKind::SelectAny => signals
            .selection
            .as_ref()
            .is_some_and(|s| !s.get_all().is_empty()),
        StepKind::Move => cube
            .map(|t| t.translation.distance(b.cube_pos) > MOVE_EPS)
            .unwrap_or(false),
        // Relative, so it reads the same whether the mesh started at 1 or 100.
        StepKind::Scale => cube
            .map(|t| (t.scale - b.cube_scale).length() / b.cube_scale.length().max(0.001) > SCALE_EPS)
            .unwrap_or(false),
        // Only counts while the simulation is actually ticking — otherwise the
        // user dragging the rotate gizmo would satisfy "your script spun it".
        StepKind::ScriptedMotion => {
            playing && (rot_sum(&signals.transforms) - b.rot_sum).abs() > SPIN_EPS
        }

        StepKind::AddShape => signals.meshes.iter().count() > b.mesh_count,
        StepKind::AddLight => signals.lights.iter().count() > b.light_count,
        // One more environment entity exists than at step start (the user added
        // one — robust even if the scene already had one).
        StepKind::AddEnvironment => signals.env_count() > b.env_count,
        StepKind::Duplicate => signals.named.iter().count() > b.entity_count,
        StepKind::Delete => signals.named.iter().count() < b.entity_count,

        // The set of panels in the dock tree changed — i.e. a different
        // workspace layout loaded. (A plain tab switch keeps the same set, so
        // it won't satisfy this; that's the AddPanel/ReorderPanel steps' job.)
        StepKind::SwitchLayout => signals
            .dock
            .as_ref()
            .map(|d| panel_set(&d.tree) != b.panel_set)
            .unwrap_or(false),
        // The Demo panel now exists somewhere in the dock (the user added it).
        StepKind::AddPanel => signals
            .dock
            .as_ref()
            .map(|d| tree_contains(&d.tree, DEMO_PANEL_ID))
            .unwrap_or(false),
        // The Demo panel's leaf neighbours changed — it was dragged to a new
        // leaf or reordered. (Excludes mere tab-switching, which leaves the
        // leaf's tab list untouched.)
        StepKind::ReorderPanel => signals
            .dock
            .as_ref()
            .map(|d| leaf_tabs_of(&d.tree, DEMO_PANEL_ID) != b.demo_neighbors)
            .unwrap_or(false),
        StepKind::OpenPanel(id) => signals
            .dock
            .as_ref()
            .map(|d| tree_contains(&d.tree, id))
            .unwrap_or(false),
        // The viewport toolbar's groups were dragged into a new arrangement
        // (`sync_toolbar_order` publishes that into `ViewportSettings`).
        StepKind::ReorderToolbar => signals
            .viewport
            .as_ref()
            .map(|v| v.toolbar_order != b.toolbar_order)
            .unwrap_or(false),
        // Any of the three gizmo snap switches flipped — the step asks for one,
        // and which one the user reaches for doesn't matter.
        StepKind::ToggleSnap => signals
            .viewport
            .as_ref()
            .map(|v| {
                (
                    v.snap.translate_enabled,
                    v.snap.rotate_enabled,
                    v.snap.scale_enabled,
                ) != b.snap
            })
            .unwrap_or(false),
        // The fly move-speed slider moved.
        StepKind::CameraSpeed => signals
            .viewport
            .as_ref()
            .map(|vp| (vp.camera.move_speed - b.move_speed).abs() > 0.5)
            .unwrap_or(false),
        // The active theme name changed.
        StepKind::ChangeTheme => signals
            .theme
            .as_ref()
            .map(|t| t.active_theme_name != b.theme_name)
            .unwrap_or(false),
        StepKind::Shortcut(action) => action_pressed(&signals, action),

        // A new model file landed under <project>/assets.
        StepKind::ImportModel => signals
            .project
            .as_ref()
            .map(|p| count_models(p) > b.asset_model_count)
            .unwrap_or(false),
        // Any new asset file at all — the marketplace install writes into the
        // project like any other download, and the hub's install state is
        // private to its crate, so the file landing IS the observable signal.
        StepKind::InstallAsset => signals
            .project
            .as_ref()
            .map(|p| count_assets(p) > b.asset_file_count)
            .unwrap_or(false),
        StepKind::ScriptFile => signals
            .project
            .as_ref()
            .map(|p| count_scripts(p) > b.script_count)
            .unwrap_or(false),
        // A new script tab opened in the code editor.
        StepKind::CreateScript => signals
            .code
            .as_ref()
            .map(|c| c.open_files.len() > b.open_files_len && active_is_script(c))
            .unwrap_or(false),

        StepKind::Simulate => signals.play.as_ref().is_some_and(|p| p.is_simulating()),
        StepKind::Play => signals.play.as_ref().is_some_and(|p| p.is_in_play_mode()),
        // Was running when the step began, isn't now.
        StepKind::StopPlay => b.was_playing && !playing,

        // The UI Canvas panel was opened.
        StepKind::CreateUi => signals
            .dock
            .as_ref()
            .map(|d| tree_contains(&d.tree, "ui_canvas") && !b.ui_panel_open)
            .unwrap_or(false),
    };

    if !done {
        return;
    }
    state.step_done = true;
    state.needs_body_rebuild = true;
    // Info steps arm instantly — no confetti for simply reading a card.
    state.fire_confetti = !matches!(step.kind, StepKind::Info);
}

/// Continue (once the step is armed) or Skip (any time) → advance to the next
/// step, recapturing the baseline so the next step measures a fresh delta.
///
/// Skip is per-*step*, not "leave the tutorial": some steps simply can't be done
/// right now — there's no model to import, no marketplace account — and losing
/// the whole chapter over one of them is a bad trade. Closing is the header's X.
pub fn handle_continue(
    mut state: ResMut<TutorialState>,
    signals: Signals,
    cont: Query<&Interaction, (Changed<Interaction>, With<TutorialContinueButton>)>,
    skip: Query<&Interaction, (Changed<Interaction>, With<TutorialSkipButton>)>,
) {
    if !state.active || state.show_picker || state.step().is_none() {
        return;
    }
    // The Continue button only exists while `step_done`, but check anyway rather
    // than trusting the UI to be the only guard.
    let advance = (state.step_done && cont.iter().any(|i| *i == Interaction::Pressed))
        || skip.iter().any(|i| *i == Interaction::Pressed);
    if !advance {
        return;
    }
    state.current += 1;
    state.step_done = false;
    state.needs_body_rebuild = true;
    if state.current < state.steps().len() {
        let (cube, prev) = (state.demo_cube, state.baseline.clone());
        state.baseline = signals.snapshot(cube, &prev);
    }
}

/// A chapter row on the picker → start that chapter.
pub fn handle_chapter_pick(
    mut state: ResMut<TutorialState>,
    signals: Signals,
    buttons: Query<(&Interaction, &TutorialChapterButton), Changed<Interaction>>,
) {
    if !state.active || !state.show_picker {
        return;
    }
    let Some(pick) = buttons
        .iter()
        .find(|(i, _)| **i == Interaction::Pressed)
        .map(|(_, c)| c.0)
    else {
        return;
    };
    // A locked row is drawn but inert — the click is swallowed here rather than
    // by omitting `Interaction`, so the row still hovers and reads as a real
    // (not-yet-available) chapter.
    let done = persistence::chapters_done(CHAPTERS.iter().map(|c| c.id));
    if !overlay_ui::is_unlocked(pick, &done) {
        return;
    }
    state.chapter = pick.min(CHAPTERS.len().saturating_sub(1));
    state.current = 0;
    state.step_done = false;
    state.show_picker = false;
    state.needs_body_rebuild = true;
    let (cube, prev) = (state.demo_cube, state.baseline.clone());
    state.baseline = signals.snapshot(cube, &prev);
}

/// Rebuild the card body whenever the step changed, and resize the progress bar.
#[allow(clippy::too_many_arguments)]
pub fn rebuild_body(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    mut state: ResMut<TutorialState>,
    children: Query<&Children>,
    mut fills: Query<&mut Node, With<TutorialProgressFill>>,
    mut titles: Query<&mut Text, With<TutorialCardTitle>>,
    mut skips: Query<&mut Node, (With<TutorialSkipButton>, Without<TutorialProgressFill>)>,
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
    let done = persistence::chapters_done(CHAPTERS.iter().map(|c| c.id));
    overlay_ui::build_body(&mut commands, fonts, body, &state, &done);

    // Skip acts on the current step, so it has nothing to do on the picker or
    // the completion card — hide it there rather than leaving a dead control.
    let show_skip = !state.show_picker && state.step().is_some();
    for mut node in &mut skips {
        let want = if show_skip { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }

    // The header names whichever chapter is running, so a user who dragged the
    // card aside still knows what they're in the middle of.
    let heading = if state.show_picker {
        "Tutorials"
    } else {
        CHAPTERS.get(state.chapter).map(|c| c.title).unwrap_or("Tutorials")
    };
    for mut text in &mut titles {
        if text.0 != heading {
            text.0 = heading.to_string();
        }
    }

    if let Some(fill) = state.fill {
        if let Ok(mut node) = fills.get_mut(fill) {
            let total = state.steps().len().max(1) as f32;
            let frac = if state.show_picker {
                0.0
            } else {
                (state.current as f32 / total).clamp(0.0, 1.0)
            };
            node.width = Val::Percent(frac * 100.0);
        }
    }
}

/// Spawn a confetti burst when a step is completed.
///
/// The burst originates from the card's **live** top edge, read from its
/// `UiGlobalTransform` each time, so it keeps erupting out of the card after the
/// user has dragged it somewhere else. (It used to be a hardcoded offset from
/// the bottom-right corner, which was only correct while the card sat where it
/// spawned.)
pub fn fire_confetti(
    mut commands: Commands,
    time: Res<Time>,
    mut state: ResMut<TutorialState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cards: Query<(&ComputedNode, &UiGlobalTransform)>,
) {
    if !state.fire_confetti {
        return;
    }
    state.fire_confetti = false;
    let Some(root) = state.confetti_root else {
        return;
    };
    let origin = state
        .root
        .and_then(|card| cards.get(card).ok())
        .map(|(cn, ugt)| {
            let isf = cn.inverse_scale_factor();
            let center = ugt.translation * isf;
            let half = cn.size() * 0.5 * isf;
            // Top-centre: the pieces fan upward, so firing from the top edge
            // means they arc over the card rather than out from behind it.
            Vec2::new(center.x, center.y - half.y)
        })
        .unwrap_or_else(|| {
            // No card yet (first frame) — the old bottom-right guess.
            let (w, h) = windows
                .single()
                .map(|win| (win.width(), win.height()))
                .unwrap_or((1280.0, 720.0));
            Vec2::new(w - 200.0, h - 230.0)
        });
    let seed = time.elapsed_secs().to_bits() ^ (state.current as u32).wrapping_mul(0x9E37_79B1);
    confetti::burst(&mut commands, root, origin, 36, seed);
}

/// The header's X closes the tutorial; Finish (on a chapter's completion card)
/// records that chapter and returns to the picker, so finishing one chapter leads
/// naturally into the next instead of dead-ending. (Skipping a *step* is
/// `handle_continue`'s job — it never ends anything.)
#[allow(clippy::too_many_arguments)]
pub fn handle_buttons(
    mut commands: Commands,
    mut state: ResMut<TutorialState>,
    dock: Option<ResMut<Dock>>,
    dock_dirty: Option<ResMut<DockDirty>>,
    close: Query<&Interaction, (Changed<Interaction>, With<TutorialCloseButton>)>,
    finish: Query<&Interaction, (Changed<Interaction>, With<TutorialFinishButton>)>,
) {
    if !state.active {
        return;
    }
    let finished = finish.iter().any(|i| *i == Interaction::Pressed);
    let closed = close.iter().any(|i| *i == Interaction::Pressed);
    if !finished && !closed {
        return;
    }

    // Closing still marks first-run as handled — nobody wants the tutorial
    // re-launching at them every time they open the editor.
    persistence::mark_completed();
    if finished {
        if let Some(chapter) = CHAPTERS.get(state.chapter) {
            persistence::mark_chapter_done(chapter.id);
        }
    }

    if finished {
        // Back to the picker for the next chapter.
        state.show_picker = true;
        state.current = 0;
        state.step_done = false;
        state.needs_body_rebuild = true;
        return;
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
        state.highlight_arrow,
    ]
    .into_iter()
    .flatten()
    {
        commands.entity(e).despawn();
    }
    *state = TutorialState::default();
}

/// Was the key bound to `action` just pressed?
///
/// Deliberately NOT `KeyBindings::just_pressed`: that consumes programmatic
/// dispatches from a shared set, so merely *observing* an action here would
/// steal it from the system meant to handle it. We read the binding and test the
/// raw keyboard instead — same rebinding-aware result, no side effect.
fn action_pressed(signals: &Signals, action: EditorAction) -> bool {
    let Some(bindings) = signals.bindings.as_ref() else {
        return false;
    };
    let Some(b) = bindings.bindings.get(&action) else {
        return false;
    };
    let k = &signals.keyboard;
    let ctrl = k.pressed(KeyCode::ControlLeft) || k.pressed(KeyCode::ControlRight);
    let shift = k.pressed(KeyCode::ShiftLeft) || k.pressed(KeyCode::ShiftRight);
    let alt = k.pressed(KeyCode::AltLeft) || k.pressed(KeyCode::AltRight);
    k.just_pressed(b.key) && ctrl == b.ctrl && shift == b.shift && alt == b.alt
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

/// Order-independent fingerprint of every entity's rotation. Summing rather than
/// collecting keeps this allocation-free per step, and it only has to answer
/// "did anything turn?", not "what turned".
fn rot_sum(transforms: &Query<&'static Transform>) -> f32 {
    transforms
        .iter()
        .map(|t| {
            let r = t.rotation;
            r.x + r.y + r.z + r.w
        })
        .sum()
}

/// Shortest absolute angular distance between two angles (radians).
fn ang_delta(a: f32, b: f32) -> f32 {
    let d = (a - b).abs() % std::f32::consts::TAU;
    d.min(std::f32::consts::TAU - d)
}

/// Whether the code editor's active tab is a Lua script.
fn active_is_script(c: &CodeEditorState) -> bool {
    c.active_tab
        .and_then(|i| c.open_files.get(i))
        .and_then(|f| f.path.extension())
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("lua"))
        .unwrap_or(false)
}

/// Model files under the project (depth-capped). Lets the import step detect "a
/// new model landed on disk" without reaching the import UI's private progress
/// state — and it also catches the silent drag-drop import path.
fn count_models(project: &CurrentProject) -> usize {
    // Same model extensions the asset browser recognises (renzora_asset_registry's
    // `AssetKind::Model`).
    const MODEL_EXTS: &[&str] = &[
        "glb", "gltf", "obj", "fbx", "usd", "usda", "usdc", "usdz", "abc", "dae", "blend",
    ];
    count_files(&project.path, |ext| MODEL_EXTS.contains(&ext))
}

/// Every importable asset file under the project — models plus textures, audio,
/// materials and themes. The marketplace can install any of those, so the import
/// step's model-only count would miss most purchases.
fn count_assets(project: &CurrentProject) -> usize {
    const ASSET_EXTS: &[&str] = &[
        "glb", "gltf", "obj", "fbx", "usd", "usda", "usdc", "usdz", "abc", "dae", "blend", "png",
        "jpg", "jpeg", "tga", "exr", "hdr", "ktx2", "basis", "rmip", "wav", "ogg", "mp3", "flac",
        "material", "particle", "anim", "lua", "wgsl", "toml",
    ];
    count_files(&project.path, |ext| ASSET_EXTS.contains(&ext))
}

/// `.lua` files anywhere in the project — the scripting chapter's "you made a
/// script" signal, which the inspector's add-script flow satisfies by writing
/// the file.
fn count_scripts(project: &CurrentProject) -> usize {
    count_files(&project.path, |ext| ext == "lua")
}

/// Count files under `root` (depth-capped, heavy build/vcs dirs skipped) whose
/// lowercased extension `keep` accepts.
fn count_files(root: &std::path::Path, keep: impl Fn(&str) -> bool + Copy) -> usize {
    const SKIP_DIRS: &[&str] = &[".git", "target", "node_modules", ".import", "cache"];
    fn walk(dir: &std::path::Path, depth: u8, n: &mut usize, keep: impl Fn(&str) -> bool + Copy) {
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
                    walk(&path, depth - 1, n, keep);
                }
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if keep(ext.to_ascii_lowercase().as_str()) {
                    *n += 1;
                }
            }
        }
    }
    let mut n = 0;
    walk(root, 5, &mut n, keep);
    n
}
