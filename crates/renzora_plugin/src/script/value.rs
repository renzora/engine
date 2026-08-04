//! The value types scripts pass around, in their boundary-crossing form.
//!
//! Each of these mirrors a type the engine already has — `ActionValue` mirrors
//! `renzora::ScriptActionValue`, `PropValue` mirrors `renzora::PropertyValue`,
//! `DrawCmd` mirrors `renzora::DrawCmd`. The mirrors exist because those live
//! in the engine's contract crate, which pulls in Bevy, and this crate compiles
//! into plugins that link none of it.
//!
//! They are cheap to keep in sync because none of them contain a Bevy type in
//! the first place: every field is an `f32`, an `i64`, a `bool`, a `String` or
//! a fixed-size float array. `Vec3` and `Color` become `[f32; 3]` and
//! `[f32; 4]`, which is what they already were in memory. The engine converts
//! at the two places these are consumed rather than threading a wire type
//! through code that has no business knowing about the boundary.

use super::wire::{Reader, WireError, Writer};

/// An argument to a generic script action.
///
/// Mirrors `renzora::ScriptActionValue`. This is the vocabulary of the
/// declarative binding system: a domain crate says "`apply_force` takes three
/// floats named x, y, z" and the language plugin builds a function that packs
/// them into these.
#[derive(Debug, Clone, PartialEq)]
pub enum ActionValue {
    Float(f32),
    Int(i64),
    Bool(bool),
    String(String),
    Vec3([f32; 3]),
}

impl ActionValue {
    pub fn encode(&self, w: &mut Writer) {
        match self {
            Self::Float(v) => {
                w.u16(0);
                w.f32(*v);
            }
            Self::Int(v) => {
                w.u16(1);
                w.i64(*v);
            }
            Self::Bool(v) => {
                w.u16(2);
                w.bool(*v);
            }
            Self::String(v) => {
                w.u16(3);
                w.str(v);
            }
            Self::Vec3(v) => {
                w.u16(4);
                w.f32x3(*v);
            }
        }
    }

    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        let tag = r.u16()?;
        Ok(match tag {
            0 => Self::Float(r.f32()?),
            1 => Self::Int(r.i64()?),
            2 => Self::Bool(r.bool()?),
            3 => Self::String(r.string()?),
            4 => Self::Vec3(r.f32x3()?),
            t => return Err(WireError::UnknownTag(t as u32)),
        })
    }
}

/// A reflected property value.
///
/// Mirrors `renzora::PropertyValue`. Travels in both directions: out as the
/// value of a `set(...)`, back in as the answer to a `get(...)`.
#[derive(Debug, Clone, PartialEq)]
pub enum PropValue {
    Float(f32),
    Int(i64),
    Bool(bool),
    String(String),
    Vec3([f32; 3]),
    Color([f32; 4]),
}

impl PropValue {
    pub fn encode(&self, w: &mut Writer) {
        match self {
            Self::Float(v) => {
                w.u16(0);
                w.f32(*v);
            }
            Self::Int(v) => {
                w.u16(1);
                w.i64(*v);
            }
            Self::Bool(v) => {
                w.u16(2);
                w.bool(*v);
            }
            Self::String(v) => {
                w.u16(3);
                w.str(v);
            }
            Self::Vec3(v) => {
                w.u16(4);
                w.f32x3(*v);
            }
            Self::Color(v) => {
                w.u16(5);
                w.f32x4(*v);
            }
        }
    }

    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        let tag = r.u16()?;
        Ok(match tag {
            0 => Self::Float(r.f32()?),
            1 => Self::Int(r.i64()?),
            2 => Self::Bool(r.bool()?),
            3 => Self::String(r.string()?),
            4 => Self::Vec3(r.f32x3()?),
            5 => Self::Color(r.f32x4()?),
            t => return Err(WireError::UnknownTag(t as u32)),
        })
    }
}

/// The value of a script *prop* — a variable a script declares and the
/// inspector edits.
///
/// Mirrors `renzora_scripting::component::ScriptValue`. Distinct from
/// [`PropValue`] despite the overlap, because the two are edited by different
/// UI and `Entity` here is a *name* the inspector resolves with an entity
/// picker, not a number.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptValue {
    Float(f32),
    Int(i32),
    Bool(bool),
    String(String),
    Entity(String),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Color([f32; 4]),
}

impl ScriptValue {
    pub fn encode(&self, w: &mut Writer) {
        match self {
            Self::Float(v) => {
                w.u16(0);
                w.f32(*v);
            }
            Self::Int(v) => {
                w.u16(1);
                w.i64(*v as i64);
            }
            Self::Bool(v) => {
                w.u16(2);
                w.bool(*v);
            }
            Self::String(v) => {
                w.u16(3);
                w.str(v);
            }
            Self::Entity(v) => {
                w.u16(4);
                w.str(v);
            }
            Self::Vec2(v) => {
                w.u16(5);
                w.f32x2(*v);
            }
            Self::Vec3(v) => {
                w.u16(6);
                w.f32x3(*v);
            }
            Self::Color(v) => {
                w.u16(7);
                w.f32x4(*v);
            }
        }
    }

    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        let tag = r.u16()?;
        Ok(match tag {
            0 => Self::Float(r.f32()?),
            1 => Self::Int(r.i64()? as i32),
            2 => Self::Bool(r.bool()?),
            3 => Self::String(r.string()?),
            4 => Self::Entity(r.string()?),
            5 => Self::Vec2(r.f32x2()?),
            6 => Self::Vec3(r.f32x3()?),
            7 => Self::Color(r.f32x4()?),
            t => return Err(WireError::UnknownTag(t as u32)),
        })
    }
}

/// One prop a script declares, as the inspector needs it.
///
/// Produced by [`ScriptOp::Props`](super::ScriptOp::Props): the host hands the
/// plugin a script's source, the plugin parses whatever its language's prop
/// syntax is, and the inspector draws rows from the result without knowing the
/// language.
#[derive(Debug, Clone, PartialEq)]
pub struct VarDef {
    pub name: String,
    pub display_name: String,
    pub default_value: ScriptValue,
    pub hint: Option<String>,
    /// Inspector group. Props sharing a tab render under one collapsible
    /// header; `None` falls into "General".
    pub tab: Option<String>,
}

impl VarDef {
    pub fn encode(&self, w: &mut Writer) {
        w.str(&self.name);
        w.str(&self.display_name);
        self.default_value.encode(w);
        w.opt_str(self.hint.as_deref());
        w.opt_str(self.tab.as_deref());
    }

    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            name: r.string()?,
            display_name: r.string()?,
            default_value: ScriptValue::decode(r)?,
            hint: r.opt_string()?,
            tab: r.opt_string()?,
        })
    }
}

/// One immediate-mode 2D draw, emitted by an `on_draw(g)` pass.
///
/// Mirrors `renzora::DrawCmd`. Kept out of the [`ScriptCommand`] list because
/// draws are not ECS commands — they are a per-frame list the UI vector
/// renderer reconciles, rebuilt from scratch every frame.
///
/// [`ScriptCommand`]: super::ScriptCommand
#[derive(Debug, Clone, PartialEq)]
pub enum DrawCmd {
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: [f32; 4],
        thickness: f32,
    },
    /// `start`/`end` in degrees (0 = +x, clockwise, y-down).
    Arc {
        cx: f32,
        cy: f32,
        r: f32,
        start: f32,
        end: f32,
        color: [f32; 4],
        thickness: f32,
    },
    Circle {
        cx: f32,
        cy: f32,
        r: f32,
        color: [f32; 4],
    },
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
    },
    Triangle {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        x3: f32,
        y3: f32,
        color: [f32; 4],
    },
    /// Baseline-anchored at `(x, y)`, centred horizontally on `x`.
    Text {
        x: f32,
        y: f32,
        text: String,
        size: f32,
        color: [f32; 4],
    },
}

impl DrawCmd {
    pub fn encode(&self, w: &mut Writer) {
        match self {
            Self::Line {
                x1,
                y1,
                x2,
                y2,
                color,
                thickness,
            } => {
                w.u16(0);
                w.f32(*x1);
                w.f32(*y1);
                w.f32(*x2);
                w.f32(*y2);
                w.f32x4(*color);
                w.f32(*thickness);
            }
            Self::Arc {
                cx,
                cy,
                r,
                start,
                end,
                color,
                thickness,
            } => {
                w.u16(1);
                w.f32(*cx);
                w.f32(*cy);
                w.f32(*r);
                w.f32(*start);
                w.f32(*end);
                w.f32x4(*color);
                w.f32(*thickness);
            }
            Self::Circle { cx, cy, r, color } => {
                w.u16(2);
                w.f32(*cx);
                w.f32(*cy);
                w.f32(*r);
                w.f32x4(*color);
            }
            Self::Rect { x, y, w: ww, h, color } => {
                w.u16(3);
                w.f32(*x);
                w.f32(*y);
                w.f32(*ww);
                w.f32(*h);
                w.f32x4(*color);
            }
            Self::Triangle {
                x1,
                y1,
                x2,
                y2,
                x3,
                y3,
                color,
            } => {
                w.u16(4);
                w.f32(*x1);
                w.f32(*y1);
                w.f32(*x2);
                w.f32(*y2);
                w.f32(*x3);
                w.f32(*y3);
                w.f32x4(*color);
            }
            Self::Text {
                x,
                y,
                text,
                size,
                color,
            } => {
                w.u16(5);
                w.f32(*x);
                w.f32(*y);
                w.str(text);
                w.f32(*size);
                w.f32x4(*color);
            }
        }
    }

    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        let tag = r.u16()?;
        Ok(match tag {
            0 => Self::Line {
                x1: r.f32()?,
                y1: r.f32()?,
                x2: r.f32()?,
                y2: r.f32()?,
                color: r.f32x4()?,
                thickness: r.f32()?,
            },
            1 => Self::Arc {
                cx: r.f32()?,
                cy: r.f32()?,
                r: r.f32()?,
                start: r.f32()?,
                end: r.f32()?,
                color: r.f32x4()?,
                thickness: r.f32()?,
            },
            2 => Self::Circle {
                cx: r.f32()?,
                cy: r.f32()?,
                r: r.f32()?,
                color: r.f32x4()?,
            },
            3 => Self::Rect {
                x: r.f32()?,
                y: r.f32()?,
                w: r.f32()?,
                h: r.f32()?,
                color: r.f32x4()?,
            },
            4 => Self::Triangle {
                x1: r.f32()?,
                y1: r.f32()?,
                x2: r.f32()?,
                y2: r.f32()?,
                x3: r.f32()?,
                y3: r.f32()?,
                color: r.f32x4()?,
            },
            5 => Self::Text {
                x: r.f32()?,
                y: r.f32()?,
                text: r.string()?,
                size: r.f32()?,
                color: r.f32x4()?,
            },
            t => return Err(WireError::UnknownTag(t as u32)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<T: PartialEq + core::fmt::Debug>(
        value: &T,
        enc: impl Fn(&T, &mut Writer),
        dec: impl Fn(&mut Reader) -> Result<T, WireError>,
    ) {
        let mut w = Writer::new();
        enc(value, &mut w);
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        assert_eq!(&dec(&mut r).unwrap(), value);
        assert_eq!(r.remaining(), 0, "decoder read fewer bytes than encoder wrote");
    }

    #[test]
    fn every_action_value_round_trips() {
        for v in [
            ActionValue::Float(1.5),
            ActionValue::Int(-9),
            ActionValue::Bool(true),
            ActionValue::String("hi".into()),
            ActionValue::Vec3([1.0, 2.0, 3.0]),
        ] {
            round_trip(&v, ActionValue::encode, ActionValue::decode);
        }
    }

    #[test]
    fn every_prop_value_round_trips() {
        for v in [
            PropValue::Float(1.5),
            PropValue::Int(-9),
            PropValue::Bool(false),
            PropValue::String("hi".into()),
            PropValue::Vec3([1.0, 2.0, 3.0]),
            PropValue::Color([1.0, 2.0, 3.0, 4.0]),
        ] {
            round_trip(&v, PropValue::encode, PropValue::decode);
        }
    }

    #[test]
    fn every_script_value_round_trips() {
        for v in [
            ScriptValue::Float(1.5),
            ScriptValue::Int(-9),
            ScriptValue::Bool(true),
            ScriptValue::String("hi".into()),
            ScriptValue::Entity("Player".into()),
            ScriptValue::Vec2([1.0, 2.0]),
            ScriptValue::Vec3([1.0, 2.0, 3.0]),
            ScriptValue::Color([1.0, 2.0, 3.0, 4.0]),
        ] {
            round_trip(&v, ScriptValue::encode, ScriptValue::decode);
        }
    }

    #[test]
    fn every_draw_cmd_round_trips() {
        let c = [1.0, 0.5, 0.25, 1.0];
        for v in [
            DrawCmd::Line {
                x1: 1.0,
                y1: 2.0,
                x2: 3.0,
                y2: 4.0,
                color: c,
                thickness: 2.0,
            },
            DrawCmd::Arc {
                cx: 1.0,
                cy: 2.0,
                r: 3.0,
                start: 0.0,
                end: 90.0,
                color: c,
                thickness: 1.0,
            },
            DrawCmd::Circle {
                cx: 1.0,
                cy: 2.0,
                r: 3.0,
                color: c,
            },
            DrawCmd::Rect {
                x: 1.0,
                y: 2.0,
                w: 3.0,
                h: 4.0,
                color: c,
            },
            DrawCmd::Triangle {
                x1: 1.0,
                y1: 2.0,
                x2: 3.0,
                y2: 4.0,
                x3: 5.0,
                y3: 6.0,
                color: c,
            },
            DrawCmd::Text {
                x: 1.0,
                y: 2.0,
                text: "score".into(),
                size: 16.0,
                color: c,
            },
        ] {
            round_trip(&v, DrawCmd::encode, DrawCmd::decode);
        }
    }

    #[test]
    fn var_def_round_trips_with_and_without_optionals() {
        round_trip(
            &VarDef {
                name: "speed".into(),
                display_name: "Speed".into(),
                default_value: ScriptValue::Float(5.0),
                hint: Some("units per second".into()),
                tab: Some("Movement".into()),
            },
            VarDef::encode,
            VarDef::decode,
        );
        round_trip(
            &VarDef {
                name: "speed".into(),
                display_name: "Speed".into(),
                default_value: ScriptValue::Float(5.0),
                hint: None,
                tab: None,
            },
            VarDef::encode,
            VarDef::decode,
        );
    }

    #[test]
    fn a_tag_from_a_newer_writer_is_refused_not_misread() {
        let mut w = Writer::new();
        w.u16(999);
        w.f32(1.0);
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        assert_eq!(ActionValue::decode(&mut r), Err(WireError::UnknownTag(999)));
    }
}
