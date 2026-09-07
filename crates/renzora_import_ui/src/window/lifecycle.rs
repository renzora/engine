//! Opening and closing the window, what it reads once at spawn, and the two
//! systems that decide when to convert.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use renzora_ember::font::EmberFonts;
use renzora_ember::reactive::Rx;

use renzora_import::settings::{SceneStructure, UpAxis};

use crate::overlay::{poll_import_task, run_import, ImportLayout, ImportOverlayState, ImportProgress};

use super::{ImportNav, ImportRoot, ImportTab, TreeItem};

pub(super) fn manage_import_modal(world: &mut World) {
    let visible = world.get_resource::<ImportOverlayState>().is_some_and(|s| s.visible);
    if visible {
        poll_import_task(world); // keep progress flowing (egui draw is gated off)
    }

    let mut q = world.query_filtered::<Entity, With<ImportRoot>>();
    let existing: Vec<Entity> = q.iter(world).collect();

    if visible && existing.is_empty() {
        let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
        // Always open on Files — the first thing the user does is add files,
        // and a stale tab from a previous open would be confusing.
        {
            let mut nav = world.resource_mut::<ImportNav>();
            nav.tab = ImportTab::Files;
            nav.reset_selection();
        }
        // Last-ditch repair for a scale that can never be right. Both routes
        // that could write one are closed now — the unit probe rejects
        // non-positive values, and `enqueue` re-detects per queue instead of
        // inheriting the last one — but `ImportOverlayState` outlives the
        // window, and a scale of zero silently collapses every model to a
        // point, so it is worth refusing to open with one.
        {
            let mut s = world.resource_mut::<ImportOverlayState>();
            if !s.settings.scale.is_finite() || s.settings.scale <= 0.0 {
                warn!(
                    "[import] scale was {}; resetting to 1.0",
                    s.settings.scale
                );
                s.settings.scale = 1.0;
            }
        }
        let init = Init::read(&Rx::new(&*world));
        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            super::frame::spawn_modal(&mut commands, &fonts, &init);
        }
        queue.apply(world);
    } else if !visible && !existing.is_empty() {
        for e in existing {
            world.entity_mut(e).despawn();
        }
    }
}

/// Initial widget values read once at spawn (the bindings keep them in sync after).
pub(super) struct Init {
    pub(super) scale: f32,
    pub(super) up_axis: usize,
    pub(super) layout: usize,
    pub(super) structure: usize,
    /// The open project's root, which the destination picker walks. `None`
    /// with no project open, and the picker is then not built at all.
    pub(super) project_root: Option<std::path::PathBuf>,
    /// Where the import currently targets, project-relative and
    /// forward-slashed (`""` = project root), so the picker opens on it.
    pub(super) target_dir: String,
    /// Sibling texture sets offered for a geometry-only queue: (stem, roles).
    /// Empty when the queue has no such model, which hides the row entirely.
    pub(super) texture_sets: Vec<(String, String)>,
    /// Index of the currently chosen set, offset by one for the "None" entry.
    pub(super) texture_set: usize,
}
impl Init {
    fn read(world: &Rx) -> Self {
        let s = world.resource::<ImportOverlayState>();
        let project_root = world
            .get_resource::<renzora::core::CurrentProject>()
            .map(|p| p.path.clone());
        let texture_sets = queue_texture_sets(s);
        let texture_set = s
            .settings
            .texture_set
            .as_deref()
            .and_then(|want| texture_sets.iter().position(|(stem, _)| stem == want))
            .map_or(0, |i| i + 1);
        Self {
            texture_sets,
            texture_set,
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
            structure: match s.settings.structure {
                SceneStructure::Preserve => 0,
                SceneStructure::FlatPerMesh => 1,
                SceneStructure::Combined => 2,
            },
            project_root,
            target_dir: s.target_directory.clone(),
        }
    }
}

/// The sibling texture sets on offer for the queued files.
///
/// Read once when the window opens rather than per staged file: a queue is
/// almost always one folder, so every model in it sees the same sets, and a
/// dropdown that reshuffled as you clicked between files would be worse than
/// one that doesn't. Returns empty unless the queue holds a geometry-only
/// model — a format that names its own textures must not be overridden by a
/// folder full of guesses.
fn queue_texture_sets(s: &ImportOverlayState) -> Vec<(String, String)> {
    use renzora_import::sibling_textures;
    s.pending_files
        .iter()
        .map(|q| q.path.as_path())
        .chain(s.last_files.iter().map(|q| q.path.as_path()))
        .find(|p| sibling_textures::is_geometry_only(p))
        .map(|p| {
            sibling_textures::discover(p)
                .into_iter()
                .map(|set| (set.stem.clone(), set.role_summary()))
                .collect()
        })
        .unwrap_or_default()
}

/// When a file stages, open it: expand the tree's roots so it is not a single
/// collapsed line, and point the 3D preview at the staged GLB. When the verdict
/// clears it, tear the preview down so its camera stops rendering.
///
/// The tab is left where the user put it. This used to switch to Scene on every
/// staged file, which in a batch meant being thrown out of the Files list once
/// per file as they finished converting.
pub(super) fn on_staged_changed(world: &mut World) {
    let path = world
        .get_resource::<ImportOverlayState>()
        .and_then(|s| s.current().map(|st| st.glb_path.clone()));
    let Some(path) = path else {
        crate::preview3d::clear(world);
        return;
    };

    let already = world
        .get_resource::<crate::preview3d::ImportPreview>()
        .and_then(|p| p.path.clone())
        .as_deref()
        == Some(path.as_path());
    if !already {
        let roots = world
            .get_resource::<ImportOverlayState>()
            .and_then(|s| s.current())
            .and_then(|s| s.stats.as_ref())
            .map(|st| st.roots.clone())
            .unwrap_or_default();
        if let Some(mut nav) = world.get_resource_mut::<ImportNav>() {
            nav.reset_selection();
            nav.expanded.extend(roots.into_iter().map(TreeItem::Node));
        }
        // The window has to be up for the user to answer; an inspecting import
        // must never hand off to the corner toast.
        let mut s = world.resource_mut::<ImportOverlayState>();
        s.visible = true;
        s.toast_active = false;
    }
    crate::preview3d::show(world, &path);
}

/// Convert whatever is queued, as soon as it is queued.
///
/// There used to be an Import button whose only job was to start the conversion
/// the user had already asked for by choosing the files, and it was misnamed
/// besides: nothing it did touched the project. Every model converts into the
/// project's cache and waits there, so starting early costs nothing and buys the
/// user a preview by the time they have finished looking at the queue. The
/// decision that matters is Add to project, at the other end.
///
/// Files added to an open window join the ones already staged rather than
/// replacing them, which is what makes dropping a second batch mid-inspection
/// work.
pub(super) fn auto_start_import(world: &mut World) {
    let ready = {
        let Some(s) = world.get_resource::<ImportOverlayState>() else {
            return;
        };
        s.visible
            && !s.pending_files.is_empty()
            && s.active_task.is_none()
            // A queued reconvert owns the next run; starting one here would
            // race it into the same staging directories.
            && !s.reimport_requested
            // `Error` holds until something new is queued — `enqueue` clears it
            // — so a file that cannot convert doesn't retry forever.
            && matches!(s.progress, ImportProgress::Idle | ImportProgress::Done(_))
    };
    // The worker writes into the project's cache directory, so there has to be
    // a project.
    if !ready || world.get_resource::<renzora::core::CurrentProject>().is_none() {
        return;
    }
    run_import(world);
}

/// Keep the settings rail honest about whether the model on screen was built
/// with the values it is showing.
///
/// This used to *do* the reconversion, on a settle timer. That read well for
/// the scale field and badly for everything else: ticking "Overdraw" in the
/// Optimize group threw away a converted 900k-triangle scene and rebuilt it for
/// half a minute, over an option that changes what is written rather than what
/// is on screen. Flipping four of them in a row meant four rebuilds, and each
/// one reset the preview camera.
///
/// So the reconvert is a button now (see `panes::build_reconvert_row`), and
/// what is left here is the one thing that still has to happen automatically:
/// once nothing is staged, the settings *are* what the next conversion will
/// use, so the "stale" state has to clear rather than sit there claiming a
/// rebuild is owed for a model that no longer exists.
///
/// The destination counts as a setting for this purpose — the worker bakes the
/// final paths into each staged import and into the `.material` writes it is
/// holding, so pointing the window at another folder genuinely does need a
/// rebuild before Import means what it says.
pub(super) fn settings_watch(world: &mut World) {
    let Some(state) = world.get_resource::<ImportOverlayState>() else {
        return;
    };
    if state.staged.is_empty() && state.active_task.is_none() && state.converted_with.is_some() {
        world.resource_mut::<ImportOverlayState>().converted_with = None;
    }
}

/// Close the window once the last staged file has been dealt with and there is
/// nothing left on its way in.
///
/// Pressing Import used to take the files and leave the window sitting there
/// empty, which reads as a dialog that has failed to notice it is finished —
/// and the corner toast, the asset browser scrolling to the new files and the
/// thumbnails appearing all say more about the result than an empty modal does.
/// A window with files still queued or converting stays up, because that one
/// really does have something left to show.
pub(super) fn close_when_finished(world: &mut World) {
    let summary = {
        let Some(s) = world.get_resource::<ImportOverlayState>() else {
            return;
        };
        let quiet = s.visible
            && s.staged.is_empty()
            && s.pending_files.is_empty()
            && s.active_task.is_none()
            && !s.reimport_requested;
        // Something has to have happened: a window the user has only just
        // opened is in exactly this state and must stay open. A run that
        // reported a failure keeps it open too, since the Results list in the
        // rail is the only place that failure is written down.
        let succeeded = !s.log_entries.is_empty() && s.log_entries.iter().all(|e| e.success);
        if !(quiet && succeeded) {
            return;
        }
        match s.log_entries.len() {
            1 => "Imported 1 file".to_string(),
            n => format!("Imported {n} files"),
        }
    };
    // Hand the result to the corner toast on the way out, so closing the window
    // is not the same as the import going unremarked.
    crate::overlay::close_overlay(world);
    let mut s = world.resource_mut::<ImportOverlayState>();
    s.progress = ImportProgress::Done(summary);
    s.toast_active = true;
}
