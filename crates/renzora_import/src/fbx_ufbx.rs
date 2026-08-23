//! FBX importer backed by the `ufbx` crate.
//!
//! Replaces the previous hand-rolled binary/ASCII/legacy parsers. ufbx
//! supports every FBX version from 3.0 through 7.7 (binary + ASCII) and
//! normalizes quirks across exporters (Maya / 3ds Max / Blender / Mixamo /
//! MotionBuilder). It bakes PreRotation / GeometricTransform into the usable
//! local transforms and exposes skin clusters with ready-to-use inverse bind
//! matrices, so we can build a skinned GLB straight from its output.

use std::path::Path;

use renzora::{write_anim_file, AnimClip, BoneTrack};

use crate::anim_extract::AnimExtractResult;
use crate::convert::{ConvertedGlb, ImportError};
use crate::glb_build::{build_glb_grouped, build_skinned_glb};
use crate::settings::ImportSettings;

// ─── Public API ────────────────────────────────────────────────────────────

/// Convert an FBX file to a GLB, preserving skeleton + skin weights when
/// present. Any FBX version (3.0 – 7.7), binary or ASCII, is accepted.
pub fn convert(path: &Path, settings: &ImportSettings) -> Result<ConvertedGlb, ImportError> {
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    let scene = load_scene(path, settings)?;
    let scene_ref: &ufbx::Scene = &scene;

    log::info!(
        "[import] {}: ufbx loaded FBX version {}, {} meshes, {} skin clusters",
        file_name,
        scene_ref.metadata.version,
        scene_ref.meshes.len(),
        scene_ref.skin_clusters.len(),
    );

    let mut all_positions: Vec<f32> = Vec::new();
    let mut all_normals: Vec<f32> = Vec::new();
    let mut all_texcoords: Vec<f32> = Vec::new();

    let mut all_joints: Vec<[u16; 4]> = Vec::new();
    let mut all_weights: Vec<[f32; 4]> = Vec::new();

    // Build the joint list once up-front so the same joint indices are valid
    // across every mesh. We include every bone node that appears as a cluster
    // target; this keeps the skeleton minimal to what actually drives skin.
    // Skip skeleton extraction entirely when the user has opted out — the
    // resulting GLB is a plain static mesh even if the source was rigged.
    let joints = if settings.extract_skeleton {
        collect_joints(scene_ref)
    } else {
        Vec::new()
    };
    let has_skin = !joints.is_empty();
    // element_id → joint index. Same key space as the parent-walk lookup.
    let eid_to_joint: std::collections::HashMap<u32, usize> = joints
        .iter()
        .enumerate()
        .map(|(i, j)| (j.element_id, i))
        .collect();

    let mut warnings: Vec<String> = Vec::new();

    // Triangles are bucketed by material as they're produced, because a GLTF
    // primitive wears exactly one material. Everything used to land in a single
    // primitive that referenced material 0, so a scene with 132 materials
    // rendered entirely in the first one. Bucket `material_count` is the
    // catch-all for faces the source assigned no material.
    //
    // `bundle.materials` is filled from `scene.materials` in order further
    // down, so a material's position in that list is its GLTF material index.
    let material_count = scene_ref.materials.len();
    let material_of_eid: std::collections::HashMap<u32, usize> = scene_ref
        .materials
        .iter()
        .enumerate()
        .map(|(i, m)| (m.element.element_id, i))
        .collect();
    let unassigned_bucket = material_count;
    let mut bucket_indices: Vec<Vec<u32>> = vec![Vec::new(); material_count + 1];

    for mesh in scene_ref.meshes.iter() {
        let vertex_count = mesh.num_vertices;
        if vertex_count == 0 {
            continue;
        }

        // Triangulate every face, keeping the results as *mesh corners* rather
        // than vertex indices — see the attribute split below for why that
        // distinction is the whole ballgame.
        let mut corners: Vec<u32> = Vec::new();
        // Which material bucket each triangle belongs to, one entry per
        // triangle (so `tri_bucket.len() * 3 == corners.len()`).
        let mut tri_bucket: Vec<usize> = Vec::new();
        let mut tri_scratch: Vec<u32> = Vec::new();
        for face_idx in 0..mesh.num_faces {
            let face = mesh.faces[face_idx];
            if face.num_indices < 3 {
                continue;
            }
            // `face_material` indexes the *mesh's* own material list, which is
            // a subset of the scene's — resolve through the element id.
            let bucket = mesh
                .face_material
                .get(face_idx)
                .and_then(|local| mesh.materials.get(*local as usize))
                .and_then(|mat| material_of_eid.get(&mat.element.element_id).copied())
                .unwrap_or(unassigned_bucket);

            tri_scratch.clear();
            tri_scratch.resize((face.num_indices as usize - 2) * 3, 0);
            let produced = ufbx::triangulate_face_vec(&mut tri_scratch, mesh, face);
            corners.extend_from_slice(&tri_scratch[..produced as usize * 3]);
            for _ in 0..produced {
                tri_bucket.push(bucket);
            }
        }
        if corners.is_empty() {
            continue;
        }

        // Skin: look at the first skin deformer on this mesh (Mixamo output has
        // exactly one). Collapse its per-vertex top-4 influences into our
        // shared joint index space.
        let mut mesh_joints = vec![[0u16; 4]; vertex_count];
        let mut mesh_weights = vec![[0.0f32; 4]; vertex_count];
        let skin_deformer = mesh.skin_deformers.into_iter().next();
        let is_skinned = skin_deformer.is_some();
        if let Some(skin) = skin_deformer {
            for v in 0..vertex_count {
                let sv = skin.vertices[v];
                let start = sv.weight_begin as usize;
                let n = sv.num_weights as usize;
                let mut infl: Vec<(u16, f32)> = (0..n)
                    .filter_map(|k| {
                        let w = skin.weights[start + k];
                        let clusters: &[ufbx::Ref<ufbx::SkinCluster>] = &skin.clusters;
                        let cluster = clusters.get(w.cluster_index as usize)?;
                        let bone = cluster.bone_node.as_ref()?;
                        let bone_eid = bone.element.element_id;
                        eid_to_joint
                            .get(&bone_eid)
                            .map(|&ji| (ji as u16, w.weight as f32))
                    })
                    .collect();
                infl.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                let top = &infl[..infl.len().min(4)];
                let mut js = [0u16; 4];
                let mut ws = [0.0f32; 4];
                for (i, (j, w)) in top.iter().enumerate() {
                    js[i] = *j;
                    ws[i] = *w;
                }
                let sum: f32 = ws.iter().sum();
                if sum > 0.0 {
                    for w in &mut ws {
                        *w /= sum;
                    }
                }
                mesh_joints[v] = js;
                mesh_weights[v] = ws;
            }
        } else if has_skin {
            warnings.push(format!(
                "mesh '{}' has no skin deformer but scene has a skeleton",
                mesh.element.name.as_ref()
            ));
        }

        // Split vertices by attribute, then deduplicate.
        //
        // FBX stores UVs and normals **per mesh corner**, not per vertex: a
        // vertex sitting on a UV seam or a hard edge has a different value in
        // each face that meets there. Reading one value per vertex — which is
        // what `vertex_uv[vertex_first_index[v]]` does — silently rewrites
        // every such vertex to whichever face happened to be visited first. On
        // this building exterior that is 26.8% of vertices given the wrong UV
        // and 16.1% given the wrong normal: textures slide off the surfaces
        // they belong to, and every hard edge shades as though it were smooth.
        //
        // glTF has no such concept — its attributes are already per vertex —
        // which is why the same scene imports cleanly as a `.glb` and only the
        // FBX path was wrong.
        //
        // `ufbx::generate_indices` is upstream's helper for exactly this: hand
        // it one entry per corner and it collapses identical tuples in place,
        // returning the unique vertex count. Splitting only where an attribute
        // genuinely differs costs 1.38× the vertex count here, rather than the
        // 4.09× that emitting every corner as its own vertex would.
        let corner_count = corners.len();
        let mut positions: Vec<[f32; 3]> = Vec::with_capacity(corner_count);
        let mut normals: Vec<[f32; 3]> = Vec::with_capacity(corner_count);
        let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(corner_count);
        let mut joints: Vec<[u16; 4]> = Vec::with_capacity(corner_count);
        let mut weights: Vec<[f32; 4]> = Vec::with_capacity(corner_count);

        for &corner in &corners {
            let corner = corner as usize;
            let p = mesh.vertex_position[corner];
            positions.push([p.x as f32, p.y as f32, p.z as f32]);

            normals.push(if mesh.vertex_normal.exists {
                let n = mesh.vertex_normal[corner];
                [n.x as f32, n.y as f32, n.z as f32]
            } else {
                [0.0; 3]
            });

            uvs.push(if mesh.vertex_uv.exists {
                let uv = mesh.vertex_uv[corner];
                fbx_uv([uv.x as f32, uv.y as f32], settings.flip_uvs)
            } else {
                [0.0; 2]
            });

            // Skinning stays per vertex; look it up through the corner.
            let v = mesh.vertex_indices[corner] as usize;
            joints.push(mesh_joints.get(v).copied().unwrap_or([0; 4]));
            weights.push(mesh_weights.get(v).copied().unwrap_or([0.0; 4]));
        }

        let mut tri_indices: Vec<u32> = vec![0; corner_count];
        let unique = {
            let mut streams = vec![
                ufbx::VertexStream::new(&mut positions),
                ufbx::VertexStream::new(&mut normals),
                ufbx::VertexStream::new(&mut uvs),
            ];
            if is_skinned {
                streams.push(ufbx::VertexStream::new(&mut joints));
                streams.push(ufbx::VertexStream::new(&mut weights));
            }
            ufbx::generate_indices(&mut streams, &mut tri_indices, Default::default()).map_err(
                |e| {
                    ImportError::ConversionError(format!(
                        "deduplicating mesh '{}': {:?}",
                        mesh.element.name.as_ref(),
                        e.type_
                    ))
                },
            )?
        };
        positions.truncate(unique);
        normals.truncate(unique);
        uvs.truncate(unique);
        joints.truncate(unique);
        weights.truncate(unique);

        // Flatten back into the component streams the GLB builder takes.
        let positions: Vec<f32> = positions.into_iter().flatten().collect();
        let normals: Vec<f32> = normals.into_iter().flatten().collect();
        let uvs: Vec<f32> = uvs.into_iter().flatten().collect();
        let mesh_joints = joints;
        let mesh_weights = weights;

        // Where the mesh actually sits in the scene.
        //
        // `mesh.vertices` is geometry space, and geometry space is *not* the
        // space `load_scene` asked ufbx for. ufbx can only fold the file →
        // target conversion (unit scale + up-axis) into the vertices when a
        // mesh has a single placement it is free to rewrite; in a scene export,
        // where hundreds of nodes each carry their own placement, it puts the
        // conversion into the node transforms instead. Emitting raw vertices
        // therefore keeps the source file's units and up-axis *and* throws away
        // every node's placement — which is how a centimetre, Z-up building
        // exterior imported 100× oversized, lying on its side, with every prop
        // collapsed onto the origin. Bake each instance's `geometry_to_world`
        // into the vertices so the GLB lands in the meters, Y-up, world-placed
        // space the rest of the engine assumes.
        //
        // Skinned meshes are the exception. The inverse bind matrices we export
        // are `cluster.geometry_to_bone`, which is defined *from* geometry
        // space, so baking the node transform into the vertices would apply it
        // a second time the moment a clip plays.
        let placements: Vec<Option<ufbx::Matrix>> = if is_skinned {
            vec![None]
        } else {
            let instances: Vec<Option<ufbx::Matrix>> = mesh
                .element
                .instances
                .iter()
                .map(|node| Some(node.geometry_to_world))
                .collect();
            // A mesh no node references still carries geometry worth keeping;
            // emit it untransformed rather than dropping it silently.
            if instances.is_empty() {
                vec![None]
            } else {
                instances
            }
        };

        for placement in placements {
            let base_vertex = (all_positions.len() / 3) as u32;

            match placement {
                Some(m) => bake_placement(
                    &m,
                    &positions,
                    &normals,
                    &mut all_positions,
                    &mut all_normals,
                ),
                None => {
                    all_positions.extend_from_slice(&positions);
                    all_normals.extend_from_slice(&normals);
                }
            }
            all_texcoords.extend_from_slice(&uvs);

            // A mirroring placement (negative determinant — a node scaled by
            // -1 on an axis, which exporters use for mirrored props) reverses
            // triangle winding. Swap two corners back so the face still points
            // outwards once the transform is baked in.
            let mirrored = placement
                .map(|m| ufbx::matrix_determinant(&m) < 0.0)
                .unwrap_or(false);
            append_indices(
                &tri_indices,
                &tri_bucket,
                base_vertex,
                mirrored,
                &mut bucket_indices,
            );

            all_joints.extend_from_slice(&mesh_joints);
            all_weights.extend_from_slice(&mesh_weights);
        }
    }

    if all_positions.is_empty() {
        return Err(ImportError::ParseError(
            "no geometry found in FBX file".into(),
        ));
    }

    // Build the GLB's materials and image references. Nothing is written or
    // baked here: images point at wherever the file was found on disk, and
    // `gltf_pass::finish_converted_glb` takes it from there — the same pass the
    // glTF importer runs, so this format gets the same roles, the same `.rmip`
    // output and the same memory budget.
    let mut material_bundle = if settings.extract_materials {
        collect_materials(scene_ref, path, &mut warnings)
    } else {
        crate::glb_build::MaterialBundle::default()
    };
    if !settings.extract_textures {
        // Drop every texture reference so the GLB doesn't name files that
        // won't be written. The mesh keeps its materials' factors.
        material_bundle.textures.clear();
        for m in material_bundle.materials.iter_mut() {
            m.base_color_texture = None;
            m.normal_texture = None;
            m.emissive_texture = None;
            m.occlusion_texture = None;
            m.opacity_texture = None;
            m.specular_texture = None;
            m.advanced.clearcoat_texture = None;
            m.advanced.clearcoat_roughness_texture = None;
            m.advanced.clearcoat_normal_texture = None;
            m.advanced.transmission_texture = None;
            m.advanced.thickness_texture = None;
            m.advanced.anisotropy_texture = None;
        }
    }

    // Turn the material buckets into primitives. The catch-all bucket sits at
    // `material_count`, past the end of the material list, so it filters down
    // to a primitive with no material — exactly what unassigned faces want.
    let groups: Vec<crate::glb_build::MaterialGroup> = bucket_indices
        .into_iter()
        .enumerate()
        .filter(|(_, indices)| !indices.is_empty())
        .map(|(bucket, indices)| crate::glb_build::MaterialGroup {
            material: (bucket < material_count).then_some(bucket),
            indices,
        })
        .collect();
    let triangle_count: usize = groups.iter().map(|g| g.indices.len() / 3).sum();

    let glb_bytes = if has_skin {
        log::info!(
            "[import] {}: building skinned GLB with {} joints, {} vertices, {} materials, {} textures",
            file_name,
            joints.len(),
            all_positions.len() / 3,
            material_bundle.materials.len(),
            material_bundle.textures.len(),
        );
        let joint_structs: Vec<crate::glb_build::SkinJoint> = joints
            .iter()
            .map(|j| crate::glb_build::SkinJoint {
                name: j.name.clone(),
                parent: j.parent,
                translation: j.translation,
                rotation: j.rotation,
                scale: j.scale,
                inverse_bind_matrix: j.inverse_bind_matrix,
            })
            .collect();
        build_skinned_glb(
            &all_positions,
            &all_normals,
            &all_texcoords,
            &groups,
            &all_joints,
            &all_weights,
            &joint_structs,
            &material_bundle,
        )?
    } else {
        build_glb_grouped(
            &all_positions,
            &all_normals,
            &all_texcoords,
            &groups,
            &material_bundle,
        )?
    };

    log::info!(
        "[import] {}: GLB output {} bytes ({} vertices, {} triangles, {} primitives)",
        file_name,
        glb_bytes.len(),
        all_positions.len() / 3,
        triangle_count,
        groups.len(),
    );

    Ok(ConvertedGlb {
        glb_bytes,
        warnings,
    })
}

/// Extract every animation stack in an FBX file to a directory of `.anim` files.
/// Measure the unit-scale discrepancy between ufbx's animation evaluator
/// (`evaluate_transform`, which applies `target_unit_meters` and yields meters)
/// and the skeleton export in [`convert`], which reads `node.local_transform`
/// verbatim in the source file's units. Returns the factor that maps evaluator
/// translations back onto skeleton-space units, or `1.0` when no reliable
/// reference bone exists (in which case translations are left untouched).
///
/// The factor is derived directly from a non-root bone rather than from ufbx's
/// reported unit metadata, so it is correct regardless of the source unit
/// (cm, inches, meters). A non-root bone's local translation is a fixed bone
/// length — animation only rotates it — so its rest offset is identical in both
/// systems apart from the unit scale. The skeleton root is skipped because its
/// translation *is* animated, so its rest offset wouldn't cleanly reveal the
/// scale. The longest such bone is used to minimize relative error.
fn animation_unit_fixup(scene: &ufbx::Scene) -> f32 {
    let anim: &ufbx::Anim = &scene.anim;
    let mut best_local_len = 0.0f64;
    let mut best_factor = 1.0f64;

    for node in &scene.nodes {
        if node.bone.is_none() {
            continue;
        }
        let parent_is_bone = node
            .parent
            .as_ref()
            .map(|p| -> &ufbx::Node { p })
            .is_some_and(|p| p.bone.is_some());
        if !parent_is_bone {
            continue;
        }

        let l = node.local_transform.translation;
        let llen = (l.x * l.x + l.y * l.y + l.z * l.z).sqrt();
        if llen <= best_local_len {
            continue;
        }

        let e = ufbx::evaluate_transform(anim, node, 0.0).translation;
        let elen = (e.x * e.x + e.y * e.y + e.z * e.z).sqrt();
        if elen > 1e-9 {
            best_local_len = llen;
            best_factor = llen / elen;
        }
    }

    best_factor as f32
}

pub fn extract_animations(
    path: &Path,
    output_dir: &Path,
    settings: &ImportSettings,
) -> Result<AnimExtractResult, String> {
    // Use the SAME settings (crucially `scale` → `target_unit_meters`) that the
    // caller passed to `convert`. Hardcoding `ImportSettings::default()` here
    // meant the mesh/skeleton honored the user's import scale while the
    // animation clips were always extracted at scale 1.0 — so importing a
    // character at any non-default scale produced clips in a different unit
    // than the skeleton, collapsing the rig when a clip played.
    let scene = load_scene(path, settings).map_err(|e| format!("{}", e))?;
    let scene_ref: &ufbx::Scene = &scene;

    let mut result = AnimExtractResult {
        written_files: Vec::new(),
        warnings: Vec::new(),
    };

    if scene_ref.anim_stacks.is_empty() {
        result.warnings.push("no animation stacks found".into());
        return Ok(result);
    }

    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("failed to create animations directory: {}", e))?;

    // Sample each stack at a fixed rate. 30 Hz matches the Mixamo default and
    // is dense enough for most gameplay. If the stack has very few keys we
    // still get at least the endpoints.
    let sample_rate: f64 = 30.0;

    // Mixamo and several other tools emit every stack with the same internal
    // name ("mixamo.com"), which makes multiple imports collide. Prefer the
    // source filename stem as the clip name, falling back to the stack name
    // only when we can't read the path, and suffixing when there are several
    // stacks in one file.
    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("clip")
        .to_string();
    let stack_count = scene_ref.anim_stacks.len();

    // ufbx's animation evaluator honors `target_unit_meters` and returns
    // translations in meters, but the mesh/skeleton export in `convert` reads
    // `node.local_transform` verbatim in the source file's units (centimeters
    // for Mixamo). Writing meter-scale translations against a centimeter
    // skeleton collapses every bone offset 100× and crumples the mesh into a
    // blob the moment a clip plays. Rescale animated translations back into the
    // skeleton's unit space so the two agree.
    let unit_fixup = animation_unit_fixup(scene_ref);
    if (unit_fixup - 1.0).abs() > 1e-3 {
        log::info!(
            "[import] {}: scaling animation translations by {:.4} to match skeleton units",
            path.display(),
            unit_fixup
        );
    }

    for (stack_i, stack_ref) in (&scene_ref.anim_stacks).into_iter().enumerate() {
        let clip_name = if stack_count == 1 {
            file_stem.clone()
        } else {
            let inner = stack_ref.element.name.as_ref();
            if inner.is_empty() {
                format!("{}_{}", file_stem, stack_i)
            } else {
                format!("{}_{}", file_stem, inner)
            }
        };

        let duration_f = (stack_ref.time_end - stack_ref.time_begin).max(0.0);
        let n_samples = ((duration_f * sample_rate).ceil() as usize + 1).max(2);
        let dt = if n_samples > 1 {
            duration_f / (n_samples as f64 - 1.0)
        } else {
            0.0
        };

        // Evaluate each bone node at each sample time.
        let anim_ref: &ufbx::Anim = &stack_ref.anim;
        let mut tracks: Vec<BoneTrack> = Vec::new();

        for node in &scene_ref.nodes {
            // Only emit tracks for bones — avoids cluttering the clip with
            // meshes, cameras, etc.
            if node.bone.is_none() {
                continue;
            }
            let name = node.element.name.as_ref();
            if name.is_empty() {
                continue;
            }

            let mut track = BoneTrack {
                bone_name: name.to_string(),
                translations: Vec::new(),
                rotations: Vec::new(),
                scales: Vec::new(),
            };

            for i in 0..n_samples {
                let t = stack_ref.time_begin + dt * i as f64;
                let tr = ufbx::evaluate_transform(anim_ref, node, t);
                let rel_t = t - stack_ref.time_begin;
                track.translations.push((
                    rel_t as f32,
                    [
                        tr.translation.x as f32 * unit_fixup,
                        tr.translation.y as f32 * unit_fixup,
                        tr.translation.z as f32 * unit_fixup,
                    ],
                ));
                track.rotations.push((
                    rel_t as f32,
                    [
                        tr.rotation.x as f32,
                        tr.rotation.y as f32,
                        tr.rotation.z as f32,
                        tr.rotation.w as f32,
                    ],
                ));
                track.scales.push((
                    rel_t as f32,
                    [tr.scale.x as f32, tr.scale.y as f32, tr.scale.z as f32],
                ));
            }

            tracks.push(track);
        }

        if tracks.is_empty() {
            result
                .warnings
                .push(format!("{}: animation stack has no bone tracks", clip_name));
            continue;
        }

        let mut clip = AnimClip {
            name: clip_name.clone(),
            duration: duration_f as f32,
            tracks,
            property_tracks: Vec::new(),
            markers: Vec::new(),
        };
        let dropped = crate::anim_decimate::decimate_clip(&mut clip);
        if dropped > 0 {
            log::info!(
                "[import] decimated {} redundant keys from '{}'",
                dropped,
                clip_name
            );
        }

        let safe_name: String = clip_name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let file_path = output_dir.join(format!("{}.anim", safe_name));
        match write_anim_file(&clip, &file_path) {
            Ok(()) => {
                log::info!(
                    "[import] wrote animation '{}' ({} tracks, {:.2}s) → {}",
                    clip_name,
                    clip.tracks.len(),
                    clip.duration,
                    file_path.display()
                );
                result.written_files.push(file_path.display().to_string());
            }
            Err(e) => {
                result
                    .warnings
                    .push(format!("{}: failed to write .anim: {}", clip_name, e));
            }
        }
    }

    Ok(result)
}

// ─── Internals ─────────────────────────────────────────────────────────────

struct JointOut {
    /// `Element::element_id` of this node — used to key cluster-to-joint lookups.
    element_id: u32,
    name: String,
    parent: Option<usize>,
    translation: [f32; 3],
    rotation: [f32; 4],
    scale: [f32; 3],
    inverse_bind_matrix: [f32; 16],
}

fn collect_joints(scene: &ufbx::Scene) -> Vec<JointOut> {
    // A joint is a node that ufbx has tagged with `bone = Some(_)`. Clusters
    // often reference the mesh node itself as a "bind pose" anchor, so we
    // deliberately do NOT flag cluster targets — that would sweep the mesh
    // into the joint list and corrupt the parent chain.
    //
    // Everything here is keyed by `Element::element_id` (the scene-wide unique
    // ID carried on every ufbx element) rather than by the node's position in
    // `scene.nodes`. The two are not equivalent: element_id is a sparse
    // identifier into `scene.elements`, while the nodes list just happens to
    // hold references. Using element_id keeps cluster-target lookups and
    // parent-walk lookups in the same key space.
    let mut eid_is_joint: std::collections::HashSet<u32> = std::collections::HashSet::new();
    for node in &scene.nodes {
        if node.bone.is_some() {
            eid_is_joint.insert(node.element.element_id);
        }
    }

    let mut eid_to_joint_idx: std::collections::HashMap<u32, usize> =
        std::collections::HashMap::new();
    let mut joints: Vec<JointOut> = Vec::new();
    for node in &scene.nodes {
        let eid = node.element.element_id;
        if !eid_is_joint.contains(&eid) {
            continue;
        }
        let name = (*node.element.name).to_string();
        let t = node.local_transform.translation;
        let r = node.local_transform.rotation;
        let s = node.local_transform.scale;
        let mut ibm = identity_mat4();
        for cluster in &scene.skin_clusters {
            if let Some(bone) = cluster.bone_node.as_ref() {
                if bone.element.element_id == eid {
                    ibm = matrix_to_gltf(&cluster.geometry_to_bone);
                    break;
                }
            }
        }
        eid_to_joint_idx.insert(eid, joints.len());
        joints.push(JointOut {
            element_id: eid,
            name,
            parent: None,
            translation: [t.x as f32, t.y as f32, t.z as f32],
            rotation: [r.x as f32, r.y as f32, r.z as f32, r.w as f32],
            scale: [s.x as f32, s.y as f32, s.z as f32],
            inverse_bind_matrix: ibm,
        });
    }

    // Link parents — walk up each joint's original parent chain until we hit
    // another joint. Non-joint helper nodes between bones are skipped.
    // We need to find each joint's Node again via element_id; build a lookup.
    let mut eid_to_node: std::collections::HashMap<u32, &ufbx::Node> =
        std::collections::HashMap::new();
    for node in &scene.nodes {
        eid_to_node.insert(node.element.element_id, node);
    }

    for joint in joints.iter_mut() {
        let eid = joint.element_id;
        let node = match eid_to_node.get(&eid) {
            Some(n) => *n,
            None => continue,
        };
        let mut walker: Option<&ufbx::Node> = node.parent.as_ref().map(|p| -> &ufbx::Node { p });
        while let Some(parent) = walker {
            let pid = parent.element.element_id;
            if let Some(&pji) = eid_to_joint_idx.get(&pid) {
                joint.parent = Some(pji);
                break;
            }
            walker = parent.parent.as_ref().map(|p| -> &ufbx::Node { p });
        }
    }

    joints
}

/// Append one instance's worth of geometry, with the node placement baked into
/// the vertices.
///
/// `positions` / `normals` are the mesh's geometry-space attributes; the
/// transformed copies are pushed onto the merged output buffers.
fn bake_placement(
    placement: &ufbx::Matrix,
    positions: &[f32],
    normals: &[f32],
    out_positions: &mut Vec<f32>,
    out_normals: &mut Vec<f32>,
) {
    // Normals take the inverse-transpose, not the matrix itself, or a node's
    // non-uniform scale shears them off the surface and every light on the
    // model reads wrong.
    let normal_matrix = ufbx::matrix_for_normals(placement);

    for v in 0..positions.len() / 3 {
        let p = ufbx::transform_position(
            placement,
            ufbx::Vec3 {
                x: positions[v * 3] as ufbx::Real,
                y: positions[v * 3 + 1] as ufbx::Real,
                z: positions[v * 3 + 2] as ufbx::Real,
            },
        );
        out_positions.push(p.x as f32);
        out_positions.push(p.y as f32);
        out_positions.push(p.z as f32);

        let n = ufbx::transform_direction(
            &normal_matrix,
            ufbx::Vec3 {
                x: normals[v * 3] as ufbx::Real,
                y: normals[v * 3 + 1] as ufbx::Real,
                z: normals[v * 3 + 2] as ufbx::Real,
            },
        );
        let len = (n.x * n.x + n.y * n.y + n.z * n.z).sqrt();
        let inv = if len > 1e-12 { 1.0 / len } else { 0.0 };
        out_normals.push((n.x * inv) as f32);
        out_normals.push((n.y * inv) as f32);
        out_normals.push((n.z * inv) as f32);
    }
}

/// Append a mesh's zero-based triangle list rebased onto `base_vertex`, sorting
/// each triangle into its material's bucket and reversing winding when the
/// placement mirrors.
fn append_indices(
    tri_indices: &[u32],
    tri_bucket: &[usize],
    base_vertex: u32,
    mirrored: bool,
    buckets: &mut [Vec<u32>],
) {
    for (tri, bucket) in tri_indices.chunks_exact(3).zip(tri_bucket) {
        let Some(out) = buckets.get_mut(*bucket) else {
            continue;
        };
        if mirrored {
            out.push(tri[0] + base_vertex);
            out.push(tri[2] + base_vertex);
            out.push(tri[1] + base_vertex);
        } else {
            out.push(tri[0] + base_vertex);
            out.push(tri[1] + base_vertex);
            out.push(tri[2] + base_vertex);
        }
    }
}

fn identity_mat4() -> [f32; 16] {
    let mut m = [0.0f32; 16];
    m[0] = 1.0;
    m[5] = 1.0;
    m[10] = 1.0;
    m[15] = 1.0;
    m
}

/// Convert ufbx's 3×4 affine matrix into a GLTF 4×4 column-major mat4.
fn matrix_to_gltf(m: &ufbx::Matrix) -> [f32; 16] {
    // GLTF column-major. ufbx stores mXY where X=row, Y=column, and
    // m03/m13/m23 are the translation column.
    [
        m.m00 as f32,
        m.m10 as f32,
        m.m20 as f32,
        0.0,
        m.m01 as f32,
        m.m11 as f32,
        m.m21 as f32,
        0.0,
        m.m02 as f32,
        m.m12 as f32,
        m.m22 as f32,
        0.0,
        m.m03 as f32,
        m.m13 as f32,
        m.m23 as f32,
        1.0,
    ]
}

fn load_scene(path: &Path, settings: &ImportSettings) -> Result<ufbx::SceneRoot, ImportError> {
    // Normalize everything to a right-handed, Y-up, meters coordinate system
    // so downstream code doesn't have to guess. ufbx applies unit scaling and
    // axis conversion to both meshes and bone transforms consistently.
    let opts = ufbx::LoadOpts {
        target_axes: ufbx::CoordinateAxes::right_handed_y_up(),
        target_unit_meters: settings.scale as ufbx::Real,
        space_conversion: ufbx::SpaceConversion::ModifyGeometry,
        generate_missing_normals: settings.generate_normals,
        ..Default::default()
    };

    let path_str = path
        .to_str()
        .ok_or_else(|| ImportError::ParseError("non-utf8 FBX path".into()))?;
    ufbx::load_file(path_str, opts)
        .map_err(|e| ImportError::ParseError(format!("ufbx load failed: {}", &*e.description)))
}

// ─── Texture + material extraction ─────────────────────────────────────────

/// Find the image file an external texture reference points at.
///
/// FBX stores three variants of the path, and none of them is reliable on its
/// own: `absolute_filename` is the exporting machine's layout (`D:\work\...`
/// on someone else's disk), while `relative_filename` is relative to wherever
/// the FBX lived at export time. Worse, both use whichever separator the
/// exporting OS preferred, so a Windows-authored path has to be re-split before
/// a Unix host can join it.
///
/// So: try the absolute path as given (right when the model never moved), then
/// the relative path resolved against the FBX's own directory (right for the
/// usual `Model.fbx` + `Textures/` layout), then the bare filename in the
/// directory itself and in the conventional `textures/` subfolder.
fn resolve_external_texture(tex: &ufbx::Texture, source_path: &Path) -> Option<std::path::PathBuf> {
    let dir = source_path.parent()?;

    let absolute = tex.absolute_filename.as_ref();
    if !absolute.is_empty() {
        let path = Path::new(absolute);
        if path.is_file() {
            return Some(path.to_path_buf());
        }
    }

    // Re-split on both separators so a path authored on either OS resolves on
    // this one, and drop any leading `./`.
    fn split(raw: &str) -> Vec<&str> {
        raw.split(['\\', '/'])
            .filter(|seg| !seg.is_empty() && *seg != ".")
            .collect()
    }

    for raw in [tex.relative_filename.as_ref(), tex.filename.as_ref()] {
        let segments = split(raw);
        if segments.is_empty() {
            continue;
        }
        // Walk `..` off the front rather than feeding it to `join`, which would
        // otherwise produce a path that only canonicalizes correctly by luck.
        let mut candidate = dir.to_path_buf();
        for segment in &segments {
            if *segment == ".." {
                candidate.pop();
            } else {
                candidate.push(segment);
            }
        }
        if candidate.is_file() {
            return Some(candidate);
        }

        // The relative path's folders may not match how the files were shipped;
        // fall back to the basename in the obvious places.
        let file_name = segments[segments.len() - 1];
        for base in [dir.to_path_buf(), dir.join("textures"), dir.join("Textures")] {
            let candidate = base.join(file_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

/// Walk `scene.textures` + `scene.materials`, producing the [`MaterialBundle`]
/// the GLB builder writes. Textures are recorded by absolute path (or embedded
/// bytes); processing them is `gltf_pass::finish_converted_glb`'s job.
fn collect_materials(
    scene: &ufbx::Scene,
    source_path: &Path,
    warnings: &mut Vec<String>,
) -> crate::glb_build::MaterialBundle {
    use crate::glb_build::{MaterialBundle, PbrMaterialDef, TextureRef};
    use std::collections::HashMap;

    let mut bundle = MaterialBundle::default();
    // texture element_id → index into bundle.textures.
    let mut tex_index: HashMap<u32, usize> = HashMap::new();
    let mut missing: Vec<String> = Vec::new();

    for tex in &scene.textures {
        // Embedded data lives either on the texture itself or its linked Video.
        let embedded: Option<Vec<u8>> = if !tex.content.is_empty() {
            Some(tex.content.to_vec())
        } else {
            tex.video
                .as_ref()
                .map(|video| video.content.to_vec())
                .filter(|content| !content.is_empty())
        };

        // Most real-world FBX doesn't embed anything — it points at image files
        // sitting beside it. Skipping those (which is what we used to do) means
        // a scene like Bistro, whose 405 textures are all external `.dds`,
        // imports with every material's texture slot empty and every surface
        // rendering flat white.
        //
        // Locating the file is the format-specific part and stops here; the
        // absolute path goes into the GLB as the image's `uri` and the shared
        // pass decides what to do with it.
        let uri = match &embedded {
            Some(_) => format!("fbx-embedded://{}", tex.element.element_id),
            None => match resolve_external_texture(tex, source_path) {
                Some(path) => path.to_string_lossy().into_owned(),
                None => {
                    if tex.has_file {
                        missing.push((*tex.filename).to_string());
                    }
                    continue;
                }
            },
        };

        tex_index.insert(tex.element.element_id, bundle.textures.len());
        bundle.textures.push(TextureRef { uri, embedded });
    }

    // One warning for the lot: a model with a missing texture folder would
    // otherwise emit hundreds of near-identical lines.
    if !missing.is_empty() {
        warnings.push(format!(
            "{} texture file(s) referenced by the FBX were not found next to it (first: {})",
            missing.len(),
            missing[0]
        ));
    }

    // Resolve the texture bound to one of ufbx's normalized PBR channels to an
    // index into the textures we extracted above. ufbx maps legacy FBX Phong
    // slots onto this PBR view — e.g. `DiffuseColor → base_color`,
    // `EmissiveColor → emission_color`, `TransparentColor → opacity`,
    // `SpecularColor → specular_color`, `Bump`/`NormalMap → normal_map` — so a
    // single code path covers both modern StingrayPBS and legacy materials.
    let tex_of = |map: &ufbx::MaterialMap| -> Option<usize> {
        map.texture
            .as_ref()
            .and_then(|t| tex_index.get(&t.element.element_id).copied())
    };

    for mat in &scene.materials {
        let pbr = &mat.pbr;

        let base_color_factor = if pbr.base_color.has_value {
            let v = pbr.base_color.value_vec4;
            [v.x as f32, v.y as f32, v.z as f32, v.w as f32]
        } else {
            [1.0, 1.0, 1.0, 1.0]
        };

        let base_color_texture = tex_of(&pbr.base_color);
        let normal_texture = tex_of(&pbr.normal_map);
        let emissive_texture = tex_of(&pbr.emission_color);
        let occlusion_texture = tex_of(&pbr.ambient_occlusion);
        let opacity_texture = tex_of(&pbr.opacity);
        // Prefer the specular color map; fall back to the scalar specular
        // factor map if that's where the reflectivity mask is bound.
        let specular_texture = tex_of(&pbr.specular_color).or_else(|| tex_of(&pbr.specular_factor));

        // Emissive factor: emission_color × emission_factor (so night-side
        // city lights etc. carry their authored intensity even with a texture).
        let emissive = if pbr.emission_color.has_value {
            let c = pbr.emission_color.value_vec4;
            let f = if pbr.emission_factor.has_value {
                pbr.emission_factor.value_vec4.x as f32
            } else {
                1.0
            };
            [c.x as f32 * f, c.y as f32 * f, c.z as f32 * f]
        } else {
            [0.0, 0.0, 0.0]
        };

        // How this material handles transparency, and whether it is two-sided.
        //
        // FBX has no direct equivalent of glTF's `alphaMode`, so it has to be
        // inferred. Three signals, in order of confidence:
        //
        // 1. A dedicated opacity map, or a constant opacity below 1 — genuinely
        //    blended (glass, a fading decal). ufbx normalizes transparency so
        //    opacity = 1 means fully opaque.
        // 2. A base-colour texture stored in an alpha-capable format. This is
        //    what foliage is: the leaf shape lives in the alpha channel and the
        //    material must be alpha-*tested*, not blended. A false positive is
        //    harmless — a `MASK` material whose alpha is 1 everywhere renders
        //    exactly like an opaque one.
        // 3. The name. Exporters carry conventions FBX itself cannot express;
        //    `.DoubleSided` and `_MASKED` are the two this asset set uses, and
        //    they are worth honouring because nothing else recovers them.
        let opacity_below_one =
            pbr.opacity.has_value && (pbr.opacity.value_vec4.x as f32) < 0.999;
        let lower = mat.element.name.to_lowercase();
        let named_masked = lower.contains("masked");
        let base_has_alpha = base_color_texture
            .and_then(|i| bundle.textures.get(i))
            .is_some_and(|t| texture_may_have_alpha(std::path::Path::new(&t.uri)));

        let alpha = if opacity_texture.is_some() || opacity_below_one {
            crate::glb_build::AlphaKind::Blend
        } else if named_masked || base_has_alpha {
            crate::glb_build::AlphaKind::Mask(0.5)
        } else {
            crate::glb_build::AlphaKind::Opaque
        };
        let double_sided = lower.contains("doublesided") || lower.contains("double_sided");

        let metallic = if pbr.metalness.has_value {
            pbr.metalness.value_vec4.x as f32
        } else {
            0.0
        };
        let roughness = if pbr.roughness.has_value {
            pbr.roughness.value_vec4.x as f32
        } else {
            0.8
        };

        // Extended PBR channels (modern StingrayPBS / Arnold / glTF FBX).
        //
        // These are only meaningful when the source material actually is a PBR
        // shader. ufbx presents *every* material through its normalized PBR
        // view, which means a legacy Phong material's slots get filled from
        // whatever Phong property is closest — and the mapping is not one the
        // values survive. `TransparencyFactor` lands in `transmission_factor`,
        // and in Phong convention transparency is `TransparentColor *
        // TransparencyFactor`, so the near-universal "black transparent colour,
        // factor 1.0" spelling of *opaque* reads back as transmission 1.0.
        //
        // Taking that at face value turned all 132 materials of a building
        // exterior into fully transmissive glass: the whole scene rendered
        // milky with the sky bleeding through it, and signage read mirrored
        // because you were seeing the back face of each surface through its own
        // transparent front.
        //
        // So gate the extended channels on the shader type. A legacy Phong or
        // Lambert material has no clearcoat, no transmission and no anisotropy
        // to describe; it gets the glTF-spec defaults, and its transparency
        // comes from `opacity` below like it should.
        let is_pbr_shader = !matches!(
            mat.shader_type,
            ufbx::ShaderType::FbxLambert
                | ufbx::ShaderType::FbxPhong
                | ufbx::ShaderType::BlenderPhong
                | ufbx::ShaderType::WavefrontMtl
                | ufbx::ShaderType::Unknown
        );
        let val = |m: &ufbx::MaterialMap, default: f32| -> f32 {
            if is_pbr_shader && m.has_value {
                m.value_vec4.x as f32
            } else {
                default
            }
        };
        let adv_uri = |m: &ufbx::MaterialMap| -> Option<String> {
            if !is_pbr_shader {
                return None;
            }
            tex_of(m).and_then(|i| bundle.textures.get(i).map(|t| t.uri.clone()))
        };
        let advanced = renzora::core::PbrAdvanced {
            clearcoat: val(&pbr.coat_factor, 0.0),
            clearcoat_roughness: val(&pbr.coat_roughness, 0.0),
            clearcoat_texture: adv_uri(&pbr.coat_factor),
            clearcoat_roughness_texture: adv_uri(&pbr.coat_roughness),
            clearcoat_normal_texture: adv_uri(&pbr.coat_normal),
            specular_transmission: val(&pbr.transmission_factor, 0.0),
            transmission_texture: adv_uri(&pbr.transmission_factor),
            diffuse_transmission: 0.0,
            thickness: 0.0,
            thickness_texture: None,
            ior: val(&pbr.specular_ior, 1.5),
            attenuation_distance: 1.0e37,
            attenuation_color: [1.0, 1.0, 1.0],
            anisotropy_strength: val(&pbr.specular_anisotropy, 0.0),
            anisotropy_rotation: val(&pbr.specular_rotation, 0.0),
            anisotropy_texture: None,
            reflectance: 0.5,
            unlit: false,
        };

        bundle.materials.push(PbrMaterialDef {
            name: (*mat.element.name).to_string(),
            base_color: base_color_factor,
            base_color_texture,
            normal_texture,
            metallic,
            roughness,
            emissive,
            emissive_texture,
            occlusion_texture,
            opacity_texture,
            specular_texture,
            roughness_texture: None,
            metallic_texture: None,
            alpha,
            double_sided,
            advanced,
        });
    }

    bundle
}

/// Whether an image file *can* carry per-pixel alpha, judged from its container.
///
/// Deliberately a capability test, not a content test: opening and scanning
/// every base-colour map would cost minutes on a scene-sized import, and the
/// consequence of a false positive is nil — a `MASK` material whose alpha is 1
/// everywhere renders identically to an opaque one. A false *negative* is the
/// expensive mistake, because it is a tree rendered as a box.
fn texture_may_have_alpha(path: &std::path::Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    match ext.to_ascii_lowercase().as_str() {
        // Always carry an alpha channel.
        "png" | "tga" | "tif" | "tiff" | "webp" => true,
        // Depends on the block format: DXT1 is opaque (or 1-bit), DXT3/DXT5
        // and the BC7/BC3 DX10 formats carry full alpha.
        "dds" => std::fs::read(path)
            .ok()
            .and_then(|b| renzora_rmip::dds::probe(&b, false).ok())
            .is_some_and(|d| d.has_alpha()),
        // JPEG has no alpha; anything unrecognized is assumed not to.
        _ => false,
    }
}

/// Convert an FBX texture coordinate to the glTF convention.
///
/// FBX stores UVs with the origin at the **bottom** left; glTF — and so Bevy —
/// samples from the top left. Flipping V is part of the format conversion, not
/// a user preference: without it every texture is upside down, which tiling
/// materials hide and anything with orientation (signage, decals, labels) does
/// not. Godot's importer, built on the same ufbx library, flips unconditionally
/// for the same reason.
///
/// `flip` is the user's own extra flip, applied on top of the correct result
/// for the occasional asset authored against the other convention.
fn fbx_uv(uv: [f32; 2], flip: bool) -> [f32; 2] {
    let v = 1.0 - uv[1];
    [uv[0], if flip { 1.0 - v } else { v }]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Column-major-ish helper: ufbx stores `mXY` with X=row, Y=column, and
    /// m03/m13/m23 as the translation column.
    fn matrix(rows: [[f64; 4]; 3]) -> ufbx::Matrix {
        ufbx::Matrix {
            m00: rows[0][0],
            m01: rows[0][1],
            m02: rows[0][2],
            m03: rows[0][3],
            m10: rows[1][0],
            m11: rows[1][1],
            m12: rows[1][2],
            m13: rows[1][3],
            m20: rows[2][0],
            m21: rows[2][1],
            m22: rows[2][2],
            m23: rows[2][3],
        }
    }

    // ─── Placement baking ───────────────────────────────────────────────

    #[test]
    fn bake_placement_applies_scale_and_axis_swap() {
        // The Bistro case in miniature: a centimetre, Z-up node placement.
        // 0.01 scale plus the Z-up → Y-up rotation (Y_out = Z_in,
        // Z_out = -Y_in), and a 2 m offset along X.
        let m = matrix([
            [0.01, 0.0, 0.0, 2.0],
            [0.0, 0.0, 0.01, 0.0],
            [0.0, -0.01, 0.0, 0.0],
        ]);
        // One vertex 300 cm "up" in the source file's Z.
        let positions = [0.0f32, 0.0, 300.0];
        let normals = [0.0f32, 0.0, 1.0];
        let mut out_positions = Vec::new();
        let mut out_normals = Vec::new();

        bake_placement(&m, &positions, &normals, &mut out_positions, &mut out_normals);

        assert_eq!(out_positions.len(), 3);
        assert!((out_positions[0] - 2.0).abs() < 1e-5, "{out_positions:?}");
        // 300 cm of source Z lands as 3 m of Y.
        assert!((out_positions[1] - 3.0).abs() < 1e-5, "{out_positions:?}");
        assert!(out_positions[2].abs() < 1e-5, "{out_positions:?}");
        // The normal follows the same rotation and stays unit length.
        assert!((out_normals[1] - 1.0).abs() < 1e-5, "{out_normals:?}");
        let len = (out_normals[0] * out_normals[0]
            + out_normals[1] * out_normals[1]
            + out_normals[2] * out_normals[2])
            .sqrt();
        assert!((len - 1.0).abs() < 1e-5, "normal not normalized: {len}");
    }

    #[test]
    fn bake_placement_renormalizes_under_non_uniform_scale() {
        // A node squashed on Y: the naive path would leave the normal
        // non-unit and lit wrong.
        let m = matrix([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 0.25, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ]);
        let positions = [0.0f32, 1.0, 0.0];
        let inv_sqrt2 = std::f32::consts::FRAC_1_SQRT_2;
        let normals = [inv_sqrt2, inv_sqrt2, 0.0];
        let mut out_positions = Vec::new();
        let mut out_normals = Vec::new();

        bake_placement(&m, &positions, &normals, &mut out_positions, &mut out_normals);

        let len = (out_normals[0] * out_normals[0]
            + out_normals[1] * out_normals[1]
            + out_normals[2] * out_normals[2])
            .sqrt();
        assert!((len - 1.0).abs() < 1e-5, "normal not normalized: {len}");
        // Inverse-transpose: the squashed axis gets *more* normal, not less.
        assert!(
            out_normals[1] > out_normals[0],
            "expected Y-dominant normal, got {out_normals:?}"
        );
    }

    // ─── Index rebasing + winding ───────────────────────────────────────

    #[test]
    fn append_indices_rebases_onto_base_vertex() {
        let mut buckets = vec![Vec::new()];
        append_indices(&[0, 1, 2, 2, 1, 3], &[0, 0], 10, false, &mut buckets);
        assert_eq!(buckets[0], vec![10, 11, 12, 12, 11, 13]);
    }

    #[test]
    fn append_indices_flips_winding_when_mirrored() {
        let mut buckets = vec![Vec::new()];
        append_indices(&[0, 1, 2], &[0], 0, true, &mut buckets);
        assert_eq!(buckets[0], vec![0, 2, 1]);
    }

    #[test]
    fn append_indices_splits_triangles_by_material() {
        // Two triangles wearing different materials must land in different
        // buckets, since a GLTF primitive carries exactly one material.
        let mut buckets = vec![Vec::new(), Vec::new(), Vec::new()];
        append_indices(&[0, 1, 2, 3, 4, 5], &[2, 0], 0, false, &mut buckets);
        assert_eq!(buckets[2], vec![0, 1, 2]);
        assert_eq!(buckets[0], vec![3, 4, 5]);
        assert!(buckets[1].is_empty());
    }

    #[test]
    fn append_indices_drops_triangles_with_an_unknown_bucket() {
        // Defensive: an out-of-range bucket must not panic the import.
        let mut buckets = vec![Vec::new()];
        append_indices(&[0, 1, 2], &[7], 0, false, &mut buckets);
        assert!(buckets[0].is_empty());
    }

    #[test]
    fn mirrored_placements_are_detected_by_determinant() {
        let mirrored = matrix([
            [-1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ]);
        let plain = matrix([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ]);
        assert!(ufbx::matrix_determinant(&mirrored) < 0.0);
        assert!(ufbx::matrix_determinant(&plain) > 0.0);
    }

    #[test]
    fn fbx_uv_flips_v_by_default() {
        // The bottom of an FBX texture (v = 0) is the top in glTF.
        let out = fbx_uv([0.25, 0.0], false);
        assert!((out[0] - 0.25).abs() < 1e-6, "U is untouched");
        assert!((out[1] - 1.0).abs() < 1e-6, "V is flipped");
    }

    #[test]
    fn fbx_uv_user_flip_undoes_the_conversion() {
        let out = fbx_uv([0.25, 0.0], true);
        assert!((out[1] - 0.0).abs() < 1e-6, "the user flip is applied on top");
    }

    #[test]
    fn fbx_uv_midpoint_is_stable_either_way() {
        assert!((fbx_uv([0.5, 0.5], false)[1] - 0.5).abs() < 1e-6);
        assert!((fbx_uv([0.5, 0.5], true)[1] - 0.5).abs() < 1e-6);
    }
}
