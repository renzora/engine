//! Plain `StandardMaterial` stand-ins for node-graph materials, for the ray
//! tracer.
//!
//! A ray tracer shades a hit from `StandardMaterial` data; it cannot run a
//! graph's generated shader at a hit point. So while one asks for them
//! ([`TracedStandInsWanted`]), every mesh drawn with a [`GraphMaterial`] gets a
//! [`TracedStandIn`] holding a copy of that material's base: the
//! `StandardMaterial` approximation of the graph that the prepass and shadow
//! passes already shade from (`standard_build::apply_base_approximation`).
//!
//! One stand-in per graph material, shared by every mesh using it, and kept in
//! step when the graph material changes (a recompile, a parameter edit, a
//! texture tier swap). The ray tracer captures a mesh's material handle once,
//! when it adds the mesh to its scene, so the handle must stay put and only the
//! asset behind it may change.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use renzora::{TracedStandIn, TracedStandInsWanted};

use super::GraphMaterial;

pub(crate) struct TracedStandInPlugin;

impl Plugin for TracedStandInPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StandIns>()
            .add_systems(PostUpdate, sync_traced_stand_ins);
    }
}

/// The stand-in for each graph material, by the graph material's id.
#[derive(Resource, Default)]
struct StandIns(HashMap<AssetId<GraphMaterial>, Handle<StandardMaterial>>);

/// Marks a [`TracedStandIn`] this module gave a graph-material mesh, so the
/// sweep below takes back only its own. Other crates give stand-ins too (a
/// terrain chunk's surface material cannot be traced either), and an unmarked
/// sweep stripped theirs every frame: whether Solari saw the terrain at all
/// then came down to system order, and with nothing else in the scene it
/// traced nothing and drew the whole view black.
#[derive(Component)]
struct GraphStandIn;

fn sync_traced_stand_ins(
    mut commands: Commands,
    wanted: Option<Res<TracedStandInsWanted>>,
    mut stand_ins: ResMut<StandIns>,
    graph_materials: Res<Assets<GraphMaterial>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
    mut events: MessageReader<AssetEvent<GraphMaterial>>,
    meshes: Query<(Entity, &MeshMaterial3d<GraphMaterial>, Option<&TracedStandIn>)>,
    tagged: Query<Entity, (With<GraphStandIn>, Without<MeshMaterial3d<GraphMaterial>>)>,
) {
    if wanted.is_none() {
        // Read either way, or a burst of edits made while Solari was off would
        // be replayed against stand-ins created after it comes back on.
        events.clear();
        if !stand_ins.0.is_empty() {
            stand_ins.0.clear();
            for (entity, _, stand_in) in &meshes {
                if stand_in.is_some() {
                    commands.entity(entity).try_remove::<(TracedStandIn, GraphStandIn)>();
                }
            }
        }
        return;
    }

    for event in events.read() {
        match event {
            AssetEvent::Modified { id } => {
                if let (Some(stand_in), Some(graph)) =
                    (stand_ins.0.get(id), graph_materials.get(*id))
                {
                    if let Some(mut standard) = standard_materials.get_mut(stand_in) {
                        *standard = graph.base.clone();
                    }
                }
            }
            AssetEvent::Removed { id } => {
                stand_ins.0.remove(id);
            }
            _ => {}
        }
    }

    for (entity, material, current) in &meshes {
        let id = material.id();
        let stand_in = match stand_ins.0.get(&id) {
            Some(handle) => handle.clone(),
            None => {
                // Not loaded yet: try again next frame.
                let Some(graph) = graph_materials.get(id) else {
                    continue;
                };
                let handle = standard_materials.add(graph.base.clone());
                stand_ins.0.insert(id, handle.clone());
                handle
            }
        };
        if current.is_none_or(|c| c.0 != stand_in) {
            commands.entity(entity).try_insert((TracedStandIn(stand_in), GraphStandIn));
        }
    }

    // A mesh whose graph material was swapped for something else keeps a
    // stand-in describing a material it no longer has.
    for entity in &tagged {
        commands.entity(entity).try_remove::<(TracedStandIn, GraphStandIn)>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stand_in_another_crate_gave_is_left_alone() {
        let mut app = App::new();
        app.init_resource::<Assets<GraphMaterial>>()
            .init_resource::<Assets<StandardMaterial>>()
            .add_message::<AssetEvent<GraphMaterial>>()
            .insert_resource(TracedStandInsWanted)
            .add_plugins(TracedStandInPlugin);
        let handle = app.world_mut().resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial::default());
        let terrain_chunk = app.world_mut().spawn(TracedStandIn(handle.clone())).id();
        app.update();
        app.update();
        assert!(app.world().get::<TracedStandIn>(terrain_chunk).is_some());
    }
}
