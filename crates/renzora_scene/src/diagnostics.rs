//! The Scene Diagnostics probe: the editor's scene health readout, shown in
//! the companion instead of a dock panel.
//!
//! It used to be an ember panel with six collapsible sections, every value a
//! reactive binding and the warning feed a `keyed_list`. Each of those was
//! UI the editor had to lay out again whenever a value moved, and the warning
//! feed alone (up to 200 multi-line rows) was enough to keep `ui_layout_system`
//! busy every frame it stayed open. As a probe the same readout is plain data
//! built a few times a second while the companion watches it, and costs the
//! editor's own UI nothing.
//!
//! The data still comes from [`SceneDiagSnapshot`] (filled by
//! `update_scene_diag_snapshot`, gated on [`renzora::probes::probe_watched`]
//! with this probe's id), [`TabAssetCache`] and [`recent_warnings`]. The
//! colours the panel drew map onto [`ProbeLevel`]: its green is `Good`, its
//! red is `Bad`, its amber is `Warn`, its muted grey `Muted`.

use bevy::prelude::*;

use renzora::probes::{ProbeDef, ProbeItem, ProbeLevel, ProbeReport, RegisterProbe};

use crate::runtime_warnings::{recent_warnings, CapturedWarning, WarningLevel};
use crate::tab_asset_cache::{CameraEntry, SceneDiagSnapshot, TabAssetCache, TabPinSnapshot};

/// The probe's id. The snapshot system in `lib.rs` gates on the same string,
/// so the walk over every mesh only runs while a companion is showing it.
pub const PROBE_ID: &str = "scene_diagnostics";

/// Registers the Scene Diagnostics probe.
pub struct SceneDiagnostics;

impl Plugin for SceneDiagnostics {
    fn build(&self, app: &mut App) {
        app.register_probe(ProbeDef {
            id: PROBE_ID,
            title: "Scene Diagnostics",
            icon: "first-aid",
            category: "World",
            build: build_report,
            act: None,
        });
    }
}

/// A count that should be zero: green when it is, red when it is not. The
/// panel's `ok_or_bad`, kept so every health counter reads the same way.
fn ok_or_bad(n: usize) -> ProbeLevel {
    if n == 0 {
        ProbeLevel::Good
    } else {
        ProbeLevel::Bad
    }
}

fn yesno(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

/// Build the whole report from the world. Reads only: a missing resource (a
/// build without the scene cache, or before the first snapshot) shows zeros
/// rather than failing, the way the panel did.
fn build_report(world: &mut World) -> ProbeReport {
    let mut report = ProbeReport::default();
    let empty = SceneDiagSnapshot::default();
    let snap = world.get_resource::<SceneDiagSnapshot>().unwrap_or(&empty);

    materials_section(&mut report, snap);

    let tabs = world
        .get_resource::<TabAssetCache>()
        .map(|c| c.snapshot_for_diagnostics())
        .unwrap_or_default();
    tabs_section(&mut report, &tabs);

    assets_section(&mut report, snap);
    entities_section(&mut report, snap);
    cameras_section(&mut report, &snap.cameras);
    warnings_section(&mut report, &recent_warnings());
    report
}

fn materials_section(report: &mut ProbeReport, snap: &SceneDiagSnapshot) {
    let m = &snap.material;
    report
        .section("Materials & textures")
        .item(ProbeItem::stat("Entities w/ StandardMaterial", m.entities_with_std_mat))
        .item(ProbeItem::stat("Unique materials", m.unique_std_mats))
        .item(ProbeItem::stat("Materials loaded", m.mats_loaded))
        .item(ProbeItem::stat("Materials without textures", m.mats_with_no_textures))
        .item(ProbeItem::stat("Image handles seen", m.image_handles_seen))
        .item(ProbeItem::stat_at("Images alive", m.images_alive, ProbeLevel::Good))
        .item(ProbeItem::stat_at("Images MISSING", m.images_missing, ok_or_bad(m.images_missing)));
    // The snapshot already caps this at five, so it is shown whole.
    if !m.missing_sample_paths.is_empty() {
        report.item(ProbeItem::note("Sample missing paths:", ProbeLevel::Bad));
        for path in &m.missing_sample_paths {
            report.item(ProbeItem::note(format!("\u{2022} {path}"), ProbeLevel::Bad));
        }
    }
}

fn tabs_section(report: &mut ProbeReport, tabs: &[TabPinSnapshot]) {
    report.section("Tab asset cache");
    if tabs.is_empty() {
        report.item(ProbeItem::note("(no tabs pinned)", ProbeLevel::Muted));
        return;
    }
    let rows = tabs
        .iter()
        .map(|t| {
            let b = &t.breakdown;
            vec![
                t.tab_id.to_string(),
                t.gltf_count.to_string(),
                t.live_total.to_string(),
                b.meshes.to_string(),
                b.std_mats.to_string(),
                b.graph_mats.to_string(),
                b.scenes.to_string(),
                b.images.to_string(),
            ]
        })
        .collect();
    report.item(ProbeItem::table(
        &["Tab", "GLBs", "Live handles", "Mesh", "Std-mat", "Graph-mat", "Scene", "Image"],
        rows,
    ));
}

fn assets_section(report: &mut ProbeReport, snap: &SceneDiagSnapshot) {
    let a = &snap.assets;
    // `None` is an asset type this build does not register, which is not the
    // same as zero of them; the panel said "n/a" for it and so does this.
    let opt = |o: Option<usize>| o.map(|n| n.to_string()).unwrap_or_else(|| "n/a".to_string());
    report
        .section("Asset inventory")
        .item(ProbeItem::stat("Images", a.images))
        .item(ProbeItem::stat("Meshes", a.meshes))
        .item(ProbeItem::stat("StandardMaterials", a.standard_materials))
        .item(ProbeItem::stat("GraphMaterials", opt(a.graph_materials)))
        .item(ProbeItem::stat("CodeShaderMaterials", opt(a.code_shader_materials)))
        .item(ProbeItem::stat("Scenes", a.scenes))
        .item(ProbeItem::stat("Gltfs", a.gltfs))
        .item(ProbeItem::stat("Shaders", a.shaders))
        .item(ProbeItem::stat("AnimationClips", a.animation_clips))
        .item(ProbeItem::stat("AudioSources", opt(a.audio_sources)));
}

fn entities_section(report: &mut ProbeReport, snap: &SceneDiagSnapshot) {
    let e = &snap.entities;
    // Which material lane the meshes ended up in. `MaterialRef` can resolve
    // to either a plain StandardMaterial (trivial graph) or a GraphMaterial
    // (procedural), and the same file swinging between the two across a
    // reload is a bug no single counter shows.
    report
        .section("Entity health")
        .item(ProbeItem::stat("Total entities", e.total_entities))
        .item(ProbeItem::stat("Meshes on GraphMaterial", e.mesh_with_graph_mat))
        .item(ProbeItem::stat("Meshes on CodeShaderMaterial", e.mesh_with_code_mat))
        .item(ProbeItem::stat_at(
            "Mesh3d without material",
            e.mesh3d_without_material,
            ok_or_bad(e.mesh3d_without_material),
        ))
        .item(ProbeItem::stat_at(
            "MaterialRef unresolved",
            e.materialref_unresolved,
            ok_or_bad(e.materialref_unresolved),
        ))
        .item(ProbeItem::stat_at(
            "PendingMeshInstanceRehydrate",
            e.pending_rehydrate,
            ok_or_bad(e.pending_rehydrate),
        ))
        .item(ProbeItem::stat_at("Empty SceneRoots", e.empty_scene_roots, ok_or_bad(e.empty_scene_roots)))
        .item(ProbeItem::stat_at(
            "B0004 (parent w/o GlobalTransform)",
            e.b0004_violations,
            ok_or_bad(e.b0004_violations),
        ));

    // The count alone can't distinguish editor chrome (gizmo handles, entity
    // labels: material types the snapshot's query doesn't track) from scene
    // geometry that genuinely lost its binding, so list the offenders. The
    // snapshot caps the sample; say so when there are more, so a short list
    // is not read as the whole story.
    let sample = &e.mesh3d_without_material_sample;
    if !sample.is_empty() {
        report.item(ProbeItem::note("Meshes without material:", ProbeLevel::Bad));
        for line in sample {
            report.item(ProbeItem::note(format!("\u{2022} {line}"), ProbeLevel::Bad));
        }
        if e.mesh3d_without_material > sample.len() {
            report.item(ProbeItem::note(
                format!("(showing {} of {})", sample.len(), e.mesh3d_without_material),
                ProbeLevel::Muted,
            ));
        }
    }
}

fn cameras_section(report: &mut ProbeReport, cameras: &[CameraEntry]) {
    report.section("Cameras");
    if cameras.is_empty() {
        report.item(ProbeItem::note("(no cameras)", ProbeLevel::Muted));
        return;
    }
    let rows = cameras
        .iter()
        .map(|c| {
            let kind = if c.is_3d {
                "3D"
            } else if c.is_2d {
                "2D"
            } else {
                "?"
            };
            // N, D, M for the normal, depth and motion-vector prepasses, a dot
            // for each one missing: the panel's compact form, which keeps three
            // flags in one narrow column.
            let prepass = format!(
                "{}{}{}",
                if c.normal_prepass { "N" } else { "\u{b7}" },
                if c.depth_prepass { "D" } else { "\u{b7}" },
                if c.motion_prepass { "M" } else { "\u{b7}" },
            );
            vec![
                c.name.clone(),
                format!("{:?}", c.entity),
                kind.to_string(),
                if c.is_active { "active" } else { "inactive" }.to_string(),
                c.render_target.label().to_string(),
                yesno(c.hdr).to_string(),
                prepass,
                yesno(c.atmosphere).to_string(),
                yesno(c.atmosphere_env_light).to_string(),
                yesno(c.env_map_light).to_string(),
            ]
        })
        .collect();
    report.item(ProbeItem::table(
        &["Camera", "Entity", "Kind", "State", "Target", "HDR", "Prepass", "Atmo", "Atmo-env", "Env-map"],
        rows,
    ));
}

fn age_label(wn: &CapturedWarning) -> String {
    let secs = wn.age().as_secs();
    if secs < 60 {
        format!("{secs}s ago")
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else {
        format!("{}h ago", secs / 3600)
    }
}

/// One note per warning, newest first. Notes rather than a table because a
/// note carries a level, and the panel's whole point here was that an error
/// reads red and a warning amber at a glance. The capture buffer is capped at
/// `MAX_WARNINGS` and coalesces repeats into one entry with a count, so this
/// cannot grow without bound.
fn warnings_section(report: &mut ProbeReport, warnings: &[CapturedWarning]) {
    report.section("Recent runtime warnings");
    if warnings.is_empty() {
        report.item(ProbeItem::note(
            "(no warnings or errors captured this session)",
            ProbeLevel::Muted,
        ));
        return;
    }
    for wn in warnings.iter().rev() {
        let level = match wn.level {
            WarningLevel::Warn => ProbeLevel::Warn,
            WarningLevel::Error => ProbeLevel::Bad,
        };
        let repeat = if wn.count > 1 { format!("  \u{d7}{}", wn.count) } else { String::new() };
        report.item(ProbeItem::note(
            format!("{}{repeat}  ({}): {}", wn.target, age_label(wn), wn.message),
            level,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora::probes::{ProbeRegistry, ProbeSection};

    fn section<'a>(report: &'a ProbeReport, title: &str) -> &'a ProbeSection {
        report
            .sections
            .iter()
            .find(|s| s.title == title)
            .unwrap_or_else(|| panic!("no section {title}"))
    }

    fn stat_level(section: &ProbeSection, label: &str) -> ProbeLevel {
        section
            .items
            .iter()
            .find_map(|i| match i {
                ProbeItem::Stat { label: l, level, .. } if l == label => Some(*level),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no stat {label}"))
    }

    #[test]
    fn an_empty_world_still_builds_every_section() {
        let report = build_report(&mut World::new());
        let titles: Vec<&str> = report.sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Materials & textures",
                "Tab asset cache",
                "Asset inventory",
                "Entity health",
                "Cameras",
                "Recent runtime warnings",
            ]
        );
        assert_eq!(stat_level(section(&report, "Entity health"), "Mesh3d without material"), ProbeLevel::Good);
    }

    #[test]
    fn problems_read_red_and_list_their_offenders() {
        let mut world = World::new();
        let mut snap = SceneDiagSnapshot::default();
        snap.material.images_missing = 2;
        snap.material.missing_sample_paths = vec!["textures/a.png".into()];
        snap.entities.mesh3d_without_material = 20;
        snap.entities.mesh3d_without_material_sample = vec!["Cube: no MaterialRef".into()];
        world.insert_resource(snap);

        let report = build_report(&mut world);
        let mats = section(&report, "Materials & textures");
        assert_eq!(stat_level(mats, "Images MISSING"), ProbeLevel::Bad);
        assert!(mats.items.iter().any(|i| matches!(
            i,
            ProbeItem::Note { text, level: ProbeLevel::Bad } if text.contains("textures/a.png")
        )));

        let health = section(&report, "Entity health");
        assert_eq!(stat_level(health, "Mesh3d without material"), ProbeLevel::Bad);
        assert!(health.items.iter().any(|i| matches!(
            i,
            ProbeItem::Note { text, .. } if text == "(showing 1 of 20)"
        )));
    }

    #[test]
    fn the_plugin_registers_the_probe() {
        let mut app = App::new();
        app.add_plugins(SceneDiagnostics);
        let registry = app.world().resource::<ProbeRegistry>();
        assert!(registry.0.iter().any(|p| p.id == PROBE_ID && p.category == "World"));
    }
}
