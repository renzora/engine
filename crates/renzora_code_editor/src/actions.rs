//! Editor text operations: comment toggle, goto line, bracket matching,
//! and the VSCode-style editing commands (duplicate line, move line,
//! indent/dedent selection, etc.).

use crate::highlight::Language;

/// Number of spaces used for an indent unit.
pub const TAB_SIZE: usize = 4;

/// Convert a 0-based line number to its starting byte offset in `content`.
/// If the line is past EOF, returns `content.len()`.
pub fn line_to_byte(content: &str, line: usize) -> usize {
    if line == 0 {
        return 0;
    }
    let mut seen = 0usize;
    for (i, b) in content.bytes().enumerate() {
        if b == b'\n' {
            seen += 1;
            if seen == line {
                return i + 1;
            }
        }
    }
    content.len()
}

/// 0-based line that contains the given byte offset.
pub fn byte_to_line(content: &str, byte_idx: usize) -> usize {
    content[..byte_idx.min(content.len())]
        .bytes()
        .filter(|b| *b == b'\n')
        .count()
}

/// Byte length of the substring `content[..byte_idx]` in characters.
/// egui's `CCursor` indexes by chars, not bytes.
pub fn byte_to_char(content: &str, byte_idx: usize) -> usize {
    content[..byte_idx.min(content.len())].chars().count()
}

/// Inverse of `byte_to_char`.
pub fn char_to_byte(content: &str, char_idx: usize) -> usize {
    let mut count = 0usize;
    for (i, _) in content.char_indices() {
        if count == char_idx {
            return i;
        }
        count += 1;
    }
    content.len()
}

/// Byte range of a single line (not including the trailing `\n`).
pub fn line_byte_range(content: &str, line: usize) -> (usize, usize) {
    let start = line_to_byte(content, line);
    let bytes = content.as_bytes();
    let mut end = start;
    while end < bytes.len() && bytes[end] != b'\n' {
        end += 1;
    }
    (start, end)
}

/// Full byte range of `line` — includes the trailing `\n` if present.
pub fn line_full_range(content: &str, line: usize) -> (usize, usize) {
    let (s, e) = line_byte_range(content, line);
    let bytes = content.as_bytes();
    if e < bytes.len() && bytes[e] == b'\n' {
        (s, e + 1)
    } else {
        (s, e)
    }
}

/// The first/last line numbers covered by the selection `[a, b]`.
/// Mirrors the "selection ending on a newline doesn't include the next line"
/// convention used elsewhere.
pub fn line_span_of_selection(content: &str, a: usize, b: usize) -> (usize, usize) {
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    let first = byte_to_line(content, lo);
    let last = {
        let raw = byte_to_line(content, hi);
        if raw > first && hi > 0 && content.as_bytes()[hi - 1] == b'\n' {
            raw - 1
        } else {
            raw
        }
    };
    (first, last)
}

/// Byte range of the identifier at `byte_idx`. Returns None if not on a word.
pub fn word_range_at_byte(content: &str, byte_idx: usize) -> Option<(usize, usize)> {
    let bytes = content.as_bytes();
    let clamped = byte_idx.min(bytes.len());
    let mut s = clamped;
    while s > 0 && (bytes[s - 1].is_ascii_alphanumeric() || bytes[s - 1] == b'_') {
        s -= 1;
    }
    let mut e = clamped;
    while e < bytes.len() && (bytes[e].is_ascii_alphanumeric() || bytes[e] == b'_') {
        e += 1;
    }
    if s == e {
        None
    } else {
        Some((s, e))
    }
}

/// Every identifier-delimited occurrence of `word` in `content`.
pub fn find_all_occurrences(content: &str, word: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    if word.is_empty() {
        return out;
    }
    let bytes = content.as_bytes();
    let wb = word.as_bytes();
    let mut i = 0;
    while i + wb.len() <= bytes.len() {
        if &bytes[i..i + wb.len()] == wb {
            let before_ok =
                i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
            let after_pos = i + wb.len();
            let after_ok = after_pos == bytes.len()
                || !(bytes[after_pos].is_ascii_alphanumeric() || bytes[after_pos] == b'_');
            if before_ok && after_ok {
                out.push((i, after_pos));
                i = after_pos;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Leading whitespace of the line containing `byte_idx`.
pub fn leading_whitespace_of_line(content: &str, byte_idx: usize) -> String {
    let line = byte_to_line(content, byte_idx);
    let (s, e) = line_byte_range(content, line);
    let slice = &content[s..e];
    slice
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect()
}

/// Smart-Home target: first press goes to start-of-indentation, second to
/// column 0.
pub fn smart_home_target(content: &str, byte_idx: usize) -> usize {
    let line = byte_to_line(content, byte_idx);
    let line_start = line_to_byte(content, line);
    let bytes = content.as_bytes();
    let mut indent_end = line_start;
    while indent_end < bytes.len()
        && (bytes[indent_end] == b' ' || bytes[indent_end] == b'\t')
    {
        indent_end += 1;
    }
    if byte_idx <= indent_end {
        // Already at-or-before indent start → go to column 0.
        line_start
    } else {
        indent_end
    }
}

/// Indent or dedent the selection. Returns the new selection range.
/// `dedent=false` → prepend `TAB_SIZE` spaces on each line.
/// `dedent=true`  → remove up to `TAB_SIZE` leading spaces (or one `\t`).
pub fn indent_selection(
    content: &mut String,
    sel_start: usize,
    sel_end: usize,
    dedent: bool,
) -> (usize, usize) {
    let indent = " ".repeat(TAB_SIZE);
    let (first, last) = line_span_of_selection(content, sel_start, sel_end);

    let mut new_start = sel_start;
    let mut new_end = sel_end;

    for line in (first..=last).rev() {
        let ls = line_to_byte(content, line);
        if dedent {
            let bytes = content.as_bytes();
            let mut removed = 0;
            while removed < TAB_SIZE
                && ls + removed < bytes.len()
                && bytes[ls + removed] == b' '
            {
                removed += 1;
            }
            if removed == 0 && ls < bytes.len() && bytes[ls] == b'\t' {
                removed = 1;
            }
            if removed > 0 {
                content.replace_range(ls..ls + removed, "");
                if ls < new_start {
                    new_start = new_start.saturating_sub(removed);
                }
                if ls < new_end {
                    new_end = new_end.saturating_sub(removed);
                }
            }
        } else {
            content.insert_str(ls, &indent);
            if ls <= new_start {
                new_start += TAB_SIZE;
            }
            if ls <= new_end {
                new_end += TAB_SIZE;
            }
        }
    }

    (new_start, new_end)
}

/// Duplicate the lines covered by `[a, b]`. The new selection tracks the copy.
pub fn duplicate_lines(content: &mut String, a: usize, b: usize) -> (usize, usize) {
    let (first, last) = line_span_of_selection(content, a, b);
    let block_start = line_to_byte(content, first);
    let (_, block_end) = line_full_range(content, last);
    let mut block = content[block_start..block_end].to_string();
    // Ensure the copy ends with a newline so it lands on its own line.
    let block_had_nl = block.ends_with('\n');
    if !block_had_nl {
        block.push('\n');
    }
    content.insert_str(block_end, &block);

    let shift = block.len();
    // New cursor/selection: same offsets inside the duplicated block.
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    let new_lo = lo + shift;
    let new_hi = hi + shift;
    (new_lo, new_hi)
}

/// Delete the lines covered by `[a, b]`. Returns the new cursor byte.
pub fn delete_lines(content: &mut String, a: usize, b: usize) -> usize {
    let (first, last) = line_span_of_selection(content, a, b);
    let block_start = line_to_byte(content, first);
    let (_, block_end) = line_full_range(content, last);
    content.replace_range(block_start..block_end, "");
    // Place cursor at the start of where the block was.
    block_start.min(content.len())
}

/// Move the selected lines up one. Returns new selection or None if at top.
pub fn move_lines_up(
    content: &mut String,
    a: usize,
    b: usize,
) -> Option<(usize, usize)> {
    let (first, last) = line_span_of_selection(content, a, b);
    if first == 0 {
        return None;
    }

    let lines: Vec<String> = content.split('\n').map(|s| s.to_string()).collect();
    let mut new_lines = lines.clone();
    let block: Vec<String> = new_lines.drain(first..=last).collect();
    new_lines.splice(first - 1..first - 1, block);
    *content = new_lines.join("\n");

    let prev_len = lines[first - 1].len() + 1; // +1 for the \n that separated them
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    Some((lo.saturating_sub(prev_len), hi.saturating_sub(prev_len)))
}

/// Move the selected lines down one. Returns new selection or None if at bottom.
pub fn move_lines_down(
    content: &mut String,
    a: usize,
    b: usize,
) -> Option<(usize, usize)> {
    let (first, last) = line_span_of_selection(content, a, b);
    let lines: Vec<String> = content.split('\n').map(|s| s.to_string()).collect();
    if last + 1 >= lines.len() {
        return None;
    }

    let mut new_lines = lines.clone();
    let block: Vec<String> = new_lines.drain(first..=last).collect();
    // Insert after the line that now sits at `first`.
    let insert_at = first + 1;
    new_lines.splice(insert_at..insert_at, block);
    *content = new_lines.join("\n");

    let next_len = lines[last + 1].len() + 1;
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    Some((lo + next_len, hi + next_len))
}

/// Find the next occurrence of `needle` starting at/after `from_byte`.
/// Wraps to the start on EOF.
pub fn find_next_match(
    content: &str,
    needle: &str,
    from_byte: usize,
    case_sensitive: bool,
) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    let from = from_byte.min(content.len());
    if case_sensitive {
        content[from..]
            .find(needle)
            .map(|i| from + i)
            .or_else(|| content[..from].find(needle))
    } else {
        let hay_lower = content.to_lowercase();
        let needle_lower = needle.to_lowercase();
        hay_lower[from..]
            .find(&needle_lower)
            .map(|i| from + i)
            .or_else(|| hay_lower[..from.min(hay_lower.len())].find(&needle_lower))
    }
}

/// Ctrl+D: pick the current selection (or the word under the cursor if no
/// selection) and find its next occurrence, returning the new selection.
pub fn select_next_occurrence(
    content: &str,
    a: usize,
    b: usize,
    case_sensitive: bool,
) -> Option<(usize, usize)> {
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    let (search_start, search_end) = if lo == hi {
        word_range_at_byte(content, lo)?
    } else {
        (lo, hi)
    };
    let needle = &content[search_start..search_end];
    let from = search_end; // start searching after the current selection/word
    let pos = find_next_match(content, needle, from, case_sensitive)?;
    if pos == search_start {
        // Wrapped and landed on the same spot — nothing new to select.
        return None;
    }
    Some((pos, pos + needle.len()))
}


/// Toggle a line comment across the lines touched by `[sel_start_byte, sel_end_byte]`.
///
/// If every non-blank line in the range is already commented, all of them are
/// uncommented. Otherwise the comment prefix is added to each non-blank line.
///
/// Returns the new selection byte range (or `None` if the language doesn't
/// support line comments / nothing changed).
pub fn toggle_line_comment(
    content: &mut String,
    sel_start_byte: usize,
    sel_end_byte: usize,
    lang: Language,
) -> Option<(usize, usize)> {
    let prefix = lang.line_comment()?;
    let prefix_with_space = format!("{} ", prefix);

    let (a, b) = if sel_start_byte <= sel_end_byte {
        (sel_start_byte, sel_end_byte)
    } else {
        (sel_end_byte, sel_start_byte)
    };

    let first_line = byte_to_line(content, a);
    // If the selection ends exactly at the start of a line (after pressing
    // Home or selecting up to a newline), the user didn't really include that
    // line — back off so we don't toggle a comment on an uninvolved row.
    let last_line = {
        let raw = byte_to_line(content, b);
        if raw > first_line && b > 0 && content.as_bytes()[b - 1] == b'\n' {
            raw - 1
        } else {
            raw
        }
    };

    // Pass 1: decide add vs remove.
    let mut any_non_blank = false;
    let mut all_commented = true;
    for line in first_line..=last_line {
        let (ls, le) = line_byte_range(content, line);
        let slice = &content[ls..le];
        let trimmed = slice.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        any_non_blank = true;
        if !trimmed.starts_with(prefix) {
            all_commented = false;
            break;
        }
    }
    if !any_non_blank {
        return None;
    }

    // Pass 2: mutate lines bottom-up so earlier edits don't shift later offsets.
    let mut new_sel_start = sel_start_byte;
    let mut new_sel_end = sel_end_byte;

    for line in (first_line..=last_line).rev() {
        let (ls, le) = line_byte_range(content, line);
        let slice = &content[ls..le];
        if slice.trim_start().is_empty() {
            continue;
        }

        let indent_len = slice
            .bytes()
            .take_while(|b| *b == b' ' || *b == b'\t')
            .count();
        let indent_end = ls + indent_len;

        if all_commented {
            // Remove "<prefix> " or "<prefix>"
            let rest = &content[indent_end..le];
            let strip_len = if rest.starts_with(&prefix_with_space) {
                prefix_with_space.len()
            } else if rest.starts_with(prefix) {
                prefix.len()
            } else {
                0
            };
            if strip_len > 0 {
                content.replace_range(indent_end..indent_end + strip_len, "");
                // Adjust selection if the removed text was before it.
                if indent_end <= new_sel_start {
                    new_sel_start = new_sel_start.saturating_sub(strip_len);
                }
                if indent_end <= new_sel_end {
                    new_sel_end = new_sel_end.saturating_sub(strip_len);
                }
            }
        } else {
            // Insert "<prefix> " at indent_end.
            content.insert_str(indent_end, &prefix_with_space);
            let added = prefix_with_space.len();
            if indent_end <= new_sel_start {
                new_sel_start += added;
            }
            if indent_end <= new_sel_end {
                new_sel_end += added;
            }
        }
    }

    Some((new_sel_start, new_sel_end))
}

/// Find the matching bracket for the one immediately before or at `byte_idx`.
/// Returns the other bracket's byte offset, or `None` if the cursor isn't on a
/// bracket or no match was found.
pub fn find_matching_bracket(content: &str, byte_idx: usize) -> Option<(usize, usize)> {
    let bytes = content.as_bytes();
    // Prefer the bracket immediately *before* the cursor (VSCode behavior when
    // the caret is right after a closing bracket). Fall back to the one at the
    // cursor.
    let candidates = [byte_idx.checked_sub(1), Some(byte_idx)];
    for cand in candidates.iter().flatten() {
        let i = *cand;
        if i >= bytes.len() {
            continue;
        }
        let ch = bytes[i];
        let (open, close, forward) = match ch {
            b'(' => (b'(', b')', true),
            b'[' => (b'[', b']', true),
            b'{' => (b'{', b'}', true),
            b')' => (b'(', b')', false),
            b']' => (b'[', b']', false),
            b'}' => (b'{', b'}', false),
            _ => continue,
        };
        if let Some(m) = scan_bracket(bytes, i, open, close, forward) {
            return Some((i, m));
        }
    }
    None
}

fn scan_bracket(bytes: &[u8], start: usize, open: u8, close: u8, forward: bool) -> Option<usize> {
    let mut depth: i32 = 0;
    let mut i = start;
    loop {
        let ch = bytes[i];
        if ch == open {
            depth += if forward { 1 } else { -1 };
        } else if ch == close {
            depth += if forward { -1 } else { 1 };
        }
        if depth == 0 {
            return Some(i);
        }
        if forward {
            i += 1;
            if i >= bytes.len() {
                return None;
            }
        } else {
            if i == 0 {
                return None;
            }
            i -= 1;
        }
    }
}
