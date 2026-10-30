//! Compile the terrain surface shader for the virtual geometry material pass,
//! without a GPU.
//!
//! A terrain chunk drawn as virtual geometry is shaded by the terrain's own
//! surface shader compiled with `MESHLET_MESH_MATERIAL_PASS` (see
//! `render/materials.rs`). A WESL error there shows only as a pipeline that
//! never becomes ready: the chunk's pixels are covered but never shaded, and
//! the terrain draws as one flat dark colour with nothing in the Console. This
//! compiles it the way the material passes do, against Bevy's own shader
//! libraries and this crate's resolve module.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::shader::Shader;
use renzora_shader::material::validate::{library_count, ShaderValidator};

const REQUIRED_LIBRARIES: [&str; 4] = [
    "bevy_pbr::render::pbr_functions",
    "bevy_pbr::render::pbr_fragment",
    "bevy_pbr::deferred::functions",
    "bevy_pbr::prepass::io",
];

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: PathBuf) -> String {
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// This crate's shader modules, under the paths the renderer registers them at.
fn vg_modules() -> Vec<(String, String)> {
    let shaders = crate_dir().join("src/shaders");
    vec![
        ("renzora_virtual_geometry::shaders::types".into(), read(shaders.join("types.wesl"))),
        ("renzora_virtual_geometry::shaders::bindings".into(), read(shaders.join("bindings.wesl"))),
        ("bevy_pbr::meshlet::visibility_buffer_resolve".into(), read(shaders.join("resolve.wesl"))),
    ]
}

fn validator() -> ShaderValidator {
    let mut app = renzora_test_harness::headless_app();
    let mut last = 0;
    let mut stable = 0;
    for _ in 0..500 {
        app.update();
        let n = library_count(app.world().resource::<Assets<Shader>>());
        if n != last {
            last = n;
            stable = 0;
            continue;
        }
        stable += 1;
        if n == 0 || stable < 3 {
            continue;
        }
        let candidate = ShaderValidator::new(app.world().resource::<Assets<Shader>>());
        if REQUIRED_LIBRARIES.iter().all(|m| candidate.has_module(m)) {
            return candidate.with_extra_modules(vg_modules());
        }
        stable = 0;
    }
    panic!("Bevy's shader libraries never all arrived");
}

#[test]
fn the_terrain_surface_compiles_for_every_virtual_geometry_pass() {
    let validator = validator();
    let source = read(crate_dir().join("../renzora_terrain/src/shaders/terrain_surface.wesl"));
    let common = [
        "MESHLET_MESH_MATERIAL_PASS",
        "VERTEX_POSITIONS",
        "VERTEX_NORMALS",
        "VERTEX_UVS",
        "VERTEX_UVS_A",
        "VERTEX_TANGENTS",
    ];
    // `MATERIAL_BIND_GROUP` the validator defines itself.
    let numeric = [("VG_GROUP", 2), ("VG_BASE", 0)];
    let passes: [&[&str]; 3] = [
        // The deferred G-buffer pass.
        &["PREPASS_PIPELINE", "PREPASS_FRAGMENT", "DEFERRED_PREPASS", "NORMAL_PREPASS_OR_DEFERRED_PREPASS"],
        // ... with motion vectors.
        &[
            "PREPASS_PIPELINE",
            "PREPASS_FRAGMENT",
            "DEFERRED_PREPASS",
            "NORMAL_PREPASS_OR_DEFERRED_PREPASS",
            "MOTION_VECTOR_PREPASS",
        ],
        // Forward.
        &[],
    ];
    let mut failures = Vec::new();
    for pass in passes {
        let flags: Vec<&str> = common.iter().chain(pass.iter()).copied().collect();
        if let Err(error) = validator.check_module("renzora_terrain::shaders::terrain_surface", &source, &flags, &numeric) {
            failures.push(format!("{pass:?}:\n{error}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// The blockout grid every new terrain wears, compiled for the forward pass it
/// draws in, with and without fog and with tonemapping in the shader (a view
/// without HDR). Fog is the define that matters: its call is compiled out
/// without it, so a mistake there only shows on a camera that has fog.
#[test]
fn the_terrain_blockout_grid_compiles_with_and_without_fog() {
    let validator = validator();
    let source = read(crate_dir().join("../renzora_terrain/src/shaders/terrain_checkerboard.wesl"));
    let mesh = ["VERTEX_POSITIONS", "VERTEX_NORMALS", "VERTEX_UVS"];
    let mut failures = Vec::new();
    for pass in [&[][..], &["DISTANCE_FOG"][..], &["DISTANCE_FOG", "TONEMAP_IN_SHADER"][..]] {
        let flags: Vec<&str> = mesh.iter().chain(pass.iter()).copied().collect();
        if let Err(error) = validator.check_module("renzora_terrain::shaders::terrain_checkerboard", &source, &flags, &[]) {
            failures.push(format!("{pass:?}:\n{error}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// A terrain's landscape material (Alpine Landscape, the engine's default:
/// sixteen textures, three Layer Blends, slope and height rules) compiled the
/// way every pass a chunk goes through builds it: the virtual geometry
/// material passes, the ordinary forward and deferred passes, and with the
/// level-of-detail dither. Its weight-map read is `textureLoad`s and flat
/// arithmetic, so it has to build in all of them.
#[test]
fn the_landscape_material_compiles_for_every_pass() {
    let validator = validator();
    let graph = renzora_shader::material::landscape_preset::alpine_landscape_graph();
    let result = renzora_shader::material::codegen::compile(&graph);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let source = result.fragment_shader;
    let mesh = ["VERTEX_POSITIONS", "VERTEX_NORMALS", "VERTEX_UVS", "VERTEX_UVS_A"];
    let deferred: &[&str] =
        &["PREPASS_PIPELINE", "PREPASS_FRAGMENT", "DEFERRED_PREPASS", "NORMAL_PREPASS_OR_DEFERRED_PREPASS"];
    let numeric = [("VG_GROUP", 2), ("VG_BASE", 0)];
    let mut failures = Vec::new();
    for (vg, pass) in [
        (true, deferred),
        (true, &[][..]),
        (false, &[][..]),
        (false, deferred),
        (false, &["VISIBILITY_RANGE_DITHER"][..]),
        (false, &["VERTEX_TANGENTS"][..]),
    ] {
        let mut flags: Vec<&str> = mesh.to_vec();
        if vg {
            flags.push("MESHLET_MESH_MATERIAL_PASS");
            flags.push("VERTEX_TANGENTS");
        }
        flags.extend_from_slice(pass);
        let numeric: &[(&str, u32)] = if vg { &numeric } else { &[] };
        if let Err(error) = validator.check_module("renzora_shader::material::alpine_landscape", &source, &flags, numeric) {
            failures.push(format!("vg {vg} {pass:?}:\n{error}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// The same shader in the ordinary mesh passes, which every chunk not drawn
/// as virtual geometry (and every stand-in and level of detail) goes through.
#[test]
fn the_terrain_surface_compiles_for_every_ordinary_pass() {
    let validator = validator();
    let source = read(crate_dir().join("../renzora_terrain/src/shaders/terrain_surface.wesl"));
    let common = ["VERTEX_POSITIONS", "VERTEX_NORMALS", "VERTEX_UVS", "VERTEX_UVS_A"];
    let passes: [&[&str]; 4] = [
        &[],
        &["PREPASS_PIPELINE", "PREPASS_FRAGMENT", "DEFERRED_PREPASS", "NORMAL_PREPASS_OR_DEFERRED_PREPASS"],
        &[
            "PREPASS_PIPELINE",
            "PREPASS_FRAGMENT",
            "DEFERRED_PREPASS",
            "NORMAL_PREPASS_OR_DEFERRED_PREPASS",
            "MOTION_VECTOR_PREPASS",
        ],
        &["VISIBILITY_RANGE_DITHER"],
    ];
    let mut failures = Vec::new();
    for pass in passes {
        let flags: Vec<&str> = common.iter().chain(pass.iter()).copied().collect();
        if let Err(error) = validator.check_module("renzora_terrain::shaders::terrain_surface", &source, &flags, &[]) {
            failures.push(format!("{pass:?}:\n{error}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
