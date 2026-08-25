//! WGSL hot-reload: pick up `.wgsl` edits made outside the editor.
//!
//! This serves **hand-written** shaders — a `.wgsl` you authored and pointed a
//! mesh at. Edit it in any text editor and the viewport picks it up.
//!
//! It deliberately does *not* serve materials. A `.material` carries its
//! compiled shader inside it (`MaterialGraph::compiled`) and writes no `.wgsl`
//! to edit, because a graph and a hand-edited copy of its output drifting
//! apart is a corrupt material with no way to tell which half is right. The
//! graph editor's own Apply re-resolves affected entities synchronously
//! (`renzora_material_editor::apply_material`), so materials need nothing from
//! this module.
//!
//! Lives in the editor crate rather than behind a feature on `renzora_shader`.
//! A feature would be unified across a `cargo build --workspace` (the editor
//! build lane), so the runtime binary staged beside the editor would compile
//! this module as dead code and carry its dependencies for nothing.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver};
use notify_debouncer_full::{
    new_debouncer,
    notify::{RecommendedWatcher, RecursiveMode},
    DebounceEventResult, Debouncer, RecommendedCache,
};

use renzora::core::CurrentProject;
use renzora_shader::material::precompiled::project_relative;
use renzora_shader::material::resolver::{MaterialCache, MaterialResolved};

/// Debounce window for file events. Long enough that an editor's save —
/// which often lands as a truncate followed by a write — arrives as one event
/// rather than two.
const DEBOUNCE: Duration = Duration::from_millis(200);

/// Lifetime handle for the watcher. Dropping it stops the watcher thread and
/// closes the channel.
///
/// The resource is inserted **even when the watch could not be started**, with
/// `rx` set to `None`. That is deliberate: the install used to bail before
/// inserting anything, so `ensure_watcher_for_current_project` saw no resource,
/// tried again the next frame, and did so forever — re-stat-ing the directory
/// every frame, and on a failing `watch()` logging a warning every frame too.
/// Recording the attempt is what makes it an attempt rather than a loop.
#[derive(Resource)]
pub struct WgslHotReload {
    pub project_root: PathBuf,
    _debouncer: Option<Debouncer<RecommendedWatcher, RecommendedCache>>,
    rx: Option<Receiver<DebounceEventResult>>,
}

impl WgslHotReload {
    /// Start a watcher over `project_root`, replacing any previous one.
    ///
    /// Watches the project root **recursively**, not `<root>/materials`. A
    /// hand-written shader can live anywhere the author puts it, and a
    /// non-recursive watch would miss even a subfolder of `materials/`.
    fn install(project_root: PathBuf) -> Self {
        let (tx, rx) = unbounded::<DebounceEventResult>();
        let handler = move |result: DebounceEventResult| {
            let _ = tx.send(result);
        };
        let debouncer = match new_debouncer(DEBOUNCE, None, handler) {
            Ok(mut d) => match d.watch(&project_root, RecursiveMode::Recursive) {
                Ok(()) => {
                    info!("WGSL hot-reload: watching {project_root:?}");
                    Some(d)
                }
                Err(e) => {
                    warn!("WGSL hot-reload: could not watch {project_root:?}: {e}");
                    None
                }
            },
            Err(e) => {
                warn!("WGSL hot-reload: could not create debouncer: {e}");
                None
            }
        };
        Self {
            project_root,
            rx: debouncer.is_some().then_some(rx),
            _debouncer: debouncer,
        }
    }
}

/// Ensure the watcher matches the open project, reinstalling on a project
/// switch.
///
/// Runs only when [`CurrentProject`] changes. It used to run every frame as an
/// exclusive system, which forced a schedule sync point on every frame of the
/// editor's life to answer a question that can only change when the user opens
/// a different project.
fn ensure_watcher_for_current_project(
    project: Res<CurrentProject>,
    existing: Option<Res<WgslHotReload>>,
    mut commands: Commands,
) {
    if existing.is_some_and(|w| w.project_root == project.path) {
        return;
    }
    commands.insert_resource(WgslHotReload::install(project.path.clone()));
}

/// Drain pending file events and invalidate the resolver cache for any changed
/// `.wgsl`.
fn drain_wgsl_events(
    reload: Option<Res<WgslHotReload>>,
    mut cache: ResMut<MaterialCache>,
    mut commands: Commands,
    resolved: Query<(Entity, &MaterialResolved)>,
) {
    let Some(reload) = reload else { return };
    let Some(rx) = reload.rx.as_ref() else { return };
    while let Ok(result) = rx.try_recv() {
        match result {
            Ok(events) => {
                for event in events {
                    for path in &event.paths {
                        invalidate_for_wgsl(
                            path,
                            &reload.project_root,
                            &mut cache,
                            &mut commands,
                            &resolved,
                        );
                    }
                }
            }
            Err(errs) => {
                for e in errs {
                    warn!("WGSL hot-reload error: {e:?}");
                }
            }
        }
    }
}

/// Invalidate whatever a changed `.wgsl` at `path` feeds.
///
/// One key, the `.wgsl`'s own — that is what the resolver caches a
/// hand-written shader under. This used to also invalidate a sibling
/// `<stem>.material`, back when saving a graph wrote its compiled shader out
/// beside it. Materials embed that shader now, so no `.wgsl` on disk belongs
/// to one and there is no second key to drop.
fn invalidate_for_wgsl(
    path: &Path,
    project_root: &Path,
    cache: &mut MaterialCache,
    commands: &mut Commands,
    resolved: &Query<(Entity, &MaterialResolved)>,
) {
    if path.extension().and_then(|e| e.to_str()) != Some("wgsl") {
        return;
    }
    let wgsl_key = project_relative(project_root, path);

    cache.invalidate(&wgsl_key);
    info!("WGSL hot-reload: {wgsl_key} changed, invalidating");

    // Re-evaluate entities using this key. Without dropping the marker the
    // resolver's `Without<MaterialResolved>` filter skips them, and they keep
    // holding a `MeshMaterial3d` whose material was just dropped from the cache.
    let mut re_eval = 0;
    for (entity, mat) in resolved.iter() {
        if mat.source_path == wgsl_key {
            commands.entity(entity).remove::<MaterialResolved>();
            re_eval += 1;
        }
    }
    if re_eval > 0 {
        info!("WGSL hot-reload: {re_eval} entity(ies) queued for re-resolve");
    }
}

/// Registers WGSL hot-reload. Editor-only by construction — nothing outside
/// `renzora_shader_editor` links this crate.
pub struct WgslHotReloadPlugin;

impl Plugin for WgslHotReloadPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                ensure_watcher_for_current_project
                    .run_if(resource_exists_and_changed::<CurrentProject>),
                drain_wgsl_events,
            )
                .chain(),
        );
    }
}
