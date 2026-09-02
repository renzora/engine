//! Interactive widget behaviors — the kernel the markup widget catalog builds
//! on. Each is a markup attribute the loader stamps as a component:
//!
//! - `toggle="Entity.Component.field"` — click flips a bool (checkbox/switch).
//! - `drag_value="Entity.Component.field" drag_min=".." drag_max=".."` — drag
//!   horizontally to set a number (slider/scrollbar).
//! - `toggles="name"` — click shows/hides the entity with that `Name`
//!   (dropdown / accordion / modal / tooltip / tabs).
//!
//! Writes route through the **scripting** layer's tested paths: component
//! fields go to `ScriptReflectionQueue` (applied by `apply_reflection_sets`),
//! script vars are written on the entity's `ScriptComponent`. Reads reuse the
//! binding resolver.

use bevy::prelude::*;
use renzora::PropertyValue;
use renzora_scripting::systems::execution::ReflectionSet;
use renzora_scripting::systems::ScriptReflectionQueue;
use renzora_scripting::{ScriptComponent, ScriptValue};

use crate::markup::binding::read_path;

// ── Components (stamped by the loader from markup attributes) ────────────────

/// `toggle="Path.bool"` — clicking flips the bound boolean.
#[derive(Component)]
pub struct Toggle {
    pub target: String,
    pub host: Entity,
}

/// `drag_value="Path.number" drag_min drag_max` — drag the node horizontally
/// to set the bound number within [min, max].
#[derive(Component)]
pub struct DragValue {
    pub target: String,
    pub min: f32,
    pub max: f32,
    pub host: Entity,
}

/// `toggles="name"` — clicking shows/hides the entity named `name`.
#[derive(Component)]
pub struct Disclose {
    pub target: String,
}

/// `fill="Path.number" fill_min fill_max` — the node's extent tracks the bound
/// value's fraction of [min, max]. The visual half of a slider / a bound
/// progress bar.
///
/// # Two ways to draw a fraction
///
/// A node drawn as a flat colour is filled by **resizing** it. A node drawn from
/// a texture has to be filled by **cropping** it instead: shrinking an
/// `ImageNode` squashes the whole picture into less space rather than showing
/// less of it, so a bar with bevelled ends and segment ticks compresses those
/// details as it drains.
///
/// So the system picks by what is on the entity — an `ImageNode` crops, anything
/// else resizes. That is not a guess about intent: squashing a bar texture is
/// never what an author wants, and every template that used `fill=` before this
/// existed was filling a coloured node and is unaffected.
///
/// `fill_dir` chooses the edge the fill grows from (`left_to_right` — the
/// default — `right_to_left`, `bottom_to_top`, `top_to_bottom`), and
/// `fill_extent` sets the on-screen size at full, in px. Both apply to either
/// drawing mode.
#[derive(Component)]
pub struct ValueFill {
    pub target: String,
    pub min: f32,
    pub max: f32,
    pub host: Entity,
    /// The edge the fill grows from.
    pub direction: crate::game_ui::components::ProgressDirection,
    /// On-screen extent along the fill axis at full, in px. `0.0` means the
    /// texture's own size for an image, or `100%` of the parent for a
    /// coloured node — each mode's natural "as authored".
    pub extent_px: f32,
}

// ── Shared write path ───────────────────────────────────────────────────────

/// Decide whether `name` is a registered component type.
fn is_component(world: &World, name: &str) -> bool {
    world
        .resource::<AppTypeRegistry>()
        .read()
        .get_with_short_type_path(name)
        .and_then(|r| r.data::<bevy::ecs::reflect::ReflectComponent>())
        .is_some()
}

fn entity_by_name(world: &World, name: &str) -> Option<Entity> {
    world
        .resource::<crate::markup::binding::MarkupNameIndex>()
        .map
        .get(name)
        .copied()
}

/// Write `value` to a target path. Component fields (`Entity.Component.field`)
/// are enqueued for scripting's reflection writer; script vars (`Entity.var`
/// or bare `var`) are written directly on the entity's `ScriptComponent`.
fn write_target(world: &mut World, host: Entity, path: &str, value: PropertyValue) {
    let segs: Vec<&str> = path.split('.').filter(|s| !s.is_empty()).collect();
    if segs.is_empty() {
        return;
    }

    // Bare `var` → script var on host.
    if segs.len() == 1 {
        write_script_var(world, host, segs[0], value);
        return;
    }

    // `Component.field...` on host (first segment is a component type).
    if is_component(world, segs[0]) {
        enqueue_set(world, host, None, segs[0], &segs[1..].join("."), value);
        return;
    }

    // `Entity.<rest>` — component on a named entity, else a script var.
    let entity_name = segs[0];
    if segs.len() >= 3 && is_component(world, segs[1]) {
        enqueue_set(
            world,
            host,
            Some(entity_name.to_string()),
            segs[1],
            &segs[2..].join("."),
            value,
        );
    } else if let Some(ent) = entity_by_name(world, entity_name) {
        write_script_var(world, ent, segs[1], value);
    }
}

fn enqueue_set(
    world: &mut World,
    source: Entity,
    entity_name: Option<String>,
    component: &str,
    field_path: &str,
    value: PropertyValue,
) {
    if let Some(mut queue) = world.get_resource_mut::<ScriptReflectionQueue>() {
        queue.sets.push(ReflectionSet {
            source_entity: source,
            entity_id: None,
            entity_name,
            component_type: component.to_string(),
            field_path: field_path.to_string(),
            value,
        });
    }
}

fn write_script_var(world: &mut World, entity: Entity, var: &str, value: PropertyValue) {
    let sv = match value {
        PropertyValue::Float(f) => ScriptValue::Float(f),
        PropertyValue::Int(i) => ScriptValue::Int(i as i32),
        PropertyValue::Bool(b) => ScriptValue::Bool(b),
        PropertyValue::String(s) => ScriptValue::String(s),
        PropertyValue::Vec3(v) => ScriptValue::Vec3(Vec3::from(v)),
        PropertyValue::Color(c) => ScriptValue::Color(Vec4::from(c)),
    };
    if let Some(mut sc) = world.get_mut::<ScriptComponent>(entity) {
        for entry in &mut sc.scripts {
            if entry.variables.get(var).is_some() {
                entry.variables.set(var.to_string(), sv);
                return;
            }
        }
        if let Some(entry) = sc.scripts.first_mut() {
            entry.variables.set(var.to_string(), sv);
        }
    }
}

fn truthy(s: &str) -> bool {
    let t = s.trim();
    !(t.is_empty() || t.eq_ignore_ascii_case("false") || t == "0")
}

// ── Systems ─────────────────────────────────────────────────────────────────

/// Toggle: on click, read the current bool and write the inverse.
fn toggle_system(world: &mut World) {
    let mut pressed: Vec<(Entity, String, Entity)> = Vec::new();
    {
        let mut q =
            world.query_filtered::<(Entity, &Interaction, &Toggle), Changed<Interaction>>();
        for (e, interaction, t) in q.iter(world) {
            if *interaction == Interaction::Pressed {
                pressed.push((e, t.target.clone(), t.host));
            }
        }
    }
    for (_e, target, host) in pressed {
        let current = read_path(world, host, &target)
            .map(|s| truthy(&s))
            .unwrap_or(false);
        write_target(world, host, &target, PropertyValue::Bool(!current));
    }
}

/// Drag-value: while the node is pressed, set the bound number from the
/// cursor's normalized X position within the node (Bevy's
/// `RelativeCursorPosition` — no manual rect/coordinate-space math).
fn drag_value_system(world: &mut World) {
    let mut active: Vec<(String, f32, f32, Entity, f32)> = Vec::new();
    {
        let mut q = world.query::<(
            &Interaction,
            &DragValue,
            &bevy::ui::RelativeCursorPosition,
        )>();
        for (interaction, dv, rel) in q.iter(world) {
            if *interaction == Interaction::Pressed {
                if let Some(n) = rel.normalized {
                    active.push((dv.target.clone(), dv.min, dv.max, dv.host, n.x.clamp(0.0, 1.0)));
                }
            }
        }
    }
    for (target, min, max, host, frac) in active {
        write_target(world, host, &target, PropertyValue::Float(min + frac * (max - min)));
    }
}

/// Value-driven fill: each frame, set the node's extent to the bound value's
/// fraction of [min, max]. Used for slider fills / progress visuals.
///
/// A node carrying an `ImageNode` is *cropped* to the fraction rather than
/// resized — see [`ValueFill`] for why the two cases have to differ.
fn value_fill_system(world: &mut World) {
    use crate::game_ui::components::ProgressDirection;
    use crate::game_ui::systems::image_fill;

    let mut items: Vec<(Entity, String, f32, f32, Entity, ProgressDirection, f32)> = Vec::new();
    {
        let mut q = world.query::<(Entity, &ValueFill)>();
        for (e, vf) in q.iter(world) {
            items.push((
                e,
                vf.target.clone(),
                vf.min,
                vf.max,
                vf.host,
                vf.direction,
                vf.extent_px,
            ));
        }
    }
    for (entity, target, min, max, host, direction, extent_px) in items {
        let Some(s) = read_path(world, host, &target) else {
            continue;
        };
        let Ok(val) = s.trim().parse::<f32>() else {
            continue;
        };
        let span = max - min;
        let frac = if span.abs() > f32::EPSILON {
            ((val - min) / span).clamp(0.0, 1.0)
        } else {
            0.0
        };

        // Textured fill — crop to the fraction rather than squashing it. Needs
        // the source dimensions, so it waits for the texture to land; until
        // then the node is left as authored rather than sized against a guess.
        let textured = world
            .get::<bevy::ui::widget::ImageNode>(entity)
            .map(|i| i.image.clone());
        if let Some(handle) = textured {
            let Some(tex) = world
                .resource::<Assets<Image>>()
                .get(&handle)
                .map(|i| i.size().as_vec2())
                .filter(|t| t.x > 0.0 && t.y > 0.0)
            else {
                continue;
            };
            let (rect, extent) = image_fill::fill_geometry(direction, frac, tex, extent_px);
            // One component at a time — `&mut World` won't hand out two.
            if let Some(mut image_node) = world.get_mut::<bevy::ui::widget::ImageNode>(entity) {
                if image_node.rect != Some(rect) {
                    image_node.rect = Some(rect);
                }
            }
            if let Some(mut node) = world.get_mut::<Node>(entity) {
                if image_fill::is_horizontal(direction) {
                    if node.width != extent {
                        node.width = extent;
                    }
                } else if node.height != extent {
                    node.height = extent;
                }
            }
            continue;
        }

        // Flat-coloured fill — resize. `extent_px == 0` keeps the historical
        // behaviour of filling the parent by percentage.
        if let Some(mut node) = world.get_mut::<Node>(entity) {
            let extent = if extent_px > 0.0 {
                Val::Px(extent_px * frac)
            } else {
                Val::Percent(frac * 100.0)
            };
            if image_fill::is_horizontal(direction) {
                node.width = extent;
            } else {
                node.height = extent;
            }
        }
    }
}

/// Disclosure: on click, flip the `Display` of the entity named `target`.
fn disclosure_system(
    interactions: Query<(&Interaction, &Disclose), Changed<Interaction>>,
    names: Query<(Entity, &Name)>,
    mut nodes: Query<&mut Node>,
) {
    for (interaction, disclose) in &interactions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some((target, _)) = names.iter().find(|(_, n)| n.as_str() == disclose.target) {
            if let Ok(mut node) = nodes.get_mut(target) {
                node.display = if node.display == Display::None {
                    Display::Flex
                } else {
                    Display::None
                };
            }
        }
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            toggle_system,
            drag_value_system,
            value_fill_system,
            disclosure_system,
        ),
    );
}
