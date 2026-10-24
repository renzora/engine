//! The Animation workspace: one isolated stage, four modes, one timeline.
//!
//! The **stage** (panel `stage`) is the character on its own: a copy of the
//! subject model drawn by the studio preview's camera on its own render layer,
//! so rigging, painting and posing happen away from the scene, without the
//! level in the way and without picking anything else by accident. It is not a
//! picture of the model, though. Everything done on it acts on the real one:
//! a bone turned on the stage is the scene's bone (that is what the clip keys
//! and what the pose layer holds), and the copy follows it every frame.
//!
//! The stage header switches between four modes, and the panels either side
//! (`anim_tools`, `anim_inspector`) change with it:
//!
//! - **Rig**: give a model a skeleton (humanoid) or a face rig, from points
//!   clicked onto it; read the skeleton.
//! - **Skin**: generate weights, paint them, bend joints to test them.
//! - **Animate**: pose with the gizmo or the face sliders, key, keep a pose
//!   library, lip sync and blink.
//! - **Logic**: the state machine and its parameters, with the stage kept in
//!   the corner.
//!
//! The timeline is the editor's own, in the bottom panel, the same in every
//! mode.
//!
//! - [`clone`]: the copy on the stage, and what poses it in each mode.
//! - [`models`]: choosing the model, from the scene or the project's files.
//! - [`view`]: the stage panel, its camera controls and where the cursor is.
//! - [`tools`]: clicking bones, the gizmo, placing rig points, drawing.
//! - [`skin`]: Skin mode's weights.
//! - [`poses`]: the pose library.
//! - [`panels`]: the panels either side.

use bevy::prelude::*;

pub(crate) mod clone;
pub(crate) mod models;
pub(crate) mod panels;
pub(crate) mod poses;
pub(crate) mod skin;
pub(crate) mod tools;
pub(crate) mod view;

/// Which of the four jobs the workspace is doing.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub(crate) enum StageMode {
    Rig,
    Skin,
    #[default]
    Animate,
    Logic,
}

impl StageMode {
    pub(crate) const ALL: [StageMode; 4] = [StageMode::Rig, StageMode::Skin, StageMode::Animate, StageMode::Logic];

    pub(crate) fn label(self) -> &'static str {
        match self {
            StageMode::Rig => "Rig",
            StageMode::Skin => "Skin",
            StageMode::Animate => "Animate",
            StageMode::Logic => "Logic",
        }
    }
}

/// What the gizmo on the stage does to a bone.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum Tool {
    #[default]
    Rotate,
    Move,
}

/// The workspace's own settings: the mode, the stage's view, and which tab
/// each side panel shows.
#[derive(Resource)]
pub(crate) struct StageUi {
    pub(crate) mode: StageMode,
    /// The camera preset last picked: front, side or perspective.
    pub(crate) view: usize,
    pub(crate) tool: Tool,
    /// Gizmo axes along the bone's own, or the world's.
    pub(crate) local: bool,
    pub(crate) show_skeleton: bool,
    /// Rig mode's template: 0 humanoid body, 1 face.
    pub(crate) template: usize,
    /// Animate mode's left tab: 0 controls, 1 face.
    pub(crate) animate_left: usize,
    /// Animate mode's right tab: 0 pose, 1 clip, 2 performance.
    pub(crate) animate_right: usize,
    /// Rig mode's right tab: 0 skeleton, 1 bone.
    pub(crate) rig_right: usize,
}

impl Default for StageUi {
    fn default() -> Self {
        Self {
            mode: StageMode::Animate,
            view: 2,
            tool: Tool::Rotate,
            local: true,
            show_skeleton: true,
            template: 0,
            animate_left: 0,
            animate_right: 0,
            rig_right: 0,
        }
    }
}

pub(crate) const STAGE_PANEL: &str = "stage";

/// Run condition: the stage is in the dock somewhere.
pub(crate) fn stage_mounted(dock: Option<Res<renzora_ember::dock::Dock>>) -> bool {
    dock.is_some_and(|d| d.tree.contains_panel(STAGE_PANEL))
}

pub(crate) fn mode(world: &World) -> StageMode {
    world.get_resource::<StageUi>().map(|u| u.mode).unwrap_or_default()
}

pub struct StagePlugin;

impl Plugin for StagePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StageUi>();
        clone::register(app);
        models::register(app);
        view::register(app);
        tools::register(app);
        skin::register(app);
        poses::register(app);
        panels::register(app);
    }
}
