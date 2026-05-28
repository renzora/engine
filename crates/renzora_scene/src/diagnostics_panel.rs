//! Scene Diagnostics editor panel.
//!
//! Single-pane "what's wrong right now" scoreboard for the scene
//! crate's data. Reads from `SceneDiagSnapshot` (refreshed each frame
//! by `update_scene_diag_snapshot`) so this `&World`-only panel
//! doesn't have to query the ECS itself.
//!
//! Sections (top to bottom, each collapsible):
//!   1. Materials & textures — distinct StandardMaterials, loaded
//!      state, image-handle resolution. `Images MISSING > 0` flags
//!      the texture-vanish-on-tab-switch bug.
//!   2. Tab asset cache — per-tab GLB + live-handle pin counts.
//!   3. Asset inventory — `Assets<T>::len()` per type.
//!   4. Entity health — counts of suspicious entity patterns
//!      (Mesh3d-without-material, unresolved MaterialRef,
//!      pending GLTF rehydrate, empty SceneRoots, B0004 violations).
//!   5. Cameras — one row per Camera entity with its activation,
//!      render target, prepass attachments, atmosphere/IBL bindings.
//!
//! `images_missing`, `mesh3d_without_material`, `pending_rehydrate`,
//! `empty_scene_roots`, and `b0004_violations` flip red when > 0.

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;

use renzora_editor::{EditorPanel, PanelLocation};
use renzora_theme::ThemeManager;

use crate::runtime_warnings::{recent_warnings, CapturedWarning, WarningLevel};
use crate::tab_asset_cache::{
    AssetInventory, CameraEntry, EntityHealth, MaterialDiagnostics, SceneDiagSnapshot,
    TabAssetCache, TabPinSnapshot,
};

pub struct SceneDiagnosticsPanel;

impl EditorPanel for SceneDiagnosticsPanel {
    fn id(&self) -> &str {
        "scene_diagnostics"
    }

    fn title(&self) -> &str {
        "Scene Diagnostics"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::STETHOSCOPE)
    }

    fn category(&self) -> &str {
        "Debug"
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Right
    }

    fn min_size(&self) -> [f32; 2] {
        [260.0, 280.0]
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = world
            .get_resource::<ThemeManager>()
            .map(|tm| tm.active_theme.clone())
            .unwrap_or_default();
        let snap = world
            .get_resource::<SceneDiagSnapshot>()
            .cloned_or_default();
        let tabs = world
            .get_resource::<TabAssetCache>()
            .map(|c| c.snapshot_for_diagnostics())
            .unwrap_or_default();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.add_space(4.0);

                collapsing(ui, "materials", regular::PALETTE, "Materials & textures", &theme, true, |ui| {
                    render_material_section(ui, &snap.material, &theme);
                });

                collapsing(ui, "tabcache", regular::STACK, "Tab asset cache", &theme, false, |ui| {
                    render_tab_section(ui, &tabs, &theme);
                });

                collapsing(ui, "assets", regular::DATABASE, "Asset inventory", &theme, false, |ui| {
                    render_asset_section(ui, &snap.assets, &theme);
                });

                collapsing(ui, "entities", regular::TREE_STRUCTURE, "Entity health", &theme, true, |ui| {
                    render_entity_section(ui, &snap.entities, &theme);
                });

                collapsing(ui, "cameras", regular::VIDEO_CAMERA, "Cameras", &theme, false, |ui| {
                    render_cameras_section(ui, &snap.cameras, &theme);
                });

                let warnings = recent_warnings();
                collapsing(
                    ui,
                    "warnings",
                    regular::WARNING,
                    &format!("Recent runtime warnings ({})", warnings.len()),
                    &theme,
                    // Default open if there's something to show; closed when
                    // clean so the panel doesn't waste screen space.
                    !warnings.is_empty(),
                    |ui| render_warnings_section(ui, &warnings, &theme),
                );
            });
    }
}

// ─── Helpers ───────────────────────────────────────────────────────────────

/// `SceneDiagSnapshot` needs `Clone` for this; provide a defaulted
/// fallback so the panel always has something to render even before
/// the update system runs once.
trait ClonedOrDefault {
    type Out;
    fn cloned_or_default(self) -> Self::Out;
}

impl ClonedOrDefault for Option<&SceneDiagSnapshot> {
    type Out = SceneDiagSnapshot;
    fn cloned_or_default(self) -> SceneDiagSnapshot {
        match self {
            Some(s) => SceneDiagSnapshot {
                material: s.material.clone(),
                assets: s.assets.clone(),
                entities: s.entities.clone(),
                cameras: s.cameras.clone(),
            },
            None => SceneDiagSnapshot::default(),
        }
    }
}

fn collapsing(
    ui: &mut egui::Ui,
    id_seed: &str,
    icon: &str,
    label: &str,
    theme: &renzora_theme::Theme,
    default_open: bool,
    body: impl FnOnce(&mut egui::Ui),
) {
    let color = theme.text.primary.to_color32();
    let id = egui::Id::new(("scene_diag_section", id_seed));
    egui::CollapsingHeader::new(
        egui::RichText::new(format!("{icon}  {label}"))
            .size(12.0)
            .strong()
            .color(color),
    )
    .id_salt(id)
    .default_open(default_open)
    .show(ui, |ui| {
        ui.add_space(2.0);
        body(ui);
        ui.add_space(4.0);
    });
}

fn stat_row(ui: &mut egui::Ui, label: &str, value: impl ToString, value_color: egui::Color32) {
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width() - 70.0, 16.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.label(label);
            },
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(value.to_string())
                    .monospace()
                    .color(value_color),
            );
        });
    });
}

fn ok_color() -> egui::Color32 {
    egui::Color32::from_rgb(120, 200, 120)
}
fn bad_color() -> egui::Color32 {
    egui::Color32::from_rgb(230, 110, 110)
}
fn ok_or_bad(n: usize) -> egui::Color32 {
    if n == 0 { ok_color() } else { bad_color() }
}

// ─── Sections ──────────────────────────────────────────────────────────────

fn render_material_section(
    ui: &mut egui::Ui,
    m: &MaterialDiagnostics,
    theme: &renzora_theme::Theme,
) {
    let neutral = theme.text.secondary.to_color32();
    stat_row(ui, "Entities w/ StandardMaterial", m.entities_with_std_mat, neutral);
    stat_row(ui, "Unique materials", m.unique_std_mats, neutral);
    stat_row(ui, "Materials loaded", m.mats_loaded, neutral);
    stat_row(ui, "Materials without textures", m.mats_with_no_textures, neutral);
    ui.add_space(4.0);
    stat_row(ui, "Image handles seen", m.image_handles_seen, neutral);
    stat_row(ui, "Images alive", m.images_alive, ok_color());
    stat_row(ui, "Images MISSING", m.images_missing, ok_or_bad(m.images_missing));

    if !m.missing_sample_paths.is_empty() {
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(format!("{}  Sample missing paths:", regular::WARNING))
                .color(bad_color()),
        );
        for p in &m.missing_sample_paths {
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                ui.label(
                    egui::RichText::new(format!("• {p}"))
                        .color(bad_color())
                        .monospace()
                        .size(11.0),
                );
            });
        }
    }
}

fn render_tab_section(
    ui: &mut egui::Ui,
    tabs: &[TabPinSnapshot],
    theme: &renzora_theme::Theme,
) {
    let neutral = theme.text.secondary.to_color32();
    let muted = theme.text.muted.to_color32();
    if tabs.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(egui::RichText::new("(no tabs pinned)").color(muted).italics());
        });
        return;
    }
    stat_row(ui, "Tabs pinned", tabs.len(), neutral);
    ui.add_space(4.0);
    for t in tabs {
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!("Tab {}", t.tab_id))
                    .strong()
                    .color(theme.text.primary.to_color32()),
            );
            ui.label(
                egui::RichText::new(format!(
                    "  {} GLB(s)  ·  {} live handles",
                    t.gltf_count, t.live_total
                ))
                .color(muted),
            );
        });
        ui.horizontal(|ui| {
            ui.add_space(18.0);
            ui.label(
                egui::RichText::new(format!(
                    "{} mesh  ·  {} std-mat  ·  {} graph-mat  ·  {} scene  ·  {} image",
                    t.breakdown.meshes,
                    t.breakdown.std_mats,
                    t.breakdown.graph_mats,
                    t.breakdown.scenes,
                    t.breakdown.images,
                ))
                .color(muted)
                .size(11.0),
            );
        });
        ui.add_space(2.0);
    }
}

fn render_asset_section(ui: &mut egui::Ui, a: &AssetInventory, theme: &renzora_theme::Theme) {
    let neutral = theme.text.secondary.to_color32();
    let muted = theme.text.muted.to_color32();
    stat_row(ui, "Images", a.images, neutral);
    stat_row(ui, "Meshes", a.meshes, neutral);
    stat_row(ui, "StandardMaterials", a.standard_materials, neutral);
    match a.graph_materials {
        Some(n) => stat_row(ui, "GraphMaterials", n, neutral),
        None => stat_row(ui, "GraphMaterials", "n/a", muted),
    }
    match a.code_shader_materials {
        Some(n) => stat_row(ui, "CodeShaderMaterials", n, neutral),
        None => stat_row(ui, "CodeShaderMaterials", "n/a", muted),
    }
    stat_row(ui, "Scenes", a.scenes, neutral);
    stat_row(ui, "Gltfs", a.gltfs, neutral);
    stat_row(ui, "Shaders", a.shaders, neutral);
    stat_row(ui, "AnimationClips", a.animation_clips, neutral);
    match a.audio_sources {
        Some(n) => stat_row(ui, "AudioSources", n, neutral),
        None => stat_row(ui, "AudioSources", "n/a", muted),
    }
}

fn render_entity_section(ui: &mut egui::Ui, e: &EntityHealth, theme: &renzora_theme::Theme) {
    let neutral = theme.text.secondary.to_color32();
    stat_row(ui, "Total entities", e.total_entities, neutral);
    ui.add_space(4.0);
    stat_row(
        ui,
        "Mesh3d without material",
        e.mesh3d_without_material,
        ok_or_bad(e.mesh3d_without_material),
    );
    stat_row(
        ui,
        "MaterialRef unresolved",
        e.materialref_unresolved,
        ok_or_bad(e.materialref_unresolved),
    );
    stat_row(
        ui,
        "PendingMeshInstanceRehydrate",
        e.pending_rehydrate,
        ok_or_bad(e.pending_rehydrate),
    );
    stat_row(
        ui,
        "Empty SceneRoots",
        e.empty_scene_roots,
        ok_or_bad(e.empty_scene_roots),
    );
    stat_row(
        ui,
        "B0004 (parent w/o GlobalTransform)",
        e.b0004_violations,
        ok_or_bad(e.b0004_violations),
    );
}

fn render_cameras_section(
    ui: &mut egui::Ui,
    cams: &[CameraEntry],
    theme: &renzora_theme::Theme,
) {
    let muted = theme.text.muted.to_color32();
    if cams.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(egui::RichText::new("(no cameras)").color(muted).italics());
        });
        return;
    }
    for c in cams {
        let primary = theme.text.primary.to_color32();
        let active_color = if c.is_active { ok_color() } else { muted };
        let kind = if c.is_3d {
            "3D"
        } else if c.is_2d {
            "2D"
        } else {
            "?"
        };
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!("{}  {}", regular::CIRCLE, c.name))
                    .strong()
                    .color(primary),
            );
            ui.label(
                egui::RichText::new(format!("  ({}, {:?})", kind, c.entity))
                    .color(muted)
                    .size(11.0),
            );
        });
        ui.horizontal(|ui| {
            ui.add_space(18.0);
            ui.label(
                egui::RichText::new(format!(
                    "{}  ·  target: {}  ·  hdr: {}",
                    if c.is_active { "active" } else { "inactive" },
                    c.render_target.label(),
                    yesno(c.hdr),
                ))
                .color(active_color)
                .size(11.0),
            );
        });
        ui.horizontal(|ui| {
            ui.add_space(18.0);
            ui.label(
                egui::RichText::new(format!(
                    "prepass: {}{}{}  ·  atmo: {}  ·  atmo-env: {}  ·  env-map: {}",
                    if c.normal_prepass { "N" } else { "·" },
                    if c.depth_prepass { "D" } else { "·" },
                    if c.motion_prepass { "M" } else { "·" },
                    yesno(c.atmosphere),
                    yesno(c.atmosphere_env_light),
                    yesno(c.env_map_light),
                ))
                .color(muted)
                .size(11.0),
            );
        });
        ui.add_space(3.0);
    }
}

fn yesno(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
}

fn render_warnings_section(
    ui: &mut egui::Ui,
    warnings: &[CapturedWarning],
    theme: &renzora_theme::Theme,
) {
    let muted = theme.text.muted.to_color32();
    if warnings.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("(no warnings or errors captured this session)")
                    .color(muted)
                    .italics(),
            );
        });
        return;
    }
    // Newest first.
    for w in warnings.iter().rev() {
        let (level_color, level_icon) = match w.level {
            WarningLevel::Warn => (
                egui::Color32::from_rgb(230, 180, 80),
                regular::WARNING,
            ),
            WarningLevel::Error => (bad_color(), regular::WARNING_CIRCLE),
        };
        let age = w.age();
        let age_label = if age.as_secs() < 60 {
            format!("{}s ago", age.as_secs())
        } else if age.as_secs() < 3600 {
            format!("{}m ago", age.as_secs() / 60)
        } else {
            format!("{}h ago", age.as_secs() / 3600)
        };
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(level_icon).color(level_color));
            ui.label(
                egui::RichText::new(&w.target)
                    .color(theme.text.primary.to_color32())
                    .size(11.0)
                    .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(age_label)
                        .color(muted)
                        .monospace()
                        .size(10.0),
                );
            });
        });
        ui.horizontal(|ui| {
            ui.add_space(18.0);
            ui.label(
                egui::RichText::new(&w.message)
                    .color(theme.text.secondary.to_color32())
                    .size(11.0),
            );
        });
        ui.add_space(2.0);
    }
}
