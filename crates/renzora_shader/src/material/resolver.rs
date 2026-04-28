//! Material resolver — watches entities with `MaterialRef` and resolves them
//! to the appropriate `GraphMaterial` or `CodeShaderMaterial`.

use std::collections::HashMap;
use std::marker::PhantomData;
use std::path::Path;

use bevy::prelude::*;
use uuid::Uuid;

use super::codegen::{self, FunctionRegistry};
use super::graph::{MaterialDomain, MaterialFunction, MaterialGraph};
use super::material_ref::MaterialRef;
use super::runtime::{FallbackTexture, GraphMaterial, GraphMaterialShaderState, new_graph_material};
use crate::runtime::{CodeShaderMaterial, ShaderCache};

/// Scan the directory containing a material file for sibling
/// `*.material_function` files and build a local registry.
fn load_sibling_functions(material_path: &str) -> FunctionRegistry {
    let mut registry: FunctionRegistry = HashMap::new();
    let Some(parent) = Path::new(material_path).parent() else {
        return registry;
    };
    let Ok(entries) = std::fs::read_dir(parent) else {
        return registry;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("material_function") {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else { continue; };
        match serde_json::from_str::<MaterialFunction>(&content) {
            Ok(mat_fn) => {
                registry.insert(mat_fn.name.clone(), mat_fn);
            }
            Err(e) => {
                warn!("Failed to parse material function '{}': {}", path.display(), e);
            }
        }
    }
    registry
}

/// Cache of compiled materials to avoid redundant recompilation.
///
/// `.material` graphs that fall in the "trivial" subset (texture + factor →
/// PBR pin) compile to plain `StandardMaterial` and live in
/// `standard_materials`. Anything procedural (math, noise, animation, custom
/// WGSL) compiles to a `GraphMaterial` (`ExtendedMaterial<StandardMaterial,
/// SurfaceGraphExt>`) and lives in `graph_materials`. A given path is in at
/// most one map at a time; `invalidate` clears both so a graph that crosses
/// the trivial/procedural boundary on edit recompiles cleanly.
#[derive(Resource, Default)]
pub struct MaterialCache {
    /// Compiled trivial materials — plain StandardMaterial assets that go
    /// through Bevy's stock PBR pipeline. The vast majority of imported
    /// .material files land here.
    standard_materials: HashMap<String, Handle<bevy::pbr::StandardMaterial>>,
    /// Compiled procedural materials — ExtendedMaterial wrapping a custom
    /// fragment shader generated from the graph.
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
        self.standard_materials.remove(path);
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

/// Outcome of compiling a `.material` file. The classifier in
/// `standard_build` decides which variant we get based on the graph's nodes.
enum CompiledMaterial {
    /// Trivial graph — compiled to a plain StandardMaterial. Renders via
    /// Bevy's stock PBR pipeline, no custom shader.
    Standard(Handle<bevy::pbr::StandardMaterial>),
    /// Procedural graph — compiled to a custom WGSL fragment running on top
    /// of StandardMaterial via ExtendedMaterial.
    Graph(Handle<GraphMaterial>),
}

/// Plugin that resolves `MaterialRef` components to actual materials.
pub struct MaterialResolverPlugin;

impl Plugin for MaterialResolverPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] MaterialResolverPlugin");
        app.init_resource::<MaterialCache>()
            .init_resource::<renzora::VirtualFileReader>()
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
    standard_materials: Option<ResMut<Assets<bevy::pbr::StandardMaterial>>>,
    graph_materials: Option<ResMut<Assets<GraphMaterial>>>,
    code_materials: Option<ResMut<Assets<CodeShaderMaterial>>>,
    shaders: Option<ResMut<Assets<Shader>>>,
    shader_state: Option<ResMut<GraphMaterialShaderState>>,
    shader_cache: Option<ResMut<ShaderCache>>,
    shader_registry: Option<Res<crate::registry::ShaderBackendRegistry>>,
    fallback_texture: Option<Res<FallbackTexture>>,
    project: Option<Res<renzora::CurrentProject>>,
    file_reader: Option<Res<renzora::VirtualFileReader>>,
    asset_server: Res<AssetServer>,
) {
    let Some(mut standard_materials) = standard_materials else { return; };
    let Some(mut graph_materials) = graph_materials else { return; };
    let Some(mut code_materials) = code_materials else { return; };
    let Some(mut shaders) = shaders else { return; };
    let Some(mut shader_state) = shader_state else { return; };
    let Some(mut shader_cache) = shader_cache else { return; };
    let Some(shader_registry) = shader_registry else { return; };
    let default_reader = renzora::VirtualFileReader::default();
    let reader = file_reader.as_deref().unwrap_or(&default_reader);
    for (entity, mat_ref) in query.iter() {
        let path = &mat_ref.0;

        // Check the trivial (StandardMaterial) cache first — it's the fast
        // path for imported materials.
        if let Some(handle) = cache.standard_materials.get(path) {
            // Strip any leftover GraphMaterial from a prior render frame so
            // the entity ends up with exactly one MeshMaterial3d component.
            commands.entity(entity).remove::<MeshMaterial3d<GraphMaterial>>();
            commands.entity(entity).try_insert((
                MeshMaterial3d(handle.clone()),
                MaterialResolved { source_path: path.clone() },
            ));
            continue;
        }

        // Check graph material cache (procedural path)
        if let Some(handle) = cache.graph_materials.get(path) {
            commands.entity(entity).remove::<MeshMaterial3d<bevy::pbr::StandardMaterial>>();
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
            match resolve_material_file(
                &fs_path,
                &mut standard_materials,
                &mut graph_materials,
                &mut shaders,
                &mut shader_state,
                &fallback_texture,
                &asset_server,
                reader,
            ) {
                Some(CompiledMaterial::Standard(handle)) => {
                    cache.standard_materials.insert(path.clone(), handle.clone());
                    // Strip a stale GraphMaterial component if present (e.g.
                    // hot-reload that crossed the trivial/procedural boundary).
                    commands.entity(entity).remove::<MeshMaterial3d<GraphMaterial>>();
                    commands.entity(entity).try_insert((
                        MeshMaterial3d(handle),
                        MaterialResolved { source_path: path.clone() },
                    ));
                }
                Some(CompiledMaterial::Graph(handle)) => {
                    cache.graph_materials.insert(path.clone(), handle.clone());
                    // Strip the GLB-decoded StandardMaterial so it doesn't
                    // render alongside the GraphMaterial.
                    commands.entity(entity).remove::<MeshMaterial3d<bevy::pbr::StandardMaterial>>();
                    commands.entity(entity).try_insert((
                        MeshMaterial3d(handle),
                        MaterialResolved { source_path: path.clone() },
                    ));
                }
                None => {
                    // File missing or malformed. Mark resolved anyway so we
                    // don't reopen it every frame — for a model with N mesh
                    // entities all referencing the same broken file, that
                    // would be N file-open syscalls per frame. The mesh
                    // keeps its existing StandardMaterial as a fallback.
                    warn!("Failed to resolve .material: {}", path);
                    commands.entity(entity).try_insert(
                        MaterialResolved { source_path: path.clone() },
                    );
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
    reader: &renzora::VirtualFileReader,
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
    reader: &renzora::VirtualFileReader,
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

/// Read and parse a `.material` file, then route through either the trivial
/// (StandardMaterial) compiler or the full graph codegen.
///
/// The classifier in `standard_build::try_build_standard_material` walks the
/// graph; if every reachable node maps onto a StandardMaterial field, we
/// produce a plain `Handle<StandardMaterial>` — same Material asset every
/// other PBR mesh in the scene uses, so we share the stock PBR pipeline and
/// pay zero per-material shader compile.
///
/// Returns `None` if the file is missing, unparseable, or — for procedural
/// graphs — fails to compile to WGSL.
fn resolve_material_file(
    path: &str,
    standard_materials: &mut Assets<bevy::pbr::StandardMaterial>,
    graph_materials: &mut Assets<GraphMaterial>,
    shaders: &mut Assets<Shader>,
    shader_state: &mut GraphMaterialShaderState,
    fallback_texture: &Option<Res<FallbackTexture>>,
    asset_server: &AssetServer,
    reader: &renzora::VirtualFileReader,
) -> Option<CompiledMaterial> {
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

    // Trivial fast path: graph is just textures + factors → PBR pins.
    // Compiles directly to StandardMaterial; no shader codegen, shared with
    // Bevy's stock PBR pipeline. Imported glTF materials all land here.
    if let Some(mat) =
        super::standard_build::try_build_standard_material(&graph, asset_server)
    {
        return Some(CompiledMaterial::Standard(standard_materials.add(mat)));
    }

    // Procedural fallback: hand off to the existing graph→WGSL codegen and
    // wrap the result in an ExtendedMaterial. Same code path as before.
    let handle = resolve_graph_material_from_graph(
        path,
        &graph,
        graph_materials,
        shaders,
        shader_state,
        fallback_texture,
        asset_server,
    )?;
    Some(CompiledMaterial::Graph(handle))
}

/// Compile an already-parsed [`MaterialGraph`] into a procedural
/// [`GraphMaterial`] (`ExtendedMaterial<StandardMaterial, SurfaceGraphExt>`).
///
/// This is the path for graphs containing procedural / math / animation /
/// custom-WGSL nodes. Same code that used to live in `resolve_graph_material`
/// — the file-read and parse steps were lifted up into `resolve_material_file`
/// so the trivial classifier can run on the parsed graph without re-reading.
fn resolve_graph_material_from_graph(
    path: &str,
    graph: &MaterialGraph,
    materials: &mut Assets<GraphMaterial>,
    shaders: &mut Assets<Shader>,
    _shader_state: &mut GraphMaterialShaderState,
    fallback_texture: &Option<Res<FallbackTexture>>,
    asset_server: &AssetServer,
) -> Option<Handle<GraphMaterial>> {

    // Load any sibling `.material_function` files in the same directory so the
    // graph can reference them via function/call nodes.
    let functions = load_sibling_functions(path);
    let registry = if functions.is_empty() { None } else { Some(&functions) };

    // Compile graph → WGSL
    let result = codegen::compile_with_functions(&graph, registry);
    if !result.errors.is_empty() {
        for err in &result.errors {
            error!("Material compile error in '{}': {}", path, err);
        }
        return None;
    }

    // Each material gets its own compiled shader, inserted at a unique
    // `Handle::Uuid`. The UUID lives in the extension's pipeline key so
    // `specialize()` can reconstruct the handle and swap it into the
    // fragment stage. See `surface_ext.rs` for why we can't store the
    // `Handle<Shader>` directly (packed-struct + non-Copy).
    let shader_uuid = Uuid::new_v4();
    let shader_handle: Handle<Shader> = Handle::Uuid(shader_uuid, PhantomData);
    let shader = Shader::from_wgsl(
        result.fragment_shader,
        format!("material://{}", path),
    );
    let _ = shaders.insert(&shader_handle, shader);

    // Build the ExtendedMaterial<StandardMaterial, SurfaceGraphExt>. The base
    // StandardMaterial supplies all the heavy PBR plumbing; our extension
    // supplies the compiled per-material fragment shader and texture slots.
    let mut mat = if let Some(fb) = fallback_texture {
        new_graph_material(fb)
    } else {
        new_graph_material(&FallbackTexture(Handle::default()))
    };
    mat.extension.shader_uuid = Some(shader_uuid);

    // Domain → base StandardMaterial configuration. This is where the graph's
    // compile-time domain choice translates into a Bevy-side feature flip
    // (unlit bypasses lighting; AlphaMode controls queue placement).
    match result.domain {
        MaterialDomain::Unlit => {
            mat.base.unlit = true;
        }
        MaterialDomain::Surface | MaterialDomain::Vegetation | MaterialDomain::TerrainLayer => {
            mat.base.unlit = false;
        }
    }

    // Transmission pins connected → must enable transmission on the CPU-side
    // StandardMaterial so Bevy's `reads_view_transmission_texture()` returns
    // true, which schedules the transmissive render pass that populates
    // `view_transmission_texture`. The runtime-shader value (driven by the
    // graph) overrides this default anyway; 0.5 is just enough to trigger
    // the pipeline decision.
    if result.requires_transmission {
        mat.base.specular_transmission = 0.5;
    }

    // Map graph-level alpha mode and double-sided flag onto Bevy's
    // StandardMaterial. Without this, transparent materials (glass, foliage,
    // decals) render as opaque garbage even when the alpha pin is wired up.
    mat.base.alpha_mode = match graph.alpha_mode {
        crate::material::graph::AlphaMode::Opaque => bevy::prelude::AlphaMode::Opaque,
        crate::material::graph::AlphaMode::Mask { cutoff } => bevy::prelude::AlphaMode::Mask(cutoff),
        crate::material::graph::AlphaMode::Blend => bevy::prelude::AlphaMode::Blend,
    };
    mat.base.cull_mode = if graph.double_sided {
        None
    } else {
        Some(bevy::render::render_resource::Face::Back)
    };

    for tb in &result.texture_bindings {
        if tb.asset_path.is_empty() {
            continue;
        }
        let handle: Handle<Image> = asset_server.load(&tb.asset_path);
        match (tb.kind, tb.binding) {
            (codegen::TextureKind::D2, 0) => mat.extension.texture_0 = Some(handle),
            (codegen::TextureKind::D2, 1) => mat.extension.texture_1 = Some(handle),
            (codegen::TextureKind::D2, 2) => mat.extension.texture_2 = Some(handle),
            (codegen::TextureKind::D2, 3) => mat.extension.texture_3 = Some(handle),
            (codegen::TextureKind::D2, 4) => mat.extension.texture_4 = Some(handle),
            (codegen::TextureKind::D2, 5) => mat.extension.texture_5 = Some(handle),
            (codegen::TextureKind::Cube, 0) => mat.extension.cube_0 = Some(handle),
            (codegen::TextureKind::D2Array, 0) => mat.extension.array_0 = Some(handle),
            (codegen::TextureKind::D3, 0) => mat.extension.volume_0 = Some(handle),
            _ => warn!(
                "Material '{}' wants more textures than the extension provides ({:?} slot {})",
                path, tb.kind, tb.binding
            ),
        }
    }

    Some(materials.add(mat))
}
