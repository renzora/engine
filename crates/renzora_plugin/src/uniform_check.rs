//! Cross-check a post-process component's `#[repr(C)]` layout against the WGSL
//! uniform block it is uploaded into.
//!
//! ## The bug this exists to catch
//!
//! A post-process plugin is a Rust struct and a shader that must agree on a byte
//! layout nothing enforces. The host copies the component's bytes straight into
//! a uniform buffer and the shader reads them back by offset — so adding a field
//! to one side, reordering two fields, or deleting a field from the shader
//! produces no error anywhere. It produces a *wrong picture*: every field from
//! the mismatch onward reads its neighbour's value. `curvature` reads
//! `chromatic_amount`, the effect looks subtly off, and the only way to find it
//! is to notice and go looking.
//!
//! That is the single most likely defect in ~59 near-identical plugins, and it
//! is entirely mechanical to check, because both sides are in the same crate:
//! the descriptor the `#[derive(Component)]` macro generates, and the `&'static
//! str` of WGSL the plugin `include_str!`s.
//!
//! ## What is and is not checked
//!
//! [`check_uniform`] parses the named WGSL struct, computes each field's offset
//! under WGSL's uniform address-space layout rules, and compares:
//!
//! - **total size** — the Rust struct's size against the WGSL struct's, which
//!   catches a field added or removed on either side;
//! - **per-field offsets** — for every *inspectable* field, that the WGSL struct
//!   has a field of the same name at the same byte offset, which catches a
//!   reorder or a type change.
//!
//! Fields marked `#[field(skip)]` are absent from the descriptor entirely, so
//! they cannot be matched by name. They are still covered by the size check —
//! `grayscale`'s three skipped luminance weights are three quarters of its
//! uniform, and dropping one would move the total.
//!
//! `alloc`-only, so it works from a `#![no_std]` plugin's test binary.

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::sys::{ComponentDesc, FieldKind};

/// One field parsed out of a WGSL struct declaration.
#[derive(Debug)]
struct WgslField {
    name: String,
    offset: usize,
    size: usize,
}

/// Size and alignment of a WGSL type in the uniform address space.
///
/// Only the types that actually appear in this workspace's effect uniforms are
/// handled; anything else is reported rather than guessed, because silently
/// assuming a size would turn this check into a source of false confidence.
fn wgsl_type_layout(ty: &str) -> Option<(usize, usize)> {
    Some(match ty {
        "f32" | "i32" | "u32" => (4, 4),
        "vec2<f32>" | "vec2<i32>" | "vec2<u32>" => (8, 8),
        // A vec3 occupies 12 bytes but aligns to 16 — the classic source of a
        // layout mismatch, and the reason this table is explicit.
        "vec3<f32>" | "vec3<i32>" | "vec3<u32>" => (12, 16),
        "vec4<f32>" | "vec4<i32>" | "vec4<u32>" => (16, 16),
        "mat4x4<f32>" => (64, 16),
        _ => return None,
    })
}

fn align_up(offset: usize, align: usize) -> usize {
    offset.div_ceil(align) * align
}

/// Parse `struct <name> { ... }` out of a WGSL source, returning its fields with
/// uniform-layout offsets and the struct's total (padded) size.
fn parse_wgsl_struct(wgsl: &str, struct_name: &str) -> Result<(Vec<WgslField>, usize), String> {
    let header = format!("struct {struct_name}");
    // A plain `find` would match `struct DemoSettings` when asked for
    // `struct Demo`, and then happily validate the wrong struct — a false pass,
    // which is the one outcome worse than no check at all. Require the name to
    // end at a boundary.
    let start = wgsl
        .match_indices(&header)
        .find(|(i, _)| {
            let rest = &wgsl[i + header.len()..];
            rest.chars()
                .next()
                .is_none_or(|c| c.is_whitespace() || c == '{')
        })
        .map(|(i, _)| i)
        .ok_or_else(|| format!("no `struct {struct_name}` in the shader"))?;
    let body_start = wgsl[start..]
        .find('{')
        .ok_or_else(|| format!("`struct {struct_name}` has no body"))?
        + start
        + 1;
    let body_end = wgsl[body_start..]
        .find('}')
        .ok_or_else(|| format!("`struct {struct_name}` is unterminated"))?
        + body_start;

    let mut fields = Vec::new();
    let mut offset = 0usize;
    let mut max_align = 1usize;

    for entry in wgsl[body_start..body_end].split(',') {
        // Drop `//` comments and any `@align(..)`-style attributes, then split
        // `name: type`.
        let entry = entry
            .lines()
            .map(|l| l.split("//").next().unwrap_or("").trim())
            .filter(|l| !l.is_empty() && !l.starts_with('@'))
            .collect::<Vec<_>>()
            .join(" ");
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let (name, ty) = entry
            .split_once(':')
            .ok_or_else(|| format!("cannot parse WGSL field `{entry}`"))?;
        let (name, ty) = (name.trim(), ty.trim());

        let (size, align) = wgsl_type_layout(ty)
            .ok_or_else(|| format!("unhandled WGSL type `{ty}` on field `{name}`"))?;
        offset = align_up(offset, align);
        max_align = max_align.max(align);
        fields.push(WgslField {
            name: String::from(name),
            offset,
            size,
        });
        offset += size;
    }

    Ok((fields, align_up(offset, max_align)))
}

/// Expected size in bytes of a descriptor field.
fn field_kind_size(kind: FieldKind) -> Option<usize> {
    Some(match kind {
        FieldKind::F32 | FieldKind::I32 => 4,
        FieldKind::Bool => 1,
        FieldKind::Vec3 => 12,
        FieldKind::Quat => 16,
        _ => return None,
    })
}

/// Compare a component descriptor against a WGSL uniform struct.
///
/// Returns every mismatch found rather than the first, so a reorder reports as
/// the handful of moved fields it is instead of one confusing line.
///
/// # Safety
///
/// `desc` must be a descriptor produced by `#[derive(Component)]` in this
/// process — its `name`/`fields` pointers must still be live. That is always
/// true of `T::descriptor()`, which is the only way callers obtain one.
pub unsafe fn check_uniform(
    desc: &ComponentDesc,
    wgsl: &str,
    struct_name: &str,
) -> Result<(), Vec<String>> {
    let (wgsl_fields, wgsl_size) = match parse_wgsl_struct(wgsl, struct_name) {
        Ok(parsed) => parsed,
        Err(e) => return Err(alloc::vec![e]),
    };

    let mut problems = Vec::new();

    if desc.size != wgsl_size {
        problems.push(format!(
            "size mismatch: the Rust struct is {} bytes, `struct {struct_name}` is {} — \
             a field has been added, removed or retyped on one side",
            desc.size, wgsl_size
        ));
    }

    let fields = core::slice::from_raw_parts(desc.fields, desc.field_count);
    for field in fields {
        let name = field.name.as_str();
        let Some(shader_field) = wgsl_fields.iter().find(|f| f.name == name) else {
            problems.push(format!(
                "`{name}` is inspectable in Rust but absent from `struct {struct_name}` — \
                 editing it in the inspector would change nothing on screen"
            ));
            continue;
        };
        if shader_field.offset != field.offset {
            problems.push(format!(
                "`{name}` is at byte {} in Rust but byte {} in the shader — \
                 it will read a neighbouring field's value",
                field.offset, shader_field.offset
            ));
        }
        if let Some(expected) = field_kind_size(field.kind) {
            if shader_field.size != expected {
                problems.push(format!(
                    "`{name}` is {} ({expected} bytes) in Rust but {} bytes in the shader",
                    field.kind.name(),
                    shader_field.size
                ));
            }
        }
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

/// Assert that a post-process component's layout matches its shader, panicking
/// with every mismatch listed.
///
/// The one-line form a plugin's test uses:
///
/// ```ignore
/// #[test]
/// fn the_uniform_matches_the_shader() {
///     assert_uniform_matches::<Crt>(WGSL, "CrtSettings");
/// }
/// ```
pub fn assert_uniform_matches<T: crate::ecs::Component>(wgsl: &str, struct_name: &str) {
    let desc = T::descriptor().expect(
        "a post-process component must be plugin-owned — \
         `descriptor()` returned None, so nothing describes its layout",
    );
    // SAFETY: `desc` comes from `T::descriptor()`, whose `name`/`fields`
    // pointers are `&'static` data emitted by the derive.
    if let Err(problems) = unsafe { check_uniform(&desc, wgsl, struct_name) } {
        panic!(
            "{} does not match `struct {struct_name}`:\n  - {}",
            T::TYPE_PATH,
            problems.join("\n  - ")
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHADER: &str = r#"
@group(0) @binding(0) var screen_texture: texture_2d<f32>;

struct DemoSettings {
    a: f32,
    b: f32,
    tint: vec4<f32>,
    count: u32,
};
@group(0) @binding(2) var<uniform> settings: DemoSettings;
"#;

    fn parse() -> (Vec<WgslField>, usize) {
        parse_wgsl_struct(SHADER, "DemoSettings").expect("should parse")
    }

    #[test]
    fn fields_are_read_in_declaration_order() {
        let (fields, _) = parse();
        let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, alloc::vec!["a", "b", "tint", "count"]);
    }

    /// The vec4 forces the offset from 8 up to 16. Getting this wrong is exactly
    /// the mismatch the whole module exists to catch, so the padding rule is
    /// asserted rather than assumed.
    #[test]
    fn a_vec4_is_padded_up_to_its_sixteen_byte_alignment() {
        let (fields, size) = parse();
        assert_eq!(fields[0].offset, 0);
        assert_eq!(fields[1].offset, 4);
        assert_eq!(fields[2].offset, 16, "vec4 must align to 16, not follow at 8");
        assert_eq!(fields[3].offset, 32);
        // 36 bytes of content, padded to the struct's 16-byte alignment.
        assert_eq!(size, 48);
    }

    /// A vec3 is 12 bytes but aligns to 16 — the classic WGSL trap.
    #[test]
    fn a_vec3_occupies_twelve_bytes_but_aligns_to_sixteen() {
        let shader = "struct S { a: f32, v: vec3<f32>, b: f32, };";
        let (fields, size) = parse_wgsl_struct(shader, "S").unwrap();
        assert_eq!(fields[1].offset, 16);
        assert_eq!(fields[1].size, 12);
        assert_eq!(fields[2].offset, 28, "a scalar may sit in the vec3's tail padding");
        assert_eq!(size, 32);
    }

    #[test]
    fn an_all_f32_struct_is_packed_end_to_end() {
        let shader = "struct S { a: f32, b: f32, c: f32, };";
        let (fields, size) = parse_wgsl_struct(shader, "S").unwrap();
        assert_eq!(
            fields.iter().map(|f| f.offset).collect::<Vec<_>>(),
            alloc::vec![0, 4, 8]
        );
        assert_eq!(size, 12);
    }

    #[test]
    fn trailing_commas_and_comments_are_tolerated() {
        let shader = "struct S {\n  a: f32, // the amount\n  b: f32,\n};";
        let (fields, _) = parse_wgsl_struct(shader, "S").unwrap();
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[1].name, "b");
    }

    #[test]
    fn a_missing_struct_is_an_error_rather_than_an_empty_pass() {
        let err = parse_wgsl_struct(SHADER, "NoSuchSettings").unwrap_err();
        assert!(err.contains("NoSuchSettings"), "{err}");
    }

    /// An unhandled type must fail loudly. Assuming a size for it would make the
    /// check quietly meaningless for that plugin — worse than not running.
    #[test]
    fn an_unknown_type_is_refused_rather_than_guessed() {
        let shader = "struct S { m: mat2x3<f32>, };";
        let err = parse_wgsl_struct(shader, "S").unwrap_err();
        assert!(err.contains("mat2x3<f32>"), "{err}");
    }

    #[test]
    fn the_struct_name_must_match_exactly() {
        // `DemoSettings` must not be found by searching for `Demo`.
        let shader = "struct DemoSettings { a: f32, };";
        assert!(parse_wgsl_struct(shader, "Demo").is_err());
    }
}
