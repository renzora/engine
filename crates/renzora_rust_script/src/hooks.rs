//! Delivering lifecycle events to Rust scripts.
//!
//! [`crate::dispatch`] calls one function per entity per frame. This module
//! calls the *other* entry point — the optional `renzora_script_hook` a script
//! exports via `renzora::script!(update, hooks = …)` — with the events Lua has
//! always had: `on_ready`, `on_ui`, `on_rpc`, `on_scene_loaded`,
//! `on_animation_event`, `on_http`, `on_player_joined` / `on_player_left`.
//!
//! # Why this reads the inboxes instead of being handed them
//!
//! Those events arrive in resources in the contract crate (`ScriptUiInbox`,
//! `ScriptSceneInbox`, …) which `renzora_scripting`'s executor drains with
//! `std::mem::take` each frame to fan out to the Lua VM. A destructive drain has
//! exactly one consumer, and it was Lua — which is why a Rust script could not
//! be told a scene had loaded, and why a loading screen was Lua-only.
//!
//! Rather than change that drain and every backend that depends on its timing,
//! this runs **before** [`ScriptingSet::ScriptExecution`] and *reads* the
//! inboxes without emptying them. Lua then drains as it always did. Both
//! backends see the same frame's events, and the ordering is explicit rather
//! than whichever system the scheduler happened to run first.
//!
//! Copying the pending events out is deliberate too: a script gets `&mut World`
//! and may despawn anything, including the resource holding the list it is being
//! told about.

use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use renzora::ScriptHook;
use renzora_scripting::ScriptComponent;

use crate::{LoadedScripts, ScriptHookFn};

/// Which `(entity, script)` pairs have had their `Ready` hook called.
///
/// A resource rather than a component because readiness is per *script*, and an
/// entity can carry several. Pruned each frame against live entities so a scene
/// change — which despawns everything and frees the ids for reuse — cannot leave
/// a stale entry that suppresses the next scene's `Ready`.
#[derive(Resource, Default)]
pub struct ReadiedScripts(HashSet<(Entity, String)>);

/// Every Rust script attached to a live entity this frame, as
/// `(entity, file name)`.
fn attached(world: &mut World) -> Vec<(Entity, String)> {
    let mut q = world.query::<(Entity, &ScriptComponent)>();
    q.iter(world)
        .flat_map(|(entity, sc)| {
            sc.scripts
                .iter()
                .filter(|e| e.enabled)
                .filter_map(|e| e.script_path.as_ref())
                .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("rs"))
                .filter_map(|p| p.file_name()?.to_str().map(|n| (entity, n.to_string())))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Resolve the hook function for a script file, if it exported one.
fn hook_fn(world: &World, file: &str) -> Option<ScriptHookFn> {
    world.resource::<LoadedScripts>().hook(file)
}

/// Call one script's hook, catching a panic the way [`crate::dispatch`] does —
/// a broken script stops working rather than taking the editor with it.
fn call(world: &mut World, entity: Entity, f: ScriptHookFn, hook: &ScriptHook<'_>) {
    if world.get_entity(entity).is_err() {
        return;
    }
    let guard = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(world, entity, hook)));
    if guard.is_err() {
        error!("[rust-script] panicked in {} hook on {entity}", hook.name());
        renzora::core::console_log::console_error(
            "Script",
            format!("panicked in the {} hook on {entity}", hook.name()),
        );
    }
}

/// Fire `Ready` once per `(entity, script)`, then broadcast the frame's events.
pub fn dispatch_hooks(world: &mut World) {
    let attached = attached(world);
    if attached.is_empty() {
        return;
    }

    // ── Ready ───────────────────────────────────────────────────────────────
    let pending_ready: Vec<(Entity, String, ScriptHookFn)> = {
        let readied = world.resource::<ReadiedScripts>();
        attached
            .iter()
            .filter(|(e, name)| !readied.0.contains(&(*e, name.clone())))
            .filter_map(|(e, name)| hook_fn(world, name).map(|f| (*e, name.clone(), f)))
            .collect()
    };
    for (entity, name, f) in pending_ready {
        call(world, entity, f, &ScriptHook::Ready);
        world.resource_mut::<ReadiedScripts>().0.insert((entity, name));
    }

    // Prune readiness for entities that no longer exist, so the next scene's
    // scripts are not mistaken for ones that have already started.
    {
        let live: HashSet<Entity> = attached.iter().map(|(e, _)| *e).collect();
        let mut readied = world.resource_mut::<ReadiedScripts>();
        readied.0.retain(|(e, _)| live.contains(e));
    }

    // ── Draw ────────────────────────────────────────────────────────────────
    //
    // Per-entity, and only for entities that actually have a surface. The UI
    // renderer publishes those sizes; a script with no canvas would otherwise
    // paint into a buffer nothing drains. Same rule the Lua path applies.
    let surfaces: Vec<(Entity, Vec2)> = world
        .get_resource::<renzora::core::ScriptDrawSurfaces>()
        .map(|s| s.per_entity.iter().map(|(e, v)| (*e, *v)).collect())
        .unwrap_or_default();
    for (entity, size) in surfaces {
        // Only if this entity runs a Rust script with hooks — the surface map
        // is shared with the Lua path and holds its canvases too.
        let Some(f) = attached
            .iter()
            .find(|(e, _)| *e == entity)
            .and_then(|(_, name)| hook_fn(world, name))
        else {
            continue;
        };
        call(
            world,
            entity,
            f,
            &ScriptHook::Draw {
                width: size.x,
                height: size.y,
            },
        );
    }

    // ── Broadcast events ────────────────────────────────────────────────────
    //
    // Copied out before any script runs: a script holds `&mut World` and may
    // despawn or replace the resources these came from.
    let scene_events: Vec<renzora::SceneEvent> = world
        .get_resource::<renzora::ScriptSceneInbox>()
        .map(|i| i.pending.clone())
        .unwrap_or_default();
    let ui_events: Vec<renzora::UiCallback> = world
        .get_resource::<renzora::ScriptUiInbox>()
        .map(|i| i.pending.clone())
        .unwrap_or_default();
    let rpc_events: Vec<renzora::IncomingRpc> = world
        .get_resource::<renzora::ScriptRpcInbox>()
        .map(|i| i.pending.clone())
        .unwrap_or_default();
    let anim_events: Vec<renzora::AnimEvent> = world
        .get_resource::<renzora::ScriptAnimEventInbox>()
        .map(|i| i.pending.clone())
        .unwrap_or_default();
    let player_events: Vec<renzora::NetPlayerEvent> = world
        .get_resource::<renzora::ScriptNetLifecycleInbox>()
        .map(|i| i.pending.clone())
        .unwrap_or_default();

    if scene_events.is_empty()
        && ui_events.is_empty()
        && rpc_events.is_empty()
        && anim_events.is_empty()
        && player_events.is_empty()
    {
        return;
    }

    // Resolved once: the same list serves every event this frame.
    let targets: Vec<(Entity, ScriptHookFn)> = attached
        .iter()
        .filter_map(|(e, name)| hook_fn(world, name).map(|f| (*e, f)))
        .collect();
    if targets.is_empty() {
        return;
    }

    for ev in &scene_events {
        let hook = ScriptHook::SceneLoaded {
            path: &ev.path,
            error: ev.error.as_deref(),
        };
        for (entity, f) in &targets {
            call(world, *entity, *f, &hook);
        }
    }
    for ev in &ui_events {
        let hook = ScriptHook::Ui {
            name: &ev.name,
            args: &ev.args,
            source: Entity::from_bits(ev.entity_bits),
        };
        for (entity, f) in &targets {
            call(world, *entity, *f, &hook);
        }
    }
    for ev in &rpc_events {
        let hook = ScriptHook::Rpc {
            name: &ev.name,
            args: &ev.args,
            from: ev.from,
        };
        for (entity, f) in &targets {
            call(world, *entity, *f, &hook);
        }
    }
    for ev in &anim_events {
        let hook = ScriptHook::AnimationEvent {
            name: &ev.name,
            source: Entity::from_bits(ev.entity_bits),
        };
        for (entity, f) in &targets {
            call(world, *entity, *f, &hook);
        }
    }
    for ev in &player_events {
        let hook = if ev.joined {
            ScriptHook::PlayerJoined { id: ev.id }
        } else {
            ScriptHook::PlayerLeft { id: ev.id }
        };
        for (entity, f) in &targets {
            call(world, *entity, *f, &hook);
        }
    }
}
