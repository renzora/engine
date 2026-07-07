//! The interim BSN text format + the [`SceneSerializer`] seam.
//!
//! ## Format (interim, Renzora-owned)
//!
//! A scene is a flat list of `entity` blocks (hierarchy is carried by the
//! reflected `ChildOf` component and remapped on load, exactly as the old RON
//! `DynamicScene` did — there is no explicit `[children]` nesting):
//!
//! ```text
//! // renzora interim bsn v1
//! entity 4294967296 {
//!     bevy_transform::components::transform::Transform: (translation:(x:0,y:1,z:0),rotation:(0,0,0,1),scale:(x:1,y:1,z:1)),
//!     bevy_ecs::name::Name: ("Player"),
//! }
//! resource my::Res: (field:1),
//! ```
//!
//! Each component/resource is `<fully::qualified::TypePath>: <RON value>,` where
//! the RON value is produced by `bevy_reflect`'s [`TypedReflectSerializer`] and
//! read back by [`TypedReflectDeserializer`] — i.e. we own the *container*
//! grammar but defer the *value* encoding to bevy's reflection serde (the
//! reliable, type-complete part). When a first-party runtime BSN loader lands
//! (bevy#23576), only the [`SceneSerializer`] impl swaps; callers are unchanged.

use crate::{DynamicEntity, DynamicScene};
use bevy::reflect::serde::{TypedReflectDeserializer, TypedReflectSerializer};
use bevy::reflect::{PartialReflect, TypeRegistration, TypeRegistry};
use bevy::ecs::entity::Entity;
use serde::de::DeserializeSeed;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::{OnceLock, RwLock};

const HEADER: &str = "// renzora interim bsn v1";

/// Errors raised while (de)serializing a scene in the interim BSN format.
#[derive(thiserror::Error, Debug)]
pub enum BsnError {
    /// A component/resource value failed to serialize through reflection.
    #[error("failed to serialize `{type_path}`: {source}")]
    Serialize {
        /// The offending type path.
        type_path: String,
        /// The underlying RON error.
        source: ron::Error,
    },
    /// A component/resource value failed to deserialize through reflection.
    #[error("failed to deserialize `{type_path}`: {message}")]
    Deserialize {
        /// The offending type path.
        type_path: String,
        /// A human-readable reason.
        message: String,
    },
    /// A type path referenced in the file is not in the registry.
    #[error("unregistered type `{type_path}` at byte {offset}")]
    UnregisteredType {
        /// The type path that wasn't found.
        type_path: String,
        /// Byte offset into the source for diagnostics.
        offset: usize,
    },
    /// The text was not valid in the interim BSN grammar.
    #[error("parse error at byte {offset}: {message}")]
    Parse {
        /// Byte offset into the source.
        offset: usize,
        /// What was expected / went wrong.
        message: String,
    },
}

/// The format seam. Swappable for a first-party BSN (de)serializer later.
pub trait SceneSerializer {
    /// Serialize a [`DynamicScene`] to text.
    fn serialize(&self, scene: &DynamicScene, registry: &TypeRegistry) -> Result<String, BsnError>;
    /// Parse text back into a [`DynamicScene`], failing on the first unregistered
    /// or un-deserializable component.
    fn deserialize(&self, text: &str, registry: &TypeRegistry) -> Result<DynamicScene, BsnError>;
    /// Like [`deserialize`](Self::deserialize), but **skips** components whose
    /// type is unregistered or that fail to reflect-deserialize, returning the
    /// sorted list of skipped type paths alongside the scene. Used by the editor
    /// load path so a scene authored with a now-absent plugin still opens.
    fn deserialize_lossy(
        &self,
        text: &str,
        registry: &TypeRegistry,
    ) -> Result<(DynamicScene, Vec<String>), BsnError>;
}

/// The interim Renzora BSN (de)serializer (see the module docs).
#[derive(Default, Clone, Copy)]
pub struct BsnSerializer;

impl SceneSerializer for BsnSerializer {
    fn serialize(&self, scene: &DynamicScene, registry: &TypeRegistry) -> Result<String, BsnError> {
        let mut out = String::new();
        let _ = writeln!(out, "{HEADER}");

        for entity in &scene.entities {
            // A blank line + the entity's `Name` (if any) as a comment make the
            // opaque numeric id navigable. Both are trivia the parser skips.
            out.push('\n');
            if let Some(name) = entity_name(entity, registry) {
                let _ = writeln!(out, "// {name}");
            }
            let _ = writeln!(out, "entity {} {{", entity.entity.to_bits());
            for component in &entity.components {
                write_value(&mut out, "    ", component.as_ref(), registry)?;
            }
            let _ = writeln!(out, "}}");
        }

        for resource in &scene.resources {
            out.push('\n');
            out.push_str("resource ");
            write_value(&mut out, "", resource.as_ref(), registry)?;
        }

        Ok(out)
    }

    fn deserialize(&self, text: &str, registry: &TypeRegistry) -> Result<DynamicScene, BsnError> {
        let mut skipped = None;
        parse(text, registry, &mut skipped)
    }

    fn deserialize_lossy(
        &self,
        text: &str,
        registry: &TypeRegistry,
    ) -> Result<(DynamicScene, Vec<String>), BsnError> {
        let mut skipped = Some(Vec::new());
        let scene = parse(text, registry, &mut skipped)?;
        let mut skipped = skipped.unwrap();
        skipped.sort();
        skipped.dedup();
        Ok((scene, skipped))
    }
}

/// Shared parse core. When `skipped` is `Some`, components that are unregistered
/// or fail to reflect-deserialize are dropped (their type paths pushed into the
/// list) instead of aborting; when `None`, the first such failure is an error.
fn parse(
    text: &str,
    registry: &TypeRegistry,
    skipped: &mut Option<Vec<String>>,
) -> Result<DynamicScene, BsnError> {
    let mut parser = Parser::new(text);
    let mut scene = DynamicScene::default();

    loop {
        parser.skip_trivia();
        if parser.at_end() {
            break;
        }
        if parser.eat_keyword("entity") {
            let bits = parser.parse_u64()?;
            // Don't panic on a corrupt id (`from_bits` would) — surface a parse error.
            let entity = Entity::try_from_bits(bits)
                .ok_or_else(|| parser.err("invalid entity id bits"))?;
            parser.expect('{')?;
            let mut components = Vec::new();
            loop {
                parser.skip_trivia();
                if parser.eat('}') {
                    break;
                }
                if parser.at_end() {
                    return Err(parser.err("unterminated `entity` block (missing `}`)"));
                }
                if let Some(component) = parser.parse_component(registry, skipped)? {
                    components.push(component);
                }
            }
            scene.entities.push(DynamicEntity { entity, components });
        } else if parser.eat_keyword("resource") {
            if let Some(resource) = parser.parse_component(registry, skipped)? {
                scene.resources.push(resource);
            }
        } else {
            return Err(parser.err("expected `entity` or `resource`"));
        }
    }

    Ok(scene)
}

/// Best-effort `Name` of an entity, for the serialization comment above its
/// block. Returns `None` if the entity has no `Name` (or it won't serialize).
fn entity_name(entity: &DynamicEntity, registry: &TypeRegistry) -> Option<String> {
    let component = entity
        .components
        .iter()
        .find(|c| c.reflect_type_path() == "bevy_ecs::name::Name")?;
    let serializer = TypedReflectSerializer::new(component.as_ref(), registry);
    let s = ron::ser::to_string(&serializer).ok()?;
    // `Name` serializes as a bare RON string (`"Cube"`); strip the quotes and
    // flatten any newline so the `//` comment stays single-line.
    let name = s.trim().trim_matches('"').replace('\n', " ");
    (!name.is_empty()).then_some(name)
}

/// Emit one `type_path: <ron value>,` line at the given indent.
fn write_value(
    out: &mut String,
    indent: &str,
    value: &dyn PartialReflect,
    registry: &TypeRegistry,
) -> Result<(), BsnError> {
    let type_path = value.reflect_type_path().to_string();
    let serializer = TypedReflectSerializer::new(value, registry);
    let ron_value = ron::ser::to_string(&serializer).map_err(|source| BsnError::Serialize {
        type_path: type_path.clone(),
        source,
    })?;
    let _ = writeln!(out, "{indent}{type_path}: {ron_value},");
    Ok(())
}

/// Process-global migration aliases: `old type-path/name` → `current short name
/// or full path`. Consulted by [`resolve_registration`] when a serialized
/// type-path no longer resolves. See [`register_component_alias`].
fn component_aliases() -> &'static RwLock<HashMap<String, String>> {
    static ALIASES: OnceLock<RwLock<HashMap<String, String>>> = OnceLock::new();
    ALIASES.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Register a migration alias so scenes that recorded an **old** component
/// type-path or name still load after a *rename*. `old` is the string as it
/// appears in existing scene files (full path or short name); `current` is the
/// present short name (or full path).
///
/// Module/crate **moves** need no alias — they're absorbed automatically by the
/// short-name fallback in [`resolve_registration`]. Use this only for genuine
/// renames (the one case short-name matching can't bridge). Idempotent.
pub fn register_component_alias(old: impl Into<String>, current: impl Into<String>) {
    if let Ok(mut map) = component_aliases().write() {
        map.insert(old.into(), current.into());
    }
}

/// The trailing `Ident` of a fully-qualified type-path (what bevy registers as
/// the *short* type path). Returns `None` for generic / tuple / array paths
/// (those contain `<`, `(`, `[`), whose short form bevy computes structurally —
/// we don't reconstruct it, the exact-path lookup already covers them.
fn short_name_of(type_path: &str) -> Option<&str> {
    if type_path.contains(['<', '(', '[', ' ', '&', ';']) {
        return None;
    }
    type_path.rsplit("::").next().filter(|s| !s.is_empty())
}

/// Resolve a serialized component type-path to a live registration, tolerant of
/// the type having **moved modules/crates** or been **renamed** since the scene
/// was saved. Three tiers, most-precise first:
///
/// 1. **Exact path** — the common case; nothing moved.
/// 2. **Short-name fallback** — match by trailing `Ident` via
///    [`TypeRegistry::get_with_short_type_path`], which bevy keeps *unambiguous*
///    (colliding short names are dropped, so this never resolves to the wrong
///    type). This makes module/crate reorganizations non-breaking for free.
/// 3. **Alias map** — an explicit `old → current` entry for genuine renames.
///
/// Returns `None` only when every tier misses (truly unknown / unregistered),
/// in which case the component is skipped exactly as before.
fn resolve_registration<'r>(
    type_path: &str,
    registry: &'r TypeRegistry,
) -> Option<&'r TypeRegistration> {
    if let Some(reg) = registry.get_with_type_path(type_path) {
        return Some(reg);
    }
    // Short-name fallback (handles moves). Skipped for ambiguous short names
    // because bevy returns `None` for them — we never guess.
    if let Some(short) = short_name_of(type_path)
        && let Some(reg) = registry.get_with_short_type_path(short)
    {
        return Some(reg);
    }
    // Alias map (handles renames). Keyed by the full path or its short name.
    if let Ok(map) = component_aliases().read() {
        let target = map
            .get(type_path)
            .or_else(|| short_name_of(type_path).and_then(|s| map.get(s)));
        if let Some(target) = target
            && let Some(reg) = registry
                .get_with_type_path(target)
                .or_else(|| registry.get_with_short_type_path(target))
        {
            return Some(reg);
        }
    }
    None
}

/// Look up `type_path` in the registry and reflect-deserialize `value_str`
/// (a RON value) into a boxed component. Resolution is refactor-tolerant — see
/// [`resolve_registration`] (moved/renamed types still load).
fn reflect_component_value(
    type_path: &str,
    value_str: &str,
    registry: &TypeRegistry,
    offset: usize,
) -> Result<Box<dyn PartialReflect>, BsnError> {
    let registration =
        resolve_registration(type_path, registry).ok_or_else(|| BsnError::UnregisteredType {
            type_path: type_path.to_string(),
            offset,
        })?;
    let seed = TypedReflectDeserializer::new(registration, registry);
    let mut ron_de =
        ron::Deserializer::from_str(value_str).map_err(|e| BsnError::Deserialize {
            type_path: type_path.to_string(),
            message: e.to_string(),
        })?;
    seed.deserialize(&mut ron_de).map_err(|e| BsnError::Deserialize {
        type_path: type_path.to_string(),
        message: e.to_string(),
    })
}

/// A tiny hand-rolled cursor over the interim BSN grammar. The only non-trivial
/// part is reading a balanced RON value (delimiters + string escapes) so we can
/// hand the exact value slice to `TypedReflectDeserializer`.
struct Parser<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src,
            bytes: src.as_bytes(),
            pos: 0,
        }
    }

    fn at_end(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    fn err(&self, message: impl Into<String>) -> BsnError {
        BsnError::Parse {
            offset: self.pos,
            message: message.into(),
        }
    }

    /// Skip whitespace and `// line comments`.
    fn skip_trivia(&mut self) {
        loop {
            while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
                self.pos += 1;
            }
            if self.src[self.pos..].starts_with("//") {
                while self.pos < self.bytes.len() && self.bytes[self.pos] != b'\n' {
                    self.pos += 1;
                }
            } else {
                break;
            }
        }
    }

    fn eat(&mut self, c: char) -> bool {
        if self.bytes.get(self.pos) == Some(&(c as u8)) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, c: char) -> Result<(), BsnError> {
        self.skip_trivia();
        if self.eat(c) {
            Ok(())
        } else {
            Err(self.err(format!("expected `{c}`")))
        }
    }

    /// Consume `kw` only if it appears at the cursor as a whole word.
    fn eat_keyword(&mut self, kw: &str) -> bool {
        if self.src[self.pos..].starts_with(kw) {
            let after = self.pos + kw.len();
            let boundary = self
                .bytes
                .get(after)
                .is_none_or(|b| !b.is_ascii_alphanumeric() && *b != b'_');
            if boundary {
                self.pos = after;
                return true;
            }
        }
        false
    }

    fn parse_u64(&mut self) -> Result<u64, BsnError> {
        self.skip_trivia();
        let start = self.pos;
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        if self.pos == start {
            return Err(self.err("expected an entity id (u64)"));
        }
        self.src[start..self.pos]
            .parse::<u64>()
            .map_err(|e| self.err(format!("invalid entity id: {e}")))
    }

    /// Parse one `type_path: <ron value>,` slot and reflect-deserialize it.
    ///
    /// Always advances the cursor past the whole slot (even on a reflect
    /// failure), so callers can keep parsing. When `skipped` is `Some`, an
    /// unregistered or un-deserializable component is recorded there and `None`
    /// is returned; when `None`, the failure is propagated as an error.
    fn parse_component(
        &mut self,
        registry: &TypeRegistry,
        skipped: &mut Option<Vec<String>>,
    ) -> Result<Option<Box<dyn PartialReflect>>, BsnError> {
        self.skip_trivia();
        let path_start = self.pos;
        // Read up to the single `:` separator. Type paths contain `::`, so a
        // colon that is part of `::` is skipped — only a lone `:` terminates.
        loop {
            match self.bytes.get(self.pos) {
                None => return Err(self.err("expected `:` after type path")),
                Some(b':') => {
                    if self.bytes.get(self.pos + 1) == Some(&b':') {
                        self.pos += 2; // path separator `::`
                    } else {
                        break; // the `type_path: value` separator
                    }
                }
                Some(_) => self.pos += 1,
            }
        }
        let type_path = self.src[path_start..self.pos].trim().to_string();
        self.pos += 1; // consume the separating ':'
        self.skip_trivia();

        let value_start = self.pos;
        self.skip_balanced_value()?;
        let value_str = self.src[value_start..self.pos].trim_end().to_string();

        // Consume the trailing comma (optional on the last item before `}`).
        self.skip_trivia();
        self.eat(',');

        match reflect_component_value(&type_path, &value_str, registry, path_start) {
            Ok(value) => Ok(Some(value)),
            Err(e) => match skipped {
                Some(list) => {
                    list.push(type_path);
                    Ok(None)
                }
                None => Err(e),
            },
        }
    }

    /// Advance the cursor past one balanced value, stopping at the top-level
    /// `,` or `}` that terminates it. Tracks `()[]{}` nesting and string
    /// literals (with `\` escapes) so commas inside the value don't fool us.
    fn skip_balanced_value(&mut self) -> Result<(), BsnError> {
        let mut depth: i32 = 0;
        let mut in_string = false;
        let mut escaped = false;
        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            if in_string {
                if escaped {
                    escaped = false;
                } else if b == b'\\' {
                    escaped = true;
                } else if b == b'"' {
                    in_string = false;
                }
                self.pos += 1;
                continue;
            }
            match b {
                b'"' => in_string = true,
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' => depth -= 1,
                b'}' => {
                    if depth == 0 {
                        // End of the enclosing entity block.
                        break;
                    }
                    depth -= 1;
                }
                b',' if depth == 0 => break,
                _ => {}
            }
            self.pos += 1;
        }
        if depth != 0 {
            return Err(self.err("unbalanced delimiters in value"));
        }
        if self.pos == 0 {
            return Err(self.err("empty value"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DynamicSceneBuilder;
    use bevy::ecs::entity::EntityHashMap;
    use bevy::ecs::reflect::AppTypeRegistry;
    use bevy::prelude::*;

    fn registry() -> AppTypeRegistry {
        let atr = AppTypeRegistry::default();
        {
            let mut w = atr.write();
            // Registering these pulls their reflected field types (Vec3/Quat/…)
            // in via the derive's dependency registration.
            w.register::<Transform>();
            w.register::<Name>();
        }
        atr
    }

    #[test]
    fn round_trips_transform_and_name_through_bsn() {
        let atr = registry();

        // Source world: one entity with a Transform + Name.
        let mut src = World::new();
        src.insert_resource(atr.clone());
        let e = src
            .spawn((Transform::from_xyz(1.0, 2.0, 3.0), Name::new("Player")))
            .id();

        let scene = DynamicSceneBuilder::from_world(&src)
            .extract_entity(e)
            .build();
        assert_eq!(scene.entities.len(), 1);

        // Serialize → text, then parse it back.
        let text = {
            let reg = atr.read();
            BsnSerializer.serialize(&scene, &reg).expect("serialize")
        };
        assert!(text.contains("Transform"), "bsn:\n{text}");
        assert!(text.contains("Player"), "bsn:\n{text}");

        let parsed = {
            let reg = atr.read();
            BsnSerializer.deserialize(&text, &reg).expect("deserialize")
        };
        assert_eq!(parsed.entities.len(), 1);

        // Write into a fresh world and verify the values survived the round trip.
        let mut dst = World::new();
        dst.insert_resource(atr.clone());
        let mut map = EntityHashMap::default();
        parsed
            .write_to_world(&mut dst, &mut map)
            .expect("write_to_world");

        let new_entity = *map.values().next().expect("one mapped entity");
        let t = dst.get::<Transform>(new_entity).expect("Transform present");
        assert_eq!(t.translation, Vec3::new(1.0, 2.0, 3.0));
        let name = dst.get::<Name>(new_entity).expect("Name present");
        assert_eq!(name.as_str(), "Player");
    }

    /// A component whose serialized type-path moved modules (here faked by
    /// rewriting `Transform`'s path) must still load via the short-name fallback
    /// — the exact regression that the `core/mod.rs` split caused for scenes
    /// keyed by `renzora::core::MeshInstanceData`.
    #[test]
    fn moved_type_resolves_by_short_name() {
        let atr = registry();
        let mut src = World::new();
        src.insert_resource(atr.clone());
        let e = src.spawn(Transform::from_xyz(4.0, 5.0, 6.0)).id();
        let scene = DynamicSceneBuilder::from_world(&src).extract_entity(e).build();

        let reg = atr.read();
        let text = BsnSerializer.serialize(&scene, &reg).expect("serialize");
        // Simulate the type having moved to a different module: same short name
        // (`Transform`), different (now non-existent) full path.
        let moved = text.replace(
            "bevy_transform::components::transform::Transform",
            "legacy::core::moved::Transform",
        );
        assert!(moved.contains("legacy::core::moved::Transform"), "{moved}");

        let parsed = BsnSerializer.deserialize(&moved, &reg).expect("deserialize");
        let mut dst = World::new();
        dst.insert_resource(atr.clone());
        let mut map = EntityHashMap::default();
        parsed.write_to_world(&mut dst, &mut map).expect("write");
        let ne = *map.values().next().expect("one entity");
        let t = dst.get::<Transform>(ne).expect("Transform resolved by short name");
        assert_eq!(t.translation, Vec3::new(4.0, 5.0, 6.0));
    }

    /// A genuinely *renamed* type (short name also changed) can't be matched by
    /// short name; an explicit alias bridges it.
    #[test]
    fn renamed_type_resolves_via_alias() {
        let atr = registry();
        let mut src = World::new();
        src.insert_resource(atr.clone());
        let e = src.spawn(Name::new("Bob")).id();
        let scene = DynamicSceneBuilder::from_world(&src).extract_entity(e).build();

        let reg = atr.read();
        let text = BsnSerializer.serialize(&scene, &reg).expect("serialize");
        // Old name has a short name (`OldLabel`) that matches nothing registered,
        // so short-name fallback can't bridge it — only the alias can.
        let renamed = text.replace("bevy_ecs::name::Name", "legacy::OldLabel");

        // Without the alias the type is genuinely unknown (strict deserialize errors).
        assert!(BsnSerializer.deserialize(&renamed, &reg).is_err());

        register_component_alias("legacy::OldLabel", "Name");
        let after = BsnSerializer.deserialize(&renamed, &reg).expect("deserialize");
        let mut dst = World::new();
        dst.insert_resource(atr.clone());
        let mut map = EntityHashMap::default();
        after.write_to_world(&mut dst, &mut map).expect("write");
        let ne = *map.values().next().expect("one entity");
        assert_eq!(dst.get::<Name>(ne).expect("Name via alias").as_str(), "Bob");
    }

    #[test]
    fn lossy_deserialize_skips_unregistered_component() {
        let atr = registry();

        // Serialize a real entity (valid component encoding), then inject a
        // bogus unregistered component line into its block.
        let mut src = World::new();
        src.insert_resource(atr.clone());
        let e = src.spawn(Name::new("Keep")).id();
        let scene = DynamicSceneBuilder::from_world(&src).extract_entity(e).build();
        let text = {
            let reg = atr.read();
            BsnSerializer.serialize(&scene, &reg).expect("serialize")
        };
        let text = text.replacen("{\n", "{\n    made::up::Type: (),\n", 1);

        let (scene, skipped) = {
            let reg = atr.read();
            BsnSerializer
                .deserialize_lossy(&text, &reg)
                .expect("lossy deserialize")
        };
        assert_eq!(skipped, vec!["made::up::Type".to_string()]);
        assert_eq!(scene.entities.len(), 1);
        // The real `Name` component survived; only the bogus one was skipped.
        assert_eq!(scene.entities[0].components.len(), 1);
    }
}
