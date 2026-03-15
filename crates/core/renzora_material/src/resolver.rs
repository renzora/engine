//! Material resolver — watches entities with `MaterialRef` and resolves them
//! to the appropriate `GraphMaterial` or `CodeShaderMaterial`.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::codegen;
use crate::graph::MaterialGraph;
use crate::material_ref::MaterialRef;
use crate::runtime::{GraphMaterial, GraphMaterialShaderState};
use renzora_shader::runtime::{CodeShaderMaterial, ShaderCache};

/// Cache of compiled materials to avoid redundant recompilation.
#[derive(Resource, Default)]
pub struct MaterialCache {
    /// Compiled graph materials by file path.
    graph_materials: HashMap<String, Handle<GraphMaterial>>,
    /// Compiled code shader materials by file path.
    code_materials: HashMap<String, Handle<CodeShaderMaterial>>,
    /// Tracks which paths have been loaded (for future hot-reload).
    #[allow(dead_code)]
    loaded_paths: HashMap<String, u64>,
}

/// Marker component added to entities that have been resolved.
#[derive(Component)]
pub struct MaterialResolved {
    pub source_path: String,
}

/// Plugin that resolves `MaterialRef` components to actual materials.
pub struct MaterialResolverPlugin;

impl Plugin for MaterialResolverPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] MaterialResolverPlugin");
        app.init_resource::<MaterialCache>()
            .register_type::<MaterialRef>()
            .register_type::<crate::material_ref::MaterialOverrides>()
            .register_type::<crate::material_ref::ParamValue>()
            .add_systems(Update, resolve_material_refs);
    }
}

/// System that finds entities with `MaterialRef` + `Mesh3d` that don't yet have a resolved material,
/// loads the file, compiles it, and attaches the material handle.
fn resolve_material_refs(
    mut commands: Commands,
    query: Query<(Entity, &MaterialRef), Without<MaterialResolved>>,
    mut cache: ResMut<MaterialCache>,
    mut graph_materials: ResMut<Assets<GraphMaterial>>,
    mut code_materials: ResMut<Assets<CodeShaderMaterial>>,
    mut shaders: ResMut<Assets<Shader>>,
    mut shader_state: ResMut<GraphMaterialShaderState>,
    mut shader_cache: ResMut<ShaderCache>,
    shader_registry: Res<renzora_shader::registry::ShaderBackendRegistry>,
) {
    for (entity, mat_ref) in query.iter() {
        let path = &mat_ref.0;

        // Check graph material cache
        if let Some(handle) = cache.graph_materials.get(path) {
            commands.entity(entity).insert((
                MeshMaterial3d(handle.clone()),
                MaterialResolved { source_path: path.clone() },
            ));
            continue;
        }

        // Check code material cache
        if let Some(handle) = cache.code_materials.get(path) {
            commands.entity(entity).insert((
                MeshMaterial3d(handle.clone()),
                MaterialResolved { source_path: path.clone() },
            ));
            continue;
        }

        // Determine file type and resolve
        if path.ends_with(".material") {
            match resolve_graph_material(path, &mut graph_materials, &mut shaders, &mut shader_state) {
                Some(handle) => {
                    cache.graph_materials.insert(path.clone(), handle.clone());
                    commands.entity(entity).insert((
                        MeshMaterial3d(handle),
                        MaterialResolved { source_path: path.clone() },
                    ));
                }
                None => {
                    warn!("Failed to resolve .material: {}", path);
                }
            }
        } else if path.ends_with(".shader") {
            match resolve_code_shader(
                path,
                &mut code_materials,
                &mut shaders,
                &mut shader_cache,
                &shader_registry,
            ) {
                Some(handle) => {
                    cache.code_materials.insert(path.clone(), handle.clone());
                    commands.entity(entity).insert((
                        MeshMaterial3d(handle),
                        MaterialResolved { source_path: path.clone() },
                    ));
                }
                None => {
                    warn!("Failed to resolve .shader: {}", path);
                    commands.entity(entity).insert(
                        MaterialResolved { source_path: path.clone() },
                    );
                }
            }
        } else {
            warn!("MaterialRef has unsupported extension: {}", path);
            commands.entity(entity).insert(
                MaterialResolved { source_path: path.clone() },
            );
        }
    }
}

/// Load a `.shader` JSON file, compile it via the shader backend, and create a `CodeShaderMaterial`.
fn resolve_code_shader(
    path: &str,
    materials: &mut Assets<CodeShaderMaterial>,
    shaders: &mut Assets<Shader>,
    shader_cache: &mut ShaderCache,
    registry: &renzora_shader::registry::ShaderBackendRegistry,
) -> Option<Handle<CodeShaderMaterial>> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to read shader file '{}': {}", path, e);
            return None;
        }
    };

    let shader_file: renzora_shader::file::ShaderFile = match serde_json::from_str(&content) {
        Ok(f) => f,
        Err(e) => {
            error!("Failed to parse shader file '{}': {}", path, e);
            return None;
        }
    };

    let language = if shader_file.language.is_empty() {
        renzora_shader::file::detect_language(&shader_file.shader_source).to_string()
    } else {
        shader_file.language.clone()
    };
    let wgsl = match registry.compile(&language, &shader_file.shader_source) {
        Ok(w) => w,
        Err(e) => {
            error!("Failed to compile shader '{}': {}", path, e.message);
            return None;
        }
    };

    let label = format!("code_shader://{}", path);
    let shader_handle = shader_cache.get_or_insert(&wgsl, &label, shaders);

    let mat = CodeShaderMaterial {
        shader_handle,
        ..Default::default()
    };
    Some(materials.add(mat))
}

/// Load a `.material` JSON file, compile its graph to WGSL, and create a `GraphMaterial` asset.
fn resolve_graph_material(
    path: &str,
    materials: &mut Assets<GraphMaterial>,
    shaders: &mut Assets<Shader>,
    _shader_state: &mut GraphMaterialShaderState,
) -> Option<Handle<GraphMaterial>> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to read material file '{}': {}", path, e);
            return None;
        }
    };

    let graph: MaterialGraph = match serde_json::from_str(&content) {
        Ok(g) => g,
        Err(e) => {
            error!("Failed to parse material file '{}': {}", path, e);
            return None;
        }
    };

    // Compile graph → WGSL
    let result = codegen::compile(&graph);
    if !result.errors.is_empty() {
        for err in &result.errors {
            error!("Material compile error in '{}': {}", path, err);
        }
        return None;
    }

    // Insert the compiled shader
    let shader = Shader::from_wgsl(
        result.fragment_shader,
        format!("material://{}", path),
    );
    let _ = shaders.insert(
        &crate::runtime::GRAPH_MATERIAL_FRAG_HANDLE,
        shader,
    );

    // Create material asset
    let mat = GraphMaterial::default();
    Some(materials.add(mat))
}
