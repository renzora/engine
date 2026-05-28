//! Keyword-based syntax highlighter.
//!
//! Produces an egui `LayoutJob` with colored tokens for the supported
//! languages. Intentionally simple: a single-pass tokenizer with per-language
//! keyword / type lists. No parser, no tree-sitter, no syntect — those are a
//! follow-up if/when we need richer grammars.

use bevy_egui::egui::{text::LayoutJob, Color32, FontId, TextFormat};
use renzora_theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Lua,
    Rhai,
    Rust,
    Wgsl,
    Python,
    Shell,
    Sql,
    Json,
    Toml,
    PlainText,
}

impl Language {
    pub fn from_extension(ext: &str) -> Self {
        match ext {
            "lua" => Language::Lua,
            "rhai" => Language::Rhai,
            "rs" => Language::Rust,
            "wgsl" | "glsl" => Language::Wgsl,
            "py" => Language::Python,
            "sh" | "bash" => Language::Shell,
            "sql" => Language::Sql,
            "json" => Language::Json,
            "toml" => Language::Toml,
            _ => Language::PlainText,
        }
    }

    /// Line-comment prefix for the language, if it supports them.
    pub fn line_comment(self) -> Option<&'static str> {
        match self {
            Language::Lua => Some("--"),
            Language::Rhai | Language::Rust | Language::Wgsl | Language::Sql => Some("//"),
            Language::Python | Language::Shell | Language::Toml => Some("#"),
            Language::Json | Language::PlainText => None,
        }
    }

    /// Whether the language uses C-style `/* */` block comments.
    fn has_c_block_comment(self) -> bool {
        matches!(self, Language::Rhai | Language::Rust | Language::Wgsl)
    }

    /// Block-comment delimiters for languages that support them. Used by the
    /// Ctrl+Shift+/ wrap action.
    pub fn block_comment(self) -> Option<(&'static str, &'static str)> {
        match self {
            Language::Rhai | Language::Rust | Language::Wgsl => Some(("/*", "*/")),
            _ => None,
        }
    }
}

pub struct TokenStyle {
    pub normal: TextFormat,
    pub keyword: TextFormat,
    pub ty: TextFormat,
    pub function: TextFormat,
    pub number: TextFormat,
    pub string: TextFormat,
    pub comment: TextFormat,
}

impl TokenStyle {
    /// Build token styles from the editor's theme.
    pub fn from_theme(font: FontId, theme: &Theme) -> Self {
        // Pull accent colors from the theme where appropriate, otherwise use
        // a Gruvbox-leaning palette that reads well on dark and light.
        let text = theme.text.primary.to_color32();
        let muted = theme.text.muted.to_color32();
        let accent = theme.semantic.accent.to_color32();
        let _ = accent;

        let keyword = Color32::from_rgb(230, 100, 90);
        let ty = Color32::from_rgb(240, 190, 80);
        let function = Color32::from_rgb(160, 210, 110);
        let number = Color32::from_rgb(210, 140, 200);
        let string = Color32::from_rgb(170, 210, 130);
        let comment = muted;

        let mk = |color: Color32, italics: bool| TextFormat {
            font_id: font.clone(),
            color,
            italics,
            ..Default::default()
        };

        Self {
            normal: mk(text, false),
            keyword: mk(keyword, false),
            ty: mk(ty, false),
            function: mk(function, false),
            number: mk(number, false),
            string: mk(string, false),
            comment: mk(comment, true),
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
        Language::Rhai => &[
            "let", "const", "fn", "if", "else", "while", "for", "in", "loop", "break", "continue",
            "return", "throw", "try", "catch", "switch", "import", "export", "as", "private",
            "this", "do", "until", "true", "false",
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
        Language::Toml | Language::Json | Language::PlainText => &[],
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

/// Build a `LayoutJob` with coloured tokens for the given source text.
/// Token category. Mapped to a [`TokenStyle`] field (color) only at assembly
/// time, so the per-line cache stays valid across theme / font changes.
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

/// Cross-line tokenizer state carried into the next line. Currently only tracks
/// whether we're inside a C-style `/* ... */` block comment (the one
/// multi-line construct the tokenizer recognises; Lua `--` long comments are
/// treated as line comments, matching the previous behaviour).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineState {
    #[default]
    Normal,
    BlockComment,
}

impl TokenStyle {
    fn fmt_for(&self, kind: TokenKind) -> &TextFormat {
        match kind {
            TokenKind::Normal => &self.normal,
            TokenKind::Keyword => &self.keyword,
            TokenKind::Type => &self.ty,
            TokenKind::Function => &self.function,
            TokenKind::Number => &self.number,
            TokenKind::String => &self.string,
            TokenKind::Comment => &self.comment,
        }
    }
}

fn line_hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// Append a line's cached spans to `job` using the current theme colours.
/// `spans` must exactly cover `line` (sum of lengths == `line.len()`), which
/// [`highlight_line`] guarantees.
fn append_line(job: &mut LayoutJob, line: &str, spans: &[(u32, TokenKind)], style: &TokenStyle) {
    let mut off = 0usize;
    for &(slen, kind) in spans {
        let end = off + slen as usize;
        job.append(&line[off..end], 0.0, style.fmt_for(kind).clone());
        off = end;
    }
}

/// Tokenize a SINGLE line (no trailing `\n`) given the incoming cross-line
/// state. Returns the spans as `(byte_len, kind)` pairs that exactly cover the
/// line, plus the outgoing state for the next line. This is the canonical
/// tokenizer; both [`highlight`] and [`highlight_cached`] build on it.
pub fn highlight_line(
    line: &str,
    lang: Language,
    incoming: LineState,
) -> (Vec<(u32, TokenKind)>, LineState) {
    let mut spans: Vec<(u32, TokenKind)> = Vec::new();
    let mut push = |len: usize, kind: TokenKind| {
        if len > 0 {
            spans.push((len as u32, kind));
        }
    };

    if matches!(lang, Language::PlainText) {
        push(line.len(), TokenKind::Normal);
        return (spans, LineState::Normal);
    }

    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut i = 0usize;

    // Continuation of a block comment started on a previous line.
    if incoming == LineState::BlockComment {
        let mut j = 0usize;
        while j + 1 < len && !(bytes[j] == b'*' && bytes[j + 1] == b'/') {
            j += 1;
        }
        if j + 1 < len {
            // Found the closing `*/` on this line.
            push(j + 2, TokenKind::Comment);
            i = j + 2;
        } else {
            push(len, TokenKind::Comment);
            return (spans, LineState::BlockComment);
        }
    }

    let keywords = keywords_for(lang);
    let types = types_for(lang);
    let line_comment = lang.line_comment();
    let has_block_comment = lang.has_c_block_comment();

    while i < len {
        let b = bytes[i];

        // --- Line comment (to end of line) ---
        if let Some(prefix) = line_comment {
            let pb = prefix.as_bytes();
            if i + pb.len() <= len && &bytes[i..i + pb.len()] == pb {
                push(len - i, TokenKind::Comment);
                i = len;
                continue;
            }
        }

        // --- Block comment /* ... */ (may not close on this line) ---
        if has_block_comment && i + 1 < len && b == b'/' && bytes[i + 1] == b'*' {
            let mut j = i + 2;
            while j + 1 < len && !(bytes[j] == b'*' && bytes[j + 1] == b'/') {
                j += 1;
            }
            if j + 1 < len {
                push((j + 2) - i, TokenKind::Comment);
                i = j + 2;
                continue;
            } else {
                push(len - i, TokenKind::Comment);
                return (spans, LineState::BlockComment);
            }
        }

        // --- String "..." or '...' (single line) ---
        if b == b'"' || b == b'\'' {
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
                has_dot = true;
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
        if b.is_ascii_alphabetic() || b == b'_' {
            let mut j = i + 1;
            while j < len && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                j += 1;
            }
            let word = &line[i..j];
            let kind = if keywords.contains(&word) {
                TokenKind::Keyword
            } else if types.contains(&word) {
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

/// One-shot whole-buffer highlight with no cache. Delegates to
/// [`highlight_line`] so tokenization has a single source of truth.
pub fn highlight(text: &str, lang: Language, style: &TokenStyle) -> LayoutJob {
    let mut job = LayoutJob::default();
    if matches!(lang, Language::PlainText) {
        job.append(text, 0.0, style.normal.clone());
        return job;
    }
    let lines: Vec<&str> = text.split('\n').collect();
    let n = lines.len();
    let mut incoming = LineState::Normal;
    for (idx, line) in lines.iter().enumerate() {
        let (spans, outgoing) = highlight_line(line, lang, incoming);
        append_line(&mut job, line, &spans, style);
        incoming = outgoing;
        if idx + 1 < n {
            job.append("\n", 0.0, style.normal.clone());
        }
    }
    job
}

/// Per-line highlight cache for one file. Stores token *kinds* (not colors) so
/// theme/font changes don't invalidate it; entries are keyed by line content
/// hash + incoming state, so only changed lines (and any whose block-comment
/// state shifted) are re-tokenized.
#[derive(Default)]
struct CachedLine {
    valid: bool,
    hash: u64,
    incoming: LineState,
    outgoing: LineState,
    spans: Vec<(u32, TokenKind)>,
}

#[derive(Default)]
pub struct FileHighlightCache {
    lang: Option<Language>,
    lines: Vec<CachedLine>,
}

/// Whole-buffer highlight that reuses unchanged lines from `cache`. Produces a
/// `LayoutJob` byte-identical to [`highlight`] for the same input, so cursor
/// mapping is unaffected.
pub fn highlight_cached(
    text: &str,
    lang: Language,
    style: &TokenStyle,
    cache: &mut FileHighlightCache,
) -> LayoutJob {
    let mut job = LayoutJob::default();
    if matches!(lang, Language::PlainText) {
        job.append(text, 0.0, style.normal.clone());
        return job;
    }

    // Language switch invalidates every cached line.
    if cache.lang != Some(lang) {
        cache.lines.clear();
        cache.lang = Some(lang);
    }

    let lines: Vec<&str> = text.split('\n').collect();
    let n = lines.len();
    if cache.lines.len() != n {
        cache.lines.resize_with(n, CachedLine::default);
    }

    let mut incoming = LineState::Normal;
    for (idx, line) in lines.iter().enumerate() {
        let h = line_hash(line);
        {
            let entry = &mut cache.lines[idx];
            if !(entry.valid && entry.hash == h && entry.incoming == incoming) {
                let (spans, outgoing) = highlight_line(line, lang, incoming);
                entry.valid = true;
                entry.hash = h;
                entry.incoming = incoming;
                entry.outgoing = outgoing;
                entry.spans = spans;
            }
        }
        let entry = &cache.lines[idx];
        append_line(&mut job, line, &entry.spans, style);
        incoming = entry.outgoing;
        if idx + 1 < n {
            job.append("\n", 0.0, style.normal.clone());
        }
    }
    job
}
