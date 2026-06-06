//! Bevy-native (ember) import overlay — the bevy_ui counterpart to the egui
//! `draw_import_overlay`. Same modal: a Files list, Import Settings, Extract,
//! Mesh Optimization, Destination, progress + output log, and Import/Cancel
//! buttons. It edits the same [`ImportOverlayState`] and reuses the worker
//! (`run_import` / `poll_import_task`). Renders only under the BevyUi backend;
//! the egui overlay renders under Egui. The egui orchestration (file drops,
//! ImportRequested, auto-import) keeps running regardless.

use std::path::PathBuf;

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::{bind_2way, bind_bg, bind_display, bind_text, bind_with, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    bind_text_input, checkbox, drag_value, dropdown, radio_group, scroll_area, spinner, text_input, OverlaySurface,
};

use renzora_import::settings::UpAxis;

use crate::overlay::{close_overlay, poll_import_task, run_import, ImportLayout, ImportOverlayState, ImportProgress};

const GREEN: (u8, u8, u8) = (89, 191, 115);
const RED: (u8, u8, u8) = (239, 68, 68);
const FILE_ORANGE: (u8, u8, u8) = (255, 170, 100);

pub(crate) fn register(app: &mut App) {
    app.add_systems(
        Update,
        (
            manage_import_modal,
            file_browse_click,
            dest_browse_click,
            import_click,
            cancel_click,
            remove_file_click,
        ),
    );
}

// ── Markers ──────────────────────────────────────────────────────────────────

#[derive(Component)]
struct ImportRoot;
#[derive(Component)]
struct FileBrowseBtn;
#[derive(Component)]
struct DestBrowseBtn;
#[derive(Component)]
struct ImportBtn;
#[derive(Component)]
struct CancelBtn;
#[derive(Component, Clone)]
struct RemoveFileBtn(PathBuf);
#[derive(Component)]
struct FilesContainer;
#[derive(Component)]
struct LogContainer;

// ── Lifecycle ────────────────────────────────────────────────────────────────

fn manage_import_modal(world: &mut World) {
    let visible = world.get_resource::<ImportOverlayState>().is_some_and(|s| s.visible);
    if visible {
        poll_import_task(world); // keep progress flowing (egui draw is gated off)
    }

    let mut q = world.query_filtered::<Entity, With<ImportRoot>>();
    let existing: Vec<Entity> = q.iter(world).collect();

    if visible && existing.is_empty() {
        let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
        let has_project = world.get_resource::<renzora::core::CurrentProject>().is_some();
        let init = Init::read(world);
        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            spawn_modal(&mut commands, &fonts, &init, has_project);
        }
        queue.apply(world);
    } else if !visible && !existing.is_empty() {
        for e in existing {
            world.entity_mut(e).despawn();
        }
    }
}

/// Initial widget values read once at spawn (the bindings keep them in sync after).
struct Init {
    scale: f32,
    up_axis: usize,
    layout: usize,
    target_dir: String,
}
impl Init {
    fn read(world: &World) -> Self {
        let s = world.resource::<ImportOverlayState>();
        Self {
            scale: s.settings.scale,
            up_axis: match s.settings.up_axis {
                UpAxis::Auto => 0,
                UpAxis::YUp => 1,
                UpAxis::ZUp => 2,
            },
            layout: match s.layout {
                ImportLayout::PerFileFolder => 0,
                ImportLayout::Combined => 1,
            },
            target_dir: s.target_directory.clone(),
        }
    }
}

fn spawn_modal(commands: &mut Commands, fonts: &EmberFonts, init: &Init, has_project: bool) {
    let backdrop = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                align_items: AlignItems::FlexStart,
                justify_content: JustifyContent::Center,
                padding: UiRect::top(Val::Vh(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.63)),
            GlobalZIndex(9300),
            FocusPolicy::Block,
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            OverlaySurface,
            ImportRoot,
            Name::new("import-modal"),
        ))
        .id();

    let panel = commands
        .spawn((
            Node {
                width: Val::Px(520.0),
                max_height: Val::Vh(84.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(20.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            BorderColor::all(rgb(border())),
            FocusPolicy::Block,
            Name::new("import-panel"),
        ))
        .id();
    commands.entity(backdrop).add_child(panel);

    // Header.
    let header = row_between(commands);
    let title = title_row(commands, fonts, "cube", "Import 3D Models");
    let close = commands
        .spawn((Node { padding: UiRect::all(Val::Px(2.0)), ..default() }, Interaction::default(), CancelBtn, hover_cursor()))
        .id();
    let close_x = icon_text(commands, &fonts.phosphor, "x", text_muted(), 16.0);
    commands.entity(close_x).insert(FocusPolicy::Pass);
    commands.entity(close).add_child(close_x);
    commands.entity(header).add_children(&[title, close]);
    commands.entity(panel).add_child(header);

    let sep = commands
        .spawn((Node { width: Val::Percent(100.0), height: Val::Px(1.0), margin: UiRect::vertical(Val::Px(8.0)), ..default() }, BackgroundColor(rgb(divider()))))
        .id();
    commands.entity(panel).add_child(sep);

    if !has_project {
        let warn = txt(commands, fonts, "No project open. Open a project before importing.", 12.0, RED);
        commands.entity(panel).add_child(warn);
        return;
    }

    // Scrollable body (sections) + fixed footer (buttons).
    let body_col = commands
        .spawn((Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(12.0), ..default() }, Name::new("import-body")))
        .id();
    let body = scroll_area(commands, body_col, 460.0);
    commands.entity(panel).add_child(body);

    build_files(commands, fonts, body_col);
    build_settings(commands, fonts, body_col, init);
    build_extract(commands, fonts, body_col);
    build_meshopt(commands, fonts, body_col);
    build_destination(commands, fonts, body_col, init);
    build_progress(commands, fonts, body_col);
    build_log(commands, fonts, body_col);

    build_footer(commands, fonts, panel);
}

// ── Sections ─────────────────────────────────────────────────────────────────

fn build_files(commands: &mut Commands, fonts: &EmberFonts, parent: Entity) {
    let sec = section(commands);
    let head = section_label(commands, fonts, "files", "Files");
    // File list frame.
    let list = commands.spawn((Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() }, FilesContainer)).id();
    keyed_list(commands, list, files_snapshot);
    let scroll = scroll_area(commands, list, 120.0);
    let frame = commands
        .spawn((
            Node { width: Val::Percent(100.0), padding: UiRect::all(Val::Px(8.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(rgb(section_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id();
    commands.entity(frame).add_child(scroll);
    // Empty hint (shown when no files).
    let empty = txt(commands, fonts, "No files added yet", 11.0, text_muted());
    bind_display(commands, empty, |w| w.get_resource::<ImportOverlayState>().is_some_and(|s| s.pending_files.is_empty()));
    commands.entity(frame).add_child(empty);

    // Drop hint + Browse.
    let addrow = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), margin: UiRect::top(Val::Px(4.0)), ..default() }).id();
    let hint = icon_label(commands, fonts, "cloud-arrow-down", "Drop files here or", text_muted(), 11.0);
    let browse = pill_button(commands, fonts, "folder-open", "Browse...");
    commands.entity(browse).insert(FileBrowseBtn);
    commands.entity(addrow).add_children(&[hint, browse]);

    commands.entity(sec).add_children(&[head, frame, addrow]);
    commands.entity(parent).add_child(sec);
}

fn build_settings(commands: &mut Commands, fonts: &EmberFonts, parent: Entity, init: &Init) {
    let sec = section(commands);
    let head = section_label(commands, fonts, "gear", "Import Settings");

    // Scale.
    let scale_row = labeled(commands, fonts, "Scale:");
    let scale = drag_value(commands, &fonts.ui, "", text_primary(), init.scale, 0.01);
    bind_2way(commands, scale, |w| g_settings(w, |s| s.scale), |w, v: &f32| s_settings(w, |s| s.scale = (*v).clamp(0.001, 1000.0)));
    commands.entity(scale_row).add_child(scale);

    // Up axis dropdown.
    let axis_row = labeled(commands, fonts, "Up Axis:");
    let axis = dropdown(commands, fonts, &["Auto", "Y-Up (GLTF/Bevy)", "Z-Up (Blender/CAD)"], init.up_axis);
    bind_2way(
        commands,
        axis,
        |w| match w.get_resource::<ImportOverlayState>().map(|s| s.settings.up_axis) {
            Some(UpAxis::YUp) => 1usize,
            Some(UpAxis::ZUp) => 2,
            _ => 0,
        },
        |w, v: &usize| s_settings(w, |s| s.up_axis = match v { 1 => UpAxis::YUp, 2 => UpAxis::ZUp, _ => UpAxis::Auto }),
    );
    commands.entity(axis_row).add_child(axis);

    let flip = check_row(commands, fonts, "Flip UVs", |s| s.flip_uvs, |s, v| s.flip_uvs = v);
    let normals = check_row(commands, fonts, "Generate normals if missing", |s| s.generate_normals, |s, v| s.generate_normals = v);

    commands.entity(sec).add_children(&[head, scale_row, axis_row, flip, normals]);
    commands.entity(parent).add_child(sec);
}

fn build_extract(commands: &mut Commands, fonts: &EmberFonts, parent: Entity) {
    let sec = section(commands);
    let head = section_label(commands, fonts, "puzzle-piece", "Extract");
    let a = check_row(commands, fonts, "Skeleton + skin weights", |s| s.extract_skeleton, |s, v| s.extract_skeleton = v);
    let b = check_row(commands, fonts, "Animations (→ animations/*.anim)", |s| s.extract_animations, |s, v| s.extract_animations = v);
    let c = check_row(commands, fonts, "Textures (→ textures/*.png)", |s| s.extract_textures, |s, v| s.extract_textures = v);
    let d = check_row(commands, fonts, "Materials (→ materials/*.material)", |s| s.extract_materials, |s, v| s.extract_materials = v);
    commands.entity(sec).add_children(&[head, a, b, c, d]);
    commands.entity(parent).add_child(sec);
}

fn build_meshopt(commands: &mut Commands, fonts: &EmberFonts, parent: Entity) {
    let sec = section(commands);
    let head = section_label(commands, fonts, "cube", "Mesh Optimization");
    let a = check_row(commands, fonts, "Optimize vertex cache", |s| s.optimize_vertex_cache, |s, v| s.optimize_vertex_cache = v);
    let b = check_row(commands, fonts, "Optimize overdraw", |s| s.optimize_overdraw, |s, v| s.optimize_overdraw = v);
    let c = check_row(commands, fonts, "Optimize vertex fetch", |s| s.optimize_vertex_fetch, |s, v| s.optimize_vertex_fetch = v);
    commands.entity(sec).add_children(&[head, a, b, c]);
    commands.entity(parent).add_child(sec);
}

fn build_destination(commands: &mut Commands, fonts: &EmberFonts, parent: Entity, init: &Init) {
    let sec = section(commands);
    let head = section_label(commands, fonts, "folder-open", "Destination");

    let path_row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }).id();
    let prefix = txt(commands, fonts, "assets/", 12.0, text_muted());
    let input = text_input(commands, &fonts.ui, "e.g. models/imported", &init.target_dir);
    commands.entity(input).insert(Node { flex_grow: 1.0, height: Val::Px(28.0), align_items: AlignItems::Center, padding: UiRect::horizontal(Val::Px(8.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() });
    bind_text_input(commands, input, |w| w.get_resource::<ImportOverlayState>().map(|s| s.target_directory.clone()).unwrap_or_default(), |w, v| { if let Some(mut s) = w.get_resource_mut::<ImportOverlayState>() { s.target_directory = v; } });
    commands.entity(path_row).add_children(&[prefix, input]);

    let org = txt(commands, fonts, "Organize", 12.0, text_muted());
    let radios = radio_group(commands, &fonts.ui, &["Separate folder per file", "Combine all files into destination"], init.layout);
    bind_2way(
        commands,
        radios,
        |w| match w.get_resource::<ImportOverlayState>().map(|s| s.layout) {
            Some(ImportLayout::Combined) => 1usize,
            _ => 0,
        },
        |w, v: &usize| { if let Some(mut s) = w.get_resource_mut::<ImportOverlayState>() { s.layout = if *v == 1 { ImportLayout::Combined } else { ImportLayout::PerFileFolder }; } },
    );

    let dest_browse = pill_button(commands, fonts, "folder", "Browse");
    commands.entity(dest_browse).insert(DestBrowseBtn);

    commands.entity(sec).add_children(&[head, path_row, org, radios, dest_browse]);
    commands.entity(parent).add_child(sec);
}

fn build_progress(commands: &mut Commands, fonts: &EmberFonts, parent: Entity) {
    let sec = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() }).id();

    // Working: spinner + label + bar.
    let working = commands.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(5.0), ..default() }).id();
    let toprow = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).id();
    let spin = spinner(commands);
    let plabel = txt(commands, fonts, "", 12.0, text_muted());
    bind_text(commands, plabel, |w| match w.get_resource::<ImportOverlayState>().map(|s| s.progress.clone()) {
        Some(ImportProgress::Working { current, total, label }) => format!("[{current}/{total}] {label}"),
        _ => String::new(),
    });
    commands.entity(toprow).add_children(&[spin, plabel]);
    let track = commands.spawn((Node { width: Val::Percent(100.0), height: Val::Px(8.0), overflow: Overflow::clip(), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() }, BackgroundColor(rgb(section_bg())))).id();
    let fill = commands.spawn((Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(rgb(accent())))).id();
    bind_with(commands, fill, progress_fraction, |w, target, v: &OrderedF32| { if let Some(mut n) = w.get_mut::<Node>(target) { n.width = Val::Percent((v.0 * 100.0).clamp(0.0, 100.0)); } });
    commands.entity(track).add_child(fill);
    commands.entity(working).add_children(&[toprow, track]);
    bind_display(commands, working, |w| matches!(w.get_resource::<ImportOverlayState>().map(|s| &s.progress), Some(ImportProgress::Working { .. })));

    // Done / Error messages.
    let (done, done_msg) = icon_msg(commands, fonts, "check-circle", GREEN);
    bind_text(commands, done_msg, |w| match w.get_resource::<ImportOverlayState>().map(|s| s.progress.clone()) {
        Some(ImportProgress::Done(m)) => m,
        _ => String::new(),
    });
    bind_display(commands, done, |w| matches!(w.get_resource::<ImportOverlayState>().map(|s| &s.progress), Some(ImportProgress::Done(_))));

    let (err, err_msg) = icon_msg(commands, fonts, "warning", RED);
    bind_text(commands, err_msg, |w| match w.get_resource::<ImportOverlayState>().map(|s| s.progress.clone()) {
        Some(ImportProgress::Error(m)) => m,
        _ => String::new(),
    });
    bind_display(commands, err, |w| matches!(w.get_resource::<ImportOverlayState>().map(|s| &s.progress), Some(ImportProgress::Error(_))));

    commands.entity(sec).add_children(&[working, done, err]);
    commands.entity(parent).add_child(sec);
}

fn build_log(commands: &mut Commands, fonts: &EmberFonts, parent: Entity) {
    let sec = section(commands);
    let head = section_label(commands, fonts, "list-bullets", "Output");
    let list = commands.spawn((Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() }, LogContainer)).id();
    keyed_list(commands, list, log_snapshot);
    let scroll = scroll_area(commands, list, 120.0);
    let frame = commands
        .spawn((Node { width: Val::Percent(100.0), padding: UiRect::all(Val::Px(8.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() }, BackgroundColor(rgb(section_bg())), BorderColor::all(rgb(border()))))
        .id();
    commands.entity(frame).add_child(scroll);
    commands.entity(sec).add_children(&[head, frame]);
    // Whole section hidden when the log is empty.
    bind_display(commands, sec, |w| w.get_resource::<ImportOverlayState>().is_some_and(|s| !s.log_entries.is_empty()));
    commands.entity(parent).add_child(sec);
}

fn build_footer(commands: &mut Commands, fonts: &EmberFonts, panel: Entity) {
    let row = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::FlexEnd, column_gap: Val::Px(8.0), margin: UiRect::top(Val::Px(12.0)), ..default() }).id();
    let cancel = wide_button(commands, fonts, "Cancel", rgb(section_bg()), text_primary(), 80.0);
    commands.entity(cancel).insert(CancelBtn);
    let import = commands
        .spawn((
            Node { min_width: Val::Px(100.0), height: Val::Px(32.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::Center, column_gap: Val::Px(6.0), border_radius: BorderRadius::all(Val::Px(5.0)), ..default() },
            BackgroundColor(rgb(accent())),
            Interaction::default(),
            ImportBtn,
            hover_cursor(),
        ))
        .id();
    bind_bg(commands, import, |w| if can_import(w) { rgb(accent()) } else { rgb(section_bg()) });
    let ic = icon_text(commands, &fonts.phosphor, "download-simple", (255, 255, 255), 14.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let it = commands.spawn((Text::new("Import".to_string()), ui_font(&fonts.ui, 13.0), TextColor(Color::WHITE), FocusPolicy::Pass)).id();
    commands.entity(import).add_children(&[ic, it]);
    commands.entity(row).add_children(&[cancel, import]);
    commands.entity(panel).add_child(row);
}

// ── Keyed lists ──────────────────────────────────────────────────────────────

fn files_snapshot(world: &World) -> KeyedSnapshot {
    use std::hash::{Hash, Hasher};
    let files: Vec<PathBuf> = world.get_resource::<ImportOverlayState>().map(|s| s.pending_files.clone()).unwrap_or_default();
    let items: Vec<(u64, u64)> = files
        .iter()
        .map(|p| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            p.hash(&mut h);
            (h.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot { items, build: Box::new(move |c, f, i| file_row(c, f, &files[i])) }
}

fn file_row(commands: &mut Commands, fonts: &EmberFonts, path: &std::path::Path) -> Entity {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?").to_string();
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_uppercase();
    let row = commands.spawn((Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }, FocusPolicy::Pass)).id();
    let icon = icon_text(commands, &fonts.phosphor, "cube", FILE_ORANGE, 12.0);
    commands.entity(icon).insert(FocusPolicy::Pass);
    let nm = commands.spawn((Text::new(name), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), FocusPolicy::Pass, Node { flex_grow: 1.0, ..default() })).id();
    let ex = commands.spawn((Text::new(format!("({ext})")), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted())), FocusPolicy::Pass)).id();
    let rm = commands.spawn((Node { padding: UiRect::all(Val::Px(2.0)), ..default() }, Interaction::default(), RemoveFileBtn(path.to_path_buf()), hover_cursor())).id();
    let rmx = icon_text(commands, &fonts.phosphor, "x", text_muted(), 11.0);
    commands.entity(rmx).insert(FocusPolicy::Pass);
    commands.entity(rm).add_child(rmx);
    commands.entity(row).add_children(&[icon, nm, ex, rm]);
    row
}

fn log_snapshot(world: &World) -> KeyedSnapshot {
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

// ── Interaction ──────────────────────────────────────────────────────────────

fn can_import(w: &World) -> bool {
    w.get_resource::<ImportOverlayState>().is_some_and(|s| !s.pending_files.is_empty() && s.active_task.is_none() && matches!(s.progress, ImportProgress::Idle | ImportProgress::Done(_) | ImportProgress::Error(_)))
}

#[derive(PartialEq)]
struct OrderedF32(f32);
fn progress_fraction(w: &World) -> OrderedF32 {
    let f = match w.get_resource::<ImportOverlayState>().map(|s| s.progress.clone()) {
        Some(ImportProgress::Working { current, total, .. }) if total > 0 => current as f32 / total as f32,
        _ => 0.0,
    };
    OrderedF32(f)
}

fn import_click(q: Query<&Interaction, (With<ImportBtn>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|w: &mut World| {
            if can_import(w) {
                run_import(w);
            }
        });
    }
}

fn cancel_click(q: Query<&Interaction, (With<CancelBtn>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|w: &mut World| close_overlay(w));
    }
}

fn remove_file_click(q: Query<(&Interaction, &RemoveFileBtn), Changed<Interaction>>, mut state: Option<ResMut<ImportOverlayState>>) {
    let Some(state) = state.as_mut() else { return };
    for (i, rm) in &q {
        if *i == Interaction::Pressed {
            state.pending_files.retain(|p| p != &rm.0);
        }
    }
}

fn file_browse_click(q: Query<&Interaction, (With<FileBrowseBtn>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(do_file_browse);
    }
}

fn do_file_browse(world: &mut World) {
    let Some(paths) = rfd::FileDialog::new()
        .set_title("Select 3D model files")
        .add_filter("3D Models", renzora_import::supported_extensions())
        .add_filter("All Files", &["*"])
        .pick_files()
    else {
        return;
    };
    let mut state = world.resource_mut::<ImportOverlayState>();
    let was_empty = state.pending_files.is_empty();
    for p in &paths {
        if !state.pending_files.contains(p) {
            state.pending_files.push(p.clone());
        }
    }
    if was_empty && state.settings.scale == 1.0 {
        if let Some(scale) = paths.first().and_then(|p| renzora_import::units::detect_unit_scale(p)) {
            state.settings.scale = scale;
        }
    }
}

fn dest_browse_click(q: Query<&Interaction, (With<DestBrowseBtn>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(do_dest_browse);
    }
}

fn do_dest_browse(world: &mut World) {
    let root = world.get_resource::<renzora::core::CurrentProject>().map(|p| p.path.clone());
    let mut dlg = rfd::FileDialog::new().set_title("Select destination folder");
    if let Some(root) = &root {
        dlg = dlg.set_directory(root);
    }
    let Some(dir) = dlg.pick_folder() else { return };
    let rel = root
        .as_ref()
        .and_then(|r| dir.strip_prefix(r).ok())
        .map(|p| p.to_path_buf())
        .unwrap_or(dir);
    let rel = rel.to_string_lossy().replace('\\', "/");
    world.resource_mut::<ImportOverlayState>().target_directory = rel;
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn hover_cursor() -> renzora_hui::cursor_icon::HoverCursor {
    renzora_hui::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer)
}

fn txt(commands: &mut Commands, fonts: &EmberFonts, s: &str, size: f32, color: (u8, u8, u8)) -> Entity {
    commands.spawn((Text::new(s.to_string()), ui_font(&fonts.ui, size), TextColor(rgb(color)))).id()
}

fn section(commands: &mut Commands) -> Entity {
    commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() }).id()
}

fn row_between(commands: &mut Commands) -> Entity {
    commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, ..default() }).id()
}

fn title_row(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str) -> Entity {
    let row = commands.spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }, FocusPolicy::Pass)).id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_primary(), 18.0);
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 18.0), TextColor(rgb(text_primary())))).id();
    commands.entity(row).add_children(&[ic, t]);
    row
}

fn section_label(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str) -> Entity {
    let row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }).id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_primary(), 13.0);
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_primary())))).id();
    commands.entity(row).add_children(&[ic, t]);
    row
}

fn icon_label(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str, color: (u8, u8, u8), size: f32) -> Entity {
    let row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }).id();
    let ic = icon_text(commands, &fonts.phosphor, icon, color, size);
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, size), TextColor(rgb(color)))).id();
    commands.entity(row).add_children(&[ic, t]);
    row
}

/// An icon + a bindable message text. Returns `(row, message_text_entity)`.
fn icon_msg(commands: &mut Commands, fonts: &EmberFonts, icon: &str, color: (u8, u8, u8)) -> (Entity, Entity) {
    let row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }).id();
    let ic = icon_text(commands, &fonts.phosphor, icon, color, 12.0);
    let t = commands.spawn((Text::new(String::new()), ui_font(&fonts.ui, 12.0), TextColor(rgb(color)))).id();
    commands.entity(row).add_children(&[ic, t]);
    (row, t)
}

fn labeled(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    let row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).id();
    let t = txt(commands, fonts, label, 12.0, text_muted());
    commands.entity(row).add_child(t);
    row
}

fn check_row(commands: &mut Commands, fonts: &EmberFonts, label: &str, get: fn(&renzora_import::settings::ImportSettings) -> bool, set: fn(&mut renzora_import::settings::ImportSettings, bool)) -> Entity {
    let row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).id();
    let cb = checkbox(commands, false);
    bind_2way(commands, cb, move |w| g_settings(w, get), move |w, v: &bool| s_settings(w, |s| set(s, *v)));
    let t = txt(commands, fonts, label, 12.0, text_primary());
    commands.entity(row).add_children(&[cb, t]);
    row
}

fn pill_button(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str) -> Entity {
    let btn = commands
        .spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(5.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() }, BackgroundColor(rgb(section_bg())), Interaction::default(), hover_cursor()))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_primary(), 11.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), FocusPolicy::Pass)).id();
    commands.entity(btn).add_children(&[ic, t]);
    btn
}

fn wide_button(commands: &mut Commands, fonts: &EmberFonts, label: &str, bg: Color, fg: (u8, u8, u8), min_w: f32) -> Entity {
    let btn = commands
        .spawn((Node { min_width: Val::Px(min_w), height: Val::Px(32.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(5.0)), ..default() }, BackgroundColor(bg), Interaction::default(), hover_cursor()))
        .id();
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 13.0), TextColor(rgb(fg)), FocusPolicy::Pass)).id();
    commands.entity(btn).add_child(t);
    btn
}

fn g_settings<T>(w: &World, get: impl Fn(&renzora_import::settings::ImportSettings) -> T) -> T
where
    T: Default,
{
    w.get_resource::<ImportOverlayState>().map(|s| get(&s.settings)).unwrap_or_default()
}
fn s_settings(w: &mut World, set: impl FnOnce(&mut renzora_import::settings::ImportSettings)) {
    if let Some(mut s) = w.get_resource_mut::<ImportOverlayState>() {
        set(&mut s.settings);
    }
}
