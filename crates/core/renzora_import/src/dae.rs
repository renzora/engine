//! Collada (.dae) → GLB converter.
//!
//! Parses Collada XML files and extracts mesh geometry (positions, normals,
//! UVs, triangulated indices). Uses `quick-xml` for XML parsing.

use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::convert::{ImportError, ImportResult};
use crate::settings::{ImportSettings, UpAxis};

pub fn convert(path: &Path, settings: &ImportSettings) -> Result<ImportResult, ImportError> {
    let xml = std::fs::read_to_string(path)
        .map_err(|e| ImportError::ParseError(format!("Failed to read DAE: {}", e)))?;

    let mut warnings = Vec::new();
    let geometries = parse_collada(&xml, &mut warnings)?;

    if geometries.is_empty() {
        return Err(ImportError::ParseError(
            "No mesh geometry found in Collada file".into(),
        ));
    }

    let mut all_positions: Vec<f32> = Vec::new();
    let mut all_normals: Vec<f32> = Vec::new();
    let mut all_texcoords: Vec<f32> = Vec::new();
    let mut all_indices: Vec<u32> = Vec::new();

    // Detect up axis from the Collada file
    let collada_up = detect_up_axis(&xml);
    let apply_z_up = match settings.up_axis {
        UpAxis::Auto => collada_up == ColladaUpAxis::ZUp,
        UpAxis::ZUp => true,
        UpAxis::YUp => false,
    };

    for geom in &geometries {
        if geom.positions.is_empty() {
            continue;
        }

        let base_vertex = (all_positions.len() / 3) as u32;
        let vertex_count = geom.positions.len() / 3;

        // Apply scale and axis conversion
        for i in 0..vertex_count {
            let (x, mut y, mut z) = (
                geom.positions[i * 3] * settings.scale,
                geom.positions[i * 3 + 1] * settings.scale,
                geom.positions[i * 3 + 2] * settings.scale,
            );

            if apply_z_up {
                let tmp = y;
                y = z;
                z = -tmp;
            }

            all_positions.extend_from_slice(&[x, y, z]);
        }

        // Normals
        if geom.normals.len() == vertex_count * 3 {
            for i in 0..vertex_count {
                let (nx, mut ny, mut nz) = (
                    geom.normals[i * 3],
                    geom.normals[i * 3 + 1],
                    geom.normals[i * 3 + 2],
                );

                if apply_z_up {
                    let tmp = ny;
                    ny = nz;
                    nz = -tmp;
                }

                all_normals.extend_from_slice(&[nx, ny, nz]);
            }
        } else if settings.generate_normals {
            // Will generate after all meshes are collected
        } else {
            all_normals.extend(std::iter::repeat(0.0f32).take(vertex_count * 3));
        }

        // UVs
        if geom.texcoords.len() == vertex_count * 2 {
            for i in 0..vertex_count {
                let u = geom.texcoords[i * 2];
                let v = if settings.flip_uvs {
                    1.0 - geom.texcoords[i * 2 + 1]
                } else {
                    geom.texcoords[i * 2 + 1]
                };
                all_texcoords.extend_from_slice(&[u, v]);
            }
        } else {
            all_texcoords.extend(std::iter::repeat(0.0f32).take(vertex_count * 2));
        }

        // Indices
        for &idx in &geom.indices {
            all_indices.push(idx + base_vertex);
        }
    }

    if all_positions.is_empty() {
        return Err(ImportError::ParseError(
            "No valid geometry found in Collada file".into(),
        ));
    }

    // Generate normals if missing
    if all_normals.len() != all_positions.len() && settings.generate_normals {
        let vertex_count = all_positions.len() / 3;
        all_normals = generate_flat_normals(&all_positions, &all_indices, vertex_count);
    }

    let glb_bytes =
        crate::obj::build_glb(&all_positions, &all_normals, &all_texcoords, &all_indices)?;

    Ok(ImportResult {
        glb_bytes,
        warnings,
    })
}

// ---------------------------------------------------------------------------
// Collada XML parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct ColladaGeometry {
    positions: Vec<f32>,
    normals: Vec<f32>,
    texcoords: Vec<f32>,
    indices: Vec<u32>,
}

#[derive(Debug, PartialEq)]
enum ColladaUpAxis {
    YUp,
    ZUp,
}

fn detect_up_axis(xml: &str) -> ColladaUpAxis {
    // Quick scan for <up_axis>Z_UP</up_axis>
    if xml.contains("<up_axis>Z_UP</up_axis>") {
        ColladaUpAxis::ZUp
    } else {
        ColladaUpAxis::YUp
    }
}

fn parse_collada(xml: &str, warnings: &mut Vec<String>) -> Result<Vec<ColladaGeometry>, ImportError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut geometries: Vec<ColladaGeometry> = Vec::new();

    // Source arrays keyed by their ID
    let mut float_sources: Vec<(String, Vec<f32>)> = Vec::new();

    // Parsing state
    let mut in_geometry = false;
    let mut in_mesh = false;
    let mut in_float_array = false;
    let mut in_triangles = false;
    let mut in_polylist = false;
    let mut in_p = false;
    let mut in_vcount = false;
    let mut current_source_id = String::new();
    let mut current_text = String::new();

    // Semantic offsets for the current triangle/polylist element
    let mut vertex_offset: usize = 0;
    let mut normal_offset: Option<usize> = None;
    let mut texcoord_offset: Option<usize> = None;
    let mut input_count: usize = 1;
    let mut position_source_id = String::new();
    let mut normal_source_id = String::new();
    let mut texcoord_source_id = String::new();
    let mut vcount_data: Vec<u32> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");

                match name {
                    "geometry" => {
                        in_geometry = true;
                    }
                    "mesh" if in_geometry => {
                        in_mesh = true;
                    }
                    "source" if in_mesh => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"id" {
                                current_source_id = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    }
                    "float_array" if in_mesh => {
                        in_float_array = true;
                        current_text.clear();
                    }
                    "vertices" if in_mesh => {
                        // Map vertices ID to the position source
                        let mut vertices_id = String::new();
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"id" {
                                vertices_id = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                        // We'll handle the input child elements
                        let _ = vertices_id;
                    }
                    "input" if in_mesh => {
                        let mut semantic = String::new();
                        let mut source = String::new();
                        let mut offset: Option<usize> = None;

                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"semantic" => semantic = String::from_utf8_lossy(&attr.value).to_string(),
                                b"source" => source = String::from_utf8_lossy(&attr.value).trim_start_matches('#').to_string(),
                                b"offset" => offset = String::from_utf8_lossy(&attr.value).parse().ok(),
                                _ => {}
                            }
                        }

                        if in_triangles || in_polylist {
                            match semantic.as_str() {
                                "VERTEX" => {
                                    vertex_offset = offset.unwrap_or(0);
                                    position_source_id = source;
                                }
                                "NORMAL" => {
                                    normal_offset = offset;
                                    normal_source_id = source;
                                }
                                "TEXCOORD" => {
                                    texcoord_offset = offset;
                                    texcoord_source_id = source;
                                }
                                _ => {}
                            }
                            if let Some(off) = offset {
                                if off + 1 > input_count {
                                    input_count = off + 1;
                                }
                            }
                        } else if semantic == "POSITION" {
                            position_source_id = source;
                        }
                    }
                    "triangles" if in_mesh => {
                        in_triangles = true;
                        input_count = 1;
                        normal_offset = None;
                        texcoord_offset = None;
                    }
                    "polylist" if in_mesh => {
                        in_polylist = true;
                        input_count = 1;
                        normal_offset = None;
                        texcoord_offset = None;
                        vcount_data.clear();
                    }
                    "p" if in_triangles || in_polylist => {
                        in_p = true;
                        current_text.clear();
                    }
                    "vcount" if in_polylist => {
                        in_vcount = true;
                        current_text.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name();
                let name = std::str::from_utf8(local.as_ref()).unwrap_or("");

                match name {
                    "geometry" => {
                        in_geometry = false;
                        in_mesh = false;
                    }
                    "mesh" => {
                        in_mesh = false;
                    }
                    "float_array" => {
                        if in_float_array {
                            let floats: Vec<f32> = current_text
                                .split_whitespace()
                                .filter_map(|s| s.parse().ok())
                                .collect();
                            float_sources.push((current_source_id.clone(), floats));
                            in_float_array = false;
                        }
                    }
                    "vcount" => {
                        if in_vcount {
                            vcount_data = current_text
                                .split_whitespace()
                                .filter_map(|s| s.parse().ok())
                                .collect();
                            in_vcount = false;
                        }
                    }
                    "p" => {
                        if in_p {
                            let p_data: Vec<i32> = current_text
                                .split_whitespace()
                                .filter_map(|s| s.parse().ok())
                                .collect();

                            // Find source arrays
                            let pos_data = find_source(&float_sources, &position_source_id);
                            let norm_data = find_source(&float_sources, &normal_source_id);
                            let tc_data = find_source(&float_sources, &texcoord_source_id);

                            if let Some(pos) = pos_data {
                                let geom = if in_polylist {
                                    build_geometry_polylist(
                                        pos,
                                        norm_data,
                                        tc_data,
                                        &p_data,
                                        &vcount_data,
                                        vertex_offset,
                                        normal_offset,
                                        texcoord_offset,
                                        input_count,
                                    )
                                } else {
                                    build_geometry_triangles(
                                        pos,
                                        norm_data,
                                        tc_data,
                                        &p_data,
                                        vertex_offset,
                                        normal_offset,
                                        texcoord_offset,
                                        input_count,
                                    )
                                };
                                geometries.push(geom);
                            }
                            in_p = false;
                        }
                    }
                    "triangles" => {
                        in_triangles = false;
                    }
                    "polylist" => {
                        in_polylist = false;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_float_array || in_p || in_vcount {
                    if let Ok(text) = e.unescape() {
                        current_text.push_str(&text);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(ImportError::ParseError(format!(
                    "Collada XML parse error: {}",
                    e
                )));
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(geometries)
}

fn find_source<'a>(sources: &'a [(String, Vec<f32>)], id: &str) -> Option<&'a [f32]> {
    // Try exact match first, then suffix match (for vertices indirection)
    sources
        .iter()
        .find(|(sid, _)| sid == id || sid.ends_with(&format!("-{}", id)) || id.ends_with(sid.as_str()))
        .map(|(_, data)| data.as_slice())
        .or_else(|| {
            // Try matching by common naming: if id is "X-vertices", look for "X-positions"
            let positions_id = id.replace("-vertices", "-positions");
            sources
                .iter()
                .find(|(sid, _)| *sid == positions_id || sid.contains("position"))
                .map(|(_, data)| data.as_slice())
        })
}

fn build_geometry_triangles(
    positions: &[f32],
    normals: Option<&[f32]>,
    texcoords: Option<&[f32]>,
    p_data: &[i32],
    vertex_offset: usize,
    normal_offset: Option<usize>,
    texcoord_offset: Option<usize>,
    input_count: usize,
) -> ColladaGeometry {
    let mut geom = ColladaGeometry::default();
    let stride = input_count;

    if stride == 0 {
        return geom;
    }

    let tri_count = p_data.len() / (stride * 3);

    for tri in 0..tri_count {
        for vert in 0..3 {
            let base = (tri * 3 + vert) * stride;

            // Position
            let pos_idx = p_data.get(base + vertex_offset).copied().unwrap_or(0) as usize;
            if pos_idx * 3 + 2 < positions.len() {
                geom.positions.extend_from_slice(&positions[pos_idx * 3..pos_idx * 3 + 3]);
            } else {
                geom.positions.extend_from_slice(&[0.0, 0.0, 0.0]);
            }

            // Normal
            if let (Some(off), Some(norms)) = (normal_offset, normals) {
                let norm_idx = p_data.get(base + off).copied().unwrap_or(0) as usize;
                if norm_idx * 3 + 2 < norms.len() {
                    geom.normals.extend_from_slice(&norms[norm_idx * 3..norm_idx * 3 + 3]);
                } else {
                    geom.normals.extend_from_slice(&[0.0, 1.0, 0.0]);
                }
            }

            // Texcoord
            if let (Some(off), Some(tcs)) = (texcoord_offset, texcoords) {
                let tc_idx = p_data.get(base + off).copied().unwrap_or(0) as usize;
                if tc_idx * 2 + 1 < tcs.len() {
                    geom.texcoords.extend_from_slice(&tcs[tc_idx * 2..tc_idx * 2 + 2]);
                } else {
                    geom.texcoords.extend_from_slice(&[0.0, 0.0]);
                }
            }

            geom.indices.push((geom.positions.len() / 3 - 1) as u32);
        }
    }

    geom
}

fn build_geometry_polylist(
    positions: &[f32],
    normals: Option<&[f32]>,
    texcoords: Option<&[f32]>,
    p_data: &[i32],
    vcount: &[u32],
    vertex_offset: usize,
    normal_offset: Option<usize>,
    texcoord_offset: Option<usize>,
    input_count: usize,
) -> ColladaGeometry {
    let mut geom = ColladaGeometry::default();
    let stride = input_count;

    if stride == 0 {
        return geom;
    }

    let mut p_offset = 0usize;

    for &count in vcount {
        let n = count as usize;
        if p_offset + n * stride > p_data.len() {
            break;
        }

        // Collect vertices for this face
        let mut face_verts: Vec<usize> = Vec::with_capacity(n);
        for v in 0..n {
            let base = p_offset + v * stride;
            let vert_idx = geom.positions.len() / 3;

            // Position
            let pos_idx = p_data.get(base + vertex_offset).copied().unwrap_or(0) as usize;
            if pos_idx * 3 + 2 < positions.len() {
                geom.positions.extend_from_slice(&positions[pos_idx * 3..pos_idx * 3 + 3]);
            } else {
                geom.positions.extend_from_slice(&[0.0, 0.0, 0.0]);
            }

            // Normal
            if let (Some(off), Some(norms)) = (normal_offset, normals) {
                let norm_idx = p_data.get(base + off).copied().unwrap_or(0) as usize;
                if norm_idx * 3 + 2 < norms.len() {
                    geom.normals.extend_from_slice(&norms[norm_idx * 3..norm_idx * 3 + 3]);
                } else {
                    geom.normals.extend_from_slice(&[0.0, 1.0, 0.0]);
                }
            }

            // Texcoord
            if let (Some(off), Some(tcs)) = (texcoord_offset, texcoords) {
                let tc_idx = p_data.get(base + off).copied().unwrap_or(0) as usize;
                if tc_idx * 2 + 1 < tcs.len() {
                    geom.texcoords.extend_from_slice(&tcs[tc_idx * 2..tc_idx * 2 + 2]);
                } else {
                    geom.texcoords.extend_from_slice(&[0.0, 0.0]);
                }
            }

            face_verts.push(vert_idx);
        }

        // Fan triangulate
        if face_verts.len() >= 3 {
            let v0 = face_verts[0] as u32;
            for i in 1..face_verts.len() - 1 {
                geom.indices.push(v0);
                geom.indices.push(face_verts[i] as u32);
                geom.indices.push(face_verts[i + 1] as u32);
            }
        }

        p_offset += n * stride;
    }

    geom
}

fn generate_flat_normals(positions: &[f32], indices: &[u32], vertex_count: usize) -> Vec<f32> {
    let mut normals = vec![0.0f32; vertex_count * 3];

    for tri in indices.chunks(3) {
        if tri.len() < 3 {
            break;
        }
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        if i0 * 3 + 2 >= positions.len()
            || i1 * 3 + 2 >= positions.len()
            || i2 * 3 + 2 >= positions.len()
        {
            continue;
        }

        let p0 = [positions[i0 * 3], positions[i0 * 3 + 1], positions[i0 * 3 + 2]];
        let p1 = [positions[i1 * 3], positions[i1 * 3 + 1], positions[i1 * 3 + 2]];
        let p2 = [positions[i2 * 3], positions[i2 * 3 + 1], positions[i2 * 3 + 2]];

        let e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        let e2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];

        let n = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];

        for &idx in &[i0, i1, i2] {
            if idx * 3 + 2 < normals.len() {
                normals[idx * 3] += n[0];
                normals[idx * 3 + 1] += n[1];
                normals[idx * 3 + 2] += n[2];
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

    normals
}
