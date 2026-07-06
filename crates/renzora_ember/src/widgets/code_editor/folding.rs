//! Fold-region detection and fold-anchor maintenance — the pure logic behind
//! the gutter chevrons.
//!
//! Foldable regions are derived from **indentation**, the same provider VSCode
//! falls back to for languages without a real folding grammar: a non-blank line
//! opens a region that covers the following more-indented lines (blank lines
//! inside stay part of it), and the region ends at the last more-indented line
//! before the indentation returns to the header's level. This is language-
//! agnostic, so it works for every grammar the editor highlights without ember
//! needing a parser.
//!
//! Edits move lines around, so [`shift_insert`]/[`shift_remove`] keep active
//! fold anchors pointing at the right buffer lines (or drop a fold when an edit
//! straddles its header). All pure and unit-tested.

use super::{Fold, TAB_WIDTH};

/// Leading-indentation width in columns (tab = [`TAB_WIDTH`]). `None` for a
/// blank/whitespace-only line, which folding treats as transparent.
fn indent(line: &str) -> Option<usize> {
    let mut cols = 0;
    for c in line.chars() {
        match c {
            ' ' => cols += 1,
            '\t' => cols += TAB_WIDTH,
            _ => return Some(cols),
        }
    }
    None
}

/// The fold region opened by `start`, or `None` if that line heads no region
/// (blank, or nothing more-indented follows it).
pub(crate) fn region(lines: &[String], start: usize) -> Option<Fold> {
    let base = indent(lines.get(start)?)?;
    let mut end = start;
    let mut j = start + 1;
    while j < lines.len() {
        match indent(&lines[j]) {
            // Blank line: tentatively inside the region, but only "closes into"
            // it if a deeper line follows — so don't advance `end` yet.
            None => {}
            Some(d) if d > base => end = j,
            Some(_) => break,
        }
        j += 1;
    }
    (end > start).then_some(Fold { start, end })
}

/// Does `line` head a foldable region?
pub(crate) fn is_foldable(lines: &[String], line: usize) -> bool {
    region(lines, line).is_some()
}

/// `count` lines were inserted starting at buffer index `at` (everything at
/// `at`.. shifted down). Move fold anchors so they keep covering the same text;
/// an insertion inside a fold body grows the fold.
pub(crate) fn shift_insert(folds: &mut [Fold], at: usize, count: usize) {
    for f in folds.iter_mut() {
        if f.start >= at {
            f.start += count;
            f.end += count;
        } else if f.end >= at {
            // start < at <= end: insertion landed inside the body.
            f.end += count;
        }
    }
}

/// `count` lines starting at buffer index `from` were removed. Shift folds fully
/// below the cut up; drop any the cut overlaps (its structure is gone).
pub(crate) fn shift_remove(folds: &mut Vec<Fold>, from: usize, count: usize) {
    let cut_end = from + count; // exclusive
    folds.retain_mut(|f| {
        if f.end < from {
            true // entirely above the cut
        } else if f.start >= cut_end {
            f.start -= count;
            f.end -= count;
            true // entirely below
        } else {
            false // overlaps → drop
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(s: &[&str]) -> Vec<String> {
        s.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn braces_by_indent() {
        let l = lines(&["fn f() {", "    a", "    b", "}"]);
        assert_eq!(region(&l, 0), Some(Fold { start: 0, end: 2 }));
        assert!(!is_foldable(&l, 3));
    }

    #[test]
    fn nested_regions() {
        let l = lines(&["a:", "    b:", "        c", "    d"]);
        assert_eq!(region(&l, 0), Some(Fold { start: 0, end: 3 }));
        assert_eq!(region(&l, 1), Some(Fold { start: 1, end: 2 }));
    }

    #[test]
    fn trailing_blank_excluded() {
        let l = lines(&["a:", "    b", "", "c"]);
        // Blank line 2 doesn't extend the region; c is back at base.
        assert_eq!(region(&l, 0), Some(Fold { start: 0, end: 1 }));
    }

    #[test]
    fn blank_interior_kept() {
        let l = lines(&["a:", "    b", "", "    c", "d"]);
        assert_eq!(region(&l, 0), Some(Fold { start: 0, end: 3 }));
    }

    #[test]
    fn insert_below_shifts() {
        let mut f = vec![Fold { start: 5, end: 8 }];
        shift_insert(&mut f, 2, 3);
        assert_eq!(f[0], Fold { start: 8, end: 11 });
    }

    #[test]
    fn insert_in_body_grows() {
        let mut f = vec![Fold { start: 2, end: 6 }];
        shift_insert(&mut f, 4, 1);
        assert_eq!(f[0], Fold { start: 2, end: 7 });
    }

    #[test]
    fn remove_below_shifts_up() {
        let mut f = vec![Fold { start: 5, end: 8 }];
        shift_remove(&mut f, 1, 2);
        assert_eq!(f[0], Fold { start: 3, end: 6 });
    }

    #[test]
    fn remove_overlapping_drops() {
        let mut f = vec![Fold { start: 2, end: 6 }];
        shift_remove(&mut f, 4, 1);
        assert!(f.is_empty());
    }
}
