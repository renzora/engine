//! FBX binary file parser and GLB converter using the `fbxcel-dom` crate.
//!
//! Uses the low-level tree API for maximum compatibility with different FBX versions.

use std::path::Path;

use super::glb_builder::{GlbBuilder, MaterialData, MeshData};
use fbxcel::low::v7400::AttributeValue;
use fbxcel::tree::v7400::NodeHandle;

/// Parse a binary FBX file and feed geometry/materials into a GlbBuilder.
pub fn convert_fbx(path: &Path, builder: &mut GlbBuilder) -> Result<(), String> {
    let file =
        std::fs::File::open(path).map_err(|e| format!("Failed to open FBX file: {}", e))?;
    let reader = std::io::BufReader::new(file);

    let any_doc = fbxcel_dom::any::AnyDocument::from_seekable_reader(reader)
        .map_err(|e| format!("Failed to parse FBX: {}", e))?;

    match any_doc {
        fbxcel_dom::any::AnyDocument::V7400(_header, doc) => {
            parse_fbx_v7400(&doc, builder)?;
        }
        _ => {
            return Err("Only FBX version 7.4+ (binary) is supported".to_string());
        }
    }

    Ok(())
}

fn parse_fbx_v7400(
    doc: &fbxcel_dom::v7400::Document,
    builder: &mut GlbBuilder,
) -> Result<(), String> {
    // Default to Z-up for FBX files (most DCC tools export Z-up)
    // Apply Z-up to Y-up rotation
    builder.set_root_rotation([
        -std::f32::consts::FRAC_1_SQRT_2,
        0.0,
        0.0,
        std::f32::consts::FRAC_1_SQRT_2,
    ]);

    // Use the tree API to extract geometry and materials
    let tree = doc.tree();

    let objects_node = tree
        .root()
        .children()
        .find(|n| n.name() == "Objects");

    let objects_node = match objects_node {
        Some(n) => n,
        None => return Err("No Objects node found in FBX".to_string()),
    };

    let mut mesh_count = 0;

    for child in objects_node.children() {
        match child.name() {
            "Geometry" => {
                if let Some(mesh_data) = parse_geometry_node(&child) {
                    builder.add_mesh(mesh_data);
                    mesh_count += 1;
                }
            }
            "Material" => {
                parse_material_node(&child, builder);
            }
            _ => {}
        }
    }

    if mesh_count == 0 {
        return Err("No mesh geometry found in FBX file".to_string());
    }

    Ok(())
}

fn parse_material_node(
    node: &NodeHandle,
    builder: &mut GlbBuilder,
) {
    let attrs = node.attributes();
    let name = attrs.get(1).and_then(get_string).map(|s| clean_fbx_name(&s));

    let mut base_color = [0.8f32, 0.8, 0.8, 1.0];

    for prop_child in node.children() {
        if prop_child.name() == "Properties70" {
            for p in prop_child.children() {
                if p.name() == "P" {
                    let p_attrs = p.attributes();
                    if let Some(prop_name) = p_attrs.get(0).and_then(get_string) {
                        if prop_name == "DiffuseColor" && p_attrs.len() >= 7 {
                            let r = get_f64(&p_attrs[4]).unwrap_or(0.8) as f32;
                            let g = get_f64(&p_attrs[5]).unwrap_or(0.8) as f32;
                            let b = get_f64(&p_attrs[6]).unwrap_or(0.8) as f32;
                            base_color = [r, g, b, 1.0];
                        }
                    }
                }
            }
        }
    }

    builder.add_material(MaterialData {
        name,
        base_color,
        metallic: 0.0,
        roughness: 0.8,
        base_color_texture_index: None,
    });
}

fn parse_geometry_node(
    node: &NodeHandle,
) -> Option<MeshData> {
    let node_attrs = node.attributes();
    let name = node_attrs.get(1).and_then(get_string).map(|s| clean_fbx_name(&s));

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals_raw: Vec<[f32; 3]> = Vec::new();
    let mut uvs_raw: Vec<[f32; 2]> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();

    for child in node.children() {
        match child.name() {
            "Vertices" => {
                if let Some(data) = child.attributes().get(0).and_then(get_f64_array) {
                    for chunk in data.chunks(3) {
                        if chunk.len() == 3 {
                            positions.push([chunk[0] as f32, chunk[1] as f32, chunk[2] as f32]);
                        }
                    }
                }
            }
            "PolygonVertexIndex" => {
                if let Some(data) = child.attributes().get(0).and_then(get_i32_array) {
                    // FBX polygon indices: negative values mark end of polygon (bitwise NOT)
                    let mut polygon_verts: Vec<i32> = Vec::new();

                    for &idx in &data {
                        let actual_idx = if idx < 0 { !idx } else { idx };
                        polygon_verts.push(actual_idx);

                        if idx < 0 {
                            // End of polygon - fan triangulate
                            if polygon_verts.len() >= 3 {
                                for i in 1..polygon_verts.len() - 1 {
                                    indices.push(polygon_verts[0] as u32);
                                    indices.push(polygon_verts[i] as u32);
                                    indices.push(polygon_verts[i + 1] as u32);
                                }
                            }
                            polygon_verts.clear();
                        }
                    }
                }
            }
            "LayerElementNormal" => {
                for sub in child.children() {
                    if sub.name() == "Normals" {
                        if let Some(data) = sub.attributes().get(0).and_then(get_f64_array) {
                            for chunk in data.chunks(3) {
                                if chunk.len() == 3 {
                                    normals_raw.push([chunk[0] as f32, chunk[1] as f32, chunk[2] as f32]);
                                }
                            }
                        }
                    }
                }
            }
            "LayerElementUV" => {
                for sub in child.children() {
                    if sub.name() == "UV" {
                        if let Some(data) = sub.attributes().get(0).and_then(get_f64_array) {
                            for chunk in data.chunks(2) {
                                if chunk.len() == 2 {
                                    uvs_raw.push([chunk[0] as f32, 1.0 - chunk[1] as f32]);
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if positions.is_empty() {
        return None;
    }

    // Normals: only use if they map 1:1 to positions (ByControlPoint)
    let normals = if normals_raw.len() == positions.len() {
        Some(normals_raw)
    } else {
        None
    };

    // UVs: only use if they map 1:1 to positions (ByControlPoint)
    let tex_coords = if uvs_raw.len() == positions.len() {
        Some(uvs_raw)
    } else {
        None
    };

    Some(MeshData {
        name,
        positions,
        normals,
        tex_coords,
        indices: if indices.is_empty() { None } else { Some(indices) },
        material_index: None,
    })
}

/// Clean FBX name strings (remove null bytes, "Model::" prefixes, etc.)
fn clean_fbx_name(s: &str) -> String {
    s.split('\0')
        .next()
        .unwrap_or(s)
        .trim_start_matches("Model::")
        .trim_start_matches("Geometry::")
        .trim_start_matches("Material::")
        .to_string()
}

// Helper functions for extracting typed values from FBX attributes

fn get_string(attr: &AttributeValue) -> Option<String> {
    match attr {
        AttributeValue::String(s) => Some(s.to_string()),
        _ => None,
    }
}

fn get_f64(attr: &AttributeValue) -> Option<f64> {
    match attr {
        AttributeValue::F64(v) => Some(*v),
        AttributeValue::F32(v) => Some(*v as f64),
        AttributeValue::I64(v) => Some(*v as f64),
        AttributeValue::I32(v) => Some(*v as f64),
        AttributeValue::I16(v) => Some(*v as f64),
        _ => None,
    }
}

fn get_f64_array(attr: &AttributeValue) -> Option<Vec<f64>> {
    match attr {
        AttributeValue::ArrF64(arr) => Some(arr.iter().copied().collect()),
        AttributeValue::ArrF32(arr) => Some(arr.iter().map(|&v| v as f64).collect()),
        _ => None,
    }
}

fn get_i32_array(attr: &AttributeValue) -> Option<Vec<i32>> {
    match attr {
        AttributeValue::ArrI32(arr) => Some(arr.iter().copied().collect()),
        AttributeValue::ArrI64(arr) => Some(arr.iter().map(|&v| v as i32).collect()),
        _ => None,
    }
}
