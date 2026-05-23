//! Mesh optimization via meshoptimizer.
//!
//! Applies lossless reordering (vertex cache, overdraw, vertex fetch) and
//! optional lossy simplification to GLB meshes.

use log::warn;
use std::collections::HashMap;

/// Settings controlling which mesh optimizations to apply.
#[derive(Debug, Clone)]
pub struct MeshOptSettings {
    /// Reorder triangles for GPU vertex cache locality (lossless).
    pub vertex_cache: bool,
    /// Reorder triangles to reduce overdraw (lossless).
    pub overdraw: bool,
    /// Reorder vertices for vertex fetch cache efficiency (lossless).
    pub vertex_fetch: bool,
    /// Simplify meshes by reducing triangle count (lossy).
    pub simplify: bool,
    /// Target ratio of triangles to keep when simplifying (0.1–1.0).
    pub simplify_ratio: f32,
    /// Quantize vertex attributes to smaller types (lossy).
    pub quantize: bool,
    /// Generate LOD meshes (lossy).
    pub generate_lods: bool,
    /// Number of LOD levels to generate.
    pub lod_levels: u32,
}

impl Default for MeshOptSettings {
    fn default() -> Self {
        Self {
            vertex_cache: true,
            overdraw: true,
            vertex_fetch: true,
            simplify: false,
            simplify_ratio: 0.5,
            quantize: false,
            generate_lods: false,
            lod_levels: 3,
        }
    }
}

impl MeshOptSettings {
    /// Returns `true` if any optimization is enabled.
    pub fn any_enabled(&self) -> bool {
        self.vertex_cache
            || self.overdraw
            || self.vertex_fetch
            || self.simplify
            || self.quantize
            || self.generate_lods
    }
}

/// Optimize all meshes in a GLB binary blob according to `settings`.
///
/// Returns the optimized GLB bytes, or the original bytes unchanged if no
/// optimizations are enabled or the GLB contains no mesh data.
pub fn optimize_glb(glb_bytes: &[u8], settings: &MeshOptSettings) -> Result<Vec<u8>, String> {
    if !settings.any_enabled() {
        return Ok(glb_bytes.to_vec());
    }

    // Strip `extensionsRequired` entries the gltf crate refuses to parse but
    // which have a usable PBR fallback (e.g. KHR_materials_pbrSpecularGlossiness).
    // Otherwise the document parse below would fail on third-party assets.
    let cleaned = crate::glb_compat::strip_unsupported_extensions(glb_bytes);
    let glb_bytes = cleaned.as_slice();

    // Parse GLB for raw chunk access
    let glb = gltf::Glb::from_slice(glb_bytes).map_err(|e| format!("GLB parse error: {e}"))?;
    let Some(bin_cow) = &glb.bin else {
        return Ok(glb_bytes.to_vec());
    };
    let mut bin = bin_cow.to_vec();

    // Parse document for high-level mesh/accessor info
    let gltf_doc =
        gltf::Gltf::from_slice(glb_bytes).map_err(|e| format!("GLTF parse error: {e}"))?;
    let doc = &gltf_doc.document;

    // Parse JSON for potential modification (simplify changes accessor count)
    let mut json: serde_json::Value =
        serde_json::from_slice(&glb.json).map_err(|e| format!("JSON parse error: {e}"))?;
    let mut json_modified = false;

    // Snapshot of buffer for reading (writes go into `bin`)
    let read_buf = bin.clone();

    // Count how many primitives reference each attribute accessor. The
    // vertex-fetch pass rewrites attribute *data* in place, so it's only
    // correct for primitives that exclusively own their attributes — if two
    // primitives share a vertex buffer (common in real assets like Sponza),
    // each would re-permute the shared data with its own table and scramble
    // the other's geometry. We skip vertex-fetch for any shared primitive;
    // the index-only passes (cache/overdraw) stay safe regardless.
    let mut accessor_usage: HashMap<usize, u32> = HashMap::new();
    for mesh in doc.meshes() {
        for primitive in mesh.primitives() {
            for (_, acc) in primitive.attributes() {
                *accessor_usage.entry(acc.index()).or_insert(0) += 1;
            }
        }
    }

    for mesh in doc.meshes() {
        for primitive in mesh.primitives() {
            let vertex_fetch_safe = settings.vertex_fetch
                && primitive
                    .attributes()
                    .all(|(_, acc)| accessor_usage.get(&acc.index()).copied() == Some(1));

            if let Err(e) = optimize_primitive(
                &primitive,
                &read_buf,
                &mut bin,
                &mut json,
                &mut json_modified,
                settings,
                vertex_fetch_safe,
            ) {
                warn!(
                    "Mesh {} prim {}: skipped optimization: {e}",
                    mesh.index(),
                    primitive.index(),
                );
            }
        }
    }

    // Update buffer byte_length if JSON was modified
    if json_modified {
        if let Some(buffers) = json.get_mut("buffers") {
            if let Some(buf0) = buffers.get_mut(0) {
                buf0["byteLength"] = serde_json::json!(bin.len());
            }
        }
    }

    // Rebuild GLB
    let json_bytes = if json_modified {
        serde_json::to_vec(&json).map_err(|e| format!("JSON serialize: {e}"))?
    } else {
        glb.json.to_vec()
    };

    rebuild_glb(&json_bytes, &bin)
}

// ---------------------------------------------------------------------------
// Per-primitive optimization
// ---------------------------------------------------------------------------

fn optimize_primitive(
    primitive: &gltf::Primitive<'_>,
    read_buf: &[u8],
    bin: &mut Vec<u8>,
    json: &mut serde_json::Value,
    json_modified: &mut bool,
    settings: &MeshOptSettings,
    // Whether the vertex-fetch attribute remap is safe for this primitive
    // (true only when its attributes aren't shared with another primitive).
    vertex_fetch_safe: bool,
) -> Result<(), String> {
    let idx_accessor = primitive.indices().ok_or("Non-indexed primitive")?;
    let pos_accessor = primitive
        .get(&gltf::Semantic::Positions)
        .ok_or("No POSITION attribute")?;

    let vertex_count = pos_accessor.count();
    let mut indices = read_indices_from_buf(&idx_accessor, read_buf)?;

    // Build position bytes for meshopt adapter (from original buffer)
    let mut pos_bytes = read_position_bytes(&pos_accessor, read_buf)?;
    let mut adapter = meshopt::VertexDataAdapter::new(&pos_bytes, 12, 0)
        .map_err(|e| format!("VertexDataAdapter: {e:?}"))?;

    // --- Lossless reordering ---
    if settings.vertex_cache {
        indices = meshopt::optimize_vertex_cache(&indices, vertex_count);
    }

    if settings.overdraw {
        meshopt::optimize_overdraw_in_place(&mut indices, &adapter, 1.05);
    }

    if vertex_fetch_safe {
        let remap = meshopt::optimize_vertex_fetch_remap(&indices, vertex_count);
        indices = meshopt::remap_index_buffer(Some(&indices), vertex_count, &remap);

        // Remap every vertex attribute in the binary buffer
        for (_sem, acc) in primitive.attributes() {
            remap_attribute_in_buffer(&acc, read_buf, bin, &remap, vertex_count)?;
        }

        // Rebuild adapter from updated buffer for subsequent ops
        pos_bytes = read_position_bytes(&pos_accessor, bin.as_slice())?;
        adapter = meshopt::VertexDataAdapter::new(&pos_bytes, 12, 0)
            .map_err(|e| format!("VertexDataAdapter: {e:?}"))?;
    }

    // --- Lossy: simplification ---
    if settings.simplify {
        let target = ((indices.len() as f32 * settings.simplify_ratio) as usize / 3) * 3;
        let target = target.max(3);
        indices = meshopt::simplify(
            &indices,
            &adapter,
            target,
            0.01,
            meshopt::SimplifyOptions::None,
            None,
        );

        let acc_idx = idx_accessor.index();
        json["accessors"][acc_idx]["count"] = serde_json::json!(indices.len());
        *json_modified = true;
    }

    // Write optimized indices back
    write_indices_to_buf(&idx_accessor, bin, &indices)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Buffer I/O helpers
// ---------------------------------------------------------------------------

/// Read indices from binary buffer as `u32`.
fn read_indices_from_buf(accessor: &gltf::Accessor<'_>, buf: &[u8]) -> Result<Vec<u32>, String> {
    let view = accessor.view().ok_or("No buffer view for indices")?;
    let base = view.offset() + accessor.offset();
    let count = accessor.count();

    let mut indices = Vec::with_capacity(count);
    match accessor.data_type() {
        gltf::accessor::DataType::U8 => {
            for i in 0..count {
                indices.push(buf[base + i] as u32);
            }
        }
        gltf::accessor::DataType::U16 => {
            for i in 0..count {
                let off = base + i * 2;
                indices.push(u16::from_le_bytes([buf[off], buf[off + 1]]) as u32);
            }
        }
        gltf::accessor::DataType::U32 => {
            for i in 0..count {
                let off = base + i * 4;
                indices.push(u32::from_le_bytes([
                    buf[off],
                    buf[off + 1],
                    buf[off + 2],
                    buf[off + 3],
                ]));
            }
        }
        other => return Err(format!("Unsupported index type: {:?}", other)),
    }
    Ok(indices)
}

/// Read vertex positions as tightly-packed f32 bytes (12 bytes per vertex).
fn read_position_bytes(accessor: &gltf::Accessor<'_>, buf: &[u8]) -> Result<Vec<u8>, String> {
    let view = accessor.view().ok_or("No buffer view for positions")?;
    let base = view.offset() + accessor.offset();
    let count = accessor.count();
    let element_size = accessor.data_type().size() * accessor.dimensions().multiplicity();
    let stride = view.stride().unwrap_or(element_size);

    let mut out = Vec::with_capacity(count * 12);
    for i in 0..count {
        let off = base + i * stride;
        // Copy 3 × f32 = 12 bytes of position data
        out.extend_from_slice(&buf[off..off + 12]);
    }
    Ok(out)
}

/// Write indices back to the binary buffer.
fn write_indices_to_buf(
    accessor: &gltf::Accessor<'_>,
    buf: &mut [u8],
    indices: &[u32],
) -> Result<(), String> {
    let view = accessor.view().ok_or("No buffer view")?;
    let base = view.offset() + accessor.offset();

    match accessor.data_type() {
        gltf::accessor::DataType::U8 => {
            for (i, &idx) in indices.iter().enumerate() {
                buf[base + i] = idx as u8;
            }
        }
        gltf::accessor::DataType::U16 => {
            for (i, &idx) in indices.iter().enumerate() {
                let off = base + i * 2;
                buf[off..off + 2].copy_from_slice(&(idx as u16).to_le_bytes());
            }
        }
        gltf::accessor::DataType::U32 => {
            for (i, &idx) in indices.iter().enumerate() {
                let off = base + i * 4;
                buf[off..off + 4].copy_from_slice(&idx.to_le_bytes());
            }
        }
        _ => return Err("Unsupported index type".into()),
    }
    Ok(())
}

/// Remap a vertex attribute's data in the binary buffer using a remap table.
///
/// `remap[old_vertex_index] = new_vertex_index` (as returned by
/// `optimize_vertex_fetch_remap`).  We read each old vertex element from
/// `read_buf` and write it to the new position in `write_buf`.
fn remap_attribute_in_buffer(
    accessor: &gltf::Accessor<'_>,
    read_buf: &[u8],
    write_buf: &mut [u8],
    remap: &[u32],
    vertex_count: usize,
) -> Result<(), String> {
    let view = accessor.view().ok_or("No buffer view")?;
    let base = view.offset() + accessor.offset();
    let element_size = accessor.data_type().size() * accessor.dimensions().multiplicity();
    let stride = view.stride().unwrap_or(element_size);

    // Apply remap manually: for each old vertex, copy its data to the new slot
    for (old_idx, &remapped) in remap.iter().enumerate().take(vertex_count) {
        let new_idx = remapped as usize;
        if new_idx >= vertex_count {
            continue; // vertex was removed by remap
        }
        let src = base + old_idx * stride;
        let dst = base + new_idx * stride;
        // Use a temp copy to handle overlapping src/dst in the same buffer
        let mut tmp = vec![0u8; element_size];
        tmp.copy_from_slice(&read_buf[src..src + element_size]);
        write_buf[dst..dst + element_size].copy_from_slice(&tmp);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// GLB reconstruction
// ---------------------------------------------------------------------------

/// Reconstruct a GLB binary from JSON and BIN chunks.
fn rebuild_glb(json_bytes: &[u8], bin: &[u8]) -> Result<Vec<u8>, String> {
    // Pad JSON to 4-byte alignment with spaces
    let json_padded = (json_bytes.len() + 3) & !3;
    let json_pad = json_padded - json_bytes.len();

    // Pad BIN to 4-byte alignment with zeros
    let bin_padded = (bin.len() + 3) & !3;
    let bin_pad = bin_padded - bin.len();

    // Total: header(12) + json_chunk_header(8) + json + bin_chunk_header(8) + bin
    let total = 12 + 8 + json_padded + 8 + bin_padded;
    let mut out = Vec::with_capacity(total);

    // GLB header
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());

    // JSON chunk
    out.extend_from_slice(&(json_padded as u32).to_le_bytes());
    out.extend_from_slice(&0x4E4F534Au32.to_le_bytes()); // "JSON"
    out.extend_from_slice(json_bytes);
    out.extend(std::iter::repeat_n(b' ', json_pad));

    // BIN chunk
    out.extend_from_slice(&(bin_padded as u32).to_le_bytes());
    out.extend_from_slice(&0x004E4942u32.to_le_bytes()); // "BIN\0"
    out.extend_from_slice(bin);
    out.extend(std::iter::repeat_n(0, bin_pad));

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pack a glTF JSON string + binary blob into a minimal GLB.
    fn pack_test_glb(json: &str, bin: &[u8]) -> Vec<u8> {
        let json_bytes = json.as_bytes();
        let json_pad = (4 - json_bytes.len() % 4) % 4;
        let bin_pad = (4 - bin.len() % 4) % 4;
        let total = 12 + 8 + json_bytes.len() + json_pad + 8 + bin.len() + bin_pad;
        let mut out = Vec::new();
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&((json_bytes.len() + json_pad) as u32).to_le_bytes());
        out.extend_from_slice(&0x4E4F534Au32.to_le_bytes());
        out.extend_from_slice(json_bytes);
        out.extend(std::iter::repeat_n(b' ', json_pad));
        out.extend_from_slice(&((bin.len() + bin_pad) as u32).to_le_bytes());
        out.extend_from_slice(&0x004E4942u32.to_le_bytes());
        out.extend_from_slice(bin);
        out.extend(std::iter::repeat_n(0u8, bin_pad));
        out
    }

    fn all_on() -> MeshOptSettings {
        MeshOptSettings {
            vertex_cache: true,
            overdraw: true,
            vertex_fetch: true,
            simplify: false,
            simplify_ratio: 0.5,
            quantize: false,
            generate_lods: false,
            lod_levels: 3,
        }
    }

    fn read_positions(bin: &[u8], base: usize, count: usize) -> Vec<[f32; 3]> {
        (0..count)
            .map(|i| {
                let o = base + i * 12;
                [
                    f32::from_le_bytes(bin[o..o + 4].try_into().unwrap()),
                    f32::from_le_bytes(bin[o + 4..o + 8].try_into().unwrap()),
                    f32::from_le_bytes(bin[o + 8..o + 12].try_into().unwrap()),
                ]
            })
            .collect()
    }

    /// Two primitives sharing one POSITION accessor: vertex-fetch must be
    /// skipped so the shared position data is left byte-for-byte intact
    /// (this is the corruption that streaked Sponza).
    #[test]
    fn vertex_fetch_skips_shared_attribute_primitives() {
        let positions: [f32; 9] = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let mut bin = Vec::new();
        for p in positions {
            bin.extend_from_slice(&p.to_le_bytes());
        }
        for i in [0u16, 1, 2] {
            bin.extend_from_slice(&i.to_le_bytes()); // indices A @ 36
        }
        for i in [0u16, 1, 2] {
            bin.extend_from_slice(&i.to_le_bytes()); // indices B @ 42
        }

        let json = r#"{"asset":{"version":"2.0"},
            "buffers":[{"byteLength":48}],
            "bufferViews":[
                {"buffer":0,"byteOffset":0,"byteLength":36},
                {"buffer":0,"byteOffset":36,"byteLength":6},
                {"buffer":0,"byteOffset":42,"byteLength":6}],
            "accessors":[
                {"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]},
                {"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"},
                {"bufferView":2,"componentType":5123,"count":3,"type":"SCALAR"}],
            "meshes":[{"primitives":[
                {"attributes":{"POSITION":0},"indices":1},
                {"attributes":{"POSITION":0},"indices":2}]}]}"#;

        let glb = pack_test_glb(json, &bin);
        let out = optimize_glb(&glb, &all_on()).expect("optimize");
        let out_glb = gltf::Glb::from_slice(&out).expect("parse");
        let out_bin = out_glb.bin.expect("bin");

        assert_eq!(
            read_positions(&out_bin, 0, 3),
            read_positions(&bin, 0, 3),
            "shared POSITION data must be untouched when vertex-fetch is skipped",
        );
    }

    /// A primitive that exclusively owns its attributes still gets vertex-fetch
    /// applied — and the resolved triangle (indices → positions) must be the
    /// same set of vertices afterward, just possibly reordered in memory.
    #[test]
    fn vertex_fetch_preserves_geometry_when_exclusive() {
        let positions: [f32; 9] = [0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 3.0, 0.0];
        let mut bin = Vec::new();
        for p in positions {
            bin.extend_from_slice(&p.to_le_bytes());
        }
        for i in [0u16, 1, 2] {
            bin.extend_from_slice(&i.to_le_bytes()); // indices @ 36
        }

        let json = r#"{"asset":{"version":"2.0"},
            "buffers":[{"byteLength":42}],
            "bufferViews":[
                {"buffer":0,"byteOffset":0,"byteLength":36},
                {"buffer":0,"byteOffset":36,"byteLength":6}],
            "accessors":[
                {"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[2,3,0]},
                {"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}],
            "meshes":[{"primitives":[{"attributes":{"POSITION":0},"indices":1}]}]}"#;

        let glb = pack_test_glb(json, &bin);
        let out = optimize_glb(&glb, &all_on()).expect("optimize");
        let out_glb = gltf::Glb::from_slice(&out).expect("parse");
        let out_bin = out_glb.bin.expect("bin");

        // Resolve indices → positions and compare the vertex set to the input.
        let out_pos = read_positions(&out_bin, 0, 3);
        let mut resolved: Vec<[f32; 3]> = (0..3)
            .map(|i| {
                let o = 36 + i * 2;
                let idx = u16::from_le_bytes(out_bin[o..o + 2].try_into().unwrap()) as usize;
                out_pos[idx]
            })
            .collect();
        let mut expected = vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 3.0, 0.0]];
        let key = |v: &[f32; 3]| (v[0].to_bits(), v[1].to_bits(), v[2].to_bits());
        resolved.sort_by_key(key);
        expected.sort_by_key(key);
        assert_eq!(resolved, expected, "triangle vertices must survive vertex-fetch");
    }
}
