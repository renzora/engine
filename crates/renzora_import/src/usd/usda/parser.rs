#![allow(unused_variables)]

//! USDA recursive descent parser.
//!
//! Parses tokenized USDA into a UsdStage by walking the token stream
//! and building prims, properties, and relationships.

use super::tokenizer::Token;
use super::super::scene::*;
use super::super::UsdResult;

/// Parse a tokenized USDA file into a UsdStage.
pub fn parse_stage(tokens: &[Token], _raw: &str) -> UsdResult<UsdStage> {
    let mut stage = UsdStage {
        meters_per_unit: 0.01,
        time_codes_per_second: 24.0,
        ..Default::default()
    };

    let mut pos = 0;

    // Parse stage-level metadata (the initial `#usda 1.0` header and parens block)
    parse_stage_metadata(tokens, &mut pos, &mut stage);

    // Parse top-level prims
    while pos < tokens.len() {
        if let Some(node) = parse_prim(tokens, &mut pos, "", &mut stage) {
            stage.root.children.push(node);
        } else {
            pos += 1; // skip unrecognized token
        }
    }

    // Resolve material bindings
    resolve_material_bindings(&mut stage);

    Ok(stage)
}

fn parse_stage_metadata(tokens: &[Token], pos: &mut usize, stage: &mut UsdStage) {
    // Skip #usda 1.0 (already handled by tokenizer as identifiers/numbers)
    // Look for opening paren block with metadata
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::OpenParen => {
                *pos += 1;
                parse_metadata_block(tokens, pos, stage);
                break;
            }
            Token::Keyword(k) if k == "def" || k == "over" || k == "class" => break,
            _ => *pos += 1,
        }
    }
}

fn parse_metadata_block(tokens: &[Token], pos: &mut usize, stage: &mut UsdStage) {
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::CloseParen => {
                *pos += 1;
                return;
            }
            Token::Identifier(name) => {
                let key = name.clone();
                *pos += 1;

                // Expect =
                if *pos < tokens.len() && tokens[*pos] == Token::Equals {
                    *pos += 1;
                }

                match key.as_str() {
                    "upAxis" => {
                        if let Some(Token::QuotedString(v)) = tokens.get(*pos) {
                            stage.up_axis = if v == "Z" { UpAxis::ZUp } else { UpAxis::YUp };
                            *pos += 1;
                        }
                    }
                    "metersPerUnit" => {
                        if let Some(Token::Number(v)) = tokens.get(*pos) {
                            stage.meters_per_unit = *v as f32;
                            *pos += 1;
                        }
                    }
                    "timeCodesPerSecond" => {
                        if let Some(Token::Number(v)) = tokens.get(*pos) {
                            stage.time_codes_per_second = *v as f32;
                            *pos += 1;
                        }
                    }
                    _ => {
                        // Skip unknown metadata value
                        skip_value(tokens, pos);
                    }
                }
            }
            _ => *pos += 1,
        }
    }
}

fn parse_prim(
    tokens: &[Token],
    pos: &mut usize,
    parent_path: &str,
    stage: &mut UsdStage,
) -> Option<UsdNode> {
    // Expect: def TypeName "Name" { ... }
    let specifier = match tokens.get(*pos) {
        Some(Token::Keyword(k)) if k == "def" || k == "over" || k == "class" => {
            *pos += 1;
            k.clone()
        }
        _ => return None,
    };

    // Type name (optional for "over")
    let type_name = match tokens.get(*pos) {
        Some(Token::Identifier(t)) => {
            let name = t.clone();
            *pos += 1;
            name
        }
        _ => String::new(),
    };

    // Prim name (quoted string)
    let prim_name = match tokens.get(*pos) {
        Some(Token::QuotedString(n)) => {
            let name = n.clone();
            *pos += 1;
            name
        }
        _ => format!("unnamed_{}", pos),
    };

    let full_path = if parent_path.is_empty() {
        format!("/{}", prim_name)
    } else {
        format!("{}/{}", parent_path, prim_name)
    };

    // Skip optional metadata in parens
    if matches!(tokens.get(*pos), Some(Token::OpenParen)) {
        *pos += 1;
        skip_until_close_paren(tokens, pos);
    }

    // Expect opening brace
    if !matches!(tokens.get(*pos), Some(Token::OpenBrace)) {
        return None;
    }
    *pos += 1;

    let mut node = UsdNode {
        name: prim_name,
        path: full_path.clone(),
        transform: super::super::xform::identity(),
        data: NodeData::Empty,
        children: Vec::new(),
    };

    // Parse prim body
    let mut mesh_data: Option<MeshBuilder> = None;
    let mut material_data: Option<UsdMaterial> = None;
    let mut skeleton_data: Option<UsdSkeleton> = None;
    let mut light_data: Option<UsdLight> = None;
    let mut camera_data: Option<UsdCamera> = None;

    match type_name.as_str() {
        "Mesh" => mesh_data = Some(MeshBuilder::new(&full_path)),
        "Material" => {
            material_data = Some(UsdMaterial {
                name: node.name.clone(),
                path: full_path.clone(),
                ..Default::default()
            });
        }
        "Skeleton" | "SkelRoot" => {
            skeleton_data = Some(UsdSkeleton {
                name: node.name.clone(),
                path: full_path.clone(),
                ..Default::default()
            });
        }
        "DistantLight" | "SphereLight" | "RectLight" | "DiskLight" | "DomeLight" => {
            light_data = Some(UsdLight {
                name: node.name.clone(),
                path: full_path.clone(),
                kind: match type_name.as_str() {
                    "DistantLight" => LightKind::Distant { angle: 0.53 },
                    "RectLight" => LightKind::Rect { width: 1.0, height: 1.0 },
                    "DiskLight" => LightKind::Disk { radius: 1.0 },
                    "DomeLight" => LightKind::Dome { texture_path: None },
                    _ => LightKind::Sphere { radius: 1.0 },
                },
                color: [1.0; 3],
                intensity: 1.0,
            });
        }
        "Camera" => {
            camera_data = Some(UsdCamera {
                name: node.name.clone(),
                path: full_path.clone(),
                ..Default::default()
            });
        }
        _ => {}
    }

    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::CloseBrace => {
                *pos += 1;
                break;
            }
            // Child prim
            Token::Keyword(k) if k == "def" || k == "over" || k == "class" => {
                if let Some(child) = parse_prim(tokens, pos, &full_path, stage) {
                    // Check if this child is a GeomSubset or Shader
                    node.children.push(child);
                }
            }
            // Property
            Token::Identifier(_)
            | Token::Keyword(_)
                if matches!(&tokens[*pos], Token::Keyword(k) if k == "custom" || k == "uniform")
                || matches!(&tokens[*pos], Token::Identifier(_)) =>
            {
                parse_property(
                    tokens,
                    pos,
                    &full_path,
                    &mut mesh_data,
                    &mut material_data,
                    &mut skeleton_data,
                    &mut light_data,
                    &mut camera_data,
                    &mut node,
                );
            }
            _ => *pos += 1,
        }
    }

    // Finalize and add data to stage
    if let Some(builder) = mesh_data {
        let mesh = builder.build();
        let idx = stage.meshes.len();
        stage.meshes.push(mesh);
        node.data = NodeData::Mesh(idx);
    }
    if let Some(mat) = material_data {
        stage.materials.push(mat);
    }
    if let Some(skel) = skeleton_data {
        let idx = stage.skeletons.len();
        stage.skeletons.push(skel);
        node.data = NodeData::Skeleton(idx);
    }
    if let Some(light) = light_data {
        let idx = stage.lights.len();
        stage.lights.push(light);
        node.data = NodeData::Light(idx);
    }
    if let Some(cam) = camera_data {
        let idx = stage.cameras.len();
        stage.cameras.push(cam);
        node.data = NodeData::Camera(idx);
    }

    Some(node)
}

fn parse_property(
    tokens: &[Token],
    pos: &mut usize,
    prim_path: &str,
    mesh: &mut Option<MeshBuilder>,
    material: &mut Option<UsdMaterial>,
    skeleton: &mut Option<UsdSkeleton>,
    light: &mut Option<UsdLight>,
    camera: &mut Option<UsdCamera>,
    node: &mut UsdNode,
) {
    // Skip qualifiers like "custom", "uniform"
    while matches!(tokens.get(*pos), Some(Token::Keyword(k)) if k == "custom" || k == "uniform") {
        *pos += 1;
    }

    // Type name (e.g. "point3f[]", "float", "rel")
    let type_name = match tokens.get(*pos) {
        Some(Token::Identifier(t)) => {
            let name = t.clone();
            *pos += 1;
            name
        }
        _ => {
            *pos += 1;
            return;
        }
    };

    // Check for relationship
    if type_name == "rel" {
        parse_relationship(tokens, pos, prim_path, mesh, material);
        return;
    }

    // Property name (may include namespaces with colons, already combined by tokenizer)
    let prop_name = match tokens.get(*pos) {
        Some(Token::Identifier(n)) => {
            let name = n.clone();
            *pos += 1;
            name
        }
        _ => {
            skip_to_next_property(tokens, pos);
            return;
        }
    };

    // Skip optional metadata in parens
    if matches!(tokens.get(*pos), Some(Token::OpenParen)) {
        *pos += 1;
        skip_until_close_paren(tokens, pos);
    }

    // Expect = or skip if no assignment
    if !matches!(tokens.get(*pos), Some(Token::Equals)) {
        return;
    }
    *pos += 1;

    // Parse the value
    match prop_name.as_str() {
        // --- Mesh properties ---
        "points" if mesh.is_some() => {
            let vals = parse_vec3_array(tokens, pos);
            if let Some(m) = mesh { m.positions = vals; }
        }
        "normals" if mesh.is_some() => {
            let vals = parse_vec3_array(tokens, pos);
            if let Some(m) = mesh { m.normals = vals; }
        }
        "faceVertexCounts" if mesh.is_some() => {
            let vals = parse_int_array(tokens, pos);
            if let Some(m) = mesh { m.face_vertex_counts = vals.iter().map(|&i| i as u32).collect(); }
        }
        "faceVertexIndices" if mesh.is_some() => {
            let vals = parse_int_array(tokens, pos);
            if let Some(m) = mesh { m.face_vertex_indices = vals.iter().map(|&i| i as u32).collect(); }
        }
        "primvars:st" | "primvars:st0" | "primvars:st1" if mesh.is_some() => {
            let vals = parse_vec2_array(tokens, pos);
            let uv_name = if prop_name == "primvars:st" { "st" } else { &prop_name[8..] };
            if let Some(m) = mesh { m.uv_sets.insert(uv_name.to_string(), vals); }
        }
        "primvars:displayColor" if mesh.is_some() => {
            let vals = parse_vec3_array(tokens, pos);
            if let Some(m) = mesh {
                m.colors = vals.iter().map(|c| [c[0], c[1], c[2], 1.0]).collect();
            }
        }
        "subdivisionScheme" if mesh.is_some() => {
            if let Some(Token::QuotedString(v)) = tokens.get(*pos) {
                if let Some(m) = mesh { m.subdivision_scheme = v.clone(); }
                *pos += 1;
            } else {
                skip_value(tokens, pos);
            }
        }

        // --- Material / shader properties ---
        "inputs:diffuseColor" if material.is_some() => {
            if let Some(c) = parse_vec3_value(tokens, pos) {
                if let Some(m) = material { m.diffuse_color = c; }
            } else { skip_value(tokens, pos); }
        }
        "inputs:emissiveColor" if material.is_some() => {
            if let Some(c) = parse_vec3_value(tokens, pos) {
                if let Some(m) = material { m.emissive_color = c; }
            } else { skip_value(tokens, pos); }
        }
        "inputs:metallic" if material.is_some() => {
            if let Some(Token::Number(v)) = tokens.get(*pos) {
                if let Some(m) = material { m.metallic = *v as f32; }
                *pos += 1;
            } else { skip_value(tokens, pos); }
        }
        "inputs:roughness" if material.is_some() => {
            if let Some(Token::Number(v)) = tokens.get(*pos) {
                if let Some(m) = material { m.roughness = *v as f32; }
                *pos += 1;
            } else { skip_value(tokens, pos); }
        }
        "inputs:opacity" if material.is_some() => {
            if let Some(Token::Number(v)) = tokens.get(*pos) {
                if let Some(m) = material { m.opacity = *v as f32; }
                *pos += 1;
            } else { skip_value(tokens, pos); }
        }
        "inputs:ior" if material.is_some() => {
            if let Some(Token::Number(v)) = tokens.get(*pos) {
                if let Some(m) = material { m.ior = *v as f32; }
                *pos += 1;
            } else { skip_value(tokens, pos); }
        }

        // --- Skeleton properties ---
        "joints" if skeleton.is_some() => {
            let vals = parse_string_array(tokens, pos);
            if let Some(s) = skeleton {
                s.parent_indices = compute_parent_indices(&vals);
                s.joints = vals;
            }
        }
        "bindTransforms" if skeleton.is_some() => {
            let vals = parse_matrix4_array(tokens, pos);
            if let Some(s) = skeleton { s.bind_transforms = vals; }
        }
        "restTransforms" if skeleton.is_some() => {
            let vals = parse_matrix4_array(tokens, pos);
            if let Some(s) = skeleton { s.rest_transforms = vals; }
        }

        // --- Light properties ---
        "inputs:color" | "color" if light.is_some() => {
            if let Some(c) = parse_vec3_value(tokens, pos) {
                if let Some(l) = light { l.color = c; }
            } else { skip_value(tokens, pos); }
        }
        "inputs:intensity" | "intensity" if light.is_some() => {
            if let Some(Token::Number(v)) = tokens.get(*pos) {
                if let Some(l) = light { l.intensity = *v as f32; }
                *pos += 1;
            } else { skip_value(tokens, pos); }
        }

        // --- Camera properties ---
        "focalLength" if camera.is_some() => {
            if let Some(Token::Number(v)) = tokens.get(*pos) {
                if let Some(c) = camera {
                    if let Projection::Perspective { focal_length, .. } = &mut c.projection {
                        *focal_length = *v as f32;
                    }
                }
                *pos += 1;
            } else { skip_value(tokens, pos); }
        }
        "clippingRange" if camera.is_some() => {
            if let Some(v) = parse_vec2_value(tokens, pos) {
                if let Some(c) = camera {
                    c.near_clip = v[0];
                    c.far_clip = v[1];
                }
            } else { skip_value(tokens, pos); }
        }

        // --- Transform ---
        "xformOp:transform" => {
            if let Some(m) = parse_matrix4_value(tokens, pos) {
                node.transform = m;
            } else {
                skip_value(tokens, pos);
            }
        }

        // Unknown property — skip value
        _ => {
            skip_value(tokens, pos);
        }
    }
}

fn parse_relationship(
    tokens: &[Token],
    pos: &mut usize,
    prim_path: &str,
    mesh: &mut Option<MeshBuilder>,
    material: &mut Option<UsdMaterial>,
) {
    // rel material:binding = </Path/To/Material>
    let rel_name = match tokens.get(*pos) {
        Some(Token::Identifier(n)) => {
            let name = n.clone();
            *pos += 1;
            name
        }
        _ => {
            skip_to_next_property(tokens, pos);
            return;
        }
    };

    if !matches!(tokens.get(*pos), Some(Token::Equals)) {
        return;
    }
    *pos += 1;

    if rel_name == "material:binding" {
        if let Some(Token::PathRef(path)) = tokens.get(*pos) {
            if let Some(m) = mesh {
                m.material_binding = Some(path.clone());
            }
            *pos += 1;
        } else {
            skip_value(tokens, pos);
        }
    } else {
        skip_value(tokens, pos);
    }
}

// ---------------------------------------------------------------------------
// Value parsers
// ---------------------------------------------------------------------------

fn parse_vec3_array(tokens: &[Token], pos: &mut usize) -> Vec<[f32; 3]> {
    let mut result = Vec::new();
    if !matches!(tokens.get(*pos), Some(Token::OpenBracket)) {
        skip_value(tokens, pos);
        return result;
    }
    *pos += 1;

    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::CloseBracket => { *pos += 1; break; }
            Token::OpenParen => {
                *pos += 1;
                if let Some(v) = read_3_numbers(tokens, pos) {
                    result.push(v);
                }
                // Skip close paren
                if matches!(tokens.get(*pos), Some(Token::CloseParen)) { *pos += 1; }
            }
            Token::Comma => *pos += 1,
            _ => *pos += 1,
        }
    }

    result
}

fn parse_vec2_array(tokens: &[Token], pos: &mut usize) -> Vec<[f32; 2]> {
    let mut result = Vec::new();
    if !matches!(tokens.get(*pos), Some(Token::OpenBracket)) {
        skip_value(tokens, pos);
        return result;
    }
    *pos += 1;

    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::CloseBracket => { *pos += 1; break; }
            Token::OpenParen => {
                *pos += 1;
                if let Some(v) = read_2_numbers(tokens, pos) {
                    result.push(v);
                }
                if matches!(tokens.get(*pos), Some(Token::CloseParen)) { *pos += 1; }
            }
            Token::Comma => *pos += 1,
            _ => *pos += 1,
        }
    }

    result
}

fn parse_int_array(tokens: &[Token], pos: &mut usize) -> Vec<i32> {
    let mut result = Vec::new();
    if !matches!(tokens.get(*pos), Some(Token::OpenBracket)) {
        skip_value(tokens, pos);
        return result;
    }
    *pos += 1;

    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::CloseBracket => { *pos += 1; break; }
            Token::Number(n) => {
                result.push(*n as i32);
                *pos += 1;
            }
            Token::Comma => *pos += 1,
            _ => *pos += 1,
        }
    }

    result
}

fn parse_string_array(tokens: &[Token], pos: &mut usize) -> Vec<String> {
    let mut result = Vec::new();
    if !matches!(tokens.get(*pos), Some(Token::OpenBracket)) {
        skip_value(tokens, pos);
        return result;
    }
    *pos += 1;

    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::CloseBracket => { *pos += 1; break; }
            Token::QuotedString(s) => {
                result.push(s.clone());
                *pos += 1;
            }
            Token::Comma => *pos += 1,
            _ => *pos += 1,
        }
    }

    result
}

fn parse_vec3_value(tokens: &[Token], pos: &mut usize) -> Option<[f32; 3]> {
    if !matches!(tokens.get(*pos), Some(Token::OpenParen)) {
        return None;
    }
    *pos += 1;
    let v = read_3_numbers(tokens, pos)?;
    if matches!(tokens.get(*pos), Some(Token::CloseParen)) { *pos += 1; }
    Some(v)
}

fn parse_vec2_value(tokens: &[Token], pos: &mut usize) -> Option<[f32; 2]> {
    if !matches!(tokens.get(*pos), Some(Token::OpenParen)) {
        return None;
    }
    *pos += 1;
    let v = read_2_numbers(tokens, pos)?;
    if matches!(tokens.get(*pos), Some(Token::CloseParen)) { *pos += 1; }
    Some(v)
}

fn parse_matrix4_value(tokens: &[Token], pos: &mut usize) -> Option<[f32; 16]> {
    // Matrix is written as (( r0 ), ( r1 ), ( r2 ), ( r3 ))
    if !matches!(tokens.get(*pos), Some(Token::OpenParen)) {
        return None;
    }
    *pos += 1;

    let mut m = [0.0f32; 16];
    for row in 0..4 {
        if !matches!(tokens.get(*pos), Some(Token::OpenParen)) {
            skip_until_close_paren(tokens, pos);
            return None;
        }
        *pos += 1;

        for col in 0..4 {
            if let Some(Token::Number(n)) = tokens.get(*pos) {
                m[row * 4 + col] = *n as f32;
                *pos += 1;
            }
            // skip comma
            if matches!(tokens.get(*pos), Some(Token::Comma)) { *pos += 1; }
        }

        if matches!(tokens.get(*pos), Some(Token::CloseParen)) { *pos += 1; }
        if matches!(tokens.get(*pos), Some(Token::Comma)) { *pos += 1; }
    }

    if matches!(tokens.get(*pos), Some(Token::CloseParen)) { *pos += 1; }
    Some(m)
}

fn parse_matrix4_array(tokens: &[Token], pos: &mut usize) -> Vec<[f32; 16]> {
    let mut result = Vec::new();
    if !matches!(tokens.get(*pos), Some(Token::OpenBracket)) {
        skip_value(tokens, pos);
        return result;
    }
    *pos += 1;

    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::CloseBracket => { *pos += 1; break; }
            Token::OpenParen => {
                if let Some(m) = parse_matrix4_value(tokens, pos) {
                    result.push(m);
                }
            }
            Token::Comma => *pos += 1,
            _ => *pos += 1,
        }
    }

    result
}

fn read_3_numbers(tokens: &[Token], pos: &mut usize) -> Option<[f32; 3]> {
    let mut vals = [0.0f32; 3];
    for i in 0..3 {
        match tokens.get(*pos) {
            Some(Token::Number(n)) => {
                vals[i] = *n as f32;
                *pos += 1;
            }
            _ => return None,
        }
        if matches!(tokens.get(*pos), Some(Token::Comma)) { *pos += 1; }
    }
    Some(vals)
}

fn read_2_numbers(tokens: &[Token], pos: &mut usize) -> Option<[f32; 2]> {
    let mut vals = [0.0f32; 2];
    for i in 0..2 {
        match tokens.get(*pos) {
            Some(Token::Number(n)) => {
                vals[i] = *n as f32;
                *pos += 1;
            }
            _ => return None,
        }
        if matches!(tokens.get(*pos), Some(Token::Comma)) { *pos += 1; }
    }
    Some(vals)
}

// ---------------------------------------------------------------------------
// Skip helpers
// ---------------------------------------------------------------------------

fn skip_value(tokens: &[Token], pos: &mut usize) {
    if *pos >= tokens.len() { return; }

    match &tokens[*pos] {
        Token::OpenBracket => { *pos += 1; skip_until_close_bracket(tokens, pos); }
        Token::OpenParen => { *pos += 1; skip_until_close_paren(tokens, pos); }
        Token::OpenBrace => { *pos += 1; skip_until_close_brace(tokens, pos); }
        Token::Keyword(k) if k == "None" => { *pos += 1; }
        _ => { *pos += 1; }
    }
}

fn skip_until_close_paren(tokens: &[Token], pos: &mut usize) {
    let mut depth = 1;
    while *pos < tokens.len() && depth > 0 {
        match &tokens[*pos] {
            Token::OpenParen => depth += 1,
            Token::CloseParen => depth -= 1,
            _ => {}
        }
        *pos += 1;
    }
}

fn skip_until_close_bracket(tokens: &[Token], pos: &mut usize) {
    let mut depth = 1;
    while *pos < tokens.len() && depth > 0 {
        match &tokens[*pos] {
            Token::OpenBracket => depth += 1,
            Token::CloseBracket => depth -= 1,
            _ => {}
        }
        *pos += 1;
    }
}

fn skip_until_close_brace(tokens: &[Token], pos: &mut usize) {
    let mut depth = 1;
    while *pos < tokens.len() && depth > 0 {
        match &tokens[*pos] {
            Token::OpenBrace => depth += 1,
            Token::CloseBrace => depth -= 1,
            _ => {}
        }
        *pos += 1;
    }
}

fn skip_to_next_property(tokens: &[Token], pos: &mut usize) {
    // Advance until we see something that looks like a new property or close brace
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::CloseBrace => return,
            Token::Keyword(k) if k == "def" || k == "over" || k == "class" || k == "custom" || k == "uniform" => return,
            _ => *pos += 1,
        }
    }
}

// ---------------------------------------------------------------------------
// Mesh builder
// ---------------------------------------------------------------------------

use std::collections::HashMap;

struct MeshBuilder {
    path: String,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uv_sets: HashMap<String, Vec<[f32; 2]>>,
    colors: Vec<[f32; 4]>,
    face_vertex_counts: Vec<u32>,
    face_vertex_indices: Vec<u32>,
    material_binding: Option<String>,
    subdivision_scheme: String,
}

impl MeshBuilder {
    fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            positions: Vec::new(),
            normals: Vec::new(),
            uv_sets: HashMap::new(),
            colors: Vec::new(),
            face_vertex_counts: Vec::new(),
            face_vertex_indices: Vec::new(),
            material_binding: None,
            subdivision_scheme: "none".into(),
        }
    }

    fn build(self) -> UsdMesh {
        let name = self.path.rsplit('/').next().unwrap_or("Mesh").to_string();
        UsdMesh {
            name,
            path: self.path,
            positions: self.positions,
            normals: self.normals,
            uv_sets: self.uv_sets,
            colors: self.colors,
            tangents: Vec::new(),
            face_vertex_counts: self.face_vertex_counts,
            face_vertex_indices: self.face_vertex_indices,
            subsets: Vec::new(),
            skin: None,
            blend_shapes: Vec::new(),
            material_binding: self.material_binding,
            material_index: None,
            subdivision_scheme: self.subdivision_scheme,
        }
    }
}

fn compute_parent_indices(joints: &[String]) -> Vec<i32> {
    joints
        .iter()
        .map(|joint| {
            if let Some(slash_pos) = joint.rfind('/') {
                let parent = &joint[..slash_pos];
                joints
                    .iter()
                    .position(|j| j == parent)
                    .map(|i| i as i32)
                    .unwrap_or(-1)
            } else {
                -1
            }
        })
        .collect()
}

fn resolve_material_bindings(stage: &mut UsdStage) {
    for mesh in &mut stage.meshes {
        if let Some(ref binding) = mesh.material_binding {
            mesh.material_index = stage
                .materials
                .iter()
                .position(|m| m.path == *binding);
        }
    }
}
