//! Import-pipeline probe: run the real conversion on a model and dump every
//! piece of data it produces, then cross-check the pieces against each other.
//!
//! This mirrors the sequence `renzora_import_ui::overlay::import_worker` runs
//! (convert → optimize → write textures → extract animations → compact →
//! write), because the interesting failures are the ones that only appear when
//! the stages are combined — a material URI that names a texture no stage ever
//! wrote, a GLB image pointing at a file that isn't there, a `.rmip` orphaned
//! by compaction.
//!
//! Usage:
//!   cargo run --profile dist -p renzora_import --example import_probe -- <model> [out_dir]

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Instant;

use renzora_import::{
    compact_glb, convert_to_glb_with_progress, optimize_glb, ExtractedPbrMaterial,
    ImportSettings, MeshOptSettings, TextureSource,
};

/// Everything the probe learned, accumulated so the summary can be printed
/// after the full report is written.
#[derive(Default)]
struct Findings {
    /// Problems worth a human looking at, most important first.
    issues: Vec<String>,
    /// Things that are unusual but not obviously wrong.
    notes: Vec<String>,
}

impl Findings {
    fn issue(&mut self, s: impl Into<String>) {
        self.issues.push(s.into());
    }
    fn note(&mut self, s: impl Into<String>) {
        self.notes.push(s.into());
    }
}

/// Minimal logger so the converters' own `log::warn!` diagnostics surface —
/// several of them are the only visibility into how far a partial format
/// reader got before giving up.
struct StderrLog;
impl log::Log for StderrLog {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }
    fn log(&self, record: &log::Record) {
        eprintln!("[{}] {}", record.level(), record.args());
    }
    fn flush(&self) {}
}
static LOGGER: StderrLog = StderrLog;

fn main() {
    // `RENZORA_LOG=1` turns on the converters' internal diagnostics.
    if std::env::var("RENZORA_LOG").is_ok() {
        let _ = log::set_logger(&LOGGER).map(|()| log::set_max_level(log::LevelFilter::Trace));
    }
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: import_probe <model-file> [out-dir]");
        std::process::exit(2);
    }
    let source = PathBuf::from(&args[1]);
    let out_dir = args
        .get(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("import_probe_out"));

    let mut report = String::new();
    let mut f = Findings::default();

    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).expect("create out dir");

    // ── Source ──────────────────────────────────────────────────────────
    let src_len = std::fs::metadata(&source).map(|m| m.len()).unwrap_or(0);
    let _ = writeln!(report, "# Import probe\n");
    let _ = writeln!(report, "source     {}", source.display());
    let _ = writeln!(report, "size       {}", human(src_len));
    let _ = writeln!(
        report,
        "format     {:?}",
        renzora_import::detect_format(&source)
    );
    let _ = writeln!(report, "out_dir    {}\n", out_dir.display());

    let mut settings = ImportSettings::default();
    // `RENZORA_STRUCTURE=flat|combined` exercises the hierarchy setting from
    // the probe without an editor.
    match std::env::var("RENZORA_STRUCTURE").as_deref() {
        Ok("flat") => settings.structure = renzora_import::SceneStructure::FlatPerMesh,
        Ok("combined") => settings.structure = renzora_import::SceneStructure::Combined,
        _ => {}
    }
    // `RENZORA_TEXTURE_SET=<stem>` binds a sibling texture set to a
    // geometry-only model, which is otherwise only reachable from the editor.
    if let Ok(set) = std::env::var("RENZORA_TEXTURE_SET") {
        settings.texture_set = Some(set);
    }
    for set in renzora_import::sibling_textures::discover(&source) {
        let _ = writeln!(
            report,
            "sibling texture set '{}' — {}",
            set.stem,
            set.role_summary()
        );
    }
    let _ = writeln!(report, "## Settings\n\n{:#?}\n", settings);

    // ── Stage 1: convert ────────────────────────────────────────────────
    let tex_count = std::sync::atomic::AtomicUsize::new(0);
    let t0 = Instant::now();
    let progress = |done: usize, total: usize, name: &str| {
        let n = tex_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // One line every 50 textures — 600+ lines of progress helps nobody.
        if n % 50 == 0 {
            eprintln!("  [tex {}/{}] {}", done, total, name);
        }
    };

    eprintln!("converting…");
    let result = match convert_to_glb_with_progress(&source, &settings, &progress) {
        Ok(r) => r,
        Err(e) => {
            let _ = writeln!(report, "## CONVERSION FAILED\n\n{e}\n");
            eprintln!("conversion failed: {e}");
            finish(&report, &out_dir, &f);
            std::process::exit(1);
        }
    };
    let convert_secs = t0.elapsed().as_secs_f64();
    eprintln!("converted in {:.1}s", convert_secs);

    let _ = writeln!(report, "## Timing\n");
    let _ = writeln!(report, "convert    {:.1}s", convert_secs);

    // ── Warnings ────────────────────────────────────────────────────────
    let _ = writeln!(
        report,
        "\n## Warnings ({})\n",
        result.warnings.len()
    );
    let mut warn_kinds: BTreeMap<String, usize> = BTreeMap::new();
    for w in &result.warnings {
        // Group by the text before the first colon so 600 near-identical
        // texture warnings collapse into one counted line.
        let key = w.split(':').next().unwrap_or(w).trim().to_string();
        *warn_kinds.entry(key).or_default() += 1;
    }
    for (k, n) in &warn_kinds {
        let _ = writeln!(report, "  {:>5}x  {}", n, k);
    }
    let _ = writeln!(report, "\n### Every warning verbatim\n");
    for w in &result.warnings {
        let _ = writeln!(report, "  - {}", w);
    }
    if !result.warnings.is_empty() {
        f.note(format!(
            "{} warnings across {} distinct kinds",
            result.warnings.len(),
            warn_kinds.len()
        ));
    }

    // ── Textures ────────────────────────────────────────────────────────
    let _ = writeln!(
        report,
        "\n## Extracted textures ({})\n",
        result.extracted_textures.len()
    );
    let mut by_ext: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_source: BTreeMap<&str, usize> = BTreeMap::new();
    let mut tex_names: HashMap<String, usize> = HashMap::new();
    for t in &result.extracted_textures {
        *by_ext.entry(t.extension.clone()).or_default() += 1;
        *by_source.entry(source_kind(&t.source)).or_default() += 1;
        *tex_names
            .entry(format!("{}.{}", t.name, t.extension))
            .or_default() += 1;
    }
    let _ = writeln!(report, "by extension:");
    for (k, n) in &by_ext {
        let _ = writeln!(report, "  {:>5}  .{}", n, k);
    }
    let _ = writeln!(report, "\nby source kind:");
    for (k, n) in &by_source {
        let _ = writeln!(report, "  {:>5}  {}", n, k);
    }

    let dupes: Vec<_> = tex_names.iter().filter(|(_, &n)| n > 1).collect();
    if !dupes.is_empty() {
        f.issue(format!(
            "{} texture output names collide — later writes overwrite earlier ones",
            dupes.len()
        ));
        let _ = writeln!(report, "\ncolliding output names:");
        for (name, n) in &dupes {
            let _ = writeln!(report, "  {}x  {}", n, name);
        }
    }

    // ── Stage 2: optimize ───────────────────────────────────────────────
    let opt = MeshOptSettings {
        vertex_cache: settings.optimize_vertex_cache,
        overdraw: settings.optimize_overdraw,
        vertex_fetch: settings.optimize_vertex_fetch,
        ..Default::default()
    };
    let pre_opt = result.glb_bytes.len();
    eprintln!("optimizing…");
    let t1 = Instant::now();
    let mut glb_bytes = match optimize_glb(&result.glb_bytes, &opt) {
        Ok(b) => b,
        Err(e) => {
            f.issue(format!("optimize_glb failed: {e}"));
            result.glb_bytes.clone()
        }
    };
    let opt_secs = t1.elapsed().as_secs_f64();
    let _ = writeln!(report, "optimize   {:.1}s", opt_secs);
    eprintln!("optimized in {:.1}s", opt_secs);

    // ── Stage 3: write textures ─────────────────────────────────────────
    let tex_dir = out_dir.join("textures");
    std::fs::create_dir_all(&tex_dir).expect("textures dir");
    eprintln!("writing {} textures…", result.extracted_textures.len());
    let t2 = Instant::now();
    let mut written: HashSet<String> = HashSet::new();
    let mut write_failures = Vec::new();
    let mut bytes_written = 0u64;
    for t in &result.extracted_textures {
        let file = format!("{}.{}", t.name, t.extension);
        let p = tex_dir.join(&file);
        match t.write_to(&p) {
            Ok(()) => {
                bytes_written += std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                written.insert(file);
            }
            Err(e) => write_failures.push(format!("{}: {}", file, e)),
        }
    }
    let write_secs = t2.elapsed().as_secs_f64();
    let _ = writeln!(report, "textures   {:.1}s", write_secs);
    if !write_failures.is_empty() {
        f.issue(format!(
            "{} textures failed to write",
            write_failures.len()
        ));
        let _ = writeln!(report, "\ntexture write failures:");
        for e in write_failures.iter().take(50) {
            let _ = writeln!(report, "  - {}", e);
        }
    }
    let _ = writeln!(
        report,
        "\ntexture bytes on disk: {}",
        human(bytes_written)
    );

    // ── Materials ───────────────────────────────────────────────────────
    let _ = writeln!(
        report,
        "\n## Extracted materials ({})\n",
        result.extracted_materials.len()
    );
    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    audit_materials(
        &result.extracted_materials,
        &written,
        &ext,
        &mut report,
        &mut f,
    );

    // ── Stage 4: animations ─────────────────────────────────────────────
    let anim_dir = out_dir.join("animations");
    std::fs::create_dir_all(&anim_dir).expect("anim dir");
    eprintln!("extracting animations…");
    let t3 = Instant::now();
    let anim = match ext.as_str() {
        "fbx" => renzora_import::extract_animations_from_fbx(&source, &anim_dir, &settings),
        "usd" | "usda" | "usdc" | "usdz" => {
            renzora_import::extract_animations_from_usd(&source, &anim_dir)
        }
        "bvh" => renzora_import::extract_animations_from_bvh(&source, &anim_dir),
        _ => renzora_import::extract_animations_from_glb(&glb_bytes, &anim_dir),
    };
    let anim_secs = t3.elapsed().as_secs_f64();
    let _ = writeln!(report, "animation  {:.1}s", anim_secs);
    let _ = writeln!(report, "\n## Animations\n");
    match &anim {
        Ok(a) => {
            let _ = writeln!(report, "clips written: {}", a.written_files.len());
            for p in a.written_files.iter().take(40) {
                let sz = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
                let _ = writeln!(report, "  - {} ({})", p, human(sz));
            }
            for w in &a.warnings {
                let _ = writeln!(report, "  warn: {}", w);
            }
            if !a.warnings.is_empty() {
                f.note(format!("{} animation warnings", a.warnings.len()));
            }
        }
        Err(e) => {
            let _ = writeln!(report, "FAILED: {}", e);
            f.note(format!("animation extraction failed: {e}"));
        }
    }

    // ── Stage 5: compact ────────────────────────────────────────────────
    let pre_compact = glb_bytes.len();
    eprintln!("compacting…");
    let t4 = Instant::now();
    match compact_glb(&glb_bytes, settings.extract_animations) {
        Ok(c) => glb_bytes = c,
        Err(e) => f.issue(format!("compact_glb failed: {e}")),
    }
    let compact_secs = t4.elapsed().as_secs_f64();
    let _ = writeln!(report, "compact    {:.1}s", compact_secs);

    // ── GLB size trail ──────────────────────────────────────────────────
    let _ = writeln!(report, "\n## GLB size through the pipeline\n");
    let _ = writeln!(report, "  source          {}", human(src_len));
    let _ = writeln!(report, "  after convert   {}", human(pre_opt as u64));
    let _ = writeln!(report, "  after optimize  {}", human(pre_compact as u64));
    let _ = writeln!(
        report,
        "  after compact   {}   ({:+.1}%)",
        human(glb_bytes.len() as u64),
        pct(pre_compact, glb_bytes.len())
    );
    if pre_compact > 0 && glb_bytes.len() as f64 / pre_compact as f64 > 0.98 {
        f.note("compaction reclaimed almost nothing — check whether it bailed on an extension");
    }

    // ── GLB structure ───────────────────────────────────────────────────
    let _ = writeln!(report, "\n## Final GLB structure\n");
    let structure = glb_stats(&glb_bytes);
    match &structure {
        Ok((json, txt)) => {
            let _ = writeln!(report, "{}", txt);
            audit_glb(json, &written, &out_dir, &mut report, &mut f);
        }
        Err(e) => {
            f.issue(format!("final GLB does not parse: {e}"));
            let _ = writeln!(report, "PARSE FAILED: {}", e);
        }
    }

    // ── Write the GLB ───────────────────────────────────────────────────
    let stem = source.file_stem().and_then(|s| s.to_str()).unwrap_or("model");
    let glb_path = out_dir.join(format!("{}.glb", stem));
    if let Err(e) = std::fs::write(&glb_path, &glb_bytes) {
        f.issue(format!("writing final GLB failed: {e}"));
    }

    let total = convert_secs + opt_secs + write_secs + anim_secs + compact_secs;
    let _ = writeln!(report, "\ntotal      {:.1}s", total);

    finish(&report, &out_dir, &f);
}

/// Check every material for internally inconsistent or unusable values, and
/// for texture URIs naming files no stage actually wrote.
fn audit_materials(
    mats: &[ExtractedPbrMaterial],
    written: &HashSet<String>,
    // Source extension, lowercased — some formats cannot carry a material at
    // all, and flagging those as a defect is noise that hides real ones.
    ext: &str,
    report: &mut String,
    f: &mut Findings,
) {
    let mut names: HashMap<String, usize> = HashMap::new();
    let mut unnamed = 0usize;
    let mut nonfinite = Vec::new();
    let mut out_of_range = Vec::new();
    let mut missing_uri: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut slot_usage: BTreeMap<&str, usize> = BTreeMap::new();
    let mut alpha_modes: BTreeMap<String, usize> = BTreeMap::new();
    let mut double_sided = 0usize;
    let mut textured = 0usize;

    for m in mats {
        if m.name.trim().is_empty() {
            unnamed += 1;
        }
        *names.entry(m.name.clone()).or_default() += 1;
        *alpha_modes
            .entry(format!("{:?}", m.alpha_mode))
            .or_default() += 1;
        if m.double_sided {
            double_sided += 1;
        }

        let floats: [(&str, f32); 4] = [
            ("metallic", m.metallic),
            ("roughness", m.roughness),
            ("alpha_cutoff", m.alpha_cutoff),
            ("base_color.a", m.base_color[3]),
        ];
        for (label, v) in floats {
            if !v.is_finite() {
                nonfinite.push(format!("{}: {} = {}", m.name, label, v));
            }
        }
        for (label, v) in [("metallic", m.metallic), ("roughness", m.roughness)] {
            if v.is_finite() && !(0.0..=1.0).contains(&v) {
                out_of_range.push(format!("{}: {} = {}", m.name, label, v));
            }
        }
        for (i, c) in m.base_color.iter().enumerate() {
            if !c.is_finite() {
                nonfinite.push(format!("{}: base_color[{}] = {}", m.name, i, c));
            }
        }
        for (i, c) in m.emissive.iter().enumerate() {
            if !c.is_finite() {
                nonfinite.push(format!("{}: emissive[{}] = {}", m.name, i, c));
            }
        }

        let slots: [(&str, &Option<String>); 10] = [
            ("base_color", &m.base_color_texture),
            ("normal", &m.normal_texture),
            ("metallic_roughness", &m.metallic_roughness_texture),
            ("roughness", &m.roughness_texture),
            ("metallic", &m.metallic_texture),
            ("emissive", &m.emissive_texture),
            ("occlusion", &m.occlusion_texture),
            ("spec_gloss", &m.specular_glossiness_texture),
            ("opacity", &m.opacity_texture),
            ("specular", &m.specular_texture),
        ];
        let mut any = false;
        for (slot, uri) in slots {
            let Some(uri) = uri else { continue };
            any = true;
            *slot_usage.entry(slot).or_default() += 1;
            // URIs are model-relative here, e.g. "textures/foo.rmip".
            let file = uri.rsplit('/').next().unwrap_or(uri).to_string();
            if !written.contains(&file) {
                missing_uri
                    .entry(file)
                    .or_default()
                    .push(format!("{}.{}", m.name, slot));
            }
        }
        if any {
            textured += 1;
        }
    }

    let unique = names.len();
    let _ = writeln!(report, "unique names:       {} of {}", unique, mats.len());
    let _ = writeln!(report, "with any texture:   {}", textured);
    let _ = writeln!(report, "double sided:       {}", double_sided);
    let _ = writeln!(report, "\nalpha modes:");
    for (k, n) in &alpha_modes {
        let _ = writeln!(report, "  {:>6}  {}", n, k);
    }
    let _ = writeln!(report, "\ntexture slots in use:");
    if slot_usage.is_empty() {
        let _ = writeln!(report, "  (none — every material is untextured)");
    }
    for (k, n) in &slot_usage {
        let _ = writeln!(report, "  {:>6}  {}", n, k);
    }

    if textured == 0 && !mats.is_empty() {
        // STL is geometry only; its single material is a placeholder we
        // synthesise, so "no texture references" is the format working as
        // designed rather than something the import lost.
        if ext == "stl" {
            f.note(format!(
                "{} materials, none textured — expected: {} stores no materials",
                mats.len(),
                ext.to_uppercase()
            ));
        } else {
            f.issue(format!(
                "all {} materials came out with NO texture references at all",
                mats.len()
            ));
        }
    }
    if unnamed > 0 {
        f.issue(format!("{} materials have an empty name", unnamed));
    }
    if unique < mats.len() {
        f.issue(format!(
            "material names collide: {} materials share {} names — the .material writer keys on \
             name, so only the last of each group survives on disk",
            mats.len(),
            unique
        ));
        let mut worst: Vec<_> = names.iter().filter(|(_, &n)| n > 1).collect();
        worst.sort_by(|a, b| b.1.cmp(a.1));
        let _ = writeln!(report, "\nworst name collisions:");
        for (name, n) in worst.iter().take(20) {
            let _ = writeln!(report, "  {:>5}x  {}", n, name);
        }
    }
    if !nonfinite.is_empty() {
        f.issue(format!("{} non-finite material values", nonfinite.len()));
        let _ = writeln!(report, "\nnon-finite values:");
        for s in nonfinite.iter().take(40) {
            let _ = writeln!(report, "  - {}", s);
        }
    }
    if !out_of_range.is_empty() {
        f.note(format!(
            "{} metallic/roughness values outside 0..1",
            out_of_range.len()
        ));
        let _ = writeln!(report, "\nout-of-range values:");
        for s in out_of_range.iter().take(40) {
            let _ = writeln!(report, "  - {}", s);
        }
    }
    if !missing_uri.is_empty() {
        f.issue(format!(
            "{} distinct texture URIs referenced by materials were never written to disk",
            missing_uri.len()
        ));
        let _ = writeln!(report, "\nmaterial texture URIs with no file behind them:");
        for (file, users) in missing_uri.iter().take(40) {
            let _ = writeln!(report, "  - {}  (used by {} slots, e.g. {})", file, users.len(), users[0]);
        }
    }

    let _ = writeln!(report, "\n### First 5 materials verbatim\n");
    for m in mats.iter().take(5) {
        let _ = writeln!(report, "{:#?}\n", m);
    }
}

/// Parse the final GLB and check its own internal references, plus that every
/// image URI names a file the texture stage actually produced.
fn audit_glb(
    json: &serde_json::Value,
    written: &HashSet<String>,
    out_dir: &Path,
    report: &mut String,
    f: &mut Findings,
) {
    let images = json
        .get("images")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut missing = Vec::new();
    let mut embedded = 0usize;
    let mut ext_counts: BTreeMap<String, usize> = BTreeMap::new();
    for (i, img) in images.iter().enumerate() {
        match img.get("uri").and_then(|u| u.as_str()) {
            Some(uri) => {
                let file = uri.rsplit('/').next().unwrap_or(uri).to_string();
                let ext = file.rsplit('.').next().unwrap_or("").to_lowercase();
                *ext_counts.entry(ext).or_default() += 1;
                if !written.contains(&file) && !out_dir.join(uri).exists() {
                    missing.push(format!("image {}: {}", i, uri));
                }
            }
            None => embedded += 1,
        }
    }
    let _ = writeln!(report, "\nimage URI extensions:");
    for (k, n) in &ext_counts {
        let _ = writeln!(report, "  {:>5}  .{}", n, k);
    }
    if embedded > 0 {
        let _ = writeln!(report, "\n{} images still embedded (no URI)", embedded);
        f.note(format!(
            "{} images remain embedded in the final GLB after extraction",
            embedded
        ));
    }
    if !missing.is_empty() {
        f.issue(format!(
            "{} GLB image URIs point at files that do not exist — the scene will fail to load them",
            missing.len()
        ));
        let _ = writeln!(report, "\nGLB image URIs with no file:");
        for m in missing.iter().take(40) {
            let _ = writeln!(report, "  - {}", m);
        }
    }

    // Materials in the GLB that reference no texture at all.
    let mats = json
        .get("materials")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let textured = mats
        .iter()
        .filter(|m| serde_json::to_string(m).map(|s| s.contains("Texture")).unwrap_or(false))
        .count();
    let _ = writeln!(
        report,
        "\nGLB materials referencing a texture: {} of {}",
        textured,
        mats.len()
    );
    if !mats.is_empty() && textured == 0 && !images.is_empty() {
        f.issue("GLB has images but no material references any of them");
    }
}

/// Parse a GLB and return its JSON plus a printable structure summary.
fn glb_stats(bytes: &[u8]) -> Result<(serde_json::Value, String), String> {
    let glb = gltf::Glb::from_slice(bytes).map_err(|e| e.to_string())?;
    let json: serde_json::Value =
        serde_json::from_slice(&glb.json).map_err(|e| e.to_string())?;
    let mut s = String::new();
    for key in [
        "scenes",
        "nodes",
        "meshes",
        "materials",
        "textures",
        "images",
        "samplers",
        "accessors",
        "bufferViews",
        "buffers",
        "animations",
        "skins",
        "cameras",
    ] {
        let n = json.get(key).and_then(|v| v.as_array()).map_or(0, |a| a.len());
        let _ = writeln!(s, "  {:<14} {}", key, n);
    }
    let _ = writeln!(
        s,
        "  {:<14} {}",
        "BIN chunk",
        human(glb.bin.as_ref().map_or(0, |b| b.len()) as u64)
    );
    let _ = writeln!(
        s,
        "  {:<14} {:?}",
        "extUsed",
        json.get("extensionsUsed")
    );
    let _ = writeln!(
        s,
        "  {:<14} {:?}",
        "extRequired",
        json.get("extensionsRequired")
    );
    Ok((json, s))
}

fn source_kind(s: &TextureSource) -> &'static str {
    match s {
        TextureSource::Embedded(_) => "Embedded (baked in memory)",
        TextureSource::File(_) => "File (copied from disk)",
        TextureSource::DdsToRmip { .. } => "DdsToRmip (block repack)",
        TextureSource::DdsClamped { .. } => "DdsClamped (mip drop)",
    }
}

fn human(n: u64) -> String {
    const U: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{} {}", n, U[i])
    } else {
        format!("{:.1} {}", v, U[i])
    }
}

fn pct(from: usize, to: usize) -> f64 {
    if from == 0 {
        return 0.0;
    }
    (to as f64 - from as f64) / from as f64 * 100.0
}

fn finish(report: &str, out_dir: &Path, f: &Findings) {
    let path = out_dir.join("report.txt");
    let mut full = String::from(report);
    let _ = writeln!(full, "\n\n## FINDINGS\n");
    let _ = writeln!(full, "### Issues ({})\n", f.issues.len());
    for i in &f.issues {
        let _ = writeln!(full, "  [!] {}", i);
    }
    let _ = writeln!(full, "\n### Notes ({})\n", f.notes.len());
    for n in &f.notes {
        let _ = writeln!(full, "  [.] {}", n);
    }
    let _ = std::fs::write(&path, &full);

    println!("\n================ FINDINGS ================");
    println!("Issues ({}):", f.issues.len());
    for i in &f.issues {
        println!("  [!] {}", i);
    }
    println!("\nNotes ({}):", f.notes.len());
    for n in &f.notes {
        println!("  [.] {}", n);
    }
    println!("\nfull report: {}", path.display());
}
