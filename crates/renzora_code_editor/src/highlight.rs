//! Keyword-based syntax highlighter.
//!
//! Produces an egui `LayoutJob` with colored tokens for the supported
//! languages. Intentionally simple: a single-pass tokenizer with per-language
//! keyword / type lists. No parser, no tree-sitter, no syntect — those are a
//! follow-up if/when we need richer grammars.

use bevy_egui::egui::{
    text::LayoutJob, Color32, FontId, TextFormat,
};
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
            "and", "break", "do", "else", "elseif", "end", "false", "for",
            "function", "goto", "if", "in", "local", "nil", "not", "or",
            "repeat", "return", "then", "true", "until", "while", "self",
        ],
        Language::Rhai => &[
            "let", "const", "fn", "if", "else", "while", "for", "in", "loop",
            "break", "continue", "return", "throw", "try", "catch", "switch",
            "import", "export", "as", "private", "this", "do", "until", "true",
            "false",
        ],
        Language::Rust => &[
            "as", "break", "const", "continue", "crate", "else", "enum",
            "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop",
            "match", "mod", "move", "mut", "pub", "ref", "return", "self",
            "Self", "static", "struct", "super", "trait", "true", "type",
            "unsafe", "use", "where", "while", "async", "await", "dyn", "box",
            "try",
        ],
        Language::Wgsl => &[
            "fn", "let", "var", "const", "struct", "if", "else", "for",
            "while", "loop", "return", "discard", "switch", "case", "default",
            "break", "continue", "enable", "override", "alias", "true", "false",
        ],
        Language::Python => &[
            "def", "class", "if", "elif", "else", "for", "while", "return",
            "True", "False", "None", "import", "from", "as", "try", "except",
            "finally", "with", "yield", "lambda", "pass", "raise", "global",
            "nonlocal", "and", "or", "not", "in", "is", "break", "continue",
            "async", "await",
        ],
        Language::Shell => &[
            "if", "then", "else", "elif", "fi", "for", "while", "do", "done",
            "case", "esac", "function", "return", "local", "export",
        ],
        Language::Sql => &[
            "SELECT", "FROM", "WHERE", "INSERT", "INTO", "UPDATE", "DELETE",
            "CREATE", "TABLE", "DROP", "JOIN", "LEFT", "RIGHT", "INNER", "ON",
            "AS", "ORDER", "BY", "GROUP", "HAVING", "LIMIT", "OFFSET", "IN",
            "NOT", "AND", "OR", "NULL", "IS", "LIKE", "VALUES",
        ],
        Language::Toml | Language::Json | Language::PlainText => &[],
    }
}

fn types_for(lang: Language) -> &'static [&'static str] {
    match lang {
        Language::Rust => &[
            "bool", "char", "u8", "u16", "u32", "u64", "u128", "usize",
            "i8", "i16", "i32", "i64", "i128", "isize", "f32", "f64",
            "str", "String", "Vec", "Option", "Result", "Box", "Rc", "Arc",
            "HashMap", "HashSet", "BTreeMap", "BTreeSet",
        ],
        Language::Wgsl => &[
            "bool", "i32", "u32", "f32", "f16",
            "vec2", "vec3", "vec4",
            "vec2i", "vec3i", "vec4i", "vec2u", "vec3u", "vec4u",
            "vec2f", "vec3f", "vec4f",
            "mat2x2", "mat2x3", "mat2x4", "mat3x2", "mat3x3", "mat3x4",
            "mat4x2", "mat4x3", "mat4x4",
            "texture_2d", "texture_3d", "texture_cube",
            "sampler", "sampler_comparison", "array", "atomic", "ptr",
        ],
        Language::Python => &["int", "float", "str", "bool", "list", "dict", "tuple", "set"],
        _ => &[],
    }
}

/// Build a `LayoutJob` with coloured tokens for the given source text.
pub fn highlight(text: &str, lang: Language, style: &TokenStyle) -> LayoutJob {
    let mut job = LayoutJob::default();

    if matches!(lang, Language::PlainText) {
        job.append(text, 0.0, style.normal.clone());
        return job;
    }

    // Strings in TOML/JSON: treat everything but strings/numbers/keywords as normal.
    let keywords = keywords_for(lang);
    let types = types_for(lang);
    let line_comment = lang.line_comment();
    let has_block_comment = lang.has_c_block_comment();

    let bytes = text.as_bytes();
    let mut i = 0;
    let len = bytes.len();

    while i < len {
        let b = bytes[i];

        // --- Line comment ---
        if let Some(prefix) = line_comment {
            let pb = prefix.as_bytes();
            if i + pb.len() <= len && &bytes[i..i + pb.len()] == pb {
                let mut j = i;
                while j < len && bytes[j] != b'\n' {
                    j += 1;
                }
                job.append(&text[i..j], 0.0, style.comment.clone());
                i = j;
                continue;
            }
        }

        // --- Block comment /* ... */ ---
        if has_block_comment && i + 1 < len && b == b'/' && bytes[i + 1] == b'*' {
            let mut j = i + 2;
            while j + 1 < len && !(bytes[j] == b'*' && bytes[j + 1] == b'/') {
                j += 1;
            }
            let end = (j + 2).min(len);
            job.append(&text[i..end], 0.0, style.comment.clone());
            i = end;
            continue;
        }

        // --- Lua long comment --[[ ... ]] ---
        if matches!(lang, Language::Lua)
            && i + 3 < len
            && &bytes[i..i + 4] == b"--[["
        {
            let mut j = i + 4;
            while j + 1 < len && !(bytes[j] == b']' && bytes[j + 1] == b']') {
                j += 1;
            }
            let end = (j + 2).min(len);
            job.append(&text[i..end], 0.0, style.comment.clone());
            i = end;
            continue;
        }

        // --- String "..." or '...' ---
        if b == b'"' || b == b'\'' {
            let quote = b;
            let mut j = i + 1;
            while j < len && bytes[j] != quote {
                if bytes[j] == b'\\' && j + 1 < len {
                    j += 2;
                    continue;
                }
                if bytes[j] == b'\n' {
                    break;
                }
                j += 1;
            }
            let end = (j + 1).min(len);
            job.append(&text[i..end], 0.0, style.string.clone());
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
            job.append(&text[i..j], 0.0, style.number.clone());
            i = j;
            continue;
        }

        // --- Identifier / keyword / type / function name ---
        if b.is_ascii_alphabetic() || b == b'_' {
            let mut j = i + 1;
            while j < len && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                j += 1;
            }
            let word = &text[i..j];
            let fmt = if keywords.iter().any(|k| *k == word) {
                &style.keyword
            } else if types.iter().any(|t| *t == word) {
                &style.ty
            } else if j < len && bytes[j] == b'(' {
                &style.function
            } else {
                &style.normal
            };
            job.append(word, 0.0, fmt.clone());
            i = j;
            continue;
        }

        // --- Anything else: one full char (UTF-8 safe) ---
        let ch_len = text[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
        let end = (i + ch_len).min(len);
        job.append(&text[i..end], 0.0, style.normal.clone());
        i = end;
    }

    job
}
