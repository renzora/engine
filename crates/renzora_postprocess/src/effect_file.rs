//! Reading a post-process effect out of its shader file.
//!
//! An effect is one `.wesl` (or `.wgsl`) file. The shader declares its settings
//! as an ordinary uniform struct at `@group(0) @binding(2)`, and a comment after
//! each field says how the inspector should show it:
//!
//! ```wgsl
//! // @effect(name = "CRT", icon = "television-simple")
//!
//! struct Settings {
//!     curvature: f32,     // @slider(0, 1, default = 0.02, step = 0.001)
//!     mode: u32,          // @slider(0, 3, label = "Wipe Mode")
//!     tint_r: f32,        // @hidden(default = 1)
//!     time: f32,          // seconds, filled in by the engine
//! };
//! @group(0) @binding(2) var<uniform> settings: Settings;
//! ```
//!
//! The field list, types and byte offsets come from naga, the same compiler
//! wgpu runs, rather than from reading the struct as text. That is what lets
//! the engine pack a buffer that matches the shader without the author writing
//! padding by hand: a hand-maintained layout was the one part of the old
//! plugin effects nothing checked, and a mismatch rendered as silently wrong
//! values. Only the comments are read as text, because naga drops them.
//!
//! The same parse checks the shader against the bindings the effect pass
//! provides. Binding something else is a wgpu validation failure at pipeline
//! creation, which takes the renderer down; catching it here turns it into a
//! message on the effect.

use naga::{AddressSpace, ImageClass, ImageDimension, ScalarKind, ShaderStage, TypeInner};

/// What the effect pass binds, in order. An effect may use any subset.
const SCREEN: u32 = 0;
const SCREEN_SAMPLER: u32 = 1;
const SETTINGS: u32 = 2;
/// Bound only to an effect that declares them: a copy of the frame as it was
/// before the effect froze it, for transitions that blend two pictures.
const SNAPSHOT: u32 = 3;
const SNAPSHOT_SAMPLER: u32 = 4;

/// Wrap point for the engine-filled `time` field. A shader that hashes time
/// into noise loses float precision after a few hours of uptime, and a grain
/// pattern that repeats every 17 minutes is not visible.
pub const TIME_WRAP: f32 = 1024.0;

/// A parsed effect: what the inspector shows and what the renderer packs.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectInfo {
    pub name: String,
    /// Phosphor icon name for the inspector header.
    pub icon: String,
    /// The shader reads binding 3, a frozen copy of the frame.
    pub snapshot: bool,
    /// While this holds, the snapshot keeps the frame it last captured.
    pub freeze: Option<FreezeRule>,
    pub fields: Vec<EffectField>,
    /// Size of the settings buffer in bytes. Zero when the shader has none.
    pub uniform_size: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EffectField {
    pub name: String,
    /// What the inspector row says.
    pub label: String,
    pub kind: FieldKind,
    /// Byte offset in the settings buffer, from naga's layout.
    pub offset: u32,
    pub role: FieldRole,
    pub default: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    F32,
    I32,
    U32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FieldRole {
    /// Shown in the inspector.
    Slider { min: f32, max: f32, step: f32 },
    /// Kept at its default, never shown.
    Hidden,
    /// Seconds since startup, wrapped at [`TIME_WRAP`].
    Time,
    /// Always 1. Effects converted from the old plugins read it.
    Enabled,
}

/// `field op value`, for example `progress < 1`.
#[derive(Clone, Debug, PartialEq)]
pub struct FreezeRule {
    pub field: String,
    pub op: CompareOp,
    pub value: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompareOp {
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Equal,
    NotEqual,
}

/// Whether a shader file is meant as an effect: it carries the `@effect`
/// header. The contract crate's test, so the editor's drop targets and the
/// renderer agree on what is an effect.
pub fn is_effect(source: &str) -> bool {
    renzora::is_effect_source(source)
}

/// Parse an effect. `fallback_name` names it when the header does not, and
/// should be the file stem.
pub fn parse(source: &str, fallback_name: &str) -> Result<EffectInfo, String> {
    let header_args = match header(source) {
        Some(args) => parse_args(args)?,
        None => Vec::new(),
    };
    let module = naga::front::wgsl::parse_str(source).map_err(|e| {
        let message = e.message().to_string();
        match e.location(source) {
            Some(loc) => format!("line {}: {message}", loc.line_number),
            None => message,
        }
    })?;

    if !module
        .entry_points
        .iter()
        .any(|ep| ep.stage == ShaderStage::Fragment)
    {
        return Err("the shader has no @fragment entry point".into());
    }

    // Parsing only checks syntax. A type error inside a function (adding a
    // vec3 to a vec4) passes it, and would otherwise fail later in the
    // pipeline cache, where nothing tells the user: the effect just would not
    // draw. Validating here puts that message on the effect instead.
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    )
    .validate(&module)
    .map_err(|e| {
        let message = e.as_inner().to_string();
        match e.spans().next().and_then(|(span, _)| span.location(source).into()) {
            Some(loc) if loc.line_number > 0 => format!("line {}: {message}", loc.line_number),
            _ => message,
        }
    })?;

    let mut settings_struct = None;
    let mut snapshot = false;
    for (_, var) in module.global_variables.iter() {
        let Some(binding) = &var.binding else { continue };
        let what = var.name.as_deref().unwrap_or("a binding");
        if binding.group != 0 {
            return Err(format!(
                "`{what}` is in group {}, but effects only get group 0",
                binding.group
            ));
        }
        let inner = &module.types[var.ty].inner;
        match binding.binding {
            SCREEN | SNAPSHOT => {
                let ok = matches!(
                    inner,
                    TypeInner::Image {
                        dim: ImageDimension::D2,
                        arrayed: false,
                        class: ImageClass::Sampled {
                            kind: ScalarKind::Float,
                            multi: false
                        },
                    }
                );
                if !ok {
                    return Err(format!(
                        "binding {} (`{what}`) must be a texture_2d<f32>",
                        binding.binding
                    ));
                }
                snapshot |= binding.binding == SNAPSHOT;
            }
            SCREEN_SAMPLER | SNAPSHOT_SAMPLER => {
                if !matches!(inner, TypeInner::Sampler { comparison: false }) {
                    return Err(format!(
                        "binding {} (`{what}`) must be a sampler",
                        binding.binding
                    ));
                }
                snapshot |= binding.binding == SNAPSHOT_SAMPLER;
            }
            SETTINGS => {
                if var.space != AddressSpace::Uniform {
                    return Err(format!("binding 2 (`{what}`) must be var<uniform>"));
                }
                let TypeInner::Struct { members, span } = inner else {
                    return Err(format!("binding 2 (`{what}`) must be a struct"));
                };
                let struct_name = module.types[var.ty].name.clone().unwrap_or_default();
                settings_struct = Some((struct_name, members.clone(), *span));
            }
            other => {
                return Err(format!(
                    "binding {other} (`{what}`) is not one the effect pass provides; \
                     it binds 0 (the frame), 1 (its sampler), 2 (the settings) and \
                     3 and 4 (a frozen copy of the frame and its sampler)"
                ));
            }
        }
    }

    let mut fields = Vec::new();
    let mut uniform_size = 0;
    if let Some((struct_name, members, span)) = settings_struct {
        let comments = field_comments(source, &struct_name);
        for member in &members {
            let name = member.name.clone().unwrap_or_default();
            let kind = match module.types[member.ty].inner {
                TypeInner::Scalar(s) if s.width == 4 => match s.kind {
                    ScalarKind::Float => FieldKind::F32,
                    ScalarKind::Sint => FieldKind::I32,
                    ScalarKind::Uint => FieldKind::U32,
                    _ => return Err(unsupported_field(&name)),
                },
                _ => return Err(unsupported_field(&name)),
            };
            let comment = comments
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, c)| c.as_str())
                .unwrap_or("");
            fields.push(field(&name, kind, member.offset, comment)?);
        }
        // A uniform binding is sized in 16-byte steps.
        uniform_size = span.max(16).div_ceil(16) * 16;
    }

    let mut name = fallback_name.to_string();
    let mut icon = "sparkle".to_string();
    let mut freeze = None;
    for arg in header_args {
        match arg {
            Arg::Named(key, value) if key == "name" => name = value.text(),
            Arg::Named(key, value) if key == "icon" => icon = value.text(),
            Arg::Named(key, value) if key == "freeze_while" => {
                let rule = parse_freeze(&value.text())?;
                if !fields.iter().any(|f| f.name == rule.field) {
                    return Err(format!(
                        "freeze_while names `{}`, which is not a settings field",
                        rule.field
                    ));
                }
                freeze = Some(rule);
            }
            Arg::Named(key, _) | Arg::Flag(key) => {
                return Err(format!("@effect does not take `{key}`"));
            }
            Arg::Positional(_) => return Err("@effect takes only named settings".into()),
        }
    }

    Ok(EffectInfo {
        name,
        icon,
        snapshot,
        freeze,
        fields,
        uniform_size,
    })
}

impl EffectInfo {
    /// The settings buffer for one effect instance.
    pub fn pack(&self, effect: &renzora::PostEffect, time: f32) -> Vec<u8> {
        let mut bytes = vec![0u8; self.uniform_size as usize];
        for f in &self.fields {
            let value = match f.role {
                FieldRole::Time => time,
                FieldRole::Enabled => 1.0,
                FieldRole::Hidden => f.default,
                FieldRole::Slider { .. } => effect.value(&f.name).unwrap_or(f.default),
            };
            let word = match f.kind {
                FieldKind::F32 => value.to_le_bytes(),
                FieldKind::I32 => (value.round() as i32).to_le_bytes(),
                FieldKind::U32 => (value.round().max(0.0) as u32).to_le_bytes(),
            };
            let at = f.offset as usize;
            bytes[at..at + 4].copy_from_slice(&word);
        }
        bytes
    }

    /// Whether the snapshot should hold its frame this frame.
    pub fn frozen(&self, effect: &renzora::PostEffect) -> bool {
        let Some(rule) = &self.freeze else {
            return false;
        };
        let default = self
            .fields
            .iter()
            .find(|f| f.name == rule.field)
            .map(|f| f.default)
            .unwrap_or(0.0);
        let v = effect.value(&rule.field).unwrap_or(default);
        match rule.op {
            CompareOp::Less => v < rule.value,
            CompareOp::LessEqual => v <= rule.value,
            CompareOp::Greater => v > rule.value,
            CompareOp::GreaterEqual => v >= rule.value,
            CompareOp::Equal => v == rule.value,
            CompareOp::NotEqual => v != rule.value,
        }
    }

    /// The fields the inspector shows.
    pub fn sliders(&self) -> impl Iterator<Item = &EffectField> {
        self.fields
            .iter()
            .filter(|f| matches!(f.role, FieldRole::Slider { .. }))
    }
}

fn unsupported_field(name: &str) -> String {
    format!("settings field `{name}` must be an f32, i32 or u32")
}

/// Build one field from its name, type and comment.
fn field(name: &str, kind: FieldKind, offset: u32, comment: &str) -> Result<EffectField, String> {
    let int = kind != FieldKind::F32;
    let mut field = EffectField {
        name: name.to_string(),
        label: title_case(name),
        kind,
        offset,
        role: FieldRole::Slider {
            min: f32::MIN,
            max: f32::MAX,
            step: if int { 1.0 } else { 0.01 },
        },
        default: 0.0,
    };

    // Names the engine owns, unless a comment says otherwise.
    match name {
        "time" => field.role = FieldRole::Time,
        "enabled" => field.role = FieldRole::Enabled,
        _ if name.starts_with('_') => field.role = FieldRole::Hidden,
        _ => {}
    }

    let Some((tag, args)) = annotation(comment) else {
        return Ok(field);
    };
    let args = parse_args(args)?;
    match tag {
        "slider" => {
            let mut numbers = Vec::new();
            let mut step = if int { 1.0 } else { 0.01 };
            for arg in args {
                match arg {
                    Arg::Positional(v) => numbers.push(v.number(name)?),
                    Arg::Named(k, v) if k == "default" => field.default = v.number(name)?,
                    Arg::Named(k, v) if k == "step" => step = v.number(name)?,
                    Arg::Named(k, v) if k == "label" => field.label = v.text(),
                    Arg::Named(k, _) | Arg::Flag(k) => {
                        return Err(format!("@slider on `{name}` does not take `{k}`"))
                    }
                }
            }
            let [min, max] = numbers[..] else {
                return Err(format!(
                    "@slider on `{name}` needs a minimum and a maximum, like @slider(0, 1)"
                ));
            };
            if min > max {
                return Err(format!("@slider on `{name}` has its minimum above its maximum"));
            }
            field.role = FieldRole::Slider { min, max, step };
            if !(min..=max).contains(&field.default) {
                field.default = field.default.clamp(min, max);
            }
        }
        "hidden" => {
            field.role = FieldRole::Hidden;
            for arg in args {
                match arg {
                    Arg::Named(k, v) if k == "default" => field.default = v.number(name)?,
                    Arg::Named(k, _) | Arg::Flag(k) => {
                        return Err(format!("@hidden on `{name}` does not take `{k}`"))
                    }
                    Arg::Positional(_) => {
                        return Err(format!("@hidden on `{name}` takes only `default = ...`"))
                    }
                }
            }
        }
        other => return Err(format!("`{name}` has an unknown annotation @{other}")),
    }
    Ok(field)
}

/// The arguments of the `// @effect(...)` header, or `None` when the file has
/// no header. `// @effect` with no parentheses is a header with no arguments.
fn header(source: &str) -> Option<&str> {
    source.lines().find_map(|line| {
        let comment = line.trim_start().strip_prefix("//")?;
        match annotation(comment)? {
            ("effect", args) => Some(args),
            _ => None,
        }
    })
}

/// `@tag(args)` at the start of a comment, as `(tag, args)`.
fn annotation(comment: &str) -> Option<(&str, &str)> {
    let rest = comment.trim().strip_prefix('@')?;
    let tag_end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    let (tag, after) = rest.split_at(tag_end);
    if tag.is_empty() {
        return None;
    }
    let after = after.trim_start();
    let args = match after.strip_prefix('(') {
        Some(inner) => inner.rfind(')').map(|end| &inner[..end])?,
        None => "",
    };
    Some((tag, args))
}

/// Each settings field's trailing comment, by field name, from the text of
/// `struct <name> { ... }`.
fn field_comments(source: &str, struct_name: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Some(start) = find_struct(source, struct_name) else {
        return out;
    };
    let body = &source[start..];
    let Some(open) = body.find('{') else {
        return out;
    };
    let body = &body[open + 1..];
    let body = &body[..body.find('}').unwrap_or(body.len())];
    for line in body.lines() {
        let (code, comment) = match line.find("//") {
            Some(i) => (&line[..i], &line[i + 2..]),
            None => continue,
        };
        let Some(colon) = code.find(':') else { continue };
        // `@align(16) name: f32` names the field after any attributes.
        let name = code[..colon].split_whitespace().last().unwrap_or("");
        if !name.is_empty() {
            out.push((name.to_string(), comment.to_string()));
        }
    }
    out
}

/// Byte offset of `struct <name>` as a whole word.
fn find_struct(source: &str, name: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(i) = source[from..].find("struct") {
        let at = from + i;
        let rest = source[at + "struct".len()..].trim_start();
        let word_end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        if &rest[..word_end] == name {
            return Some(at);
        }
        from = at + "struct".len();
    }
    None
}

#[derive(Debug, PartialEq)]
enum Arg {
    Positional(Value),
    Named(String, Value),
    Flag(String),
}

#[derive(Debug, PartialEq)]
enum Value {
    Text(String),
    Bare(String),
}

impl Value {
    fn text(&self) -> String {
        match self {
            Value::Text(s) | Value::Bare(s) => s.clone(),
        }
    }

    fn number(&self, field: &str) -> Result<f32, String> {
        let s = match self {
            Value::Bare(s) => s,
            Value::Text(s) => s,
        };
        s.trim()
            .parse::<f32>()
            .map_err(|_| format!("`{s}` on `{field}` is not a number"))
    }
}

/// Split `0, 1, default = 0.5, label = "Wipe, Mode"` into arguments. Commas
/// inside quotes do not split.
fn parse_args(args: &str) -> Result<Vec<Arg>, String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for c in args.chars() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                current.push(c);
            }
            ',' if !in_quotes => parts.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    if in_quotes {
        return Err(format!("unclosed quote in `{args}`"));
    }
    parts.push(current);

    let mut out = Vec::new();
    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let value = |s: &str| {
            let s = s.trim();
            match s.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
                Some(inner) => Value::Text(inner.to_string()),
                None => Value::Bare(s.to_string()),
            }
        };
        match part.split_once('=') {
            Some((key, v)) => out.push(Arg::Named(key.trim().to_string(), value(v))),
            None if part.starts_with(|c: char| c.is_ascii_alphabetic()) => {
                out.push(Arg::Flag(part.to_string()))
            }
            None => out.push(Arg::Positional(value(part))),
        }
    }
    Ok(out)
}

fn parse_freeze(rule: &str) -> Result<FreezeRule, String> {
    const OPS: [(&str, CompareOp); 6] = [
        ("<=", CompareOp::LessEqual),
        (">=", CompareOp::GreaterEqual),
        ("==", CompareOp::Equal),
        ("!=", CompareOp::NotEqual),
        ("<", CompareOp::Less),
        (">", CompareOp::Greater),
    ];
    for (token, op) in OPS {
        if let Some((field, value)) = rule.split_once(token) {
            let value = value
                .trim()
                .parse::<f32>()
                .map_err(|_| format!("freeze_while `{rule}` must compare a field to a number"))?;
            return Ok(FreezeRule {
                field: field.trim().to_string(),
                op,
                value,
            });
        }
    }
    Err(format!(
        "freeze_while `{rule}` must look like `progress < 1`"
    ))
}

/// `scanline_intensity` -> `Scanline Intensity`.
pub fn title_case(name: &str) -> String {
    name.split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(first) => first.to_uppercase().chain(c).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora::PostEffect;

    const CRT: &str = r#"
// @effect(name = "CRT", icon = "television-simple")
@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

struct Settings {
    curvature: f32,        // @slider(0, 1, default = 0.02, step = 0.001)
    mode: u32,             // @slider(0, 3, label = "Wipe Mode")
    tint: f32,             // @hidden(default = 0.5)
    time: f32,
    enabled: f32,
    unannotated: f32,
};
@group(0) @binding(2) var<uniform> settings: Settings;

@fragment
fn fragment(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    return textureSample(screen_texture, texture_sampler, uv) * settings.tint;
}
"#;

    #[test]
    fn the_header_and_every_field_annotation_are_read() {
        let info = parse(CRT, "crt").unwrap();
        assert_eq!(info.name, "CRT");
        assert_eq!(info.icon, "television-simple");
        assert!(!info.snapshot);
        let names: Vec<&str> = info.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["curvature", "mode", "tint", "time", "enabled", "unannotated"]);

        let curvature = &info.fields[0];
        assert_eq!(curvature.default, 0.02);
        assert_eq!(
            curvature.role,
            FieldRole::Slider { min: 0.0, max: 1.0, step: 0.001 }
        );
        let mode = &info.fields[1];
        assert_eq!(mode.kind, FieldKind::U32);
        assert_eq!(mode.label, "Wipe Mode");
        assert_eq!(mode.role, FieldRole::Slider { min: 0.0, max: 3.0, step: 1.0 });
        assert_eq!(info.fields[2].role, FieldRole::Hidden);
        assert_eq!(info.fields[3].role, FieldRole::Time);
        assert_eq!(info.fields[4].role, FieldRole::Enabled);
        assert_eq!(info.fields[5].label, "Unannotated");
        assert_eq!(info.sliders().count(), 3);
    }

    #[test]
    fn values_are_packed_at_naga_offsets_with_defaults_filling_gaps() {
        let info = parse(CRT, "crt").unwrap();
        // Six 4-byte fields round up to two 16-byte rows.
        assert_eq!(info.uniform_size, 32);

        let mut effect = PostEffect::new("crt.wesl");
        effect.set_value("mode", 2.4);
        let bytes = info.pack(&effect, 7.5);
        let word = |i: usize| <[u8; 4]>::try_from(&bytes[i * 4..i * 4 + 4]).unwrap();
        assert_eq!(f32::from_le_bytes(word(0)), 0.02, "unset slider takes its default");
        assert_eq!(u32::from_le_bytes(word(1)), 2, "an integer field rounds");
        assert_eq!(f32::from_le_bytes(word(2)), 0.5, "hidden keeps its default");
        assert_eq!(f32::from_le_bytes(word(3)), 7.5, "time is the engine's");
        assert_eq!(f32::from_le_bytes(word(4)), 1.0, "enabled is always on");
    }

    #[test]
    fn a_snapshot_effect_is_recognised_from_its_bindings_and_freezes_by_rule() {
        let src = r#"
// @effect(name = "Wipe", freeze_while = "progress < 1")
@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;
struct Settings { progress: f32, };  // unused
@group(0) @binding(2) var<uniform> settings: Settings;
@group(0) @binding(3) var frozen: texture_2d<f32>;
@group(0) @binding(4) var frozen_sampler: sampler;
@fragment
fn fragment(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let a = textureSample(frozen, frozen_sampler, uv);
    let b = textureSample(screen_texture, screen_sampler, uv);
    return mix(a, b, settings.progress);
}
"#;
        let info = parse(src, "wipe").unwrap();
        assert!(info.snapshot);
        let mut effect = PostEffect::new("wipe.wesl");
        effect.set_value("progress", 0.3);
        assert!(info.frozen(&effect));
        effect.set_value("progress", 1.0);
        assert!(!info.frozen(&effect));
    }

    #[test]
    fn a_binding_the_pass_does_not_provide_is_refused_by_name() {
        let src = r#"
@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(7) var lut: texture_2d<f32>;
@fragment
fn fragment() -> @location(0) vec4<f32> { return vec4(1.0); }
"#;
        let err = parse(src, "x").unwrap_err();
        assert!(err.contains("binding 7") && err.contains("lut"), "{err}");
    }

    #[test]
    fn a_vector_settings_field_is_refused_rather_than_misread() {
        let src = r#"
struct Settings { color: vec3<f32>, };
@group(0) @binding(2) var<uniform> settings: Settings;
@fragment
fn fragment() -> @location(0) vec4<f32> { return vec4(settings.color, 1.0); }
"#;
        assert!(parse(src, "x").unwrap_err().contains("`color`"));
    }

    #[test]
    fn a_type_error_inside_a_function_is_caught_not_left_to_the_gpu() {
        let src = r#"
// @effect
@fragment
fn fragment() -> @location(0) vec4<f32> {
    let a = vec3(1.0);
    return a + vec4(1.0);
}
"#;
        assert!(parse(src, "x").is_err());
    }

    #[test]
    fn a_syntax_error_reports_its_line() {
        let err = parse("@fragment\nfn fragment( -> f32 {}", "x").unwrap_err();
        assert!(err.starts_with("line 2"), "{err}");
    }

    #[test]
    fn a_shader_without_the_header_is_not_offered_as_an_effect() {
        assert!(is_effect(CRT));
        assert!(!is_effect("@fragment fn fragment() -> @location(0) vec4<f32> { return vec4(1.0); }"));
        assert!(is_effect("// @effect\n"));
    }

    #[test]
    fn bad_annotations_name_the_field() {
        let src = |ann: &str| {
            format!(
                "struct S {{ amount: f32, // {ann}\n}};\n@group(0) @binding(2) var<uniform> s: S;\n\
                 @fragment fn fragment() -> @location(0) vec4<f32> {{ return vec4(s.amount); }}"
            )
        };
        assert!(parse(&src("@slider(1)"), "x").unwrap_err().contains("minimum and a maximum"));
        assert!(parse(&src("@slider(2, 1)"), "x").unwrap_err().contains("above its maximum"));
        assert!(parse(&src("@sldier(0, 1)"), "x").unwrap_err().contains("@sldier"));
        assert!(parse(&src("@slider(0, 1, defualt = 2)"), "x").unwrap_err().contains("defualt"));
    }
}
