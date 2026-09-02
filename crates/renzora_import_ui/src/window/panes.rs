//! The three regions inside the frame: the left list pane, the centre viewport
//! and the right properties rail — plus the destination folder picker the
//! Destination tab shows.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_2way, bind_bg, bind_display, bind_text, bind_with, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::{drag_value, dropdown, radio_group, scroll_view};

use renzora_import::settings::{SceneStructure, UpAxis};

use crate::overlay::{ImportLayout, ImportOverlayState, ImportProgress};

use super::lifecycle::Init;
use super::lists::{
    files_snapshot, findings_snapshot, log_snapshot, materials_snapshot, meshes_snapshot,
    selection_properties, staged_snapshot,
};
use super::rows::{active_tab, has_staged, showing_material, staged};
use super::tree::scene_snapshot;
use super::widgets::{field_row, g_settings, hover_cursor, s_settings, toggle_row};
use super::{
    DestFolderRow, FileBrowseBtn, FilesContainer, FolderBrowseBtn, ImportColumns, ImportTab,
    LogContainer, Side,
};

// ── Left pane ────────────────────────────────────────────────────────────────

pub(super) fn build_left_pane(
    commands: &mut Commands,
    fonts: &EmberFonts,
    init: &Init,
    has_project: bool,
) -> Entity {
    let col = commands
        .spawn((
            Node {
                width: Val::Px(310.0),
                flex_shrink: 0.0,
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(8.0),
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
        ))
        .id();
    bind_column_width(commands, col, Side::Left);

    // Files — drop zone + queue.
    let files = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    bind_display(commands, files, |w| active_tab(w) == ImportTab::Files);
    let browse_row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let b1 = super::widgets::pill_button(commands, fonts, "file", "Files");
    commands.entity(b1).insert(FileBrowseBtn);
    let b2 = super::widgets::pill_button(commands, fonts, "folder-open", "Folder");
    commands.entity(b2).insert(FolderBrowseBtn);
    commands.entity(browse_row).add_children(&[b1, b2]);
    let list = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                ..default()
            },
            FilesContainer,
        ))
        .id();
    keyed_list(commands, list, files_snapshot);
    let staged_list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            ..default()
        })
        .id();
    keyed_list(commands, staged_list, staged_snapshot);
    let stack = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            ..default()
        })
        .id();
    commands.entity(stack).add_children(&[staged_list, list]);
    let files_scroll = scroll_view(commands, stack);
    commands
        .entity(files)
        .add_children(&[browse_row, files_scroll]);

    // Scene — flattened tree with expand state.
    let scene = list_pane(commands, ImportTab::Scene, scene_snapshot);
    let meshes = list_pane(commands, ImportTab::Meshes, meshes_snapshot);
    let materials = list_pane(commands, ImportTab::Materials, materials_snapshot);

    // Destination — where a committed import lands.
    let dest = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    bind_display(commands, dest, |w| active_tab(w) == ImportTab::Destination);
    let mut dest_kids = Vec::new();
    if has_project {
        let tree = commands
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            })
            .id();
        let mut rows = vec![dest_folder_row(commands, fonts, String::new(), 0, "assets")];
        for (rel, depth, name) in &init.dest_folders {
            rows.push(dest_folder_row(commands, fonts, rel.clone(), depth + 1, name));
        }
        commands.entity(tree).add_children(&rows);
        dest_kids.push(scroll_view(commands, tree));
    }
    let org = radio_group(
        commands,
        &fonts.ui,
        &["Folder per file", "All in one folder"],
        init.layout,
    );
    bind_2way(
        commands,
        org,
        |w| match w.get_resource::<ImportOverlayState>().map(|s| s.layout) {
            Some(ImportLayout::Combined) => 1usize,
            _ => 0,
        },
        |w, v: &usize| {
            if let Some(mut s) = w.get_resource_mut::<ImportOverlayState>() {
                s.layout = if *v == 1 {
                    ImportLayout::Combined
                } else {
                    ImportLayout::PerFileFolder
                };
            }
        },
    );
    dest_kids.push(org);
    commands.entity(dest).add_children(&dest_kids);

    commands
        .entity(col)
        .add_children(&[files, scene, meshes, materials, dest]);
    col
}

/// A tab-gated scrolling keyed list, used for the tree and the two flat lists.
fn list_pane(commands: &mut Commands, tab: ImportTab, snapshot: fn(&Rx) -> KeyedSnapshot) -> Entity {
    let holder = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .id();
    bind_display(commands, holder, move |w| active_tab(w) == tab);
    let list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(1.0),
            ..default()
        })
        .id();
    keyed_list(commands, list, snapshot);
    let scroll = scroll_view(commands, list);
    commands.entity(holder).add_child(scroll);
    holder
}

// ── Centre ───────────────────────────────────────────────────────────────────

pub(super) fn build_centre(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let centre = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                height: Val::Percent(100.0),
                min_width: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                // No padding: the render fills the region edge to edge, so
                // there is no letterbox between it and the columns.
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
        ))
        .id();

    // The staged model, filling the region.
    let view = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            ImageNode::default(),
            Interaction::default(),
            // Blocks the press from reaching the editor viewport behind; the
            // orbit handler reads this node's own `Interaction`.
            FocusPolicy::Block,
            crate::preview3d::ImportPreviewViewport,
        ))
        .id();
    bind_display(commands, view, |w| has_staged(w) && !showing_material(w));
    bind_with(
        commands,
        view,
        |w| {
            w.get_resource::<crate::preview3d::ImportPreviewImage>()
                .map(|i| i.handle.id())
        },
        |world, entity, _| {
            let Some(handle) = crate::preview3d::preview_image(world) else {
                return;
            };
            if let Some(mut node) = world.get_mut::<ImageNode>(entity) {
                node.image = handle;
            }
        },
    );

    // The selected material, shown in the main viewport rather than a
    // thumbnail in the rail — a 190px square is not enough to judge a surface.
    let mat_view = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            ImageNode::default(),
            Interaction::default(),
            FocusPolicy::Block,
            crate::matpreview::MaterialPreviewViewport,
        ))
        .id();
    bind_display(commands, mat_view, showing_material);
    bind_with(
        commands,
        mat_view,
        |w| {
            w.get_resource::<crate::matpreview::MaterialPreviewImage>()
                .map(|i| i.handle.id())
        },
        |world, entity, _| {
            let Some(handle) = crate::matpreview::preview_image(world) else {
                return;
            };
            if let Some(mut node) = world.get_mut::<ImageNode>(entity) {
                node.image = handle;
            }
        },
    );

    // Before anything is staged the centre explains what the window is for.
    let placeholder = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    bind_display(commands, placeholder, |w| !has_staged(w));
    let ph_icon = icon_text(commands, &fonts.phosphor, "cube-transparent", text_muted(), 40.0);
    let ph_text = commands
        .spawn((
            Text::new(String::new()),
            ui_font(&fonts.ui, 12.5),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_text(commands, ph_text, |w| {
        let Some(s) = w.get_resource::<ImportOverlayState>() else {
            return String::new();
        };
        // Conversion starts on its own once files are chosen, so this reports
        // what is happening rather than asking for another click.
        if s.active_task.is_some() {
            return match &s.progress {
                ImportProgress::Working { label, .. } if !label.is_empty() => label.clone(),
                _ => "Converting…".to_string(),
            };
        }
        match s.pending_files.len() {
            0 => "Choose a model to import".to_string(),
            1 => "1 file queued".to_string(),
            n => format!("{n} files queued"),
        }
    });
    commands
        .entity(placeholder)
        .add_children(&[ph_icon, ph_text]);

    commands.entity(centre).add_children(&[view, mat_view, placeholder]);
    centre
}

// ── Right rail ───────────────────────────────────────────────────────────────

pub(super) fn build_right_rail(commands: &mut Commands, fonts: &EmberFonts, init: &Init) -> Entity {
    let col = commands
        .spawn((
            Node {
                width: Val::Px(320.0),
                flex_shrink: 0.0,
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
        ))
        .id();
    bind_column_width(commands, col, Side::Right);

    let inner = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(10.0),
            padding: UiRect::all(Val::Px(12.0)),
            ..default()
        })
        .id();

    // ── Selected-item properties (staged only) ──────────────────────────
    let props = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    bind_display(commands, props, has_staged);
    let props_head = group_label(commands, fonts, "Properties");
    let props_body = commands
        .spawn((
            Text::new(String::new()),
            ui_font(&fonts.mono, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_text(commands, props_body, selection_properties);
    commands
        .entity(props)
        .add_children(&[props_head, props_body]);

    // ── Findings (staged only) ──────────────────────────────────────────
    let findings = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    bind_display(commands, findings, has_staged);
    let f_head = commands
        .spawn((
            Text::new(String::new()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node { margin: UiRect::top(Val::Px(6.0)), ..default() },
        ))
        .id();
    bind_text(commands, f_head, |w| {
        staged(w)
            .map(|s| match s.problems() {
                0 => "FINDINGS — nothing looks wrong".to_string(),
                n => format!("FINDINGS — {n} to look at"),
            })
            .unwrap_or_default()
    });
    let f_list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    keyed_list(commands, f_list, findings_snapshot);
    commands.entity(findings).add_children(&[f_head, f_list]);

    // ── Import settings (before staging) ────────────────────────────────
    let settings = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let s_head = group_label(commands, fonts, "Import");

    let scale = drag_value(commands, &fonts.ui, "", text_primary(), init.scale, 0.01);
    bind_2way(
        commands,
        scale,
        |w| g_settings(w, |s| s.scale),
        |w, v: &f32| {
            s_settings(w, |s| s.scale = (*v).clamp(0.001, 1000.0));
            // Only reached when the widget's value differs from state, i.e. the
            // user scrubbed or typed — so it marks a deliberate choice and stops
            // the next queue auto-detecting over the top of it.
            if let Some(mut s) = w.get_resource_mut::<ImportOverlayState>() {
                s.scale_is_user_set = true;
            }
        },
    );
    let scale_row = field_row(commands, fonts, "Scale", scale);

    let axis = dropdown(
        commands,
        fonts,
        &["Auto", "Y-Up (GLTF/Bevy)", "Z-Up (Blender/CAD)"],
        init.up_axis,
    );
    bind_2way(
        commands,
        axis,
        |w| match w.get_resource::<ImportOverlayState>().map(|s| s.settings.up_axis) {
            Some(UpAxis::YUp) => 1usize,
            Some(UpAxis::ZUp) => 2,
            _ => 0,
        },
        |w, v: &usize| {
            s_settings(w, |s| {
                s.up_axis = match v {
                    1 => UpAxis::YUp,
                    2 => UpAxis::ZUp,
                    _ => UpAxis::Auto,
                }
            })
        },
    );
    let axis_row = field_row(commands, fonts, "Up axis", axis);

    // How the scene graph comes out. `Combined` is what the transcoders do
    // today; `One node per mesh` is the way to undo it and get pickable,
    // independently-culled objects back.
    let structure = dropdown(
        commands,
        fonts,
        &["As authored", "One node per mesh", "Combine meshes"],
        init.structure,
    );
    bind_2way(
        commands,
        structure,
        |w| match w.get_resource::<ImportOverlayState>().map(|s| s.settings.structure) {
            Some(SceneStructure::FlatPerMesh) => 1usize,
            Some(SceneStructure::Combined) => 2,
            _ => 0,
        },
        |w, v: &usize| {
            s_settings(w, |s| {
                s.structure = match v {
                    1 => SceneStructure::FlatPerMesh,
                    2 => SceneStructure::Combined,
                    _ => SceneStructure::Preserve,
                }
            })
        },
    );
    let structure_row = field_row(commands, fonts, "Hierarchy", structure);

    // Sibling texture sets, for a format that stores no materials of its own.
    // Only built when the queue actually offers some, so the row is absent
    // rather than empty for every other format.
    let texture_set_row = (!init.texture_sets.is_empty()).then(|| {
        let mut labels = vec!["None".to_string()];
        labels.extend(
            init.texture_sets
                .iter()
                .map(|(stem, roles)| format!("{stem}  ({roles})")),
        );
        let refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
        let picker = dropdown(commands, fonts, &refs, init.texture_set);
        // The set list is captured rather than re-read: it is fixed for the
        // window's lifetime, and the binding stores the *name* so the choice
        // survives a reimport even if the folder gains a file.
        let stems: Vec<String> = init.texture_sets.iter().map(|(s, _)| s.clone()).collect();
        let get_stems = stems.clone();
        bind_2way(
            commands,
            picker,
            move |w| {
                w.get_resource::<ImportOverlayState>()
                    .and_then(|s| s.settings.texture_set.clone())
                    .and_then(|want| get_stems.iter().position(|s| *s == want))
                    .map_or(0usize, |i| i + 1)
            },
            move |w, v: &usize| {
                let chosen = v.checked_sub(1).and_then(|i| stems.get(i).cloned());
                s_settings(w, |s| s.texture_set = chosen);
            },
        );
        field_row(commands, fonts, "Textures", picker)
    });

    let flip = toggle_row(commands, fonts, "Flip UVs", |s| s.flip_uvs, |s, v| s.flip_uvs = v);
    let normals = toggle_row(
        commands,
        fonts,
        "Generate normals",
        |s| s.generate_normals,
        |s, v| s.generate_normals = v,
    );

    let e_head = group_label(commands, fonts, "Extract");
    let e1 = toggle_row(commands, fonts, "Skeleton + skin", |s| s.extract_skeleton, |s, v| s.extract_skeleton = v);
    let e2 = toggle_row(commands, fonts, "Animations", |s| s.extract_animations, |s, v| s.extract_animations = v);
    let e3 = toggle_row(commands, fonts, "Textures", |s| s.extract_textures, |s, v| s.extract_textures = v);
    let e4 = toggle_row(commands, fonts, "Materials", |s| s.extract_materials, |s, v| s.extract_materials = v);

    let o_head = group_label(commands, fonts, "Optimize");
    let o1 = toggle_row(commands, fonts, "Vertex cache", |s| s.optimize_vertex_cache, |s, v| s.optimize_vertex_cache = v);
    let o2 = toggle_row(commands, fonts, "Overdraw", |s| s.optimize_overdraw, |s, v| s.optimize_overdraw = v);
    let o3 = toggle_row(commands, fonts, "Vertex fetch", |s| s.optimize_vertex_fetch, |s, v| s.optimize_vertex_fetch = v);

    let mut kids = vec![s_head, scale_row, axis_row, structure_row];
    kids.extend(texture_set_row);
    kids.extend([
        flip, normals, e_head, e1, e2, e3, e4, o_head, o1, o2, o3,
    ]);
    // `add_children` takes a slice, and the settings column is past the
    // tuple-bundle limit, so build the vector and hand it over in one call.
    commands.entity(settings).add_children(&kids);

    // Per-file results from the last run. Hidden until something has been
    // logged, so the rail is not carrying an empty heading most of the time.
    let results = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            ..default()
        })
        .id();
    bind_display(commands, results, |w| {
        !has_staged(w)
            && w.get_resource::<ImportOverlayState>()
                .is_some_and(|s| !s.log_entries.is_empty())
    });
    let r_head = group_label(commands, fonts, "Results");
    let r_list = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            LogContainer,
        ))
        .id();
    keyed_list(commands, r_list, log_snapshot);
    commands.entity(results).add_children(&[r_head, r_list]);

    commands
        .entity(inner)
        .add_children(&[props, findings, settings, results]);
    let scroll = scroll_view(commands, inner);
    commands.entity(col).add_child(scroll);
    col
}

/// Keep a column's width in step with [`ImportColumns`].
fn bind_column_width(commands: &mut Commands, target: Entity, side: Side) {
    bind_with(
        commands,
        target,
        move |w| {
            let c = w.get_resource::<ImportColumns>();
            let v = match side {
                Side::Left => c.map(|c| c.left).unwrap_or(310.0),
                Side::Right => c.map(|c| c.right).unwrap_or(320.0),
            };
            // Bindings compare by value, and f32 is not Eq — round to whole
            // pixels so this only fires when the width actually changes.
            v.round() as i32
        },
        |world, e, px| {
            if let Some(mut node) = world.get_mut::<Node>(e) {
                node.width = Val::Px(*px as f32);
            }
        },
    );
}

/// A small uppercase group heading for the right rail.
fn group_label(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    commands
        .spawn((
            Text::new(label.to_uppercase()),
            ui_font(&fonts.ui, 10.5),
            TextColor(rgb(text_muted())),
            Node {
                margin: UiRect::top(Val::Px(6.0)),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id()
}

// ── Destination picker ───────────────────────────────────────────────────────

/// One selectable row in the destination folder tree. `rel` is the
/// project-relative target path (`""` = project root); selection highlights the
/// row whose path matches `ImportOverlayState::target_directory`.
fn dest_folder_row(commands: &mut Commands, fonts: &EmberFonts, rel: String, depth: usize, name: &str) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(22.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::left(Val::Px(8.0 + depth as f32 * 14.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            DestFolderRow(rel.clone()),
            hover_cursor(),
        ))
        .id();
    let p = rel.clone();
    bind_bg(commands, row, move |w| {
        let selected = w.get_resource::<ImportOverlayState>().map(|s| s.target_directory == p).unwrap_or(false);
        if selected {
            rgb(accent()).with_alpha(0.20)
        } else if matches!(w.get::<Interaction>(row), Some(Interaction::Hovered) | Some(Interaction::Pressed)) {
            rgb(hover_bg())
        } else {
            Color::NONE
        }
    });
    let icon = icon_text(commands, &fonts.phosphor, "folder", text_muted(), 12.0);
    commands.entity(icon).insert(FocusPolicy::Pass);
    let lbl = commands.spawn((Text::new(name.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), FocusPolicy::Pass)).id();
    commands.entity(row).add_children(&[icon, lbl]);
    row
}

/// Recursively list the project's directories (two levels deep) as
/// project-relative forward-slashed paths, skipping hidden / build / dependency
/// folders. Mirrors the marketplace install picker's `scan_dirs`.
pub(super) fn scan_dest_dirs(root: &std::path::Path) -> Vec<(String, usize, String)> {
    fn rec(root: &std::path::Path, dir: &std::path::Path, depth: usize, max: usize, out: &mut Vec<(String, usize, String)>) {
        if depth > max || out.len() > 300 {
            return;
        }
        let Ok(read) = std::fs::read_dir(dir) else { return };
        let mut entries: Vec<PathBuf> = read.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
        entries.sort();
        for path in entries {
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            let rel = path.strip_prefix(root).ok().map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
            out.push((rel, depth, name));
            rec(root, &path, depth + 1, max, out);
        }
    }
    let mut out = Vec::new();
    rec(root, root, 0, 1, &mut out);
    out
}
