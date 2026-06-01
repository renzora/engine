//! Single-pass keyword highlighter — splits a line into colored token runs.
//! No parser; mirrors the editor's keyword/type palette.

pub(crate) const C_TEXT: (u8, u8, u8) = (208, 208, 222);
const C_KEYWORD: (u8, u8, u8) = (230, 100, 90);
const C_TYPE: (u8, u8, u8) = (240, 190, 80);
const C_FN: (u8, u8, u8) = (160, 210, 110);
const C_NUM: (u8, u8, u8) = (210, 140, 200);
const C_STR: (u8, u8, u8) = (170, 210, 130);
const C_COMMENT: (u8, u8, u8) = (120, 120, 135);

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

/// Split a line into colored `(text, color)` token runs.
pub(crate) fn tokenize(line: &str) -> Vec<(String, (u8, u8, u8))> {
    let chars: Vec<char> = line.chars().collect();
    let n = chars.len();
    let mut out: Vec<(String, (u8, u8, u8))> = Vec::new();
    let mut normal = String::new();
    let mut i = 0;
    while i < n {
        let c = chars[i];
        if c == '/' && i + 1 < n && chars[i + 1] == '/' {
            if !normal.is_empty() {
                out.push((std::mem::take(&mut normal), C_TEXT));
            }
            out.push((chars[i..].iter().collect(), C_COMMENT));
            return out;
        }
        if c == '"' {
            if !normal.is_empty() {
                out.push((std::mem::take(&mut normal), C_TEXT));
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
            out.push((s, C_STR));
            continue;
        }
        if c.is_alphanumeric() || c == '_' {
            if !normal.is_empty() {
                out.push((std::mem::take(&mut normal), C_TEXT));
            }
            let start = i;
            while i < n && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let color = if word.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                C_NUM
            } else if KEYWORDS.contains(&word.as_str()) {
                C_KEYWORD
            } else if TYPES.contains(&word.as_str()) {
                C_TYPE
            } else if i < n && chars[i] == '(' {
                C_FN
            } else {
                C_TEXT
            };
            out.push((word, color));
            continue;
        }
        normal.push(c);
        i += 1;
    }
    if !normal.is_empty() {
        out.push((normal, C_TEXT));
    }
    out
}
