//! OBJ/MTL file parser and GLB converter using the `tobj` crate.

use std::path::Path;

use super::glb_builder::{GlbBuilder, MaterialData, MeshData, TextureData};

/// Parse an OBJ file and feed geometry/materials into a GlbBuilder.
pub fn convert_obj(path: &Path, builder: &mut GlbBuilder) -> Result<(), String> {
    let parent_dir = path.parent().unwrap_or(Path::new("."));

    let load_options = tobj::LoadOptions {
        triangulate: true,
        single_index: true,
        ..Default::default()
    };

    let (models, materials_result) =
        tobj::load_obj(path, &load_options).map_err(|e| format!("Failed to load OBJ: {}", e))?;

    // Load materials (if MTL file exists)
    let materials = match materials_result {
        Ok(mats) => mats,
        Err(e) => {
            log::warn!("Failed to load MTL for {}: {}. Using defaults.", path.display(), e);
            Vec::new()
        }
    };

    // Build GLB materials + textures from MTL materials
    let mut material_map: Vec<Option<usize>> = Vec::new();
    for mtl in &materials {
        let mut base_color = [1.0, 1.0, 1.0, 1.0];
        if let Some(diffuse) = mtl.diffuse {
            base_color = [diffuse[0], diffuse[1], diffuse[2], 1.0];
        }
        if let Some(d) = mtl.dissolve {
            base_color[3] = d;
        }

        // Try to load diffuse texture
        let texture_index = if let Some(ref tex_name) = mtl.diffuse_texture {
            load_texture(parent_dir, tex_name, builder)
        } else {
            None
        };

        let mat_idx = builder.add_material(MaterialData {
            name: Some(mtl.name.clone()),
            base_color,
            metallic: 0.0,
            roughness: 1.0,
            base_color_texture_index: texture_index,
        });
        material_map.push(Some(mat_idx));
    }

    // Build meshes
    for model in &models {
        let mesh = &model.mesh;

        // Positions
        let vertex_count = mesh.positions.len() / 3;
        let mut positions = Vec::with_capacity(vertex_count);
        for i in 0..vertex_count {
            positions.push([
                mesh.positions[i * 3],
                mesh.positions[i * 3 + 1],
                mesh.positions[i * 3 + 2],
            ]);
        }

        // Normals
        let normals = if !mesh.normals.is_empty() {
            let mut norms = Vec::with_capacity(vertex_count);
            for i in 0..vertex_count {
                norms.push([
                    mesh.normals[i * 3],
                    mesh.normals[i * 3 + 1],
                    mesh.normals[i * 3 + 2],
                ]);
            }
            Some(norms)
        } else {
            None
        };

        // Texture coordinates
        let tex_coords = if !mesh.texcoords.is_empty() {
            let mut uvs = Vec::with_capacity(vertex_count);
            for i in 0..vertex_count {
                uvs.push([
                    mesh.texcoords[i * 2],
                    // Flip V coordinate (OBJ uses bottom-left origin, glTF uses top-left)
                    1.0 - mesh.texcoords[i * 2 + 1],
                ]);
            }
            Some(uvs)
        } else {
            None
        };

        // Indices
        let indices = if !mesh.indices.is_empty() {
            Some(mesh.indices.clone())
        } else {
            None
        };

        // Material reference
        let material_index = mesh
            .material_id
            .and_then(|id| material_map.get(id).copied().flatten());

        builder.add_mesh(MeshData {
            name: Some(model.name.clone()),
            positions,
            normals,
            tex_coords,
            indices,
            material_index,
        });
    }

    Ok(())
}

/// Try to load a texture file from disk and embed it in the GLB
fn load_texture(parent_dir: &Path, tex_name: &str, builder: &mut GlbBuilder) -> Option<usize> {
    let tex_path = parent_dir.join(tex_name);
    if !tex_path.exists() {
        log::warn!("Texture not found: {}", tex_path.display());
        return None;
    }

    let data = std::fs::read(&tex_path).ok()?;
    let ext = tex_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let mime_type = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        _ => "application/octet-stream",
    };

    Some(builder.add_texture(TextureData {
        name: Some(tex_name.to_string()),
        mime_type: mime_type.to_string(),
        data,
    }))
}
