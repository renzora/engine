//! Bevy-native (ember) Scene Diagnostics panel — a faithful port of the egui
//! `SceneDiagnosticsPanel`: six collapsible sections (materials & textures, tab
//! asset cache, asset inventory, entity health, cameras, recent warnings) read
//! from `SceneDiagSnapshot` / `TabAssetCache` / `recent_warnings()`. Every value
//! is a binding; the variable-length sections are `keyed_list`s.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, bind_text, bind_text_color, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::collapsible;

use crate::runtime_warnings::{recent_warnings, CapturedWarning, WarningLevel};
use crate::tab_asset_cache::{CameraEntry, SceneDiagSnapshot, TabAssetCache, TabPinSnapshot};

const SECONDARY: (u8, u8, u8) = (170, 170, 180);
const OK: (u8, u8, u8) = (120, 200, 120);
const BAD: (u8, u8, u8) = (230, 110, 110);

/// Registers the bevy-native Scene Diagnostics content.
pub struct NativeSceneDiagnostics;

impl Plugin for NativeSceneDiagnostics {
    fn build(&self, app: &mut App) {
        app.register_panel_content("scene_diagnostics", true, build);
    }
}

fn snap<R: Default>(w: &World, f: impl FnOnce(&SceneDiagSnapshot) -> R) -> R {
    w.get_resource::<SceneDiagSnapshot>().map(f).unwrap_or_default()
}

fn ok_or_bad(n: usize) -> Color {
    if n == 0 {
        rgb(OK)
    } else {
        rgb(BAD)
    }
}

fn hash_str(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

// ── Small builders ───────────────────────────────────────────────────────────

/// A `label …… value` row (label left, mono value right, both bindings).
fn stat_row<V, C>(commands: &mut Commands, fonts: &EmberFonts, label: &str, value: V, color: C) -> Entity
where
    V: Fn(&World) -> String + Send + Sync + 'static,
    C: Fn(&World) -> Color + Send + Sync + 'static,
{
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            width: Val::Percent(100.0),
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let l = commands
        .spawn((Text::new(label), ui_font(&fonts.ui, 12.0), TextColor(rgb(SECONDARY))))
        .id();
    let gap = commands
        .spawn(Node {
            flex_grow: 1.0,
            ..default()
        })
        .id();
    let v = commands
        .spawn((Text::new(""), ui_font(&fonts.mono, 12.0), TextColor(rgb(text_primary()))))
        .id();
    bind_text(commands, v, value);
    bind_text_color(commands, v, color);
    commands.entity(row).add_children(&[l, gap, v]);
    row
}

/// A static `label …… value` row with a fixed color (no binding).
fn neutral_row<V>(commands: &mut Commands, fonts: &EmberFonts, label: &str, value: V) -> Entity
where
    V: Fn(&World) -> String + Send + Sync + 'static,
{
    stat_row(commands, fonts, label, value, |_| rgb(SECONDARY))
}

fn note(commands: &mut Commands, fonts: &EmberFonts, text: &str, color: (u8, u8, u8)) -> Entity {
    commands
        .spawn((Text::new(text), ui_font(&fonts.ui, 11.0), TextColor(rgb(color))))
        .id()
}

fn spacer(commands: &mut Commands, h: f32) -> Entity {
    commands
        .spawn(Node {
            height: Val::Px(h),
            ..default()
        })
        .id()
}

// ── Panel ────────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            padding: UiRect::all(Val::Px(8.0)),
            row_gap: Val::Px(2.0),
            ..default()
        })
        .id();

    let (m, m_body) = collapsible(commands, fonts, Some("palette"), "Materials & textures", true);
    materials_section(commands, fonts, m_body);

    let (t, t_body) = collapsible(commands, fonts, Some("stack"), "Tab asset cache", false);
    keyed_list(commands, t_body, tabs_snapshot);

    let (a, a_body) = collapsible(commands, fonts, Some("database"), "Asset inventory", false);
    assets_section(commands, fonts, a_body);

    let (e, e_body) = collapsible(commands, fonts, Some("tree-structure"), "Entity health", true);
    entities_section(commands, fonts, e_body);

    let (c, c_body) = collapsible(commands, fonts, Some("video-camera"), "Cameras", false);
    keyed_list(commands, c_body, cameras_snapshot);

    let (w, w_body) = collapsible(commands, fonts, Some("warning"), "Recent runtime warnings", true);
    keyed_list(commands, w_body, warnings_snapshot);

    commands.entity(root).add_children(&[m, t, a, e, c, w]);
    root
}

fn materials_section(commands: &mut Commands, fonts: &EmberFonts, body: Entity) {
    let rows = [
        neutral_row(commands, fonts, "Entities w/ StandardMaterial", |w| snap(w, |s| s.material.entities_with_std_mat).to_string()),
        neutral_row(commands, fonts, "Unique materials", |w| snap(w, |s| s.material.unique_std_mats).to_string()),
        neutral_row(commands, fonts, "Materials loaded", |w| snap(w, |s| s.material.mats_loaded).to_string()),
        neutral_row(commands, fonts, "Materials without textures", |w| snap(w, |s| s.material.mats_with_no_textures).to_string()),
        spacer(commands, 4.0),
        neutral_row(commands, fonts, "Image handles seen", |w| snap(w, |s| s.material.image_handles_seen).to_string()),
        stat_row(commands, fonts, "Images alive", |w| snap(w, |s| s.material.images_alive).to_string(), |_| rgb(OK)),
        stat_row(commands, fonts, "Images MISSING", |w| snap(w, |s| s.material.images_missing).to_string(), |w| ok_or_bad(snap(w, |s| s.material.images_missing))),
    ];
    commands.entity(body).add_children(&rows);

    let hdr = note(commands, fonts, "\u{26a0}  Sample missing paths:", BAD);
    bind_display(commands, hdr, |w| !snap(w, |s| s.material.missing_sample_paths.is_empty()));
    let paths = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            ..default()
        })
        .id();
    keyed_list(commands, paths, missing_paths_snapshot);
    commands.entity(body).add_children(&[hdr, paths]);
}

fn assets_section(commands: &mut Commands, fonts: &EmberFonts, body: Entity) {
    let opt = |o: Option<usize>| o.map(|n| n.to_string()).unwrap_or_else(|| "n/a".to_string());
    let rows = [
        neutral_row(commands, fonts, "Images", |w| snap(w, |s| s.assets.images).to_string()),
        neutral_row(commands, fonts, "Meshes", |w| snap(w, |s| s.assets.meshes).to_string()),
        neutral_row(commands, fonts, "StandardMaterials", |w| snap(w, |s| s.assets.standard_materials).to_string()),
        neutral_row(commands, fonts, "GraphMaterials", move |w| opt(snap(w, |s| s.assets.graph_materials))),
        neutral_row(commands, fonts, "CodeShaderMaterials", move |w| opt(snap(w, |s| s.assets.code_shader_materials))),
        neutral_row(commands, fonts, "Scenes", |w| snap(w, |s| s.assets.scenes).to_string()),
        neutral_row(commands, fonts, "Gltfs", |w| snap(w, |s| s.assets.gltfs).to_string()),
        neutral_row(commands, fonts, "Shaders", |w| snap(w, |s| s.assets.shaders).to_string()),
        neutral_row(commands, fonts, "AnimationClips", |w| snap(w, |s| s.assets.animation_clips).to_string()),
        neutral_row(commands, fonts, "AudioSources", move |w| opt(snap(w, |s| s.assets.audio_sources))),
    ];
    commands.entity(body).add_children(&rows);
}

fn entities_section(commands: &mut Commands, fonts: &EmberFonts, body: Entity) {
    let total = neutral_row(commands, fonts, "Total entities", |w| snap(w, |s| s.entities.total_entities).to_string());
    let sp = spacer(commands, 4.0);
    let m = stat_row(commands, fonts, "Mesh3d without material", |w| snap(w, |s| s.entities.mesh3d_without_material).to_string(), |w| ok_or_bad(snap(w, |s| s.entities.mesh3d_without_material)));
    let r = stat_row(commands, fonts, "MaterialRef unresolved", |w| snap(w, |s| s.entities.materialref_unresolved).to_string(), |w| ok_or_bad(snap(w, |s| s.entities.materialref_unresolved)));
    let p = stat_row(commands, fonts, "PendingMeshInstanceRehydrate", |w| snap(w, |s| s.entities.pending_rehydrate).to_string(), |w| ok_or_bad(snap(w, |s| s.entities.pending_rehydrate)));
    let e = stat_row(commands, fonts, "Empty SceneRoots", |w| snap(w, |s| s.entities.empty_scene_roots).to_string(), |w| ok_or_bad(snap(w, |s| s.entities.empty_scene_roots)));
    let b = stat_row(commands, fonts, "B0004 (parent w/o GlobalTransform)", |w| snap(w, |s| s.entities.b0004_violations).to_string(), |w| ok_or_bad(snap(w, |s| s.entities.b0004_violations)));
    commands.entity(body).add_children(&[total, sp, m, r, p, e, b]);
}

// ── Lists ────────────────────────────────────────────────────────────────────

fn missing_paths_snapshot(world: &World) -> KeyedSnapshot {
    let paths = snap(world, |s| s.material.missing_sample_paths.clone());
    let items: Vec<(u64, u64)> = paths.iter().map(|p| (hash_str(p), 0)).collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            c.spawn((
                Text::new(format!("\u{2022} {}", paths[i])),
                ui_font(&f.mono, 11.0),
                TextColor(rgb(BAD)),
                Node {
                    margin: UiRect::left(Val::Px(14.0)),
                    ..default()
                },
            ))
            .id()
        }),
    }
}

fn tabs_snapshot(world: &World) -> KeyedSnapshot {
    let tabs: Vec<TabPinSnapshot> = world
        .get_resource::<TabAssetCache>()
        .map(|c| c.snapshot_for_diagnostics())
        .unwrap_or_default();
    if tabs.is_empty() {
        return note_snapshot("(no tabs pinned)");
    }
    let items: Vec<(u64, u64)> = tabs
        .iter()
        .map(|t| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (t.tab_id, t.gltf_count, t.live_total).hash(&mut h);
            (t.tab_id, h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| tab_row(c, f, &tabs[i])),
    }
}

fn tab_row(commands: &mut Commands, fonts: &EmberFonts, t: &TabPinSnapshot) -> Entity {
    let col = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            margin: UiRect::bottom(Val::Px(2.0)),
            ..default()
        })
        .id();
    let head = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    let id = commands
        .spawn((Text::new(format!("Tab {}", t.tab_id)), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary()))))
        .id();
    let counts = commands
        .spawn((
            Text::new(format!("{} GLB(s)  \u{b7}  {} live handles", t.gltf_count, t.live_total)),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    commands.entity(head).add_children(&[id, counts]);
    let b = &t.breakdown;
    let sub = commands
        .spawn((
            Text::new(format!(
                "{} mesh  \u{b7}  {} std-mat  \u{b7}  {} graph-mat  \u{b7}  {} scene  \u{b7}  {} image",
                b.meshes, b.std_mats, b.graph_mats, b.scenes, b.images
            )),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node {
                margin: UiRect::left(Val::Px(14.0)),
                ..default()
            },
        ))
        .id();
    commands.entity(col).add_children(&[head, sub]);
    col
}

fn cameras_snapshot(world: &World) -> KeyedSnapshot {
    let cams = snap(world, |s| s.cameras.clone());
    if cams.is_empty() {
        return note_snapshot("(no cameras)");
    }
    let items: Vec<(u64, u64)> = cams
        .iter()
        .map(|c| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (
                c.is_active, c.is_3d, c.is_2d, c.hdr, c.normal_prepass, c.depth_prepass,
                c.motion_prepass, c.atmosphere, c.atmosphere_env_light, c.env_map_light,
            )
                .hash(&mut h);
            (c.entity.to_bits(), h.finish() ^ hash_str(&c.name))
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| camera_row(c, f, &cams[i])),
    }
}

fn yesno(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

fn camera_row(commands: &mut Commands, fonts: &EmberFonts, c: &CameraEntry) -> Entity {
    let col = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            margin: UiRect::bottom(Val::Px(3.0)),
            ..default()
        })
        .id();
    let kind = if c.is_3d {
        "3D"
    } else if c.is_2d {
        "2D"
    } else {
        "?"
    };
    let head = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    let dot = icon_text(commands, &fonts.phosphor, "circle", SECONDARY, 10.0);
    let name = commands
        .spawn((Text::new(c.name.clone()), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary()))))
        .id();
    let meta = commands
        .spawn((
            Text::new(format!("({}, {:?})", kind, c.entity)),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    commands.entity(head).add_children(&[dot, name, meta]);

    let active_color = if c.is_active { rgb(OK) } else { rgb(text_muted()) };
    let line1 = commands
        .spawn((
            Text::new(format!(
                "{}  \u{b7}  target: {}  \u{b7}  hdr: {}",
                if c.is_active { "active" } else { "inactive" },
                c.render_target.label(),
                yesno(c.hdr),
            )),
            ui_font(&fonts.ui, 11.0),
            TextColor(active_color),
            Node {
                margin: UiRect::left(Val::Px(14.0)),
                ..default()
            },
        ))
        .id();
    let line2 = commands
        .spawn((
            Text::new(format!(
                "prepass: {}{}{}  \u{b7}  atmo: {}  \u{b7}  atmo-env: {}  \u{b7}  env-map: {}",
                if c.normal_prepass { "N" } else { "\u{b7}" },
                if c.depth_prepass { "D" } else { "\u{b7}" },
                if c.motion_prepass { "M" } else { "\u{b7}" },
                yesno(c.atmosphere),
                yesno(c.atmosphere_env_light),
                yesno(c.env_map_light),
            )),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node {
                margin: UiRect::left(Val::Px(14.0)),
                ..default()
            },
        ))
        .id();
    commands.entity(col).add_children(&[head, line1, line2]);
    col
}

fn warnings_snapshot(world: &World) -> KeyedSnapshot {
    let _ = world;
    let warnings = recent_warnings();
    if warnings.is_empty() {
        return note_snapshot("(no warnings or errors captured this session)");
    }
    // Newest first.
    let list: Vec<CapturedWarning> = warnings.into_iter().rev().collect();
    let items: Vec<(u64, u64)> = list
        .iter()
        .enumerate()
        .map(|(i, wn)| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (&wn.target, &wn.message).hash(&mut h);
            (i as u64, h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| warning_row(c, f, &list[i])),
    }
}

fn warning_row(commands: &mut Commands, fonts: &EmberFonts, wn: &CapturedWarning) -> Entity {
    let (icon, color) = match wn.level {
        WarningLevel::Warn => ("warning", (230, 180, 80)),
        WarningLevel::Error => ("warning-circle", BAD),
    };
    let age = wn.age();
    let age_label = if age.as_secs() < 60 {
        format!("{}s ago", age.as_secs())
    } else if age.as_secs() < 3600 {
        format!("{}m ago", age.as_secs() / 60)
    } else {
        format!("{}h ago", age.as_secs() / 3600)
    };

    let col = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            margin: UiRect::bottom(Val::Px(2.0)),
            ..default()
        })
        .id();
    let head = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            width: Val::Percent(100.0),
            column_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, color, 12.0);
    let target = commands
        .spawn((Text::new(wn.target.clone()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary()))))
        .id();
    let gap = commands
        .spawn(Node {
            flex_grow: 1.0,
            ..default()
        })
        .id();
    let age_e = commands
        .spawn((Text::new(age_label), ui_font(&fonts.mono, 10.0), TextColor(rgb(text_muted()))))
        .id();
    commands.entity(head).add_children(&[ic, target, gap, age_e]);
    let msg = commands
        .spawn((
            Text::new(wn.message.clone()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(SECONDARY)),
            Node {
                margin: UiRect::left(Val::Px(18.0)),
                ..default()
            },
        ))
        .id();
    commands.entity(col).add_children(&[head, msg]);
    col
}

fn note_snapshot(text: &'static str) -> KeyedSnapshot {
    KeyedSnapshot {
        items: vec![(u64::MAX, 0)],
        build: Box::new(move |c, f, _| note(c, f, text, text_muted())),
    }
}
