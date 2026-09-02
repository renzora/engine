//! Opening and closing the dialog, and the three reads it defers until they are
//! actually needed: the Docker probe, the plugin scan, and the project file crawl.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use renzora_ember::font::EmberFonts;

use crate::overlay::{
    ensure_release_fetch, poll_download_task, poll_export_task, poll_release_fetch,
    ExportOverlayState, ExportProgress, PackagingMode,
};
use crate::templates::{Platform, TemplateManager};

use super::{ExportRoot, FilesAction, FilesBulk, FilesPanel, LogScroll};

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

pub(super) fn manage_export_modal(world: &mut World) {
    let visible = world.get_resource::<ExportOverlayState>().is_some_and(|s| s.visible);
    let mut q = world.query_filtered::<Entity, With<ExportRoot>>();
    let existing: Vec<Entity> = q.iter(world).collect();
    // The modal is opening this frame: throw away the previous scan so the
    // features and plugins are re-detected from the project as it stands right
    // now. Editing a scene and re-opening the dialog should change what it
    // offers, and before this the first scan of the session was the only one — a
    // project that gained a terrain after the dialog had been opened once still
    // exported without it.
    //
    // Unconditional, saved presets included. A preset's feature map is captured
    // automatically (`sync_active_preset` runs on every close and every export),
    // so it records the last state rather than a decision, and honouring it here
    // pinned every project that had ever been exported to whatever its map
    // happened to hold. A preset is for the platform, packaging, output path and
    // window — the features are the project's own answer, re-read each time.
    //
    // Done before `scan_plugins` rather than in the spawn branch below, because
    // the checkboxes are built from `capabilities` at spawn time and a re-scan a
    // frame later would leave them showing the old answer.
    if visible && existing.is_empty() {
        if let Some(mut s) = world.get_resource_mut::<ExportOverlayState>() {
            s.plugins_scanned = false;
            s.plugins_scanned_for = None;
            s.choices_pinned = false;
        }
    }
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
        follow_log_tail(world);
        resolve_included_files(world);
        poll_download_task(world);
        scan_plugins(world);
    }

    if visible && existing.is_empty() {
        let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
        let has_project = world.get_resource::<renzora::core::CurrentProject>().is_some();
        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            super::frame::spawn_modal(&mut commands, &fonts, has_project);
        }
        queue.apply(world);
    } else if !visible && !existing.is_empty() {
        for e in existing {
            world.entity_mut(e).despawn();
        }
    }
}

/// Fill the Files tab's ticks the first time it is looked at, and run its
/// buttons.
///
/// The starting ticks are what the automatic crawl would pack, and working that
/// out means reading the project — every scene, every script, every referenced
/// file. That is the same cost the export itself pays, and it is far too much to
/// pay when the dialog merely opens, so it is deferred until this tab is
/// actually on screen. The checkboxes bind reactively, so they simply fill in on
/// the frame the answer arrives.
fn resolve_included_files(world: &mut World) {
    // Bulk buttons first: "Reset to detected" clears the selection, and the
    // block below then recomputes it in the same frame.
    let mut action = None;
    {
        let mut q = world.query::<(&Interaction, &FilesBulk)>();
        for (interaction, bulk) in q.iter(world) {
            if *interaction == Interaction::Pressed {
                action = Some(bulk.0);
            }
        }
    }
    if let Some(action) = action {
        let all: Vec<String> = world
            .get_resource::<ExportOverlayState>()
            .map(|s| s.project_files.clone())
            .unwrap_or_default();
        if let Some(mut s) = world.get_resource_mut::<ExportOverlayState>() {
            match action {
                FilesAction::All => s.included_files = Some(all.into_iter().collect()),
                FilesAction::None => s.included_files = Some(Default::default()),
                FilesAction::Detected => s.included_files = None,
            }
        }
    }

    // Is the tab on screen? `tabs()` shows one panel at a time by toggling
    // `Node.display`, so that is the question to ask.
    let showing = {
        let mut q = world.query_filtered::<&Node, With<FilesPanel>>();
        q.iter(world).any(|n| n.display != Display::None)
    };
    if !showing {
        return;
    }
    let needs = world
        .get_resource::<ExportOverlayState>()
        .is_some_and(|s| s.included_files.is_none());
    if !needs {
        return;
    }
    let Some(root) = world
        .get_resource::<renzora::core::CurrentProject>()
        .map(|p| p.path.clone())
    else {
        return;
    };
    let detected = renzora_rpak::referenced_keys(&root).unwrap_or_default();
    if let Some(mut s) = world.get_resource_mut::<ExportOverlayState>() {
        s.included_files = Some(detected.into_iter().collect());
    }
}

/// Put the end of the build log on screen the moment the build stops.
///
/// The log view already follows the tail while output streams in, and already
/// releases that follow if the reader scrolls up — which is right during a
/// build, and wrong the instant it ends. What matters then is the last line: an
/// export that failed says why on it, and cargo will have pushed that a long way
/// below the fold. A reader who had scrolled up to watch the packing list went
/// looking for an error they could not see.
///
/// Fires once per finish, on the transition into `Done`/`Error`, so it never
/// fights someone scrolling back through a finished log.
fn follow_log_tail(world: &mut World) {
    let finished = matches!(
        world.get_resource::<ExportOverlayState>().map(|s| &s.progress),
        Some(ExportProgress::Done(_)) | Some(ExportProgress::Error(_))
    );
    {
        let mut state = world.get_resource_or_insert_with(LogTailState::default);
        if finished == state.finished {
            return;
        }
        state.finished = finished;
    }
    if !finished {
        return;
    }
    let mut q = world.query_filtered::<&mut renzora_ember::widgets::EmberScroll, With<LogScroll>>();
    for mut scroll in q.iter_mut(world) {
        scroll.stick_to_bottom();
    }
}

/// Whether [`follow_log_tail`] has already handled the current finish.
#[derive(Resource, Default)]
struct LogTailState {
    finished: bool,
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

    // Read the project once, and let it choose both halves of the dialog: which
    // plugins to pre-tick, and which engine features to leave on.
    //
    // A plugin id is the dll stem (e.g. `renzora_matrix`); a scene names
    // components by their defining crate (`renzora_matrix::MatrixSettings`), so
    // the needle is `<id>::`. The scan reads scripts, markup and authored assets
    // as well as scenes, so a plugin a script reaches for is now found too —
    // this was scene-only, and the note admitting it said the user could tick
    // those by hand.
    //
    // With no project, or a project holding no scenes, `scan` is `None` and both
    // halves fall back to selecting everything / plain defaults.
    let project_root = world
        .get_resource::<renzora::core::CurrentProject>()
        .map(|p| p.path.clone());
    let needles: Vec<(String, String)> = plugins
        .iter()
        .map(|p| {
            let crate_name = p.id.strip_prefix("lib").unwrap_or(p.id.as_str());
            (p.id.clone(), format!("{crate_name}::"))
        })
        .collect();
    let extra: Vec<String> = needles.iter().map(|(_, n)| n.clone()).collect();
    let scan = project_root
        .as_deref()
        .map(|root| crate::capabilities::scan_project(root, &extra))
        .filter(|s| s.saw_scene);

    let mut s = world.resource_mut::<ExportOverlayState>();
    // Both halves are skipped when a preset is driving the dialog: a preset is a
    // saved answer to exactly these two questions, and re-deriving them here
    // would throw it away one frame after the user loaded it.
    // The project's file list, for the Files tab. Cleared with the rest when the
    // scan finds no project, so a stale list cannot outlive the project it came
    // from.
    s.project_files = scan.as_ref().map(|sc| sc.files.clone()).unwrap_or_default();
    if !s.choices_pinned {
        // A fresh open re-reads the project, so any selection made against the
        // previous read is stale — a file that has since been deleted would keep
        // being asked for, and a new one would be invisible. Back to automatic.
        s.included_files = None;
        // Replaced, not merged. This runs again on every fresh open, so merging
        // would make the selection a high-water mark — a plugin detected once
        // would stay ticked after the scene that used it was deleted.
        s.selected_plugins.clear();
        for (id, needle) in &needles {
            if scan.as_ref().is_none_or(|scan| scan.saw(needle)) {
                s.selected_plugins.insert(id.clone());
            }
        }
        let selected: Vec<String> = s.selected_plugins.iter().cloned().collect();
        s.capabilities = crate::capabilities::defaults_from_scan(&selected, scan.as_ref());
    }
    s.available_plugins = plugins;
    s.plugins_scanned = true;
    s.plugins_scanned_for = Some(platform);
}
