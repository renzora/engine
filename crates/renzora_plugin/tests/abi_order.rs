//! Every type in `sys.rs` whose memory layout is part of the ABI, pinned.
//!
//! Both sides of this boundary compile their own copy of `sys.rs` from
//! independent source trees. There is no link step between them, no symbol to
//! mismatch and no version that moves on its own — **the layout is the entire
//! contract**. A field moved on one side and not the other is a wrong-offset
//! read, and the compiler has nothing to say about it.
//!
//! The append-only rule was broken three times before anything noticed:
//!
//! - `SystemEntry` gained a return value under a MINOR. The process died with no
//!   diagnostic (recorded at `sys.rs:44`).
//! - `add_material_shader` (MINOR 9) and `add_image` (MINOR 11) were each
//!   *inserted* into the middle of `Interface` and recorded as appended.
//!
//! ## Why this pins everything rather than a chosen few
//!
//! An earlier version listed seven structs by hand — the ones a plugin reads
//! field-by-field by offset. A census of the boundary found **67 types whose
//! layout is part of the contract**, and the seven were not the most exposed:
//! `QueryView` and `InputState` are walked every frame by every system, `StrRef`
//! is embedded by value in seven other structs, and `PanelAction` is an eighth
//! offset-keyed table that was simply left out.
//!
//! The failure mode of a curated list is omission, and omission is silent. So
//! this enumerates every `#[repr(C)]` and `#[repr(transparent)]` type in the file
//! and diffs the whole set. Adding a type fails until it is written into
//! `GOLDEN`, which forces the question "does this cross the boundary?" to be
//! answered once, by someone, rather than assumed.
//!
//! ## What a failure means
//!
//! - **A listed type's fields changed** — you edited a layout both sides depend
//!   on. If it is a table read by offset, appending at the very end is safe and
//!   you update the golden text. Anything else is a MAJOR bump, because every
//!   already-built plugin now reads the wrong bytes.
//! - **A type is in the source and not in the golden** — you added one. Decide
//!   whether it crosses; if it does, this file is where that is recorded.
//!
//! ## What this still cannot cover
//!
//! Values rather than layouts. `Key`'s numbering *is* the bit index into
//! `InputState`; the `SERVICE` ids are baked into every shipped plugin; `Easing`'s
//! ordinals must track the engine's own enum. Those are frozen too, and nothing
//! here checks them.

use renzora_plugin::sys::Interface;

/// One layout-carrying type, as `"name: type"` per field.
///
/// Tuple structs record their positions as `0`, `1`, … A `#[repr(transparent)]`
/// newtype is included because its layout is its inner type's: widening
/// `Access(pub u32)` to `u64` changes every struct that embeds it.
struct Golden {
    name: &'static str,
    fields: &'static [&'static str],
}

// Lives in `tests/data/` rather than beside this file because cargo compiles
// every `tests/*.rs` as its own test binary, and a bare list of `Golden`
// literals does not compile on its own.
include!("data/abi_golden.rs");

/// Parse every `#[repr(C)]` / `#[repr(transparent)]` type out of the source.
///
/// Reading the declaration rather than reflecting it is deliberate: these types
/// have no reflection and often no `Debug`, and a test that checked something
/// other than what ships would be worse than none.
fn declared_types(src: &str) -> Vec<(String, Vec<String>)> {
    let mut out = Vec::new();
    let lines: Vec<&str> = src.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let l = lines[i].trim();

        // Skip a `macro_rules!` body wholesale. The `interface!` definition
        // contains a literal `#[repr(C)] pub struct Interface {` as its template,
        // which this parser would otherwise read as a second, differently-shaped
        // declaration — the fields there are `$field: $ty`, not real ones.
        if lines[i].starts_with("macro_rules!") {
            i += 1;
            while i < lines.len() && lines[i] != "}" {
                i += 1;
            }
            i += 1;
            continue;
        }

        // Function-pointer aliases carry a signature and no fields, so nothing
        // above reaches them — and they are the most dangerous declarations in
        // the file, because a signature change is invisible everywhere else. A
        // fn pointer is one `usize` whatever its arity, so no struct that holds
        // one changes size or field text when its shape moves.
        //
        // This is not hypothetical for `SystemEntry` specifically: it gained a
        // return value under a MINOR and killed the process with no diagnostic
        // (`sys.rs:44`). Pinned as a single field holding the whole signature.
        if l.starts_with("pub type ") && l.contains("extern \"C\" fn") {
            let mut sig = l.to_string();
            let mut k = i;
            while !sig.ends_with(';') && k + 1 < lines.len() {
                k += 1;
                sig.push(' ');
                sig.push_str(lines[k].trim());
            }
            let name: String = sig["pub type ".len()..]
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            let rhs = sig.split_once('=').map(|(_, r)| r.trim().trim_end_matches(';').trim());
            out.push((name, vec![format!("= {}", rhs.unwrap_or(""))]));
            i = k + 1;
            continue;
        }

        // `Interface` is emitted by the `interface!` macro rather than written
        // out, because its field list also produces the load-time prefix hashes.
        let macro_form = l == "interface! {";
        if !(l == "#[repr(C)]" || l == "#[repr(transparent)]" || macro_form) {
            i += 1;
            continue;
        }

        let (name, mut j) = if macro_form {
            ("Interface".to_string(), i)
        } else {
            // Skip derives and docs to reach the declaration itself.
            let mut k = i + 1;
            while k < lines.len() {
                let t = lines[k].trim();
                if t.starts_with("pub struct ") || t.starts_with("pub enum ") {
                    break;
                }
                if !(t.starts_with('#') || t.starts_with("//") || t.is_empty()) {
                    break;
                }
                k += 1;
            }
            if k >= lines.len() {
                i += 1;
                continue;
            }
            // An enum carries a discriminant, not a field layout. Its *values*
            // are frozen too, but that is not what this test checks.
            let Some(rest) = lines[k].trim().strip_prefix("pub struct ") else {
                i = k + 1;
                continue;
            };
            let n: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            (n, k)
        };

        let decl = lines[j].trim();
        if !macro_form && decl.contains('(') && decl.ends_with(");") {
            // One-line tuple struct: `pub struct Entity(pub u64);`
            let inner = &decl[decl.find('(').unwrap() + 1..decl.rfind(')').unwrap()];
            let fields: Vec<String> = inner
                .split(',')
                .map(|p| p.trim().trim_start_matches("pub ").trim().to_string())
                .filter(|p| !p.is_empty())
                .enumerate()
                .map(|(n, ty)| format!("{n}: {ty}"))
                .collect();
            out.push((name, fields));
            i = j + 1;
            continue;
        }
        if !decl.ends_with('{') {
            i = j + 1;
            continue;
        }

        // Braced struct: accumulate fields until the closing brace at column 0.
        j += 1;
        let mut fields = Vec::new();
        let mut current = String::new();
        let mut depth = 0usize;
        while j < lines.len() {
            if lines[j] == "}" {
                break;
            }
            let t = lines[j].trim();
            if current.is_empty() {
                let head = t.strip_prefix("pub ").unwrap_or(t);
                let looks_like_field = t.contains(':')
                    && head.split(':').next().is_some_and(|n| {
                        !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    });
                if !looks_like_field {
                    j += 1;
                    continue;
                }
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(t);
            depth += t.matches('(').count();
            depth -= t.matches(')').count().min(depth);
            if depth == 0 && current.ends_with(',') {
                current.pop();
                fields.push(current.trim_start_matches("pub ").to_string());
                current.clear();
            }
            j += 1;
        }
        out.push((name, fields));
        i = j + 1;
    }
    out
}

#[test]
fn boundary_layouts_are_pinned() {
    let actual = declared_types(include_str!("../src/sys.rs"));
    let mut problems = Vec::new();

    for (name, fields) in &actual {
        match GOLDEN.iter().find(|g| g.name == name) {
            None => problems.push(format!(
                "NEW TYPE `{name}` is not in the golden list. If it crosses the boundary, add \
                 it with its fields; if it does not, add it anyway with a note saying so — the \
                 point is that somebody decided."
            )),
            Some(g) if g.fields != fields.as_slice() => problems.push(format!(
                "`{name}` CHANGED SHAPE.\n       was: {:?}\n       now: {:?}",
                g.fields, fields
            )),
            Some(_) => {}
        }
    }
    for g in GOLDEN {
        if !actual.iter().any(|(n, _)| n == g.name) {
            problems.push(format!(
                "`{}` was REMOVED or renamed. Every plugin built against it now reads a type \
                 that no longer exists.",
                g.name
            ));
        }
    }

    assert!(
        problems.is_empty(),
        "\n\n{} boundary layout problem(s):\n\n  - {}\n\n\
         These types are compiled independently by the host and by every plugin, so their \
         layout IS the ABI. Appending to the END of a table read by offset is safe and needs \
         only a golden update plus a VERSION_MINOR bump. Anything else — inserting, \
         reordering, retyping, or touching a struct passed by pointer at all — is a \
         VERSION_MAJOR change, because both sides read every field of those.\n",
        problems.len(),
        problems.join("\n  - ")
    );
}

/// A cross-check on the golden list's own length.
///
/// The list can be edited to match a bad struct in one pass; the compiler's
/// answer cannot. This catches a field the parser's heuristic skipped, which
/// would otherwise leave the golden silently short.
#[test]
fn interface_size_matches_its_field_list() {
    let iface = GOLDEN
        .iter()
        .find(|g| g.name == "Interface")
        .expect("Interface missing from the golden list");
    // Two leading `u32`s, then one pointer-sized field each.
    let ptrs = iface.fields.len() - 2;
    assert_eq!(
        core::mem::size_of::<Interface>(),
        2 * core::mem::size_of::<u32>() + ptrs * core::mem::size_of::<usize>(),
        "Interface size does not match its golden field list — a field was added or removed \
         without the list being updated"
    );
}



