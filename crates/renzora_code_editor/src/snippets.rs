//! Per-language code snippets surfaced through the autocomplete popup.
//!
//! Snippet bodies use `$0` to mark where the caret should land after
//! insertion. If no `$0` is present, the caret goes to the end of the
//! inserted text.

use crate::highlight::Language;

#[derive(Debug, Clone, Copy)]
pub struct Snippet {
    pub prefix: &'static str,
    pub label: &'static str,
    pub body: &'static str,
    pub langs: &'static [Language],
}

const LUA: &[Language] = &[Language::Lua];
const RHAI: &[Language] = &[Language::Rhai];
const RUST: &[Language] = &[Language::Rust];
const PYTHON: &[Language] = &[Language::Python];
const WGSL: &[Language] = &[Language::Wgsl];

pub const SNIPPETS: &[Snippet] = &[
    // Lua
    Snippet {
        prefix: "fn",
        label: "function name(args) ... end",
        body: "function $0()\n    \nend",
        langs: LUA,
    },
    Snippet {
        prefix: "if",
        label: "if cond then ... end",
        body: "if $0 then\n    \nend",
        langs: LUA,
    },
    Snippet {
        prefix: "for",
        label: "for i = 1, n do ... end",
        body: "for $0 = 1, 10 do\n    \nend",
        langs: LUA,
    },
    Snippet {
        prefix: "while",
        label: "while cond do ... end",
        body: "while $0 do\n    \nend",
        langs: LUA,
    },
    Snippet {
        prefix: "ready",
        label: "function on_ready(ctx, vars)",
        body: "function on_ready(ctx, vars)\n    $0\nend",
        langs: LUA,
    },
    Snippet {
        prefix: "update",
        label: "function on_update(ctx, vars)",
        body: "function on_update(ctx, vars)\n    $0\nend",
        langs: LUA,
    },

    // Rhai
    Snippet {
        prefix: "fn",
        label: "fn name(args) { ... }",
        body: "fn $0() {\n    \n}",
        langs: RHAI,
    },
    Snippet {
        prefix: "if",
        label: "if cond { ... }",
        body: "if $0 {\n    \n}",
        langs: RHAI,
    },
    Snippet {
        prefix: "for",
        label: "for x in iter { ... }",
        body: "for $0 in 0..10 {\n    \n}",
        langs: RHAI,
    },
    Snippet {
        prefix: "while",
        label: "while cond { ... }",
        body: "while $0 {\n    \n}",
        langs: RHAI,
    },

    // Rust
    Snippet {
        prefix: "fn",
        label: "fn name(args) { ... }",
        body: "fn $0() {\n    \n}",
        langs: RUST,
    },
    Snippet {
        prefix: "pubfn",
        label: "pub fn name(args) { ... }",
        body: "pub fn $0() {\n    \n}",
        langs: RUST,
    },
    Snippet {
        prefix: "if",
        label: "if cond { ... }",
        body: "if $0 {\n    \n}",
        langs: RUST,
    },
    Snippet {
        prefix: "for",
        label: "for x in iter { ... }",
        body: "for $0 in iter {\n    \n}",
        langs: RUST,
    },
    Snippet {
        prefix: "match",
        label: "match expr { ... }",
        body: "match $0 {\n    _ => {}\n}",
        langs: RUST,
    },
    Snippet {
        prefix: "struct",
        label: "struct Name { ... }",
        body: "struct $0 {\n    \n}",
        langs: RUST,
    },
    Snippet {
        prefix: "impl",
        label: "impl Name { ... }",
        body: "impl $0 {\n    \n}",
        langs: RUST,
    },

    // Python
    Snippet {
        prefix: "def",
        label: "def name(args):",
        body: "def $0():\n    pass",
        langs: PYTHON,
    },
    Snippet {
        prefix: "class",
        label: "class Name:",
        body: "class $0:\n    pass",
        langs: PYTHON,
    },
    Snippet {
        prefix: "if",
        label: "if cond:",
        body: "if $0:\n    pass",
        langs: PYTHON,
    },
    Snippet {
        prefix: "for",
        label: "for x in iter:",
        body: "for $0 in iter:\n    pass",
        langs: PYTHON,
    },

    // WGSL
    Snippet {
        prefix: "fn",
        label: "fn name(args) -> ret { ... }",
        body: "fn $0() {\n    \n}",
        langs: WGSL,
    },
    Snippet {
        prefix: "vs",
        label: "vertex shader entry point",
        body: "@vertex\nfn $0(in: VertexInput) -> VertexOutput {\n    var out: VertexOutput;\n    return out;\n}",
        langs: WGSL,
    },
    Snippet {
        prefix: "fs",
        label: "fragment shader entry point",
        body: "@fragment\nfn $0(in: VertexOutput) -> @location(0) vec4<f32> {\n    return vec4(0.0, 0.0, 0.0, 1.0);\n}",
        langs: WGSL,
    },
];

/// Snippets matching the language whose prefix starts with the typed word
/// (case-insensitive). When `prefix` is empty, all snippets for the language
/// are returned.
pub fn matching_snippets(lang: Language, prefix: &str) -> Vec<&'static Snippet> {
    let lower = prefix.to_lowercase();
    let mut out: Vec<&'static Snippet> = SNIPPETS
        .iter()
        .filter(|s| s.langs.contains(&lang))
        .filter(|s| {
            if lower.is_empty() {
                true
            } else {
                s.prefix.to_lowercase().starts_with(&lower)
            }
        })
        .collect();
    out.sort_by(|a, b| a.prefix.cmp(b.prefix));
    out
}
