//! What a field's reset button resets it to.
//!
//! A field's value is all the inspector holds: a `FieldDef` is a getter and a
//! setter, and says nothing of where its default lives. The reset used to write
//! the value type's zero (`FieldValue::type_default`), which put a camera's FOV,
//! a light's intensity or a bloom threshold at 0 rather than back where they
//! started.
//!
//! The default is recovered from the component instead, in two ways:
//!
//! 1. **Reflection.** For each component on the entity that registers
//!    `ReflectDefault`, the section's own first, swap its default in, read the
//!    field, and put the original back. The first component whose default
//!    changes the reading is the one the field reads. This covers Bevy's
//!    components and fields that read a component other than their section's
//!    (the Camera section's Anti-Aliasing row reads `AntiAliasing`).
//! 2. **Add Component.** Otherwise the section's own `add_fn`, which is what Add
//!    Component inserts, builds the component on a scratch world, and the field
//!    is read there. This covers components that derive `Default` without
//!    registering it for reflection, which is most of Renzora's settings.
//!
//! When neither finds one, the reset does nothing: a field left as it is reads
//! as a reset that did not take, where a zero breaks the scene.

use std::any::TypeId;

use bevy::ecs::component::ComponentId;
use bevy::prelude::*;
use bevy::reflect::std_traits::ReflectDefault;
use renzora_editor_framework::{FieldValue, InspectorRegistry};

use super::GetFn;

/// The value `get_fn` reads on `entity` once its component is back at its
/// default, or `None` if no default could be found. See the module docs.
pub(crate) fn default_value(
    world: &mut World,
    entity: Entity,
    get_fn: &GetFn,
    type_id: &'static str,
    section: Option<ComponentId>,
) -> Option<FieldValue> {
    let current = get_fn(world, entity)?;
    by_reflection(world, entity, get_fn, &current, section).or_else(|| by_add_fn(world, get_fn, type_id))
}

fn by_reflection(
    world: &mut World,
    entity: Entity,
    get_fn: &GetFn,
    current: &FieldValue,
    section: Option<ComponentId>,
) -> Option<FieldValue> {
    let registry = world.get_resource::<AppTypeRegistry>()?.clone();
    let mut candidates: Vec<(ComponentId, TypeId)> = world
        .inspect_entity(entity)
        .ok()?
        .filter_map(|(id, info)| Some((id, info.type_id()?)))
        .collect();
    // The section's own component first: it is the usual owner, and trying it
    // first keeps a field that two components both affect on the right one.
    candidates.sort_by_key(|(cid, _)| Some(*cid) != section);

    for (_, type_id) in candidates {
        let (component, default) = {
            let registry = registry.read();
            let Some(reg) = registry.get(type_id) else { continue };
            let (Some(component), Some(default)) =
                (reg.data::<ReflectComponent>(), reg.data::<ReflectDefault>())
            else {
                continue;
            };
            (component.clone(), default.default())
        };
        // Without a copy to put back, the component cannot be swapped safely.
        // Fails for a component with a `reflect(ignore)` field.
        let Some(saved) = component
            .reflect(world.entity(entity))
            .and_then(|r| r.reflect_clone().ok())
        else {
            continue;
        };
        // Both swaps bypass change detection. The component ends exactly as it
        // started, and a change tick here would have every router that watches
        // it rebuild its effect (the sky, an IBL bake) for nothing.
        if !swap(world, entity, &component, default) {
            continue;
        }
        let read = get_fn(world, entity);
        swap(world, entity, &component, saved);
        if let Some(value) = read {
            if value != *current {
                return Some(value);
            }
        }
    }
    None
}

/// Replace the component's value in place, without marking it changed.
fn swap(world: &mut World, entity: Entity, component: &ReflectComponent, value: Box<dyn Reflect>) -> bool {
    let Ok(mut e) = world.get_entity_mut(entity) else { return false };
    let Some(mut target) = component.reflect_mut(&mut e) else { return false };
    target.bypass_change_detection().set(value).is_ok()
}

fn by_add_fn(world: &World, get_fn: &GetFn, type_id: &'static str) -> Option<FieldValue> {
    let add_fn = world
        .get_resource::<InspectorRegistry>()?
        .iter()
        .find(|e| e.type_id == type_id)?
        .add_fn?;
    // A scratch world, so the component's insert hooks and observers never run
    // on the real entity. A hook may expect a resource the scratch world lacks
    // and panic; that only means this way finds no default.
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut scratch = World::new();
        let e = scratch.spawn_empty().id();
        add_fn(&mut scratch, e);
        get_fn(&scratch, e)
    }))
    .ok()
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[derive(Component, Reflect, Clone)]
    #[reflect(Component, Default)]
    struct Lens {
        fov: f32,
        name: String,
    }

    impl Default for Lens {
        fn default() -> Self {
            Self { fov: 45.0, name: "lens".into() }
        }
    }

    fn fov() -> GetFn {
        Arc::new(|w: &World, e: Entity| w.get::<Lens>(e).map(|l| FieldValue::Float(l.fov)))
    }

    fn world() -> World {
        let mut world = World::new();
        let registry = AppTypeRegistry::default();
        registry.write().register::<Lens>();
        world.insert_resource(registry);
        world
    }

    #[test]
    fn resets_to_the_components_default_not_zero() {
        let mut world = world();
        let e = world.spawn(Lens { fov: 90.0, name: "custom".into() }).id();
        let got = default_value(&mut world, e, &fov(), "lens", None);
        assert_eq!(got, Some(FieldValue::Float(45.0)));
    }

    #[test]
    fn the_component_is_left_as_it_was_and_unchanged() {
        let mut world = world();
        let e = world.spawn(Lens { fov: 90.0, name: "custom".into() }).id();
        world.clear_trackers();
        let before = world.change_tick();
        default_value(&mut world, e, &fov(), "lens", None);
        let lens = world.entity(e).get_ref::<Lens>().unwrap();
        assert_eq!((lens.fov, lens.name.as_str()), (90.0, "custom"));
        assert!(!lens.last_changed().is_newer_than(before, world.change_tick()));
    }

    /// Like most of Renzora's settings: `Default` derived, but not registered
    /// for reflection, so only the section's Add Component knows it.
    #[derive(Component, Reflect, Clone)]
    #[reflect(Component)]
    struct Glow {
        intensity: f32,
    }

    #[test]
    fn falls_back_to_what_add_component_inserts() {
        let mut world = World::new();
        let registry = AppTypeRegistry::default();
        registry.write().register::<Glow>();
        world.insert_resource(registry);
        let mut inspectors = InspectorRegistry::default();
        inspectors.register(renzora_editor_framework::InspectorEntry {
            type_id: "glow",
            display_name: "Glow",
            icon: "",
            category: "",
            has_fn: |w, e| w.get::<Glow>(e).is_some(),
            add_fn: Some(|w, e| {
                w.entity_mut(e).insert(Glow { intensity: 0.15 });
            }),
            remove_fn: None,
            is_enabled_fn: None,
            set_enabled_fn: None,
            fields: Vec::new(),
        });
        world.insert_resource(inspectors);
        let e = world.spawn(Glow { intensity: 2.0 }).id();
        let get: GetFn = Arc::new(|w: &World, e: Entity| w.get::<Glow>(e).map(|g| FieldValue::Float(g.intensity)));
        assert_eq!(default_value(&mut world, e, &get, "glow", None), Some(FieldValue::Float(0.15)));
    }

    #[test]
    fn no_default_anywhere_means_no_reset() {
        let mut world = World::new();
        world.insert_resource(AppTypeRegistry::default());
        let e = world.spawn(Lens { fov: 90.0, name: String::new() }).id();
        assert_eq!(default_value(&mut world, e, &fov(), "lens", None), None);
    }
}
