//! Root motion and event markers, applied after Bevy has posed the skeleton.
//!
//! # Root motion
//!
//! A walk clip moves the hips forward. Played as-is, the mesh walks away from
//! its entity and snaps back on every loop, while the entity (and its
//! collider, its camera) stays put. With [`RootMotion`] on, each frame the
//! horizontal movement the playing clips gave the root bone is taken off the
//! bone and given to the entity instead (optionally its turn about the up
//! axis too), so the character travels exactly as far as its feet say.
//!
//! The movement is measured from the clips' curves, not from the posed bone:
//! a posed bone jumps back to the start of the clip on every loop, and only
//! the clip knows that the stretch played was "the end of one loop and the
//! start of the next" (see [`step_delta`]). Each playing base clip contributes
//! by its blend weight, so crossfades and blend trees move the entity by the
//! mix of their motion.
//!
//! The bone keeps its vertical movement (a jump, a crouch) and is held at its
//! rest position horizontally, in the animator's space, so the mesh stays
//! centred on the entity.
//!
//! # Markers
//!
//! The same pass fires every playing clip's markers, from the same playback
//! steps (see [`crate::markers`]).

use bevy::animation::animation_curves::AnimatableProperty;
use bevy::animation::{animated_field, AnimationTargetId};
use bevy::math::Affine3A;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::component::{AnimatorComponent, AnimatorState};
use crate::graph_builder::NodeRole;
use crate::markers::{markers_crossed, playback_step, NodeCursor, PlaybackStep};
use crate::property_playback::PropertyClip;

/// What root motion does with the root bone's movement.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub enum RootMotionMode {
    /// The clip moves the bone; the entity stays put.
    #[default]
    Off,
    /// The movement is taken off the bone and added to the entity's `Transform`.
    Transform,
    /// The movement is taken off the bone and published in
    /// `AnimatorReadState::root_motion_delta` for gameplay to apply (a
    /// character controller, a physics body), leaving the `Transform` alone.
    Extract,
}

/// Root motion settings of an animator.
#[derive(Clone, Debug, Default, PartialEq, Reflect, Serialize, Deserialize)]
pub struct RootMotion {
    #[serde(default)]
    #[reflect(default)]
    pub mode: RootMotionMode,
    /// The root bone. `None` finds the first bone named like hips or a pelvis.
    #[serde(default)]
    #[reflect(default)]
    pub bone: Option<String>,
    /// Also carry the root's turn about the up axis over to the entity.
    #[serde(default)]
    #[reflect(default)]
    pub yaw: bool,
}

impl RootMotion {
    pub fn enabled(&self) -> bool {
        self.mode != RootMotionMode::Off
    }
}

// ============================================================================
// Math
// ============================================================================

/// The change of a sampled quantity over a playback step, counting loop
/// wraps: the rest of the old loop, any whole loops, then the start of the
/// new one. `diff(a, b)` is the change from clip time `a` to `b`.
pub fn step_delta<V>(step: &PlaybackStep, duration: f32, diff: impl Fn(f32, f32) -> V) -> V
where
    V: std::ops::Add<Output = V> + std::ops::Mul<f32, Output = V>,
{
    let (start, end) = if step.forward { (0.0, duration) } else { (duration, 0.0) };
    let from = step.from.unwrap_or(start);
    if step.wraps == 0 {
        return diff(from, step.to);
    }
    let whole = (step.wraps - 1) as f32;
    diff(from, end) + diff(start, end) * whole + diff(start, step.to)
}

/// An angle wrapped to `-PI..=PI`.
pub fn wrap_angle(a: f32) -> f32 {
    let w = (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    if w.is_finite() { w } else { 0.0 }
}

/// The turn of a rotation about the up axis (its twist about Y).
pub fn yaw_of(q: Quat) -> f32 {
    wrap_angle(2.0 * q.y.atan2(q.w))
}

/// The root bone's local transform with its horizontal offset from rest (and,
/// with `yaw`, its turn from rest) taken off, measured in animator space.
/// `to_animator` maps the bone's parent space into the animator's.
pub fn strip_root(local: Transform, rest: Transform, to_animator: Affine3A, yaw: bool) -> Transform {
    let mut out = local;
    let p = to_animator.transform_point3(local.translation);
    let r = to_animator.transform_point3(rest.translation);
    let held = Vec3::new(r.x, p.y, r.z);
    out.translation = to_animator.inverse().transform_point3(held);
    if yaw {
        let (_, m, _) = to_animator.to_scale_rotation_translation();
        let q = m * local.rotation;
        let offset = wrap_angle(yaw_of(q) - yaw_of(m * rest.rotation));
        out.rotation = (m.inverse() * Quat::from_rotation_y(-offset) * q).normalize();
    }
    out
}

/// One clip's root movement over a step, in animator space: the horizontal
/// translation and the yaw. With `yaw`, the translation is turned back by the
/// root's turn at the start of the step, since that turn is moved onto the
/// entity and the movement must follow the way the entity now faces.
pub fn clip_root_motion(
    sample: impl Fn(f32) -> (Option<Vec3>, Option<Quat>),
    step: &PlaybackStep,
    duration: f32,
    to_animator: Affine3A,
    rest_rotation: Quat,
    yaw: bool,
) -> (Vec3, f32) {
    let (_, m, _) = to_animator.to_scale_rotation_translation();
    let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
    let translation = step_delta(step, duration, |a, b| match (sample(a).0, sample(b).0) {
        (Some(pa), Some(pb)) => flat(to_animator.transform_vector3(pb - pa)),
        _ => Vec3::ZERO,
    });
    if !yaw {
        return (translation, 0.0);
    }
    let yaw_at = |t: f32| sample(t).1.map(|q| yaw_of(m * q));
    let turn = step_delta(step, duration, |a, b| match (yaw_at(a), yaw_at(b)) {
        (Some(ya), Some(yb)) => wrap_angle(yb - ya),
        _ => 0.0,
    });
    let from = step.from.unwrap_or(if step.forward { 0.0 } else { duration });
    let facing = yaw_at(from).map(|y| wrap_angle(y - yaw_of(m * rest_rotation))).unwrap_or(0.0);
    (Quat::from_rotation_y(-facing) * translation, turn)
}

/// Sample a clip's translation and rotation curves for one target.
pub fn sample_target(clip: &AnimationClip, target: AnimationTargetId, t: f32) -> (Option<Vec3>, Option<Quat>) {
    let translation_field = animated_field!(Transform::translation);
    let rotation_field = animated_field!(Transform::rotation);
    let mut out = (None, None);
    let Some(curves) = clip.curves_for_target(target) else { return out };
    for curve in curves {
        let id = curve.0.evaluator_id();
        if id == translation_field.evaluator_id() {
            out.0 = curve.0.sample_clamped(t).downcast_ref::<Vec3>().copied();
        } else if id == rotation_field.evaluator_id() {
            out.1 = curve.0.sample_clamped(t).downcast_ref::<Quat>().copied();
        }
    }
    out
}

// ============================================================================
// System
// ============================================================================

/// Fire markers and apply root motion for every animator. Runs after Bevy's
/// animation pass and before transforms propagate.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn post_animation(
    graphs: Res<Assets<AnimationGraph>>,
    clips: Res<Assets<AnimationClip>>,
    prop_clips: Res<Assets<PropertyClip>>,
    mut inbox: ResMut<renzora::ScriptAnimEventInbox>,
    mut animators: Query<(Entity, &AnimatorComponent, &mut AnimatorState)>,
    players: Query<&AnimationPlayer>,
    mut transforms: Query<&mut Transform>,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    names: Query<(&Name, Option<&renzora::BindName>)>,
) {
    for (entity, animator, mut state) in animators.iter_mut() {
        if !state.initialized {
            continue;
        }
        let (Some(player_entity), Some(graph_handle)) = (state.player_entity, state.graph_handle.clone()) else {
            continue;
        };
        let (Ok(player), Some(graph)) = (players.get(player_entity), graphs.get(&graph_handle)) else {
            continue;
        };
        let state = &mut *state;

        // One step per playing clip node.
        let mut steps: Vec<(AnimationNodeIndex, PlaybackStep, f32)> = Vec::new();
        let mut seen = Vec::new();
        for (&node, anim) in player.playing_animations() {
            seen.push(node);
            let now = NodeCursor::of(anim);
            let prev = state.cursors.insert(node, now);
            let Some(duration) = clip_of(graph, &clips, node).map(|c| c.duration()) else { continue };
            if anim.is_paused() {
                continue;
            }
            if let Some(step) = playback_step(prev, now, anim.last_seek_time(), duration, anim.speed(), anim.is_finished()) {
                steps.push((node, step, duration));
            }
        }
        state.cursors.retain(|n, _| seen.contains(n));

        // Markers.
        for (node, step, duration) in &steps {
            let Some((slot, role)) = state.layout.roles.get(node) else { continue };
            let weight = player.animation(*node).map_or(0.0, |a| a.weight());
            if weight <= 0.0 || (*role == NodeRole::Base && state.marker_suppressed.contains(slot)) {
                continue;
            }
            let Some(markers) = state.prop_clip_handles.get(slot).and_then(|h| prop_clips.get(h)).map(|c| &c.markers) else {
                continue;
            };
            if markers.is_empty() {
                continue;
            }
            let times: Vec<f32> = markers.iter().map(|m| m.time).collect();
            for i in markers_crossed(&times, *duration, step) {
                inbox.pending.push(renzora::AnimEvent { name: markers[i].name.clone(), entity_bits: entity.to_bits() });
            }
        }

        // Root motion.
        if !animator.root_motion.enabled() {
            state.root_motion_delta = Vec3::ZERO;
            state.root_motion_yaw = 0.0;
            continue;
        }
        if state.root_bone.is_none() {
            state.root_bone = find_root_bone(entity, animator, state, &children, &names);
        }
        let Some((bone, bone_name)) = state.root_bone.clone() else { continue };
        let Some(to_animator) = parent_to_animator(bone, entity, &transforms, &parents) else {
            state.root_bone = None;
            continue;
        };
        let rest = state.target_rest.as_ref().and_then(|r| r.get(&bone_name)).map(|e| e.local);
        let Some(rest) = rest else { continue };
        let target = crate::bone_target(&bone_name);
        let yaw = animator.root_motion.yaw;

        let mut moved = Vec3::ZERO;
        let mut turned = 0.0;
        let mut total_weight = 0.0;
        for (node, step, duration) in &steps {
            if state.layout.roles.get(node).map(|(_, r)| *r) != Some(NodeRole::Base) {
                continue;
            }
            let weight = player.animation(*node).map_or(0.0, |a| a.weight());
            let Some(clip) = clip_of(graph, &clips, *node) else { continue };
            if weight <= 0.0 || clip.curves_for_target(target).is_none() {
                continue;
            }
            let (d, y) = clip_root_motion(|t| sample_target(clip, target, t), step, *duration, to_animator, rest.rotation, yaw);
            moved += d * weight;
            turned += y * weight;
            total_weight += weight;
        }
        if total_weight > 0.0 {
            moved /= total_weight;
            turned /= total_weight;
        }

        // Off the bone, every frame, whether or not it moved: the clip writes
        // the bone's full translation on each pass.
        if let Ok(mut t) = transforms.get_mut(bone) {
            *t = strip_root(*t, rest, to_animator, yaw);
        }

        let Ok(mut t) = transforms.get_mut(entity) else { continue };
        let world_step = t.rotation * (t.scale * moved);
        state.root_motion_delta = world_step;
        state.root_motion_yaw = turned;
        if animator.root_motion.mode == RootMotionMode::Transform {
            t.translation += world_step;
            if yaw && turned != 0.0 {
                t.rotation = (t.rotation * Quat::from_rotation_y(turned)).normalize();
            }
        }
    }
}

fn clip_of<'a>(graph: &AnimationGraph, clips: &'a Assets<AnimationClip>, node: AnimationNodeIndex) -> Option<&'a AnimationClip> {
    match &graph.get(node)?.node_type {
        AnimationNodeType::Clip(handle) => clips.get(handle),
        _ => None,
    }
}

/// The root bone under the animator: the configured one, or the first named
/// like hips or a pelvis.
fn find_root_bone(
    root: Entity,
    animator: &AnimatorComponent,
    state: &AnimatorState,
    children: &Query<&Children>,
    names: &Query<(&Name, Option<&renzora::BindName>)>,
) -> Option<(Entity, String)> {
    let wanted = match &state.target_rest {
        Some(rest) => rest.root_bone(animator.root_motion.bone.as_deref())?,
        None => animator.root_motion.bone.clone()?,
    };
    let key = renzora::sanitize_id(&wanted);
    let mut stack: Vec<Entity> = children.get(root).map(|c| c.iter().collect()).unwrap_or_default();
    while let Some(e) = stack.pop() {
        if let Ok((name, bind)) = names.get(e) {
            let bound = renzora::BindName::of(name, bind);
            if renzora::sanitize_id(bound) == key {
                return Some((e, bound.to_string()));
            }
        }
        if let Ok(c) = children.get(e) {
            stack.extend(c.iter());
        }
    }
    None
}

/// The transform from the bone's parent space into the animator's local space.
fn parent_to_animator(bone: Entity, animator: Entity, transforms: &Query<&mut Transform>, parents: &Query<&ChildOf>) -> Option<Affine3A> {
    let mut m = Affine3A::IDENTITY;
    let mut at = parents.get(bone).ok()?.parent();
    let mut depth = 0;
    while at != animator {
        let t = transforms.get(at).ok()?;
        m = t.compute_affine() * m;
        at = parents.get(at).ok()?.parent();
        depth += 1;
        if depth > 512 {
            return None;
        }
    }
    Some(m)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(from: Option<f32>, to: f32, wraps: u32) -> PlaybackStep {
        PlaybackStep { from, to, wraps, forward: true }
    }

    /// A clip walking 2 m per 1 s loop. Over a wrap, the motion is the rest of
    /// the old loop plus the start of the new one, never the jump back.
    #[test]
    fn root_motion_carries_on_across_a_loop() {
        let walk = |t: f32| (Some(Vec3::new(0.0, 1.0, 2.0 * t)), None);
        let (d, _) = clip_root_motion(walk, &step(Some(0.9), 0.1, 1), 1.0, Affine3A::IDENTITY, Quat::IDENTITY, false);
        assert!(d.distance(Vec3::new(0.0, 0.0, 0.4)) < 1e-5, "{d}");
        // Two whole loops in one step add their full distance.
        let (d, _) = clip_root_motion(walk, &step(Some(0.5), 0.5, 3), 1.0, Affine3A::IDENTITY, Quat::IDENTITY, false);
        assert!(d.distance(Vec3::new(0.0, 0.0, 6.0)) < 1e-4, "{d}");
    }

    /// The vertical part stays with the bone, and the parent space's scale
    /// and rotation are honoured (an FBX armature at 0.01 lying on its back).
    #[test]
    fn root_motion_is_horizontal_in_animator_space() {
        let armature = Affine3A::from_scale_rotation_translation(Vec3::splat(0.01), Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2), Vec3::ZERO);
        // In the armature's space +Y is forward and +Z is up.
        let clip = |t: f32| (Some(Vec3::new(0.0, 100.0 * t, 50.0 * t)), None);
        let (d, _) = clip_root_motion(clip, &step(Some(0.0), 1.0, 0), 1.0, armature, Quat::IDENTITY, false);
        assert!(d.y.abs() < 1e-5, "vertical motion must stay on the bone: {d}");
        assert!((d.length() - 1.0).abs() < 1e-4, "{d}");
    }

    #[test]
    fn stripping_holds_the_bone_at_rest_horizontally() {
        let rest = Transform::from_xyz(0.0, 1.0, 0.0);
        let posed = Transform::from_xyz(0.3, 1.2, 4.0);
        let out = strip_root(posed, rest, Affine3A::IDENTITY, false);
        assert!(out.translation.distance(Vec3::new(0.0, 1.2, 0.0)) < 1e-5);
    }

    /// A turn on the bone moves to the entity, and the bone is left facing
    /// its rest direction.
    #[test]
    fn yaw_moves_from_the_bone_to_the_entity() {
        let turn = |t: f32| (Some(Vec3::ZERO), Some(Quat::from_rotation_y(t)));
        let (_, y) = clip_root_motion(turn, &step(Some(0.2), 0.7, 0), 1.0, Affine3A::IDENTITY, Quat::IDENTITY, true);
        assert!((y - 0.5).abs() < 1e-5, "{y}");
        let posed = Transform::from_rotation(Quat::from_rotation_y(0.8) * Quat::from_rotation_x(0.3));
        let out = strip_root(posed, Transform::IDENTITY, Affine3A::IDENTITY, true);
        assert!(yaw_of(out.rotation).abs() < 1e-4);
        assert!(out.rotation.dot(Quat::from_rotation_x(0.3)).abs() > 1.0 - 1e-4);
    }

    #[test]
    fn angles_wrap_into_one_turn() {
        // A half turn is the boundary, where either sign is the same angle.
        assert!((wrap_angle(3.0 * std::f32::consts::PI).abs() - std::f32::consts::PI).abs() < 1e-4);
        assert!((wrap_angle(2.0 * std::f32::consts::TAU + 0.5) - 0.5).abs() < 1e-4);
        assert!(wrap_angle(0.25).eq(&0.25));
        assert_eq!(wrap_angle(f32::NAN), 0.0);
    }
}
