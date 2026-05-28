use crate::{build::HtmlNode, data::HtmlTemplate};
use bevy::{
    ecs::system::{EntityCommands, SystemId, SystemParam},
    platform::collections::HashMap,
    prelude::*,
};

pub struct BindingPlugin;
impl Plugin for BindingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FunctionBindings>()
            .init_resource::<ComponentBindings>()
            // .add_event::<UiChangedEvent>()
            .add_systems(Update, (observe_interactions, observe_on_spawn))
            .add_observer(observe_node_changed);
    }
}

/// A user triggered event to notify about a change. This
/// will trigger any attached [crate::build::OnUiChange]
/// via an entity obverser function binding.
///
/// In template `on_change="my_func_binging"`
///
/// commonly used to build widgets like sliders/input that should
/// react to any change
#[derive(Event)]
pub struct UiChangedEvent {
    pub entity: Entity,
}

pub type SpawnFunction = dyn Fn(EntityCommands) + Send + Sync + 'static;

#[derive(SystemParam)]
pub struct HtmlFunctions<'w, 's> {
    bindings: ResMut<'w, FunctionBindings>,
    cmd: Commands<'w, 's>,
}

impl<'w, 's> HtmlFunctions<'w, 's> {
    pub fn register<S, M>(&mut self, name: impl Into<String>, func: S)
    where
        S: IntoSystem<In<Entity>, (), M> + 'static,
    {
        let id = self.cmd.register_system(func);
        self.bindings.register(name, id);
    }
}

#[derive(SystemParam)]
pub struct HtmlComponents<'w> {
    comps: ResMut<'w, ComponentBindings>,
}

impl<'w> HtmlComponents<'w> {
    /// link any custom html node to your template
    pub fn register(&mut self, name: impl Into<String>, template: Handle<HtmlTemplate>) {
        self.comps.register(name, move |mut cmd| {
            cmd.insert(HtmlNode(template.clone()));
        });
    }

    /// takes a closure with acces to `EntityCommands`
    /// attach custom components on spawn
    pub fn register_with_spawn_fn<SF>(
        &mut self,
        name: impl Into<String>,
        template: Handle<HtmlTemplate>,
        func: SF,
    ) where
        SF: Fn(EntityCommands) + Send + Sync + 'static,
    {
        self.comps.register(name, move |mut cmd| {
            cmd.insert(HtmlNode(template.clone()));
            func(cmd);
        });
    }
}

/// # Register custom node tags
///
/// then use in your templats `<my_comp></my_comp>`
/// `
/// ComponenRegistry.register("my_comp", &|mut cmd: EntityCommands| cmd.insert(MyBundle::default()))
/// `
#[derive(Resource, Default, Deref, DerefMut)]
pub struct ComponentBindings(HashMap<String, Box<SpawnFunction>>);

impl ComponentBindings {
    pub fn register<F>(&mut self, key: impl Into<String>, f: F)
    where
        F: Fn(EntityCommands) + Send + Sync + 'static,
    {
        let key: String = key.into();
        self.insert(key, Box::new(f));
    }

    pub fn try_spawn(&self, key: &String, entity: Entity, cmd: &mut Commands) {
        self.get(key)
            .map(|f| {
                let cmd = cmd.entity(entity);
                f(cmd);
            })
            .unwrap_or_else(|| warn!("custom tag `{key}` is not bound"));
    }
}

/// # Function binding resource
///
/// maps an oneshot system to a callable action, passing the Entity the action is
/// bound to.
///
/// in templates: `click="start_game"`
///
/// backend:
///
/// `
/// let system_id = app.register_system(|entity: In<Entity>| {})
/// FunctionBindings.register("start_game", system_id);
/// `
#[derive(Resource, Default, Deref, DerefMut, Debug)]
pub struct FunctionBindings(HashMap<String, SystemId<In<Entity>>>);

impl FunctionBindings {
    pub fn register(&mut self, key: impl Into<String>, system_id: SystemId<In<Entity>>) {
        let key: String = key.into();
        self.insert(key, system_id);
    }

    pub fn maybe_run(&self, key: &String, entity: Entity, cmd: &mut Commands) {
        self.get(key)
            .map(|id| {
                cmd.run_system_with(*id, entity);
            })
            .unwrap_or_else(|| warn!("function `{key}` is not bound"));
    }
}

fn observe_on_spawn(
    mut cmd: Commands,
    function_bindings: Res<FunctionBindings>,
    on_spawn: Query<(Entity, &crate::prelude::OnUiSpawn)>,
) {
    on_spawn.iter().for_each(|(entity, on_spawn)| {
        for spawn_fn in on_spawn.iter() {
            function_bindings.maybe_run(spawn_fn, entity, &mut cmd);
        }

        cmd.entity(entity).remove::<crate::prelude::OnUiSpawn>();
    });
}

#[rustfmt::skip]
fn observe_interactions(
    mut cmd: Commands,
    interactions: Query<(Entity, &Interaction), Changed<Interaction>>,
    function_bindings: Res<FunctionBindings>,
    on_pressed : Query<&crate::prelude::OnUiPress>,
    on_enter : Query<&crate::prelude::OnUiEnter>,
    on_exit : Query<&crate::prelude::OnUiExit>,
){
    interactions.iter().for_each(|(entity, interaction)|{
        match interaction {
            Interaction::Pressed => {
                if let Ok(crate::prelude::OnUiPress(funcs)) = on_pressed.get(entity){
                    for fn_str in funcs.iter(){
                        function_bindings.maybe_run(fn_str, entity, &mut cmd);
                    }
                }
            }
            Interaction::Hovered => {
                if let Ok(crate::prelude::OnUiEnter(funcs)) = on_enter.get(entity){
                    for fn_str in funcs.iter(){
                        function_bindings.maybe_run(fn_str, entity, &mut cmd);
                    }
                }
            },
            Interaction::None => {
                if let Ok(crate::prelude::OnUiExit(funcs)) = on_exit.get(entity){
                    for fn_str in funcs.iter(){
                        function_bindings.maybe_run(fn_str, entity, &mut cmd);
                    }
                }
            },
        }
    });
}

/// runs any attached `on_change` function when the user
/// triggers the [UiChangedEvent] on the target enttiy.
fn observe_node_changed(
    trigger: On<UiChangedEvent>,
    mut cmd: Commands,
    on_change: Query<&crate::prelude::OnUiChange>,
    function_bindings: Res<FunctionBindings>,
) {
    let entity = trigger.event().entity;

    let Ok(funcs) = on_change.get(entity) else {
        return;
    };

    for fn_str in funcs.iter() {
        function_bindings.maybe_run(fn_str, entity, &mut cmd);
    }
}
