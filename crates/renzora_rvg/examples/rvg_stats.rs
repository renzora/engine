//! Where an `.rvg` file's bytes go, section by section, and what the renderer
//! uploads from it.
//!
//! `cargo run --profile dist -p renzora_rvg --example rvg_stats -- model.rvg`
//!
//! For deciding what to compress or stream: the file's sections and the GPU
//! buffers they become are not the same size, since the renderer packs some
//! attributes and drops others.

fn mb(bytes: usize) -> String {
    format!("{:8.1} MB", bytes as f64 / 1e6)
}

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: rvg_stats <model.rvg>");
        std::process::exit(2);
    };
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let started = std::time::Instant::now();
    let file = renzora_rvg::RvgFile::from_bytes(&bytes).unwrap_or_else(|e| panic!("{path}: {e:?}"));
    println!("read in {:.2?}", started.elapsed());

    let mut vertices = 0usize;
    let mut sections = [0usize; 12];
    let names = [
        "positions",
        "normals",
        "uv0",
        "tangents",
        "uv1",
        "colors",
        "joints+weights",
        "clusters",
        "cluster vertex lists",
        "cluster triangles",
        "groups+nodes",
        "proxy (rebuilt on load)",
    ];
    let mut clusters = 0usize;
    let mut original_triangles = 0usize;
    let mut dag_triangles = 0usize;
    for m in &file.meshes {
        vertices += m.positions.len();
        sections[0] += m.positions.len() * 12;
        sections[1] += m.normals.len() * 12;
        sections[2] += m.uv0.len() * 8;
        sections[3] += m.tangents.len() * 16;
        sections[4] += m.uv1.len() * 8;
        sections[5] += m.colors.len() * 16;
        sections[6] += m.joints.len() * 8 + m.weights.len() * 16;
        sections[7] += m.clusters.len() * 72;
        sections[8] += m.cluster_vertices.len() * 4;
        sections[9] += m.cluster_triangles.len();
        sections[10] += m.groups.len() * 24 + m.nodes.len() * 32;
        sections[11] += m.proxy_indices().len() * 4;
        clusters += m.clusters.len();
        original_triangles += m.original_triangle_count();
        dag_triangles += m.clusters.iter().map(|c| c.triangle_count as usize).sum::<usize>();
    }
    println!(
        "{path}: {} on disk, {} meshes, {vertices} vertices, {clusters} clusters, \
         {original_triangles} triangles at full detail, {dag_triangles} across the whole DAG",
        mb(bytes.len()).trim(),
        file.meshes.len()
    );
    println!("\nfile sections:");
    for (name, size) in names.iter().zip(sections) {
        println!("  {name:22} {}", mb(size));
    }

    // What `render/scene.rs` uploads: positions as three floats, normal,
    // tangent and UV packed into 16 bytes, an 80-byte cluster record, and the
    // vertex lists plus the triangle bytes in one index buffer.
    let gpu = [
        ("positions", vertices * 12),
        ("packed attributes", vertices * 16),
        ("clusters", clusters * 80),
        ("indices", sections[8] + sections[9]),
    ];
    println!("\nGPU buffers:");
    for (name, size) in gpu {
        println!("  {name:22} {}", mb(size));
    }
    println!("  {:22} {}", "total", mb(gpu.iter().map(|(_, s)| s).sum()));

    // Streaming pages are self-contained, so a vertex used by several groups
    // is stored once per group. This is what everything resident would take,
    // and what the roots alone (always resident) take.
    let mut page_bytes = 0usize;
    let mut root_bytes = 0usize;
    for m in &file.meshes {
        let graph = m.group_graph();
        for (g, range) in graph.clusters.iter().enumerate() {
            let mut unique = std::collections::HashSet::new();
            let mut lists = 0usize;
            let mut triangles = 0usize;
            for c in &m.clusters[range.start as usize..range.end as usize] {
                let vertices = &m.cluster_vertices[c.vertex_offset as usize..][..c.vertex_count as usize];
                unique.extend(vertices.iter().copied());
                lists += vertices.len();
                triangles += c.triangle_count as usize * 3;
            }
            let bytes = unique.len() * 28 + lists * 4 + triangles.div_ceil(4) * 4;
            page_bytes += bytes;
            if graph.parents[g].is_empty() {
                root_bytes += bytes;
            }
        }
    }
    println!("\nstreaming pages:");
    println!("  {:22} {}", "everything resident", mb(page_bytes));
    println!("  {:22} {}", "roots (always resident)", mb(root_bytes));

    scene(&file, original_triangles);
}

/// The model's scene, for an `.rvg` model: the GLB it carries, and how much
/// of its geometry is the proxy against what the clusters hold.
fn scene(file: &renzora_rvg::RvgFile, full_triangles: usize) {
    let Some(glb) = file.scene.as_deref() else {
        println!("\nno scene: virtual geometry beside a .glb, not a model");
        return;
    };
    let json = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    println!("\nscene (glTF):");
    println!("  {:22} {}", "whole GLB", mb(glb.len()));
    println!("  {:22} {}", "JSON", mb(json));
    println!("  {:22} {}", "binary (proxies etc.)", mb(glb.len() - json - 20));
    #[cfg(feature = "bake")]
    {
        let Ok(doc) = gltf::Gltf::from_slice(glb) else {
            println!("  not a readable GLB");
            return;
        };
        let primitives: Vec<_> = doc.meshes().flat_map(|m| m.primitives().collect::<Vec<_>>()).collect();
        let proxy: usize = primitives.iter().filter_map(|p| p.indices()).map(|a| a.count() / 3).sum();
        println!(
            "  {} nodes, {} meshes, {} primitives, {} materials, {} skins, {} animations, {} images",
            doc.nodes().count(),
            doc.meshes().count(),
            primitives.len(),
            doc.materials().count(),
            doc.skins().count(),
            doc.animations().count(),
            doc.images().count(),
        );
        println!(
            "  proxy triangles: {proxy} ({:.0}% of the {full_triangles} at full detail)",
            proxy as f64 * 100.0 / full_triangles.max(1) as f64
        );
        // Where the binary chunk goes. Images are normally external files
        // (`textures/*.rmip`), so bytes here mean a copy is embedded too.
        let embedded: usize = doc.images().filter_map(|i| match i.source() {
            gltf::image::Source::View { view, .. } => Some(view.length()),
            _ => None,
        }).sum();
        // By accessor, not by view: primitives often share one view, and
        // counting it per primitive would count it many times over.
        let bytes = |a: &gltf::Accessor| a.count() * a.size();
        let mut by_attribute: std::collections::BTreeMap<String, usize> = Default::default();
        for p in &primitives {
            for (semantic, accessor) in p.attributes() {
                *by_attribute.entry(format!("{semantic:?}")).or_default() += bytes(&accessor);
            }
            if let Some(i) = p.indices() {
                *by_attribute.entry("indices".into()).or_default() += bytes(&i);
            }
            for target in p.morph_targets() {
                for a in target.positions().into_iter().chain(target.normals()).chain(target.tangents()) {
                    *by_attribute.entry("morph targets".into()).or_default() += bytes(&a);
                }
            }
        }
        let views: usize = doc.views().map(|v| v.length()).sum();
        println!("  {:22} {} in {} views", "all views", mb(views), doc.views().count());
        println!("  {:22} {}", "embedded images", mb(embedded));
        for (name, size) in by_attribute {
            println!("  {name:22} {}", mb(size));
        }
    }
}
