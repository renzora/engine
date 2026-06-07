//! Inspector registration for the Gauges component.
//!
//! The `Gauges` marker has no editable fields of its own — attributes are
//! defined by scripts/code and shown live in the native Gauges panel. The
//! inspector entry just exposes add/remove so the component can be attached
//! from the Add Component overlay.

use bevy::prelude::*;
use renzora::InspectorEntry;

use crate::{Attributes, Gauges};

/// Build the inspector entry for the `Gauges` component.
pub fn gauges_inspector_entry() -> InspectorEntry {
    InspectorEntry {
        type_id: "gauges",
        display_name: "Gauges",
        icon: "gauge",
        category: "gameplay",
        has_fn: |world, entity| world.get::<Gauges>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.commands().entity(entity).try_insert(Gauges);
        }),
        remove_fn: Some(|world, entity| {
            world.commands().entity(entity).remove::<Gauges>();
            world.commands().entity(entity).remove::<Attributes>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![],
    }
}
