//! Scripts on the plugin bus (`renzora::bus`).
//!
//! Scripts already had an event system of their own: `emit(name, args)` queues
//! a [`GameEvent`], and every script hears it through `on_event` (Lua) or
//! `ScriptHook::Event` (Rust). This joins that to the bus rather than giving
//! scripts a second one, so there is one event space for the whole game:
//!
//! - **Script to plugin.** Every `GameEvent` is emitted on the bus under the
//!   same name, so a plugin's `on_event("quest.started", ...)` hears a script's
//!   `emit("quest.started", {...})`.
//! - **Plugin to script.** Every bus event is queued as a `GameEvent`, so a
//!   script's `on_event` hears a plugin's `world.emit(...)` the next frame.
//! - **Script calls a service.** `action("inventory.add_item", { id = "sword",
//!   callback = "on_added" })` calls the bus service of that name; the answer
//!   arrives as an event named by `callback`. A script cannot block on an
//!   answer (it runs inside a hook, and its commands apply after it returns),
//!   which is the same reason the existing slow functions such as
//!   `mysql_query` answer through a callback event.
//!
//! Language backends need nothing new: `emit`, `on_event` and `action` were
//! already in every one. Rust scripts can also skip the bridge and use the bus
//! directly through `ctx.world()`, since they hold `&mut World`.
//!
//! ## Not echoing
//!
//! The two directions would feed each other forever: a bus event relayed into
//! the queue is triggered as a `GameEvent` when the queue drains, which would
//! relay it back onto the bus, and so on every frame. Each direction therefore
//! marks what it relays. A relay onto the bus records the event's name, and the
//! catch-all listener (which the bus runs before any named listener) recognises
//! it as the first thing it hears and lets it go by. A relay into the queue
//! records the event, and the `GameEvent` observer drops the first match.

use std::collections::HashMap;

use bevy::prelude::*;
use renzora_bus_api::{Bus, BusValue, BusWorldExt};
use renzora_log_api::console_warn;
use renzora_scripting_api::{GameEvent, GameEventQueue, ScriptAction, ScriptActionValue};

use crate::ScriptsActive;

/// An argument the bridge takes off a service call: the event to answer with.
pub const CALLBACK_ARG: &str = "callback";

/// More pending echoes than this means the queue is not being drained (nothing
/// is running scripts), so they are forgotten rather than kept forever.
const MAX_ECHOES: usize = 1024;

#[derive(Resource, Default)]
struct BusBridge {
    /// The event this bridge is emitting onto the bus right now.
    relaying: Option<String>,
    /// Events this bridge put into the script queue, not yet seen coming back
    /// out of it.
    echoes: Vec<(String, HashMap<String, ScriptActionValue>)>,
}

pub(crate) fn install(app: &mut App) {
    app.init_resource::<BusBridge>()
        .add_observer(relay_game_event_to_bus)
        .add_observer(call_service_from_script)
        .add_systems(Update, forget_echoes_while_stopped);
    app.world_mut().subscribe_all(relay_bus_event_to_scripts);
}

/// Script (or engine) event onto the bus.
fn relay_game_event_to_bus(trigger: On<GameEvent>, mut commands: Commands) {
    let ev = trigger.event().clone();
    commands.queue(move |world: &mut World| {
        if ev.from.is_none() {
            let mut bridge = world.resource_mut::<BusBridge>();
            if let Some(i) = bridge.echoes.iter().position(|(n, a)| *n == ev.name && *a == ev.args) {
                bridge.echoes.swap_remove(i);
                return;
            }
        }
        world.resource_mut::<BusBridge>().relaying = Some(ev.name.clone());
        world.emit(&ev.name, args_to_bus(&ev.args));
        // Cleared here as well, for an emit the bus refused before any
        // listener ran (nesting too deep).
        world.resource_mut::<BusBridge>().relaying = None;
    });
}

/// Bus event into the script queue.
fn relay_bus_event_to_scripts(
    world: &mut World,
    name: &str,
    payload: &BusValue,
) -> Result<(), renzora_bus_api::BusError> {
    let Some(mut bridge) = world.get_resource_mut::<BusBridge>() else {
        return Ok(());
    };
    if bridge.relaying.as_deref() == Some(name) {
        bridge.relaying = None;
        return Ok(());
    }
    deliver_to_scripts(world, name.to_string(), bus_to_args(payload));
    Ok(())
}

/// `action(name, args)` from a script, for a name the bus provides.
fn call_service_from_script(
    trigger: On<ScriptAction>,
    bus: Option<Res<Bus>>,
    mut commands: Commands,
) {
    let action = trigger.event();
    if !bus.is_some_and(|b| b.has_service(&action.name)) {
        return;
    }
    let mut args = action.args.clone();
    let service = action.name.clone();
    let callback = match args.remove(CALLBACK_ARG) {
        Some(ScriptActionValue::String(cb)) => Some(cb),
        // Not a callback name, so it is the service's own argument.
        Some(other) => {
            args.insert(CALLBACK_ARG.to_string(), other);
            None
        }
        None => None,
    };
    commands.queue(move |world: &mut World| {
        let answer = world.call(&service, args_to_bus(&args));
        match callback {
            Some(callback) => {
                let mut out = match &answer {
                    Ok(value) => bus_to_args(value),
                    Err(_) => HashMap::new(),
                };
                out.insert("ok".into(), ScriptActionValue::Bool(answer.is_ok()));
                if let Err(err) = &answer {
                    out.insert("error".into(), ScriptActionValue::String(err.to_string()));
                }
                deliver_to_scripts(world, callback, out);
            }
            // Nobody will hear the error otherwise.
            None => {
                if let Err(err) = answer {
                    console_warn("Scripting", format!("{service}: {err}"));
                }
            }
        }
    });
}

fn deliver_to_scripts(world: &mut World, name: String, args: HashMap<String, ScriptActionValue>) {
    // Outside play the queue is never drained, so anything put in it would
    // pile up and arrive all at once when Play is pressed.
    if !world.get_resource::<ScriptsActive>().is_some_and(|a| a.0) {
        return;
    }
    if let Some(mut bridge) = world.get_resource_mut::<BusBridge>() {
        if bridge.echoes.len() >= MAX_ECHOES {
            bridge.echoes.clear();
        }
        bridge.echoes.push((name.clone(), args.clone()));
    }
    world
        .get_resource_or_insert_with(GameEventQueue::default)
        .pending
        .push(GameEvent { name, args, from: None });
}

fn forget_echoes_while_stopped(active: Res<ScriptsActive>, mut bridge: ResMut<BusBridge>) {
    if !active.0 && !bridge.echoes.is_empty() {
        bridge.echoes.clear();
    }
}

/// Script arguments as a bus payload: always a map, keyed as the script wrote
/// them.
pub fn args_to_bus(args: &HashMap<String, ScriptActionValue>) -> BusValue {
    let mut map = BusValue::map();
    for (key, value) in args {
        let value = match value {
            ScriptActionValue::Float(f) => BusValue::Float((*f).into()),
            ScriptActionValue::Int(i) => BusValue::Int(*i),
            ScriptActionValue::Bool(b) => BusValue::Bool(*b),
            ScriptActionValue::String(s) => BusValue::String(s.clone()),
            ScriptActionValue::Vec3(v) => BusValue::Vec3(Vec3::from_array(*v)),
        };
        map = map.with(key.clone(), value);
    }
    map
}

/// A bus payload as script arguments.
///
/// A map becomes one argument per key. Anything else becomes a single argument
/// named `value`, so a service that answers a bare number still reads as
/// `args.value` in the callback.
///
/// Script arguments are flat, so values without a script equivalent are
/// converted: an entity becomes its id (what every script function takes), a
/// `Vec2` a `Vec3` with `z = 0`, and a list, a nested map or a `Vec4` a JSON
/// string the script can decode. A `Null` is left out.
pub fn bus_to_args(payload: &BusValue) -> HashMap<String, ScriptActionValue> {
    let mut out = HashMap::new();
    match payload {
        BusValue::Map(map) => {
            for (key, value) in map {
                if let Some(v) = to_script_value(value) {
                    out.insert(key.clone(), v);
                }
            }
        }
        other => {
            if let Some(v) = to_script_value(other) {
                out.insert("value".to_string(), v);
            }
        }
    }
    out
}

fn to_script_value(value: &BusValue) -> Option<ScriptActionValue> {
    Some(match value {
        BusValue::Null => return None,
        BusValue::Bool(b) => ScriptActionValue::Bool(*b),
        BusValue::Int(i) => ScriptActionValue::Int(*i),
        BusValue::Float(f) => ScriptActionValue::Float(*f as f32),
        BusValue::String(s) => ScriptActionValue::String(s.clone()),
        BusValue::Entity(e) => ScriptActionValue::Int(e.to_bits() as i64),
        BusValue::Vec2(v) => ScriptActionValue::Vec3([v.x, v.y, 0.0]),
        BusValue::Vec3(v) => ScriptActionValue::Vec3(v.to_array()),
        BusValue::Vec4(_) | BusValue::List(_) | BusValue::Map(_) => {
            ScriptActionValue::String(to_json(value).to_string())
        }
    })
}

fn to_json(value: &BusValue) -> serde_json::Value {
    use serde_json::Value;
    match value {
        BusValue::Null => Value::Null,
        BusValue::Bool(b) => Value::Bool(*b),
        BusValue::Int(i) => Value::from(*i),
        BusValue::Float(f) => Value::from(*f),
        BusValue::String(s) => Value::String(s.clone()),
        BusValue::Entity(e) => Value::from(e.to_bits()),
        BusValue::Vec2(v) => Value::from(v.to_array().to_vec()),
        BusValue::Vec3(v) => Value::from(v.to_array().to_vec()),
        BusValue::Vec4(v) => Value::from(v.to_array().to_vec()),
        BusValue::List(list) => Value::Array(list.iter().map(to_json).collect()),
        BusValue::Map(map) => {
            Value::Object(map.iter().map(|(k, v)| (k.clone(), to_json(v))).collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora_bus_api::ServiceDecl;

    #[derive(Resource, Default)]
    struct Heard(Vec<(String, BusValue)>);

    /// A world with the bridge installed and scripts running. The queue is
    /// drained by hand the way `run_scripts` drains it, triggering each event.
    fn world() -> World {
        let mut app = App::new();
        app.insert_resource(ScriptsActive(true));
        app.init_resource::<GameEventQueue>();
        install(&mut app);
        let mut world = std::mem::take(app.world_mut());
        world.init_resource::<Heard>();
        world
    }

    fn drain(world: &mut World) -> Vec<GameEvent> {
        let events = std::mem::take(&mut world.resource_mut::<GameEventQueue>().pending);
        for ev in &events {
            world.trigger(ev.clone());
        }
        world.flush();
        events
    }

    fn script_emit(world: &mut World, name: &str, args: HashMap<String, ScriptActionValue>) {
        let from = Some(world.spawn_empty().id());
        world.resource_mut::<GameEventQueue>().pending.push(GameEvent { name: name.into(), args, from });
    }

    #[test]
    fn a_script_event_reaches_a_plugin_once() {
        let mut world = world();
        world.subscribe("quest.started", |world, ev| {
            world.resource_mut::<Heard>().0.push(("quest.started".into(), ev.clone()));
            Ok(())
        });
        let args = HashMap::from([("id".to_string(), ScriptActionValue::String("dragon".into()))]);
        script_emit(&mut world, "quest.started", args);
        drain(&mut world);

        let heard = &world.resource::<Heard>().0;
        assert_eq!(heard.len(), 1);
        assert_eq!(heard[0].1.field::<String>("id"), Ok("dragon".into()));
        // Not relayed back into the queue for scripts to hear a second time.
        assert!(drain(&mut world).is_empty());
    }

    #[test]
    fn a_plugin_event_reaches_scripts_once() {
        let mut world = world();
        let e = world.spawn_empty().id();
        world.emit("combat.damaged", BusValue::map().with("entity", e).with("amount", 25.0));

        let events = drain(&mut world);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "combat.damaged");
        assert_eq!(events[0].args["entity"], ScriptActionValue::Int(e.to_bits() as i64));
        assert_eq!(events[0].args["amount"], ScriptActionValue::Float(25.0));
        // Draining triggered it as a GameEvent; that must not go back onto the
        // bus and round again.
        assert!(drain(&mut world).is_empty());
    }

    #[test]
    fn a_plugin_reply_to_a_script_event_still_reaches_scripts() {
        let mut world = world();
        world.subscribe("ping", |world, _| {
            world.emit("pong", BusValue::map().with("n", 1));
            Ok(())
        });
        script_emit(&mut world, "ping", HashMap::new());
        drain(&mut world);
        let next = drain(&mut world);
        assert_eq!(next.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["pong"]);
    }

    #[test]
    fn a_script_calls_a_service_and_hears_the_answer() {
        let mut world = world();
        world.provide(
            ServiceDecl::new("inventory.add_item").arg("id", renzora_bus_api::BusKind::String),
            |_, args| {
                let id: String = args.field("id")?;
                Ok(BusValue::map().with("added", id).with("slot", 3))
            },
        );
        let me = world.spawn_empty().id();
        let action = |args: Vec<(&str, ScriptActionValue)>| ScriptAction {
            name: "inventory.add_item".into(),
            entity: me,
            target_entity: None,
            args: args.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        };

        world.trigger(action(vec![
            ("id", ScriptActionValue::String("sword".into())),
            ("callback", ScriptActionValue::String("on_added".into())),
        ]));
        world.flush();
        let events = drain(&mut world);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "on_added");
        assert_eq!(events[0].args["ok"], ScriptActionValue::Bool(true));
        assert_eq!(events[0].args["added"], ScriptActionValue::String("sword".into()));
        assert_eq!(events[0].args["slot"], ScriptActionValue::Int(3));

        // A refused call answers too, with the reason.
        world.trigger(action(vec![("callback", ScriptActionValue::String("on_added".into()))]));
        world.flush();
        let events = drain(&mut world);
        assert_eq!(events[0].args["ok"], ScriptActionValue::Bool(false));
        assert!(matches!(&events[0].args["error"], ScriptActionValue::String(m) if m.contains("`id`")));
    }

    #[test]
    fn an_action_nobody_provides_is_left_alone() {
        let mut world = world();
        let me = world.spawn_empty().id();
        world.trigger(ScriptAction {
            name: "apply_force".into(),
            entity: me,
            target_entity: None,
            args: HashMap::from([("callback".to_string(), ScriptActionValue::String("x".into()))]),
        });
        world.flush();
        assert!(drain(&mut world).is_empty());
    }

    #[test]
    fn nothing_queues_while_scripts_are_stopped() {
        let mut world = world();
        world.insert_resource(ScriptsActive(false));
        world.emit("combat.damaged", 1);
        assert!(world.resource::<GameEventQueue>().pending.is_empty());
    }

    #[test]
    fn values_without_a_script_type_are_converted() {
        let e = Entity::from_raw_u32(7).unwrap();
        let args = bus_to_args(
            &BusValue::map()
                .with("who", e)
                .with("at", Vec2::new(1.0, 2.0))
                .with("items", vec![1, 2])
                .with("none", BusValue::Null),
        );
        assert_eq!(args["who"], ScriptActionValue::Int(e.to_bits() as i64));
        assert_eq!(args["at"], ScriptActionValue::Vec3([1.0, 2.0, 0.0]));
        assert_eq!(args["items"], ScriptActionValue::String("[1,2]".into()));
        assert!(!args.contains_key("none"));
        assert_eq!(bus_to_args(&BusValue::Int(5))["value"], ScriptActionValue::Int(5));
    }
}
