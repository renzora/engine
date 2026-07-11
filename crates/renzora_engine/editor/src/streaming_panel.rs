//! Streaming debug panel — live view of every streaming subsystem so you can
//! *watch* it work: the world-streaming gate + camera, in-flight scene
//! streams, streamed `SceneInstance` load/unload state with distances, terrain
//! chunk residency, mesh-LOD bands, and texture tier demotions.
//!
//! Same architecture as the Scene Diagnostics panel: an exclusive snapshot
//! system (queries need `&mut World`) refreshes a resource at 4 Hz while the
//! panel is the active tab, and the panel content is pure reactive bindings
//! reading that resource — hidden panel costs nothing.

use bevy::prelude::*;
use std::hash::{Hash, Hasher};

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::{bind_text, bind_text_color, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::collapsible;

use renzora_engine::mesh_lod::{LodApplied, LodProbed, PendingLodSpawn};
use renzora_engine::scene_io::{SceneLoadPhase, SceneLoadState};
use renzora_engine::scene_stream::SceneStreams;
use renzora_engine::texture_stream::TextureStreamingSettings;
use renzora_terrain::data::{TerrainChunkOf, TerrainChunkStreamedOut, TerrainData};

const SECONDARY: (u8, u8, u8) = (170, 170, 180);
const OK: (u8, u8, u8) = (120, 200, 120);
const ACTIVE: (u8, u8, u8) = (120, 180, 240);
const IDLE: (u8, u8, u8) = (150, 150, 158);
const WARN: (u8, u8, u8) = (230, 190, 110);

// ── Snapshot ─────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct InstanceRow {
    pub entity: Entity,
    pub name: String,
    pub distance: f32,
    pub load_radius: f32,
    pub unload_radius: f32,
    /// "loaded" / "loading…" / "unloaded".
    pub state: &'static str,
}

#[derive(Clone)]
pub struct TerrainRow {
    pub name: String,
    pub streaming: bool,
    pub radius: f32,
    pub resident: usize,
    pub streamed_out: usize,
}

#[derive(Clone)]
pub struct LodRow {
    pub name: String,
    pub distance: f32,
    /// e.g. "0‥40  1‥100  2‥∞".
    pub bands: String,
    /// Still waiting on variant GLBs to load.
    pub pending: bool,
}

/// Everything the panel shows, refreshed at 4 Hz while the panel is active.
#[derive(Resource, Default)]
pub struct StreamingDebugSnapshot {
    pub streaming_active: bool,
    pub camera_pos: Option<Vec3>,
    pub load_phase: String,
    pub load_progress: f32,
    pub load_path: String,
    pub streams: Vec<(String, String)>, // (path, stage)
    pub instances: Vec<InstanceRow>,
    pub terrains: Vec<TerrainRow>,
    pub lods: Vec<LodRow>,
    pub models_without_lods: usize,
    pub tex_enabled: bool,
    pub tex_full_distance: f32,
    pub tex_low_distance: f32,
    pub tex_tracked_materials: usize,
    pub tex_demoted_materials: usize,
    pub tex_demoted_sample: Vec<String>,
}

fn snap<R: Default>(w: &World, f: impl FnOnce(&StreamingDebugSnapshot) -> R) -> R {
    w.get_resource::<StreamingDebugSnapshot>()
        .map(f)
        .unwrap_or_default()
}

/// Exclusive refresh — every streaming subsystem queried in one pass.
pub fn update_streaming_debug_snapshot(world: &mut World) {
    let streaming_active = renzora::world_streaming_active(world);
    let camera_pos = renzora::streaming_camera_pos(world);
    let cam = camera_pos.unwrap_or(Vec3::ZERO);

    let (load_phase, load_progress, load_path) = world
        .get_resource::<SceneLoadState>()
        .map(|s| {
            (
                match s.phase {
                    SceneLoadPhase::Idle => "idle".to_string(),
                    SceneLoadPhase::Loading => "loading".to_string(),
                    SceneLoadPhase::Ready => "ready".to_string(),
                    SceneLoadPhase::Failed => "FAILED".to_string(),
                },
                s.progress,
                s.current_path.clone().unwrap_or_default(),
            )
        })
        .unwrap_or_else(|| ("-".into(), 0.0, String::new()));

    let streams: Vec<(String, String)> = world
        .get_resource::<SceneStreams>()
        .map(|s| {
            s.summaries()
                .into_iter()
                .map(|s| (s.path, s.stage))
                .collect()
        })
        .unwrap_or_default();

    // Streamed scene instances.
    let mut instances: Vec<InstanceRow> = Vec::new();
    {
        let candidates: Vec<Entity> = {
            let mut q = world.query_filtered::<Entity, With<renzora::SceneInstance>>();
            q.iter(world).collect()
        };
        let in_flight: Vec<Entity> = world
            .get_resource::<SceneStreams>()
            .map(|streams| {
                candidates
                    .into_iter()
                    .filter(|&e| streams.has_stream_under(e))
                    .collect()
            })
            .unwrap_or_default();
        let mut q = world.query::<(
            Entity,
            &renzora::SceneInstance,
            Option<&Name>,
            Option<&GlobalTransform>,
            Option<&Children>,
        )>();
        for (entity, inst, name, transform, children) in q.iter(world) {
            if !inst.streamed {
                continue;
            }
            let expanded = children.is_some_and(|c| c.iter().count() > 0);
            let state = if expanded {
                "loaded"
            } else if in_flight.contains(&entity) {
                "loading…"
            } else {
                "unloaded"
            };
            instances.push(InstanceRow {
                entity,
                name: name.map(|n| n.to_string()).unwrap_or_else(|| "?".into()),
                distance: transform
                    .map(|t| cam.distance(t.translation()))
                    .unwrap_or(f32::NAN),
                load_radius: inst.load_radius,
                unload_radius: inst.unload_radius,
                state,
            });
        }
        instances.sort_by(|a, b| a.distance.total_cmp(&b.distance));
    }

    // Terrain chunk residency.
    let mut terrains: Vec<TerrainRow> = Vec::new();
    {
        let mut counts: std::collections::HashMap<Entity, (usize, usize)> = Default::default();
        {
            let mut q = world.query::<(&TerrainChunkOf, Has<TerrainChunkStreamedOut>)>();
            for (chunk_of, out) in q.iter(world) {
                let entry = counts.entry(chunk_of.0).or_default();
                if out {
                    entry.1 += 1;
                } else {
                    entry.0 += 1;
                }
            }
        }
        let mut q = world.query::<(Entity, &TerrainData, Option<&Name>)>();
        for (entity, terrain, name) in q.iter(world) {
            let (resident, streamed_out) = counts.get(&entity).copied().unwrap_or((0, 0));
            terrains.push(TerrainRow {
                name: name.map(|n| n.to_string()).unwrap_or_else(|| "Terrain".into()),
                streaming: terrain.stream_chunks,
                radius: terrain.stream_radius,
                resident,
                streamed_out,
            });
        }
    }

    // Mesh LODs.
    let mut lods: Vec<LodRow> = Vec::new();
    let mut models_without_lods = 0usize;
    {
        // Model roots only (MeshInstanceData carriers).
        let mut q = world.query::<(
            &renzora::MeshInstanceData,
            Option<&Name>,
            Option<&GlobalTransform>,
            Option<&LodApplied>,
            Has<PendingLodSpawn>,
            Has<LodProbed>,
        )>();
        for (_, name, transform, applied, pending, probed) in q.iter(world) {
            match applied {
                Some(applied) => {
                    let bands = applied
                        .bands()
                        .iter()
                        .map(|(level, _, end)| {
                            if end.is_finite() {
                                format!("L{level}‥{end:.0}m")
                            } else {
                                format!("L{level}‥∞")
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("  ");
                    lods.push(LodRow {
                        name: name.map(|n| n.to_string()).unwrap_or_else(|| "?".into()),
                        distance: transform
                            .map(|t| cam.distance(t.translation()))
                            .unwrap_or(f32::NAN),
                        bands,
                        pending: false,
                    });
                }
                None if pending => lods.push(LodRow {
                    name: name.map(|n| n.to_string()).unwrap_or_else(|| "?".into()),
                    distance: transform
                        .map(|t| cam.distance(t.translation()))
                        .unwrap_or(f32::NAN),
                    bands: String::new(),
                    pending: true,
                }),
                None if probed => models_without_lods += 1,
                None => {}
            }
        }
        lods.sort_by(|a, b| a.distance.total_cmp(&b.distance));
    }

    // Texture tiers: count materials with any .rmip slot, and how many are
    // currently demoted (holding a `#low` handle).
    let (tex_tracked, tex_demoted, demoted_sample) = {
        let asset_server = world.resource::<AssetServer>().clone();
        let mut tracked = 0usize;
        let mut demoted = 0usize;
        let mut sample: Vec<String> = Vec::new();
        if let Some(materials) = world.get_resource::<Assets<StandardMaterial>>() {
            for (_, material) in materials.iter() {
                let slots = [
                    material.base_color_texture.as_ref(),
                    material.normal_map_texture.as_ref(),
                    material.metallic_roughness_texture.as_ref(),
                    material.occlusion_texture.as_ref(),
                    material.emissive_texture.as_ref(),
                ];
                let mut has_rmip = false;
                let mut has_low = false;
                for handle in slots.into_iter().flatten() {
                    if let Some(path) = asset_server.get_path(handle.id()) {
                        let p = path.path().to_string_lossy();
                        if p.ends_with(".rmip") {
                            has_rmip = true;
                            if path.label() == Some("low") {
                                has_low = true;
                                if sample.len() < 12 {
                                    let s = p.to_string();
                                    if !sample.contains(&s) {
                                        sample.push(s);
                                    }
                                }
                            }
                        }
                    }
                }
                if has_rmip {
                    tracked += 1;
                }
                if has_low {
                    demoted += 1;
                }
            }
        }
        (tracked, demoted, sample)
    };
    let (tex_enabled, tex_full_distance, tex_low_distance) = world
        .get_resource::<TextureStreamingSettings>()
        .map(|s| (s.enabled, s.full_distance, s.low_distance))
        .unwrap_or((false, 0.0, 0.0));

    world.insert_resource(StreamingDebugSnapshot {
        streaming_active,
        camera_pos,
        load_phase,
        load_progress,
        load_path,
        streams,
        instances,
        terrains,
        lods,
        models_without_lods,
        tex_enabled,
        tex_full_distance,
        tex_low_distance,
        tex_tracked_materials: tex_tracked,
        tex_demoted_materials: tex_demoted,
        tex_demoted_sample: demoted_sample,
    });
}

// ── UI helpers (mirror native_diagnostics' vocabulary) ───────────────────────

fn hash_str(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

fn stat_row<V, C>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    value: V,
    color: C,
) -> Entity
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

fn neutral_row<V>(commands: &mut Commands, fonts: &EmberFonts, label: &str, value: V) -> Entity
where
    V: Fn(&World) -> String + Send + Sync + 'static,
{
    stat_row(commands, fonts, label, value, |_| rgb(text_primary()))
}

fn note(commands: &mut Commands, fonts: &EmberFonts, text: &str, color: (u8, u8, u8)) -> Entity {
    commands
        .spawn((Text::new(text), ui_font(&fonts.ui, 11.0), TextColor(rgb(color))))
        .id()
}

fn note_snapshot(text: &'static str) -> KeyedSnapshot {
    KeyedSnapshot {
        items: vec![(u64::MAX, 0)],
        build: Box::new(move |c, f, _| note(c, f, text, IDLE)),
    }
}

fn list_line(commands: &mut Commands, fonts: &EmberFonts, text: String, color: (u8, u8, u8)) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.mono, 11.0),
            TextColor(rgb(color)),
            Node {
                margin: UiRect::left(Val::Px(6.0)),
                ..default()
            },
        ))
        .id()
}

// ── Panel content ────────────────────────────────────────────────────────────

pub fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
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

    // ── Status ──
    let (s, s_body) = collapsible(commands, fonts, Some("broadcast"), "Status", true);
    let rows = [
        stat_row(
            commands,
            fonts,
            "World streaming",
            |w| {
                if snap(w, |s| s.streaming_active) {
                    "ACTIVE".into()
                } else {
                    "inactive (edit mode)".into()
                }
            },
            |w| {
                if snap(w, |s| s.streaming_active) {
                    rgb(OK)
                } else {
                    rgb(IDLE)
                }
            },
        ),
        neutral_row(commands, fonts, "Streaming camera", |w| {
            snap(w, |s| s.camera_pos)
                .map(|p| format!("{:.1}, {:.1}, {:.1}", p.x, p.y, p.z))
                .unwrap_or_else(|| "none".into())
        }),
    ];
    commands.entity(s_body).add_children(&rows);
    let hint = note(
        commands,
        fonts,
        "Streaming runs in Play / Simulate and in exported games. Edit mode keeps everything resident.",
        IDLE,
    );
    commands.entity(s_body).add_child(hint);

    // ── Scene streams ──
    let (l, l_body) = collapsible(commands, fonts, Some("download"), "Scene streams", true);
    let rows = [
        stat_row(
            commands,
            fonts,
            "Load phase",
            |w| snap(w, |s| s.load_phase.clone()),
            |w| match snap(w, |s| s.load_phase.clone()).as_str() {
                "loading" => rgb(ACTIVE),
                "FAILED" => rgb((230, 110, 110)),
                _ => rgb(text_primary()),
            },
        ),
        neutral_row(commands, fonts, "Progress", |w| {
            format!("{:.0}%", snap(w, |s| s.load_progress) * 100.0)
        }),
        neutral_row(commands, fonts, "Scene", |w| {
            let p = snap(w, |s| s.load_path.clone());
            if p.is_empty() {
                "-".into()
            } else {
                short_path(&p)
            }
        }),
    ];
    commands.entity(l_body).add_children(&rows);
    let streams_list = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            ..default()
        })
        .id();
    keyed_list(commands, streams_list, streams_snapshot);
    commands.entity(l_body).add_child(streams_list);

    // ── Streamed instances ──
    let (i, i_body) = collapsible(commands, fonts, Some("map-pin"), "Streamed scene instances", true);
    keyed_list(commands, i_body, instances_snapshot);

    // ── Terrain ──
    let (t, t_body) = collapsible(commands, fonts, Some("mountains"), "Terrain chunks", true);
    keyed_list(commands, t_body, terrains_snapshot);

    // ── Mesh LODs ──
    let (m, m_body) = collapsible(commands, fonts, Some("stack"), "Mesh LODs", true);
    let no_lods = neutral_row(commands, fonts, "Models without LOD files", |w| {
        snap(w, |s| s.models_without_lods).to_string()
    });
    commands.entity(m_body).add_child(no_lods);
    let lod_list = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            ..default()
        })
        .id();
    keyed_list(commands, lod_list, lods_snapshot);
    commands.entity(m_body).add_child(lod_list);

    // ── Textures ──
    let (x, x_body) = collapsible(commands, fonts, Some("image"), "Texture tiers", true);
    let rows = [
        stat_row(
            commands,
            fonts,
            "Texture streaming",
            |w| {
                if snap(w, |s| s.tex_enabled) {
                    "enabled".into()
                } else {
                    "disabled".into()
                }
            },
            |w| {
                if snap(w, |s| s.tex_enabled) {
                    rgb(OK)
                } else {
                    rgb(IDLE)
                }
            },
        ),
        neutral_row(commands, fonts, "Full / low thresholds", |w| {
            format!(
                "{:.0}m / {:.0}m",
                snap(w, |s| s.tex_full_distance),
                snap(w, |s| s.tex_low_distance)
            )
        }),
        neutral_row(commands, fonts, "Materials with .rmip textures", |w| {
            snap(w, |s| s.tex_tracked_materials).to_string()
        }),
        stat_row(
            commands,
            fonts,
            "Demoted to #low",
            |w| snap(w, |s| s.tex_demoted_materials).to_string(),
            |w| {
                if snap(w, |s| s.tex_demoted_materials) > 0 {
                    rgb(ACTIVE)
                } else {
                    rgb(text_primary())
                }
            },
        ),
    ];
    commands.entity(x_body).add_children(&rows);
    let demoted_list = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            ..default()
        })
        .id();
    keyed_list(commands, demoted_list, demoted_snapshot);
    commands.entity(x_body).add_child(demoted_list);

    commands.entity(root).add_children(&[s, l, i, t, m, x]);
    root
}

// ── List snapshots ───────────────────────────────────────────────────────────

fn streams_snapshot(world: &World) -> KeyedSnapshot {
    let streams = snap(world, |s| s.streams.clone());
    if streams.is_empty() {
        return note_snapshot("(no streams in flight)");
    }
    let items: Vec<(u64, u64)> = streams
        .iter()
        .map(|(p, stage)| (hash_str(p), hash_str(stage)))
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (path, stage) = &streams[i];
            list_line(c, f, format!("{} — {}", short_path(path), stage), ACTIVE)
        }),
    }
}

fn instances_snapshot(world: &World) -> KeyedSnapshot {
    let rows = snap(world, |s| s.instances.clone());
    if rows.is_empty() {
        return note_snapshot("(no streamed instances — tick \u{201c}Streamed\u{201d} on a Scene Instance)");
    }
    let items: Vec<(u64, u64)> = rows
        .iter()
        .map(|r| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (r.entity, r.state, (r.distance * 10.0) as i64).hash(&mut h);
            (r.entity.to_bits(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let r = &rows[i];
            let color = match r.state {
                "loaded" => OK,
                "loading…" => ACTIVE,
                _ => IDLE,
            };
            list_line(
                c,
                f,
                format!(
                    "{}  {:>6.1}m  [{}]  load {:.0} / unload {:.0}",
                    r.name, r.distance, r.state, r.load_radius, r.unload_radius
                ),
                color,
            )
        }),
    }
}

fn terrains_snapshot(world: &World) -> KeyedSnapshot {
    let rows = snap(world, |s| s.terrains.clone());
    if rows.is_empty() {
        return note_snapshot("(no terrain in scene)");
    }
    let items: Vec<(u64, u64)> = rows
        .iter()
        .map(|r| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (r.resident, r.streamed_out, r.streaming).hash(&mut h);
            (hash_str(&r.name), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let r = &rows[i];
            let (text, color) = if r.streaming {
                (
                    format!(
                        "{}  resident {} / streamed-out {}  (radius {:.0}m)",
                        r.name, r.resident, r.streamed_out, r.radius
                    ),
                    if r.streamed_out > 0 { ACTIVE } else { OK },
                )
            } else {
                (
                    format!("{}  streaming off  ({} chunks resident)", r.name, r.resident),
                    IDLE,
                )
            };
            list_line(c, f, text, color)
        }),
    }
}

fn lods_snapshot(world: &World) -> KeyedSnapshot {
    let rows = snap(world, |s| s.lods.clone());
    if rows.is_empty() {
        return note_snapshot("(no models with _lodN.glb variants)");
    }
    let items: Vec<(u64, u64)> = rows
        .iter()
        .map(|r| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            ((r.distance * 10.0) as i64, r.pending).hash(&mut h);
            (hash_str(&r.name), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let r = &rows[i];
            if r.pending {
                list_line(c, f, format!("{}  loading LOD variants…", r.name), WARN)
            } else {
                list_line(
                    c,
                    f,
                    format!("{}  {:>6.1}m  {}", r.name, r.distance, r.bands),
                    OK,
                )
            }
        }),
    }
}

fn demoted_snapshot(world: &World) -> KeyedSnapshot {
    let paths = snap(world, |s| s.tex_demoted_sample.clone());
    if paths.is_empty() {
        return note_snapshot("(no textures demoted right now)");
    }
    let items: Vec<(u64, u64)> = paths.iter().map(|p| (hash_str(p), 0)).collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            list_line(c, f, format!("\u{2193} {}#low", short_path(&paths[i])), ACTIVE)
        }),
    }
}

fn short_path(p: &str) -> String {
    std::path::Path::new(p)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| p.to_string())
}
