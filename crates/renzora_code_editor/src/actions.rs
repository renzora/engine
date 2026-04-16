//! Editor text operations: comment toggle, goto line, bracket matching.

use crate::highlight::Language;

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
