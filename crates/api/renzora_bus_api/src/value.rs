//! The one payload type every plugin agrees on.
//!
//! Two native plugins are compiled separately, so a struct defined in one is a
//! type the other has never seen. What they can both name is a type in the
//! contract, and this is it: a small JSON-like tree with the engine types a game
//! actually passes around (`Entity`, the vectors) as first-class leaves, so an
//! entity does not have to survive a trip through a string or an integer.

use std::collections::BTreeMap;

use bevy::prelude::{Entity, Vec2, Vec3, Vec4};

use crate::BusError;

/// A value carried by an event or a service call.
#[derive(Clone, Debug, PartialEq, Default)]
pub enum BusValue {
    #[default]
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Entity(Entity),
    Vec2(Vec2),
    Vec3(Vec3),
    /// Also how a colour travels: linear RGBA.
    Vec4(Vec4),
    List(Vec<BusValue>),
    /// Keys are sorted, so a payload prints and compares the same however it
    /// was built.
    Map(BTreeMap<String, BusValue>),
}

/// The shape a declared field or return value has. Used only for checking and
/// for listing what a plugin offers; a value never carries one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BusKind {
    /// Anything, `Null` included. What an undeclared return value is.
    Any,
    Null,
    Bool,
    Int,
    /// Also accepts an `Int`, so a caller that writes `25` for a float field is
    /// not refused over a missing `.0`.
    Float,
    String,
    /// Also accepts an `Int` holding `Entity::to_bits`, which is how a script
    /// passes an entity: script languages have no entity type of their own.
    Entity,
    Vec2,
    Vec3,
    Vec4,
    List,
    Map,
}

impl BusKind {
    /// Whether `value` is acceptable where this kind was declared.
    pub fn accepts(self, value: &BusValue) -> bool {
        match self {
            BusKind::Any => true,
            BusKind::Float => matches!(value, BusValue::Float(_) | BusValue::Int(_)),
            BusKind::Entity => Entity::from_bus(value).is_some(),
            kind => value.kind() == kind,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            BusKind::Any => "Any",
            BusKind::Null => "Null",
            BusKind::Bool => "Bool",
            BusKind::Int => "Int",
            BusKind::Float => "Float",
            BusKind::String => "String",
            BusKind::Entity => "Entity",
            BusKind::Vec2 => "Vec2",
            BusKind::Vec3 => "Vec3",
            BusKind::Vec4 => "Vec4",
            BusKind::List => "List",
            BusKind::Map => "Map",
        }
    }
}

impl std::fmt::Display for BusKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

impl BusValue {
    /// An empty map, the usual start of a payload:
    /// `BusValue::map().with("entity", e).with("amount", 25.0)`.
    pub fn map() -> Self {
        BusValue::Map(BTreeMap::new())
    }

    /// Adds `key` to a map. On anything that is not a map, the value is
    /// replaced by a map holding just this key, so a chain started from
    /// `BusValue::Null` still does what it reads as.
    pub fn with(mut self, key: impl Into<String>, value: impl Into<BusValue>) -> Self {
        match &mut self {
            BusValue::Map(map) => {
                map.insert(key.into(), value.into());
                self
            }
            _ => BusValue::map().with(key, value),
        }
    }

    pub fn kind(&self) -> BusKind {
        match self {
            BusValue::Null => BusKind::Null,
            BusValue::Bool(_) => BusKind::Bool,
            BusValue::Int(_) => BusKind::Int,
            BusValue::Float(_) => BusKind::Float,
            BusValue::String(_) => BusKind::String,
            BusValue::Entity(_) => BusKind::Entity,
            BusValue::Vec2(_) => BusKind::Vec2,
            BusValue::Vec3(_) => BusKind::Vec3,
            BusValue::Vec4(_) => BusKind::Vec4,
            BusValue::List(_) => BusKind::List,
            BusValue::Map(_) => BusKind::Map,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, BusValue::Null)
    }

    /// A key of a map; `None` for a missing key or a value that is not a map.
    pub fn get(&self, key: &str) -> Option<&BusValue> {
        match self {
            BusValue::Map(map) => map.get(key),
            _ => None,
        }
    }

    /// A field of a map payload, converted, with an error that names the field.
    /// Written for handlers, so `?` turns a malformed payload into a `BadArgs`
    /// answer instead of a panic: `let amount: f32 = args.field("amount")?;`
    pub fn field<T: FromBusValue>(&self, key: &str) -> Result<T, BusError> {
        match self.get(key) {
            Some(value) => T::from_bus(value).ok_or_else(|| {
                BusError::BadArgs(format!(
                    "`{key}` should be {}, got {}",
                    T::KIND,
                    value.kind()
                ))
            }),
            None => Err(BusError::BadArgs(format!("`{key}` is missing"))),
        }
    }

    /// Like [`field`](Self::field), but a missing key or a `Null` is `None`
    /// rather than an error. A present value of the wrong kind is still one.
    pub fn opt_field<T: FromBusValue>(&self, key: &str) -> Result<Option<T>, BusError> {
        match self.get(key) {
            None | Some(BusValue::Null) => Ok(None),
            Some(_) => self.field(key).map(Some),
        }
    }

    /// This value converted, for a payload that is a bare value rather than a
    /// map.
    pub fn to<T: FromBusValue>(&self) -> Result<T, BusError> {
        T::from_bus(self).ok_or_else(|| {
            BusError::BadArgs(format!("expected {}, got {}", T::KIND, self.kind()))
        })
    }

    pub fn as_bool(&self) -> Option<bool> {
        bool::from_bus(self)
    }
    pub fn as_int(&self) -> Option<i64> {
        i64::from_bus(self)
    }
    /// An `Int` reads as a float too, matching [`BusKind::Float`].
    pub fn as_float(&self) -> Option<f64> {
        f64::from_bus(self)
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            BusValue::String(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_entity(&self) -> Option<Entity> {
        Entity::from_bus(self)
    }
    pub fn as_list(&self) -> Option<&[BusValue]> {
        match self {
            BusValue::List(list) => Some(list),
            _ => None,
        }
    }
    pub fn as_map(&self) -> Option<&BTreeMap<String, BusValue>> {
        match self {
            BusValue::Map(map) => Some(map),
            _ => None,
        }
    }
}

/// A Rust type a [`BusValue`] converts into, for [`BusValue::field`].
pub trait FromBusValue: Sized {
    /// What the error says was expected.
    const KIND: BusKind;
    fn from_bus(value: &BusValue) -> Option<Self>;
}

impl FromBusValue for BusValue {
    const KIND: BusKind = BusKind::Any;
    fn from_bus(value: &BusValue) -> Option<Self> {
        Some(value.clone())
    }
}

impl FromBusValue for bool {
    const KIND: BusKind = BusKind::Bool;
    fn from_bus(value: &BusValue) -> Option<Self> {
        match value {
            BusValue::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

impl FromBusValue for i64 {
    const KIND: BusKind = BusKind::Int;
    fn from_bus(value: &BusValue) -> Option<Self> {
        match value {
            BusValue::Int(i) => Some(*i),
            _ => None,
        }
    }
}

/// Out-of-range is a mismatch, not a silent wrap.
impl FromBusValue for i32 {
    const KIND: BusKind = BusKind::Int;
    fn from_bus(value: &BusValue) -> Option<Self> {
        i64::from_bus(value).and_then(|i| i32::try_from(i).ok())
    }
}

impl FromBusValue for u32 {
    const KIND: BusKind = BusKind::Int;
    fn from_bus(value: &BusValue) -> Option<Self> {
        i64::from_bus(value).and_then(|i| u32::try_from(i).ok())
    }
}

impl FromBusValue for usize {
    const KIND: BusKind = BusKind::Int;
    fn from_bus(value: &BusValue) -> Option<Self> {
        i64::from_bus(value).and_then(|i| usize::try_from(i).ok())
    }
}

impl FromBusValue for f64 {
    const KIND: BusKind = BusKind::Float;
    fn from_bus(value: &BusValue) -> Option<Self> {
        match value {
            BusValue::Float(f) => Some(*f),
            BusValue::Int(i) => Some(*i as f64),
            _ => None,
        }
    }
}

impl FromBusValue for f32 {
    const KIND: BusKind = BusKind::Float;
    fn from_bus(value: &BusValue) -> Option<Self> {
        f64::from_bus(value).map(|f| f as f32)
    }
}

impl FromBusValue for String {
    const KIND: BusKind = BusKind::String;
    fn from_bus(value: &BusValue) -> Option<Self> {
        value.as_str().map(str::to_string)
    }
}

impl FromBusValue for Entity {
    const KIND: BusKind = BusKind::Entity;
    fn from_bus(value: &BusValue) -> Option<Self> {
        match value {
            BusValue::Entity(e) => Some(*e),
            // A script's entity id. Bits that are not a valid entity (a stray
            // `0` meaning "none") are a mismatch rather than a panic.
            BusValue::Int(bits) => u64::try_from(*bits).ok().and_then(Entity::try_from_bits),
            _ => None,
        }
    }
}

impl FromBusValue for Vec2 {
    const KIND: BusKind = BusKind::Vec2;
    fn from_bus(value: &BusValue) -> Option<Self> {
        match value {
            BusValue::Vec2(v) => Some(*v),
            _ => None,
        }
    }
}

impl FromBusValue for Vec3 {
    const KIND: BusKind = BusKind::Vec3;
    fn from_bus(value: &BusValue) -> Option<Self> {
        match value {
            BusValue::Vec3(v) => Some(*v),
            _ => None,
        }
    }
}

impl FromBusValue for Vec4 {
    const KIND: BusKind = BusKind::Vec4;
    fn from_bus(value: &BusValue) -> Option<Self> {
        match value {
            BusValue::Vec4(v) => Some(*v),
            _ => None,
        }
    }
}

impl From<()> for BusValue {
    fn from(_: ()) -> Self {
        BusValue::Null
    }
}
impl From<bool> for BusValue {
    fn from(v: bool) -> Self {
        BusValue::Bool(v)
    }
}
impl From<i32> for BusValue {
    fn from(v: i32) -> Self {
        BusValue::Int(v.into())
    }
}
impl From<i64> for BusValue {
    fn from(v: i64) -> Self {
        BusValue::Int(v)
    }
}
impl From<u32> for BusValue {
    fn from(v: u32) -> Self {
        BusValue::Int(v.into())
    }
}
impl From<f32> for BusValue {
    fn from(v: f32) -> Self {
        BusValue::Float(v.into())
    }
}
impl From<f64> for BusValue {
    fn from(v: f64) -> Self {
        BusValue::Float(v)
    }
}
impl From<&str> for BusValue {
    fn from(v: &str) -> Self {
        BusValue::String(v.to_string())
    }
}
impl From<String> for BusValue {
    fn from(v: String) -> Self {
        BusValue::String(v)
    }
}
impl From<Entity> for BusValue {
    fn from(v: Entity) -> Self {
        BusValue::Entity(v)
    }
}
impl From<Vec2> for BusValue {
    fn from(v: Vec2) -> Self {
        BusValue::Vec2(v)
    }
}
impl From<Vec3> for BusValue {
    fn from(v: Vec3) -> Self {
        BusValue::Vec3(v)
    }
}
impl From<Vec4> for BusValue {
    fn from(v: Vec4) -> Self {
        BusValue::Vec4(v)
    }
}
impl<T: Into<BusValue>> From<Vec<T>> for BusValue {
    fn from(v: Vec<T>) -> Self {
        BusValue::List(v.into_iter().map(Into::into).collect())
    }
}
impl<T: Into<BusValue>> From<Option<T>> for BusValue {
    fn from(v: Option<T>) -> Self {
        v.map_or(BusValue::Null, Into::into)
    }
}
impl From<BTreeMap<String, BusValue>> for BusValue {
    fn from(v: BTreeMap<String, BusValue>) -> Self {
        BusValue::Map(v)
    }
}
