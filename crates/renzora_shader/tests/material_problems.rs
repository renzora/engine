//! End-to-end: a broken material must be visible, and repairing it must clear.
//! Drives the real `MaterialResolverPlugin` against files on disk, so what is
//! asserted is what the editor's Problems panel shows.

use bevy::prelude::*;
use renzora::content_problems::{ContentProblems, ProblemSeverity};
use renzora_shader::material::graph::{MaterialDomain, MaterialGraph, PinValue};
use renzora_shader::material::material_ref::MaterialRef;
use renzora_shader::material::resolver::{MaterialCache, MaterialResolved};

/// A graph whose only node is a `custom/code` snippet driving `base_color`.
///
/// `custom/code` is the one node that still lets a user write arbitrary WGSL,
/// which makes it the only way to author a graph that codegen accepts and the
/// shader compiler rejects — exactly the case the panel exists for.
fn graph_with_snippet(name: &str, code: &str) -> MaterialGraph {
    let mut graph = MaterialGraph::new(name, MaterialDomain::Surface);
    let node = graph.add_node("custom/code", [0.0, 0.0]);
    graph
        .get_node_mut(node)
        .unwrap()
        .input_values
        .insert("code".to_string(), PinValue::String(code.to_string()));
    let output = graph.output_node().unwrap().id;
    graph.connect(node, "result", output, "base_color");
    graph
}

struct Fixture {
    app: App,
    dir: std::path::PathBuf,
    /// Project-relative, the key the panel reports under.
    rel: String,
}

impl Fixture {
    fn new(test_name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "renzora_material_problems_{}_{}",
            std::process::id(),
            test_name
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("materials")).unwrap();

        let mut app = renzora_test_harness::headless_app_with(|app| {
            // The resolver alone resolves nothing: a graph material needs the
            // asset type and its pipeline registered before it can be built.
            app.add_plugins((
                renzora_shader::ShaderPlugin,
                renzora_shader::material::runtime::GraphMaterialPlugin,
                renzora_shader::material::resolver::MaterialResolverPlugin,
            ));
        });
        app.insert_resource(renzora::CurrentProject {
            path: dir.clone(),
            config: Default::default(),
        });

        Self {
            app,
            dir,
            rel: "materials/t.material".to_string(),
        }
    }

    /// Write the graph, embedding the compiled artifact the way an editor
    /// save does.
    fn write(&mut self, code: &str) {
        let fs_path = self.dir.join(&self.rel);
        let mut graph = graph_with_snippet("t", code);
        let (json, _) = renzora_shader::material::precompiled::save_compiled_and_serialize(
            &mut graph, &fs_path,
        )
        .unwrap();
        std::fs::write(&fs_path, json).unwrap();
    }

    /// Write the graph JSON with no embedded artifact, so the resolver runs
    /// its live codegen path — the one that owns codegen warnings.
    fn write_raw(&mut self, graph: &MaterialGraph) {
        let fs_path = self.dir.join(&self.rel);
        let json = serde_json::to_string_pretty(graph).unwrap();
        std::fs::write(&fs_path, json).unwrap();
    }

    fn spawn_user(&mut self) {
        self.app.world_mut().spawn(MaterialRef(self.rel.clone()));
    }

    /// Drop everything cached for the material and let the resolver run again —
    /// the same two steps the editor performs after a save.
    fn recompile(&mut self) {
        let rel = self.rel.clone();
        self.app
            .world_mut()
            .resource_mut::<MaterialCache>()
            .invalidate(&rel);
        let entities: Vec<Entity> = self
            .app
            .world_mut()
            .query_filtered::<Entity, With<MaterialResolved>>()
            .iter(self.app.world())
            .collect();
        for entity in entities {
            self.app
                .world_mut()
                .entity_mut(entity)
                .remove::<MaterialResolved>();
        }
        self.pump();
    }

    /// The validator is built from shader libraries that arrive as assets, so a
    /// single frame is not always enough for the first resolve.
    fn pump(&mut self) {
        for _ in 0..8 {
            self.app.update();
        }
    }

    /// How many times the resolver has compiled the material — the number that
    /// tells a settled material apart from one being rebuilt every frame.
    fn compile_count(&self) -> u64 {
        self.app
            .world()
            .resource::<renzora_shader::material::perf::MaterialPerfStats>()
            .per_path
            .get(&self.rel)
            .map(|p| p.compile_count)
            .unwrap_or(0)
    }

    fn problems(&self) -> Vec<(ProblemSeverity, String)> {
        self.app
            .world()
            .resource::<ContentProblems>()
            .get(&self.rel)
            .iter()
            .map(|p| (p.severity, p.message.clone()))
            .collect()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

const GOOD: &str = "result = vec4<f32>(1.0, 0.0, 0.0, 1.0);";
/// `no_such_symbol` parses as WGSL and passes codegen; only the shader compiler
/// knows it is not a thing.
const BROKEN: &str = "result = vec4<f32>(no_such_symbol, 0.0, 0.0, 1.0);";

/// A graph with `params` uniquely-named `param/float` nodes, chained through
/// `math/add` so the fast path cannot claim it and every parameter gets a
/// slot. Past `MAX_PARAMETER_SLOTS` unique names, codegen warns.
fn graph_with_many_params(name: &str, params: usize) -> MaterialGraph {
    let mut graph = MaterialGraph::new(name, MaterialDomain::Surface);
    let output = graph.output_node().unwrap().id;
    // (node, its output pin) — param/float emits "value", math/add "result".
    let mut acc: Option<(u64, &'static str)> = None;
    for i in 0..params {
        let param = graph.add_node("param/float", [i as f32 * 10.0, 0.0]);
        graph
            .get_node_mut(param)
            .unwrap()
            .input_values
            .insert("name".to_string(), PinValue::String(format!("P{i}")));
        acc = Some(match acc {
            None => (param, "value"),
            Some((prev, prev_pin)) => {
                let add = graph.add_node("math/add", [i as f32 * 10.0, 50.0]);
                graph.connect(prev, prev_pin, add, "a");
                graph.connect(param, "value", add, "b");
                (add, "result")
            }
        });
    }
    let (last, pin) = acc.unwrap();
    graph.connect(last, pin, output, "base_color");
    graph
}

#[test]
fn a_healthy_material_reports_nothing() {
    let mut f = Fixture::new("healthy");
    f.write(GOOD);
    f.spawn_user();
    f.pump();

    assert!(
        f.problems().is_empty(),
        "a material that compiles must report nothing, got {:?}",
        f.problems()
    );
}

#[test]
fn a_broken_material_is_reported_against_its_own_path() {
    let mut f = Fixture::new("broken");
    f.write(BROKEN);
    f.spawn_user();
    f.pump();

    let problems = f.problems();
    assert_eq!(
        problems.len(),
        1,
        "expected exactly one report, got {problems:?}"
    );
    assert_eq!(problems[0].0, ProblemSeverity::Error);
    assert!(
        problems[0].1.contains("no_such_symbol"),
        "the report must name what is actually wrong, got {:?}",
        problems[0].1
    );
}

/// The one the user asked for: a repaired material must stop being reported.
#[test]
fn repairing_a_material_clears_its_report() {
    let mut f = Fixture::new("repair");
    f.write(BROKEN);
    f.spawn_user();
    f.pump();
    assert!(!f.problems().is_empty(), "the break must register first");

    f.write(GOOD);
    f.recompile();

    assert!(
        f.problems().is_empty(),
        "a repaired material must clear, still reporting {:?}",
        f.problems()
    );
}

/// A material nobody touched must not be recompiled.
///
/// Every compile mints a fresh `shader_uuid`, which is part of the pipeline
/// key, so a material that recompiles on a timer gets a fresh pipeline on a
/// timer — which is what a flickering surface looks like. This measures the
/// resolver's half of that: whether the compile itself repeats.
#[test]
fn an_untouched_material_is_compiled_once() {
    let mut f = Fixture::new("stable");
    f.write(GOOD);
    f.spawn_user();
    f.pump();
    assert_eq!(f.compile_count(), 1, "the first resolve must compile once");

    for _ in 0..30 {
        f.app.update();
    }
    assert_eq!(
        f.compile_count(),
        1,
        "nothing changed, so nothing may be compiled again"
    );
}

/// Breaking one material must not silence another, and must not report against
/// the wrong file.
#[test]
fn one_broken_material_does_not_hide_a_healthy_one() {
    let mut f = Fixture::new("two");
    f.write(BROKEN);
    f.spawn_user();

    let other = "materials/other.material".to_string();
    let other_fs = f.dir.join(&other);
    let mut graph = graph_with_snippet("other", GOOD);
    let (json, _) = renzora_shader::material::precompiled::save_compiled_and_serialize(
        &mut graph, &other_fs,
    )
    .unwrap();
    std::fs::write(&other_fs, json).unwrap();
    f.app.world_mut().spawn(MaterialRef(other.clone()));
    f.pump();

    let all = f.app.world().resource::<ContentProblems>();
    assert_eq!(all.get(&other).len(), 0, "the healthy material must be clean");
    assert_eq!(all.error_count(), 1, "only the broken one may be reported");
}

/// The one warning codegen produces — more unique parameter names than the
/// parameter buffer has slots — must reach the panel as a Warning row, and
/// repairing the graph must clear it.
#[test]
fn a_parameter_overflow_warns_and_a_repair_clears_it() {
    let mut f = Fixture::new("overflow");
    let over = renzora_shader::material::codegen::MAX_PARAMETER_SLOTS + 1;
    f.write_raw(&graph_with_many_params("t", over));
    f.spawn_user();
    f.pump();

    let problems = f.problems();
    assert_eq!(
        problems.len(),
        1,
        "one overflow, one row — got {problems:?}"
    );
    assert_eq!(problems[0].0, ProblemSeverity::Warning);
    assert!(
        problems[0].1.contains("exceeds"),
        "the row must say what overflowed, got {:?}",
        problems[0].1
    );

    f.write_raw(&graph_with_many_params("t", 2));
    f.recompile();

    assert!(
        f.problems().is_empty(),
        "a repaired graph must clear its warning, still reporting {:?}",
        f.problems()
    );
}
