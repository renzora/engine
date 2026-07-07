//! Bridge between the animation timeline editor and other editor panels.
//!
//! The inspector wants a per-property "add keyframe" affordance whenever the
//! timeline has a clip open — but it must not link `renzora_animation_editor`
//! (an editor-only crate). So the two talk through these contract types, which
//! live in the shared `renzora` dylib and therefore have ONE `TypeId` on both
//! sides (an rlib would duplicate them per crate and the resources wouldn't
//! unify).
//!
//! - The timeline editor PUBLISHES [`ActiveTimeline`] each frame: which entity
//!   the open clip animates, the playhead, and the `(component, field)` paths
//!   that already have a bound property track.
//! - The inspector POSTS [`KeyframeRequests`]; the timeline editor drains them,
//!   finds the matching track, and keys the entity's live value at the playhead.

use bevy::prelude::*;

/// Which entity, if any, the timeline editor currently has an open clip for.
/// Published every frame so panels that can't link the animation editor (the
/// inspector) can gate on it — the inspector shows its per-field keyframe buttons
/// only while the selected entity is the one being animated. Reset to its default
/// (`entity: None`) whenever no clip is open.
#[derive(Resource, Default)]
pub struct ActiveTimeline {
    /// The entity the open clip animates. `None` when no clip is loaded, i.e.
    /// the timeline isn't "active".
    pub entity: Option<Entity>,
}

impl ActiveTimeline {
    /// Whether a clip is open (the timeline is active).
    pub fn is_open(&self) -> bool {
        self.entity.is_some()
    }

    /// Whether `entity` is the entity the open clip animates — the gate for
    /// showing the inspector's per-field keyframe buttons. (A clip is only open,
    /// so `entity` only `Some`, while the timeline is active.)
    pub fn animates(&self, entity: Entity) -> bool {
        self.entity == Some(entity)
    }
}

/// Normalize a component/field identifier for matching: drop separators
/// (`_ . -` and spaces) and lowercase, so `directional_light` == `DirectionalLight`.
pub fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '_' | '.' | '-' | ' '))
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// One "add a keyframe for this property" request, posted by the inspector and
/// drained by the animation timeline editor.
pub struct KeyframeRequest {
    /// The entity whose live value should be captured.
    pub entity: Entity,
    /// Reflected component short-name (the inspector's guess; matched normalized).
    pub component: String,
    /// Reflection field path (the inspector's guess; matched normalized).
    pub field: String,
}

/// Queue of pending [`KeyframeRequest`]s. Both the inspector (writer) and the
/// timeline editor (drainer) `init_resource` it, so it exists whichever loads
/// first; when the timeline editor isn't present the queue simply never drains.
#[derive(Resource, Default)]
pub struct KeyframeRequests(Vec<KeyframeRequest>);

impl KeyframeRequests {
    /// Queue a keyframe-add for `(component, field)` on `entity`.
    pub fn push(&mut self, entity: Entity, component: impl Into<String>, field: impl Into<String>) {
        self.0.push(KeyframeRequest {
            entity,
            component: component.into(),
            field: field.into(),
        });
    }

    /// Whether the queue is empty (nothing to drain this frame).
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Take all pending requests, leaving the queue empty.
    pub fn drain(&mut self) -> Vec<KeyframeRequest> {
        std::mem::take(&mut self.0)
    }
}
