//! Opening and closing the window, what it reads once at spawn, and the two
//! systems that decide when to convert.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use renzora_ember::font::EmberFonts;
use renzora_ember::reactive::Rx;

use renzora_import::settings::{SceneStructure, UpAxis};

use crate::overlay::{poll_import_task, run_import, ImportLayout, ImportOverlayState, ImportProgress};

use super::panes::scan_dest_dirs;
use super::{GridSuppressed, ImportNav, ImportRoot, ImportTab, TreeItem};

pub(super) fn manage_import_modal(world: &mut World) {
    let visible = world.get_resource::<ImportOverlayState>().is_some_and(|s| s.visible);
    if visible {
        poll_import_task(world); // keep progress flowing (egui draw is gated off)
    }

    let mut q = world.query_filtered::<Entity, With<ImportRoot>>();
    let existing: Vec<Entity> = q.iter(world).collect();

    if visible && existing.is_empty() {
        let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
        let has_project = world.get_resource::<renzora::core::CurrentProject>().is_some();
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
        if let Some(mut vp) = world.get_resource_mut::<renzora::core::viewport_types::ViewportSettings>() {
            let was = vp.show_grid;
            vp.show_grid = false;
            world.insert_resource(GridSuppressed(was));
        }
        let init = Init::read(&Rx::new(&*world));
        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            super::frame::spawn_modal(&mut commands, &fonts, &init, has_project);
        }
        queue.apply(world);
    } else if !visible && !existing.is_empty() {
        for e in existing {
            world.entity_mut(e).despawn();
        }
        if let Some(prev) = world.remove_resource::<GridSuppressed>() {
            if let Some(mut vp) = world.get_resource_mut::<renzora::core::viewport_types::ViewportSettings>() {
                vp.show_grid = prev.0;
            }
        }
    }
}

/// Initial widget values read once at spawn (the bindings keep them in sync after).
pub(super) struct Init {
    pub(super) scale: f32,
    pub(super) up_axis: usize,
    pub(super) layout: usize,
    pub(super) structure: usize,
    /// Project directory tree for the destination picker: (rel_path, depth, name),
    /// `rel_path` forward-slashed and relative to the project root (`""` = root).
    pub(super) dest_folders: Vec<(String, usize, String)>,
    /// Sibling texture sets offered for a geometry-only queue: (stem, roles).
    /// Empty when the queue has no such model, which hides the row entirely.
    pub(super) texture_sets: Vec<(String, String)>,
    /// Index of the currently chosen set, offset by one for the "None" entry.
    pub(super) texture_set: usize,
}
impl Init {
    fn read(world: &Rx) -> Self {
        let s = world.resource::<ImportOverlayState>();
        let dest_folders = world
            .get_resource::<renzora::core::CurrentProject>()
            .map(|p| scan_dest_dirs(&p.path))
            .unwrap_or_default();
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
            dest_folders,
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

/// When a file stages, open it: switch to the Scene tab, expand the roots so
/// the tree is not a single collapsed line, and point the 3D preview at the
/// staged GLB. When the verdict clears it, tear the preview down so its camera
/// stops rendering.
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
            nav.tab = ImportTab::Scene;
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

/// How long the settings have to stop changing before the window reconverts.
/// Long enough to drag a scale field across its range as one edit rather than
/// forty.
const SETTINGS_SETTLE_SECS: f64 = 0.9;

/// Reconvert when the import settings change under a staged model.
///
/// Without this the settings rail would be dead controls after the first
/// conversion: the model on screen was built with the old values, and the only
/// thing that could rebuild it was the Reimport button this replaces. Making it
/// automatic is what lets the window be "it converts, you adjust, you add".
///
/// The destination counts as a setting here — the worker bakes the final paths
/// into each staged import and into the `.material` writes it is holding, so
/// pointing the window at another folder has to rebuild them too.
pub(super) fn settings_watch(
    world: &mut World,
    mut seen: Local<Option<crate::overlay::ConvertedWith>>,
    mut due: Local<Option<f64>>,
) {
    let Some(state) = world.get_resource::<ImportOverlayState>() else {
        return;
    };
    if !state.visible {
        *seen = None;
        *due = None;
        return;
    }
    let now = crate::overlay::ConvertedWith {
        settings: state.settings.clone(),
        target_directory: state.target_directory.clone(),
        layout: state.layout,
    };
    let changed_this_frame = seen.as_ref().is_some_and(|prev| *prev != now);
    let differs = state.converted_with.as_ref().is_some_and(|c| *c != now);
    let idle = state.active_task.is_none() && !state.reimport_requested;
    let staged = !state.staged.is_empty();
    *seen = Some(now);

    let elapsed = world
        .get_resource::<Time>()
        .map(|t| t.elapsed_secs_f64())
        .unwrap_or(0.0);
    if !differs {
        // Back to what is already on disk — including a value edited away and
        // then edited back, which needs no work at all.
        *due = None;
        return;
    }
    // Push the deadline out on every keystroke or drag tick, so a value being
    // scrubbed reconverts once, when it settles.
    if changed_this_frame || due.is_none() {
        *due = Some(elapsed + SETTINGS_SETTLE_SECS);
    }
    let Some(at) = *due else { return };
    // A change made *during* a conversion stays armed rather than being
    // dropped: the run in flight is building the model with the old value, so
    // the reconvert is still owed once it finishes.
    if elapsed < at || !idle || !staged {
        return;
    }
    *due = None;
    crate::overlay::request_reimport(world);
    if let Some(mut nav) = world.get_resource_mut::<ImportNav>() {
        nav.reset_selection();
    }
}

/// Header label reflecting the queue: uniform-kind queues get a specific title,
/// empty / mixed queues get the generic "Import Assets".
pub(super) fn import_title(w: &Rx) -> String {
    use crate::kinds::{detect_kind, AssetKind};
    let Some(state) = w.get_resource::<ImportOverlayState>() else {
        return "Import Assets".to_string();
    };
    if state.pending_files.is_empty() {
        return "Import Assets".to_string();
    }
    let kinds: Vec<AssetKind> = state
        .pending_files
        .iter()
        .filter_map(|q| detect_kind(&q.path))
        .collect();
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
        Some(AssetKind::GaussianSplat) => "Import Gaussian Splats",
        None => "Import Assets",
    }
    .to_string()
}
