//! Compile the overlay shader without a GPU.
//!
//! A WESL mistake only surfaces when the pipeline is built, as a surface that
//! never shows its reflection and one log line. This compiles it the way its
//! pipeline does, against Bevy's own shader libraries, under every define that
//! changes the code, so a broken shader fails here instead. Same approach as
//! `renzora_particles`' shader test.

use bevy::prelude::*;
use bevy::shader::Shader;
use renzora_shader::material::validate::{library_count, ShaderValidator};

const OVERLAY: &str = include_str!("../src/shaders/reflection_overlay.wesl");
const MODULE: &str = "renzora_planar_reflection::shaders::reflection_overlay";

/// The libraries the overlay imports, and the ones they import in turn. They
/// stream in over several frames, so each must be waited for by name:
/// `mesh_bindings` was missing from this list, and `mesh_functions` imports it,
/// so the test passed or failed on whether it happened to arrive in time.
const REQUIRED_LIBRARIES: [&str; 4] = [
    "bevy_pbr::render::mesh_functions",
    "bevy_pbr::render::mesh_bindings",
    "bevy_pbr::render::mesh_view_bindings",
    "bevy_core_pipeline::tonemapping",
];

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
            return candidate;
        }
        stable = 0;
    }
    let v = ShaderValidator::new(app.world().resource::<Assets<Shader>>());
    let missing: Vec<&str> = REQUIRED_LIBRARIES
        .iter()
        .copied()
        .filter(|m| !v.has_module(m))
        .collect();
    panic!("Bevy's shader libraries never all arrived; missing {missing:?}");
}

#[test]
fn overlay_shader_compiles() {
    let validator = validator();
    let mut failures = Vec::new();
    // Tonemapping in the shader (an LDR viewing camera) or not (HDR).
    for flags in [&[][..], &["TONEMAP_IN_SHADER"][..]] {
        if let Err(error) = validator.check_module(MODULE, OVERLAY, flags, &[]) {
            failures.push(format!("{flags:?}:\n{error}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// The check is only worth something if it can fail.
#[test]
fn a_broken_overlay_shader_is_caught() {
    let validator = validator();
    let broken = OVERLAY.replacen("let strength = params.shading.x;", "let strength = no_such_symbol;", 1);
    assert_ne!(broken, OVERLAY, "the line to break has moved");
    let error = validator
        .check_module(MODULE, &broken, &[], &[])
        .expect_err("an undeclared symbol must fail");
    assert!(error.contains("no_such_symbol"), "{error}");
}
