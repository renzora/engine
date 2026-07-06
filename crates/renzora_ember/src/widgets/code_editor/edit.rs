//! Document editing — every mutation and cursor motion the editor supports,
//! applied to the [`CodeEditor`] buffer/cursor/selection.
//!
//! Plain typing and caret motion arrive as logical keys through [`edit`]; the
//! discrete commands (undo, clipboard, comment toggle, line move/duplicate,
//! block indent) are separate entry points called from [`super::systems`] when
//! it recognises a chord. All edits funnel through [`CodeEditor::push_history`]
//! so undo/redo and coalescing stay consistent, and through [`after_edit`] so
//! the cursor is kept on-screen and fold anchors stay valid.

use bevy::input::keyboard::Key;

use super::folding;
use super::history::EditKind;
use super::layout::{self, char_len};
use super::{CodeEditor, TAB_WIDTH};

// ---------------------------------------------------------------------------
// Selection / query helpers
// ---------------------------------------------------------------------------

pub(crate) fn has_selection(ed: &CodeEditor) -> bool {
    (ed.anchor_line, ed.anchor_col) != (ed.cursor_line, ed.cursor_col)
}

/// The selection as ordered `((start_line, start_col), (end_line, end_col))`.
pub(crate) fn sel_range(ed: &CodeEditor) -> ((usize, usize), (usize, usize)) {
    let a = (ed.anchor_line, ed.anchor_col);
    let c = (ed.cursor_line, ed.cursor_col);
    if a <= c {
        (a, c)
    } else {
        (c, a)
    }
}

fn chars(ed: &CodeEditor, line: usize) -> Vec<char> {
    ed.text.get(line).map(|l| l.chars().collect()).unwrap_or_default()
}

pub(crate) fn line_len(ed: &CodeEditor, i: usize) -> usize {
    char_len(&ed.text, i)
}

/// A word character for word-wise motion/selection (identifier-ish).
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

// ---------------------------------------------------------------------------
// History / lifecycle plumbing
// ---------------------------------------------------------------------------

impl CodeEditor {
    /// Record a pre-edit snapshot for undo (respecting coalescing) before a
    /// mutation of `kind`.
    pub(crate) fn push_history(&mut self, kind: EditKind) {
        if self.history.should_record(kind) {
            let snap = self.snapshot();
            self.history.push(snap);
        }
        self.history.note(kind);
    }

    fn do_undo(&mut self) {
        let cur = self.snapshot();
        if let Some(s) = self.history.undo(cur) {
            self.restore(s);
            after_move_only(self);
        }
    }

    fn do_redo(&mut self) {
        let cur = self.snapshot();
        if let Some(s) = self.history.redo(cur) {
            self.restore(s);
            after_move_only(self);
        }
    }
}

/// Finish a pure caret move: collapse-or-extend already done by the caller.
/// Seals the undo group (so the next typing burst is its own step) and keeps the
/// caret visible.
fn after_move_only(ed: &mut CodeEditor) {
    ed.history.seal();
    ed.dirty = true;
    ensure_visible(ed);
}

/// Finish a text edit: collapse the selection onto the caret, mark the buffer
/// dirty, clear the sticky vertical goal, keep the caret visible.
fn after_edit(ed: &mut CodeEditor) {
    ed.anchor_line = ed.cursor_line;
    ed.anchor_col = ed.cursor_col;
    ed.goal_col = None;
    ed.content_dirty = true;
    ed.dirty = true;
    ensure_visible(ed);
}

// ---------------------------------------------------------------------------
// Bracket matching (unchanged logic, still used by the render overlay)
// ---------------------------------------------------------------------------

/// If the caret sits next to a bracket, returns that bracket's cell and its
/// matching bracket's cell as `((line, col), (line, col))`, for highlighting.
/// Prefers the bracket immediately *left* of the caret, then the one to the
/// right. Scans nesting-aware across lines, bounded so a huge unbalanced file
/// can't stall the render.
pub(crate) fn bracket_match(ed: &CodeEditor) -> Option<((usize, usize), (usize, usize))> {
    const SCAN_CAP: usize = 100_000;
    const OPEN: [char; 3] = ['(', '[', '{'];
    const CLOSE: [char; 3] = [')', ']', '}'];

    let cur: Vec<char> = ed.text.get(ed.cursor_line)?.chars().collect();
    let pick = |col: usize| -> Option<(usize, char)> {
        cur.get(col).copied().filter(|c| OPEN.contains(c) || CLOSE.contains(c)).map(|c| (col, c))
    };
    let (bcol, bch) = ed.cursor_col.checked_sub(1).and_then(&pick).or_else(|| pick(ed.cursor_col))?;

    let (open_ch, close_ch) = match bch {
        '(' | ')' => ('(', ')'),
        '[' | ']' => ('[', ']'),
        _ => ('{', '}'),
    };
    let forward = OPEN.contains(&bch);
    let mut depth = 0i32;
    let mut scanned = 0usize;
    let here = (ed.cursor_line, bcol);

    if forward {
        let (mut l, mut c) = (ed.cursor_line, bcol);
        loop {
            let chars: Vec<char> = ed.text[l].chars().collect();
            while c < chars.len() {
                match chars[c] {
                    ch if ch == open_ch => depth += 1,
                    ch if ch == close_ch => {
                        depth -= 1;
                        if depth == 0 {
                            return Some((here, (l, c)));
                        }
                    }
                    _ => {}
                }
                c += 1;
                scanned += 1;
                if scanned > SCAN_CAP {
                    return None;
                }
            }
            l += 1;
            if l >= ed.text.len() {
                return None;
            }
            c = 0;
        }
    } else {
        let (mut l, mut c) = (ed.cursor_line, bcol as isize);
        loop {
            let chars: Vec<char> = ed.text[l].chars().collect();
            while c >= 0 {
                match chars[c as usize] {
                    ch if ch == close_ch => depth += 1,
                    ch if ch == open_ch => {
                        depth -= 1;
                        if depth == 0 {
                            return Some((here, (l, c as usize)));
                        }
                    }
                    _ => {}
                }
                c -= 1;
                scanned += 1;
                if scanned > SCAN_CAP {
                    return None;
                }
            }
            if l == 0 {
                return None;
            }
            l -= 1;
            c = ed.text[l].chars().count() as isize - 1;
        }
    }
}

// ---------------------------------------------------------------------------
// Key dispatch (typing + caret motion)
// ---------------------------------------------------------------------------

/// Apply one logical key. `ctrl` folds in Super/Cmd; it turns arrows/Home/End
/// into word/document motion and Backspace/Delete into word deletes. Chord
/// commands (Ctrl+Z, Ctrl+/, Alt+Up, …) are handled upstream in
/// [`super::systems::code_input`], not here.
pub(crate) fn edit(ed: &mut CodeEditor, key: &Key, shift: bool, ctrl: bool) {
    match key {
        Key::Character(s) => {
            if ctrl {
                return; // a Ctrl+letter chord, not text
            }
            for c in s.chars() {
                if !c.is_control() {
                    type_char(ed, c);
                }
            }
        }
        Key::Space if !ctrl => type_char(ed, ' '),
        Key::Tab if !ctrl => {
            if shift {
                dedent(ed);
            } else {
                tab(ed);
            }
        }
        Key::Enter if !ctrl => newline(ed),
        Key::Backspace => {
            if has_selection(ed) {
                ed.push_history(EditKind::Other);
                delete_selection(ed);
                after_edit(ed);
            } else if ctrl {
                delete_word_left(ed);
            } else {
                ed.push_history(EditKind::Delete);
                backspace(ed);
                after_edit(ed);
            }
        }
        Key::Delete => {
            if has_selection(ed) {
                ed.push_history(EditKind::Other);
                delete_selection(ed);
                after_edit(ed);
            } else if ctrl {
                delete_word_right(ed);
            } else {
                ed.push_history(EditKind::Delete);
                delete_fwd(ed);
                after_edit(ed);
            }
        }
        Key::ArrowLeft => move_horizontal(ed, false, shift, ctrl),
        Key::ArrowRight => move_horizontal(ed, true, shift, ctrl),
        Key::ArrowUp => move_vertical(ed, -1, shift),
        Key::ArrowDown => move_vertical(ed, 1, shift),
        Key::Home => move_home(ed, shift, ctrl),
        Key::End => move_end(ed, shift, ctrl),
        Key::PageUp => page(ed, -1, shift),
        Key::PageDown => page(ed, 1, shift),
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Caret motion
// ---------------------------------------------------------------------------

fn move_horizontal(ed: &mut CodeEditor, right: bool, shift: bool, ctrl: bool) {
    // A plain (unshifted) arrow over a selection collapses to its near edge.
    if !shift && has_selection(ed) {
        let ((sl, sc), (el, ec)) = sel_range(ed);
        let (l, c) = if right { (el, ec) } else { (sl, sc) };
        ed.cursor_line = l;
        ed.cursor_col = c;
        ed.anchor_line = l;
        ed.anchor_col = c;
        ed.goal_col = Some(c);
        after_move_only(ed);
        return;
    }
    if right {
        let len = line_len(ed, ed.cursor_line);
        if ctrl {
            ed.cursor_col = word_right_col(&chars(ed, ed.cursor_line), ed.cursor_col);
        } else if ed.cursor_col < len {
            ed.cursor_col += 1;
        } else if let Some(n) = layout::next_visible(&ed.folds, &ed.text, ed.cursor_line) {
            ed.cursor_line = n;
            ed.cursor_col = 0;
        }
    } else if ctrl {
        ed.cursor_col = word_left_col(&chars(ed, ed.cursor_line), ed.cursor_col);
    } else if ed.cursor_col > 0 {
        ed.cursor_col -= 1;
    } else if let Some(p) = layout::prev_visible(&ed.folds, ed.cursor_line) {
        ed.cursor_line = p;
        ed.cursor_col = line_len(ed, p);
    }
    if !shift {
        ed.anchor_line = ed.cursor_line;
        ed.anchor_col = ed.cursor_col;
    }
    ed.goal_col = Some(ed.cursor_col);
    after_move_only(ed);
}

fn move_vertical(ed: &mut CodeEditor, dir: i32, shift: bool) {
    // Sticky column: remember the target x so moving through shorter lines
    // returns to it, like every editor.
    let goal = ed.goal_col.unwrap_or(ed.cursor_col);
    let target = if dir < 0 {
        layout::prev_visible(&ed.folds, ed.cursor_line)
    } else {
        layout::next_visible(&ed.folds, &ed.text, ed.cursor_line)
    };
    if let Some(t) = target {
        ed.cursor_line = t;
        ed.cursor_col = goal.min(line_len(ed, t));
    }
    ed.goal_col = Some(goal);
    if !shift {
        ed.anchor_line = ed.cursor_line;
        ed.anchor_col = ed.cursor_col;
    }
    ed.history.seal();
    ed.dirty = true;
    ensure_visible(ed);
}

fn move_home(ed: &mut CodeEditor, shift: bool, ctrl: bool) {
    if ctrl {
        ed.cursor_line = 0;
        ed.cursor_col = 0;
    } else {
        // Smart Home: first non-whitespace, toggling to column 0 when already
        // there.
        let cs = chars(ed, ed.cursor_line);
        let first_ns = cs.iter().position(|c| !c.is_whitespace()).unwrap_or(0);
        ed.cursor_col = if ed.cursor_col == first_ns { 0 } else { first_ns };
    }
    if !shift {
        ed.anchor_line = ed.cursor_line;
        ed.anchor_col = ed.cursor_col;
    }
    ed.goal_col = Some(ed.cursor_col);
    after_move_only(ed);
}

fn move_end(ed: &mut CodeEditor, shift: bool, ctrl: bool) {
    if ctrl {
        ed.cursor_line = ed.text.len().saturating_sub(1);
    }
    ed.cursor_col = line_len(ed, ed.cursor_line);
    if !shift {
        ed.anchor_line = ed.cursor_line;
        ed.anchor_col = ed.cursor_col;
    }
    ed.goal_col = Some(ed.cursor_col);
    after_move_only(ed);
}

fn page(ed: &mut CodeEditor, dir: i32, shift: bool) {
    let goal = ed.goal_col.unwrap_or(ed.cursor_col);
    let steps = ed.visible.max(1);
    for _ in 0..steps {
        let next = if dir < 0 {
            layout::prev_visible(&ed.folds, ed.cursor_line)
        } else {
            layout::next_visible(&ed.folds, &ed.text, ed.cursor_line)
        };
        match next {
            Some(l) => ed.cursor_line = l,
            None => break,
        }
    }
    ed.cursor_col = goal.min(line_len(ed, ed.cursor_line));
    ed.goal_col = Some(goal);
    if !shift {
        ed.anchor_line = ed.cursor_line;
        ed.anchor_col = ed.cursor_col;
    }
    ed.history.seal();
    ed.dirty = true;
    ensure_visible(ed);
}

fn word_left_col(cs: &[char], col: usize) -> usize {
    let mut c = col.min(cs.len());
    while c > 0 && cs[c - 1].is_whitespace() {
        c -= 1;
    }
    if c > 0 {
        let word = is_word(cs[c - 1]);
        while c > 0 && is_word(cs[c - 1]) == word && !cs[c - 1].is_whitespace() {
            c -= 1;
        }
    }
    c
}

fn word_right_col(cs: &[char], col: usize) -> usize {
    let n = cs.len();
    let mut c = col.min(n);
    if c < n {
        let word = is_word(cs[c]);
        while c < n && is_word(cs[c]) == word && !cs[c].is_whitespace() {
            c += 1;
        }
    }
    while c < n && cs[c].is_whitespace() {
        c += 1;
    }
    c
}

// ---------------------------------------------------------------------------
// Character insertion (with auto-close pairs)
// ---------------------------------------------------------------------------

/// Closing partner for an auto-close opener, if any.
fn close_of(c: char) -> Option<char> {
    match c {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        '"' => Some('"'),
        '\'' => Some('\''),
        '`' => Some('`'),
        _ => None,
    }
}

fn is_closer(c: char) -> bool {
    matches!(c, ')' | ']' | '}' | '"' | '\'' | '`')
}

/// Insert one typed character, applying auto-close-pair behaviour when enabled:
/// wrap a selection, type over an existing closer, or insert the matching pair.
fn type_char(ed: &mut CodeEditor, c: char) {
    ed.push_history(EditKind::Type);

    if ed.auto_close && has_selection(ed) {
        if let Some(close) = close_of(c) {
            wrap_selection(ed, c, close);
            ed.content_dirty = true;
            ed.dirty = true;
            ed.goal_col = None;
            ensure_visible(ed);
            return;
        }
    }

    delete_selection(ed);

    if ed.auto_close {
        let after = chars(ed, ed.cursor_line);
        let next = after.get(ed.cursor_col).copied();
        // Type over the closer we (or the user) already placed.
        if is_closer(c) && next == Some(c) {
            ed.cursor_col += 1;
            after_edit(ed);
            return;
        }
        if let Some(close) = close_of(c) {
            let prev = ed.cursor_col.checked_sub(1).and_then(|i| after.get(i)).copied();
            // Only pair when it won't be a nuisance: next cell is empty / a
            // separator, and for quotes the previous cell isn't a word char
            // (avoids pairing on `isn't`, identifiers, etc.).
            let next_ok = next.is_none_or(|n| n.is_whitespace() || is_closer(n) || n == ',' || n == ';');
            let quote_ok = !(close == c && prev.is_some_and(is_word));
            if next_ok && quote_ok {
                insert_char(ed, c);
                insert_char(ed, close);
                ed.cursor_col -= 1; // sit between the pair
                after_edit(ed);
                return;
            }
        }
    }

    insert_char(ed, c);
    after_edit(ed);
}

/// Wrap the current selection in `open`/`close`, keeping the inner text
/// selected.
fn wrap_selection(ed: &mut CodeEditor, open: char, close: char) {
    let ((sl, sc), (el, ec)) = sel_range(ed);
    // Insert the closer first so the opener insertion can't shift its position.
    insert_at(ed, el, ec, close);
    insert_at(ed, sl, sc, open);
    // New selection spans the original text, now offset by the opener.
    ed.anchor_line = sl;
    ed.anchor_col = sc + 1;
    ed.cursor_line = el;
    ed.cursor_col = if sl == el { ec + 1 } else { ec };
}

fn insert_at(ed: &mut CodeEditor, line: usize, col: usize, c: char) {
    let mut cs = chars(ed, line);
    let col = col.min(cs.len());
    cs.insert(col, c);
    ed.text[line] = cs.into_iter().collect();
}

fn insert_char(ed: &mut CodeEditor, c: char) {
    let line = ed.cursor_line;
    let mut cs = chars(ed, line);
    let col = ed.cursor_col.min(cs.len());
    cs.insert(col, c);
    ed.text[line] = cs.into_iter().collect();
    ed.cursor_col = col + 1;
}

// ---------------------------------------------------------------------------
// Newline with auto-indent
// ---------------------------------------------------------------------------

fn leading_ws(line: &str) -> String {
    line.chars().take_while(|c| *c == ' ' || *c == '\t').collect()
}

fn newline(ed: &mut CodeEditor) {
    ed.push_history(EditKind::Other);
    delete_selection(ed);
    let line = ed.cursor_line;
    let cs = chars(ed, line);
    let col = ed.cursor_col.min(cs.len());
    let head: String = cs[..col].iter().collect();
    let tail: String = cs[col..].iter().collect();

    // Carry the current line's indentation onto the new line; add a level after
    // an opening bracket, and if the very next char is the matching closer,
    // push it down a further, dedented line (the classic `{|}` → expand).
    let indent = leading_ws(&head);
    let opens = head.trim_end().ends_with(['{', '(', '[']);
    let closes_next = tail.starts_with([']', ')', '}']);

    ed.text[line] = head;
    if opens && closes_next {
        let inner = format!("{indent}{}", " ".repeat(TAB_WIDTH));
        ed.text.insert(line + 1, inner.clone());
        ed.text.insert(line + 2, format!("{indent}{tail}"));
        folding::shift_insert(&mut ed.folds, line + 1, 2);
        ed.cursor_line = line + 1;
        ed.cursor_col = inner.chars().count();
    } else {
        let new = if opens {
            format!("{indent}{}{tail}", " ".repeat(TAB_WIDTH))
        } else {
            format!("{indent}{tail}")
        };
        let indent_len = if opens { indent.chars().count() + TAB_WIDTH } else { indent.chars().count() };
        ed.text.insert(line + 1, new);
        folding::shift_insert(&mut ed.folds, line + 1, 1);
        ed.cursor_line = line + 1;
        ed.cursor_col = indent_len;
    }
    after_edit(ed);
}

// ---------------------------------------------------------------------------
// Deletion
// ---------------------------------------------------------------------------

fn delete_selection(ed: &mut CodeEditor) {
    if !has_selection(ed) {
        return;
    }
    let ((sl, sc), (el, ec)) = sel_range(ed);
    if sl == el {
        let mut cs = chars(ed, sl);
        cs.drain(sc..ec.min(cs.len()));
        ed.text[sl] = cs.into_iter().collect();
    } else {
        let head: String = chars(ed, sl).into_iter().take(sc).collect();
        let tail: String = chars(ed, el).into_iter().skip(ec).collect();
        ed.text.drain(sl + 1..=el);
        ed.text[sl] = head + tail.as_str();
        folding::shift_remove(&mut ed.folds, sl + 1, el - sl);
    }
    ed.cursor_line = sl;
    ed.cursor_col = sc;
    ed.anchor_line = sl;
    ed.anchor_col = sc;
}

fn backspace(ed: &mut CodeEditor) {
    if ed.cursor_col > 0 {
        let line = ed.cursor_line;
        let mut cs = chars(ed, line);
        // Delete an empty auto-close pair in one press: `(|)` → ``.
        let left = cs[ed.cursor_col - 1];
        let right = cs.get(ed.cursor_col).copied();
        if close_of(left) == right && right.is_some() {
            cs.remove(ed.cursor_col);
        }
        cs.remove(ed.cursor_col - 1);
        ed.text[line] = cs.into_iter().collect();
        ed.cursor_col -= 1;
    } else if ed.cursor_line > 0 {
        let cur = ed.text.remove(ed.cursor_line);
        folding::shift_remove(&mut ed.folds, ed.cursor_line, 1);
        ed.cursor_line -= 1;
        ed.cursor_col = line_len(ed, ed.cursor_line);
        ed.text[ed.cursor_line].push_str(&cur);
    }
}

fn delete_fwd(ed: &mut CodeEditor) {
    let line = ed.cursor_line;
    if ed.cursor_col < line_len(ed, line) {
        let mut cs = chars(ed, line);
        cs.remove(ed.cursor_col);
        ed.text[line] = cs.into_iter().collect();
    } else if line + 1 < ed.text.len() {
        let next = ed.text.remove(line + 1);
        folding::shift_remove(&mut ed.folds, line + 1, 1);
        ed.text[line].push_str(&next);
    }
}

fn delete_word_left(ed: &mut CodeEditor) {
    if ed.cursor_col == 0 {
        ed.push_history(EditKind::Delete);
        backspace(ed);
        after_edit(ed);
        return;
    }
    ed.push_history(EditKind::Other);
    let cs = chars(ed, ed.cursor_line);
    let target = word_left_col(&cs, ed.cursor_col);
    let mut cs = cs;
    cs.drain(target..ed.cursor_col);
    ed.text[ed.cursor_line] = cs.into_iter().collect();
    ed.cursor_col = target;
    after_edit(ed);
}

fn delete_word_right(ed: &mut CodeEditor) {
    let len = line_len(ed, ed.cursor_line);
    if ed.cursor_col >= len {
        ed.push_history(EditKind::Delete);
        delete_fwd(ed);
        after_edit(ed);
        return;
    }
    ed.push_history(EditKind::Other);
    let cs = chars(ed, ed.cursor_line);
    let target = word_right_col(&cs, ed.cursor_col);
    let mut cs = cs;
    cs.drain(ed.cursor_col..target);
    ed.text[ed.cursor_line] = cs.into_iter().collect();
    after_edit(ed);
}

// ---------------------------------------------------------------------------
// Tab / block indent
// ---------------------------------------------------------------------------

fn tab(ed: &mut CodeEditor) {
    let ((sl, _), (el, _)) = sel_range(ed);
    if has_selection(ed) && el > sl {
        indent_selection(ed);
        return;
    }
    ed.push_history(EditKind::Other);
    delete_selection(ed);
    // Align to the next tab stop rather than always inserting four.
    let n = TAB_WIDTH - (ed.cursor_col % TAB_WIDTH);
    for _ in 0..n {
        insert_char(ed, ' ');
    }
    after_edit(ed);
}

fn indent_selection(ed: &mut CodeEditor) {
    ed.push_history(EditKind::Other);
    let ((sl, _), (el, _)) = sel_range(ed);
    let pad = " ".repeat(TAB_WIDTH);
    for l in sl..=el {
        // Skip blank lines so we don't add trailing whitespace.
        if !ed.text[l].trim().is_empty() {
            ed.text[l].insert_str(0, &pad);
        }
    }
    shift_cols_for_indent(ed, sl, el, TAB_WIDTH as isize);
    ed.content_dirty = true;
    ed.dirty = true;
    ed.goal_col = None;
    ensure_visible(ed);
}

fn dedent(ed: &mut CodeEditor) {
    ed.push_history(EditKind::Other);
    let ((sl, _), (el, _)) = if has_selection(ed) {
        sel_range(ed)
    } else {
        ((ed.cursor_line, 0), (ed.cursor_line, 0))
    };
    let mut removed_cursor = 0usize;
    let mut removed_anchor = 0usize;
    for l in sl..=el {
        let cs = chars(ed, l);
        let removable = cs.iter().take(TAB_WIDTH).take_while(|c| **c == ' ').count();
        if removable > 0 {
            let rest: String = cs[removable..].iter().collect();
            ed.text[l] = rest;
        }
        if l == ed.cursor_line {
            removed_cursor = removable;
        }
        if l == ed.anchor_line {
            removed_anchor = removable;
        }
    }
    ed.cursor_col = ed.cursor_col.saturating_sub(removed_cursor);
    ed.anchor_col = ed.anchor_col.saturating_sub(removed_anchor);
    ed.content_dirty = true;
    ed.dirty = true;
    ed.goal_col = None;
    ensure_visible(ed);
}

/// Shift the cursor/anchor columns after indenting lines `sl..=el` by `delta`
/// columns (only the endpoints on selected lines need adjusting).
fn shift_cols_for_indent(ed: &mut CodeEditor, sl: usize, el: usize, delta: isize) {
    let adjust = |line: usize, col: usize, blank: bool| -> usize {
        if line >= sl && line <= el && !blank {
            (col as isize + delta).max(0) as usize
        } else {
            col
        }
    };
    let cursor_blank = ed.text[ed.cursor_line].trim().is_empty();
    let anchor_blank = ed.text[ed.anchor_line].trim().is_empty();
    ed.cursor_col = adjust(ed.cursor_line, ed.cursor_col, cursor_blank);
    ed.anchor_col = adjust(ed.anchor_line, ed.anchor_col, anchor_blank);
}

// ---------------------------------------------------------------------------
// Discrete commands (called from systems on chords)
// ---------------------------------------------------------------------------

pub(crate) fn undo(ed: &mut CodeEditor) {
    ed.do_undo();
}

pub(crate) fn redo(ed: &mut CodeEditor) {
    ed.do_redo();
}

pub(crate) fn select_all(ed: &mut CodeEditor) {
    ed.anchor_line = 0;
    ed.anchor_col = 0;
    ed.cursor_line = ed.text.len().saturating_sub(1);
    ed.cursor_col = line_len(ed, ed.cursor_line);
    ed.history.seal();
    ed.dirty = true;
}

/// The text to copy: the selection, or (VSCode-style) the whole current line
/// plus its newline when there's no selection.
pub(crate) fn copy_text(ed: &CodeEditor) -> String {
    if has_selection(ed) {
        selected_text(ed)
    } else {
        format!("{}\n", ed.text[ed.cursor_line])
    }
}

fn selected_text(ed: &CodeEditor) -> String {
    let ((sl, sc), (el, ec)) = sel_range(ed);
    if sl == el {
        chars(ed, sl).into_iter().skip(sc).take(ec - sc).collect()
    } else {
        let mut out: String = chars(ed, sl).into_iter().skip(sc).collect();
        for l in sl + 1..el {
            out.push('\n');
            out.push_str(&ed.text[l]);
        }
        out.push('\n');
        let last: String = chars(ed, el).into_iter().take(ec).collect();
        out.push_str(&last);
        out
    }
}

pub(crate) fn cut(ed: &mut CodeEditor) -> String {
    let text = copy_text(ed);
    ed.push_history(EditKind::Other);
    if has_selection(ed) {
        delete_selection(ed);
    } else {
        // No selection → cut the whole line.
        delete_current_line(ed);
    }
    after_edit(ed);
    text
}

pub(crate) fn paste(ed: &mut CodeEditor, s: &str) {
    if s.is_empty() {
        return;
    }
    ed.push_history(EditKind::Other);
    delete_selection(ed);
    insert_text(ed, s);
    after_edit(ed);
}

// --- OS clipboard bridge (kept here so `arboard` stays out of the ECS glue) ---

/// Ctrl+C: push the selection (or current line) to the system clipboard.
pub(crate) fn clipboard_copy(ed: &CodeEditor) {
    clipboard_set(&copy_text(ed));
}

/// Ctrl+X: copy then remove.
pub(crate) fn clipboard_cut(ed: &mut CodeEditor) {
    let text = cut(ed);
    clipboard_set(&text);
}

/// Ctrl+V: paste the system clipboard at the caret (replacing any selection).
pub(crate) fn clipboard_paste(ed: &mut CodeEditor) {
    if let Some(text) = clipboard_get() {
        // Normalise CRLF so pasted Windows text doesn't leave stray `\r`.
        paste(ed, &text.replace("\r\n", "\n").replace('\r', "\n"));
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn clipboard_set(s: &str) {
    if let Ok(mut cb) = arboard::Clipboard::new() {
        let _ = cb.set_text(s.to_string());
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn clipboard_get() -> Option<String> {
    arboard::Clipboard::new().ok().and_then(|mut cb| cb.get_text().ok())
}

// The browser clipboard is async-only and gated behind a user gesture, so the
// synchronous editor path just no-ops on wasm (copy/paste fall back to nothing).
#[cfg(target_arch = "wasm32")]
fn clipboard_set(_s: &str) {}

#[cfg(target_arch = "wasm32")]
fn clipboard_get() -> Option<String> {
    None
}

fn insert_text(ed: &mut CodeEditor, s: &str) {
    let parts: Vec<&str> = s.split('\n').collect();
    let line = ed.cursor_line;
    let cs = chars(ed, line);
    let col = ed.cursor_col.min(cs.len());
    let head: String = cs[..col].iter().collect();
    let tail: String = cs[col..].iter().collect();
    if parts.len() == 1 {
        ed.text[line] = format!("{head}{}{tail}", parts[0]);
        ed.cursor_col = col + parts[0].chars().count();
    } else {
        ed.text[line] = format!("{head}{}", parts[0]);
        let mut insert_idx = line + 1;
        for mid in &parts[1..parts.len() - 1] {
            ed.text.insert(insert_idx, mid.to_string());
            insert_idx += 1;
        }
        let last = parts[parts.len() - 1];
        ed.text.insert(insert_idx, format!("{last}{tail}"));
        folding::shift_insert(&mut ed.folds, line + 1, parts.len() - 1);
        ed.cursor_line = insert_idx;
        ed.cursor_col = last.chars().count();
    }
}

fn delete_current_line(ed: &mut CodeEditor) {
    if ed.text.len() == 1 {
        ed.text[0].clear();
        ed.cursor_col = 0;
    } else {
        let l = ed.cursor_line;
        ed.text.remove(l);
        folding::shift_remove(&mut ed.folds, l, 1);
        ed.cursor_line = l.min(ed.text.len() - 1);
        ed.cursor_col = ed.cursor_col.min(line_len(ed, ed.cursor_line));
    }
    ed.anchor_line = ed.cursor_line;
    ed.anchor_col = ed.cursor_col;
}

/// Delete every line the selection touches (Ctrl+Shift+K).
pub(crate) fn delete_lines(ed: &mut CodeEditor) {
    ed.push_history(EditKind::Other);
    let ((sl, _), (el, _)) = sel_range(ed);
    if el >= ed.text.len() {
        return;
    }
    let count = el - sl + 1;
    if count >= ed.text.len() {
        ed.text = vec![String::new()];
        ed.folds.clear();
    } else {
        ed.text.drain(sl..=el);
        folding::shift_remove(&mut ed.folds, sl, count);
    }
    ed.cursor_line = sl.min(ed.text.len() - 1);
    ed.cursor_col = 0;
    after_edit(ed);
}

/// Move the selected lines up or down (Alt+Up / Alt+Down).
pub(crate) fn move_lines(ed: &mut CodeEditor, up: bool) {
    let ((sl, _), (el, _)) = sel_range(ed);
    if up && sl == 0 {
        return;
    }
    if !up && el + 1 >= ed.text.len() {
        return;
    }
    ed.push_history(EditKind::Other);
    if up {
        let moved = ed.text.remove(sl - 1);
        ed.text.insert(el, moved);
        ed.cursor_line -= 1;
        ed.anchor_line -= 1;
    } else {
        let moved = ed.text.remove(el + 1);
        ed.text.insert(sl, moved);
        ed.cursor_line += 1;
        ed.anchor_line += 1;
    }
    // A line move reshuffles anchors unpredictably; drop folds to stay correct.
    ed.folds.clear();
    ed.content_dirty = true;
    ed.dirty = true;
    ensure_visible(ed);
}

/// Duplicate the selected lines below (Shift+Alt+Down) or above (Shift+Alt+Up).
pub(crate) fn duplicate_lines(ed: &mut CodeEditor, up: bool) {
    ed.push_history(EditKind::Other);
    let ((sl, _), (el, _)) = sel_range(ed);
    let block: Vec<String> = ed.text[sl..=el].to_vec();
    let count = block.len();
    let at = el + 1;
    for (i, l) in block.into_iter().enumerate() {
        ed.text.insert(at + i, l);
    }
    folding::shift_insert(&mut ed.folds, at, count);
    if !up {
        // Move the caret onto the copy so repeated presses stack downward.
        ed.cursor_line += count;
        ed.anchor_line += count;
    }
    ed.content_dirty = true;
    ed.dirty = true;
    ensure_visible(ed);
}

/// Toggle line comments over the selection using `token` (`"//"`, `"--"`, …).
/// Adds the token to every touched line if any line is uncommented, else removes
/// it — matching VSCode's Ctrl+/.
pub(crate) fn toggle_comment(ed: &mut CodeEditor, token: &str) {
    ed.push_history(EditKind::Other);
    let ((sl, _), (el, _)) = sel_range(ed);
    let with_space = format!("{token} ");
    // If every non-blank line is already commented, we uncomment.
    let all_commented = (sl..=el)
        .filter(|&l| !ed.text[l].trim().is_empty())
        .all(|l| ed.text[l].trim_start().starts_with(token));
    for l in sl..=el {
        if ed.text[l].trim().is_empty() {
            continue;
        }
        let indent_len = ed.text[l].len() - ed.text[l].trim_start().len();
        if all_commented {
            let body = &ed.text[l][indent_len..];
            let stripped = body
                .strip_prefix(&with_space)
                .or_else(|| body.strip_prefix(token))
                .unwrap_or(body);
            ed.text[l] = format!("{}{}", &ed.text[l][..indent_len], stripped);
        } else {
            ed.text[l].insert_str(indent_len, &with_space);
        }
    }
    ed.cursor_col = ed.cursor_col.min(line_len(ed, ed.cursor_line));
    ed.anchor_col = ed.anchor_col.min(line_len(ed, ed.anchor_line));
    ed.content_dirty = true;
    ed.dirty = true;
    ensure_visible(ed);
}

// ---------------------------------------------------------------------------
// Selection by word / line (double / triple click)
// ---------------------------------------------------------------------------

/// Select the word under `(line, col)` (double-click).
pub(crate) fn select_word_at(ed: &mut CodeEditor, line: usize, col: usize) {
    let cs = chars(ed, line);
    if cs.is_empty() {
        return;
    }
    let mut start = col.min(cs.len());
    let mut end = start;
    // Grow around the cell; anchor on whichever side has a word char.
    let at = start.min(cs.len().saturating_sub(1));
    if is_word(cs[at]) {
        while start > 0 && is_word(cs[start - 1]) {
            start -= 1;
        }
        while end < cs.len() && is_word(cs[end]) {
            end += 1;
        }
    } else {
        end = (start + 1).min(cs.len());
    }
    ed.anchor_line = line;
    ed.anchor_col = start;
    ed.cursor_line = line;
    ed.cursor_col = end;
    ed.dirty = true;
}

/// Select the whole line at `line` (triple-click), newline included.
pub(crate) fn select_line_at(ed: &mut CodeEditor, line: usize) {
    ed.anchor_line = line;
    ed.anchor_col = 0;
    if let Some(n) = layout::next_visible(&ed.folds, &ed.text, line) {
        ed.cursor_line = n;
        ed.cursor_col = 0;
    } else {
        ed.cursor_line = line;
        ed.cursor_col = line_len(ed, line);
    }
    ed.dirty = true;
}

// ---------------------------------------------------------------------------
// Folding commands
// ---------------------------------------------------------------------------

/// Toggle the fold headed by `line` (create it from the indentation region, or
/// remove the existing one).
pub(crate) fn toggle_fold(ed: &mut CodeEditor, line: usize) {
    if let Some(pos) = ed.folds.iter().position(|f| f.start == line) {
        ed.folds.remove(pos);
    } else if let Some(f) = folding::region(&ed.text, line) {
        ed.folds.push(f);
        // Don't leave the caret stranded inside the newly-hidden body.
        if ed.cursor_line > f.start && ed.cursor_line <= f.end {
            ed.cursor_line = f.start;
            ed.cursor_col = line_len(ed, f.start);
            ed.anchor_line = ed.cursor_line;
            ed.anchor_col = ed.cursor_col;
        }
    }
    ed.dirty = true;
    ensure_visible(ed);
}

pub(crate) fn is_line_foldable(ed: &CodeEditor, line: usize) -> bool {
    folding::is_foldable(&ed.text, line)
}

pub(crate) fn is_folded(ed: &CodeEditor, line: usize) -> bool {
    ed.folds.iter().any(|f| f.start == line)
}

// ---------------------------------------------------------------------------
// Keep the caret on-screen (in visual rows)
// ---------------------------------------------------------------------------

pub(crate) fn ensure_visible(ed: &mut CodeEditor) {
    if ed.visible == 0 {
        return;
    }
    // The caret must never rest on a hidden (folded) line.
    if layout::is_hidden(&ed.folds, ed.cursor_line) {
        ed.cursor_line = layout::clamp_visible(&ed.folds, ed.cursor_line);
        ed.cursor_col = ed.cursor_col.min(line_len(ed, ed.cursor_line));
    }
    let rows = ed.rows();
    let cr = layout::row_of(&rows, ed.cursor_line, ed.cursor_col);
    if cr < ed.scroll {
        ed.scroll = cr;
    } else if cr >= ed.scroll + ed.visible {
        ed.scroll = cr + 1 - ed.visible;
    }
    let max = rows.len().saturating_sub(1);
    ed.scroll = ed.scroll.min(max);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::code_editor::code_editor_for_test as mk;

    #[test]
    fn auto_close_inserts_pair_between() {
        let mut ed = mk("");
        ed.auto_close = true;
        type_char(&mut ed, '(');
        assert_eq!(ed.text[0], "()");
        assert_eq!(ed.cursor_col, 1);
    }

    #[test]
    fn type_over_closer() {
        let mut ed = mk("");
        ed.auto_close = true;
        type_char(&mut ed, '(');
        type_char(&mut ed, ')');
        assert_eq!(ed.text[0], "()");
        assert_eq!(ed.cursor_col, 2);
    }

    #[test]
    fn backspace_deletes_empty_pair() {
        let mut ed = mk("");
        ed.auto_close = true;
        type_char(&mut ed, '(');
        backspace(&mut ed);
        assert_eq!(ed.text[0], "");
    }

    #[test]
    fn newline_carries_indent_and_opens_block() {
        let mut ed = mk("    if x {");
        let end = ed.text[0].chars().count();
        ed.cursor_line = 0;
        ed.cursor_col = end;
        ed.anchor_line = 0;
        ed.anchor_col = end; // no selection
        ed.auto_close = false;
        newline(&mut ed);
        assert_eq!(ed.text[1], "        "); // +4 indent after `{`
        assert_eq!(ed.cursor_col, 8);
    }

    #[test]
    fn toggle_comment_adds_then_removes() {
        let mut ed = mk("let x = 1;\nlet y = 2;");
        ed.anchor_line = 0;
        ed.anchor_col = 0;
        ed.cursor_line = 1;
        ed.cursor_col = 0;
        toggle_comment(&mut ed, "//");
        assert_eq!(ed.text[0], "// let x = 1;");
        assert_eq!(ed.text[1], "// let y = 2;");
        toggle_comment(&mut ed, "//");
        assert_eq!(ed.text[0], "let x = 1;");
    }

    #[test]
    fn indent_and_dedent_block() {
        let mut ed = mk("a\nb");
        ed.anchor_line = 0;
        ed.anchor_col = 0;
        ed.cursor_line = 1;
        ed.cursor_col = 1;
        indent_selection(&mut ed);
        assert_eq!(ed.text[0], "    a");
        assert_eq!(ed.text[1], "    b");
        dedent(&mut ed);
        assert_eq!(ed.text[0], "a");
        assert_eq!(ed.text[1], "b");
    }

    #[test]
    fn move_and_duplicate_lines() {
        let mut ed = mk("one\ntwo\nthree");
        ed.cursor_line = 2;
        ed.anchor_line = 2;
        move_lines(&mut ed, true);
        assert_eq!(ed.text, vec!["one", "three", "two"]);
        duplicate_lines(&mut ed, false);
        assert_eq!(ed.text[1], "three");
        assert_eq!(ed.text[2], "three");
    }

    #[test]
    fn undo_redo_roundtrip() {
        let mut ed = mk("");
        type_char(&mut ed, 'a');
        type_char(&mut ed, 'b');
        undo(&mut ed);
        assert_eq!(ed.text[0], "");
        redo(&mut ed);
        assert_eq!(ed.text[0], "ab");
    }

    #[test]
    fn paste_multiline() {
        let mut ed = mk("XY");
        ed.cursor_col = 1;
        ed.anchor_col = 1; // caret between X and Y, no selection
        paste(&mut ed, "1\n2\n3");
        assert_eq!(ed.text, vec!["X1", "2", "3Y"]);
        assert_eq!((ed.cursor_line, ed.cursor_col), (2, 1));
    }

    #[test]
    fn word_motion() {
        let cs: Vec<char> = "foo bar".chars().collect();
        assert_eq!(word_right_col(&cs, 0), 4); // past "foo "
        assert_eq!(word_left_col(&cs, 7), 4); // back to start of "bar"
    }
}
