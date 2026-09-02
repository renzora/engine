//! The keyed-list snapshots behind every pane, and the properties text the
//! right rail shows for whatever is selected.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_ember::font::{icon_text, ui_font};
use renzora_ember::font::EmberFonts;
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;

use crate::overlay::ImportOverlayState;
use crate::staged::{human_bytes, thousands};

use super::rows::{list_row, staged, RowSpec};
use super::tree::surviving;
use super::widgets::hover_cursor;
use super::{ImportNav, ImportTab, MatRow, MeshRow, RemoveFileBtn, StagedRow, TreeItem, AMBER, GREEN, RED};

/// The staged models, so a multi-file import can be flipped through. Each row
/// carries its findings count, which is the thing worth comparing across a
/// batch — one bad file in twenty is easy to miss otherwise.
pub(super) fn staged_snapshot(world: &Rx) -> KeyedSnapshot {
    let Some(state) = world.get_resource::<ImportOverlayState>() else {
        return KeyedSnapshot {
            items: Vec::new(),
            build: Box::new(|_, _, _| Entity::PLACEHOLDER),
        };
    };
    let active = state.active;
    let rows: Vec<(usize, String, String, bool)> = state
        .staged
        .iter()
        .enumerate()
        .map(|(i, st)| {
            let detail = match st.problems() {
                0 => human_bytes(st.glb_bytes as u64),
                n => format!("{n} to look at"),
            };
            (i, st.file_name.clone(), detail, i == active)
        })
        .collect();
    let items: Vec<(u64, u64)> = rows
        .iter()
        .map(|r| (r.0 as u64, hash_of((r.0, &r.1, &r.2, r.3))))
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (idx, name, detail, selected) = &rows[i];
            let row = list_row(
                c,
                f,
                RowSpec {
                    selected: *selected,
                    ..RowSpec::plain(name, detail, "cube")
                },
            );
            c.entity(row).insert(StagedRow(*idx));
            row
        }),
    }
}

pub(super) fn meshes_snapshot(world: &Rx) -> KeyedSnapshot {
    let Some(st) = staged(world) else {
        return KeyedSnapshot { items: Vec::new(), build: Box::new(|_, _, _| Entity::PLACEHOLDER) };
    };
    let Some(stats) = st.stats.clone() else {
        return KeyedSnapshot { items: Vec::new(), build: Box::new(|_, _, _| Entity::PLACEHOLDER) };
    };
    let selected = world.get_resource::<ImportNav>().and_then(|n| n.sel_mesh);
    let (live_meshes, _) = surviving(&stats, &st.excluded);
    let rows: Vec<(usize, String, String, bool, bool)> = stats
        .mesh_list
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let dim = !live_meshes.contains(&i);
            let detail = format!(
                "{} prims · {} tris{}",
                m.primitives.len(),
                thousands(m.triangles()),
                if dim { EXCLUDED_SUFFIX } else { "" }
            );
            (i, m.name.clone(), detail, selected == Some(i), dim)
        })
        .collect();
    let items: Vec<(u64, u64)> = rows
        .iter()
        .map(|r| (r.0 as u64, hash_of((r.0, &r.1, &r.2, r.3, r.4))))
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (idx, name, detail, selected, dim) = &rows[i];
            let row = list_row(
                c,
                f,
                RowSpec {
                    selected: *selected,
                    dim: *dim,
                    ..RowSpec::plain(name, detail, "polygon")
                },
            );
            c.entity(row).insert(MeshRow(*idx));
            row
        }),
    }
}

/// What a mesh or material row says when the scene tree has left it with
/// nothing referencing it.
const EXCLUDED_SUFFIX: &str = " · not imported";

pub(super) fn materials_snapshot(world: &Rx) -> KeyedSnapshot {
    let Some(st) = staged(world) else {
        return KeyedSnapshot { items: Vec::new(), build: Box::new(|_, _, _| Entity::PLACEHOLDER) };
    };
    let selected = world.get_resource::<ImportNav>().and_then(|n| n.sel_material);
    // A material with nothing left using it is one the commit will drop, along
    // with its `.material` file and any texture only it read.
    let live_materials = st
        .stats
        .as_ref()
        .map(|stats| surviving(stats, &st.excluded).1);
    let rows: Vec<(usize, String, String, bool, bool)> = st
        .materials
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let dim = live_materials.as_ref().is_some_and(|live| !live.contains(&i));
            let detail = format!(
                "{}{}{}",
                m.alpha_mode,
                if m.double_sided { " · 2-sided" } else { "" },
                if dim { EXCLUDED_SUFFIX } else { "" }
            );
            (i, m.name.clone(), detail, selected == Some(i), dim)
        })
        .collect();
    let items: Vec<(u64, u64)> = rows
        .iter()
        .map(|r| (r.0 as u64, hash_of((r.0, &r.1, &r.2, r.3, r.4))))
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (idx, name, detail, selected, dim) = &rows[i];
            let row = list_row(
                c,
                f,
                RowSpec {
                    selected: *selected,
                    dim: *dim,
                    ..RowSpec::plain(name, detail, "circle-half-tilt")
                },
            );
            c.entity(row).insert(MatRow(*idx));
            row
        }),
    }
}

pub(super) fn findings_snapshot(world: &Rx) -> KeyedSnapshot {
    let rows: Vec<(bool, String)> = staged(world)
        .map(|s| {
            s.flags
                .iter()
                .map(|f| (f.level == crate::staged::FlagLevel::Problem, f.text.clone()))
                .collect()
        })
        .unwrap_or_default();
    let items: Vec<(u64, u64)> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| (i as u64, hash_of((i, r.0, &r.1))))
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (problem, text) = &rows[i];
            let row = c
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(5.0),
                        ..default()
                    },
                    FocusPolicy::Pass,
                ))
                .id();
            let colour = if *problem { AMBER } else { text_muted() };
            let ic = icon_text(
                c,
                &f.phosphor,
                if *problem { "warning" } else { "info" },
                colour,
                11.0,
            );
            c.entity(ic).insert(FocusPolicy::Pass);
            let tx = c
                .spawn((
                    Text::new(text.clone()),
                    ui_font(&f.ui, 10.5),
                    TextColor(rgb(if *problem { text_primary() } else { text_muted() })),
                    Node { flex_grow: 1.0, ..default() },
                    FocusPolicy::Pass,
                ))
                .id();
            c.entity(row).add_children(&[ic, tx]);
            row
        }),
    }
}

/// The right rail's properties block: whatever the active tab has selected,
/// falling back to a summary of the whole import when nothing is.
pub(super) fn selection_properties(w: &Rx) -> String {
    let Some(st) = staged(w) else {
        return String::new();
    };
    let nav = w.get_resource::<ImportNav>();
    let stats = st.stats.as_ref();

    match nav.map(|n| n.tab) {
        Some(ImportTab::Scene) => {
            if let (Some(stats), Some(item)) = (stats, nav.and_then(|n| n.sel_item)) {
                match item {
                    TreeItem::Node(idx) => {
                        if let Some(node) = stats.node_list.get(idx) {
                            let mesh = node
                                .mesh
                                .and_then(|m| stats.mesh_list.get(m))
                                .map(|m| format!("mesh        {}", m.name))
                                .unwrap_or_else(|| "mesh        (none)".to_string());
                            return format!(
                                "node        {}\nchildren    {}\ntransform   {}\n{}",
                                node.name,
                                node.children.len(),
                                if node.has_transform { "yes" } else { "identity" },
                                mesh
                            );
                        }
                    }
                    TreeItem::Mesh(mi) => {
                        if let Some(m) = stats.mesh_list.get(mi) {
                            return format!(
                                "mesh        {}\nsurfaces    {}\ntriangles   {}\nvertices    {}",
                                m.name,
                                m.primitives.len(),
                                thousands(m.triangles()),
                                thousands(m.vertices())
                            );
                        }
                    }
                    TreeItem::Prim(mi, k) => {
                        if let Some(p) = stats.mesh_list.get(mi).and_then(|m| m.primitives.get(k)) {
                            let mat = p
                                .material
                                .and_then(|x| stats.material_names.get(x))
                                .cloned()
                                .unwrap_or_else(|| "(none)".into());
                            return format!(
                                "surface     {}\nmaterial    {}\ntriangles   {}\nvertices    {}\nattributes  {}",
                                k,
                                mat,
                                thousands(p.triangles),
                                thousands(p.vertices),
                                p.attributes.join(" ")
                            );
                        }
                    }
                }
            }
        }
        Some(ImportTab::Meshes) => {
            if let (Some(stats), Some(idx)) = (stats, nav.and_then(|n| n.sel_mesh)) {
                if let Some(m) = stats.mesh_list.get(idx) {
                    let mut out = format!(
                        "name        {}\nprimitives  {}\ntriangles   {}\nvertices    {}\n",
                        m.name,
                        m.primitives.len(),
                        thousands(m.triangles()),
                        thousands(m.vertices())
                    );
                    for (i, p) in m.primitives.iter().take(8).enumerate() {
                        let mat = p
                            .material
                            .and_then(|mi| stats.material_names.get(mi))
                            .cloned()
                            .unwrap_or_else(|| "(none)".into());
                        out.push_str(&format!(
                            "\n  [{}] {}\n      {} tris · {}",
                            i,
                            mat,
                            thousands(p.triangles),
                            p.attributes.join(" ")
                        ));
                    }
                    if m.primitives.len() > 8 {
                        out.push_str(&format!("\n  … {} more", m.primitives.len() - 8));
                    }
                    return out;
                }
            }
        }
        Some(ImportTab::Materials) => {
            if let Some(idx) = nav.and_then(|n| n.sel_material) {
                if let Some(m) = st.materials.get(idx) {
                    return format!(
                        "name        {}\nalpha       {}\ntwo-sided   {}\nmetallic    {:.3}\nroughness   {:.3}\nbase color  {:.2} {:.2} {:.2} {:.2}\ntextures    {}",
                        m.name,
                        m.alpha_mode,
                        if m.double_sided { "yes" } else { "no" },
                        m.metallic,
                        m.roughness,
                        m.base_color[0],
                        m.base_color[1],
                        m.base_color[2],
                        m.base_color[3],
                        if m.slots.is_empty() {
                            "none".to_string()
                        } else {
                            m.slots.join(", ")
                        }
                    );
                }
            }
        }
        _ => {}
    }

    // Nothing selected — describe the import as a whole, leading with where it
    // came from. Without the full path it is genuinely hard to tell two files
    // with the same stem apart, and a wrong pick reads as a broken importer.
    let source = st.source.display().to_string();
    let Some(s) = stats else {
        return format!("source
  {source}

No structure could be read from the converted model.");
    };
    format!(
        "source
  {source}

{}",
        format_args!(
        "nodes       {}\nmeshes      {}  ({} prims)\ntriangles   {}\nvertices    {}\nmaterials   {}\ntextures    {}  ({})\nanimations  {}\nskins       {}\nattributes  {}\nGLB         {}",
        thousands(s.nodes),
        thousands(s.meshes),
        thousands(s.primitives),
        thousands(s.triangles),
        thousands(s.vertices),
        thousands(s.materials),
        thousands(st.textures.len()),
        human_bytes(st.texture_bytes),
        thousands(st.animations.len()),
        thousands(s.skins),
        if s.attributes.is_empty() {
            "none".to_string()
        } else {
            s.attributes.join(" ")
        },
        human_bytes(st.glb_bytes as u64),
        )
    )
}

pub(super) fn hash_of<T: std::hash::Hash>(v: T) -> u64 {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}

// ── The file queue, and the per-file results log ─────────────────────────────

pub(super) fn files_snapshot(world: &Rx) -> KeyedSnapshot {
    use std::hash::{Hash, Hasher};
    use crate::kinds::QueuedAsset;
    let files: Vec<QueuedAsset> = world
        .get_resource::<ImportOverlayState>()
        .map(|s| s.pending_files.clone())
        .unwrap_or_default();
    let items: Vec<(u64, u64)> = files
        .iter()
        .map(|q| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (&q.path, &q.relative_dir).hash(&mut h);
            (h.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| file_row(c, f, &files[i])),
    }
}

/// Row label for a queued asset: the bare filename for a flat pick, or the
/// mirrored `sub/dir/file.png` path for a folder import.
///
/// Deep pack paths are elided in the middle (`Pack/…/textures/a.png`). The row
/// is a fixed 26px, so an un-elided path wraps out of it — and the filename is
/// the half worth keeping, which a plain right-clip would be the half to lose.
fn queued_label(asset: &crate::kinds::QueuedAsset) -> String {
    let file = asset.path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
    if asset.relative_dir.is_empty() {
        return file.to_string();
    }
    const MAX: usize = 52;
    let full = format!("{}/{}", asset.relative_dir, file);
    if full.chars().count() <= MAX {
        return full;
    }
    // Keep the root folder (which pack this is) and the tail (where in it).
    let segs: Vec<&str> = asset.relative_dir.split('/').collect();
    let root = segs.first().copied().unwrap_or("");
    let tail = segs.last().copied().unwrap_or("");
    if segs.len() > 2 {
        format!("{}/…/{}/{}", root, tail, file)
    } else {
        format!("{}/…/{}", root, file)
    }
}

fn file_row(commands: &mut Commands, fonts: &EmberFonts, asset: &crate::kinds::QueuedAsset) -> Entity {
    let path = &asset.path;
    let name = queued_label(asset);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_uppercase();
    let row = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(26.0), flex_shrink: 0.0, flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(7.0), padding: UiRect::axes(Val::Px(7.0), Val::Px(0.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(rgb(section_bg())),
            FocusPolicy::Pass,
        ))
        .id();
    let (glyph, color) = crate::kinds::kind_icon(path);
    let icon = icon_text(commands, &fonts.phosphor, glyph, color, 12.0);
    commands.entity(icon).insert(FocusPolicy::Pass);
    let nm = commands.spawn((Text::new(name), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), FocusPolicy::Pass, Node { flex_grow: 1.0, ..default() })).id();
    let ex = commands.spawn((Text::new(ext), ui_font(&fonts.ui, 9.0), TextColor(rgb(text_muted())), FocusPolicy::Pass)).id();
    let rm = commands.spawn((Node { padding: UiRect::all(Val::Px(2.0)), ..default() }, Interaction::default(), RemoveFileBtn(path.to_path_buf()), hover_cursor())).id();
    let rmx = icon_text(commands, &fonts.phosphor, "x", text_muted(), 11.0);
    commands.entity(rmx).insert(FocusPolicy::Pass);
    commands.entity(rm).add_child(rmx);
    commands.entity(row).add_children(&[icon, nm, ex, rm]);
    row
}

pub(super) fn log_snapshot(world: &Rx) -> KeyedSnapshot {
    use std::hash::{Hash, Hasher};
    let entries: Vec<(String, bool, String)> = world.get_resource::<ImportOverlayState>().map(|s| s.log_entries.iter().map(|e| (e.file_name.clone(), e.success, e.message.clone())).collect()).unwrap_or_default();
    let items: Vec<(u64, u64)> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (i, &e.0, e.1, &e.2).hash(&mut h);
            (i as u64, h.finish())
        })
        .collect();
    KeyedSnapshot { items, build: Box::new(move |c, f, i| log_row(c, f, &entries[i])) }
}

fn log_row(commands: &mut Commands, fonts: &EmberFonts, e: &(String, bool, String)) -> Entity {
    let (name, ok, msg) = e;
    let row = commands.spawn((Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }, FocusPolicy::Pass)).id();
    let icon = icon_text(commands, &fonts.phosphor, if *ok { "check-circle" } else { "warning" }, if *ok { GREEN } else { RED }, 11.0);
    commands.entity(icon).insert(FocusPolicy::Pass);
    let nm = commands.spawn((Text::new(name.clone()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), FocusPolicy::Pass)).id();
    let mc = if *ok { text_muted() } else { RED };
    let mg = commands.spawn((Text::new(msg.clone()), ui_font(&fonts.ui, 11.0), TextColor(rgb(mc)), FocusPolicy::Pass, Node { flex_grow: 1.0, ..default() })).id();
    commands.entity(row).add_children(&[icon, nm, mg]);
    row
}
