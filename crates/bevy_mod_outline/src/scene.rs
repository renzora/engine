use std::mem;

use bevy::{
    ecs::{lifecycle::HookContext, system::SystemId, world::DeferredWorld},
    prelude::*,
    scene::{InstanceId, SceneInstance, SceneInstanceReady},
};

use crate::{ComputedOutline, InheritOutline};

#[derive(Default)]
enum InternalState {
    #[default]
    Pending,
    WaitingForSceneReady(Entity),
    SceneProcessed(InstanceId),
}

/// Automatically inherits outlines for the entities in a scene.
///
/// Once a `SceneInstance` marked with this component is ready, it will add
/// `InheritOutline` to all of the scene's entities. If this component is
/// removed then the `InheritOutline` components will be removed too.
#[derive(Component, Default)]
#[component(on_add = add_hook, on_remove = remove_hook)]
pub struct AsyncSceneInheritOutline {
    state: InternalState,
}

fn add_hook(mut world: DeferredWorld<'_>, context: HookContext) {
    let add_outline = world
        .resource::<AsyncSceneInheritOutlineSystems>()
        .add_outline;
    world
        .commands()
        .run_system_with(add_outline, context.entity);
}

fn add_outline(
    entity_input: In<Entity>,
    mut commands: Commands,
    mut query: Query<(&mut AsyncSceneInheritOutline, Option<&SceneInstance>)>,
    systems: Res<AsyncSceneInheritOutlineSystems>,
    scene_spawner: Res<SceneSpawner>,
) {
    let Ok((mut scene_outline, scene_instance)) = query.get_mut(*entity_input) else {
        return;
    };
    let mut ready = false;
    if let Some(scene_instance) = scene_instance {
        let iid = **scene_instance;
        if scene_spawner.instance_is_ready(iid) {
            for child in scene_spawner.iter_instance_entities(iid) {
                if let Ok(mut ecmds) = commands.get_entity(child) {
                    ecmds.insert(InheritOutline);
                }
            }
            if let InternalState::WaitingForSceneReady(observer) = scene_outline.state {
                if let Ok(mut ecmds) = commands.get_entity(observer) {
                    ecmds.despawn();
                }
            }
            scene_outline.state = InternalState::SceneProcessed(iid);
            ready = true;
        }
    }
    if !ready {
        let add_outline = systems.add_outline;
        let observer = commands
            .spawn(
                Observer::new(
                    move |trigger: On<SceneInstanceReady>, mut commands: Commands| {
                        commands.run_system_with(add_outline, trigger.event_target());
                    },
                )
                .with_entity(*entity_input),
            )
            .id();
        scene_outline.state = InternalState::WaitingForSceneReady(observer);
    }
}

fn remove_hook(mut world: DeferredWorld<'_>, context: HookContext) {
    let remove_outline = world
        .resource::<AsyncSceneInheritOutlineSystems>()
        .remove_outline;
    let mut entity_ref = world.entity_mut(context.entity);
    let outline = mem::take(
        entity_ref
            .get_mut::<AsyncSceneInheritOutline>()
            .unwrap()
            .into_inner(),
    );
    world.commands().run_system_with(remove_outline, outline);
}

fn remove_outline(
    input: In<AsyncSceneInheritOutline>,
    mut commands: Commands,
    scene_spawner: Res<SceneSpawner>,
) {
    match input.state {
        InternalState::WaitingForSceneReady(observer) => {
            if let Ok(mut ecmds) = commands.get_entity(observer) {
                ecmds.despawn();
            }
        }
        InternalState::SceneProcessed(iid) => {
            for child in scene_spawner.iter_instance_entities(iid) {
                if let Ok(mut ecmds) = commands.get_entity(child) {
                    ecmds.remove::<(InheritOutline, ComputedOutline)>();
                }
            }
        }
        InternalState::Pending => {}
    }
}

#[derive(Resource)]
pub(crate) struct AsyncSceneInheritOutlineSystems {
    add_outline: SystemId<In<Entity>, ()>,
    remove_outline: SystemId<In<AsyncSceneInheritOutline>, ()>,
}

impl FromWorld for AsyncSceneInheritOutlineSystems {
    fn from_world(world: &mut World) -> Self {
        Self {
            add_outline: world.register_system(add_outline),
            remove_outline: world.register_system(remove_outline),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{ecs::system::RunSystemOnce, scene::ScenePlugin};

    #[derive(Component, Reflect, Default)]
    #[reflect(Component, Default)]
    struct SpawnedEntity;

    fn setup() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins((AssetPlugin::default(), ScenePlugin))
            .register_type::<SpawnedEntity>()
            .init_resource::<AsyncSceneInheritOutlineSystems>();

        // Create scene
        let mut scene_world = World::new();
        scene_world.insert_resource(
            app.world()
                .get_resource::<AppTypeRegistry>()
                .unwrap()
                .clone(),
        );
        scene_world.spawn(SpawnedEntity);
        scene_world.spawn(SpawnedEntity);
        let scene_handle = app
            .world_mut()
            .get_resource_mut::<AssetServer>()
            .unwrap()
            .add(DynamicScene::from_world(&scene_world));

        // Prepare scene to spawn at next update
        let scene_entity = app.world_mut().spawn(DynamicSceneRoot(scene_handle)).id();
        assert_counts(&mut app, 0, 0);
        (app, scene_entity)
    }

    fn assert_counts(app: &mut App, without: usize, with: usize) {
        app.world_mut()
            .run_system_once(
                move |without_query: Query<&SpawnedEntity, Without<InheritOutline>>,
                      with_query: Query<&SpawnedEntity, With<InheritOutline>>| {
                    assert_eq!(without_query.iter().count(), without);
                    assert_eq!(with_query.iter().count(), with);
                },
            )
            .expect("Failed to run system.");
    }

    #[test]
    fn test_add_before_scene_ready() {
        let (mut app, scene_entity) = setup();
        app.world_mut()
            .get_entity_mut(scene_entity)
            .unwrap()
            .insert(AsyncSceneInheritOutline::default());
        app.update();
        assert_counts(&mut app, 0, 2);
    }

    #[test]
    fn test_add_after_scene_ready() {
        let (mut app, scene_entity) = setup();
        app.update();
        assert_counts(&mut app, 2, 0);
        app.world_mut()
            .get_entity_mut(scene_entity)
            .unwrap()
            .insert(AsyncSceneInheritOutline::default());
        app.update();
        assert_counts(&mut app, 0, 2);
    }

    #[test]
    fn test_remove_after_scene_ready() {
        let (mut app, scene_entity) = setup();
        app.world_mut()
            .get_entity_mut(scene_entity)
            .unwrap()
            .insert(AsyncSceneInheritOutline::default());
        app.update();
        assert_counts(&mut app, 0, 2);

        // Remove marker component
        app.world_mut()
            .get_entity_mut(scene_entity)
            .unwrap()
            .remove::<AsyncSceneInheritOutline>();
        app.update();
        assert_counts(&mut app, 2, 0);
    }

    #[test]
    fn test_remove_before_scene_ready() {
        let (mut app, scene_entity) = setup();
        app.world_mut()
            .get_entity_mut(scene_entity)
            .unwrap()
            .insert(AsyncSceneInheritOutline::default());
        app.world_mut()
            .get_entity_mut(scene_entity)
            .unwrap()
            .remove::<AsyncSceneInheritOutline>();
        app.update();
        assert_counts(&mut app, 2, 0);
    }
}
