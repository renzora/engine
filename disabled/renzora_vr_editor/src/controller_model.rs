//! VR controller visuals — procedural hand models and laser pointer.
//!
//! Spawns capsule-bone hand meshes driven by OpenXR hand tracking joints
//! (26 per hand from `VrHandTrackingState`). When hand tracking is unavailable,
//! falls back to a simplified controller shape at the grip pose.
//!
//! Laser pointers emit from each hand's aim pose. Color indicates state:
//! cyan (default), green (hovering panel), yellow (hovering resize edge).
//!
//! All visual entities are parented under `XrTrackingRoot` so their local
//! transforms map directly to OpenXR tracking space.

use bevy::prelude::*;
use renzora_xr::reexports::XrTrackingRoot;
use renzora_xr::{VrControllerState, VrHand, VrHandTrackingState};

use crate::{VrHandBone, VrHandPalm, VrPointerHit};

/// Marker: sphere mesh representing a tracked VR controller (legacy, kept for despawn).
#[derive(Component)]
pub struct VrControllerOrb(pub VrHand);

/// Marker: cylinder mesh representing a laser pointer ray.
#[derive(Component)]
pub struct VrLaserPointer(pub VrHand);

/// Marker: fallback controller shape shown when hand tracking is unavailable.
#[derive(Component)]
pub struct VrControllerFallback(pub VrHand);

/// Tracks whether controller models have been spawned and stores material handles.
#[derive(Resource, Default)]
pub struct ControllerModelState {
    pub spawned: bool,
    pub default_laser_mat: Handle<StandardMaterial>,
    pub hover_laser_mat: Handle<StandardMaterial>,
    pub resize_laser_mat: Handle<StandardMaterial>,
}

/// Bone segments per hand: (from_joint, to_joint).
/// Joint indices follow OpenXR: Palm(0), Wrist(1), Thumb(2-5), Index(6-10),
/// Middle(11-15), Ring(16-20), Little(21-25).
const BONE_SEGMENTS: &[(usize, usize)] = &[
    // Wrist → Palm
    (1, 0),
    // Thumb
    (0, 2), (2, 3), (3, 4), (4, 5),
    // Index
    (0, 6), (6, 7), (7, 8), (8, 9), (9, 10),
    // Middle
    (0, 11), (11, 12), (12, 13), (13, 14), (14, 15),
    // Ring
    (0, 16), (16, 17), (17, 18), (18, 19), (19, 20),
    // Little
    (0, 21), (21, 22), (22, 23), (23, 24), (24, 25),
];

/// Spawn procedural hand bones, controller fallbacks, and laser pointers.
///
/// Deferred until `XrTrackingRoot` exists.
pub fn spawn_controller_models(
    mut state: ResMut<ControllerModelState>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    tracking_root: Query<Entity, With<XrTrackingRoot>>,
) {
    if state.spawned {
        return;
    }

    let Ok(root_entity) = tracking_root.single() else {
        return;
    };

    // ── Hand bone material — semi-transparent skin tone, unlit ──
    let bone_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.85, 0.72, 0.62, 0.7),
        emissive: LinearRgba::new(0.3, 0.25, 0.2, 1.0),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });

    // ── Shared meshes ──
    let bone_mesh = meshes.add(Capsule3d::new(0.008, 0.04)); // ~8mm radius, 4cm default length
    let palm_mesh = meshes.add(Cuboid::new(0.08, 0.015, 0.06)); // 8cm × 1.5cm × 6cm

    let mut children = Vec::new();

    // Spawn bone entities for each hand
    for hand in [VrHand::Left, VrHand::Right] {
        let hand_name = match hand {
            VrHand::Left => "Left",
            VrHand::Right => "Right",
        };

        // Palm box
        let palm = commands.spawn((
            VrHandPalm(hand),
            Mesh3d(palm_mesh.clone()),
            MeshMaterial3d(bone_mat.clone()),
            Transform::default(),
            Visibility::Hidden,
            Name::new(format!("VR {} Palm", hand_name)),
        )).id();
        children.push(palm);

        // Bone capsules
        for &(from, to) in BONE_SEGMENTS {
            let bone = commands.spawn((
                VrHandBone(hand, from, to),
                Mesh3d(bone_mesh.clone()),
                MeshMaterial3d(bone_mat.clone()),
                Transform::default(),
                Visibility::Hidden,
                Name::new(format!("VR {} Bone {}-{}", hand_name, from, to)),
            )).id();
            children.push(bone);
        }
    }

    // ── Controller fallback shapes (box + cylinder) ──
    let fallback_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.4, 0.45, 0.5),
        emissive: LinearRgba::new(0.2, 0.25, 0.3, 1.0),
        unlit: true,
        ..default()
    });
    let fallback_mesh = meshes.add(Cuboid::new(0.04, 0.03, 0.12));

    for hand in [VrHand::Left, VrHand::Right] {
        let name = match hand {
            VrHand::Left => "Left",
            VrHand::Right => "Right",
        };
        let fb = commands.spawn((
            VrControllerFallback(hand),
            Mesh3d(fallback_mesh.clone()),
            MeshMaterial3d(fallback_mat.clone()),
            Transform::default(),
            Visibility::Hidden,
            Name::new(format!("VR {} Controller Fallback", name)),
        )).id();
        children.push(fb);
    }

    // ── Laser pointers ──
    let laser_mesh = meshes.add(Cylinder::new(0.003, 1.0));

    let default_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.0, 0.7, 1.0),
        emissive: LinearRgba::new(0.0, 1.5, 3.0, 1.0),
        unlit: true,
        ..default()
    });

    let hover_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.0, 1.0, 0.3),
        emissive: LinearRgba::new(0.0, 3.0, 0.6, 1.0),
        unlit: true,
        ..default()
    });

    let resize_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.9, 0.0),
        emissive: LinearRgba::new(3.0, 2.7, 0.0, 1.0),
        unlit: true,
        ..default()
    });

    let laser_right = commands.spawn((
        VrLaserPointer(VrHand::Right),
        Mesh3d(laser_mesh.clone()),
        MeshMaterial3d(default_mat.clone()),
        Transform::default(),
        Visibility::Hidden,
        Name::new("VR Laser Pointer (Right)"),
    )).id();
    children.push(laser_right);

    let laser_left = commands.spawn((
        VrLaserPointer(VrHand::Left),
        Mesh3d(laser_mesh),
        MeshMaterial3d(default_mat.clone()),
        Transform::default(),
        Visibility::Hidden,
        Name::new("VR Laser Pointer (Left)"),
    )).id();
    children.push(laser_left);

    // Parent all under XrTrackingRoot
    commands.entity(root_entity).add_children(&children);

    state.default_laser_mat = default_mat;
    state.hover_laser_mat = hover_mat;
    state.resize_laser_mat = resize_mat;
    state.spawned = true;

    info!("VR hand models + laser pointers spawned (parented under XrTrackingRoot)");
}

/// Default open-hand joint offsets (tracking space, relative to palm at origin).
/// Used as fallback when hand tracking is unavailable but controller is tracked.
fn default_hand_joints() -> Vec<Transform> {
    let mut joints = vec![Transform::default(); 26];
    // Palm at origin
    joints[0] = Transform::from_xyz(0.0, 0.0, 0.0);
    // Wrist slightly behind
    joints[1] = Transform::from_xyz(0.0, -0.03, 0.04);
    // Thumb
    joints[2] = Transform::from_xyz(0.03, 0.0, -0.01);
    joints[3] = Transform::from_xyz(0.05, 0.0, -0.02);
    joints[4] = Transform::from_xyz(0.065, 0.0, -0.03);
    joints[5] = Transform::from_xyz(0.075, 0.0, -0.04);
    // Index
    joints[6] = Transform::from_xyz(0.02, 0.0, -0.04);
    joints[7] = Transform::from_xyz(0.02, 0.0, -0.07);
    joints[8] = Transform::from_xyz(0.02, 0.0, -0.09);
    joints[9] = Transform::from_xyz(0.02, 0.0, -0.105);
    joints[10] = Transform::from_xyz(0.02, 0.0, -0.115);
    // Middle
    joints[11] = Transform::from_xyz(0.0, 0.0, -0.04);
    joints[12] = Transform::from_xyz(0.0, 0.0, -0.075);
    joints[13] = Transform::from_xyz(0.0, 0.0, -0.098);
    joints[14] = Transform::from_xyz(0.0, 0.0, -0.115);
    joints[15] = Transform::from_xyz(0.0, 0.0, -0.125);
    // Ring
    joints[16] = Transform::from_xyz(-0.02, 0.0, -0.04);
    joints[17] = Transform::from_xyz(-0.02, 0.0, -0.07);
    joints[18] = Transform::from_xyz(-0.02, 0.0, -0.09);
    joints[19] = Transform::from_xyz(-0.02, 0.0, -0.103);
    joints[20] = Transform::from_xyz(-0.02, 0.0, -0.113);
    // Little
    joints[21] = Transform::from_xyz(-0.04, 0.0, -0.035);
    joints[22] = Transform::from_xyz(-0.04, 0.0, -0.055);
    joints[23] = Transform::from_xyz(-0.04, 0.0, -0.07);
    joints[24] = Transform::from_xyz(-0.04, 0.0, -0.08);
    joints[25] = Transform::from_xyz(-0.04, 0.0, -0.088);
    joints
}

/// Update hand bone entities from hand tracking joints each frame.
///
/// When hand tracking is available, positions each capsule between its two joints.
/// When unavailable but controller is tracked, shows the fallback controller shape.
pub fn update_controller_models(
    controllers: Option<Res<VrControllerState>>,
    hand_tracking: Option<Res<VrHandTrackingState>>,
    mut bones: Query<(&VrHandBone, &mut Transform, &mut Visibility)>,
    mut palms: Query<(&VrHandPalm, &mut Transform, &mut Visibility), Without<VrHandBone>>,
    mut fallbacks: Query<
        (&VrControllerFallback, &mut Transform, &mut Visibility),
        (Without<VrHandBone>, Without<VrHandPalm>),
    >,
) {
    let controllers = controllers.as_deref();
    let hand_tracking = hand_tracking.as_deref();

    for hand in [VrHand::Left, VrHand::Right] {
        let ht_state = hand_tracking.map(|ht| ht.hand(hand));
        let hand_tracked = ht_state.map_or(false, |h| h.tracked && h.joints.len() >= 26);

        let ctrl_state = controllers.map(|c| match hand {
            VrHand::Left => &c.left,
            VrHand::Right => &c.right,
        });
        let ctrl_tracked = ctrl_state.map_or(false, |c| c.tracked);

        if hand_tracked {
            let joints = &ht_state.unwrap().joints;

            // Show bones
            for (bone, mut tf, mut vis) in bones.iter_mut() {
                if bone.0 != hand {
                    continue;
                }
                let from_idx = bone.1;
                let to_idx = bone.2;
                if from_idx >= joints.len() || to_idx >= joints.len() {
                    *vis = Visibility::Hidden;
                    continue;
                }

                let from_pos = joints[from_idx].translation;
                let to_pos = joints[to_idx].translation;
                let midpoint = (from_pos + to_pos) * 0.5;
                let diff = to_pos - from_pos;
                let length = diff.length();

                if length < 0.001 {
                    *vis = Visibility::Hidden;
                    continue;
                }

                let direction = diff / length;
                let rotation = Quat::from_rotation_arc(Vec3::Y, direction);

                tf.translation = midpoint;
                tf.rotation = rotation;
                // Scale Y to match bone length (capsule default height = 0.04 + 2*0.008 caps)
                tf.scale = Vec3::new(1.0, length / 0.056, 1.0);
                *vis = Visibility::Visible;
            }

            // Show palm
            for (palm, mut tf, mut vis) in palms.iter_mut() {
                if palm.0 != hand {
                    continue;
                }
                tf.translation = joints[0].translation;
                tf.rotation = joints[0].rotation;
                *vis = Visibility::Visible;
            }

            // Hide fallback
            for (fb, _, mut vis) in fallbacks.iter_mut() {
                if fb.0 == hand {
                    *vis = Visibility::Hidden;
                }
            }
        } else if ctrl_tracked {
            let ctrl = ctrl_state.unwrap();
            // Use default hand pose positioned at grip
            let default_joints = default_hand_joints();

            // Position bones using default joints offset by grip pose
            for (bone, mut tf, mut vis) in bones.iter_mut() {
                if bone.0 != hand {
                    continue;
                }
                let from_idx = bone.1;
                let to_idx = bone.2;
                if from_idx >= default_joints.len() || to_idx >= default_joints.len() {
                    *vis = Visibility::Hidden;
                    continue;
                }

                let from_pos = ctrl.grip_position + ctrl.grip_rotation * default_joints[from_idx].translation;
                let to_pos = ctrl.grip_position + ctrl.grip_rotation * default_joints[to_idx].translation;
                let midpoint = (from_pos + to_pos) * 0.5;
                let diff = to_pos - from_pos;
                let length = diff.length();

                if length < 0.001 {
                    *vis = Visibility::Hidden;
                    continue;
                }

                let direction = diff / length;
                let rotation = Quat::from_rotation_arc(Vec3::Y, direction);

                tf.translation = midpoint;
                tf.rotation = rotation;
                tf.scale = Vec3::new(1.0, length / 0.056, 1.0);
                *vis = Visibility::Visible;
            }

            // Show palm at grip
            for (palm, mut tf, mut vis) in palms.iter_mut() {
                if palm.0 != hand {
                    continue;
                }
                tf.translation = ctrl.grip_position;
                tf.rotation = ctrl.grip_rotation;
                *vis = Visibility::Visible;
            }

            // Show fallback controller shape too (subtle backup indicator)
            for (fb, mut tf, mut vis) in fallbacks.iter_mut() {
                if fb.0 == hand {
                    tf.translation = ctrl.grip_position;
                    tf.rotation = ctrl.grip_rotation;
                    *vis = Visibility::Hidden; // Hide when we have hand pose
                }
            }
        } else {
            // Nothing tracked — hide everything
            for (bone, _, mut vis) in bones.iter_mut() {
                if bone.0 == hand {
                    *vis = Visibility::Hidden;
                }
            }
            for (palm, _, mut vis) in palms.iter_mut() {
                if palm.0 == hand {
                    *vis = Visibility::Hidden;
                }
            }
            for (fb, _, mut vis) in fallbacks.iter_mut() {
                if fb.0 == hand {
                    *vis = Visibility::Hidden;
                }
            }
        }
    }
}

/// Update laser pointers for both hands. Runs after `vr_panel_interaction`.
///
/// Laser entities are children of `XrTrackingRoot`, so their local transforms are
/// in tracking space — matching the coordinates stored in `VrPointerHit`.
pub fn update_laser_pointer(
    pointer_hit: Res<VrPointerHit>,
    controllers: Option<Res<VrControllerState>>,
    model_state: Res<ControllerModelState>,
    mut lasers: Query<(
        &VrLaserPointer,
        &mut Transform,
        &mut Visibility,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let Some(controllers) = controllers.as_ref() else {
        return;
    };

    for (laser, mut tf, mut vis, mut mat) in lasers.iter_mut() {
        let (hand_state, hand_ray) = match laser.0 {
            VrHand::Right => (&controllers.right, &pointer_hit.right),
            VrHand::Left => (&controllers.left, &pointer_hit.left),
        };

        if !hand_state.tracked || hand_ray.ray_direction.length_squared() < 1e-6 {
            *vis = Visibility::Hidden;
            continue;
        }

        let ray_len = hand_ray.hit_distance.unwrap_or(5.0);
        let hovering_panel = hand_ray.hit_distance.is_some();

        // Bevy Cylinder is oriented along Y axis, center at origin.
        // Rotate so local Y aligns with ray_dir, then position at midpoint along ray.
        let rotation = Quat::from_rotation_arc(Vec3::Y, hand_ray.ray_direction);
        let position = hand_ray.ray_origin + hand_ray.ray_direction * (ray_len * 0.5);

        tf.translation = position;
        tf.rotation = rotation;
        tf.scale = Vec3::new(1.0, ray_len, 1.0);

        // Yellow when hovering a resize edge, green when hovering a panel, cyan otherwise
        if hand_ray.hovered_edge.is_some() {
            mat.0 = model_state.resize_laser_mat.clone();
        } else if hovering_panel {
            mat.0 = model_state.hover_laser_mat.clone();
        } else {
            mat.0 = model_state.default_laser_mat.clone();
        }

        *vis = Visibility::Visible;
    }
}
