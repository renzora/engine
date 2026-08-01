//! The [`Interface`] table is read by offset, so its field *order* is the ABI.
//!
//! This test exists because that guarantee was broken in practice, twice, and
//! nothing noticed. `add_mesh_data` was correctly appended at MINOR 5; then
//! `add_material_shader` (MINOR 9) and `add_image` (MINOR 11) were each inserted
//! *above* it while the changelog recorded both as "appended". A plugin built
//! against MINOR 5-10 would have called the slot it compiled against and landed
//! in a different function — handing a `MeshDataDesc*` to a reader expecting an
//! `ImageDesc*`, or running `from_utf8_unchecked` over vertex positions. That is
//! a segfault, and `guard_host` catches panics rather than those.
//!
//! Reviewing a diff does not catch it: inserting a field next to related ones
//! reads as *tidier* than appending it three screens away, which is exactly why
//! it happened twice. So the order is pinned here instead of trusted.
//!
//! **When you add a function**, append it to the end of `Interface` and to the
//! end of `EXPECTED` below. If this test fails any other way, you have moved an
//! existing field and every already-built plugin now calls the wrong one.

use renzora_plugin::sys::Interface;

/// Every field of [`Interface`], in declaration order.
///
/// Grouped by the MINOR that added it, so the append-only rule is visible rather
/// than merely stated: a new entry can only ever go at the bottom.
const EXPECTED: &[&str] = &[
    // The handshake. First, and never anywhere else — a plugin reads these
    // before it trusts a single other field.
    "version_major",
    "version_minor",
    // ── 3.0 base ──
    "register_component",
    "component_id_by_name",
    "add_system",
    "log",
    "add_render_pass",
    "render_set_pipeline",
    "render_draw",
    "add_post_process",
    "add_mesh",
    "add_material",
    "register_resource",
    "insert_resource",
    // ── MINOR 1 ──
    "add_panel",
    // ── MINOR 3 ──
    "set_field_range",
    // ── MINOR 5 ──
    "add_mesh_data",
    // ── MINOR 9 ──
    "add_material_shader",
    // ── MINOR 11 ──
    "add_image",
];

/// The two leading `u32` version fields, which are not function pointers.
const VERSION_FIELDS: usize = 2;

/// The field list, parsed out of the source rather than reflected.
///
/// `Interface` is a `#[repr(C)]` struct of raw function pointers with no
/// reflection and no `Debug`, so there is nothing to enumerate at runtime. The
/// declaration is the only source of truth, so the test reads it — which also
/// means the test cannot silently pass by checking something other than what
/// ships.
fn declared_fields() -> Vec<String> {
    let src = include_str!("../src/sys.rs");
    let start = src
        .find("pub struct Interface {")
        .expect("Interface struct not found — was it renamed?");
    let body = &src[start..];
    let end = body
        .find("\n}")
        .expect("unterminated Interface struct");

    body[..end]
        .lines()
        .filter_map(|line| {
            // Field declarations are indented exactly four spaces at the top
            // level of the struct; a `host: *mut Host,` argument inside a
            // multi-line fn type is indented eight, so depth alone separates
            // them without needing to parse Rust.
            let rest = line.strip_prefix("    pub ")?;
            if line.starts_with("        ") {
                return None;
            }
            rest.split(':').next().map(str::trim).map(str::to_owned)
        })
        .collect()
}

#[test]
fn interface_field_order() {
    let actual = declared_fields();

    // Reported as a whole list rather than field-by-field: when this fails, the
    // useful question is "what moved", and that is only visible side by side.
    assert_eq!(
        actual, EXPECTED,
        "\n\nInterface field order changed.\n\
         \n\
         This table is read by OFFSET. Reordering or inserting a field means \
         every plugin built against an earlier version now calls a different \
         function than the one it compiled against — a wrong-type pointer \
         dereference, not a clean failure.\n\
         \n\
         If you ADDED a function: put it at the END of the struct and the END of \
         EXPECTED in this file, then bump VERSION_MINOR.\n\
         If you MOVED or REMOVED one: that is a VERSION_MAJOR change. There is no \
         MINOR that makes it safe.\n"
    );
}

/// A second, cheaper guard for the same mistake.
///
/// The list above can be edited to match a bad struct in one pass; the size
/// cannot, because it is the compiler's answer rather than the author's.
#[test]
fn interface_field_count() {
    let fns = EXPECTED.len() - VERSION_FIELDS;
    assert_eq!(
        core::mem::size_of::<Interface>(),
        VERSION_FIELDS * core::mem::size_of::<u32>() + fns * core::mem::size_of::<usize>(),
        "Interface size does not match its documented field list — a field was \
         added or removed without updating EXPECTED"
    );
}
