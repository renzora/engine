//! The bus itself, and the `App` / `World` / `Commands` methods that reach it.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use bevy::log::warn;
use bevy::prelude::*;

use crate::decl::{check, EventDecl, ServiceDecl};
use crate::{BusError, BusValue};

/// How deep events and calls may nest (a listener emitting, a service calling
/// another) before the bus refuses. Two plugins that each answer the other's
/// event would otherwise recurse until the stack overflowed, and a stack
/// overflow takes the editor down with no message at all.
pub const MAX_DEPTH: u32 = 32;

type Listener = Arc<dyn Fn(&mut World, &BusValue) -> Result<(), BusError> + Send + Sync>;
type AnyListener = Arc<dyn Fn(&mut World, &str, &BusValue) -> Result<(), BusError> + Send + Sync>;
type Handler = Arc<dyn Fn(&mut World, &BusValue) -> Result<BusValue, BusError> + Send + Sync>;

/// Returned by [`BusWorldExt::subscribe`], for [`BusWorldExt::unsubscribe`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ListenerId(u64);

struct Service {
    decl: ServiceDecl,
    handler: Handler,
}

/// Every event declaration, service and listener in the process.
///
/// A `Resource` rather than a static: it lives and dies with its `World`, so a
/// test or a second app gets a bus of its own. Created on first use, so nothing
/// has to install a plugin before the first `provide`.
#[derive(Resource, Default)]
pub struct Bus {
    events: BTreeMap<String, EventDecl>,
    services: BTreeMap<String, Service>,
    listeners: HashMap<String, Vec<(ListenerId, Listener)>>,
    /// Listeners to every event, for bridges (scripting) and debugging tools.
    any_listeners: Vec<(ListenerId, AnyListener)>,
    next_listener: u64,
    depth: u32,
    /// Problems already logged. A malformed event is usually emitted every
    /// frame, and a log line per frame stalls the editor, so each one is said
    /// once.
    warned: HashSet<String>,
}

impl Bus {
    /// Every declared event, sorted by name.
    pub fn events(&self) -> impl Iterator<Item = &EventDecl> {
        self.events.values()
    }

    pub fn event(&self, name: &str) -> Option<&EventDecl> {
        self.events.get(name)
    }

    /// Every provided service, sorted by name.
    pub fn services(&self) -> impl Iterator<Item = &ServiceDecl> {
        self.services.values().map(|s| &s.decl)
    }

    pub fn service(&self, name: &str) -> Option<&ServiceDecl> {
        self.services.get(name).map(|s| &s.decl)
    }

    /// Whether anything answers `name`: how a plugin finds out that an
    /// optional companion plugin is installed.
    pub fn has_service(&self, name: &str) -> bool {
        self.services.contains_key(name)
    }

    pub fn listener_count(&self, event: &str) -> usize {
        self.listeners.get(event).map_or(0, Vec::len)
    }

    /// How many events and calls are running right now, one inside another.
    /// Zero between dispatches; a nonzero value seen from a system means a
    /// dispatch unwound without restoring it.
    pub fn depth(&self) -> u32 {
        self.depth
    }

    fn warn_once(&mut self, key: String, message: impl FnOnce() -> String) {
        if self.warned.insert(key) {
            warn!("[bus] {}", message());
        }
    }
}

/// Registration while a plugin builds.
pub trait BusAppExt {
    /// Says this plugin emits `decl.name`, with the fields it lists.
    fn declare_event(&mut self, decl: impl Into<EventDecl>) -> &mut Self;

    /// Answers calls to a service. A second provider of the same name is
    /// refused with a warning: the first one keeps answering.
    fn provide<F>(&mut self, decl: impl Into<ServiceDecl>, handler: F) -> &mut Self
    where
        F: Fn(&mut World, &BusValue) -> Result<BusValue, BusError> + Send + Sync + 'static;

    /// Runs `listener` every time `event` is emitted, by anyone.
    fn on_event<F>(&mut self, event: &str, listener: F) -> &mut Self
    where
        F: Fn(&mut World, &BusValue) -> Result<(), BusError> + Send + Sync + 'static;

    /// Runs `listener` for every event, with its name. See
    /// [`BusWorldExt::subscribe_all`].
    fn on_any_event<F>(&mut self, listener: F) -> &mut Self
    where
        F: Fn(&mut World, &str, &BusValue) -> Result<(), BusError> + Send + Sync + 'static;
}

impl BusAppExt for App {
    fn declare_event(&mut self, decl: impl Into<EventDecl>) -> &mut Self {
        self.world_mut().declare_event(decl);
        self
    }

    fn provide<F>(&mut self, decl: impl Into<ServiceDecl>, handler: F) -> &mut Self
    where
        F: Fn(&mut World, &BusValue) -> Result<BusValue, BusError> + Send + Sync + 'static,
    {
        self.world_mut().provide(decl, handler);
        self
    }

    fn on_event<F>(&mut self, event: &str, listener: F) -> &mut Self
    where
        F: Fn(&mut World, &BusValue) -> Result<(), BusError> + Send + Sync + 'static,
    {
        self.world_mut().subscribe(event, listener);
        self
    }

    fn on_any_event<F>(&mut self, listener: F) -> &mut Self
    where
        F: Fn(&mut World, &str, &BusValue) -> Result<(), BusError> + Send + Sync + 'static,
    {
        self.world_mut().subscribe_all(listener);
        self
    }
}

/// The bus from anything holding `&mut World`: an exclusive system, a
/// listener, a service handler, a Rust script.
pub trait BusWorldExt {
    fn declare_event(&mut self, decl: impl Into<EventDecl>);

    /// `false` when the name was already provided, in which case nothing
    /// changed.
    fn provide<F>(&mut self, decl: impl Into<ServiceDecl>, handler: F) -> bool
    where
        F: Fn(&mut World, &BusValue) -> Result<BusValue, BusError> + Send + Sync + 'static;

    fn subscribe<F>(&mut self, event: &str, listener: F) -> ListenerId
    where
        F: Fn(&mut World, &BusValue) -> Result<(), BusError> + Send + Sync + 'static;

    /// Runs `listener` for every event, with its name. Meant for bridges and
    /// tools, not gameplay: a plugin that reacts to everything is coupled to
    /// everything. Runs before the named listeners of the same emit.
    fn subscribe_all<F>(&mut self, listener: F) -> ListenerId
    where
        F: Fn(&mut World, &str, &BusValue) -> Result<(), BusError> + Send + Sync + 'static;

    /// `false` when the id was not subscribed (already removed).
    fn unsubscribe(&mut self, id: ListenerId) -> bool;

    /// Delivers `payload` to every listener of `event` now, before returning.
    /// Nobody listening is not an error: the emitter cannot know which plugins
    /// are installed, and should not have to.
    fn emit(&mut self, event: &str, payload: impl Into<BusValue>);

    /// Calls a service and returns its answer.
    fn call(&mut self, service: &str, args: impl Into<BusValue>) -> Result<BusValue, BusError>;
}

impl BusWorldExt for World {
    fn declare_event(&mut self, decl: impl Into<EventDecl>) {
        let decl = decl.into();
        let mut bus = self.get_resource_or_init::<Bus>();
        if let Some(same_fields) = bus.events.get(&decl.name).map(|e| e.fields == decl.fields) {
            // Two plugins may well both emit one event; only disagreeing about
            // its fields is worth saying, since the first declaration is the
            // one payloads are checked against.
            if !same_fields {
                let name = decl.name.clone();
                bus.warn_once(format!("redeclare:{name}"), || {
                    format!("event `{name}` was declared again with different fields; keeping the first")
                });
            }
            return;
        }
        bus.events.insert(decl.name.clone(), decl);
    }

    fn provide<F>(&mut self, decl: impl Into<ServiceDecl>, handler: F) -> bool
    where
        F: Fn(&mut World, &BusValue) -> Result<BusValue, BusError> + Send + Sync + 'static,
    {
        let decl = decl.into();
        let mut bus = self.get_resource_or_init::<Bus>();
        if bus.services.contains_key(&decl.name) {
            // Keeping the first rather than the last: which plugin builds
            // second is load order, and a service that silently changed hands
            // with load order would be a bug nobody could reproduce.
            warn!("[bus] service `{}` is already provided; the second provider was ignored", decl.name);
            return false;
        }
        bus.services.insert(decl.name.clone(), Service { decl, handler: Arc::new(handler) });
        true
    }

    fn subscribe<F>(&mut self, event: &str, listener: F) -> ListenerId
    where
        F: Fn(&mut World, &BusValue) -> Result<(), BusError> + Send + Sync + 'static,
    {
        let mut bus = self.get_resource_or_init::<Bus>();
        let id = ListenerId(bus.next_listener);
        bus.next_listener += 1;
        bus.listeners.entry(event.to_string()).or_default().push((id, Arc::new(listener)));
        id
    }

    fn subscribe_all<F>(&mut self, listener: F) -> ListenerId
    where
        F: Fn(&mut World, &str, &BusValue) -> Result<(), BusError> + Send + Sync + 'static,
    {
        let mut bus = self.get_resource_or_init::<Bus>();
        let id = ListenerId(bus.next_listener);
        bus.next_listener += 1;
        bus.any_listeners.push((id, Arc::new(listener)));
        id
    }

    fn unsubscribe(&mut self, id: ListenerId) -> bool {
        let Some(mut bus) = self.get_resource_mut::<Bus>() else {
            return false;
        };
        if let Some(i) = bus.any_listeners.iter().position(|(l, _)| *l == id) {
            bus.any_listeners.remove(i);
            return true;
        }
        for list in bus.listeners.values_mut() {
            if let Some(i) = list.iter().position(|(l, _)| *l == id) {
                list.remove(i);
                return true;
            }
        }
        false
    }

    fn emit(&mut self, event: &str, payload: impl Into<BusValue>) {
        let payload = payload.into();
        let (any, listeners): (Vec<AnyListener>, Vec<Listener>) = {
            // No bus yet means nobody has subscribed to anything.
            let Some(mut bus) = self.get_resource_mut::<Bus>() else {
                return;
            };
            let bus = bus.as_mut();
            // Delivered anyway: a listener may cope with the payload, and
            // dropping it would turn one plugin's mistake into another plugin's
            // missing behaviour.
            let problem = bus.events.get(event).and_then(|d| check(&d.fields, &payload).err());
            if let Some(problem) = problem {
                bus.warn_once(format!("emit:{event}"), || format!("event `{event}`: {problem}"));
            }
            if bus.depth >= MAX_DEPTH {
                bus.warn_once(format!("deep:{event}"), || {
                    format!("event `{event}` not delivered: events and calls nested {MAX_DEPTH} deep")
                });
                return;
            }
            // Cloned out so the bus is not borrowed while listeners run: a
            // listener may emit, call, subscribe or unsubscribe. One that
            // unsubscribes still runs for this event, which is already on its
            // way.
            let any: Vec<AnyListener> = bus.any_listeners.iter().map(|(_, l)| l.clone()).collect();
            let listeners: Vec<Listener> = bus
                .listeners
                .get(event)
                .map(|list| list.iter().map(|(_, l)| l.clone()).collect())
                .unwrap_or_default();
            if any.is_empty() && listeners.is_empty() {
                return;
            }
            bus.depth += 1;
            (any, listeners)
        };

        // Catch-all first. A bridge relaying an event onto the bus from
        // elsewhere recognises its own relay as the first thing it hears, before
        // a named listener can emit something new in reply.
        for listener in any {
            if let Err(err) = listener(self, event, &payload) {
                if let Some(mut bus) = self.get_resource_mut::<Bus>() {
                    bus.warn_once(format!("any:{event}:{err}"), || {
                        format!("a catch-all listener failed on `{event}`: {err}")
                    });
                }
            }
        }

        for listener in listeners {
            if let Err(err) = listener(self, &payload) {
                if let Some(mut bus) = self.get_resource_mut::<Bus>() {
                    bus.warn_once(format!("listener:{event}:{err}"), || {
                        format!("a listener of `{event}` failed: {err}")
                    });
                }
            }
        }

        if let Some(mut bus) = self.get_resource_mut::<Bus>() {
            bus.depth = bus.depth.saturating_sub(1);
        }
    }

    fn call(&mut self, service: &str, args: impl Into<BusValue>) -> Result<BusValue, BusError> {
        let args = args.into();
        let handler = {
            let Some(mut bus) = self.get_resource_mut::<Bus>() else {
                return Err(BusError::NotProvided(service.to_string()));
            };
            let Some(entry) = bus.services.get(service) else {
                return Err(BusError::NotProvided(service.to_string()));
            };
            check(&entry.decl.args, &args)
                .map_err(|problem| BusError::BadArgs(format!("{service}: {problem}")))?;
            let handler = entry.handler.clone();
            if bus.depth >= MAX_DEPTH {
                return Err(BusError::TooDeep(service.to_string()));
            }
            bus.depth += 1;
            handler
        };

        let answer = handler(self, &args);

        if let Some(mut bus) = self.get_resource_mut::<Bus>() {
            bus.depth = bus.depth.saturating_sub(1);
        }
        answer
    }
}

/// The bus from an ordinary system, which has no `&mut World`. Both queue a
/// command, so delivery happens when the system's commands are applied.
pub trait BusCommandsExt {
    fn emit(&mut self, event: impl Into<String>, payload: impl Into<BusValue>);

    /// Calls a service once commands are applied and hands the answer to
    /// `then`, which gets `&mut World` to act on it.
    fn call_then<F>(&mut self, service: impl Into<String>, args: impl Into<BusValue>, then: F)
    where
        F: FnOnce(&mut World, Result<BusValue, BusError>) + Send + 'static;
}

impl BusCommandsExt for Commands<'_, '_> {
    fn emit(&mut self, event: impl Into<String>, payload: impl Into<BusValue>) {
        let event = event.into();
        let payload = payload.into();
        self.queue(move |world: &mut World| world.emit(&event, payload));
    }

    fn call_then<F>(&mut self, service: impl Into<String>, args: impl Into<BusValue>, then: F)
    where
        F: FnOnce(&mut World, Result<BusValue, BusError>) + Send + 'static,
    {
        let service = service.into();
        let args = args.into();
        self.queue(move |world: &mut World| {
            let answer = world.call(&service, args);
            then(world, answer);
        });
    }
}
