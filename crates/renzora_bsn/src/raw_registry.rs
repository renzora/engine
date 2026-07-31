//! What this crate knows about types it has no Rust definition for.
//!
//! [`RawComponentRegistry`] is deliberately plain data. `renzora_bsn` does not
//! depend on `renzora_plugin`, and should not — the scene format has no business
//! knowing that the C-ABI plugin system exists, and the dependency would run the
//! wrong way besides. Something upstream mirrors the plugin host's schemas into
//! this resource; everything here works off that copy.
//!
//! Keeping it a resource rather than a parameter is what makes this land without
//! touching a single existing call site: `DynamicSceneBuilder::from_world` and
//! `write_entity_to_world` already hold the world, so they pick it up on their
//! own.

use crate::dynamic_scene::RawField;
use bevy::ecs::component::ComponentId;
use bevy::prelude::*;
use bevy::platform::collections::HashMap;
use std::sync::Arc;

/// Everything needed to read, write and migrate one raw type.
#[derive(Clone, Debug)]
pub struct RawTypeInfo {
    /// The id in **this session**. Never serialized — ids are assigned in load
    /// order and mean nothing across runs.
    pub component_id: ComponentId,
    pub type_path: String,
    /// The size the component occupies **in this world**, which is the padded
    /// layout size and not necessarily the size the plugin declared.
    pub size: usize,
    pub is_resource: bool,
    /// Never written to a scene, and ignored if a scene names it.
    pub transient: bool,
    /// A default-valued instance, used to seed bytes a migration cannot recover.
    pub default_value: Vec<u8>,
    pub fields: Vec<RawField>,
}

/// The lookup tables, shared behind an `Arc`.
#[derive(Default, Debug)]
pub struct RawTypeTable {
    pub by_path: HashMap<String, RawTypeInfo>,
    /// Short name (the last `::` segment) to full path.
    ///
    /// `None` marks a short name two types claim. Guessing between them would
    /// load the wrong component's bytes into the right-looking slot, so an
    /// ambiguous short name resolves to nothing at all — the same choice
    /// `TypeRegistry::get_with_short_type_path` makes.
    pub by_short: HashMap<String, Option<String>>,
    pub by_component: HashMap<ComponentId, String>,
    /// Old type path to current, for types that were renamed.
    pub aliases: HashMap<String, String>,
}

impl RawTypeTable {
    /// Resolve a name from a scene file to a live type.
    ///
    /// Tries the full path, then an alias, then the short name — the same ladder
    /// the reflected path uses, so a plugin author moving a component between
    /// modules costs nothing and only a genuine rename needs an alias.
    pub fn resolve(&self, name: &str) -> Option<&RawTypeInfo> {
        if let Some(info) = self.by_path.get(name) {
            return Some(info);
        }
        if let Some(info) = self.aliases.get(name).and_then(|c| self.by_path.get(c)) {
            return Some(info);
        }
        let short = name.rsplit("::").next()?;
        match self.by_short.get(short) {
            Some(Some(path)) => self.by_path.get(path),
            _ => None,
        }
    }
}

#[derive(Resource, Clone, Default, Debug)]
pub struct RawComponentRegistry(pub Arc<RawTypeTable>);

impl RawComponentRegistry {
    pub fn is_empty(&self) -> bool {
        self.0.by_path.is_empty()
    }
}

/// Blobs from a scene whose type is not registered in this session.
///
/// Held on the entity verbatim so a round trip through a build without the
/// plugin does not destroy data. Loading a scene in the editor with a plugin
/// disabled and saving it again would otherwise silently strip every component
/// that plugin owns — the kind of loss you only notice much later.
///
/// Deliberately **not** `register_type`'d: the reflected extraction path skips
/// what it cannot find in the type registry, so staying unregistered is what
/// stops these being emitted twice.
#[derive(Component, Default, Clone, Debug)]
pub struct OrphanedRawComponents(pub Vec<crate::dynamic_scene::RawComponent>);

/// The same, for resources, plus the schemas they came with.
#[derive(Resource, Default, Clone, Debug)]
pub struct OrphanedRawScene {
    pub resources: Vec<crate::dynamic_scene::RawComponent>,
    pub schemas: Vec<crate::dynamic_scene::RawSchema>,
}

/// Reshape `bytes`, written against `from`, into this session's layout.
///
/// Matching is by **field name**, so adding, removing or reordering a field is
/// survivable: what still exists keeps its value, and what does not is left at
/// the default. Byte-for-byte reuse would silently reinterpret a saved `f32` as
/// whatever now occupies that offset.
///
/// Returns `None` when the layouts already agree, which is the common case and
/// avoids a copy.
pub fn migrate(info: &RawTypeInfo, from: &crate::dynamic_scene::RawSchema, bytes: &[u8]) -> Option<Vec<u8>> {
    let same = from.size == info.size
        && from.fields.len() == info.fields.len()
        && from
            .fields
            .iter()
            .zip(&info.fields)
            .all(|(a, b)| a.name == b.name && a.kind == b.kind && a.offset == b.offset);
    if same {
        return None;
    }

    // Start from the default rather than from zeroes: a field the old scene did
    // not have should arrive at the value the plugin author chose for it, not at
    // whatever zeroed memory happens to mean.
    let mut out = if info.default_value.len() == info.size {
        info.default_value.clone()
    } else {
        vec![0u8; info.size]
    };

    for field in &info.fields {
        let Some(old) = from.fields.iter().find(|f| f.name == field.name) else {
            continue;
        };
        // A field that changed type cannot be carried across — the bytes mean
        // something different now.
        if old.kind != field.kind {
            continue;
        }
        let width = field_width(&field.kind);
        if old.offset + width > bytes.len() || field.offset + width > out.len() {
            continue;
        }
        out[field.offset..field.offset + width]
            .copy_from_slice(&bytes[old.offset..old.offset + width]);
    }
    Some(out)
}

/// Byte width of a field kind, as spelled in the scene file.
///
/// Unknown kinds are zero-width, so a field written by a newer ABI is skipped
/// rather than copied at a guessed size.
fn field_width(kind: &str) -> usize {
    match kind {
        "f32" | "i32" => 4,
        "bool" => 1,
        "vec3" => 12,
        "quat" => 16,
        // `sys::Str256`: 252 payload bytes + a u32 length.
        "str" => 256,
        "entity" => 8,
        _ => 0,
    }
}
