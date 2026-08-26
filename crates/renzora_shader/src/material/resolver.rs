//! Material resolver — watches entities with `MaterialRef` and resolves them
//! to the appropriate `GraphMaterial` or `CodeShaderMaterial`.

use std::collections::HashMap;
use std::marker::PhantomData;
use std::path::Path;

use bevy::prelude::*;
use uuid::Uuid;

use super::codegen::{self, FunctionRegistry, MaterialParam};
use super::graph::{MaterialDomain, MaterialFunction, MaterialGraph};
use super::material_ref::MaterialRef;
use super::runtime::{
    new_graph_material, FallbackTexture, GraphMaterial, GraphMaterialShaderState,
};
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
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        match serde_json::from_str::<MaterialFunction>(&content) {
            Ok(mat_fn) => {
                registry.insert(mat_fn.name.clone(), mat_fn);
            }
            Err(e) => {
                warn!(
                    "Failed to parse material function '{}': {}",
                    path.display(),
                    e
                );
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
/// most one map at a time; `invalidate` clears all maps so a graph that
/// crosses the trivial/procedural boundary on edit recompiles cleanly.
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
    /// Per-master metadata extracted at compile time. Material instances of
    /// procedural masters look this up to find slot indices for their
    /// override map and to copy the master's authored defaults into the
    /// instance's parameter buffer. Keyed by master `.material` path.
    master_meta: HashMap<String, MasterMeta>,
    /// `(node, first line, last line)` source maps from codegen, keyed by
    /// `.material` path. What lets `report_material_problems` put a compile
    /// error on a graph node. Sits here rather than in `MasterMeta` because
    /// the precompiled path reads it from the saved `.material`, not from
    /// a compile.
    node_line_maps: HashMap<String, Vec<(u64, u32, u32)>>,
    /// Tracks which paths have been loaded (for future hot-reload).
    #[allow(dead_code)]
    loaded_paths: HashMap<String, u64>,
}

/// Compile-time metadata about a master `.material` file that material
/// instances need to build their per-instance overrides.
#[derive(Clone)]
pub struct MasterMeta {
    /// Parameters in slot order — index in this `Vec` equals the
    /// `material_params.slots[i]` index the master's WGSL reads from.
    pub parameters: Vec<MaterialParam>,
}

impl MaterialCache {
    /// Remove a cached material so the resolver re-loads it from disk.
    pub fn invalidate(&mut self, path: &str) {
        self.standard_materials.remove(path);
        self.graph_materials.remove(path);
        self.code_materials.remove(path);
        self.master_meta.remove(path);
        self.node_line_maps.remove(path);
        self.loaded_paths.remove(path);
    }

    /// Number of `.material` files that compiled to plain
    /// `StandardMaterial` (the trivial fast path).
    pub fn standard_count(&self) -> usize {
        self.standard_materials.len()
    }

    /// Number of `.material` files that compiled to procedural
    /// `GraphMaterial` (custom WGSL fragment).
    pub fn graph_count(&self) -> usize {
        self.graph_materials.len()
    }

    /// Number of `.shader` / `.wgsl` files compiled into
    /// `CodeShaderMaterial`.
    pub fn code_count(&self) -> usize {
        self.code_materials.len()
    }

    /// Number of compiled masters whose parameter metadata is cached
    /// for derived material instances.
    pub fn master_meta_count(&self) -> usize {
        self.master_meta.len()
    }
}

/// Marker component added to entities that have been resolved. Defined in the
/// contract crate (every panel that rebinds a material has to remove it) and
/// re-exported here so the resolver's own module path keeps working.
pub use renzora::core::MaterialResolved;

/// Outcome of compiling a `.material` file. The classifier in
/// `standard_build` decides which variant we get based on the graph's nodes.
enum CompiledMaterial {
    /// Trivial graph — compiled to a plain StandardMaterial. Renders via
    /// Bevy's stock PBR pipeline, no custom shader.
    Standard(Handle<bevy::pbr::StandardMaterial>),
    /// Procedural graph — compiled to a custom WGSL fragment running on top
    /// of StandardMaterial via ExtendedMaterial. Carries the parameter list
    /// so callers can stash per-master metadata for instance overrides.
    Graph {
        handle: Handle<GraphMaterial>,
        parameters: Vec<MaterialParam>,
        /// Codegen warnings. `Some` only when this resolve ran the codegen
        /// itself — the precompiled path compiles at save time, where the
        /// save site owns the warning, so `None` means "not ours to report".
        warnings: Option<Vec<String>>,
        /// `(node, first line, last line)` — the source map compile errors
        /// are attributed through. From codegen on the live path, from the
        /// embedded artifact's meta (or the legacy `.wgsl.meta` sidecar) on
        /// the precompiled one.
        node_lines: Vec<(u64, u32, u32)>,
    },
}

/// Plugin that resolves `MaterialRef` components to actual materials.
pub struct MaterialResolverPlugin;

impl Plugin for MaterialResolverPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] MaterialResolverPlugin");
        app.init_resource::<MaterialCache>()
            .init_resource::<super::perf::MaterialPerfStats>()
            .init_resource::<renzora::VirtualFileReader>()
            .init_resource::<renzora::content_problems::ContentProblems>()
            .register_type::<MaterialRef>()
            .register_type::<super::material_ref::MaterialOverrides>()
            .register_type::<super::material_ref::ParamValue>()
            .add_systems(Update, resolve_material_refs);
        // Problems-panel validation: re-composes every graph material's WGSL
        // against all of bevy_pbr's modules and naga-validates it on the main
        // thread. Editor-only — a shipped game has no Problems panel.
        //
        // Gated TWICE, because neither gate alone is enough:
        //
        //   * The `editor` feature keeps `validate.rs` and its `naga_oil`
        //     dependency out of a build that resolves only runtime crates.
        //     That is a binary-size win, and it is ALL it is: `xtask` builds
        //     with one `cargo build --workspace`, so cargo unifies features
        //     across both executables and `renzora_material_editor`'s opt-in
        //     turns this feature on for the runtime binary too. Confirm with
        //     `cargo tree -p renzora_app -p renzora_editor_app -i
        //     renzora_shader -e features` — `editor` is active there, and
        //     absent from `-p renzora_app` alone.
        //   * So correctness rides on `EditorSession`, decided at runtime from
        //     whether the editor bundle is present. Same signal the rest of
        //     the engine branches on, and the only one a shipped game cannot
        //     accidentally satisfy.
        #[cfg(feature = "editor")]
        app
            // `PreUpdate`: the validator arrives via `Commands`, and a material
            // resolved the same frame reports nothing, then is never looked at
            // again. `resource_changed` because the check otherwise allocates
            // a map over every shader each frame, forever; library assets only
            // stream in while the asset set is actually changing.
            .add_systems(
                PreUpdate,
                ensure_shader_validator
                    .run_if(resource_changed::<Assets<Shader>>)
                    .run_if(in_editor_session),
            )
            .add_systems(
                Update,
                report_material_problems
                    .after(resolve_material_refs)
                    .run_if(in_editor_session),
            );
    }
}

/// Whether this process is an editor session rather than a shipped game.
///
/// Absent resource reads as "game": `EditorSession` is inserted during runtime
/// setup, so a system scheduled before it lands must not assume editor.
#[cfg(feature = "editor")]
fn in_editor_session(session: Option<Res<renzora::EditorSession>>) -> bool {
    session.is_some_and(|s| s.is_editor())
}

/// Compile the WGSL each graph material binds and record what the compiler says.
///
/// Keyed on the shader uuid rather than driven off the resolve, because the
/// validator needs bevy_pbr's libraries and those land a frame after the first
/// material resolves. A fresh uuid per compile catches recompiles too.
#[cfg(feature = "editor")]
fn report_material_problems(
    mut checked: Local<HashMap<String, (Uuid, usize)>>,
    cache: Res<MaterialCache>,
    graph_materials: Option<Res<Assets<GraphMaterial>>>,
    shaders: Option<Res<Assets<Shader>>>,
    validator: Option<ResMut<super::validate::ShaderValidator>>,
    mut problems: ResMut<renzora::content_problems::ContentProblems>,
) {
    let (Some(mut validator), Some(graph_materials), Some(shaders)) =
        (validator, graph_materials, shaders)
    else {
        return;
    };
    let libraries = validator.library_count();

    checked.retain(|path, _| cache.graph_materials.contains_key(path));
    for (path, handle) in &cache.graph_materials {
        let Some(uuid) = graph_materials
            .get(handle)
            .and_then(|m| m.extension.shader_uuid)
        else {
            continue;
        };
        // The library count is part of the key: a verdict reached against a
        // validator that had fewer libraries was reached against imports that
        // had not arrived yet, and has to be taken again.
        if checked.get(path) == Some(&(uuid, libraries)) {
            continue;
        }
        let Some(source) = shaders
            .get(&Handle::<Shader>::Uuid(uuid, PhantomData))
            .and_then(|shader| match &shader.source {
                bevy::shader::Source::Wgsl(src) => Some(src.to_string()),
                _ => None,
            })
        else {
            continue;
        };
        let node_lines = cache.node_line_maps.get(path).map(Vec::as_slice).unwrap_or(&[]);
        let found = validator.problems_for_source(&source, node_lines);
        // Logged to both consoles, once per compile (the uuid key): `warn!`
        // feeds Scene Diagnostics via `runtime_warnings`, `console_error`
        // feeds the Console panel's buffer.
        for problem in &found {
            warn!("{}: {}", path, problem.message);
            renzora::console_log::console_error("Material", format!("{path}: {}", problem.message));
        }
        // Errors only — the compile sites own the Warning rows for this path.
        problems.set_severity(
            path.clone(),
            renzora::content_problems::ProblemSeverity::Error,
            found,
        );
        checked.insert(path.clone(), (uuid, libraries));
    }
}

/// Build the validator once the shader libraries have loaded, and rebuild it
/// whenever their number changes.
///
/// Not at plugin-build time: bevy_pbr's libraries are embedded assets and only
/// land in `Assets<Shader>` over the frames that follow, so a validator built
/// on the first one that appears is missing most of the imports a material
/// needs.
#[cfg(feature = "editor")]
fn ensure_shader_validator(
    mut commands: Commands,
    existing: Option<Res<super::validate::ShaderValidator>>,
    shaders: Option<Res<Assets<Shader>>>,
) {
    let Some(shaders) = shaders else { return };
    let count = super::validate::library_count(&shaders);
    if count == 0 || existing.map(|v| v.library_count()) == Some(count) {
        return;
    }
    commands.insert_resource(super::validate::ShaderValidator::new(&shaders));
}

/// System that finds entities with `MaterialRef` + `Mesh3d` that don't yet have a resolved material,
/// loads the file, compiles it, and attaches the material handle.
#[allow(clippy::too_many_arguments)]
fn resolve_material_refs(
    mut commands: Commands,
    query: Query<(Entity, &MaterialRef), Without<MaterialResolved>>,
    mut cache: ResMut<MaterialCache>,
    mut perf: ResMut<super::perf::MaterialPerfStats>,
    mut problems: ResMut<renzora::content_problems::ContentProblems>,
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
    use super::perf::MaterialKind;
    // `bevy::platform::time::Instant`, never `std`'s — std's panics on wasm.
    use bevy::platform::time::Instant;
    let Some(mut standard_materials) = standard_materials else {
        return;
    };
    let Some(mut graph_materials) = graph_materials else {
        return;
    };
    let Some(mut code_materials) = code_materials else {
        return;
    };
    let Some(mut shaders) = shaders else {
        return;
    };
    let Some(mut shader_state) = shader_state else {
        return;
    };
    let Some(mut shader_cache) = shader_cache else {
        return;
    };
    let Some(shader_registry) = shader_registry else {
        return;
    };
    let default_reader = renzora::VirtualFileReader::default();
    let reader = file_reader.as_deref().unwrap_or(&default_reader);
    for (entity, mat_ref) in query.iter() {
        let path = &mat_ref.0;

        // Check the trivial (StandardMaterial) cache first — it's the fast
        // path for imported materials.
        if let Some(handle) = cache.standard_materials.get(path) {
            // Strip any leftover GraphMaterial from a prior render frame so
            // the entity ends up with exactly one MeshMaterial3d component.
            commands
                .entity(entity)
                .remove::<MeshMaterial3d<GraphMaterial>>();
            commands.entity(entity).try_insert((
                MeshMaterial3d(handle.clone()),
                MaterialResolved {
                    source_path: path.clone(),
                },
            ));
            perf.record_cache_hit(path, MaterialKind::Standard);
            continue;
        }

        // Check graph material cache (procedural path)
        if let Some(handle) = cache.graph_materials.get(path) {
            commands
                .entity(entity)
                .remove::<MeshMaterial3d<bevy::pbr::StandardMaterial>>();
            commands.entity(entity).try_insert((
                MeshMaterial3d(handle.clone()),
                MaterialResolved {
                    source_path: path.clone(),
                },
            ));
            perf.record_cache_hit(path, MaterialKind::Graph);
            continue;
        }

        // Check code material cache
        if let Some(handle) = cache.code_materials.get(path) {
            commands.entity(entity).try_insert((
                MeshMaterial3d(handle.clone()),
                MaterialResolved {
                    source_path: path.clone(),
                },
            ));
            perf.record_cache_hit(path, MaterialKind::Code);
            continue;
        }

        // Resolve asset-relative path to full filesystem path via CurrentProject.
        // Normalize to forward slashes and strip leading "./" for rpak compatibility.
        let fs_path = if std::path::Path::new(path).is_absolute() {
            path.clone()
        } else if let Some(ref proj) = project {
            let raw = proj.resolve_path(path).to_string_lossy().to_string();
            let normalized = raw.replace('\\', "/");
            normalized
                .strip_prefix("./")
                .unwrap_or(&normalized)
                .to_string()
        } else {
            path.clone()
        };

        // Determine file type and resolve. `.material` is the unified
        // extension for both masters and derived (instance) files —
        // content distinguishes (presence of a non-empty `master`
        // field). `.material_instance` is kept as a deprecated suffix
        // so projects with old files keep loading; new instances are
        // always written as `.material`.
        if path.ends_with(".material") || path.ends_with(".material_instance") {
            let is_derived = is_derived_material_file(&fs_path, reader);
            let start = Instant::now();
            let result = if is_derived {
                resolve_material_instance_file(
                    &fs_path,
                    project.as_deref(),
                    &mut cache,
                    &mut standard_materials,
                    &mut graph_materials,
                    &mut shaders,
                    &mut shader_state,
                    &fallback_texture,
                    &asset_server,
                    reader,
                )
            } else {
                resolve_material_file(
                    &fs_path,
                    project.as_deref(),
                    &mut standard_materials,
                    &mut graph_materials,
                    &mut shaders,
                    &mut shader_state,
                    &fallback_texture,
                    &asset_server,
                    reader,
                )
            };
            let compile_dur = start.elapsed();
            match result {
                Some(CompiledMaterial::Standard(handle)) => {
                    // A trivial material is a plain `StandardMaterial` on Bevy's
                    // own pipeline — no generated WGSL, so nothing to be wrong.
                    problems.clear_path(path);
                    cache
                        .standard_materials
                        .insert(path.clone(), handle.clone());
                    commands
                        .entity(entity)
                        .remove::<MeshMaterial3d<GraphMaterial>>();
                    commands.entity(entity).try_insert((
                        MeshMaterial3d(handle),
                        MaterialResolved {
                            source_path: path.clone(),
                        },
                    ));
                    perf.record_compile_success(path, MaterialKind::Standard, compile_dur);
                }
                Some(CompiledMaterial::Graph { handle, parameters, warnings, node_lines }) => {
                    cache.graph_materials.insert(path.clone(), handle.clone());
                    // This resolve ran the codegen, so it owns the path's
                    // Warning rows. `None` (precompiled, instance) leaves
                    // them to whoever compiled the shader.
                    if let Some(warnings) = warnings {
                        problems.set_severity(
                            path.clone(),
                            renzora::content_problems::ProblemSeverity::Warning,
                            warnings
                                .iter()
                                .map(|w| renzora::content_problems::ContentProblem {
                                    severity: renzora::content_problems::ProblemSeverity::Warning,
                                    message: w.clone(),
                                    line: None,
                                    node_id: None,
                                })
                                .collect(),
                        );
                    }
                    // Master-meta only applies to non-derived files —
                    // derived files inherit their master's parameter
                    // list, which we don't re-cache here.
                    if !is_derived {
                        cache
                            .master_meta
                            .insert(path.clone(), MasterMeta { parameters });
                        cache.node_line_maps.insert(path.clone(), node_lines);
                    }
                    commands
                        .entity(entity)
                        .remove::<MeshMaterial3d<bevy::pbr::StandardMaterial>>();
                    commands.entity(entity).try_insert((
                        MeshMaterial3d(handle),
                        MaterialResolved {
                            source_path: path.clone(),
                        },
                    ));
                    perf.record_compile_success(path, MaterialKind::Graph, compile_dur);
                }
                None => {
                    let err = format!(
                        "compile returned None (derived={}, fs_path={})",
                        is_derived, fs_path
                    );
                    warn!("Failed to resolve material file: {} ({})", path, err);
                    renzora::console_log::console_error("Material", format!("Failed to resolve {path}: {err}"));
                    problems.set(
                        path.clone(),
                        vec![renzora::content_problems::ContentProblem {
                            severity: renzora::content_problems::ProblemSeverity::Error,
                            message: err.clone(),
                            line: None,
                            node_id: None,
                        }],
                    );
                    commands.entity(entity).try_insert(MaterialResolved {
                        source_path: path.clone(),
                    });
                    perf.record_compile_failure(path, compile_dur, err);
                }
            }
        } else if path.ends_with(".shader") {
            let start = Instant::now();
            let result = resolve_code_shader(
                &fs_path,
                &mut code_materials,
                &mut shaders,
                &mut shader_cache,
                &shader_registry,
                reader,
            );
            let compile_dur = start.elapsed();
            match result {
                Some(handle) => {
                    cache.code_materials.insert(path.clone(), handle.clone());
                    commands
                        .entity(entity)
                        .remove::<MeshMaterial3d<bevy::pbr::StandardMaterial>>();
                    commands.entity(entity).try_insert((
                        MeshMaterial3d(handle),
                        MaterialResolved {
                            source_path: path.clone(),
                        },
                    ));
                    perf.record_compile_success(path, MaterialKind::Code, compile_dur);
                }
                None => {
                    let err = format!("resolve_code_shader returned None ({})", fs_path);
                    warn!("Failed to resolve .shader: {}", path);
                    commands.entity(entity).try_insert(MaterialResolved {
                        source_path: path.clone(),
                    });
                    perf.record_compile_failure(path, compile_dur, err);
                }
            }
        } else if path.ends_with(".wgsl")
            || path.ends_with(".glsl")
            || path.ends_with(".frag")
            || path.ends_with(".vert")
        {
            // A `.wgsl` paired with a `<path>.meta` sidecar is compiled output
            // from the legacy three-file layout — assemble it as a
            // GraphMaterial so a scene still renders if something points
            // straight at one. Current editors embed the shader in the
            // `.material` and write no `.wgsl` at all.
            //
            // No sidecar → hand-written shader, which is a first-class asset
            // in its own right: it falls through to the raw-shader path and
            // stays editable in place (including hot-reload).
            if path.ends_with(".wgsl") {
                if let Some((wgsl, meta)) =
                    load_compiled_from_vfs(&fs_path, project.as_deref(), reader)
                {
                    let (handle, parameters) = assemble_graph_material(
                        &fs_path,
                        wgsl,
                        meta.domain,
                        meta.alpha_mode,
                        meta.double_sided,
                        meta.requires_transmission,
                        &meta.texture_bindings,
                        meta.parameters,
                        // Legacy `.wgsl` + `.meta` layout — no graph beside it.
                        None,
                        &mut graph_materials,
                        &mut shaders,
                        &fallback_texture,
                        &asset_server,
                    );
                    cache.graph_materials.insert(path.clone(), handle.clone());
                    cache
                        .master_meta
                        .insert(path.clone(), MasterMeta { parameters });
                    commands
                        .entity(entity)
                        .remove::<MeshMaterial3d<bevy::pbr::StandardMaterial>>();
                    commands.entity(entity).try_insert((
                        MeshMaterial3d(handle),
                        MaterialResolved {
                            source_path: path.clone(),
                        },
                    ));
                    continue;
                }
            }
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
                    commands
                        .entity(entity)
                        .remove::<MeshMaterial3d<bevy::pbr::StandardMaterial>>();
                    commands.entity(entity).try_insert((
                        MeshMaterial3d(handle),
                        MaterialResolved {
                            source_path: path.clone(),
                        },
                    ));
                }
                None => {
                    warn!("Failed to resolve raw shader: {}", path);
                    commands.entity(entity).try_insert(MaterialResolved {
                        source_path: path.clone(),
                    });
                }
            }
        } else {
            warn!("MaterialRef has unsupported extension: {}", path);
            commands.entity(entity).try_insert(MaterialResolved {
                source_path: path.clone(),
            });
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
/// Detect whether a `.material` file is a *derived* (instance) material
/// — i.e. it has a non-empty `master` field pointing at another file —
/// or a *master* (graph) material. Used by the resolver to dispatch
/// without relying on file-extension hints. Reads the file once via
/// the same VFS reader the resolvers use.
///
/// A master `.material` deserializes as [`MaterialGraph`] (`nodes`,
/// `domain`, etc.), which has no `master` field; `MaterialInstance`
/// deserialization fails on it because `master` is required.
/// A derived `.material` has the inverse shape.
///
/// Returns `false` on read errors / parse failures so missing or
/// malformed files fall through to the master-resolver path, which
/// already handles those cases gracefully.
fn is_derived_material_file(path: &str, reader: &renzora::VirtualFileReader) -> bool {
    use super::instance::MaterialInstance;
    let Some(content) = reader.read_string(path) else {
        return false;
    };
    serde_json::from_str::<MaterialInstance>(&content)
        .map(|inst| !inst.master.is_empty())
        .unwrap_or(false)
}

/// produce a plain `Handle<StandardMaterial>` — same Material asset every
/// other PBR mesh in the scene uses, so we share the stock PBR pipeline and
/// pay zero per-material shader compile.
///
/// Returns `None` if the file is missing, unparseable, or — for procedural
/// graphs — fails to compile to WGSL.
fn resolve_material_file(
    path: &str,
    project: Option<&renzora::CurrentProject>,
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
    if let Some(mat) = super::standard_build::try_build_standard_material(&graph, asset_server) {
        return Some(CompiledMaterial::Standard(standard_materials.add(mat)));
    }

    // Compiled shader baked into the `.material` on save — the normal path.
    // Everything the renderer needs is right here, so codegen never runs.
    if let Some(compiled) = graph.compiled.as_ref() {
        let meta = &compiled.meta;
        let node_lines = meta.node_line_map.clone();
        let (handle, parameters) = assemble_graph_material(
            path,
            compiled.wgsl.clone(),
            meta.domain,
            meta.alpha_mode,
            meta.double_sided,
            meta.requires_transmission,
            &meta.texture_bindings,
            meta.parameters.clone(),
            Some(&graph),
            graph_materials,
            shaders,
            fallback_texture,
            asset_server,
        );
        return Some(CompiledMaterial::Graph { handle, parameters, warnings: None, node_lines });
    }

    // Legacy three-file layout: the shader lives in a `.wgsl` beside this
    // material with a `.wgsl.meta` sidecar. Materials written before the
    // artifact was embedded land here until their next save.
    if let Some(wgsl_path) = graph.wgsl_path.as_deref() {
        if let Some((wgsl, meta)) = load_compiled_from_vfs(wgsl_path, project, reader) {
            let node_lines = meta.node_line_map.clone();
            let (handle, parameters) = assemble_graph_material(
                wgsl_path,
                wgsl,
                meta.domain,
                meta.alpha_mode,
                meta.double_sided,
                meta.requires_transmission,
                &meta.texture_bindings,
                meta.parameters,
                // The shader came from a sidecar, but the graph is right here
                // in the `.material` — approximate the base from it.
                Some(&graph),
                graph_materials,
                shaders,
                fallback_texture,
                asset_server,
            );
            return Some(CompiledMaterial::Graph { handle, parameters, warnings: None, node_lines });
        }
        warn!(
            "Material '{}' references missing or unreadable wgsl '{}'; falling back to live codegen",
            path, wgsl_path
        );
    }

    // Nothing compiled to load — a graph that has never been saved, or one
    // whose last compile failed. Run the full graph→WGSL codegen now so it
    // still renders while the user is working on it.
    let (handle, parameters, warnings, node_lines) = resolve_graph_material_from_graph(
        path,
        &graph,
        graph_materials,
        shaders,
        shader_state,
        fallback_texture,
        asset_server,
    )?;
    Some(CompiledMaterial::Graph { handle, parameters, warnings: Some(warnings), node_lines })
}

/// Read a `.wgsl` plus its `.wgsl.meta` sidecar via the project VFS. Both
/// must be present for this to succeed — a `.wgsl` without a sidecar isn't
/// a graph-material artifact (could be a hand-written code shader, handled
/// elsewhere) and can't be assembled into a `GraphMaterial`.
///
/// This reads the **legacy** three-file layout. Materials saved by a current
/// editor carry their shader in `MaterialGraph::compiled` and never reach
/// here; this exists so a project authored before that keeps rendering until
/// its materials are next saved.
///
/// `wgsl_path` is project-relative (as stored in `MaterialGraph::wgsl_path`).
/// The runtime VFS reader uses project-relative keys directly. The editor's
/// disk-backed default reader needs absolute paths, so resolve through the
/// project the same way the `.material` lookup does.
fn load_compiled_from_vfs(
    wgsl_path: &str,
    project: Option<&renzora::CurrentProject>,
    reader: &renzora::VirtualFileReader,
) -> Option<(String, super::precompiled::CompiledMaterialMeta)> {
    let resolve = |key: &str| -> String {
        if std::path::Path::new(key).is_absolute() {
            return key.to_string();
        }
        if let Some(proj) = project {
            let raw = proj.resolve_path(key).to_string_lossy().to_string();
            let normalized = raw.replace('\\', "/");
            return normalized
                .strip_prefix("./")
                .unwrap_or(&normalized)
                .to_string();
        }
        key.to_string()
    };

    let wgsl = reader.read_string(&resolve(wgsl_path))?;
    let meta_key = format!("{}.meta", wgsl_path);
    let meta_json = reader.read_string(&resolve(&meta_key))?;
    let meta: super::precompiled::CompiledMaterialMeta = match serde_json::from_str(&meta_json) {
        Ok(m) => m,
        Err(e) => {
            error!("Failed to parse '{}': {}", meta_key, e);
            return None;
        }
    };
    Some((wgsl, meta))
}

/// Read a `.material_instance` file, resolve its master, splice in the
/// instance's parameter overrides, and produce a fresh material handle.
///
/// Two paths:
///
/// **Trivial master** — classifier accepts `param/*` nodes by reading the
/// (override-patched) defaults, so overrides land on `StandardMaterial`
/// fields naturally. Each instance gets its own `Handle<StandardMaterial>`.
///
/// **Procedural master** — the master is compiled once (cached under its
/// own path); each instance clones the master's `GraphMaterial` and rewrites
/// only the parameter UBO with the instance's overrides on top of the
/// master's authored defaults. The shader UUID and texture bindings are
/// inherited from the master, so wgpu reuses the same specialized pipeline
/// across every instance.
fn resolve_material_instance_file(
    path: &str,
    project: Option<&renzora::CurrentProject>,
    cache: &mut MaterialCache,
    standard_materials: &mut Assets<bevy::pbr::StandardMaterial>,
    graph_materials: &mut Assets<GraphMaterial>,
    shaders: &mut Assets<Shader>,
    shader_state: &mut GraphMaterialShaderState,
    fallback_texture: &Option<Res<FallbackTexture>>,
    asset_server: &AssetServer,
    reader: &renzora::VirtualFileReader,
) -> Option<CompiledMaterial> {
    use super::instance::{
        apply_overrides_to_param_slots, graph_with_overrides_applied, MaterialInstance,
    };

    let content = match reader.read_string(path) {
        Some(c) => c,
        None => {
            error!("Failed to read material_instance file '{}'", path);
            return None;
        }
    };

    let instance: MaterialInstance = match serde_json::from_str(&content) {
        Ok(i) => i,
        Err(e) => {
            error!("Failed to parse material_instance '{}': {}", path, e);
            return None;
        }
    };

    // The instance stores `master` as an asset-relative path. Use it both
    // as the cache key (for downstream `cache.graph_materials` and
    // `cache.master_meta` lookups) and resolve a filesystem variant for the
    // file reader.
    let master_key = instance.master.clone();
    let master_fs_path = if Path::new(&master_key).is_absolute() {
        master_key.clone()
    } else if let Some(proj) = project {
        let raw = proj.resolve_path(&master_key).to_string_lossy().to_string();
        let normalized = raw.replace('\\', "/");
        normalized
            .strip_prefix("./")
            .unwrap_or(&normalized)
            .to_string()
    } else {
        master_key.clone()
    };

    let master_content = match reader.read_string(&master_fs_path) {
        Some(c) => c,
        None => {
            error!(
                "material_instance '{}' references missing master '{}'",
                path, master_fs_path
            );
            return None;
        }
    };

    let master_graph: MaterialGraph = match serde_json::from_str(&master_content) {
        Ok(g) => g,
        Err(e) => {
            error!(
                "Failed to parse master '{}' for instance '{}': {}",
                master_fs_path, path, e
            );
            return None;
        }
    };

    // ── Trivial path ────────────────────────────────────────────────────
    //
    // Patch overrides into the graph and re-classify. If the master qualifies
    // as trivial (and overrides don't introduce a node that would push it
    // off the trivial path — they can't, since they only edit param node
    // defaults), we get a fresh StandardMaterial whose factors and texture
    // handles reflect the overrides. No shader compilation, no uniform
    // plumbing — just a different StandardMaterial asset.
    let patched = graph_with_overrides_applied(&master_graph, &instance.overrides);
    if let Some(mat) = super::standard_build::try_build_standard_material(&patched, asset_server) {
        return Some(CompiledMaterial::Standard(standard_materials.add(mat)));
    }

    // ── Procedural path ────────────────────────────────────────────────
    //
    // 1. Ensure the master is compiled. If it isn't, compile it now and
    //    cache both the GraphMaterial handle and the parameter list. Two
    //    instances of the same master share this compilation.
    if !cache.graph_materials.contains_key(&master_key) {
        // Embedded artifact first, then the legacy `.wgsl` + sidecar for a
        // master an older editor wrote. Either way every instance reuses the
        // master's compiled shader; only the parameter UBO differs.
        let from_precompiled = match master_graph.compiled.as_ref() {
            Some(compiled) => Some((
                master_key.clone(),
                (compiled.wgsl.clone(), compiled.meta.clone()),
            )),
            None => master_graph.wgsl_path.as_deref().and_then(|wp| {
                load_compiled_from_vfs(wp, project, reader).map(|loaded| (wp.to_string(), loaded))
            }),
        };
        let (handle, parameters, node_lines) = if let Some((wgsl_key, (wgsl, meta))) = from_precompiled {
            let node_lines = meta.node_line_map.clone();
            let (handle, parameters) = assemble_graph_material(
                &wgsl_key,
                wgsl,
                meta.domain,
                meta.alpha_mode,
                meta.double_sided,
                meta.requires_transmission,
                &meta.texture_bindings,
                meta.parameters,
                Some(&master_graph),
                graph_materials,
                shaders,
                fallback_texture,
                asset_server,
            );
            (handle, parameters, node_lines)
        } else {
            // Warnings drop here: the console already has them, and the panel
            // hears them under the master's own path when it resolves.
            let (handle, parameters, _, node_lines) = resolve_graph_material_from_graph(
                &master_fs_path,
                &master_graph,
                graph_materials,
                shaders,
                shader_state,
                fallback_texture,
                asset_server,
            )?;
            (handle, parameters, node_lines)
        };
        cache.graph_materials.insert(master_key.clone(), handle);
        cache
            .master_meta
            .insert(master_key.clone(), MasterMeta { parameters });
        cache.node_line_maps.insert(master_key.clone(), node_lines);
    }

    // 2. Pull the master GraphMaterial out of the asset store. We can't
    //    just clone the handle — instances need their own asset (different
    //    parameter UBO contents) — so we deep-clone the asset and add a
    //    new entry.
    let master_handle = cache.graph_materials.get(&master_key)?.clone();
    let master_meta = cache.master_meta.get(&master_key)?.clone();
    let mut instance_mat = graph_materials.get(&master_handle)?.clone();

    // 3. Overlay the instance's overrides on top of the master's defaults.
    //    The master's authored defaults are already in the slots from the
    //    initial compile; we only touch slots whose names appear in the
    //    override map.
    apply_overrides_to_param_slots(
        &mut instance_mat.extension.params.slots,
        &master_meta.parameters,
        &instance.overrides,
    );

    Some(CompiledMaterial::Graph {
        handle: graph_materials.add(instance_mat),
        // No new parameters were discovered — the instance shares the
        // master's parameter list. Returning empty so the dispatch site
        // doesn't accidentally insert a stale entry under the instance path.
        parameters: Vec::new(),
        warnings: None,
        node_lines: Vec::new(),
    })
}

/// Compile an already-parsed [`MaterialGraph`] into a procedural
/// [`GraphMaterial`] (`ExtendedMaterial<StandardMaterial, SurfaceGraphExt>`).
///
/// Returns the asset handle, the parameter list the codegen discovered, and
/// its warnings. The parameter list is what callers cache in `MasterMeta` so
/// material instances of this master can write into the right uniform slots.
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
) -> Option<(Handle<GraphMaterial>, Vec<MaterialParam>, Vec<String>, Vec<(u64, u32, u32)>)> {
    // Load any sibling `.material_function` files in the same directory so the
    // graph can reference them via function/call nodes.
    let functions = load_sibling_functions(path);
    let registry = if functions.is_empty() {
        None
    } else {
        Some(&functions)
    };

    // Compile graph → WGSL
    let result = codegen::compile_with_functions(graph, registry);
    if !result.errors.is_empty() {
        for err in &result.errors {
            error!("Material compile error in '{}': {}", path, err);
            renzora::console_log::console_error("Material", format!("{path}: {err}"));
        }
        return None;
    }
    for warning in &result.warnings {
        warn!("Material compile warning in '{}': {}", path, warning);
        renzora::console_log::console_warn("Material", format!("{path}: {warning}"));
    }

    let (handle, parameters) = assemble_graph_material(
        path,
        result.fragment_shader,
        result.domain,
        graph.alpha_mode,
        graph.double_sided,
        result.requires_transmission,
        &result.texture_bindings,
        result.parameters,
        Some(graph),
        materials,
        shaders,
        fallback_texture,
        asset_server,
    );
    Some((handle, parameters, result.warnings, result.node_lines))
}

/// Build the `GraphMaterial` asset from already-compiled WGSL plus the
/// graph-level fields (alpha mode, double-sided) needed by the asset
/// assembler. Shared between live graph codegen and precompiled artifacts
/// loaded from a packaged build.
#[allow(clippy::too_many_arguments)]
fn assemble_graph_material(
    path: &str,
    fragment_shader: String,
    domain: MaterialDomain,
    alpha_mode: super::graph::AlphaMode,
    double_sided: bool,
    requires_transmission: bool,
    texture_bindings: &[super::codegen::TextureBinding],
    parameters: Vec<MaterialParam>,
    // The graph this material came from, when the caller has it. Used to fill
    // the base `StandardMaterial` with an approximation of the graph — see
    // `standard_build::apply_base_approximation` for why the base being empty
    // broke prepass cutouts, shadows, deferred and Lumen at once. `None` only
    // for the legacy `.wgsl` + `.meta` layout, which has no graph beside it.
    graph: Option<&MaterialGraph>,
    materials: &mut Assets<GraphMaterial>,
    shaders: &mut Assets<Shader>,
    fallback_texture: &Option<Res<FallbackTexture>>,
    asset_server: &AssetServer,
) -> (Handle<GraphMaterial>, Vec<MaterialParam>) {
    // Each material gets its own compiled shader, inserted at a unique
    // `Handle::Uuid`. The UUID lives in the extension's pipeline key so
    // `specialize()` can reconstruct the handle and swap it into the
    // fragment stage. See `surface_ext.rs` for why we can't store the
    // `Handle<Shader>` directly (packed-struct + non-Copy).
    let shader_uuid = Uuid::new_v4();
    let shader_handle: Handle<Shader> = Handle::Uuid(shader_uuid, PhantomData);
    let shader = Shader::from_wgsl(fragment_shader, format!("material://{}", path));
    let _ = shaders.insert(&shader_handle, shader);

    let mut mat = if let Some(fb) = fallback_texture {
        new_graph_material(fb)
    } else {
        new_graph_material(&FallbackTexture(Handle::default()))
    };
    mat.extension.shader_uuid = Some(shader_uuid);

    // Fill the base with as much of the graph as a StandardMaterial can hold,
    // before the authoritative per-material state below overwrites the fields
    // it owns. Every pass except the forward one — prepass, shadow, deferred —
    // shades from this, and Lumen reads its `base_color` on the CPU.
    if let Some(graph) = graph {
        super::standard_build::apply_base_approximation(&mut mat.base, graph, asset_server);
    }

    match domain {
        MaterialDomain::Unlit => {
            mat.base.unlit = true;
        }
        MaterialDomain::Surface | MaterialDomain::Vegetation | MaterialDomain::TerrainLayer => {
            mat.base.unlit = false;
        }
    }

    if requires_transmission {
        mat.base.specular_transmission = 0.5;
    }

    mat.base.alpha_mode = match alpha_mode {
        crate::material::graph::AlphaMode::Opaque => bevy::prelude::AlphaMode::Opaque,
        crate::material::graph::AlphaMode::Mask { cutoff } => {
            bevy::prelude::AlphaMode::Mask(cutoff)
        }
        crate::material::graph::AlphaMode::Blend => bevy::prelude::AlphaMode::Blend,
    };
    mat.base.cull_mode = if double_sided {
        None
    } else {
        Some(bevy::render::render_resource::Face::Back)
    };
    // Both halves of "double sided", not just the culling half. `cull_mode`
    // decides whether the back face is *drawn*; this flag is what sets
    // `STANDARD_MATERIAL_FLAGS_DOUBLE_SIDED_BIT`, which is what makes Bevy
    // flip the normal on the faces it draws. Setting only the first left back
    // faces shaded with the front's normal — a leaf lit from behind came out
    // lit from the front.
    mat.base.double_sided = double_sided;

    for tb in texture_bindings {
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

    mat.extension.params.slots = super::instance::build_default_param_slots(&parameters);

    (materials.add(mat), parameters)
}
