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
use bevy_gauge::prelude::InstantExt;
use renzora_scripting::systems::execution::ScriptCommandQueue;
use renzora_scripting::{ScriptCommand, ScriptExtensions, ScriptingSet};

mod script_extension;
pub use script_extension::{GaugeCommand, GaugeScriptExtension};

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
        app.add_systems(
            Update,
            ensure_attributes.in_set(ScriptingSet::PreScript),
        );
        app.add_systems(PostUpdate, update_gauges_snapshot);

        // Process gauge commands after script command processing
        app.add_systems(
            Update,
            process_gauge_commands.in_set(ScriptingSet::CommandProcessing),
        );

        // Register the gauge script extension
        app.world_mut()
            .resource_mut::<ScriptExtensions>()
            .register(GaugeScriptExtension);

        #[cfg(feature = "editor")]
        {
            use renzora_editor::AppEditorExt;
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
        commands.entity(entity).insert(Attributes::new());
    }
}

// ── Command processing ───────────────────────────────────────────────────

/// Process gauge-related script commands (Extension variant with GaugeCommand).
fn process_gauge_commands(
    cmd_queue: Res<ScriptCommandQueue>,
    mut attrs: AttributesMut,
) {
    for (source_entity, cmd) in &cmd_queue.commands {
        let ScriptCommand::Extension(ext_cmd) = cmd else { continue };
        let Some(gauge_cmd) = ext_cmd.as_any().downcast_ref::<GaugeCommand>() else { continue };

        let resolve = |target: &Option<u64>| {
            target.map(Entity::from_bits).unwrap_or(*source_entity)
        };

        match gauge_cmd {
            GaugeCommand::Set { attribute, value, target } => {
                attrs.set(resolve(target), attribute, *value);
            }
            GaugeCommand::AddModifier { attribute, value, target } => {
                attrs.add_modifier(resolve(target), attribute, Modifier::Flat(*value));
            }
            GaugeCommand::RemoveModifier { attribute, value, target } => {
                attrs.remove_modifier(resolve(target), attribute, &Modifier::Flat(*value));
            }
            GaugeCommand::AddExprModifier { attribute, expression, target } => {
                if let Err(e) = attrs.add_expr_modifier(resolve(target), attribute, expression) {
                    warn!("Gauge expr modifier error on '{}': {:?}", attribute, e);
                }
            }
            GaugeCommand::Instant { attribute, op, value, target } => {
                let mut instant = InstantModifierSet::new();
                match op.as_str() {
                    "add" => instant.push_add(attribute, *value),
                    "subtract" | "sub" => instant.push_sub(attribute, *value),
                    "set" => instant.push_set(attribute, *value),
                    _ => {
                        warn!("Unknown gauge instant op '{}', use add/subtract/set", op);
                    }
                }
                attrs.apply_instant(&instant, &[], resolve(target));
            }
            GaugeCommand::InstantExpr { attribute, op, expression, roles, target } => {
                let mut instant = InstantModifierSet::new();
                match op.as_str() {
                    "add" => instant.push_add(attribute.as_str(), expression.as_str()),
                    "subtract" | "sub" => instant.push_sub(attribute.as_str(), expression.as_str()),
                    "set" => instant.push_set(attribute.as_str(), expression.as_str()),
                    _ => {
                        warn!("Unknown gauge instant op '{}', use add/subtract/set", op);
                    }
                }
                let role_pairs: Vec<(&str, Entity)> = roles
                    .iter()
                    .map(|(name, id)| (name.as_str(), Entity::from_bits(*id)))
                    .collect();
                attrs.apply_instant(&instant, &role_pairs, resolve(target));
            }
        }
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
