//! Lit 3D thumbnails for the Shape Library.
//!
//! Each registered shape has a rendered picture of its actual mesh. The
//! phosphor icons the grid used before were shared between unrelated shapes
//! (Sphere and Hemisphere were the same globe; Cone, Wedge, Ramp and Funnel
//! were all a triangle), so the label was often the only way to tell two tiles
//! apart.
//!
//! The pictures are PNGs in the machine-wide cache (see `capture`), so after
//! the first session the editor only loads thirty small files. A shape with no
//! picture yet is rendered by the companion when there is one, and only
//! otherwise here; until it arrives its tile shows the shape's icon. While the
//! companion is still starting the editor waits for it rather than rendering
//! in the frames a project is loading, which is exactly when ten extra cameras
//! hurt most.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use bevy::prelude::*;

use renzora::core::ShapeRegistry;
use renzora_ember::theme::{card_bg, rgb};

use crate::capture::{load_png, shape_thumb_path, ShapeCaptureRequest, ShapeCaptures};

/// Longest the editor waits for a companion that is starting before it renders
/// the missing pictures itself.
const COMPANION_WAIT_SECS: f32 = 30.0;
/// Cached pictures decoded per frame. Each is a 192 px PNG, well under a
/// millisecond, but thirty at once on the first frame is a visible hitch.
const LOADS_PER_FRAME: usize = 6;

/// One image per shape id, filled in as pictures load.
#[derive(Resource, Default)]
pub(crate) struct ShapeThumbs {
    pub(crate) images: HashMap<&'static str, Handle<Image>>,
    /// What is being done about each shape's picture.
    state: HashMap<&'static str, Want>,
    /// Companion jobs out, by id.
    jobs: HashMap<renzora::CompanionJobId, &'static str>,
    /// Shapes the companion could not render, which this editor renders.
    local_only: HashSet<&'static str>,
    /// The tile colour the pictures were made for; a theme with another one
    /// starts over, since the colour is baked into each picture.
    clear: Option<Color>,
    /// Seconds spent waiting for a starting companion.
    waited: f32,
}

#[derive(Clone, Debug, PartialEq)]
enum Want {
    /// On disk: decode it.
    Load(PathBuf),
    /// Not on disk: have it rendered.
    Render(PathBuf),
    /// The companion or this editor is rendering it.
    Rendering,
    /// Loaded, or given up on (the icon stays).
    Settled,
}

pub(crate) fn register(app: &mut App) {
    app.init_resource::<ShapeThumbs>().add_systems(
        Update,
        (plan_thumbs, load_thumbs, render_thumbs, collect_thumbs)
            .chain()
            .run_if(in_state(renzora_editor_framework::SplashState::Editor)),
    );
}

/// Work out where every registered shape's picture lives and whether it is
/// there. Does work only when the registry has grown (a plugin registered a
/// shape after startup) or the theme's tile colour changed.
fn plan_thumbs(registry: Option<Res<ShapeRegistry>>, mut thumbs: ResMut<ShapeThumbs>, mut meshes: ResMut<Assets<Mesh>>) {
    let Some(registry) = registry else { return };
    let clear = rgb(card_bg());
    if thumbs.clear != Some(clear) {
        let first = thumbs.clear.is_none();
        thumbs.clear = Some(clear);
        if !first {
            thumbs.state.clear();
            thumbs.images.clear();
            thumbs.jobs.clear();
        }
    }
    if registry.iter().count() == thumbs.state.len() {
        return;
    }
    for shape in registry.iter() {
        if thumbs.state.contains_key(shape.id) {
            continue;
        }
        // Built only to name its file. The handle is dropped at the end of the
        // loop, which frees the mesh again.
        let handle = (shape.create_mesh)(&mut meshes);
        let path = meshes.get(&handle).and_then(|mesh| shape_thumb_path(shape.id, mesh, clear));
        let want = match path {
            Some(path) if path.is_file() => Want::Load(path),
            Some(path) => Want::Render(path),
            // No home directory to cache in: nothing to load or render into.
            None => Want::Settled,
        };
        thumbs.state.insert(shape.id, want);
    }
}

/// Decode a few cached pictures. A file that will not decode is deleted and
/// rendered again.
fn load_thumbs(mut thumbs: ResMut<ShapeThumbs>, mut images: ResMut<Assets<Image>>) {
    let thumbs = &mut *thumbs;
    let ready: Vec<(&'static str, PathBuf)> = thumbs
        .state
        .iter()
        .filter_map(|(id, w)| match w {
            Want::Load(path) => Some((*id, path.clone())),
            _ => None,
        })
        .take(LOADS_PER_FRAME)
        .collect();
    for (id, path) in ready {
        match load_png(&path) {
            Ok(image) => {
                thumbs.images.insert(id, images.add(image));
                thumbs.state.insert(id, Want::Settled);
            }
            Err(why) => {
                warn!("[shapes] the cached thumbnail {} did not load ({why}); rendering it again", path.display());
                let _ = std::fs::remove_file(&path);
                thumbs.state.insert(id, Want::Render(path));
            }
        }
    }
}

/// Have the missing pictures rendered: by the companion when it is connected,
/// by waiting when it is on its way, and here otherwise.
fn render_thumbs(
    mut thumbs: ResMut<ShapeThumbs>,
    jobs: Option<ResMut<renzora::CompanionJobs>>,
    mut captures: ResMut<ShapeCaptures>,
    time: Res<Time<Real>>,
) {
    let thumbs = &mut *thumbs;
    let missing: Vec<(&'static str, PathBuf)> = thumbs
        .state
        .iter()
        .filter_map(|(id, w)| match w {
            Want::Render(path) => Some((*id, path.clone())),
            _ => None,
        })
        .collect();
    if missing.is_empty() {
        return;
    }
    let Some(clear) = thumbs.clear else { return };
    let mut jobs = jobs;
    let starting = jobs.as_ref().is_some_and(|j| j.starting && !j.available);
    if starting && thumbs.waited < COMPANION_WAIT_SECS {
        thumbs.waited += time.delta_secs();
        return;
    }
    for (id, path) in missing {
        let args = renzora::job_args::ShapeThumbnail {
            id: id.to_string(),
            path: path.clone(),
            clear: clear.to_linear().to_f32_array(),
        };
        let to_companion = !thumbs.local_only.contains(id);
        if let Some(job) = jobs.as_mut().filter(|_| to_companion).and_then(|j| j.submit("shape_thumbnail", id, &args)) {
            thumbs.jobs.insert(job, id);
        } else {
            captures.request(ShapeCaptureRequest { id: id.to_string(), path, clear });
        }
        thumbs.state.insert(id, Want::Rendering);
    }
}

/// Take finished pictures from the companion and from this editor's own
/// renders. A companion that could not render one (a shape a plugin registered
/// here, which the companion does not have) hands it to this editor.
fn collect_thumbs(
    mut thumbs: ResMut<ShapeThumbs>,
    mut done: MessageReader<renzora::CompanionJobDone>,
    mut captures: ResMut<ShapeCaptures>,
) {
    let thumbs = &mut *thumbs;
    for answer in done.read() {
        let Some(id) = thumbs.jobs.remove(&answer.id) else { continue };
        match &answer.result {
            Ok(path) => {
                thumbs.state.insert(id, Want::Load(PathBuf::from(path)));
            }
            Err(why) => {
                debug!("[shapes] the companion did not render {id} ({why}); rendering it here");
                // Forgotten, so `plan_thumbs` names its file again, and marked
                // so `render_thumbs` does not hand it straight back.
                thumbs.local_only.insert(id);
                thumbs.state.remove(id);
            }
        }
    }
    for (id, result) in captures.take_finished() {
        let Some((&id, _)) = thumbs.state.get_key_value(id.as_str()) else { continue };
        match result {
            Ok(path) => {
                thumbs.state.insert(id, Want::Load(path));
            }
            Err(why) => {
                warn!("[shapes] could not render the thumbnail of {id}: {why}");
                thumbs.state.insert(id, Want::Settled);
            }
        }
    }
}
