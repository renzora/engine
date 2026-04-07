//! Material resolver — watches entities with `MaterialRef` and resolves them
//! to the appropriate `GraphMaterial` or `CodeShaderMaterial`.

use std::collections::HashMap;

use bevy::prelude::*;

use super::codegen;
use super::graph::MaterialGraph;
use super::material_ref::MaterialRef;
use super::runtime::{FallbackTexture, GraphMaterial, GraphMaterialShaderState, new_graph_material};
use crate::runtime::{CodeShaderMaterial, ShaderCache};

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

impl MaterialCache {
    /// Remove a cached material so the resolver re-loads it from disk.
    pub fn invalidate(&mut self, path: &str) {
        self.graph_materials.remove(path);
        self.code_materials.remove(path);
        self.loaded_paths.remove(path);
    }
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
            .init_resource::<renzora_core::VirtualFileReader>()
            .register_type::<MaterialRef>()
            .register_type::<super::material_ref::MaterialOverrides>()
            .register_type::<super::material_ref::ParamValue>()
            .add_systems(Update, resolve_material_refs);
    }
}

/// System that finds entities with `MaterialRef` + `Mesh3d` that don't yet have a resolved material,
/// loads the file, compiles it, and attaches the material handle.
fn resolve_material_refs(
    mut commands: Commands,
    query: Query<(Entity, &MaterialRef), Without<MaterialResolved>>,
    mut cache: ResMut<MaterialCache>,
    graph_materials: Option<ResMut<Assets<GraphMaterial>>>,
    code_materials: Option<ResMut<Assets<CodeShaderMaterial>>>,
    shaders: Option<ResMut<Assets<Shader>>>,
    shader_state: Option<ResMut<GraphMaterialShaderState>>,
    shader_cache: Option<ResMut<ShaderCache>>,
    shader_registry: Option<Res<crate::registry::ShaderBackendRegistry>>,
    fallback_texture: Option<Res<FallbackTexture>>,
    project: Option<Res<renzora_core::CurrentProject>>,
    file_reader: Option<Res<renzora_core::VirtualFileReader>>,
    asset_server: Res<AssetServer>,
) {
    let Some(mut graph_materials) = graph_materials else { return; };
    let Some(mut code_materials) = code_materials else { return; };
    let Some(mut shaders) = shaders else { return; };
    let Some(mut shader_state) = shader_state else { return; };
    let Some(mut shader_cache) = shader_cache else { return; };
    let Some(shader_registry) = shader_registry else { return; };
    let default_reader = renzora_core::VirtualFileReader::default();
    let reader = file_reader.as_deref().unwrap_or(&default_reader);
    for (entity, mat_ref) in query.iter() {
        let path = &mat_ref.0;

        // Check graph material cache
        if let Some(handle) = cache.graph_materials.get(path) {
            commands.entity(entity).try_insert((
                MeshMaterial3d(handle.clone()),
                MaterialResolved { source_path: path.clone() },
            ));
            continue;
        }

        // Check code material cache
        if let Some(handle) = cache.code_materials.get(path) {
            commands.entity(entity).try_insert((
                MeshMaterial3d(handle.clone()),
                MaterialResolved { source_path: path.clone() },
            ));
            continue;
        }

        // Resolve asset-relative path to full filesystem path via CurrentProject.
        // Normalize to forward slashes and strip leading "./" for rpak compatibility.
        let fs_path = if std::path::Path::new(path).is_absolute() {
            path.clone()
        } else if let Some(ref proj) = project {
            let raw = proj.resolve_path(path).to_string_lossy().to_string();
            let normalized = raw.replace('\\', "/");
            normalized.strip_prefix("./").unwrap_or(&normalized).to_string()
        } else {
            path.clone()
        };

        // Determine file type and resolve
        if path.ends_with(".material") {
            match resolve_graph_material(&fs_path, &mut graph_materials, &mut shaders, &mut shader_state, &fallback_texture, &asset_server, reader) {
                Some(handle) => {
                    cache.graph_materials.insert(path.clone(), handle.clone());
                    // Remove StandardMaterial so it doesn't keep rendering over the custom one
                    commands.entity(entity).remove::<MeshMaterial3d<bevy::pbr::StandardMaterial>>();
                    commands.entity(entity).try_insert((
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
                &fs_path,
                &mut code_materials,
                &mut shaders,
                &mut shader_cache,
                &shader_registry,
                reader,
            ) {
                Some(handle) => {
                    cache.code_materials.insert(path.clone(), handle.clone());
                    commands.entity(entity).remove::<MeshMaterial3d<bevy::pbr::StandardMaterial>>();
                    commands.entity(entity).try_insert((
                        MeshMaterial3d(handle),
                        MaterialResolved { source_path: path.clone() },
                    ));
                }
                None => {
                    warn!("Failed to resolve .shader: {}", path);
                    commands.entity(entity).try_insert(
                        MaterialResolved { source_path: path.clone() },
                    );
                }
            }
        } else if path.ends_with(".wgsl") || path.ends_with(".glsl") || path.ends_with(".frag") || path.ends_with(".vert") {
            match resolve_raw_shader(
                &fs_path,
                &mut code_materials,
                &mut shaders,
                &mut shader_cache,
                &shader_registry,
                reader,
            ) {
                Some(handle) => {
                    cache.code_materials.insert(path.clone(), handle.clone());
                    commands.entity(entity).remove::<MeshMaterial3d<bevy::pbr::StandardMaterial>>();
                    commands.entity(entity).try_insert((
                        MeshMaterial3d(handle),
                        MaterialResolved { source_path: path.clone() },
                    ));
                }
                None => {
                    warn!("Failed to resolve raw shader: {}", path);
                    commands.entity(entity).try_insert(
                        MaterialResolved { source_path: path.clone() },
                    );
                }
            }
        } else {
            warn!("MaterialRef has unsupported extension: {}", path);
            commands.entity(entity).try_insert(
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
    registry: &crate::registry::ShaderBackendRegistry,
    reader: &renzora_core::VirtualFileReader,
) -> Option<Handle<CodeShaderMaterial>> {
    let content = match reader.read_string(path) {
        Some(c) => c,
        None => {
            error!("Failed to read shader file '{}'", path);
            return None;
        }
    };

    let shader_file: crate::file::ShaderFile = match serde_json::from_str(&content) {
        Ok(f) => f,
        Err(e) => {
            error!("Failed to parse shader file '{}': {}", path, e);
            return None;
        }
    };

    let language = if shader_file.language.is_empty() {
        crate::file::detect_language(&shader_file.shader_source).to_string()
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

/// Load a raw shader file (.wgsl, .glsl, .frag, .vert), auto-detect language,
/// compile it, and create a `CodeShaderMaterial`.
fn resolve_raw_shader(
    path: &str,
    materials: &mut Assets<CodeShaderMaterial>,
    shaders: &mut Assets<Shader>,
    shader_cache: &mut ShaderCache,
    registry: &crate::registry::ShaderBackendRegistry,
    reader: &renzora_core::VirtualFileReader,
) -> Option<Handle<CodeShaderMaterial>> {
    let source = match reader.read_string(path) {
        Some(c) => c,
        None => {
            error!("Failed to read raw shader file '{}'", path);
            return None;
        }
    };

    let language = crate::file::detect_language(&source).to_string();
    let wgsl = match registry.compile(&language, &source) {
        Ok(w) => w,
        Err(e) => {
            error!("Failed to compile raw shader '{}': {}", path, e.message);
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
    fallback_texture: &Option<Res<FallbackTexture>>,
    asset_server: &AssetServer,
    reader: &renzora_core::VirtualFileReader,
) -> Option<Handle<GraphMaterial>> {
    let content = match reader.read_string(path) {
        Some(c) => c,
        None => {
            error!("Failed to read material file '{}'", path);
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

    // Each material gets its own shader handle — no more shared overwriting.
    let shader = Shader::from_wgsl(
        result.fragment_shader,
        format!("material://{}", path),
    );
    let shader_handle = shaders.add(shader);

    // Create material — start with fallback textures, then load actual ones
    let mut mat = if let Some(fb) = fallback_texture {
        new_graph_material(fb)
    } else {
        new_graph_material(&FallbackTexture(Handle::default()))
    };
    mat.shader = Some(shader_handle);

    for tb in &result.texture_bindings {
        if tb.asset_path.is_empty() {
            continue;
        }
        let handle: Handle<Image> = asset_server.load(&tb.asset_path);
        match tb.binding {
            0 => mat.texture_0 = Some(handle),
            1 => mat.texture_1 = Some(handle),
            2 => mat.texture_2 = Some(handle),
            3 => mat.texture_3 = Some(handle),
            _ => {}
        }
    }

    Some(materials.add(mat))
}
