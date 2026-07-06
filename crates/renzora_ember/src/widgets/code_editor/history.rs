//! Undo/redo — full-buffer snapshots with typing coalescing.
//!
//! The buffer is small enough (a source file) that snapshotting the whole
//! `Vec<String>` per undo step is simpler and less bug-prone than a diff/patch
//! log, and cheap in practice. The one refinement that matters for feel is
//! **coalescing**: a run of ordinary character inserts collapses into a single
//! undo step (so Ctrl+Z doesn't rewind one letter at a time), while structural
//! edits (newline, delete, paste, line moves) always start a fresh step. A
//! caret-only move also seals the current step so the *next* typing burst is
//! grouped on its own.
//!
//! The API is split into [`History::should_record`] / [`History::push`] /
//! [`History::note`] rather than one method so the editor can snapshot itself
//! (an immutable borrow) without aliasing the `History` field it mutates.

/// What kind of edit produced a snapshot, for coalescing decisions.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditKind {
    /// Ordinary character typing — coalesces with an immediately preceding type.
    Type,
    /// Backspace/Delete of a single char — coalesces with preceding deletes.
    Delete,
    /// Anything else (newline, paste, indent, line move, auto-close, …): never
    /// coalesces, always its own step.
    Other,
}

#[derive(Clone)]
pub(crate) struct Snapshot {
    pub text: Vec<String>,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub anchor_line: usize,
    pub anchor_col: usize,
}

/// Undo/redo stacks. `last` holds the previously recorded kind so a same-kind
/// follow-up can skip pushing a new step.
#[derive(Default)]
pub(crate) struct History {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    last: Option<EditKind>,
}

/// Cap on undo depth — plenty for editing, bounded so a long session can't grow
/// memory without limit.
const MAX_DEPTH: usize = 500;

impl History {
    /// Should an edit of `kind` push a fresh pre-edit snapshot? `false` when it
    /// coalesces with the previous same-kind edit (the earlier snapshot already
    /// marks the group's start).
    pub(crate) fn should_record(&self, kind: EditKind) -> bool {
        let coalesce = matches!(kind, EditKind::Type | EditKind::Delete) && self.last == Some(kind);
        !coalesce
    }

    /// Push a pre-edit snapshot as a new undo step; clears the redo stack (the
    /// timeline just forked). Call only when [`should_record`] returned `true`.
    pub(crate) fn push(&mut self, snap: Snapshot) {
        self.undo.push(snap);
        if self.undo.len() > MAX_DEPTH {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Remember the kind just applied (drives coalescing of the next edit).
    pub(crate) fn note(&mut self, kind: EditKind) {
        self.last = Some(kind);
    }

    /// Break coalescing so the next edit starts a new undo step (call on caret
    /// moves / focus changes).
    pub(crate) fn seal(&mut self) {
        self.last = None;
    }

    /// Reset both stacks (document switched underneath us).
    pub(crate) fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.last = None;
    }

    /// Pop one undo step, returning the snapshot to restore. `current` (the live
    /// state) is moved onto the redo stack so it can be re-applied — but only
    /// when there was something to undo.
    pub(crate) fn undo(&mut self, current: Snapshot) -> Option<Snapshot> {
        let snap = self.undo.pop()?;
        self.redo.push(current);
        self.last = None;
        Some(snap)
    }

    /// Pop one redo step; `current` is moved back onto the undo stack.
    pub(crate) fn redo(&mut self, current: Snapshot) -> Option<Snapshot> {
        let snap = self.redo.pop()?;
        self.undo.push(current);
        self.last = None;
        Some(snap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(text: &[&str], line: usize, col: usize) -> Snapshot {
        Snapshot {
            text: text.iter().map(|s| s.to_string()).collect(),
            cursor_line: line,
            cursor_col: col,
            anchor_line: line,
            anchor_col: col,
        }
    }

    /// Mimic the editor's `push_history`: record a pre-edit snapshot iff needed.
    fn record(h: &mut History, kind: EditKind, s: Snapshot) {
        if h.should_record(kind) {
            h.push(s);
        }
        h.note(kind);
    }

    #[test]
    fn typing_coalesces_into_one_step() {
        let mut h = History::default();
        record(&mut h, EditKind::Type, snap(&[""], 0, 0));
        record(&mut h, EditKind::Type, snap(&["a"], 0, 1));
        record(&mut h, EditKind::Type, snap(&["ab"], 0, 2));
        // Only the first snapshot was kept.
        let restored = h.undo(snap(&["abc"], 0, 3)).unwrap();
        assert_eq!(restored.text, vec!["".to_string()]);
        assert!(h.undo(snap(&[""], 0, 0)).is_none());
    }

    #[test]
    fn kind_change_starts_new_step() {
        let mut h = History::default();
        record(&mut h, EditKind::Type, snap(&[""], 0, 0));
        record(&mut h, EditKind::Other, snap(&["a"], 0, 1));
        assert_eq!(h.undo(snap(&["a\n"], 1, 0)).unwrap().text, vec!["a".to_string()]);
        assert_eq!(h.undo(snap(&["a"], 0, 1)).unwrap().text, vec!["".to_string()]);
    }

    #[test]
    fn seal_breaks_coalescing() {
        let mut h = History::default();
        record(&mut h, EditKind::Type, snap(&[""], 0, 0));
        h.seal();
        record(&mut h, EditKind::Type, snap(&["a"], 0, 1));
        assert_eq!(h.undo(snap(&["ab"], 0, 2)).unwrap().text, vec!["a".to_string()]);
    }

    #[test]
    fn redo_reapplies() {
        let mut h = History::default();
        record(&mut h, EditKind::Other, snap(&[""], 0, 0));
        let u = h.undo(snap(&["x"], 0, 1)).unwrap();
        assert_eq!(u.text, vec!["".to_string()]);
        let r = h.redo(snap(&[""], 0, 0)).unwrap();
        assert_eq!(r.text, vec!["x".to_string()]);
    }

    #[test]
    fn push_clears_redo() {
        let mut h = History::default();
        record(&mut h, EditKind::Other, snap(&[""], 0, 0));
        h.undo(snap(&["x"], 0, 1));
        record(&mut h, EditKind::Other, snap(&["y"], 0, 1));
        assert!(h.redo(snap(&["y"], 0, 1)).is_none());
    }
}
