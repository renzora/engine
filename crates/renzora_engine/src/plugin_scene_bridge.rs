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

/// The widths `renzora_bsn` uses to write into live plugin component storage,
/// checked against the types they are meant to describe.
///
/// `renzora_bsn` has no `renzora_plugin` dependency on purpose, so it reconstructs
/// those widths from literals — `raw_registry::field_width` returns 256 for
/// `"str"`, and `bsn_tree` writes a string's length at a hardcoded offset of 252.
/// Both are correct today and derived from nothing, and they are used to write at
/// computed offsets into memory a plugin owns. If `Str256` were ever resized, a
/// scene load would put every later field at the wrong place, silently.
///
/// This module is the one place that imports both sides, so the equivalence is
/// asserted here rather than by adding a dependency edge that was deliberately
/// avoided. A failure here means a literal in `renzora_bsn` needs updating.
const _: () = {
    use renzora_plugin::sys;
    assert!(sys::STR_CAP == 252, "bsn_tree writes a string's length at a literal 252");
    assert!(size_of::<sys::Str256>() == 256, "raw_registry maps \"str\" to a literal 256");
    assert!(size_of::<sys::Vec3>() == 12, "raw_registry maps \"vec3\" to a literal 12");
    assert!(size_of::<sys::Quat>() == 16, "raw_registry maps \"quat\" to a literal 16");
    assert!(size_of::<f32>() == 4 && size_of::<i32>() == 4);
    // `field_width` also has an `"entity"` arm, which `kind_name` above can never
    // produce — `FieldKind` has no such value. Harmless, but it means the two
    // tables were written from different ideas of what the kinds are.
};

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
            renzora_bsn::bsn_tree::spawn_source,
        ));
        app.init_resource::<RawComponentRegistry>().add_systems(
            Startup,
            // Startup rather than plugin-build: plugins are loaded by
            // `load_global_plugins` during app build, and the schemas do not
            // exist until that has run.
            //
            // Ordered before the scene load, which reads this registry to
            // rebuild plugin components from a scene file. Both are exclusive
            // systems in the same schedule, so they cannot overlap — but nothing
            // said which ran first, and the order that happens to hold comes from
            // this plugin being added a few lines above the one that schedules
            // the load. Lose that race and every plugin-owned component in the
            // scene is dropped on load. Runtime-only, since the editor loads
            // scenes on demand long after Startup, so it would have shown up
            // exclusively in exported games.
            (|world: &mut World| refresh_raw_component_registry(world))
                .before(crate::scene_io::load_current_scene),
        );
    }
}
