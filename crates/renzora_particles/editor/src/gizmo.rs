//! The particle system's viewport gizmo.
//!
//! An effect has nothing solid to look at or aim for: between bursts it can be
//! empty, and when it is running its shape is wherever the particles happen to
//! be. So every particle system gets a small sparkle icon at its origin, sized
//! to stay the same on screen (it is also what the click lands on, through the
//! entity's `PickProxy`), and the selected one also shows where its emitters
//! spawn: the shape nodes in its graph, drawn as wireframes in the entity's
//! space, and what its particles collide with.

use bevy::prelude::*;

use renzora_editor_framework::{EditorCamera, EditorSelection};
use renzora_gizmo::OverlayGizmoGroup;
use renzora_particles::asset::{PinValue, VfxAsset, VfxNode};
use renzora_particles::ParticleSystem;

/// Warm, like a flame, so the icon reads as "effect" next to the yellow lights.
const ICON: Color = Color::srgb(1.0, 0.62, 0.3);
const ICON_DIM: Color = Color::srgba(1.0, 0.62, 0.3, 0.55);
const SHAPE: Color = Color::srgb(1.0, 0.72, 0.42);
const COLLIDER: Color = Color::srgb(0.45, 0.78, 1.0);
const FORCE: Color = Color::srgb(0.78, 0.55, 1.0);

/// Icon size as a fraction of the distance to the camera: constant on screen.
const ICON_SCALE: f32 = 0.03;

pub(crate) fn draw_particle_gizmos(
    mut gizmos: Gizmos<OverlayGizmoGroup>,
    selection: Res<EditorSelection>,
    camera: Query<&GlobalTransform, With<EditorCamera>>,
    // The effect editor's preview entity is not scene content and is drawn
    // on its own layer; a sparkle for it would float at the origin.
    systems: Query<(Entity, &GlobalTransform, &ParticleSystem), Without<crate::preview::PreviewEffect>>,
    assets: Res<Assets<VfxAsset>>,
) {
    let Ok(cam) = camera.single() else { return };
    let (right, up) = (cam.right().as_vec3(), cam.up().as_vec3());
    let selected = selection.get_all();
    for (entity, gt, system) in &systems {
        let is_selected = selected.contains(&entity);
        let origin = gt.translation();
        let size = cam.translation().distance(origin) * ICON_SCALE;
        draw_sparkle(&mut gizmos, origin, right, up, size, if is_selected { ICON } else { ICON_DIM });
        if !is_selected {
            continue;
        }
        let Some(asset) = assets.get(&system.asset) else { continue };
        for emitter in asset.emitters.iter().filter(|e| e.enabled) {
            for node in &emitter.graph.nodes {
                draw_shape(&mut gizmos, node, gt);
            }
        }
    }
}

/// A four-pointed star in the camera's plane.
fn draw_sparkle(gizmos: &mut Gizmos<OverlayGizmoGroup>, at: Vec3, right: Vec3, up: Vec3, size: f32, color: Color) {
    let points: Vec<Vec3> = (0..=8)
        .map(|i| {
            let a = i as f32 * std::f32::consts::FRAC_PI_4;
            let r = if i % 2 == 0 { size } else { size * 0.32 };
            at + (right * a.sin() + up * a.cos()) * r
        })
        .collect();
    gizmos.linestrip(points, color);
}

fn float(node: &VfxNode, pin: &str, default: f32) -> f32 {
    match node.input_values.get(pin) {
        Some(PinValue::Float(v)) => *v,
        _ => default,
    }
}

fn vec(node: &VfxNode, pin: &str, default: Vec3) -> Vec3 {
    match node.input_values.get(pin) {
        Some(PinValue::Vec3(v)) => Vec3::from(*v),
        Some(PinValue::Vec2(v)) => Vec3::new(v[0], v[1], 0.0),
        Some(PinValue::Float(v)) => Vec3::splat(*v),
        _ => default,
    }
}

/// One shape node as a wireframe in the entity's space. Defaults match the
/// node catalog's, so an unset pin draws what the simulation uses.
fn draw_shape(gizmos: &mut Gizmos<OverlayGizmoGroup>, node: &VfxNode, gt: &GlobalTransform) {
    let (scale, rotation, translation) = gt.to_scale_rotation_translation();
    let s = scale.max_element();
    let iso = Isometry3d::new(translation, rotation);
    match node.node_type.as_str() {
        "shape/sphere" => {
            gizmos.sphere(iso, float(node, "radius", 1.0) * s, SHAPE);
        }
        "shape/box" => {
            let size = vec(node, "size", Vec3::ONE);
            gizmos.cube(Transform { translation, rotation, scale: size * scale }, SHAPE);
        }
        "shape/cone" => {
            // The emission disc, then the cone's spread drawn out to one unit:
            // the directions particles leave in, which is what the angle sets.
            let radius = float(node, "radius", 0.5) * s;
            let angle = float(node, "angle", 25.0).to_radians();
            let flat = Isometry3d::new(translation, rotation * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2));
            gizmos.circle(flat, radius, SHAPE);
            let reach = s;
            let top_radius = radius + angle.sin() * reach;
            let top = Isometry3d::new(
                translation + rotation * Vec3::Y * angle.cos() * reach,
                rotation * Quat::from_rotation_x(std::f32::consts::FRAC_PI_2),
            );
            gizmos.circle(top, top_radius, SHAPE);
            for i in 0..4 {
                let a = i as f32 * std::f32::consts::FRAC_PI_2;
                let out = Vec3::new(a.cos(), 0.0, a.sin());
                let base = translation + rotation * (out * radius);
                let tip = translation + rotation * (out * top_radius + Vec3::Y * angle.cos() * reach);
                gizmos.line(base, tip, SHAPE);
            }
        }
        "shape/circle" => {
            gizmos.circle(iso, float(node, "radius", 1.0) * s, SHAPE);
        }
        "shape/rect" => {
            let size = vec(node, "size", Vec3::new(1.0, 1.0, 0.0));
            gizmos.rect(iso, Vec2::new(size.x * scale.x, size.y * scale.y), SHAPE);
        }
        t if t.starts_with(renzora_particles::nodes::COLLIDER_PREFIX) => draw_collider(gizmos, node, gt),
        // Where the shaped forces act, in a third colour: the sphere a
        // Conform To Sphere holds particles on, an Orbit's ring (or its axis
        // when it has none) and a Gravity Well's softened core.
        "force/conform_sphere" => {
            let center = gt.transform_point(vec(node, "center", Vec3::ZERO));
            gizmos.sphere(Isometry3d::new(center, rotation), float(node, "radius", 1.0) * s, FORCE);
        }
        "force/orbit" => {
            let center = gt.transform_point(vec(node, "center", Vec3::ZERO));
            let axis = (rotation * vec(node, "axis", Vec3::Y)).normalize_or_zero();
            if axis == Vec3::ZERO {
                return;
            }
            let radius = float(node, "radius", 0.0) * s;
            if radius > 0.0 {
                let facing = Quat::from_rotation_arc(Vec3::Z, axis);
                gizmos.circle(Isometry3d::new(center, facing), radius, FORCE);
            }
            gizmos.arrow(center - axis * 0.5 * s, center + axis * 0.5 * s, FORCE);
        }
        "force/gravity_well" => {
            let point = gt.transform_point(vec(node, "point", Vec3::ZERO));
            gizmos.sphere(Isometry3d::new(point, rotation), float(node, "softening", 0.25).max(0.02) * s, FORCE);
        }
        _ => {}
    }
}

/// A collision node, in a cooler colour than the spawn shapes so the two read
/// apart: where particles start, and what they bounce off. In the entity's
/// space, or the world's with World Space set, as the simulation reads it.
fn draw_collider(gizmos: &mut Gizmos<OverlayGizmoGroup>, node: &VfxNode, gt: &GlobalTransform) {
    let world = matches!(node.input_values.get("world"), Some(PinValue::Bool(true)));
    let gt = if world { GlobalTransform::IDENTITY } else { *gt };
    let (scale, rotation, _) = gt.to_scale_rotation_translation();
    match node.node_type.as_str() {
        "collide/plane" => {
            let point = gt.transform_point(vec(node, "point", Vec3::ZERO));
            let normal = (gt.affine().inverse().matrix3.transpose() * vec(node, "normal", Vec3::Y)).normalize_or_zero();
            if normal == Vec3::ZERO {
                return;
            }
            // An infinite plane, shown as a square a couple of metres across
            // with its normal, which is the side particles stay on.
            let facing = Quat::from_rotation_arc(Vec3::Z, normal);
            gizmos.rect(Isometry3d::new(point, facing), Vec2::splat(2.0 * scale.max_element()), COLLIDER);
            gizmos.arrow(point, point + normal * 0.4 * scale.max_element(), COLLIDER);
        }
        "collide/sphere" => {
            let center = gt.transform_point(vec(node, "center", Vec3::ZERO));
            gizmos.sphere(Isometry3d::new(center, rotation), float(node, "radius", 1.0) * scale.max_element(), COLLIDER);
        }
        "collide/box" => {
            let center = gt.transform_point(vec(node, "center", Vec3::ZERO));
            let size = vec(node, "size", Vec3::ONE).abs();
            gizmos.cube(Transform { translation: center, rotation, scale: size * scale }, COLLIDER);
        }
        _ => {}
    }
}
