//! Document editing — applies a keystroke (with shift state) to the
//! [`CodeEditor`]'s text, cursor and selection.

use bevy::input::keyboard::Key;

use super::CodeEditor;

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

/// If the caret sits next to a bracket, returns that bracket's cell and its
/// matching bracket's cell as `((line, col), (line, col))`, for highlighting.
/// Prefers the bracket immediately *left* of the caret (like most editors), then
/// the one to the right. Scans nesting-aware across lines, bounded so a huge
/// unbalanced file can't stall the render.
pub(crate) fn bracket_match(ed: &CodeEditor) -> Option<((usize, usize), (usize, usize))> {
    /// Max chars scanned before giving up (keeps render O(1)-ish on big files).
    const SCAN_CAP: usize = 100_000;
    const OPEN: [char; 3] = ['(', '[', '{'];
    const CLOSE: [char; 3] = [')', ']', '}'];

    let cur: Vec<char> = ed.text.get(ed.cursor_line)?.chars().collect();
    // Candidate bracket cell: left of the caret first, else right.
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

/// Apply one key (with `shift` held) to the document; sets `dirty` and keeps the
/// cursor on-screen.
pub(crate) fn edit(ed: &mut CodeEditor, key: &Key, shift: bool) {
    let is_move = matches!(
        key,
        Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End
    );
    match key {
        Key::Character(s) => {
            delete_selection(ed);
            for c in s.chars() {
                if !c.is_control() {
                    insert_char(ed, c);
                }
            }
        }
        Key::Space => {
            delete_selection(ed);
            insert_char(ed, ' ');
        }
        Key::Tab => {
            delete_selection(ed);
            for _ in 0..4 {
                insert_char(ed, ' ');
            }
        }
        Key::Enter => {
            delete_selection(ed);
            insert_newline(ed);
        }
        Key::Backspace => {
            if has_selection(ed) {
                delete_selection(ed);
            } else {
                backspace(ed);
            }
        }
        Key::Delete => {
            if has_selection(ed) {
                delete_selection(ed);
            } else {
                delete_fwd(ed);
            }
        }
        Key::ArrowLeft => {
            if ed.cursor_col > 0 {
                ed.cursor_col -= 1;
            } else if ed.cursor_line > 0 {
                ed.cursor_line -= 1;
                ed.cursor_col = line_len(ed, ed.cursor_line);
            }
        }
        Key::ArrowRight => {
            if ed.cursor_col < line_len(ed, ed.cursor_line) {
                ed.cursor_col += 1;
            } else if ed.cursor_line + 1 < ed.text.len() {
                ed.cursor_line += 1;
                ed.cursor_col = 0;
            }
        }
        Key::ArrowUp => {
            if ed.cursor_line > 0 {
                ed.cursor_line -= 1;
                ed.cursor_col = ed.cursor_col.min(line_len(ed, ed.cursor_line));
            }
        }
        Key::ArrowDown => {
            if ed.cursor_line + 1 < ed.text.len() {
                ed.cursor_line += 1;
                ed.cursor_col = ed.cursor_col.min(line_len(ed, ed.cursor_line));
            }
        }
        Key::Home => ed.cursor_col = 0,
        Key::End => ed.cursor_col = line_len(ed, ed.cursor_line),
        _ => return,
    }
    // Moves without shift collapse the selection; edits always collapse it.
    if !is_move || !shift {
        ed.anchor_line = ed.cursor_line;
        ed.anchor_col = ed.cursor_col;
    }
    // Anything that isn't a pure caret move mutated the document.
    if !is_move {
        ed.content_dirty = true;
    }
    ed.dirty = true;
    ensure_visible(ed);
}

pub(crate) fn line_len(ed: &CodeEditor, i: usize) -> usize {
    ed.text.get(i).map(|l| l.chars().count()).unwrap_or(0)
}

fn delete_selection(ed: &mut CodeEditor) {
    if !has_selection(ed) {
        return;
    }
    let ((sl, sc), (el, ec)) = sel_range(ed);
    if sl == el {
        let mut chars: Vec<char> = ed.text[sl].chars().collect();
        chars.drain(sc..ec.min(chars.len()));
        ed.text[sl] = chars.into_iter().collect();
    } else {
        let head: String = ed.text[sl].chars().take(sc).collect();
        let tail: String = ed.text[el].chars().skip(ec).collect();
        ed.text.drain(sl + 1..=el);
        ed.text[sl] = head + tail.as_str();
    }
    ed.cursor_line = sl;
    ed.cursor_col = sc;
    ed.anchor_line = sl;
    ed.anchor_col = sc;
}

fn insert_char(ed: &mut CodeEditor, c: char) {
    let line = ed.cursor_line;
    let mut chars: Vec<char> = ed.text[line].chars().collect();
    let col = ed.cursor_col.min(chars.len());
    chars.insert(col, c);
    ed.text[line] = chars.into_iter().collect();
    ed.cursor_col = col + 1;
}

fn insert_newline(ed: &mut CodeEditor) {
    let line = ed.cursor_line;
    let chars: Vec<char> = ed.text[line].chars().collect();
    let col = ed.cursor_col.min(chars.len());
    let tail: String = chars[col..].iter().collect();
    ed.text[line] = chars[..col].iter().collect();
    ed.text.insert(line + 1, tail);
    ed.cursor_line = line + 1;
    ed.cursor_col = 0;
}

fn backspace(ed: &mut CodeEditor) {
    if ed.cursor_col > 0 {
        let line = ed.cursor_line;
        let mut chars: Vec<char> = ed.text[line].chars().collect();
        chars.remove(ed.cursor_col - 1);
        ed.text[line] = chars.into_iter().collect();
        ed.cursor_col -= 1;
    } else if ed.cursor_line > 0 {
        let cur = ed.text.remove(ed.cursor_line);
        ed.cursor_line -= 1;
        ed.cursor_col = line_len(ed, ed.cursor_line);
        ed.text[ed.cursor_line].push_str(&cur);
    }
}

fn delete_fwd(ed: &mut CodeEditor) {
    let line = ed.cursor_line;
    if ed.cursor_col < line_len(ed, line) {
        let mut chars: Vec<char> = ed.text[line].chars().collect();
        chars.remove(ed.cursor_col);
        ed.text[line] = chars.into_iter().collect();
    } else if line + 1 < ed.text.len() {
        let next = ed.text.remove(line + 1);
        ed.text[line].push_str(&next);
    }
}

fn ensure_visible(ed: &mut CodeEditor) {
    if ed.visible == 0 {
        return;
    }
    if ed.cursor_line < ed.scroll {
        ed.scroll = ed.cursor_line;
    } else if ed.cursor_line >= ed.scroll + ed.visible {
        ed.scroll = ed.cursor_line + 1 - ed.visible;
    }
}
