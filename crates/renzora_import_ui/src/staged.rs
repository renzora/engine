//! Staging an import so it can be inspected before it touches the project.
//!
//! The importer already hands back everything in memory ([`ImportResult`]) and
//! writes nothing — the worker does that afterwards. That gap is where an
//! inspection step belongs, but a preview needs the GLB **and its textures**
//! sitting next to each other on disk, because the GLB references them by
//! relative URI and Bevy resolves those against the file's own folder.
//!
//! So a staged import writes the complete output tree into
//! `<project>/.cache/import_staging/<n>/` and stops. The user inspects it, and
//! then either:
//!
//! * **Commit** — the tree is moved into its real destination. Staging lives in
//!   the project's own cache directory specifically so this is a same-volume
//!   rename rather than a copy of (for a scene like Bistro) half a gigabyte.
//! * **Cancel** — the tree is deleted and the project never saw it.
//!
//! `.cache/` is not scanned by the asset browser, so a staged import is
//! invisible until it is committed.

use std::path::{Path, PathBuf};

use renzora_import::{ExtractedPbrMaterial, ExtractedTexture, GlbStats, TextureSource};

/// Where a staged import lives while it waits for a verdict.
pub const STAGING_SUBDIR: &str = ".cache/import_staging";

/// What the user decided about a staged import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewDecision {
    /// Move the staged tree into the project.
    Commit,
    /// Discard this file and continue with the rest of the queue.
    Skip,
    /// Discard this file and abandon everything still queued.
    CancelAll,
}

/// How serious an inspector finding is. Drives the colour of the chip, and the
/// order findings are listed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FlagLevel {
    /// Something is likely wrong with the result.
    Problem,
    /// Worth knowing, not necessarily wrong.
    Notice,
}

/// One thing the inspector noticed about a staged import.
#[derive(Debug, Clone)]
pub struct Flag {
    pub level: FlagLevel,
    pub text: String,
}

/// A material as the inspector shows it — flattened from
/// [`ExtractedPbrMaterial`] so the UI needs no import-crate types beyond this.
#[derive(Debug, Clone)]
pub struct MaterialRow {
    pub name: String,
    pub alpha_mode: String,
    pub double_sided: bool,
    /// Names of the texture slots this material actually fills.
    pub slots: Vec<String>,
    pub metallic: f32,
    pub roughness: f32,
    pub base_color: [f32; 4],
    pub emissive: [f32; 3],
    /// Model-relative URIs (`textures/foo.rmip`) for the slots a preview can
    /// actually render. Kept as paths rather than slot names because the
    /// material preview loads them straight off the staged tree.
    pub base_color_uri: Option<String>,
    pub normal_uri: Option<String>,
    pub metallic_roughness_uri: Option<String>,
    pub emissive_uri: Option<String>,
    pub occlusion_uri: Option<String>,
}

/// A texture as the inspector shows it.
#[derive(Debug, Clone)]
pub struct TextureRow {
    pub name: String,
    pub extension: String,
    /// Which pipeline produced it — baked in memory, copied, or repacked from
    /// a block-compressed DDS.
    pub origin: String,
    pub bytes: u64,
}

/// Everything the inspector needs about one staged file.
///
/// Deliberately plain data: it crosses the worker-thread boundary, so it holds
/// no handles, no `World` references and nothing that must be dropped on a
/// particular thread.
#[derive(Debug, Clone)]
pub struct StagedImport {
    pub file_name: String,
    pub source: PathBuf,
    /// Where the tree currently sits.
    pub staging_dir: PathBuf,
    /// Where it will land if committed.
    pub final_dir: PathBuf,
    /// The staged GLB, for the 3D preview to load.
    pub glb_path: PathBuf,
    pub stem: String,
    pub stats: Option<GlbStats>,
    pub materials: Vec<MaterialRow>,
    pub textures: Vec<TextureRow>,
    pub animations: Vec<String>,
    pub warnings: Vec<String>,
    pub glb_bytes: usize,
    pub texture_bytes: u64,
    pub flags: Vec<Flag>,
    /// Position in the queue, for "file 3 of 12".
    pub index: usize,
    pub total: usize,
    /// The `.material` writes this import owes, held until it is accepted.
    ///
    /// They ride along with the tree rather than firing from the worker,
    /// because the observer that handles them writes a file the moment it is
    /// triggered — and until the user accepts, nothing may touch the project.
    pub material_events: Vec<renzora::core::PbrMaterialExtracted>,
    /// What the user unchecked in the scene tree, in the staged GLB's own index
    /// space. Applied by [`apply_exclusions`] just before the tree is committed
    /// — never while it is being inspected, so the preview always shows the
    /// model the conversion actually produced and unchecking stays reversible.
    pub excluded: renzora_import::PruneSpec,
}

impl StagedImport {
    /// Findings at [`FlagLevel::Problem`], which is what the summary tab leads
    /// with and what the tab badge counts.
    pub fn problems(&self) -> usize {
        self.flags
            .iter()
            .filter(|f| f.level == FlagLevel::Problem)
            .count()
    }
}

/// Flatten an [`ExtractedPbrMaterial`] into a display row.
pub fn material_row(m: &ExtractedPbrMaterial) -> MaterialRow {
    let slots: Vec<String> = [
        ("base color", &m.base_color_texture),
        ("normal", &m.normal_texture),
        ("metal-rough", &m.metallic_roughness_texture),
        ("roughness", &m.roughness_texture),
        ("metallic", &m.metallic_texture),
        ("emissive", &m.emissive_texture),
        ("occlusion", &m.occlusion_texture),
        ("spec-gloss", &m.specular_glossiness_texture),
        ("opacity", &m.opacity_texture),
        ("specular", &m.specular_texture),
    ]
    .into_iter()
    .filter(|(_, uri)| uri.is_some())
    .map(|(name, _)| name.to_string())
    .collect();

    MaterialRow {
        name: m.name.clone(),
        alpha_mode: format!("{:?}", m.alpha_mode),
        double_sided: m.double_sided,
        slots,
        metallic: m.metallic,
        roughness: m.roughness,
        base_color: m.base_color,
        emissive: m.emissive,
        base_color_uri: m.base_color_texture.clone(),
        normal_uri: m.normal_texture.clone(),
        metallic_roughness_uri: m.metallic_roughness_texture.clone(),
        emissive_uri: m.emissive_texture.clone(),
        occlusion_uri: m.occlusion_texture.clone(),
    }
}

/// Flatten an [`ExtractedTexture`] into a display row. `bytes` is the size the
/// file actually took on disk, which the caller measures after writing —
/// several texture sources stream from disk and never know their own length.
pub fn texture_row(t: &ExtractedTexture, bytes: u64) -> TextureRow {
    let origin = match &t.source {
        TextureSource::Embedded(_) => "baked",
        TextureSource::File(_) => "copied",
        TextureSource::DdsToRmip { .. } => "DDS repack",
        TextureSource::DdsClamped { .. } => "DDS clamp",
    };
    TextureRow {
        name: t.name.clone(),
        extension: t.extension.clone(),
        origin: origin.to_string(),
        bytes,
    }
}

/// Suffixes an exporter uses to say "this material is cut out" or "this
/// material is two-sided" — conventions FBX itself cannot express, so an
/// importer that ignores them loses the information for good.
const EXPLICIT_MARKERS: &[&str] = &["doublesided", "double_sided", "masked"];

/// Words that merely *suggest* transparency. Much weaker evidence: a material
/// called `Pavement_Cobble_Leaves_BLENDSHADER` is a cobbled street with fallen
/// leaves on it — thoroughly opaque — and flagging it as a problem is how a
/// findings list teaches you to ignore findings.
const SOFT_HINTS: &[&str] = &[
    "foliage", "leaf", "leaves", "ivy", "hedge", "flower", "glass", "curtain", "fence", "grate",
];

/// Names that read as a surface, not a cutout. A hint inside one of these is
/// describing what is *printed on* the material rather than its transparency.
const SURFACE_WORDS: &[&str] = &[
    "blendshader", "pavement", "cobble", "ground", "floor", "wall", "brick", "concrete", "road",
    "terrain", "asphalt",
];

/// Look over a finished conversion and collect anything the user would want to
/// see before committing it.
///
/// This is the part that turns the pipeline's own data into an opinion. It only
/// ever reports; nothing here alters the import.
pub fn collect_flags(
    stats: Option<&GlbStats>,
    materials: &[MaterialRow],
    textures: &[TextureRow],
    animations: &[String],
    warnings: &[String],
    source: &Path,
) -> Vec<Flag> {
    let mut flags = Vec::new();
    let problem = |text: String| Flag {
        level: FlagLevel::Problem,
        text,
    };
    let notice = |text: String| Flag {
        level: FlagLevel::Notice,
        text,
    };

    if let Some(s) = stats {
        // One node holding many primitives means the source hierarchy was
        // merged away. Geometry is intact, but nothing in the scene can be
        // selected, moved or culled independently afterwards.
        if s.nodes <= 1 && s.primitives > 1 {
            flags.push(problem(format!(
                "Scene hierarchy flattened — {} primitives collapsed into {} node. \
                 Individual objects can't be selected or moved after import.",
                s.primitives, s.nodes
            )));
        }
        if s.has_uv_gap() {
            let uv = s
                .attribute_coverage
                .iter()
                .find(|(n, _)| n == "TEXCOORD_0")
                .map(|(_, n)| *n)
                .unwrap_or(0);
            flags.push(problem(format!(
                "{} of {} primitives have no UVs — textured materials render flat on them.",
                s.primitives - uv,
                s.primitives
            )));
        }
        if s.primitives == 0 {
            flags.push(problem("No geometry in the converted model.".into()));
        }
        if !s.extensions_required.is_empty() {
            flags.push(notice(format!(
                "Requires glTF extensions: {}",
                s.extensions_required.join(", ")
            )));
        }
    }

    // A material carrying an explicit exporter marker that still came out
    // opaque and single-sided means the importer dropped something it was told.
    // That is a problem worth acting on.
    let opaque_single = |m: &&MaterialRow| {
        m.alpha_mode.eq_ignore_ascii_case("opaque") && !m.double_sided
    };
    let explicit: Vec<&MaterialRow> = materials
        .iter()
        .filter(|m| {
            let lower = m.name.to_lowercase();
            EXPLICIT_MARKERS.iter().any(|k| lower.contains(k))
        })
        .filter(opaque_single)
        .collect();
    if !explicit.is_empty() {
        let sample: Vec<&str> = explicit.iter().take(3).map(|m| m.name.as_str()).collect();
        flags.push(problem(format!(
            "{} materials are named as cut-out or two-sided but imported opaque single-sided              (e.g. {}).",
            explicit.len(),
            sample.join(", ")
        )));
    }

    // Soft hints are a heads-up, not a defect — and never for something whose
    // name also reads as a solid surface.
    let suspect: Vec<&MaterialRow> = materials
        .iter()
        .filter(|m| {
            let lower = m.name.to_lowercase();
            SOFT_HINTS.iter().any(|h| lower.contains(h))
                && !SURFACE_WORDS.iter().any(|w| lower.contains(w))
                && !EXPLICIT_MARKERS.iter().any(|k| lower.contains(k))
        })
        .filter(opaque_single)
        .collect();
    if !suspect.is_empty() {
        let sample: Vec<&str> = suspect.iter().take(3).map(|m| m.name.as_str()).collect();
        flags.push(notice(format!(
            "{} materials have transparency-sounding names but no alpha in their textures, so they imported opaque (e.g. {}). Usually correct — worth a look if one should be see-through.",
            suspect.len(),
            sample.join(", ")
        )));
    }

    let ext = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // Formats that cannot carry a material at all. Saying "the source file
    // contains no textures" about one of these is true but useless — it reads
    // as though the file *could* have had them and didn't, when in fact no STL
    // ever does. Naming the format turns a shrug into an explanation.
    let format_has_no_materials = matches!(ext.as_str(), "stl");

    // A geometry-only format that *did* come out textured got those maps from a
    // sibling folder, bound over coordinates the importer invented. Say so:
    // whether it looks right depends entirely on what kind of map it is, and
    // that is not something the importer can decide for the user. Trying to —
    // by testing the image for edge continuity — read a packed atlas as
    // seamless often enough to be useless, so it states the fact instead.
    if format_has_no_materials && materials.iter().any(|m| !m.slots.is_empty()) {
        flags.push(notice(format!(
            "These textures came from a folder beside the model, not from the file. {} stores no \
             UVs, so the coordinates were projected from the bounding box: a tileable surface map \
             will look right, but one baked for a specific unwrap will not line up, and nothing in \
             the file can recover the original layout.",
            ext.to_uppercase()
        )));
    }

    if !materials.is_empty() && materials.iter().all(|m| m.slots.is_empty()) {
        // Distinguish "the source had none" from "the import lost them" — the
        // two read identically in a texture count, and only one is a problem.
        let source_had_none = stats.is_some_and(|s| s.images == 0);
        if format_has_no_materials {
            flags.push(notice(format!(
                "{} stores geometry only — no materials, textures or UVs — so the mesh imported \
                 with one neutral placeholder material. UV coordinates were projected from the \
                 model's bounding box, so a texture set picked in Import settings will map to \
                 something sensible.",
                ext.to_uppercase()
            )));
        } else if source_had_none {
            flags.push(notice(format!(
                "The source file contains no textures — all {} materials are colour-only in the \
                 file itself. Nothing was dropped on import.",
                materials.len()
            )));
        } else {
            flags.push(problem(format!(
                "None of the {} materials reference a texture, although the source has images — \
                 they were lost during conversion.",
                materials.len()
            )));
        }
    }

    // An FBX/USD whose animation extraction produced nothing is worth calling
    // out — those formats keep their clips in the source file, so an empty
    // result means the extractor found nothing it could read.
    if animations.is_empty() && matches!(ext.as_str(), "fbx" | "usd" | "usda" | "usdc" | "usdz") {
        flags.push(notice("No animation clips were extracted.".into()));
    }

    // Surface the converter's own warnings — currently they only reach the log.
    for w in warnings {
        flags.push(notice(w.clone()));
    }

    if textures.is_empty() && !materials.is_empty() && stats.is_some_and(|s| s.images > 0) {
        flags.push(notice("No textures were written for this model.".into()));
    }

    flags.sort_by_key(|f| f.level);
    flags
}

/// Rewrite the staged tree to drop everything the user unchecked.
///
/// Runs against the staging directory, so the model that lands in the project
/// is the edited one and nothing has to be undone afterwards. Three things go:
/// the geometry (out of the GLB, then out of its binary chunk), the `.material`
/// files for materials nothing uses any more (by returning their names, which
/// the caller uses to withhold the writes), and the texture files those
/// materials were the last reader of.
///
/// Best-effort by design. A model that cannot be re-read or re-written is
/// committed as it stands — importing the whole thing is a far better failure
/// than refusing the import over a checkbox.
///
/// Returns the names of the materials that were dropped.
pub fn apply_exclusions(staged: &StagedImport) -> Vec<String> {
    if staged.excluded.is_empty() {
        return Vec::new();
    }
    let Ok(bytes) = std::fs::read(&staged.glb_path) else {
        return Vec::new();
    };
    let pruned = match renzora_import::prune_glb(&bytes, &staged.excluded) {
        Ok(p) => p,
        Err(e) => {
            bevy::log::warn!("[import] prune failed for {}: {e}", staged.file_name);
            return Vec::new();
        }
    };
    // Removing primitives orphans their accessors; this is what actually
    // reclaims the bytes they were using. Animations are already extracted to
    // sibling `.anim` files by this point, so nothing here should drop them.
    let final_bytes = match renzora_import::compact_glb(&pruned.glb, false) {
        Ok(compacted) => compacted,
        Err(e) => {
            bevy::log::warn!("[import] compaction after prune failed: {e}");
            pruned.glb.clone()
        }
    };
    if let Err(e) = std::fs::write(&staged.glb_path, &final_bytes) {
        bevy::log::warn!("[import] could not write pruned GLB: {e}");
        return Vec::new();
    }

    // Delete the texture files nothing reads any more — but only after checking
    // the written GLB really has no reference left. The prune tracks textures
    // structurally; this is the cheap belt-and-braces test that a file which is
    // still named somewhere in the document is never deleted out from under it.
    let json = renzora_import::prune::glb_json_text(&final_bytes);
    for uri in &pruned.dropped_texture_uris {
        if json.as_deref().is_some_and(|j| j.contains(uri.as_str())) {
            continue;
        }
        let path = staged.staging_dir.join(uri);
        if path.starts_with(&staged.staging_dir) {
            let _ = std::fs::remove_file(&path);
        }
    }

    pruned.dropped_materials
}

/// Accept a staged tree into the project: move the files, and hand back the
/// `.material` events for the caller to fire once they are in place.
///
/// Returns the committed `.glb` path so the caller can request a thumbnail.
pub fn commit(staged: &StagedImport) -> std::io::Result<PathBuf> {
    merge_move(&staged.staging_dir, &staged.final_dir)?;
    let _ = std::fs::remove_dir_all(&staged.staging_dir);
    Ok(staged.final_dir.join(format!("{}.glb", staged.stem)))
}

/// Throw a staged tree away. Best-effort: a tree that is already gone is not
/// an error, and a locked file is not worth failing an otherwise-fine import.
pub fn discard(staged: &StagedImport) {
    let _ = std::fs::remove_dir_all(&staged.staging_dir);
}

/// Allocate an empty staging directory under the project cache.
///
/// Numbered rather than named after the model so two files with the same stem
/// in one queue can stage at once.
pub fn staging_dir(project_root: &Path, index: usize) -> PathBuf {
    project_root
        .join(STAGING_SUBDIR)
        .join(format!("{:04}", index))
}

/// Move everything in `from` into `to`, creating directories as needed and
/// merging into whatever is already there.
///
/// A plain directory rename would be enough for the per-file-folder layout
/// (its destination is always fresh), but the combined layout merges into an
/// existing folder, so this walks and moves file by file. Rename is tried
/// first and falls back to copy+remove, which is what happens when staging and
/// destination somehow land on different volumes.
pub fn merge_move(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            merge_move(&src, &dst)?;
            // Ignore failure: a directory that isn't empty yet is not fatal,
            // the files that mattered have already moved.
            let _ = std::fs::remove_dir(&src);
        } else {
            if std::fs::rename(&src, &dst).is_err() {
                std::fs::copy(&src, &dst)?;
                std::fs::remove_file(&src)?;
            }
        }
    }
    Ok(())
}

/// Total size of every file under `dir`.
pub fn dir_size(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .filter_map(|e| e.ok())
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_size(&e.path()),
            _ => e.metadata().map(|m| m.len()).unwrap_or(0),
        })
        .sum()
}

/// Format a byte count for display.
pub fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}

/// Format a count with thousands separators, so 2100000 reads as `2,100,000`.
pub fn thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mat(name: &str, alpha: &str, double: bool, slots: &[&str]) -> MaterialRow {
        MaterialRow {
            name: name.into(),
            alpha_mode: alpha.into(),
            double_sided: double,
            slots: slots.iter().map(|s| s.to_string()).collect(),
            metallic: 0.0,
            roughness: 0.5,
            base_color: [1.0; 4],
            emissive: [0.0; 3],
            base_color_uri: None,
            normal_uri: None,
            metallic_roughness_uri: None,
            emissive_uri: None,
            occlusion_uri: None,
        }
    }

    fn stats(nodes: usize, primitives: usize) -> GlbStats {
        GlbStats {
            nodes,
            primitives,
            attribute_coverage: vec![("TEXCOORD_0".into(), primitives)],
            ..Default::default()
        }
    }

    #[test]
    fn flags_flattened_hierarchy() {
        let f = collect_flags(Some(&stats(1, 132)), &[], &[], &[], &[], Path::new("a.fbx"));
        assert!(f.iter().any(|f| f.text.contains("flattened")));
    }

    #[test]
    fn no_flatten_flag_for_a_single_primitive_model() {
        let f = collect_flags(Some(&stats(1, 1)), &[], &[], &[], &[], Path::new("a.fbx"));
        assert!(!f.iter().any(|f| f.text.contains("flattened")));
    }

    #[test]
    fn flags_alpha_suspects_by_name() {
        let mats = vec![
            mat("Foliage_Leaves.DoubleSided", "Opaque", false, &["base color"]),
            mat("Concrete", "Opaque", false, &["base color"]),
        ];
        let f = collect_flags(None, &mats, &[], &[], &[], Path::new("a.fbx"));
        let hit = f
            .iter()
            .find(|f| f.text.contains("named as cut-out"))
            .expect("the explicitly-marked material should be flagged");
        assert!(hit.text.starts_with('1'), "and only that one");
    }

    #[test]
    fn alpha_suspect_not_flagged_when_already_masked() {
        let mats = vec![mat("Foliage_Leaves", "Mask", true, &["base color"])];
        let f = collect_flags(None, &mats, &[], &[], &[], Path::new("a.fbx"));
        assert!(!f.iter().any(|f| f.text.contains("alpha-tested")));
    }

    #[test]
    fn flags_when_no_material_has_a_texture() {
        let mats = vec![mat("A", "Opaque", false, &[]), mat("B", "Opaque", false, &[])];
        let f = collect_flags(None, &mats, &[], &[], &[], Path::new("a.glb"));
        assert!(f.iter().any(|f| f.text.contains("reference a texture")));
    }

    #[test]
    fn problems_sort_before_notices() {
        let mats = vec![mat("Glass_Window", "Opaque", false, &["base color"])];
        let f = collect_flags(
            Some(&stats(1, 4)),
            &mats,
            &[],
            &[],
            &["some warning".into()],
            Path::new("a.fbx"),
        );
        let first_notice = f.iter().position(|f| f.level == FlagLevel::Notice);
        let last_problem = f.iter().rposition(|f| f.level == FlagLevel::Problem);
        assert!(last_problem.unwrap() < first_notice.unwrap());
    }

    #[test]
    fn human_bytes_scales() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(2048), "2.0 KB");
        assert_eq!(human_bytes(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn thousands_groups_digits() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(2100000), "2,100,000");
    }

    #[test]
    fn merge_move_merges_into_existing_tree() {
        let base = std::env::temp_dir().join("renzora_merge_move_test");
        let _ = std::fs::remove_dir_all(&base);
        let from = base.join("from");
        let to = base.join("to");
        std::fs::create_dir_all(from.join("textures")).unwrap();
        std::fs::create_dir_all(to.join("textures")).unwrap();
        std::fs::write(from.join("a.glb"), b"glb").unwrap();
        std::fs::write(from.join("textures/new.rmip"), b"new").unwrap();
        std::fs::write(to.join("textures/existing.rmip"), b"old").unwrap();

        merge_move(&from, &to).unwrap();

        assert!(to.join("a.glb").exists());
        assert!(to.join("textures/new.rmip").exists());
        assert!(
            to.join("textures/existing.rmip").exists(),
            "pre-existing files must survive the merge"
        );
        assert!(!from.join("a.glb").exists(), "source should be moved, not copied");
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn untextured_source_is_a_notice_not_a_problem() {
        let mats = vec![mat("A", "Opaque", false, &[])];
        let st = GlbStats {
            images: 0,
            ..stats(1, 1)
        };
        let f = collect_flags(Some(&st), &mats, &[], &[], &[], Path::new("a.glb"));
        let hit = f
            .iter()
            .find(|f| f.text.contains("source file contains no textures"))
            .expect("explains that the source had none");
        assert_eq!(hit.level, FlagLevel::Notice, "nothing went wrong");
    }

    #[test]
    fn an_untextured_stl_explains_the_format_rather_than_shrugging() {
        // "The source file contains no textures" is true of an STL but useless:
        // it reads as though the file could have had them. Name the format.
        let mats = vec![mat("Default", "Opaque", false, &[])];
        let st = GlbStats {
            images: 0,
            ..stats(1, 1)
        };
        let f = collect_flags(Some(&st), &mats, &[], &[], &[], Path::new("a.stl"));
        let hit = f
            .iter()
            .find(|f| f.text.contains("stores geometry only"))
            .expect("names STL as the reason");
        assert_eq!(hit.level, FlagLevel::Notice);
        assert!(
            !f.iter().any(|f| f.text.contains("lost during conversion")),
            "an STL never had textures to lose"
        );
    }

    #[test]
    fn a_bound_sibling_set_warns_that_uvs_were_projected() {
        // The honest caveat: whether these line up depends on whether the map
        // was baked for an unwrap, which the importer cannot tell from the file.
        let mats = vec![mat("Steel", "Opaque", false, &["base color", "normal"])];
        let f = collect_flags(None, &mats, &[], &[], &[], Path::new("a.stl"));
        let hit = f
            .iter()
            .find(|f| f.text.contains("came from a folder beside the model"))
            .expect("explains where the textures came from");
        assert_eq!(hit.level, FlagLevel::Notice);
        assert!(hit.text.contains("baked for a specific unwrap"));
    }

    #[test]
    fn a_textured_gltf_gets_no_projection_warning() {
        // The caveat is specific to a format with no UVs of its own.
        let mats = vec![mat("Steel", "Opaque", false, &["base color"])];
        let f = collect_flags(None, &mats, &[], &[], &[], Path::new("a.glb"));
        assert!(!f.iter().any(|f| f.text.contains("came from a folder beside")));
    }

    #[test]
    fn losing_textures_that_exist_is_a_problem() {
        let mats = vec![mat("A", "Opaque", false, &[])];
        let st = GlbStats {
            images: 4,
            ..stats(1, 1)
        };
        let f = collect_flags(Some(&st), &mats, &[], &[], &[], Path::new("a.glb"));
        let hit = f
            .iter()
            .find(|f| f.text.contains("lost during conversion"))
            .expect("flags a real loss");
        assert_eq!(hit.level, FlagLevel::Problem);
    }

    #[test]
    fn explicit_marker_still_opaque_is_a_problem() {
        let mats = vec![mat("Leaves.DoubleSided", "Opaque", false, &["base color"])];
        let f = collect_flags(None, &mats, &[], &[], &[], Path::new("a.fbx"));
        let hit = f.iter().find(|f| f.text.contains("named as cut-out")).unwrap();
        assert_eq!(hit.level, FlagLevel::Problem);
    }

    #[test]
    fn a_cobbled_street_with_leaves_on_it_is_not_flagged() {
        // The false positive that made the findings list untrustworthy.
        let mats = vec![
            mat("Pavement_Cobble_Leaves_BLENDSHADER", "Opaque", false, &["base color"]),
            mat("Pavement_Cobblestone_Wet_Leaves_BLENDSHADER", "Opaque", false, &["base color"]),
        ];
        let f = collect_flags(None, &mats, &[], &[], &[], Path::new("a.fbx"));
        assert!(
            !f.iter().any(|f| f.text.contains("named as cut-out")),
            "a surface material must never be reported as a defect"
        );
        assert!(!f.iter().any(|f| f.text.contains("transparency-sounding")));
    }

    #[test]
    fn a_soft_hint_is_only_a_notice() {
        let mats = vec![mat("Spotlight_Glass_Emissive", "Opaque", false, &["base color"])];
        let f = collect_flags(None, &mats, &[], &[], &[], Path::new("a.fbx"));
        let hit = f
            .iter()
            .find(|f| f.text.contains("transparency-sounding"))
            .unwrap();
        assert_eq!(hit.level, FlagLevel::Notice);
    }

    #[test]
    fn a_correctly_masked_material_is_not_flagged_at_all() {
        let mats = vec![mat("Foliage_Leaves.DoubleSided", "Mask", true, &["base color"])];
        let f = collect_flags(None, &mats, &[], &[], &[], Path::new("a.fbx"));
        assert!(!f.iter().any(|f| f.text.contains("cut-out")));
        assert!(!f.iter().any(|f| f.text.contains("transparency-sounding")));
    }
}
