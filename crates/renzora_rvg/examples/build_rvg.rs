//! Build the `.rvg` for a GLB from the command line and report what came out.
//!
//! `cargo run --profile dist -p renzora_rvg --features bake --example build_rvg -- [--model] model.glb|model.rvg [out.rvg]`
//!
//! Without `--model` this builds the virtual geometry alone, as kept beside a
//! `.glb`; with it, a model `.rvg` that replaces the GLB, scene included.
//!
//! The editor builds these itself; this exists to measure the builder on a
//! real model without starting one.

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let model = args.first().is_some_and(|a| a == "--model");
    if model {
        args.remove(0);
    }
    let mut args = args.into_iter();
    let Some(input) = args.next() else {
        eprintln!("usage: build_rvg [--model] <model.glb> [out.rvg]");
        std::process::exit(2);
    };
    let mut glb = std::fs::read(&input).unwrap_or_else(|e| panic!("{input}: {e}"));
    // An `.rvg` model is rebuilt from the full-detail GLB it holds, which is
    // how a model imported by an older builder is measured against this one.
    if input.to_ascii_lowercase().ends_with(".rvg") {
        let file = renzora_rvg::RvgFile::from_bytes(&glb).unwrap_or_else(|e| panic!("{input}: {e:?}"));
        glb = renzora_rvg::bake::full_detail_glb(&file).unwrap_or_else(|e| panic!("{input}: {e}"));
    }
    let started = std::time::Instant::now();
    let (file, skipped) = if model {
        (renzora_rvg::bake::build_model(&glb).unwrap_or_else(|e| panic!("{e}")), Vec::new())
    } else {
        let built = renzora_rvg::bake::build_from_glb(&glb).unwrap_or_else(|e| panic!("{e}"));
        (built.file, built.skipped)
    };
    let elapsed = started.elapsed();
    let bytes = file.to_bytes();

    let triangles: usize = file.meshes.iter().map(|m| m.original_triangle_count()).sum();
    let clusters: usize = file.meshes.iter().map(|m| m.clusters.len()).sum();
    let proxy: usize = file.meshes.iter().map(|m| m.proxy_indices().len() / 3).sum();
    let depth = file
        .meshes
        .iter()
        .flat_map(|m| m.groups.iter().map(|g| g.depth))
        .max()
        .unwrap_or(0);
    println!(
        "{} primitives, {} skipped, {triangles} triangles, {clusters} clusters, \
         DAG depth {depth}, proxy {proxy} triangles, {:.1} MB, built in {:.2?}",
        file.meshes.len(),
        skipped.len(),
        bytes.len() as f64 / 1e6,
        elapsed,
    );
    if let Some(scene) = &file.scene {
        println!(
            "scene {:.1} MB (from a {:.1} MB GLB) before compression",
            scene.len() as f64 / 1e6,
            glb.len() as f64 / 1e6
        );
    }
    for s in &skipped {
        println!("  skipped mesh {} primitive {}: {}", s.mesh_index, s.primitive_index, s.reason);
    }
    if let Some(out) = args.next() {
        std::fs::write(&out, &bytes).unwrap_or_else(|e| panic!("{out}: {e}"));
    }
}
