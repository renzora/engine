//! Compile the particle shaders without a GPU.
//!
//! A WESL mistake in them only surfaces when a pipeline is built, as an empty
//! viewport and a log line. This compiles both the way their pipelines do,
//! against Bevy's own shader libraries, under each define the pipelines can set,
//! so a broken shader fails here instead. Same approach as `renzora_solari`'s
//! shader test, using the material validator's compiler.

use bevy::prelude::*;
use bevy::shader::Shader;
use renzora_shader::material::validate::{library_count, ShaderValidator};

const SHADER_3D: &str = include_str!("../src/shaders/particle3d.wesl");
const SHADER_2D: &str = include_str!("../src/shaders/particle2d.wesl");
const SHADER_SURFACE: &str = include_str!("../src/shaders/particle_surface.wesl");

/// Libraries the particle shaders import, waited for the way the material
/// validator's own test waits: they stream in over several frames.
const REQUIRED_LIBRARIES: [&str; 18] = [
    "bevy_pbr::render::mesh_functions",
    "bevy_pbr::render::mesh_view_bindings",
    "bevy_pbr::render::view_transformations",
    "bevy_pbr::render::pbr_functions",
    "bevy_pbr::render::pbr_types",
    "bevy_pbr::render::mesh_types",
    "bevy_pbr::render::pbr_fragment",
    "bevy_pbr::render::pbr_prepass_functions",
    "bevy_pbr::render::rgb9e5",
    "bevy_pbr::prepass::utils",
    "bevy_pbr::prepass::io",
    "bevy_pbr::prepass::bindings",
    "bevy_pbr::deferred::functions",
    "bevy_pbr::deferred::types",
    "bevy_render::globals",
    "bevy_sprite_render::mesh2d::functions",
    "bevy_sprite_render::mesh2d::bindings",
    "bevy_core_pipeline::tonemapping",
];

fn validator() -> ShaderValidator {
    validator_and_library("bevy_pbr::prepass::io").0
}

/// The validator, and the WESL text of one of Bevy's libraries as the app
/// registered it, for the tests that hold our shaders to Bevy's interfaces.
fn validator_and_library(library: &str) -> (ShaderValidator, String) {
    let source = |app: &App| {
        app.world().resource::<Assets<Shader>>().iter().find_map(|(_, s)| match (&s.import_path, &s.source) {
            (bevy::shader::ShaderImport::Custom(name), bevy::shader::Source::Wesl(text)) if name == library => Some(text.to_string()),
            _ => None,
        })
    };
    let mut app = renzora_test_harness::headless_app();
    // The 2D mesh bindings are embedded at build but only loaded once a render
    // device says whether storage buffers exist, which a headless app never
    // does. Load the embedded file directly so `functions` can import it.
    let bindings: Handle<Shader> = app
        .world()
        .resource::<AssetServer>()
        .load("embedded://bevy_sprite_render/mesh2d/bindings.wesl");
    // Settled means the count has held for both some frames and some wall
    // time. The list above is only what the shaders import directly; Bevy's
    // libraries pull in many more (lighting, shadows, clustering, fog), and
    // they arrive on the IO pool, not per frame. With the suite's tests all
    // building apps at once, that pool lagged past a three-frame window, the
    // snapshot missed a transitive library, and whichever test compiled a
    // config needing it failed at random. Time is what the pool lags by.
    const SETTLE: std::time::Duration = std::time::Duration::from_millis(300);
    let mut last = 0;
    let mut stable = 0;
    let mut since = std::time::Instant::now();
    for _ in 0..5000 {
        app.update();
        let n = library_count(app.world().resource::<Assets<Shader>>());
        if n != last {
            last = n;
            stable = 0;
            since = std::time::Instant::now();
            continue;
        }
        stable += 1;
        if n == 0 || stable < 3 || since.elapsed() < SETTLE {
            std::thread::sleep(std::time::Duration::from_millis(1));
            continue;
        }
        let candidate = ShaderValidator::new(app.world().resource::<Assets<Shader>>());
        if REQUIRED_LIBRARIES.iter().all(|m| candidate.has_module(m)) {
            drop(bindings);
            let text = source(&app).unwrap_or_else(|| panic!("{library} is not a registered WESL library"));
            return (candidate, text);
        }
        stable = 0;
    }
    let v = ShaderValidator::new(app.world().resource::<Assets<Shader>>());
    let missing: Vec<&str> = REQUIRED_LIBRARIES.iter().copied().filter(|m| !v.has_module(m)).collect();
    panic!("Bevy's shader libraries never all arrived; missing {missing:?}");
}

/// Every define combination the pipelines can compile these under that changes
/// the code: tonemapping in the shader or not, and the view's depth prepass
/// (single and multisampled), which is what switches soft particles on. The 2D
/// shader ignores the prepass defines; compiling it under them costs nothing
/// and proves it.
///
/// `GPU_PARTICLES` is the GPU backend's pipeline: the same shader with the
/// vertex input swapped for a fetch from the simulation's storage buffer.
/// `MESH_PARTICLES` is mesh particles on either backend (the 2D shader never
/// gets it, and compiling it there proves it is inert).
const CONFIGS: [&[&str]; 12] = [
    &[],
    &["TONEMAP_IN_SHADER"],
    &["DEPTH_PREPASS"],
    &["DEPTH_PREPASS", "MULTISAMPLED"],
    &["DEPTH_PREPASS", "TONEMAP_IN_SHADER", "WEBGL2"],
    &["GPU_PARTICLES"],
    &["GPU_PARTICLES", "TONEMAP_IN_SHADER"],
    &["GPU_PARTICLES", "DEPTH_PREPASS", "MULTISAMPLED"],
    &["MESH_PARTICLES"],
    &["MESH_PARTICLES", "DEPTH_PREPASS", "TONEMAP_IN_SHADER"],
    &["GPU_PARTICLES", "MESH_PARTICLES"],
    &["GPU_PARTICLES", "MESH_PARTICLES", "DEPTH_PREPASS", "MULTISAMPLED"],
];

/// The prepass and shadow pipelines Bevy builds from the 3D shaders, as its
/// prepass sets their defines: a shadow pass with nothing to write (and its
/// alpha-cut variant), the full depth, normal and motion-vector prepass, a
/// directional shadow without depth clip control (which writes its own
/// depth), each on quads and meshes, CPU and GPU.
const PREPASS_CONFIGS: [&[&str]; 9] = [
    &["PREPASS_PIPELINE"],
    &["PREPASS_PIPELINE", "MAY_DISCARD"],
    &["PREPASS_PIPELINE", "PREPASS_FRAGMENT", "NORMAL_PREPASS", "NORMAL_PREPASS_OR_DEFERRED_PREPASS", "MOTION_VECTOR_PREPASS", "DEPTH_PREPASS"],
    &["PREPASS_PIPELINE", "PREPASS_FRAGMENT", "MOTION_VECTOR_PREPASS", "MESH_PARTICLES"],
    &["PREPASS_PIPELINE", "PREPASS_FRAGMENT", "NORMAL_PREPASS", "NORMAL_PREPASS_OR_DEFERRED_PREPASS", "GPU_PARTICLES"],
    &["PREPASS_PIPELINE", "PREPASS_FRAGMENT", "MOTION_VECTOR_PREPASS", "GPU_PARTICLES", "MESH_PARTICLES"],
    &["PREPASS_PIPELINE", "PREPASS_FRAGMENT", "UNCLIPPED_DEPTH_ORTHO_EMULATION"],
    &["PREPASS_PIPELINE", "PREPASS_FRAGMENT", "UNCLIPPED_DEPTH_ORTHO_EMULATION", "GPU_PARTICLES", "MESH_PARTICLES"],
    &["PREPASS_PIPELINE", "PREPASS_FRAGMENT", "NORMAL_PREPASS", "NORMAL_PREPASS_OR_DEFERRED_PREPASS", "MOTION_VECTOR_PREPASS", "UNCLIPPED_DEPTH_ORTHO_EMULATION", "MESH_PARTICLES"],
];

/// `particle_surface.wesl` under every define its pipelines set: the main
/// pass (which always has the UV, colour and tangent the surface specialize
/// turns on) and the prepass, each on quads and meshes, CPU and GPU.
const SURFACE_CONFIGS: [&[&str]; 10] = [
    &["VERTEX_UVS_A", "VERTEX_COLORS", "VERTEX_TANGENTS", "RENZORA_PARTICLE_SURFACE"],
    &["VERTEX_UVS_A", "VERTEX_COLORS", "VERTEX_TANGENTS", "RENZORA_PARTICLE_SURFACE", "MESH_PARTICLES"],
    &["VERTEX_UVS_A", "VERTEX_COLORS", "VERTEX_TANGENTS", "RENZORA_PARTICLE_SURFACE", "GPU_PARTICLES"],
    &["VERTEX_UVS_A", "VERTEX_COLORS", "VERTEX_TANGENTS", "RENZORA_PARTICLE_SURFACE", "GPU_PARTICLES", "MESH_PARTICLES"],
    &["PREPASS_PIPELINE"],
    &["PREPASS_PIPELINE", "VERTEX_UVS_A", "VERTEX_COLORS", "VERTEX_TANGENTS", "MAY_DISCARD"],
    &["PREPASS_PIPELINE", "VERTEX_UVS_A", "VERTEX_COLORS", "VERTEX_TANGENTS", "PREPASS_FRAGMENT", "NORMAL_PREPASS", "NORMAL_PREPASS_OR_DEFERRED_PREPASS", "MOTION_VECTOR_PREPASS"],
    &["PREPASS_PIPELINE", "VERTEX_UVS_A", "VERTEX_TANGENTS", "PREPASS_FRAGMENT", "NORMAL_PREPASS_OR_DEFERRED_PREPASS", "MOTION_VECTOR_PREPASS", "MESH_PARTICLES"],
    &["PREPASS_PIPELINE", "VERTEX_UVS_A", "VERTEX_COLORS", "PREPASS_FRAGMENT", "MOTION_VECTOR_PREPASS", "GPU_PARTICLES", "MESH_PARTICLES"],
    &["PREPASS_PIPELINE", "PREPASS_FRAGMENT", "UNCLIPPED_DEPTH_ORTHO_EMULATION", "GPU_PARTICLES"],
];

#[test]
fn both_particle_shaders_compile() {
    let validator = validator();
    let mut failures = Vec::new();
    for (module, source) in [
        ("renzora_particles::shaders::particle3d", SHADER_3D),
        ("renzora_particles::shaders::particle2d", SHADER_2D),
    ] {
        for flags in CONFIGS {
            if let Err(error) = validator.check_module(module, source, flags, &[]) {
                failures.push(format!("{module} {flags:?}:\n{error}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
fn the_3d_prepass_and_shadow_pipelines_compile() {
    let validator = validator();
    let module = "renzora_particles::shaders::particle3d";
    let failures: Vec<String> = PREPASS_CONFIGS
        .iter()
        .filter_map(|flags| validator.check_module(module, SHADER_3D, flags, &[]).err().map(|e| format!("{flags:?}:\n{e}")))
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
fn the_surface_vertex_shader_compiles_in_every_pipeline() {
    let validator = validator();
    let module = "renzora_particles::shaders::particle_surface";
    let failures: Vec<String> = SURFACE_CONFIGS
        .iter()
        .filter_map(|flags| validator.check_module(module, SHADER_SURFACE, flags, &[]).err().map(|e| format!("{flags:?}:\n{e}")))
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// Motion vectors are written only in the motion-vector prepass: the call
/// that writes the previous position, broken, fails under the define and
/// still compiles in the main pass and a plain shadow pass, in both 3D
/// shaders. (`previous_world` itself is shared expansion code, compiled in
/// every pipeline like the rest of the block.)
#[test]
fn motion_vectors_compile_only_in_the_motion_vector_prepass() {
    let validator = validator();
    let line = "out.previous_world_position = vec4<f32>(previous_world(p.world, p.velocity, pv.center, v.instance_index), 1.0);";
    for (module, source, main) in [
        ("renzora_particles::shaders::particle3d", SHADER_3D, &[][..]),
        ("renzora_particles::shaders::particle_surface", SHADER_SURFACE, &SURFACE_CONFIGS[0][..]),
    ] {
        let broken = source.replacen(line, "out.previous_world_position = no_such_symbol;", 1);
        assert_ne!(broken, source, "the motion line has moved in {module}");
        assert!(validator.check_module(module, &broken, main, &[]).is_ok(), "{module}: motion compiled into the main pass");
        assert!(validator.check_module(module, &broken, &["PREPASS_PIPELINE"], &[]).is_ok(), "{module}: and a shadow pass");
        let error = validator
            .check_module(module, &broken, &["PREPASS_PIPELINE", "PREPASS_FRAGMENT", "MOTION_VECTOR_PREPASS"], &[])
            .expect_err("the previous position must compile in the motion prepass");
        assert!(error.contains("no_such_symbol"), "{module}: {error}");
    }
    // The previous position follows the playback rate, so a paused effect
    // writes no motion.
    let rate = "let back = world - velocity * (globals.delta_time * max(params.motion.x, 0.0));";
    assert!(SHADER_3D.contains(rate) && SHADER_SURFACE.contains(rate));
}

/// The prepass throws away what the main pass would not draw solid, from the
/// particle's own colour: broken, every prepass that runs a fragment fails,
/// and a depth-only shadow (no fragment at all) is untouched only because
/// Bevy never builds one for a material with a prepass fragment that may
/// discard.
#[test]
fn the_prepass_discard_runs_the_particle_colour() {
    let validator = validator();
    let module = "renzora_particles::shaders::particle3d";
    let broken = SHADER_3D.replacen("if coverage(particle_color(in)) < params.surface.w {", "if no_such_symbol {", 1);
    assert_ne!(broken, SHADER_3D, "the prepass discard has moved");
    assert!(validator.check_module(module, &broken, &[], &[]).is_ok(), "the discard is prepass only");
    for flags in PREPASS_CONFIGS {
        let error = validator.check_module(module, &broken, flags, &[]).expect_err("a broken prepass discard must fail");
        assert!(error.contains("no_such_symbol"), "{flags:?}: {error}");
    }
}

/// The check is only worth something if it can fail. Break a reachable line and
/// expect it caught.
#[test]
fn a_broken_shader_is_caught() {
    let validator = validator();
    let broken = SHADER_3D.replacen("let size = pv.size_rot_frame.xy;", "let size = no_such_symbol;", 1);
    assert_ne!(broken, SHADER_3D, "the line to break has moved");
    let error = validator
        .check_module("renzora_particles::shaders::particle3d", &broken, &[], &[])
        .expect_err("an undeclared symbol must fail");
    assert!(error.contains("no_such_symbol"), "{error}");
}

/// The shading modes are chosen by a uniform, so all three are always in the
/// compiled shader; this proves it by breaking a line in each procedural mode,
/// in both shaders, and expecting the compiler to see it. A mode left behind
/// an `@if` nobody sets would pass `both_particle_shaders_compile` untested.
#[test]
fn the_flame_and_smoke_shading_are_compiled() {
    let validator = validator();
    let lines = [
        "let lick = fbm(vec2<f32>(y * 2.2 - t * 1.9, s), 2) - 0.5;",
        "let body = 1.0 - smoothstep(0.35, 1.0, length(q));",
        "let threshold = e * life * 1.12 - 0.12;",
        "f1 = min(f1, frames - 1.0);",
    ];
    for (module, source) in [
        ("renzora_particles::shaders::particle3d", SHADER_3D),
        ("renzora_particles::shaders::particle2d", SHADER_2D),
    ] {
        for line in lines {
            let broken = source.replacen(line, "let broken = no_such_symbol;", 1);
            assert_ne!(broken, source, "the line to break has moved: {line}");
            let error = validator
                .check_module(module, &broken, &[], &[])
                .expect_err("a broken procedural line must fail");
            assert!(error.contains("no_such_symbol"), "{module}: {error}");
        }
    }
}

/// The ribbon expansion is chosen by the facing uniform, like the shading
/// modes, so it is compiled in every pipeline; this breaks it in both shaders
/// and expects the compiler to notice, so a ribbon path that drifted behind a
/// define nobody sets would fail here rather than draw nothing.
#[test]
fn the_ribbon_path_is_compiled() {
    let validator = validator();
    let line = "offset = vec2<f32>(pv.corner.x * size.x, 0.0);";
    for (module, source) in [
        ("renzora_particles::shaders::particle3d", SHADER_3D),
        ("renzora_particles::shaders::particle2d", SHADER_2D),
    ] {
        let broken = source.replacen(line, "offset = no_such_symbol;", 1);
        assert_ne!(broken, source, "the ribbon line has moved in {module}");
        for flags in CONFIGS {
            let error = validator
                .check_module(module, &broken, flags, &[])
                .expect_err("a broken ribbon line must fail");
            assert!(error.contains("no_such_symbol"), "{module} {flags:?}: {error}");
        }
    }
}

/// Soft particles exist only under the depth prepass: broken there, the shader
/// fails with the define and still compiles without it, which is what lets a
/// camera with no prepass draw them with hard edges instead of failing.
#[test]
fn soft_particles_compile_only_with_a_depth_prepass() {
    let validator = validator();
    let line = "return clamp((here - scene) / distance, 0.0, 1.0);";
    let broken = SHADER_3D.replacen(line, "return no_such_symbol;", 1);
    assert_ne!(broken, SHADER_3D, "the line to break has moved");
    let module = "renzora_particles::shaders::particle3d";
    assert!(validator.check_module(module, &broken, &[], &[]).is_ok(), "soft path compiled without a prepass");
    let error = validator
        .check_module(module, &broken, &["DEPTH_PREPASS"], &[])
        .expect_err("the soft path must compile under DEPTH_PREPASS");
    assert!(error.contains("no_such_symbol"), "{error}");
}

/// The procedural block is copied between the two shaders (WESL has no shared
/// module of ours to import from an embedded asset without registering a
/// library), so the copies must stay identical or one dimension's fire quietly
/// diverges from the other's.
#[test]
fn the_shared_block_is_identical_in_both_shaders() {
    fn block(source: &str) -> &str {
        let start = source.find("// BEGIN SHARED").expect("BEGIN SHARED marker");
        let end = source.find("// END SHARED").expect("END SHARED marker");
        &source[start..end]
    }
    assert_eq!(block(SHADER_3D), block(SHADER_2D), "particle2d.wesl's shared block has drifted from particle3d.wesl's");
}

fn between<'a>(source: &'a str, begin: &str, end: &str) -> &'a str {
    let start = source.find(begin).unwrap_or_else(|| panic!("{begin} marker"));
    let stop = source.find(end).unwrap_or_else(|| panic!("{end} marker"));
    &source[start..stop]
}

/// The GPU backend's fetch is copied between the two shaders like the
/// shading block, and its `Particle` struct is the simulation's own: a field
/// moved in one and not the other would draw every particle from the wrong
/// bytes, which no compiler can see.
#[test]
fn the_gpu_fetch_and_particle_struct_are_shared() {
    let fetch = |s| between(s, "// BEGIN GPU FETCH", "// END GPU FETCH");
    assert_eq!(fetch(SHADER_3D), fetch(SHADER_2D), "particle2d.wesl's GPU fetch has drifted from particle3d.wesl's");
    let library = renzora_particles::gpu::codegen::LIBRARY;
    let particle = |s| between(s, "// BEGIN PARTICLE STRUCT", "// END PARTICLE STRUCT");
    assert_eq!(particle(SHADER_3D), particle(library), "the shader's Particle differs from the simulation's");
    assert_eq!(particle(SHADER_2D), particle(library), "the 2D shader's Particle differs from the simulation's");
}

/// The surface shader carries copies of particle3d.wesl's fetch, expansion,
/// `sheet_uv` and `hash12`: a drift would place a material-wearing particle
/// somewhere the particle shader would not, which no compiler can see.
#[test]
fn the_surface_shader_shares_the_fetch_and_the_expansion() {
    let fetch = |s| between(s, "// BEGIN GPU FETCH", "// END GPU FETCH");
    assert_eq!(fetch(SHADER_SURFACE), fetch(SHADER_3D), "particle_surface.wesl's GPU fetch has drifted from particle3d.wesl's");
    let expansion = |s| between(s, "// BEGIN VERTEX EXPANSION", "// END VERTEX EXPANSION");
    assert_eq!(expansion(SHADER_SURFACE), expansion(SHADER_3D), "particle_surface.wesl's expansion has drifted");
    for name in ["fn sheet_uv(", "fn hash12("] {
        assert_eq!(function(SHADER_SURFACE, name), function(SHADER_3D, name), "{name} has drifted");
    }
}

/// The text of the function starting `name`, up to its closing brace.
fn function<'a>(source: &'a str, name: &str) -> &'a str {
    let start = source.find(name).unwrap_or_else(|| panic!("{name}"));
    let end = source[start..].find("\n}\n").unwrap_or_else(|| panic!("{name} end")) + start;
    &source[start..end]
}

/// The particle data a material graph reads crosses two crates: the surface
/// vertex shader writes it and the material codegen declares it. Both must
/// agree on each location's place and type, or the pipeline fails to link.
#[test]
fn the_surface_outputs_match_what_the_material_codegen_reads() {
    use renzora_shader::material::codegen;
    use renzora_shader::material::graph::{MaterialDomain, MaterialGraph};
    assert!(SHADER_SURFACE.contains("@location(10) particle: vec4<f32>,"));
    assert!(SHADER_SURFACE.contains("@location(11) particle_velocity: vec3<f32>,"));
    let mut graph = MaterialGraph::new("spark", MaterialDomain::Surface);
    let age = graph.add_node("input/particle_age", [0.0, 0.0]);
    let vel = graph.add_node("input/particle_velocity", [0.0, 0.0]);
    let out = graph.output_node().unwrap().id;
    graph.connect(age, "age", out, "roughness");
    graph.connect(vel, "velocity", out, "emissive");
    let fragment = codegen::compile(&graph).fragment_shader;
    assert!(fragment.contains("@location(10) particle_in: vec4<f32>"), "{fragment}");
    assert!(fragment.contains("@location(11) particle_velocity_in: vec3<f32>"), "{fragment}");
    // `mat_particle` is (life, seed, frame, speed), which the vertex writes as
    // `vec4(p.extra.y, p.extra.x, p.extra.z, length(p.velocity))`.
    assert!(SHADER_SURFACE.contains("out.particle = vec4<f32>(p.extra.y, p.extra.x, p.extra.z, length(p.velocity));"));
    assert!(fragment.contains("mat_particle.x"), "age is life: {fragment}");
    // The define the pipeline sets is the one the fragment tests.
    assert!(fragment.contains(renzora_particles::render::surface::PARTICLE_SURFACE_DEF));
}

/// Mesh particles are a vertex path of their own, compiled only under
/// `MESH_PARTICLES`: broken there, the quad pipelines still compile and both
/// mesh pipelines (CPU and GPU) do not. Their orientation math is shared by
/// every mesh pipeline, so breaking it is caught under both too.
#[test]
fn the_mesh_path_compiles_only_for_mesh_pipelines() {
    let validator = validator();
    let module = "renzora_particles::shaders::particle3d";
    for (line, with) in [
        ("p = place_mesh(pv, local, normal, world_from_local, center);", "p = no_such_symbol;"),
        ("return align_y(dir) * axis_angle(vec3<f32>(0.0, 1.0, 0.0), rotation);", "return no_such_symbol;"),
    ] {
        let broken = SHADER_3D.replacen(line, with, 1);
        assert_ne!(broken, SHADER_3D, "the mesh line has moved: {line}");
        if line.starts_with("p = place_mesh") {
            assert!(validator.check_module(module, &broken, &[], &[]).is_ok(), "the mesh call compiled into the quad pipeline");
            assert!(validator.check_module(module, &broken, &["GPU_PARTICLES"], &[]).is_ok());
        }
        for flags in [&["MESH_PARTICLES"][..], &["GPU_PARTICLES", "MESH_PARTICLES"][..]] {
            let error = validator.check_module(module, &broken, flags, &[]).expect_err("a broken mesh line must fail");
            assert!(error.contains("no_such_symbol"), "{flags:?}: {error}");
        }
    }
}

/// Lighting, screen-space size and the alpha mask are chosen by the uniform,
/// so they are in every 3D pipeline, quad and mesh, CPU and GPU: broken, each
/// fails everywhere. The 2D shader has its own screen-space size and mask.
#[test]
fn lighting_screen_space_size_and_the_mask_are_compiled_everywhere() {
    let validator = validator();
    let module3 = "renzora_particles::shaders::particle3d";
    let module2 = "renzora_particles::shaders::particle2d";
    let pixels = "return 2.0 * w / (view.clip_from_view[1][1] * max(view.viewport.w, 1.0));";
    let mask3 = "if blend == 4u && out.a < params.surface.w {";
    let coverage = "out.a = clamp((out.a - params.surface.w) / max(edge, 1e-4) + 0.5, 0.0, 1.0);";
    let mask2 = "if blend != 3u && out.a < params.surface.w {";
    let multiply = "if blend == 2u || blend == 5u {";
    let cases = [
        (module3, SHADER_3D, "let lit = pbr_functions::apply_pbr_lighting(pbr_input).rgb;", "let lit = no_such_symbol;"),
        (module3, SHADER_3D, pixels, "return no_such_symbol;"),
        (module3, SHADER_3D, mask3, "if no_such_symbol {"),
        (module3, SHADER_3D, coverage, "out.a = no_such_symbol;"),
        (module3, SHADER_3D, multiply, "if no_such_symbol {"),
        (module2, SHADER_2D, pixels, "return no_such_symbol;"),
        (module2, SHADER_2D, mask2, "if no_such_symbol {"),
        (module2, SHADER_2D, multiply, "if no_such_symbol {"),
    ];
    for (module, source, line, with) in cases {
        let broken = source.replacen(line, with, 1);
        assert_ne!(broken, source, "the line has moved in {module}: {line}");
        for flags in CONFIGS {
            let error = validator.check_module(module, &broken, flags, &[]).expect_err("a broken uniform-chosen line must fail");
            assert!(error.contains("no_such_symbol"), "{module} {flags:?}: {error}");
        }
    }
}

/// The fetch exists only under `GPU_PARTICLES`: broken there, the CPU path
/// still compiles and the GPU path does not, in both shaders.
#[test]
fn the_gpu_fetch_compiles_only_for_the_gpu_pipeline() {
    let validator = validator();
    let line = "out.size_rot_frame = vec4<f32>(size, p.rotation, sheet_frame(p.age, p.lifetime));";
    for (module, source) in [
        ("renzora_particles::shaders::particle3d", SHADER_3D),
        ("renzora_particles::shaders::particle2d", SHADER_2D),
    ] {
        let broken = source.replacen(line, "out.size_rot_frame = no_such_symbol;", 1);
        assert_ne!(broken, source, "the fetch line has moved in {module}");
        assert!(validator.check_module(module, &broken, &[], &[]).is_ok(), "{module}: fetch compiled without GPU_PARTICLES");
        let error = validator
            .check_module(module, &broken, &["GPU_PARTICLES"], &[])
            .expect_err("the fetch must compile under GPU_PARTICLES");
        assert!(error.contains("no_such_symbol"), "{module}: {error}");
    }
}

/// The GPU ribbon fetch (a Trail's history ring and a Connect segment) is
/// part of the GPU fetch, so it compiles in every GPU pipeline of all three
/// shaders and in none of the CPU ones: broken in either branch, the GPU
/// pipelines fail and the CPU ones do not notice.
#[test]
fn the_gpu_ribbon_fetch_compiles_only_for_the_gpu_pipeline() {
    let validator = validator();
    let lines = [
        "let ring = (head + points - ((k - 1u + skip) % points)) % points;",
        "let g = particle_state[keys + segment] >> 24u;",
        "out.size_rot_frame = vec4<f32>(p.size.x * (1.0 + (params.shape.z - 1.0) * u), 0.0, 0.0, frame);",
    ];
    let surface = &SURFACE_CONFIGS[0][..];
    let with_gpu: Vec<&str> = surface.iter().copied().chain(["GPU_PARTICLES"]).collect();
    for (module, source, cpu, gpu) in [
        ("renzora_particles::shaders::particle3d", SHADER_3D, &[][..], &["GPU_PARTICLES"][..]),
        ("renzora_particles::shaders::particle2d", SHADER_2D, &[][..], &["GPU_PARTICLES"][..]),
        ("renzora_particles::shaders::particle_surface", SHADER_SURFACE, surface, &with_gpu[..]),
    ] {
        for line in lines {
            let broken = source.replacen(line, "let broken = no_such_symbol;", 1);
            assert_ne!(broken, source, "the ribbon fetch line has moved in {module}: {line}");
            assert!(validator.check_module(module, &broken, cpu, &[]).is_ok(), "{module}: the ribbon fetch compiled into a CPU pipeline");
            let error = validator.check_module(module, &broken, gpu, &[]).expect_err("the ribbon fetch must compile under GPU_PARTICLES");
            assert!(error.contains("no_such_symbol"), "{module}: {error}");
        }
    }
}

/// The tube is chosen by the facing uniform, like the flat ribbon, so it is in
/// every 3D quad pipeline (CPU and GPU, main pass and prepass) and in the
/// surface shader; broken, each of those fails. Mesh pipelines never reach it.
#[test]
fn the_tube_path_is_compiled_in_every_quad_pipeline() {
    let validator = validator();
    let line = "let out_dir = side * cos(angle) + toward * sin(angle);";
    for (module, source, configs) in [
        ("renzora_particles::shaders::particle3d", SHADER_3D, &CONFIGS[..]),
        ("renzora_particles::shaders::particle_surface", SHADER_SURFACE, &SURFACE_CONFIGS[..]),
    ] {
        let broken = source.replacen(line, "let out_dir = no_such_symbol;", 1);
        assert_ne!(broken, source, "the tube line has moved in {module}");
        for flags in configs.iter().filter(|f| !f.contains(&"MESH_PARTICLES")) {
            let error = validator.check_module(module, &broken, flags, &[]).expect_err("a broken tube line must fail");
            assert!(error.contains("no_such_symbol"), "{module} {flags:?}: {error}");
        }
    }
}

// ── Deferred ────────────────────────────────────────────────────────────────

/// What `bevy_pbr`'s prepass pipeline sets for the deferred G-buffer pass of
/// a camera with a `DeferredPrepass`, before anything optional.
const DEFERRED: [&str; 5] = [
    "PREPASS_PIPELINE",
    "PREPASS_FRAGMENT",
    "DEFERRED_PREPASS",
    "NORMAL_PREPASS_OR_DEFERRED_PREPASS",
    "MOTION_VECTOR_PREPASS_OR_DEFERRED_PREPASS",
];

/// The G-buffer pass of `particle3d.wesl`, on top of [`DEFERRED`]: bare, with
/// the normal, motion-vector and depth prepasses a camera may carry beside
/// it, mesh particles, the GPU fetch (quads and meshes), a mask's discard,
/// and WebGL2's packing of the G-buffer. Ribbons and tubes are quad pipelines
/// chosen by the facing uniform, so every quad config compiles them, and the
/// flame and smoke shading are chosen by the shading uniform, so every config
/// compiles those (`the_gbuffer_branches_are_compiled_in_every_deferred_pass`).
const DEFERRED_EXTRAS: [&[&str]; 8] = [
    &[],
    &["NORMAL_PREPASS", "MOTION_VECTOR_PREPASS", "DEPTH_PREPASS"],
    &["MESH_PARTICLES"],
    &["MESH_PARTICLES", "MOTION_VECTOR_PREPASS"],
    &["GPU_PARTICLES"],
    &["GPU_PARTICLES", "MESH_PARTICLES", "NORMAL_PREPASS", "MOTION_VECTOR_PREPASS"],
    &["MAY_DISCARD", "NORMAL_PREPASS"],
    &["WEBGL2"],
];

/// The G-buffer pass of `particle_surface.wesl`: [`DEFERRED`] plus the
/// attributes the surface specialize always turns on, on quads and meshes,
/// CPU and GPU, with and without the camera's other prepasses.
const SURFACE_DEFERRED_EXTRAS: [&[&str]; 6] = [
    &[],
    &["MESH_PARTICLES"],
    &["GPU_PARTICLES"],
    &["GPU_PARTICLES", "MESH_PARTICLES"],
    &["NORMAL_PREPASS", "MOTION_VECTOR_PREPASS"],
    &["GPU_PARTICLES", "MESH_PARTICLES", "NORMAL_PREPASS", "MOTION_VECTOR_PREPASS", "MAY_DISCARD"],
];

const SURFACE_ATTRIBUTES: [&str; 4] = ["VERTEX_UVS_A", "VERTEX_COLORS", "VERTEX_TANGENTS", "RENZORA_PARTICLE_SURFACE"];

fn deferred_configs() -> Vec<Vec<&'static str>> {
    DEFERRED_EXTRAS.iter().map(|extra| DEFERRED.iter().chain(extra.iter()).copied().collect()).collect()
}

fn surface_deferred_configs() -> Vec<Vec<&'static str>> {
    SURFACE_DEFERRED_EXTRAS
        .iter()
        .map(|extra| DEFERRED.iter().chain(SURFACE_ATTRIBUTES.iter()).chain(extra.iter()).copied().collect())
        .collect()
}

#[test]
fn the_deferred_gbuffer_pipelines_compile() {
    let validator = validator();
    let mut failures = Vec::new();
    for flags in deferred_configs() {
        if let Err(e) = validator.check_module("renzora_particles::shaders::particle3d", SHADER_3D, &flags, &[]) {
            failures.push(format!("particle3d {flags:?}:\n{e}"));
        }
    }
    for flags in surface_deferred_configs() {
        if let Err(e) = validator.check_module("renzora_particles::shaders::particle_surface", SHADER_SURFACE, &flags, &[]) {
            failures.push(format!("particle_surface {flags:?}:\n{e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// The G-buffer write exists only in the G-buffer pass: broken, every
/// deferred config fails, while the main pass, the depth, normal and motion
/// prepasses and the shadow passes (which a forward camera runs, and which
/// must stay exactly as they were) still compile.
#[test]
fn the_gbuffer_write_compiles_only_in_the_deferred_pass() {
    let validator = validator();
    let module = "renzora_particles::shaders::particle3d";
    let broken = SHADER_3D.replacen("out.deferred = particle_gbuffer(in, front);", "out.deferred = no_such_symbol;", 1);
    assert_ne!(broken, SHADER_3D, "the G-buffer write has moved");
    for flags in CONFIGS.iter().chain(PREPASS_CONFIGS.iter()) {
        assert!(validator.check_module(module, &broken, flags, &[]).is_ok(), "{flags:?}: the G-buffer write leaked out of its pass");
    }
    for flags in deferred_configs() {
        let error = validator.check_module(module, &broken, &flags, &[]).expect_err("the G-buffer write must compile in the deferred pass");
        assert!(error.contains("no_such_symbol"), "{flags:?}: {error}");
    }
}

/// Lit, unlit, the flame's emissive and a sprite's HDR overflow are chosen
/// by the uniform inside the G-buffer write, and the procedural flame and
/// smoke reach it through the shared colour, so every one of them is in every
/// deferred pipeline: broken, each fails everywhere.
#[test]
fn the_gbuffer_branches_are_compiled_in_every_deferred_pass() {
    let validator = validator();
    let module = "renzora_particles::shaders::particle3d";
    let cases = [
        ("pbr_input.material.flags = pbr_types::STANDARD_MATERIAL_FLAGS_UNLIT_BIT;", "pbr_input.material.flags = no_such_symbol;"),
        ("pbr_input.material.emissive = vec4<f32>(rgb, 1.0);", "pbr_input.material.emissive = no_such_symbol;"),
        ("pbr_input.material.emissive = vec4<f32>(rgb - base, 1.0);", "pbr_input.material.emissive = no_such_symbol;"),
        ("pbr_input.material.perceptual_roughness = params.surface.y;", "pbr_input.material.perceptual_roughness = no_such_symbol;"),
        ("let lick = fbm(vec2<f32>(y * 2.2 - t * 1.9, s), 2) - 0.5;", "let lick = no_such_symbol;"),
        ("let body = 1.0 - smoothstep(0.35, 1.0, length(q));", "let body = no_such_symbol;"),
    ];
    for (line, with) in cases {
        let broken = SHADER_3D.replacen(line, with, 1);
        assert_ne!(broken, SHADER_3D, "the line has moved: {line}");
        for flags in deferred_configs() {
            let error = validator.check_module(module, &broken, &flags, &[]).expect_err("a broken G-buffer branch must fail");
            assert!(error.contains("no_such_symbol"), "{flags:?}: {error}");
        }
    }
}

/// The G-buffer's normal is the one the forward lighting uses, card or mesh
/// with a back face flipped, and its lighting pass is StandardMaterial's.
#[test]
fn the_gbuffer_takes_the_forward_lit_normal() {
    let gbuffer = function(SHADER_3D, "fn particle_gbuffer(");
    assert!(gbuffer.contains("let n = seen_normal(in, front);"), "{gbuffer}");
    let seen = function(SHADER_3D, "fn seen_normal(");
    let light = function(SHADER_3D, "fn light(");
    for line in ["var n = in.world_normal / max(length(in.world_normal), 1e-6);", "if params.shape.x > 0.5 && !front {"] {
        assert!(seen.contains(line) && light.contains(line), "the G-buffer normal has drifted from the lit one: {line}");
    }
}

/// The particle data a graph reads in the G-buffer pass: written only there,
/// at the locations the main pass uses, so the depth prepass and shadows
/// (whose fragments never read them) keep the interface they had.
#[test]
fn the_surface_writes_particle_data_only_in_the_gbuffer_pass() {
    let validator = validator();
    let module = "renzora_particles::shaders::particle_surface";
    let line = "out.particle = vec4<f32>(p.extra.y, p.extra.x, p.extra.z, length(p.velocity));";
    // The second copy is the prepass entry point's; the first is the main pass's.
    let at = SHADER_SURFACE.rfind(line).expect("the prepass particle line");
    assert_ne!(Some(at), SHADER_SURFACE.find(line), "the main and prepass entry points both write it");
    let broken = format!("{}out.particle = no_such_symbol;{}", &SHADER_SURFACE[..at], &SHADER_SURFACE[at + line.len()..]);
    for flags in SURFACE_CONFIGS {
        assert!(validator.check_module(module, &broken, flags, &[]).is_ok(), "{flags:?}: particle data leaked out of the G-buffer pass");
    }
    for flags in surface_deferred_configs() {
        let error = validator.check_module(module, &broken, &flags, &[]).expect_err("particle data must be written in the G-buffer pass");
        assert!(error.contains("no_such_symbol"), "{flags:?}: {error}");
    }
}

/// `(define, location line)` for every `@location` field of `name` in
/// `source`, in order, with the `@if` that guards it ("" when none).
fn locations(source: &str, name: &str) -> Vec<(String, String)> {
    let start = source.find(&format!("struct {name} {{")).unwrap_or_else(|| panic!("struct {name}"));
    let body = &source[start..start + source[start..].find("\n}").unwrap_or_else(|| panic!("{name} end"))];
    let mut out = Vec::new();
    let mut guard = String::new();
    for line in body.lines().map(str::trim) {
        if line.starts_with("@if(") {
            guard = line.to_string();
        } else if line.starts_with("@location(") {
            out.push((std::mem::take(&mut guard), line.trim_end_matches(',').to_string()));
        } else if line.starts_with("@builtin(") {
            guard.clear();
        }
    }
    out
}

/// The location contract of the G-buffer pass, across three crates:
///
/// - the surface's prepass output carries `bevy_pbr::prepass::io`'s
///   VertexOutput location for location (which StandardMaterial's G-buffer
///   fragment and a graph's deferred branch read as that struct), read from
///   the very library Bevy registers, so a Bevy upgrade that moves one fails
///   here rather than as a pipeline link error;
/// - past them, under `DEFERRED_PREPASS`, the particle's locations 10 and 11,
///   the same as the main pass writes;
/// - the material codegen's deferred header reads the prepass io there, and a
///   graph with Particle nodes declares 10 and 11 in its deferred branch too
///   and compiles in every particle G-buffer configuration.
#[test]
fn the_gbuffer_locations_match_bevy_and_the_material_codegen() {
    use renzora_shader::material::codegen;
    use renzora_shader::material::graph::{MaterialDomain, MaterialGraph};
    let (validator, io) = validator_and_library("bevy_pbr::prepass::io");
    let bevy = locations(&io, "VertexOutput");
    assert!(bevy.len() >= 8, "Bevy's prepass VertexOutput did not parse: {bevy:?}");
    let ours = locations(SHADER_SURFACE, "PrepassOut");
    let (particle, prepass): (Vec<_>, Vec<_>) = ours.into_iter().partition(|(_, l)| l.starts_with("@location(10)") || l.starts_with("@location(11)"));
    assert_eq!(prepass, bevy, "particle_surface.wesl's PrepassOut has drifted from bevy_pbr::prepass::io's VertexOutput");
    assert_eq!(
        particle,
        vec![
            ("@if(DEFERRED_PREPASS)".to_string(), "@location(10) particle: vec4<f32>".to_string()),
            ("@if(DEFERRED_PREPASS)".to_string(), "@location(11) particle_velocity: vec3<f32>".to_string()),
        ]
    );
    // The main pass writes the same two.
    let main = locations(SHADER_SURFACE, "SurfaceOut");
    for (_, l) in &particle {
        assert!(main.iter().any(|(_, m)| m == l), "the main pass does not write {l}");
    }

    let mut graph = MaterialGraph::new("ember", MaterialDomain::Surface);
    let age = graph.add_node("input/particle_age", [0.0, 0.0]);
    let vel = graph.add_node("input/particle_velocity", [0.0, 0.0]);
    let out = graph.output_node().unwrap().id;
    graph.connect(age, "age", out, "roughness");
    graph.connect(vel, "velocity", out, "emissive");
    let fragment = codegen::compile(&graph).fragment_shader;
    assert!(codegen::supports_deferred(&fragment), "a Particle graph must keep its G-buffer branch: {fragment}");
    assert!(fragment.contains("@if(PREPASS_PIPELINE) import bevy_pbr::prepass::io::{VertexOutput, FragmentOutput};"), "{fragment}");
    assert!(fragment.contains("@location(10) particle_in: vec4<f32>") && fragment.contains("@location(11) particle_velocity_in: vec3<f32>"));
    let failures: Vec<String> = surface_deferred_configs()
        .iter()
        .filter_map(|flags| validator.check_module("renzora_particles::test::ember", &fragment, flags, &[]).err().map(|e| format!("{flags:?}:\n{e}")))
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

