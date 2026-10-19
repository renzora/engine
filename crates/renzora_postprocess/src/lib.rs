//! Post-process effects written as shader files.
//!
//! An effect is one `.wesl` file that declares its own sliders in comments (see
//! [`effect_file`]). A [`renzora::PostEffects`] component on a scene entity lists
//! the effects to run, in order, and is routed to cameras like every other
//! effect. Nothing is compiled per effect: one render pass runs whatever the
//! list holds, so adding an effect to a project is copying a file into it.
//!
//! This replaced about fifty effect plugins, each a single shader plus a Rust
//! struct restating its settings. Building those took minutes on every engine
//! update and the two copies of each settings layout were never checked
//! against each other.
//!
//! Split in two: [`library`] and the chain systems here run in the main world
//! and decide what each camera draws; [`render`] draws it.

pub mod effect_file;
pub mod library;
mod render;

use bevy::prelude::*;
use bevy::render::extract_component::{ExtractComponent, ExtractComponentPlugin};
use renzora::{EffectRouting, PostEffects};

pub use effect_file::{EffectField, EffectInfo, FieldKind, FieldRole};
pub use library::EffectLibrary;

/// What one camera draws this frame, resolved from its routed
/// [`PostEffects`]: only effects that are on and whose shader parsed.
///
/// Rebuilt by the main world and copied to the render world each frame, so the
/// render side never reads the authored list or the shader text.
#[derive(Component, Clone, Debug, Default, PartialEq, ExtractComponent)]
#[extract_app(bevy::render::RenderApp)]
pub struct PostEffectChain {
    pub passes: Vec<ChainPass>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChainPass {
    pub shader: Handle<Shader>,
    /// The packed settings buffer.
    pub settings: Vec<u8>,
    /// The shader reads a frozen copy of the frame at bindings 3 and 4.
    pub snapshot: bool,
    /// Hold the copy rather than refreshing it this frame.
    pub frozen: bool,
}

#[derive(Default)]
pub struct PostProcessPlugin;

impl Plugin for PostProcessPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] PostProcessPlugin");
        app.register_type::<PostEffects>()
            .register_type::<renzora::PostEffect>()
            .register_type::<renzora::PostEffectValue>()
            .init_resource::<EffectLibrary>()
            .add_plugins(ExtractComponentPlugin::<PostEffectChain>::default())
            .add_systems(
                Update,
                (
                    library::request_shaders,
                    library::read_effects,
                    build_chains,
                )
                    .chain()
                    .run_if(
                        any_with_component::<PostEffects>
                            .or_else(any_with_component::<PostEffectChain>),
                    ),
            );
        render::build(app);
    }
}

renzora::add!(PostProcessPlugin, Runtime);

/// Resolve every routed camera's chain.
///
/// Runs every frame while any list exists, because an effect with a `time`
/// field changes its settings every frame. A chain that comes out the same as
/// the camera's current one is not written back, so a still scene does not
/// mark the camera changed and re-extract it.
fn build_chains(
    mut commands: Commands,
    routing: Res<EffectRouting>,
    sources: Query<&PostEffects>,
    existing: Query<(Entity, &PostEffectChain)>,
    library: Res<EffectLibrary>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs() % effect_file::TIME_WRAP;
    let mut built: Vec<Entity> = Vec::new();

    for (camera, source_list) in routing.iter() {
        // The first source carrying a list wins, as for every routed effect.
        let Some(list) = source_list.iter().find_map(|&s| sources.get(s).ok()) else {
            continue;
        };
        let passes: Vec<ChainPass> = list
            .effects
            .iter()
            .filter(|e| e.enabled)
            .filter_map(|e| {
                let (handle, info) = library.ready(&e.shader)?;
                Some(ChainPass {
                    shader: handle.clone(),
                    settings: info.pack(e, now),
                    snapshot: info.snapshot,
                    frozen: info.frozen(e),
                })
            })
            .collect();
        if passes.is_empty() {
            continue;
        }
        built.push(*camera);
        let chain = PostEffectChain { passes };
        let unchanged = existing.get(*camera).is_ok_and(|(_, c)| *c == chain);
        if !unchanged {
            // `try_insert`: a routed camera can be despawned the frame its
            // route is still in the table (closing the camera preview).
            commands.entity(*camera).try_insert(chain);
        }
    }

    for (camera, _) in &existing {
        if !built.contains(&camera) {
            commands.entity(camera).try_remove::<PostEffectChain>();
        }
    }
}
