//! Every click the dialog answers, and the save-before-export prompt that
//! stands between the Export button and the build.

use std::sync::atomic::Ordering;

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use renzora_ember::font::EmberFonts;
use renzora_ember::reactive::Rx;
use renzora_ember::theme::text_primary;

use crate::download::{self, DownloadProgress};
use crate::overlay::{run_export, ExportOverlayState, ExportProgress, ExportView};
use crate::templates::TemplateManager;

use super::frame::can_export;
use super::widgets::txt;
use super::{
    CancelOrBackBtn, CloseBtn, CopyLogBtn, DownloadBtn, ExportBtn, IconBrowseBtn, IconClearBtn,
    InstallBtn, OutputBrowseBtn, PresetBtn, PresetDelBtn, PresetDupBtn, SectionToggle,
    SourceDownloadBtn,
};

pub(super) fn preset_click(q: Query<(&Interaction, &PresetBtn), Changed<Interaction>>, mut state: Option<ResMut<ExportOverlayState>>) {
    let Some(state) = state.as_mut() else { return };
    for (i, b) in &q {
        if *i == Interaction::Pressed {
            // Carries the outgoing preset's edits across and persists them.
            state.select_preset(b.0);
        }
    }
}

pub(super) fn preset_dup_click(q: Query<&Interaction, (With<PresetDupBtn>, Changed<Interaction>)>, mut state: Option<ResMut<ExportOverlayState>>) {
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

pub(super) fn preset_del_click(q: Query<&Interaction, (With<PresetDelBtn>, Changed<Interaction>)>, mut state: Option<ResMut<ExportOverlayState>>) {
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

pub(super) fn close_click(q: Query<&Interaction, (With<CloseBtn>, Changed<Interaction>)>, mut state: Option<ResMut<ExportOverlayState>>) {
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
pub(super) fn cancel_or_back_click(
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
pub(super) fn copy_log_click(
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
pub(super) fn section_toggle_click(
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

pub(super) fn icon_clear_click(q: Query<&Interaction, (With<IconClearBtn>, Changed<Interaction>)>, mut state: Option<ResMut<ExportOverlayState>>) {
    let Some(state) = state.as_mut() else { return };
    if q.iter().any(|i| *i == Interaction::Pressed) {
        state.icon_path = None;
    }
}

/// Marks the save-before-export prompt, so its buttons can dismiss it.
#[derive(Component)]
struct SavePromptRoot;

/// "Save and export" — save the scene, then run the build.
#[derive(Component)]
pub(super) struct SavePromptSaveBtn;

/// "Export without saving" — run the build against what is on disk.
#[derive(Component)]
pub(super) struct SavePromptSkipBtn;

/// Ask before every export, and deliberately *not* only when the scene is dirty.
///
/// The editor tracks modified-ness per scene tab, so it can tell. The prompt
/// still always appears, because the question an export raises is not "have you
/// typed since the last save" — it is "is what is on disk the thing you want
/// built", and a dialog that only sometimes appears trains you to click through
/// it without reading on the occasions it does.
fn spawn_save_prompt(world: &mut World) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    let mut queue = CommandQueue::default();
    {
        let mut c = Commands::new(&mut queue, world);
        let (overlay, content) =
            renzora_ember::widgets::overlay_sized(&mut c, &fonts, "Export", 420.0, 190.0, true);
        // Above the export dialog (9300) that opened it.
        c.entity(overlay).insert((GlobalZIndex(9800), SavePromptRoot));

        let msg = txt(
            &mut c,
            &fonts,
            "Save the project before exporting? The build uses what is on disk.",
            12.0,
            text_primary(),
        );
        c.entity(msg).insert(Node { margin: UiRect::bottom(Val::Px(14.0)), ..default() });

        let row = c
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::FlexEnd,
                column_gap: Val::Px(8.0),
                margin: UiRect::top(Val::Px(8.0)),
                ..default()
            })
            .id();
        let skip = renzora_ember::widgets::button(&mut c, &fonts.ui, "Export without saving");
        c.entity(skip).insert(SavePromptSkipBtn);
        let save = renzora_ember::widgets::button(&mut c, &fonts.ui, "Save and export");
        c.entity(save).insert(SavePromptSaveBtn);
        c.entity(row).add_children(&[skip, save]);
        c.entity(content).add_children(&[msg, row]);
    }
    queue.apply(world);
}

/// Run the build that the prompt was standing in front of.
fn start_export(w: &mut World) {
    if let Some(mut state) = w.get_resource_mut::<ExportOverlayState>() {
        state.sync_active_preset();
        state.save_presets();
    }
    let name = w
        .get_resource::<renzora::core::CurrentProject>()
        .map(|p| p.config.name.clone())
        .unwrap_or_default();
    run_export(w, &name);
}

/// Dismiss the prompt, then either save first or go straight to the build.
pub(super) fn save_prompt_click(
    save: Query<&Interaction, (With<SavePromptSaveBtn>, Changed<Interaction>)>,
    skip: Query<&Interaction, (With<SavePromptSkipBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    let pressed = |q: &Query<&Interaction, _>| q.iter().any(|i| *i == Interaction::Pressed);
    let want_save = save.iter().any(|i| *i == Interaction::Pressed);
    let want_skip = pressed(&skip);
    if !want_save && !want_skip {
        return;
    }
    commands.queue(move |w: &mut World| {
        let mut roots = w.query_filtered::<Entity, With<SavePromptRoot>>();
        let ids: Vec<Entity> = roots.iter(w).collect();
        for e in ids {
            if w.get_entity(e).is_ok() {
                w.entity_mut(e).despawn();
            }
        }
        if want_save {
            // The same request the Save command uses, so this goes through the
            // one save path rather than a second one that could drift from it.
            w.insert_resource(renzora::core::SaveSceneRequested);
        }
        start_export(w);
    });
}

pub(super) fn export_click(q: Query<&Interaction, (With<ExportBtn>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|w: &mut World| {
            if can_export(&Rx::new(&*w)) {
                spawn_save_prompt(w);
            }
        });
    }
}

pub(super) fn output_browse_click(q: Query<&Interaction, (With<OutputBrowseBtn>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|w: &mut World| {
            if let Some(dir) = rfd::FileDialog::new().set_title(renzora::lang::t("export.dialog.select_output")).pick_folder() {
                w.resource_mut::<ExportOverlayState>().output_dir = dir.to_string_lossy().to_string();
            }
        });
    }
}

pub(super) fn icon_browse_click(q: Query<&Interaction, (With<IconBrowseBtn>, Changed<Interaction>)>, mut commands: Commands) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|w: &mut World| {
            // The list is `crate::icon::PICKABLE` rather than a literal so the
            // dialog can never offer a format the conversion then refuses. It
            // used to include `svg`, which `image` cannot decode at any feature
            // level — picking one produced a working-looking export with no icon
            // anywhere.
            if let Some(f) = rfd::FileDialog::new().set_title(renzora::lang::t("export.dialog.select_icon")).add_filter(renzora::lang::t("export.filter.images"), crate::icon::PICKABLE).pick_file() {
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
pub(super) fn source_download_click(
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

pub(super) fn download_click(q: Query<&Interaction, (With<DownloadBtn>, Changed<Interaction>)>, mut commands: Commands) {
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

pub(super) fn install_click(q: Query<&Interaction, (With<InstallBtn>, Changed<Interaction>)>, mut commands: Commands) {
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
