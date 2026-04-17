use bevy::prelude::*;

use crate::value::BoundValue;

/// The *read* side of a binding: how a widget pulls its current value
/// out of the `World`.
///
/// All variants hand the world out via a raw function pointer so bindings
/// stay `Clone + Send + Sync` and can live on UI entities (a closure would
/// require boxing per-entity). The function signatures match the panel
/// inspector's existing `get_fn` style — you can lift an inspector field
/// getter straight into a binding source.
#[derive(Clone)]
pub enum BindSource {
    /// A value read directly off a specific entity. The getter picks one
    /// field out of one component; different fields share the same source
    /// variant because the reactive layer doesn't need to know which
    /// component is behind the getter — only the entity + the lambda.
    EntityField {
        entity: Entity,
        getter: fn(&World, Entity) -> BoundValue,
    },

    /// A value read from a resource (editor settings, theme, etc).
    ResourceField {
        getter: fn(&World) -> BoundValue,
    },

    /// A value tied to whatever entity is the primary editor selection.
    /// The reactive layer resolves "the current selection" at read time,
    /// so widgets survive selection changes without re-binding.
    SelectedField {
        getter: fn(&World, Entity) -> BoundValue,
    },

    /// A derived / computed value (e.g. FPS, memory stats, "number of
    /// selected entities"). No backing storage; always recomputed.
    Computed {
        getter: fn(&World) -> BoundValue,
    },
}

impl BindSource {
    /// Read the current value. Returns `BoundValue::Unit` when the source
    /// is temporarily unresolvable (e.g. selection is empty for a
    /// `SelectedField`). Widgets treat `Unit` as "nothing to display" —
    /// this keeps hot-swapping selection cheap.
    pub fn read(&self, world: &World) -> BoundValue {
        match self {
            BindSource::EntityField { entity, getter } => {
                if world.get_entity(*entity).is_err() {
                    return BoundValue::Unit;
                }
                getter(world, *entity)
            }
            BindSource::ResourceField { getter } => getter(world),
            BindSource::SelectedField { getter } => {
                let entity = current_selection(world);
                match entity {
                    Some(e) if world.get_entity(e).is_ok() => getter(world, e),
                    _ => BoundValue::Unit,
                }
            }
            BindSource::Computed { getter } => getter(world),
        }
    }
}

/// The *write* side of a binding. Optional — omit for read-only bindings
/// (labels, derived stats). Sinks run inside an exclusive system so they
/// can take `&mut World`.
#[derive(Clone)]
pub enum BindSink {
    EntityField {
        entity: Entity,
        setter: fn(&mut World, Entity, BoundValue),
    },
    ResourceField {
        setter: fn(&mut World, BoundValue),
    },
    SelectedField {
        setter: fn(&mut World, Entity, BoundValue),
    },
}

impl BindSink {
    /// Apply a new value. No-ops gracefully when the target entity / the
    /// current selection has disappeared — a commit event that arrives
    /// one frame after a delete shouldn't panic the editor.
    pub fn apply(&self, world: &mut World, value: BoundValue) {
        match self {
            BindSink::EntityField { entity, setter } => {
                if world.get_entity(*entity).is_ok() {
                    setter(world, *entity, value);
                }
            }
            BindSink::ResourceField { setter } => setter(world, value),
            BindSink::SelectedField { setter } => {
                if let Some(e) = current_selection(world) {
                    if world.get_entity(e).is_ok() {
                        setter(world, e, value);
                    }
                }
            }
        }
    }
}

/// Resolve the current primary selection. Returns `None` if the
/// `SelectionProvider` resource isn't installed (e.g. in tests) or the
/// selection is empty.
///
/// The reactive layer deliberately does *not* depend on
/// `renzora_editor_framework` — we'd get a cycle. Instead, callers
/// install a small `SelectionProvider` resource that vends the current
/// selection. `renzora_editor_framework` installs it at startup.
fn current_selection(world: &World) -> Option<Entity> {
    world
        .get_resource::<SelectionProvider>()
        .and_then(|p| (p.get)(world))
}

/// Adapter resource letting the reactive layer resolve "the current
/// selection" without depending on the selection type itself. The
/// editor framework installs one of these that reads its
/// `EditorSelection` resource.
#[derive(Resource, Clone)]
pub struct SelectionProvider {
    pub get: fn(&World) -> Option<Entity>,
}
