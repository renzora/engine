//! Turning a skeleton and a weighting rule into a skin on a model.
//!
//! Both rigs this crate builds (a face, a humanoid body) end the same way: some
//! bones, placed in the model's space, and for every vertex of every mesh a
//! rule saying which bones move it and how much. [`apply`] does that part for
//! both, so the rigs only have to decide where bones go and who follows whom.

use bevy::math::{Mat4, Quat, Vec3};
use serde_json::json;

use crate::glb::{trs_json, Glb, GlbError};

/// One bone, placed in the model's space (relative to the scene root).
#[derive(Clone, Debug)]
pub struct Bone {
    pub name: String,
    pub parent: Option<usize>,
    /// Where the joint is.
    pub head: Vec3,
    /// Its orientation in the model's space.
    pub rotation: Quat,
}

impl Bone {
    pub fn matrix(&self) -> Mat4 {
        Mat4::from_rotation_translation(self.rotation, self.head)
    }
}

/// A vertex being weighted, with what the rule may want to know about it.
pub struct Vertex<'a> {
    pub mesh_name: &'a str,
    pub node_name: &'a str,
    /// In the model's space.
    pub position: Vec3,
    /// Its index in [`Self::surface`]'s positions.
    pub index: usize,
    /// The primitive it belongs to, for a rule that has to know how vertices
    /// connect and not only where each one is: two lips can be a millimetre
    /// apart in space and far apart across the skin.
    pub surface: Option<&'a Surface>,
}

/// One primitive's geometry, in the model's space.
pub struct Surface {
    /// Which mesh and primitive this is, so a rule can work something out
    /// once per surface and reuse it for every vertex.
    pub mesh: usize,
    pub prim: usize,
    pub positions: Vec<Vec3>,
    /// Empty when the primitive's connectivity could not be read.
    pub triangles: Vec<[u32; 3]>,
}

/// What [`apply`] did, for the editor to report.
#[derive(Debug, Default)]
pub struct Applied {
    pub bones: usize,
    pub meshes: usize,
    pub vertices: usize,
}

/// Up to four influences, largest first, summing to 1. Bones not mentioned get
/// nothing; a vertex with no influence at all goes to `fallback`.
pub fn top_four(mut raw: Vec<(usize, f32)>, fallback: usize) -> [(u16, f32); 4] {
    raw.retain(|(_, w)| w.is_finite() && *w > 1e-4);
    raw.sort_by(|a, b| b.1.total_cmp(&a.1));
    raw.truncate(4);
    let sum: f32 = raw.iter().map(|(_, w)| w).sum();
    let mut out = [(fallback as u16, 0.0f32); 4];
    if sum <= 1e-6 {
        out[0] = (fallback as u16, 1.0);
        return out;
    }
    for (slot, (b, w)) in out.iter_mut().zip(raw) {
        *slot = (b as u16, w / sum);
    }
    out
}

/// Add `bones` to the model as joint nodes, and skin every mesh the rule
/// weights. `weigh` returns `(bone index, weight)` pairs for a vertex, or
/// `None` to leave that whole mesh unskinned (it is asked about each mesh's
/// first vertex). Weights need not sum to 1; [`top_four`] normalises them, and
/// `root` takes any vertex given nothing.
///
/// Refuses a model that already has a skin on a mesh this would weight: a
/// second skin over the first would leave two skeletons fighting over it.
pub fn apply(
    glb: &mut Glb,
    bones: &[Bone],
    root: usize,
    mut weigh: impl FnMut(&Vertex) -> Option<Vec<(usize, f32)>>,
) -> Result<Applied, GlbError> {
    let world = glb.world_matrices();

    // The joints, each local to its parent bone; root bones hang off the scene.
    let mut joint_nodes = Vec::with_capacity(bones.len());
    for (i, b) in bones.iter().enumerate() {
        let local = match b.parent {
            Some(p) if p < i => bones[p].matrix().inverse() * b.matrix(),
            _ => b.matrix(),
        };
        let (_, rotation, translation) = local.to_scale_rotation_translation();
        let parent = b.parent.filter(|&p| p < i).map(|p| joint_nodes[p]);
        joint_nodes.push(glb.add_node(trs_json(&b.name, translation, rotation), parent));
    }

    let mut applied = Applied { bones: bones.len(), ..Default::default() };
    let mesh_nodes: Vec<(usize, usize)> = glb
        .nodes()
        .iter()
        .enumerate()
        .filter_map(|(i, n)| Some((i, n.get("mesh")?.as_u64()? as usize)))
        .collect();
    for (node, mesh) in mesh_nodes {
        if joint_nodes.contains(&node) {
            continue;
        }
        let node_world = world.get(node).copied().unwrap_or(Mat4::IDENTITY);
        let mesh_name = glb.mesh_name(mesh);
        let node_name = glb.node_name(node);
        let prim_count = glb.meshes()[mesh].get("primitives").and_then(|p| p.as_array()).map_or(0, Vec::len);
        let mut skinned_any = false;
        for prim in 0..prim_count {
            let positions: Vec<Vec3> = glb.positions(mesh, prim)?.iter().map(|p| node_world.transform_point3(*p)).collect();
            if positions.is_empty() {
                continue;
            }
            let triangles = match glb.indices(mesh, prim)? {
                Some(idx) => idx.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect(),
                None => (0..positions.len() as u32 / 3).map(|t| [t * 3, t * 3 + 1, t * 3 + 2]).collect(),
            };
            let surface = Surface { mesh, prim, positions, triangles };
            let vertex = |index: usize| Vertex {
                mesh_name: &mesh_name,
                node_name: &node_name,
                position: surface.positions[index],
                index,
                surface: Some(&surface),
            };
            if weigh(&vertex(0)).is_none() {
                continue;
            }
            if glb.nodes()[node].get("skin").is_some() {
                return Err(GlbError::Unsupported(format!("`{node_name}` already has a skin; rigging it again would give it two skeletons")));
            }
            let influences: Vec<[(u16, f32); 4]> =
                (0..surface.positions.len()).map(|i| top_four(weigh(&vertex(i)).unwrap_or_default(), root)).collect();
            applied.vertices += influences.len();
            glb.set_skin_attributes(mesh, prim, &influences);
            skinned_any = true;
        }
        if skinned_any {
            // One skin per mesh node: the inverse bind matrices carry the
            // node's own placement, so the skinned mesh lands where it was.
            let inverse_binds: Vec<Mat4> = bones.iter().map(|b| b.matrix().inverse() * node_world).collect();
            let skin = glb.add_skin(&format!("{node_name}_skin"), &joint_nodes, &inverse_binds, joint_nodes.get(root).copied());
            glb.set_node_skin(node, skin);
            applied.meshes += 1;
        }
    }
    if applied.meshes == 0 {
        return Err(GlbError::Unsupported("no mesh in the model could be weighted".into()));
    }
    // Recorded so the editor can tell a rig it built from one it imported.
    glb.json["asset"]["extras"] = json!({ "renzora_rig": true });
    Ok(applied)
}

/// `0` at `edge0`, `1` at `edge1`, smooth between (either order).
pub fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if (edge1 - edge0).abs() < 1e-9 {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_four_normalises_and_keeps_the_largest() {
        let w = top_four(vec![(1, 2.0), (2, 1.0), (3, 0.5), (4, 0.25), (5, 0.25)], 0);
        assert_eq!(w[0].0, 1);
        let sum: f32 = w.iter().map(|(_, w)| w).sum();
        assert!((sum - 1.0).abs() < 1e-5);
        assert!(!w.iter().any(|(b, _)| *b == 5 || *b == 4) || w.iter().filter(|(b, _)| *b == 4 || *b == 5).count() == 1);
    }

    #[test]
    fn a_vertex_with_no_weight_goes_to_the_root() {
        let w = top_four(vec![], 7);
        assert_eq!(w[0], (7, 1.0));
    }
}
