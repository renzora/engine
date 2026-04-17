use bevy::prelude::Entity;

/// The value a binding carries. Mirrors the shape of the existing
/// `FieldValue` in the inspector registry so inspector getters/setters
/// can be reused as bindings without an adapter layer.
///
/// Equality is structural — two `BoundValue`s compare equal iff their
/// variants and contents match exactly. `sync_bindings` uses this to
/// decide whether to fire a `BindingChanged` event: no spurious updates
/// when the upstream component's `Changed<T>` flag fires but the specific
/// bound field didn't actually move.
#[derive(Debug, Clone, PartialEq)]
pub enum BoundValue {
    Unit,
    Bool(bool),
    I32(i32),
    U32(u32),
    F32(f32),
    F64(f64),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    /// RGBA, 0..=1 per channel.
    Color([f32; 4]),
    String(String),
    Entity(Option<Entity>),
    /// Project-relative asset path (or `None` for "unset").
    Asset(Option<String>),
    /// Opaque enum value identified by a string tag. Widgets deciding how
    /// to render a dropdown use the tag; the full list of tags is static
    /// and owned by the widget's registration, not the binding.
    EnumTag(String),
}

impl BoundValue {
    /// Best-effort conversion to display text (for labels, tooltips).
    pub fn as_display(&self) -> String {
        match self {
            BoundValue::Unit => String::new(),
            BoundValue::Bool(b) => b.to_string(),
            BoundValue::I32(v) => v.to_string(),
            BoundValue::U32(v) => v.to_string(),
            BoundValue::F32(v) => format!("{v:.3}"),
            BoundValue::F64(v) => format!("{v:.3}"),
            BoundValue::Vec2(v) => format!("{:.3}, {:.3}", v[0], v[1]),
            BoundValue::Vec3(v) => format!("{:.3}, {:.3}, {:.3}", v[0], v[1], v[2]),
            BoundValue::Vec4(v) => format!("{:.3}, {:.3}, {:.3}, {:.3}", v[0], v[1], v[2], v[3]),
            BoundValue::Color(c) => {
                let [r, g, b, a] = *c;
                format!("#{:02X}{:02X}{:02X}{:02X}",
                    (r.clamp(0.0, 1.0) * 255.0) as u8,
                    (g.clamp(0.0, 1.0) * 255.0) as u8,
                    (b.clamp(0.0, 1.0) * 255.0) as u8,
                    (a.clamp(0.0, 1.0) * 255.0) as u8,
                )
            }
            BoundValue::String(s) => s.clone(),
            BoundValue::Entity(Some(e)) => format!("{e:?}"),
            BoundValue::Entity(None) => "<none>".into(),
            BoundValue::Asset(Some(p)) => p.clone(),
            BoundValue::Asset(None) => "<unset>".into(),
            BoundValue::EnumTag(t) => t.clone(),
        }
    }
}
