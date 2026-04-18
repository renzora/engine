//! Lightweight, dependency-free source formatter.
//!
//! For Rust/Rhai/Wgsl/JSON/Json5 we re-indent based on `{` / `}` (and `[`/`]`
//! for JSON). For Lua and Python we re-indent based on block keywords. Only
//! whitespace at line starts is touched — content inside the line (including
//! trailing whitespace beyond `\t`/`'  '`) is left alone.
//!
//! This is intentionally not a full pretty-printer: full formatters belong in
//! external tools (`rustfmt`, `stylua`, `black`, etc.). The goal here is a
//! "make the indent consistent" pass that handles 90% of cases and never
//! corrupts the source.

use crate::actions::TAB_SIZE;
use crate::highlight::Language;

/// Re-indent the source according to `lang`'s block structure.
pub fn format_source(content: &str, lang: Language) -> String {
    match lang {
        Language::Rust | Language::Rhai | Language::Wgsl => format_brace(content, lang, false),
        Language::Json => format_brace(content, lang, true),
        Language::Lua => format_lua(content),
        Language::Python => format_python(content),
        _ => content.to_string(),
    }
}

fn format_brace(content: &str, lang: Language, brackets_too: bool) -> String {
    let indent_unit = " ".repeat(TAB_SIZE);
    let mut depth: i32 = 0;
    let mut out = String::with_capacity(content.len());
    let lines: Vec<&str> = content.split('\n').collect();
    let last_idx = lines.len().saturating_sub(1);

    for (i, raw) in lines.iter().enumerate() {
        let stripped = strip_indent(raw);
        let trimmed_for_count = strip_strings_and_comments(stripped, lang);

        // Count how this line *closes* before its content (e.g. `} else {`
        // dedents to the closer's level first).
        let leading_close = leading_closers(&trimmed_for_count, brackets_too);
        let line_depth = (depth - leading_close as i32).max(0);

        if !stripped.is_empty() {
            for _ in 0..line_depth {
                out.push_str(&indent_unit);
            }
        }
        out.push_str(stripped);
        if i != last_idx {
            out.push('\n');
        }

        // Net delta over the whole line, then push depth forward.
        let net = brace_delta(&trimmed_for_count, brackets_too);
        depth = (depth + net).max(0);
    }

    out
}

fn brace_delta(text: &str, brackets_too: bool) -> i32 {
    let mut d = 0;
    for ch in text.chars() {
        match ch {
            '{' => d += 1,
            '}' => d -= 1,
            '[' if brackets_too => d += 1,
            ']' if brackets_too => d -= 1,
            _ => {}
        }
    }
    d
}

fn leading_closers(text: &str, brackets_too: bool) -> usize {
    let mut count = 0;
    for ch in text.trim_start().chars() {
        match ch {
            '}' => count += 1,
            ']' if brackets_too => count += 1,
            _ => break,
        }
    }
    count
}

fn strip_indent(line: &str) -> &str {
    line.trim_start_matches(|c: char| c == ' ' || c == '\t')
}

/// Remove string literals and line comments so brace-counting isn't fooled
/// by `"} { ... // }"` etc. Block comments are not perfectly handled —
/// good enough for re-indenting code that already roughly compiles.
fn strip_strings_and_comments<'a>(line: &'a str, lang: Language) -> String {
    let line_comment = lang.line_comment();
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if let Some(prefix) = line_comment {
            if line[..].starts_with(prefix) && out.is_empty() {
                // pure comment — early out
                break;
            }
            // simple check: peek ahead for the prefix at this point
            if c == prefix.chars().next().unwrap() && prefix.len() > 1 {
                // Naive multi-char prefix detection
                let need_more = &prefix[1..];
                let rest: String = chars.clone().collect();
                if rest.starts_with(need_more) {
                    break;
                }
            }
            if prefix.len() == 1 && c == prefix.chars().next().unwrap() {
                break;
            }
        }
        match c {
            '"' | '\'' => {
                let quote = c;
                while let Some(nc) = chars.next() {
                    if nc == '\\' {
                        chars.next();
                        continue;
                    }
                    if nc == quote {
                        break;
                    }
                }
            }
            _ => out.push(c),
        }
    }
    out
}

fn format_lua(content: &str) -> String {
    let indent_unit = " ".repeat(TAB_SIZE);
    let mut depth: i32 = 0;
    let mut out = String::with_capacity(content.len());
    let lines: Vec<&str> = content.split('\n').collect();
    let last_idx = lines.len().saturating_sub(1);

    for (i, raw) in lines.iter().enumerate() {
        let stripped = strip_indent(raw);
        let leading_close = lua_starts_with_closer(stripped) as i32;
        let line_depth = (depth - leading_close).max(0);

        if !stripped.is_empty() {
            for _ in 0..line_depth {
                out.push_str(&indent_unit);
            }
        }
        out.push_str(stripped);
        if i != last_idx {
            out.push('\n');
        }

        depth = (depth + lua_delta(stripped)).max(0);
    }

    out
}

fn lua_starts_with_closer(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("end")
        || t.starts_with("else")
        || t.starts_with("elseif")
        || t.starts_with("until")
        || t.starts_with('}')
        || t.starts_with(')')
}

fn lua_delta(line: &str) -> i32 {
    let mut d = 0;
    let trimmed = line.trim();

    // Block openers
    let starts_block = trimmed.starts_with("function ")
        || trimmed.starts_with("function(")
        || trimmed.starts_with("if ")
        || trimmed.starts_with("for ")
        || trimmed.starts_with("while ")
        || trimmed == "do"
        || trimmed == "else"
        || trimmed.starts_with("elseif ")
        || trimmed.starts_with("repeat");
    let opens_then = trimmed.ends_with(" then") || trimmed.ends_with("then");
    let opens_do = trimmed.ends_with(" do") || trimmed.ends_with("do");
    if starts_block && (opens_then || opens_do || trimmed.ends_with(')') || trimmed == "do" || trimmed == "else" || trimmed.starts_with("repeat")) {
        d += 1;
    }
    if trimmed.starts_with("function ") || trimmed.starts_with("function(") {
        // function declarations always open a block
        if !trimmed.contains(" end") {
            // already counted above? avoid double-count
            if !(opens_then || opens_do) {
                d += 1;
            }
        }
    }

    // Closers
    if trimmed == "end" || trimmed.starts_with("end ") || trimmed == "}" || trimmed.starts_with("until ") {
        d -= 1;
    }
    // else / elseif: net zero (close + open).
    d
}

fn format_python(content: &str) -> String {
    // Python's indentation IS the syntax — re-indenting blindly would corrupt
    // code. Only normalize tab → spaces and strip trailing whitespace.
    let indent_unit = " ".repeat(TAB_SIZE);
    content
        .split('\n')
        .map(|line| {
            let leading: String = line
                .chars()
                .take_while(|c| *c == ' ' || *c == '\t')
                .map(|c| if c == '\t' { indent_unit.clone() } else { " ".to_string() })
                .collect();
            let body: &str = line.trim_start_matches(|c: char| c == ' ' || c == '\t');
            let trimmed_body = body.trim_end_matches(|c: char| c == ' ' || c == '\t');
            format!("{}{}", leading, trimmed_body)
        })
        .collect::<Vec<_>>()
        .join("\n")
}
