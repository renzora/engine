//! FBX binary parser for legacy versions (< 7400).
//!
//! The fbxcel library only supports FBX 7.4+. This module handles older binary
//! FBX files (e.g. version 6100) which share the same binary node structure but
//! are rejected by fbxcel's version check.

use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::Path;

use crate::convert::{ImportError, ImportResult};
use crate::obj::build_glb;
use crate::settings::{ImportSettings, UpAxis};

// ─── Node tree ──────────────────────────────────────────────────────────────

#[derive(Debug)]
pub(crate) enum FbxProp {
    Bool(bool),
    I16(i16),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    String(String),
    Raw(Vec<u8>),
    ArrF32(Vec<f32>),
    ArrF64(Vec<f64>),
    ArrI32(Vec<i32>),
    ArrI64(Vec<i64>),
    ArrBool(Vec<bool>),
}

#[derive(Debug)]
pub(crate) struct FbxNode {
    pub name: String,
    pub properties: Vec<FbxProp>,
    pub children: Vec<FbxNode>,
}

// ─── Binary parser ─────────────────────────────────────────────────────────

const HEADER_LEN: u64 = 27; // 23 magic + 2 padding + 4 version
const NULL_RECORD_LEN_V32: usize = 13; // 3×u32 + 1×u8 all zero (FBX < 7.5)
const NULL_RECORD_LEN_V64: usize = 25; // 3×u64 + 1×u8 all zero (FBX >= 7.5)

/// FBX 7.5 switched node record headers from u32 to u64 for end_offset,
/// num_properties, and property_list_len. Files at or above this version
/// use the wider header layout.
const FBX_VERSION_U64_HEADER: u32 = 7500;

fn read_u8(r: &mut Cursor<&[u8]>) -> Result<u8, ImportError> {
    let mut buf = [0u8; 1];
    r.read_exact(&mut buf)?;
    Ok(buf[0])
}

fn read_u16_le(r: &mut Cursor<&[u8]>) -> Result<u16, ImportError> {
    let mut buf = [0u8; 2];
    r.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

fn read_u32_le(r: &mut Cursor<&[u8]>) -> Result<u32, ImportError> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_i16_le(r: &mut Cursor<&[u8]>) -> Result<i16, ImportError> {
    let mut buf = [0u8; 2];
    r.read_exact(&mut buf)?;
    Ok(i16::from_le_bytes(buf))
}

fn read_i32_le(r: &mut Cursor<&[u8]>) -> Result<i32, ImportError> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(i32::from_le_bytes(buf))
}

fn read_i64_le(r: &mut Cursor<&[u8]>) -> Result<i64, ImportError> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    Ok(i64::from_le_bytes(buf))
}

fn read_f32_le(r: &mut Cursor<&[u8]>) -> Result<f32, ImportError> {
    let mut buf = [0u8; 4];
    r.read_exact(&mut buf)?;
    Ok(f32::from_le_bytes(buf))
}

fn read_f64_le(r: &mut Cursor<&[u8]>) -> Result<f64, ImportError> {
    let mut buf = [0u8; 8];
    r.read_exact(&mut buf)?;
    Ok(f64::from_le_bytes(buf))
}

fn read_bytes(r: &mut Cursor<&[u8]>, len: usize) -> Result<Vec<u8>, ImportError> {
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf)?;
    Ok(buf)
}

/// Decode a binary array property. Layout: u32 count, u32 encoding, u32 compressed_len, data.
fn read_array_data(r: &mut Cursor<&[u8]>, elem_size: usize) -> Result<Vec<u8>, ImportError> {
    let count = read_u32_le(r)? as usize;
    let encoding = read_u32_le(r)?;
    let compressed_len = read_u32_le(r)? as usize;

    let raw = read_bytes(r, compressed_len)?;

    let data = if encoding == 1 {
        // zlib compressed
        use flate2::read::ZlibDecoder;
        let mut decoder = ZlibDecoder::new(&raw[..]);
        let mut decompressed = vec![0u8; count * elem_size];
        decoder.read_exact(&mut decompressed).map_err(|e| {
            ImportError::ParseError(format!("zlib decompression failed: {}", e))
        })?;
        decompressed
    } else {
        raw
    };

    if data.len() < count * elem_size {
        return Err(ImportError::ParseError("array data too short".into()));
    }

    Ok(data)
}

fn parse_property(r: &mut Cursor<&[u8]>) -> Result<FbxProp, ImportError> {
    let type_code = read_u8(r)?;
    match type_code {
        b'C' => Ok(FbxProp::Bool(read_u8(r)? != 0)),
        b'Y' => Ok(FbxProp::I16(read_i16_le(r)?)),
        b'I' => Ok(FbxProp::I32(read_i32_le(r)?)),
        b'L' => Ok(FbxProp::I64(read_i64_le(r)?)),
        b'F' => Ok(FbxProp::F32(read_f32_le(r)?)),
        b'D' => Ok(FbxProp::F64(read_f64_le(r)?)),
        b'S' => {
            let len = read_u32_le(r)? as usize;
            let bytes = read_bytes(r, len)?;
            Ok(FbxProp::String(String::from_utf8_lossy(&bytes).into_owned()))
        }
        b'R' => {
            let len = read_u32_le(r)? as usize;
            Ok(FbxProp::Raw(read_bytes(r, len)?))
        }
        b'f' => {
            let data = read_array_data(r, 4)?;
            let arr: Vec<f32> = data
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            Ok(FbxProp::ArrF32(arr))
        }
        b'd' => {
            let data = read_array_data(r, 8)?;
            let arr: Vec<f64> = data
                .chunks_exact(8)
                .map(|c| f64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]))
                .collect();
            Ok(FbxProp::ArrF64(arr))
        }
        b'i' => {
            let data = read_array_data(r, 4)?;
            let arr: Vec<i32> = data
                .chunks_exact(4)
                .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            Ok(FbxProp::ArrI32(arr))
        }
        b'l' => {
            let data = read_array_data(r, 8)?;
            let arr: Vec<i64> = data
                .chunks_exact(8)
                .map(|c| i64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]))
                .collect();
            Ok(FbxProp::ArrI64(arr))
        }
        b'b' => {
            let data = read_array_data(r, 1)?;
            let arr: Vec<bool> = data.iter().map(|&b| b != 0).collect();
            Ok(FbxProp::ArrBool(arr))
        }
        _ => Err(ImportError::ParseError(format!(
            "unknown FBX property type: 0x{:02X} ('{}')",
            type_code,
            type_code as char,
        ))),
    }
}

fn parse_node(
    r: &mut Cursor<&[u8]>,
    file_len: u64,
    use_u64_headers: bool,
) -> Result<Option<FbxNode>, ImportError> {
    let start = r.position();

    let null_record_len = if use_u64_headers {
        NULL_RECORD_LEN_V64
    } else {
        NULL_RECORD_LEN_V32
    };

    // Check if we have enough bytes for a record header
    if start + null_record_len as u64 > file_len {
        return Ok(None);
    }

    let (end_offset, num_properties, property_list_len) = if use_u64_headers {
        (
            read_i64_le(r)? as u64,
            read_i64_le(r)? as u32,
            read_i64_le(r)? as u64,
        )
    } else {
        (
            read_u32_le(r)? as u64,
            read_u32_le(r)? as u32,
            read_u32_le(r)? as u64,
        )
    };
    let name_len = read_u8(r)? as usize;

    // Null sentinel: all zeros
    if end_offset == 0 && num_properties == 0 && property_list_len == 0 && name_len == 0 {
        return Ok(None);
    }

    if end_offset > file_len {
        return Err(ImportError::ParseError(format!(
            "node end_offset {} exceeds file length {}",
            end_offset, file_len
        )));
    }

    let name_bytes = read_bytes(r, name_len)?;
    let name = String::from_utf8_lossy(&name_bytes).into_owned();

    // Parse properties
    let props_end = r.position() + property_list_len;
    let mut properties = Vec::with_capacity(num_properties as usize);
    for _ in 0..num_properties {
        properties.push(parse_property(r)?);
    }
    // Skip any remaining property bytes (shouldn't happen, but be safe)
    if r.position() < props_end {
        r.seek(SeekFrom::Start(props_end))?;
    }

    // Parse children until end_offset
    let mut children = Vec::new();
    while r.position() < end_offset {
        // Check for null sentinel
        if r.position() + null_record_len as u64 > end_offset {
            break;
        }
        match parse_node(r, file_len, use_u64_headers)? {
            Some(child) => children.push(child),
            None => break, // null sentinel
        }
    }

    // Ensure we're at end_offset
    r.seek(SeekFrom::Start(end_offset))?;

    Ok(Some(FbxNode {
        name,
        properties,
        children,
    }))
}

pub(crate) fn parse_document(data: &[u8]) -> Result<(u32, Vec<FbxNode>), ImportError> {
    if data.len() < HEADER_LEN as usize {
        return Err(ImportError::ParseError("file too short for FBX header".into()));
    }

    let version = u32::from_le_bytes([data[23], data[24], data[25], data[26]]);
    let use_u64_headers = version >= FBX_VERSION_U64_HEADER;

    let mut r = Cursor::new(data);
    r.seek(SeekFrom::Start(HEADER_LEN))?;

    let file_len = data.len() as u64;
    let mut nodes = Vec::new();
    while r.position() < file_len {
        match parse_node(&mut r, file_len, use_u64_headers)? {
            Some(node) => nodes.push(node),
            None => break,
        }
    }

    Ok((version, nodes))
}

// ─── Data extraction helpers ────────────────────────────────────────────────

pub(crate) fn find_child<'a>(node: &'a FbxNode, name: &str) -> Option<&'a FbxNode> {
    node.children.iter().find(|c| c.name == name)
}

pub(crate) fn find_node_recursive<'a>(nodes: &'a [FbxNode], name: &str) -> Option<&'a FbxNode> {
    for node in nodes {
        if node.name == name {
            return Some(node);
        }
        if let Some(found) = find_node_recursive(&node.children, name) {
            return Some(found);
        }
    }
    None
}

pub(crate) fn find_all_recursive<'a>(nodes: &'a [FbxNode], name: &str, out: &mut Vec<&'a FbxNode>) {
    for node in nodes {
        if node.name == name {
            out.push(node);
        }
        find_all_recursive(&node.children, name, out);
    }
}

/// Extract f64 values from a node's first array property, or from individual numeric properties.
pub(crate) fn extract_f64_array(node: &FbxNode) -> Vec<f64> {
    for prop in &node.properties {
        match prop {
            FbxProp::ArrF64(arr) => return arr.clone(),
            FbxProp::ArrF32(arr) => return arr.iter().map(|&v| v as f64).collect(),
            _ => {}
        }
    }
    // Fall back: collect individual numeric properties
    let mut values = Vec::new();
    for prop in &node.properties {
        match prop {
            FbxProp::F64(v) => values.push(*v),
            FbxProp::F32(v) => values.push(*v as f64),
            FbxProp::I32(v) => values.push(*v as f64),
            FbxProp::I64(v) => values.push(*v as f64),
            _ => {}
        }
    }
    values
}

/// Extract i32 values from a node's first array property, or from individual numeric properties.
pub(crate) fn extract_i32_array(node: &FbxNode) -> Vec<i32> {
    for prop in &node.properties {
        match prop {
            FbxProp::ArrI32(arr) => return arr.clone(),
            FbxProp::ArrI64(arr) => return arr.iter().map(|&v| v as i32).collect(),
            _ => {}
        }
    }
    let mut values = Vec::new();
    for prop in &node.properties {
        match prop {
            FbxProp::I32(v) => values.push(*v),
            FbxProp::I64(v) => values.push(*v as i32),
            _ => {}
        }
    }
    values
}

/// Extract i64 values from a node's first i64 array property.
pub(crate) fn extract_i64_array(node: &FbxNode) -> Vec<i64> {
    for prop in &node.properties {
        match prop {
            FbxProp::ArrI64(arr) => return arr.clone(),
            FbxProp::ArrI32(arr) => return arr.iter().map(|&v| v as i64).collect(),
            _ => {}
        }
    }
    let mut values = Vec::new();
    for prop in &node.properties {
        match prop {
            FbxProp::I64(v) => values.push(*v),
            FbxProp::I32(v) => values.push(*v as i64),
            _ => {}
        }
    }
    values
}

/// Extract f32 values from a node's first f32 array property.
pub(crate) fn extract_f32_array(node: &FbxNode) -> Vec<f32> {
    for prop in &node.properties {
        match prop {
            FbxProp::ArrF32(arr) => return arr.clone(),
            FbxProp::ArrF64(arr) => return arr.iter().map(|&v| v as f32).collect(),
            _ => {}
        }
    }
    let mut values = Vec::new();
    for prop in &node.properties {
        match prop {
            FbxProp::F32(v) => values.push(*v),
            FbxProp::F64(v) => values.push(*v as f32),
            FbxProp::I32(v) => values.push(*v as f32),
            _ => {}
        }
    }
    values
}

pub(crate) fn get_i64_prop(node: &FbxNode, index: usize) -> Option<i64> {
    node.properties.get(index).and_then(|p| match p {
        FbxProp::I64(v) => Some(*v),
        FbxProp::I32(v) => Some(*v as i64),
        _ => None,
    })
}

fn extract_mapping_type(node: &FbxNode) -> Option<String> {
    find_child(node, "MappingInformationType").and_then(|n| {
        n.properties.iter().find_map(|p| match p {
            FbxProp::String(s) => Some(s.clone()),
            _ => None,
        })
    })
}

pub(crate) fn get_string_prop(node: &FbxNode, index: usize) -> Option<&str> {
    node.properties.get(index).and_then(|p| match p {
        FbxProp::String(s) => Some(s.as_str()),
        _ => None,
    })
}

fn detect_up_axis(nodes: &[FbxNode]) -> Option<UpAxis> {
    let settings = find_node_recursive(nodes, "GlobalSettings")?;
    let props = find_child(settings, "Properties60")
        .or_else(|| find_child(settings, "Properties70"))?;

    for child in &props.children {
        if child.name == "P" || child.name == "Property" {
            if get_string_prop(child, 0) == Some("UpAxis") {
                // Value is typically the last property
                for prop in child.properties.iter().rev() {
                    match prop {
                        FbxProp::I32(v) => {
                            return match v {
                                2 => Some(UpAxis::ZUp),
                                _ => Some(UpAxis::YUp),
                            };
                        }
                        FbxProp::I64(v) => {
                            return match *v as i32 {
                                2 => Some(UpAxis::ZUp),
                                _ => Some(UpAxis::YUp),
                            };
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    None
}

// ─── Conversion ─────────────────────────────────────────────────────────────

fn convert_axis(_x: &mut f32, y: &mut f32, z: &mut f32, up_axis: UpAxis) {
    if up_axis == UpAxis::ZUp {
        let tmp = *y;
        *y = *z;
        *z = -tmp;
    }
}

fn decode_fbx_index(raw: i32) -> u32 {
    if raw < 0 { (-raw - 1) as u32 } else { raw as u32 }
}

pub fn convert(path: &Path, settings: &ImportSettings) -> Result<ImportResult, ImportError> {
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");

    let data = std::fs::read(path)?;
    let (version, nodes) = parse_document(&data)?;

    log::info!(
        "[import] {}: legacy FBX binary version {}, parsed {} top-level nodes",
        file_name,
        version,
        nodes.len()
    );

    if nodes.is_empty() {
        return Err(ImportError::ParseError(
            "no nodes found in FBX binary file".into(),
        ));
    }

    let mut all_positions = Vec::new();
    let mut all_normals = Vec::new();
    let mut all_texcoords = Vec::new();
    let mut all_indices = Vec::new();
    let mut warnings = Vec::new();

    let effective_up_axis = if settings.up_axis == UpAxis::Auto {
        detect_up_axis(&nodes).unwrap_or(UpAxis::YUp)
    } else {
        settings.up_axis
    };

    // Find all Geometry nodes
    let mut geometry_nodes = Vec::new();
    find_all_recursive(&nodes, "Geometry", &mut geometry_nodes);

    // FBX 6.x: geometry may be directly inside Model nodes
    if geometry_nodes.is_empty() {
        log::info!(
            "[import] {}: no Geometry nodes, scanning Model nodes (FBX 6.x style)",
            file_name
        );
        let mut model_nodes = Vec::new();
        find_all_recursive(&nodes, "Model", &mut model_nodes);
        for model in model_nodes {
            if find_child(model, "Vertices").is_some() {
                geometry_nodes.push(model);
            }
        }
    }

    log::info!(
        "[import] {}: found {} geometry objects",
        file_name,
        geometry_nodes.len()
    );

    for geo_node in &geometry_nodes {
        let raw_vertices = match find_child(geo_node, "Vertices") {
            Some(n) => extract_f64_array(n),
            None => continue,
        };

        if raw_vertices.is_empty() {
            continue;
        }

        let raw_indices = match find_child(geo_node, "PolygonVertexIndex") {
            Some(n) => extract_i32_array(n),
            None => {
                warnings.push("geometry has no PolygonVertexIndex".into());
                continue;
            }
        };

        // Normals
        let normal_layer = find_child(geo_node, "LayerElementNormal");
        let raw_normals = normal_layer
            .and_then(|n| find_child(n, "Normals"))
            .map(extract_f64_array)
            .unwrap_or_default();
        let normal_mapping = normal_layer.and_then(extract_mapping_type);

        // UVs
        let uv_layer = find_child(geo_node, "LayerElementUV");
        let raw_uvs = uv_layer
            .and_then(|n| find_child(n, "UV"))
            .map(extract_f64_array)
            .unwrap_or_default();
        let uv_indices = uv_layer
            .and_then(|n| find_child(n, "UVIndex"))
            .map(extract_i32_array)
            .unwrap_or_default();
        let uv_mapping = uv_layer.and_then(extract_mapping_type);

        let base_vertex = (all_positions.len() / 3) as u32;
        let vertex_count = raw_vertices.len() / 3;

        // Add positions
        for i in 0..vertex_count {
            let (mut x, mut y, mut z) = (
                raw_vertices[i * 3] as f32 * settings.scale,
                raw_vertices[i * 3 + 1] as f32 * settings.scale,
                raw_vertices[i * 3 + 2] as f32 * settings.scale,
            );
            convert_axis(&mut x, &mut y, &mut z, effective_up_axis);
            all_positions.extend_from_slice(&[x, y, z]);
        }

        let mut geo_normals = vec![0.0f32; vertex_count * 3];
        let mut geo_texcoords = vec![0.0f32; vertex_count * 2];
        let mut geo_has_normals = false;

        // Parse polygons and triangulate
        let mut polygon_start = 0usize;
        let mut polygon_vertex_idx = 0usize;

        for (raw_idx_pos, &raw_idx) in raw_indices.iter().enumerate() {
            let is_end = raw_idx < 0;
            let vertex_idx = if is_end {
                (-raw_idx - 1) as usize
            } else {
                raw_idx as usize
            };

            // Map normals
            if !raw_normals.is_empty() {
                let ni = match normal_mapping.as_deref() {
                    Some("ByPolygonVertex") => polygon_vertex_idx,
                    Some("ByVertice") | Some("ByVertex") => vertex_idx,
                    _ => polygon_vertex_idx,
                };

                if ni * 3 + 2 < raw_normals.len() {
                    let (mut nx, mut ny, mut nz) = (
                        raw_normals[ni * 3] as f32,
                        raw_normals[ni * 3 + 1] as f32,
                        raw_normals[ni * 3 + 2] as f32,
                    );
                    convert_axis(&mut nx, &mut ny, &mut nz, effective_up_axis);
                    geo_normals[vertex_idx * 3] = nx;
                    geo_normals[vertex_idx * 3 + 1] = ny;
                    geo_normals[vertex_idx * 3 + 2] = nz;
                    geo_has_normals = true;
                }
            }

            // Map UVs
            if !raw_uvs.is_empty() {
                let ui = if !uv_indices.is_empty() {
                    if polygon_vertex_idx < uv_indices.len() {
                        uv_indices[polygon_vertex_idx] as usize
                    } else {
                        0
                    }
                } else {
                    match uv_mapping.as_deref() {
                        Some("ByPolygonVertex") => polygon_vertex_idx,
                        Some("ByVertice") | Some("ByVertex") => vertex_idx,
                        _ => polygon_vertex_idx,
                    }
                };

                if ui * 2 + 1 < raw_uvs.len() {
                    let u = raw_uvs[ui * 2] as f32;
                    let v = if settings.flip_uvs {
                        1.0 - raw_uvs[ui * 2 + 1] as f32
                    } else {
                        raw_uvs[ui * 2 + 1] as f32
                    };
                    geo_texcoords[vertex_idx * 2] = u;
                    geo_texcoords[vertex_idx * 2 + 1] = v;
                }
            }

            polygon_vertex_idx += 1;

            if is_end {
                let poly_len = raw_idx_pos - polygon_start + 1;
                if poly_len >= 3 {
                    let first_vi = decode_fbx_index(raw_indices[polygon_start]);
                    for i in 1..poly_len - 1 {
                        let v1 = decode_fbx_index(raw_indices[polygon_start + i]);
                        let v2 = decode_fbx_index(raw_indices[polygon_start + i + 1]);
                        all_indices.push(first_vi + base_vertex);
                        all_indices.push(v1 + base_vertex);
                        all_indices.push(v2 + base_vertex);
                    }
                }
                polygon_start = raw_idx_pos + 1;
            }
        }

        // Generate normals if needed
        if !geo_has_normals && settings.generate_normals {
            generate_flat_normals(
                &all_positions,
                &all_indices,
                base_vertex,
                vertex_count,
                &mut geo_normals,
            );
        }

        all_normals.extend_from_slice(&geo_normals);
        all_texcoords.extend_from_slice(&geo_texcoords);
    }

    if all_positions.is_empty() {
        return Err(ImportError::ParseError(
            "no geometry found in legacy FBX binary file".into(),
        ));
    }

    let vertex_count = all_positions.len() / 3;
    let tri_count = all_indices.len() / 3;
    log::info!(
        "[import] {}: {} vertices, {} triangles, {} warnings",
        file_name,
        vertex_count,
        tri_count,
        warnings.len()
    );
    for w in &warnings {
        log::warn!("[import] {}: {}", file_name, w);
    }

    let glb_bytes = build_glb(&all_positions, &all_normals, &all_texcoords, &all_indices, &crate::obj::MaterialBundle::default())?;

    log::info!(
        "[import] {}: GLB output {} bytes",
        file_name,
        glb_bytes.len()
    );

    Ok(ImportResult {
        glb_bytes,
        warnings, extracted_textures: Vec::new(),
    })
}

fn generate_flat_normals(
    positions: &[f32],
    indices: &[u32],
    base_vertex: u32,
    vertex_count: usize,
    normals: &mut [f32],
) {
    for tri in indices.chunks(3) {
        if tri.len() < 3 {
            break;
        }
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);

        let p0 = &positions[i0 * 3..i0 * 3 + 3];
        let p1 = &positions[i1 * 3..i1 * 3 + 3];
        let p2 = &positions[i2 * 3..i2 * 3 + 3];

        let e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        let e2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
        let n = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];

        for &idx in &[i0, i1, i2] {
            let local = idx - base_vertex as usize;
            if local < vertex_count {
                normals[local * 3] += n[0];
                normals[local * 3 + 1] += n[1];
                normals[local * 3 + 2] += n[2];
            }
        }
    }

    for i in 0..vertex_count {
        let (x, y, z) = (normals[i * 3], normals[i * 3 + 1], normals[i * 3 + 2]);
        let len = (x * x + y * y + z * z).sqrt();
        if len > 1e-8 {
            normals[i * 3] /= len;
            normals[i * 3 + 1] /= len;
            normals[i * 3 + 2] /= len;
        } else {
            normals[i * 3 + 1] = 1.0;
        }
    }
}
