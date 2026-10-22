//! Structural checks on every generator in this directory.
//!
//! None of this needs a GPU: a mesh that is closed, outward-wound and free of
//! slivers is checkable from its arrays, and those are exactly the defects
//! that show on screen as a black facet, a see-through gap or a stray line.

use bevy::mesh::{Indices, VertexAttributeValues};
use bevy::prelude::*;

use super::*;

fn all() -> Vec<(&'static str, Mesh)> {
    vec![
        ("tetrahedron", create_tetrahedron_mesh()),
        ("octahedron", create_octahedron_mesh()),
        ("dodecahedron", create_dodecahedron_mesh()),
        ("icosahedron", create_icosahedron_mesh()),
        ("rounded_cube", create_rounded_cube_mesh()),
        ("disc", create_disc_mesh()),
        ("star_prism", create_star_prism_mesh()),
        ("ellipsoid", create_ellipsoid_mesh()),
        ("hex_prism", create_hex_prism_mesh()),
        ("domed_cylinder", create_domed_cylinder_mesh()),
        ("beveled_block", create_beveled_block_mesh()),
        ("step_block", create_step_block_mesh()),
        ("inner_corner_wedge", create_inner_corner_wedge_mesh()),
        ("outer_corner_wedge", create_outer_corner_wedge_mesh()),
        ("slope_2_1", create_slope_2_1_mesh()),
        ("rounded_edge_block", create_rounded_edge_block_mesh()),
        ("gear", create_gear_mesh()),
        ("frame", create_frame_mesh()),
        ("column", create_column_mesh()),
        ("gable_roof", create_gable_roof_mesh()),
        ("hip_roof", create_hip_roof_mesh()),
        ("dome_shell", create_dome_shell_mesh()),
        ("archway", create_archway_mesh()),
        ("crenellated_wall", create_crenellated_wall_mesh()),
        ("railing", create_railing_mesh()),
        ("fence", create_fence_mesh()),
        ("window_frame", create_window_frame_mesh()),
        ("door_frame", create_door_frame_mesh()),
        ("wall_corner", create_wall_corner_mesh()),
        ("floor_tile", create_floor_tile_mesh()),
        ("balcony", create_balcony_mesh()),
        ("l_stairs", create_l_stairs_mesh()),
        ("u_stairs", create_u_stairs_mesh()),
        ("curved_ramp", create_curved_ramp_mesh()),
        ("ladder", create_ladder_mesh()),
        ("slab_stairs", create_slab_stairs_mesh()),
        ("pipe_elbow", create_pipe_elbow_mesh()),
        ("pipe_tee", create_pipe_tee_mesh()),
        ("pipe_cross", create_pipe_cross_mesh()),
        ("spring", create_spring_mesh()),
        ("torus_knot", create_torus_knot_mesh()),
        ("pipe_flange", create_pipe_flange_mesh()),
        ("channel_bend", create_channel_bend_mesh()),
        ("crate", create_crate_mesh()),
        ("barrel", create_barrel_mesh()),
        ("platform", create_platform_mesh()),
        ("bridge", create_bridge_mesh()),
        ("bench", create_bench_mesh()),
        ("table", create_table_mesh()),
        ("pedestal", create_pedestal_mesh()),
        ("rock", create_rock_mesh()),
        ("sign_post", create_sign_post_mesh()),
        ("lamp_post", create_lamp_post_mesh()),
    ]
}

fn vec3s(mesh: &Mesh, attr: bevy::mesh::MeshVertexAttribute) -> Vec<Vec3> {
    match mesh.attribute(attr) {
        Some(VertexAttributeValues::Float32x3(v)) => v.iter().map(|p| Vec3::from(*p)).collect(),
        _ => panic!("missing Float32x3 attribute {}", attr.name),
    }
}

fn uvs(mesh: &Mesh) -> Vec<Vec2> {
    match mesh.attribute(Mesh::ATTRIBUTE_UV_0) {
        Some(VertexAttributeValues::Float32x2(v)) => v.iter().map(|p| Vec2::from(*p)).collect(),
        _ => panic!("missing UV_0"),
    }
}

fn indices(mesh: &Mesh) -> Vec<u32> {
    match mesh.indices() {
        Some(Indices::U32(v)) => v.clone(),
        Some(Indices::U16(v)) => v.iter().map(|&i| i as u32).collect(),
        None => panic!("mesh has no indices"),
    }
}

fn bounds(p: &[Vec3]) -> (Vec3, Vec3) {
    p.iter().fold((Vec3::MAX, Vec3::MIN), |(lo, hi), q| (lo.min(*q), hi.max(*q)))
}

#[test]
fn every_mesh_is_well_formed() {
    for (name, mesh) in all() {
        let p = vec3s(&mesh, Mesh::ATTRIBUTE_POSITION);
        let n = vec3s(&mesh, Mesh::ATTRIBUTE_NORMAL);
        let uv = uvs(&mesh);
        let idx = indices(&mesh);
        assert!(!p.is_empty() && !idx.is_empty(), "{name}: empty mesh");
        assert_eq!(p.len(), n.len(), "{name}: position/normal count mismatch");
        assert_eq!(p.len(), uv.len(), "{name}: position/uv count mismatch");
        assert_eq!(idx.len() % 3, 0, "{name}: index count not a multiple of 3");
        assert!(idx.iter().all(|&i| (i as usize) < p.len()), "{name}: index out of range");
        assert!(p.iter().all(|v| v.is_finite()), "{name}: non-finite position");
        assert!(uv.iter().all(|v| v.is_finite()), "{name}: non-finite uv");
        for v in &n {
            assert!(v.is_finite() && (v.length() - 1.0).abs() < 1e-3, "{name}: bad normal {v:?}");
        }
        // Compaction leaves no vertex that no triangle uses.
        let mut used = vec![false; p.len()];
        idx.iter().for_each(|&i| used[i as usize] = true);
        assert!(used.iter().all(|u| *u), "{name}: unreferenced vertices");
    }
}

/// No sliver triangles in space or in UV space, and every triangle faces the
/// way its normals say. A zero-area UV triangle samples one texel across its
/// whole face, which reads as a smear.
#[test]
fn triangles_are_solid_and_outward() {
    for (name, mesh) in all() {
        let p = vec3s(&mesh, Mesh::ATTRIBUTE_POSITION);
        let n = vec3s(&mesh, Mesh::ATTRIBUTE_NORMAL);
        let uv = uvs(&mesh);
        for t in indices(&mesh).chunks(3) {
            let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
            let face = (p[b] - p[a]).cross(p[c] - p[a]);
            assert!(face.length() > 1e-7, "{name}: degenerate triangle {t:?}");
            let uv_area = (uv[b] - uv[a]).perp_dot(uv[c] - uv[a]).abs();
            assert!(uv_area > 1e-9, "{name}: degenerate uv triangle {t:?}");
            assert!(
                face.dot(n[a] + n[b] + n[c]) > 0.0,
                "{name}: triangle {t:?} wound against its normals"
            );
        }
    }
}

/// Closed and outward: the area-weighted normals of a closed surface sum to
/// zero (holes leave a remainder), and its signed volume is positive only if
/// the faces point out. Both hold for a union of closed parts as well, which
/// is what the box-built props are.
#[test]
fn every_mesh_is_closed_with_positive_volume() {
    for (name, mesh) in all() {
        let p = vec3s(&mesh, Mesh::ATTRIBUTE_POSITION);
        let mut sum = Vec3::ZERO;
        let mut area = 0.0;
        let mut volume = 0.0;
        for t in indices(&mesh).chunks(3) {
            let (a, b, c) = (p[t[0] as usize], p[t[1] as usize], p[t[2] as usize]);
            let cross = (b - a).cross(c - a);
            sum += cross;
            area += cross.length();
            volume += a.dot(b.cross(c)) / 6.0;
        }
        assert!(sum.length() < 1e-4 * area, "{name}: surface not closed (open area {})", sum.length() / 2.0);
        assert!(volume > 0.0, "{name}: signed volume {volume} is not positive");
    }
}

/// Every shape except the hub-pivoted pipe fittings is centred on its bounds,
/// like the original shapes, so it lands centred where it is dropped.
#[test]
fn pivots_follow_the_bounds_centre_convention() {
    for (name, mesh) in all() {
        if name == "pipe_tee" || name == "pipe_cross" {
            continue;
        }
        let (lo, hi) = bounds(&vec3s(&mesh, Mesh::ATTRIBUTE_POSITION));
        assert!(((lo + hi) * 0.5).length() < 1e-4, "{name}: bounds centre {:?}", (lo + hi) * 0.5);
    }
}

/// The fittings take the Pipe shape's bore: open ends at 0.5 m outside and
/// 0.35 m inside, square to the axis, so a placed Pipe meets them flush.
#[test]
fn tee_openings_match_the_pipe_bore() {
    let p = vec3s(&create_pipe_tee_mesh(), Mesh::ATTRIBUTE_POSITION);
    let end: Vec<Vec3> = p.iter().copied().filter(|v| (v.x - 1.0).abs() < 1e-5).collect();
    assert!(!end.is_empty(), "tee has no +X opening at x = 1");
    for v in &end {
        let r = Vec2::new(v.y, v.z).length();
        assert!((r - 0.5).abs() < 1e-4 || (r - 0.35).abs() < 1e-4, "opening radius {r}");
    }
    let (lo, hi) = bounds(&p);
    assert!((lo.x + 1.0).abs() < 1e-4 && (hi.x - 1.0).abs() < 1e-4 && (hi.y - 1.0).abs() < 1e-4);
}

#[test]
fn elbow_ends_are_square_to_their_axes() {
    let p = vec3s(&create_pipe_elbow_mesh(), Mesh::ATTRIBUTE_POSITION);
    let (lo, hi) = bounds(&p);
    let bottom: Vec<&Vec3> = p.iter().filter(|v| (v.y - lo.y).abs() < 1e-4).collect();
    let side: Vec<&Vec3> = p.iter().filter(|v| (v.x - hi.x).abs() < 1e-4).collect();
    // A square-cut end puts the whole rim (24 outer + 24 inner corners) on
    // one plane; a tilted one would leave a single point on the extreme.
    assert!(bottom.len() >= 48, "bottom end has {} rim points on its plane", bottom.len());
    assert!(side.len() >= 48, "side end has {} rim points on its plane", side.len());
}

/// The rock is seeded, so a saved scene reopens with the same rock.
#[test]
fn rock_is_deterministic() {
    let a = vec3s(&create_rock_mesh(), Mesh::ATTRIBUTE_POSITION);
    let b = vec3s(&create_rock_mesh(), Mesh::ATTRIBUTE_POSITION);
    assert_eq!(a, b);
}

#[test]
fn polyhedra_have_their_face_counts() {
    // Flat shaded with one normal per face, so distinct normals count faces.
    for (name, mesh, faces) in [
        ("tetrahedron", create_tetrahedron_mesh(), 4),
        ("octahedron", create_octahedron_mesh(), 8),
        ("dodecahedron", create_dodecahedron_mesh(), 12),
        ("icosahedron", create_icosahedron_mesh(), 20),
        ("beveled_block", create_beveled_block_mesh(), 26),
    ] {
        let mut normals: Vec<Vec3> = Vec::new();
        for n in vec3s(&mesh, Mesh::ATTRIBUTE_NORMAL) {
            if !normals.iter().any(|m| m.distance(n) < 1e-4) {
                normals.push(n);
            }
        }
        assert_eq!(normals.len(), faces, "{name}");
    }
}
