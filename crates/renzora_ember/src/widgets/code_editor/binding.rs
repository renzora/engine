//! Two-way binding that drives a [`CodeEditor`] from host state (open files,
//! the active tab, save flags). Ember owns no document model — the host crate
//! supplies four closures and ember shuttles text in/out of the buffer:
//!
//! - `doc_key` identifies the visible document. When it changes, ember reloads
//!   the buffer from `load`, resets the cursor, and rebuilds the highlighter.
//! - `load` returns the current document's full text.
//! - `store` writes the edited buffer back (called the frame after an edit).
//! - `make_highlighter` builds the per-language highlighter for the current doc.
//!
//! [`code_sync`] is an exclusive system so the closures get free `&World` /
//! `&mut World` access; the [`CodeBinding`] component is taken out for the call
//! and re-inserted, avoiding aliasing with the `CodeEditor` it mutates.

use bevy::prelude::*;

use super::{CodeEditor, Highlighter};
use crate::reactive::Rx;

/// The host-supplied closures driving one bound [`CodeEditor`]. See the module
/// docs for the contract. `doc_key` must NOT depend on the document *content*
/// (only its identity, e.g. tab index + path) or every edit would reload.
pub struct CodeBindingSpec {
    pub doc_key: Box<dyn Fn(&Rx) -> u64 + Send + Sync>,
    pub load: Box<dyn Fn(&Rx) -> String + Send + Sync>,
    pub store: Box<dyn Fn(&mut World, &str) + Send + Sync>,
    pub make_highlighter: Box<dyn Fn(&Rx) -> Highlighter + Send + Sync>,
    /// Optional live code-font size (logical px), e.g. the host's zoom level. Read
    /// every sync and pushed onto the editor; `None` keeps the widget default.
    pub font_size: Option<Box<dyn Fn(&Rx) -> f32 + Send + Sync>>,
    /// Live word-wrap toggle (host setting). `None` leaves wrap off.
    pub word_wrap: Option<Box<dyn Fn(&Rx) -> bool + Send + Sync>>,
    /// Live whitespace-marker toggle (host setting).
    pub show_whitespace: Option<Box<dyn Fn(&Rx) -> bool + Send + Sync>>,
    /// Live auto-close-pairs toggle (host setting). `None` keeps the default on.
    pub auto_close: Option<Box<dyn Fn(&Rx) -> bool + Send + Sync>>,
    /// The active document's line-comment token (`"//"`, `"--"`, `"#"`, …) for
    /// the Ctrl+/ toggle. `None` disables comment toggling.
    pub line_comment: Option<Box<dyn Fn(&Rx) -> Option<String> + Send + Sync>>,
}

#[derive(Component)]
pub(crate) struct CodeBinding(pub CodeBindingSpec);

/// Attach a host-driven binding to an existing code editor (from
/// [`super::code_editor`]).
pub fn bind_code(commands: &mut Commands, editor: Entity, spec: CodeBindingSpec) {
    commands.entity(editor).insert(CodeBinding(spec));
}

/// Apply an edit made outside the editor to the document every code editor is
/// showing, line by line: `edit` returns a line's replacement, or `None` to
/// leave it.
///
/// For a change the host has already made to its own copy, such as a moved
/// asset's path being rewritten in an open script. The widget holds its own
/// buffer and reloads from the host only when the document changes, so without
/// this the next keystroke would push the old text back. Reloading instead
/// would throw away the caret, the folds and the undo history; editing in
/// place keeps all three, and the history is rewritten with the text so that
/// Ctrl+Z cannot restore what the edit replaced.
///
/// Not an edit as far as the host is concerned: the host's copy already agrees,
/// so nothing is pushed back and the document is not marked modified by it.
pub fn rewrite_code_lines(world: &mut World, edit: &dyn Fn(&str) -> Option<String>) {
    let mut editors = world.query::<&mut CodeEditor>();
    for mut ed in editors.iter_mut(world) {
        let mut changed = false;
        for line in &mut ed.text {
            if let Some(new) = edit(line) {
                *line = new;
                changed = true;
            }
        }
        ed.history.rewrite_lines(edit);
        if !changed {
            continue;
        }
        // A replacement can be shorter than what it replaced, so a caret past
        // the new end of its line would point at nothing.
        let cursor = super::edit::line_len(&ed, ed.cursor_line);
        let anchor = super::edit::line_len(&ed, ed.anchor_line);
        ed.cursor_col = ed.cursor_col.min(cursor);
        ed.anchor_col = ed.anchor_col.min(anchor);
        ed.dirty = true;
    }
}

fn split(text: String) -> Vec<String> {
    if text.is_empty() {
        vec![String::new()]
    } else {
        text.lines().map(|s| s.to_string()).collect()
    }
}

pub(crate) fn code_sync(world: &mut World) {
    let mut q = world.query_filtered::<Entity, (With<CodeBinding>, With<CodeEditor>)>();
    let ents: Vec<Entity> = q.iter(world).collect();
    for e in ents {
        // Take the binding so its closures can borrow the world without
        // aliasing the CodeEditor we mutate below; re-inserted before moving on.
        let Some(binding) = world.entity_mut(e).take::<CodeBinding>() else {
            continue;
        };
        let spec = &binding.0;

        let key = (spec.doc_key)(&Rx::new(&*world));
        let last = world.get::<CodeEditor>(e).and_then(|c| c.last_key);

        if last != Some(key) {
            // Document switched (or first mount): reload the buffer.
            let text = (spec.load)(&Rx::new(&*world));
            let hl = (spec.make_highlighter)(&Rx::new(&*world));
            if let Some(mut ed) = world.get_mut::<CodeEditor>(e) {
                ed.text = split(text);
                ed.cursor_line = 0;
                ed.cursor_col = 0;
                ed.anchor_line = 0;
                ed.anchor_col = 0;
                ed.scroll = 0;
                ed.goal_col = None;
                ed.last_key = Some(key);
                ed.content_dirty = false;
                ed.dirty = true;
                ed.highlighter = Some(hl);
                // A different document starts with a clean history and no folds
                // (fold anchors are line indices into the previous buffer).
                ed.reset_view_state();
            }
        } else if world.get::<CodeEditor>(e).is_some_and(|c| c.content_dirty) {
            // Same document, edited since last sync: push the buffer back.
            let joined = world.get::<CodeEditor>(e).map(|c| c.text.join("\n")).unwrap_or_default();
            (spec.store)(world, &joined);
            if let Some(mut ed) = world.get_mut::<CodeEditor>(e) {
                ed.content_dirty = false;
            }
        }

        // Push the host's live font size (zoom). `code_metrics` picks up the
        // change next and recomputes the derived metrics + flags a re-render.
        if let Some(fs) = spec.font_size.as_ref() {
            let size = fs(&Rx::new(&*world)).clamp(6.0, 48.0);
            if let Some(mut ed) = world.get_mut::<CodeEditor>(e) {
                if (ed.font_size - size).abs() > f32::EPSILON {
                    ed.font_size = size;
                }
            }
        }

        // Push the live view preferences (each read is a cheap resource lookup).
        let wrap = spec.word_wrap.as_ref().map(|f| f(&Rx::new(&*world)));
        let ws = spec.show_whitespace.as_ref().map(|f| f(&Rx::new(&*world)));
        let ac = spec.auto_close.as_ref().map(|f| f(&Rx::new(&*world)));
        let comment = spec.line_comment.as_ref().map(|f| f(&Rx::new(&*world)));
        if let Some(mut ed) = world.get_mut::<CodeEditor>(e) {
            if let Some(w) = wrap {
                if ed.wrap != w {
                    ed.wrap = w;
                    ed.dirty = true;
                }
            }
            if let Some(w) = ws {
                if ed.show_whitespace != w {
                    ed.show_whitespace = w;
                    ed.dirty = true;
                }
            }
            if let Some(a) = ac {
                ed.auto_close = a;
            }
            if let Some(c) = comment {
                ed.line_comment = c;
            }
        }

        world.entity_mut(e).insert(binding);
    }
}

#[cfg(test)]
mod tests {
    use super::super::code_editor_for_test as mk;
    use super::super::history::Snapshot;
    use super::*;

    fn rename(line: &str) -> Option<String> {
        line.contains("\"a.mp3\"").then(|| line.replace("\"a.mp3\"", "\"sfx/a.mp3\""))
    }

    #[test]
    fn the_text_and_its_history_both_follow() {
        let mut world = World::new();
        let mut ed = mk("x = 1\nplay(\"a.mp3\")");
        // An earlier state of the buffer, one Ctrl+Z away.
        ed.history.push(Snapshot {
            text: vec!["play(\"a.mp3\")".into()],
            cursor_line: 0,
            cursor_col: 0,
            anchor_line: 0,
            anchor_col: 0,
        });
        let e = world.spawn(ed).id();

        rewrite_code_lines(&mut world, &rename);

        let mut ed = world.get_mut::<CodeEditor>(e).unwrap();
        assert_eq!(ed.text, ["x = 1", "play(\"sfx/a.mp3\")"]);
        assert!(!ed.content_dirty, "the host already has the new text");
        let current = Snapshot {
            text: ed.text.clone(),
            cursor_line: 0,
            cursor_col: 0,
            anchor_line: 0,
            anchor_col: 0,
        };
        let undone = ed.history.undo(current).unwrap();
        assert_eq!(undone.text, ["play(\"sfx/a.mp3\")"], "undo must not bring the old path back");
    }

    #[test]
    fn a_caret_past_a_shortened_line_is_pulled_back() {
        let mut world = World::new();
        let mut ed = mk("play(\"long/path.mp3\")");
        ed.cursor_col = 21;
        ed.anchor_col = 21;
        let e = world.spawn(ed).id();

        rewrite_code_lines(&mut world, &|l| Some(l.replace("long/path.mp3", "p.mp3")));

        let ed = world.get::<CodeEditor>(e).unwrap();
        assert_eq!(ed.text[0], "play(\"p.mp3\")");
        assert_eq!(ed.cursor_col, 13);
        assert_eq!(ed.anchor_col, 13);
    }
}
