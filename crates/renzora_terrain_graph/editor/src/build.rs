//! Full-resolution builds in the background.
//!
//! A [`BuildTerrainGraph`] message starts one: the request is gathered on
//! the main thread (the graph, the terrain's grid, its current generator
//! layer), the graph evaluates on the async compute pool, and the finished
//! result is committed back on the main thread by
//! `renzora_terrain_graph::world::commit`, which keeps the sculpt layer and
//! records one undo step. Progress and the outcome are in
//! [`TerrainGraphBuilds`], for the inspector's Build button and progress bar.
//!
//! A second build of the same terrain cancels the first. Sculpting while a
//! build runs is fine: the sculpt layer is read at commit time, not at start.

use bevy::prelude::*;
use bevy::tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task};
use renzora_terrain_graph::{
    evaluate, BuildState, BuildTerrainGraph, Cancelled, EvalCache, EvalControl, EvalResult, TerrainGenerator,
    TerrainGraph, TerrainGraphBuilds,
};

use crate::state::{target_terrain, TerrainGraphEditorState};

struct Job {
    terrain: Entity,
    task: Task<Result<EvalResult, Cancelled>>,
    control: EvalControl,
}

#[derive(Resource, Default)]
pub struct BuildJobs {
    jobs: Vec<Job>,
}

impl BuildJobs {
    pub fn is_building(&self, terrain: Entity) -> bool {
        self.jobs.iter().any(|j| j.terrain == terrain)
    }
}

fn now(world: &World) -> f64 {
    world.get_resource::<Time<Real>>().map_or(0.0, |t| t.elapsed_secs_f64())
}

/// The graph a build of `generator` uses: the open document's working copy
/// when it is the same file (unsaved edits included, so a build matches the
/// previews), else the file on disk.
fn graph_for(world: &World, generator: &TerrainGenerator) -> Result<TerrainGraph, String> {
    if let Some(st) = world.get_resource::<TerrainGraphEditorState>() {
        if st.is_open() && st.path.as_deref() == Some(generator.graph.as_str()) {
            return Ok(st.graph.clone());
        }
    }
    renzora_terrain_graph::world::load_graph(world, &generator.graph)
}

fn start(world: &mut World, terrain: Entity) -> Result<(), String> {
    let generator = world.get::<TerrainGenerator>(terrain).cloned().ok_or("the terrain has no Terrain Generator")?;
    let graph = graph_for(world, &generator)?;
    let req = renzora_terrain_graph::world::build_request(world, terrain, &generator)?;
    let mut jobs = world.resource_mut::<BuildJobs>();
    jobs.jobs.retain(|j| {
        if j.terrain == terrain {
            j.control.cancel();
        }
        j.terrain != terrain
    });
    let control = EvalControl::new();
    let ctl = control.clone();
    // No cache: a build's fields are full size and are not reused.
    let task = AsyncComputeTaskPool::get().spawn(async move { evaluate(&graph, &req, &mut EvalCache::with_capacity(0), &ctl) });
    jobs.jobs.push(Job { terrain, task, control });
    Ok(())
}

/// Start the builds asked for this frame.
pub fn start_builds(world: &mut World) {
    let requests: Vec<BuildTerrainGraph> = match world.get_resource_mut::<Messages<BuildTerrainGraph>>() {
        Some(mut m) => m.drain().collect(),
        None => return,
    };
    for r in requests {
        let t = now(world);
        let state = match start(world, r.terrain) {
            Ok(()) => BuildState::Running { progress: 0.0 },
            Err(error) => {
                renzora::console_log::console_warn("Terrain Graph", format!("build failed: {error}"));
                BuildState::Failed { error }
            }
        };
        world.resource_mut::<TerrainGraphBuilds>().set(r.terrain, state, t);
    }
}

/// Report progress, and commit the builds that finished.
pub fn poll_builds(world: &mut World) {
    let mut finished = Vec::new();
    {
        let mut jobs = world.resource_mut::<BuildJobs>();
        if jobs.jobs.is_empty() {
            return;
        }
        let mut still = Vec::new();
        for mut job in std::mem::take(&mut jobs.jobs) {
            match block_on(future::poll_once(&mut job.task)) {
                Some(result) => finished.push((job.terrain, result)),
                None => still.push(job),
            }
        }
        jobs.jobs = still;
    }
    let t = now(world);
    let progress: Vec<(Entity, f32)> = world.resource::<BuildJobs>().jobs.iter().map(|j| (j.terrain, j.control.progress())).collect();
    for (terrain, p) in progress {
        // Shown to a progress bar; whole percents are plenty and keep the
        // resource from changing every frame.
        let p = (p * 100.0).floor() / 100.0;
        world.resource_mut::<TerrainGraphBuilds>().set(terrain, BuildState::Running { progress: p }, t);
    }
    for (terrain, result) in finished {
        let state = match result {
            Err(Cancelled) => BuildState::Cancelled,
            Ok(_) if world.get_entity(terrain).is_err() => BuildState::Cancelled,
            Ok(result) => match renzora_terrain_graph::world::commit(world, terrain, &result, "Build Terrain Graph") {
                Ok(summary) => {
                    renzora::console_log::console_info("Terrain Graph", summary.clone());
                    BuildState::Done { summary }
                }
                Err(error) => {
                    renzora::console_log::console_warn("Terrain Graph", format!("build failed: {error}"));
                    BuildState::Failed { error }
                }
            },
        };
        world.resource_mut::<TerrainGraphBuilds>().set(terrain, state, t);
    }
}

/// Ask for a build of `terrain`: the call a Build button makes.
pub fn request_build(world: &mut World, terrain: Entity) {
    world.write_message(BuildTerrainGraph { terrain });
}

/// Cancel `terrain`'s running build, if it has one.
pub fn cancel_build(world: &mut World, terrain: Entity) {
    for j in world.resource::<BuildJobs>().jobs.iter().filter(|j| j.terrain == terrain) {
        j.control.cancel();
    }
}

/// The graph panel's Build: build the open graph into its target terrain,
/// pointing the terrain at the graph first if it named another (or none).
pub fn build_open_graph(world: &mut World) {
    let Some(path) = world.resource::<TerrainGraphEditorState>().path.clone() else { return };
    let Some(terrain) = target_terrain(world) else {
        renzora::console_log::console_warn("Terrain Graph", "select a terrain to build this graph into");
        return;
    };
    let mut generator = world.get::<TerrainGenerator>(terrain).cloned().unwrap_or_else(|| TerrainGenerator::new("", 1337));
    if generator.graph != path {
        generator.graph = path;
        world.entity_mut(terrain).insert(generator);
    }
    request_build(world, terrain);
}
