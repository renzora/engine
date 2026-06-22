//! Single-pass keyword highlighter — splits a line into colored token runs.
//! No parser; pulls token colors from the active theme's [`SyntaxPalette`] so
//! the fallback tokenizer recolors with the theme like everything else.

use crate::theme::syntax_palette;

const KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for",
    "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where",
    "while", "async", "await", "dyn",
];
const TYPES: &[&str] = &[
    "bool", "char", "u8", "u16", "u32", "u64", "usize", "i8", "i16", "i32", "i64", "isize", "f32",
    "f64", "str", "String", "Vec", "Option", "Result", "Box", "Entity", "Vec2", "Vec3", "Color",
    "Commands", "Query", "Node",
];

/// Split a line into colored `(text, color)` token runs, coloring from the
/// active theme's [`SyntaxPalette`].
pub(crate) fn tokenize(line: &str) -> Vec<(String, (u8, u8, u8))> {
    let sp = syntax_palette();
    let chars: Vec<char> = line.chars().collect();
    let n = chars.len();
    let mut out: Vec<(String, (u8, u8, u8))> = Vec::new();
    let mut normal = String::new();
    let mut i = 0;
    while i < n {
        let c = chars[i];
        if c == '/' && i + 1 < n && chars[i + 1] == '/' {
            if !normal.is_empty() {
                out.push((std::mem::take(&mut normal), sp.normal));
            }
            out.push((chars[i..].iter().collect(), sp.comment));
            return out;
        }
        if c == '"' {
            if !normal.is_empty() {
                out.push((std::mem::take(&mut normal), sp.normal));
            }
            let mut s = String::from('"');
            i += 1;
            while i < n {
                s.push(chars[i]);
                let closed = chars[i] == '"';
                i += 1;
                if closed {
                    break;
                }
            }
            out.push((s, sp.string));
            continue;
        }
        if c.is_alphanumeric() || c == '_' {
            if !normal.is_empty() {
                out.push((std::mem::take(&mut normal), sp.normal));
            }
            let start = i;
            while i < n && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let color = if word.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                sp.number
            } else if KEYWORDS.contains(&word.as_str()) {
                sp.keyword
            } else if TYPES.contains(&word.as_str()) {
                sp.type_
            } else if i < n && chars[i] == '(' {
                sp.function
            } else {
                sp.normal
            };
            out.push((word, color));
            continue;
        }
        normal.push(c);
        i += 1;
    }
    if !normal.is_empty() {
        out.push((normal, sp.normal));
    }
    out
}
