//! FBX → GLB converter using fbxcel-dom (pure Rust).

use std::path::Path;

use fbxcel::low::v7400::AttributeValue;

use crate::convert::{ImportError, ImportResult};
use crate::obj::build_glb;
use crate::settings::{ImportSettings, UpAxis};

/// FBX binary magic header.
const FBX_BINARY_MAGIC: &[u8; 23] = b"Kaydara FBX Binary  \x00\x1a\x00";

/// Returns `true` if the file starts with the FBX binary magic bytes.
fn is_binary_fbx(path: &Path) -> std::io::Result<bool> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut buf = [0u8; 23];
    let n = file.read(&mut buf)?;
    Ok(n == 23 && buf == *FBX_BINARY_MAGIC)
}

pub fn convert(path: &Path, settings: &ImportSettings) -> Result<ImportResult, ImportError> {
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");

    // Detect ASCII vs Binary and dispatch
    if !is_binary_fbx(path)? {
        log::info!("[import] {}: detected FBX ASCII format, using ASCII parser", file_name);
        return crate::fbx_ascii::convert(path, settings);
    }

    log::info!("[import] {}: detected FBX Binary format", file_name);

    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);

    let doc = match fbxcel_dom::any::AnyDocument::from_seekable_reader(reader) {
        Ok(fbxcel_dom::any::AnyDocument::V7400(ver, doc)) => {
            log::info!("[import] {}: FBX version {:?}", file_name, ver);
            doc
        }
        Ok(_) => {
            log::error!("[import] {}: unsupported FBX binary version", file_name);
            return Err(ImportError::ParseError(
                "unsupported FBX version (only FBX 7.4+ binary is supported)".into(),
            ))
        }
        Err(e) => {
            log::error!("[import] {}: FBX parse error: {}", file_name, e);
            return Err(ImportError::ParseError(format!("FBX parse error: {}", e)))
        }
    };

    let mut all_positions = Vec::new();
    let mut all_normals = Vec::new();
    let mut all_texcoords = Vec::new();
    let mut all_indices = Vec::new();
    let mut warnings = Vec::new();

    // Determine global up axis from FBX settings
    let effective_up_axis = if settings.up_axis == UpAxis::Auto {
        detect_fbx_up_axis(&doc).unwrap_or(UpAxis::YUp)
    } else {
        settings.up_axis
    };

    // Iterate over all objects looking for Geometry nodes
    for obj in doc.objects() {
        if obj.class() != "Geometry" || obj.subclass() != "Mesh" {
            continue;
        }

        let node = obj.node();

        // Extract vertices
        let raw_vertices: Vec<f64> = match find_child_array_f64(&node, "Vertices") {
            Some(v) => v,
            None => {
                warnings.push(format!("geometry object {:?} has no Vertices", obj.object_id()));
                continue;
            }
        };

        if raw_vertices.is_empty() {
            continue;
        }

        // Extract polygon indices
        let raw_indices: Vec<i32> = match find_child_array_i32(&node, "PolygonVertexIndex") {
            Some(v) => v,
            None => {
                warnings.push(format!(
                    "geometry object {:?} has no PolygonVertexIndex",
                    obj.object_id()
                ));
                continue;
            }
        };

        // Extract normals (if present)
        let raw_normals = extract_layer_element_data(&node, "LayerElementNormal", "Normals");
        let normal_mapping = extract_mapping_type(&node, "LayerElementNormal");

        // Extract UVs (if present)
        let raw_uvs = extract_layer_element_data(&node, "LayerElementUV", "UV");
        let uv_indices = extract_layer_element_indices(&node, "LayerElementUV", "UVIndex");
        let uv_mapping = extract_mapping_type(&node, "LayerElementUV");

        let base_vertex = (all_positions.len() / 3) as u32;
        let vertex_count = raw_vertices.len() / 3;

        // Add all positions
        for i in 0..vertex_count {
            let (mut x, mut y, mut z) = (
                raw_vertices[i * 3] as f32 * settings.scale,
                raw_vertices[i * 3 + 1] as f32 * settings.scale,
                raw_vertices[i * 3 + 2] as f32 * settings.scale,
            );

            convert_axis(&mut x, &mut y, &mut z, effective_up_axis);
            all_positions.extend_from_slice(&[x, y, z]);
        }

        // Initialize normals and UVs for this geometry
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
                // End of polygon — triangulate using fan
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
            generate_normals_inplace(&all_positions, &all_indices, base_vertex, vertex_count, &mut geo_normals);
        }

        all_normals.extend_from_slice(&geo_normals);
        all_texcoords.extend_from_slice(&geo_texcoords);
    }

    if all_positions.is_empty() {
        log::error!("[import] {}: no geometry found in FBX binary file", file_name);
        return Err(ImportError::ParseError(
            "no geometry found in FBX file".into(),
        ));
    }

    let vertex_count = all_positions.len() / 3;
    let tri_count = all_indices.len() / 3;
    log::info!(
        "[import] {}: {} vertices, {} triangles, {} warnings",
        file_name, vertex_count, tri_count, warnings.len()
    );
    for w in &warnings {
        log::warn!("[import] {}: {}", file_name, w);
    }

    let glb_bytes = build_glb(&all_positions, &all_normals, &all_texcoords, &all_indices)?;

    log::info!("[import] {}: GLB output {} bytes", file_name, glb_bytes.len());

    Ok(ImportResult {
        glb_bytes,
        warnings,
    })
}

fn decode_fbx_index(raw: i32) -> u32 {
    if raw < 0 { (-raw - 1) as u32 } else { raw as u32 }
}

fn convert_axis(_x: &mut f32, y: &mut f32, z: &mut f32, up_axis: UpAxis) {
    if up_axis == UpAxis::ZUp {
        let tmp = *y;
        *y = *z;
        *z = -tmp;
    }
}

fn generate_normals_inplace(
    positions: &[f32],
    indices: &[u32],
    base_vertex: u32,
    vertex_count: usize,
    normals: &mut [f32],
) {
    for tri in indices.chunks(3) {
        if tri.len() < 3 { break; }
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

// ─── FBX tree helpers ──────────────────────────────────────────────────────

use fbxcel_dom::v7400::Document;

fn detect_fbx_up_axis(doc: &Document) -> Option<UpAxis> {
    for obj in doc.objects() {
        if obj.class() != "GlobalSettings" {
            continue;
        }
        for child in obj.node().children() {
            if child.name() != "Properties70" {
                continue;
            }
            for prop in child.children() {
                if prop.name() != "P" {
                    continue;
                }
                let attrs = prop.attributes();
                if let Some(name) = attrs.first().and_then(|a| a.get_string()) {
                    if name == "UpAxis" {
                        if let Some(val) = attrs.get(4).and_then(|a| a.get_i32()) {
                            return match val {
                                2 => Some(UpAxis::ZUp),
                                _ => Some(UpAxis::YUp),
                            };
                        }
                    }
                }
            }
        }
    }
    None
}

fn find_child_array_f64(
    node: &fbxcel::tree::v7400::NodeHandle<'_>,
    name: &str,
) -> Option<Vec<f64>> {
    let child = node.children().find(|n| n.name() == name)?;
    let attrs = child.attributes();
    let first = attrs.first()?;
    extract_f64_from_attr(first)
}

fn find_child_array_i32(
    node: &fbxcel::tree::v7400::NodeHandle<'_>,
    name: &str,
) -> Option<Vec<i32>> {
    let child = node.children().find(|n| n.name() == name)?;
    let attrs = child.attributes();
    let first = attrs.first()?;
    extract_i32_from_attr(first)
}

fn extract_layer_element_data(
    node: &fbxcel::tree::v7400::NodeHandle<'_>,
    layer_name: &str,
    data_name: &str,
) -> Vec<f64> {
    node.children()
        .find(|n| n.name() == layer_name)
        .and_then(|layer| {
            layer
                .children()
                .find(|n| n.name() == data_name)
                .and_then(|n| n.attributes().first())
                .and_then(extract_f64_from_attr)
        })
        .unwrap_or_default()
}

fn extract_layer_element_indices(
    node: &fbxcel::tree::v7400::NodeHandle<'_>,
    layer_name: &str,
    index_name: &str,
) -> Vec<i32> {
    node.children()
        .find(|n| n.name() == layer_name)
        .and_then(|layer| {
            layer
                .children()
                .find(|n| n.name() == index_name)
                .and_then(|n| n.attributes().first())
                .and_then(extract_i32_from_attr)
        })
        .unwrap_or_default()
}

fn extract_mapping_type(
    node: &fbxcel::tree::v7400::NodeHandle<'_>,
    layer_name: &str,
) -> Option<String> {
    node.children()
        .find(|n| n.name() == layer_name)
        .and_then(|layer| {
            layer
                .children()
                .find(|n| n.name() == "MappingInformationType")
                .and_then(|n| n.attributes().first())
                .and_then(|a| a.get_string().map(|s| s.to_string()))
        })
}

fn extract_f64_from_attr(attr: &AttributeValue) -> Option<Vec<f64>> {
    match attr {
        AttributeValue::ArrF64(arr) => Some(arr.clone()),
        AttributeValue::ArrF32(arr) => Some(arr.iter().map(|&v| v as f64).collect()),
        _ => None,
    }
}

fn extract_i32_from_attr(attr: &AttributeValue) -> Option<Vec<i32>> {
    match attr {
        AttributeValue::ArrI32(arr) => Some(arr.clone()),
        AttributeValue::ArrI64(arr) => Some(arr.iter().map(|&v| v as i32).collect()),
        _ => None,
    }
}
