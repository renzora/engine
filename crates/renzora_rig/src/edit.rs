//! Editing a skeleton by hand: reading the bones a model has, and building
//! the model again with a changed set.
//!
//! The templates (humanoid, face, chain) place bones from a body plan. Any
//! other skeleton (a quadruped, a prop with a hinge, a creature, a rig that
//! needs one more bone) is made by adding, moving, renaming, reparenting and
//! deleting bones. [`rebuild`] then rigs the model's original again with the
//! edited bones and fresh weights, measured across the skin
//! ([`crate::weights::surface_raw`]): moving a joint changes which skin is
//! nearest which bone, so weights from the old layout would be wrong.
//!
//! An imported skeleton is edited the same way: [`strip_skin`] takes the old
//! skin off the original first, leaving the mesh where the skin held it, and
//! the edited bones go on in its place. The original file itself is kept, as
//! with every rig, so Remove Rig puts the imported skeleton back.

use std::collections::{HashMap, HashSet};

use bevy::math::{Mat4, Quat, Vec3};
use serde_json::{json, Value};

use crate::glb::{Glb, GlbError};
use crate::skin::{self, Applied, Bone};
use crate::weights::{self, Segment};

/// The model's skeleton: the joints of its skins in their rest places (model
/// space), every parent before its children.
pub fn read_bones(glb: &Glb) -> Vec<Bone> {
    let joints: Vec<usize> = joint_nodes(glb).into_iter().collect();
    let world = glb.world_matrices();
    let parents = glb.parents();
    let set: HashSet<usize> = joints.iter().copied().collect();
    // Nearest joint above each joint.
    let joint_parent = |n: usize| {
        let mut p = parents.get(n).copied().flatten();
        while let Some(x) = p {
            if set.contains(&x) {
                return Some(x);
            }
            p = parents.get(x).copied().flatten();
        }
        None
    };
    // Parents first: roots, then each layer below.
    let mut order: Vec<usize> = Vec::new();
    let mut placed: HashSet<usize> = HashSet::new();
    while order.len() < joints.len() {
        let before = order.len();
        for &j in &joints {
            if !placed.contains(&j) && joint_parent(j).is_none_or(|p| placed.contains(&p)) {
                order.push(j);
                placed.insert(j);
            }
        }
        if order.len() == before {
            break;
        }
    }
    let index: HashMap<usize, usize> = order.iter().enumerate().map(|(i, n)| (*n, i)).collect();
    order
        .iter()
        .map(|&n| {
            let (_, rotation, head) = world[n].to_scale_rotation_translation();
            Bone { name: glb.node_name(n), parent: joint_parent(n).and_then(|p| index.get(&p).copied()), head, rotation }
        })
        .collect()
}

/// Every node that is a joint of some skin.
fn joint_nodes(glb: &Glb) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    for skin in glb.skins() {
        for j in skin.get("joints").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_u64) {
            if !out.contains(&(j as usize)) {
                out.push(j as usize);
            }
        }
    }
    out
}

/// Take every skin off the model: the skins, the per-vertex joints and
/// weights, the joint nodes and the animations that drove them.
///
/// A skinned mesh is drawn where its skin puts it, not where its node is, so
/// each skinned mesh node is first moved to where the skin held it at rest
/// (its first joint's rest place times that joint's inverse bind matrix);
/// without that, a model whose armature carried a scale or a turn (a Mixamo
/// export in centimetres) would jump when the skin came off. A node that hung
/// under a joint and is not one (a sword in a hand) is kept, moved up to the
/// scene at its rest place.
pub fn strip_skin(glb: &mut Glb) -> Result<(), GlbError> {
    let joints: HashSet<usize> = joint_nodes(glb).into_iter().collect();
    if joints.is_empty() {
        return Ok(());
    }
    let world = glb.world_matrices();
    let parents = glb.parents();

    // Where each skinned mesh sits at rest.
    let mut placements: Vec<(usize, Mat4)> = Vec::new();
    for (node, n) in glb.nodes().iter().enumerate() {
        let Some(skin) = n.get("skin").and_then(Value::as_u64) else { continue };
        let Some(skin_json) = glb.skins().get(skin as usize) else { continue };
        let Some(first) = skin_json.get("joints").and_then(Value::as_array).and_then(|j| j.first()).and_then(Value::as_u64) else { continue };
        let ibm = skin_json
            .get("inverseBindMatrices")
            .and_then(Value::as_u64)
            .and_then(|a| glb.mat4s(a as usize).ok())
            .and_then(|m| m.first().copied())
            .unwrap_or(Mat4::IDENTITY);
        placements.push((node, world[first as usize] * ibm));
    }
    for (node, at) in placements {
        let parent_world = parents.get(node).copied().flatten().map_or(Mat4::IDENTITY, |p| world[p]);
        set_local(&mut glb.json["nodes"][node], parent_world.inverse() * at);
        glb.json["nodes"][node].as_object_mut().map(|o| o.remove("skin"));
    }

    // Lift the non-joint children of joints to the scene, at rest.
    let mut lifted: Vec<usize> = Vec::new();
    for &j in &joints {
        let kids: Vec<usize> = glb.nodes()[j].get("children").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_u64).map(|k| k as usize).collect();
        for k in kids.into_iter().filter(|k| !joints.contains(k)) {
            set_local(&mut glb.json["nodes"][k], world[k]);
            lifted.push(k);
        }
    }
    // Detach every joint from its parent and from the scenes. The nodes stay
    // in the array, unreferenced, so no other index moves.
    let detach = |list: &mut Value| {
        if let Some(a) = list.as_array_mut() {
            a.retain(|v| v.as_u64().is_none_or(|i| !joints.contains(&(i as usize))));
        }
    };
    for node in glb.json["nodes"].as_array_mut().into_iter().flatten() {
        if let Some(children) = node.get_mut("children") {
            detach(children);
        }
    }
    for scene in glb.json["scenes"].as_array_mut().into_iter().flatten() {
        if let Some(roots) = scene.get_mut("nodes") {
            detach(roots);
        }
    }
    for &j in &joints {
        glb.json["nodes"][j]["children"] = json!([]);
    }
    let scene = glb.json.get("scene").and_then(Value::as_u64).unwrap_or(0) as usize;
    for k in lifted {
        if let Some(roots) = glb.json["scenes"][scene].get_mut("nodes").and_then(Value::as_array_mut) {
            roots.push(json!(k));
        }
    }
    for mesh in glb.json["meshes"].as_array_mut().into_iter().flatten() {
        for prim in mesh.get_mut("primitives").and_then(Value::as_array_mut).into_iter().flatten() {
            if let Some(attrs) = prim.get_mut("attributes").and_then(Value::as_object_mut) {
                attrs.retain(|k, _| !k.starts_with("JOINTS_") && !k.starts_with("WEIGHTS_"));
            }
        }
    }
    // Clips here keyed the old joints; the model's clips live in .anim files.
    glb.json.as_object_mut().map(|o| o.remove("animations"));
    glb.json["skins"] = json!([]);
    Ok(())
}

fn set_local(node: &mut Value, m: Mat4) {
    let (s, r, t) = m.to_scale_rotation_translation();
    if let Some(o) = node.as_object_mut() {
        o.remove("matrix");
    }
    node["translation"] = json!([t.x, t.y, t.z]);
    node["rotation"] = json!([r.x, r.y, r.z, r.w]);
    node["scale"] = json!([s.x, s.y, s.z]);
}

/// Turn each edited bone so it still points at its first child: a moved
/// joint changes the direction of its own bone and of its parent's.
///
/// `before[i]` is bone `i` as it was (`None` for a bone added since). A bone
/// keeps its old twist, swung by the change in direction; a new bone points
/// +Y at its child (or along its parent, as a leaf), the way the generated
/// rigs' bones do.
pub fn aim(before: &[Option<Bone>], after: &mut [Bone]) {
    let first_child = |bones: &[Bone], i: usize| bones.iter().position(|b| b.parent == Some(i));
    for i in 0..after.len() {
        let new_dir = first_child(after, i).and_then(|c| (after[c].head - after[i].head).try_normalize());
        match (&before[i], new_dir) {
            (Some(old), Some(new_dir)) => {
                // The old direction to the same child, when it was a child then.
                let old_dir = first_child(after, i)
                    .and_then(|c| before.get(c).cloned().flatten())
                    .and_then(|c| (c.head - old.head).try_normalize())
                    .unwrap_or(old.rotation * Vec3::Y);
                after[i].rotation = (Quat::from_rotation_arc(old_dir, new_dir) * old.rotation).normalize();
            }
            (Some(old), None) => after[i].rotation = old.rotation,
            (None, Some(new_dir)) => after[i].rotation = Quat::from_rotation_arc(Vec3::Y, new_dir),
            (None, None) => {
                let along = after[i].parent.map(|p| after[p].rotation).unwrap_or(Quat::IDENTITY);
                after[i].rotation = along;
            }
        }
    }
}

/// One vertex's unnormalised `(bone, weight)` pairs.
type RawWeights = Vec<(usize, f32)>;

/// Rig `glb` (the model's original) again with `bones`, weighted across the
/// skin. Bones must list every parent before its children.
pub fn rebuild(glb: &mut Glb, bones: &[Bone]) -> Result<Applied, GlbError> {
    if bones.is_empty() {
        return Err(GlbError::Unsupported("a skeleton needs at least one bone".into()));
    }
    if bones.iter().enumerate().any(|(i, b)| b.parent.is_some_and(|p| p >= i)) {
        return Err(GlbError::Unsupported("every bone must come after its parent".into()));
    }
    strip_skin(glb)?;
    let size = model_size(glb)?;
    let segments: Vec<Segment> = bones
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let tail = bones.iter().find(|c| c.parent == Some(i)).map(|c| c.head).unwrap_or(b.head + b.rotation * Vec3::Y * size * 0.05);
            Segment { joint: i as u16, a: b.head, b: tail }
        })
        .collect();
    let mut across: HashMap<(usize, usize), Vec<RawWeights>> = HashMap::new();
    let root = bones.iter().position(|b| b.parent.is_none()).unwrap_or(0);
    skin::apply(glb, bones, root, |v| {
        let raw = match v.surface.filter(|s| !s.triangles.is_empty()) {
            Some(s) => across
                .entry((s.mesh, s.prim))
                .or_insert_with(|| {
                    let indices: Vec<u32> = s.triangles.iter().flatten().copied().collect();
                    weights::surface_raw(&s.positions, &indices, &segments, 6.0)
                })
                .get(v.index)
                .cloned()
                .unwrap_or_default(),
            None => weights::envelope(&[v.position], &segments, 6.0)[0].iter().map(|(j, w)| (*j as usize, *w)).collect(),
        };
        Some(raw)
    })
}

/// The diagonal of the model's meshes' bounds.
fn model_size(glb: &Glb) -> Result<f32, GlbError> {
    let world = glb.world_matrices();
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for (node, n) in glb.nodes().iter().enumerate() {
        let Some(mesh) = n.get("mesh").and_then(Value::as_u64) else { continue };
        let prims = glb.meshes()[mesh as usize].get("primitives").and_then(Value::as_array).map_or(0, Vec::len);
        for prim in 0..prims {
            for p in glb.positions(mesh as usize, prim)? {
                let p = world[node].transform_point3(p);
                lo = lo.min(p);
                hi = hi.max(p);
            }
        }
    }
    Ok((hi - lo).length().max(1e-4))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::{self, ChainSpec};
    use crate::glb::Geometry;

    /// A strip two units tall, rigged with a three-bone chain.
    fn rigged_strip() -> Glb {
        let mut positions = Vec::new();
        let mut indices = Vec::new();
        for i in 0..=8 {
            let y = i as f32 * 0.25;
            positions.extend([Vec3::new(-0.2, y, 0.0), Vec3::new(0.2, y, 0.0)]);
            if i < 8 {
                let b = (i * 2) as u32;
                indices.extend([b, b + 1, b + 2, b + 1, b + 3, b + 2]);
            }
        }
        let g = Geometry { name: "strip", positions: &positions, normals: &[], uvs: &[], indices: &indices, base_color: [1.0; 4] };
        let mut glb = Glb::from_geometry(&g).expect("written");
        chain::build(&mut glb, &ChainSpec { start: Vec3::ZERO, end: Vec3::Y * 2.0, segments: 3 }).expect("rigged");
        Glb::parse(&glb.to_bytes()).expect("reparsed")
    }

    #[test]
    fn a_skeleton_reads_back_parents_first() {
        let bones = read_bones(&rigged_strip());
        assert_eq!(bones.len(), 4);
        assert_eq!(bones[0].parent, None);
        for (i, b) in bones.iter().enumerate() {
            assert!(b.parent.is_none_or(|p| p < i), "{} before its parent", b.name);
        }
        assert!((bones[3].head.y - 2.0).abs() < 1e-4);
    }

    /// Stripping leaves a plain mesh: no skins, no joint attributes, no joint
    /// in the scene, the mesh still there.
    #[test]
    fn stripping_leaves_the_mesh_alone() {
        let mut glb = rigged_strip();
        strip_skin(&mut glb).expect("stripped");
        assert!(glb.skins().is_empty());
        let attrs = &glb.json["meshes"][0]["primitives"][0]["attributes"];
        assert!(attrs.get("JOINTS_0").is_none() && attrs.get("POSITION").is_some());
        let roots = glb.json["scenes"][0]["nodes"].as_array().expect("roots").clone();
        assert_eq!(roots.len(), 1, "only the mesh is left in the scene: {roots:?}");
    }

    /// Adding a bone and moving a joint, then rebuilding, gives the edited
    /// skeleton, with the moved bone's parent turned to point at it.
    #[test]
    fn an_edited_skeleton_rebuilds() {
        let glb = rigged_strip();
        let before = read_bones(&glb);
        let mut after = before.clone();
        after[2].head += Vec3::X * 0.5;
        after.push(Bone { name: "fin".into(), parent: Some(1), head: Vec3::new(0.3, 1.0, 0.0), rotation: Quat::IDENTITY });
        let mut was: Vec<Option<Bone>> = before.iter().cloned().map(Some).collect();
        was.push(None);
        aim(&was, &mut after);
        let pointing = after[1].rotation * Vec3::Y;
        let to_child = (after[2].head - after[1].head).normalize();
        assert!(pointing.dot(to_child) > 0.999, "{pointing:?} vs {to_child:?}");
        let mut rebuilt = glb;
        let applied = rebuild(&mut rebuilt, &after).expect("rebuilt");
        assert_eq!(applied.bones, 5);
        let reread = read_bones(&Glb::parse(&rebuilt.to_bytes()).expect("parsed"));
        assert_eq!(reread.len(), 5);
        assert!(reread.iter().any(|b| b.name == "fin"));
    }
}
