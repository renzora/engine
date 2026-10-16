//! A WGSL tokenizer that keeps everything, comments and whitespace included.
//!
//! The translator only needs the shape of the code (where declarations,
//! statements, struct members and parameters begin and end), never its
//! meaning. Keeping the trivia means the output reads like the input with
//! annotations added, rather than like a pretty-printer's idea of it.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Ident,
    Number,
    Punct,
    /// Whitespace or a comment.
    Trivia,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub kind: Kind,
    pub text: String,
}

impl Token {
    pub fn is(&self, s: &str) -> bool {
        self.kind != Kind::Trivia && self.text == s
    }
}

pub fn tokenize(text: &str) -> Vec<Token> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        let c = bytes[i];
        let kind = if c.is_ascii_whitespace() {
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            Kind::Trivia
        } else if text[i..].starts_with("//") {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            Kind::Trivia
        } else if text[i..].starts_with("/*") {
            // WGSL block comments nest. An unterminated one runs to the end of
            // the chunk rather than failing: a chunk ends at a preprocessor
            // line, and a directive inside a comment is the author's business.
            // Bytes, not `str` slices: comments hold any UTF-8, and stepping one
            // byte at a time lands inside multi-byte characters.
            let mut depth = 0usize;
            while i < bytes.len() {
                if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
                    depth += 1;
                    i += 2;
                } else if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            Kind::Trivia
        } else if c.is_ascii_alphabetic() || c == b'_' {
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            Kind::Ident
        } else if c.is_ascii_digit() || (c == b'.' && bytes.get(i + 1).is_some_and(u8::is_ascii_digit)) {
            while i < bytes.len() {
                let b = bytes[i];
                let exponent_sign = (b == b'+' || b == b'-')
                    && matches!(bytes[i - 1], b'e' | b'E' | b'p' | b'P')
                    && !text[start..i].starts_with("0x");
                if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' || exponent_sign {
                    i += 1;
                } else {
                    break;
                }
            }
            Kind::Number
        } else {
            let two = text.get(i..i + 2);
            if matches!(two, Some("::") | Some("->")) {
                i += 2;
            } else {
                i += text[i..].chars().next().map_or(1, char::len_utf8);
            }
            Kind::Punct
        };
        out.push(Token { kind, text: text[start..i].to_string() });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sig(src: &str) -> Vec<String> {
        tokenize(src)
            .into_iter()
            .filter(|t| t.kind != Kind::Trivia)
            .map(|t| t.text)
            .collect()
    }

    #[test]
    fn splits_paths_attributes_and_numbers() {
        assert_eq!(
            sig("@group(0) var x = bevy_pbr::utils::PI * 1.5e-3f; // c"),
            ["@", "group", "(", "0", ")", "var", "x", "=", "bevy_pbr", "::", "utils", "::", "PI", "*", "1.5e-3f", ";"]
        );
    }

    #[test]
    fn nested_block_comments_are_one_token() {
        let toks = tokenize("a /* x /* y */ z */ b");
        assert_eq!(toks.iter().filter(|t| t.kind == Kind::Trivia).count(), 3);
        assert_eq!(sig("a /* x /* y */ z */ b"), ["a", "b"]);
    }

    #[test]
    fn non_ascii_comments() {
        assert_eq!(sig("a /* ≈ é → */ b // ✓\nc"), ["a", "b", "c"]);
    }
}
