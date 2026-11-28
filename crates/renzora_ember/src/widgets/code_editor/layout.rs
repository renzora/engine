//! The **visual-row model** — the layer that lets the monospace editor support
//! code folding and word wrap without the rest of the widget having to reason
//! about either.
//!
//! The buffer is still `Vec<String>` (one entry per *logical* line), but what
//! the user sees, clicks, and scrolls through is a flat list of [`VisualRow`]s:
//!
//! - **Folding** removes rows: a fold over buffer lines `start..=end` hides
//!   every line *after* the header (`start+1..=end`), so those lines produce no
//!   visual rows at all. The header line stays, flagged [`VisualRow::fold_header`]
//!   so the renderer can draw the `⋯` badge.
//! - **Word wrap** adds rows: a logical line longer than `wrap_cols` columns is
//!   split into several visual rows, each covering a `[start_col, end_col)` slice
//!   of the line. The first carries the gutter number / fold chevron
//!   ([`VisualRow::first`]); the continuations don't.
//!
//! Everything else (caret placement, click hit-testing, scrolling, selection,
//! `ensure_visible`) is expressed purely in terms of this row list, so folding
//! and wrapping compose for free. Because monospace makes column→pixel exact
//! (`col * char_w`), a visual row only needs its column span, not per-glyph
//! metrics. The one exception is the wide scripts (Hangul, CJK), drawn from a
//! fallback font at their own advance: [`span_w`] and [`col_at_x`] price those
//! at `wide_w`. Word wrap still splits by character count.
//!
//! All functions here are pure and unit-tested — the ECS glue in [`super::systems`]
//! stays thin on purpose, since the editor can't be GPU-driven in tests.

use super::Fold;

/// One on-screen row. `start_col`/`end_col` are **character** columns into
/// `line`'s text (`end_col` exclusive); for an unwrapped line they're
/// `0..char_count`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct VisualRow {
    /// Buffer line this row draws from.
    pub line: usize,
    /// First character column shown on this row.
    pub start_col: usize,
    /// One-past-last character column shown on this row.
    pub end_col: usize,
    /// True for the first visual row of a logical line — the only row that shows
    /// the gutter number and a fold chevron. Wrap continuations are `false`.
    pub first: bool,
    /// True when this line is the header of an active fold (draw the `⋯` badge).
    pub fold_header: bool,
}

/// Character count of buffer `line` (0 if out of range).
pub(crate) fn char_len(lines: &[String], line: usize) -> usize {
    lines.get(line).map(|l| l.chars().count()).unwrap_or(0)
}

/// Whether `c` is drawn wider than one monospace cell: Hangul (syllables and
/// jamo), CJK ideographs, kana and the fullwidth forms. A mono font carries
/// none of these, so they come from a fallback font at that font's own advance,
/// which `code_probe` measures as `wide_w`; drawing them at `char_w` put every
/// caret and selection after a Korean word in the wrong place.
pub(crate) fn is_wide(c: char) -> bool {
    matches!(
        c as u32,
        0x1100..=0x115F
            | 0x2E80..=0x303E
            | 0x3041..=0x33FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xA960..=0xA97F
            | 0xAC00..=0xD7A3
            | 0xD7B0..=0xD7FF
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x20000..=0x3FFFD
    )
}

/// Drawn width (logical px) of the characters `[from, to)` of `line`.
pub(crate) fn span_w(line: &str, from: usize, to: usize, char_w: f32, wide_w: f32) -> f32 {
    line.chars()
        .skip(from)
        .take(to.saturating_sub(from))
        .map(|c| if is_wide(c) { wide_w } else { char_w })
        .sum()
}

/// The column offset inside `[from, to)` of `line` nearest to `x` px from that
/// span's left edge: the inverse of [`span_w`], for click hit-testing.
pub(crate) fn col_at_x(line: &str, from: usize, to: usize, x: f32, char_w: f32, wide_w: f32) -> usize {
    let mut left = 0.0;
    let mut n = 0;
    for c in line.chars().skip(from).take(to.saturating_sub(from)) {
        let w = if is_wide(c) { wide_w } else { char_w };
        if x < left + w * 0.5 {
            return n;
        }
        left += w;
        n += 1;
    }
    n
}

/// Is `line` hidden inside an active fold? A fold `start..=end` hides its body
/// (`start+1..=end`) but keeps the header (`start`) visible.
pub(crate) fn is_hidden(folds: &[Fold], line: usize) -> bool {
    folds.iter().any(|f| line > f.start && line <= f.end)
}

/// The nearest visible buffer line at or above `line` (walks up out of a fold
/// body onto its header). Always terminates at 0.
pub(crate) fn clamp_visible(folds: &[Fold], mut line: usize) -> usize {
    while line > 0 && is_hidden(folds, line) {
        line -= 1;
    }
    line
}

/// First visible buffer line strictly after `line` (skips fold bodies), or
/// `None` if `line` is already at/after the last visible line.
pub(crate) fn next_visible(folds: &[Fold], lines: &[String], line: usize) -> Option<usize> {
    let mut l = line + 1;
    while l < lines.len() {
        if !is_hidden(folds, l) {
            return Some(l);
        }
        l += 1;
    }
    None
}

/// Last visible buffer line strictly before `line` (skips fold bodies), or
/// `None` if `line` is at/before the first line.
pub(crate) fn prev_visible(folds: &[Fold], line: usize) -> Option<usize> {
    let mut l = line;
    while l > 0 {
        l -= 1;
        if !is_hidden(folds, l) {
            return Some(l);
        }
    }
    None
}

/// Build the visual-row list for the whole buffer. `wrap_cols == 0` disables
/// wrapping (one row per visible line); otherwise each line is split every
/// `wrap_cols` columns. Fold headers are never wrapped — the header shows its
/// full first line plus the `⋯` badge regardless of width, matching how editors
/// render a collapsed region.
pub(crate) fn build_rows(lines: &[String], folds: &[Fold], wrap_cols: usize) -> Vec<VisualRow> {
    let mut rows = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        if is_hidden(folds, i) {
            i += 1;
            continue;
        }
        let n = char_len(lines, i);
        let fold_header = folds.iter().any(|f| f.start == i);
        if wrap_cols == 0 || fold_header || n <= wrap_cols {
            rows.push(VisualRow { line: i, start_col: 0, end_col: n, first: true, fold_header });
        } else {
            let mut c = 0;
            let mut first = true;
            while c < n {
                let end = (c + wrap_cols).min(n);
                rows.push(VisualRow { line: i, start_col: c, end_col: end, first, fold_header: false });
                c = end;
                first = false;
            }
        }
        i += 1;
    }
    // An empty buffer still needs one row so the caret has somewhere to live.
    if rows.is_empty() {
        rows.push(VisualRow { line: 0, start_col: 0, end_col: 0, first: true, fold_header: false });
    }
    rows
}

/// Index into `rows` of the visual row that hosts buffer position `(line, col)`.
/// For a wrapped line the caret at an exact segment boundary belongs to the row
/// that *ends* there (so it sits at the right edge, like every editor), except
/// at the very end of the line where it stays on the last row.
pub(crate) fn row_of(rows: &[VisualRow], line: usize, col: usize) -> usize {
    let mut fallback = 0;
    let mut last_of_line = None;
    for (idx, r) in rows.iter().enumerate() {
        if r.line != line {
            if last_of_line.is_some() && r.line > line {
                break;
            }
            continue;
        }
        last_of_line = Some(idx);
        fallback = idx;
        if col < r.end_col || (col == r.end_col && col >= char_len_from_row(rows, line)) {
            return idx;
        }
        if col >= r.start_col && col <= r.end_col {
            fallback = idx;
        }
    }
    last_of_line.unwrap_or(fallback)
}

/// The logical line's character length, recovered from its last row's `end_col`
/// (avoids threading `lines` through [`row_of`]).
fn char_len_from_row(rows: &[VisualRow], line: usize) -> usize {
    rows.iter().filter(|r| r.line == line).map(|r| r.end_col).max().unwrap_or(0)
}

/// Buffer position `(line, col)` for a click that landed on visual row
/// `row_index` at character column `x_col` (clamped into the row's span).
pub(crate) fn buffer_pos(rows: &[VisualRow], row_index: usize, x_col: usize) -> (usize, usize) {
    let r = rows.get(row_index.min(rows.len().saturating_sub(1)));
    match r {
        Some(r) => (r.line, (r.start_col + x_col).min(r.end_col)),
        None => (0, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(s: &[&str]) -> Vec<String> {
        s.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn no_wrap_no_fold_is_one_row_per_line() {
        let l = lines(&["a", "bb", "ccc"]);
        let rows = build_rows(&l, &[], 0);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1], VisualRow { line: 1, start_col: 0, end_col: 2, first: true, fold_header: false });
    }

    #[test]
    fn fold_hides_body_keeps_header() {
        let l = lines(&["fn f() {", "  x", "  y", "}"]);
        let folds = [Fold { start: 0, end: 2 }];
        let rows = build_rows(&l, &folds, 0);
        // Header (0), the hidden body 1..=2 gone, then line 3.
        assert_eq!(rows.len(), 2);
        assert!(rows[0].fold_header);
        assert_eq!(rows[0].line, 0);
        assert_eq!(rows[1].line, 3);
        assert!(is_hidden(&folds, 1));
        assert!(is_hidden(&folds, 2));
        assert!(!is_hidden(&folds, 3));
    }

    #[test]
    fn wrap_splits_long_line() {
        let l = lines(&["0123456789ab"]); // 12 chars
        let rows = build_rows(&l, &[], 5);
        assert_eq!(rows.len(), 3);
        assert_eq!((rows[0].start_col, rows[0].end_col, rows[0].first), (0, 5, true));
        assert_eq!((rows[1].start_col, rows[1].end_col, rows[1].first), (5, 10, false));
        assert_eq!((rows[2].start_col, rows[2].end_col, rows[2].first), (10, 12, false));
    }

    #[test]
    fn wrap_and_fold_compose() {
        let l = lines(&["header longer than five", "hidden", "tail"]);
        let folds = [Fold { start: 0, end: 1 }];
        let rows = build_rows(&l, &folds, 5);
        // Fold header is not wrapped even though it's long; body line 1 hidden.
        assert_eq!(rows[0].line, 0);
        assert!(rows[0].fold_header);
        assert_eq!(rows[0].end_col, char_len(&l, 0));
        // "tail" (4 chars) fits in one row.
        assert_eq!(rows.last().unwrap().line, 2);
    }

    #[test]
    fn row_of_and_back_roundtrip_wrapped() {
        let l = lines(&["0123456789ab"]);
        let rows = build_rows(&l, &[], 5);
        // col 7 lives on the second row (covers 5..10) at x=2.
        let ri = row_of(&rows, 0, 7);
        assert_eq!(ri, 1);
        assert_eq!(buffer_pos(&rows, ri, 2), (0, 7));
        // End of line sits on the last row.
        assert_eq!(row_of(&rows, 0, 12), 2);
    }

    #[test]
    fn visible_navigation_skips_folds() {
        let l = lines(&["a", "b", "c", "d"]);
        let folds = [Fold { start: 0, end: 2 }];
        assert_eq!(next_visible(&folds, &l, 0), Some(3));
        assert_eq!(prev_visible(&folds, 3), Some(0));
        assert_eq!(clamp_visible(&folds, 2), 0);
    }

    #[test]
    fn a_wide_char_takes_its_own_advance() {
        // "a한b": one narrow, one Hangul, one narrow.
        assert_eq!(span_w("a한b", 0, 3, 7.0, 13.0), 27.0);
        assert_eq!(span_w("a한b", 1, 2, 7.0, 13.0), 13.0);
        assert!(is_wide('ㅁ') && is_wide('漢') && !is_wide('a'));
    }

    #[test]
    fn a_click_lands_on_the_nearest_boundary_past_a_wide_char() {
        // Cells: a [0,7) 한 [7,20) b [20,27).
        assert_eq!(col_at_x("a한b", 0, 3, 3.0, 7.0, 13.0), 0);
        assert_eq!(col_at_x("a한b", 0, 3, 12.0, 7.0, 13.0), 1);
        assert_eq!(col_at_x("a한b", 0, 3, 15.0, 7.0, 13.0), 2);
        assert_eq!(col_at_x("a한b", 0, 3, 99.0, 7.0, 13.0), 3);
    }

    #[test]
    fn empty_buffer_has_one_row() {
        let l = lines(&[""]);
        assert_eq!(build_rows(&l, &[], 0).len(), 1);
    }
}
