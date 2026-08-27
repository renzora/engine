//! Compiles generated material shaders the way the engine does — naga_oil's
//! composer over the app's own `Assets<Shader>`, then naga.

use bevy::prelude::*;
use bevy::shader::{Shader, ShaderImport};
use naga_oil::compose::{
    Composer, ComposerError, ComposerErrorInner, ErrSource, NagaModuleDescriptor,
    ShaderDefValue, ShaderType,
};
use renzora::content_problems::{ContentProblem, ProblemSeverity};
use std::collections::HashMap;

use super::codegen::CompileResult;

/// naga_oil tags composed-module spans with the source-module index in the
/// high bits (`compose/mod.rs` — private there, so mirrored here). Index 0 is
/// the top-level generated source; anything else is a bevy_pbr library,
/// whose errors are engine bugs, not user bugs.
const SPAN_SHIFT: usize = 21;

#[derive(Debug, Clone)]
pub struct ValidationError {
    /// The pipeline defines this configuration was compiled under, for
    /// example `"VERTEX_UVS_A,VERTEX_COLORS"`. Empty for the no-defines pass.
    pub defines: String,
    /// codespan-rendered diagnostic against the composed shader source.
    pub message: String,
    /// 1-based line in the *generated* fragment shader, when the span points
    /// at it. `None` for errors inside bevy_pbr's libraries or in
    /// preprocessor directives — those are engine bugs, not user bugs.
    pub line: Option<u32>,
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
                line: None,
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
    ///
    /// `node_lines` is the source map `CompileResult::node_lines` produced
    /// (and the embedded artifact's meta persisted) at codegen time; it is what
    /// puts an error on a node. Pass `&[]` when the source came from
    /// somewhere with no map — the problem then carries no line or node.
    pub fn problems_for_source(
        &mut self,
        source: &str,
        node_lines: &[(u64, u32, u32)],
    ) -> Vec<ContentProblem> {
        let mut errors = Vec::new();
        for defines in FRAGMENT_CONFIGS {
            self.compile_one(source, defines, &mut errors);
        }

        let mut order: Vec<String> = Vec::new();
        let mut configs: HashMap<String, (Vec<String>, Option<u32>)> = HashMap::new();
        for error in errors {
            let seen = configs.entry(error.message.clone()).or_insert_with(|| {
                order.push(error.message.clone());
                (Vec::new(), error.line)
            });
            seen.0.push(error.defines);
        }

        order
            .into_iter()
            .map(|message| {
                let (seen, line) = &configs[&message];
                let message = if seen.len() == FRAGMENT_CONFIGS.len() {
                    message
                } else {
                    format!("[defines: {}] {message}", seen.join(" | "))
                };
                ContentProblem {
                    severity: ProblemSeverity::Error,
                    message,
                    line: line.map(|l| l as usize),
                    node_id: line.and_then(|l| super::codegen::node_for_line(node_lines, l)),
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
                        // Unreachable in practice: `make_naga_module` already
                        // validated the composed module, so its own error
                        // (with a decodable span) fires first. No line here.
                        message: format!("{err:?}"),
                        line: None,
                    });
                }
            }
            Err(err) => errors.push(ValidationError {
                defines: label(),
                line: error_line(&err, &self.composer),
                message: err.emit_to_string(&self.composer),
            }),
        }
    }
}

/// The 1-based line a `ComposerError` points at in the *generated* source —
/// decodable because naga_oil's preprocessor is line-preserving, so a line in
/// the preprocessed text is the same line in what codegen emitted.
///
/// Only `ErrSource::Constructing` (the top-level generated shader) is
/// decodable. Errors inside imported bevy_pbr modules, and preprocessor
/// errors on directive lines, return `None`: they are engine bugs, and a
/// node attribution would send the user hunting in the wrong place.
fn error_line(err: &ComposerError, composer: &Composer) -> Option<u32> {
    let ErrSource::Constructing { .. } = err.source else {
        return None;
    };
    let source = err.source.source(composer);
    let range = match &err.inner {
        ComposerErrorInner::WgslParseError(parse) => {
            parse.labels().next().and_then(|(span, _)| span.to_range())
        }
        ComposerErrorInner::ShaderValidationError(with_span) => {
            with_span.spans().last().and_then(|(span, _)| span.to_range())
        }
        _ => None,
    }?;
    if range.start >> SPAN_SHIFT != 0 {
        return None;
    }
    let byte = (range.start & ((1 << SPAN_SHIFT) - 1)).saturating_sub(err.source.offset());
    Some(line_of_byte(&source, byte))
}

/// 1-based line number of byte offset `byte` in `source`.
fn line_of_byte(source: &str, byte: usize) -> u32 {
    source[..byte.min(source.len())].matches('\n').count() as u32 + 1
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
    ///
    /// Built once per process and shared: the libraries are embedded assets
    /// that stream in over several frames through the shared IO task pool,
    /// and several headless apps pumping in parallel starve each other — an
    /// app whose turn has not come yet sees a partial set, and its composer
    /// then reports the missing ones as unresolved imports. `get_or_init`
    /// blocks the other callers, so the one app that loads does so alone.
    /// `compile_one` only composes from the registered modules, so sharing
    /// the validator across tests is safe.
    fn validator() -> std::sync::MutexGuard<'static, ShaderValidator> {
        static VALIDATOR: std::sync::OnceLock<std::sync::Mutex<ShaderValidator>> =
            std::sync::OnceLock::new();
        let mutex = VALIDATOR.get_or_init(|| {
            let mut app = renzora_test_harness::headless_app();
            // Pump until every library the generated shaders import is present.
            //
            // "Stopped growing" alone is NOT enough, and trusting it is what made
            // this fail in CI: the libraries stream in over several frames
            // through the shared IO task pool, so a set that is merely PARTIAL
            // holds still for three frames just as convincingly as a complete
            // one. The old spot-check missed it because it asked for
            // `pbr_functions`, which had arrived, while `pbr_fragment` had not —
            // and every one of the 152 node types then failed with
            // `required import 'bevy_pbr::pbr_fragment' not found`, which reads
            // like 152 broken nodes rather than one unfinished load.
            //
            // So the loop waits on the thing that actually has to be true. The
            // frame budget is only a backstop; stability is still required on top,
            // to catch a library that arrives after the ones named here.
            let required = ["bevy_pbr::pbr_functions", "bevy_pbr::pbr_fragment"];
            let present = |app: &bevy::app::App, name: &str| {
                importable_libraries(app.world().resource::<Assets<Shader>>())
                    .keys()
                    .any(|i| i.module_name().as_ref() == name)
            };
            let mut last = 0usize;
            let mut stable = 0;
            for _ in 0..500 {
                app.update();
                let n = importable_libraries(app.world().resource::<Assets<Shader>>()).len();
                if n == last {
                    stable += 1;
                    if stable >= 3 && required.iter().all(|r| present(&app, r)) {
                        break;
                    }
                } else {
                    stable = 0;
                    last = n;
                }
            }
            let missing: Vec<&str> = required
                .iter()
                .copied()
                .filter(|r| !present(&app, r))
                .collect();
            assert!(
                missing.is_empty(),
                "bevy_pbr's shader libraries did not finish loading in 500 frames \
                 (missing: {missing:?}); every import would be reported unresolved"
            );
            let shaders = app.world().resource::<Assets<Shader>>();
            std::sync::Mutex::new(ShaderValidator::new(shaders))
        });
        // A panicking test poisons the lock; the validator itself is intact.
        mutex.lock().unwrap_or_else(|e| e.into_inner())
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

    /// A broken `custom/code` snippet must point at its own line — and,
    /// through the source map, at its own node.
    #[test]
    fn a_broken_snippet_attributes_line_and_node() {
        use crate::material::graph::PinValue;

        let mut graph = MaterialGraph::new("attr", crate::material::graph::MaterialDomain::Surface);
        let node = graph.add_node("custom/code", [0.0, 0.0]);
        graph
            .get_node_mut(node)
            .unwrap()
            .input_values
            .insert(
                "code".to_string(),
                PinValue::String("result = vec4<f32>(no_such_symbol, 0.0, 0.0, 1.0);".to_string()),
            );
        let output_id = graph.output_node().unwrap().id;
        graph.connect(node, "result", output_id, "base_color");

        let result = codegen::compile(&graph);
        let snippet_line = result
            .fragment_shader
            .lines()
            .position(|l| l.contains("no_such_symbol"))
            .map(|i| i as u32 + 1)
            .unwrap();

        let mut validator = validator();
        let errors = validator
            .validate(&result)
            .expect_err("the snippet must fail to compile");
        assert!(
            errors.iter().any(|e| e.line == Some(snippet_line)),
            "no error on the snippet's line {snippet_line}: {errors:?}"
        );

        let problems = validator.problems_for_source(&result.fragment_shader, &result.node_lines);
        assert_eq!(problems.len(), 1, "one fault, one row: {problems:?}");
        assert_eq!(problems[0].line, Some(snippet_line as usize));
        assert_eq!(problems[0].node_id, Some(node));
    }

    // ── Math-node vectorization ─────────────────────────────────────────────

    /// The five ranks a math node can latch to, each with the parameter node
    /// whose `value` output drives it.
    const RANK_SOURCES: [(&str, PinType); 5] = [
        ("param/float", PinType::Float),
        ("param/vec2", PinType::Vec2),
        ("param/vec3", PinType::Vec3),
        ("param/vec4", PinType::Vec4),
        ("param/color", PinType::Color),
    ];

    /// What feeds the node's second dynamic input pin, when it has one.
    #[derive(Clone, Copy, Debug)]
    enum Sibling {
        Unconnected,
        Scalar,
        SameRank,
    }

    /// One matrix cell: `node_type` driven on its first dynamic input by
    /// `src_type`'s `value` output, `result` wired into the output node's
    /// `base_color` so codegen actually visits the node.
    fn vectorization_cell(
        node_type: &str,
        src_type: &str,
        sibling: Sibling,
    ) -> (MaterialGraph, u64) {
        use crate::material::graph::is_dynamic_pin;

        let mut graph =
            MaterialGraph::new("vec", crate::material::graph::MaterialDomain::Surface);
        let m = graph.add_node(node_type, [0.0, 0.0]);
        let s = graph.add_node(src_type, [-200.0, 0.0]);
        let output_id = graph.output_node().unwrap().id;

        let def = nodes::node_def(node_type).unwrap();
        let dyn_inputs: Vec<String> = (def.pins)()
            .into_iter()
            .filter(|p| p.direction == PinDir::Input && is_dynamic_pin(node_type, &p.name))
            .map(|p| p.name)
            .collect();
        graph.connect(s, "value", m, &dyn_inputs[0]);
        if dyn_inputs.len() > 1 {
            match sibling {
                Sibling::Unconnected => {}
                Sibling::Scalar => {
                    let f = graph.add_node("param/float", [-200.0, 100.0]);
                    graph.connect(f, "value", m, &dyn_inputs[1]);
                }
                Sibling::SameRank => {
                    graph.connect(s, "value", m, &dyn_inputs[1]);
                }
            }
        }
        graph.connect(m, "result", output_id, "base_color");
        (graph, m)
    }

    /// Every math node × every driving rank × every sibling shape must latch
    /// to the widest wire and still compile to valid WGSL — this is where the
    /// rank-aware guard literals (`max({b}, vec4<f32>(0.000001))` and friends)
    /// get exercised, since a scalar guard under a latched vector node is a
    /// naga type error.
    #[test]
    fn math_nodes_vectorize_to_widest_wire() {
        use crate::material::graph::{is_dynamic_pin, resolve_math_ranks};

        let mut validator = validator();
        let mut failures = Vec::new();
        let mut count = 0;
        for def in nodes::ALL_NODES {
            let node_type = def.node_type;
            if !node_type.starts_with("math/") {
                continue;
            }
            let dynamic_inputs = (def.pins)()
                .into_iter()
                .filter(|p| p.direction == PinDir::Input && is_dynamic_pin(node_type, &p.name))
                .count();
            for (src_type, rank) in RANK_SOURCES {
                let siblings: &[Sibling] = if dynamic_inputs > 1 {
                    &[Sibling::Unconnected, Sibling::Scalar, Sibling::SameRank]
                } else {
                    &[Sibling::Unconnected]
                };
                for sibling in siblings {
                    count += 1;
                    let (graph, m) = vectorization_cell(node_type, src_type, *sibling);
                    let latched = resolve_math_ranks(&graph)
                        .get(&m)
                        .copied()
                        .unwrap_or(PinType::Float);
                    if latched != rank {
                        failures.push(format!(
                            "{node_type} driven by {src_type} ({sibling:?}): latched {latched:?}, want {rank:?}"
                        ));
                        continue;
                    }
                    let result = codegen::compile(&graph);
                    if let Err(errors) = validator.validate(&result) {
                        for err in errors {
                            failures.push(format!(
                                "{node_type} driven by {src_type} ({sibling:?}): {err}"
                            ));
                        }
                    }
                }
            }
        }
        assert!(
            failures.is_empty(),
            "{count} vectorization cells, {} failures:\n{}",
            failures.len(),
            failures.join("\n\n")
        );
    }

    /// A wide consumer on `result` must not widen the node's inputs — the
    /// node never dictates types downstream, the sink's `cast_expr` narrows.
    #[test]
    fn math_node_does_not_latch_from_downstream() {
        use crate::material::graph::resolve_math_ranks;

        let mut graph =
            MaterialGraph::new("no_back_latch", crate::material::graph::MaterialDomain::Surface);
        let m = graph.add_node("math/add", [0.0, 0.0]);
        let output_id = graph.output_node().unwrap().id;
        // base_color is a Color (rank-4) sink.
        graph.connect(m, "result", output_id, "base_color");
        let latched = resolve_math_ranks(&graph)
            .get(&m)
            .copied()
            .unwrap_or(PinType::Float);
        assert_eq!(latched, PinType::Float, "downstream sink must not latch");

        let result = codegen::compile(&graph);
        validator().validate(&result).unwrap_or_else(|errors| {
            panic!("unlatched add into base_color must validate: {errors:?}")
        });
    }

    /// Removing the wide wire drops the node back to Float, and both shapes
    /// compile — resolution is recomputed, never remembered.
    #[test]
    fn math_node_unlatches_when_the_wide_wire_detaches() {
        use crate::material::graph::resolve_math_ranks;

        let mut validator = validator();
        let (mut graph, m) = vectorization_cell("math/add", "param/vec4", Sibling::Unconnected);
        assert_eq!(resolve_math_ranks(&graph)[&m], PinType::Vec4);
        let result = codegen::compile(&graph);
        validator
            .validate(&result)
            .unwrap_or_else(|e| panic!("latched vec4 add must validate: {e:?}"));

        graph.disconnect(m, "a");
        assert_eq!(resolve_math_ranks(&graph)[&m], PinType::Float);
        let result = codegen::compile(&graph);
        validator
            .validate(&result)
            .unwrap_or_else(|e| panic!("detached add must validate as Float: {e:?}"));
    }

    /// The divide guard splats its epsilon at the latched rank. Widening
    /// through `cast_expr` instead would fill w with `1.0`, clamping the
    /// guard's last component against 1.0 rather than the epsilon.
    #[test]
    fn divide_guard_splats_at_latched_rank() {
        let (graph, _m) = vectorization_cell("math/divide", "param/vec4", Sibling::Unconnected);
        let result = codegen::compile(&graph);
        assert!(
            result.fragment_shader.contains("vec4<f32>(0.000001)"),
            "guard literal must splat at the latched rank:\n{}",
            result.fragment_shader
        );
    }

    /// Latched ranks are re-derived from the loaded graph: a serialized
    /// vec3-latched Add loads, resolves identically, and validates.
    #[test]
    fn latched_graph_round_trips_through_serialization() {
        use crate::material::graph::resolve_math_ranks;

        let (graph, m) = vectorization_cell("math/add", "param/vec3", Sibling::Scalar);
        let json = serde_json::to_string(&graph).unwrap();
        let loaded: MaterialGraph = serde_json::from_str(&json).unwrap();
        assert_eq!(
            resolve_math_ranks(&graph).get(&m),
            resolve_math_ranks(&loaded).get(&m),
            "ranks are a pure function of the graph; a round-trip must not move them"
        );

        let result = codegen::compile(&loaded);
        validator()
            .validate(&result)
            .unwrap_or_else(|e| panic!("round-tripped latched graph must validate: {e:?}"));
    }
}
