//! Editor-only WGSL hot-reload.
//!
//! Watches the current project's `materials/` directory for `.wgsl` file
//! changes and invalidates the corresponding entry in `MaterialCache` so the
//! resolver re-compiles the material on the next frame. Without this, edits
//! to the WGSL file (Apply in the material graph editor, or any external
//! edit) are not picked up by the running editor — the resolver would
//! re-use the cached compiled material by path.
//!
//! The watcher's lifetime is tied to the [`WgslHotReload`] resource. When
//! the project root changes (the user opens a different project), the
//! previous watcher is dropped and a new one is started.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver};
use notify_debouncer_full::{
    new_debouncer,
    notify::{RecommendedWatcher, RecursiveMode},
    DebounceEventResult, Debouncer, RecommendedCache,
};

use super::resolver::{MaterialCache, MaterialResolved};
use renzora::core::CurrentProject;

/// Lifetime handle for the WGSL hot-reload watcher. Dropping it stops the
/// watcher thread and closes the channel. The system in [`drain_wgsl_events`]
/// reads from `rx` to invalidate cached materials.
#[derive(Resource)]
pub struct WgslHotReload {
    pub project_root: PathBuf,
    _debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
    pub rx: Receiver<DebounceEventResult>,
}

impl WgslHotReload {
    /// Start a watcher for `<project_root>/materials`. Stores the handle in
    /// the world as a `WgslHotReload` resource. If a previous
    /// `WgslHotReload` exists it is replaced and its watcher thread stops.
    ///
    /// Returns silently on error (logs a warning). The watcher's failure is
    /// not fatal — the editor still works, just without hot-reload.
    pub fn install(world: &mut World, project_root: PathBuf) {
        let materials_dir = project_root.join("materials");
        if !materials_dir.is_dir() {
            return;
        }

        let (tx, rx) = unbounded::<DebounceEventResult>();
        let event_handler = move |result: DebounceEventResult| {
            let _ = tx.send(result);
        };
        let mut debouncer = match new_debouncer(Duration::from_millis(200), None, event_handler) {
            Ok(d) => d,
            Err(e) => {
                warn!("WGSL hot-reload: could not create debouncer: {e}");
                return;
            }
        };
        if let Err(e) = debouncer.watch(&materials_dir, RecursiveMode::NonRecursive) {
            warn!("WGSL hot-reload: could not watch {materials_dir:?}: {e}");
            return;
        }

        info!("WGSL hot-reload: watching {materials_dir:?}");
        world.insert_resource(WgslHotReload {
            project_root,
            _debouncer: debouncer,
            rx,
        });
    }
}

/// System: drain pending file-change events from the watcher channel and
/// invalidate the resolver cache for any changed `.wgsl` file. The matching
/// `.material` is found by stem (`foo.wgsl` → `materials/foo.material`).
/// In addition to dropping the cache entry, this removes the `MaterialResolved`
/// marker from any entity using that material so the resolver re-evaluates
/// and re-binds a fresh `MeshMaterial3d` on the next frame.
pub fn drain_wgsl_events(
    reload: Option<Res<WgslHotReload>>,
    mut cache: ResMut<MaterialCache>,
    mut commands: Commands,
    resolved: Query<(Entity, &MaterialResolved)>,
) {
    let Some(reload) = reload else {
        return;
    };
    while let Ok(result) = reload.rx.try_recv() {
        match result {
            Ok(events) => {
                for event in events {
                    for path in &event.paths {
                        invalidate_for_wgsl(path, &mut cache, &mut commands, &resolved);
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

fn invalidate_for_wgsl(
    path: &Path,
    cache: &mut MaterialCache,
    commands: &mut Commands,
    resolved: &Query<(Entity, &MaterialResolved)>,
) {
    if path.extension().and_then(|e| e.to_str()) != Some("wgsl") {
        return;
    }
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return;
    };
    let material_path = format!("materials/{stem}.material");
    info!("WGSL hot-reload: {path:?} changed, invalidating {material_path}");
    cache.invalidate(&material_path);

    // Re-evaluate entities that use this material. Without removing the
    // marker, the resolver's `Without<MaterialResolved>` filter would skip
    // them on the next pass and the entity would keep holding the stale
    // `MeshMaterial3d` handle whose material was just dropped from the
    // cache.
    let mut re_eval = 0;
    for (entity, mat_resolved) in resolved.iter() {
        if mat_resolved.source_path == material_path {
            commands.entity(entity).remove::<MaterialResolved>();
            re_eval += 1;
        }
    }
    if re_eval > 0 {
        info!("WGSL hot-reload: {re_eval} entity(ies) queued for re-resolve");
    }
}

/// Exclusive system: ensures a [`WgslHotReload`] exists for the current
/// `CurrentProject`. If the project root changed since the last install,
/// replaces the watcher. Registered as an exclusive system because
/// installing the watcher requires `&mut World` to construct and insert
/// the `WgslHotReload` resource (which holds a non-Clone notify debouncer).
pub fn ensure_watcher_for_current_project(world: &mut World) {
    let Some(project) = world.get_resource::<CurrentProject>().cloned() else {
        return;
    };
    let path = project.path;
    let needs_install = match world.get_resource::<WgslHotReload>() {
        Some(reload) => reload.project_root != path,
        None => true,
    };
    if needs_install {
        WgslHotReload::install(world, path);
    }
}

/// Plugin: registers the [`drain_wgsl_events`] system (regular) and the
/// exclusive [`ensure_watcher_for_current_project`] system. They are
/// registered separately because the latter requires `&mut World` and
/// cannot be combined in a system tuple with the former.
pub struct WgslHotReloadPlugin;

impl Plugin for WgslHotReloadPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, drain_wgsl_events);
        app.add_systems(Update, ensure_watcher_for_current_project);
    }
}
