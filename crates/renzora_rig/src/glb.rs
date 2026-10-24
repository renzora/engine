//! Reading and rewriting a glTF binary (`.glb`) in place.
//!
//! Rigging adds to a model and changes nothing that is already there: new
//! nodes for the bones, a skin, and two vertex attributes per primitive
//! (`JOINTS_0`, `WEIGHTS_0`). So rather than decoding the model and writing a
//! new one, which would have to round-trip every material, texture and
//! extension the importer produced, this keeps the JSON as a value and the
//! binary chunk as bytes, and only appends. Anything this module does not
//! understand passes through untouched.

use bevy::math::{Mat4, Quat, Vec3};
use serde_json::{json, Value};

/// A parsed `.glb`: its JSON document and its binary chunk.
pub struct Glb {
    pub json: Value,
    pub bin: Vec<u8>,
}

#[derive(Debug)]
pub enum GlbError {
    NotGlb,
    Json(String),
    Unsupported(String),
}

impl std::fmt::Display for GlbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GlbError::NotGlb => write!(f, "not a glTF binary (.glb)"),
            GlbError::Json(e) => write!(f, "unreadable glTF JSON: {e}"),
            GlbError::Unsupported(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for GlbError {}

const MAGIC: &[u8; 4] = b"glTF";
const CHUNK_JSON: u32 = 0x4E4F_534A;
const CHUNK_BIN: u32 = 0x004E_4942;

/// glTF component types this module reads or writes.
const FLOAT: u64 = 5126;
const UNSIGNED_BYTE: u64 = 5121;
const UNSIGNED_SHORT: u64 = 5123;
const UNSIGNED_INT: u64 = 5125;

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

impl Glb {
    pub fn parse(bytes: &[u8]) -> Result<Glb, GlbError> {
        if bytes.get(0..4) != Some(MAGIC.as_slice()) {
            return Err(GlbError::NotGlb);
        }
        let mut at = 12;
        let mut json = None;
        let mut bin = Vec::new();
        while at + 8 <= bytes.len() {
            let len = u32_at(bytes, at).ok_or(GlbError::NotGlb)? as usize;
            let kind = u32_at(bytes, at + 4).ok_or(GlbError::NotGlb)?;
            let body = bytes.get(at + 8..at + 8 + len).ok_or(GlbError::NotGlb)?;
            match kind {
                CHUNK_JSON => json = Some(serde_json::from_slice::<Value>(body).map_err(|e| GlbError::Json(e.to_string()))?),
                CHUNK_BIN => bin = body.to_vec(),
                _ => {}
            }
            at += 8 + len;
        }
        let json = json.ok_or(GlbError::NotGlb)?;
        if let Some(req) = json.get("extensionsRequired").and_then(Value::as_array) {
            // Compressed geometry cannot be read here, so it cannot be rigged.
            if let Some(ext) = req.iter().filter_map(Value::as_str).find(|e| e.contains("draco") || e.contains("meshopt")) {
                return Err(GlbError::Unsupported(format!("the model uses {ext} compression, which rigging cannot read")));
            }
        }
        Ok(Glb { json, bin })
    }

    /// Serialise back to `.glb` bytes. The buffer length in the JSON is kept
    /// in step with the binary chunk.
    pub fn to_bytes(&mut self) -> Vec<u8> {
        while !self.bin.len().is_multiple_of(4) {
            self.bin.push(0);
        }
        if let Some(b0) = self.json.get_mut("buffers").and_then(|b| b.get_mut(0)) {
            b0["byteLength"] = json!(self.bin.len());
        }
        let mut json_bytes = serde_json::to_vec(&self.json).unwrap_or_default();
        while !json_bytes.len().is_multiple_of(4) {
            json_bytes.push(b' ');
        }
        let total = 12 + 8 + json_bytes.len() + 8 + self.bin.len();
        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(&CHUNK_JSON.to_le_bytes());
        out.extend_from_slice(&json_bytes);
        out.extend_from_slice(&(self.bin.len() as u32).to_le_bytes());
        out.extend_from_slice(&CHUNK_BIN.to_le_bytes());
        out.extend_from_slice(&self.bin);
        out
    }

    fn array(&self, key: &str) -> &[Value] {
        self.json.get(key).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
    }

    fn array_mut(&mut self, key: &str) -> &mut Vec<Value> {
        if !self.json.get(key).is_some_and(Value::is_array) {
            self.json[key] = json!([]);
        }
        self.json[key].as_array_mut().expect("just made an array")
    }

    pub fn nodes(&self) -> &[Value] {
        self.array("nodes")
    }

    pub fn meshes(&self) -> &[Value] {
        self.array("meshes")
    }

    pub fn skins(&self) -> &[Value] {
        self.array("skins")
    }

    pub fn node_name(&self, node: usize) -> String {
        self.nodes().get(node).and_then(|n| n.get("name")).and_then(Value::as_str).unwrap_or("").to_string()
    }

    pub fn mesh_name(&self, mesh: usize) -> String {
        self.meshes().get(mesh).and_then(|m| m.get("name")).and_then(Value::as_str).unwrap_or("").to_string()
    }

    /// Each node's local transform.
    pub fn local_matrix(&self, node: usize) -> Mat4 {
        let Some(n) = self.nodes().get(node) else { return Mat4::IDENTITY };
        if let Some(m) = n.get("matrix").and_then(Value::as_array) {
            let v: Vec<f32> = m.iter().filter_map(Value::as_f64).map(|f| f as f32).collect();
            if v.len() == 16 {
                return Mat4::from_cols_slice(&v);
            }
        }
        let f = |key: &str, n_: usize, default: &[f32]| -> Vec<f32> {
            n.get(key)
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_f64).map(|f| f as f32).collect::<Vec<_>>())
                .filter(|v| v.len() == n_)
                .unwrap_or_else(|| default.to_vec())
        };
        let t = f("translation", 3, &[0.0, 0.0, 0.0]);
        let r = f("rotation", 4, &[0.0, 0.0, 0.0, 1.0]);
        let s = f("scale", 3, &[1.0, 1.0, 1.0]);
        Mat4::from_scale_rotation_translation(
            Vec3::new(s[0], s[1], s[2]),
            Quat::from_xyzw(r[0], r[1], r[2], r[3]).normalize(),
            Vec3::new(t[0], t[1], t[2]),
        )
    }

    /// Each node's parent, or `None` for a root.
    pub fn parents(&self) -> Vec<Option<usize>> {
        let mut parents = vec![None; self.nodes().len()];
        for (i, n) in self.nodes().iter().enumerate() {
            for c in n.get("children").and_then(Value::as_array).into_iter().flatten() {
                if let Some(c) = c.as_u64() {
                    if let Some(p) = parents.get_mut(c as usize) {
                        *p = Some(i);
                    }
                }
            }
        }
        parents
    }

    /// Each node's transform relative to the scene root (the model's space).
    pub fn world_matrices(&self) -> Vec<Mat4> {
        let parents = self.parents();
        let n = self.nodes().len();
        let mut out: Vec<Option<Mat4>> = vec![None; n];
        fn resolve(i: usize, g: &Glb, parents: &[Option<usize>], out: &mut [Option<Mat4>], depth: usize) -> Mat4 {
            if let Some(m) = out[i] {
                return m;
            }
            let local = g.local_matrix(i);
            let m = match parents[i] {
                Some(p) if depth < 256 => resolve(p, g, parents, out, depth + 1) * local,
                _ => local,
            };
            out[i] = Some(m);
            m
        }
        (0..n).map(|i| resolve(i, self, &parents, &mut out, 0)).collect()
    }

    /// The `POSITION`s of one primitive, in the mesh's own space.
    pub fn positions(&self, mesh: usize, prim: usize) -> Result<Vec<Vec3>, GlbError> {
        let accessor = self
            .meshes()
            .get(mesh)
            .and_then(|m| m.get("primitives"))
            .and_then(|p| p.get(prim))
            .and_then(|p| p.get("attributes"))
            .and_then(|a| a.get("POSITION"))
            .and_then(Value::as_u64)
            .ok_or_else(|| GlbError::Unsupported("a primitive has no positions".into()))?;
        self.read_vec3(accessor as usize)
    }

    /// The triangle indices of one primitive: `None` for a primitive drawn
    /// without an index buffer, whose triangles are its vertices in order.
    ///
    /// Only triangle lists are read; strips and fans (rare in practice, and
    /// never written by the importer) come back as `None` too, which a caller
    /// treats as "no connectivity known" rather than as wrong triangles.
    pub fn indices(&self, mesh: usize, prim: usize) -> Result<Option<Vec<u32>>, GlbError> {
        let Some(p) = self.meshes().get(mesh).and_then(|m| m.get("primitives")).and_then(|p| p.get(prim)) else {
            return Ok(None);
        };
        if p.get("mode").and_then(Value::as_u64).unwrap_or(4) != 4 {
            return Ok(None);
        }
        let Some(accessor) = p.get("indices").and_then(Value::as_u64) else {
            return Ok(None);
        };
        let acc = self.array("accessors").get(accessor as usize).ok_or_else(|| GlbError::Unsupported("missing accessor".into()))?;
        let size = match acc.get("componentType").and_then(Value::as_u64) {
            Some(UNSIGNED_BYTE) => 1,
            Some(UNSIGNED_SHORT) => 2,
            Some(UNSIGNED_INT) => 4,
            _ => return Err(GlbError::Unsupported("indices of an unknown type".into())),
        };
        let count = acc.get("count").and_then(Value::as_u64).unwrap_or(0) as usize;
        let Some(view_i) = acc.get("bufferView").and_then(Value::as_u64) else {
            return Ok(None);
        };
        let view = self.array("bufferViews").get(view_i as usize).ok_or_else(|| GlbError::Unsupported("missing buffer view".into()))?;
        let base = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize
            + acc.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
        let bytes = self
            .bin
            .get(base..base + count * size)
            .ok_or_else(|| GlbError::Unsupported("indices run past the end of the binary".into()))?;
        Ok(Some(
            bytes
                .chunks_exact(size)
                .map(|b| match size {
                    1 => b[0] as u32,
                    2 => u16::from_le_bytes([b[0], b[1]]) as u32,
                    _ => u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                })
                .collect(),
        ))
    }

    fn read_vec3(&self, accessor: usize) -> Result<Vec<Vec3>, GlbError> {
        let acc = self.array("accessors").get(accessor).ok_or_else(|| GlbError::Unsupported("missing accessor".into()))?;
        if acc.get("componentType").and_then(Value::as_u64) != Some(FLOAT) || acc.get("type").and_then(Value::as_str) != Some("VEC3") {
            return Err(GlbError::Unsupported("positions are not plain floats (quantised geometry cannot be rigged)".into()));
        }
        let count = acc.get("count").and_then(Value::as_u64).unwrap_or(0) as usize;
        let Some(view_i) = acc.get("bufferView").and_then(Value::as_u64) else {
            return Ok(vec![Vec3::ZERO; count]);
        };
        let view = self.array("bufferViews").get(view_i as usize).ok_or_else(|| GlbError::Unsupported("missing buffer view".into()))?;
        if view.get("buffer").and_then(Value::as_u64).unwrap_or(0) != 0 {
            return Err(GlbError::Unsupported("geometry in an external buffer".into()));
        }
        let base = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize
            + acc.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize;
        let stride = view.get("byteStride").and_then(Value::as_u64).map(|s| s as usize).unwrap_or(12).max(12);
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            let at = base + i * stride;
            let f = |o: usize| self.bin.get(at + o..at + o + 4).map(|b| f32::from_le_bytes(b.try_into().expect("4 bytes")));
            match (f(0), f(4), f(8)) {
                (Some(x), Some(y), Some(z)) => out.push(Vec3::new(x, y, z)),
                _ => return Err(GlbError::Unsupported("positions run past the end of the binary".into())),
            }
        }
        Ok(out)
    }

    /// Append bytes to the binary chunk as a new buffer view; returns its index.
    fn push_view(&mut self, data: &[u8], target: Option<u64>) -> usize {
        while !self.bin.len().is_multiple_of(4) {
            self.bin.push(0);
        }
        let offset = self.bin.len();
        self.bin.extend_from_slice(data);
        if self.array("buffers").is_empty() {
            self.array_mut("buffers").push(json!({ "byteLength": 0 }));
        }
        let mut view = json!({ "buffer": 0, "byteOffset": offset, "byteLength": data.len() });
        if let Some(t) = target {
            view["target"] = json!(t);
        }
        let views = self.array_mut("bufferViews");
        views.push(view);
        views.len() - 1
    }

    fn push_accessor(&mut self, accessor: Value) -> usize {
        let accs = self.array_mut("accessors");
        accs.push(accessor);
        accs.len() - 1
    }

    /// Add a node; returns its index. `parent` of `None` makes it a root of
    /// the default scene.
    pub fn add_node(&mut self, node: Value, parent: Option<usize>) -> usize {
        let index = {
            let nodes = self.array_mut("nodes");
            nodes.push(node);
            nodes.len() - 1
        };
        match parent {
            Some(p) => {
                let n = &mut self.json["nodes"][p];
                if !n.get("children").is_some_and(Value::is_array) {
                    n["children"] = json!([]);
                }
                n["children"].as_array_mut().expect("array").push(json!(index));
            }
            None => {
                let scene = self.json.get("scene").and_then(Value::as_u64).unwrap_or(0) as usize;
                if self.array("scenes").is_empty() {
                    self.array_mut("scenes").push(json!({ "nodes": [] }));
                }
                let s = &mut self.json["scenes"][scene];
                if !s.get("nodes").is_some_and(Value::is_array) {
                    s["nodes"] = json!([]);
                }
                s["nodes"].as_array_mut().expect("array").push(json!(index));
            }
        }
        index
    }

    /// Add `JOINTS_0`/`WEIGHTS_0` to one primitive. Each vertex carries up to
    /// four `(joint index within the skin, weight)` pairs.
    pub fn set_skin_attributes(&mut self, mesh: usize, prim: usize, influences: &[[(u16, f32); 4]]) {
        let count = influences.len();
        let mut joints = Vec::with_capacity(count * 8);
        let mut weights = Vec::with_capacity(count * 16);
        for inf in influences {
            for (j, _) in inf {
                joints.extend_from_slice(&j.to_le_bytes());
            }
            for (_, w) in inf {
                weights.extend_from_slice(&w.to_le_bytes());
            }
        }
        const ARRAY_BUFFER: u64 = 34962;
        let jv = self.push_view(&joints, Some(ARRAY_BUFFER));
        let ja = self.push_accessor(json!({ "bufferView": jv, "componentType": UNSIGNED_SHORT, "count": count, "type": "VEC4" }));
        let wv = self.push_view(&weights, Some(ARRAY_BUFFER));
        let wa = self.push_accessor(json!({ "bufferView": wv, "componentType": FLOAT, "count": count, "type": "VEC4" }));
        let attrs = &mut self.json["meshes"][mesh]["primitives"][prim]["attributes"];
        attrs["JOINTS_0"] = json!(ja);
        attrs["WEIGHTS_0"] = json!(wa);
    }

    /// Add a skin over `joints` with the given inverse bind matrices; returns
    /// its index.
    pub fn add_skin(&mut self, name: &str, joints: &[usize], inverse_binds: &[Mat4], skeleton: Option<usize>) -> usize {
        let mut data = Vec::with_capacity(inverse_binds.len() * 64);
        for m in inverse_binds {
            for f in m.to_cols_array() {
                data.extend_from_slice(&f.to_le_bytes());
            }
        }
        let view = self.push_view(&data, None);
        let acc = self.push_accessor(json!({ "bufferView": view, "componentType": FLOAT, "count": inverse_binds.len(), "type": "MAT4" }));
        let mut skin = json!({ "name": name, "joints": joints, "inverseBindMatrices": acc });
        if let Some(s) = skeleton {
            skin["skeleton"] = json!(s);
        }
        let skins = self.array_mut("skins");
        skins.push(skin);
        skins.len() - 1
    }

    pub fn set_node_skin(&mut self, node: usize, skin: usize) {
        self.json["nodes"][node]["skin"] = json!(skin);
    }
}

/// A node's local TRS as the glTF JSON wants it.
pub fn trs_json(name: &str, translation: Vec3, rotation: Quat) -> Value {
    let r = rotation.normalize();
    json!({
        "name": name,
        "translation": [translation.x, translation.y, translation.z],
        "rotation": [r.x, r.y, r.z, r.w],
    })
}
