//! The keyed-list snapshots behind every pane, and the properties text the
//! right rail shows for whatever is selected.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_ember::font::{icon_text, ui_font};
use renzora_ember::font::EmberFonts;
use renzora_ember::reactive::tracked::{bind_bg, bind_text};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::{progress_indeterminate, HoverTooltip};

use crate::overlay::ImportOverlayState;
use crate::staged::{human_bytes, thousands};

use super::rows::{list_row, staged, RowSpec};
use super::tree::surviving;
use super::widgets::hover_cursor;
use super::{
    DiscardStagedBtn, ImportNav, ImportTab, MatRow, MeshRow, RemoveFileBtn, StagedRow, TreeItem,
    AMBER, GREEN, RED,
};

/// The staged models, so a multi-file import can be flipped through. Each row
/// carries its findings count, which is the thing worth comparing across a
/// batch — one bad file in twenty is easy to miss otherwise, and a trash to
/// throw that one away without touching the rest.
pub(super) fn staged_snapshot(world: &Rx) -> KeyedSnapshot {
    let Some(state) = world.get_resource::<ImportOverlayState>() else {
        return KeyedSnapshot {
            items: Vec::new(),
            build: Box::new(|_, _, _| Entity::PLACEHOLDER),
        };
    };
    let active = state.active;
    let rows: Vec<FileRow> = state
        .staged
        .iter()
        .enumerate()
        .map(|(i, st)| {
            let detail = match st.problems() {
                0 => format!("{} · ready to import", human_bytes(st.glb_bytes as u64)),
                1 => "1 finding · ready to import".to_string(),
                n => format!("{n} findings · ready to import"),
            };
            FileRow {
                label: st.file_name.clone(),
                detail,
                icon: "cube",
                icon_color: GREEN,
                selected: i == active,
                discard: Discard::Staged(i),
                phase: Phase::Ready,
            }
        })
        .collect();
    let items: Vec<(u64, u64)> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| (i as u64, hash_of((i, &r.label, &r.detail, r.selected))))
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let row = file_row(c, f, &rows[i]);
            c.entity(row).insert(StagedRow(i));
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
            // The name alone. A mesh list from a scanned building is forty
            // rows of forty-character names in a 310px column, and there is no
            // width left for a count that is the same `1 prims` on nearly all
            // of them. Both counts are in the properties rail for whichever
            // mesh is selected, which has the room for them.
            let detail = if dim {
                EXCLUDED_SUFFIX.trim_start_matches(" · ").to_string()
            } else {
                String::new()
            };
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
            // The name only. `Opaque · 2-sided` was the same two words on almost
            // every row — it distinguished nothing while overrunning the names
            // that do — and both are in the properties rail for whichever
            // material is selected. Only the exclusion note is worth a row's
            // width, because that one differs per row and has consequences.
            let detail = if dim { EXCLUDED_SUFFIX.trim_start_matches(" · ").to_string() } else { String::new() };
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
    let queued: Vec<QueuedAsset> = world
        .get_resource::<ImportOverlayState>()
        .map(|s| s.pending_files.clone())
        .unwrap_or_default();
    let converting = world
        .get_resource::<ImportOverlayState>()
        .and_then(|s| s.converting.clone());
    // The worker walks the queue in order, so the file it reports being on also
    // reports that everything before it is finished. Those rows are dropped
    // rather than drawn done: a converted model is already sitting in the
    // staged list above with its findings and its trash, and a copied file is
    // in the project. Showing it twice is what made a batch import look like it
    // had queued everything twice.
    let started = converting
        .as_deref()
        .and_then(|c| queued.iter().position(|q| q.path == c))
        .unwrap_or(0);
    let files: Vec<QueuedAsset> = queued.into_iter().skip(started).collect();
    let rows: Vec<FileRow> = files
        .iter()
        .map(|q| {
            let (icon, icon_color) = crate::kinds::kind_icon(&q.path);
            let phase = if converting.as_deref() == Some(q.path.as_path()) {
                Phase::Working
            } else {
                Phase::Waiting
            };
            FileRow {
                label: queued_label(q),
                detail: q
                    .path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_uppercase(),
                icon,
                icon_color,
                selected: false,
                discard: Discard::Queued(q.path.clone()),
                phase,
            }
        })
        .collect();
    // The key is the source path (stable as the queue shortens); the hash
    // carries the phase, so a row rebuilds the moment the worker reaches it.
    let items: Vec<(u64, u64)> = files
        .iter()
        .zip(&rows)
        .map(|(q, row)| {
            let mut key = std::collections::hash_map::DefaultHasher::new();
            (&q.path, &q.relative_dir).hash(&mut key);
            let mut content = std::collections::hash_map::DefaultHasher::new();
            (&q.path, &q.relative_dir, row.phase).hash(&mut content);
            (key.finish(), content.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| file_row(c, f, &rows[i])),
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

/// One row of the Files tab, whether it is still queued or already converted.
///
/// Both used to be built separately, and the queued one was cramped enough that
/// a long name wrapped out of its own 26px row — which is what made the list
/// look like a stack of half-drawn text. One builder, one 44px two-line shape,
/// and the name gets the whole width because the extension and the trash sit on
/// the second line under it.
pub(super) struct FileRow {
    pub(super) label: String,
    /// Muted second line: a size, a findings count, or the file's extension.
    pub(super) detail: String,
    pub(super) icon: &'static str,
    pub(super) icon_color: (u8, u8, u8),
    pub(super) selected: bool,
    pub(super) discard: Discard,
    pub(super) phase: Phase,
}

/// Where a row is in the pipeline, which is the whole point of the list: a
/// batch of twenty files is otherwise twenty identical rows with no way to tell
/// what has happened to any of them.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Phase {
    /// Queued behind something else. Drawn grey, and says so on hover.
    Waiting,
    /// The worker is on this one now. Carries a sweeping progress bar.
    Working,
    /// Converted and staged, waiting on the verdict.
    Ready,
}

/// What this row's trash throws away.
pub(super) enum Discard {
    /// A file that has not converted yet: drop it from the queue.
    Queued(std::path::PathBuf),
    /// A converted file waiting on a verdict: delete its staged tree.
    Staged(usize),
}

fn file_row(commands: &mut Commands, fonts: &EmberFonts, spec: &FileRow) -> Entity {
    // A file nothing has happened to yet is drawn back: its icon loses its kind
    // colour and its name drops to the muted text, so a batch mid-import reads
    // at a glance as "these are done, this one is going, those are waiting".
    let waiting = spec.phase == Phase::Waiting;
    let outer = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                // The progress bar is flush with the bottom edge, so the corner
                // radius has to cut it rather than the bar squaring the row off.
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(if spec.selected {
                rgb(accent()).with_alpha(0.16)
            } else {
                rgb(section_bg())
            }),
            BorderColor::all(if spec.selected {
                rgb(accent())
            } else {
                Color::NONE
            }),
            Interaction::default(),
            // A row is a click target (it picks which staged file is on show),
            // so the press must not also reach whatever is behind the window.
            FocusPolicy::Block,
            hover_cursor(),
        ))
        .id();
    if waiting {
        // Hovering a greyed row should say *why* it is grey rather than leaving
        // the user to work out that the list is ordered by progress. Inserted
        // straight onto the row, which already tracks `Interaction`; the
        // wrapper form of the tooltip widget would add a node between the row
        // and the list and break the column's spacing.
        commands
            .entity(outer)
            .insert(HoverTooltip::new("Waiting in queue"));
    }
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                min_height: Val::Px(42.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(9.0),
                padding: UiRect::axes(Val::Px(9.0), Val::Px(6.0)),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let icon_color = if waiting { text_muted() } else { spec.icon_color };
    let icon = icon_text(commands, &fonts.phosphor, spec.icon, icon_color, 16.0);
    commands.entity(icon).insert(FocusPolicy::Pass);

    // Name over detail. The name is given the row's spare width and allowed to
    // wrap to a second line rather than being clipped: which of two similarly
    // named downloads this is often lives in the part a clip would remove.
    let text_col = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let nm = commands
        .spawn((
            Text::new(spec.label.clone()),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(if waiting { text_muted() } else { text_primary() })),
            // Break anywhere, not just at word boundaries. A downloaded model is
            // called `free_1975_porsche_911_930_turbo.glb`, and word-boundary
            // wrapping finds no break in that at all — so the name ran straight
            // under the trash icon instead of wrapping above it.
            TextLayout {
                linebreak: bevy::text::LineBreak::AnyCharacter,
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let dt = commands
        .spawn((
            Text::new(spec.detail.clone()),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
            FocusPolicy::Pass,
        ))
        .id();
    // While the worker is on this file, the second line reports the phase it is
    // in rather than the extension. Bound rather than baked into the row,
    // because the phase changes several times a second during texture baking
    // and rebuilding the row that often would fight the keyed list.
    if spec.phase == Phase::Working {
        bind_text(commands, dt, |w| {
            match w.get_resource::<ImportOverlayState>().map(|s| s.progress.clone()) {
                Some(crate::overlay::ImportProgress::Working { label, .. }) if !label.is_empty() => {
                    label
                }
                _ => "Working…".to_string(),
            }
        });
    }
    commands.entity(text_col).add_children(&[nm, dt]);

    let rm = commands
        .spawn((
            Node {
                width: Val::Px(22.0),
                height: Val::Px(22.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            // Without blocking, deleting a row would also select it.
            FocusPolicy::Block,
            hover_cursor(),
        ))
        .id();
    match &spec.discard {
        Discard::Queued(path) => {
            commands.entity(rm).insert(RemoveFileBtn(path.clone()));
        }
        Discard::Staged(index) => {
            commands.entity(rm).insert(DiscardStagedBtn(*index));
        }
    }
    bind_bg(commands, rm, move |w| {
        if matches!(
            w.get::<Interaction>(rm),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        ) {
            rgb(RED).with_alpha(0.22)
        } else {
            Color::NONE
        }
    });
    let trash = icon_text(commands, &fonts.phosphor, "trash", text_muted(), 13.0);
    commands.entity(trash).insert(FocusPolicy::Pass);
    commands.entity(rm).add_child(trash);

    commands.entity(row).add_children(&[icon, text_col, rm]);
    commands.entity(outer).add_child(row);

    // A sweep, not a fraction. The worker's `[done/total]` counts *files* until
    // texture baking starts and then counts textures, and the other phases
    // (optimize, animation extraction, compaction) report no number at all — so
    // a determinate bar on one file would jump backwards and stall. What the
    // row has to say is "this is the one that is going", which a sweep says
    // without claiming to know how far along it is.
    if spec.phase == Phase::Working {
        let bar = progress_indeterminate(commands, 0.0, 3.0);
        commands.entity(bar).entry::<Node>().and_modify(|mut n| {
            n.width = Val::Percent(100.0);
        });
        commands.entity(bar).insert(FocusPolicy::Pass);
        commands.entity(outer).add_child(bar);
    }
    outer
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
