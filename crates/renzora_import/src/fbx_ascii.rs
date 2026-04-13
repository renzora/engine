//! FBX ASCII (6.x) → GLB converter.
//!
//! Parses the text-based FBX format used by older exporters (FBX SDK ~2010,
//! some Mixamo downloads, etc.) and extracts geometry into GLB.

use std::path::Path;

use crate::convert::{ImportError, ImportResult};
use crate::obj::build_glb;
use crate::settings::{ImportSettings, UpAxis};

// ─── Node tree ──────────────────────────────────────────────────────────────

#[derive(Debug)]
struct FbxNode {
    name: String,
    properties: Vec<String>,
    children: Vec<FbxNode>,
}

// ─── Parser ─────────────────────────────────────────────────────────────────

struct AsciiParser<'a> {
    chars: &'a [u8],
    pos: usize,
}

impl<'a> AsciiParser<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { chars: data, pos: 0 }
    }

    fn peek(&self) -> Option<u8> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    fn skip_whitespace_and_comments(&mut self) {
        while self.pos < self.chars.len() {
            let ch = self.chars[self.pos];
            if ch == b';' {
                // Skip to end of line
                while self.pos < self.chars.len() && self.chars[self.pos] != b'\n' {
                    self.pos += 1;
                }
            } else if ch.is_ascii_whitespace() {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn parse_quoted_string(&mut self) -> String {
        // Skip opening quote
        self.advance();
        let start = self.pos;
        while self.pos < self.chars.len() && self.chars[self.pos] != b'"' {
            self.pos += 1;
        }
        let s = String::from_utf8_lossy(&self.chars[start..self.pos]).to_string();
        if self.peek() == Some(b'"') {
            self.advance();
        }
        s
    }

    fn parse_token(&mut self) -> String {
        let start = self.pos;
        while self.pos < self.chars.len() {
            let ch = self.chars[self.pos];
            if ch.is_ascii_whitespace() || ch == b',' || ch == b'{' || ch == b'}' || ch == b':' {
                break;
            }
            self.pos += 1;
        }
        String::from_utf8_lossy(&self.chars[start..self.pos]).to_string()
    }

    /// Parse the property list after a node name (up to `{` or newline).
    fn parse_property_list(&mut self) -> Vec<String> {
        let mut props = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            match self.peek() {
                None | Some(b'{') | Some(b'}') => break,
                Some(b'\n') | Some(b'\r') => {
                    // Properties end at newline if no brace follows on next lines
                    // Peek ahead to see if a '{' follows
                    let saved = self.pos;
                    self.skip_whitespace_and_comments();
                    if self.peek() == Some(b'{') {
                        break;
                    } else {
                        self.pos = saved;
                        break;
                    }
                }
                Some(b',') => {
                    self.advance();
                    continue;
                }
                Some(b'"') => {
                    props.push(self.parse_quoted_string());
                }
                _ => {
                    let tok = self.parse_token();
                    if !tok.is_empty() {
                        props.push(tok);
                    }
                }
            }
        }
        props
    }

    /// Parse a single node: `Name: prop, prop, ... { children }` or `Name: prop, prop, ...`
    fn parse_node(&mut self) -> Option<FbxNode> {
        self.skip_whitespace_and_comments();
        if self.pos >= self.chars.len() || self.peek() == Some(b'}') {
            return None;
        }

        let name = self.parse_token();
        if name.is_empty() {
            return None;
        }

        // Skip optional colon
        self.skip_whitespace_and_comments();
        if self.peek() == Some(b':') {
            self.advance();
        }

        let properties = self.parse_property_list();

        let mut children = Vec::new();

        self.skip_whitespace_and_comments();
        if self.peek() == Some(b'{') {
            self.advance(); // skip '{'
            loop {
                self.skip_whitespace_and_comments();
                if self.peek() == Some(b'}') {
                    self.advance();
                    break;
                }
                if self.pos >= self.chars.len() {
                    break;
                }
                if let Some(child) = self.parse_node() {
                    children.push(child);
                } else {
                    // Skip unknown character to avoid infinite loop
                    self.advance();
                }
            }
        }

        Some(FbxNode {
            name,
            properties,
            children,
        })
    }

    fn parse_document(&mut self) -> Vec<FbxNode> {
        let mut nodes = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            if self.pos >= self.chars.len() {
                break;
            }
            if let Some(node) = self.parse_node() {
                nodes.push(node);
            } else {
                self.advance();
            }
        }
        nodes
    }
}

// ─── Data extraction helpers ────────────────────────────────────────────────

fn find_node<'a>(nodes: &'a [FbxNode], name: &str) -> Option<&'a FbxNode> {
    for node in nodes {
        if node.name == name {
            return Some(node);
        }
    }
    None
}

fn find_node_recursive<'a>(nodes: &'a [FbxNode], name: &str) -> Option<&'a FbxNode> {
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

fn find_all_recursive<'a>(nodes: &'a [FbxNode], name: &str, out: &mut Vec<&'a FbxNode>) {
    for node in nodes {
        if node.name == name {
            out.push(node);
        }
        find_all_recursive(&node.children, name, out);
    }
}

/// Parse a node whose children are numeric values (the FBX ASCII array pattern).
/// In ASCII FBX, arrays look like:
/// ```text
/// Vertices: *N {
///     a: 1.0,2.0,3.0,4.0,...
/// }
/// ```
/// Or the older flat style:
/// ```text
/// Vertices: 1.0,2.0,3.0,...
/// ```
fn extract_f64_array(node: &FbxNode) -> Vec<f64> {
    let mut values = Vec::new();

    // Check if properties contain the values directly (flat style)
    for prop in &node.properties {
        // Skip the *N count marker
        if prop.starts_with('*') {
            continue;
        }
        if let Ok(v) = prop.parse::<f64>() {
            values.push(v);
        }
    }

    // Check children for the `a:` data node
    if let Some(a_node) = find_node(&node.children, "a") {
        for prop in &a_node.properties {
            if let Ok(v) = prop.parse::<f64>() {
                values.push(v);
            }
        }
    }

    values
}

fn extract_i32_array(node: &FbxNode) -> Vec<i32> {
    let mut values = Vec::new();

    for prop in &node.properties {
        if prop.starts_with('*') {
            continue;
        }
        if let Ok(v) = prop.parse::<i32>() {
            values.push(v);
        }
    }

    if let Some(a_node) = find_node(&node.children, "a") {
        for prop in &a_node.properties {
            if let Ok(v) = prop.parse::<i32>() {
                values.push(v);
            }
        }
    }

    values
}

fn extract_mapping_type(node: &FbxNode) -> Option<String> {
    find_node(&node.children, "MappingInformationType")
        .and_then(|n| n.properties.first())
        .cloned()
}

fn detect_up_axis(nodes: &[FbxNode]) -> Option<UpAxis> {
    let settings = find_node_recursive(nodes, "GlobalSettings")?;
    let props = find_node(&settings.children, "Properties60")
        .or_else(|| find_node(&settings.children, "Properties70"))?;

    for child in &props.children {
        if child.name == "Property" || child.name == "P" {
            if child.properties.first().map(|s| s.as_str()) == Some("UpAxis") {
                // The value is the last property
                if let Some(val) = child.properties.last().and_then(|s| s.parse::<i32>().ok()) {
                    return match val {
                        2 => Some(UpAxis::ZUp),
                        _ => Some(UpAxis::YUp),
                    };
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
    log::info!("[import] {}: parsing FBX ASCII format", file_name);

    let data = std::fs::read(path)?;
    log::info!("[import] {}: file size {} bytes", file_name, data.len());

    let mut parser = AsciiParser::new(&data);
    let nodes = parser.parse_document();

    if nodes.is_empty() {
        log::error!("[import] {}: failed to parse FBX ASCII — no nodes found", file_name);
        return Err(ImportError::ParseError("failed to parse FBX ASCII file".into()));
    }

    log::info!("[import] {}: parsed {} top-level nodes", file_name, nodes.len());

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

    // Find all Geometry nodes (they contain Vertices, PolygonVertexIndex, etc.)
    let mut geometry_nodes = Vec::new();
    find_all_recursive(&nodes, "Geometry", &mut geometry_nodes);

    // In FBX 6.x, geometry is directly inside Model nodes (no separate Geometry object)
    if geometry_nodes.is_empty() {
        log::info!("[import] {}: no Geometry nodes found, scanning Model nodes (FBX 6.x style)", file_name);
        let mut model_nodes = Vec::new();
        find_all_recursive(&nodes, "Model", &mut model_nodes);
        for model in model_nodes {
            // Check if this Model has Vertices child (i.e., it contains mesh data)
            if find_node(&model.children, "Vertices").is_some() {
                geometry_nodes.push(model);
            }
        }
    }

    log::info!("[import] {}: found {} geometry objects", file_name, geometry_nodes.len());

    for geo_node in &geometry_nodes {
        let raw_vertices = match find_node(&geo_node.children, "Vertices") {
            Some(n) => extract_f64_array(n),
            None => continue,
        };

        if raw_vertices.is_empty() {
            continue;
        }

        let raw_indices = match find_node(&geo_node.children, "PolygonVertexIndex") {
            Some(n) => extract_i32_array(n),
            None => {
                warnings.push("geometry has no PolygonVertexIndex".into());
                continue;
            }
        };

        // Normals
        let normal_layer = find_node(&geo_node.children, "LayerElementNormal");
        let raw_normals = normal_layer
            .and_then(|n| find_node(&n.children, "Normals"))
            .map(extract_f64_array)
            .unwrap_or_default();
        let normal_mapping = normal_layer.and_then(extract_mapping_type);

        // UVs
        let uv_layer = find_node(&geo_node.children, "LayerElementUV");
        let raw_uvs = uv_layer
            .and_then(|n| find_node(&n.children, "UV"))
            .map(extract_f64_array)
            .unwrap_or_default();
        let uv_indices = uv_layer
            .and_then(|n| find_node(&n.children, "UVIndex"))
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
                // Triangulate polygon using fan
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
        log::error!("[import] {}: no geometry found in FBX ASCII file", file_name);
        return Err(ImportError::ParseError(
            "no geometry found in FBX ASCII file".into(),
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

    let glb_bytes = build_glb(&all_positions, &all_normals, &all_texcoords, &all_indices, &crate::obj::MaterialBundle::default())?;

    log::info!("[import] {}: GLB output {} bytes", file_name, glb_bytes.len());

    Ok(ImportResult {
        glb_bytes,
        warnings, extracted_textures: Vec::new(), extracted_materials: Vec::new(),
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
