//! USDA lexer / tokenizer.

/// Token types produced by the USDA lexer.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// `def`, `over`, `class`
    Keyword(String),
    /// A quoted string: `"Mesh"`, `"hello world"`
    QuotedString(String),
    /// An unquoted identifier or type name: `Mesh`, `point3f[]`
    Identifier(String),
    /// A numeric literal: `1.0`, `-3`, `1e-5`
    Number(f64),
    /// `(`
    OpenParen,
    /// `)`
    CloseParen,
    /// `[`
    OpenBracket,
    /// `]`
    CloseBracket,
    /// `{`
    OpenBrace,
    /// `}`
    CloseBrace,
    /// `=`
    Equals,
    /// `,`
    Comma,
    /// `.`
    Dot,
    /// `:`
    Colon,
    /// `;`
    Semicolon,
    /// `@path@` — asset references
    AssetRef(String),
    /// `</Path/To/Prim>` — path references
    PathRef(String),
}

/// Tokenize USDA text into a flat list of tokens.
pub fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let c = chars[i];

        // Skip whitespace
        if c.is_whitespace() {
            i += 1;
            continue;
        }

        // Skip line comments
        if c == '#' {
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        // Skip block comments /* ... */
        if c == '/' && i + 1 < len && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i += 2;
            continue;
        }

        // Single-char tokens
        match c {
            '(' => {
                tokens.push(Token::OpenParen);
                i += 1;
                continue;
            }
            ')' => {
                tokens.push(Token::CloseParen);
                i += 1;
                continue;
            }
            '[' => {
                tokens.push(Token::OpenBracket);
                i += 1;
                continue;
            }
            ']' => {
                tokens.push(Token::CloseBracket);
                i += 1;
                continue;
            }
            '{' => {
                tokens.push(Token::OpenBrace);
                i += 1;
                continue;
            }
            '}' => {
                tokens.push(Token::CloseBrace);
                i += 1;
                continue;
            }
            '=' => {
                tokens.push(Token::Equals);
                i += 1;
                continue;
            }
            ',' => {
                tokens.push(Token::Comma);
                i += 1;
                continue;
            }
            ';' => {
                tokens.push(Token::Semicolon);
                i += 1;
                continue;
            }
            ':' => {
                tokens.push(Token::Colon);
                i += 1;
                continue;
            }
            '.' => {
                // Could be start of a number like .5
                if i + 1 < len && chars[i + 1].is_ascii_digit() {
                    let (num, end) = read_number(&chars, i);
                    tokens.push(Token::Number(num));
                    i = end;
                } else {
                    tokens.push(Token::Dot);
                    i += 1;
                }
                continue;
            }
            _ => {}
        }

        // Quoted string
        if c == '"' {
            let (s, end) = read_quoted_string(&chars, i);
            tokens.push(Token::QuotedString(s));
            i = end;
            continue;
        }

        // Triple-quoted string
        if c == '\'' && i + 2 < len && chars[i + 1] == '\'' && chars[i + 2] == '\'' {
            let (s, end) = read_triple_quoted(&chars, i);
            tokens.push(Token::QuotedString(s));
            i = end;
            continue;
        }

        // Asset reference @...@
        if c == '@' {
            let (s, end) = read_asset_ref(&chars, i);
            tokens.push(Token::AssetRef(s));
            i = end;
            continue;
        }

        // Path reference </...>
        if c == '<' {
            let (s, end) = read_path_ref(&chars, i);
            tokens.push(Token::PathRef(s));
            i = end;
            continue;
        }

        // Number (including negative)
        if c.is_ascii_digit()
            || (c == '-' && i + 1 < len && (chars[i + 1].is_ascii_digit() || chars[i + 1] == '.'))
        {
            let (num, end) = read_number(&chars, i);
            tokens.push(Token::Number(num));
            i = end;
            continue;
        }

        // Identifier or keyword
        if c.is_alphanumeric() || c == '_' {
            let start = i;
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == ':') {
                i += 1;
            }
            // Check for type array suffix []
            if i + 1 < len && chars[i] == '[' && chars[i + 1] == ']' {
                i += 2;
            }
            let word: String = chars[start..i].iter().collect();

            match word.as_str() {
                "def" | "over" | "class" | "None" | "true" | "false" | "prepend" | "append"
                | "delete" | "add" | "reorder" | "variantSet" | "variant" | "payload"
                | "references" | "inherits" | "specializes" | "custom" | "uniform" => {
                    tokens.push(Token::Keyword(word));
                }
                _ => {
                    tokens.push(Token::Identifier(word));
                }
            }
            continue;
        }

        // Skip unknown characters
        i += 1;
    }

    tokens
}

fn read_quoted_string(chars: &[char], start: usize) -> (String, usize) {
    let mut i = start + 1; // skip opening quote
    let mut s = String::new();
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            match chars[i + 1] {
                'n' => s.push('\n'),
                't' => s.push('\t'),
                '"' => s.push('"'),
                '\\' => s.push('\\'),
                other => {
                    s.push('\\');
                    s.push(other);
                }
            }
            i += 2;
        } else if chars[i] == '"' {
            i += 1;
            break;
        } else {
            s.push(chars[i]);
            i += 1;
        }
    }
    (s, i)
}

fn read_triple_quoted(chars: &[char], start: usize) -> (String, usize) {
    let mut i = start + 3;
    let mut s = String::new();
    while i + 2 < chars.len() {
        if chars[i] == '\'' && chars[i + 1] == '\'' && chars[i + 2] == '\'' {
            i += 3;
            return (s, i);
        }
        s.push(chars[i]);
        i += 1;
    }
    (s, chars.len())
}

fn read_asset_ref(chars: &[char], start: usize) -> (String, usize) {
    // @@ for empty, or @path@ for asset reference
    let mut i = start + 1;
    if i < chars.len() && chars[i] == '@' {
        // Check for @@@ (double-@ delimited)
        i += 1;
        let mut s = String::new();
        while i + 1 < chars.len() {
            if chars[i] == '@' && chars[i + 1] == '@' {
                i += 2;
                return (s, i);
            }
            s.push(chars[i]);
            i += 1;
        }
        return (s, chars.len());
    }
    let mut s = String::new();
    while i < chars.len() && chars[i] != '@' {
        s.push(chars[i]);
        i += 1;
    }
    if i < chars.len() {
        i += 1; // skip closing @
    }
    (s, i)
}

fn read_path_ref(chars: &[char], start: usize) -> (String, usize) {
    let mut i = start + 1; // skip <
    let mut s = String::new();
    while i < chars.len() && chars[i] != '>' {
        s.push(chars[i]);
        i += 1;
    }
    if i < chars.len() {
        i += 1; // skip >
    }
    (s, i)
}

fn read_number(chars: &[char], start: usize) -> (f64, usize) {
    let mut i = start;
    let mut s = String::new();

    // Optional negative sign
    if i < chars.len() && chars[i] == '-' {
        s.push('-');
        i += 1;
    }

    // Integer part
    while i < chars.len() && chars[i].is_ascii_digit() {
        s.push(chars[i]);
        i += 1;
    }

    // Decimal part
    if i < chars.len() && chars[i] == '.' {
        s.push('.');
        i += 1;
        while i < chars.len() && chars[i].is_ascii_digit() {
            s.push(chars[i]);
            i += 1;
        }
    }

    // Exponent
    if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
        s.push('e');
        i += 1;
        if i < chars.len() && (chars[i] == '+' || chars[i] == '-') {
            s.push(chars[i]);
            i += 1;
        }
        while i < chars.len() && chars[i].is_ascii_digit() {
            s.push(chars[i]);
            i += 1;
        }
    }

    let num = s.parse::<f64>().unwrap_or(0.0);
    (num, i)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_keywords_vs_identifiers() {
        let toks = tokenize("def Mesh \"Cube\"");
        assert_eq!(
            toks,
            vec![
                Token::Keyword("def".into()),
                Token::Identifier("Mesh".into()),
                Token::QuotedString("Cube".into()),
            ]
        );
    }

    #[test]
    fn tokenize_punctuation() {
        let toks = tokenize("( ) [ ] { } = , ; :");
        assert_eq!(
            toks,
            vec![
                Token::OpenParen,
                Token::CloseParen,
                Token::OpenBracket,
                Token::CloseBracket,
                Token::OpenBrace,
                Token::CloseBrace,
                Token::Equals,
                Token::Comma,
                Token::Semicolon,
                Token::Colon,
            ]
        );
    }

    #[test]
    fn tokenize_numbers_including_negative_decimal_exponent() {
        let toks = tokenize("1 -3 1.5 .5 1e-5 -2.0");
        assert_eq!(
            toks,
            vec![
                Token::Number(1.0),
                Token::Number(-3.0),
                Token::Number(1.5),
                Token::Number(0.5),
                Token::Number(1e-5),
                Token::Number(-2.0),
            ]
        );
    }

    #[test]
    fn tokenize_skips_line_and_block_comments() {
        let toks = tokenize("# a line comment\n1 /* block */ 2");
        assert_eq!(toks, vec![Token::Number(1.0), Token::Number(2.0)]);
    }

    #[test]
    fn tokenize_quoted_string_with_escapes() {
        let toks = tokenize(r#""a\nb\"c""#);
        assert_eq!(toks, vec![Token::QuotedString("a\nb\"c".into())]);
    }

    #[test]
    fn tokenize_asset_and_path_refs() {
        let toks = tokenize("@./tex.png@ </Root/Mesh>");
        assert_eq!(
            toks,
            vec![
                Token::AssetRef("./tex.png".into()),
                Token::PathRef("/Root/Mesh".into()),
            ]
        );
    }

    #[test]
    fn tokenize_type_array_suffix_kept_on_identifier() {
        // `point3f[]` should tokenize as a single identifier including `[]`.
        let toks = tokenize("point3f[] points");
        assert_eq!(
            toks,
            vec![
                Token::Identifier("point3f[]".into()),
                Token::Identifier("points".into()),
            ]
        );
    }

    #[test]
    fn tokenize_namespaced_identifier_keeps_colons() {
        let toks = tokenize("primvars:st");
        assert_eq!(toks, vec![Token::Identifier("primvars:st".into())]);
    }

    #[test]
    fn tokenize_empty_input() {
        assert!(tokenize("").is_empty());
        assert!(tokenize("   \n\t # comment\n").is_empty());
    }
}
