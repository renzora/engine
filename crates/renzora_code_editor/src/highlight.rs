//! Keyword-based syntax tokenizer.
//!
//! A single-pass tokenizer with per-language keyword / type lists, producing
//! `(byte_len, TokenKind)` spans per line. Intentionally simple: no parser, no
//! tree-sitter, no syntect — those are a follow-up if/when we need richer
//! grammars. The bevy-native code editor consumes [`highlight_line`] directly.

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

/// Tokenize a SINGLE line (no trailing `\n`) given the incoming cross-line
/// state. Returns the spans as `(byte_len, kind)` pairs that exactly cover the
/// line, plus the outgoing state for the next line. This is the canonical
/// tokenizer the native editor builds on.
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
