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
use renzora_ember::reactive::{react, KeyedSnapshot};
use renzora_ember::reactive::Rx;
use renzora_ember::reactive::tracked::{bind_2way, bind_bg, bind_display, bind_text, bind_text_color, keyed_list};
use renzora_ember::theme::*;
use renzora_ember::widgets::{bind_text_input, checkbox, drag_value, icon_menu_button, radio_group, scroll_area, scroll_view_pinned, section, spinner, tabs, text_input, toggle_switch, OverlaySurface};

use crate::download::{self, DownloadProgress};
use crate::overlay::{ensure_release_fetch, poll_download_task, poll_export_task, poll_release_fetch, run_export, ExportOverlayState, ExportProgress, ExportView, PackagingMode, PluginLinkMode};
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
            preset_click,
            preset_dup_click,
            preset_del_click,
            output_browse_click,
            icon_browse_click,
            icon_clear_click,
            download_click,
            source_download_click,
            install_click,
            export_click,
            cancel_or_back_click,
            copy_log_click,
            close_click,
            section_toggle_click,
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
struct PresetBtn(usize);
#[derive(Component)]
struct PresetDupBtn;
#[derive(Component)]
struct PresetDelBtn;
#[derive(Component)]
struct OutputBrowseBtn;
#[derive(Component)]
struct IconBrowseBtn;
#[derive(Component)]
struct IconClearBtn;
#[derive(Component)]
struct DownloadBtn;
#[derive(Component)]
struct SourceDownloadBtn;
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
/// Features-tab section header — click folds/unfolds that section.
#[derive(Component)]
struct SectionToggle(&'static str);

// ── Lifecycle ────────────────────────────────────────────────────────────────

/// Probe Docker once per selected platform.
///
/// Only for a cross-platform lean build, which is the only combination that
/// needs a container: a lean build recompiles the engine, and cargo can only
/// target the host. Probing spawns a process, so it is keyed on the platform and
/// runs again only when that changes — never per frame.
fn probe_docker(world: &mut World) {
    let Some(state) = world.get_resource::<ExportOverlayState>() else { return };
    let needed = state.packaging_mode == PackagingMode::LeanSingleBinary
        && Platform::current() != Some(state.platform);
    let platform = state.platform;
    if !needed {
        // Clear so re-selecting a cross platform re-probes: the user may have
        // started Docker Desktop in between, and a stale "not running" would be
        // a dead end with no way to retry.
        if state.docker.is_some() {
            if let Some(mut s) = world.get_resource_mut::<ExportOverlayState>() {
                s.docker = None;
                s.docker_probed_for = None;
            }
        }
        return;
    }
    if state.docker_probed_for == Some(platform) {
        return;
    }
    let status = crate::docker::probe();
    if let Some(mut s) = world.get_resource_mut::<ExportOverlayState>() {
        s.docker = Some(status);
        s.docker_probed_for = Some(platform);
    }
}

fn manage_export_modal(world: &mut World) {
    let visible = world.get_resource::<ExportOverlayState>().is_some_and(|s| s.visible);
    if visible {
        // Presets belong to the open project, so this both fills an empty list
        // on first open and swaps it when the user changes project without
        // closing the editor. It no-ops once loaded for that path.
        if let Some(root) =
            world.get_resource::<renzora::core::CurrentProject>().map(|p| p.path.clone())
        {
            if let Some(mut state) = world.get_resource_mut::<ExportOverlayState>() {
                state.load_presets(&root);
            }
        }
        probe_docker(world);
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
    // Re-scan when the target platform changes: the plugin set is whatever that
    // platform's template brought with it, not whatever the editor happens to
    // have loaded. `plugins_scanned` alone would pin the first platform's list.
    let platform = world.resource::<ExportOverlayState>().platform;
    {
        let s = world.resource::<ExportOverlayState>();
        if s.plugins_scanned && s.plugins_scanned_for == Some(platform) {
            return;
        }
    }
    let dir = world.resource::<TemplateManager>().plugins_dir_for(platform);
    let mut plugins = renzora_plugin::host::loader::scan_plugins(world, &dir);

    // Native plugins too, which the C-ABI scan above cannot see: it looks for
    // library FILES exporting `renzora_plugin_init`, and a native plugin is a
    // directory holding a `build/` — so the picker listed only half of what an
    // export ships, and the half it hid was the half users actually install.
    //
    // They come from the EDITOR's `plugins/`, not the platform template's. A
    // native plugin links the real Bevy and is compiled against this editor's
    // shared images; the library that ships is the one the editor built, which
    // is also why only a host, copy-based export can take them.
    //
    // Editor-scope ones are left out entirely rather than shown and refused.
    // They can never ship, so offering a switch for one would be a control whose
    // only setting is off.
    if let Some(editor_dir) = crate::build::editor_dir() {
        let lib_ext = match platform {
            Platform::WindowsX64 | Platform::WindowsArm64 => "dll",
            Platform::MacOSX64 | Platform::MacOSArm64 => "dylib",
            _ => "so",
        };
        for p in renzora_native_plugin::installed(&editor_dir.join("plugins"), lib_ext) {
            if p.scope != renzora::NativePluginScope::Runtime {
                continue;
            }
            plugins.push(renzora_plugin::host::loader::PluginInfo {
                id: p.id,
                path: p.lib,
                scope: renzora_plugin::sys::PluginScope::Runtime,
            });
        }
        plugins.sort_by(|a, b| a.id.cmp(&b.id));
    }

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
    s.plugins_scanned_for = Some(platform);
}

/// The plugin ids referenced by any `.ron` scene/prefab under `root`. Matches
/// each plugin's crate prefix (`<id>::`, with a leading `lib` stripped for unix
/// dll names) against the serialized component type paths. Returns `None` if no
/// `.ron` could be read, so the caller falls back to selecting all plugins.
fn scene_used_plugin_ids(
    root: &std::path::Path,
    available: &[renzora_plugin::host::loader::PluginInfo],
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
                // Wider than the 760 it started at: the plugin picker is a grid
                // of thumbnail cards now, and four columns of artwork plus the
                // 180px preset sidebar does not fit in 760.
                width: Val::Px(980.0),
                // Explicit height, not just a cap. The dialog used to be propped
                // open by a sidebar listing twelve platforms; the preset list
                // replacing it starts EMPTY, so the modal collapsed to the height
                // of whichever tab was showing and jumped every time you switched
                // tab or added a preset. A fixed height keeps the tabs, the log
                // view and the Export button in one place regardless of content —
                // `max_height` keeps it on screen on a short display.
                height: Val::Vh(78.0),
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
    let title = icon_title(commands, fonts, "package", &renzora::lang::t("export.title"));
    let close = commands.spawn((Node { padding: UiRect::all(Val::Px(2.0)), ..default() }, Interaction::default(), CloseBtn, cursor())).id();
    let cx = icon_text(commands, &fonts.phosphor, "x", text_muted(), 16.0);
    commands.entity(cx).insert(FocusPolicy::Pass);
    commands.entity(close).add_child(cx);
    commands.entity(header).add_children(&[title, close]);
    commands.entity(panel).add_child(header);
    let sep = commands.spawn((Node { width: Val::Percent(100.0), height: Val::Px(1.0), margin: UiRect::vertical(Val::Px(8.0)), ..default() }, BackgroundColor(rgb(divider())))).id();
    commands.entity(panel).add_child(sep);

    if !has_project {
        let w = txt(commands, fonts, &renzora::lang::t("export.no_project"), 12.0, RED);
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
    let right = commands.spawn((Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), min_width: Val::Px(0.0), min_height: Val::Px(0.0), ..default() }, RightPane { sig: None })).id();
    // Every setting in the right pane belongs to the selected preset, so with
    // nothing selected there is nothing to configure. Showing the form anyway
    // invited edits that had nowhere to be saved to — and offered an Export
    // button for a configuration that does not exist.
    bind_display(commands, right, |w| {
        w.get_resource::<ExportOverlayState>().is_some_and(|s| s.active_preset.is_some())
    });

    // What stands in its place: say what to do, rather than leaving the pane
    // blank next to a sidebar that already says "press +".
    let right_empty = commands
        .spawn(Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, align_items: AlignItems::Center, justify_content: JustifyContent::Center, row_gap: Val::Px(6.0), min_width: Val::Px(0.0), ..default() })
        .id();
    let ei = icon_text(commands, &fonts.phosphor, "package", text_muted(), 30.0);
    let et = txt(commands, fonts, &renzora::lang::t("export.presets.none_selected"), 12.0, text_muted());
    commands.entity(right_empty).add_children(&[ei, et]);
    bind_display(commands, right_empty, |w| {
        w.get_resource::<ExportOverlayState>().is_some_and(|s| s.active_preset.is_none())
    });

    commands.entity(cols).add_children(&[sidebar, right, right_empty]);
    commands.entity(settings_view).add_child(cols);

    // Export button sits below the columns; the output fields (name, directory,
    // icon) now live in the Output tab inside the right pane. Hidden with the
    // form for the same reason — there is nothing to export without a preset.
    let export_row = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() }).id();
    commands.entity(settings_view).add_child(export_row);
    bind_display(commands, export_row, |w| {
        w.get_resource::<ExportOverlayState>().is_some_and(|s| s.active_preset.is_some())
    });
    build_export_btn(commands, fonts, export_row);

    // Log view — the live build terminal + progress bar + cancel. Shown while and
    // after an export runs.
    let log_view = build_log_view(commands, fonts);
    commands.entity(panel).add_child(log_view);
    bind_display(commands, log_view, |w| {
        matches!(w.get_resource::<ExportOverlayState>().map(|s| s.view), Some(ExportView::Log))
    });
}

// ── Sidebar (presets) ────────────────────────────────────────────────────────
//
// This used to list every platform, with the settings for whichever was
// selected held in memory and lost at the next launch. A preset is that
// configuration given a name, so two shipping configurations for the same
// platform can sit side by side — a demo build and a full one, say — and both
// survive a restart. The platform is now a property OF a preset, chosen when it
// is added, rather than a separate axis.

fn build_sidebar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = commands.spawn(Node { width: Val::Px(180.0), flex_shrink: 0.0, flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() }).id();

    // Header: title on the left, add-menu on the right.
    let head_row = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }).id();
    let head = section_label(commands, fonts, "sliders-horizontal", &renzora::lang::t("export.section.presets"));
    commands.entity(head).insert(Node { flex_grow: 1.0, ..default() });
    // The platform picker lives here, which is the whole reason a bare "+" is
    // enough: adding a preset IS choosing a platform, so there is no separate
    // list to keep in step with the selection.
    let names: Vec<&str> = Platform::ALL.iter().map(|p| p.display_name()).collect();
    let add = icon_menu_button(
        commands,
        fonts,
        "plus",
        "desktop-tower",
        &names,
        |world, i| {
            let Some(&platform) = Platform::ALL.get(i) else { return };
            let Some(mut state) = world.get_resource_mut::<ExportOverlayState>() else { return };
            // Keep the outgoing preset's edits before the new one replaces the
            // working fields.
            state.sync_active_preset();
            let name = crate::presets::unique_name(platform.display_name(), &state.presets);
            state.presets.push(crate::presets::ExportPreset::new(name, platform));
            let last = state.presets.len() - 1;
            // Not `select_preset`: that early-returns when the index is already
            // active and would also re-sync the preset we just pushed.
            state.active_preset = Some(last);
            if let Some(p) = state.presets.get(last).cloned() {
                p.apply(&mut state);
            }
            state.save_presets();
        },
    );
    commands.entity(head_row).add_children(&[head, add]);
    commands.entity(col).add_child(head_row);

    // The list itself — keyed, so adding or removing one preset rebuilds that
    // row rather than the whole sidebar (and never disturbs the rest).
    let list = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() }).id();
    keyed_list(commands, list, preset_list_snapshot);
    commands.entity(col).add_child(list);

    // Empty state. A project with no presets shows nothing at all otherwise,
    // and "the export dialog is blank" is a worse first impression than a line
    // of text saying what to press.
    let empty = txt(commands, fonts, &renzora::lang::t("export.presets.empty"), 11.0, text_muted());
    commands.entity(empty).insert(Node { margin: UiRect::vertical(Val::Px(8.0)), ..default() });
    bind_display(commands, empty, |w| {
        w.get_resource::<ExportOverlayState>().is_some_and(|s| s.presets.is_empty())
    });
    commands.entity(col).add_child(empty);

    // Duplicate / Remove act on the selection, so they are hidden when there
    // isn't one rather than sitting there doing nothing.
    let actions = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, column_gap: Val::Px(4.0), margin: UiRect::top(Val::Px(6.0)), ..default() }).id();
    let dup = small_button(commands, fonts, "copy", &renzora::lang::t("export.presets.duplicate"), PresetDupBtn);
    let del = small_button(commands, fonts, "trash", &renzora::lang::t("export.presets.remove"), PresetDelBtn);
    commands.entity(actions).add_children(&[dup, del]);
    bind_display(commands, actions, |w| {
        w.get_resource::<ExportOverlayState>().is_some_and(|s| s.active_preset.is_some())
    });
    commands.entity(col).add_child(actions);
    // Release info status.
    let rel = txt(commands, fonts, "", 11.0, text_muted());
    commands.entity(rel).insert(Node { margin: UiRect::top(Val::Px(8.0)), ..default() });
    bind_text(commands, rel, |w| {
        let s = w.resource::<ExportOverlayState>();
        if let Some(err) = &s.release_fetch_error {
            format!("⚠ {err}")
        } else if let Some(info) = &s.release_info {
            // Say when this is the nightly fallback rather than a release for
            // this exact version — "where did this runtime come from?" should be
            // answerable without reading the source.
            let key = if info.is_fallback {
                "export.status.nightly"
            } else {
                "export.status.latest"
            };
            format!("{} {}", renzora::lang::t(key), info.tag_name)
        } else if s.release_fetch_started {
            renzora::lang::t("export.status.loading_release")
        } else {
            String::new()
        }
    });
    commands.entity(col).add_child(rel);
    col
}

/// A snapshot with no rows — what the preset list shows before the overlay
/// resource exists.
fn empty_snapshot() -> KeyedSnapshot {
    KeyedSnapshot { items: Vec::new(), build: Box::new(|c, _, _| c.spawn(Node::default()).id()) }
}

fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}

/// This frame's preset rows, keyed by index.
///
/// The hash carries everything a row draws — name, platform, and whether it is
/// the selection — so renaming a preset or moving the selection rebuilds only
/// the rows that actually changed.
fn preset_list_snapshot(world: &Rx) -> KeyedSnapshot {
    use std::hash::{Hash, Hasher};
    let Some(state) = world.get_resource::<ExportOverlayState>() else {
        return empty_snapshot();
    };
    let rows: Vec<(String, Platform, bool)> = state
        .presets
        .iter()
        .enumerate()
        .map(|(i, p)| (p.name.clone(), p.platform, state.active_preset == Some(i)))
        .collect();
    let items: Vec<(u64, u64)> = rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            row.hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (name, platform, selected) = &rows[i];
            preset_row(c, f, i, name, *platform, *selected)
        }),
    }
}

/// One preset row: platform icon, name, and the template-status dot that used to
/// sit on the platform button.
///
/// The dot still earns its place — a preset for a platform whose runtime
/// template is not installed cannot export, and saying so here is what makes the
/// Download button in the right pane make sense when you reach it.
fn preset_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    index: usize,
    name: &str,
    p: Platform,
    selected: bool,
) -> Entity {
    let btn = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(40.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), padding: UiRect::horizontal(Val::Px(10.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(if selected { rgb(section_bg()) } else { Color::NONE }),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            PresetBtn(index),
            cursor(),
        ))
        .id();
    // Selection is baked into the row hash, so only hover needs to be live.
    bind_bg(commands, btn, move |w| {
        let hov = matches!(w.get::<Interaction>(btn), Some(Interaction::Hovered));
        if selected { rgb(section_bg()) } else if hov { ca(255, 255, 255, 10) } else { Color::NONE }
    });
    let ic = icon_text(commands, &fonts.phosphor, platform_icon(p), text_primary(), 18.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let nm = commands.spawn((Text::new(name.to_string()), ui_font(&fonts.ui, 12.5), TextColor(rgb(text_primary())), FocusPolicy::Pass, Node { flex_grow: 1.0, ..default() }, bevy::text::TextLayout::no_wrap())).id();
    let dot = commands.spawn((Node { width: Val::Px(8.0), height: Val::Px(8.0), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() }, BackgroundColor(rgb(text_muted())), FocusPolicy::Pass)).id();
    bind_bg(commands, dot, move |w| {
        let installed = w.get_resource::<TemplateManager>().is_some_and(|t| t.is_installed(p));
        let available = w.get_resource::<ExportOverlayState>().and_then(|s| s.release_info.as_ref().map(|r| r.available_platforms.contains(&p))).unwrap_or(false);
        if installed { rgb(GREEN) } else if available { rgb(AMBER) } else { ca(130, 138, 160, 150) }
    });
    commands.entity(btn).add_children(&[ic, nm, dot]);
    btn
}

/// A compact icon+label button for the sidebar's Duplicate / Remove pair.
fn small_button<M: Component>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
    marker: M,
) -> Entity {
    let btn = commands
        .spawn((
            Node { flex_grow: 1.0, height: Val::Px(26.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::Center, column_gap: Val::Px(5.0), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() },
            BackgroundColor(Color::NONE),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            marker,
            cursor(),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        if matches!(w.get::<Interaction>(btn), Some(Interaction::Hovered)) {
            ca(255, 255, 255, 10)
        } else {
            Color::NONE
        }
    });
    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 13.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let tx = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), FocusPolicy::Pass, bevy::text::TextLayout::no_wrap())).id();
    commands.entity(btn).add_children(&[ic, tx]);
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
    // Measured here rather than inside the tab builders, which have `Commands`
    // and no way to reach a `Window`. Logical (not physical) pixels, because the
    // `Val::Px` cap it feeds is logical too — using the raw resolution would
    // over-size the cap by the DPI scale factor on a scaled display.
    let window_height = world
        .query_filtered::<&bevy::window::Window, With<bevy::window::PrimaryWindow>>()
        .iter(world)
        .next()
        .map(|w| w.resolution.height() / w.resolution.scale_factor())
        .unwrap_or(0.0);
    let tab_max = tab_content_max(window_height);
    let mut queue = CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        for k in kids {
            commands.entity(k).despawn();
        }
        build_settings(&mut commands, &fonts, pane, platform, tab_max);
    }
    queue.apply(world);
    if let Some(mut r) = world.get_mut::<RightPane>(pane) {
        r.sig = Some(sig);
    }
}

fn build_settings(commands: &mut Commands, fonts: &EmberFonts, pane: Entity, p: Platform, tab_max: f32) {
    let desktop = matches!(p, Platform::WindowsX64 | Platform::LinuxX64 | Platform::MacOSX64 | Platform::MacOSArm64);
    // Not "is this the host?" any more, but "can a lean binary be produced for
    // this platform at all?" Everything downstream (the lean radio option,
    // engine-feature stripping, linking plugins in) depends on a lean build
    // existing, not on it being local.
    //
    // Two conditions, and the source one is easy to forget. A lean build
    // RECOMPILES the engine, so it needs the engine source — which a canonical
    // editor download does not have and cannot fetch (releases publish
    // templates, not source). Offering the option there just moves the failure
    // from "greyed out" to a runtime error after the asset scan. The copy-based
    // modes are unaffected: they copy a prebuilt runtime template and are the
    // normal path for anyone without a checkout.
    let host = crate::docker::lean_supported(p) && lean_source_available();

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
    // Four tabs, not six. Compression folded into Packaging and Options into
    // Output, because both were a tab holding one idea: six tabs for what is
    // really three decisions — what am I making, how is it built, what goes in —
    // meant hunting for a setting rather than reading down a page.
    let panels = vec![
        build_output_tab(commands, fonts, p, desktop, tab_max),
        build_packaging_tab(commands, fonts, p, desktop, host, tab_max),
        build_features_tab(commands, fonts, host, tab_max),
        build_plugins_tab(commands, fonts, host, tab_max),
    ];
    let tab_labels = [
        renzora::lang::t("export.tab.output"),
        renzora::lang::t("export.tab.packaging"),
        renzora::lang::t("export.tab.features"),
        renzora::lang::t("export.tab.plugins"),
    ];
    let tab_refs: Vec<&str> = tab_labels.iter().map(|s| s.as_str()).collect();
    let strip = tabs(
        commands,
        &fonts.ui,
        &tab_refs,
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

/// Is there an engine source checkout for a lean build to recompile?
///
/// A few `is_file`/`is_dir` calls up a short path, and only on a right-pane
/// rebuild (platform change), so it is not worth caching.
fn lean_source_available() -> bool {
    crate::build::resolve_engine_source().is_some()
}

/// Max height a single tab's content scrolls within. The platform header + tab
/// bar live above the panels and stay fixed; only this inner content scrolls.
///
/// A `max_height` rather than a flex fill, and that is the load-bearing detail:
/// `scroll_area` gives the viewport a DEFINITE height, which is what lets it
/// clip and what makes the scrollbar appear (the bar is driven by the viewport's
/// own overflow). Filling by flex instead was tried and does not work here —
/// the panel is five flex levels below the dialog and never gets a definite
/// height of its own, so `height: 100%` grew to the content (clipped by the
/// dialog, no bar) and `flex_basis: 0` collapsed to nothing (blank tab).
///
/// Derived from the window rather than the old hardcoded 380px, because the
/// dialog is now a fixed 78vh: a constant cap left a band of dead space between
/// a short tab and the Export button. The subtraction is the dialog's fixed
/// chrome — title, separator, platform header, tab bar, Export row, padding.
fn tab_content_max(window_height: f32) -> f32 {
    // The dialog's fixed chrome: title row, separator, platform header, tab bar,
    // Export row, panel padding.
    const CHROME: f32 = 230.0;
    // No window to measure (headless, or before one exists): the constant this
    // replaced, which is known to be safe on any display rather than merely
    // likely. Also the floor, so a very short window cannot produce a cap so
    // small the tab is unusable — it scrolls instead.
    const FALLBACK: f32 = 380.0;
    if window_height <= 0.0 {
        return FALLBACK;
    }
    (window_height * MODAL_VH - CHROME).max(FALLBACK)
}

/// The dialog's height as a fraction of the window. Kept beside the `Val::Vh`
/// in `spawn_modal` — the two have to agree or the tab cap is computed against a
/// dialog of a different size.
const MODAL_VH: f32 = 0.78;

/// Finish a tab: stack its `sections` in a column and wrap that in one capped
/// scroll viewport, so the content scrolls under the fixed header/tab bar
/// (sizes to content when short, scrolls past `TAB_CONTENT_MAX`).
fn finish_tab(commands: &mut Commands, panel: Entity, sections: &[Entity], tab_max: f32) {
    let content = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(6.0), ..default() })
        .id();
    commands.entity(content).add_children(sections);
    let scroll = scroll_area(commands, content, tab_max);
    commands.entity(panel).add_child(scroll);
}

// ── Output tab: binary name, export directory, icon ──────────────────────────

fn build_output_tab(commands: &mut Commands, fonts: &EmberFonts, p: Platform, desktop: bool, tab_max: f32) -> Entity {
    let panel = tab_panel(commands);
    let (sec, body) = section(commands, fonts, "folder-open", &renzora::lang::t("export.section.output"), accent());

    // Binary name. (Empty initial value; `bind_text_input` reflects the current
    // state in on the first frame, so no `Init` snapshot is needed here.)
    let name_row = labeled(commands, fonts, &renzora::lang::t("export.field.name"));
    let name = text_input(commands, &fonts.ui, &renzora::lang::t("export.placeholder.binary_name"), "");
    style_input(commands, name);
    bind_text_input(commands, name, |w| w.get_resource::<ExportOverlayState>().map(|s| s.binary_name.clone()).unwrap_or_default(), |w, v| { if let Some(mut s) = w.get_resource_mut::<ExportOverlayState>() { s.binary_name = v; } });
    commands.entity(name_row).add_child(name);

    // Export directory.
    let dir_row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }).id();
    let dir_lbl = txt(commands, fonts, &renzora::lang::t("export.field.folder"), 12.0, text_muted());
    let dir = text_input(commands, &fonts.ui, &renzora::lang::t("export.placeholder.output_dir"), "");
    style_input(commands, dir);
    bind_text_input(commands, dir, |w| w.get_resource::<ExportOverlayState>().map(|s| s.output_dir.clone()).unwrap_or_default(), |w, v| { if let Some(mut s) = w.get_resource_mut::<ExportOverlayState>() { s.output_dir = v; } });
    let dir_browse = pill_button(commands, fonts, "folder", &renzora::lang::t("export.btn.browse"));
    commands.entity(dir_browse).insert(OutputBrowseBtn);
    commands.entity(dir_row).add_children(&[dir_lbl, dir, dir_browse]);

    // Icon.
    let icon_row = commands.spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), margin: UiRect::top(Val::Px(2.0)), ..default() }, Name::new("icon-row"))).id();
    let icon_lbl = txt(commands, fonts, &renzora::lang::t("export.field.icon"), 12.0, text_muted());
    let icon_path = commands.spawn((Text::new(renzora::lang::t("common.none")), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), Node { flex_grow: 1.0, ..default() }, bevy::text::TextLayout::no_wrap())).id();
    bind_text(commands, icon_path, |w| w.get_resource::<ExportOverlayState>().and_then(|s| s.icon_path.clone()).unwrap_or_else(|| renzora::lang::t("common.none")));
    let clear = commands.spawn((Node { padding: UiRect::all(Val::Px(2.0)), ..default() }, Interaction::default(), IconClearBtn, cursor())).id();
    let clx = icon_text(commands, &fonts.phosphor, "x", text_muted(), 12.0);
    commands.entity(clx).insert(FocusPolicy::Pass);
    commands.entity(clear).add_child(clx);
    bind_display(commands, clear, |w| w.get_resource::<ExportOverlayState>().is_some_and(|s| s.icon_path.is_some()));
    let icon_browse = pill_button(commands, fonts, "image", &renzora::lang::t("export.btn.browse"));
    commands.entity(icon_browse).insert(IconBrowseBtn);
    commands.entity(icon_row).add_children(&[icon_lbl, icon_path, clear, icon_browse]);

    commands.entity(body).add_children(&[name_row, dir_row, icon_row]);
    // Window, logging and dedicated-server options follow the output fields:
    // they all describe the artefact being produced, and splitting them across
    // two tabs meant "what am I making?" was answered in two places.
    let mut secs = vec![sec];
    secs.extend(options_sections(commands, fonts, p, desktop));
    finish_tab(commands, panel, &secs, tab_max);
    panel
}

// ── Packaging tab: packaging mode + runtime template status ──────────────────

fn build_packaging_tab(commands: &mut Commands, fonts: &EmberFonts, p: Platform, desktop: bool, host: bool, tab_max: f32) -> Entity {
    let panel = tab_panel(commands);
    let mut secs = Vec::new();

    // Packaging mode (desktop). The lean static single-binary mode recompiles
    // from source, which native cargo can only do for the host triple — so it's
    // offered only when exporting for the platform the editor is running on.
    if desktop {
        let (sec, body) = section(commands, fonts, "file-archive", &renzora::lang::t("export.section.packaging_mode"), accent());
        let separate = renzora::lang::t("export.packaging.separate");
        let single = renzora::lang::t("export.packaging.single_exe");
        let lean = renzora::lang::t("export.packaging.lean");
        let labels: Vec<&str> = if host {
            vec![separate.as_str(), single.as_str(), lean.as_str()]
        } else {
            vec![separate.as_str(), single.as_str()]
        };
        let radios = radio_group(commands, &fonts.ui, &labels, 0);
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
        // Which mode to actually ship. Said here rather than left implicit,
        // because the two copy-based modes are the fast ones and therefore the
        // ones a person reaches for by habit — while what they produce is the
        // editor's own runtime and its dylibs, not a build made for this game.
        let guidance = txt(commands, fonts, &renzora::lang::t("export.packaging.guidance"), 11.0, text_muted());
        commands.entity(body).add_child(guidance);
        if host {
            let hint = txt(commands, fonts, &renzora::lang::t("export.packaging.lean_hint"), 11.0, text_muted());
            commands.entity(body).add_child(hint);
        }
        // No source, no lean build — but that is a missing download rather than
        // a permanent limitation, so offer the download instead of leaving the
        // option greyed out with no way forward. A canonical editor ships
        // binaries only; the source rides the release as its own asset.
        if !lean_source_available() {
            let why = txt(commands, fonts, &renzora::lang::t("export.packaging.needs_source"), 11.0, AMBER);
            commands.entity(body).add_child(why);
            let btn = small_button(commands, fonts, "download-simple", &renzora::lang::t("export.packaging.get_source"), SourceDownloadBtn);
            commands.entity(btn).insert(Node { width: Val::Px(190.0), height: Val::Px(26.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::Center, column_gap: Val::Px(5.0), margin: UiRect::top(Val::Px(4.0)), border: UiRect::all(Val::Px(1.0)), border_radius: BorderRadius::all(Val::Px(4.0)), ..default() });
            commands.entity(body).add_child(btn);
        }
        secs.push(sec);
    }

    secs.push(build_runtime_status(commands, fonts, p));
    secs.push(build_modding_section(commands, fonts));
    // Compression and mesh optimisation are packaging decisions — how the build
    // is packed, not what it contains — so they sit here rather than behind a
    // tab of their own.
    secs.extend(compression_sections(commands, fonts));
    finish_tab(commands, panel, &secs, tab_max);
    panel
}

/// Runtime-template status section (installed line + Download/Install buttons +
/// download progress). Returns the section root for the caller to place.
/// Whether the exported game ships the plugin SDK, and can therefore compile
/// plugins a player adds.
///
/// On by default. A moddable game is the norm for this engine — the plugin
/// system is the same one the editor uses — and the cost of a wrong default
/// points one way: shipped without it a game cannot be modded at all, shipped
/// with it a game is merely larger.
fn build_modding_section(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let (sec, body) = section(commands, fonts, "puzzle-piece", &renzora::lang::t("export.section.modding"), accent());

    let row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() }).id();
    let cb = checkbox(commands, true);
    bind_2way(
        commands,
        cb,
        |w| w.get_resource::<ExportOverlayState>().is_some_and(|s| s.enable_modding),
        |w, v: &bool| {
            if let Some(mut s) = w.get_resource_mut::<ExportOverlayState>() {
                s.enable_modding = *v;
            }
        },
    );
    let label = txt(commands, fonts, &renzora::lang::t("export.modding.enable"), 12.5, text_primary());
    commands.entity(row).add_children(&[cb, label]);
    commands.entity(body).add_child(row);

    let hint = txt(commands, fonts, &renzora::lang::t("export.modding.hint"), 11.0, text_muted());
    commands.entity(body).add_child(hint);

    // A lean build links Bevy statically and shares no image, so there is
    // nothing for a plugin library to bind to — the SDK would ship and be
    // unusable. Said rather than silently ignored, since the checkbox is on by
    // default and a user picking lean would otherwise expect it to apply.
    let note = txt(commands, fonts, &renzora::lang::t("export.modding.lean_note"), 11.0, AMBER);
    bind_display(commands, note, |w| {
        w.get_resource::<ExportOverlayState>()
            .is_some_and(|s| s.packaging_mode == PackagingMode::LeanSingleBinary)
    });
    commands.entity(body).add_child(note);

    sec
}

fn build_runtime_status(commands: &mut Commands, fonts: &EmberFonts, p: Platform) -> Entity {
    let (sec, body) = section(commands, fonts, "download-simple", &renzora::lang::t("export.section.runtime_template"), accent());
    // Installed / not status line.
    let (line, msg) = icon_msg(commands, fonts, "check-circle", text_muted());
    bind_text(commands, msg, move |w| if w.get_resource::<TemplateManager>().is_some_and(|t| t.is_installed(p)) { renzora::lang::t("export.runtime.installed") } else { renzora::lang::t("export.runtime.not_installed") });
    commands.entity(body).add_child(line);
    // Buttons.
    let btns = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() }).id();
    let dl = pill_button(commands, fonts, "download-simple", &renzora::lang::t("export.btn.download_github"));
    commands.entity(dl).insert(DownloadBtn);
    let inst = pill_button(commands, fonts, "folder-open", &renzora::lang::t("export.btn.install_from_file"));
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

fn build_features_tab(commands: &mut Commands, fonts: &EmberFonts, host: bool, tab_max: f32) -> Entity {
    let panel = tab_panel(commands);
    let (sec, body) = section(commands, fonts, "sliders-horizontal", &renzora::lang::t("export.section.engine_features"), accent());
    if host {
        let note = txt(commands, fonts, &renzora::lang::t("export.features.note_host"), 11.0, text_muted());
        commands.entity(body).add_child(note);
        let list = commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), ..default() }).id();
        // Grouped into sections (3D rendering, 2D rendering, Systems, …) so the
        // two pipelines can be compared side by side instead of reading as one
        // 60-row wall. Within a section: parents first, each followed by its own
        // children, so the nesting still reads as a tree. Both orderings are
        // DERIVED rather than relying on the const being sorted — a capability
        // declared anywhere in the list lands in the right section, under the
        // right parent.
        let mut ordered: Vec<(Option<&str>, &crate::capabilities::Capability)> = Vec::new();
        for (sid, heading) in crate::capabilities::SECTIONS {
            let mut first = true;
            for parent in crate::capabilities::CAPABILITIES
                .iter()
                .filter(|c| c.group.is_none() && c.section == *sid)
            {
                // The heading rides on the first row of the section, so a section
                // that ends up empty never leaves a dangling header.
                ordered.push((first.then_some(*heading), parent));
                first = false;
                for child in crate::capabilities::CAPABILITIES
                    .iter()
                    .filter(|c| c.group == Some(parent.id))
                {
                    ordered.push((None, child));
                }
            }
        }
        for (idx, (heading, cap)) in ordered.into_iter().enumerate() {
            if let Some(heading) = heading {
                let sid = cap.section;
                // Header: [checkbox] TITLE ......... [chevron]
                //
                // The row itself is NOT interactive. The checkbox and the
                // fold zone are separate siblings, each owning its own
                // `Interaction` — an earlier version made the whole row the fold
                // control with buttons inside it, and pressing a button folded
                // the section as well as doing its job.
                let hrow = commands.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(8.0),
                        padding: UiRect { left: Val::Px(6.0), right: Val::Px(6.0), top: Val::Px(4.0), bottom: Val::Px(4.0) },
                        margin: UiRect { top: Val::Px(if idx == 0 { 0.0 } else { 8.0 }), bottom: Val::Px(2.0), ..default() },
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(ca(255, 255, 255, 10)),
                )).id();
                // Section checkbox: on when every capability in the section is on,
                // and writing it sets them all. Children included — a child is
                // meaningless without its parent, and the nested entries are where
                // most of the size lives.
                let scb = checkbox(commands, false);
                bind_2way(
                    commands,
                    scb,
                    move |w| {
                        w.get_resource::<ExportOverlayState>().is_some_and(|s| {
                            section_members(sid)
                                .all(|c| s.capabilities.get(c.id).copied().unwrap_or(c.default_on))
                        })
                    },
                    move |w, v: &bool| {
                        if let Some(mut s) = w.get_resource_mut::<ExportOverlayState>() {
                            for c in section_members(sid) {
                                s.capabilities.insert(c.id.to_string(), *v);
                            }
                        }
                    },
                );
                // Everything right of the checkbox folds the section.
                let fold = commands.spawn((
                    Node {
                        flex_grow: 1.0,
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.0),
                        ..default()
                    },
                    Interaction::default(),
                    SectionToggle(sid),
                    cursor(),
                )).id();
                let ht = commands.spawn((
                    Text::new(heading.to_string()),
                    ui_font(&fonts.ui, 11.0),
                    TextColor(rgb(text_primary())),
                    Node { flex_grow: 1.0, ..default() },
                    FocusPolicy::Pass,
                )).id();
                // Chevron direction tracks the fold state, so the row reads as a
                // control rather than decoration.
                let chev = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 11.0);
                commands.entity(chev).insert(FocusPolicy::Pass);
                bind_text(commands, chev, move |w| {
                    let collapsed = w
                        .get_resource::<ExportOverlayState>()
                        .is_some_and(|s| s.collapsed_sections.contains(sid));
                    let name = if collapsed { "caret-right" } else { "caret-down" };
                    renzora_ember::phosphor_map::icon_glyph(name)
                        .unwrap_or('\u{E4C6}')
                        .to_string()
                });
                commands.entity(fold).add_children(&[ht, chev]);
                commands.entity(hrow).add_children(&[scb, fold]);
                commands.entity(list).add_child(hrow);
            }
            let id = cap.id;
            let child = cap.group.is_some();
            // One padded, zebra-striped item per capability (checkbox + label + help).
            // Children are indented and sit flush against the parent row rather
            // than being striped, so the grouping is legible without a header.
            let item = commands.spawn((Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(2.0), padding: UiRect { left: Val::Px(if child { 24.0 } else { 6.0 }), right: Val::Px(6.0), top: Val::Px(5.0), bottom: Val::Px(5.0) }, border_radius: BorderRadius::all(Val::Px(3.0)), ..default() }, BackgroundColor(if child { Color::NONE } else { row_stripe(idx) }))).id();
            // Fold: hide the row when its section is collapsed. Reactive rather
            // than a rebuild, so the checkboxes and scroll position survive.
            let sid = cap.section;
            bind_display(commands, item, move |w| {
                !w.get_resource::<ExportOverlayState>()
                    .is_some_and(|s| s.collapsed_sections.contains(sid))
            });
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
            // Localize the capability label + help (the Features list). Keys are
            // `export.cap.<id>.{label,help}`, falling back to the English const.
            let cap_label = renzora::lang::t_or(&format!("export.cap.{id}.label"), cap.label);
            let cap_help = renzora::lang::t_or(&format!("export.cap.{id}.help"), cap.help);
            let t = txt(commands, fonts, &cap_label, 12.0, text_primary());
            commands.entity(row).add_children(&[cb, t]);
            let help = txt(commands, fonts, &cap_help, 10.0, text_muted());
            commands.entity(item).add_children(&[row, help]);
            commands.entity(list).add_child(item);
        }
        commands.entity(body).add_child(list);
    } else {
        let note = txt(commands, fonts, &renzora::lang::t("export.features.note_nonhost"), 11.0, text_muted());
        commands.entity(body).add_child(note);
    }
    finish_tab(commands, panel, &[sec], tab_max);
    panel
}

// ── Plugins tab ──────────────────────────────────────────────────────────────

fn build_plugins_tab(commands: &mut Commands, fonts: &EmberFonts, host: bool, tab_max: f32) -> Entity {
    let panel = tab_panel(commands);
    let mut secs = Vec::new();

    // How the plugins get there: files beside the binary, or compiled into it.
    // Offered only on the host platform, because linking in requires the lean
    // recompile and that can only target the triple the editor is running on.
    if host {
        let (lsec, lbody) = section(commands, fonts, "link", &renzora::lang::t("export.section.plugin_link"), accent());
        let files = renzora::lang::t("export.plugin_link.files");
        let linked = renzora::lang::t("export.plugin_link.linked");
        let labels: Vec<&str> = vec![files.as_str(), linked.as_str()];
        let radios = radio_group(commands, &fonts.ui, &labels, 0);
        bind_2way(
            commands,
            radios,
            |w| match w.resource::<ExportOverlayState>().plugin_link_mode {
                PluginLinkMode::ShipFiles => 0usize,
                PluginLinkMode::LinkIn => 1,
            },
            |w, v: &usize| {
                w.resource_mut::<ExportOverlayState>().plugin_link_mode = match *v {
                    1 => PluginLinkMode::LinkIn,
                    _ => PluginLinkMode::ShipFiles,
                };
            },
        );
        commands.entity(lbody).add_child(radios);
        let hint = txt(commands, fonts, &renzora::lang::t("export.plugin_link.hint"), 11.0, text_muted());
        commands.entity(lbody).add_child(hint);
        // Linking in needs something to compile into, and only the lean mode
        // compiles. Rather than disable the radio from the other tab (where the
        // reason would be invisible), say so — and only when it applies.
        let warn = txt(commands, fonts, &renzora::lang::t("export.plugin_link.needs_lean"), 11.0, AMBER);
        bind_display(commands, warn, |w| {
            w.get_resource::<ExportOverlayState>().is_some_and(|s| {
                s.plugin_link_mode == PluginLinkMode::LinkIn
                    && s.packaging_mode != PackagingMode::LeanSingleBinary
            })
        });
        commands.entity(lbody).add_child(warn);
        secs.push(lsec);
    }

    let (sec, body) = section(commands, fonts, "puzzle-piece", &renzora::lang::t("export.section.plugins"), accent());
    // A wrapping grid of thumbnail cards, matching Settings → Plugins. This was
    // a zebra-striped list of checkboxes: seventy identical rows in which the
    // only way to tell one plugin from another was to read it. The artwork does
    // that work, and the two panels now answer "which plugins?" the same way.
    let list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(8.0),
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    commands.entity(body).add_child(list);

    // Said only when it applies, like the Plugin Linking warning above it. The
    // native plugins in this list are real choices in a host copy-based export
    // and silently ignored in any other, so the list would otherwise be quietly
    // lying in exactly the configurations where it matters most.
    let native_note = txt(
        commands,
        fonts,
        &renzora::lang::t("export.plugins.native_host_only"),
        11.0,
        AMBER,
    );
    bind_display(commands, native_note, |w| {
        w.get_resource::<ExportOverlayState>().is_some_and(|s| {
            !matches!(
                s.packaging_mode,
                PackagingMode::SeparateFiles | PackagingMode::SingleBinary
            ) || Platform::current() != Some(s.platform)
        })
    });
    commands.entity(body).add_child(native_note);

    // Filled by a command that can read the world (the plugin list is stable
    // after the scan).
    commands.queue(move |world: &mut World| {
        let plugins: Vec<(String, String)> = world.get_resource::<ExportOverlayState>().map(|s| s.available_plugins.iter().map(|p| (p.id.clone(), format!("{:?}", p.scope))).collect()).unwrap_or_default();
        let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
        let mut queue = CommandQueue::default();
        {
            let mut c = Commands::new(&mut queue, world);
            if plugins.is_empty() {
                let note = c.spawn((Text::new(renzora::lang::t("export.plugins.none")), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())))).id();
                c.entity(list).add_child(note);
            }
            for (id, scope) in plugins.into_iter() {
                let card = c
                    .spawn((
                        Node {
                            // Four columns, expressed as a percentage basis
                            // rather than a pixel one. 22% × 4 = 88%, and the
                            // three 8px gaps between them fit in the remaining
                            // 12% at any realistic panel width — so four wrap
                            // onto a row and a fifth cannot, whatever the dialog
                            // is resized to. `flex_grow` then shares the leftover
                            // space so the row still fills edge to edge. A pixel
                            // basis would give four columns at exactly one width.
                            flex_basis: Val::Percent(22.0),
                            flex_grow: 1.0,
                            min_width: Val::Px(0.0),
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(6.0),
                            padding: UiRect::all(Val::Px(9.0)),
                            border_radius: BorderRadius::all(Val::Px(6.0)),
                            // The card is the clipping boundary for a long plugin
                            // name. Without it `chromatic_aberration` ran out
                            // past the card's edge and over its neighbour.
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        BackgroundColor(rgb(card_bg())),
                    ))
                    .id();

                let thumb = renzora_ember::widgets::file_image_tile(
                    &mut c,
                    &fonts,
                    renzora::core::plugin_thumbnail_path(&id).unwrap_or_default(),
                    "puzzle-piece",
                    text_muted(),
                    10.0,
                );

                // The name gets the card's full width on its own line. It used to
                // share a row with the switch, which left a narrow column for a
                // name like `chromatic_aberration` and pushed it off the card.
                // `width: 100%` matters as much as `no_wrap` here: a no-wrap text
                // node sizes itself to its content, so clipping it needs a width
                // to clip against.
                let name = c
                    .spawn((
                        Text::new(id.clone()),
                        ui_font(&fonts.ui, 11.5),
                        TextColor(rgb(text_primary())),
                        bevy::text::TextLayout::no_wrap(),
                        Node {
                            width: Val::Percent(100.0),
                            min_width: Val::Px(0.0),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                    ))
                    .id();

                // Footer: scope on the left, the switch pinned right. The switch
                // is a fixed 28px, so putting it at the end of a row the name no
                // longer competes for keeps every card's control in the same
                // place — a column of switches you can run your eye down.
                let foot = c
                    .spawn(Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.0),
                        ..default()
                    })
                    .id();
                let scope_t = c
                    .spawn((
                        Text::new(scope),
                        ui_font(&fonts.ui, 9.0),
                        TextColor(rgb(text_muted())),
                        bevy::text::TextLayout::no_wrap(),
                        Node { flex_grow: 1.0, min_width: Val::Px(0.0), overflow: Overflow::clip(), ..default() },
                    ))
                    .id();

                // A switch, not a checkbox: this is "ship it / don't", which is
                // an on-off state rather than an item ticked off a list, and it
                // matches the switch the Settings panel uses for the same
                // decision about the same plugins.
                let sw = toggle_switch(&mut c, true);
                // Bevy 0.19 defaults `FocusPolicy` to `Pass`, so a switch that
                // does not block hands its press to everything behind it.
                c.entity(sw).insert(FocusPolicy::Block);
                let id2 = id.clone();
                bind_2way(&mut c, sw, move |w| w.get_resource::<ExportOverlayState>().is_some_and(|s| s.selected_plugins.contains(&id2)), {
                    let id3 = id.clone();
                    move |w, v: &bool| {
                        if let Some(mut s) = w.get_resource_mut::<ExportOverlayState>() {
                            if *v { s.selected_plugins.insert(id3.clone()); } else { s.selected_plugins.remove(&id3); }
                        }
                    }
                });
                c.entity(foot).add_children(&[scope_t, sw]);

                c.entity(card).add_children(&[thumb, name, foot]);
                c.entity(list).add_child(card);
            }
        }
        queue.apply(world);
    });
    secs.push(sec);
    finish_tab(commands, panel, &secs, tab_max);
    panel
}

// ── Compression tab: asset compression + mesh optimization ───────────────────

/// Compression + mesh-optimisation sections.
///
/// Returns sections rather than a tab: both are decisions about how the build is
/// packed, so they live under Packaging rather than behind a tab of their own.
fn compression_sections(commands: &mut Commands, fonts: &EmberFonts) -> Vec<Entity> {
    // Asset compression level.
    let (csec, cbody) = section(commands, fonts, "file-archive", &renzora::lang::t("export.section.compression"), accent());
    let crow = labeled(commands, fonts, &renzora::lang::t("export.field.compression_level"));
    let dv = drag_value(commands, &fonts.ui, "", text_primary(), 0.0, 1.0);
    bind_2way(commands, dv, |w| w.resource::<ExportOverlayState>().compression_level as f32, |w, v: &f32| w.resource_mut::<ExportOverlayState>().compression_level = (v.round() as i32).clamp(1, 22));
    commands.entity(crow).add_child(dv);
    commands.entity(cbody).add_child(crow);

    // Binary compression (UPX). Sits in the same section as the asset
    // compression level because the two answer one question — how small is the
    // shipped folder — even though one is an rpak setting and the other a
    // post-build pass over the executable.
    let upx = check_state(commands, fonts, &renzora::lang::t("export.compression.upx"), |s| s.upx_compress, |s, v| s.upx_compress = v);
    commands.entity(cbody).add_child(upx);
    let upx_help = txt(commands, fonts, &renzora::lang::t("export.compression.upx_help"), 10.0, text_muted());
    commands.entity(cbody).add_child(upx_help);

    // Mesh optimization.
    let (msec, mbody) = section(commands, fonts, "cube", &renzora::lang::t("export.section.mesh_opt"), accent());
    let simplify = check_state(commands, fonts, &renzora::lang::t("export.mesh.simplify"), |s| s.mesh_simplify, |s, v| s.mesh_simplify = v);
    commands.entity(mbody).add_child(simplify);
    let ratio = labeled(commands, fonts, &renzora::lang::t("export.field.keep_ratio"));
    let dvr = drag_value(commands, &fonts.ui, "", text_primary(), 0.0, 0.01);
    bind_2way(commands, dvr, |w| w.resource::<ExportOverlayState>().mesh_simplify_ratio, |w, v: &f32| w.resource_mut::<ExportOverlayState>().mesh_simplify_ratio = v.clamp(0.1, 1.0));
    commands.entity(ratio).add_child(dvr);
    commands.entity(ratio).insert(Node { margin: UiRect::left(Val::Px(20.0)), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() });
    bind_display(commands, ratio, |w| w.resource::<ExportOverlayState>().mesh_simplify);
    commands.entity(mbody).add_child(ratio);
    let quant = check_state(commands, fonts, &renzora::lang::t("export.mesh.quantize"), |s| s.mesh_quantize, |s, v| s.mesh_quantize = v);
    let lods = check_state(commands, fonts, &renzora::lang::t("export.mesh.generate_lods"), |s| s.mesh_generate_lods, |s, v| s.mesh_generate_lods = v);
    commands.entity(mbody).add_children(&[quant, lods]);
    let levels = labeled(commands, fonts, &renzora::lang::t("export.field.lod_levels"));
    let dvl = drag_value(commands, &fonts.ui, "", text_primary(), 0.0, 1.0);
    bind_2way(commands, dvl, |w| w.resource::<ExportOverlayState>().mesh_lod_levels as f32, |w, v: &f32| w.resource_mut::<ExportOverlayState>().mesh_lod_levels = (v.round() as u32).clamp(1, 5));
    commands.entity(levels).add_child(dvl);
    commands.entity(levels).insert(Node { margin: UiRect::left(Val::Px(20.0)), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() });
    bind_display(commands, levels, |w| w.resource::<ExportOverlayState>().mesh_generate_lods);
    commands.entity(mbody).add_child(levels);
    vec![csec, msec]
}

// ── Options tab: window + flags ──────────────────────────────────────────────

/// Window / logging / server sections.
///
/// Returns sections rather than a tab: these describe the thing being produced —
/// what window it opens, whether it logs — so they belong with Output.
fn options_sections(commands: &mut Commands, fonts: &EmberFonts, p: Platform, desktop: bool) -> Vec<Entity> {
    let mut secs = Vec::new();

    // Window (desktop).
    if desktop {
        let (wsec, wbody) = section(commands, fonts, "monitor", &renzora::lang::t("export.section.window"), accent());
        let windowed = renzora::lang::t("export.window.windowed");
        let fullscreen = renzora::lang::t("export.window.fullscreen");
        let borderless = renzora::lang::t("export.window.borderless");
        let radios = radio_group(commands, &fonts.ui, &[windowed.as_str(), fullscreen.as_str(), borderless.as_str()], 0);
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
        let szl = txt(commands, fonts, &renzora::lang::t("export.field.size"), 12.0, text_muted());
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
    let (osec, obody) = section(commands, fonts, "gear", &renzora::lang::t("export.section.options"), accent());
    let console = check_state(commands, fonts, &renzora::lang::t("export.options.console_logging"), |s| s.console_logging, |s, v| s.console_logging = v);
    commands.entity(obody).add_child(console);
    if desktop && p.supports_dedicated_server() {
        let server = check_state(commands, fonts, &renzora::lang::t("export.options.include_server"), |s| s.include_server, |s, v| s.include_server = v);
        commands.entity(obody).add_child(server);
    }
    secs.push(osec);
    secs
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
    value: impl Fn(&Rx) -> f32 + Send + Sync + 'static,
) {
    react(commands, move |w: &mut World| {
        if w.get_entity(target).is_err() {
            return false;
        }
        let v = value(&Rx::new(&*w)).clamp(0.0, 1.0);
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
        Some(ExportProgress::Done(_)) => renzora::lang::t("export.status.complete"),
        Some(ExportProgress::Error(_)) => renzora::lang::t("export.status.failed"),
        _ => renzora::lang::t("export.status.exporting"),
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
    let copy_btn = pill_button(commands, fonts, "clipboard", &renzora::lang::t("export.btn.copy_log"));
    commands.entity(copy_btn).insert(CopyLogBtn);
    let btn = commands.spawn((Node { min_width: Val::Px(100.0), height: Val::Px(32.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, justify_content: JustifyContent::Center, border_radius: BorderRadius::all(Val::Px(5.0)), ..default() }, BackgroundColor(rgb(section_bg())), Interaction::default(), CancelOrBackBtn, cursor())).id();
    let btn_label = commands.spawn((Text::new(renzora::lang::t("common.cancel")), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_primary())), FocusPolicy::Pass)).id();
    bind_text(commands, btn_label, |w| {
        if w.get_resource::<ExportOverlayState>().map(|s| s.active_task.is_some()).unwrap_or(false) {
            renzora::lang::t("common.cancel")
        } else {
            renzora::lang::t("export.btn.back")
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
    let t = commands.spawn((Text::new(renzora::lang::t("common.export")), ui_font(&fonts.ui, 13.0), TextColor(Color::WHITE), FocusPolicy::Pass)).id();
    commands.entity(btn).add_children(&[ic, t]);
    commands.entity(row).add_child(btn);
    commands.entity(panel).add_child(row);
}

// ── Interaction ──────────────────────────────────────────────────────────────

fn can_export(w: &Rx) -> bool {
    let Some(s) = w.get_resource::<ExportOverlayState>() else { return false };
    let installed = w.get_resource::<TemplateManager>().is_some_and(|t| t.is_installed(s.platform));
    installed && !s.output_dir.is_empty() && s.active_task.is_none() && matches!(s.progress, ExportProgress::Idle | ExportProgress::Done(_) | ExportProgress::Error(_))
}

fn preset_click(q: Query<(&Interaction, &PresetBtn), Changed<Interaction>>, mut state: Option<ResMut<ExportOverlayState>>) {
    let Some(state) = state.as_mut() else { return };
    for (i, b) in &q {
        if *i == Interaction::Pressed {
            // Carries the outgoing preset's edits across and persists them.
            state.select_preset(b.0);
        }
    }
}

fn preset_dup_click(q: Query<&Interaction, (With<PresetDupBtn>, Changed<Interaction>)>, mut state: Option<ResMut<ExportOverlayState>>) {
    let Some(state) = state.as_mut() else { return };
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    // Duplicate what is on screen, not what was last saved — the edits made
    // since selecting are the reason to duplicate rather than add.
    state.sync_active_preset();
    let Some(src) = state.active_preset.and_then(|i| state.presets.get(i)).cloned() else { return };
    let mut copy = src;
    copy.name = crate::presets::unique_name(&copy.name, &state.presets);
    state.presets.push(copy);
    state.active_preset = Some(state.presets.len() - 1);
    state.save_presets();
}

fn preset_del_click(q: Query<&Interaction, (With<PresetDelBtn>, Changed<Interaction>)>, mut state: Option<ResMut<ExportOverlayState>>) {
    let Some(state) = state.as_mut() else { return };
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let Some(i) = state.active_preset else { return };
    if i >= state.presets.len() {
        return;
    }
    state.presets.remove(i);
    // Select the neighbour that took its place, or the new last one if the
    // removed preset was at the end. `None` when nothing is left, which the
    // sidebar renders as the empty state.
    state.active_preset = if state.presets.is_empty() {
        None
    } else {
        Some(i.min(state.presets.len() - 1))
    };
    if let Some(p) = state.active_preset.and_then(|i| state.presets.get(i)).cloned() {
        p.apply(state);
    }
    state.save_presets();
}

fn close_click(q: Query<&Interaction, (With<CloseBtn>, Changed<Interaction>)>, mut state: Option<ResMut<ExportOverlayState>>) {
    let Some(state) = state.as_mut() else { return };
    if q.iter().any(|i| *i == Interaction::Pressed) {
        // Persist before hiding. Every field in the form writes to the flat
        // working state rather than into the preset, so without this an edit
        // made and then closed would look accepted and be gone at the next
        // open — the exact failure the preset system exists to remove. The
        // other paths that lose the working state (switch, add, duplicate,
        // remove) already sync; closing was the one that did not.
        state.sync_active_preset();
        state.save_presets();

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
        // See `crash_overlay`: the browser clipboard is async + gesture-gated,
        // so there is nothing to call from a sync system. No-op on wasm.
        #[cfg(not(target_arch = "wasm32"))]
        if let Ok(mut cb) = arboard::Clipboard::new() {
            let _ = cb.set_text(state.build_log.join("\n"));
        }
    }
}

/// Fold / unfold a Features-tab section.
fn section_toggle_click(
    q: Query<(&Interaction, &SectionToggle), Changed<Interaction>>,
    mut state: Option<ResMut<ExportOverlayState>>,
) {
    let Some(state) = state.as_mut() else { return };
    for (i, sec) in q.iter() {
        if *i == Interaction::Pressed && !state.collapsed_sections.remove(sec.0) {
            state.collapsed_sections.insert(sec.0.to_string());
        }
    }
}

/// Every capability rendered under one section heading, parents and children.
///
/// A child is placed by its PARENT's section — `group` decides nesting and
/// `section` decides placement, and the two agree by construction, but resolving
/// through the parent means a mismatch can't leave a visible row out of the
/// header checkbox's reach.
fn section_members(sid: &'static str) -> impl Iterator<Item = &'static crate::capabilities::Capability> {
    crate::capabilities::CAPABILITIES.iter().filter(move |c| {
        let owning = c
            .group
            .and_then(|p| crate::capabilities::CAPABILITIES.iter().find(|x| x.id == p))
            .map_or(c.section, |p| p.section);
        owning == sid
    })
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
            if can_export(&Rx::new(&*w)) {
                // Persist what is about to be built. An export is the moment a
                // configuration is proven to be the one you wanted, and a build
                // that takes minutes is exactly when the editor is most likely
                // to be closed or to crash before the settings were saved.
                if let Some(mut state) = w.get_resource_mut::<ExportOverlayState>() {
                    state.sync_active_preset();
                    state.save_presets();
                }
                let name = w.get_resource::<renzora::core::CurrentProject>().map(|p| p.config.name.clone()).unwrap_or_default();
                run_export(w, &name);
            }
        });
    }
}

fn output_browse_click(q: Query<&Interaction, (With<OutputBrowseBtn>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|w: &mut World| {
            if let Some(dir) = rfd::FileDialog::new().set_title(renzora::lang::t("export.dialog.select_output")).pick_folder() {
                w.resource_mut::<ExportOverlayState>().output_dir = dir.to_string_lossy().to_string();
            }
        });
    }
}

fn icon_browse_click(q: Query<&Interaction, (With<IconBrowseBtn>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|w: &mut World| {
            if let Some(f) = rfd::FileDialog::new().set_title(renzora::lang::t("export.dialog.select_icon")).add_filter(renzora::lang::t("export.filter.images"), &["png", "ico", "svg"]).pick_file() {
                w.resource_mut::<ExportOverlayState>().icon_path = Some(f.to_string_lossy().to_string());
            }
        });
    }
}

/// Fetch the engine source so a canonical editor can do a lean build.
///
/// Reuses the template download task and its progress line, so the modal renders
/// it with no extra plumbing. The lean radio option becomes selectable on the
/// next right-pane rebuild, once `resolve_engine_source` can see the extracted
/// tree.
fn source_download_click(
    q: Query<&Interaction, (With<SourceDownloadBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|w: &mut World| {
            let p = w.resource::<ExportOverlayState>().platform;
            let Some(release) = w.resource::<ExportOverlayState>().release_info.clone() else {
                let mut s = w.resource_mut::<ExportOverlayState>();
                s.download_status = Some((
                    p,
                    DownloadProgress::Error(renzora::lang::t("export.status.no_release")),
                ));
                return;
            };
            let task = download::spawn_source_download(p, release);
            let mut s = w.resource_mut::<ExportOverlayState>();
            s.download_task = Some(task);
            s.download_status = Some((
                p,
                DownloadProgress::Fetching(renzora::lang::t("export.status.download_starting")),
            ));
        });
    }
}

fn download_click(q: Query<&Interaction, (With<DownloadBtn>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|w: &mut World| {
            let p = w.resource::<ExportOverlayState>().platform;
            // The release is resolved as soon as the modal opens, so a click can
            // reuse it — no second GitHub round-trip, and nothing to resolve on
            // the worker thread that could have resolved differently.
            let Some(release) = w.resource::<ExportOverlayState>().release_info.clone() else {
                let mut s = w.resource_mut::<ExportOverlayState>();
                s.download_status = Some((p, DownloadProgress::Error(renzora::lang::t("export.status.no_release"))));
                return;
            };
            let task = download::spawn_download(p, release);
            let mut s = w.resource_mut::<ExportOverlayState>();
            s.download_task = Some(task);
            s.download_status = Some((p, DownloadProgress::Fetching(renzora::lang::t("export.status.download_starting"))));
        });
    }
}

fn install_click(q: Query<&Interaction, (With<InstallBtn>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|w: &mut World| {
            let Some(file) = rfd::FileDialog::new().set_title(renzora::lang::t("export.dialog.select_runtime")).pick_file() else { return };
            let p = w.resource::<ExportOverlayState>().platform;
            // Into the per-user template store, NOT the editor's own directory.
            // `runtime_binary_name()` for a desktop platform is `renzora[.exe]`,
            // so the old destination would have overwritten the editor's own
            // runtime with a foreign-platform binary — and taken Play with it.
            let Some(dir) = crate::templates::user_template_dir(p) else { return };
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::copy(&file, dir.join(p.runtime_binary_name()));
            w.resource_mut::<TemplateManager>().scan();
        });
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn platform_icon(p: Platform) -> &'static str {
    match p {
        Platform::WindowsX64 | Platform::WindowsArm64 => "windows-logo",
        Platform::LinuxX64 | Platform::LinuxArm64 => "linux-logo",
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

/// Zebra-stripe background for list rows — long lists (features, plugins) read
/// as discrete rows.
///
/// BOTH rows are tinted, which looks like a needless change and is not: an
/// unchecked checkbox is a 1px border over `Color::NONE`, and against the bare
/// panel that border is invisible. Odd rows carried a faint overlay and even
/// rows nothing, so exactly half the feature list appeared to have no control at
/// all — every other row looked like a label. The lighter of the two tints is
/// what makes an empty checkbox legible; the difference between them is what
/// still stripes the list.
fn row_stripe(idx: usize) -> Color {
    if idx % 2 == 1 { ca(255, 255, 255, 14) } else { ca(255, 255, 255, 6) }
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
