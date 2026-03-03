//! VR Editor Mode — In-Headset Scene Editing
//!
//! Renders editor panels as floating 3D quads in VR space using bevy_egui's
//! render-to-image pipeline. Each panel gets its own EguiContext + camera that
//! renders to an Image texture, displayed on a Plane3d mesh.
//!
//! This crate provides infrastructure only. Panel content rendering happens in
//! the main crate's `src/vr.rs` (which has access to panel state resources).

pub mod controller_model;
pub mod interaction;
pub mod layout;
pub mod locomotion;
pub mod panel_quad;
pub mod panel_spawner;

use bevy::ecs::schedule::ScheduleLabel;
use bevy::prelude::*;

use std::sync::atomic::{AtomicU32, Ordering};

/// Atomic counter for unique VR panel schedule IDs.
static NEXT_VR_PANEL_ID: AtomicU32 = AtomicU32::new(0);

/// Returns a unique ID for a new VR panel schedule.
pub fn next_vr_panel_id() -> u32 {
    NEXT_VR_PANEL_ID.fetch_add(1, Ordering::Relaxed)
}

/// Schedule label for a VR panel egui rendering pass.
/// Each VR panel context requires its own unique schedule (bevy_egui requirement).
#[derive(ScheduleLabel, Clone, Debug, PartialEq, Eq, Hash)]
pub struct VrPanelPass(pub u32);

/// Marker component on the 3D quad entity that displays a VR panel.
#[derive(Component)]
pub struct VrPanel {
    /// Identifies which panel content to render (e.g. "vr_session", "hierarchy").
    pub panel_type: String,
    /// The camera entity whose EguiContext renders this panel's UI.
    pub context_entity: Entity,
    /// Handle to the render target image (shared between camera and material).
    pub image_handle: Handle<Image>,
    /// Panel width in meters.
    pub width_meters: f32,
    /// Panel height in meters.
    pub height_meters: f32,
    /// Unique schedule ID for this panel's egui pass.
    pub schedule_id: u32,
}

/// Marker added to VrPanel entities once their render system has been
/// registered in their schedule. Used to detect newly-spawned panels.
#[derive(Component)]
pub struct VrPanelRegistered;

/// Edge or corner of a panel being resized.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ResizeEdge {
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Per-hand resize state.
#[derive(Clone, Default)]
pub struct HandResize {
    pub active: bool,
    pub edge: Option<ResizeEdge>,
    pub panel_entity: Option<Entity>,
    pub start_width: f32,
    pub start_height: f32,
    pub start_hit_world: Vec3,
}

/// Per-hand ray-cast data.
#[derive(Clone, Default)]
pub struct HandRay {
    pub ray_origin: Vec3,
    pub ray_direction: Vec3,
    pub hit_distance: Option<f32>,
    /// Entity of the panel being hit (if any).
    pub hit_entity: Option<Entity>,
    /// Edge being hovered (for resize visual feedback).
    pub hovered_edge: Option<ResizeEdge>,
}

/// Shares ray-cast results from `vr_panel_interaction` to the laser pointer renderer.
#[derive(Resource, Default)]
pub struct VrPointerHit {
    pub left: HandRay,
    pub right: HandRay,
}

/// Per-hand grab state.
#[derive(Clone, Default)]
pub struct HandGrab {
    pub grabbed_panel: Option<Entity>,
    pub grab_offset: Transform,
}

/// Tracks VR editor mode state.
#[derive(Resource, Default)]
pub struct VrEditorState {
    /// Whether VR editor panels are currently active.
    pub active: bool,
    /// Per-hand grab state — each hand can independently grab a panel.
    pub left: HandGrab,
    pub right: HandGrab,
    /// Per-hand resize state.
    pub left_resize: HandResize,
    pub right_resize: HandResize,
}

/// Marker: the solid 3D backing behind a VR panel's textured front face.
#[derive(Component)]
pub struct VrPanelBacking;

/// Panel backing thickness in meters (1.2cm).
pub const PANEL_DEPTH: f32 = 0.012;

/// VR pointer state passed from interaction system to egui input injection.
///
/// Written during `Update` by `vr_panel_interaction`, consumed by
/// `inject_vr_panel_egui_input` which writes into `EguiInput` components
/// before bevy_egui's multipass loop processes them in PostUpdate.
///
/// Button mapping:
/// - **Right trigger** = click/drag (egui primary button)
/// - **Left trigger** = scroll (hold + move pointer to scroll)
/// - **Grip** = grab/move panels
/// - **X button** (left hand) = resize panels
#[derive(Resource, Default)]
pub struct VrPanelInput {
    /// Context entity of the currently hovered panel (if any).
    pub hovered_context: Option<Entity>,
    /// Pointer position in egui pixel coordinates.
    pub pointer_pos: Vec2,
    /// Previous frame's pointer position (for computing scroll delta).
    pub prev_pointer_pos: Vec2,
    // ── Right trigger = click/drag ──
    /// Whether the right trigger is currently held.
    pub click_pressed: bool,
    /// Rising edge: right trigger was just pressed this frame.
    pub click_just_pressed: bool,
    /// Falling edge: right trigger was just released this frame.
    pub click_just_released: bool,
    // ── Left trigger = scroll ──
    /// Whether the left trigger is currently held (enables drag-to-scroll).
    pub scroll_active: bool,
    /// Left trigger analog value (0.0–1.0) for scroll speed scaling.
    pub scroll_trigger_value: f32,
}

/// What kind of asset is being dragged across VR panels.
#[derive(Default, Clone, Debug)]
pub enum VrDragKind {
    #[default]
    None,
    Shape,
    Asset,
}

/// Tracks cross-panel drag-and-drop state in VR.
///
/// Desktop drag-and-drop works because all panels share one egui context.
/// In VR each panel has an isolated context, so we bridge the drag via this
/// Bevy resource instead.
#[derive(Resource, Default)]
pub struct VrDragState {
    pub active: bool,
    pub kind: VrDragKind,
    pub prev_trigger_pressed: bool,
}

/// Smoothed locomotion state for camera lerping.
#[derive(Resource)]
pub struct VrLocomotionSmoothing {
    pub velocity: Vec3,
    pub yaw_velocity: f32,
}

impl Default for VrLocomotionSmoothing {
    fn default() -> Self {
        Self {
            velocity: Vec3::ZERO,
            yaw_velocity: 0.0,
        }
    }
}

/// Marker: capsule/box mesh representing a bone in a procedural VR hand.
///
/// Fields: (hand, from_joint_index, to_joint_index).
/// Joint indices follow OpenXR convention (0=Palm, 1=Wrist, 2-5=Thumb, etc.).
#[derive(Component)]
pub struct VrHandBone(pub renzora_xr::VrHand, pub usize, pub usize);

/// Marker: palm box mesh for procedural hand.
#[derive(Component)]
pub struct VrHandPalm(pub renzora_xr::VrHand);

/// Plugin that adds VR editor panel infrastructure.
///
/// Requires `renzora_xr::VrModeActive` resource to be present for systems to run.
pub struct VrEditorPlugin;

impl Plugin for VrEditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VrEditorState>()
            .init_resource::<VrPointerHit>()
            .init_resource::<VrPanelInput>()
            .init_resource::<VrLocomotionSmoothing>()
            .init_resource::<VrDragState>()
            .init_resource::<controller_model::ControllerModelState>()
            .init_resource::<panel_spawner::VrPanelMenu>()
            .add_systems(
                Update,
                (
                    interaction::vr_panel_interaction,
                    interaction::vr_panel_grab
                        .after(interaction::vr_panel_interaction),
                    interaction::vr_panel_resize
                        .after(interaction::vr_panel_interaction),
                    panel_spawner::vr_panel_menu_system,
                    panel_spawner::handle_panel_close,
                    controller_model::spawn_controller_models,
                    controller_model::update_controller_models,
                    controller_model::update_laser_pointer
                        .after(interaction::vr_panel_interaction),
                    locomotion::editor_locomotion,
                    locomotion::editor_camera_rotation,
                )
                    .run_if(resource_exists::<renzora_xr::VrModeActive>),
            );
    }
}
