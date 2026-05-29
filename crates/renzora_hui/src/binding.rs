//! Reactive data binding for markup text — `{{ Component.field }}`.
//!
//! A `<text>` whose content contains a `{{ ... }}` token gets a [`TextBinding`]
//! stamped by the loader (see `loader::apply_xnode_to`). Each frame,
//! [`update_text_bindings`] re-resolves every binding against the live ECS via
//! reflection and rewrites the `Text` when the rendered string changes.
//!
//! Path grammar (first segment decides the entity):
//!   * `{{ Sun.azimuth }}`            — `Sun` is a registered component →
//!     read the component on the **host entity** (the one holding the
//!     `HtmlTemplatePath` this tree was built from).
//!   * `{{ Environment.Sun.azimuth }}` — `Environment` is **not** a component
//!     type → treat it as an entity `Name`, then `Sun.azimuth` on that entity.
//!
//! No Lua, no view-model store: the binding reads whatever a normal Bevy
//! system / physics / script last wrote into the component.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::reflect::{GetPath, PartialReflect, TypeRegistry};
use bevy::ui::Display;

/// Attached to a `<text>` entity whose content holds `{{ ... }}` tokens.
#[derive(Component)]
pub struct TextBinding {
    /// The text template with `{{ path }}` tokens still in place,
    /// e.g. `"Azimuth: {{ Sun.azimuth }}"`.
    template: String,
    /// Entity whose components bare (component-first) paths resolve against.
    source: Entity,
    /// Last rendered string — skip the `Text` write when unchanged.
    last: String,
}

impl TextBinding {
    pub fn new(template: String, source: Entity) -> Self {
        Self {
            template,
            source,
            last: String::new(),
        }
    }
}

/// Re-render every text binding each frame. Exclusive system because it needs
/// reflection reads of arbitrary components (immutable world) followed by
/// `Text` writes (mutable world).
pub fn update_text_bindings(world: &mut World) {
    // Snapshot the bindings up front so we're not holding a query borrow while
    // doing reflection reads + writes.
    let mut binding_q = world.query::<(Entity, &TextBinding)>();
    let bindings: Vec<(Entity, String, Entity)> = binding_q
        .iter(world)
        .map(|(e, b)| (e, b.template.clone(), b.source))
        .collect();
    if bindings.is_empty() {
        return;
    }

    // Name → entity map for cross-entity paths (`{{ Environment.Sun.x }}`).
    let mut names: HashMap<String, Entity> = HashMap::default();
    {
        let mut name_q = world.query::<(Entity, &Name)>();
        for (e, n) in name_q.iter(world) {
            names.insert(n.as_str().to_string(), e);
        }
    }

    // Clone the registry handle (Arc) so its read-guard doesn't borrow `world`
    // — leaves us free to read entities reflectively in the same scope.
    let type_registry = world.resource::<AppTypeRegistry>().clone();

    let mut updates: Vec<(Entity, String)> = Vec::new();
    {
        let registry = type_registry.read();
        for (text_ent, template, source) in &bindings {
            let rendered = render_template(world, &registry, &names, *source, template);
            updates.push((*text_ent, rendered));
        }
    }

    for (text_ent, rendered) in updates {
        // Skip the write when nothing changed (also updates `last`).
        let changed = match world.get_mut::<TextBinding>(text_ent) {
            Some(mut b) if b.last != rendered => {
                b.last = rendered.clone();
                true
            }
            Some(_) => false,
            None => false,
        };
        if changed {
            if let Some(mut text) = world.get_mut::<Text>(text_ent) {
                text.0 = rendered;
            }
        }
    }
}

/// Replace every `{{ path }}` token in `template` with its resolved value.
/// Unresolved tokens are left verbatim so authors can see what failed to bind.
fn render_template(
    world: &World,
    registry: &TypeRegistry,
    names: &HashMap<String, Entity>,
    host: Entity,
    template: &str,
) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else {
            // No closing — emit the rest verbatim and stop.
            out.push_str("{{");
            rest = after;
            continue;
        };
        let path = after[..end].trim();
        match resolve_path(world, registry, names, host, path) {
            Some(v) => out.push_str(&v),
            None => {
                // Leave the literal token visible for debugging.
                out.push_str("{{ ");
                out.push_str(path);
                out.push_str(" }}");
            }
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

/// Resolve one `Component.field…` or `Entity.Component.field…` path to a
/// display string, reading the live component via reflection.
fn resolve_path(
    world: &World,
    registry: &TypeRegistry,
    names: &HashMap<String, Entity>,
    host: Entity,
    path: &str,
) -> Option<String> {
    let segments: Vec<&str> = path.split('.').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return None;
    }

    // Bare single segment (`{{ _time }}`) → a script variable on the host.
    if segments.len() == 1 {
        return resolve_script_var(world, host, segments[0]);
    }

    // Decide whether segment 0 is a component type (→ host entity) or an
    // entity name (→ that entity, with segment 1 as the member).
    let first_is_component = registry
        .get_with_short_type_path(segments[0])
        .and_then(|r| r.data::<ReflectComponent>())
        .is_some();

    let (entity, member, field_segments): (Entity, &str, &[&str]) = if first_is_component {
        (host, segments[0], &segments[1..])
    } else {
        (*names.get(segments[0])?, segments[1], &segments[2..])
    };

    // `member` is either a component type (→ reflect its field) or, failing
    // that, a script variable on the entity. Components win.
    if let Some(reflect_component) = registry
        .get_with_short_type_path(member)
        .and_then(|r| r.data::<ReflectComponent>())
    {
        // For a bare component path (`{{ Sun.azimuth }}`) resolve against
        // `entity`, walking UP the `ChildOf` chain if it doesn't have the
        // component — so a UI subtree under a game entity reads its ancestor's
        // component. Named-entity paths don't walk (the author named it).
        let mut current = entity;
        let reflected = loop {
            if let Ok(entity_ref) = world.get_entity(current) {
                if let Some(r) = reflect_component.reflect(entity_ref) {
                    break Some(r);
                }
            }
            if !first_is_component {
                break None;
            }
            match world.get::<ChildOf>(current) {
                Some(parent) => current = parent.parent(),
                None => break None,
            }
        };
        let reflected = reflected?;
        let value: &dyn PartialReflect = if field_segments.is_empty() {
            reflected.as_partial_reflect()
        } else {
            reflected
                .reflect_path(format!(".{}", field_segments.join(".")).as_str())
                .ok()?
        };
        return Some(format_reflect(value));
    }

    // Not a component — try `member` as a script variable on the entity.
    resolve_script_var(world, entity, member)
}

/// Read a live script variable off an entity's `ScriptComponent`. Scans every
/// attached script's variables (read back from the Lua VM each frame), so a
/// `props()`-declared value like `_time` shows its current runtime value.
fn resolve_script_var(world: &World, entity: Entity, name: &str) -> Option<String> {
    let sc = world.get::<renzora_scripting::ScriptComponent>(entity)?;
    for entry in &sc.scripts {
        if let Some(value) = entry.variables.get(name) {
            return Some(format_script_value(value));
        }
    }
    None
}

/// Display formatting for a script variable value.
fn format_script_value(value: &renzora_scripting::ScriptValue) -> String {
    use renzora_scripting::ScriptValue as V;
    match value {
        V::Float(f) => trim_float(*f as f64),
        V::Int(i) => i.to_string(),
        V::Bool(b) => b.to_string(),
        V::String(s) => s.clone(),
        V::Entity(e) => e.clone(),
        V::Vec2(v) => format!("{}, {}", trim_float(v.x as f64), trim_float(v.y as f64)),
        V::Vec3(v) => format!(
            "{}, {}, {}",
            trim_float(v.x as f64),
            trim_float(v.y as f64),
            trim_float(v.z as f64)
        ),
        V::Color(c) => format!(
            "{}, {}, {}, {}",
            trim_float(c.x as f64),
            trim_float(c.y as f64),
            trim_float(c.z as f64),
            trim_float(c.w as f64)
        ),
    }
}

/// Format a reflected scalar for display. Floats are trimmed of trailing
/// zeros; everything else falls back to its `Debug`.
fn format_reflect(value: &dyn PartialReflect) -> String {
    if let Some(v) = value.try_downcast_ref::<f32>() {
        return trim_float(*v as f64);
    }
    if let Some(v) = value.try_downcast_ref::<f64>() {
        return trim_float(*v);
    }
    if let Some(v) = value.try_downcast_ref::<i32>() {
        return v.to_string();
    }
    if let Some(v) = value.try_downcast_ref::<i64>() {
        return v.to_string();
    }
    if let Some(v) = value.try_downcast_ref::<u32>() {
        return v.to_string();
    }
    if let Some(v) = value.try_downcast_ref::<u64>() {
        return v.to_string();
    }
    if let Some(v) = value.try_downcast_ref::<usize>() {
        return v.to_string();
    }
    if let Some(v) = value.try_downcast_ref::<bool>() {
        return v.to_string();
    }
    if let Some(v) = value.try_downcast_ref::<String>() {
        return v.clone();
    }
    if let Some(v) = value.try_downcast_ref::<&'static str>() {
        return v.to_string();
    }
    // Fallback: best-effort debug of the dynamic value.
    format!("{value:?}")
}

/// `12.0` → `"12"`, `12.34` → `"12.34"`. Two decimals max, no trailing zeros.
fn trim_float(n: f64) -> String {
    if (n.fract()).abs() < f64::EPSILON {
        format!("{}", n as i64)
    } else {
        let s = format!("{n:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

// ── Conditional visibility — `show="{{ cond }}"` ────────────────────────────

/// Attached to an entity with a `show="..."` attribute. Toggles the node's
/// `Display` between its authored value and `Display::None` based on whether
/// the (possibly `{{ }}`-bound) condition is truthy.
#[derive(Component)]
pub struct ShowBinding {
    /// Raw attribute value, e.g. `"{{ Player.Stats.is_admin }}"` or `"true"`.
    expr: String,
    /// Host entity for bare component paths inside `{{ }}`.
    source: Entity,
    /// The node's `Display` when shown (its authored value), restored on true.
    display_when_shown: Display,
    /// Last applied state — skip the write when unchanged.
    last: Option<bool>,
}

impl ShowBinding {
    pub fn new(expr: String, source: Entity, display_when_shown: Display) -> Self {
        Self {
            expr,
            source,
            display_when_shown,
            last: None,
        }
    }
}

/// A rendered condition string is truthy unless it's empty, `false`, or a
/// number equal to zero. Covers bound bools (`true`/`false`), numbers
/// (`0` → false), and plain strings (non-empty → true).
fn truthy(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("false") {
        return false;
    }
    if let Ok(n) = t.parse::<f64>() {
        return n != 0.0;
    }
    true
}

/// Evaluate a `show=` condition to a bool. Supports a single comparison
/// (`<`, `>`, `<=`, `>=`, `==`, `!=`) between two operands, or a bare operand
/// (truthy test). Operands are either a `{{ }}`-free binding path
/// (`Player.Health.current`), a number, a `"quoted"` / bare string literal, or
/// `true`/`false`. No boolean combinators (`and`/`or`) yet — one comparison.
///
/// `raw` is the attribute value, e.g. `"{{ Player.Health.current < 20 }}"` or
/// a literal `"true"`. A single `{{ }}` wrapper is stripped; the *inside* is
/// the expression (so the comparison operator isn't mistaken for markup).
fn eval_condition(
    world: &World,
    registry: &TypeRegistry,
    names: &HashMap<String, Entity>,
    host: Entity,
    raw: &str,
) -> bool {
    let inner = strip_binding_wrapper(raw);

    // Two-char operators first so `<=` isn't read as `<`.
    const OPS: &[&str] = &["==", "!=", "<=", ">=", "<", ">"];
    for op in OPS {
        if let Some(idx) = find_top_level_op(inner, op) {
            let lhs = resolve_operand(world, registry, names, host, inner[..idx].trim());
            let rhs = resolve_operand(world, registry, names, host, inner[idx + op.len()..].trim());
            return compare(&lhs, &rhs, op);
        }
    }
    // No operator → truthy test of the single resolved operand.
    truthy(&resolve_operand(world, registry, names, host, inner.trim()))
}

/// Strip a single `{{ ... }}` wrapper, returning the inner expression. If
/// there's no wrapper (a literal like `true`), returns the input unchanged.
fn strip_binding_wrapper(raw: &str) -> &str {
    let t = raw.trim();
    if let Some(rest) = t.strip_prefix("{{") {
        if let Some(inner) = rest.strip_suffix("}}") {
            return inner.trim();
        }
    }
    t
}

/// Find an operator outside of any quoted string. Returns its byte index.
fn find_top_level_op(s: &str, op: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let op_bytes = op.as_bytes();
    let mut in_quote = false;
    let mut i = 0;
    while i + op_bytes.len() <= bytes.len() {
        let c = bytes[i];
        if c == b'"' {
            in_quote = !in_quote;
            i += 1;
            continue;
        }
        if !in_quote && &bytes[i..i + op_bytes.len()] == op_bytes {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Resolve one operand to a display string: a `"quoted"` literal (quotes
/// stripped), a bare number / `true` / `false` literal (returned as-is), or a
/// component/entity binding path resolved against the world.
fn resolve_operand(
    world: &World,
    registry: &TypeRegistry,
    names: &HashMap<String, Entity>,
    host: Entity,
    operand: &str,
) -> String {
    let o = operand.trim();
    if let Some(inner) = o.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        return inner.to_string();
    }
    if o.parse::<f64>().is_ok() || o.eq_ignore_ascii_case("true") || o.eq_ignore_ascii_case("false")
    {
        return o.to_string();
    }
    // Otherwise treat as a binding path; fall back to the literal if unresolved.
    resolve_path(world, registry, names, host, o).unwrap_or_else(|| o.to_string())
}

/// Compare two resolved operand strings. Numeric when both parse as numbers;
/// otherwise string equality for `==`/`!=` (ordering ops require numbers).
fn compare(lhs: &str, rhs: &str, op: &str) -> bool {
    match (lhs.trim().parse::<f64>(), rhs.trim().parse::<f64>()) {
        (Ok(l), Ok(r)) => match op {
            "==" => l == r,
            "!=" => l != r,
            "<" => l < r,
            ">" => l > r,
            "<=" => l <= r,
            ">=" => l >= r,
            _ => false,
        },
        _ => match op {
            "==" => lhs.eq_ignore_ascii_case(rhs),
            "!=" => !lhs.eq_ignore_ascii_case(rhs),
            // Ordering on non-numeric operands isn't meaningful — treat false.
            _ => false,
        },
    }
}

/// Evaluate every `show` condition each frame and toggle `Node.display`.
pub fn update_show_bindings(world: &mut World) {
    let mut binding_q = world.query::<(Entity, &ShowBinding)>();
    let bindings: Vec<(Entity, String, Entity)> = binding_q
        .iter(world)
        .map(|(e, b)| (e, b.expr.clone(), b.source))
        .collect();
    if bindings.is_empty() {
        return;
    }

    let mut names: HashMap<String, Entity> = HashMap::default();
    {
        let mut name_q = world.query::<(Entity, &Name)>();
        for (e, n) in name_q.iter(world) {
            names.insert(n.as_str().to_string(), e);
        }
    }

    let type_registry = world.resource::<AppTypeRegistry>().clone();
    let mut updates: Vec<(Entity, bool)> = Vec::new();
    {
        let registry = type_registry.read();
        for (ent, expr, source) in &bindings {
            updates.push((*ent, eval_condition(world, &registry, &names, *source, expr)));
        }
    }

    for (ent, shown) in updates {
        let (changed, display_shown) = match world.get_mut::<ShowBinding>(ent) {
            Some(mut b) if b.last != Some(shown) => {
                b.last = Some(shown);
                (true, b.display_when_shown)
            }
            _ => (false, Display::Flex),
        };
        if changed {
            if let Some(mut node) = world.get_mut::<Node>(ent) {
                node.display = if shown { display_shown } else { Display::None };
            }
        }
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, (update_text_bindings, update_show_bindings));
}
