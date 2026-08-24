//! Compiled material artifact — the codegen output the editor bakes into a
//! `.material` every time the graph is saved.
//!
//! **One file per material.** `foo.material` holds the graph *and* the shader
//! it compiles to: node data for the editor, plus a [`CompiledArtifact`] with
//! the WGSL text and the binding metadata the renderer needs. At runtime the
//! resolver reads the embedded artifact and skips graph parsing and codegen
//! entirely.
//!
//! This used to be three files — `foo.material`, `foo.wgsl` and a
//! `foo.wgsl.meta` JSON sidecar. Splitting them meant three things could drift
//! out of step, and it invited hand-editing the generated `foo.wgsl`, which
//! silently desynced the shader from the graph that supposedly described it.
//! Folding them together makes the graph authoritative by construction. A
//! hand-written shader is a different kind of asset (`.wgsl` / `.shader`
//! resolved on their own) and is still editable in place — it just isn't
//! *this*.
//!
//! Files written by the old three-file layout still load: the resolver falls
//! back to [`MaterialGraph::wgsl_path`] when no artifact is embedded, and
//! [`save_compiled`] cleans up the stale pair the first time such a material
//! is saved.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::codegen::{self, MaterialParam, TextureBinding};
use super::graph::{AlphaMode, MaterialDomain, MaterialGraph};

/// Everything the renderer needs to assemble a `GraphMaterial` without
/// re-parsing the graph: the compiled WGSL plus the codegen outputs the shader
/// text alone can't express.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledArtifact {
    /// The fragment shader emitted by codegen.
    pub wgsl: String,
    /// Bindings, parameters and render state that go with it.
    pub meta: CompiledMaterialMeta,
}

/// Codegen outputs that live alongside the WGSL.
///
/// `alpha_mode` and `double_sided` are copied off the graph rather than read
/// from it at resolve time so the whole artifact is self-contained — the
/// resolver touches one struct, not two halves of the file that could disagree
/// if a graph edit ever landed without a recompile.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledMaterialMeta {
    pub domain: MaterialDomain,
    pub alpha_mode: AlphaMode,
    pub double_sided: bool,
    pub requires_transmission: bool,
    pub texture_bindings: Vec<TextureBinding>,
    pub parameters: Vec<MaterialParam>,
}

/// Filesystem path of the legacy `.wgsl` for a `.material` at
/// `material_fs_path`: same directory, same stem, `.wgsl` extension.
///
/// Nothing writes this any more — [`save_compiled`] embeds the shader instead.
/// It survives so the save path can delete the stale artifacts a
/// pre-embedding editor left behind.
pub fn legacy_wgsl_path_for_material(material_fs_path: &Path) -> PathBuf {
    material_fs_path.with_extension("wgsl")
}

/// Filesystem path of the legacy meta sidecar for a given `.wgsl` path. Just
/// appends `.meta`, kept centralized so all callers stay in sync.
pub fn legacy_meta_path_for_wgsl(wgsl_path: &Path) -> PathBuf {
    let mut p = wgsl_path.as_os_str().to_owned();
    p.push(".meta");
    PathBuf::from(p)
}

/// Compute a project-relative version of `fs_path`, normalised to forward
/// slashes. Returns the absolute path stringified if it can't be made
/// relative — the caller normally avoids that case by ensuring the target
/// lives under `project_root`.
pub fn project_relative(project_root: &Path, fs_path: &Path) -> String {
    fs_path
        .strip_prefix(project_root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| fs_path.to_string_lossy().replace('\\', "/"))
}

/// What a save-compile has to say. `errors` non-empty means no artifact was
/// embedded. `warnings` means one was, but the graph is on borrowed time.
#[derive(Debug, Default)]
pub struct SaveReport {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Run codegen on `graph` and store the result in [`MaterialGraph::compiled`]
/// so a subsequent `serde_json::to_string_pretty(&graph)` in the caller writes
/// the shader into the `.material` file itself.
///
/// Returns the [`SaveReport`]. Empty `errors` means the artifact was embedded
/// successfully. On codegen error the artifact is cleared, so the resolver
/// falls back to live codegen rather than rendering a stale shader.
///
/// The caller is responsible for writing the updated graph back to
/// `material_fs_path` — this function only produces the compiled output. It
/// touches the filesystem for one reason: removing the `.wgsl` / `.wgsl.meta`
/// pair a pre-embedding editor wrote next to this material. Leaving them would
/// strand files that no longer describe the graph, and which the resolver
/// would still happily load if something pointed at them directly.
pub fn save_compiled(
    graph: &mut MaterialGraph,
    material_fs_path: &Path,
) -> io::Result<SaveReport> {
    let result = codegen::compile_with_functions(graph, None);
    if !result.errors.is_empty() {
        graph.compiled = None;
        graph.wgsl_path = None;
        return Ok(SaveReport {
            errors: result.errors,
            warnings: result.warnings,
        });
    }

    graph.compiled = Some(CompiledArtifact {
        wgsl: result.fragment_shader,
        meta: CompiledMaterialMeta {
            domain: result.domain,
            alpha_mode: graph.alpha_mode,
            double_sided: graph.double_sided,
            requires_transmission: result.requires_transmission,
            texture_bindings: result.texture_bindings,
            parameters: result.parameters,
        },
    });
    graph.wgsl_path = None;
    remove_legacy_artifacts(material_fs_path);
    Ok(SaveReport {
        errors: Vec::new(),
        warnings: result.warnings,
    })
}

/// Delete the `.wgsl` + `.wgsl.meta` pair the three-file layout wrote next to
/// `material_fs_path`, if they're still there.
///
/// Best-effort: a material that was never saved by an older editor has nothing
/// to remove, and a failure to delete (read-only file, a text editor holding a
/// handle) isn't worth failing the save over — the embedded artifact is what
/// gets loaded either way.
fn remove_legacy_artifacts(material_fs_path: &Path) {
    let wgsl = legacy_wgsl_path_for_material(material_fs_path);
    let meta = legacy_meta_path_for_wgsl(&wgsl);
    for stale in [&meta, &wgsl] {
        if stale.exists() {
            match std::fs::remove_file(stale) {
                Ok(()) => {
                    bevy::log::info!("Removed stale compiled artifact {}", stale.display());
                }
                Err(e) => {
                    bevy::log::warn!("Could not remove {}: {e}", stale.display());
                }
            }
        }
    }
}

/// One-shot: run [`save_compiled`] then serialise the updated `graph` to a
/// pretty JSON string. Editor save sites use this to produce the `.material`
/// JSON they then write to disk.
pub fn save_compiled_and_serialize(
    graph: &mut MaterialGraph,
    material_fs_path: &Path,
) -> io::Result<(String, SaveReport)> {
    let report = save_compiled(graph, material_fs_path)?;
    let json = serde_json::to_string_pretty(graph).map_err(io::Error::other)?;
    Ok((json, report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material::graph::MaterialDomain;

    /// The whole point of the format: what a save writes is what a load reads,
    /// with no second file involved. If the artifact didn't survive the JSON
    /// round-trip, every material would silently fall back to live codegen —
    /// which still *renders*, so nothing would look broken.
    #[test]
    fn compiled_artifact_survives_a_save_load_round_trip() {
        let mut graph = MaterialGraph::new("RoundTrip", MaterialDomain::Surface);
        let (json, report) =
            save_compiled_and_serialize(&mut graph, Path::new("materials/round_trip.material"))
                .expect("save");
        assert!(report.errors.is_empty(), "codegen errors: {:?}", report.errors);

        let reloaded: MaterialGraph = serde_json::from_str(&json).expect("parse");
        let artifact = reloaded.compiled.as_ref().expect("artifact embedded");
        assert!(artifact.wgsl.contains("apply_pbr_lighting"));
        assert_eq!(artifact.meta.domain, MaterialDomain::Surface);
        assert_eq!(reloaded.compiled, graph.compiled);
    }

    /// A `.material` from before the merge has no `compiled` field at all.
    /// Parsing must still succeed — `#[serde(default)]` — and leave the
    /// `wgsl_path` link intact so the resolver can follow it.
    #[test]
    fn legacy_material_without_an_artifact_still_parses() {
        let legacy = r#"{
            "name": "Legacy",
            "domain": "Surface",
            "nodes": [],
            "connections": [],
            "next_id": 1,
            "wgsl_path": "materials/legacy.wgsl"
        }"#;
        let graph: MaterialGraph = serde_json::from_str(legacy).expect("parse");
        assert!(graph.compiled.is_none());
        assert_eq!(graph.wgsl_path.as_deref(), Some("materials/legacy.wgsl"));
    }

    /// Saving a legacy material drops the `.wgsl` + `.wgsl.meta` pair beside
    /// it. Leaving them would strand files that no longer describe the graph,
    /// and the resolver would still load one if a `MaterialRef` named it.
    #[test]
    fn saving_removes_the_legacy_pair() {
        let dir = std::env::temp_dir().join("renzora_precompiled_legacy_test");
        let _ = std::fs::create_dir_all(&dir);
        let material = dir.join("stale.material");
        let wgsl = dir.join("stale.wgsl");
        let meta = dir.join("stale.wgsl.meta");
        std::fs::write(&wgsl, "// old").expect("seed wgsl");
        std::fs::write(&meta, "{}").expect("seed meta");

        let mut graph = MaterialGraph::new("Stale", MaterialDomain::Surface);
        graph.wgsl_path = Some("stale.wgsl".into());
        save_compiled(&mut graph, &material).expect("save");

        assert!(!wgsl.exists(), "stale .wgsl should be gone");
        assert!(!meta.exists(), "stale .wgsl.meta should be gone");
        assert!(graph.wgsl_path.is_none(), "legacy link should be cleared");
        assert!(graph.compiled.is_some(), "artifact should be embedded");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
