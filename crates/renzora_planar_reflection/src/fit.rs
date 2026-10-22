//! Fitting the reflection plane to the mesh's actual glass.
//!
//! [`PlanarReflection::fit_to_mesh`] used to put the plane through the middle
//! of the mesh's bounds, facing exactly along the chosen local axis. Imported
//! scenes break both halves of that. Bistro's `MASTER_Glass_Exterior` is a
//! row of three panes on a wall turned 21.5 degrees off the mesh's Z axis: the
//! bounds are 3 m deep along Z although the glass is flat, and a mirror facing
//! straight down Z reflected the table beside it rotated and metres off.
//!
//! So the fit reads the triangles and finds the plane the glass is actually
//! in, normal included. The chosen axis only says which glass to look for:
//! faces within [`MAX_TILT`] of it count, the plane takes the most common
//! direction among them by area, and then the biggest sheet along that
//! direction, which for a merged facade is the wall with the most glass.
//! Which way that sheet's faces point (by area) sets the side the reflection
//! faces, unless the panes are double sided and the two cancel out.
//!
//! Two passes over hashed bins keep it linear in the triangle count: a merged
//! building mesh can carry hundreds of thousands of them.

use std::collections::HashMap;

use bevy::mesh::{Indices, VertexAttributeValues};
use bevy::prelude::*;

use crate::component::{NormalAxis, PlanarReflection};

/// The plane found in a surface's mesh, in its local space. Cached on the
/// entity and recomputed when the mesh or the axis changes.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct MeshPlaneFit {
    pub mesh: AssetId<Mesh>,
    pub axis: NormalAxis,
    /// A point on the sheet, local space.
    pub local_point: Vec3,
    /// The sheet's unit normal in local space, already turned toward the side
    /// its faces point.
    pub local_normal: Vec3,
    /// Whether that one sheet is (nearly) all of the mesh. False for merged
    /// glass across several walls, where the gizmo says only part of it
    /// reflects correctly.
    pub flat: bool,
    /// False when the mesh had no readable faces near the axis. Kept rather
    /// than removed so a big mesh is not re-scanned every frame; readers then
    /// use the bounds' middle and the axis.
    pub found: bool,
}

/// The result of [`fit_mesh`], before it is tied to an entity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaneFit {
    pub point: Vec3,
    pub normal: Vec3,
    pub flat: bool,
}

/// How far a face may lean from the chosen axis and still be glass facing
/// that way, as the cosine: 60 degrees, so a wall turned well off the axis is
/// still found while the walls at right angles to it are not.
const MAX_TILT: f32 = 0.5;
/// Direction bin size per normal component. Panes of one wall agree to well
/// under this; a wall a few degrees round is another direction.
const DIR_BIN: f32 = 0.02;
/// How closely a face must match the chosen direction to be in its sheet.
const SAME_DIR: f32 = 0.996;
/// Bin width along the normal, in local units: panes of one wall within a
/// couple of centimetres of each other are one sheet.
const BIN: f32 = 0.02;
/// The share of the mesh's area one sheet must hold to count as the whole.
const FLAT_SHARE: f32 = 0.9;

/// Find the dominant sheet of glass near `axis` in `mesh`. `None` when the
/// mesh data is not readable on the CPU or has no faces near that axis; the
/// caller then falls back to the bounds' middle.
pub fn fit_mesh(mesh: &Mesh, axis: NormalAxis) -> Option<PlaneFit> {
    let Ok(VertexAttributeValues::Float32x3(positions)) = mesh.try_attribute(Mesh::ATTRIBUTE_POSITION) else {
        return None;
    };
    let tri = |a: usize, b: usize, c: usize| -> Option<(Vec3, Vec3, f32)> {
        let (pa, pb, pc) = (Vec3::from(*positions.get(a)?), Vec3::from(*positions.get(b)?), Vec3::from(*positions.get(c)?));
        let cross = (pb - pa).cross(pc - pa);
        let area = cross.length() * 0.5;
        (area > 1e-9).then(|| ((pa + pb + pc) / 3.0, cross / (area * 2.0), area))
    };
    let mut tris: Vec<(Vec3, Vec3, f32)> = Vec::new();
    match mesh.try_indices_option().ok()? {
        Some(Indices::U32(ix)) => tris.extend(ix.chunks_exact(3).filter_map(|t| tri(t[0] as usize, t[1] as usize, t[2] as usize))),
        Some(Indices::U16(ix)) => tris.extend(ix.chunks_exact(3).filter_map(|t| tri(t[0] as usize, t[1] as usize, t[2] as usize))),
        None => tris.extend((0..positions.len() / 3).filter_map(|i| tri(i * 3, i * 3 + 1, i * 3 + 2))),
    }
    let total_area: f32 = tris.iter().map(|t| t.2).sum();
    let a = axis.local();

    // Pass 1: the most common direction near the axis. Normals are folded
    // onto the axis's side first so the two faces of double-sided glass vote
    // for one direction, not two.
    let fold = |n: Vec3| if n.dot(a) < 0.0 { -n } else { n };
    let mut dirs: HashMap<(i32, i32, i32), (f32, Vec3)> = HashMap::new();
    for &(_, n, area) in &tris {
        let n = fold(n);
        if n.dot(a) < MAX_TILT {
            continue;
        }
        let k = ((n.x / DIR_BIN).round() as i32, (n.y / DIR_BIN).round() as i32, (n.z / DIR_BIN).round() as i32);
        let e = dirs.entry(k).or_insert((0.0, Vec3::ZERO));
        e.0 += area;
        e.1 += n * area;
    }
    let neighbourhood = |k: (i32, i32, i32)| {
        let mut sum = (0.0, Vec3::ZERO);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(e) = dirs.get(&(k.0 + dx, k.1 + dy, k.2 + dz)) {
                        sum.0 += e.0;
                        sum.1 += e.1;
                    }
                }
            }
        }
        sum
    };
    let best_dir = dirs.keys().map(|&k| (k, neighbourhood(k).0)).max_by(|x, y| x.1.total_cmp(&y.1))?.0;
    let dir = neighbourhood(best_dir).1.normalize_or_zero();
    if dir == Vec3::ZERO {
        return None;
    }

    // Pass 2: the biggest sheet along that direction.
    // bin -> (area, signed area, area-weighted centroid sum)
    let mut bins: HashMap<i64, (f32, f32, Vec3)> = HashMap::new();
    for &(centroid, n, area) in &tris {
        let along = n.dot(dir);
        if along.abs() < SAME_DIR {
            continue;
        }
        let e = bins.entry((centroid.dot(dir) / BIN).round() as i64).or_insert((0.0, 0.0, Vec3::ZERO));
        e.0 += area;
        e.1 += area * along.signum();
        e.2 += centroid * area;
    }
    // Merge each bin with its neighbours so a sheet straddling a bin edge
    // still counts as one, then take the biggest.
    let best = bins
        .keys()
        .map(|k| (*k, (k - 1..=k + 1).filter_map(|j| bins.get(&j)).map(|b| b.0).sum::<f32>()))
        .max_by(|x, y| x.1.total_cmp(&y.1))?
        .0;
    let (mut area, mut signed, mut sum) = (0.0, 0.0, Vec3::ZERO);
    for b in (best - 1..=best + 1).filter_map(|j| bins.get(&j)) {
        area += b.0;
        signed += b.1;
        sum += b.2;
    }
    let outward = if signed.abs() > area * 0.5 { signed.signum() } else { 1.0 };
    Some(PlaneFit {
        point: sum / area,
        normal: dir * outward,
        flat: area >= total_area * FLAT_SHARE,
    })
}

/// Keep each surface's [`MeshPlaneFit`] current with its mesh and axis.
pub fn update_plane_fits(
    mut commands: Commands,
    meshes: Res<Assets<Mesh>>,
    surfaces: Query<(Entity, &PlanarReflection, &Mesh3d, Option<&MeshPlaneFit>)>,
) {
    for (entity, reflection, mesh, fit) in &surfaces {
        let id = mesh.0.id();
        if fit.is_some_and(|f| f.mesh == id && f.axis == reflection.normal_axis) {
            continue;
        }
        let Some(data) = meshes.get(id) else { continue };
        let axis = reflection.normal_axis;
        let fit = match fit_mesh(data, axis) {
            Some(f) => MeshPlaneFit { mesh: id, axis, local_point: f.point, local_normal: f.normal, flat: f.flat, found: true },
            None => MeshPlaneFit { mesh: id, axis, local_point: Vec3::ZERO, local_normal: axis.local(), flat: false, found: false },
        };
        commands.entity(entity).try_insert(fit);
    }
}
