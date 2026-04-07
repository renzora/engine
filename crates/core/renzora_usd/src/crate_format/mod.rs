//! USDC (binary Crate format) parser.
//!
//! Parses the Pixar USD binary container format. The format consists of:
//! 1. Header (magic, version, TOC offset)
//! 2. Token table (deduplicated strings, optionally LZ4-compressed)
//! 3. String table (indices into token table)
//! 4. Field table (key-value pairs with typed values)
//! 5. FieldSet table (groups of fields forming a "property sheet")
//! 6. Path table (scene hierarchy as a flattened tree)
//! 7. Spec table (associates paths with types and fieldsets)

mod header;
mod sections;
mod tokens;
mod paths;
mod specs;
mod fields;
mod values;
mod compression;

use crate::scene::*;
use crate::UsdResult;

pub use values::Value;

/// Parse USDC binary data into a UsdStage.
pub fn parse(data: &[u8]) -> UsdResult<UsdStage> {
    let header = header::Header::read(data)?;
    let toc = sections::TableOfContents::read(data, &header)?;

    let tokens = tokens::read_tokens(data, &toc)?;
    log::warn!("USDC: read {} tokens", tokens.len());
    let strings = sections::read_string_indices(data, &toc)?;
    log::warn!("USDC: read {} strings", strings.len());
    let fields = fields::read_fields(data, &toc, &tokens)?;
    log::warn!("USDC: read {} fields", fields.len());
    let field_sets = fields::read_field_sets(data, &toc)?;
    log::warn!("USDC: read {} fieldset entries", field_sets.len());
    let paths = paths::read_paths(data, &toc, &tokens)?;
    log::warn!("USDC: read {} paths", paths.len());
    let specs = specs::read_specs(data, &toc)?;
    log::warn!("USDC: read {} specs", specs.len());

    log::warn!("USDC: {} tokens, {} fields, {} fieldset entries, {} paths, {} specs",
        tokens.len(), fields.len(), field_sets.len(), paths.len(), specs.len());

    let mut stage = UsdStage {
        meters_per_unit: 0.01,
        time_codes_per_second: 24.0,
        ..Default::default()
    };

    // Build the scene by walking specs and resolving field sets
    let resolver = Resolver {
        tokens: &tokens,
        strings: &strings,
        fields: &fields,
        field_sets: &field_sets,
        paths: &paths,
        specs: &specs,
        data,
        toc: &toc,
    };

    resolver.build_stage(&mut stage)?;

    Ok(stage)
}

/// Internal resolver that walks the spec table and builds the scene.
struct Resolver<'a> {
    tokens: &'a [String],
    strings: &'a [u32],
    fields: &'a [fields::Field],
    field_sets: &'a [u32],
    paths: &'a [paths::PathEntry],
    specs: &'a [specs::Spec],
    data: &'a [u8],
    toc: &'a sections::TableOfContents,
}

impl<'a> Resolver<'a> {
    fn build_stage(&self, stage: &mut UsdStage) -> UsdResult<()> {
        let mut prim_count = 0u32;
        let mut attr_count = 0u32;

        // Log raw spec data for diagnostics
        for (i, spec) in self.specs.iter().enumerate().take(15) {
            let path_name = if (spec.path_index as usize) < self.paths.len() {
                &self.paths[spec.path_index as usize].name
            } else {
                "??"
            };
            log::warn!("  raw spec[{}]: path_idx={} name='{}' fieldset_idx={} spec_type={}",
                i, spec.path_index, path_name, spec.fieldset_index, spec.spec_type);
        }

        // Walk all specs and extract prims based on their type
        for spec in self.specs {
            if (spec.path_index as usize) >= self.paths.len() {
                continue;
            }

            let full_path = self.resolve_path(spec.path_index);

            // Only process prim specs (type 1), not property specs
            if spec.spec_type != specs::SPEC_TYPE_PRIM {
                if spec.spec_type == specs::SPEC_TYPE_ATTRIBUTE {
                    attr_count += 1;
                }
                continue;
            }

            prim_count += 1;
            let field_values = self.get_field_set(spec.fieldset_index);

            // Determine prim type from the "typeName" field
            let type_name = field_values
                .iter()
                .find(|(k, _)| k == "typeName")
                .and_then(|(_, v)| v.as_token())
                .unwrap_or("");

            log::warn!("USDC prim: '{}' type='{}' fields={} fieldset_idx={}",
                full_path, type_name, field_values.len(), spec.fieldset_index);

            match type_name {
                "Mesh" => {
                    if let Ok(mesh) = self.extract_mesh(&full_path, &field_values, spec) {
                        let idx = stage.meshes.len();
                        stage.meshes.push(mesh);
                        self.insert_node(stage, &full_path, NodeData::Mesh(idx));
                    }
                }
                "Material" => {
                    if let Ok(mat) = self.extract_material(&full_path, &field_values) {
                        stage.materials.push(mat);
                    }
                }
                "Skeleton" | "SkelRoot" => {
                    if let Ok(skel) = self.extract_skeleton(&full_path, &field_values) {
                        let idx = stage.skeletons.len();
                        stage.skeletons.push(skel);
                        self.insert_node(stage, &full_path, NodeData::Skeleton(idx));
                    }
                }
                "SkelAnimation" => {
                    if let Ok(anim) = self.extract_animation(&full_path, &field_values) {
                        stage.animations.push(anim);
                    }
                }
                "DistantLight" | "SphereLight" | "RectLight" | "DiskLight" | "DomeLight" => {
                    if let Ok(light) = self.extract_light(&full_path, type_name, &field_values) {
                        let idx = stage.lights.len();
                        stage.lights.push(light);
                        self.insert_node(stage, &full_path, NodeData::Light(idx));
                    }
                }
                "Camera" => {
                    if let Ok(cam) = self.extract_camera(&full_path, &field_values) {
                        let idx = stage.cameras.len();
                        stage.cameras.push(cam);
                        self.insert_node(stage, &full_path, NodeData::Camera(idx));
                    }
                }
                "Xform" | "Scope" | "" => {
                    // Xform/Scope nodes: just add to the scene graph with their transform
                    self.insert_node(stage, &full_path, NodeData::Empty);
                }
                "GeomSubset" => {
                    // Handled during mesh extraction
                }
                _ => {
                    // Unknown prim type — add as empty node
                    self.insert_node(stage, &full_path, NodeData::Empty);
                }
            }
        }

        log::warn!("USDC summary: {} prims, {} attributes, {} meshes found",
            prim_count, attr_count, stage.meshes.len());

        // Extract stage metadata from the pseudo-root
        self.extract_stage_metadata(stage);

        // Resolve material bindings
        self.resolve_material_bindings(stage);

        Ok(())
    }

    fn get_field_set(&self, fieldset_index: u32) -> Vec<(String, Value)> {
        let mut result = Vec::new();
        let idx = fieldset_index as usize;

        if idx >= self.field_sets.len() {
            return result;
        }

        // Walk field set entries until we hit a sentinel (u32::MAX)
        let mut i = idx;
        while i < self.field_sets.len() {
            let field_idx = self.field_sets[i];
            if field_idx == u32::MAX {
                break;
            }
            if let Some(field) = self.fields.get(field_idx as usize) {
                let name = if (field.token_index as usize) < self.tokens.len() {
                    self.tokens[field.token_index as usize].clone()
                } else {
                    String::new()
                };
                result.push((name, field.value.clone()));
            }
            i += 1;
        }

        result
    }

    fn resolve_path(&self, path_index: u32) -> String {
        let idx = path_index as usize;
        if idx >= self.paths.len() {
            return String::new();
        }
        let entry = &self.paths[idx];

        if entry.parent_index < 0 {
            // Root
            return format!("/{}", entry.name);
        }

        let parent = self.resolve_path(entry.parent_index as u32);
        if parent == "/" {
            format!("/{}", entry.name)
        } else {
            format!("{}/{}", parent, entry.name)
        }
    }

    fn insert_node(&self, stage: &mut UsdStage, path: &str, data: NodeData) {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            return;
        }

        let mut current = &mut stage.root;
        for (i, part) in parts.iter().enumerate() {
            let is_last = i == parts.len() - 1;

            let child_idx = current.children.iter().position(|c| c.name == *part);

            if let Some(idx) = child_idx {
                if is_last {
                    current.children[idx].data = data.clone();
                    // Try to extract transform for this node
                    if let Some(xform) = self.find_xform(path) {
                        current.children[idx].transform = xform;
                    }
                    return;
                }
                current = &mut current.children[idx];
            } else {
                let mut node = UsdNode {
                    name: part.to_string(),
                    path: parts[..=i].join("/"),
                    transform: identity_matrix(),
                    data: if is_last { data.clone() } else { NodeData::Empty },
                    children: Vec::new(),
                };
                if is_last {
                    if let Some(xform) = self.find_xform(path) {
                        node.transform = xform;
                    }
                }
                current.children.push(node);
                if is_last {
                    return;
                }
                let last = current.children.len() - 1;
                current = &mut current.children[last];
            }
        }
    }

    fn find_xform(&self, path: &str) -> Option<[f32; 16]> {
        // Find the spec for this path and look for xformOp:transform or xformOpOrder
        for spec in self.specs {
            if spec.spec_type != specs::SPEC_TYPE_PRIM {
                continue;
            }
            let resolved = self.resolve_path(spec.path_index);
            if resolved != path {
                continue;
            }

            let fields = self.get_field_set(spec.fieldset_index);
            return crate::xform::extract_transform(&fields);
        }
        None
    }

    fn extract_mesh(
        &self,
        path: &str,
        fields: &[(String, Value)],
        _spec: &specs::Spec,
    ) -> UsdResult<UsdMesh> {
        let name = path.rsplit('/').next().unwrap_or("Mesh").to_string();
        let mut mesh = UsdMesh {
            name,
            path: path.to_string(),
            ..Default::default()
        };

        for (key, value) in fields {
            match key.as_str() {
                "points" => {
                    if let Some(pts) = value.as_vec3f_array() {
                        mesh.positions = pts;
                    }
                }
                "normals" => {
                    if let Some(n) = value.as_vec3f_array() {
                        mesh.normals = n;
                    }
                }
                "faceVertexCounts" => {
                    if let Some(c) = value.as_int_array() {
                        mesh.face_vertex_counts = c.iter().map(|&i| i as u32).collect();
                    }
                }
                "faceVertexIndices" => {
                    if let Some(c) = value.as_int_array() {
                        mesh.face_vertex_indices = c.iter().map(|&i| i as u32).collect();
                    }
                }
                "subdivisionScheme" => {
                    if let Some(s) = value.as_token() {
                        mesh.subdivision_scheme = s.to_string();
                    }
                }
                _ => {}
            }
        }

        // Look for primvar properties (UVs, colors) in child property specs
        self.extract_mesh_primvars(path, &mut mesh);

        // Look for material binding
        self.extract_material_binding(path, &mut mesh);

        // Extract GeomSubsets
        self.extract_geom_subsets(path, &mut mesh);

        // Extract skinning data
        self.extract_skin_binding(path, &mut mesh);

        Ok(mesh)
    }

    fn extract_mesh_primvars(&self, mesh_path: &str, mesh: &mut UsdMesh) {
        for spec in self.specs {
            if spec.spec_type != specs::SPEC_TYPE_ATTRIBUTE {
                continue;
            }
            let prop_path = self.resolve_path(spec.path_index);
            if !prop_path.starts_with(mesh_path) || prop_path == mesh_path {
                continue;
            }

            let prop_name = prop_path.rsplit('/').next().unwrap_or("");
            let fields = self.get_field_set(spec.fieldset_index);

            // UV sets
            if prop_name.starts_with("primvars:st") || prop_name.contains("texCoord") {
                let uv_name = if prop_name == "primvars:st" {
                    "st".to_string()
                } else {
                    prop_name.replace("primvars:", "")
                };

                for (key, value) in &fields {
                    if key == "default" || key == "timeSamples" {
                        if let Some(uvs) = value.as_vec2f_array() {
                            mesh.uv_sets.insert(uv_name.clone(), uvs);
                        }
                    }
                }
            }

            // Vertex colors
            if prop_name == "primvars:displayColor" {
                for (key, value) in &fields {
                    if key == "default" || key == "timeSamples" {
                        if let Some(colors) = value.as_vec3f_array() {
                            mesh.colors = colors
                                .iter()
                                .map(|c| [c[0], c[1], c[2], 1.0])
                                .collect();
                        }
                    }
                }
            }

            // Normals (can also be a property)
            if prop_name == "normals" {
                for (key, value) in &fields {
                    if key == "default" || key == "timeSamples" {
                        if let Some(n) = value.as_vec3f_array() {
                            if mesh.normals.is_empty() {
                                mesh.normals = n;
                            }
                        }
                    }
                }
            }
        }
    }

    fn extract_material_binding(&self, prim_path: &str, mesh: &mut UsdMesh) {
        for spec in self.specs {
            if spec.spec_type != specs::SPEC_TYPE_RELATIONSHIP {
                continue;
            }
            let prop_path = self.resolve_path(spec.path_index);
            // material:binding relationship
            if prop_path == format!("{}/material:binding", prim_path) {
                let fields = self.get_field_set(spec.fieldset_index);
                for (key, value) in &fields {
                    if key == "targetPaths" || key == "default" {
                        if let Some(path_str) = value.as_path_or_token() {
                            mesh.material_binding = Some(path_str);
                        }
                    }
                }
            }
        }
    }

    fn extract_geom_subsets(&self, mesh_path: &str, mesh: &mut UsdMesh) {
        for spec in self.specs {
            if spec.spec_type != specs::SPEC_TYPE_PRIM {
                continue;
            }
            let path = self.resolve_path(spec.path_index);
            // GeomSubsets are direct children of the mesh
            if !path.starts_with(mesh_path) || path == mesh_path {
                continue;
            }
            // Must be a direct child
            let relative = &path[mesh_path.len()..];
            if relative.matches('/').count() != 1 {
                continue;
            }

            let fields = self.get_field_set(spec.fieldset_index);
            let type_name = fields
                .iter()
                .find(|(k, _)| k == "typeName")
                .and_then(|(_, v)| v.as_token())
                .unwrap_or("");

            if type_name != "GeomSubset" {
                continue;
            }

            let name = path.rsplit('/').next().unwrap_or("").to_string();
            let mut subset = GeomSubset {
                name,
                face_indices: Vec::new(),
                material_binding: None,
                material_index: None,
            };

            for (key, value) in &fields {
                if key == "indices" {
                    if let Some(indices) = value.as_int_array() {
                        subset.face_indices = indices.iter().map(|&i| i as u32).collect();
                    }
                }
            }

            // Check for material binding on the subset
            for sub_spec in self.specs {
                if sub_spec.spec_type != specs::SPEC_TYPE_RELATIONSHIP {
                    continue;
                }
                let sub_path = self.resolve_path(sub_spec.path_index);
                if sub_path == format!("{}/material:binding", path) {
                    let sub_fields = self.get_field_set(sub_spec.fieldset_index);
                    for (key, value) in &sub_fields {
                        if key == "targetPaths" || key == "default" {
                            if let Some(path_str) = value.as_path_or_token() {
                                subset.material_binding = Some(path_str);
                            }
                        }
                    }
                }
            }

            mesh.subsets.push(subset);
        }
    }

    fn extract_skin_binding(&self, mesh_path: &str, mesh: &mut UsdMesh) {
        // Look for skel:joints, skel:jointIndices, skel:jointWeights
        let mut joint_indices_flat: Vec<i32> = Vec::new();
        let mut joint_weights_flat: Vec<f32> = Vec::new();
        let mut skeleton_path = String::new();
        let mut _influences_per_vert = 4usize;

        for spec in self.specs {
            if spec.spec_type != specs::SPEC_TYPE_ATTRIBUTE {
                continue;
            }
            let prop_path = self.resolve_path(spec.path_index);
            if !prop_path.starts_with(mesh_path) || prop_path == mesh_path {
                continue;
            }

            let prop_name = prop_path.rsplit('/').next().unwrap_or("");
            let fields = self.get_field_set(spec.fieldset_index);

            match prop_name {
                "skel:jointIndices" | "primvars:skel:jointIndices" => {
                    for (key, value) in &fields {
                        if key == "default" {
                            if let Some(indices) = value.as_int_array() {
                                joint_indices_flat = indices;
                            }
                        }
                        if key == "elementSize" {
                            if let Some(n) = value.as_int() {
                                _influences_per_vert = n as usize;
                            }
                        }
                    }
                }
                "skel:jointWeights" | "primvars:skel:jointWeights" => {
                    for (key, value) in &fields {
                        if key == "default" {
                            if let Some(w) = value.as_float_array() {
                                joint_weights_flat = w;
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        // Look for skel:skeleton relationship
        for spec in self.specs {
            if spec.spec_type != specs::SPEC_TYPE_RELATIONSHIP {
                continue;
            }
            let prop_path = self.resolve_path(spec.path_index);
            if prop_path == format!("{}/skel:skeleton", mesh_path) {
                let fields = self.get_field_set(spec.fieldset_index);
                for (key, value) in &fields {
                    if key == "targetPaths" || key == "default" {
                        if let Some(p) = value.as_path_or_token() {
                            skeleton_path = p;
                        }
                    }
                }
            }
        }

        if !joint_indices_flat.is_empty() && !joint_weights_flat.is_empty() {
            let vert_count = mesh.positions.len();
            let influences = joint_indices_flat.len() / vert_count.max(1);
            let influences = influences.min(4);

            let mut joints = Vec::with_capacity(vert_count);
            let mut weights = Vec::with_capacity(vert_count);

            for v in 0..vert_count {
                let mut j = [0u16; 4];
                let mut w = [0.0f32; 4];
                for i in 0..influences {
                    let idx = v * influences + i;
                    if idx < joint_indices_flat.len() {
                        j[i] = joint_indices_flat[idx] as u16;
                    }
                    if idx < joint_weights_flat.len() {
                        w[i] = joint_weights_flat[idx];
                    }
                }
                joints.push(j);
                weights.push(w);
            }

            mesh.skin = Some(MeshSkin {
                joints,
                weights,
                skeleton_path,
            });
        }
    }

    fn extract_material(
        &self,
        path: &str,
        _fields: &[(String, Value)],
    ) -> UsdResult<UsdMaterial> {
        let name = path.rsplit('/').next().unwrap_or("Material").to_string();
        let mut mat = UsdMaterial {
            name,
            path: path.to_string(),
            ..Default::default()
        };

        // Find child UsdPreviewSurface shader
        for spec in self.specs {
            if spec.spec_type != specs::SPEC_TYPE_PRIM {
                continue;
            }
            let child_path = self.resolve_path(spec.path_index);
            if !child_path.starts_with(path) || child_path == path {
                continue;
            }

            let child_fields = self.get_field_set(spec.fieldset_index);
            let info_id = child_fields
                .iter()
                .find(|(k, _)| k == "info:id")
                .and_then(|(_, v)| v.as_token())
                .unwrap_or("");

            if info_id == "UsdPreviewSurface" {
                // Extract surface shader properties
                self.extract_preview_surface(&child_path, &mut mat);
            }
        }

        Ok(mat)
    }

    fn extract_preview_surface(&self, shader_path: &str, mat: &mut UsdMaterial) {
        for spec in self.specs {
            if spec.spec_type != specs::SPEC_TYPE_ATTRIBUTE {
                continue;
            }
            let prop_path = self.resolve_path(spec.path_index);
            if !prop_path.starts_with(shader_path) || prop_path == shader_path {
                continue;
            }

            let prop_name = prop_path.rsplit('/').next().unwrap_or("");
            let fields = self.get_field_set(spec.fieldset_index);

            let get_float = |fs: &[(String, Value)]| -> Option<f32> {
                fs.iter()
                    .find(|(k, _)| k == "default")
                    .and_then(|(_, v)| v.as_float())
            };

            let get_vec3 = |fs: &[(String, Value)]| -> Option<[f32; 3]> {
                fs.iter()
                    .find(|(k, _)| k == "default")
                    .and_then(|(_, v)| v.as_vec3f())
            };

            match prop_name {
                "inputs:diffuseColor" => {
                    if let Some(c) = get_vec3(&fields) {
                        mat.diffuse_color = c;
                    }
                }
                "inputs:emissiveColor" => {
                    if let Some(c) = get_vec3(&fields) {
                        mat.emissive_color = c;
                    }
                }
                "inputs:metallic" => {
                    if let Some(v) = get_float(&fields) {
                        mat.metallic = v;
                    }
                }
                "inputs:roughness" => {
                    if let Some(v) = get_float(&fields) {
                        mat.roughness = v;
                    }
                }
                "inputs:opacity" => {
                    if let Some(v) = get_float(&fields) {
                        mat.opacity = v;
                    }
                }
                "inputs:ior" => {
                    if let Some(v) = get_float(&fields) {
                        mat.ior = v;
                    }
                }
                _ => {}
            }
        }
    }

    fn extract_skeleton(
        &self,
        path: &str,
        _fields: &[(String, Value)],
    ) -> UsdResult<UsdSkeleton> {
        let name = path.rsplit('/').next().unwrap_or("Skeleton").to_string();
        let mut skel = UsdSkeleton {
            name,
            path: path.to_string(),
            ..Default::default()
        };

        // Look for skeleton properties in child attributes
        for spec in self.specs {
            if spec.spec_type != specs::SPEC_TYPE_ATTRIBUTE {
                continue;
            }
            let prop_path = self.resolve_path(spec.path_index);
            if !prop_path.starts_with(path) || prop_path == path {
                continue;
            }

            let prop_name = prop_path.rsplit('/').next().unwrap_or("");
            let f = self.get_field_set(spec.fieldset_index);

            match prop_name {
                "joints" => {
                    for (key, value) in &f {
                        if key == "default" {
                            if let Some(joints) = value.as_token_array() {
                                skel.joints = joints;
                                // Compute parent indices from joint paths
                                skel.parent_indices = compute_parent_indices(&skel.joints);
                            }
                        }
                    }
                }
                "bindTransforms" => {
                    for (key, value) in &f {
                        if key == "default" {
                            if let Some(xforms) = value.as_matrix4d_array() {
                                skel.bind_transforms = xforms;
                            }
                        }
                    }
                }
                "restTransforms" => {
                    for (key, value) in &f {
                        if key == "default" {
                            if let Some(xforms) = value.as_matrix4d_array() {
                                skel.rest_transforms = xforms;
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(skel)
    }

    fn extract_animation(
        &self,
        path: &str,
        _fields: &[(String, Value)],
    ) -> UsdResult<UsdAnimation> {
        let name = path.rsplit('/').next().unwrap_or("Animation").to_string();
        let mut anim = UsdAnimation {
            name,
            path: path.to_string(),
            duration: 0.0,
            joint_tracks: Vec::new(),
            blend_shape_tracks: Vec::new(),
        };

        // SkelAnimation stores joints, translations, rotations, scales as time-sampled arrays
        let mut joint_names: Vec<String> = Vec::new();

        for spec in self.specs {
            if spec.spec_type != specs::SPEC_TYPE_ATTRIBUTE {
                continue;
            }
            let prop_path = self.resolve_path(spec.path_index);
            if !prop_path.starts_with(path) || prop_path == path {
                continue;
            }

            let prop_name = prop_path.rsplit('/').next().unwrap_or("");
            let f = self.get_field_set(spec.fieldset_index);

            match prop_name {
                "joints" => {
                    for (key, value) in &f {
                        if key == "default" {
                            if let Some(j) = value.as_token_array() {
                                joint_names = j;
                            }
                        }
                    }
                }
                "translations" | "rotations" | "scales" => {
                    // Time-sampled data would be extracted here
                    // For now, extract default values
                }
                _ => {}
            }
        }

        // Create empty tracks for each joint
        for joint in &joint_names {
            anim.joint_tracks.push(JointTrack {
                joint_path: joint.clone(),
                translations: Vec::new(),
                rotations: Vec::new(),
                scales: Vec::new(),
            });
        }

        Ok(anim)
    }

    fn extract_light(
        &self,
        path: &str,
        type_name: &str,
        _fields: &[(String, Value)],
    ) -> UsdResult<UsdLight> {
        let name = path.rsplit('/').next().unwrap_or("Light").to_string();
        let mut color = [1.0f32; 3];
        let mut intensity = 1.0f32;

        // Extract light properties from child attributes
        for spec in self.specs {
            if spec.spec_type != specs::SPEC_TYPE_ATTRIBUTE {
                continue;
            }
            let prop_path = self.resolve_path(spec.path_index);
            if !prop_path.starts_with(path) || prop_path == path {
                continue;
            }

            let prop_name = prop_path.rsplit('/').next().unwrap_or("");
            let f = self.get_field_set(spec.fieldset_index);

            match prop_name {
                "inputs:color" | "color" => {
                    for (key, value) in &f {
                        if key == "default" {
                            if let Some(c) = value.as_vec3f() {
                                color = c;
                            }
                        }
                    }
                }
                "inputs:intensity" | "intensity" => {
                    for (key, value) in &f {
                        if key == "default" {
                            if let Some(v) = value.as_float() {
                                intensity = v;
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        let kind = match type_name {
            "DistantLight" => LightKind::Distant { angle: 0.53 },
            "SphereLight" => LightKind::Sphere { radius: 1.0 },
            "RectLight" => LightKind::Rect {
                width: 1.0,
                height: 1.0,
            },
            "DiskLight" => LightKind::Disk { radius: 1.0 },
            "DomeLight" => LightKind::Dome { texture_path: None },
            _ => LightKind::Sphere { radius: 1.0 },
        };

        Ok(UsdLight {
            name,
            path: path.to_string(),
            kind,
            color,
            intensity,
        })
    }

    fn extract_camera(
        &self,
        path: &str,
        _fields: &[(String, Value)],
    ) -> UsdResult<UsdCamera> {
        let name = path.rsplit('/').next().unwrap_or("Camera").to_string();
        let mut cam = UsdCamera {
            name,
            path: path.to_string(),
            ..Default::default()
        };

        let mut focal_length = 50.0f32;
        let mut h_aperture = 20.955f32;

        for spec in self.specs {
            if spec.spec_type != specs::SPEC_TYPE_ATTRIBUTE {
                continue;
            }
            let prop_path = self.resolve_path(spec.path_index);
            if !prop_path.starts_with(path) || prop_path == path {
                continue;
            }

            let prop_name = prop_path.rsplit('/').next().unwrap_or("");
            let f = self.get_field_set(spec.fieldset_index);

            match prop_name {
                "focalLength" => {
                    for (key, value) in &f {
                        if key == "default" {
                            if let Some(v) = value.as_float() {
                                focal_length = v;
                            }
                        }
                    }
                }
                "horizontalAperture" => {
                    for (key, value) in &f {
                        if key == "default" {
                            if let Some(v) = value.as_float() {
                                h_aperture = v;
                            }
                        }
                    }
                }
                "clippingRange" => {
                    for (key, value) in &f {
                        if key == "default" {
                            if let Some(r) = value.as_vec2f() {
                                cam.near_clip = r[0];
                                cam.far_clip = r[1];
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        let fov_h = 2.0 * (h_aperture / (2.0 * focal_length)).atan().to_degrees();
        cam.projection = Projection::Perspective {
            fov_horizontal: fov_h,
            focal_length,
        };

        Ok(cam)
    }

    fn extract_stage_metadata(&self, stage: &mut UsdStage) {
        // The pseudo-root is typically the first spec with path "/"
        if let Some(spec) = self.specs.first() {
            let fields = self.get_field_set(spec.fieldset_index);
            for (key, value) in &fields {
                match key.as_str() {
                    "upAxis" => {
                        if let Some(axis) = value.as_token() {
                            stage.up_axis = match axis {
                                "Z" => UpAxis::ZUp,
                                _ => UpAxis::YUp,
                            };
                        }
                    }
                    "metersPerUnit" => {
                        if let Some(v) = value.as_float() {
                            stage.meters_per_unit = v;
                        }
                    }
                    "timeCodesPerSecond" => {
                        if let Some(v) = value.as_float() {
                            stage.time_codes_per_second = v;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn resolve_material_bindings(&self, stage: &mut UsdStage) {
        for mesh in &mut stage.meshes {
            if let Some(ref binding) = mesh.material_binding {
                mesh.material_index = stage
                    .materials
                    .iter()
                    .position(|m| m.path == *binding);
            }
            for subset in &mut mesh.subsets {
                if let Some(ref binding) = subset.material_binding {
                    subset.material_index = stage
                        .materials
                        .iter()
                        .position(|m| m.path == *binding);
                }
            }
        }
    }
}

fn identity_matrix() -> [f32; 16] {
    [
        1.0, 0.0, 0.0, 0.0,
        0.0, 1.0, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ]
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
