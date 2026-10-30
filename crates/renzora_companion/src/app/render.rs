//! Rendering jobs: work that needs the GPU, done with the engine's own code.
//!
//! Model thumbnails render the model: its meshes and textures uploaded, a
//! camera, a dozen frames, a readback. Material thumbnails compile the
//! material's shader and render a sphere with it. In the editor both compete
//! with the scene being edited. Here the companion runs the editor's capture
//! code unchanged (`renzora_asset_browser::model_thumbnails`,
//! `renzora_material_editor::file_thumbnails`), with the loaders and material
//! plugins they need, so a thumbnail looks the same wherever it was made and
//! lands in the same cache file.
//!
//! Projects: the asset reader resolves project-relative paths against
//! `ProjectAssetPath`, so each job points it at its project before asking,
//! and the capture code finds its `CurrentProject` as it does in the editor.
//! One project at a time: a job for another project waits until the running
//! ones finish.
//!
//! Frames: the companion's window updates a few times a second when idle,
//! which would stretch a capture's dozen frames to seconds. While a rendering
//! job runs it updates continuously, and goes back to idling after.
//!
//! GPU memory: a capture's model is freed when its capture ends (the capture
//! code drops its handles), and these jobs wait while a game plays or a
//! profile records (see `jobs::held`), so the companion never holds a model
//! in GPU memory while either is measured.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::winit::{UpdateMode, WinitSettings};
use renzora::job_args::{MaterialThumbnail, ModelThumbnail};
use renzora_editor_framework::{MaterialThumbnailRegistry, ModelThumbnailRegistry};

use super::jobs::{JobCtx, JobQueue};
use crate::protocol::Job;

/// Longest a capture may take before it is given up: a model that never
/// loads must not hold the queue.
const CAPTURE_TIMEOUT_SECS: f32 = 60.0;

/// What a capture renders. Each kind has its own capture code and registry.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Model,
    Material,
}

/// One capture to start: its kind, project and file.
struct Capture {
    kind: Kind,
    project: PathBuf,
    path: PathBuf,
}

/// The thumbnails being rendered, and the project they are in.
#[derive(Resource, Default)]
struct Rendering {
    project: Option<PathBuf>,
    jobs: Vec<Active>,
    /// Taken from the queue but for another project: start when the
    /// running ones finish.
    waiting: Vec<(Capture, JobCtx)>,
}

struct Active {
    kind: Kind,
    path: PathBuf,
    ctx: JobCtx,
    started: f32,
}

pub(crate) fn build(app: &mut App) {
    // Bevy's glTF plugin is in already (`add_companion_rendering`); the
    // `.rvg` model loader is built from what it leaves behind.
    app.add_plugins(renzora_rvg::model::RvgModelPlugin)
        .init_asset_loader::<renzora_rmip::RmipAssetLoader>()
        .init_resource::<ModelThumbnailRegistry>()
        .add_plugins(renzora_asset_browser::model_thumbnails::ModelThumbnailPlugin)
        // The graph material, its sprite twin and the resolver's caches: what
        // a material capture compiles and renders with.
        .add_plugins(renzora_shader::material::MaterialPlugin)
        .add_plugins(renzora_material_editor::file_thumbnails::MaterialFileThumbnailPlugin)
        .init_resource::<Rendering>()
        .add_systems(Update, (take_jobs, finish_jobs, pace_frames).chain());
}

/// Read a rendering job's arguments.
fn parse(kind: &str, payload: &str) -> Result<Capture, String> {
    let bad = |e: serde_json::Error| format!("could not read the job's arguments: {e}");
    match kind {
        "model_thumbnail" => serde_json::from_str::<ModelThumbnail>(payload)
            .map(|a| Capture { kind: Kind::Model, project: a.project, path: a.model })
            .map_err(bad),
        "material_thumbnail" => serde_json::from_str::<MaterialThumbnail>(payload)
            .map(|a| Capture { kind: Kind::Material, project: a.project, path: a.material })
            .map_err(bad),
        _ => Err(format!("the companion cannot render a {kind} job")),
    }
}

/// Take rendering jobs the queue started and begin each capture.
fn take_jobs(world: &mut World) {
    let taken: Vec<(Job, JobCtx)> = std::mem::take(&mut world.resource_mut::<JobQueue>().main_thread);
    let mut rendering = world.remove_resource::<Rendering>().unwrap_or_default();
    for (job, ctx) in taken {
        let Job::Run { kind, payload, .. } = job else {
            ctx.finish(Err("not a rendering job".into()));
            continue;
        };
        match parse(&kind, &payload) {
            Ok(capture) => rendering.waiting.push((capture, ctx)),
            Err(why) => ctx.finish(Err(why)),
        }
    }

    // Start waiting jobs for the current project; switch project only when
    // nothing is running in the old one.
    let now = world.resource::<Time<Real>>().elapsed_secs();
    let mut i = 0;
    while i < rendering.waiting.len() {
        let project = rendering.waiting[i].0.project.clone();
        let same = rendering.project.as_ref() == Some(&project);
        if !same && !rendering.jobs.is_empty() {
            i += 1;
            continue;
        }
        if !same {
            if let Err(why) = open_project(world, &project) {
                let (_, ctx) = rendering.waiting.remove(i);
                ctx.finish(Err(why));
                continue;
            }
            rendering.project = Some(project);
        }
        let (capture, ctx) = rendering.waiting.remove(i);
        // Already rendering (the editor asked for one the companion was
        // rendering ahead of time): answered by that capture.
        let running = rendering.jobs.iter().any(|a| a.kind == capture.kind && a.path == capture.path);
        // A file rendered before has a handle here; forget it so the capture
        // code renders the changed file rather than answering from memory.
        match capture.kind {
            _ if running => {}
            Kind::Model => {
                let mut registry = world.resource_mut::<ModelThumbnailRegistry>();
                registry.invalidate(&capture.path);
                registry.request(capture.path.clone());
            }
            Kind::Material => {
                let mut registry = world.resource_mut::<MaterialThumbnailRegistry>();
                registry.invalidate(&capture.path);
                registry.request(capture.path.clone());
            }
        }
        rendering.jobs.push(Active { kind: capture.kind, path: capture.path, ctx, started: now });
    }
    world.insert_resource(rendering);
}

/// Point the asset reader and the capture code at `project`.
fn open_project(world: &mut World, project: &std::path::Path) -> Result<(), String> {
    let current = renzora::core::session::open_project(&project.join("project.toml"))
        .map_err(|e| format!("could not open the project at {}: {e}", project.display()))?;
    if let Some(asset_path) = world.get_resource::<renzora_engine::ProjectAssetPath>() {
        if let Ok(mut path) = asset_path.0.write() {
            *path = Some(current.path.clone());
        }
    }
    world.insert_resource(current);
    Ok(())
}

/// Answer each capture the capture code finished, gave up on, or that ran
/// out of time.
fn finish_jobs(
    mut rendering: ResMut<Rendering>,
    mut models: ResMut<ModelThumbnailRegistry>,
    mut materials: ResMut<MaterialThumbnailRegistry>,
    project: Option<Res<renzora::core::CurrentProject>>,
    time: Res<Time<Real>>,
) {
    let now = time.elapsed_secs();
    rendering.jobs.retain(|job| {
        let (done, in_flight) = match job.kind {
            Kind::Model => (models.handle(&job.path).is_some(), models.is_in_flight(&job.path)),
            Kind::Material => (materials.handle(&job.path).is_some(), materials.is_in_flight(&job.path)),
        };
        let mut cancel = || match job.kind {
            Kind::Model => models.cancel(&job.path),
            Kind::Material => materials.cancel(&job.path),
        };
        let result = if done {
            let png = project
                .as_ref()
                .map(|p| match job.kind {
                    Kind::Model => renzora_editor_framework::model_thumb_path(&job.path, p),
                    Kind::Material => renzora_editor_framework::material_thumb_path(&job.path, p),
                })
                .unwrap_or_default();
            Some(Ok(png.display().to_string()))
        } else if !in_flight {
            Some(Err(format!("{} could not be rendered (it did not load, or has no scene)", job.path.display())))
        } else if now - job.started > CAPTURE_TIMEOUT_SECS {
            cancel();
            Some(Err(format!("{} took over {CAPTURE_TIMEOUT_SECS:.0} s to render", job.path.display())))
        } else if job.ctx.cancelled() {
            cancel();
            Some(Err("cancelled".into()))
        } else {
            None
        };
        match result {
            Some(result) => {
                job.ctx.finish(result);
                false
            }
            None => true,
        }
    });
}

/// Run continuously while rendering, idle otherwise.
fn pace_frames(rendering: Res<Rendering>, mut winit: ResMut<WinitSettings>, mut busy: Local<bool>) {
    let now = !rendering.jobs.is_empty() || !rendering.waiting.is_empty();
    if now == *busy {
        return;
    }
    *busy = now;
    if now {
        winit.focused_mode = UpdateMode::Continuous;
        winit.unfocused_mode = UpdateMode::Continuous;
    } else {
        // The idle rates `add_companion_rendering` set.
        winit.focused_mode = UpdateMode::reactive(std::time::Duration::from_millis(100));
        winit.unfocused_mode = UpdateMode::reactive_low_power(std::time::Duration::from_millis(250));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_thumbnail_kinds_are_read_and_others_are_refused() {
        let model = serde_json::to_string(&ModelThumbnail { project: "p".into(), model: "p/a.glb".into() }).unwrap();
        let capture = parse("model_thumbnail", &model).unwrap();
        assert_eq!((capture.kind, capture.path), (Kind::Model, PathBuf::from("p/a.glb")));

        let material =
            serde_json::to_string(&MaterialThumbnail { project: "p".into(), material: "p/m.material".into() }).unwrap();
        let capture = parse("material_thumbnail", &material).unwrap();
        assert_eq!((capture.kind, capture.path), (Kind::Material, PathBuf::from("p/m.material")));

        assert!(parse("texture_thumbnail", "{}").is_err());
        assert!(parse("material_thumbnail", "not json").is_err());
    }
}
