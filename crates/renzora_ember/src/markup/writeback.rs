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
//! - **Hot-reload coupling.** This writeback dirties the asset (`get_mut`
//!   emits `Modified`) and rewrites the file (the file watcher emits another
//!   `Modified`), but it has already updated the live entity in place, so a
//!   rebuild would only despawn the node the user is editing. The Phase-C
//!   reload handler in `template.rs` therefore rebuilds **only** for asset ids
//!   the code editor registered via `TemplateReloadRequests` on save — these
//!   writeback `Modified`s aren't registered, so they're ignored. No self-write
//!   flag needed: the gate is the request set, not the event.
//! - **Atomic multi-attribute writes.** Each call writes one attribute.
//!   Two inspector edits = two file writes. Fine in practice for now.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_hui::prelude::{AttrSpan, HtmlTemplate, Span, XNode};

use crate::markup::provenance::MarkupSource;

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

    // Scoped so the `Assets<HtmlTemplate>` borrow ends before the commit below,
    // which needs `&mut World` to record the undo entry.
    let Some((before_bytes, after_bytes)) = ({
    let mut templates = world.resource_mut::<Assets<HtmlTemplate>>();
    let Some(mut template) = templates.get_mut(&handle) else {
        warn!("renzora_hui writeback: HtmlTemplate not loaded for handle");
        return;
    };
    // 0.19: `Assets::get_mut` yields `AssetMut` (a change-tracked smart pointer)
    // which can't do disjoint field borrows the way `&mut T` can. Reborrow to a
    // plain `&mut HtmlTemplate` so `template.root` (held by `node`) and
    // `template.source` can be borrowed separately below. The deref_mut here
    // still flags the asset as changed.
    let template = &mut *template;
    // Captured before any splice, so the undo entry has the file as it stood.
    let before_bytes = template.source.clone();

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
    Some((before_bytes, template.source.clone()))
    }) else {
        return;
    };
    commit_with(
        world,
        &asset_path,
        project_root,
        before_bytes,
        after_bytes,
        "Edit UI attribute",
        // No rebuild: the live entity was updated in place by whoever called
        // this, and rebuilding would despawn the node being edited.
        false,
    );
}

/// Rewrite a node's text content — the bytes between `<text>` and `</text>`.
///
/// The counterpart to [`write_attr_to_markup`] for the one part of a node that
/// is not an attribute. Editing a label was the single most common change a
/// template needs and the only way to make it stick was the code editor.
///
/// Declines a node with no `content_span`: that is a self-closing or
/// element-only tag, which has no text to replace — writing one would mean
/// inventing a body, a different edit from changing the one that is there.
pub fn write_content_to_markup(world: &mut World, entity: Entity, new_text: &str) {
    let Some(source_ref) = world.get::<MarkupSource>(entity) else {
        return;
    };
    let handle = source_ref.template_handle.clone();
    let path: Vec<u32> = source_ref.node_path.clone();

    let Some(asset_path) = world
        .resource::<AssetServer>()
        .get_path(&handle)
        .map(|p| p.to_string())
    else {
        return;
    };
    let project_root = world
        .get_resource::<renzora::core::CurrentProject>()
        .map(|cp| cp.path.clone());

    let Some((before_bytes, after_bytes)) = ({
        let mut templates = world.resource_mut::<Assets<HtmlTemplate>>();
        let Some(mut template) = templates.get_mut(&handle) else {
            return;
        };
        let template = &mut *template;
        let before = template.source.clone();
        let Some(node) = walk_node_mut(&mut template.root, &path) else {
            return;
        };
        let Some(span) = node.content_span else {
            warn!("ui text: this node has no text content to replace");
            return;
        };
        if &template.source[span.as_range()] == new_text.as_bytes() {
            return;
        }
        let delta = new_text.len() as i32 - span.len() as i32;
        template.source.splice(span.as_range(), new_text.bytes());
        // The content span itself moves (its end shifts by the delta), and so
        // does everything after it — same bookkeeping an attribute edit does, so
        // a second edit in the same session still targets the right bytes.
        node.content_span = Some(Span {
            start: span.start,
            end: (span.end as i32 + delta).max(span.start as i32) as u32,
        });
        shift_spans_after(node, span.end, delta);
        Some((before, template.source.clone()))
    }) else {
        return;
    };
    commit_with(
        world,
        &asset_path,
        project_root,
        before_bytes,
        after_bytes,
        "Edit UI text",
        // The caller already set the live `Text`; a rebuild would despawn the
        // node mid-edit.
        false,
    );
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

// ── Structural writeback: moving a node ─────────────────────────────────────
//
// Unlike an attribute edit, a move changes the *shape* of the tree, so there is
// no way to patch the live entities into agreement with the file — a node has a
// new parent and new siblings, and every span after the splice has moved. So
// this path deliberately does the opposite of the attribute path: it writes the
// file and asks for a rebuild, letting the loader reparse and respawn. That is
// why it does not bother with `shift_spans_after`, which only fixes up the
// edited node's own subtree anyway.

/// Start of the whitespace that indents the byte at `at`, including the newline
/// before it.
///
/// Returns `at` unchanged when the run does not reach a newline — an element
/// written inline (`<a/><b/>`) has no indentation to carry, and inventing one
/// would reformat a line the user chose to keep tight.
fn line_lead_start(source: &[u8], at: usize) -> usize {
    let mut i = at.min(source.len());
    while i > 0 && matches!(source[i - 1], b' ' | b'\t') {
        i -= 1;
    }
    if i > 0 && source[i - 1] == b'\n' {
        i - 1
    } else {
        at
    }
}

/// Where a moved element lands among its new siblings.
#[derive(Clone, Copy, Debug)]
pub enum Dest {
    /// Immediately before the sibling whose element begins at this offset.
    Before(u32),
    /// Immediately after the sibling occupying this range. Both ends are
    /// needed: `end` is where the text goes, `start` is whose indentation it
    /// copies.
    After { start: u32, end: u32 },
}

/// Resolve a [`Dest`] to `(byte offset, prefix, suffix)`.
///
/// `prefix`/`suffix` are the indentation placed on whichever side of the
/// inserted text keeps it on a line of its own — before it when landing after a
/// sibling, after it when landing before one.
fn dest_parts(source: &[u8], dest: Dest) -> Option<(usize, &[u8], &[u8])> {
    match dest {
        Dest::Before(p) => {
            let p = p as usize;
            if p > source.len() {
                return None;
            }
            Some((p, &[], &source[line_lead_start(source, p)..p]))
        }
        Dest::After { start, end } => {
            let (s, e) = (start as usize, end as usize);
            if e > source.len() || s > source.len() {
                return None;
            }
            Some((e, &source[line_lead_start(source, s)..s], &[]))
        }
    }
}

/// Splice new markup into `source` at `dest`, returning the new bytes.
///
/// The snippet's own indentation is *relative*: every line after the first gets
/// the destination's indentation prepended, so a two-level snippet dropped three
/// levels deep comes out at five, not two. Without that, everything inserted by
/// the palette would sit flush against the left margin regardless of where it
/// landed.
pub fn insert_element(source: &[u8], dest: Dest, markup: &str) -> Option<Vec<u8>> {
    let (at, prefix, suffix) = dest_parts(source, dest)?;
    let lead = if prefix.is_empty() { suffix } else { prefix };
    let indented = match std::str::from_utf8(lead) {
        Ok(l) if l.contains('\n') => markup.replace('\n', l),
        _ => markup.to_string(),
    };
    let mut out = Vec::with_capacity(source.len() + indented.len() + lead.len());
    out.extend_from_slice(&source[..at]);
    out.extend_from_slice(prefix);
    out.extend_from_slice(indented.as_bytes());
    out.extend_from_slice(suffix);
    out.extend_from_slice(&source[at..]);
    Some(out)
}

/// Cut the element at `element` out of `source`, returning the new bytes.
///
/// The cut reaches back over the whitespace indenting the element, so removing
/// a node does not leave the blank line it occupied. This is the same cut
/// [`move_element`] makes; a move is this plus a re-insert.
pub fn remove_element(source: &[u8], element: Span) -> Option<Vec<u8>> {
    let (start, end) = (element.start as usize, element.end as usize);
    if start >= end || end > source.len() {
        return None;
    }
    let cut_start = line_lead_start(source, start);
    let mut out = Vec::with_capacity(source.len());
    out.extend_from_slice(&source[..cut_start]);
    out.extend_from_slice(&source[end..]);
    Some(out)
}

/// Move the element at `element` to `dest` within `source`, returning the new
/// bytes. `None` when the span is malformed or the destination lies inside the
/// element being moved (which is not a move).
///
/// The element carries its own line with it: the cut reaches back over the
/// whitespace indenting it, and the insert re-adds the indentation of its new
/// neighbour. Without that, moving a node down a file leaves a blank ragged line
/// where it was and jams it against the sibling where it lands.
pub fn move_element(source: &[u8], element: Span, dest: Dest) -> Option<Vec<u8>> {
    let (el_start, el_end) = (element.start as usize, element.end as usize);
    if el_start >= el_end || el_end > source.len() {
        return None;
    }
    let cut_start = line_lead_start(source, el_start);
    let moved = &source[el_start..el_end];

    let (at, prefix, suffix) = dest_parts(source, dest)?;
    if at > cut_start && at < el_end {
        return None;
    }

    let mut out = Vec::with_capacity(source.len() + prefix.len() + suffix.len());
    if at <= cut_start {
        out.extend_from_slice(&source[..at]);
        out.extend_from_slice(prefix);
        out.extend_from_slice(moved);
        out.extend_from_slice(suffix);
        out.extend_from_slice(&source[at..cut_start]);
        out.extend_from_slice(&source[el_end..]);
    } else {
        out.extend_from_slice(&source[..cut_start]);
        out.extend_from_slice(&source[el_end..at]);
        out.extend_from_slice(prefix);
        out.extend_from_slice(moved);
        out.extend_from_slice(suffix);
        out.extend_from_slice(&source[at..]);
    }
    Some(out)
}

/// Walk into `roots` by an index chain, immutably. See [`walk_node_mut`].
fn walk_node<'a>(roots: &'a [XNode], path: &[u32]) -> Option<&'a XNode> {
    let mut cursor = roots.first()?;
    for idx in path.iter().copied() {
        cursor = cursor.children.get(idx as usize)?;
    }
    Some(cursor)
}

/// Reorder `entity` to be child number `insert_index` of `new_parent`, in the
/// `.html` and in the live tree.
///
/// This is the write-back for a drag that reorders a flex child. It is the
/// structural counterpart to [`write_attr_to_markup`], and it works the other
/// way round on purpose: the attribute path patches bytes and leaves the live
/// entity alone, whereas this writes the file and asks for a rebuild. A move
/// gives the node a new parent and new siblings and shifts every span after the
/// splice, so there is nothing to patch the live tree *into* — reparsing is both
/// simpler and the only version that is actually correct.
///
/// The live reorder still happens first so the drop looks instant. The rebuild
/// throws that away a frame or two later and replaces it with the same thing.
pub fn move_node_in_markup(
    world: &mut World,
    entity: Entity,
    new_parent: Entity,
    before: Option<Entity>,
) {
    let Some(src) = world.get::<MarkupSource>(entity) else {
        return;
    };
    let handle = src.template_handle.clone();
    let node_path = src.node_path.clone();
    let Some(parent_src) = world.get::<MarkupSource>(new_parent) else {
        return;
    };
    // Moving between two different `.html` files would mean deleting from one
    // and inserting into the other — a different operation, and not one any
    // drag in a single canvas can ask for.
    if parent_src.template_handle != handle {
        return;
    }
    let parent_path = parent_src.node_path.clone();
    // The sibling's own recorded position, not a counted index — see
    // `DropTarget::before` for why the two lists cannot be assumed to line up.
    let before_path = match before {
        Some(b) => match world.get::<MarkupSource>(b) {
            Some(s) if s.template_handle == handle => Some(s.node_path.clone()),
            // A sibling with no provenance (or from another file) is not
            // something we can address in the source, so fall back to appending
            // rather than guessing at a slot.
            _ => None,
        },
        None => None,
    };

    let asset_path = world
        .resource::<AssetServer>()
        .get_path(&handle)
        .map(|p| p.to_string());
    let Some(asset_path) = asset_path else {
        return;
    };
    let project_root = world
        .get_resource::<renzora::core::CurrentProject>()
        .map(|cp| cp.path.clone());

    let (before_bytes, new_source) = {
        let templates = world.resource::<Assets<HtmlTemplate>>();
        let Some(template) = templates.get(&handle) else {
            return;
        };
        let Some(node) = walk_node(&template.root, &node_path) else {
            return;
        };
        let Some(parent) = walk_node(&template.root, &parent_path) else {
            return;
        };
        let element = node.element;

        // Dropping a node back onto its own slot is a no-op, not a move onto
        // itself.
        let dest = match before_path.as_deref().and_then(|p| walk_node(&template.root, p)) {
            Some(next) if next.element.start == element.start => return,
            Some(next) => Dest::Before(next.element.start),
            None => {
                let Some(last) = parent.children.last() else {
                    return;
                };
                if last.element.start == element.start {
                    return;
                }
                Dest::After {
                    start: last.element.start,
                    end: last.element.end,
                }
            }
        };
        match move_element(&template.source, element, dest) {
            Some(bytes) => (template.source.clone(), bytes),
            None => return,
        }
    };

    if !commit(
        world,
        &asset_path,
        project_root,
        before_bytes,
        new_source,
        "Move UI node",
    ) {
        return;
    }

    // Show the new order immediately; the rebuild below replaces it with the
    // same arrangement once the asset has re-read from disk. The live index is
    // found from the live children, which is the one place counting them is
    // right — this list is the thing being reordered.
    let live_index = world
        .get::<Children>(new_parent)
        .and_then(|kids| {
            before.and_then(|b| kids.iter().position(|c| c == b))
        })
        .unwrap_or_else(|| {
            world
                .get::<Children>(new_parent)
                .map(|k| k.len())
                .unwrap_or(0)
        });
    if let Ok(mut em) = world.get_entity_mut(new_parent) {
        em.insert_children(live_index, &[entity]);
    }
}

/// Delete a markup node from its template.
///
/// Deleting the *entity* is not enough and is worse than doing nothing: the
/// live tree is rebuilt from the file, so a despawned node reappears at the next
/// rebuild — and since a rebuild is what an insert or a move triggers, editing
/// after a delete resurrected everything that had been deleted.
///
/// Returns whether it removed anything, so the caller can tell a markup delete
/// (the file is the record) from a scene delete (the undo stack is).
pub fn remove_node_in_markup(world: &mut World, entity: Entity) -> bool {
    let Some(src) = world.get::<MarkupSource>(entity) else {
        return false;
    };
    let handle = src.template_handle.clone();
    let node_path = src.node_path.clone();
    // The template root is the template — deleting it would empty the file
    // rather than remove a node, which is not what a Delete keypress means.
    if node_path.is_empty() {
        warn!("ui delete: the template root cannot be deleted; clear the UI Template slot instead");
        return true;
    }
    let Some(asset_path) = world
        .resource::<AssetServer>()
        .get_path(&handle)
        .map(|p| p.to_string())
    else {
        return false;
    };
    let project_root = world
        .get_resource::<renzora::core::CurrentProject>()
        .map(|cp| cp.path.clone());

    let (before_bytes, new_source) = {
        let templates = world.resource::<Assets<HtmlTemplate>>();
        let Some(template) = templates.get(&handle) else {
            warn!("ui delete: {asset_path} is not loaded");
            return false;
        };
        let Some(node) = walk_node(&template.root, &node_path) else {
            warn!("ui delete: node_path {node_path:?} is not in {asset_path}");
            return false;
        };
        match remove_element(&template.source, node.element) {
            Some(bytes) => (template.source.clone(), bytes),
            None => return false,
        }
    };
    commit(
        world,
        &asset_path,
        project_root,
        before_bytes,
        new_source,
        "Delete UI node",
    );
    true
}

// ── Undo ────────────────────────────────────────────────────────────────────

/// One markup edit, as the file before and after it.
///
/// A snapshot rather than an inverse operation, because every structural edit
/// here already *is* a whole-file rewrite: `move_element`, `insert_element` and
/// `remove_element` each produce a new `Vec<u8>` and hand it to `fs::write`. The
/// inverse of "write these bytes" is "write those bytes", and expressing it any
/// other way would mean maintaining a second, subtler description of each edit
/// that could disagree with the first.
///
/// It costs a copy of the template per edit. Templates are a few kilobytes and
/// this only happens on a deliberate action, which is a price worth paying to
/// have Ctrl+Z work at all — without it a delete was unrecoverable, and delete
/// is the one edit people expect to take back.
struct MarkupEdit {
    asset_path: String,
    disk_path: PathBuf,
    before: Vec<u8>,
    after: Vec<u8>,
    label: &'static str,
}

impl renzora::undo::UndoCommand for MarkupEdit {
    fn label(&self) -> &str {
        self.label
    }
    /// Also the redo path — `execute` runs on the initial push *and* on redo,
    /// so writing `after` is correct in both cases.
    fn execute(&mut self, world: &mut World) {
        write_and_rebuild(world, &self.disk_path, &self.asset_path, &self.after);
    }
    fn undo(&mut self, world: &mut World) {
        write_and_rebuild(world, &self.disk_path, &self.asset_path, &self.before);
    }
}

fn write_and_rebuild(world: &mut World, disk: &Path, asset_path: &str, bytes: &[u8]) {
    if let Err(err) = std::fs::write(disk, bytes) {
        warn!("markup undo: failed to write {} — {err}", disk.display());
        return;
    }
    request_rebuild(world, asset_path);
}

/// Apply a structural edit: write it, rebuild, and make it undoable.
///
/// The single exit for `move` / `insert` / `remove`, so none of them can forget
/// one of the three.
fn commit(
    world: &mut World,
    asset_path: &str,
    project_root: Option<PathBuf>,
    before: Vec<u8>,
    after: Vec<u8>,
    label: &'static str,
) -> bool {
    commit_with(world, asset_path, project_root, before, after, label, true)
}

/// [`commit`], with control over whether the live tree is rebuilt.
///
/// An attribute edit passes `false`: it has already updated the live entity in
/// place, and rebuilding would despawn the node the user is mid-edit on. Undo
/// and redo of that same edit *do* rebuild, because restoring the file is the
/// only thing they do — see [`MarkupEdit`].
fn commit_with(
    world: &mut World,
    asset_path: &str,
    project_root: Option<PathBuf>,
    before: Vec<u8>,
    after: Vec<u8>,
    label: &'static str,
    rebuild: bool,
) -> bool {
    if before == after {
        return true;
    }
    let disk_path: PathBuf = match project_root {
        Some(root) => root.join(asset_path),
        None => PathBuf::from(asset_path),
    };
    if let Err(err) = std::fs::write(&disk_path, &after) {
        warn!("markup edit: failed to write {} — {err}", disk_path.display());
        return false;
    }
    if rebuild {
        info!("{label}: {asset_path}");
        request_rebuild(world, asset_path);
    }
    // Recorded, not executed: the write above already happened, and `execute`
    // would repeat it.
    renzora::undo::record(
        world,
        renzora::undo::UndoContext::Scene,
        Box::new(MarkupEdit {
            asset_path: asset_path.to_string(),
            disk_path,
            before,
            after,
            label,
        }),
    );
    true
}

/// Re-read a template from disk and let `hot_reload_templates` act on it.
///
/// The registration has to come first: the markup plugin rebuilds only for
/// `Modified` events whose asset id is in the request set, so that an inspector
/// attribute writeback — which has already patched the live entity — does not
/// despawn the node the user is mid-edit on.
///
/// The reload is the part that was missing. Marking the asset modified with
/// `Assets::get_mut` fires a `Modified` too, but for the *stale in-memory* AST:
/// the rebuild then re-ran from the old parse (so nothing on screen changed) and
/// consumed the request, so the file watcher's later, genuine `Modified` was
/// ignored. The write landed in the file and the editor never showed it.
///
/// `reload` re-reads and re-parses, so the event that arrives carries the bytes
/// we just wrote.
fn request_rebuild(world: &mut World, asset_path: &str) {
    let server = world.resource::<AssetServer>().clone();
    if let Some(mut requests) =
        world.get_resource_mut::<crate::markup::template::TemplateReloadRequests>()
    {
        requests.request(&server, asset_path);
    }
    server.reload(asset_path.to_string());
}

/// Insert `markup` into the template as a child of `parent`, ahead of `before`
/// (or last when that is `None`).
///
/// The palette's write-back. Same file-then-rebuild path as
/// [`move_node_in_markup`] and for the same reason: new elements have no live
/// entities to patch, so the loader has to build them from source.
pub fn insert_node_in_markup(
    world: &mut World,
    parent: Entity,
    before: Option<Entity>,
    markup: &str,
) {
    // Every early return below is a silent no-op from the user's side — the
    // click just does nothing — so each one says why. An insert that quietly
    // declines is indistinguishable from a dead button.
    let Some(parent_src) = world.get::<MarkupSource>(parent) else {
        warn!("ui insert: {parent} is not a markup node (no MarkupSource)");
        return;
    };
    let handle = parent_src.template_handle.clone();
    let parent_path = parent_src.node_path.clone();
    let before_path = before.and_then(|b| match world.get::<MarkupSource>(b) {
        Some(s) if s.template_handle == handle => Some(s.node_path.clone()),
        _ => None,
    });

    let Some(asset_path) = world
        .resource::<AssetServer>()
        .get_path(&handle)
        .map(|p| p.to_string())
    else {
        warn!("ui insert: the template handle has no asset path");
        return;
    };
    let project_root = world
        .get_resource::<renzora::core::CurrentProject>()
        .map(|cp| cp.path.clone());

    let (before_bytes, new_source) = {
        let templates = world.resource::<Assets<HtmlTemplate>>();
        let Some(template) = templates.get(&handle) else {
            warn!("ui insert: {asset_path} is not loaded");
            return;
        };
        let Some(parent_node) = walk_node(&template.root, &parent_path) else {
            warn!("ui insert: node_path {parent_path:?} is not in {asset_path}");
            return;
        };
        let dest = match before_path
            .as_deref()
            .and_then(|p| walk_node(&template.root, p))
        {
            Some(next) => Dest::Before(next.element.start),
            None => match parent_node.children.last() {
                Some(last) => Dest::After {
                    start: last.element.start,
                    end: last.element.end,
                },
                // An empty container has no sibling to anchor to, so land just
                // past its open tag — the one position that is always inside it.
                //
                // `open_tag_close` points at the *first* byte of `>` or `/>`, so
                // stepping one past it is only inside the element when the tag
                // ends in a plain `>`. On a self-closing tag that byte is the
                // `/`, and inserting after it writes `<node/<child/>>` — a file
                // that no longer parses. A self-closed element has no inside to
                // put anything in; it would have to be rewritten as a pair
                // first, which is a different edit.
                None => {
                    let at = parent_node.open_tag_close.start as usize;
                    if template.source.get(at).copied() != Some(b'>') {
                        warn!("ui insert: {asset_path} — cannot add a child to a self-closing tag");
                        return;
                    }
                    Dest::Before(parent_node.open_tag_close.start + 1)
                }
            },
        };
        match insert_element(&template.source, dest, markup) {
            Some(bytes) => (template.source.clone(), bytes),
            None => {
                warn!("ui insert: could not splice into {asset_path} at {dest:?}");
                return;
            }
        }
    };
    commit(
        world,
        &asset_path,
        project_root,
        before_bytes,
        new_source,
        "Add UI node",
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Locate `needle` in `hay` and return it as a span, so the tests can talk
    /// about elements by their text instead of hard-coded byte offsets.
    fn span_of(hay: &str, needle: &str) -> Span {
        let start = hay.find(needle).expect("needle present") as u32;
        Span {
            start,
            end: start + needle.len() as u32,
        }
    }

    const TREE: &str = "<node>\n    <a/>\n    <b/>\n    <c/>\n</node>";

    #[test]
    fn move_last_to_front() {
        let out = move_element(
            TREE.as_bytes(),
            span_of(TREE, "<c/>"),
            Dest::Before(span_of(TREE, "<a/>").start),
        )
        .expect("moves");
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "<node>\n    <c/>\n    <a/>\n    <b/>\n</node>"
        );
    }

    #[test]
    fn move_first_to_back() {
        let c = span_of(TREE, "<c/>");
        let out = move_element(
            TREE.as_bytes(),
            span_of(TREE, "<a/>"),
            Dest::After {
                start: c.start,
                end: c.end,
            },
        )
        .expect("moves");
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "<node>\n    <b/>\n    <c/>\n    <a/>\n</node>"
        );
    }

    #[test]
    fn move_into_a_deeper_parent_keeps_that_parents_indentation() {
        let src = "<node>\n    <a/>\n    <box>\n        <x/>\n    </box>\n</node>";
        let out = move_element(
            src.as_bytes(),
            span_of(src, "<a/>"),
            Dest::Before(span_of(src, "<x/>").start),
        )
        .expect("moves");
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "<node>\n    <box>\n        <a/>\n        <x/>\n    </box>\n</node>"
        );
    }

    /// A node written inline has no indentation to carry, and gaining one would
    /// reformat a line the author chose to keep tight.
    #[test]
    fn inline_siblings_stay_inline() {
        let src = "<node><a/><b/></node>";
        let out = move_element(
            src.as_bytes(),
            span_of(src, "<b/>"),
            Dest::Before(span_of(src, "<a/>").start),
        )
        .expect("moves");
        assert_eq!(String::from_utf8(out).unwrap(), "<node><b/><a/></node>");
    }

    /// Moving a subtree takes its children with it.
    #[test]
    fn move_carries_children() {
        let src = "<node>\n    <box>\n        <x/>\n    </box>\n    <a/>\n</node>";
        let a = span_of(src, "<a/>");
        let out = move_element(
            src.as_bytes(),
            span_of(src, "<box>\n        <x/>\n    </box>"),
            Dest::After {
                start: a.start,
                end: a.end,
            },
        )
        .expect("moves");
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "<node>\n    <a/>\n    <box>\n        <x/>\n    </box>\n</node>"
        );
    }

    /// The empty-container anchor, both ways round. `open_tag_close` points at
    /// the *first* byte of the closer, so `+1` is inside the element only when
    /// that closer is a plain `>`.
    #[test]
    fn empty_container_anchor_is_inside_only_for_a_paired_tag() {
        let paired = "<node><box></box></node>";
        let at = paired.find("></box>").unwrap() as u32; // the `>` of `<box>`
        assert_eq!(paired.as_bytes()[at as usize], b'>');
        let out = insert_element(paired.as_bytes(), Dest::Before(at + 1), "<x/>").expect("inserts");
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "<node><box><x/></box></node>"
        );

        // The self-closing case is the one `insert_node_in_markup` refuses: the
        // byte at `open_tag_close` is `/`, and stepping past it would land
        // between the `/` and the `>`.
        let selfclosed = "<node><box/></node>";
        let slash = selfclosed.find("/>").unwrap();
        assert_eq!(selfclosed.as_bytes()[slash], b'/');
    }

    /// A removal takes the line the element sat on with it, so deleting from a
    /// list does not leave a gap where the node was.
    #[test]
    fn remove_takes_the_elements_line() {
        let out = remove_element(TREE.as_bytes(), span_of(TREE, "<b/>")).expect("removes");
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "<node>\n    <a/>\n    <c/>\n</node>"
        );
    }

    #[test]
    fn remove_carries_children() {
        let src = "<node>\n    <box>\n        <x/>\n    </box>\n    <a/>\n</node>";
        let out = remove_element(
            src.as_bytes(),
            span_of(src, "<box>\n        <x/>\n    </box>"),
        )
        .expect("removes");
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "<node>\n    <a/>\n</node>"
        );
    }

    #[test]
    fn insert_before_a_sibling_takes_its_indentation() {
        let out = insert_element(
            TREE.as_bytes(),
            Dest::Before(span_of(TREE, "<b/>").start),
            "<x/>",
        )
        .expect("inserts");
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "<node>\n    <a/>\n    <x/>\n    <b/>\n    <c/>\n</node>"
        );
    }

    #[test]
    fn insert_after_the_last_child_appends() {
        let c = span_of(TREE, "<c/>");
        let out = insert_element(
            TREE.as_bytes(),
            Dest::After { start: c.start, end: c.end },
            "<x/>",
        )
        .expect("inserts");
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "<node>\n    <a/>\n    <b/>\n    <c/>\n    <x/>\n</node>"
        );
    }

    /// A snippet's own indentation is relative — it has to gain the
    /// destination's on top, or everything the palette inserts sits flush left
    /// no matter how deep it lands.
    #[test]
    fn a_multi_line_snippet_is_reindented_to_its_destination() {
        let src = "<node>\n    <box>\n        <x/>\n    </box>\n</node>";
        let out = insert_element(
            src.as_bytes(),
            Dest::Before(span_of(src, "<x/>").start),
            "<button>\n    <text>Go</text>\n</button>",
        )
        .expect("inserts");
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "<node>\n    <box>\n        <button>\n            <text>Go</text>\n        </button>\n        <x/>\n    </box>\n</node>"
        );
    }

    #[test]
    fn refuses_a_destination_inside_the_moved_element() {
        let src = "<node>\n    <box>\n        <x/>\n    </box>\n</node>";
        assert!(move_element(
            src.as_bytes(),
            span_of(src, "<box>\n        <x/>\n    </box>"),
            Dest::Before(span_of(src, "<x/>").start),
        )
        .is_none());
    }
}
