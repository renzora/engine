//! Compiles generated material shaders the way the engine does — naga_oil's
//! composer over the app's own `Assets<Shader>`, then naga.

use bevy::prelude::*;
use bevy::shader::{Shader, ShaderImport};
use naga_oil::compose::{Composer, NagaModuleDescriptor, ShaderDefValue, ShaderType};
use renzora::content_problems::{ContentProblem, ProblemSeverity};
use std::collections::HashMap;

use super::codegen::CompileResult;

#[derive(Debug, Clone)]
pub struct ValidationError {
    /// The pipeline defines this configuration was compiled under, for
    /// example `"VERTEX_UVS_A,VERTEX_COLORS"`. Empty for the no-defines pass.
    pub defines: String,
    /// codespan-rendered diagnostic against the composed shader source.
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[defines: {}] {}", self.defines, self.message)
    }
}

/// A composer preloaded with every shader library an app has registered.
///
/// Building one walks all of bevy_pbr's modules, so it is built once and held
/// rather than made per material.
#[derive(Resource)]
pub struct ShaderValidator {
    composer: Composer,
    library_count: usize,
}

impl ShaderValidator {
    /// Load the shader *libraries* in `shaders` into a composer.
    ///
    /// A module has to be added after everything it imports, so this recurses
    /// through `Shader::imports` rather than trusting `Assets` iteration order.
    pub fn new(shaders: &Assets<Shader>) -> Self {
        let by_path = importable_libraries(shaders);
        let mut composer = Composer::default();
        for shader in by_path.values() {
            add_module(&mut composer, &by_path, shader);
        }
        Self {
            library_count: by_path.len(),
            composer,
        }
    }

    /// How many libraries this composer was built from.
    ///
    /// Shader libraries stream in as assets over several frames, so a validator
    /// built early knows only some of them and rejects valid shaders for an
    /// import it has not seen. Callers compare this against
    /// [`library_count`] to notice they are holding a stale one.
    pub fn library_count(&self) -> usize {
        self.library_count
    }

    /// Compile a [`CompileResult`]'s fragment shader, once per pipeline-define
    /// configuration codegen branches on. `Err` carries one entry per failing
    /// configuration.
    pub fn validate(&mut self, result: &CompileResult) -> Result<(), Vec<ValidationError>> {
        // A codegen error already means the shader is known-incomplete. Compiling
        // its half-built output reports secondary noise instead of the cause.
        if !result.errors.is_empty() {
            return Err(vec![ValidationError {
                defines: String::new(),
                message: format!("codegen errors: {}", result.errors.join("; ")),
            }]);
        }

        let mut errors = Vec::new();
        for defines in FRAGMENT_CONFIGS {
            self.compile_one(&result.fragment_shader, defines, &mut errors);
        }

        // `result.vertex_shader` is left alone: nothing writes it to disk or binds it.

        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    /// What the Problems panel shows for one material: whatever the shader
    /// compiler rejects about the WGSL that is actually going to be bound.
    ///
    /// Takes the source rather than a [`CompileResult`] so it covers a shader
    /// embedded in a saved `.material` as well as one freshly generated — the
    /// resolver has both, and only one of them ever had a `CompileResult`.
    ///
    /// An empty result is what clears a repaired file from the panel.
    ///
    /// One entry per distinct compiler message, not per configuration: a broken
    /// expression fails under all of them, and three copies of one diagnostic
    /// reads as three faults. A message only some configurations produce keeps
    /// its `[defines: …]` prefix, because there the configuration *is* the
    /// finding.
    pub fn problems_for_source(&mut self, source: &str) -> Vec<ContentProblem> {
        let mut errors = Vec::new();
        for defines in FRAGMENT_CONFIGS {
            self.compile_one(source, defines, &mut errors);
        }

        let mut order: Vec<String> = Vec::new();
        let mut configs: HashMap<String, Vec<String>> = HashMap::new();
        for error in errors {
            let seen = configs.entry(error.message.clone()).or_insert_with(|| {
                order.push(error.message.clone());
                Vec::new()
            });
            seen.push(error.defines);
        }

        order
            .into_iter()
            .map(|message| {
                let seen = &configs[&message];
                let message = if seen.len() == FRAGMENT_CONFIGS.len() {
                    message
                } else {
                    format!("[defines: {}] {message}", seen.join(" | "))
                };
                ContentProblem {
                    severity: ProblemSeverity::Error,
                    message,
                    line: None,
                }
            })
            .collect()
    }

    fn compile_one(&mut self, source: &str, defines: &[&str], errors: &mut Vec<ValidationError>) {
        let mut shader_defs = pipeline_shader_defs();
        for d in defines {
            shader_defs.insert((*d).to_string(), ShaderDefValue::Bool(true));
        }

        let module = self.composer.make_naga_module(NagaModuleDescriptor {
            source,
            file_path: "generated.wgsl",
            shader_type: ShaderType::Wgsl,
            shader_defs,
            ..Default::default()
        });

        let label = || defines.join(",");
        match module {
            Ok(module) => {
                if let Err(err) = renzora::wgsl::validate(&module) {
                    errors.push(ValidationError {
                        defines: label(),
                        // Debug dump — validation errors have no `emit_to_string`.
                        message: format!("{err:?}"),
                    });
                }
            }
            Err(err) => errors.push(ValidationError {
                defines: label(),
                message: err.emit_to_string(&self.composer),
            }),
        }
    }
}

/// What `PipelineCache` and `MeshPipeline` put in front of every material
/// shader they compile. Without them bevy_pbr's own libraries do not
/// preprocess, so this is the floor the generated code sits on rather than a
/// choice.
///
/// The storage-buffer count is a device limit in the real pipeline
/// (`PipelineCache::new`). There is no device here, so it takes wgpu's default
/// — the branch every desktop backend picks.
fn pipeline_shader_defs() -> HashMap<String, ShaderDefValue> {
    let limits = bevy::render::settings::WgpuLimits::default();
    HashMap::from([
        (
            "AVAILABLE_STORAGE_BUFFER_BINDINGS".to_string(),
            ShaderDefValue::UInt(limits.max_storage_buffers_per_shader_stage),
        ),
        (
            "MATERIAL_BIND_GROUP".to_string(),
            ShaderDefValue::UInt(bevy::pbr::MATERIAL_BIND_GROUP_INDEX as u32),
        ),
        (
            "VERTEX_OUTPUT_INSTANCE_INDEX".to_string(),
            ShaderDefValue::Bool(true),
        ),
        ("VERTEX_POSITIONS".to_string(), ShaderDefValue::Bool(true)),
    ])
}

/// Every configuration that ships gets compiled: with and without the mesh
/// attributes, and with and without a camera's environment map. The prepass
/// defines are left out — the prepass builds its own pipeline from a different
/// entry point, so those branches are not part of this shader's compilation.
const FRAGMENT_CONFIGS: [&[&str]; 3] = [
    &[],
    &["VERTEX_UVS_A", "VERTEX_COLORS"],
    &["VERTEX_UVS_A", "VERTEX_COLORS", "ENVIRONMENT_MAP"],
];

/// The importable libraries in `shaders`, keyed by import path.
///
/// Only `ShaderImport::Custom` modules qualify — that variant means the
/// source declared `#define_import_path`, which is what makes it importable.
/// The rest are plain files, generated material shaders among them, and
/// adding those puts every compiled material in the import namespace.
fn importable_libraries(shaders: &Assets<Shader>) -> HashMap<&ShaderImport, &Shader> {
    shaders
        .iter()
        .map(|(_, s)| (&s.import_path, s))
        .filter(|(path, _)| matches!(path, ShaderImport::Custom(_)))
        .collect()
}

/// The number of shader libraries currently registered — what
/// [`ShaderValidator::new`] builds from right now.
pub fn library_count(shaders: &Assets<Shader>) -> usize {
    importable_libraries(shaders).len()
}

fn add_module(composer: &mut Composer, by_path: &HashMap<&ShaderImport, &Shader>, shader: &Shader) {
    if composer.contains_module(&shader.import_path.module_name()) {
        return;
    }
    for import in &shader.imports {
        if let Some(dep) = by_path.get(import) {
            add_module(composer, by_path, dep);
        }
    }
    // A library that fails to add is one this validator cannot resolve imports
    // through; the material that needs it then fails with that name in the
    // message, which is more useful than a panic here naming only the library.
    let _ = composer.add_composable_module(shader.into());
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material::codegen;
    use crate::material::graph::{MaterialGraph, PinDir, PinType};
    use crate::material::nodes;
    use std::path::PathBuf;

    fn materials_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/materials")
    }

    /// A validator over the shader libraries a real engine app registers.
    ///
    /// `headless_app` builds `RenderPlugin` with no backend, so there is no
    /// adapter and no `RenderApp` — but `PbrPlugin::build` still runs and its
    /// shader libraries are ordinary assets, which is all the composer needs.
    fn validator() -> ShaderValidator {
        let mut app = renzora_test_harness::headless_app();
        // The libraries are embedded assets, so they arrive through the asset
        // server rather than at plugin-build time. One frame is enough.
        app.update();
        let shaders = app.world().resource::<Assets<Shader>>();
        assert!(
            shaders.iter().any(|(_, s)| s.import_path.module_name().as_ref() == "bevy_pbr::pbr_functions"),
            "bevy_pbr's shader libraries are missing; the composer would resolve no imports"
        );
        ShaderValidator::new(shaders)
    }

    #[test]
    fn every_shipped_material_compiles_to_valid_wgsl() {
        let mut validator = validator();
        let dir = materials_dir();
        let mut count = 0;
        let mut failures = Vec::new();
        for entry in std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("material") {
                continue;
            }
            count += 1;
            let json = std::fs::read_to_string(&path).unwrap();
            let graph: MaterialGraph = match serde_json::from_str(&json) {
                Ok(g) => g,
                Err(e) => {
                    failures.push(format!("{}: graph JSON does not parse: {e}", path.display()));
                    continue;
                }
            };
            let result = codegen::compile(&graph);
            if let Err(errors) = validator.validate(&result) {
                for err in errors {
                    failures.push(format!("{}: {err}", path.display()));
                }
            }
        }
        assert!(count > 0, "no .material files found under {}", dir.display());
        assert!(
            failures.is_empty(),
            "{count} materials, {} validation failures:\n{}",
            failures.len(),
            failures.join("\n\n")
        );
    }

    /// Build a single-node graph exercising `node_type`: add the node, wire
    /// every one of its output pins into a compatible input on the output
    /// node, and compile. Nodes whose outputs no output pin accepts
    /// (Texture2D, Sampler, String) are wired through one intermediate node
    /// that does accept them — codegen only visits nodes reachable from the
    /// output node, so an unconnected node generates nothing and the test
    /// passes vacuously.
    fn graph_exercising(node_type: &str) -> Option<MaterialGraph> {
        let def = nodes::node_def(node_type)?;
        let mut graph = MaterialGraph::new("coverage", crate::material::graph::MaterialDomain::Surface);
        let node_id = graph.add_node(node_type, [0.0, 0.0]);
        let output_id = graph.output_node().unwrap().id;

        let outputs: Vec<_> = (def.pins)()
            .into_iter()
            .filter(|p| p.direction == PinDir::Output)
            .collect();

        for pin in &outputs {
            // A pin with no route to the output node stays disconnected.
            // Other pins can still reach it.
            let _ = try_wire(&mut graph, node_id, &pin.name, pin.pin_type, output_id);
        }
        Some(graph)
    }

    /// Wire `from_pin` on `from_node` toward the output node, directly or via
    /// one intermediate. Returns the connection made.
    fn try_wire(
        graph: &mut MaterialGraph,
        from_node: u64,
        from_pin: &str,
        from_type: PinType,
        output_id: u64,
    ) -> Option<()> {
        // Direct: a compatible input on the output node.
        let output_def = nodes::node_def(&graph.get_node(output_id)?.node_type)?;
        if let Some(target) = (output_def.pins)().into_iter().find(|p| {
            p.direction == PinDir::Input && PinType::compatible(from_type, p.pin_type)
        }) {
            graph.connect(from_node, from_pin, output_id, &target.name);
            return Some(());
        }
        // Via one intermediate node that accepts this type and can itself
        // reach the output node.
        for mid in nodes::ALL_NODES {
            if mid.node_type.starts_with("output/") || mid.node_type.starts_with("function/") {
                continue;
            }
            let mid_pins = (mid.pins)();
            let Some(mid_input) = mid_pins.iter().find(|p| {
                p.direction == PinDir::Input && PinType::compatible(from_type, p.pin_type)
            }) else {
                continue;
            };
            let Some(mid_output) = mid_pins.iter().find(|p| {
                p.direction == PinDir::Output
                    && (output_def.pins)().iter().any(|t| {
                        t.direction == PinDir::Input && PinType::compatible(p.pin_type, t.pin_type)
                    })
            }) else {
                continue;
            };
            let mid_id = graph.add_node(mid.node_type, [100.0, 0.0]);
            let mid_input_name = mid_input.name.clone();
            let mid_output_name = mid_output.name.clone();
            graph.connect(from_node, from_pin, mid_id, &mid_input_name);
            let target = (output_def.pins)()
                .into_iter()
                .find(|t| {
                    t.direction == PinDir::Input
                        && PinType::compatible(mid_output.pin_type, t.pin_type)
                })?;
            graph.connect(mid_id, &mid_output_name, output_id, &target.name);
            return Some(());
        }
        None
    }

    #[test]
    fn every_node_type_generates_valid_wgsl() {
        let mut validator = validator();
        let mut failures = Vec::new();
        let mut count = 0;
        for def in nodes::ALL_NODES {
            let node_type = def.node_type;
            // Output nodes are the sink the harness builds around, and
            // function bracket nodes only codegen inside a MaterialFunction.
            if node_type.starts_with("output/") || node_type.starts_with("function/") {
                continue;
            }
            count += 1;
            let Some(graph) = graph_exercising(node_type) else {
                failures.push(format!("{node_type}: unknown to nodes::node_def"));
                continue;
            };
            let result = codegen::compile(&graph);
            if let Err(errors) = validator.validate(&result) {
                for err in errors {
                    failures.push(format!("{node_type}: {err}"));
                }
            }
        }
        assert!(
            failures.is_empty(),
            "{count} node types exercised, {} validation failures:\n{}",
            failures.len(),
            failures.join("\n\n")
        );
    }

    /// `param/bool` is the only Bool *output* pin in the node set and its
    /// codegen emits a real WGSL `bool`, which `cast_expr` used to hand to a
    /// float pin unchanged.
    #[test]
    fn param_bool_wired_into_float_pin_validates() {
        let mut graph = MaterialGraph::new(
            "bool_into_float",
            crate::material::graph::MaterialDomain::Surface,
        );
        let param = graph.add_node("param/bool", [0.0, 0.0]);
        let output_id = graph.output_node().unwrap().id;
        graph.connect(param, "value", output_id, "metallic");

        let result = codegen::compile(&graph);
        validator().validate(&result).unwrap_or_else(|errors| {
            panic!(
                "param/bool → metallic must validate:\n{}",
                errors
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
    }
}
