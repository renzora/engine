//! WGSL hot-reload: pick up `.wgsl` edits made outside the editor.
//!
//! The material graph editor's own Apply already invalidates the resolver cache
//! and re-resolves the affected entities synchronously
//! (`renzora_material_editor::apply_material`), so this is not what makes Apply
//! work. What it adds is the other direction: editing a compiled `.wgsl` in a
//! text editor and having the viewport pick it up.
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

/// Debounce window for file events. Long enough that a compile writing `.wgsl`
/// and `.wgsl.meta` back to back arrives as one event rather than two.
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
    /// Watches the project root **recursively**, not `<root>/materials`.
    /// `precompiled::save_compiled` writes each `.wgsl` next to its `.material`,
    /// so a model-imported material's compiled shader lands in
    /// `models/<name>/materials/` — outside a `<root>/materials` watch, and
    /// outside a non-recursive one even for a subfolder of `materials/` itself.
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
/// Both keys are dropped, because either can be the one the resolver cached:
/// a `MaterialRef` normally names the `.material`, but it can name a `.wgsl`
/// directly — the resolver assembles a `.wgsl` + `.wgsl.meta` pair into a
/// `GraphMaterial` in its own right.
///
/// The `.material` path is derived from where the `.wgsl` actually sits rather
/// than assumed to be `materials/<stem>.material`. Compiled output lives beside
/// its source, so that assumption pointed at a non-existent cache key for every
/// material that isn't in the project's top-level `materials/` folder.
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
    let material_key = project_relative(project_root, &path.with_extension("material"));

    cache.invalidate(&wgsl_key);
    cache.invalidate(&material_key);
    info!("WGSL hot-reload: {wgsl_key} changed, invalidating {material_key}");

    // Re-evaluate entities using either key. Without dropping the marker the
    // resolver's `Without<MaterialResolved>` filter skips them, and they keep
    // holding a `MeshMaterial3d` whose material was just dropped from the cache.
    let mut re_eval = 0;
    for (entity, mat) in resolved.iter() {
        if mat.source_path == wgsl_key || mat.source_path == material_key {
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
