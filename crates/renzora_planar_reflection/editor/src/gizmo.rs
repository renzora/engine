//! The planar reflection's viewport gizmo: which surface reflects, and which
//! way.
//!
//! The component reflects through its entity's own mesh, toward one local
//! axis, and from the scene alone neither is visible. Put on an entity with no
//! mesh (a particle system, say) it simply draws nothing, with nothing to say
//! why. So the selected reflection draws its mesh's outline in the reflecting
//! plane, an arrow along the normal it reflects toward, and turns red (and
//! logs once) when there is no mesh to reflect on.

use bevy::camera::primitives::Aabb;
use bevy::prelude::*;

use renzora::core::console_log::console_warn;
use renzora_editor_framework::EditorSelection;
use renzora_gizmo::OverlayGizmoGroup;
use renzora_planar_reflection::fit::MeshPlaneFit;
use renzora_planar_reflection::PlanarReflection;

const OK: Color = Color::srgb(0.45, 0.8, 1.0);
const BAD: Color = Color::srgb(1.0, 0.35, 0.3);
/// Amber: there is a mesh, but it is too deep to be one flat mirror.
const THICK: Color = Color::srgb(1.0, 0.72, 0.25);

pub(crate) fn draw_reflection_gizmos(
    mut gizmos: Gizmos<OverlayGizmoGroup>,
    selection: Res<EditorSelection>,
    surfaces: Query<(
        Entity,
        &GlobalTransform,
        &PlanarReflection,
        Option<&Mesh3d>,
        Option<&Aabb>,
        Option<&Name>,
        Option<&MeshPlaneFit>,
    )>,
    mut warned: Local<bevy::platform::collections::HashSet<Entity>>,
    mut warned_thick: Local<bevy::platform::collections::HashSet<Entity>>,
) {
    let selected = selection.get_all();
    for (entity, gt, reflection, mesh, aabb, name, fit) in &surfaces {
        // A planar reflection is one flat mirror. A mesh holding glass on
        // several walls has no single plane that is right for all of it, and
        // the reflection lands in the wrong place on the rest. Say so once.
        //
        // The fit decides when it found the glass: it knows the real plane,
        // and the bounds do not. A flat window wall turned off the axis (all
        // of Bistro's) has bounds metres deep along the axis and is still one
        // flat mirror, which the bounds test alone called thick.
        let fitted = fit.filter(|f| reflection.fit_to_mesh && f.found && f.axis == reflection.normal_axis);
        let thick = match (fitted, aabb) {
            (Some(f), _) => !f.flat,
            (None, Some(b)) => {
                let half = Vec3::from(b.half_extents) * gt.compute_transform().scale.abs();
                let local = reflection.normal_axis.local();
                let thickness = half.dot(local) * 2.0;
                let span = (half - half * local).max_element() * 2.0;
                thickness > (span * 0.1).max(0.05)
            }
            (None, None) => false,
        };
        // Through `console_warn`, not `warn!`: the editor Console shows only
        // its own log, and a warning in the terminal is one the person looking
        // at the amber gizmo never sees.
        if thick && warned_thick.insert(entity) {
            console_warn(
                "Planar Reflection",
                format!(
                    "'{}' is not one flat surface: it has glass facing more than one way, or sheets at different depths. \
                     The reflection is fitted to its largest sheet, and the rest reflects wrongly. \
                     Use a mesh with just the one window or floor, or lay a Mirror plane over it.",
                    name.map(|n| n.as_str()).unwrap_or("entity"),
                ),
            );
        }
        if !thick {
            warned_thick.remove(&entity);
        }
        if mesh.is_none() && warned.insert(entity) {
            console_warn(
                "Planar Reflection",
                format!(
                    "'{}' has a Planar Reflection but no mesh, so nothing reflects. \
                     Put the component on the surface that should reflect (a floor, window or Plane), \
                     not on the object in front of it.",
                    name.map(|n| n.as_str()).unwrap_or("entity")
                ),
            );
        }
        if !selected.contains(&entity) {
            continue;
        }
        // The plane the capture actually uses (fitted, flipped, offset), so
        // what is drawn is what reflects.
        let Some(plane) = renzora_planar_reflection::math::ReflectionPlane::for_surface(gt, reflection, aabb, fit) else {
            continue;
        };
        let normal = plane.normal;
        let color = if mesh.is_none() { BAD } else if thick { THICK } else { OK };

        // Two in-plane axes from the plane's own normal, which after a fit
        // need not be any of the entity's axes. Upright planes keep "up" as
        // one edge so a window's outline reads as a window.
        let up = if normal.y.abs() > 0.9 { Vec3::X } else { Vec3::Y };
        let u = up.cross(normal).normalize();
        let v = normal.cross(u);

        // The surface's extent in the plane: the mesh bounds' corners laid
        // onto it, boxed in those axes. Without a mesh there is nothing to
        // outline, so a 1 m square marks the plane that would reflect.
        let (centre, half) = match aabb {
            Some(b) => (Vec3::from(b.center), Vec3::from(b.half_extents)),
            None => (Vec3::ZERO, Vec3::splat(0.5)),
        };
        let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        for i in 0..8 {
            let sign = Vec3::new(
                if i & 1 == 0 { -1.0 } else { 1.0 },
                if i & 2 == 0 { -1.0 } else { 1.0 },
                if i & 4 == 0 { -1.0 } else { 1.0 },
            );
            let d = gt.transform_point(centre + half * sign) - plane.point;
            let p = Vec2::new(d.dot(u), d.dot(v));
            lo = lo.min(p);
            hi = hi.max(p);
        }
        let at = |a: f32, b: f32| plane.point + u * a + v * b;
        gizmos.linestrip([at(lo.x, lo.y), at(hi.x, lo.y), at(hi.x, hi.y), at(lo.x, hi.y), at(lo.x, lo.y)], color);

        // The normal: the side the reflection faces. A camera behind the
        // surface sees only its material.
        let mid = (lo + hi) * 0.5;
        let origin = at(mid.x, mid.y);
        let reach = ((hi - lo).max_element() * 0.5).clamp(0.5, 3.0);
        gizmos.arrow(origin, origin + normal * reach, color);
    }
}
