//! XR/VR plugin for Renzora Engine
//!
//! Dynamic plugin that adds OpenXR stereo rendering, head tracking,
//! controller input, and locomotion via bevy_mod_openxr.
//!
//! # Architecture
//!
//! This plugin has two phases:
//! 1. **Rendering init** (`xr_init_rendering`) — called by the engine BEFORE
//!    DefaultPlugins, replaces the standard render pipeline with OpenXR stereo
//!    rendering via `bevy_mod_openxr`.
//! 2. **Plugin build** (`XrPlugin::build`) — called during normal plugin loading,
//!    adds VR systems (input, locomotion, skybox sync).
//!
//! The engine detects this DLL in the plugins directory and calls
//! `xr_init_rendering` early. The standard `plugin_create` export provides
//! the VR gameplay systems.

use std::borrow::Cow;

use bevy::prelude::*;
use bevy::core_pipeline::Skybox;
use bevy::window::PresentMode;
use bevy_mod_xr::session::XrTrackingRoot;
use bevy_mod_xr::camera::XrCamera;
use bevy_xr_utils::actions::{
    ActionType, XRUtilsAction, XRUtilsActionSet, XRUtilsActionState,
    XRUtilsActionsPlugin, XRUtilsActionSystems,
};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// FFI: Early rendering initialization
// ---------------------------------------------------------------------------

/// Called by the engine BEFORE DefaultPlugins are added.
/// Replaces the standard render pipeline with OpenXR stereo rendering.
///
/// # Safety
/// `app_ptr` must be a valid pointer to a `bevy::app::App`.
/// Both sides must be built against the same Bevy version (guaranteed by workspace).
#[no_mangle]
pub unsafe extern "C" fn xr_init_rendering(app_ptr: *mut std::ffi::c_void) {
    let app = unsafe { &mut *(app_ptr as *mut App) };

    let mut exts = bevy_mod_openxr::exts::OxrExtensions::default();
    exts.enable_hand_tracking();
    exts.enable_fb_passthrough();
    exts.raw_mut().fb_display_refresh_rate = true;

    let plugins = bevy_mod_openxr::add_xr_plugins(
        DefaultPlugins
            .build()
            .disable::<bevy::render::pipelined_rendering::PipelinedRenderingPlugin>(),
    )
    .set(bevy_mod_xr::session::XrSessionPlugin { auto_handle: true })
    .set(bevy::window::WindowPlugin {
        primary_window: Some(bevy::window::Window {
            title: "Renzora (VR)".into(),
            present_mode: PresentMode::AutoNoVsync,
            ..default()
        }),
        ..default()
    })
    .set(bevy::asset::AssetPlugin {
        unapproved_path_mode: bevy::asset::UnapprovedPathMode::Allow,
        ..default()
    })
    .set(ImagePlugin {
        default_sampler: bevy::image::ImageSamplerDescriptor {
            address_mode_u: bevy::image::ImageAddressMode::Repeat,
            address_mode_v: bevy::image::ImageAddressMode::Repeat,
            address_mode_w: bevy::image::ImageAddressMode::Repeat,
            ..default()
        },
        ..default()
    })
    .set(bevy_mod_openxr::init::OxrInitPlugin {
        exts,
        ..default()
    });

    app.add_plugins(plugins);
    app.add_plugins(XRUtilsActionsPlugin);
    info!("[XR] OpenXR rendering pipeline initialized");
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Master VR configuration resource.
#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
pub struct VrConfig {
    pub move_speed: f32,
    pub snap_turn_angle: f32,
    pub snap_turn_cooldown: f32,
    pub thumbstick_deadzone: f32,
    pub smooth_turn: bool,
}

impl Default for VrConfig {
    fn default() -> Self {
        Self {
            move_speed: 4.0,
            snap_turn_angle: 45.0,
            snap_turn_cooldown: 0.3,
            thumbstick_deadzone: 0.2,
            smooth_turn: false,
        }
    }
}

/// Controller input state — updated every frame from OpenXR actions.
#[derive(Resource, Default, Debug)]
pub struct VrInput {
    pub left_thumbstick: Vec2,
    pub right_thumbstick: Vec2,
    pub left_trigger: f32,
    pub right_trigger: f32,
    pub left_grip: f32,
    pub right_grip: f32,
    pub button_a: bool,
    pub button_b: bool,
    pub button_x: bool,
    pub button_y: bool,
    pub menu: bool,
}

/// Marker components for identifying our action entities.
#[derive(Component)]
struct VrAction(Cow<'static, str>);

#[derive(Default)]
pub struct XrPlugin;

impl Plugin for XrPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] XrPlugin");

        app.init_resource::<VrConfig>()
            .init_resource::<VrInput>();

        // OpenXR action setup — runs at startup to create controller bindings
        app.add_systems(
            Startup,
            setup_action_sets.before(XRUtilsActionSystems::CreateEvents),
        );

        // Input sync — read OpenXR action states into VrInput resource
        app.add_systems(
            Update,
            sync_input.after(XRUtilsActionSystems::SyncActionStates),
        );

        // Locomotion — thumbstick movement and turning
        app.add_systems(
            Update,
            (smooth_locomotion, snap_turn)
                .chain()
                .after(sync_input),
        );

        // Skybox sync — copy from desktop viewport camera to XR cameras
        app.add_systems(Update, sync_skybox_to_xr_cameras);
    }
}

renzora::add!(XrPlugin);

// ===========================================================================
// OpenXR Action Setup
// ===========================================================================

fn setup_action_sets(mut commands: Commands) {
    let set = commands
        .spawn(XRUtilsActionSet {
            name: "renzora_vr".into(),
            pretty_name: "Renzora VR Controls".into(),
            priority: 0,
        })
        .id();

    let oculus = "/interaction_profiles/oculus/touch_controller";

    // Each action gets a single-hand binding so we can read left/right independently.
    // bevy_xr_utils syncs with Path::NULL which returns the state of whichever hand
    // has the binding, so separate actions per hand is the cleanest approach.

    // Left thumbstick
    let lt = commands
        .spawn((
            XRUtilsAction {
                action_name: "left_thumbstick".into(),
                localized_name: "Left Thumbstick".into(),
                action_type: ActionType::Vector,
            },
            VrAction("left_thumbstick".into()),
        ))
        .set_parent_in_place(set)
        .id();
    commands
        .spawn(bevy_xr_utils::actions::XRUtilsBinding {
            profile: Cow::Borrowed(oculus),
            binding: Cow::Borrowed("/user/hand/left/input/thumbstick"),
        })
        .set_parent_in_place(lt);

    // Right thumbstick
    let rt = commands
        .spawn((
            XRUtilsAction {
                action_name: "right_thumbstick".into(),
                localized_name: "Right Thumbstick".into(),
                action_type: ActionType::Vector,
            },
            VrAction("right_thumbstick".into()),
        ))
        .set_parent_in_place(set)
        .id();
    commands
        .spawn(bevy_xr_utils::actions::XRUtilsBinding {
            profile: Cow::Borrowed(oculus),
            binding: Cow::Borrowed("/user/hand/right/input/thumbstick"),
        })
        .set_parent_in_place(rt);

    // Left trigger
    let ltrig = commands
        .spawn((
            XRUtilsAction {
                action_name: "left_trigger".into(),
                localized_name: "Left Trigger".into(),
                action_type: ActionType::Float,
            },
            VrAction("left_trigger".into()),
        ))
        .set_parent_in_place(set)
        .id();
    commands
        .spawn(bevy_xr_utils::actions::XRUtilsBinding {
            profile: Cow::Borrowed(oculus),
            binding: Cow::Borrowed("/user/hand/left/input/trigger/value"),
        })
        .set_parent_in_place(ltrig);

    // Right trigger
    let rtrig = commands
        .spawn((
            XRUtilsAction {
                action_name: "right_trigger".into(),
                localized_name: "Right Trigger".into(),
                action_type: ActionType::Float,
            },
            VrAction("right_trigger".into()),
        ))
        .set_parent_in_place(set)
        .id();
    commands
        .spawn(bevy_xr_utils::actions::XRUtilsBinding {
            profile: Cow::Borrowed(oculus),
            binding: Cow::Borrowed("/user/hand/right/input/trigger/value"),
        })
        .set_parent_in_place(rtrig);

    // Left grip
    let lgrip = commands
        .spawn((
            XRUtilsAction {
                action_name: "left_grip".into(),
                localized_name: "Left Grip".into(),
                action_type: ActionType::Float,
            },
            VrAction("left_grip".into()),
        ))
        .set_parent_in_place(set)
        .id();
    commands
        .spawn(bevy_xr_utils::actions::XRUtilsBinding {
            profile: Cow::Borrowed(oculus),
            binding: Cow::Borrowed("/user/hand/left/input/squeeze/value"),
        })
        .set_parent_in_place(lgrip);

    // Right grip
    let rgrip = commands
        .spawn((
            XRUtilsAction {
                action_name: "right_grip".into(),
                localized_name: "Right Grip".into(),
                action_type: ActionType::Float,
            },
            VrAction("right_grip".into()),
        ))
        .set_parent_in_place(set)
        .id();
    commands
        .spawn(bevy_xr_utils::actions::XRUtilsBinding {
            profile: Cow::Borrowed(oculus),
            binding: Cow::Borrowed("/user/hand/right/input/squeeze/value"),
        })
        .set_parent_in_place(rgrip);

    // Buttons — A/B on right, X/Y on left (Quest controllers)
    for (name, path) in [
        ("button_a", "/user/hand/right/input/a/click"),
        ("button_b", "/user/hand/right/input/b/click"),
        ("button_x", "/user/hand/left/input/x/click"),
        ("button_y", "/user/hand/left/input/y/click"),
        ("menu", "/user/hand/left/input/menu/click"),
    ] {
        let ent = commands
            .spawn((
                XRUtilsAction {
                    action_name: name.into(),
                    localized_name: name.into(),
                    action_type: ActionType::Bool,
                },
                VrAction(name.into()),
            ))
            .set_parent_in_place(set)
            .id();
        commands
            .spawn(bevy_xr_utils::actions::XRUtilsBinding {
                profile: Cow::Borrowed(oculus),
                binding: Cow::Borrowed(path),
            })
            .set_parent_in_place(ent);
    }
}

// ===========================================================================
// Input Sync
// ===========================================================================

fn sync_input(
    actions: Query<(&VrAction, &XRUtilsActionState)>,
    mut input: ResMut<VrInput>,
) {
    for (vr_action, state) in actions.iter() {
        match vr_action.0.as_ref() {
            "left_thumbstick" => {
                if let XRUtilsActionState::Vector(v) = state {
                    input.left_thumbstick = Vec2::new(v.current_state[0], v.current_state[1]);
                }
            }
            "right_thumbstick" => {
                if let XRUtilsActionState::Vector(v) = state {
                    input.right_thumbstick = Vec2::new(v.current_state[0], v.current_state[1]);
                }
            }
            "left_trigger" => {
                if let XRUtilsActionState::Float(f) = state {
                    input.left_trigger = f.current_state;
                }
            }
            "right_trigger" => {
                if let XRUtilsActionState::Float(f) = state {
                    input.right_trigger = f.current_state;
                }
            }
            "left_grip" => {
                if let XRUtilsActionState::Float(f) = state {
                    input.left_grip = f.current_state;
                }
            }
            "right_grip" => {
                if let XRUtilsActionState::Float(f) = state {
                    input.right_grip = f.current_state;
                }
            }
            "button_a" => {
                if let XRUtilsActionState::Bool(b) = state {
                    input.button_a = b.current_state;
                }
            }
            "button_b" => {
                if let XRUtilsActionState::Bool(b) = state {
                    input.button_b = b.current_state;
                }
            }
            "button_x" => {
                if let XRUtilsActionState::Bool(b) = state {
                    input.button_x = b.current_state;
                }
            }
            "button_y" => {
                if let XRUtilsActionState::Bool(b) = state {
                    input.button_y = b.current_state;
                }
            }
            "menu" => {
                if let XRUtilsActionState::Bool(b) = state {
                    input.menu = b.current_state;
                }
            }
            _ => {}
        }
    }
}

// ===========================================================================
// Locomotion
// ===========================================================================

/// Left thumbstick moves the tracking root relative to head forward direction.
fn smooth_locomotion(
    input: Res<VrInput>,
    config: Res<VrConfig>,
    time: Res<Time>,
    mut tracking_root: Query<&mut Transform, With<XrTrackingRoot>>,
    xr_cameras: Query<&Transform, (With<XrCamera>, Without<XrTrackingRoot>)>,
) {
    let stick = input.left_thumbstick;
    if stick.length() < config.thumbstick_deadzone {
        return;
    }

    let Ok(mut root_tf) = tracking_root.single_mut() else {
        return;
    };

    // Head forward projected onto XZ plane
    let head_forward = if let Some(cam_tf) = xr_cameras.iter().next() {
        let fwd = cam_tf.forward().as_vec3();
        Vec3::new(fwd.x, 0.0, fwd.z).normalize_or_zero()
    } else {
        root_tf.forward().as_vec3()
    };

    let head_right = Vec3::new(-head_forward.z, 0.0, head_forward.x);
    let movement = (head_forward * stick.y + head_right * stick.x)
        * config.move_speed
        * time.delta_secs();

    root_tf.translation += movement;
}

/// Right thumbstick X-axis rotates the tracking root (snap or smooth turn).
fn snap_turn(
    input: Res<VrInput>,
    config: Res<VrConfig>,
    time: Res<Time>,
    mut tracking_root: Query<&mut Transform, With<XrTrackingRoot>>,
    mut cooldown: Local<f32>,
) {
    *cooldown -= time.delta_secs();

    let stick_x = input.right_thumbstick.x;
    if stick_x.abs() < 0.7 || *cooldown > 0.0 {
        if stick_x.abs() < 0.3 {
            // Reset cooldown when stick returns to center
            *cooldown = 0.0;
        }
        if !config.smooth_turn {
            return;
        }
    }

    let Ok(mut root_tf) = tracking_root.single_mut() else {
        return;
    };

    if config.smooth_turn {
        let turn_speed = 90.0_f32.to_radians();
        if stick_x.abs() > config.thumbstick_deadzone {
            root_tf.rotate_y(-stick_x * turn_speed * time.delta_secs());
        }
    } else {
        // Snap turn
        let angle = if stick_x > 0.0 {
            -config.snap_turn_angle.to_radians()
        } else {
            config.snap_turn_angle.to_radians()
        };
        root_tf.rotate_y(angle);
        *cooldown = config.snap_turn_cooldown;
    }
}

// ===========================================================================
// Skybox Sync
// ===========================================================================

/// Copy Skybox and clear_color from any non-XR camera to XR cameras
/// so the headset sees the same environment as the desktop viewport.
fn sync_skybox_to_xr_cameras(
    mut commands: Commands,
    viewport_cameras: Query<(Option<&Skybox>, &Camera), Without<XrCamera>>,
    xr_cameras: Query<(Entity, Option<&Skybox>), With<XrCamera>>,
    mut xr_cam_settings: Query<&mut Camera, With<XrCamera>>,
) {
    let Some((viewport_skybox, viewport_cam)) = viewport_cameras.iter().next() else {
        return;
    };
    let viewport_clear = viewport_cam.clear_color.clone();

    for (entity, existing_skybox) in xr_cameras.iter() {
        match (viewport_skybox, existing_skybox) {
            (Some(sky), Some(existing)) => {
                if existing.image != sky.image || existing.brightness != sky.brightness {
                    commands.entity(entity).insert(sky.clone());
                }
            }
            (Some(sky), None) => {
                commands.entity(entity).insert(sky.clone());
            }
            (None, Some(_)) => {
                commands.entity(entity).remove::<Skybox>();
            }
            (None, None) => {}
        }

        if let Ok(mut xr_cam) = xr_cam_settings.get_mut(entity) {
            xr_cam.clear_color = viewport_clear.clone();
        }
    }
}
