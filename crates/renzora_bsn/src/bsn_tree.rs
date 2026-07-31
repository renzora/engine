//! A runtime parser for BSN syntax, and a spawner for what it produces.
//!
//! Bevy 0.19's `bsn!` is a **compile-time** macro — it expands to Rust that
//! constructs components — and there is no first-party runtime text parser yet
//! (bevy#23576). Scenes need one, and so does anything that wants to describe an
//! entity tree without being compiled against the engine. This is that parser.
//!
//! ## The syntax
//!
//! ```text
//! (
//!     #Cube                                        // optional key
//!     Transform { translation: Vec3(0.0, 0.5, 0.0) }
//!     Spinner { speed: 1.5 }
//!     PointLight { intensity: 4000.0 }
//!     [                                            // children
//!         ( Text("label") ),
//!         ( Button [ ( Text("Spawn") ) ] ),
//!     ]
//! )
//! ```
//!
//! Components are **space-separated**, not comma-separated — that is BSN, and it
//! is what lets a component body use commas without ambiguity. Children nest in
//! `[…]`, which is the whole reason this beats an imperative builder: the shape
//! of the text is the shape of the tree, with no closure per level.
//!
//! ## Why component *names* rather than types
//!
//! A name is resolvable by both of the engine's registries, and a `TypeId` is
//! resolvable by only one. Engine components go through `AppTypeRegistry` and
//! `bevy_reflect`; components owned by a C-ABI plugin have no Rust type at all
//! and go through [`RawComponentRegistry`], which knows their fields by name and
//! byte offset. Keying on the name means one syntax covers both, and a plugin
//! author never has to know which side of the boundary a component lives on.
//!
//! ## Values
//!
//! Component bodies are translated to RON and handed to `bevy_reflect`'s
//! deserializer — the same division of labour the scene format already uses. We
//! own the container grammar; reflection owns the values, because it is the part
//! that is type-complete and already correct.

use crate::raw_registry::RawComponentRegistry;
use crate::{RawComponent, RawField};
use bevy::ecs::reflect::{AppTypeRegistry, ReflectComponent};
use bevy::prelude::*;
use bevy::reflect::serde::TypedReflectDeserializer;
use bevy::reflect::std_traits::ReflectDefault;
use bevy::reflect::structs::DynamicStruct;
use bevy::reflect::tuple_struct::DynamicTupleStruct;
use bevy::reflect::{PartialReflect, Reflect, TypeInfo, TypeRegistration, TypeRegistry};
use serde::de::DeserializeSeed;

/// One entity and its subtree.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BsnTree {
    /// The `#Name` prefix, if any. Becomes a `Name` component.
    pub key: Option<String>,
    /// `(component name, RON body)`. The body is `()` for a bare marker.
    pub components: Vec<(String, String)>,
    /// Fields written as `bind(Something.field)` rather than a literal. Lifted
    /// out of the body during parse — see [`BsnBinding`].
    pub bindings: Vec<BsnBinding>,
    pub children: Vec<BsnTree>,
}

/// A field whose value comes from somewhere else, live, in both directions.
///
/// `EmberSliderWidget { value: bind(FlockSettings.cohesion) }` yields
/// `{ component: "EmberSliderWidget", field: "value", target:
/// "FlockSettings.cohesion" }`, and `value` is *removed* from the RON body so
/// reflection defaults it. The literal would be thrown away a frame later
/// anyway, and leaving it in would mean two sources of truth for one field.
///
/// The target stays an unresolved string here on purpose. This crate parses
/// scenes and knows nothing about plugin resources; whoever spawns the tree
/// resolves the name in its own namespace — which is also what keeps one
/// plugin's binding from reaching another's state.
#[derive(Clone, Debug, PartialEq)]
pub struct BsnBinding {
    pub component: String,
    pub field: String,
    pub target: String,
}

#[derive(Debug, PartialEq)]
pub struct BsnError {
    pub offset: usize,
    pub message: String,
}

impl std::fmt::Display for BsnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "bsn: {} (at byte {})", self.message, self.offset)
    }
}

impl std::error::Error for BsnError {}

struct Parser<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str) -> Self {
        Self { src, pos: 0 }
    }

    fn rest(&self) -> &'a str {
        &self.src[self.pos..]
    }

    fn err(&self, message: impl Into<String>) -> BsnError {
        BsnError {
            offset: self.pos,
            message: message.into(),
        }
    }

    fn skip_trivia(&mut self) {
        loop {
            let before = self.pos;
            self.pos += self.rest().len() - self.rest().trim_start().len();
            if self.rest().starts_with("//") {
                match self.rest().find('\n') {
                    Some(i) => self.pos += i,
                    None => self.pos = self.src.len(),
                }
            }
            if self.pos == before {
                return;
            }
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.skip_trivia();
        self.rest().chars().next()
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.pos += c.len_utf8();
            true
        } else {
            false
        }
    }

    /// A component path: `Ident` or `some::path::Ident`, plus an optional
    /// `::<Generic>` which is recorded as part of the name.
    fn path(&mut self) -> Option<String> {
        self.skip_trivia();
        let start = self.pos;
        let mut depth = 0usize;
        for (i, c) in self.rest().char_indices() {
            match c {
                '<' => depth += 1,
                '>' => depth = depth.saturating_sub(1),
                _ if c.is_alphanumeric() || c == '_' || c == ':' => {}
                _ if depth > 0 => {}
                _ => {
                    self.pos = start + i;
                    break;
                }
            }
            if start + i + c.len_utf8() == self.src.len() {
                self.pos = self.src.len();
            }
        }
        (self.pos > start).then(|| self.src[start..self.pos].to_string())
    }

    /// Copy a balanced `(..)`, `{..}` or `[..]` run, string-aware.
    fn balanced(&mut self) -> Result<&'a str, BsnError> {
        self.skip_trivia();
        let open = self.rest().chars().next().ok_or_else(|| self.err("expected a delimiter"))?;
        let close = match open {
            '(' => ')',
            '{' => '}',
            '[' => ']',
            _ => return Err(self.err("expected `(`, `{` or `[`")),
        };
        let start = self.pos;
        let mut depth = 0usize;
        let mut in_string = false;
        let mut chars = self.rest().char_indices();
        for (i, c) in chars.by_ref() {
            if in_string {
                // No escape handling: BSN string values are labels and paths, and
                // an escape grammar nobody remembers is worse than not having one.
                if c == '"' {
                    in_string = false;
                }
                continue;
            }
            match c {
                '"' => in_string = true,
                c if c == open => depth += 1,
                c if c == close => {
                    depth -= 1;
                    if depth == 0 {
                        self.pos = start + i + c.len_utf8();
                        return Ok(&self.src[start..self.pos]);
                    }
                }
                _ => {}
            }
        }
        Err(self.err(format!("unterminated `{open}`")))
    }

    /// One entity.
    ///
    /// Two spellings, both of which BSN uses: a bare run of components (the root
    /// of a `bsn!`, or a single-component entry in a list) and a parenthesised
    /// run (a multi-component entry in a list, where the parens are what say
    /// where one entity stops and the next begins).
    fn entity(&mut self, in_list: bool) -> Result<BsnTree, BsnError> {
        self.skip_trivia();
        let parenthesised = in_list && self.peek() == Some('(');
        if parenthesised {
            self.eat('(');
        }

        let mut tree = BsnTree::default();
        loop {
            self.skip_trivia();
            match self.peek() {
                None if parenthesised => return Err(self.err("unterminated entity (missing `)`)")),
                None => return Ok(tree),
                Some(')') if parenthesised => {
                    self.eat(')');
                    return Ok(tree);
                }
                // A list separator, or the end of the enclosing `[ … ]`. Both
                // end this entity without consuming anything.
                Some(',') | Some(']') if in_list && !parenthesised => return Ok(tree),
                Some('#') => {
                    self.eat('#');
                    tree.key = self.path();
                }
                _ => self.component(&mut tree)?,
            }
        }
    }

    /// One component: a path, then an optional `{ … }` / `( … )` body — or a
    /// `[ … ]` child list, if the path named a relationship.
    fn component(&mut self, tree: &mut BsnTree) -> Result<(), BsnError> {
        let Some(name) = self.path() else {
            return Err(self.err("expected a component name"));
        };
        self.skip_trivia();
        match self.rest().chars().next() {
            // `Children [ … ]`. The relationship names itself rather than the
            // brackets being magic, which is what lets a scene one day nest
            // through some relationship other than `Children` without the
            // grammar changing.
            Some('[') => {
                let block = self.balanced()?;
                let inner = &block[1..block.len() - 1];
                if !name.ends_with("Children") {
                    warn!(
                        "bsn: `{name} [ … ]` — only `Children` is supported as a nesting \
                         relationship, treating it as children"
                    );
                }
                tree.children = parse_list(inner)?;
            }
            Some('{') | Some('(') => {
                let raw = self.balanced()?;
                let (body, bound) = extract_bindings(&to_ron(raw));
                for (field, target) in bound {
                    tree.bindings.push(BsnBinding {
                        component: name.clone(),
                        field,
                        target,
                    });
                }
                tree.components.push((name, body));
            }
            // A bare marker: `Button`, `Camera3d`, `Sword`. Reflection wants a
            // unit struct's body spelled as an empty tuple.
            _ => tree.components.push((name, "()".to_string())),
        }
        Ok(())
    }
}

/// Pull `field: bind(Target.path)` entries out of a RON-shaped component body.
///
/// Returns the body with those entries removed, plus the `(field, target)` pairs.
/// Removal is what makes this work at all: reflection has no idea what `bind(…)`
/// means and would reject the body, whereas an *absent* field is already handled
/// — the partial-fill path defaults it.
///
/// Only top-level fields are considered. `bind` nested inside a struct value
/// (`tracks: [ ( name: bind(…) ) ]`) is left alone rather than half-supported,
/// because the binding would have nowhere sensible to write: the widget rebuilds
/// its whole subtree from the component, so a live value has to be a field the
/// binding can address on its own.
fn extract_bindings(body: &str) -> (String, Vec<(String, String)>) {
    // Fast path, and it is the common one — most bodies have no bindings.
    if !body.contains("bind(") || !body.starts_with('(') || !body.ends_with(')') {
        return (body.to_string(), Vec::new());
    }
    let inner = &body[1..body.len() - 1];

    let mut kept: Vec<&str> = Vec::new();
    let mut found: Vec<(String, String)> = Vec::new();
    for item in split_top_level(inner) {
        match parse_bind(item.trim()) {
            Some((field, target)) => found.push((field, target)),
            None => kept.push(item),
        }
    }
    if found.is_empty() {
        return (body.to_string(), found);
    }
    (format!("({})", kept.join(",")), found)
}

/// Split a comma-separated body at nesting depth zero, ignoring commas inside
/// nested brackets and string literals.
fn split_top_level(inner: &str) -> Vec<&str> {
    let mut items = Vec::new();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut start = 0usize;
    for (i, c) in inner.char_indices() {
        if in_string {
            if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                items.push(&inner[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    if !inner[start..].trim().is_empty() {
        items.push(&inner[start..]);
    }
    items
}

/// Match `field: bind(target)` exactly, or return `None` to leave the item be.
fn parse_bind(item: &str) -> Option<(String, String)> {
    let (field, value) = item.split_once(':')?;
    let field = field.trim();
    if field.is_empty() || !field.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }
    let value = value.trim();
    let target = value.strip_prefix("bind")?.trim_start().strip_prefix('(')?.strip_suffix(')')?;
    let target = target.trim();
    if target.is_empty() {
        return None;
    }
    Some((field.to_string(), target.to_string()))
}

/// Translate a BSN component body into the RON `bevy_reflect` expects.
///
/// Struct bodies are `{ a: 1 }` in BSN and `(a: 1)` in RON, so braces become
/// parens. Everything else — tuple bodies, enum variants like `Px(8.0)`,
/// literals — is already RON-shaped and passes through untouched. String
/// contents are skipped so a label containing a brace survives.
fn to_ron(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut in_string = false;
    for c in raw.chars() {
        if in_string {
            out.push(c);
            if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                out.push(c);
            }
            '{' => out.push('('),
            '}' => out.push(')'),
            _ => out.push(c),
        }
    }
    out
}

/// Parse a comma-separated list of entities — the body of a `Children [ … ]`
/// block, or a whole `bsn_list!`.
pub fn parse_list(text: &str) -> Result<Vec<BsnTree>, BsnError> {
    let mut parser = Parser::new(text);
    let mut out = Vec::new();
    loop {
        parser.skip_trivia();
        if parser.rest().is_empty() {
            return Ok(out);
        }
        let tree = parser.entity(true)?;
        // An entity that consumed nothing would loop forever; a stray separator
        // is the likely cause and skipping it recovers.
        if tree == BsnTree::default() && !parser.eat(',') {
            parser.pos += parser.rest().chars().next().map_or(0, char::len_utf8);
            continue;
        }
        out.push(tree);
        parser.skip_trivia();
        parser.eat(',');
    }
}

/// Parse a single entity — a whole `bsn!`.
pub fn parse(text: &str) -> Result<BsnTree, BsnError> {
    let mut parser = Parser::new(text);
    let tree = parser.entity(false)?;
    parser.skip_trivia();
    if !parser.rest().is_empty() {
        return Err(parser.err("trailing input after the entity"));
    }
    Ok(tree)
}

// ── Spawning ─────────────────────────────────────────────────────────────────

/// Bindings the spawner found but could not resolve, left on the entity for
/// whoever owns the namespace they name.
///
/// This crate deliberately does not resolve them. A binding target like
/// `FlockSettings.cohesion` only means anything relative to a particular
/// plugin's registered resources, and resolving it here would either mean this
/// crate learning about the plugin host, or a global lookup — which is the shape
/// of bug that had every plugin's panel buttons dispatching into the first
/// plugin to register. So the resolver is whoever spawned the tree, and it looks
/// only in its own namespace.
///
/// The consumer removes the component once it has wired the binding up, so an
/// entity still carrying one after that frame is an unresolved target.
#[derive(Component, Clone, Debug)]
pub struct PendingBindings(pub Vec<BsnBinding>);

/// Spawn a parsed tree into the world, returning the root.
///
/// Components resolve by name against whichever registry knows them, and a name
/// neither knows is logged and skipped rather than aborting the tree — losing
/// one component beats losing the layout it was part of, and the message names
/// what was missed.
pub fn spawn(world: &mut World, tree: &BsnTree, parent: Option<Entity>) -> Entity {
    let entity = world.spawn_empty().id();
    if let Some(parent) = parent {
        world.entity_mut(entity).insert(ChildOf(parent));
    }
    if let Some(key) = &tree.key {
        world.entity_mut(entity).insert(Name::new(key.clone()));
    }

    for (name, body) in &tree.components {
        insert_component(world, entity, name, body);
    }
    if !tree.bindings.is_empty() {
        world.entity_mut(entity).insert(PendingBindings(tree.bindings.clone()));
    }

    // Depth-first, so a child's `ChildOf` lands on a parent that exists.
    for child in &tree.children {
        spawn(world, child, Some(entity));
    }
    entity
}

/// Spawn a list of trees as siblings.
pub fn spawn_list(world: &mut World, trees: &[BsnTree], parent: Option<Entity>) -> Vec<Entity> {
    trees.iter().map(|t| spawn(world, t, parent)).collect()
}

/// Spawn `tree` onto an entity that already exists.
///
/// Used when the caller reserved an id up front — a plugin spawning through the
/// command queue gets its root's id in the same frame it asked for it, and that
/// id has to be the one the tree actually lands on.
pub fn spawn_into(world: &mut World, tree: &BsnTree, entity: Entity) {
    if let Some(key) = &tree.key {
        world.entity_mut(entity).insert(Name::new(key.clone()));
    }
    for (name, body) in &tree.components {
        insert_component(world, entity, name, body);
    }
    if !tree.bindings.is_empty() {
        world.entity_mut(entity).insert(PendingBindings(tree.bindings.clone()));
    }
    for child in &tree.children {
        spawn(world, child, Some(entity));
    }
}

/// Parse `source` and spawn it, using `root` for the first top-level entity.
///
/// The single entry point a host bridge needs: parse errors are logged with the
/// byte offset rather than propagated, because the caller is a deferred command
/// with nowhere useful to return a `Result` to.
pub fn spawn_source(world: &mut World, root: Entity, source: &str) {
    let trees = match parse_list(source) {
        Ok(t) => t,
        Err(e) => {
            error!("{e}");
            return;
        }
    };
    let mut trees = trees.into_iter();
    match trees.next() {
        Some(first) => spawn_into(world, &first, root),
        // Nothing to spawn. The reserved entity stays empty rather than being
        // despawned — the plugin already holds its id.
        None => return,
    }
    for tree in trees {
        spawn(world, &tree, None);
    }
}

fn insert_component(world: &mut World, entity: Entity, name: &str, body: &str) {
    // The engine's own components first: a plugin cannot shadow `Transform`,
    // and checking reflection first means it never accidentally does.
    if insert_reflected(world, entity, name, body) {
        return;
    }
    if insert_raw(world, entity, name, body) {
        return;
    }
    warn!(
        "bsn: no component called `{name}` — it is neither a registered type nor \
         owned by a loaded plugin"
    );
}

/// An engine component, via `bevy_reflect`.
///
/// ## Why this builds a partial rather than deserializing the body whole
///
/// `TypedReflectDeserializer` wants a **complete** value: every field of the
/// struct, in order. That is fine for a scene file, which was written by
/// serializing a real instance, and useless for hand-written BSN — nobody is
/// going to spell out all thirty fields of `Node` to set `flex_direction`.
///
/// Bevy's own `bsn!` does not have this problem because it is a compile-time
/// macro and expands to `Node { flex_direction: Column, ..Default::default() }`.
/// A runtime parser has to perform that fill itself: start from the type's
/// `Default`, deserialize only the fields the source mentions, and apply them
/// over the top.
fn insert_reflected(world: &mut World, entity: Entity, name: &str, body: &str) -> bool {
    let registry = match world.get_resource::<AppTypeRegistry>() {
        Some(r) => r.clone(),
        None => return false,
    };
    let read = registry.read();
    let Some(registration) = lookup(&read, name) else {
        return false;
    };
    let Some(reflect_component) = registration.data::<ReflectComponent>().cloned() else {
        warn!("bsn: `{name}` is registered but is not a component (missing `#[reflect(Component)]`)");
        return true;
    };
    let Some(value) = build_value(&read, registration, name, body) else {
        return true;
    };
    drop(read);

    let read = registry.read();
    reflect_component.insert(&mut world.entity_mut(entity), value.as_partial_reflect(), &read);
    true
}

/// Build a complete value of `registration`'s type from a partial BSN body.
fn build_value(
    registry: &TypeRegistry,
    registration: &TypeRegistration,
    name: &str,
    body: &str,
) -> Option<Box<dyn Reflect>> {
    // Without a `Default` there is nothing to fill the unmentioned fields from,
    // so the body has to be complete and the ordinary deserializer handles it.
    // Plenty of components derive `Reflect` without `#[reflect(Default)]`, so
    // this is the common case rather than an edge one — and when it fails, the
    // message says which of the two problems it was.
    let Some(default) = registration.data::<ReflectDefault>() else {
        return match deserialize(registration, registry, body) {
            Some(v) => v.try_into_reflect().ok(),
            None => {
                warn!(
                    "bsn: `{name}` could not read `{body}`. Without `#[reflect(Default)]` \
                     the body must name every field — add it to allow partial bodies."
                );
                None
            }
        };
    };
    let mut out = default.default();

    match registration.type_info() {
        // `Node { a: 1, b: 2 }` — a named-field struct, the common case.
        TypeInfo::Struct(info) => {
            let mut patch = DynamicStruct::default();
            for (field, value) in parse_fields(body) {
                let Some(field_info) = info.field(&field) else {
                    warn!("bsn: `{name}` has no field `{field}`");
                    continue;
                };
                let Some(reg) = registry.get(field_info.type_id()) else {
                    warn!("bsn: `{name}.{field}` is of an unregistered type");
                    continue;
                };
                match deserialize(reg, registry, &value) {
                    Some(v) => patch.insert_boxed(&field, v),
                    None => warn!("bsn: `{name}.{field}` could not read `{value}`"),
                }
            }
            out.apply(&patch);
        }
        // `Text("hi")`, `Score(0)` — positional fields.
        TypeInfo::TupleStruct(info) => {
            let args = split_args(body);
            let mut patch = DynamicTupleStruct::default();
            for (i, arg) in args.iter().enumerate() {
                let Some(field_info) = info.field_at(i) else {
                    warn!("bsn: `{name}` takes {} values, got {}", info.field_len(), args.len());
                    break;
                };
                let Some(reg) = registry.get(field_info.type_id()) else {
                    warn!("bsn: `{name}` field {i} is of an unregistered type");
                    break;
                };
                match deserialize(reg, registry, arg) {
                    Some(v) => patch.insert_boxed(v),
                    None => {
                        warn!("bsn: `{name}` could not read `{arg}`");
                        break;
                    }
                }
            }
            if patch.field_len() == info.field_len() {
                out.apply(&patch);
            }
        }
        // An enum body is complete by construction — `Px(6.0)` names its
        // variant and carries its payload — so it goes through the ordinary
        // deserializer.
        _ => {
            if body != "()" {
                if let Some(v) = deserialize(registration, registry, body) {
                    out.apply(v.as_partial_reflect());
                }
            }
        }
    }
    Some(out)
}

fn deserialize(
    registration: &TypeRegistration,
    registry: &TypeRegistry,
    text: &str,
) -> Option<Box<dyn PartialReflect>> {
    let mut de = ron::Deserializer::from_str(text).ok()?;
    TypedReflectDeserializer::new(registration, registry)
        .deserialize(&mut de)
        .ok()
}

/// Split a `(a, b, c)` tuple body at depth zero.
fn split_args(body: &str) -> Vec<String> {
    let inner = body
        .trim()
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .unwrap_or(body);
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut start = 0usize;
    for (i, c) in inner.char_indices() {
        if in_string {
            if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                let chunk = inner[start..i].trim();
                if !chunk.is_empty() {
                    out.push(chunk.to_string());
                }
                start = i + 1;
            }
            _ => {}
        }
    }
    let last = inner[start..].trim();
    if !last.is_empty() {
        out.push(last.to_string());
    }
    out
}

/// Resolve a name against the type registry: full path first, then short name.
///
/// The short-name fallback is what lets `Transform` be written rather than
/// `bevy_transform::components::transform::Transform`. It returns `None` on an
/// ambiguous short name, and that is the correct answer — two types called
/// `Settings` and a coin flip between them is worse than a clear miss.
fn lookup<'a>(
    registry: &'a TypeRegistry,
    name: &str,
) -> Option<&'a bevy::reflect::TypeRegistration> {
    registry
        .get_with_type_path(name)
        .or_else(|| registry.get_with_short_type_path(name))
}

/// A plugin-owned component, via the schema the host holds for it.
///
/// The same registry that serializes these into scene files: it knows each
/// field's name, kind and byte offset, which is exactly enough to build one from
/// a BSN body without any Rust type existing for it.
fn insert_raw(world: &mut World, entity: Entity, name: &str, body: &str) -> bool {
    let Some(registry) = world.get_resource::<RawComponentRegistry>().cloned() else {
        return false;
    };
    let Some(info) = registry.0.resolve(name) else {
        return false;
    };
    if info.is_resource {
        warn!("bsn: `{name}` is a resource, not a component");
        return true;
    }

    let mut bytes = if info.default_value.len() == info.size {
        info.default_value.clone()
    } else {
        vec![0u8; info.size]
    };
    for (field, value) in parse_fields(body) {
        let Some(f) = info.fields.iter().find(|f| f.name == field) else {
            warn!("bsn: `{name}` has no field `{field}`");
            continue;
        };
        if !write_field(&mut bytes, f, &value) {
            warn!("bsn: `{name}.{field}` could not read `{value}` as {}", f.kind);
        }
    }

    let component = RawComponent {
        type_path: info.type_path.clone(),
        bytes,
    };
    crate::dynamic_scene::insert_raw_component(world, entity, info.component_id, &component.bytes);
    true
}

/// Split a `(a: 1, b: 2)` body into field/value pairs.
///
/// Depth-aware so a nested `Vec3(1, 2, 3)` stays with its field rather than
/// being split on its own commas.
fn parse_fields(body: &str) -> Vec<(String, String)> {
    let inner = body
        .trim()
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .unwrap_or(body);
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut in_string = false;
    let mut start = 0usize;
    let bytes = inner.as_bytes();
    for i in 0..=inner.len() {
        let at_end = i == inner.len();
        if !at_end {
            let c = bytes[i] as char;
            if in_string {
                if c == '"' {
                    in_string = false;
                }
                continue;
            }
            match c {
                '"' => in_string = true,
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth = depth.saturating_sub(1),
                ',' if depth == 0 => {}
                _ => continue,
            }
            if !(c == ',' && depth == 0) {
                continue;
            }
        }
        let chunk = inner[start..i].trim();
        start = i + 1;
        if chunk.is_empty() {
            continue;
        }
        if let Some((k, v)) = chunk.split_once(':') {
            out.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    out
}

/// Write one field into a plugin component's bytes.
///
/// Unaligned because the offset came from `offset_of!` in the plugin: correct
/// for that type's layout, but applied here to a `Vec<u8>` that carries no
/// alignment guarantee of its own.
fn write_field(bytes: &mut [u8], field: &RawField, value: &str) -> bool {
    let at = field.offset;
    match field.kind.as_str() {
        "f32" => match value.parse::<f32>() {
            Ok(v) if at + 4 <= bytes.len() => {
                bytes[at..at + 4].copy_from_slice(&v.to_ne_bytes());
                true
            }
            _ => false,
        },
        "i32" => match value.parse::<i32>() {
            Ok(v) if at + 4 <= bytes.len() => {
                bytes[at..at + 4].copy_from_slice(&v.to_ne_bytes());
                true
            }
            _ => false,
        },
        // One byte, not four. A `bool` sits next to whatever the plugin declared
        // after it, and a wide write lands in that neighbour.
        "bool" => match value {
            "true" | "false" if at < bytes.len() => {
                bytes[at] = (value == "true") as u8;
                true
            }
            _ => false,
        },
        // `sys::Str256`: 252 payload bytes then a `u32` length, written as one
        // 256-byte block. The whole block is zeroed first — a shorter string
        // over a longer one would otherwise leave the old tail in place, which
        // is invisible until the length field is ever trusted over the content.
        "str" => {
            const CAP: usize = 252;
            let text = value.trim_matches('"');
            let mut end = text.len().min(CAP);
            while end > 0 && !text.is_char_boundary(end) {
                end -= 1;
            }
            if at + CAP + 4 > bytes.len() {
                return false;
            }
            bytes[at..at + CAP + 4].fill(0);
            bytes[at..at + end].copy_from_slice(&text.as_bytes()[..end]);
            bytes[at + CAP..at + CAP + 4].copy_from_slice(&(end as u32).to_ne_bytes());
            true
        }
        _ => false,
    }
}
