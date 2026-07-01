//! Bevy-native (ember) import overlay — the bevy_ui counterpart to the egui
//! `draw_import_overlay`. Two-pane modal: a left sidebar of sections (Files,
//! Settings, Extract, Optimize, Destination) and a right content pane showing
//! the active section. It edits the same [`ImportOverlayState`] and reuses the
//! worker (`run_import` / `poll_import_task`). Renders only under the BevyUi
//! backend; the egui overlay renders under Egui. The egui orchestration (file
//! drops, ImportRequested, auto-import) keeps running regardless.
//!
//! Why a sidebar instead of one long scroll: the import options span five
//! unrelated concerns. Stacked vertically they read as an undifferentiated wall
//! of checkboxes; split into named sections the user sees one focused panel at a
//! time and the modal stays a fixed, predictable size.

use std::path::PathBuf;

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::{bind_2way, bind_bg, bind_display, bind_text, bind_with, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    checkbox, drag_value, dropdown, radio_group, scroll_area, spinner, OverlaySurface,
};

use renzora_import::settings::UpAxis;

use crate::overlay::{close_overlay, poll_import_task, run_import, ImportLayout, ImportOverlayState, ImportProgress};

const GREEN: (u8, u8, u8) = (89, 191, 115);
const RED: (u8, u8, u8) = (239, 68, 68);

/// Which sidebar section is showing in the right pane. Lives in [`ImportNav`] so
/// the pane-visibility bindings and the nav-row highlights read one source of
/// truth; reset to [`ImportSection::Files`] each time the modal opens.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ImportSection {
    Files,
    Settings,
    Extract,
    Optimize,
    Destination,
    /// Per-file import results. Its nav row is hidden until there are results;
    /// the modal usually hands an import off to the corner toast, but if one
    /// runs with the modal open this is where the log lands.
    Output,
}

#[derive(Resource)]
struct ImportNav {
    active: ImportSection,
}
impl Default for ImportNav {
    fn default() -> Self {
        Self { active: ImportSection::Files }
    }
}

pub(crate) fn register(app: &mut App) {
    app.init_resource::<ImportNav>().add_systems(
        Update,
        (
            manage_import_modal,
            manage_import_toast,
            file_browse_click,
            dest_folder_click,
            nav_click,
            enforce_nav_section,
            import_click,
            cancel_click,
            toast_dismiss_click,
            remove_file_click,
        ),
    );
}

// ── Markers ──────────────────────────────────────────────────────────────────

#[derive(Component)]
struct ImportRoot;
#[derive(Component)]
struct FileBrowseBtn;
/// A clickable sidebar nav row; switches the active pane on press.
#[derive(Component, Clone, Copy)]
struct NavBtn(ImportSection);
/// A row in the destination folder tree. Holds the project-relative path it
/// targets (forward-slashed, `""` = project root).
#[derive(Component, Clone)]
struct DestFolderRow(String);
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
/// Root of the corner progress toast shown after the modal closes on Import.
#[derive(Component)]
struct ToastRoot;
/// The toast's close/dismiss button.
#[derive(Component)]
struct ToastDismissBtn;

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
        // Always open on the Files pane — the first thing the user does is add
        // files, and a stale section from a previous open would be confusing.
        world.resource_mut::<ImportNav>().active = ImportSection::Files;
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
    /// Project directory tree for the destination picker: (rel_path, depth, name),
    /// `rel_path` forward-slashed and relative to the project root (`""` = root).
    dest_folders: Vec<(String, usize, String)>,
}
impl Init {
    fn read(world: &World) -> Self {
        let s = world.resource::<ImportOverlayState>();
        let dest_folders = world
            .get_resource::<renzora::core::CurrentProject>()
            .map(|p| scan_dest_dirs(&p.path))
            .unwrap_or_default();
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
            dest_folders,
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
                padding: UiRect::top(Val::Vh(7.0)),
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
                width: Val::Px(660.0),
                max_height: Val::Vh(88.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(20.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            BorderColor::all(rgb(border())),
            FocusPolicy::Block,
            Name::new("import-panel"),
        ))
        .id();
    commands.entity(backdrop).add_child(panel);

    // Header. The title tracks the queued files' kind — "Import 3D Models" for a
    // pure model queue, "Import Images"/"Import Audio"/… for a uniform non-model
    // queue, and the generic "Import Assets" for an empty or mixed queue.
    let header = row_between(commands);
    let title = commands
        .spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }, FocusPolicy::Pass))
        .id();
    let title_ic = icon_text(commands, &fonts.phosphor, "package", text_primary(), 18.0);
    let title_tx = commands
        .spawn((Text::new("Import Assets".to_string()), ui_font(&fonts.ui, 18.0), TextColor(rgb(text_primary()))))
        .id();
    bind_text(commands, title_tx, import_title);
    commands.entity(title).add_children(&[title_ic, title_tx]);
    let close = commands
        .spawn((Node { padding: UiRect::all(Val::Px(2.0)), ..default() }, Interaction::default(), CancelBtn, hover_cursor()))
        .id();
    let close_x = icon_text(commands, &fonts.phosphor, "x", text_muted(), 16.0);
    commands.entity(close_x).insert(FocusPolicy::Pass);
    commands.entity(close).add_child(close_x);
    commands.entity(header).add_children(&[title, close]);
    commands.entity(panel).add_child(header);

    let sep = commands
        .spawn((Node { width: Val::Percent(100.0), height: Val::Px(1.0), margin: UiRect::vertical(Val::Px(14.0)), ..default() }, BackgroundColor(rgb(divider()))))
        .id();
    commands.entity(panel).add_child(sep);

    if !has_project {
        let warn = txt(commands, fonts, "No project open. Open a project before importing.", 12.0, RED);
        commands.entity(panel).add_child(warn);
        return;
    }

    // Body: fixed-height row of [sidebar | content]. A fixed height keeps the
    // modal from resizing as the user flips between short and tall sections.
    let body = commands
        .spawn(Node { width: Val::Percent(100.0), height: Val::Px(400.0), flex_direction: FlexDirection::Row, ..default() })
        .id();

    let sidebar = commands
        .spawn((
            Node {
                width: Val::Px(176.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                padding: UiRect::right(Val::Px(16.0)),
                border: UiRect::right(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(rgb(divider())),
        ))
        .id();
    // Files and Destination apply to every kind and are always shown. The
    // Settings/Extract/Optimize sections only make sense for models (they feed
    // the GLB conversion), so their nav rows hide whenever the queue has no
    // model in it — a pure image/audio/… import is just a copy-to-destination.
    let mut items: Vec<Entity> = Vec::new();
    items.push(nav_item(commands, fonts, "files", "Files", ImportSection::Files));
    for (icon, label, sec) in [
        ("gear", "Settings", ImportSection::Settings),
        ("puzzle-piece", "Extract", ImportSection::Extract),
        ("cube", "Optimize", ImportSection::Optimize),
    ] {
        let row = nav_item(commands, fonts, icon, label, sec);
        bind_display(commands, row, queue_has_model);
        items.push(row);
    }
    items.push(nav_item(commands, fonts, "folder-open", "Destination", ImportSection::Destination));
    // Output nav row: only present once an import has logged results.
    let output = nav_item(commands, fonts, "list-bullets", "Output", ImportSection::Output);
    bind_display(commands, output, |w| w.get_resource::<ImportOverlayState>().is_some_and(|s| !s.log_entries.is_empty()));
    items.push(output);
    commands.entity(sidebar).add_children(&items);

    // Content: each pane stacks here but only the active one displays. Wrapped
    // in a scroll so a long section (the folder tree) can overflow gracefully.
    let stack = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() })
        .id();
    build_files_pane(commands, fonts, stack);
    build_settings_pane(commands, fonts, stack, init);
    build_extract_pane(commands, fonts, stack);
    build_optimize_pane(commands, fonts, stack);
    build_destination_pane(commands, fonts, stack, init);
    build_output_pane(commands, fonts, stack);

    let scroll = scroll_area(commands, stack, 400.0);
    let content = commands
        .spawn(Node { flex_grow: 1.0, min_width: Val::Px(0.0), flex_direction: FlexDirection::Column, padding: UiRect::left(Val::Px(18.0)), ..default() })
        .id();
    commands.entity(content).add_child(scroll);

    commands.entity(body).add_children(&[sidebar, content]);
    commands.entity(panel).add_child(body);

    build_progress(commands, fonts, panel);
    build_footer(commands, fonts, panel);
}

// ── Sidebar ──────────────────────────────────────────────────────────────────

/// One sidebar row. Highlights when its section is active; the Files row also
/// carries a count badge so the user sees how many files are queued without
/// leaving whatever section they're on.
fn nav_item(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str, sec: ImportSection) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(34.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(9.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(0.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            NavBtn(sec),
            hover_cursor(),
        ))
        .id();
    bind_bg(commands, row, move |w| {
        let active = w.get_resource::<ImportNav>().is_some_and(|n| n.active == sec);
        if active {
            rgb(accent()).with_alpha(0.18)
        } else if matches!(w.get::<Interaction>(row), Some(Interaction::Hovered) | Some(Interaction::Pressed)) {
            rgb(hover_bg())
        } else {
            Color::NONE
        }
    });

    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 14.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    bind_with(commands, ic, move |w| active_flag(w, sec), |w, t, v: &Active| {
        if let Some(mut c) = w.get_mut::<TextColor>(t) {
            c.0 = rgb(if v.0 { accent() } else { text_muted() });
        }
    });

    let lbl = commands
        .spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_muted())), FocusPolicy::Pass, Node { flex_grow: 1.0, ..default() }))
        .id();
    bind_with(commands, lbl, move |w| active_flag(w, sec), |w, t, v: &Active| {
        if let Some(mut c) = w.get_mut::<TextColor>(t) {
            c.0 = rgb(if v.0 { text_primary() } else { text_muted() });
        }
    });
    commands.entity(row).add_children(&[ic, lbl]);

    // Files-only: a queued-count badge pinned to the right of the row.
    if matches!(sec, ImportSection::Files) {
        let badge = commands
            .spawn((
                Node {
                    min_width: Val::Px(18.0),
                    height: Val::Px(16.0),
                    padding: UiRect::axes(Val::Px(5.0), Val::Px(0.0)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(rgb(accent())),
                FocusPolicy::Pass,
            ))
            .id();
        let bt = commands
            .spawn((Text::new(String::new()), ui_font(&fonts.ui, 10.0), TextColor(Color::WHITE), FocusPolicy::Pass))
            .id();
        bind_text(commands, bt, |w| w.get_resource::<ImportOverlayState>().map(|s| s.pending_files.len()).filter(|n| *n > 0).map(|n| n.to_string()).unwrap_or_default());
        bind_display(commands, badge, |w| w.get_resource::<ImportOverlayState>().is_some_and(|s| !s.pending_files.is_empty()));
        commands.entity(badge).add_child(bt);
        commands.entity(row).add_child(badge);
    }
    row
}

#[derive(PartialEq)]
struct Active(bool);
fn active_flag(w: &World, sec: ImportSection) -> Active {
    Active(w.get_resource::<ImportNav>().is_some_and(|n| n.active == sec))
}

/// True if the queue holds at least one 3D model — the gate for the model-only
/// sidebar sections (Settings / Extract / Optimize) and the model options.
fn queue_has_model(w: &World) -> bool {
    w.get_resource::<ImportOverlayState>().is_some_and(|s| {
        s.pending_files
            .iter()
            .any(|p| renzora_import::formats::detect_format(p).is_some())
    })
}

/// Header label reflecting the queue: uniform-kind queues get a specific title,
/// empty / mixed queues get the generic "Import Assets".
fn import_title(w: &World) -> String {
    use crate::kinds::{detect_kind, AssetKind};
    let Some(state) = w.get_resource::<ImportOverlayState>() else {
        return "Import Assets".to_string();
    };
    if state.pending_files.is_empty() {
        return "Import Assets".to_string();
    }
    let kinds: Vec<AssetKind> = state.pending_files.iter().filter_map(|p| detect_kind(p)).collect();
    let first = kinds.first().copied();
    let uniform = first.is_some_and(|k| kinds.iter().all(|&x| x == k));
    match first.filter(|_| uniform) {
        Some(AssetKind::Model) => "Import 3D Models",
        Some(AssetKind::Image) => "Import Images",
        Some(AssetKind::Audio) => "Import Audio",
        Some(AssetKind::Scene) => "Import Scenes",
        Some(AssetKind::Particle) => "Import Particles",
        Some(AssetKind::Material) => "Import Materials",
        Some(AssetKind::Font) => "Import Fonts",
        Some(AssetKind::Script) => "Import Scripts",
        None => "Import Assets",
    }
    .to_string()
}

/// Keep the active section valid: if a model-only section is showing but the
/// queue no longer has a model (the user removed the last one), fall back to
/// Files so the content pane never shows options that don't apply.
fn enforce_nav_section(state: Option<Res<ImportOverlayState>>, nav: Option<ResMut<ImportNav>>) {
    let (Some(state), Some(mut nav)) = (state, nav) else { return };
    let model_only = matches!(
        nav.active,
        ImportSection::Settings | ImportSection::Extract | ImportSection::Optimize
    );
    if model_only
        && !state
            .pending_files
            .iter()
            .any(|p| renzora_import::formats::detect_format(p).is_some())
    {
        nav.active = ImportSection::Files;
    }
}

fn nav_click(q: Query<(&Interaction, &NavBtn), Changed<Interaction>>, mut nav: Option<ResMut<ImportNav>>) {
    let Some(nav) = nav.as_mut() else { return };
    for (i, b) in &q {
        if *i == Interaction::Pressed && nav.active != b.0 {
            nav.active = b.0;
        }
    }
}

// ── Panes ────────────────────────────────────────────────────────────────────

/// A content pane shown only while its section is active.
fn pane(commands: &mut Commands, sec: ImportSection) -> Entity {
    let p = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(12.0), ..default() })
        .id();
    bind_display(commands, p, move |w| w.get_resource::<ImportNav>().is_some_and(|n| n.active == sec));
    p
}

/// A pane's title + one-line description, used at the top of every section.
fn pane_header(commands: &mut Commands, fonts: &EmberFonts, title: &str, subtitle: &str) -> Entity {
    let col = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), margin: UiRect::bottom(Val::Px(2.0)), ..default() }).id();
    let t = commands.spawn((Text::new(title.to_string()), ui_font(&fonts.ui, 15.0), TextColor(rgb(text_primary())))).id();
    let s = commands.spawn((Text::new(subtitle.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())))).id();
    commands.entity(col).add_children(&[t, s]);
    col
}

fn build_files_pane(commands: &mut Commands, fonts: &EmberFonts, parent: Entity) {
    let p = pane(commands, ImportSection::Files);
    let head = pane_header(commands, fonts, "Files", "Add the files you want to import — models, images, audio, and more.");

    // Dropzone — the empty state is this panel, so there's no separate "no
    // files" hint. Only the Browse button is clickable; the card itself is
    // passive (real OS drag-drop is handled by the egui orchestration layer).
    let dz = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(116.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(8.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id();
    let cloud = icon_text(commands, &fonts.phosphor, "cloud-arrow-down", text_muted(), 30.0);
    let dz_t = txt(commands, fonts, "Drag & drop files here", 12.0, text_primary());
    let browse = pill_button(commands, fonts, "folder-open", "Browse files");
    commands.entity(browse).insert(FileBrowseBtn);
    commands.entity(dz).add_children(&[cloud, dz_t, browse]);

    // Queued-file list.
    let list = commands.spawn((Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() }, FilesContainer)).id();
    keyed_list(commands, list, files_snapshot);
    let list_scroll = scroll_area(commands, list, 150.0);

    commands.entity(p).add_children(&[head, dz, list_scroll]);
    commands.entity(parent).add_child(p);
}

fn build_settings_pane(commands: &mut Commands, fonts: &EmberFonts, parent: Entity, init: &Init) {
    let p = pane(commands, ImportSection::Settings);
    let head = pane_header(commands, fonts, "Import Settings", "Scale, axis orientation and mesh fix-ups.");

    let scale = drag_value(commands, &fonts.ui, "", text_primary(), init.scale, 0.01);
    bind_2way(commands, scale, |w| g_settings(w, |s| s.scale), |w, v: &f32| s_settings(w, |s| s.scale = (*v).clamp(0.001, 1000.0)));
    let scale_row = field_row(commands, fonts, "Scale", scale);

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
    let axis_row = field_row(commands, fonts, "Up axis", axis);

    let flip = toggle_row(commands, fonts, "Flip UVs", |s| s.flip_uvs, |s, v| s.flip_uvs = v);
    let normals = toggle_row(commands, fonts, "Generate normals if missing", |s| s.generate_normals, |s, v| s.generate_normals = v);

    commands.entity(p).add_children(&[head, scale_row, axis_row, flip, normals]);
    commands.entity(parent).add_child(p);
}

fn build_extract_pane(commands: &mut Commands, fonts: &EmberFonts, parent: Entity) {
    let p = pane(commands, ImportSection::Extract);
    let head = pane_header(commands, fonts, "Extract", "Pull sub-assets out into their own project files.");
    let a = toggle_row(commands, fonts, "Skeleton + skin weights", |s| s.extract_skeleton, |s, v| s.extract_skeleton = v);
    let b = toggle_row(commands, fonts, "Animations  →  animations/*.anim", |s| s.extract_animations, |s, v| s.extract_animations = v);
    let c = toggle_row(commands, fonts, "Textures  →  textures/*.png", |s| s.extract_textures, |s, v| s.extract_textures = v);
    let d = toggle_row(commands, fonts, "Materials  →  materials/*.material", |s| s.extract_materials, |s, v| s.extract_materials = v);
    commands.entity(p).add_children(&[head, a, b, c, d]);
    commands.entity(parent).add_child(p);
}

fn build_optimize_pane(commands: &mut Commands, fonts: &EmberFonts, parent: Entity) {
    let p = pane(commands, ImportSection::Optimize);
    let head = pane_header(commands, fonts, "Mesh Optimization", "Reorder vertices for faster GPU rendering.");
    let a = toggle_row(commands, fonts, "Optimize vertex cache", |s| s.optimize_vertex_cache, |s, v| s.optimize_vertex_cache = v);
    let b = toggle_row(commands, fonts, "Optimize overdraw", |s| s.optimize_overdraw, |s, v| s.optimize_overdraw = v);
    let c = toggle_row(commands, fonts, "Optimize vertex fetch", |s| s.optimize_vertex_fetch, |s, v| s.optimize_vertex_fetch = v);
    commands.entity(p).add_children(&[head, a, b, c]);
    commands.entity(parent).add_child(p);
}

fn build_destination_pane(commands: &mut Commands, fonts: &EmberFonts, parent: Entity, init: &Init) {
    let p = pane(commands, ImportSection::Destination);
    let head = pane_header(commands, fonts, "Destination", "Pick the project folder to import into.");

    // Folder picker: the project's own directory tree (mirrors the marketplace
    // install flow). Clicking a row sets `target_directory` to that
    // project-relative path. The project root is the first, always-present row.
    let tree = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(1.0), ..default() })
        .id();
    let mut rows = vec![dest_folder_row(commands, fonts, String::new(), 0, "assets (project root)")];
    for (rel, depth, name) in &init.dest_folders {
        rows.push(dest_folder_row(commands, fonts, rel.clone(), *depth + 1, name));
    }
    commands.entity(tree).add_children(&rows);
    let tree_scroll = scroll_area(commands, tree, 150.0);
    let tree_box = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                overflow: Overflow::clip(),
                padding: UiRect::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id();
    commands.entity(tree_box).add_child(tree_scroll);

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

    commands.entity(p).add_children(&[head, tree_box, org, radios]);
    commands.entity(parent).add_child(p);
}

fn build_output_pane(commands: &mut Commands, fonts: &EmberFonts, parent: Entity) {
    let p = pane(commands, ImportSection::Output);
    let head = pane_header(commands, fonts, "Output", "Per-file results from the last import.");
    let list = commands.spawn((Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() }, LogContainer)).id();
    keyed_list(commands, list, log_snapshot);
    let scroll = scroll_area(commands, list, 300.0);
    let frame = commands
        .spawn((Node { width: Val::Percent(100.0), padding: UiRect::all(Val::Px(8.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() }, BackgroundColor(rgb(section_bg())), BorderColor::all(rgb(border()))))
        .id();
    commands.entity(frame).add_child(scroll);
    commands.entity(p).add_children(&[head, frame]);
    commands.entity(parent).add_child(p);
}

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
fn scan_dest_dirs(root: &std::path::Path) -> Vec<(String, usize, String)> {
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

fn build_progress(commands: &mut Commands, fonts: &EmberFonts, parent: Entity) {
    // A slim status strip above the footer. The modal normally hands an import
    // off to the corner toast on Import, so this only shows if an import is
    // somehow still running with the modal open — kept for honest feedback.
    let sec = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), margin: UiRect::top(Val::Px(12.0)), ..default() }).id();

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
    // Whole strip hidden while idle, so the footer hugs the body.
    bind_display(commands, sec, |w| !matches!(w.get_resource::<ImportOverlayState>().map(|s| &s.progress), Some(ImportProgress::Idle) | None));
    commands.entity(parent).add_child(sec);
}

fn build_footer(commands: &mut Commands, fonts: &EmberFonts, panel: Entity) {
    let row = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::FlexEnd, column_gap: Val::Px(8.0), margin: UiRect::top(Val::Px(16.0)), ..default() }).id();
    let cancel = wide_button(commands, fonts, "Cancel", rgb(section_bg()), text_primary(), 80.0);
    commands.entity(cancel).insert(CancelBtn);
    let import = commands
        .spawn((
            Node { min_width: Val::Px(110.0), height: Val::Px(32.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::Center, column_gap: Val::Px(6.0), border_radius: BorderRadius::all(Val::Px(6.0)), ..default() },
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

// ── Keyed list (files) ─────────────────────────────────────────────────────────

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
                // Dismiss the modal and hand progress off to the corner toast.
                // We keep `active_task` / `progress` / `pending_files` intact so
                // the toast system can keep polling and rendering the bar.
                let mut s = w.resource_mut::<ImportOverlayState>();
                s.visible = false;
                s.toast_active = true;
                s.toast_dismiss_at = None;
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
        commands.queue(|w: &mut World| { pick_and_queue_files(w); });
    }
}

/// Open the OS file picker (filtered to every importable kind) and append the
/// chosen files to the queue. Returns `true` if at least one new file was added.
/// Shared by the asset-browser Import trigger (`lib.rs`) and the overlay's own
/// **Browse files** button, so both honour the same filter and de-dup rules.
pub(crate) fn pick_and_queue_files(world: &mut World) -> bool {
    let Some(paths) = crate::kinds::pick_importable_files() else {
        return false;
    };
    let mut state = world.resource_mut::<ImportOverlayState>();
    let was_empty = state.pending_files.is_empty();
    let mut added = false;
    for p in &paths {
        if !state.pending_files.contains(p) {
            state.pending_files.push(p.clone());
            added = true;
        }
    }
    // Auto-detect unit scale from the first model in a fresh queue (no-op for
    // non-model picks — `detect_unit_scale` returns None for those).
    if was_empty && state.settings.scale == 1.0 {
        if let Some(scale) = paths.first().and_then(|p| renzora_import::units::detect_unit_scale(p)) {
            state.settings.scale = scale;
        }
    }
    added
}

/// Click a destination folder row → it becomes the import target directory.
fn dest_folder_click(q: Query<(&Interaction, &DestFolderRow), Changed<Interaction>>, mut state: Option<ResMut<ImportOverlayState>>) {
    let Some(state) = state.as_mut() else { return };
    for (i, row) in &q {
        if *i == Interaction::Pressed && state.target_directory != row.0 {
            state.target_directory = row.0.clone();
        }
    }
}

// ── Corner progress toast ──────────────────────────────────────────────────────

/// Owns the corner progress toast: polls the running import, spawns/despawns the
/// toast entity, and auto-dismisses a few seconds after the import finishes.
fn manage_import_toast(world: &mut World) {
    let active = world.resource::<ImportOverlayState>().toast_active;
    if active {
        poll_import_task(world); // keep the bar moving while the modal is closed
    }

    // Once the import reaches a terminal state, arm a short auto-dismiss timer
    // so the success/error toast lingers briefly before clearing itself.
    if active {
        let terminal = matches!(
            world.resource::<ImportOverlayState>().progress,
            ImportProgress::Done(_) | ImportProgress::Error(_)
        );
        if terminal {
            let now = world.resource::<Time>().elapsed_secs_f64();
            let dismiss_at = world.resource::<ImportOverlayState>().toast_dismiss_at;
            match dismiss_at {
                None => world.resource_mut::<ImportOverlayState>().toast_dismiss_at = Some(now + 5.0),
                Some(t) if now >= t => {
                    let mut s = world.resource_mut::<ImportOverlayState>();
                    s.toast_active = false;
                    s.toast_dismiss_at = None;
                    s.progress = ImportProgress::Idle;
                    s.pending_files.clear();
                    s.log_entries.clear();
                }
                _ => {}
            }
        }
    }

    let want = world.resource::<ImportOverlayState>().toast_active;
    let mut q = world.query_filtered::<Entity, With<ToastRoot>>();
    let existing: Vec<Entity> = q.iter(world).collect();
    if want && existing.is_empty() {
        let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            spawn_toast(&mut commands, &fonts);
        }
        queue.apply(world);
    } else if !want && !existing.is_empty() {
        for e in existing {
            world.entity_mut(e).despawn();
        }
    }
}

fn spawn_toast(commands: &mut Commands, fonts: &EmberFonts) {
    // Fixed bottom-right card, above the viewport chrome.
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(16.0),
                bottom: Val::Px(16.0),
                width: Val::Px(320.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            BorderColor::all(rgb(border())),
            GlobalZIndex(9200),
            OverlaySurface,
            ToastRoot,
            Name::new("import-toast"),
        ))
        .id();

    // Header: title + dismiss ×.
    let header = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, ..default() }).id();
    let title = icon_label(commands, fonts, "download-simple", "Importing assets", text_primary(), 12.0);
    let close = commands.spawn((Node { padding: UiRect::all(Val::Px(2.0)), ..default() }, Interaction::default(), ToastDismissBtn, hover_cursor())).id();
    let close_x = icon_text(commands, &fonts.phosphor, "x", text_muted(), 13.0);
    commands.entity(close_x).insert(FocusPolicy::Pass);
    commands.entity(close).add_child(close_x);
    commands.entity(header).add_children(&[title, close]);
    commands.entity(root).add_child(header);

    // Working: label + progress bar.
    let working = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() }).id();
    let toprow = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).id();
    let spin = spinner(commands);
    let plabel = txt(commands, fonts, "", 11.0, text_muted());
    bind_text(commands, plabel, |w| match w.get_resource::<ImportOverlayState>().map(|s| s.progress.clone()) {
        Some(ImportProgress::Working { current, total, label }) => format!("[{current}/{total}] {label}"),
        _ => "Starting…".to_string(),
    });
    commands.entity(toprow).add_children(&[spin, plabel]);
    let track = commands.spawn((Node { width: Val::Percent(100.0), height: Val::Px(6.0), overflow: Overflow::clip(), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() }, BackgroundColor(rgb(section_bg())))).id();
    let fill = commands.spawn((Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(rgb(accent())))).id();
    bind_with(commands, fill, progress_fraction, |w, target, v: &OrderedF32| { if let Some(mut n) = w.get_mut::<Node>(target) { n.width = Val::Percent((v.0 * 100.0).clamp(0.0, 100.0)); } });
    commands.entity(track).add_child(fill);
    commands.entity(working).add_children(&[toprow, track]);
    bind_display(commands, working, |w| matches!(w.get_resource::<ImportOverlayState>().map(|s| &s.progress), Some(ImportProgress::Working { .. }) | Some(ImportProgress::Idle)));
    commands.entity(root).add_child(working);

    // Done / Error result lines.
    let (done, done_msg) = icon_msg(commands, fonts, "check-circle", GREEN);
    bind_text(commands, done_msg, |w| match w.get_resource::<ImportOverlayState>().map(|s| s.progress.clone()) {
        Some(ImportProgress::Done(m)) => m,
        _ => String::new(),
    });
    bind_display(commands, done, |w| matches!(w.get_resource::<ImportOverlayState>().map(|s| &s.progress), Some(ImportProgress::Done(_))));
    commands.entity(root).add_child(done);

    let (err, err_msg) = icon_msg(commands, fonts, "warning", RED);
    bind_text(commands, err_msg, |w| match w.get_resource::<ImportOverlayState>().map(|s| s.progress.clone()) {
        Some(ImportProgress::Error(m)) => m,
        _ => String::new(),
    });
    bind_display(commands, err, |w| matches!(w.get_resource::<ImportOverlayState>().map(|s| &s.progress), Some(ImportProgress::Error(_))));
    commands.entity(root).add_child(err);
}

fn toast_dismiss_click(q: Query<&Interaction, (With<ToastDismissBtn>, Changed<Interaction>)>, mut state: Option<ResMut<ImportOverlayState>>) {
    let Some(state) = state.as_mut() else { return };
    if q.iter().any(|i| *i == Interaction::Pressed) {
        // Hide the toast. A still-running import keeps writing in the background;
        // dismissing only removes the notification.
        state.toast_active = false;
        state.toast_dismiss_at = None;
        if matches!(state.progress, ImportProgress::Done(_) | ImportProgress::Error(_)) {
            state.progress = ImportProgress::Idle;
            state.pending_files.clear();
            state.log_entries.clear();
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn hover_cursor() -> renzora_ember::cursor_icon::HoverCursor {
    renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer)
}

fn txt(commands: &mut Commands, fonts: &EmberFonts, s: &str, size: f32, color: (u8, u8, u8)) -> Entity {
    commands.spawn((Text::new(s.to_string()), ui_font(&fonts.ui, size), TextColor(rgb(color)))).id()
}

fn row_between(commands: &mut Commands) -> Entity {
    commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, ..default() }).id()
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

/// A settings row: a left-aligned label and a right-aligned control.
fn field_row(commands: &mut Commands, fonts: &EmberFonts, label: &str, control: Entity) -> Entity {
    let row = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, column_gap: Val::Px(12.0), min_height: Val::Px(26.0), ..default() }).id();
    let t = txt(commands, fonts, label, 12.0, text_primary());
    commands.entity(row).add_children(&[t, control]);
    row
}

/// A boolean settings row: label on the left, checkbox on the right.
fn toggle_row(commands: &mut Commands, fonts: &EmberFonts, label: &str, get: fn(&renzora_import::settings::ImportSettings) -> bool, set: fn(&mut renzora_import::settings::ImportSettings, bool)) -> Entity {
    let cb = checkbox(commands, false);
    bind_2way(commands, cb, move |w| g_settings(w, get), move |w, v: &bool| s_settings(w, |s| set(s, *v)));
    field_row(commands, fonts, label, cb)
}

fn pill_button(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str) -> Entity {
    let btn = commands
        .spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)), border_radius: BorderRadius::all(Val::Px(5.0)), ..default() }, BackgroundColor(rgb(accent())), Interaction::default(), hover_cursor()))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, (255, 255, 255), 12.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 11.0), TextColor(Color::WHITE), FocusPolicy::Pass)).id();
    commands.entity(btn).add_children(&[ic, t]);
    btn
}

fn wide_button(commands: &mut Commands, fonts: &EmberFonts, label: &str, bg: Color, fg: (u8, u8, u8), min_w: f32) -> Entity {
    let btn = commands
        .spawn((Node { min_width: Val::Px(min_w), height: Val::Px(32.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(6.0)), ..default() }, BackgroundColor(bg), Interaction::default(), hover_cursor()))
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
