#![allow(unused_mut, dead_code, unused_variables)]

//! OBJ (Wavefront) → GLB converter.

use std::path::Path;

use crate::convert::{ConvertedGlb, ImportError};
use crate::glb_build::{build_glb, MaterialBundle, PbrMaterialDef, TextureRef};
use crate::settings::{ImportSettings, UpAxis};

pub fn convert(path: &Path, settings: &ImportSettings) -> Result<ConvertedGlb, ImportError> {
    let load_options = tobj::LoadOptions {
        triangulate: true,
        single_index: true,
        ..Default::default()
    };

    let (models, materials_result) = tobj::load_obj(path, &load_options)
        .map_err(|e| ImportError::ParseError(format!("OBJ parse error: {}", e)))?;

    let mut warnings = Vec::new();
    let mtl_materials = match materials_result {
        Ok(m) => m,
        Err(e) => {
            warnings.push(format!("MTL parse: {} (materials skipped)", e));
            Vec::new()
        }
    };

    if models.is_empty() {
        return Err(ImportError::ParseError(
            "OBJ file contains no meshes".into(),
        ));
    }

    let mut all_positions: Vec<f32> = Vec::new();
    let mut all_normals: Vec<f32> = Vec::new();
    let mut all_texcoords: Vec<f32> = Vec::new();
    let mut all_indices: Vec<u32> = Vec::new();

    for model in &models {
        let mesh = &model.mesh;
        let vertex_count = mesh.positions.len() / 3;

        if vertex_count == 0 {
            warnings.push(format!("mesh '{}' has no vertices, skipping", model.name));
            continue;
        }

        let base_vertex = (all_positions.len() / 3) as u32;

        for i in 0..vertex_count {
            let (x, mut y, mut z) = (
                mesh.positions[i * 3] * settings.scale,
                mesh.positions[i * 3 + 1] * settings.scale,
                mesh.positions[i * 3 + 2] * settings.scale,
            );

            if settings.up_axis == UpAxis::ZUp {
                let tmp = y;
                y = z;
                z = -tmp;
            }

            all_positions.extend_from_slice(&[x, y, z]);
        }

        let has_normals = mesh.normals.len() == vertex_count * 3;
        if has_normals {
            for i in 0..vertex_count {
                let (nx, mut ny, mut nz) = (
                    mesh.normals[i * 3],
                    mesh.normals[i * 3 + 1],
                    mesh.normals[i * 3 + 2],
                );

                if settings.up_axis == UpAxis::ZUp {
                    let tmp = ny;
                    ny = nz;
                    nz = -tmp;
                }

                all_normals.extend_from_slice(&[nx, ny, nz]);
            }
        } else if settings.generate_normals {
            let normals = generate_flat_normals(
                &all_positions[base_vertex as usize * 3..],
                &mesh.indices,
                vertex_count,
            );
            all_normals.extend_from_slice(&normals);
        } else {
            all_normals.extend(std::iter::repeat_n(0.0f32, vertex_count * 3));
        }

        let has_texcoords = mesh.texcoords.len() == vertex_count * 2;
        if has_texcoords {
            for i in 0..vertex_count {
                let u = mesh.texcoords[i * 2];
                let v = if settings.flip_uvs {
                    1.0 - mesh.texcoords[i * 2 + 1]
                } else {
                    mesh.texcoords[i * 2 + 1]
                };
                all_texcoords.extend_from_slice(&[u, v]);
            }
        } else {
            all_texcoords.extend(std::iter::repeat_n(0.0f32, vertex_count * 2));
        }

        for &idx in &mesh.indices {
            all_indices.push(idx + base_vertex);
        }
    }

    if all_positions.is_empty() {
        return Err(ImportError::ParseError(
            "no valid geometry found in OBJ".into(),
        ));
    }

    // Walk MTL materials into the GLB's MaterialBundle, recording where each
    // referenced texture lives. Reading and processing those files is
    // `gltf_pass::finish_converted_glb`'s job.
    let material_bundle = if settings.extract_textures || settings.extract_materials {
        extract_obj_materials(path, &mtl_materials, settings, &mut warnings)
    } else {
        MaterialBundle::default()
    };

    let glb_bytes = build_glb(
        &all_positions,
        &all_normals,
        &all_texcoords,
        &all_indices,
        &material_bundle,
    )?;

    Ok(ConvertedGlb {
        glb_bytes,
        warnings,
    })
}

/// Read every MTL-referenced texture file relative to the OBJ, sniff the
/// format, and build a [`MaterialBundle`] + [`ExtractedPbrMaterial`] list.
/// Missing files surface as warnings; the material entry is still emitted
/// without that particular map.
fn extract_obj_materials(
    obj_path: &Path,
    mtl_materials: &[tobj::Material],
    settings: &ImportSettings,
    warnings: &mut Vec<String>,
) -> MaterialBundle {
    let mut bundle = MaterialBundle::default();
    // MTL texture path (relative to .obj) → index in `bundle.textures`.
    let mut tex_paths: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let obj_dir = obj_path.parent().unwrap_or(Path::new("."));

    // Record where a texture lives; reading and processing it is
    // `gltf_pass::finish_converted_glb`'s job, the same as for every other
    // format. Returns the index into `bundle.textures`.
    let mut load_texture = |rel_path: &str,
                            bundle: &mut MaterialBundle,
                            tex_paths: &mut std::collections::HashMap<String, usize>,
                            warnings: &mut Vec<String>|
     -> Option<usize> {
        if let Some(&i) = tex_paths.get(rel_path) {
            return Some(i);
        }
        let abs = obj_dir.join(rel_path);
        if !abs.is_file() {
            warnings.push(format!("texture '{}': not found", rel_path));
            return None;
        }
        let idx = bundle.textures.len();
        bundle.textures.push(TextureRef {
            uri: abs.to_string_lossy().into_owned(),
            embedded: None,
        });
        tex_paths.insert(rel_path.to_string(), idx);
        Some(idx)
    };

    for mat in mtl_materials {
        let base_color = if let Some(d) = mat.diffuse {
            [d[0], d[1], d[2], mat.dissolve.unwrap_or(1.0)]
        } else {
            [1.0, 1.0, 1.0, 1.0]
        };

        let base_tex = mat.diffuse_texture.as_ref().and_then(|p| {
            if !settings.extract_textures {
                return None;
            }
            load_texture(p, &mut bundle, &mut tex_paths, warnings)
        });
        let normal_tex = mat.normal_texture.as_ref().and_then(|p| {
            if !settings.extract_textures {
                return None;
            }
            load_texture(p, &mut bundle, &mut tex_paths, warnings)
        });

        // PBR-MTL extension. The modern MTL spec adds `Pr` (roughness), `Pm`
        // (metallic), `Ps` (sheen), `Pc`/`Pcr` (clearcoat), `Ke` (emissive),
        // `aniso`/`anisor` (anisotropy) and their `map_*` variants. tobj keeps
        // these unrecognized keywords in `unknown_param`. We honor them when
        // present and fall back to the legacy shininess→roughness heuristic
        // otherwise so plain OBJ files still import sensibly.
        let param_f32 = |key: &str| -> Option<f32> {
            mat.unknown_param
                .get(key)
                .and_then(|v| v.split_whitespace().next())
                .and_then(|s| s.parse::<f32>().ok())
        };
        let param_vec3 = |key: &str| -> Option<[f32; 3]> {
            let v = mat.unknown_param.get(key)?;
            let nums: Vec<f32> = v.split_whitespace().filter_map(|s| s.parse().ok()).collect();
            match nums.len() {
                0 => None,
                1 => Some([nums[0]; 3]),
                _ => Some([nums[0], nums[1], nums[2]]),
            }
        };

        let roughness = param_f32("Pr").unwrap_or_else(|| {
            mat.shininess
                .map(|s| (1.0 - (s / 1000.0)).clamp(0.05, 1.0))
                .unwrap_or(0.8)
        });
        let metallic = param_f32("Pm").unwrap_or(0.0);
        let emissive = param_vec3("Ke").unwrap_or([0.0, 0.0, 0.0]);
        let advanced = renzora::core::PbrAdvanced {
            clearcoat: param_f32("Pc").unwrap_or(0.0),
            clearcoat_roughness: param_f32("Pcr").unwrap_or(0.0),
            ior: mat.optical_density.unwrap_or(1.5),
            anisotropy_strength: param_f32("aniso").unwrap_or(0.0),
            anisotropy_rotation: param_f32("anisor").unwrap_or(0.0),
            ..Default::default()
        };
        let alpha = if mat.dissolve.map(|d| d < 1.0).unwrap_or(false) {
            crate::glb_build::AlphaKind::Blend
        } else {
            crate::glb_build::AlphaKind::Opaque
        };

        // Load the separate PBR map images, if any.
        let mut load_param_tex = |key: &str, bundle: &mut MaterialBundle| -> Option<usize> {
            if !settings.extract_textures {
                return None;
            }
            let p = mat.unknown_param.get(key)?.split_whitespace().last()?;
            load_texture(p, bundle, &mut tex_paths, warnings)
        };
        let roughness_map = load_param_tex("map_Pr", &mut bundle);
        let metallic_map = load_param_tex("map_Pm", &mut bundle);
        let emissive_map = load_param_tex("map_Ke", &mut bundle);

        if settings.extract_materials {
            bundle.materials.push(PbrMaterialDef {
                name: mat.name.clone(),
                base_color,
                base_color_texture: base_tex,
                normal_texture: normal_tex,
                metallic,
                roughness,
                emissive,
                emissive_texture: emissive_map,
                occlusion_texture: None,
                opacity_texture: None,
                specular_texture: None,
                roughness_texture: roughness_map,
                metallic_texture: metallic_map,
                alpha,
                double_sided: false,
                advanced,
            });
        }
    }

    bundle
}

fn sanitize_name(input: &str) -> String {
    if input.is_empty() {
        return "texture".into();
    }
    input
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn sniff_image_ext(data: &[u8]) -> &'static str {
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "png"
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "jpg"
    } else if data.starts_with(b"DDS ") {
        "dds"
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        "gif"
    } else if data.starts_with(b"BM") {
        "bmp"
    } else if data.starts_with(&[0x52, 0x49, 0x46, 0x46]) && data.get(8..12) == Some(b"WEBP") {
        "webp"
    } else {
        "bin"
    }
}

fn generate_flat_normals(positions: &[f32], indices: &[u32], vertex_count: usize) -> Vec<f32> {
    let mut normals = vec![0.0f32; vertex_count * 3];

    for tri in indices.chunks(3) {
        if tri.len() < 3 {
            break;
        }
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);

        let p0 = [
            positions[i0 * 3],
            positions[i0 * 3 + 1],
            positions[i0 * 3 + 2],
        ];
        let p1 = [
            positions[i1 * 3],
            positions[i1 * 3 + 1],
            positions[i1 * 3 + 2],
        ];
        let p2 = [
            positions[i2 * 3],
            positions[i2 * 3 + 1],
            positions[i2 * 3 + 2],
        ];

        let e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        let e2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];

        let n = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];

        for &idx in &[i0, i1, i2] {
            normals[idx * 3] += n[0];
            normals[idx * 3 + 1] += n[1];
            normals[idx * 3 + 2] += n[2];
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



















#[cfg(test)]
mod tests {
    use super::*;

    // ─── byte casting ───────────────────────────────────────────────────



    // ─── name sanitizing ────────────────────────────────────────────────

    #[test]
    fn sanitize_keeps_safe_chars() {
        assert_eq!(sanitize_name("abc_DEF-1.2"), "abc_DEF-1.2");
    }

    #[test]
    fn sanitize_replaces_unsafe_chars() {
        assert_eq!(sanitize_name("a b/c\\d"), "a_b_c_d");
    }

    #[test]
    fn sanitize_empty_falls_back() {
        assert_eq!(sanitize_name(""), "texture");
    }

    // ─── image format sniffing ──────────────────────────────────────────

    #[test]
    fn sniff_known_magic_bytes() {
        assert_eq!(sniff_image_ext(&[0x89, 0x50, 0x4E, 0x47, 0, 0]), "png");
        assert_eq!(sniff_image_ext(&[0xFF, 0xD8, 0xFF, 0xE0]), "jpg");
        assert_eq!(sniff_image_ext(b"DDS  abc"), "dds");
        assert_eq!(sniff_image_ext(b"GIF89a..."), "gif");
        assert_eq!(sniff_image_ext(b"BM......"), "bmp");
    }

    #[test]
    fn sniff_webp_needs_riff_and_webp() {
        let mut data = b"RIFF".to_vec();
        data.extend_from_slice(&[0, 0, 0, 0]); // size
        data.extend_from_slice(b"WEBP");
        assert_eq!(sniff_image_ext(&data), "webp");
        // RIFF without WEBP fourcc should not match webp.
        let mut other = b"RIFF".to_vec();
        other.extend_from_slice(&[0, 0, 0, 0]);
        other.extend_from_slice(b"WAVE");
        assert_eq!(sniff_image_ext(&other), "bin");
    }

    #[test]
    fn sniff_unknown_is_bin() {
        assert_eq!(sniff_image_ext(b"hello world"), "bin");
        assert_eq!(sniff_image_ext(&[]), "bin");
    }

    // ─── flat normal generation ─────────────────────────────────────────

    #[test]
    fn flat_normals_single_triangle_in_xy_plane() {
        // Triangle wound CCW in the XY plane → normal +Z.
        let positions = [
            0.0, 0.0, 0.0, // v0
            1.0, 0.0, 0.0, // v1
            0.0, 1.0, 0.0, // v2
        ];
        let indices = [0u32, 1, 2];
        let normals = generate_flat_normals(&positions, &indices, 3);
        assert_eq!(normals.len(), 9);
        for v in 0..3 {
            assert!((normals[v * 3] - 0.0).abs() < 1e-6);
            assert!((normals[v * 3 + 1] - 0.0).abs() < 1e-6);
            assert!((normals[v * 3 + 2] - 1.0).abs() < 1e-6, "vertex {} z", v);
        }
    }

    #[test]
    fn flat_normals_unreferenced_vertex_defaults_up() {
        // A vertex never touched by a triangle gets the +Y fallback.
        let positions = [0.0, 0.0, 0.0]; // single, unreferenced vertex
        let indices: [u32; 0] = [];
        let normals = generate_flat_normals(&positions, &indices, 1);
        assert_eq!(normals, vec![0.0, 1.0, 0.0]);
    }

    // ─── build_glb end-to-end (no GPU, pure bytes) ──────────────────────







}






