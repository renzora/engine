use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use bevy::prelude::*;

use crate::prelude::*;
use crate::{Bus, MAX_DEPTH};

#[derive(Resource, Default)]
struct Total(f32);

#[test]
fn an_event_reaches_every_listener() {
    let mut world = World::new();
    world.init_resource::<Total>();
    for _ in 0..2 {
        world.subscribe("combat.damaged", |world, ev| {
            let amount: f32 = ev.field("amount")?;
            world.resource_mut::<Total>().0 += amount;
            Ok(())
        });
    }
    world.emit("combat.damaged", BusValue::map().with("amount", 5.0));
    assert_eq!(world.resource::<Total>().0, 10.0);
}

#[test]
fn emitting_with_nobody_listening_is_fine() {
    let mut world = World::new();
    world.emit("nobody.home", ());
    assert!(world.get_resource::<Bus>().is_none());
}

#[test]
fn a_call_returns_the_answer() {
    let mut world = World::new();
    world.provide(
        ServiceDecl::new("math.add").arg("a", BusKind::Int).arg("b", BusKind::Int),
        |_, args| Ok((args.field::<i64>("a")? + args.field::<i64>("b")?).into()),
    );
    let sum = world.call("math.add", BusValue::map().with("a", 2).with("b", 3));
    assert_eq!(sum, Ok(BusValue::Int(5)));
}

#[test]
fn a_missing_service_says_so() {
    let mut world = World::new();
    assert_eq!(
        world.call("inventory.add_item", ()),
        Err(BusError::NotProvided("inventory.add_item".into()))
    );
}

#[test]
fn declared_arguments_are_checked_before_the_handler_runs() {
    let mut world = World::new();
    let ran = Arc::new(AtomicU32::new(0));
    let seen = ran.clone();
    world.provide(
        ServiceDecl::new("inventory.add_item")
            .arg("id", BusKind::String)
            .optional_arg("count", BusKind::Int),
        move |_, _| {
            seen.fetch_add(1, Ordering::Relaxed);
            Ok(BusValue::Null)
        },
    );

    let missing = world.call("inventory.add_item", BusValue::map().with("count", 2));
    assert!(matches!(missing, Err(BusError::BadArgs(m)) if m.contains("`id` is missing")));
    let wrong = world.call("inventory.add_item", BusValue::map().with("id", 7));
    assert!(matches!(wrong, Err(BusError::BadArgs(m)) if m.contains("should be String")));
    assert_eq!(ran.load(Ordering::Relaxed), 0);

    assert!(world.call("inventory.add_item", BusValue::map().with("id", "sword")).is_ok());
    assert_eq!(ran.load(Ordering::Relaxed), 1);
}

#[test]
fn an_int_is_accepted_where_a_float_was_declared() {
    let mut world = World::new();
    world.provide(ServiceDecl::new("half").arg("x", BusKind::Float), |_, args| {
        Ok((args.field::<f64>("x")? / 2.0).into())
    });
    assert_eq!(world.call("half", BusValue::map().with("x", 3)), Ok(BusValue::Float(1.5)));
}

#[test]
fn the_first_provider_keeps_the_name() {
    let mut world = World::new();
    assert!(world.provide("who", |_, _| Ok("first".into())));
    assert!(!world.provide("who", |_, _| Ok("second".into())));
    assert_eq!(world.call("who", ()), Ok("first".into()));
}

#[test]
fn a_listener_can_emit_and_call() {
    let mut world = World::new();
    world.init_resource::<Total>();
    world.provide("double", |_, x| Ok((x.to::<f64>()? * 2.0).into()));
    world.subscribe("outer", |world, ev| {
        let doubled = world.call("double", ev.clone())?;
        world.emit("inner", doubled);
        Ok(())
    });
    world.subscribe("inner", |world, ev| {
        world.resource_mut::<Total>().0 = ev.to::<f32>()?;
        Ok(())
    });
    world.emit("outer", 4.0);
    assert_eq!(world.resource::<Total>().0, 8.0);
    assert_eq!(world.resource::<Bus>().depth(), 0);
}

#[test]
fn two_plugins_answering_each_other_stop_at_the_limit() {
    let mut world = World::new();
    let count = Arc::new(AtomicU32::new(0));
    let (a, b) = (count.clone(), count.clone());
    world.subscribe("ping", move |world, _| {
        a.fetch_add(1, Ordering::Relaxed);
        world.emit("pong", ());
        Ok(())
    });
    world.subscribe("pong", move |world, _| {
        b.fetch_add(1, Ordering::Relaxed);
        world.emit("ping", ());
        Ok(())
    });
    world.emit("ping", ());
    assert_eq!(count.load(Ordering::Relaxed), MAX_DEPTH);
    assert_eq!(world.resource::<Bus>().depth(), 0);
}

#[test]
fn an_unsubscribed_listener_stops_hearing() {
    let mut world = World::new();
    world.init_resource::<Total>();
    let id = world.subscribe("tick", |world, _| {
        world.resource_mut::<Total>().0 += 1.0;
        Ok(())
    });
    world.emit("tick", ());
    assert!(world.unsubscribe(id));
    assert!(!world.unsubscribe(id));
    world.emit("tick", ());
    assert_eq!(world.resource::<Total>().0, 1.0);
}

#[test]
fn commands_deliver_when_applied() {
    let mut world = World::new();
    world.init_resource::<Total>();
    world.provide("seven", |_, _| Ok(7.0.into()));
    world.subscribe("set", |world, ev| {
        world.resource_mut::<Total>().0 = ev.to()?;
        Ok(())
    });

    world.commands().call_then("seven", (), |world, answer| {
        let seven: f32 = answer.and_then(|v| v.to()).unwrap_or_default();
        world.emit("set", seven);
    });
    assert_eq!(world.resource::<Total>().0, 0.0);
    world.flush();
    assert_eq!(world.resource::<Total>().0, 7.0);

    world.commands().emit("set", 1.0);
    world.flush();
    assert_eq!(world.resource::<Total>().0, 1.0);
}

#[test]
fn declarations_are_listed_for_discovery() {
    let mut world = World::new();
    world.declare_event(EventDecl::new("b.event").field("x", BusKind::Int));
    world.declare_event("a.event");
    world.provide(ServiceDecl::new("svc").describe("does a thing").returns(BusKind::Bool), |_, _| {
        Ok(true.into())
    });
    let bus = world.resource::<Bus>();
    let events: Vec<_> = bus.events().map(|e| e.name.as_str()).collect();
    assert_eq!(events, ["a.event", "b.event"]);
    assert!(bus.has_service("svc"));
    assert_eq!(bus.service("svc").map(|s| s.returns), Some(BusKind::Bool));
}

#[test]
fn a_catch_all_hears_every_event_first() {
    #[derive(Resource, Default)]
    struct Heard(Vec<String>);
    let mut world = World::new();
    world.init_resource::<Heard>();
    world.subscribe("a", |world, _| {
        world.resource_mut::<Heard>().0.push("named a".into());
        Ok(())
    });
    let id = world.subscribe_all(|world, name, _| {
        world.resource_mut::<Heard>().0.push(format!("any {name}"));
        Ok(())
    });
    world.emit("a", ());
    world.emit("b", ());
    assert_eq!(world.resource::<Heard>().0, ["any a", "named a", "any b"]);
    assert!(world.unsubscribe(id));
    world.emit("b", ());
    assert_eq!(world.resource::<Heard>().0.len(), 3);
}

#[test]
fn a_script_entity_id_reads_as_an_entity() {
    let e = Entity::from_raw_u32(42).unwrap();
    let payload = BusValue::map().with("target", e.to_bits() as i64);
    assert_eq!(payload.field::<Entity>("target"), Ok(e));
    assert!(BusKind::Entity.accepts(&BusValue::Int(e.to_bits() as i64)));
    assert!(!BusKind::Entity.accepts(&BusValue::Int(-1)));
}

#[test]
fn field_errors_name_the_field() {
    let payload = BusValue::map().with("entity", Entity::PLACEHOLDER).with("name", "x");
    assert_eq!(payload.field::<Entity>("entity"), Ok(Entity::PLACEHOLDER));
    assert!(matches!(payload.field::<f32>("name"), Err(BusError::BadArgs(m)) if m.contains("`name`")));
    assert_eq!(payload.opt_field::<i64>("count"), Ok(None));
    assert!(payload.opt_field::<i64>("name").is_err());
}
