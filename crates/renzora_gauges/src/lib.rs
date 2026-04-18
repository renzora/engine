//! Renzora Gauges — attribute/stat system powered by `bevy_gauge`.
//!
//! Provides a `Gauges` marker component for the inspector, the `bevy_gauge`
//! plugin, and an optional editor panel for live attribute debugging.
//!
//! Script bindings are registered via the `ScriptExtension` trait, keeping
//! the scripting crate decoupled from gauge logic.

pub use bevy_gauge;
pub use bevy_gauge::prelude::*;

use bevy::prelude::*;

mod script_extension;

#[cfg(feature = "editor")]
mod inspector;
#[cfg(feature = "editor")]
mod panel;

// ── Marker component ─────────────────────────────────────────────────────

/// Marker component that indicates an entity uses the gauge/attribute system.
///
/// When added through the inspector, this inserts `Attributes::new()` onto
/// the entity so `bevy_gauge` systems can manage it. The actual attribute
/// data lives in `bevy_gauge::Attributes`.
#[derive(Component, Default)]
pub struct Gauges;

// ── Plugin ────────────────────────────────────────────────────────────────

/// Renzora gauges plugin — adds `bevy_gauge::AttributesPlugin` and editor
/// integrations when the `editor` feature is enabled.
pub struct GaugesPlugin;

impl Plugin for GaugesPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(AttributesPlugin);

        app.init_resource::<GaugesSnapshot>();
        // ensure_attributes must run before script execution so gauge commands
        // issued in on_ready (the very first frame) find an Attributes component.
        app.add_systems(Update, ensure_attributes);
        app.add_systems(PostUpdate, update_gauges_snapshot);

        // Script actions (decoupled — observes ScriptAction events)
        app.add_observer(script_extension::handle_gauge_script_actions);

        #[cfg(feature = "editor")]
        {
            use renzora_editor_framework::AppEditorExt;
            app.register_inspector(inspector::gauges_inspector_entry());
            app.register_panel(panel::GaugesPanel::default());
        }
    }
}

/// Ensure every entity with `Gauges` also has an `Attributes` component.
fn ensure_attributes(
    mut commands: Commands,
    query: Query<Entity, (With<Gauges>, Without<Attributes>)>,
) {
    for entity in &query {
        commands.entity(entity).try_insert(Attributes::new());
    }
}


// ── Snapshot resource for editor panels ────────────────────────────────────

/// Snapshot of all gauge entities and their attribute values, updated each frame.
/// Panels read this instead of querying `&World` directly.
#[derive(Resource, Default, Clone)]
pub struct GaugesSnapshot {
    pub entries: Vec<GaugeEntitySnapshot>,
}

/// Snapshot of a single entity's gauge state.
#[derive(Clone)]
pub struct GaugeEntitySnapshot {
    pub entity: Entity,
    pub name: Option<String>,
    pub attributes: Vec<(String, f32)>,
}

/// System that collects gauge data into `GaugesSnapshot` each frame.
fn update_gauges_snapshot(
    query: Query<(Entity, &Attributes, Option<&Name>), With<Gauges>>,
    mut snapshot: ResMut<GaugesSnapshot>,
) {
    snapshot.entries.clear();
    for (entity, attrs, name) in &query {
        let attributes: Vec<(String, f32)> = attrs
            .iter()
            .map(|(id, val)| (bevy_gauge::attribute_id::Interner::global().resolve(id).to_string(), val))
            .collect();
        snapshot.entries.push(GaugeEntitySnapshot {
            entity,
            name: name.map(|n| n.as_str().to_string()),
            attributes,
        });
    }
    snapshot.entries.sort_by_key(|e| e.entity);
}
