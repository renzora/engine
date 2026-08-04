//! The command vocabulary — everything a script can ask the engine to do.
//!
//! The enum itself now lives in [`renzora_plugin::script`], and this module
//! re-exports it. That is worth explaining, because the obvious arrangement is
//! the opposite one: define it here, where it is applied, and mirror it at the
//! boundary.
//!
//! A mirror needs a conversion arm per variant — 115 of them — and every
//! command added afterwards has to be added in three places, two of which are
//! in a different crate. That stays correct for about two months. Defining it
//! once, in the crate both the engine and every language plugin compile,
//! removes the question.
//!
//! The cost is that the fields lost their Bevy vocabulary: `Vec3` is `[f32; 3]`
//! and `Vec2` is `[f32; 2]`. In memory they were always that; the conversion
//! now happens where the commands are *applied*, which is one place per command
//! instead of one per construction site.
//!
//! ## Two value types that look alike
//!
//! [`PropValue`] and [`ActionValue`] are the boundary's versions of
//! [`PropertyValue`] and [`renzora::ScriptActionValue`]. The engine-wide ones
//! stay in the contract crate, because ten other crates observe
//! `ScriptAction` events and none of them should learn about the scripting
//! boundary to do it. The two meet in [`systems::commands`](crate::systems),
//! through the free functions below — free rather than `From` impls because
//! both types are foreign to this crate and the orphan rule forbids it.

pub use renzora::{CharacterCommand, CharacterCommandQueue};

/// Commands scripts issue, applied after the hook returns.
///
/// Language-agnostic — a Lua binding, a Wren binding and the blueprint compiler
/// all produce these same values.
pub use renzora_plugin::script::ScriptCommand;

/// The boundary's reflected-property value. See the module docs.
pub use renzora_plugin::script::PropValue;

/// The boundary's script-action argument. See the module docs.
pub use renzora_plugin::script::ActionValue;

/// The engine-wide reflected-property value, as every other crate knows it.
pub use renzora::PropertyValue;

/// Boundary value → engine value.
pub fn to_engine_prop(v: PropValue) -> PropertyValue {
    match v {
        PropValue::Float(f) => PropertyValue::Float(f),
        PropValue::Int(i) => PropertyValue::Int(i),
        PropValue::Bool(b) => PropertyValue::Bool(b),
        PropValue::String(s) => PropertyValue::String(s),
        PropValue::Vec3(v) => PropertyValue::Vec3(v),
        PropValue::Color(c) => PropertyValue::Color(c),
    }
}

/// Engine value → boundary value, for a `get(...)` answer on its way back to a
/// script.
pub fn to_wire_prop(v: PropertyValue) -> PropValue {
    match v {
        PropertyValue::Float(f) => PropValue::Float(f),
        PropertyValue::Int(i) => PropValue::Int(i),
        PropertyValue::Bool(b) => PropValue::Bool(b),
        PropertyValue::String(s) => PropValue::String(s),
        PropertyValue::Vec3(v) => PropValue::Vec3(v),
        PropertyValue::Color(c) => PropValue::Color(c),
    }
}

/// Boundary action argument → engine action argument.
pub fn to_engine_action(v: ActionValue) -> renzora::ScriptActionValue {
    use renzora::ScriptActionValue as E;
    match v {
        ActionValue::Float(f) => E::Float(f),
        ActionValue::Int(i) => E::Int(i),
        ActionValue::Bool(b) => E::Bool(b),
        ActionValue::String(s) => E::String(s),
        ActionValue::Vec3(v) => E::Vec3(v),
    }
}

/// Engine action argument → boundary action argument, for the arguments of an
/// inbound `on_rpc` / `on_ui` hook.
pub fn to_wire_action(v: renzora::ScriptActionValue) -> ActionValue {
    use renzora::ScriptActionValue as E;
    match v {
        E::Float(f) => ActionValue::Float(f),
        E::Int(i) => ActionValue::Int(i),
        E::Bool(b) => ActionValue::Bool(b),
        E::String(s) => ActionValue::String(s),
        E::Vec3(v) => ActionValue::Vec3(v),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both directions for every variant. Cheap, and the failure it guards
    /// against — a variant silently mapping to the wrong one — is invisible
    /// until a script sets a colour and gets a position.
    #[test]
    fn prop_values_survive_a_round_trip_in_both_directions() {
        for v in [
            PropValue::Float(1.5),
            PropValue::Int(-9),
            PropValue::Bool(true),
            PropValue::String("hi".into()),
            PropValue::Vec3([1.0, 2.0, 3.0]),
            PropValue::Color([1.0, 2.0, 3.0, 4.0]),
        ] {
            assert_eq!(to_wire_prop(to_engine_prop(v.clone())), v);
        }
    }

    #[test]
    fn action_values_survive_a_round_trip_in_both_directions() {
        for v in [
            ActionValue::Float(1.5),
            ActionValue::Int(-9),
            ActionValue::Bool(true),
            ActionValue::String("hi".into()),
            ActionValue::Vec3([1.0, 2.0, 3.0]),
        ] {
            assert_eq!(to_wire_action(to_engine_action(v.clone())), v);
        }
    }
}
