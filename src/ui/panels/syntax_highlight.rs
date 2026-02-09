#![allow(dead_code)]

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

// =============================================================================
// Rust Syntax Highlighting
// =============================================================================

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn",
    "else", "enum", "extern", "false", "fn", "for", "if", "impl", "in",
    "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while", "async", "await", "try",
];

const RUST_TYPES: &[&str] = &[
    "bool", "char", "str", "u8", "u16", "u32", "u64", "u128", "usize",
    "i8", "i16", "i32", "i64", "i128", "isize", "f32", "f64",
    "String", "Vec", "Option", "Result", "Box", "Rc", "Arc", "Cell",
    "RefCell", "HashMap", "HashSet", "BTreeMap", "BTreeSet", "Path", "PathBuf",
];

const RUST_BUILTINS: &[&str] = &[
    "Some", "None", "Ok", "Err", "Default", "Clone", "Copy", "Send", "Sync",
    "Sized", "Drop", "Fn", "FnMut", "FnOnce", "From", "Into", "AsRef", "AsMut",
    "Iterator", "IntoIterator", "Debug", "Display", "PartialEq", "Eq",
    "PartialOrd", "Ord", "Hash", "Default",
];

#[derive(Clone, Copy, PartialEq)]
enum RustTokenType {
    Keyword,
    Type,
    Builtin,
    String,
    Char,
    Number,
    Comment,
    Attribute,
    Macro,
    Lifetime,
    Operator,
    Function,
    Normal,
}

impl RustTokenType {
    fn color(self) -> Color32 {
        match self {
            RustTokenType::Keyword => Color32::from_rgb(198, 120, 221),   // Purple
            RustTokenType::Type => Color32::from_rgb(229, 192, 123),      // Yellow
            RustTokenType::Builtin => Color32::from_rgb(229, 192, 123),   // Yellow
            RustTokenType::String => Color32::from_rgb(152, 195, 121),    // Green
            RustTokenType::Char => Color32::from_rgb(152, 195, 121),      // Green
            RustTokenType::Number => Color32::from_rgb(209, 154, 102),    // Orange
            RustTokenType::Comment => Color32::from_rgb(92, 99, 112),     // Gray
            RustTokenType::Attribute => Color32::from_rgb(86, 182, 194),  // Cyan
            RustTokenType::Macro => Color32::from_rgb(97, 175, 239),      // Blue
            RustTokenType::Lifetime => Color32::from_rgb(209, 154, 102),  // Orange
            RustTokenType::Operator => Color32::from_rgb(86, 182, 194),   // Cyan
            RustTokenType::Function => Color32::from_rgb(97, 175, 239),   // Blue
            RustTokenType::Normal => Color32::from_rgb(171, 178, 191),    // Light gray
        }
    }
}

pub fn highlight_rust(code: &str, font_size: f32) -> LayoutJob {
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
            append_rust_token(&mut job, &chars[start..i], RustTokenType::Comment, &font_id);
            continue;
        }

        // Block comment
        if ch == '/' && i + 1 < len && chars[i + 1] == '*' {
            i += 2;
            let mut depth = 1;
            while i + 1 < len && depth > 0 {
                if chars[i] == '/' && chars[i + 1] == '*' {
                    depth += 1;
                    i += 1;
                } else if chars[i] == '*' && chars[i + 1] == '/' {
                    depth -= 1;
                    i += 1;
                }
                i += 1;
            }
            if i < len {
                i += 1;
            }
            append_rust_token(&mut job, &chars[start..i], RustTokenType::Comment, &font_id);
            continue;
        }

        // Attribute #[...] or #![...]
        if ch == '#' && i + 1 < len && (chars[i + 1] == '[' || (chars[i + 1] == '!' && i + 2 < len && chars[i + 2] == '[')) {
            i += 1;
            if i < len && chars[i] == '!' {
                i += 1;
            }
            if i < len && chars[i] == '[' {
                let mut depth = 1;
                i += 1;
                while i < len && depth > 0 {
                    if chars[i] == '[' {
                        depth += 1;
                    } else if chars[i] == ']' {
                        depth -= 1;
                    }
                    i += 1;
                }
            }
            append_rust_token(&mut job, &chars[start..i], RustTokenType::Attribute, &font_id);
            continue;
        }

        // String
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
            append_rust_token(&mut job, &chars[start..i], RustTokenType::String, &font_id);
            continue;
        }

        // Raw string r"..." or r#"..."#
        if ch == 'r' && i + 1 < len && (chars[i + 1] == '"' || chars[i + 1] == '#') {
            i += 1;
            let mut hashes = 0;
            while i < len && chars[i] == '#' {
                hashes += 1;
                i += 1;
            }
            if i < len && chars[i] == '"' {
                i += 1;
                loop {
                    if i >= len {
                        break;
                    }
                    if chars[i] == '"' {
                        let mut end_hashes = 0;
                        let quote_pos = i;
                        i += 1;
                        while i < len && chars[i] == '#' && end_hashes < hashes {
                            end_hashes += 1;
                            i += 1;
                        }
                        if end_hashes == hashes {
                            break;
                        }
                        i = quote_pos + 1;
                    } else {
                        i += 1;
                    }
                }
            }
            append_rust_token(&mut job, &chars[start..i], RustTokenType::String, &font_id);
            continue;
        }

        // Char literal
        if ch == '\'' && i + 1 < len {
            // Check if it's a lifetime or a char
            let peek = i + 1;
            if chars[peek].is_alphabetic() || chars[peek] == '_' {
                // Could be lifetime 'a or char 'x'
                i += 1;
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                if i < len && chars[i] == '\'' {
                    // It's a char like 'a'
                    i += 1;
                    append_rust_token(&mut job, &chars[start..i], RustTokenType::Char, &font_id);
                } else {
                    // It's a lifetime
                    append_rust_token(&mut job, &chars[start..i], RustTokenType::Lifetime, &font_id);
                }
                continue;
            } else {
                // Regular char literal
                i += 1;
                if i < len && chars[i] == '\\' && i + 1 < len {
                    i += 2;
                } else if i < len {
                    i += 1;
                }
                if i < len && chars[i] == '\'' {
                    i += 1;
                }
                append_rust_token(&mut job, &chars[start..i], RustTokenType::Char, &font_id);
                continue;
            }
        }

        // Number
        if ch.is_ascii_digit() {
            // Handle different number formats
            if ch == '0' && i + 1 < len {
                match chars[i + 1] {
                    'x' | 'X' => {
                        i += 2;
                        while i < len && (chars[i].is_ascii_hexdigit() || chars[i] == '_') {
                            i += 1;
                        }
                    }
                    'o' | 'O' => {
                        i += 2;
                        while i < len && (chars[i].is_digit(8) || chars[i] == '_') {
                            i += 1;
                        }
                    }
                    'b' | 'B' => {
                        i += 2;
                        while i < len && (chars[i] == '0' || chars[i] == '1' || chars[i] == '_') {
                            i += 1;
                        }
                    }
                    _ => {
                        parse_decimal_number(&chars, &mut i, len);
                    }
                }
            } else {
                parse_decimal_number(&chars, &mut i, len);
            }
            // Type suffix
            let suffixes = ["u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize", "f32", "f64"];
            for suffix in suffixes {
                if i + suffix.len() <= len {
                    let potential: String = chars[i..i + suffix.len()].iter().collect();
                    if potential == suffix {
                        i += suffix.len();
                        break;
                    }
                }
            }
            append_rust_token(&mut job, &chars[start..i], RustTokenType::Number, &font_id);
            continue;
        }

        // Identifier, keyword, type, or macro
        if ch.is_alphabetic() || ch == '_' {
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();

            // Check if it's a macro (followed by !)
            if i < len && chars[i] == '!' {
                i += 1;
                append_rust_token(&mut job, &chars[start..i], RustTokenType::Macro, &font_id);
                continue;
            }

            // Check if followed by '(' for function detection
            let mut peek = i;
            while peek < len && chars[peek] == ' ' {
                peek += 1;
            }
            let is_function = peek < len && chars[peek] == '(';

            // Check for :: which indicates a type/module path
            let is_type_context = peek < len && peek + 1 < len && chars[peek] == ':' && chars[peek + 1] == ':';

            let token_type = if RUST_KEYWORDS.contains(&word.as_str()) {
                RustTokenType::Keyword
            } else if RUST_TYPES.contains(&word.as_str()) {
                RustTokenType::Type
            } else if RUST_BUILTINS.contains(&word.as_str()) {
                RustTokenType::Builtin
            } else if is_function {
                RustTokenType::Function
            } else if is_type_context || word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                RustTokenType::Type
            } else {
                RustTokenType::Normal
            };

            append_rust_token(&mut job, &chars[start..i], token_type, &font_id);
            continue;
        }

        // Operators
        if is_rust_operator(ch) {
            while i < len && is_rust_operator(chars[i]) && !is_comment_start(&chars, i) {
                i += 1;
            }
            append_rust_token(&mut job, &chars[start..i], RustTokenType::Operator, &font_id);
            continue;
        }

        // Whitespace and other characters
        i += 1;
        append_rust_token(&mut job, &chars[start..i], RustTokenType::Normal, &font_id);
    }

    job
}

fn parse_decimal_number(chars: &[char], i: &mut usize, len: usize) {
    while *i < len && (chars[*i].is_ascii_digit() || chars[*i] == '_') {
        *i += 1;
    }
    // Decimal part
    if *i < len && chars[*i] == '.' && *i + 1 < len && chars[*i + 1].is_ascii_digit() {
        *i += 1;
        while *i < len && (chars[*i].is_ascii_digit() || chars[*i] == '_') {
            *i += 1;
        }
    }
    // Exponent
    if *i < len && (chars[*i] == 'e' || chars[*i] == 'E') {
        *i += 1;
        if *i < len && (chars[*i] == '+' || chars[*i] == '-') {
            *i += 1;
        }
        while *i < len && (chars[*i].is_ascii_digit() || chars[*i] == '_') {
            *i += 1;
        }
    }
}

fn is_rust_operator(ch: char) -> bool {
    matches!(ch, '+' | '-' | '*' | '/' | '%' | '=' | '!' | '<' | '>' | '&' | '|' | '^' | '~' | '?' | ':' | ';' | ',' | '.' | '@')
}

fn append_rust_token(job: &mut LayoutJob, chars: &[char], token_type: RustTokenType, font_id: &FontId) {
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

// =============================================================================
// WGSL Syntax Highlighting
// =============================================================================

const WGSL_KEYWORDS: &[&str] = &[
    "fn", "let", "var", "const", "struct", "if", "else", "for", "while", "loop",
    "return", "discard", "switch", "case", "default", "break", "continue",
    "enable", "override", "alias", "diagnostic", "const_assert",
    "continuing", "fallthrough",
];

const WGSL_TYPES: &[&str] = &[
    "bool", "i32", "u32", "f32", "f16",
    "vec2", "vec3", "vec4",
    "vec2i", "vec3i", "vec4i",
    "vec2u", "vec3u", "vec4u",
    "vec2f", "vec3f", "vec4f",
    "vec2h", "vec3h", "vec4h",
    "mat2x2", "mat2x3", "mat2x4",
    "mat3x2", "mat3x3", "mat3x4",
    "mat4x2", "mat4x3", "mat4x4",
    "mat2x2f", "mat2x3f", "mat2x4f",
    "mat3x2f", "mat3x3f", "mat3x4f",
    "mat4x2f", "mat4x3f", "mat4x4f",
    "texture_1d", "texture_2d", "texture_2d_array", "texture_3d",
    "texture_cube", "texture_cube_array", "texture_multisampled_2d",
    "texture_storage_1d", "texture_storage_2d", "texture_storage_2d_array", "texture_storage_3d",
    "texture_depth_2d", "texture_depth_2d_array", "texture_depth_cube", "texture_depth_cube_array",
    "texture_depth_multisampled_2d", "texture_external",
    "sampler", "sampler_comparison",
    "array", "atomic", "ptr",
];

const WGSL_BUILTINS: &[&str] = &[
    "textureSample", "textureSampleLevel", "textureSampleBias", "textureSampleGrad",
    "textureSampleCompare", "textureSampleCompareLevel",
    "textureLoad", "textureStore", "textureDimensions", "textureNumLayers",
    "textureNumLevels", "textureNumSamples", "textureGather", "textureGatherCompare",
    "dot", "cross", "normalize", "length", "distance",
    "mix", "clamp", "smoothstep", "step", "fma",
    "abs", "acos", "asin", "atan", "atan2", "ceil", "floor", "round", "trunc", "fract",
    "cos", "sin", "tan", "cosh", "sinh", "tanh",
    "exp", "exp2", "log", "log2", "pow", "sqrt", "inverseSqrt",
    "min", "max", "sign", "saturate",
    "select", "pack4x8snorm", "pack4x8unorm", "pack2x16snorm", "pack2x16unorm", "pack2x16float",
    "unpack4x8snorm", "unpack4x8unorm", "unpack2x16snorm", "unpack2x16unorm", "unpack2x16float",
    "determinant", "transpose", "faceForward", "reflect", "refract",
    "dpdx", "dpdy", "fwidth", "dpdxCoarse", "dpdyCoarse", "fwidthCoarse",
    "dpdxFine", "dpdyFine", "fwidthFine",
    "storageBarrier", "workgroupBarrier", "workgroupUniformLoad",
    "atomicLoad", "atomicStore", "atomicAdd", "atomicSub", "atomicMax", "atomicMin",
    "atomicAnd", "atomicOr", "atomicXor", "atomicExchange", "atomicCompareExchangeWeak",
    "arrayLength", "countOneBits", "reverseBits",
    "all", "any", "countLeadingZeros", "countTrailingZeros",
    "extractBits", "insertBits", "firstLeadingBit", "firstTrailingBit",
];

#[derive(Clone, Copy, PartialEq)]
enum WgslTokenType {
    Keyword,
    Type,
    Builtin,
    String,
    Number,
    Comment,
    Attribute,
    Operator,
    Function,
    Normal,
}

impl WgslTokenType {
    fn color(self) -> Color32 {
        match self {
            WgslTokenType::Keyword => Color32::from_rgb(198, 120, 221),   // Purple
            WgslTokenType::Type => Color32::from_rgb(229, 192, 123),      // Yellow
            WgslTokenType::Builtin => Color32::from_rgb(97, 175, 239),    // Blue
            WgslTokenType::String => Color32::from_rgb(152, 195, 121),    // Green
            WgslTokenType::Number => Color32::from_rgb(209, 154, 102),    // Orange
            WgslTokenType::Comment => Color32::from_rgb(92, 99, 112),     // Gray
            WgslTokenType::Attribute => Color32::from_rgb(86, 182, 194),  // Cyan
            WgslTokenType::Operator => Color32::from_rgb(86, 182, 194),   // Cyan
            WgslTokenType::Function => Color32::from_rgb(97, 175, 239),   // Blue
            WgslTokenType::Normal => Color32::from_rgb(171, 178, 191),    // Light gray
        }
    }
}

pub fn highlight_wgsl(code: &str, font_size: f32) -> LayoutJob {
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
            append_wgsl_token(&mut job, &chars[start..i], WgslTokenType::Comment, &font_id);
            continue;
        }

        // Block comment (WGSL supports nested /* */)
        if ch == '/' && i + 1 < len && chars[i + 1] == '*' {
            i += 2;
            let mut depth = 1;
            while i + 1 < len && depth > 0 {
                if chars[i] == '/' && chars[i + 1] == '*' {
                    depth += 1;
                    i += 1;
                } else if chars[i] == '*' && chars[i + 1] == '/' {
                    depth -= 1;
                    i += 1;
                }
                i += 1;
            }
            if i < len {
                i += 1;
            }
            append_wgsl_token(&mut job, &chars[start..i], WgslTokenType::Comment, &font_id);
            continue;
        }

        // Attribute: @vertex, @fragment, @binding(n), @group(n), @location(n), etc.
        if ch == '@' {
            i += 1;
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            // Include parenthesized arguments like @binding(0)
            if i < len && chars[i] == '(' {
                i += 1;
                let mut depth = 1;
                while i < len && depth > 0 {
                    if chars[i] == '(' { depth += 1; }
                    if chars[i] == ')' { depth -= 1; }
                    i += 1;
                }
            }
            append_wgsl_token(&mut job, &chars[start..i], WgslTokenType::Attribute, &font_id);
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
            append_wgsl_token(&mut job, &chars[start..i], WgslTokenType::String, &font_id);
            continue;
        }

        // Number
        if ch.is_ascii_digit() || (ch == '.' && i + 1 < len && chars[i + 1].is_ascii_digit()) {
            // Hex
            if ch == '0' && i + 1 < len && (chars[i + 1] == 'x' || chars[i + 1] == 'X') {
                i += 2;
                while i < len && (chars[i].is_ascii_hexdigit() || chars[i] == '_') {
                    i += 1;
                }
            } else {
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
            // Type suffix: i, u, f, h
            if i < len && matches!(chars[i], 'i' | 'u' | 'f' | 'h') {
                i += 1;
            }
            append_wgsl_token(&mut job, &chars[start..i], WgslTokenType::Number, &font_id);
            continue;
        }

        // Identifier, keyword, type, or builtin
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
            let is_function_call = peek < len && chars[peek] == '(';

            // Check for generic type usage like vec2<f32>
            let is_generic_type = peek < len && chars[peek] == '<';

            let token_type = if WGSL_KEYWORDS.contains(&word.as_str()) {
                WgslTokenType::Keyword
            } else if WGSL_TYPES.contains(&word.as_str()) {
                WgslTokenType::Type
            } else if WGSL_BUILTINS.contains(&word.as_str()) {
                WgslTokenType::Builtin
            } else if word == "true" || word == "false" {
                WgslTokenType::Number
            } else if is_generic_type {
                WgslTokenType::Type
            } else if is_function_call {
                WgslTokenType::Function
            } else {
                WgslTokenType::Normal
            };

            append_wgsl_token(&mut job, &chars[start..i], token_type, &font_id);
            continue;
        }

        // Operators
        if is_wgsl_operator(ch) {
            while i < len && is_wgsl_operator(chars[i]) && !is_comment_start(&chars, i) {
                i += 1;
            }
            append_wgsl_token(&mut job, &chars[start..i], WgslTokenType::Operator, &font_id);
            continue;
        }

        // Whitespace and other characters
        i += 1;
        append_wgsl_token(&mut job, &chars[start..i], WgslTokenType::Normal, &font_id);
    }

    job
}

fn is_wgsl_operator(ch: char) -> bool {
    matches!(ch, '+' | '-' | '*' | '/' | '%' | '=' | '!' | '<' | '>' | '&' | '|' | '^' | '~' | ':' | ';' | ',' | '.')
}

fn append_wgsl_token(job: &mut LayoutJob, chars: &[char], token_type: WgslTokenType, font_id: &FontId) {
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
