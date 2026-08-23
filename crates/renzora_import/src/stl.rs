//! STL → GLB converter.
//!
//! STL is the thinnest of the mesh formats: a bag of triangles with a facet
//! normal each, and nothing else. No hierarchy, no names, no materials, no
//! textures, no UVs, no units. Everything beyond position and winding has to
//! come from convention or be synthesised here — see [`effective_up_axis`] and
//! [`box_project_uvs`] for the two places that matters.

use std::path::Path;

use crate::convert::{ConvertedGlb, ImportError};
use crate::glb_build::build_glb;
use crate::settings::{ImportSettings, UpAxis};

/// Resolve `Auto` for a format that stores no axis metadata.
///
/// There is nothing in an STL to detect from, so the convention *is* the
/// detection: STL is a CAD and 3D-printing interchange format whose build plate
/// is the XY plane, and effectively every producer writes +Z up. Treating
/// `Auto` as "leave it alone" meant every STL arrived on its back — a building
/// lying face-up rather than standing on the ground. Picking Y-Up explicitly
/// still overrides this.
fn effective_up_axis(settings: &ImportSettings) -> UpAxis {
    match settings.up_axis {
        UpAxis::Auto => UpAxis::ZUp,
        explicit => explicit,
    }
}

/// Synthesise a UV set from a box projection.
///
/// STL stores no UVs, and we were writing an all-zero `TEXCOORD_0` to fill the
/// attribute. That is worse than it sounds: the mesh *claims* to have UVs, so
/// any texture assigned later samples one texel and renders as a flat block of
/// colour, with nothing to indicate why. These packs routinely ship a
/// `textures/` folder next to the geometry, so the material is texturable in
/// principle — it just has no coordinates to sample with.
///
/// Each vertex is projected along whichever axis its (already averaged) normal
/// faces most strongly, then normalised into 0..1 across the model's bounds. It
/// is not a real unwrap — vertices on a corner pick one face and seam — but it
/// is a usable starting point, which zeros never are.
fn box_project_uvs(positions: &[f32], normals: &[f32], flip_v: bool) -> Vec<f32> {
    let vertex_count = positions.len() / 3;
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for v in 0..vertex_count {
        for c in 0..3 {
            let p = positions[v * 3 + c];
            min[c] = min[c].min(p);
            max[c] = max[c].max(p);
        }
    }
    // A flat or degenerate axis would divide by zero and produce NaN UVs, which
    // fail glTF validation rather than merely looking wrong.
    let extent = [
        (max[0] - min[0]).max(1e-6),
        (max[1] - min[1]).max(1e-6),
        (max[2] - min[2]).max(1e-6),
    ];

    let mut uvs = Vec::with_capacity(vertex_count * 2);
    for v in 0..vertex_count {
        let p = [
            positions[v * 3],
            positions[v * 3 + 1],
            positions[v * 3 + 2],
        ];
        let n = [
            normals[v * 3].abs(),
            normals[v * 3 + 1].abs(),
            normals[v * 3 + 2].abs(),
        ];
        // Pick the plane the surface faces, so the projection stretches least.
        // The axis pairs keep the winding consistent per plane.
        let (a, b) = if n[0] >= n[1] && n[0] >= n[2] {
            (2, 1) // facing X → project onto ZY
        } else if n[1] >= n[2] {
            (0, 2) // facing Y → project onto XZ
        } else {
            (0, 1) // facing Z → project onto XY
        };
        uvs.push((p[a] - min[a]) / extent[a]);
        // glTF's V axis runs downward, so flip to keep the projection upright.
        // `Flip UVs` inverts that again, same as it does for a real UV set.
        let v = (p[b] - min[b]) / extent[b];
        uvs.push(if flip_v { v } else { 1.0 - v });
    }
    uvs
}

pub fn convert(path: &Path, settings: &ImportSettings) -> Result<ConvertedGlb, ImportError> {
    let mut file = std::fs::OpenOptions::new().read(true).open(path)?;

    let mesh = stl_io::read_stl(&mut file)
        .map_err(|e| ImportError::ParseError(format!("STL parse error: {}", e)))?;

    if mesh.faces.is_empty() {
        return Err(ImportError::ParseError(
            "STL file contains no triangles".into(),
        ));
    }

    let warnings = Vec::new();
    let up_axis = effective_up_axis(settings);

    // Build vertex arrays from indexed mesh
    let mut positions = Vec::with_capacity(mesh.vertices.len() * 3);
    let mut indices: Vec<u32> = Vec::with_capacity(mesh.faces.len() * 3);

    // Add all vertices
    for v in &mesh.vertices {
        let (x, mut y, mut z) = (
            v.0[0] * settings.scale,
            v.0[1] * settings.scale,
            v.0[2] * settings.scale,
        );

        if up_axis == UpAxis::ZUp {
            let tmp = y;
            y = z;
            z = -tmp;
        }

        positions.extend_from_slice(&[x, y, z]);
    }

    // Per-vertex normals accumulated from face normals. Accumulating
    // un-normalized face normals area-weights the average, which gives smoother
    // results on irregular meshes than a plain sum of unit normals.
    let vertex_count = mesh.vertices.len();
    let mut vert_normals = vec![0.0f32; vertex_count * 3];

    let pos = |vi: usize| {
        [
            positions[vi * 3],
            positions[vi * 3 + 1],
            positions[vi * 3 + 2],
        ]
    };

    for face in &mesh.faces {
        // Use the stored facet normal when meaningful. A large share of
        // real-world STLs (especially ASCII exports and many slicers) write
        // (0,0,0) facet normals and rely on the importer deriving them from the
        // triangle winding (STL is CCW = outward). Fall back to the geometric
        // normal of the already-transformed triangle in that case so flat
        // prints still light correctly.
        let stored = {
            let (nx, mut ny, mut nz) = (face.normal.0[0], face.normal.0[1], face.normal.0[2]);
            if up_axis == UpAxis::ZUp {
                let tmp = ny;
                ny = nz;
                nz = -tmp;
            }
            [nx, ny, nz]
        };
        let normal = if stored[0] * stored[0] + stored[1] * stored[1] + stored[2] * stored[2] > 1e-12
        {
            stored
        } else {
            let a = pos(face.vertices[0]);
            let b = pos(face.vertices[1]);
            let c = pos(face.vertices[2]);
            let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            [
                ab[1] * ac[2] - ab[2] * ac[1],
                ab[2] * ac[0] - ab[0] * ac[2],
                ab[0] * ac[1] - ab[1] * ac[0],
            ]
        };

        for &vi in &face.vertices {
            vert_normals[vi * 3] += normal[0];
            vert_normals[vi * 3 + 1] += normal[1];
            vert_normals[vi * 3 + 2] += normal[2];
            indices.push(vi as u32);
        }
    }

    // Normalize
    for i in 0..vertex_count {
        let (x, y, z) = (
            vert_normals[i * 3],
            vert_normals[i * 3 + 1],
            vert_normals[i * 3 + 2],
        );
        let len = (x * x + y * y + z * z).sqrt();
        if len > 1e-8 {
            vert_normals[i * 3] /= len;
            vert_normals[i * 3 + 1] /= len;
            vert_normals[i * 3 + 2] /= len;
        } else {
            vert_normals[i * 3 + 1] = 1.0;
        }
    }

    // STL has no UVs of its own; project a usable set rather than write zeros.
    let texcoords = box_project_uvs(&positions, &vert_normals, settings.flip_uvs);

    // STL carries no materials, but we still emit one neutral "Default"
    // material so the mesh imports with an editable `.material` binding rather
    // than the engine's fallback. The single bundle entry makes `build_glb`
    // reference material 0 on the primitive; the shared pass reads it back out
    // of the GLB and binds it by name.
    let mut bundle = crate::glb_build::MaterialBundle::default();
    if settings.extract_materials {
        // A pack that ships a `textures/` folder beside a geometry-only model
        // has textures we can use, but never a statement of which set belongs
        // to which model — so the choice comes from the inspector rather than
        // from a guess here. See `sibling_textures` for why guessing is wrong.
        let chosen = settings.texture_set.as_deref().and_then(|want| {
            crate::sibling_textures::discover(path)
                .into_iter()
                .find(|s| s.stem == want)
        });

        // Record where each map lives; reading and processing it is
        // `gltf_pass::finish_converted_glb`'s job, exactly as for OBJ's
        // MTL-referenced textures.
        let mut slot = |role: crate::sibling_textures::MapRole| -> Option<usize> {
            if !settings.extract_textures {
                return None;
            }
            let file = chosen.as_ref()?.get(role)?;
            let idx = bundle.textures.len();
            bundle.textures.push(crate::glb_build::TextureRef {
                uri: file.to_string_lossy().into_owned(),
                embedded: None,
            });
            Some(idx)
        };

        use crate::sibling_textures::MapRole;
        let base_color_texture = slot(MapRole::BaseColor);
        let normal_texture = slot(MapRole::Normal);
        let roughness_texture = slot(MapRole::Roughness);
        let metallic_texture = slot(MapRole::Metallic);
        let occlusion_texture = slot(MapRole::Occlusion);
        let specular_texture = slot(MapRole::Specular);
        let emissive_texture = slot(MapRole::Emissive);
        let opacity_texture = slot(MapRole::Opacity);

        bundle.materials.push(crate::glb_build::PbrMaterialDef {
            // Naming it after the bound set beats a generic "Default" once
            // there is something to name it after — the material browser shows
            // this, and "Steel" is findable in a way "Default" is not.
            name: chosen
                .as_ref()
                .map(|s| s.stem.clone())
                .unwrap_or_else(|| "Default".into()),
            // A textured material must not tint its base colour map. The grey
            // placeholder only makes sense when there is no map to show.
            base_color: if base_color_texture.is_some() {
                [1.0, 1.0, 1.0, 1.0]
            } else {
                [0.8, 0.8, 0.8, 1.0]
            },
            base_color_texture,
            normal_texture,
            metallic: 0.0,
            roughness: 0.7,
            emissive: [0.0, 0.0, 0.0],
            emissive_texture,
            occlusion_texture,
            opacity_texture,
            specular_texture,
            roughness_texture,
            metallic_texture,
            alpha: crate::glb_build::AlphaKind::Opaque,
            double_sided: false,
            advanced: renzora::core::PbrAdvanced::default(),
        });
    }


    let glb_bytes = build_glb(&positions, &vert_normals, &texcoords, &indices, &bundle)?;

    Ok(ConvertedGlb {
        glb_bytes,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(up: UpAxis) -> ImportSettings {
        ImportSettings {
            up_axis: up,
            ..Default::default()
        }
    }

    #[test]
    fn auto_means_z_up_because_stl_stores_no_axis() {
        // The whole point: an STL has no metadata to detect from, so `Auto` has
        // to fall back to the format's convention rather than to "do nothing".
        assert_eq!(effective_up_axis(&settings(UpAxis::Auto)), UpAxis::ZUp);
    }

    #[test]
    fn an_explicit_axis_still_wins() {
        assert_eq!(effective_up_axis(&settings(UpAxis::YUp)), UpAxis::YUp);
        assert_eq!(effective_up_axis(&settings(UpAxis::ZUp)), UpAxis::ZUp);
    }

    #[test]
    fn projected_uvs_span_the_unit_square() {
        // A unit quad in the XY plane facing +Z: the projection should use the
        // full 0..1 range on both axes rather than collapsing to a point.
        let positions = [
            0.0, 0.0, 0.0, //
            1.0, 0.0, 0.0, //
            1.0, 1.0, 0.0, //
            0.0, 1.0, 0.0,
        ];
        let normals: Vec<f32> = (0..4).flat_map(|_| [0.0f32, 0.0, 1.0]).collect();
        let uvs = box_project_uvs(&positions, &normals, false);

        assert_eq!(uvs.len(), 8);
        let us: Vec<f32> = uvs.iter().step_by(2).copied().collect();
        let vs: Vec<f32> = uvs.iter().skip(1).step_by(2).copied().collect();
        assert_eq!(us.iter().cloned().fold(f32::MAX, f32::min), 0.0);
        assert_eq!(us.iter().cloned().fold(f32::MIN, f32::max), 1.0);
        assert_eq!(vs.iter().cloned().fold(f32::MAX, f32::min), 0.0);
        assert_eq!(vs.iter().cloned().fold(f32::MIN, f32::max), 1.0);
    }

    #[test]
    fn projected_uvs_are_never_all_zero() {
        // The bug this replaces: every vertex got (0,0), so a textured surface
        // sampled one texel and rendered as a flat block of colour.
        let positions = [
            0.0, 0.0, 0.0, //
            2.0, 0.0, 0.0, //
            2.0, 3.0, 0.0, //
            0.0, 3.0, 1.5,
        ];
        let normals: Vec<f32> = (0..4).flat_map(|_| [0.0f32, 0.0, 1.0]).collect();
        let uvs = box_project_uvs(&positions, &normals, false);
        assert!(uvs.iter().any(|&c| c != 0.0), "projection produced zeros");
        assert!(uvs.iter().all(|c| c.is_finite()), "projection produced NaN");
    }

    #[test]
    fn a_flat_axis_does_not_produce_nan() {
        // A perfectly planar model has zero extent on one axis; dividing by it
        // would write NaN UVs, which fail glTF validation outright.
        let positions = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0];
        let normals: Vec<f32> = (0..3).flat_map(|_| [0.0f32, 1.0, 0.0]).collect();
        let uvs = box_project_uvs(&positions, &normals, false);
        assert!(uvs.iter().all(|c| c.is_finite()));
    }

    #[test]
    fn flip_uvs_inverts_v() {
        let positions = [0.0, 0.0, 0.0, 1.0, 1.0, 0.0];
        let normals: Vec<f32> = (0..2).flat_map(|_| [0.0f32, 0.0, 1.0]).collect();
        let normal = box_project_uvs(&positions, &normals, false);
        let flipped = box_project_uvs(&positions, &normals, true);
        assert_eq!(normal[0], flipped[0], "U is unaffected");
        assert!((normal[1] + flipped[1] - 1.0).abs() < 1e-6, "V mirrors");
    }
}
