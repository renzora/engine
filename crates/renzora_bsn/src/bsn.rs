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
use bevy::reflect::{PartialReflect, TypeRegistry};
use bevy::ecs::entity::Entity;
use serde::de::DeserializeSeed;
use std::fmt::Write as _;

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
            let _ = writeln!(out, "entity {} {{", entity.entity.to_bits());
            for component in &entity.components {
                write_value(&mut out, "    ", component.as_ref(), registry)?;
            }
            let _ = writeln!(out, "}}");
        }

        for resource in &scene.resources {
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

/// Look up `type_path` in the registry and reflect-deserialize `value_str`
/// (a RON value) into a boxed component.
fn reflect_component_value(
    type_path: &str,
    value_str: &str,
    registry: &TypeRegistry,
    offset: usize,
) -> Result<Box<dyn PartialReflect>, BsnError> {
    let registration =
        registry
            .get_with_type_path(type_path)
            .ok_or_else(|| BsnError::UnregisteredType {
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
