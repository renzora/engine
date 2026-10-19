//! Compile the forked `bevy_solari`'s shaders without a GPU.
//!
//! A WESL error in them only shows up when a ray-tracing GPU builds the
//! pipeline, and then as a dark viewport and a log line. This compiles every
//! entry-point shader the way its pipeline does, against Bevy's own shader
//! libraries, so a broken patch fails here instead.
//!
//! Solari's plugin cannot be added to a headless app (it needs a device that
//! can ray trace), so its `.wesl` files are read from disk and registered under
//! the module paths `load_shader_library!` would give them.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::shader::Shader;
use renzora_shader::material::validate::{library_count, ShaderValidator};

/// Libraries Solari imports that arrive late, the way the material validator's
/// own test waits for them: `mesh_bindings` streams in after the embedded ones.
const REQUIRED_LIBRARIES: [&str; 5] = [
    "bevy_pbr::render::pbr_functions",
    "bevy_pbr::render::pbr_lighting",
    "bevy_pbr::deferred::types",
    "bevy_pbr::prepass::bindings",
    "bevy_core_pipeline::tonemapping",
];

/// `WORLD_CACHE_SIZE` as `prepare.rs` defines it.
const WORLD_CACHE_SIZE: u32 = 1 << 20;

fn solari_src() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bevy_solari/src")
}

/// Every `.wesl` under `bevy_solari/src`, as (module path, source).
fn solari_modules() -> Vec<(String, String)> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, root, out);
            } else if path.extension().is_some_and(|e| e == "wesl") {
                let relative = path.strip_prefix(root).unwrap().with_extension("");
                let module = std::iter::once("bevy_solari".to_string())
                    .chain(relative.components().map(|c| c.as_os_str().to_string_lossy().into_owned()))
                    .collect::<Vec<_>>()
                    .join("::");
                out.push((module, std::fs::read_to_string(&path).unwrap()));
            }
        }
    }
    let root = solari_src();
    let mut out = Vec::new();
    walk(&root, &root, &mut out);
    out
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
            return candidate.with_extra_modules(solari_modules());
        }
        stable = 0;
    }
    panic!("Bevy's shader libraries never all arrived");
}

/// One pipeline: the file, and the flags and numeric defines `node.rs` or
/// `denoise.rs` queues it with. Flags the device decides
/// (`MOTION_VECTORS_READ_WRITE`) are tried both ways.
struct Pipeline {
    file: &'static str,
    flags: &'static [&'static str],
    numeric: &'static [(&'static str, u32)],
}

fn pipelines() -> Vec<Pipeline> {
    vec![
        Pipeline { file: "realtime/restir", flags: &["RESTIR"], numeric: &[] },
        Pipeline { file: "realtime/restir", flags: &["RESTIR", "SPATIAL_MERGE"], numeric: &[] },
        Pipeline { file: "realtime/no_restir", flags: &[], numeric: &[] },
        Pipeline { file: "realtime/presample_light_tiles", flags: &[], numeric: &[] },
        Pipeline { file: "realtime/world_cache_compact", flags: &["WORLD_CACHE_NON_ATOMIC_LIFE_BUFFER"], numeric: &[] },
        Pipeline { file: "realtime/world_cache_update", flags: &[], numeric: &[] },
        Pipeline { file: "realtime/world_cache_update", flags: &["WORLD_CACHE_QUERY_ATOMIC_MAX_LIFETIME"], numeric: &[] },
        Pipeline { file: "realtime/denoise", flags: &["DENOISE_TEMPORAL"], numeric: &[] },
        Pipeline { file: "realtime/denoise", flags: &["DENOISE_ATROUS", "DENOISE_FEEDBACK"], numeric: &[("DENOISE_STEP", 1)] },
        Pipeline { file: "realtime/denoise", flags: &["DENOISE_ATROUS"], numeric: &[("DENOISE_STEP", 16)] },
        Pipeline { file: "realtime/denoise", flags: &["DENOISE_COMPOSITE"], numeric: &[] },
    ]
}

/// The check above is only worth something if it can fail: WESL drops
/// declarations nothing uses, so a mistake has to land in a reachable function
/// to be seen. Break one in the denoiser's temporal entry point and expect it
/// caught.
#[test]
fn a_broken_shader_is_caught() {
    let validator = validator();
    let source = std::fs::read_to_string(solari_src().join("realtime/denoise.wesl")).unwrap();
    let broken = source.replacen(
        "let depth = textureLoad(depth_buffer, pixel_id, 0);",
        "let depth = no_such_symbol;",
        1,
    );
    assert_ne!(broken, source, "the line to break has moved");
    let result = validator.check_module(
        "bevy_solari::realtime::denoise",
        &broken,
        &["DENOISE_TEMPORAL"],
        &[("WORLD_CACHE_SIZE", WORLD_CACHE_SIZE)],
    );
    let error = result.expect_err("an undeclared symbol must fail");
    assert!(error.contains("no_such_symbol"), "{error}");
}

#[test]
fn every_solari_pipeline_compiles() {
    let validator = validator();
    let mut failures = Vec::new();
    for pipeline in pipelines() {
        let path = solari_src().join(format!("{}.wesl", pipeline.file));
        let source = std::fs::read_to_string(&path).unwrap();
        let module = format!("bevy_solari::{}", pipeline.file.replace('/', "::"));
        for motion_vectors_read_write in [false, true] {
            let mut flags = pipeline.flags.to_vec();
            if motion_vectors_read_write {
                flags.push("MOTION_VECTORS_READ_WRITE");
            }
            let mut numeric = pipeline.numeric.to_vec();
            numeric.push(("WORLD_CACHE_SIZE", WORLD_CACHE_SIZE));
            if let Err(error) = validator.check_module(&module, &source, &flags, &numeric) {
                failures.push(format!("{} {:?}:\n{error}", pipeline.file, flags));
            }
        }
    }
    assert!(failures.is_empty(), "{} configuration(s) failed:\n\n{}", failures.len(), failures.join("\n\n"));
}
