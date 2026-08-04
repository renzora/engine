//! What a backend hands back from a call.
//!
//! One struct for every op rather than a shape per op. Most calls fill exactly
//! one of these fields and leave the rest empty, which costs four bytes per
//! empty list — set against a decoder per op on the host side, and an
//! op/shape pairing that has to stay correct in two crates, that is a good
//! trade. It also means appending an op later needs no new decoder.

use super::command::{decode_list, encode_list, ScriptCommand};
use super::value::{DrawCmd, ScriptValue, VarDef};
use super::wire::{Reader, WireError, Writer};
use super::ByteSink;

/// Everything a call can produce.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScriptReply {
    /// World changes, applied by the engine after the hook returns.
    pub commands: Vec<ScriptCommand>,
    /// Immediate-mode 2D draws from an `on_draw(g)` pass.
    pub draws: Vec<DrawCmd>,
    /// Prop values the script wrote back. Only the ones that changed need
    /// sending, but sending all of them is also correct.
    pub vars: Vec<(String, ScriptValue)>,
    /// Prop declarations, for [`ScriptOp::Props`](super::ScriptOp::Props).
    pub props: Vec<VarDef>,
    /// The result of [`ScriptOp::Eval`](super::ScriptOp::Eval).
    pub text: Option<String>,
    /// The script's error message, when the status is
    /// [`ScriptStatus::Error`](super::ScriptStatus::Error).
    ///
    /// Carried in the reply rather than as a status code because the useful
    /// part is the text — `"player.lua on_update: attempt to index a nil
    /// value"` — and a status is an integer.
    pub error: Option<String>,
}

impl ScriptReply {
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
            && self.draws.is_empty()
            && self.vars.is_empty()
            && self.props.is_empty()
            && self.text.is_none()
            && self.error.is_none()
    }

    pub fn encode(&self, w: &mut Writer) {
        encode_list(w, &self.commands);
        w.count(self.draws.len());
        for d in &self.draws {
            d.encode(w);
        }
        w.count(self.vars.len());
        for (k, v) in &self.vars {
            w.str(k);
            v.encode(w);
        }
        w.count(self.props.len());
        for p in &self.props {
            p.encode(w);
        }
        w.opt_str(self.text.as_deref());
        w.opt_str(self.error.as_deref());
    }

    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            commands: decode_list(r)?,
            draws: r.list(DrawCmd::decode)?,
            vars: r.list(|r| Ok((r.string()?, ScriptValue::decode(r)?)))?,
            props: r.list(VarDef::decode)?,
            text: r.opt_string()?,
            error: r.opt_string()?,
        })
    }

    /// Encode straight into the host's sink.
    ///
    /// # Safety
    /// `sink` must be the one from the call currently being served — its
    /// `write` function points at host state that dies when the call returns.
    pub unsafe fn write_to(&self, sink: &ByteSink) {
        let mut w = Writer::with_capacity(256);
        self.encode(&mut w);
        let bytes = w.bytes();
        (sink.write)(sink.ctx, bytes.as_ptr(), bytes.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::value::PropValue;

    #[test]
    fn a_populated_reply_round_trips() {
        let reply = ScriptReply {
            commands: vec![
                ScriptCommand::SetPosition {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                },
                ScriptCommand::Log {
                    level: "info".into(),
                    message: "hi".into(),
                },
            ],
            draws: vec![DrawCmd::Circle {
                cx: 1.0,
                cy: 2.0,
                r: 3.0,
                color: [1.0; 4],
            }],
            vars: vec![("speed".into(), ScriptValue::Float(5.0))],
            props: vec![VarDef {
                name: "speed".into(),
                display_name: "Speed".into(),
                default_value: ScriptValue::Float(5.0),
                hint: None,
                tab: None,
            }],
            text: Some("42".into()),
            error: None,
        };

        let mut w = Writer::new();
        reply.encode(&mut w);
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        assert_eq!(ScriptReply::decode(&mut r).unwrap(), reply);
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn an_empty_reply_round_trips() {
        let reply = ScriptReply::default();
        assert!(reply.is_empty());

        let mut w = Writer::new();
        reply.encode(&mut w);
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        assert_eq!(ScriptReply::decode(&mut r).unwrap(), reply);
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn an_error_reply_carries_its_message() {
        let reply = ScriptReply {
            error: Some("player.lua on_update: attempt to index a nil value".into()),
            ..Default::default()
        };
        assert!(!reply.is_empty());

        let mut w = Writer::new();
        reply.encode(&mut w);
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        let back = ScriptReply::decode(&mut r).unwrap();
        assert_eq!(back.error.as_deref(), Some(
            "player.lua on_update: attempt to index a nil value"
        ));
    }

    /// The sink is how every reply actually leaves the plugin, so exercise it
    /// with a stand-in host buffer rather than only testing `encode`.
    #[test]
    fn write_to_a_sink_produces_decodable_bytes() {
        unsafe extern "C" fn push(ctx: *mut core::ffi::c_void, bytes: *const u8, len: usize) {
            let buf = &mut *(ctx as *mut Vec<u8>);
            buf.extend_from_slice(core::slice::from_raw_parts(bytes, len));
        }

        let mut buf: Vec<u8> = Vec::new();
        let sink = ByteSink {
            ctx: &mut buf as *mut Vec<u8> as *mut core::ffi::c_void,
            write: push,
        };

        let reply = ScriptReply {
            commands: vec![ScriptCommand::DespawnSelf],
            vars: vec![("hp".into(), ScriptValue::Int(3))],
            ..Default::default()
        };
        unsafe { reply.write_to(&sink) };

        let mut r = Reader::new(&buf);
        assert_eq!(ScriptReply::decode(&mut r).unwrap(), reply);
    }

    /// `PropValue` reaches the plugin through host-call replies, which use the
    /// same sink machinery. Cheap check that the two agree.
    #[test]
    fn a_host_call_reply_round_trips_through_the_same_encoding() {
        let mut w = Writer::new();
        w.bool(true);
        PropValue::Float(1.5).encode(&mut w);
        let bytes = w.into_bytes();

        let mut r = Reader::new(&bytes);
        assert!(r.bool().unwrap());
        assert_eq!(PropValue::decode(&mut r).unwrap(), PropValue::Float(1.5));
    }
}
