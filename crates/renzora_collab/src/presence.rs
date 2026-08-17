//! Presence: where everyone is looking, and what they have hold of.
//!
//! This is the cheapest part of the feature and the one that makes it feel like
//! a session rather than a file transfer. Without it a collaborator's edits
//! appear out of nowhere; with it you can see them fly across the level, watch
//! them line up a shot, and know not to grab the thing they are clearly about to
//! move.
//!
//! Presence is **state, not events**. Every message is a complete statement of
//! where a peer is, so a dropped one costs nothing — the next arrives 100 ms
//! later and is just as complete. That is why it is safe to send this
//! continuously while everything else on the link is sent only on change.

use bevy::prelude::*;

use crate::identity::{CollabId, CollabIds};
use crate::protocol::{CamPose, CollabMsg};
use crate::session::CollabSession;

/// Presence updates per second. Fast enough that a peer's camera reads as
/// moving rather than teleporting; slow enough to be background noise on the
/// link next to a scene snapshot.
const PRESENCE_HZ: f32 = 10.0;

/// How far in front of a peer's camera their frustum marker is drawn. Far enough
/// to show which way they are facing, near enough not to clutter the level.
const FRUSTUM_DEPTH: f32 = 1.5;

#[derive(Resource, Default)]
pub struct PresenceTimer(f32);

/// Tell everyone where this editor is looking.
pub fn broadcast_presence(
    session: Res<CollabSession>,
    mut timer: ResMut<PresenceTimer>,
    time: Res<Time>,
    camera: Query<(&GlobalTransform, &Projection), With<renzora::core::EditorCamera>>,
    selection: Option<Res<renzora::EditorSelection>>,
    ids: Query<&CollabId>,
) {
    if !session.is_active() {
        return;
    }
    timer.0 += time.delta_secs();
    if timer.0 < 1.0 / PRESENCE_HZ {
        return;
    }
    timer.0 = 0.0;

    let camera = camera.iter().next().map(|(transform, projection)| {
        let (_, rotation, translation) = transform.to_scale_rotation_translation();
        CamPose {
            translation: translation.to_array(),
            rotation: rotation.to_array(),
            // The field of view lives inside the projection enum, so an
            // orthographic editor camera has none — 60° is only a stand-in for
            // drawing a marker, never a claim about the peer's actual view.
            fov: match projection {
                Projection::Perspective(p) => p.fov,
                _ => std::f32::consts::FRAC_PI_3,
            },
        }
    });

    let selected: Vec<u64> = selection
        .map(|s| {
            s.get_all().iter().filter_map(|&e| ids.get(e).ok().map(|id| id.0)).collect()
        })
        .unwrap_or_default();

    session.send_up(CollabMsg::Presence { peer: session.slot as u64, camera, selection: selected });
}

/// Draw every peer: a marker where their camera is, and an outline around
/// whatever they have selected.
pub fn draw_peers(
    mut gizmos: Gizmos,
    session: Res<CollabSession>,
    ids: Res<CollabIds>,
    transforms: Query<&GlobalTransform>,
    bounds: Query<&bevy::camera::primitives::Aabb>,
) {
    if !session.is_active() {
        return;
    }
    for peer in session.peers.values() {
        if !peer.ready {
            continue;
        }
        let color = Color::srgb_u8(peer.color[0], peer.color[1], peer.color[2]);

        if let Some(pose) = peer.camera {
            draw_camera_marker(&mut gizmos, pose, color);
        }

        // Their selection, outlined in their colour. Drawn from the same `Aabb`
        // the local selection outline uses, so a peer's highlight sits exactly
        // where yours would.
        for id in &peer.selection {
            let Some(entity) = ids.entity(*id) else {
                continue;
            };
            let Ok(transform) = transforms.get(entity) else {
                continue;
            };
            match bounds.get(entity) {
                Ok(aabb) => {
                    let centre = transform.transform_point(Vec3::from(aabb.center));
                    let (scale, rotation, _) = transform.to_scale_rotation_translation();
                    let half = Vec3::from(aabb.half_extents) * scale;
                    draw_box(&mut gizmos, centre, rotation, half, color);
                }
                Err(_) => {
                    gizmos.sphere(transform.translation(), 0.35, color);
                }
            }
        }
    }
}

/// A wireframe box, drawn as its twelve edges.
///
/// Written out rather than reached for from `Gizmos`, which has no oriented-box
/// primitive — only screen-space rects and unrotated shapes. A peer's selection
/// has to follow the entity's rotation or the outline stops matching the thing
/// it is outlining as soon as anyone turns it.
fn draw_box(gizmos: &mut Gizmos, centre: Vec3, rotation: Quat, half: Vec3, color: Color) {
    // The eight corners, in the order that makes the loops below the four
    // bottom edges, the four top edges, and the four uprights.
    let corner = |x: f32, y: f32, z: f32| centre + rotation * (half * Vec3::new(x, y, z));
    let bottom = [
        corner(-1.0, -1.0, -1.0),
        corner(1.0, -1.0, -1.0),
        corner(1.0, -1.0, 1.0),
        corner(-1.0, -1.0, 1.0),
    ];
    let top = [
        corner(-1.0, 1.0, -1.0),
        corner(1.0, 1.0, -1.0),
        corner(1.0, 1.0, 1.0),
        corner(-1.0, 1.0, 1.0),
    ];
    for i in 0..4 {
        let next = (i + 1) % 4;
        gizmos.line(bottom[i], bottom[next], color);
        gizmos.line(top[i], top[next], color);
        gizmos.line(bottom[i], top[i], color);
    }
}

/// A small pyramid opening the way the peer is facing — recognisably a camera at
/// a glance, and cheap enough to draw for every peer every frame.
fn draw_camera_marker(gizmos: &mut Gizmos, pose: CamPose, color: Color) {
    let origin = Vec3::from_array(pose.translation);
    let rotation = Quat::from_array(pose.rotation).normalize();
    let half = (pose.fov * 0.5).tan() * FRUSTUM_DEPTH;
    let forward = rotation * Vec3::NEG_Z * FRUSTUM_DEPTH;
    let right = rotation * Vec3::X * half;
    let up = rotation * Vec3::Y * half;

    let corners = [
        origin + forward + right + up,
        origin + forward - right + up,
        origin + forward - right - up,
        origin + forward + right - up,
    ];
    for corner in corners {
        gizmos.line(origin, corner, color);
    }
    for i in 0..4 {
        gizmos.line(corners[i], corners[(i + 1) % 4], color);
    }
}
