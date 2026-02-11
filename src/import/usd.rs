//! USD/USDZ file parser and GLB converter.
//!
//! - `.usdz`: ZIP archive containing `.usda`/`.usdc` + textures
//! - `.usda`: Text-based USD format (basic mesh extraction supported)
//! - `.usdc`: Binary Crate format (not supported - logged as warning)

use std::path::Path;

use super::glb_builder::{GlbBuilder, MeshData, TextureData};

/// Parse a USD/USDZ file and feed geometry into a GlbBuilder.
pub fn convert_usd(path: &Path, builder: &mut GlbBuilder) -> Result<(), String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "usdz" => convert_usdz(path, builder),
        "usda" => {
            let content =
                std::fs::read_to_string(path).map_err(|e| format!("Failed to read USDA: {}", e))?;
            parse_usda(&content, builder);
            Ok(())
        }
        "usd" => {
            // USD can be either text or binary - try text first
            if let Ok(content) = std::fs::read_to_string(path) {
                if content.contains("#usda") || content.contains("def ") {
                    parse_usda(&content, builder);
                    return Ok(());
                }
            }
            // Binary USD (usdc)
            log::warn!(
                "Binary USD (.usdc) format is not supported for direct mesh extraction. \
                 Please convert to .usda text format or .usdz."
            );
            Err("Binary USD (.usdc) format is not supported. Convert to .usda or .usdz.".to_string())
        }
        _ => Err(format!("Unknown USD extension: {}", ext)),
    }
}

/// Extract a USDZ archive and parse the contained USDA/USDC files.
fn convert_usdz(path: &Path, builder: &mut GlbBuilder) -> Result<(), String> {
    let file =
        std::fs::File::open(path).map_err(|e| format!("Failed to open USDZ: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Failed to read USDZ archive: {}", e))?;

    let mut usda_content: Option<String> = None;
    let mut texture_files: Vec<(String, Vec<u8>)> = Vec::new();

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read USDZ entry: {}", e))?;

        let name = entry.name().to_string();
        let ext = name
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            "usda" => {
                let mut content = String::new();
                std::io::Read::read_to_string(&mut entry, &mut content)
                    .map_err(|e| format!("Failed to read USDA from USDZ: {}", e))?;
                usda_content = Some(content);
            }
            "usdc" => {
                log::warn!(
                    "Binary USD (.usdc) found in USDZ archive. \
                     Only .usda text format is supported for mesh extraction."
                );
            }
            "png" | "jpg" | "jpeg" => {
                let mut data = Vec::new();
                std::io::Read::read_to_end(&mut entry, &mut data)
                    .map_err(|e| format!("Failed to read texture from USDZ: {}", e))?;
                texture_files.push((name, data));
            }
            _ => {}
        }
    }

    // Embed textures
    for (name, data) in &texture_files {
        let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
        let mime_type = match ext.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            _ => "application/octet-stream",
        };
        builder.add_texture(TextureData {
            name: Some(name.clone()),
            mime_type: mime_type.to_string(),
            data: data.clone(),
        });
    }

    // Parse USDA content if found
    if let Some(content) = usda_content {
        parse_usda(&content, builder);
    } else {
        return Err(
            "No .usda file found in USDZ archive. Binary .usdc is not supported.".to_string(),
        );
    }

    Ok(())
}

/// Basic USDA text parser. Extracts `def Mesh` blocks with positions, face indices, normals, and UVs.
fn parse_usda(content: &str, builder: &mut GlbBuilder) {
    // Very simple state-machine parser for USDA text format
    let mut in_mesh = false;
    let mut mesh_name = String::new();
    let mut brace_depth = 0i32;
    let mut current_attr = String::new();
    let mut collecting_array = false;
    let mut array_content = String::new();

    let mut positions: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut tex_coords: Vec<[f32; 2]> = Vec::new();
    let mut face_vertex_counts: Vec<u32> = Vec::new();
    let mut face_vertex_indices: Vec<u32> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // Track brace depth
        let opens = trimmed.matches('{').count() as i32;
        let closes = trimmed.matches('}').count() as i32;

        if !in_mesh {
            // Look for "def Mesh"
            if trimmed.starts_with("def Mesh") || trimmed.starts_with("def \"Mesh") {
                in_mesh = true;
                mesh_name = extract_def_name(trimmed);
                brace_depth = 0;
                positions.clear();
                normals.clear();
                tex_coords.clear();
                face_vertex_counts.clear();
                face_vertex_indices.clear();
            }
        }

        if in_mesh {
            brace_depth += opens - closes;

            if collecting_array {
                array_content.push_str(trimmed);
                if trimmed.contains(']') {
                    collecting_array = false;
                    // Process collected array
                    match current_attr.as_str() {
                        "points" => positions = parse_vec3_array(&array_content),
                        "normals" => normals = parse_vec3_array(&array_content),
                        "texCoords" | "st" => tex_coords = parse_vec2_array(&array_content),
                        "faceVertexCounts" => face_vertex_counts = parse_int_array(&array_content),
                        "faceVertexIndices" => face_vertex_indices = parse_int_array(&array_content),
                        _ => {}
                    }
                    array_content.clear();
                }
                continue;
            }

            // Detect array attributes
            if let Some(attr_name) = detect_attribute(trimmed) {
                current_attr = attr_name;
                if trimmed.contains('[') {
                    if trimmed.contains(']') {
                        // Single-line array
                        match current_attr.as_str() {
                            "points" => positions = parse_vec3_array(trimmed),
                            "normals" => normals = parse_vec3_array(trimmed),
                            "texCoords" | "st" => tex_coords = parse_vec2_array(trimmed),
                            "faceVertexCounts" => face_vertex_counts = parse_int_array(trimmed),
                            "faceVertexIndices" => face_vertex_indices = parse_int_array(trimmed),
                            _ => {}
                        }
                    } else {
                        // Multi-line array
                        collecting_array = true;
                        array_content = trimmed.to_string();
                    }
                }
            }

            if brace_depth <= 0 && !trimmed.is_empty() {
                // End of mesh block - build the mesh
                if !positions.is_empty() {
                    let triangulated_indices =
                        triangulate_faces(&face_vertex_counts, &face_vertex_indices);

                    // Remap normals from per-face-vertex to per-vertex if sizes don't match
                    let final_normals = if normals.len() == positions.len() {
                        Some(normals.clone())
                    } else {
                        None
                    };

                    // Flip UV v-coordinates (USD uses bottom-left origin)
                    let final_uvs = if tex_coords.len() == positions.len() {
                        Some(
                            tex_coords
                                .iter()
                                .map(|uv| [uv[0], 1.0 - uv[1]])
                                .collect(),
                        )
                    } else {
                        None
                    };

                    builder.add_mesh(MeshData {
                        name: Some(if mesh_name.is_empty() {
                            "Mesh".to_string()
                        } else {
                            mesh_name.clone()
                        }),
                        positions: positions.clone(),
                        normals: final_normals,
                        tex_coords: final_uvs,
                        indices: if triangulated_indices.is_empty() {
                            None
                        } else {
                            Some(triangulated_indices)
                        },
                        material_index: None,
                    });
                }

                in_mesh = false;
            }
        }
    }
}

fn extract_def_name(line: &str) -> String {
    // Extract name from: def Mesh "MyMesh" {
    let parts: Vec<&str> = line.split('"').collect();
    if parts.len() >= 2 {
        parts[1].to_string()
    } else {
        "Mesh".to_string()
    }
}

fn detect_attribute(line: &str) -> Option<String> {
    // Match patterns like:
    //   point3f[] points = [...]
    //   int[] faceVertexCounts = [...]
    //   normal3f[] normals = [...]
    //   texCoord2f[] primvars:st = [...]
    //   float2[] primvars:st = [...]
    let line = line.trim();

    if line.contains("points") && line.contains('=') {
        return Some("points".to_string());
    }
    if line.contains("faceVertexCounts") && line.contains('=') {
        return Some("faceVertexCounts".to_string());
    }
    if line.contains("faceVertexIndices") && line.contains('=') {
        return Some("faceVertexIndices".to_string());
    }
    if (line.contains("normals") || line.contains("normal3f")) && line.contains('=') {
        return Some("normals".to_string());
    }
    if (line.contains("primvars:st") || line.contains("texCoord")) && line.contains('=') {
        return Some("st".to_string());
    }

    None
}

fn parse_vec3_array(text: &str) -> Vec<[f32; 3]> {
    let mut result = Vec::new();
    // Find content between [ and ]
    let start = text.find('[').unwrap_or(0);
    let end = text.rfind(']').unwrap_or(text.len());
    let inner = &text[start + 1..end];

    // Match (x, y, z) tuples
    let mut i = 0;
    let chars: Vec<char> = inner.chars().collect();
    while i < chars.len() {
        if chars[i] == '(' {
            let tuple_end = inner[i..].find(')').map(|p| p + i).unwrap_or(inner.len());
            let tuple_str = &inner[i + 1..tuple_end];
            let vals: Vec<f32> = tuple_str
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            if vals.len() == 3 {
                result.push([vals[0], vals[1], vals[2]]);
            }
            i = tuple_end + 1;
        } else {
            i += 1;
        }
    }

    result
}

fn parse_vec2_array(text: &str) -> Vec<[f32; 2]> {
    let mut result = Vec::new();
    let start = text.find('[').unwrap_or(0);
    let end = text.rfind(']').unwrap_or(text.len());
    let inner = &text[start + 1..end];

    let mut i = 0;
    let chars: Vec<char> = inner.chars().collect();
    while i < chars.len() {
        if chars[i] == '(' {
            let tuple_end = inner[i..].find(')').map(|p| p + i).unwrap_or(inner.len());
            let tuple_str = &inner[i + 1..tuple_end];
            let vals: Vec<f32> = tuple_str
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            if vals.len() == 2 {
                result.push([vals[0], vals[1]]);
            }
            i = tuple_end + 1;
        } else {
            i += 1;
        }
    }

    result
}

fn parse_int_array(text: &str) -> Vec<u32> {
    let start = text.find('[').unwrap_or(0);
    let end = text.rfind(']').unwrap_or(text.len());
    let inner = &text[start + 1..end];

    inner
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect()
}

/// Triangulate polygon faces given faceVertexCounts and faceVertexIndices.
fn triangulate_faces(face_vertex_counts: &[u32], face_vertex_indices: &[u32]) -> Vec<u32> {
    let mut result = Vec::new();
    let mut idx_offset = 0usize;

    for &count in face_vertex_counts {
        let n = count as usize;
        if n < 3 || idx_offset + n > face_vertex_indices.len() {
            idx_offset += n;
            continue;
        }

        // Fan triangulation from first vertex
        let v0 = face_vertex_indices[idx_offset];
        for i in 1..n - 1 {
            result.push(v0);
            result.push(face_vertex_indices[idx_offset + i]);
            result.push(face_vertex_indices[idx_offset + i + 1]);
        }

        idx_offset += n;
    }

    result
}
