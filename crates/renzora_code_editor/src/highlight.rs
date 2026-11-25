//! Keyword-based syntax tokenizer.
//!
//! A single-pass tokenizer driven by a [`CodeSyntax`] (keyword and type lists,
//! comment and string delimiters), producing `(byte_len, TokenKind)` spans per
//! line. Intentionally simple: no parser, no tree-sitter, no syntect.
//!
//! The formats the editor itself owns are the [`Language`] enum, each turned
//! into a [`CodeSyntax`] by [`Language::syntax`]. A scripting language is a
//! plugin, so it brings its own [`CodeSyntax`] through `add_code_syntax`, and
//! [`highlight_syntax`] colours it with the same code.

use renzora_scripting_api::CodeSyntax;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Lua,
    Rust,
    Wgsl,
    Python,
    Shell,
    Sql,
    Json,
    Toml,
    /// Renzora's scene format (Bevy Scene Notation). Rust-like: `//`/`/* */`
    /// comments, `entity`/`resource` keywords, PascalCase component type paths.
    Bsn,
    /// Markup UI (ember/`bevy_hui` `.html` templates). Tag-structured; tokenized
    /// by [`highlight_html_line`] rather than the generic identifier path.
    Html,
    PlainText,
}

impl Language {
    pub fn from_extension(ext: &str) -> Self {
        match ext {
            "lua" => Language::Lua,
            "rs" => Language::Rust,
            "wgsl" | "wesl" | "glsl" => Language::Wgsl,
            "py" => Language::Python,
            "sh" | "bash" => Language::Shell,
            "sql" => Language::Sql,
            "json" => Language::Json,
            "toml" => Language::Toml,
            "bsn" => Language::Bsn,
            "html" | "htm" => Language::Html,
            _ => Language::PlainText,
        }
    }

    /// What the status bar calls the format.
    pub fn name(self) -> &'static str {
        match self {
            Language::Lua => "Lua",
            Language::Rust => "Rust",
            Language::Wgsl => "WGSL",
            Language::Python => "Python",
            Language::Shell => "Shell",
            Language::Sql => "SQL",
            Language::Json => "JSON",
            Language::Toml => "TOML",
            Language::Bsn => "BSN",
            Language::Html => "HTML",
            Language::PlainText => "Text",
        }
    }

    /// Line-comment prefix for the language, if it supports them.
    pub fn line_comment(self) -> Option<&'static str> {
        match self {
            Language::Lua | Language::Sql => Some("--"),
            Language::Rust | Language::Wgsl | Language::Bsn => Some("//"),
            Language::Python | Language::Shell | Language::Toml => Some("#"),
            Language::Json | Language::Html | Language::PlainText => None,
        }
    }

    /// Block-comment delimiters for languages that support them. Used by the
    /// Ctrl+Shift+/ wrap action.
    pub fn block_comment(self) -> Option<(&'static str, &'static str)> {
        match self {
            Language::Rust | Language::Wgsl | Language::Bsn => Some(("/*", "*/")),
            Language::Html => Some(("<!--", "-->")),
            _ => None,
        }
    }

    /// The format as the generic tokenizer reads it. HTML's block comment is
    /// left out: HTML has its own tokenizer, which owns `<!-- -->`.
    pub fn syntax(self) -> CodeSyntax {
        CodeSyntax {
            keywords: keywords_for(self),
            types: types_for(self),
            // BSN writes components as PascalCase type paths.
            caps_are_types: matches!(self, Language::Bsn),
            line_comment: self.line_comment(),
            block_comment: if self == Language::Html { None } else { self.block_comment() },
            multiline_strings: if self == Language::Python { &["\"\"\"", "'''"] } else { &[] },
            ..CodeSyntax::new(self.name(), &[])
        }
    }
}

fn keywords_for(lang: Language) -> &'static [&'static str] {
    match lang {
        Language::Lua => &[
            "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto",
            "if", "in", "local", "nil", "not", "or", "repeat", "return", "then", "true", "until",
            "while", "self",
        ],
        Language::Rust => &[
            "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
            "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
            "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
            "unsafe", "use", "where", "while", "async", "await", "dyn", "box", "try",
        ],
        Language::Wgsl => &[
            "fn", "let", "var", "const", "struct", "if", "else", "for", "while", "loop", "return",
            "discard", "switch", "case", "default", "break", "continue", "enable", "override",
            "alias", "true", "false",
        ],
        Language::Python => &[
            "def", "class", "if", "elif", "else", "for", "while", "return", "True", "False",
            "None", "import", "from", "as", "try", "except", "finally", "with", "yield", "lambda",
            "pass", "raise", "global", "nonlocal", "and", "or", "not", "in", "is", "break",
            "continue", "async", "await",
        ],
        Language::Shell => &[
            "if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac",
            "function", "return", "local", "export",
        ],
        Language::Sql => &[
            "SELECT", "FROM", "WHERE", "INSERT", "INTO", "UPDATE", "DELETE", "CREATE", "TABLE",
            "DROP", "JOIN", "LEFT", "RIGHT", "INNER", "ON", "AS", "ORDER", "BY", "GROUP", "HAVING",
            "LIMIT", "OFFSET", "IN", "NOT", "AND", "OR", "NULL", "IS", "LIKE", "VALUES",
        ],
        Language::Bsn => &[
            "entity", "resource", "true", "false", "Some", "None",
        ],
        Language::Toml | Language::Json | Language::Html | Language::PlainText => &[],
    }
}

fn types_for(lang: Language) -> &'static [&'static str] {
    match lang {
        Language::Rust => &[
            "bool", "char", "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64",
            "i128", "isize", "f32", "f64", "str", "String", "Vec", "Option", "Result", "Box", "Rc",
            "Arc", "HashMap", "HashSet", "BTreeMap", "BTreeSet",
        ],
        Language::Wgsl => &[
            "bool",
            "i32",
            "u32",
            "f32",
            "f16",
            "vec2",
            "vec3",
            "vec4",
            "vec2i",
            "vec3i",
            "vec4i",
            "vec2u",
            "vec3u",
            "vec4u",
            "vec2f",
            "vec3f",
            "vec4f",
            "mat2x2",
            "mat2x3",
            "mat2x4",
            "mat3x2",
            "mat3x3",
            "mat3x4",
            "mat4x2",
            "mat4x3",
            "mat4x4",
            "texture_2d",
            "texture_3d",
            "texture_cube",
            "sampler",
            "sampler_comparison",
            "array",
            "atomic",
            "ptr",
        ],
        Language::Python => &[
            "int", "float", "str", "bool", "list", "dict", "tuple", "set",
        ],
        _ => &[],
    }
}

/// Token category. The renderer maps each kind to a color at assembly time,
/// so cached token kinds stay valid across theme / font changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Normal,
    Keyword,
    Type,
    Function,
    Number,
    String,
    Comment,
}

/// Cross-line tokenizer state carried into the next line: whether we're inside
/// a block comment, an HTML `<!-- ... -->` comment, or a string that spans
/// lines (the index of its delimiter in [`CodeSyntax::multiline_strings`]).
/// Lua `--[[` long comments are treated as line comments.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineState {
    #[default]
    Normal,
    BlockComment,
    HtmlComment,
    MultilineString(u8),
}

/// [`LineState::MultilineString`]'s encoding starts here, clear of the others.
const MULTILINE_STRING_BASE: u32 = 16;

impl LineState {
    /// Encode for the ember widget, which threads cross-line state as an opaque
    /// `u32` between lines.
    pub fn to_u32(self) -> u32 {
        match self {
            LineState::Normal => 0,
            LineState::BlockComment => 1,
            LineState::HtmlComment => 2,
            LineState::MultilineString(i) => MULTILINE_STRING_BASE + i as u32,
        }
    }

    /// Decode the widget's threaded state back into a [`LineState`].
    pub fn from_u32(v: u32) -> Self {
        match v {
            1 => LineState::BlockComment,
            2 => LineState::HtmlComment,
            v if v >= MULTILINE_STRING_BASE => {
                LineState::MultilineString((v - MULTILINE_STRING_BASE).min(u8::MAX as u32) as u8)
            }
            _ => LineState::Normal,
        }
    }
}

/// Tokenize a SINGLE line (no trailing `\n`) of one of the editor's own
/// formats, given the incoming cross-line state. Returns the spans as
/// `(byte_len, kind)` pairs that exactly cover the line, plus the outgoing
/// state for the next line.
pub fn highlight_line(
    line: &str,
    lang: Language,
    incoming: LineState,
) -> (Vec<(u32, TokenKind)>, LineState) {
    match lang {
        Language::PlainText => {
            let spans = if line.is_empty() { Vec::new() } else { vec![(line.len() as u32, TokenKind::Normal)] };
            (spans, LineState::Normal)
        }
        // HTML is tag-structured, not identifier-structured, so it has its own
        // tokenizer (which also owns the `<!-- -->` continuation).
        Language::Html => highlight_html_line(line, incoming),
        _ => highlight_syntax(line, &lang.syntax(), incoming),
    }
}

/// Where `needle` sits in `bytes` at `i`.
fn starts_at(bytes: &[u8], i: usize, needle: &str) -> bool {
    let n = needle.as_bytes();
    !n.is_empty() && i + n.len() <= bytes.len() && &bytes[i..i + n.len()] == n
}

/// Scan a string body from `from` for `close`, skipping backslash escapes.
/// Returns the index just past `close`, or `None` if the line ends first.
fn string_end(bytes: &[u8], from: usize, close: &str) -> Option<usize> {
    let mut j = from;
    while j < bytes.len() {
        if bytes[j] == b'\\' {
            j += 2;
            continue;
        }
        if starts_at(bytes, j, close) {
            return Some(j + close.len());
        }
        j += 1;
    }
    None
}

/// Tokenize a SINGLE line by `syntax`: the path every identifier-structured
/// language takes, the editor's own and every plugin's alike.
pub fn highlight_syntax(
    line: &str,
    syntax: &CodeSyntax,
    incoming: LineState,
) -> (Vec<(u32, TokenKind)>, LineState) {
    let mut spans: Vec<(u32, TokenKind)> = Vec::new();
    let mut push = |len: usize, kind: TokenKind| {
        if len > 0 {
            spans.push((len as u32, kind));
        }
    };

    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;

    // Continuation of a block comment started on a previous line.
    if incoming == LineState::BlockComment {
        if let Some((_, close)) = syntax.block_comment {
            match find_sub(bytes, close.as_bytes(), 0) {
                Some(p) => {
                    push(p + close.len(), TokenKind::Comment);
                    i = p + close.len();
                }
                None => {
                    push(len, TokenKind::Comment);
                    return (spans, LineState::BlockComment);
                }
            }
        }
    }

    // Continuation of a string started on a previous line.
    if let LineState::MultilineString(k) = incoming {
        if let Some(delim) = syntax.multiline_strings.get(k as usize) {
            match string_end(bytes, 0, delim) {
                Some(end) => {
                    push(end, TokenKind::String);
                    i = end;
                }
                None => {
                    push(len, TokenKind::String);
                    return (spans, incoming);
                }
            }
        }
    }

    let keywords = syntax.keywords;
    let types = syntax.types;
    let quotes = syntax.quotes.as_bytes();
    let ident_extra = syntax.identifier_chars.as_bytes();
    let is_ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || ident_extra.contains(&c);

    'scan: while i < len {
        let b = bytes[i];

        // --- Block comment (may not close on this line) ---
        // Before the line comment, so Lua's `--[[` is not taken for `--`.
        if let Some((open, close)) = syntax.block_comment {
            if starts_at(bytes, i, open) {
                match find_sub(bytes, close.as_bytes(), i + open.len()) {
                    Some(p) => {
                        push(p + close.len() - i, TokenKind::Comment);
                        i = p + close.len();
                        continue;
                    }
                    None => {
                        push(len - i, TokenKind::Comment);
                        return (spans, LineState::BlockComment);
                    }
                }
            }
        }

        // --- Line comment (to end of line) ---
        if let Some(prefix) = syntax.line_comment {
            if starts_at(bytes, i, prefix) {
                push(len - i, TokenKind::Comment);
                break;
            }
        }

        // --- String that may span lines: `...` or """...""" ---
        for (k, delim) in syntax.multiline_strings.iter().enumerate() {
            if starts_at(bytes, i, delim) {
                match string_end(bytes, i + delim.len(), delim) {
                    Some(end) => {
                        push(end - i, TokenKind::String);
                        i = end;
                        continue 'scan;
                    }
                    None => {
                        push(len - i, TokenKind::String);
                        return (spans, LineState::MultilineString(k.min(u8::MAX as usize) as u8));
                    }
                }
            }
        }

        // --- String "..." or '...' (single line) ---
        if quotes.contains(&b) {
            let quote = b;
            let mut j = i + 1;
            while j < len && bytes[j] != quote {
                if bytes[j] == b'\\' && j + 1 < len {
                    j += 2;
                    continue;
                }
                j += 1;
            }
            let end = (j + 1).min(len);
            push(end - i, TokenKind::String);
            i = end;
            continue;
        }

        // --- Number literal ---
        if b.is_ascii_digit() || (b == b'.' && i + 1 < len && bytes[i + 1].is_ascii_digit()) {
            let mut j = i;
            let mut has_dot = false;
            let mut has_exp = false;
            if bytes[j] == b'.' {
                // Consume the leading `.` of a `.5`-style literal HERE. Only
                // marking `has_dot` left `j == i`: the scan below sees the dot,
                // finds `has_dot` already set, and breaks on its first
                // iteration, so the span is empty, `i` never advances, and this
                // branch is re-entered forever. That froze the whole editor on
                // any line containing `.` followed by a digit — `.5`, or a Rust
                // tuple index like `v.0` — since the tokenizer runs on every
                // visible line every frame. Reported as a "crash" while holding
                // Delete (issue #84): deleting the `0` from `0.5` produces `.5`.
                has_dot = true;
                j += 1;
            }
            while j < len {
                let c = bytes[j];
                if c.is_ascii_digit() || c == b'_' {
                    j += 1;
                } else if c == b'.' && !has_dot && !has_exp {
                    has_dot = true;
                    j += 1;
                } else if (c == b'e' || c == b'E') && !has_exp {
                    has_exp = true;
                    j += 1;
                    if j < len && (bytes[j] == b'+' || bytes[j] == b'-') {
                        j += 1;
                    }
                } else {
                    break;
                }
            }
            // Optional Rust-style numeric suffix: f32, i64, u8, etc.
            if j < len && (bytes[j] == b'f' || bytes[j] == b'i' || bytes[j] == b'u') {
                let sfx_start = j + 1;
                let mut k = sfx_start;
                while k < len && bytes[k].is_ascii_digit() {
                    k += 1;
                }
                if k > sfx_start {
                    j = k;
                }
            }
            push(j - i, TokenKind::Number);
            i = j;
            continue;
        }

        // --- Identifier / keyword / type / function name ---
        if b.is_ascii_alphabetic() || b == b'_' || ident_extra.contains(&b) {
            let mut j = i + 1;
            while j < len && is_ident(bytes[j]) {
                j += 1;
            }
            let word = &line[i..j];
            let kind = if keywords.contains(&word) {
                TokenKind::Keyword
            } else if types.contains(&word)
                || (syntax.caps_are_types && word.as_bytes()[0].is_ascii_uppercase())
            {
                TokenKind::Type
            } else if j < len && bytes[j] == b'(' {
                TokenKind::Function
            } else {
                TokenKind::Normal
            };
            push(j - i, kind);
            i = j;
            continue;
        }

        // --- Anything else: one full char (UTF-8 safe) ---
        let ch_len = line[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
        let end = (i + ch_len).min(len);
        push(end - i, TokenKind::Normal);
        i = end;
    }

    (spans, LineState::Normal)
}

/// Find `needle` in `hay[from..]`, returning the absolute start index.
fn find_sub(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || from >= hay.len() || needle.len() > hay.len() {
        return None;
    }
    (from..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
}

/// Tokenize one line of HTML. Handles `<!-- -->` comments (carried across lines
/// via [`LineState::HtmlComment`]), tags (name = keyword, attributes = type,
/// quoted values = string), `&entity;` references, and text. Tags are parsed
/// per-line — a tag whose attributes wrap across lines highlights the
/// continuation as text, which is the same pragmatic limit as the rest of this
/// non-parser tokenizer.
fn highlight_html_line(line: &str, incoming: LineState) -> (Vec<(u32, TokenKind)>, LineState) {
    let mut spans: Vec<(u32, TokenKind)> = Vec::new();
    fn push(spans: &mut Vec<(u32, TokenKind)>, len: usize, kind: TokenKind) {
        if len > 0 {
            spans.push((len as u32, kind));
        }
    }

    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;

    // Continuation of a comment opened on a previous line.
    if incoming == LineState::HtmlComment {
        match find_sub(bytes, b"-->", 0) {
            Some(p) => {
                push(&mut spans, p + 3, TokenKind::Comment);
                i = p + 3;
            }
            None => {
                push(&mut spans, len, TokenKind::Comment);
                return (spans, LineState::HtmlComment);
            }
        }
    }

    while i < len {
        let b = bytes[i];

        // --- Comment <!-- ... --> (may not close on this line) ---
        if b == b'<' && i + 4 <= len && &bytes[i..i + 4] == b"<!--" {
            match find_sub(bytes, b"-->", i + 4) {
                Some(p) => {
                    push(&mut spans, (p + 3) - i, TokenKind::Comment);
                    i = p + 3;
                }
                None => {
                    push(&mut spans, len - i, TokenKind::Comment);
                    return (spans, LineState::HtmlComment);
                }
            }
            continue;
        }

        // --- Tag <...> (or to end of line if it doesn't close here) ---
        if b == b'<' {
            let stop = find_sub(bytes, b">", i + 1).map(|p| p + 1).unwrap_or(len);
            push_tag(&mut spans, bytes, i, stop);
            i = stop;
            continue;
        }

        // --- Entity reference &name; / &#123; ---
        if b == b'&' {
            let mut j = i + 1;
            while j < len && bytes[j] != b';' && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'#') {
                j += 1;
            }
            if j < len && bytes[j] == b';' {
                push(&mut spans, (j + 1) - i, TokenKind::Number);
                i = j + 1;
                continue;
            }
            push(&mut spans, 1, TokenKind::Normal);
            i += 1;
            continue;
        }

        // --- Text until the next tag or entity ---
        let mut j = i;
        while j < len && bytes[j] != b'<' && bytes[j] != b'&' {
            j += 1;
        }
        push(&mut spans, j - i, TokenKind::Normal);
        i = j;
    }

    (spans, LineState::Normal)
}

/// Tokenize a single tag span `bytes[start..stop]` (from `<` up to and including
/// `>`, or to the slice end). Opening punctuation and tag name first, then
/// attributes (name = type, `="value"` = string).
fn push_tag(spans: &mut Vec<(u32, TokenKind)>, bytes: &[u8], start: usize, stop: usize) {
    fn is_name(c: u8) -> bool {
        c.is_ascii_alphanumeric() || c == b'-' || c == b'_' || c == b':' || c == b'!'
    }
    let mut i = start;
    // Leading '<' and an optional '/' (closing tag).
    spans.push((1, TokenKind::Normal));
    i += 1;
    if i < stop && bytes[i] == b'/' {
        spans.push((1, TokenKind::Normal));
        i += 1;
    }
    // Tag name (includes `!doctype`, `!DOCTYPE`).
    let s = i;
    while i < stop && is_name(bytes[i]) {
        i += 1;
    }
    if i > s {
        spans.push(((i - s) as u32, TokenKind::Keyword));
    }
    // Remainder: attributes, values, punctuation.
    while i < stop {
        let b = bytes[i];
        if b == b'"' || b == b'\'' {
            let quote = b;
            let mut j = i + 1;
            while j < stop && bytes[j] != quote {
                j += 1;
            }
            let end = (j + 1).min(stop);
            spans.push(((end - i) as u32, TokenKind::String));
            i = end;
        } else if b.is_ascii_alphabetic() || b == b'_' {
            let s = i;
            while i < stop && is_name(bytes[i]) {
                i += 1;
            }
            spans.push(((i - s) as u32, TokenKind::Type));
        } else {
            spans.push((1, TokenKind::Normal));
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reconstruct `(text, kind)` runs so assertions can match on substrings.
    fn toks(line: &str, lang: Language, st: LineState) -> (Vec<(String, TokenKind)>, LineState) {
        let (spans, out) = highlight_line(line, lang, st);
        let mut runs = Vec::new();
        let mut i = 0usize;
        for (len, kind) in spans {
            let l = len as usize;
            runs.push((line[i..i + l].to_string(), kind));
            i += l;
        }
        // Invariant the renderer relies on: spans exactly cover the line.
        assert_eq!(i, line.len(), "spans must cover the whole line: {line:?}");
        (runs, out)
    }

    fn has(runs: &[(String, TokenKind)], text: &str, kind: TokenKind) -> bool {
        runs.iter().any(|(t, k)| t == text && *k == kind)
    }

    #[test]
    fn bsn_entity_and_component() {
        let (r, _) = toks("entity 4294967296 {", Language::Bsn, LineState::Normal);
        assert!(has(&r, "entity", TokenKind::Keyword));
        assert!(has(&r, "4294967296", TokenKind::Number));

        // PascalCase component/type names color as Type; lowercase path
        // segments stay Normal.
        let (r, _) = toks("    bevy::transform::Transform(translation: (0.0))", Language::Bsn, LineState::Normal);
        assert!(has(&r, "Transform", TokenKind::Type));
        assert!(has(&r, "bevy", TokenKind::Normal));
        assert!(has(&r, "translation", TokenKind::Normal));
        assert!(has(&r, "0.0", TokenKind::Number));
    }

    #[test]
    fn bsn_line_and_block_comments() {
        let (r, _) = toks("// renzora interim bsn v1", Language::Bsn, LineState::Normal);
        assert!(matches!(r.as_slice(), [(_, TokenKind::Comment)]));

        // Block comment that doesn't close carries into the next line.
        let (_, st) = toks("Foo /* open", Language::Bsn, LineState::Normal);
        assert_eq!(st, LineState::BlockComment);
        let (r, st) = toks("still */ Bar", Language::Bsn, LineState::BlockComment);
        assert_eq!(st, LineState::Normal);
        assert!(has(&r, "still */", TokenKind::Comment));
        assert!(has(&r, "Bar", TokenKind::Type));
    }

    #[test]
    fn html_tag_attrs_and_text() {
        let (r, _) = toks(r#"<div class="box">hi</div>"#, Language::Html, LineState::Normal);
        assert!(has(&r, "div", TokenKind::Keyword));
        assert!(has(&r, "class", TokenKind::Type));
        assert!(has(&r, "\"box\"", TokenKind::String));
        assert!(has(&r, "hi", TokenKind::Normal));
    }

    #[test]
    fn html_entity_and_comment_threading() {
        let (r, _) = toks("a &amp; b", Language::Html, LineState::Normal);
        assert!(has(&r, "&amp;", TokenKind::Number));

        // A comment opened on one line stays a comment until `-->`.
        let (r, st) = toks("<!-- start", Language::Html, LineState::Normal);
        assert_eq!(st, LineState::HtmlComment);
        assert!(matches!(r.as_slice(), [(_, TokenKind::Comment)]));
        let (r, st) = toks("mid", Language::Html, LineState::HtmlComment);
        assert_eq!(st, LineState::HtmlComment);
        assert!(has(&r, "mid", TokenKind::Comment));
        let (_, st) = toks("end --><b>", Language::Html, LineState::HtmlComment);
        assert_eq!(st, LineState::Normal);
    }

    /// Regression for issue #84. A `.`-leading number literal used to leave the
    /// scan at `j == i`, so the tokenizer never advanced and spun forever — the
    /// editor froze (no panic, no crash log) as soon as a visible line contained
    /// `.` followed by a digit. A regression here shows up as this test hanging,
    /// not failing.
    #[test]
    fn dot_leading_number_terminates() {
        let (r, _) = toks("    .5 * delta", Language::Lua, LineState::Normal);
        assert!(has(&r, ".5", TokenKind::Number));

        // Same shape, and perfectly ordinary source: a Rust tuple index.
        let (r, _) = toks("let a = v.0;", Language::Rust, LineState::Normal);
        assert!(has(&r, ".0", TokenKind::Number));

        // Every intermediate state of deleting forward through `0.5 * delta`
        // (what the reporter did by holding Delete) must terminate.
        let line = "    0.5 * delta";
        for cut in 0..line.chars().count() {
            let partial: String = line
                .chars()
                .enumerate()
                .filter(|(i, _)| *i < 4 || *i >= 4 + cut)
                .map(|(_, c)| c)
                .collect();
            toks(&partial, Language::Lua, LineState::Normal);
        }

        // Multiple dots must not re-enter the number branch on the same byte.
        let (r, _) = toks("1.2.3", Language::Lua, LineState::Normal);
        assert!(has(&r, "1.2", TokenKind::Number));
        assert!(has(&r, ".3", TokenKind::Number));
    }

    /// A plugin-registered syntax shaped like JavaScript's.
    const JS: CodeSyntax = CodeSyntax {
        keywords: &["function", "const", "return"],
        caps_are_types: true,
        line_comment: Some("//"),
        block_comment: Some(("/*", "*/")),
        multiline_strings: &["`"],
        identifier_chars: "$",
        ..CodeSyntax::new("JavaScript", &["js"])
    };

    fn stoks(line: &str, st: LineState) -> (Vec<(String, TokenKind)>, LineState) {
        let (spans, out) = highlight_syntax(line, &JS, st);
        let mut runs = Vec::new();
        let mut i = 0usize;
        for (len, kind) in spans {
            runs.push((line[i..i + len as usize].to_string(), kind));
            i += len as usize;
        }
        assert_eq!(i, line.len(), "spans must cover the whole line: {line:?}");
        (runs, out)
    }

    #[test]
    fn plugin_syntax_keywords_types_and_dollar_identifiers() {
        let (r, _) = stoks("const $el = Mood.Angry; // note", LineState::Normal);
        assert!(has(&r, "const", TokenKind::Keyword));
        assert!(has(&r, "$el", TokenKind::Normal));
        assert!(has(&r, "Mood", TokenKind::Type));
        assert!(has(&r, "// note", TokenKind::Comment));
    }

    #[test]
    fn plugin_syntax_template_string_spans_lines() {
        let (r, st) = stoks("log(`one ${a}", LineState::Normal);
        assert!(has(&r, "log", TokenKind::Function));
        assert!(has(&r, "`one ${a}", TokenKind::String));
        assert_eq!(st, LineState::MultilineString(0));
        let (r, st) = stoks("two \\` still` + 1", st);
        assert!(has(&r, "two \\` still`", TokenKind::String));
        assert!(has(&r, "1", TokenKind::Number));
        assert_eq!(st, LineState::Normal);
    }

    #[test]
    fn plugin_syntax_block_comment_spans_lines() {
        let (_, st) = stoks("x /* open", LineState::Normal);
        assert_eq!(st, LineState::BlockComment);
        let (r, st) = stoks("done */ return", st);
        assert!(has(&r, "done */", TokenKind::Comment));
        assert!(has(&r, "return", TokenKind::Keyword));
        assert_eq!(st, LineState::Normal);
    }

    /// A block comment whose opener starts with the line comment: Lua's
    /// `--[[` must not be taken for `--`.
    #[test]
    fn block_comment_wins_over_its_line_comment_prefix() {
        const LUA: CodeSyntax = CodeSyntax {
            keywords: &["local"],
            line_comment: Some("--"),
            block_comment: Some(("--[[", "]]")),
            ..CodeSyntax::new("Lua", &["lua"])
        };
        let (_, st) = highlight_syntax("--[[ open", &LUA, LineState::Normal);
        assert_eq!(st, LineState::BlockComment);
        let (spans, st) = highlight_syntax("]] local", &LUA, st);
        assert_eq!(st, LineState::Normal);
        assert_eq!(spans.last(), Some(&(5, TokenKind::Keyword)));
        let (_, st) = highlight_syntax("-- plain", &LUA, LineState::Normal);
        assert_eq!(st, LineState::Normal);
    }

    #[test]
    fn python_triple_quotes_span_lines() {
        let (_, st) = toks("s = \"\"\"doc", Language::Python, LineState::Normal);
        assert_eq!(st, LineState::MultilineString(0));
        let (r, st) = toks("end\"\"\" if x", Language::Python, st);
        assert!(has(&r, "if", TokenKind::Keyword));
        assert_eq!(st, LineState::Normal);
    }

    #[test]
    fn line_state_u32_roundtrip() {
        for s in [LineState::Normal, LineState::BlockComment, LineState::HtmlComment, LineState::MultilineString(0), LineState::MultilineString(1)] {
            assert_eq!(LineState::from_u32(s.to_u32()), s);
        }
    }
}
