//! Bevy-native (ember) export overlay — the bevy_ui counterpart to the egui
//! `draw_export_overlay`. Two-column modal: a platform sidebar (with installed/
//! available status dots + release fetch) and a per-platform settings pane
//! (packaging, compression, mesh-opt, window, options, plugins, icon), plus
//! output dir, progress, and the Export button. Edits the same
//! [`ExportOverlayState`] and reuses the worker (`run_export` + the pollers).
//! Renders only under the BevyUi backend; egui renders under Egui.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora::core::WindowMode;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::{bind_2way, bind_bg, bind_display, bind_text};
use renzora_ember::theme::*;
use renzora_ember::widgets::{bind_text_input, checkbox, drag_value, radio_group, scroll_area, spinner, text_input, OverlaySurface};

use crate::download::{self, DownloadProgress};
use crate::overlay::{ensure_release_fetch, poll_download_task, poll_export_task, poll_release_fetch, run_export, ExportOverlayState, ExportProgress, PackagingMode};
use crate::templates::{Platform, TemplateManager};

const GREEN: (u8, u8, u8) = (89, 191, 115);
const AMBER: (u8, u8, u8) = (242, 166, 64);
const RED: (u8, u8, u8) = (239, 68, 68);

pub(crate) fn register(app: &mut App) {
    app.add_systems(
        Update,
        (
            manage_export_modal,
            rebuild_right_pane,
            platform_click,
            output_browse_click,
            icon_browse_click,
            icon_clear_click,
            download_click,
            install_click,
            export_click,
            close_click,
        ),
    );
}

// ── Markers ──────────────────────────────────────────────────────────────────

#[derive(Component)]
struct ExportRoot;
#[derive(Component)]
struct RightPane {
    sig: Option<u8>,
}
#[derive(Component, Clone, Copy)]
struct PlatformBtn(Platform);
#[derive(Component)]
struct OutputBrowseBtn;
#[derive(Component)]
struct IconBrowseBtn;
#[derive(Component)]
struct IconClearBtn;
#[derive(Component)]
struct DownloadBtn;
#[derive(Component)]
struct InstallBtn;
#[derive(Component)]
struct ExportBtn;
#[derive(Component)]
struct CloseBtn;

// ── Lifecycle ────────────────────────────────────────────────────────────────

fn manage_export_modal(world: &mut World) {
    let visible = world.get_resource::<ExportOverlayState>().is_some_and(|s| s.visible);
    if visible {
        ensure_release_fetch(world);
        poll_release_fetch(world);
        poll_export_task(world);
        poll_download_task(world);
        scan_plugins(world);
    }
    let mut q = world.query_filtered::<Entity, With<ExportRoot>>();
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

fn scan_plugins(world: &mut World) {
    if world.resource::<ExportOverlayState>().plugins_scanned {
        return;
    }
    let dir = world.resource::<TemplateManager>().runtime_plugins_dir();
    let plugins = dynamic_plugin_loader::scan_plugins(&dir);

    // Pre-select only the plugins a scene actually references, so the export
    // ships just the effects it uses instead of all 50+. A plugin id is the dll
    // stem (e.g. `renzora_matrix`); scenes name components by their defining
    // crate (`renzora_matrix::MatrixSettings`), so we match `<id>::` in the
    // project's `.ron` files. Effects added purely from scripts won't be
    // detected — the user can still tick those manually. If there's no open
    // project / no scenes to scan, fall back to selecting everything.
    let project_root = world
        .get_resource::<renzora::core::CurrentProject>()
        .map(|p| p.path.clone());
    let used = project_root
        .as_deref()
        .and_then(|root| scene_used_plugin_ids(root, &plugins));

    let mut s = world.resource_mut::<ExportOverlayState>();
    for p in &plugins {
        let select = used.as_ref().map_or(true, |set| set.contains(&p.id));
        if select {
            s.selected_plugins.insert(p.id.clone());
        }
    }
    s.available_plugins = plugins;
    s.plugins_scanned = true;
}

/// The plugin ids referenced by any `.ron` scene/prefab under `root`. Matches
/// each plugin's crate prefix (`<id>::`, with a leading `lib` stripped for unix
/// dll names) against the serialized component type paths. Returns `None` if no
/// `.ron` could be read, so the caller falls back to selecting all plugins.
fn scene_used_plugin_ids(
    root: &std::path::Path,
    available: &[dynamic_plugin_loader::DynamicPluginInfo],
) -> Option<std::collections::HashSet<String>> {
    let needles: Vec<(String, String)> = available
        .iter()
        .map(|p| {
            let crate_name = p.id.strip_prefix("lib").unwrap_or(p.id.as_str());
            (p.id.clone(), format!("{crate_name}::"))
        })
        .collect();

    let mut ron_files = Vec::new();
    collect_ron_files(root, &mut ron_files);
    if ron_files.is_empty() {
        return None;
    }

    let mut used = std::collections::HashSet::new();
    for file in &ron_files {
        let Ok(text) = std::fs::read_to_string(file) else { continue };
        for (id, needle) in &needles {
            if !used.contains(id) && text.contains(needle.as_str()) {
                used.insert(id.clone());
            }
        }
    }
    Some(used)
}

/// Recursively collect `.ron` files under `dir`, skipping dot-directories
/// (`.editor`, `.cache`, `.git`, …).
fn collect_ron_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let is_dot = path
                .file_name()
                .and_then(|n| n.to_str())
                .map_or(false, |n| n.starts_with('.'));
            if !is_dot {
                collect_ron_files(&path, out);
            }
        } else if path.extension().and_then(|x| x.to_str()) == Some("ron") {
            out.push(path);
        }
    }
}

struct Init {
    binary_name: String,
    output_dir: String,
}
impl Init {
    fn read(world: &World) -> Self {
        let s = world.resource::<ExportOverlayState>();
        Self { binary_name: s.binary_name.clone(), output_dir: s.output_dir.clone() }
    }
}

fn spawn_modal(commands: &mut Commands, fonts: &EmberFonts, init: &Init, has_project: bool) {
    let backdrop = commands
        .spawn((
            fullscreen(),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.63)),
            GlobalZIndex(9300),
            FocusPolicy::Block,
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            OverlaySurface,
            ExportRoot,
            Name::new("export-modal"),
        ))
        .id();
    let panel = commands
        .spawn((
            Node {
                width: Val::Px(760.0),
                max_height: Val::Vh(86.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(20.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            BorderColor::all(rgb(border())),
            FocusPolicy::Block,
            Name::new("export-panel"),
        ))
        .id();
    commands.entity(backdrop).add_child(panel);

    // Header.
    let header = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, ..default() }).id();
    let title = icon_title(commands, fonts, "package", "Export Project");
    let close = commands.spawn((Node { padding: UiRect::all(Val::Px(2.0)), ..default() }, Interaction::default(), CloseBtn, cursor())).id();
    let cx = icon_text(commands, &fonts.phosphor, "x", text_muted(), 16.0);
    commands.entity(cx).insert(FocusPolicy::Pass);
    commands.entity(close).add_child(cx);
    commands.entity(header).add_children(&[title, close]);
    commands.entity(panel).add_child(header);
    let sep = commands.spawn((Node { width: Val::Percent(100.0), height: Val::Px(1.0), margin: UiRect::vertical(Val::Px(8.0)), ..default() }, BackgroundColor(rgb(divider())))).id();
    commands.entity(panel).add_child(sep);

    if !has_project {
        let w = txt(commands, fonts, "No project open. Open a project before exporting.", 12.0, RED);
        commands.entity(panel).add_child(w);
        return;
    }

    // Two columns.
    let cols = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, column_gap: Val::Px(16.0), flex_grow: 1.0, min_height: Val::Px(0.0), ..default() }).id();
    let sidebar = build_sidebar(commands, fonts);
    let right = commands.spawn((Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), min_width: Val::Px(0.0), ..default() }, RightPane { sig: None })).id();
    let right_scroll = scroll_area(commands, right, 440.0);
    commands.entity(cols).add_children(&[sidebar, right_scroll]);
    commands.entity(panel).add_child(cols);

    // Output.
    build_output(commands, fonts, panel, init);
    // Progress.
    build_progress(commands, fonts, panel);
    // Export button.
    build_export_btn(commands, fonts, panel);
}

// ── Sidebar (platforms) ──────────────────────────────────────────────────────

fn build_sidebar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = commands.spawn(Node { width: Val::Px(180.0), flex_shrink: 0.0, flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() }).id();
    let head = section_label(commands, fonts, "desktop-tower", "Platform");
    commands.entity(col).add_child(head);
    for &p in Platform::ALL {
        let btn = platform_button(commands, fonts, p);
        commands.entity(col).add_child(btn);
    }
    // Release info status.
    let rel = txt(commands, fonts, "", 11.0, text_muted());
    commands.entity(rel).insert(Node { margin: UiRect::top(Val::Px(8.0)), ..default() });
    bind_text(commands, rel, |w| {
        let s = w.resource::<ExportOverlayState>();
        if let Some(err) = &s.release_fetch_error {
            format!("⚠ {err}")
        } else if let Some(info) = &s.release_info {
            format!("Latest: {}", info.tag_name)
        } else if s.release_fetch_started {
            "Loading release…".to_string()
        } else {
            String::new()
        }
    });
    commands.entity(col).add_child(rel);
    col
}

fn platform_button(commands: &mut Commands, fonts: &EmberFonts, p: Platform) -> Entity {
    let btn = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(40.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), padding: UiRect::horizontal(Val::Px(10.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(Color::NONE),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            PlatformBtn(p),
            cursor(),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        let sel = w.get_resource::<ExportOverlayState>().is_some_and(|s| s.platform == p);
        let hov = matches!(w.get::<Interaction>(btn), Some(Interaction::Hovered));
        if sel { rgb(section_bg()) } else if hov { ca(255, 255, 255, 10) } else { Color::NONE }
    });
    let ic = icon_text(commands, &fonts.phosphor, platform_icon(p), text_primary(), 18.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let nm = commands.spawn((Text::new(p.display_name().to_string()), ui_font(&fonts.ui, 12.5), TextColor(rgb(text_primary())), FocusPolicy::Pass, Node { flex_grow: 1.0, ..default() }, bevy::text::TextLayout::new_with_no_wrap())).id();
    let dot = commands.spawn((Node { width: Val::Px(8.0), height: Val::Px(8.0), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() }, BackgroundColor(rgb(text_muted())), FocusPolicy::Pass)).id();
    bind_bg(commands, dot, move |w| {
        let installed = w.get_resource::<TemplateManager>().is_some_and(|t| t.is_installed(p));
        let available = w.get_resource::<ExportOverlayState>().and_then(|s| s.release_info.as_ref().map(|r| r.available_platforms.contains(&p))).unwrap_or(false);
        if installed { rgb(GREEN) } else if available { rgb(AMBER) } else { ca(130, 138, 160, 150) }
    });
    commands.entity(btn).add_children(&[ic, nm, dot]);
    btn
}

// ── Right pane (per-platform, rebuilt on platform change) ────────────────────

fn rebuild_right_pane(world: &mut World) {
    if world.query_filtered::<(), With<ExportRoot>>().iter(world).next().is_none() {
        return;
    }
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
    let platform = world.resource::<ExportOverlayState>().platform;
    let sig = Platform::ALL.iter().position(|p| *p == platform).unwrap_or(0) as u8;

    let mut q = world.query::<(Entity, &RightPane)>();
    let Some((pane, old)) = q.iter(world).map(|(e, r)| (e, r.sig)).next() else { return };
    if old == Some(sig) {
        return;
    }
    let kids: Vec<Entity> = world.get::<Children>(pane).map(|c| c.iter().collect()).unwrap_or_default();
    let mut queue = CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        for k in kids {
            commands.entity(k).despawn();
        }
        build_settings(&mut commands, &fonts, pane, platform);
    }
    queue.apply(world);
    if let Some(mut r) = world.get_mut::<RightPane>(pane) {
        r.sig = Some(sig);
    }
}

fn build_settings(commands: &mut Commands, fonts: &EmberFonts, pane: Entity, p: Platform) {
    let desktop = matches!(p, Platform::WindowsX64 | Platform::LinuxX64 | Platform::MacOSX64 | Platform::MacOSArm64);

    // Platform header.
    let hdr = commands.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), margin: UiRect::bottom(Val::Px(4.0)), ..default() }).id();
    let title = icon_title(commands, fonts, platform_icon(p), p.display_name());
    let sub = txt(commands, fonts, p.supported_devices(), 11.0, text_muted());
    commands.entity(hdr).add_children(&[title, sub]);
    commands.entity(pane).add_child(hdr);

    // Runtime template status.
    build_runtime_status(commands, fonts, pane, p);

    // Packaging (desktop).
    if desktop {
        let sec = section(commands);
        let h = section_label(commands, fonts, "file-archive", "Packaging");
        let radios = radio_group(commands, &fonts.ui, &["Binary + .rpak", "Single executable"], 0);
        bind_2way(
            commands,
            radios,
            |w| if matches!(w.resource::<ExportOverlayState>().packaging_mode, PackagingMode::SingleBinary) { 1usize } else { 0 },
            |w, v: &usize| w.resource_mut::<ExportOverlayState>().packaging_mode = if *v == 1 { PackagingMode::SingleBinary } else { PackagingMode::SeparateFiles },
        );
        commands.entity(sec).add_children(&[h, radios]);
        commands.entity(pane).add_child(sec);
    }

    // Compression.
    {
        let row = labeled(commands, fonts, "Compression:");
        let dv = drag_value(commands, &fonts.ui, "", text_primary(), 0.0, 1.0);
        bind_2way(commands, dv, |w| w.resource::<ExportOverlayState>().compression_level as f32, |w, v: &f32| w.resource_mut::<ExportOverlayState>().compression_level = (v.round() as i32).clamp(1, 22));
        commands.entity(row).add_child(dv);
        commands.entity(pane).add_child(row);
    }

    // Mesh optimization.
    {
        let sec = section(commands);
        let h = section_label(commands, fonts, "cube", "Mesh Optimization");
        commands.entity(sec).add_child(h);
        let simplify = check_state(commands, fonts, "Simplify meshes", |s| s.mesh_simplify, |s, v| s.mesh_simplify = v);
        commands.entity(sec).add_child(simplify);
        let ratio = labeled(commands, fonts, "Keep ratio:");
        let dv = drag_value(commands, &fonts.ui, "", text_primary(), 0.0, 0.01);
        bind_2way(commands, dv, |w| w.resource::<ExportOverlayState>().mesh_simplify_ratio, |w, v: &f32| w.resource_mut::<ExportOverlayState>().mesh_simplify_ratio = v.clamp(0.1, 1.0));
        commands.entity(ratio).add_child(dv);
        commands.entity(ratio).insert(Node { margin: UiRect::left(Val::Px(20.0)), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() });
        bind_display(commands, ratio, |w| w.resource::<ExportOverlayState>().mesh_simplify);
        commands.entity(sec).add_child(ratio);
        let quant = check_state(commands, fonts, "Quantize vertex attributes", |s| s.mesh_quantize, |s, v| s.mesh_quantize = v);
        let lods = check_state(commands, fonts, "Generate LODs", |s| s.mesh_generate_lods, |s, v| s.mesh_generate_lods = v);
        commands.entity(sec).add_children(&[quant, lods]);
        let levels = labeled(commands, fonts, "Levels:");
        let dv2 = drag_value(commands, &fonts.ui, "", text_primary(), 0.0, 1.0);
        bind_2way(commands, dv2, |w| w.resource::<ExportOverlayState>().mesh_lod_levels as f32, |w, v: &f32| w.resource_mut::<ExportOverlayState>().mesh_lod_levels = (v.round() as u32).clamp(1, 5));
        commands.entity(levels).add_child(dv2);
        commands.entity(levels).insert(Node { margin: UiRect::left(Val::Px(20.0)), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() });
        bind_display(commands, levels, |w| w.resource::<ExportOverlayState>().mesh_generate_lods);
        commands.entity(sec).add_child(levels);
        commands.entity(pane).add_child(sec);
    }

    // Window (desktop).
    if desktop {
        let sec = section(commands);
        let h = section_label(commands, fonts, "monitor", "Window");
        let radios = radio_group(commands, &fonts.ui, &["Windowed", "Fullscreen", "Borderless"], 0);
        bind_2way(
            commands,
            radios,
            |w| match w.resource::<ExportOverlayState>().window_mode {
                WindowMode::Fullscreen => 1usize,
                WindowMode::Borderless => 2,
                _ => 0,
            },
            |w, v: &usize| w.resource_mut::<ExportOverlayState>().window_mode = match v { 1 => WindowMode::Fullscreen, 2 => WindowMode::Borderless, _ => WindowMode::Windowed },
        );
        commands.entity(sec).add_children(&[h, radios]);
        let size = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }).id();
        let szl = txt(commands, fonts, "Size:", 12.0, text_muted());
        let dw = drag_value(commands, &fonts.ui, "", text_primary(), 0.0, 10.0);
        bind_2way(commands, dw, |w| w.resource::<ExportOverlayState>().window_width as f32, |w, v: &f32| w.resource_mut::<ExportOverlayState>().window_width = (v.round() as u32).clamp(320, 7680));
        let xl = txt(commands, fonts, "x", 12.0, text_muted());
        let dh = drag_value(commands, &fonts.ui, "", text_primary(), 0.0, 10.0);
        bind_2way(commands, dh, |w| w.resource::<ExportOverlayState>().window_height as f32, |w, v: &f32| w.resource_mut::<ExportOverlayState>().window_height = (v.round() as u32).clamp(240, 4320));
        commands.entity(size).add_children(&[szl, dw, xl, dh]);
        bind_display(commands, size, |w| matches!(w.resource::<ExportOverlayState>().window_mode, WindowMode::Windowed));
        commands.entity(sec).add_child(size);
        commands.entity(pane).add_child(sec);
    }

    // Options.
    {
        let sec = section(commands);
        let h = section_label(commands, fonts, "gear", "Options");
        let console = check_state(commands, fonts, "Console logging", |s| s.console_logging, |s, v| s.console_logging = v);
        commands.entity(sec).add_children(&[h, console]);
        if desktop && p.supports_dedicated_server() {
            let server = check_state(commands, fonts, "Include dedicated server", |s| s.include_server, |s, v| s.include_server = v);
            commands.entity(sec).add_child(server);
        }
        commands.entity(pane).add_child(sec);
    }

    // Plugins.
    build_plugins(commands, fonts, pane);

    // Icon.
    build_icon(commands, fonts, pane);
}

fn build_runtime_status(commands: &mut Commands, fonts: &EmberFonts, pane: Entity, _p: Platform) {
    let sec = commands.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), margin: UiRect::bottom(Val::Px(4.0)), ..default() }).id();
    // Installed / not status line.
    let (line, msg) = icon_msg(commands, fonts, "check-circle", text_muted());
    let p = _p;
    bind_text(commands, msg, move |w| if w.get_resource::<TemplateManager>().is_some_and(|t| t.is_installed(p)) { "Runtime template installed".to_string() } else { "Runtime template not installed".to_string() });
    commands.entity(sec).add_child(line);
    // Buttons.
    let btns = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }).id();
    let dl = pill_button(commands, fonts, "download-simple", "Download from GitHub");
    commands.entity(dl).insert(DownloadBtn);
    let inst = pill_button(commands, fonts, "folder-open", "Install from file...");
    commands.entity(inst).insert(InstallBtn);
    commands.entity(btns).add_children(&[dl, inst]);
    commands.entity(sec).add_child(btns);
    // Download progress.
    let (prog, pmsg) = icon_msg(commands, fonts, "spinner", text_muted());
    bind_text(commands, pmsg, move |w| match w.get_resource::<ExportOverlayState>().and_then(|s| s.download_status.clone()) {
        Some((dp, DownloadProgress::Fetching(m))) if dp == p => m,
        Some((dp, DownloadProgress::Done(m))) if dp == p => m,
        Some((dp, DownloadProgress::Error(m))) if dp == p => m,
        _ => String::new(),
    });
    bind_display(commands, prog, move |w| w.get_resource::<ExportOverlayState>().and_then(|s| s.download_status.as_ref().map(|(dp, _)| *dp == p)).unwrap_or(false));
    commands.entity(sec).add_child(prog);
    commands.entity(pane).add_child(sec);
}

fn build_plugins(commands: &mut Commands, fonts: &EmberFonts, pane: Entity) {
    // Built from a one-shot read; the plugin list is stable after scan.
    let sec = section(commands);
    let h = section_label(commands, fonts, "puzzle-piece", "Plugins");
    commands.entity(sec).add_child(h);
    let list = commands.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() }).id();
    let scroll = scroll_area(commands, list, 160.0);
    commands.entity(sec).add_child(scroll);
    // Filled by a command that can read the world.
    commands.queue(move |world: &mut World| {
        let plugins: Vec<(String, String)> = world.get_resource::<ExportOverlayState>().map(|s| s.available_plugins.iter().map(|p| (p.id.clone(), format!("{:?}", p.scope))).collect()).unwrap_or_default();
        let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
        let mut queue = CommandQueue::default();
        {
            let mut c = Commands::new(&mut queue, world);
            if plugins.is_empty() {
                c.entity(sec).insert(Node { display: Display::None, ..default() });
            }
            for (id, scope) in plugins {
                let row = c.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).id();
                let cb = checkbox(&mut c, true);
                let id2 = id.clone();
                bind_2way(&mut c, cb, move |w| w.get_resource::<ExportOverlayState>().is_some_and(|s| s.selected_plugins.contains(&id2)), {
                    let id3 = id.clone();
                    move |w, v: &bool| {
                        if let Some(mut s) = w.get_resource_mut::<ExportOverlayState>() {
                            if *v { s.selected_plugins.insert(id3.clone()); } else { s.selected_plugins.remove(&id3); }
                        }
                    }
                });
                let t = c.spawn((Text::new(format!("{id} ({scope})")), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())))).id();
                c.entity(row).add_children(&[cb, t]);
                c.entity(list).add_child(row);
            }
        }
        queue.apply(world);
    });
    commands.entity(pane).add_child(sec);
}

fn build_icon(commands: &mut Commands, fonts: &EmberFonts, pane: Entity) {
    let row = commands.spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), margin: UiRect::top(Val::Px(4.0)), ..default() }, Name::new("icon-row"))).id();
    let lbl = txt(commands, fonts, "Icon:", 12.0, text_muted());
    let path = commands.spawn((Text::new("None".to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), Node { flex_grow: 1.0, ..default() }, bevy::text::TextLayout::new_with_no_wrap())).id();
    bind_text(commands, path, |w| w.get_resource::<ExportOverlayState>().and_then(|s| s.icon_path.clone()).unwrap_or_else(|| "None".to_string()));
    let clear = commands.spawn((Node { padding: UiRect::all(Val::Px(2.0)), ..default() }, Interaction::default(), IconClearBtn, cursor())).id();
    let clx = icon_text(commands, &fonts.phosphor, "x", text_muted(), 12.0);
    commands.entity(clx).insert(FocusPolicy::Pass);
    commands.entity(clear).add_child(clx);
    bind_display(commands, clear, |w| w.get_resource::<ExportOverlayState>().is_some_and(|s| s.icon_path.is_some()));
    let browse = pill_button(commands, fonts, "image", "Browse");
    commands.entity(browse).insert(IconBrowseBtn);
    commands.entity(row).add_children(&[lbl, path, clear, browse]);
    commands.entity(pane).add_child(row);
}

// ── Output / progress / export button ────────────────────────────────────────

fn build_output(commands: &mut Commands, fonts: &EmberFonts, panel: Entity, init: &Init) {
    let sec = section(commands);
    let h = section_label(commands, fonts, "folder-open", "Output");
    let name_row = labeled(commands, fonts, "Name:");
    let name = text_input(commands, &fonts.ui, "Binary name", &init.binary_name);
    style_input(commands, name);
    bind_text_input(commands, name, |w| w.get_resource::<ExportOverlayState>().map(|s| s.binary_name.clone()).unwrap_or_default(), |w, v| { if let Some(mut s) = w.get_resource_mut::<ExportOverlayState>() { s.binary_name = v; } });
    commands.entity(name_row).add_child(name);
    let dir_row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }).id();
    let dir = text_input(commands, &fonts.ui, "Export directory...", &init.output_dir);
    style_input(commands, dir);
    bind_text_input(commands, dir, |w| w.get_resource::<ExportOverlayState>().map(|s| s.output_dir.clone()).unwrap_or_default(), |w, v| { if let Some(mut s) = w.get_resource_mut::<ExportOverlayState>() { s.output_dir = v; } });
    let browse = pill_button(commands, fonts, "folder", "Browse");
    commands.entity(browse).insert(OutputBrowseBtn);
    commands.entity(dir_row).add_children(&[dir, browse]);
    commands.entity(sec).add_children(&[h, name_row, dir_row]);
    commands.entity(sec).insert(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), margin: UiRect::top(Val::Px(8.0)), ..default() });
    commands.entity(panel).add_child(sec);
}

fn build_progress(commands: &mut Commands, fonts: &EmberFonts, panel: Entity) {
    let sec = commands.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), margin: UiRect::top(Val::Px(8.0)), ..default() }).id();
    let working = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).id();
    let sp = spinner(commands);
    let wl = txt(commands, fonts, "", 12.0, text_muted());
    bind_text(commands, wl, |w| match w.get_resource::<ExportOverlayState>().map(|s| s.progress.clone()) { Some(ExportProgress::Working(m)) => m, _ => String::new() });
    commands.entity(working).add_children(&[sp, wl]);
    bind_display(commands, working, |w| matches!(w.get_resource::<ExportOverlayState>().map(|s| &s.progress), Some(ExportProgress::Working(_))));
    let (done, dmsg) = icon_msg(commands, fonts, "check-circle", GREEN);
    bind_text(commands, dmsg, |w| match w.get_resource::<ExportOverlayState>().map(|s| s.progress.clone()) { Some(ExportProgress::Done(m)) => m, _ => String::new() });
    bind_display(commands, done, |w| matches!(w.get_resource::<ExportOverlayState>().map(|s| &s.progress), Some(ExportProgress::Done(_))));
    let (err, emsg) = icon_msg(commands, fonts, "warning", RED);
    bind_text(commands, emsg, |w| match w.get_resource::<ExportOverlayState>().map(|s| s.progress.clone()) { Some(ExportProgress::Error(m)) => m, _ => String::new() });
    bind_display(commands, err, |w| matches!(w.get_resource::<ExportOverlayState>().map(|s| &s.progress), Some(ExportProgress::Error(_))));
    commands.entity(sec).add_children(&[working, done, err]);
    commands.entity(panel).add_child(sec);
}

fn build_export_btn(commands: &mut Commands, fonts: &EmberFonts, panel: Entity) {
    let row = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, justify_content: JustifyContent::FlexEnd, margin: UiRect::top(Val::Px(8.0)), ..default() }).id();
    let btn = commands.spawn((Node { min_width: Val::Px(100.0), height: Val::Px(32.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::Center, column_gap: Val::Px(6.0), border_radius: BorderRadius::all(Val::Px(5.0)), ..default() }, BackgroundColor(rgb(accent())), Interaction::default(), ExportBtn, cursor())).id();
    bind_bg(commands, btn, |w| if can_export(w) { rgb(accent()) } else { rgb(section_bg()) });
    let ic = icon_text(commands, &fonts.phosphor, "rocket-launch", (255, 255, 255), 14.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let t = commands.spawn((Text::new("Export".to_string()), ui_font(&fonts.ui, 13.0), TextColor(Color::WHITE), FocusPolicy::Pass)).id();
    commands.entity(btn).add_children(&[ic, t]);
    commands.entity(row).add_child(btn);
    commands.entity(panel).add_child(row);
}

// ── Interaction ──────────────────────────────────────────────────────────────

fn can_export(w: &World) -> bool {
    let Some(s) = w.get_resource::<ExportOverlayState>() else { return false };
    let installed = w.get_resource::<TemplateManager>().is_some_and(|t| t.is_installed(s.platform));
    installed && !s.output_dir.is_empty() && s.active_task.is_none() && matches!(s.progress, ExportProgress::Idle | ExportProgress::Done(_) | ExportProgress::Error(_))
}

fn platform_click(q: Query<(&Interaction, &PlatformBtn), Changed<Interaction>>, mut state: Option<ResMut<ExportOverlayState>>) {
    let Some(state) = state.as_mut() else { return };
    for (i, b) in &q {
        if *i == Interaction::Pressed {
            state.platform = b.0;
        }
    }
}

fn close_click(q: Query<&Interaction, (With<CloseBtn>, Changed<Interaction>)>, mut state: Option<ResMut<ExportOverlayState>>) {
    let Some(state) = state.as_mut() else { return };
    if q.iter().any(|i| *i == Interaction::Pressed) {
        state.visible = false;
        state.active_task = None;
    }
}

fn icon_clear_click(q: Query<&Interaction, (With<IconClearBtn>, Changed<Interaction>)>, mut state: Option<ResMut<ExportOverlayState>>) {
    let Some(state) = state.as_mut() else { return };
    if q.iter().any(|i| *i == Interaction::Pressed) {
        state.icon_path = None;
    }
}

fn export_click(q: Query<&Interaction, (With<ExportBtn>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|w: &mut World| {
            if can_export(w) {
                let name = w.get_resource::<renzora::core::CurrentProject>().map(|p| p.config.name.clone()).unwrap_or_default();
                run_export(w, &name);
            }
        });
    }
}

fn output_browse_click(q: Query<&Interaction, (With<OutputBrowseBtn>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|w: &mut World| {
            if let Some(dir) = rfd::FileDialog::new().set_title("Select output directory").pick_folder() {
                w.resource_mut::<ExportOverlayState>().output_dir = dir.to_string_lossy().to_string();
            }
        });
    }
}

fn icon_browse_click(q: Query<&Interaction, (With<IconBrowseBtn>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|w: &mut World| {
            if let Some(f) = rfd::FileDialog::new().set_title("Select icon").add_filter("Images", &["png", "ico", "svg"]).pick_file() {
                w.resource_mut::<ExportOverlayState>().icon_path = Some(f.to_string_lossy().to_string());
            }
        });
    }
}

fn download_click(q: Query<&Interaction, (With<DownloadBtn>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|w: &mut World| {
            let p = w.resource::<ExportOverlayState>().platform;
            let runtime_dir = w.resource::<TemplateManager>().runtime_dir();
            let task = download::spawn_download(p, runtime_dir);
            let mut s = w.resource_mut::<ExportOverlayState>();
            s.download_task = Some(task);
            s.download_status = Some((p, DownloadProgress::Fetching("Starting download…".to_string())));
        });
    }
}

fn install_click(q: Query<&Interaction, (With<InstallBtn>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|w: &mut World| {
            let Some(file) = rfd::FileDialog::new().set_title("Select runtime template binary").pick_file() else { return };
            let p = w.resource::<ExportOverlayState>().platform;
            let runtime_dir = w.resource::<TemplateManager>().runtime_dir();
            let _ = std::fs::create_dir_all(&runtime_dir);
            let dest = runtime_dir.join(p.runtime_binary_name());
            let _ = std::fs::copy(&file, &dest);
            w.resource_mut::<TemplateManager>().scan();
        });
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn platform_icon(p: Platform) -> &'static str {
    match p {
        Platform::WindowsX64 => "windows-logo",
        Platform::LinuxX64 => "linux-logo",
        Platform::MacOSX64 | Platform::MacOSArm64 => "apple-logo",
        Platform::IOSArm64 => "device-mobile",
        Platform::TvOSArm64 => "television-simple",
        Platform::AndroidArm64 | Platform::AndroidX86_64 => "android-logo",
        Platform::FireTVArm64 => "television",
        Platform::WebWasm32 => "globe",
    }
}

fn fullscreen() -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(0.0),
        top: Val::Px(0.0),
        right: Val::Px(0.0),
        bottom: Val::Px(0.0),
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        ..default()
    }
}

fn ca(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::srgba_u8(r, g, b, a)
}

fn cursor() -> renzora_ember::cursor_icon::HoverCursor {
    renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer)
}

fn txt(commands: &mut Commands, fonts: &EmberFonts, s: &str, size: f32, color: (u8, u8, u8)) -> Entity {
    commands.spawn((Text::new(s.to_string()), ui_font(&fonts.ui, size), TextColor(rgb(color)))).id()
}

fn section(commands: &mut Commands) -> Entity {
    commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() }).id()
}

fn labeled(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    let row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).id();
    let t = txt(commands, fonts, label, 12.0, text_muted());
    commands.entity(row).add_child(t);
    row
}

fn icon_title(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str) -> Entity {
    let row = commands.spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }, FocusPolicy::Pass)).id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_primary(), 16.0);
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 15.0), TextColor(rgb(text_primary())))).id();
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

fn icon_msg(commands: &mut Commands, fonts: &EmberFonts, icon: &str, color: (u8, u8, u8)) -> (Entity, Entity) {
    let row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }).id();
    let ic = icon_text(commands, &fonts.phosphor, icon, color, 12.0);
    let t = commands.spawn((Text::new(String::new()), ui_font(&fonts.ui, 11.0), TextColor(rgb(color)))).id();
    commands.entity(row).add_children(&[ic, t]);
    (row, t)
}

fn check_state(commands: &mut Commands, fonts: &EmberFonts, label: &str, get: fn(&ExportOverlayState) -> bool, set: fn(&mut ExportOverlayState, bool)) -> Entity {
    let row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).id();
    let cb = checkbox(commands, false);
    bind_2way(commands, cb, move |w| w.get_resource::<ExportOverlayState>().map(get).unwrap_or(false), move |w, v: &bool| { if let Some(mut s) = w.get_resource_mut::<ExportOverlayState>() { set(&mut s, *v); } });
    let t = txt(commands, fonts, label, 12.0, text_primary());
    commands.entity(row).add_children(&[cb, t]);
    row
}

fn pill_button(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str) -> Entity {
    let btn = commands.spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(5.0), padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() }, BackgroundColor(rgb(section_bg())), Interaction::default(), cursor())).id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_primary(), 11.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), FocusPolicy::Pass)).id();
    commands.entity(btn).add_children(&[ic, t]);
    btn
}

fn style_input(commands: &mut Commands, input: Entity) {
    commands.entity(input).insert(Node { flex_grow: 1.0, height: Val::Px(28.0), align_items: AlignItems::Center, padding: UiRect::horizontal(Val::Px(8.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() });
}
