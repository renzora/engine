//! Symbol extraction for the Outline panel — lists functions / types in a
//! source file. The bevy-native Outline panel (`native_outline`) renders these.

use crate::highlight::Language;

#[derive(Debug, Clone)]
pub struct OutlineSymbol {
    pub name: String,
    pub line: usize, // 0-based
    pub kind: SymbolKind,
}

#[derive(Debug, Clone, Copy)]
pub enum SymbolKind {
    Function,
    Class,
}

pub fn extract_symbols(content: &str, lang: Language) -> Vec<OutlineSymbol> {
    let mut out = Vec::new();
    for (line_idx, line) in content.lines().enumerate() {
        let trimmed = line.trim_start();
        match lang {
            Language::Lua => {
                if let Some(name) = parse_lua_function(trimmed) {
                    out.push(OutlineSymbol {
                        name,
                        line: line_idx,
                        kind: SymbolKind::Function,
                    });
                }
            }
            Language::Rust | Language::Rhai | Language::Wgsl => {
                if let Some(name) = parse_c_style_fn(trimmed) {
                    out.push(OutlineSymbol {
                        name,
                        line: line_idx,
                        kind: SymbolKind::Function,
                    });
                }
                if matches!(lang, Language::Rust) {
                    if let Some(name) = parse_rust_struct_or_enum(trimmed) {
                        out.push(OutlineSymbol {
                            name,
                            line: line_idx,
                            kind: SymbolKind::Class,
                        });
                    }
                }
            }
            Language::Python => {
                if let Some(name) = parse_python_def(trimmed) {
                    out.push(OutlineSymbol {
                        name,
                        line: line_idx,
                        kind: SymbolKind::Function,
                    });
                }
                if let Some(name) = parse_python_class(trimmed) {
                    out.push(OutlineSymbol {
                        name,
                        line: line_idx,
                        kind: SymbolKind::Class,
                    });
                }
            }
            _ => {}
        }
    }
    out
}

fn parse_lua_function(line: &str) -> Option<String> {
    let s = line.strip_prefix("local ").unwrap_or(line);
    let s = s.strip_prefix("function ")?;
    let end = s
        .find(|c: char| c == '(' || c.is_whitespace())
        .unwrap_or(s.len());
    let name = s[..end].trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn parse_c_style_fn(line: &str) -> Option<String> {
    let s = line.strip_prefix("pub ").unwrap_or(line);
    let s = s.strip_prefix("async ").unwrap_or(s);
    let s = s.strip_prefix("fn ")?;
    let end = s
        .find(|c: char| c == '(' || c == '<' || c.is_whitespace())
        .unwrap_or(s.len());
    let name = s[..end].trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn parse_rust_struct_or_enum(line: &str) -> Option<String> {
    let s = line.strip_prefix("pub ").unwrap_or(line);
    for prefix in ["struct ", "enum ", "trait ", "impl "] {
        if let Some(rest) = s.strip_prefix(prefix) {
            let end = rest
                .find(|c: char| c == '<' || c == '{' || c == '(' || c.is_whitespace())
                .unwrap_or(rest.len());
            let name = rest[..end].trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn parse_python_def(line: &str) -> Option<String> {
    let s = line.strip_prefix("def ")?;
    let end = s.find('(').unwrap_or(s.len());
    let name = s[..end].trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn parse_python_class(line: &str) -> Option<String> {
    let s = line.strip_prefix("class ")?;
    let end = s
        .find(|c: char| c == '(' || c == ':' || c.is_whitespace())
        .unwrap_or(s.len());
    let name = s[..end].trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}
