use bevy_egui::egui::{Color32, FontId, FontFamily, text::LayoutJob, TextFormat};

#[derive(Clone, Copy, PartialEq)]
enum TokenType {
    Keyword,
    String,
    Number,
    Comment,
    Operator,
    Function,
    Boolean,
    Normal,
}

impl TokenType {
    fn color(self) -> Color32 {
        match self {
            TokenType::Keyword => Color32::from_rgb(198, 120, 221),    // Purple
            TokenType::String => Color32::from_rgb(152, 195, 121),     // Green
            TokenType::Number => Color32::from_rgb(209, 154, 102),     // Orange
            TokenType::Comment => Color32::from_rgb(92, 99, 112),      // Gray
            TokenType::Operator => Color32::from_rgb(86, 182, 194),    // Cyan
            TokenType::Function => Color32::from_rgb(97, 175, 239),    // Blue
            TokenType::Boolean => Color32::from_rgb(209, 154, 102),    // Orange
            TokenType::Normal => Color32::from_rgb(171, 178, 191),     // Light gray
        }
    }
}

const KEYWORDS: &[&str] = &[
    "let", "const", "fn", "if", "else", "while", "for", "in", "loop",
    "break", "continue", "return", "throw", "try", "catch", "switch",
    "import", "export", "as", "private", "this", "is", "type_of",
    "print", "debug", "do", "until",
];

const BOOLEANS: &[&str] = &["true", "false", "null"];

pub fn highlight_rhai(code: &str, font_size: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let font_id = FontId::new(font_size, FontFamily::Monospace);

    let chars: Vec<char> = code.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let start = i;
        let ch = chars[i];

        // Line comment
        if ch == '/' && i + 1 < len && chars[i + 1] == '/' {
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            append_token(&mut job, &chars[start..i], TokenType::Comment, &font_id);
            continue;
        }

        // Block comment
        if ch == '/' && i + 1 < len && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            if i + 1 < len {
                i += 2;
            }
            append_token(&mut job, &chars[start..i], TokenType::Comment, &font_id);
            continue;
        }

        // String (double quotes)
        if ch == '"' {
            i += 1;
            while i < len && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < len {
                    i += 1;
                }
                i += 1;
            }
            if i < len {
                i += 1;
            }
            append_token(&mut job, &chars[start..i], TokenType::String, &font_id);
            continue;
        }

        // String (single quotes / char)
        if ch == '\'' {
            i += 1;
            while i < len && chars[i] != '\'' {
                if chars[i] == '\\' && i + 1 < len {
                    i += 1;
                }
                i += 1;
            }
            if i < len {
                i += 1;
            }
            append_token(&mut job, &chars[start..i], TokenType::String, &font_id);
            continue;
        }

        // Backtick strings (template strings in Rhai)
        if ch == '`' {
            i += 1;
            while i < len && chars[i] != '`' {
                if chars[i] == '\\' && i + 1 < len {
                    i += 1;
                }
                i += 1;
            }
            if i < len {
                i += 1;
            }
            append_token(&mut job, &chars[start..i], TokenType::String, &font_id);
            continue;
        }

        // Number
        if ch.is_ascii_digit() || (ch == '.' && i + 1 < len && chars[i + 1].is_ascii_digit()) {
            // Handle hex numbers
            if ch == '0' && i + 1 < len && (chars[i + 1] == 'x' || chars[i + 1] == 'X') {
                i += 2;
                while i < len && (chars[i].is_ascii_hexdigit() || chars[i] == '_') {
                    i += 1;
                }
            } else {
                // Regular number (int or float)
                while i < len && (chars[i].is_ascii_digit() || chars[i] == '_') {
                    i += 1;
                }
                // Decimal part
                if i < len && chars[i] == '.' && i + 1 < len && chars[i + 1].is_ascii_digit() {
                    i += 1;
                    while i < len && (chars[i].is_ascii_digit() || chars[i] == '_') {
                        i += 1;
                    }
                }
                // Exponent
                if i < len && (chars[i] == 'e' || chars[i] == 'E') {
                    i += 1;
                    if i < len && (chars[i] == '+' || chars[i] == '-') {
                        i += 1;
                    }
                    while i < len && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                }
            }
            append_token(&mut job, &chars[start..i], TokenType::Number, &font_id);
            continue;
        }

        // Identifier or keyword
        if ch.is_alphabetic() || ch == '_' {
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();

            // Check if followed by '(' for function detection
            let mut peek = i;
            while peek < len && chars[peek] == ' ' {
                peek += 1;
            }
            let is_function = peek < len && chars[peek] == '(';

            let token_type = if KEYWORDS.contains(&word.as_str()) {
                TokenType::Keyword
            } else if BOOLEANS.contains(&word.as_str()) {
                TokenType::Boolean
            } else if is_function {
                TokenType::Function
            } else {
                TokenType::Normal
            };

            append_token(&mut job, &chars[start..i], token_type, &font_id);
            continue;
        }

        // Operators
        if is_operator(ch) {
            while i < len && is_operator(chars[i]) && !is_comment_start(&chars, i) {
                i += 1;
            }
            append_token(&mut job, &chars[start..i], TokenType::Operator, &font_id);
            continue;
        }

        // Whitespace and other characters
        i += 1;
        append_token(&mut job, &chars[start..i], TokenType::Normal, &font_id);
    }

    job
}

fn is_operator(ch: char) -> bool {
    matches!(ch, '+' | '-' | '*' | '/' | '%' | '=' | '!' | '<' | '>' | '&' | '|' | '^' | '~' | '?' | ':')
}

fn is_comment_start(chars: &[char], i: usize) -> bool {
    chars[i] == '/' && i + 1 < chars.len() && (chars[i + 1] == '/' || chars[i + 1] == '*')
}

fn append_token(job: &mut LayoutJob, chars: &[char], token_type: TokenType, font_id: &FontId) {
    let text: String = chars.iter().collect();
    job.append(
        &text,
        0.0,
        TextFormat {
            font_id: font_id.clone(),
            color: token_type.color(),
            ..Default::default()
        },
    );
}
