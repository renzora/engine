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
use std::sync::atomic::Ordering;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::{bind_2way, bind_bg, bind_display, bind_text, bind_text_color, react};
use renzora_ember::theme::*;
use renzora_ember::widgets::{bind_text_input, checkbox, drag_value, radio_group, scroll_area, scroll_view_pinned, section, spinner, tabs, text_input, OverlaySurface};

use crate::download::{self, DownloadProgress};
use crate::overlay::{ensure_release_fetch, poll_download_task, poll_export_task, poll_release_fetch, run_export, ExportOverlayState, ExportProgress, ExportView, PackagingMode};
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
            cancel_or_back_click,
            copy_log_click,
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
/// Cancel the running export, or (once finished) go back to the settings view.
#[derive(Component)]
struct CancelOrBackBtn;
/// Copy the full build log to the system clipboard.
#[derive(Component)]
struct CopyLogBtn;

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
        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            spawn_modal(&mut commands, &fonts, has_project);
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
        let select = used.as_ref().is_none_or(|set| set.contains(&p.id));
        if select {
            s.selected_plugins.insert(p.id.clone());
        }
    }
    s.available_plugins = plugins;
    // Default the engine-feature toggles (Solari follows its plugin; codecs are
    // auto-enabled from the project's asset files).
    let selected: Vec<String> = s.selected_plugins.iter().cloned().collect();
    s.capabilities = crate::capabilities::defaults(&selected, project_root.as_deref());
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
                .is_some_and(|n| n.starts_with('.'));
            if !is_dot {
                collect_ron_files(&path, out);
            }
        } else if matches!(
            path.extension().and_then(|x| x.to_str()),
            Some("ron") | Some("bsn")
        ) {
            // `.bsn` = interim scene format; `.ron` = sidecars/config still in RON.
            out.push(path);
        }
    }
}

fn spawn_modal(commands: &mut Commands, fonts: &EmberFonts, has_project: bool) {
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

    // Settings view — the export form. Hidden once an export starts.
    let settings_view = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), flex_grow: 1.0, min_height: Val::Px(0.0), ..default() })
        .id();
    commands.entity(panel).add_child(settings_view);
    bind_display(commands, settings_view, |w| {
        matches!(w.get_resource::<ExportOverlayState>().map(|s| s.view), Some(ExportView::Settings))
    });

    // Two columns.
    let cols = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, column_gap: Val::Px(16.0), flex_grow: 1.0, min_height: Val::Px(0.0), ..default() }).id();
    let sidebar = build_sidebar(commands, fonts);
    // The right column is NOT scrolled as a whole — the platform header and tab
    // bar inside it stay fixed; each tab caps and scrolls its own content (see
    // `finish_tab`). That keeps the top chrome put while a long list scrolls.
    let right = commands.spawn((Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), min_width: Val::Px(0.0), ..default() }, RightPane { sig: None })).id();
    commands.entity(cols).add_children(&[sidebar, right]);
    commands.entity(settings_view).add_child(cols);

    // Export button sits below the columns; the output fields (name, directory,
    // icon) now live in the Output tab inside the right pane.
    build_export_btn(commands, fonts, settings_view);

    // Log view — the live build terminal + progress bar + cancel. Shown while and
    // after an export runs.
    let log_view = build_log_view(commands, fonts);
    commands.entity(panel).add_child(log_view);
    bind_display(commands, log_view, |w| {
        matches!(w.get_resource::<ExportOverlayState>().map(|s| s.view), Some(ExportView::Log))
    });
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
    let nm = commands.spawn((Text::new(p.display_name().to_string()), ui_font(&fonts.ui, 12.5), TextColor(rgb(text_primary())), FocusPolicy::Pass, Node { flex_grow: 1.0, ..default() }, bevy::text::TextLayout::no_wrap())).id();
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
    let host = Platform::current() == Some(p);

    // Platform header — context above the category tabs.
    let hdr = commands.spawn(Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), margin: UiRect::bottom(Val::Px(6.0)), ..default() }).id();
    let title = icon_title(commands, fonts, platform_icon(p), p.display_name());
    let sub = txt(commands, fonts, p.supported_devices(), 11.0, text_muted());
    commands.entity(hdr).add_children(&[title, sub]);
    commands.entity(pane).add_child(hdr);

    // The six horizontal category tabs. Each builder returns a panel container;
    // `tabs()` shows one at a time (the ember tab widget the editor uses
    // elsewhere). Within a panel, each group is an ember collapsible `section`
    // — the same widget the inspector/settings panels use for their categories.
    let panels = vec![
        build_output_tab(commands, fonts),
        build_packaging_tab(commands, fonts, p, desktop, host),
        build_features_tab(commands, fonts, host),
        build_plugins_tab(commands, fonts),
        build_compression_tab(commands, fonts),
        build_options_tab(commands, fonts, p, desktop),
    ];
    let strip = tabs(
        commands,
        &fonts.ui,
        &["Output", "Packaging", "Features", "Plugins", "Compression", "Options"],
        panels.clone(),
    );
    // `tabs()` overwrites each panel's `Node` with `default() + display`, so a
    // column layout has to be re-applied here — preserving the initial
    // visibility it set (only panel 0 shown). `tab_select` later toggles only
    // the `display` field, leaving these other fields intact.
    for (i, &panel) in panels.iter().enumerate() {
        commands.entity(panel).insert(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            display: if i == 0 { Display::Flex } else { Display::None },
            ..default()
        });
    }
    commands.entity(pane).add_child(strip);
}

/// A placeholder container for one tab. Its `Node` is finalized in
/// `build_settings` after `tabs()` (which clobbers whatever we set here).
fn tab_panel(commands: &mut Commands) -> Entity {
    commands.spawn(Node::default()).id()
}

/// Max height a single tab's content scrolls within. The platform header + tab
/// bar live above the panels and stay fixed; only this inner content scrolls.
const TAB_CONTENT_MAX: f32 = 380.0;

/// Finish a tab: stack its `sections` in a column and wrap that in one capped
/// scroll viewport, so the content scrolls under the fixed header/tab bar
/// (sizes to content when short, scrolls past `TAB_CONTENT_MAX`).
fn finish_tab(commands: &mut Commands, panel: Entity, sections: &[Entity]) {
    let content = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() })
        .id();
    commands.entity(content).add_children(sections);
    let scroll = scroll_area(commands, content, TAB_CONTENT_MAX);
    commands.entity(panel).add_child(scroll);
}

// ── Output tab: binary name, export directory, icon ──────────────────────────

fn build_output_tab(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let panel = tab_panel(commands);
    let (sec, body) = section(commands, fonts, "folder-open", "Output", accent());

    // Binary name. (Empty initial value; `bind_text_input` reflects the current
    // state in on the first frame, so no `Init` snapshot is needed here.)
    let name_row = labeled(commands, fonts, "Name:");
    let name = text_input(commands, &fonts.ui, "Binary name", "");
    style_input(commands, name);
    bind_text_input(commands, name, |w| w.get_resource::<ExportOverlayState>().map(|s| s.binary_name.clone()).unwrap_or_default(), |w, v| { if let Some(mut s) = w.get_resource_mut::<ExportOverlayState>() { s.binary_name = v; } });
    commands.entity(name_row).add_child(name);

    // Export directory.
    let dir_row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }).id();
    let dir_lbl = txt(commands, fonts, "Folder:", 12.0, text_muted());
    let dir = text_input(commands, &fonts.ui, "Export directory...", "");
    style_input(commands, dir);
    bind_text_input(commands, dir, |w| w.get_resource::<ExportOverlayState>().map(|s| s.output_dir.clone()).unwrap_or_default(), |w, v| { if let Some(mut s) = w.get_resource_mut::<ExportOverlayState>() { s.output_dir = v; } });
    let dir_browse = pill_button(commands, fonts, "folder", "Browse");
    commands.entity(dir_browse).insert(OutputBrowseBtn);
    commands.entity(dir_row).add_children(&[dir_lbl, dir, dir_browse]);

    // Icon.
    let icon_row = commands.spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), margin: UiRect::top(Val::Px(2.0)), ..default() }, Name::new("icon-row"))).id();
    let icon_lbl = txt(commands, fonts, "Icon:", 12.0, text_muted());
    let icon_path = commands.spawn((Text::new("None".to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), Node { flex_grow: 1.0, ..default() }, bevy::text::TextLayout::no_wrap())).id();
    bind_text(commands, icon_path, |w| w.get_resource::<ExportOverlayState>().and_then(|s| s.icon_path.clone()).unwrap_or_else(|| "None".to_string()));
    let clear = commands.spawn((Node { padding: UiRect::all(Val::Px(2.0)), ..default() }, Interaction::default(), IconClearBtn, cursor())).id();
    let clx = icon_text(commands, &fonts.phosphor, "x", text_muted(), 12.0);
    commands.entity(clx).insert(FocusPolicy::Pass);
    commands.entity(clear).add_child(clx);
    bind_display(commands, clear, |w| w.get_resource::<ExportOverlayState>().is_some_and(|s| s.icon_path.is_some()));
    let icon_browse = pill_button(commands, fonts, "image", "Browse");
    commands.entity(icon_browse).insert(IconBrowseBtn);
    commands.entity(icon_row).add_children(&[icon_lbl, icon_path, clear, icon_browse]);

    commands.entity(body).add_children(&[name_row, dir_row, icon_row]);
    finish_tab(commands, panel, &[sec]);
    panel
}

// ── Packaging tab: packaging mode + runtime template status ──────────────────

fn build_packaging_tab(commands: &mut Commands, fonts: &EmberFonts, p: Platform, desktop: bool, host: bool) -> Entity {
    let panel = tab_panel(commands);
    let mut secs = Vec::new();

    // Packaging mode (desktop). The lean static single-binary mode recompiles
    // from source, which native cargo can only do for the host triple — so it's
    // offered only when exporting for the platform the editor is running on.
    if desktop {
        let (sec, body) = section(commands, fonts, "file-archive", "Packaging Mode", accent());
        let labels: &[&str] = if host {
            &["Binary + .rpak", "Single executable", "Lean single binary"]
        } else {
            &["Binary + .rpak", "Single executable"]
        };
        let radios = radio_group(commands, &fonts.ui, labels, 0);
        bind_2way(
            commands,
            radios,
            |w| match w.resource::<ExportOverlayState>().packaging_mode {
                PackagingMode::SeparateFiles => 0usize,
                PackagingMode::SingleBinary => 1,
                PackagingMode::LeanSingleBinary => 2,
            },
            |w, v: &usize| {
                w.resource_mut::<ExportOverlayState>().packaging_mode = match *v {
                    2 => PackagingMode::LeanSingleBinary,
                    1 => PackagingMode::SingleBinary,
                    _ => PackagingMode::SeparateFiles,
                };
            },
        );
        commands.entity(body).add_child(radios);
        if host {
            let hint = txt(commands, fonts, "Lean: recompiles a stripped static single binary (no engine dylibs). Installs Rust automatically if missing.", 11.0, text_muted());
            commands.entity(body).add_child(hint);
        }
        secs.push(sec);
    }

    secs.push(build_runtime_status(commands, fonts, p));
    finish_tab(commands, panel, &secs);
    panel
}

/// Runtime-template status section (installed line + Download/Install buttons +
/// download progress). Returns the section root for the caller to place.
fn build_runtime_status(commands: &mut Commands, fonts: &EmberFonts, p: Platform) -> Entity {
    let (sec, body) = section(commands, fonts, "download-simple", "Runtime Template", accent());
    // Installed / not status line.
    let (line, msg) = icon_msg(commands, fonts, "check-circle", text_muted());
    bind_text(commands, msg, move |w| if w.get_resource::<TemplateManager>().is_some_and(|t| t.is_installed(p)) { "Runtime template installed".to_string() } else { "Runtime template not installed".to_string() });
    commands.entity(body).add_child(line);
    // Buttons.
    let btns = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }).id();
    let dl = pill_button(commands, fonts, "download-simple", "Download from GitHub");
    commands.entity(dl).insert(DownloadBtn);
    let inst = pill_button(commands, fonts, "folder-open", "Install from file...");
    commands.entity(inst).insert(InstallBtn);
    commands.entity(btns).add_children(&[dl, inst]);
    commands.entity(body).add_child(btns);
    // Download progress.
    let (prog, pmsg) = icon_msg(commands, fonts, "spinner", text_muted());
    bind_text(commands, pmsg, move |w| match w.get_resource::<ExportOverlayState>().and_then(|s| s.download_status.clone()) {
        Some((dp, DownloadProgress::Fetching(m))) if dp == p => m,
        Some((dp, DownloadProgress::Done(m))) if dp == p => m,
        Some((dp, DownloadProgress::Error(m))) if dp == p => m,
        _ => String::new(),
    });
    bind_display(commands, prog, move |w| w.get_resource::<ExportOverlayState>().and_then(|s| s.download_status.as_ref().map(|(dp, _)| *dp == p)).unwrap_or(false));
    commands.entity(body).add_child(prog);
    sec
}

// ── Features tab: the lean engine-feature strip ──────────────────────────────

fn build_features_tab(commands: &mut Commands, fonts: &EmberFonts, host: bool) -> Entity {
    let panel = tab_panel(commands);
    let (sec, body) = section(commands, fonts, "sliders-horizontal", "Engine Features", accent());
    if host {
        let note = txt(commands, fonts, "Strip unused engine capabilities from the lean single-binary recompile. Auto-detected from the project where possible.", 11.0, text_muted());
        commands.entity(body).add_child(note);
        let list = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() }).id();
        for (idx, cap) in crate::capabilities::CAPABILITIES.iter().enumerate() {
            let id = cap.id;
            // One padded, zebra-striped item per capability (checkbox + label + help).
            let item = commands.spawn((Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), padding: UiRect::axes(Val::Px(6.0), Val::Px(5.0)), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() }, BackgroundColor(row_stripe(idx)))).id();
            // Inlined `check_state` so the closures can capture the capability id.
            let row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).id();
            let cb = checkbox(commands, false);
            bind_2way(
                commands,
                cb,
                move |w| w.get_resource::<ExportOverlayState>().map(|s| s.capabilities.get(id).copied().unwrap_or(false)).unwrap_or(false),
                move |w, v: &bool| {
                    if let Some(mut s) = w.get_resource_mut::<ExportOverlayState>() {
                        s.capabilities.insert(id.to_string(), *v);
                    }
                },
            );
            let t = txt(commands, fonts, cap.label, 12.0, text_primary());
            commands.entity(row).add_children(&[cb, t]);
            let help = txt(commands, fonts, cap.help, 10.0, text_muted());
            commands.entity(item).add_children(&[row, help]);
            commands.entity(list).add_child(item);
        }
        commands.entity(body).add_child(list);
    } else {
        let note = txt(commands, fonts, "Engine-feature stripping is part of the lean single-binary export, which only builds for the host platform you're running the editor on.", 11.0, text_muted());
        commands.entity(body).add_child(note);
    }
    finish_tab(commands, panel, &[sec]);
    panel
}

// ── Plugins tab ──────────────────────────────────────────────────────────────

fn build_plugins_tab(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let panel = tab_panel(commands);
    let (sec, body) = section(commands, fonts, "puzzle-piece", "Plugins", accent());
    let list = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() }).id();
    commands.entity(body).add_child(list);
    // Filled by a command that can read the world (the plugin list is stable
    // after the scan).
    commands.queue(move |world: &mut World| {
        let plugins: Vec<(String, String)> = world.get_resource::<ExportOverlayState>().map(|s| s.available_plugins.iter().map(|p| (p.id.clone(), format!("{:?}", p.scope))).collect()).unwrap_or_default();
        let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
        let mut queue = CommandQueue::default();
        {
            let mut c = Commands::new(&mut queue, world);
            if plugins.is_empty() {
                let note = c.spawn((Text::new("No plugins detected in this project.".to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())))).id();
                c.entity(list).add_child(note);
            }
            for (idx, (id, scope)) in plugins.into_iter().enumerate() {
                let row = c.spawn((Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)), border_radius: BorderRadius::all(Val::Px(3.0)), ..default() }, BackgroundColor(row_stripe(idx)))).id();
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
    finish_tab(commands, panel, &[sec]);
    panel
}

// ── Compression tab: asset compression + mesh optimization ───────────────────

fn build_compression_tab(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let panel = tab_panel(commands);

    // Asset compression level.
    let (csec, cbody) = section(commands, fonts, "file-archive", "Compression", accent());
    let crow = labeled(commands, fonts, "Level (zstd):");
    let dv = drag_value(commands, &fonts.ui, "", text_primary(), 0.0, 1.0);
    bind_2way(commands, dv, |w| w.resource::<ExportOverlayState>().compression_level as f32, |w, v: &f32| w.resource_mut::<ExportOverlayState>().compression_level = (v.round() as i32).clamp(1, 22));
    commands.entity(crow).add_child(dv);
    commands.entity(cbody).add_child(crow);

    // Mesh optimization.
    let (msec, mbody) = section(commands, fonts, "cube", "Mesh Optimization", accent());
    let simplify = check_state(commands, fonts, "Simplify meshes", |s| s.mesh_simplify, |s, v| s.mesh_simplify = v);
    commands.entity(mbody).add_child(simplify);
    let ratio = labeled(commands, fonts, "Keep ratio:");
    let dvr = drag_value(commands, &fonts.ui, "", text_primary(), 0.0, 0.01);
    bind_2way(commands, dvr, |w| w.resource::<ExportOverlayState>().mesh_simplify_ratio, |w, v: &f32| w.resource_mut::<ExportOverlayState>().mesh_simplify_ratio = v.clamp(0.1, 1.0));
    commands.entity(ratio).add_child(dvr);
    commands.entity(ratio).insert(Node { margin: UiRect::left(Val::Px(20.0)), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() });
    bind_display(commands, ratio, |w| w.resource::<ExportOverlayState>().mesh_simplify);
    commands.entity(mbody).add_child(ratio);
    let quant = check_state(commands, fonts, "Quantize vertex attributes", |s| s.mesh_quantize, |s, v| s.mesh_quantize = v);
    let lods = check_state(commands, fonts, "Generate LODs", |s| s.mesh_generate_lods, |s, v| s.mesh_generate_lods = v);
    commands.entity(mbody).add_children(&[quant, lods]);
    let levels = labeled(commands, fonts, "Levels:");
    let dvl = drag_value(commands, &fonts.ui, "", text_primary(), 0.0, 1.0);
    bind_2way(commands, dvl, |w| w.resource::<ExportOverlayState>().mesh_lod_levels as f32, |w, v: &f32| w.resource_mut::<ExportOverlayState>().mesh_lod_levels = (v.round() as u32).clamp(1, 5));
    commands.entity(levels).add_child(dvl);
    commands.entity(levels).insert(Node { margin: UiRect::left(Val::Px(20.0)), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() });
    bind_display(commands, levels, |w| w.resource::<ExportOverlayState>().mesh_generate_lods);
    commands.entity(mbody).add_child(levels);
    finish_tab(commands, panel, &[csec, msec]);
    panel
}

// ── Options tab: window + flags ──────────────────────────────────────────────

fn build_options_tab(commands: &mut Commands, fonts: &EmberFonts, p: Platform, desktop: bool) -> Entity {
    let panel = tab_panel(commands);
    let mut secs = Vec::new();

    // Window (desktop).
    if desktop {
        let (wsec, wbody) = section(commands, fonts, "monitor", "Window", accent());
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
        commands.entity(wbody).add_child(radios);
        let size = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }).id();
        let szl = txt(commands, fonts, "Size:", 12.0, text_muted());
        let dw = drag_value(commands, &fonts.ui, "", text_primary(), 0.0, 10.0);
        bind_2way(commands, dw, |w| w.resource::<ExportOverlayState>().window_width as f32, |w, v: &f32| w.resource_mut::<ExportOverlayState>().window_width = (v.round() as u32).clamp(320, 7680));
        let xl = txt(commands, fonts, "x", 12.0, text_muted());
        let dh = drag_value(commands, &fonts.ui, "", text_primary(), 0.0, 10.0);
        bind_2way(commands, dh, |w| w.resource::<ExportOverlayState>().window_height as f32, |w, v: &f32| w.resource_mut::<ExportOverlayState>().window_height = (v.round() as u32).clamp(240, 4320));
        commands.entity(size).add_children(&[szl, dw, xl, dh]);
        bind_display(commands, size, |w| matches!(w.resource::<ExportOverlayState>().window_mode, WindowMode::Windowed));
        commands.entity(wbody).add_child(size);
        secs.push(wsec);
    }

    // Flags.
    let (osec, obody) = section(commands, fonts, "gear", "Options", accent());
    let console = check_state(commands, fonts, "Console logging", |s| s.console_logging, |s, v| s.console_logging = v);
    commands.entity(obody).add_child(console);
    if desktop && p.supports_dedicated_server() {
        let server = check_state(commands, fonts, "Include dedicated server", |s| s.include_server, |s, v| s.include_server = v);
        commands.entity(obody).add_child(server);
    }
    secs.push(osec);
    finish_tab(commands, panel, &secs);
    panel
}

// ── Progress / export button ─────────────────────────────────────────────────

/// Rough crate count for a full lean build, used only to scale the progress bar
/// (cargo gives no total in piped mode). Over/under-estimating just makes the bar
/// move a little fast or slow; it snaps to full on "Finished"/Done.
const LEAN_BUILD_ESTIMATE: f32 = 480.0;

/// Progress-bar fraction (0..1) from compiled-crate count, full once finished.
fn build_fraction(s: &ExportOverlayState) -> f32 {
    if s.build_finished {
        1.0
    } else if s.build_compiled == 0 {
        0.02
    } else {
        (s.build_compiled as f32 / LEAN_BUILD_ESTIMATE).min(0.97)
    }
}

/// Reactively drive a node's width as a percentage (for the progress fill).
fn bind_width_pct(
    commands: &mut Commands,
    target: Entity,
    value: impl Fn(&World) -> f32 + Send + Sync + 'static,
) {
    react(commands, move |w: &mut World| {
        if w.get_entity(target).is_err() {
            return false;
        }
        let v = value(w).clamp(0.0, 1.0);
        if let Some(mut n) = w.get_mut::<Node>(target) {
            n.width = Val::Percent(v * 100.0);
        }
        true
    });
}

/// The export log view: a heading, a progress bar, a scrolling terminal of the
/// live build output, and a Cancel/Back button. Replaces the old inline progress.
fn build_log_view(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let view = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), flex_grow: 1.0, min_height: Val::Px(0.0), ..default() })
        .id();

    // Heading — reflects the current phase.
    let heading_row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).id();
    let sp = spinner(commands);
    bind_display(commands, sp, |w| {
        w.get_resource::<ExportOverlayState>().map(|s| s.active_task.is_some()).unwrap_or(false)
    });
    let heading = txt(commands, fonts, "", 14.0, text_primary());
    bind_text(commands, heading, |w| match w.get_resource::<ExportOverlayState>().map(|s| s.progress.clone()) {
        Some(ExportProgress::Done(_)) => "Export complete".to_string(),
        Some(ExportProgress::Error(_)) => "Export failed".to_string(),
        _ => "Exporting…".to_string(),
    });
    bind_text_color(commands, heading, |w| match w.get_resource::<ExportOverlayState>().map(|s| s.progress.clone()) {
        Some(ExportProgress::Done(_)) => rgb(GREEN),
        Some(ExportProgress::Error(_)) => rgb(RED),
        _ => rgb(text_primary()),
    });
    commands.entity(heading_row).add_children(&[sp, heading]);

    // Progress bar.
    let track = commands.spawn((Node { width: Val::Percent(100.0), height: Val::Px(8.0), border_radius: BorderRadius::all(Val::Px(4.0)), overflow: Overflow::clip(), ..default() }, BackgroundColor(rgb(card_bg())))).id();
    let fill = commands.spawn((Node { width: Val::Percent(2.0), height: Val::Percent(100.0), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() }, BackgroundColor(rgb(accent())))).id();
    commands.entity(track).add_child(fill);
    bind_width_pct(commands, fill, |w| w.get_resource::<ExportOverlayState>().map(build_fraction).unwrap_or(0.0));
    bind_bg(commands, fill, |w| match w.get_resource::<ExportOverlayState>().map(|s| s.progress.clone()) {
        Some(ExportProgress::Error(_)) => rgb(RED),
        Some(ExportProgress::Done(_)) => rgb(GREEN),
        _ => rgb(accent()),
    });

    // Terminal — dark monospace box with the FULL build log in a pinned scroll
    // view: it auto-follows the bottom as new output streams in, but releases if
    // the user scrolls up to read back (an error can otherwise be pushed out of
    // view by cargo's huge linker-command dump). `build_log` is tail-capped at 600
    // lines upstream so this stays bounded.
    let term = commands.spawn((Node { width: Val::Percent(100.0), height: Val::Px(360.0), flex_direction: FlexDirection::Column, padding: UiRect::all(Val::Px(8.0)), overflow: Overflow::clip(), ..default() }, BackgroundColor(rgb((14, 16, 20))))).id();
    let log_text = commands.spawn((Text::new(""), ui_font(&fonts.mono, 11.0), TextColor(rgb(text_muted())), FocusPolicy::Pass)).id();
    bind_text(commands, log_text, |w| {
        w.get_resource::<ExportOverlayState>().map(|s| s.build_log.join("\n")).unwrap_or_default()
    });
    let log_scroll = scroll_view_pinned(commands, log_text);
    commands.entity(term).add_child(log_scroll);

    // Buttons: Copy log (left), Cancel/Back (right).
    let btn_row = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, ..default() }).id();
    let copy_btn = pill_button(commands, fonts, "clipboard", "Copy log");
    commands.entity(copy_btn).insert(CopyLogBtn);
    let btn = commands.spawn((Node { min_width: Val::Px(100.0), height: Val::Px(32.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(5.0)), ..default() }, BackgroundColor(rgb(section_bg())), Interaction::default(), CancelOrBackBtn, cursor())).id();
    let btn_label = commands.spawn((Text::new("Cancel"), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_primary())), FocusPolicy::Pass)).id();
    bind_text(commands, btn_label, |w| {
        if w.get_resource::<ExportOverlayState>().map(|s| s.active_task.is_some()).unwrap_or(false) {
            "Cancel".to_string()
        } else {
            "Back".to_string()
        }
    });
    commands.entity(btn).add_child(btn_label);
    commands.entity(btn_row).add_children(&[copy_btn, btn]);

    commands.entity(view).add_children(&[heading_row, track, term, btn_row]);
    view
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
        // Closing mid-export cancels the build rather than leaving it running
        // detached. Reset to the settings view for next time.
        if let Some(task) = state.active_task.as_ref() {
            task.cancel.store(true, Ordering::Relaxed);
        }
        state.visible = false;
        state.active_task = None;
        state.view = ExportView::Settings;
    }
}

/// Cancel the running build, or — once it has finished — return to the settings
/// form. The button's label flips between "Cancel" and "Back" accordingly.
fn cancel_or_back_click(
    q: Query<&Interaction, (With<CancelOrBackBtn>, Changed<Interaction>)>,
    mut state: Option<ResMut<ExportOverlayState>>,
) {
    let Some(state) = state.as_mut() else { return };
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    if let Some(task) = state.active_task.as_ref() {
        // Running → request cancellation; the worker kills the cargo build and
        // the poll loop flips to the cancelled/error state.
        task.cancel.store(true, Ordering::Relaxed);
    } else {
        // Finished → back to the settings form.
        state.view = ExportView::Settings;
        state.progress = ExportProgress::Idle;
    }
}

/// Copy the full build log to the system clipboard.
fn copy_log_click(
    q: Query<&Interaction, (With<CopyLogBtn>, Changed<Interaction>)>,
    state: Option<Res<ExportOverlayState>>,
) {
    let Some(state) = state else { return };
    if q.iter().any(|i| *i == Interaction::Pressed) {
        if let Ok(mut cb) = arboard::Clipboard::new() {
            let _ = cb.set_text(state.build_log.join("\n"));
        }
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

/// Zebra-stripe background for list rows — a faint overlay on odd rows so long
/// lists (features, plugins) read as discrete rows.
fn row_stripe(idx: usize) -> Color {
    if idx % 2 == 1 { ca(255, 255, 255, 4) } else { Color::NONE }
}

fn cursor() -> renzora_ember::cursor_icon::HoverCursor {
    renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer)
}

fn txt(commands: &mut Commands, fonts: &EmberFonts, s: &str, size: f32, color: (u8, u8, u8)) -> Entity {
    commands.spawn((Text::new(s.to_string()), ui_font(&fonts.ui, size), TextColor(rgb(color)))).id()
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
