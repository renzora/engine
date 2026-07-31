//! Teaches the scene format about types the C-ABI plugin system owns.
//!
//! `renzora_bsn` has no dependency on `renzora_plugin` and should not gain one —
//! the scene format has no business knowing the plugin system exists, and the
//! dependency would point the wrong way. So it exposes a plain-data
//! [`RawComponentRegistry`] and something upstream fills it in. This is that
//! something.
//!
//! The mirroring is one-way and cheap: `PluginComponentSchemas` only changes
//! when a plugin registers a type, which happens during load.

use bevy::prelude::*;
use renzora_bsn::{RawComponentRegistry, RawField, RawTypeInfo, RawTypeTable};
use renzora_plugin::host::PluginComponentSchemas;
use renzora_plugin::sys::FieldKind;
use std::sync::Arc;

/// Spell a field kind for the scene file.
///
/// A string rather than the ABI's `u32` so the format stays readable and does
/// not pin itself to one ABI revision. An unrecognised kind — from a plugin
/// built against a newer ABI — is written as `k<N>` so it round-trips: the
/// loader will not know how to migrate it, but it will know it is the same
/// unknown thing and leave the bytes alone rather than confusing it with a
/// kind it does understand.
fn kind_name(kind: FieldKind) -> String {
    match kind {
        FieldKind::F32 => "f32".into(),
        FieldKind::I32 => "i32".into(),
        FieldKind::Bool => "bool".into(),
        FieldKind::Vec3 => "vec3".into(),
        FieldKind::Quat => "quat".into(),
        FieldKind::Str => "str".into(),
        other => format!("k{}", other.0),
    }
}

/// Rebuild [`RawComponentRegistry`] from the plugin host's schemas.
///
/// Idempotent and cheap enough to call directly from a save path rather than
/// relying on ordering.
pub fn refresh_raw_component_registry(world: &mut World) {
    let Some(schemas) = world.get_resource::<PluginComponentSchemas>() else {
        return;
    };

    let mut table = RawTypeTable::default();
    for info in &schemas.0 {
        // The **live** layout size, not `info.size`. The plugin declares an
        // unpadded size and `register_component` pads it to alignment before
        // registering, so the two differ for anything whose alignment exceeds
        // its trailing field. Reading `info.size` bytes out of storage would
        // then short-read every instance of such a component.
        let size = world
            .components()
            .get_info(info.id)
            .map(|i| i.layout().size())
            .unwrap_or(info.size);

        table.by_component.insert(info.id, info.type_path.clone());

        // An ambiguous short name resolves to nothing rather than to a guess —
        // two plugins may each define a `Settings`, and loading one's bytes into
        // the other would be silent corruption of exactly the right length.
        let short = info
            .type_path
            .rsplit("::")
            .next()
            .unwrap_or(&info.type_path)
            .to_string();
        table
            .by_short
            .entry(short)
            .and_modify(|e| *e = None)
            .or_insert_with(|| Some(info.type_path.clone()));

        table.by_path.insert(
            info.type_path.clone(),
            RawTypeInfo {
                component_id: info.id,
                type_path: info.type_path.clone(),
                size,
                is_resource: info.is_resource,
                // No transient flag crosses the ABI yet, so everything persists.
                // That default is deliberate and the opposite of Bevy's: a Bevy
                // component opts *in* to serialization because it may hold
                // handles and pointers reflection cannot encode, whereas a
                // plugin component is plain data by host enforcement. An opt-in
                // authors forget would reproduce the exact bug this removes.
                transient: false,
                default_value: info.default_value.clone(),
                fields: info
                    .fields
                    .iter()
                    .map(|f| RawField {
                        name: f.name.clone(),
                        kind: kind_name(f.kind),
                        offset: f.offset,
                    })
                    .collect(),
            },
        );
    }

    world.insert_resource(RawComponentRegistry(Arc::new(table)));
}

pub struct PluginScenePlugin;

impl Plugin for PluginScenePlugin {
    fn build(&self, app: &mut App) {
        // The other half of the bridge: `renzora_plugin` cannot call
        // `renzora_bsn` directly (it publishes to crates.io and so cannot take a
        // path dependency), so it holds a function pointer and this installs it.
        app.insert_resource(renzora_plugin::host::BsnSpawner(
            |world, root, source| renzora_bsn::bsn_tree::spawn_source(world, root, source),
        ));
        app.init_resource::<RawComponentRegistry>().add_systems(
            Startup,
            // Startup rather than plugin-build: plugins are loaded by
            // `load_global_plugins` during app build, and the schemas do not
            // exist until that has run.
            |world: &mut World| refresh_raw_component_registry(world),
        );
    }
}
