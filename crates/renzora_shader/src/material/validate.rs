//! Compiles generated material shaders the way the engine does: the WESL
//! compiler over the app's own `Assets<Shader>` libraries, then naga.
//!
//! Material codegen, and every `.material` saved before Bevy 0.20, writes the
//! old naga_oil dialect. It reaches the GPU through
//! [`renzora::shader_source`], which translates it to WESL, so this compiles
//! the same translation and maps errors back to lines of the source codegen
//! wrote, which is what the node source map indexes.

use bevy::prelude::*;
use bevy::shader::{Shader, ShaderDefVal, ShaderImport, Source};
use renzora::content_problems::{ContentProblem, ProblemSeverity};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use wesl::syntax::{ModulePath, PathOrigin};

use super::codegen::CompileResult;

/// The module path the generated shader is compiled under. Never imported by
/// anything; it only has to differ from every library's.
const GENERATED_MODULE: &str = "renzora_generated_material";

#[derive(Debug, Clone)]
pub struct ValidationError {
    /// The pipeline defines this configuration was compiled under, for
    /// example `"VERTEX_UVS_A,VERTEX_COLORS"`. Empty for the no-defines pass.
    pub defines: String,
    /// codespan-rendered diagnostic against the composed shader source.
    pub message: String,
    /// 1-based line in the *generated* fragment shader, when the span points
    /// at it. `None` for errors inside bevy_pbr's libraries or on a line the
    /// translation synthesized: those are engine bugs, not user bugs.
    pub line: Option<u32>,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[defines: {}] {}", self.defines, self.message)
    }
}

/// Every shader library an app has registered, by WESL module path.
///
/// A snapshot of the sources, taken once and held rather than per material:
/// the WESL compiler resolves imports from it on demand, so unlike the old
/// composer nothing has to be added in dependency order.
///
/// Cheap to clone (the sources sit behind an `Arc`) and compiles through
/// `&self`, so the editor can hand a copy to a background task: one material
/// costs tens of milliseconds to compile, and doing a scene's worth of them on
/// the main thread froze the editor while a project loaded.
#[derive(Resource, Clone)]
pub struct ShaderValidator {
    modules: Arc<HashMap<ModulePath, String>>,
    /// `const NAME = value;` for every numeric pipeline define, served as the
    /// `constants` module the way Bevy's pipeline cache does.
    constants: String,
    /// Flags every compile enables: the pipeline's, plus each numeric define's
    /// name (a numeric define also turns on its flag in 0.20) and any boolean
    /// define a library was loaded with.
    flags: Vec<String>,
    library_count: usize,
    unresolved: Vec<String>,
}

impl wesl::Resolver for ShaderValidator {
    fn resolve_source<'a>(
        &'a self,
        path: &ModulePath,
    ) -> Result<Cow<'a, str>, wesl::error::ResolveError> {
        let path = self.canonical_path(path);
        if path.origin == PathOrigin::Package("constants".into()) && path.components.is_empty() {
            return Ok(Cow::Borrowed(&self.constants));
        }
        self.modules
            .get(&path)
            .map(|s| Cow::Borrowed(s.as_str()))
            .ok_or_else(|| {
                wesl::error::ResolveError::ModuleNotFound(path.clone(), "not registered".into())
            })
    }

    // Bevy's own resolver does the same: a `bevy_pbr::...` path written inside
    // package `x` arrives as package `x/bevy_pbr`.
    fn canonical_path(&self, path: &ModulePath) -> ModulePath {
        match &path.origin {
            PathOrigin::Package(pkg) if pkg.contains('/') => ModulePath {
                origin: PathOrigin::Package(pkg.rsplit('/').next().unwrap_or(pkg).to_string()),
                components: path.components.clone(),
            },
            _ => path.clone(),
        }
    }
}

/// The libraries plus the one generated module being compiled.
///
/// The generated module is served from here rather than added to the shared
/// snapshot, which is what lets several compiles run at once against it.
struct WithGenerated<'a> {
    libraries: &'a ShaderValidator,
    main: &'a ModulePath,
    source: &'a str,
}

impl wesl::Resolver for WithGenerated<'_> {
    fn resolve_source<'a>(
        &'a self,
        path: &ModulePath,
    ) -> Result<Cow<'a, str>, wesl::error::ResolveError> {
        if path == self.main {
            return Ok(Cow::Borrowed(self.source));
        }
        self.libraries.resolve_source(path)
    }

    fn canonical_path(&self, path: &ModulePath) -> ModulePath {
        self.libraries.canonical_path(path)
    }
}

impl ShaderValidator {
    /// Snapshot the shader *libraries* in `shaders`.
    pub fn new(shaders: &Assets<Shader>) -> Self {
        let libraries = importable_libraries(shaders);

        // Bevy attaches some defines to a library rather than a pipeline
        // (`mesh_view_types` carries the light-count limits) and gathers them
        // from every imported library at compile time. Taking the union of
        // all of them here is the same thing for any shader that imports them.
        let mut numeric: std::collections::BTreeMap<String, String> = pipeline_numeric_defs()
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect();
        let mut flags: Vec<String> = PIPELINE_FLAGS.iter().map(|f| f.to_string()).collect();
        for (_, shader) in &libraries {
            for def in &shader.shader_defs {
                match def {
                    ShaderDefVal::Bool(name, true) => flags.push(name.to_string()),
                    ShaderDefVal::Bool(_, false) => {}
                    ShaderDefVal::Int(name, v) => {
                        numeric.insert(name.to_string(), v.to_string());
                    }
                    ShaderDefVal::UInt(name, v) => {
                        numeric.insert(name.to_string(), v.to_string());
                    }
                }
            }
        }
        flags.extend(numeric.keys().cloned());
        let constants = numeric
            .iter()
            .map(|(name, value)| format!("const {name} = {value};\n"))
            .collect();

        let modules: HashMap<ModulePath, String> = libraries
            .into_iter()
            .map(|(path, shader)| (path, shader.source.as_str().to_string()))
            .collect();
        Self {
            library_count: modules.len(),
            modules: Arc::new(modules),
            constants,
            flags,
            unresolved: Vec::new(),
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

    /// Whether a library is in the snapshot, by its WESL module path
    /// (`bevy_pbr::render::pbr_functions`).
    ///
    /// The snapshot only holds sources; a library whose own imports are still
    /// streaming in is present here and fails at compile time naming the
    /// missing import, which is the dependency that was actually late.
    pub fn has_module(&self, module: &str) -> bool {
        module_path(module).is_some_and(|p| self.modules.contains_key(&p))
    }

    /// Libraries that could not be registered, each as `module: reason`.
    ///
    /// Always empty now: WESL resolves imports at compile time, so a library
    /// can no longer be refused on its own. Kept so callers that report it
    /// still compile; a missing import shows up in the material's own error.
    pub fn unresolved_libraries(&self) -> &[String] {
        &self.unresolved
    }

    /// Compile a [`CompileResult`]'s fragment shader, once per pipeline-define
    /// configuration codegen branches on. `Err` carries one entry per failing
    /// configuration.
    pub fn validate(&self, result: &CompileResult) -> Result<(), Vec<ValidationError>> {
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
        let prepared = Prepared::new(&result.fragment_shader);
        for defines in FRAGMENT_CONFIGS {
            self.compile_one(&prepared, defines, &mut errors);
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
        &self,
        source: &str,
        node_lines: &[(u64, u32, u32)],
    ) -> Vec<ContentProblem> {
        let mut errors = Vec::new();
        let prepared = Prepared::new(source);
        for defines in FRAGMENT_CONFIGS {
            self.compile_one(&prepared, defines, &mut errors);
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

    fn compile_one(&self, prepared: &Prepared, defines: &[&str], errors: &mut Vec<ValidationError>) {
        let label = || defines.join(",");
        let mut options = wesl::CompileOptions {
            imports: true,
            condcomp: true,
            // What Bevy's pipeline cache passes; with it on, importing a
            // library item Bevy did not mark public is an error Bevy never sees.
            visibility: false,
            ..Default::default()
        };
        for flag in self.flags.iter().map(String::as_str).chain(defines.iter().copied()) {
            options.features.flags.insert(flag.to_string(), true.into());
        }

        let main = ModulePath {
            origin: PathOrigin::Package(GENERATED_MODULE.into()),
            components: Vec::new(),
        };
        let resolver = WithGenerated {
            libraries: self,
            main: &main,
            source: &prepared.wesl,
        };
        let compiled = wesl::compile(&main, &options, &resolver);

        let wgsl = match compiled {
            Ok(out) => out.to_string(),
            Err(err) => {
                let diagnostic = err.diagnostic();
                // Only an error in the generated module can be a user's: one
                // inside a library is an engine bug, and naming a node for it
                // would send the user hunting in the wrong place.
                let in_generated = diagnostic.detail.module_path.as_ref().is_none_or(|p| *p == main);
                let line = diagnostic
                    .detail
                    .span
                    .filter(|_| in_generated)
                    .and_then(|span| prepared.original_line(span.range().start));
                errors.push(ValidationError {
                    defines: label(),
                    message: diagnostic.render_plain(),
                    line,
                });
                return;
            }
        };

        // What wgpu then does with the output. The WESL compiler resolves
        // names and conditionals but does not type-check, so a wrong type in a
        // user snippet only surfaces here, against the compiled text.
        if let Err(message) = renzora::wgsl::check(&wgsl) {
            let line = first_reported_line(&message)
                .and_then(|l| wgsl.lines().nth(l.saturating_sub(1) as usize))
                .and_then(|text| prepared.line_of_text(text));
            errors.push(ValidationError {
                defines: label(),
                message,
                line,
            });
        }
    }
}

/// A source made ready to compile: translated to WESL once, with what is
/// needed to map a position in the translation back to the line codegen wrote.
struct Prepared {
    original: String,
    wesl: String,
}

impl Prepared {
    fn new(source: &str) -> Self {
        Self {
            original: source.to_string(),
            wesl: renzora::shader_source::to_wesl(source).source,
        }
    }

    /// The original line holding the WESL text at byte `byte`.
    fn original_line(&self, byte: usize) -> Option<u32> {
        let line = self.wesl[..byte.min(self.wesl.len())].lines().count().max(1);
        let text = self.wesl.lines().nth(line - 1)?;
        self.line_of_text(text)
    }

    /// The 1-based original line whose code matches `text`. The translation
    /// moves lines (imports to the top, `@if` attributes in front) but leaves
    /// the code of an ordinary statement as codegen wrote it, so matching the
    /// trimmed text finds it; a line the translation synthesized, or one it
    /// rewrote (an import path), matches nothing and gets no line.
    fn line_of_text(&self, text: &str) -> Option<u32> {
        let want = strip_condition(text.trim());
        if want.is_empty() {
            return None;
        }
        let mut hits = self
            .original
            .lines()
            .enumerate()
            .filter(|(_, l)| l.trim() == want);
        let (index, _) = hits.next()?;
        // Two identical lines are ambiguous: better no node than the wrong one.
        if hits.next().is_some() {
            return None;
        }
        Some(index as u32 + 1)
    }
}

/// `@if(FLAG) statement` with the attribute removed.
fn strip_condition(line: &str) -> &str {
    let Some(rest) = line.strip_prefix("@if(") else { return line };
    let mut depth = 1usize;
    for (i, c) in rest.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return rest[i + 1..].trim_start();
                }
            }
            _ => {}
        }
    }
    line
}

/// The first `:line:col` naga's rendered diagnostic points at, as a line.
fn first_reported_line(message: &str) -> Option<u32> {
    message.lines().find_map(|l| {
        let l = l.trim();
        let rest = l.strip_prefix("┌─ ").or_else(|| l.strip_prefix("--> "))?;
        rest.split(':').nth(1)?.parse().ok()
    })
}

/// Boolean defines `MeshPipeline` puts in front of every material shader.
const PIPELINE_FLAGS: [&str; 2] = ["VERTEX_OUTPUT_INSTANCE_INDEX", "VERTEX_POSITIONS"];

/// Numeric defines `PipelineCache` and `MeshPipeline` add. In 0.20 each is
/// readable as `constants::NAME` and enables a flag of the same name.
///
/// The storage-buffer count is a device limit in the real pipeline
/// (`PipelineCache::new`). There is no device here, so it takes wgpu's default,
/// the branch every desktop backend picks.
fn pipeline_numeric_defs() -> [(&'static str, u32); 3] {
    let limits = bevy::render::settings::WgpuLimits::default();
    [
        (
            "AVAILABLE_STORAGE_BUFFER_BINDINGS",
            limits.max_storage_buffers_per_shader_stage,
        ),
        ("MATERIAL_BIND_GROUP", bevy::pbr::MATERIAL_BIND_GROUP_INDEX as u32),
        // `MeshPipeline::specialize` always sets it, from the camera's
        // transmission quality; 8 is the default (medium) setting.
        ("SCREEN_SPACE_SPECULAR_TRANSMISSION_BLUR_TAPS", 8),
    ]
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

/// The importable WESL libraries in `shaders`, keyed by module path.
///
/// Only `ShaderImport::Custom` modules qualify: that is what an embedded
/// library (`embedded://bevy_pbr/render/mesh_functions.wesl`) gets, and it is
/// what makes it importable by package path. The rest are plain files,
/// generated material shaders among them, and adding those puts every
/// compiled material in the import namespace.
fn importable_libraries(shaders: &Assets<Shader>) -> Vec<(ModulePath, &Shader)> {
    shaders
        .iter()
        .filter(|(_, s)| matches!(s.source, Source::Wesl(_)))
        .filter_map(|(_, s)| match &s.import_path {
            ShaderImport::Custom(name) => Some((module_path(name)?, s)),
            ShaderImport::AssetPath(_) => None,
        })
        .collect()
}

/// `bevy_pbr::render::forward_io` as a package module path, the way Bevy's
/// pipeline cache reads a `ShaderImport::Custom`.
fn module_path(name: &str) -> Option<ModulePath> {
    let mut segments = name.split("::");
    let package = segments.next().filter(|s| !s.is_empty())?;
    let components = segments
        .map(|s| (!s.is_empty()).then(|| s.to_string()))
        .collect::<Option<Vec<_>>>()?;
    Some(ModulePath {
        origin: PathOrigin::Package(package.to_string()),
        components,
    })
}

/// The number of shader libraries currently registered, which is what
/// [`ShaderValidator::new`] builds from right now.
pub fn library_count(shaders: &Assets<Shader>) -> usize {
    importable_libraries(shaders).len()
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
            // Pump until every library the generated shaders import is *usable*.
            // Two weaker conditions were tried here first, and both let CI flake:
            //
            // 1. "The set stopped growing." The libraries stream in over several
            //    frames through the shared IO task pool, so a merely PARTIAL set
            //    holds still for three frames just as convincingly as a complete
            //    one.
            //
            // 2. "The library is loaded." Subtler, and the one that matters: a
            //    library is only usable once **its own imports** have arrived too.
            //    Until then it is a present, valid `Shader` asset that the
            //    composer *rejects*. Ask `Assets` and `pbr_fragment` looks ready
            //    while `mesh_view_bindings` is still in flight — the composer then
            //    drops `pbr_fragment`, and all 152 node types fail with
            //    `required import 'bevy_pbr::pbr_fragment' not found`, naming the
            //    library that is fine instead of the one that is late. It reads
            //    like 152 broken nodes rather than one unfinished load.
            //
            // So the wait condition is now the condition the tests actually depend
            // on: the module is *in the composer*. That is only knowable by
            // building one, hence the rebuild — throttled to at most one per three
            // stable frames, and in practice satisfied on the first attempt.
            let required = ["bevy_pbr::render::pbr_functions", "bevy_pbr::render::pbr_fragment"];
            let mut last = 0usize;
            let mut stable = 0;
            let mut built: Option<ShaderValidator> = None;
            for _ in 0..500 {
                app.update();
                let n = library_count(app.world().resource::<Assets<Shader>>());
                if n != last {
                    stable = 0;
                    last = n;
                    continue;
                }
                stable += 1;
                // `n > 0` because frame one reports zero, which is "stable" too.
                if n == 0 || stable < 3 {
                    continue;
                }
                let candidate = ShaderValidator::new(app.world().resource::<Assets<Shader>>());
                if required.iter().all(|r| candidate.has_module(r)) {
                    built = Some(candidate);
                    break;
                }
                // Something is still arriving. Re-accumulate stability rather
                // than pay for another full composer build next frame.
                stable = 0;
            }
            let validator = built.unwrap_or_else(|| {
                let v = ShaderValidator::new(app.world().resource::<Assets<Shader>>());
                let missing: Vec<&str> = required
                    .iter()
                    .copied()
                    .filter(|r| !v.has_module(r))
                    .collect();
                panic!(
                    "bevy_pbr's shader libraries did not become usable in 500 frames.\n\
                     missing from the composer: {missing:?}\n\
                     refused by the composer ({} of {} libraries): {:#?}",
                    v.unresolved_libraries().len(),
                    v.library_count(),
                    v.unresolved_libraries(),
                )
            });
            std::sync::Mutex::new(validator)
        });
        // A panicking test poisons the lock; the validator itself is intact.
        mutex.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The libraries generated materials import are in the **composer**, not
    /// merely loaded as assets.
    ///
    /// This is the distinction the wait loop turns on, asserted directly so a
    /// regression says which library is missing instead of producing hundreds
    /// of errors that all name the wrong one. A refused library is invisible at
    /// the point of failure — it is dropped, and the error surfaces later on
    /// whichever *material* imported it. When `mesh_view_bindings` was late in
    /// CI, that came out as 456 identical `required import
    /// 'bevy_pbr::pbr_fragment' not found` errors across 152 node types, none
    /// of which mentioned `mesh_view_bindings` at all.
    ///
    /// Note it is only asserted of the modules materials actually import.
    /// "Every library composes" is NOT true here and must not be asserted: a
    /// headless PBR app legitimately refuses `bevy_sprite::mesh2d_functions`
    /// and `bevy_pbr::ssr`, whose own imports belong to pipelines this app
    /// never builds. That is also why the wait loop waits on the required
    /// modules rather than on an empty refusal list — the latter would never
    /// come true and every run would spin out to the frame cap.
    #[test]
    fn required_libraries_reach_the_composer() {
        let validator = validator();
        for module in ["bevy_pbr::render::pbr_functions", "bevy_pbr::render::pbr_fragment"] {
            assert!(
                validator.has_module(module),
                "{module} is not in the composer, so every material importing \
                 it fails naming it whatever the real cause. Refused libraries \
                 ({} of {}): {:#?}",
                validator.unresolved_libraries().len(),
                validator.library_count(),
                validator.unresolved_libraries(),
            );
        }
    }

    #[test]
    fn every_shipped_material_compiles_to_valid_wgsl() {
        let validator = validator();
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
        let validator = validator();
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

        let validator = validator();
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

        let validator = validator();
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

        let validator = validator();
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
