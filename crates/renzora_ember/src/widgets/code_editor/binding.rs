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

/// The host-supplied closures driving one bound [`CodeEditor`]. See the module
/// docs for the contract. `doc_key` must NOT depend on the document *content*
/// (only its identity, e.g. tab index + path) or every edit would reload.
pub struct CodeBindingSpec {
    pub doc_key: Box<dyn Fn(&World) -> u64 + Send + Sync>,
    pub load: Box<dyn Fn(&World) -> String + Send + Sync>,
    pub store: Box<dyn Fn(&mut World, &str) + Send + Sync>,
    pub make_highlighter: Box<dyn Fn(&World) -> Highlighter + Send + Sync>,
}

#[derive(Component)]
pub(crate) struct CodeBinding(pub CodeBindingSpec);

/// Attach a host-driven binding to an existing code editor (from
/// [`super::code_editor`]).
pub fn bind_code(commands: &mut Commands, editor: Entity, spec: CodeBindingSpec) {
    commands.entity(editor).insert(CodeBinding(spec));
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

        let key = (spec.doc_key)(world);
        let last = world.get::<CodeEditor>(e).and_then(|c| c.last_key);

        if last != Some(key) {
            // Document switched (or first mount): reload the buffer.
            let text = (spec.load)(world);
            let hl = (spec.make_highlighter)(world);
            if let Some(mut ed) = world.get_mut::<CodeEditor>(e) {
                ed.text = split(text);
                ed.cursor_line = 0;
                ed.cursor_col = 0;
                ed.anchor_line = 0;
                ed.anchor_col = 0;
                ed.scroll = 0;
                ed.last_key = Some(key);
                ed.content_dirty = false;
                ed.dirty = true;
                ed.highlighter = Some(hl);
            }
        } else if world.get::<CodeEditor>(e).is_some_and(|c| c.content_dirty) {
            // Same document, edited since last sync: push the buffer back.
            let joined = world.get::<CodeEditor>(e).map(|c| c.text.join("\n")).unwrap_or_default();
            (spec.store)(world, &joined);
            if let Some(mut ed) = world.get_mut::<CodeEditor>(e) {
                ed.content_dirty = false;
            }
        }

        world.entity_mut(e).insert(binding);
    }
}
