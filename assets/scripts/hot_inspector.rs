//! A reflection "hot inspector": once a second, dump every reflected component
//! on this entity, and watch chosen fields for changes every frame. The live
//! reflected state is enough for a script to act as a debugger overlay without
//! engine support. Watched fields are diffed frame to frame and only logged on
//! change, because a log line every frame stalls frames.
//!
//! Read-only, so it is safe anywhere, networked or not.
//!
//! Setup: attach to any entity you want to introspect. `WATCH` lists the
//! `(component, field)` pairs to follow.

use bevy::prelude::*;
use renzora::core::reflection::{get_all_component_fields, get_entity_component_names, get_reflected_field};
use renzora::script_bridge::PropertyValue;
use renzora::{ScriptCtx, ScriptHook};

const WATCH: [(&str, &str); 2] = [("Transform", "translation.y"), ("Transform", "scale.x")];
/// Seconds between full dumps.
const INSPECT_EVERY: f32 = 1.0;

#[derive(Component)]
struct Inspector {
    timer: f32,
    last: Vec<Option<PropertyValue>>,
}

fn update(ctx: &mut ScriptCtx) {
    let dt = ctx.delta();
    let me = ctx.entity();
    let current = watched(ctx.world(), me);
    let Some(mut inspector) = ctx.get_mut::<Inspector>() else { return };
    for (i, value) in current.into_iter().enumerate() {
        if inspector.last[i] != value {
            let (comp, field) = WATCH[i];
            renzora::clog_info!(
                "Script",
                "[watch] {}.{}: {} -> {}",
                comp,
                field,
                show(&inspector.last[i]),
                show(&value)
            );
            inspector.last[i] = value;
        }
    }
    inspector.timer -= dt;
    let due = inspector.timer <= 0.0;
    if due {
        inspector.timer += INSPECT_EVERY;
    }
    drop(inspector);
    if due {
        snapshot(ctx);
    }
}

fn hooks(ctx: &mut ScriptCtx, hook: &ScriptHook) {
    if !matches!(hook, ScriptHook::Ready) {
        return;
    }
    let me = ctx.entity();
    let last = watched(ctx.world(), me);
    ctx.insert(Inspector { timer: INSPECT_EVERY, last });
}

fn watched(world: &World, entity: Entity) -> Vec<Option<PropertyValue>> {
    WATCH.iter().map(|(comp, field)| get_reflected_field(world, entity, comp, field)).collect()
}

fn snapshot(ctx: &mut ScriptCtx) {
    let me = ctx.entity();
    let label = ctx.name().unwrap_or("?").to_string();
    let world = ctx.world();
    let names = get_entity_component_names(world, me);
    renzora::clog_info!("Script", "[inspect] {} has {} components", label, names.len());
    for name in names {
        let Some(fields) = get_all_component_fields(world, me, &name) else { continue };
        // Top-level keys only, to keep the dump readable.
        let mut keys: Vec<&str> = fields.keys().map(|k| k.split('.').next().unwrap_or(k)).collect();
        keys.sort_unstable();
        keys.dedup();
        renzora::clog_info!("Script", "  - {} {{ {} }}", name, keys.join(", "));
    }
}

fn show(value: &Option<PropertyValue>) -> String {
    match value {
        None => "nil".to_string(),
        Some(PropertyValue::Float(f)) => format!("{f}"),
        Some(PropertyValue::Int(i)) => format!("{i}"),
        Some(PropertyValue::Bool(b)) => format!("{b}"),
        Some(PropertyValue::String(s)) => s.clone(),
        Some(PropertyValue::Vec3(v)) => format!("{v:?}"),
        Some(PropertyValue::Color(c)) => format!("{c:?}"),
    }
}

renzora::script!(update, hooks = hooks);
