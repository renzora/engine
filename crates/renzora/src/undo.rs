//! Command-based undo/redo — the core, shared across the dlopen boundary.
//!
//! Every user action is an [`UndoCommand`]. Call sites do not mutate directly:
//! they build a command and hand it to [`execute`], which applies it and stores
//! it on the stack. Redo replays `execute`; undo runs the command's `undo`.
//!
//! ## Why the core is here and the commands are not
//!
//! An editing tool is the most obvious thing to ship as a plugin — a mesh
//! drawer, a shape library, a blueprint editor — and the one thing every one of
//! them must do is make its edits undoable. That was impossible while the whole
//! of undo lived in `renzora_undo`, which a plugin cannot link: it depends on
//! `renzora_editor_framework`, `renzora_ui`, `renzora_ember` and
//! `renzora_engine`.
//!
//! Those dependencies turn out to belong to the *concrete commands* — spawn a
//! shape, reparent, retitle a document tab — not to the machinery. The trait,
//! the context, the stacks and the four entry points below need nothing but
//! `bevy`, so they move here and a plugin can implement `UndoCommand` for its
//! own edits and push them onto the same history the editor shows.
//!
//! [`UndoStacks`] is a `Resource`, so unlike [`crate::net`] this is not about
//! process-global state — it is about the `TypeId`. A plugin with a private copy
//! of the trait and the resource would push onto a stack the editor's Ctrl+Z
//! never reads, which is worse than not recording at all: the edit looks
//! undoable and silently is not.
//!
//! `renzora_undo` keeps the plugin, the shortcut wiring and every concrete
//! command, and re-exports this module so existing `renzora_undo::execute`
//! paths still resolve.

use std::any::Any;
use std::collections::{HashMap, VecDeque};

use bevy::prelude::*;

#[derive(Clone, Debug, Eq, Hash, PartialEq, Default)]
pub enum UndoContext {
    #[default]
    Scene,
    MaterialGraph(String),
    Blueprint(String),
    Lifecycle,
    Other(String),
}

/// A single undoable action. `execute` is called on initial push AND on redo.
/// `undo` reverses the action. Both take `&mut self` so the command can
/// refresh captured state (e.g. update an entity id after respawn).
pub trait UndoCommand: Any + Send + Sync {
    fn label(&self) -> &str {
        "edit"
    }
    fn execute(&mut self, world: &mut World);
    fn undo(&mut self, world: &mut World);
    fn merge(&mut self, _other: &dyn UndoCommand) -> bool {
        false
    }
}

#[derive(Resource, Default)]
pub struct UndoStacks {
    stacks: HashMap<UndoContext, ContextStack>,
    pub active: UndoContext,
    /// Set by [`record`] whenever a `Scene` edit lands, cleared by the editor
    /// once it has marked the active document tab modified.
    ///
    /// A flag rather than the write itself: the tab lives in `renzora_ui`, which
    /// this crate cannot reach and a plugin cannot link. Leaving a bool for the
    /// editor to drain keeps the seam pure data — no callback to register, no
    /// function pointer to install, and a plugin that records an edit marks the
    /// scene dirty without knowing a document tab exists.
    pub scene_edited: bool,
}

#[derive(Default)]
struct ContextStack {
    undo: VecDeque<Box<dyn UndoCommand>>,
    redo: VecDeque<Box<dyn UndoCommand>>,
    /// When set, the next `record` will NOT merge into the back entry — it
    /// starts a fresh one instead. This is how a gesture boundary (a mouse
    /// release, a text-field commit) stops a *later* edit of the same field from
    /// folding into the earlier one. Reset to `false` on every push.
    sealed_back: bool,
}

/// How many entries one context's undo stack keeps before the oldest is
/// evicted.
///
/// Public because `renzora_undo` tests the eviction against it. A test that
/// hardcoded its own 500 would keep passing after this changed, asserting the
/// old cap against the new behaviour — which is the failure a shared constant
/// exists to prevent.
pub const MAX_DEPTH: usize = 500;

impl UndoStacks {
    pub fn clear(&mut self, context: &UndoContext) {
        if let Some(s) = self.stacks.get_mut(context) {
            s.undo.clear();
            s.redo.clear();
        }
    }
    pub fn clear_all(&mut self) {
        self.stacks.clear();
    }
    pub fn can_undo(&self, context: &UndoContext) -> bool {
        self.stacks.get(context).is_some_and(|s| !s.undo.is_empty())
    }
    pub fn can_redo(&self, context: &UndoContext) -> bool {
        self.stacks.get(context).is_some_and(|s| !s.redo.is_empty())
    }
    /// Returns `(undo_labels, redo_labels)` for the given context.
    /// `undo` is ordered front=oldest → back=most recent;
    /// `redo` is ordered front=oldest-undone → back=next-to-redo.
    pub fn labels(&self, context: &UndoContext) -> (Vec<String>, Vec<String>) {
        self.stacks
            .get(context)
            .map(|s| {
                (
                    s.undo.iter().map(|c| c.label().to_string()).collect(),
                    s.redo.iter().map(|c| c.label().to_string()).collect(),
                )
            })
            .unwrap_or_default()
    }

    /// Pop the most recent command off `context`'s undo stack, for the caller to
    /// run `undo` on and hand back via [`UndoStacks::push_redo`].
    ///
    /// Split this way because undoing needs `&mut World` while the stack is a
    /// resource *in* that world; the editor's `undo_once` owns that dance.
    pub fn pop_undo(&mut self, context: &UndoContext) -> Option<Box<dyn UndoCommand>> {
        self.stacks.get_mut(context).and_then(|s| s.undo.pop_back())
    }

    /// The mirror of [`UndoStacks::pop_undo`].
    pub fn pop_redo(&mut self, context: &UndoContext) -> Option<Box<dyn UndoCommand>> {
        self.stacks.get_mut(context).and_then(|s| s.redo.pop_back())
    }

    pub fn push_undo(&mut self, context: UndoContext, cmd: Box<dyn UndoCommand>) {
        self.stacks.entry(context).or_default().undo.push_back(cmd);
    }

    pub fn push_redo(&mut self, context: UndoContext, cmd: Box<dyn UndoCommand>) {
        self.stacks.entry(context).or_default().redo.push_back(cmd);
    }
}

/// Execute `cmd` and push it onto the active (or supplied) stack.
///
/// Prefer this over mutating the world directly — it's the single entry
/// point that keeps the history in sync with the session.
pub fn execute(world: &mut World, context: UndoContext, mut cmd: Box<dyn UndoCommand>) {
    cmd.execute(world);
    record(world, context, cmd);
}

/// Push `cmd` onto the stack WITHOUT executing it. Use when the mutation
/// has already happened via code that can't easily be expressed as a single
/// command (e.g. complex reparent with sibling index preservation).
pub fn record(world: &mut World, context: UndoContext, cmd: Box<dyn UndoCommand>) {
    let is_scene = matches!(context, UndoContext::Scene);
    // No stacks means no undo system — `UndoPlugin` is `Editor`-scoped, so a
    // shipped game has none. Recording history where nothing can replay it is
    // a no-op, not an error, and `resource_scope` would have made it a panic:
    // every caller but one is an `_editor` crate, but `markup::writeback` sits
    // in `renzora_ember`, which is compiled into the game. `seal` and the
    // `scene_edited` write below already tolerate this; only this half did not.
    if !world.contains_resource::<UndoStacks>() {
        return;
    }
    world.resource_scope(|_w, mut stacks: Mut<UndoStacks>| {
        let stack = stacks.stacks.entry(context).or_default();
        // A sealed back entry is a committed gesture — never merge into it.
        if !stack.sealed_back {
            if let Some(back) = stack.undo.back_mut() {
                if back.merge(cmd.as_ref()) {
                    stack.redo.clear();
                    return;
                }
            }
        }
        stack.undo.push_back(cmd);
        stack.sealed_back = false;
        stack.redo.clear();
        while stack.undo.len() > MAX_DEPTH {
            stack.undo.pop_front();
        }
    });
    if is_scene {
        // The editor drains this into the document tab's `is_modified`. See the
        // field's note for why it is a flag rather than the write.
        if let Some(mut stacks) = world.get_resource_mut::<UndoStacks>() {
            stacks.scene_edited = true;
        }
    }
}

/// Seal the back entry of `context` so the next `record` starts a fresh undo
/// step instead of merging. Call this at a gesture boundary — mouse release,
/// text-field commit — so two separate edits of the same field (e.g. scrub a
/// value, release, scrub it again) become two undo steps rather than one.
/// A no-op if the stack is empty or already sealed.
pub fn seal(world: &mut World, context: &UndoContext) {
    if let Some(mut stacks) = world.get_resource_mut::<UndoStacks>() {
        if let Some(stack) = stacks.stacks.get_mut(context) {
            stack.sealed_back = true;
        }
    }
}

/// The context Ctrl+Z currently targets. Panels that record edits for "the
/// thing the user is looking at" (the inspector, most tools) should push into
/// this rather than hard-coding `Scene`, so their edits land on the focused
/// document's stack. `route_undo_context` keeps it in sync with the UI.
pub fn active_context(world: &World) -> UndoContext {
    world
        .get_resource::<UndoStacks>()
        .map(|s| s.active.clone())
        .unwrap_or_default()
}
