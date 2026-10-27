//! Blend shapes (glTF morph targets): reading them out of a `.glb` and writing
//! them back.
//!
//! A glTF mesh keeps its blend shapes per primitive (`primitive.targets`, one
//! object per shape holding `POSITION`, and optionally `NORMAL` and `TANGENT`,
//! displacement accessors) and names them once for the whole mesh, in
//! `mesh.extras.targetNames`, which is where every exporter writes them and
//! the only place Bevy's loader reads them from. The format's rules, which a
//! rewrite must keep:
//!
//! - Every primitive of a mesh has the same number of targets, in the same
//!   order: target `i` is one shape across the whole mesh, and a clip's morph
//!   track drives it by that index.
//! - Within a primitive, every target has the same attributes. Bevy reads a
//!   target with none at all as no vertices, which shifts every later shape
//!   onto the wrong ones, so a target that does not move a primitive is still
//!   written there, as zeros.
//!
//! Like the rest of this crate, writing appends: new displacement data goes on
//! the end of the binary chunk, a shape that was not edited keeps its old
//! accessors untouched, and data a rewrite stops using stays in the file,
//! unreferenced.

use std::collections::HashMap;

use bevy::math::Vec3;
use serde_json::{json, Value};

use crate::glb::{Glb, GlbError};

const FLOAT: u64 = 5126;
const UNSIGNED_BYTE: u64 = 5121;
const UNSIGNED_SHORT: u64 = 5123;
const UNSIGNED_INT: u64 = 5125;

/// glTF caps nothing, but Bevy refuses a mesh with more targets than this
/// (`bevy::mesh::morph::MAX_MORPH_WEIGHTS`), so writing more would make a model
/// that no longer loads.
pub const MAX_TARGETS: usize = 256;

/// One shape's displacements on one primitive, in the primitive's vertex
/// order and its own space.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TargetData {
    pub positions: Vec<Vec3>,
    pub normals: Option<Vec<Vec3>>,
}

/// A mesh's blend shapes as the file has them.
#[derive(Clone, Debug, Default)]
pub struct MeshTargets {
    /// One per target. A target the file does not name is called `Shape N`.
    pub names: Vec<String>,
    /// `[primitive][target]`.
    pub primitives: Vec<Vec<TargetData>>,
}

/// One shape to write into a mesh, in the order the mesh is to have them.
#[derive(Clone, Debug, Default)]
pub struct TargetEdit {
    pub name: String,
    /// The target this one was in the file, if it was there.
    pub source: Option<usize>,
    /// New displacements, by primitive index. `None` keeps the source's data
    /// exactly as it is (a shape renamed or moved, not sculpted). A primitive
    /// missing from the list keeps the source's data there, or gets zeros for
    /// a new shape.
    pub data: Option<Vec<(usize, TargetData)>>,
}

fn primitives(glb: &Glb, mesh: usize) -> &[Value] {
    glb.meshes().get(mesh).and_then(|m| m.get("primitives")).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

/// How many targets the mesh has: the most any primitive has, which is what
/// Bevy's loader takes when a file breaks the same-count rule.
pub fn target_count(glb: &Glb, mesh: usize) -> usize {
    primitives(glb, mesh).iter().map(|p| p.get("targets").and_then(Value::as_array).map_or(0, Vec::len)).max().unwrap_or(0)
}

/// The mesh's target names, one per target.
pub fn target_names(glb: &Glb, mesh: usize) -> Vec<String> {
    let named: Vec<String> = glb
        .meshes()
        .get(mesh)
        .and_then(|m| m.get("extras"))
        .and_then(|e| e.get("targetNames"))
        .and_then(Value::as_array)
        .map(|a| a.iter().map(|n| n.as_str().unwrap_or("").to_string()).collect())
        .unwrap_or_default();
    (0..target_count(glb, mesh))
        .map(|i| named.get(i).filter(|n| !n.is_empty()).cloned().unwrap_or_else(|| format!("Shape {i}")))
        .collect()
}

/// Every target of every primitive of `mesh`.
pub fn read_targets(glb: &Glb, mesh: usize) -> Result<MeshTargets, GlbError> {
    let mut out = MeshTargets { names: target_names(glb, mesh), primitives: Vec::new() };
    for p in primitives(glb, mesh) {
        let count = p
            .get("attributes")
            .and_then(|a| a.get("POSITION"))
            .and_then(Value::as_u64)
            .and_then(|a| glb.json.get("accessors")?.get(a as usize)?.get("count")?.as_u64())
            .unwrap_or(0) as usize;
        let mut targets = Vec::new();
        for t in p.get("targets").and_then(Value::as_array).into_iter().flatten() {
            let read = |key: &str| -> Result<Option<Vec<Vec3>>, GlbError> {
                match t.get(key).and_then(Value::as_u64) {
                    Some(a) => glb.vec3s(a as usize).map(Some),
                    None => Ok(None),
                }
            };
            let positions = read("POSITION")?.unwrap_or_else(|| vec![Vec3::ZERO; count]);
            targets.push(TargetData { positions, normals: read("NORMAL")? });
        }
        out.primitives.push(targets);
    }
    Ok(out)
}

impl Glb {
    /// A `VEC3` float accessor's values, sparse or not. An accessor with no
    /// buffer view is zeros (the glTF rule), which is how exporters write a
    /// target that leaves a primitive alone; a sparse one lists only the
    /// vertices that move, which is how Blender writes most shape keys.
    pub fn vec3s(&self, accessor: usize) -> Result<Vec<Vec3>, GlbError> {
        let unsupported = |m: &str| GlbError::Unsupported(m.to_string());
        let acc = self.json.get("accessors").and_then(|a| a.get(accessor)).ok_or_else(|| unsupported("missing accessor"))?;
        if acc.get("componentType").and_then(Value::as_u64) != Some(FLOAT) || acc.get("type").and_then(Value::as_str) != Some("VEC3") {
            return Err(unsupported("blend shape data is not plain floats (quantised geometry cannot be edited)"));
        }
        let count = acc.get("count").and_then(Value::as_u64).unwrap_or(0) as usize;
        let offset = acc.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
        let mut out = match acc.get("bufferView").and_then(Value::as_u64) {
            Some(view) => self.floats3(view as usize, offset, count)?,
            None => vec![Vec3::ZERO; count],
        };
        if let Some(sparse) = acc.get("sparse") {
            let n = sparse.get("count").and_then(Value::as_u64).unwrap_or(0) as usize;
            let idx = sparse.get("indices").ok_or_else(|| unsupported("sparse accessor without indices"))?;
            let vals = sparse.get("values").ok_or_else(|| unsupported("sparse accessor without values"))?;
            let size = match idx.get("componentType").and_then(Value::as_u64) {
                Some(UNSIGNED_BYTE) => 1,
                Some(UNSIGNED_SHORT) => 2,
                Some(UNSIGNED_INT) => 4,
                _ => return Err(unsupported("sparse indices of an unknown type")),
            };
            let iv = idx.get("bufferView").and_then(Value::as_u64).ok_or_else(|| unsupported("sparse indices without a buffer view"))? as usize;
            let base = self.view_start(iv)? + idx.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
            let bytes = self.bin.get(base..base + n * size).ok_or_else(|| unsupported("sparse indices run past the end of the binary"))?;
            let indices: Vec<usize> = bytes
                .chunks_exact(size)
                .map(|b| match size {
                    1 => b[0] as usize,
                    2 => u16::from_le_bytes([b[0], b[1]]) as usize,
                    _ => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize,
                })
                .collect();
            let vv = vals.get("bufferView").and_then(Value::as_u64).ok_or_else(|| unsupported("sparse values without a buffer view"))? as usize;
            let values = self.floats3(vv, vals.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize, n)?;
            for (i, v) in indices.into_iter().zip(values) {
                if let Some(slot) = out.get_mut(i) {
                    *slot = v;
                }
            }
        }
        Ok(out)
    }

    fn view_start(&self, view: usize) -> Result<usize, GlbError> {
        let v = self.json.get("bufferViews").and_then(|b| b.get(view)).ok_or_else(|| GlbError::Unsupported("missing buffer view".into()))?;
        if v.get("buffer").and_then(Value::as_u64).unwrap_or(0) != 0 {
            return Err(GlbError::Unsupported("geometry in an external buffer".into()));
        }
        Ok(v.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize)
    }

    fn floats3(&self, view: usize, offset: usize, count: usize) -> Result<Vec<Vec3>, GlbError> {
        let base = self.view_start(view)? + offset;
        let stride = self
            .json
            .get("bufferViews")
            .and_then(|b| b.get(view))
            .and_then(|v| v.get("byteStride"))
            .and_then(Value::as_u64)
            .map(|s| s as usize)
            .unwrap_or(12)
            .max(12);
        (0..count)
            .map(|i| {
                let at = base + i * stride;
                let b = self.bin.get(at..at + 12).ok_or_else(|| GlbError::Unsupported("blend shape data runs past the end of the binary".into()))?;
                let f = |o: usize| f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
                Ok(Vec3::new(f(0), f(4), f(8)))
            })
            .collect()
    }

    /// Append `VEC3` floats as a vertex attribute accessor, with the
    /// `min`/`max` glTF requires on a `POSITION`.
    fn add_vec3s(&mut self, data: &[Vec3], bounds: bool) -> usize {
        const ARRAY_BUFFER: u64 = 34962;
        let bytes: Vec<u8> = data.iter().flat_map(|v| v.to_array()).flat_map(f32::to_le_bytes).collect();
        let view = self.push_view(&bytes, Some(ARRAY_BUFFER));
        let mut acc = json!({ "bufferView": view, "componentType": FLOAT, "count": data.len(), "type": "VEC3" });
        if bounds {
            let (lo, hi) = data.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
            let (lo, hi) = if data.is_empty() { (Vec3::ZERO, Vec3::ZERO) } else { (lo, hi) };
            acc["min"] = json!(lo.to_array());
            acc["max"] = json!(hi.to_array());
        }
        self.push_accessor(acc)
    }
}

/// Rewrite `mesh`'s blend shapes to be exactly `targets`, in that order: the
/// shapes it keeps, renamed, removed or reordered, and new ones. See the
/// module doc for the rules this keeps.
pub fn write_targets(glb: &mut Glb, mesh: usize, targets: &[TargetEdit]) -> Result<(), GlbError> {
    if targets.len() > MAX_TARGETS {
        return Err(GlbError::Unsupported(format!("a mesh can have at most {MAX_TARGETS} blend shapes")));
    }
    if glb.meshes().get(mesh).is_none() {
        return Err(GlbError::Unsupported(format!("the model has no mesh {mesh}")));
    }
    let prims: Vec<Value> = primitives(glb, mesh).to_vec();
    // One zero accessor per vertex count, shared by every target slot that
    // does not move a primitive: glTF lets accessors be reused, and it keeps
    // a hundred shapes on a head from writing a hundred copies of zeros onto
    // the eyes.
    let mut zeros: HashMap<usize, usize> = HashMap::new();
    let mut new_prims = Vec::with_capacity(prims.len());
    for (pi, p) in prims.iter().enumerate() {
        let count = p
            .get("attributes")
            .and_then(|a| a.get("POSITION"))
            .and_then(Value::as_u64)
            .and_then(|a| glb.json.get("accessors")?.get(a as usize)?.get("count")?.as_u64())
            .ok_or_else(|| GlbError::Unsupported(format!("mesh {mesh} primitive {pi} has no positions")))? as usize;
        let old: Vec<Value> = p.get("targets").and_then(Value::as_array).cloned().unwrap_or_default();
        let mut slots: Vec<serde_json::Map<String, Value>> = Vec::with_capacity(targets.len());
        for t in targets {
            let fresh = t.data.as_ref().and_then(|d| d.iter().find(|(i, _)| *i == pi)).map(|(_, d)| d);
            let kept = t.source.and_then(|s| old.get(s)).and_then(Value::as_object);
            let slot = match (fresh, kept) {
                (Some(d), _) => {
                    if d.positions.len() != count || d.normals.as_ref().is_some_and(|n| n.len() != count) {
                        return Err(GlbError::Unsupported(format!(
                            "shape \"{}\" has {} displacements for mesh {mesh} primitive {pi}, which has {count} vertices",
                            t.name,
                            d.positions.len()
                        )));
                    }
                    let mut s = serde_json::Map::new();
                    s.insert("POSITION".into(), json!(glb.add_vec3s(&d.positions, true)));
                    if let Some(n) = &d.normals {
                        s.insert("NORMAL".into(), json!(glb.add_vec3s(n, false)));
                    }
                    s
                }
                (None, Some(k)) => k.clone(),
                (None, None) => serde_json::Map::new(),
            };
            slots.push(slot);
        }
        // Every target of the primitive carries every attribute any of them
        // does; the missing ones point at zeros.
        let mut keys: Vec<String> = Vec::new();
        for s in &slots {
            for k in s.keys() {
                if !keys.contains(k) {
                    keys.push(k.clone());
                }
            }
        }
        if !keys.iter().any(|k| k == "POSITION") && !slots.is_empty() {
            keys.insert(0, "POSITION".into());
        }
        for s in &mut slots {
            for k in &keys {
                if !s.contains_key(k) {
                    let z = *zeros.entry(count).or_insert_with(|| glb.add_vec3s(&vec![Vec3::ZERO; count], true));
                    s.insert(k.clone(), json!(z));
                }
            }
        }
        let mut np = p.clone();
        if slots.is_empty() {
            if let Some(o) = np.as_object_mut() {
                o.remove("targets");
            }
        } else {
            np["targets"] = Value::Array(slots.into_iter().map(Value::Object).collect());
        }
        new_prims.push(np);
    }

    let m = &mut glb.json["meshes"][mesh];
    let old_weights: Option<Vec<f64>> = m.get("weights").and_then(Value::as_array).map(|w| w.iter().map(|v| v.as_f64().unwrap_or(0.0)).collect());
    m["primitives"] = Value::Array(new_prims);
    let obj = m.as_object_mut().ok_or_else(|| GlbError::Unsupported("a mesh is not an object".into()))?;
    if targets.is_empty() {
        obj.remove("weights");
        if let Some(e) = obj.get_mut("extras").and_then(Value::as_object_mut) {
            e.remove("targetNames");
        }
        return Ok(());
    }
    if let Some(w) = old_weights {
        // A mesh's default weights follow their shapes; a new shape rests at 0.
        let weights: Vec<f64> = targets.iter().map(|t| t.source.and_then(|s| w.get(s).copied()).unwrap_or(0.0)).collect();
        obj.insert("weights".into(), json!(weights));
    }
    if !obj.get("extras").is_some_and(Value::is_object) {
        obj.insert("extras".into(), json!({}));
    }
    let names: Vec<&str> = targets.iter().map(|t| t.name.as_str()).collect();
    obj.get_mut("extras").and_then(Value::as_object_mut).expect("just made an object").insert("targetNames".into(), json!(names));
    Ok(())
}

/// Where a model's original is kept before its blend shapes are first edited.
pub fn unshaped_path(path: &std::path::Path) -> std::path::PathBuf {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("model");
    let rvg = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("rvg"));
    path.with_file_name(format!("{stem}.unshaped.{}", if rvg { "rvg" } else { "glb" }))
}

/// Edit a model file's blend shapes in place, keeping the file as it was
/// beside it as `<stem>.unshaped.glb` the first time, the way rigging keeps
/// `<stem>.unrigged.glb`.
///
/// `build` sees the parsed file (to check vertex counts and spaces against
/// what was sculpted) and returns, per mesh, the full list of shapes it is to
/// have ([`write_targets`]). The file is rewritten rather than saved as a
/// copy so every scene using the model gets the shapes without being changed.
pub fn write_shapes(
    path: &std::path::Path,
    build: impl FnOnce(&Glb) -> Result<Vec<(usize, Vec<TargetEdit>)>, GlbError>,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = crate::read_model(path)?;
    let mut glb = Glb::parse(&bytes)?;
    let edits = build(&glb)?;
    for (mesh, targets) in &edits {
        write_targets(&mut glb, *mesh, targets)?;
    }
    let original = unshaped_path(path);
    if !original.exists() {
        std::fs::copy(path, &original)?;
    }
    crate::write_model(path, &glb.to_bytes())
}

/// The sign per axis that takes positions as the engine loaded them to
/// positions as the file stores them: `(1, 1, 1)`, or `(-1, 1, -1)` when the
/// project's glTF import turns models to face the other way. `None` when the
/// two are not the same vertices at all (different counts, or a mesh the
/// engine rebuilt), in which case nothing sculpted on it can be saved.
pub fn file_axes(file: &[Vec3], engine: &[Vec3]) -> Option<Vec3> {
    if file.len() != engine.len() {
        return None;
    }
    let size = file.iter().fold(0.0f32, |m, p| m.max(p.abs().max_element())).max(1e-6);
    let tol = size * 1e-4;
    [Vec3::ONE, Vec3::new(-1.0, 1.0, -1.0)].into_iter().find(|s| file.iter().zip(engine).all(|(f, e)| (*e * *s - *f).abs().max_element() <= tol))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glb::Geometry;

    fn tri() -> Glb {
        let positions = [Vec3::ZERO, Vec3::X, Vec3::Y];
        let normals = [Vec3::Z; 3];
        let g = Geometry { name: "Face", positions: &positions, normals: &normals, uvs: &[], indices: &[0, 1, 2], base_color: [1.0; 4] };
        Glb::from_geometry(&g).unwrap()
    }

    fn reparse(glb: &mut Glb) -> Glb {
        Glb::parse(&glb.to_bytes()).unwrap()
    }

    fn data(d: [Vec3; 3]) -> TargetData {
        TargetData { positions: d.to_vec(), normals: None }
    }

    #[test]
    fn new_shapes_round_trip_with_names() {
        let mut glb = tri();
        let smile = TargetEdit { name: "Smile".into(), source: None, data: Some(vec![(0, TargetData { positions: vec![Vec3::Y, Vec3::ZERO, Vec3::ZERO], normals: Some(vec![Vec3::X; 3]) })]) };
        let blink = TargetEdit { name: "Blink".into(), source: None, data: Some(vec![(0, data([Vec3::ZERO, Vec3::ZERO, -Vec3::Y]))]) };
        write_targets(&mut glb, 0, &[smile, blink]).unwrap();
        let back = reparse(&mut glb);
        let t = read_targets(&back, 0).unwrap();
        assert_eq!(t.names, vec!["Smile", "Blink"]);
        assert_eq!(t.primitives[0][0].positions, vec![Vec3::Y, Vec3::ZERO, Vec3::ZERO]);
        assert_eq!(t.primitives[0][0].normals, Some(vec![Vec3::X; 3]));
        assert_eq!(t.primitives[0][1].positions[2], -Vec3::Y);
        // Blink had no normals, but Smile did: Blink gets zero normals, so the
        // primitive's targets all carry the same attributes.
        assert_eq!(t.primitives[0][1].normals, Some(vec![Vec3::ZERO; 3]));
        // POSITION accessors carry min/max.
        let acc = back.json["meshes"][0]["primitives"][0]["targets"][1]["POSITION"].as_u64().unwrap() as usize;
        assert_eq!(back.json["accessors"][acc]["min"], json!([0.0, -1.0, 0.0]));
    }

    #[test]
    fn keeps_renames_and_deletes_existing_shapes() {
        let mut glb = tri();
        let a = TargetEdit { name: "A".into(), source: None, data: Some(vec![(0, data([Vec3::X; 3]))]) };
        let b = TargetEdit { name: "B".into(), source: None, data: Some(vec![(0, data([Vec3::Y; 3]))]) };
        let c = TargetEdit { name: "C".into(), source: None, data: Some(vec![(0, data([Vec3::Z; 3]))]) };
        write_targets(&mut glb, 0, &[a, b, c]).unwrap();
        glb.json["meshes"][0]["weights"] = json!([0.1, 0.2, 0.3]);
        let mut glb = reparse(&mut glb);
        // Delete A, rename C, keep B's data untouched, add D.
        let edits = [
            TargetEdit { name: "B".into(), source: Some(1), data: None },
            TargetEdit { name: "Sea".into(), source: Some(2), data: None },
            TargetEdit { name: "D".into(), source: None, data: Some(vec![(0, data([-Vec3::X; 3]))]) },
        ];
        let b_accessor = glb.json["meshes"][0]["primitives"][0]["targets"][1]["POSITION"].clone();
        write_targets(&mut glb, 0, &edits).unwrap();
        let back = reparse(&mut glb);
        let t = read_targets(&back, 0).unwrap();
        assert_eq!(t.names, vec!["B", "Sea", "D"]);
        assert_eq!(t.primitives[0][0].positions, vec![Vec3::Y; 3]);
        assert_eq!(t.primitives[0][1].positions, vec![Vec3::Z; 3]);
        assert_eq!(t.primitives[0][2].positions, vec![-Vec3::X; 3]);
        assert_eq!(back.json["meshes"][0]["primitives"][0]["targets"][0]["POSITION"], b_accessor, "an unedited shape keeps its data");
        let w: Vec<f64> = back.json["meshes"][0]["weights"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        assert!((w[0] - 0.2).abs() < 1e-6 && (w[1] - 0.3).abs() < 1e-6 && w[2] == 0.0);
        // Removing every shape removes the targets and the names.
        write_targets(&mut glb, 0, &[]).unwrap();
        assert!(glb.json["meshes"][0]["primitives"][0].get("targets").is_none());
        assert!(glb.json["meshes"][0]["extras"].get("targetNames").is_none());
        assert_eq!(target_count(&glb, 0), 0);
    }

    #[test]
    fn every_primitive_gets_every_shape() {
        let mut glb = tri();
        // A second primitive of the same mesh, sharing the first's data.
        let p0 = glb.json["meshes"][0]["primitives"][0].clone();
        glb.json["meshes"][0]["primitives"].as_array_mut().unwrap().push(p0);
        let edits = [TargetEdit { name: "Only First".into(), source: None, data: Some(vec![(0, data([Vec3::X; 3]))]) }];
        write_targets(&mut glb, 0, &edits).unwrap();
        let t = read_targets(&reparse(&mut glb), 0).unwrap();
        assert_eq!(t.primitives.len(), 2);
        assert_eq!(t.primitives[1].len(), 1, "the same count on every primitive");
        assert_eq!(t.primitives[1][0].positions, vec![Vec3::ZERO; 3]);
    }

    #[test]
    fn refuses_the_wrong_vertex_count() {
        let mut glb = tri();
        let bad = TargetEdit { name: "X".into(), source: None, data: Some(vec![(0, TargetData { positions: vec![Vec3::X; 4], normals: None })]) };
        assert!(write_targets(&mut glb, 0, &[bad]).is_err());
    }

    #[test]
    fn reads_sparse_and_bufferless_targets() {
        let mut glb = tri();
        // Indices [2] (u16) and values [(0, 0, 5)], the way Blender writes a
        // shape key that moves one vertex.
        let idx = glb.push_view(&[2u8, 0, 0, 0], None);
        let vals = glb.push_view(&[0.0f32, 0.0, 5.0].iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>(), None);
        let sparse = glb.push_accessor(json!({
            "componentType": FLOAT, "count": 3, "type": "VEC3",
            "sparse": { "count": 1, "indices": { "bufferView": idx, "componentType": UNSIGNED_SHORT }, "values": { "bufferView": vals } }
        }));
        let empty = glb.push_accessor(json!({ "componentType": FLOAT, "count": 3, "type": "VEC3" }));
        glb.json["meshes"][0]["primitives"][0]["targets"] = json!([{ "POSITION": sparse }, { "POSITION": empty }]);
        let t = read_targets(&reparse(&mut glb), 0).unwrap();
        assert_eq!(t.names, vec!["Shape 0", "Shape 1"]);
        assert_eq!(t.primitives[0][0].positions, vec![Vec3::ZERO, Vec3::ZERO, Vec3::new(0.0, 0.0, 5.0)]);
        assert_eq!(t.primitives[0][1].positions, vec![Vec3::ZERO; 3]);
    }

    #[test]
    fn finds_the_import_axis_flip() {
        let file = [Vec3::new(1.0, 2.0, 3.0), Vec3::new(-4.0, 0.5, 0.0)];
        assert_eq!(file_axes(&file, &file), Some(Vec3::ONE));
        let turned: Vec<Vec3> = file.iter().map(|p| Vec3::new(-p.x, p.y, -p.z)).collect();
        assert_eq!(file_axes(&file, &turned), Some(Vec3::new(-1.0, 1.0, -1.0)));
        assert_eq!(file_axes(&file, &[Vec3::ZERO; 2]), None);
        assert_eq!(file_axes(&file, &file[..1]), None);
    }

    #[test]
    fn write_shapes_keeps_the_original() {
        let dir = std::env::temp_dir().join(format!("renzora_morphs_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("head.glb");
        std::fs::write(&path, tri().to_bytes()).unwrap();
        let before = std::fs::read(&path).unwrap();
        write_shapes(&path, |g| {
            assert_eq!(g.positions(0, 0).unwrap().len(), 3);
            Ok(vec![(0, vec![TargetEdit { name: "Open".into(), source: None, data: Some(vec![(0, data([Vec3::Y; 3]))]) }])])
        })
        .unwrap();
        assert_eq!(std::fs::read(unshaped_path(&path)).unwrap(), before);
        let after = Glb::parse(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(target_names(&after, 0), vec!["Open"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
