//! Span-tracked attribute writeback for the HUI inspector.
//!
//! When the inspector edits an attribute on a markup-built entity, the
//! [`write_attr_to_markup`] helper looks up the entity's [`MarkupSource`],
//! finds the recorded byte range for that attribute in the cached
//! [`HtmlTemplate::source`], patches the bytes, and rewrites the `.html` file
//! on disk. All downstream spans on the same node (and following siblings, if
//! we ever care) are delta-shifted so subsequent edits in the same session
//! keep targeting the right ranges.
//!
//! What's intentionally NOT here:
//! - **Hot-reload guard.** `renzora_hui` doesn't currently re-run the loader
//!   on `AssetEvent<HtmlTemplate>::Modified` (the Phase C feature in
//!   `template.rs`), so writing the file does not trigger a respawn. When
//!   that lands, this module will need to set a "self-write" flag the
//!   reload-handler checks before despawning.
//! - **Atomic multi-attribute writes.** Each call writes one attribute.
//!   Two inspector edits = two file writes. Fine in practice for now.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy_hui::prelude::{AttrSpan, HtmlTemplate, Span, XNode};

use crate::provenance::MarkupSource;

/// Rewrite (or insert) an attribute on the entity's source markup.
///
/// - `attr_ident` is the literal key the user wrote (e.g. `"font_size"`,
///   `"flex_direction"`).
/// - `new_value` is the unquoted text that lands between the `"…"` after the
///   `=`. Callers are responsible for formatting it correctly (`"row"`,
///   `"100%"`, `"#8A93A2"`).
///
/// Idempotent on no-op: if the new text equals the existing bytes, no file
/// I/O happens. Silent on missing entity / missing provenance / template not
/// loaded — those are recoverable "user clicked a non-markup entity" cases.
pub fn write_attr_to_markup(world: &mut World, entity: Entity, attr_ident: &str, new_value: &str) {
    let Some(source_ref) = world.get::<MarkupSource>(entity) else {
        return;
    };
    let handle = source_ref.template_handle.clone();
    let path: Vec<u32> = source_ref.node_path.clone();

    // Resolve the asset's project-relative path BEFORE we take an exclusive
    // borrow of the templates store — the AssetServer borrow has to drop
    // before we mutate Assets<HtmlTemplate>.
    let asset_path = world
        .resource::<AssetServer>()
        .get_path(&handle)
        .map(|p| p.to_string());
    let Some(asset_path) = asset_path else {
        warn!("renzora_hui writeback: template handle has no asset path");
        return;
    };

    let project_root = world
        .get_resource::<renzora::core::CurrentProject>()
        .map(|cp| cp.path.clone());

    let mut templates = world.resource_mut::<Assets<HtmlTemplate>>();
    let Some(template) = templates.get_mut(&handle) else {
        warn!("renzora_hui writeback: HtmlTemplate not loaded for handle");
        return;
    };

    // Walk to the target XNode by descent.
    let Some(node) = walk_node_mut(&mut template.root, &path) else {
        warn!(
            "renzora_hui writeback: node_path {:?} does not exist in template (asset changed under us?)",
            path
        );
        return;
    };

    // Existing attribute → in-place value rewrite.
    let existing_idx = node
        .attr_spans
        .iter()
        .position(|a| a.key_ident == attr_ident);

    if let Some(idx) = existing_idx {
        let value_span = node.attr_spans[idx].value;
        // No-op short-circuit. Cheaper than re-writing the file every keystroke
        // when the user spins a DragValue back to its original value.
        if &template.source[value_span.as_range()] == new_value.as_bytes() {
            return;
        }
        let delta = new_value.len() as i32 - value_span.len() as i32;
        // Patch bytes.
        template
            .source
            .splice(value_span.as_range(), new_value.bytes());
        // Fix up every span on this node and its descendants that starts at
        // or after the patched range so the in-memory cache stays coherent.
        let after = value_span.start;
        shift_spans_after(node, after, delta);
    } else {
        // Brand-new attribute → insert `\n   attr_ident="new_value"` at the
        // open-tag-close position. The leading space + linebreak mimic the
        // surrounding indentation of an existing well-formatted file; we
        // don't try to be clever about matching the user's exact style.
        let close = node.open_tag_close.start;
        let insertion = format!("\n    {attr_ident}=\"{new_value}\"");
        let bytes = insertion.as_bytes();
        let delta = bytes.len() as i32;
        template
            .source
            .splice((close as usize)..(close as usize), bytes.iter().copied());
        // Insertion shifts everything at `close` or after by `delta`. Because
        // open_tag_close is at `close`, it itself moves; track it.
        shift_spans_after(node, close, delta);
        // Record the new attribute span pointing at the inserted bytes.
        let key_start = close + 1 /* \n */ + 4 /* 4 spaces */;
        let key_end = key_start + attr_ident.len() as u32;
        let value_start = key_end + 2 /* =" */;
        let value_end = value_start + new_value.len() as u32;
        node.attr_spans.push(AttrSpan {
            key_ident: attr_ident.to_string(),
            prefix: None,
            key: Span {
                start: key_start,
                end: key_end,
            },
            value: Span {
                start: value_start,
                end: value_end,
            },
        });
    }

    // Persist to disk. `asset_path` is what was passed to `AssetServer::load`
    // — for a drag-dropped template that's already project-rooted
    // (`assets/ui/foo.html`), so joining with `project.path` rebuilds the
    // disk location the editor's `EmbeddedAssetReader` would resolve to.
    // Falls back to treating `asset_path` as absolute when there's no
    // `CurrentProject` (headless test setups).
    let disk_path: PathBuf = match project_root {
        Some(root) => root.join(&asset_path),
        None => PathBuf::from(&asset_path),
    };
    if let Err(err) = std::fs::write(&disk_path, &template.source) {
        warn!(
            "renzora_hui writeback: failed to write {} — {err}",
            disk_path.display()
        );
    }
}

/// Walk into `roots` by the index chain in `path`. Returns the addressed
/// `XNode` mutably, or `None` if any step lands out of bounds. `path` is
/// rooted at `roots[0]` — the loader only spawns from there, so the first
/// element of the chain is the first child *of* `roots[0]`, not an index
/// into `roots` itself.
fn walk_node_mut<'a>(roots: &'a mut [XNode], path: &[u32]) -> Option<&'a mut XNode> {
    let root = roots.first_mut()?;
    let mut cursor = root;
    for idx in path.iter().copied() {
        cursor = cursor.children.get_mut(idx as usize)?;
    }
    Some(cursor)
}

/// Bump every span on `node` and its descendants whose start byte is `>= after`
/// by `delta` (signed). Called after an in-place edit / insertion to keep the
/// in-memory `HtmlTemplate` coherent until the AssetServer reparses the file.
fn shift_spans_after(node: &mut XNode, after: u32, delta: i32) {
    fn shift_span(span: &mut Span, after: u32, delta: i32) {
        if span.start >= after {
            span.start = (span.start as i32 + delta).max(0) as u32;
        }
        if span.end >= after {
            span.end = (span.end as i32 + delta).max(0) as u32;
        }
    }
    for attr in &mut node.attr_spans {
        shift_span(&mut attr.key, after, delta);
        shift_span(&mut attr.value, after, delta);
    }
    shift_span(&mut node.open_tag_close, after, delta);
    if let Some(content) = node.content_span.as_mut() {
        shift_span(content, after, delta);
    }
    for child in &mut node.children {
        shift_spans_after(child, after, delta);
    }
}
