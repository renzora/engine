//! Parse naga_oil's line directives into a tree of code, conditional chains
//! and imports.
//!
//! naga_oil was a line-based preprocessor: a line starting with `#` was a
//! directive and every other line was WGSL, with `#{NAME}` substituted inline.
//! The tree keeps that shape, so the rest of the translator works with real
//! `#ifdef` / `#else` / `#endif` structure instead of re-deriving it.

use crate::cond::Cond;
use crate::Diagnostic;

#[derive(Clone, Debug)]
pub enum Node {
    /// WGSL source lines, with `#{NAME}` already rewritten. `line` is the
    /// 1-based source line the text starts on.
    Code { text: String, line: usize },
    Chain(Chain),
    Import { text: String, line: usize },
}

#[derive(Clone, Debug)]
pub struct Chain {
    pub id: usize,
    /// The `#ifdef` line.
    pub line: usize,
    /// The `#endif` line.
    pub end_line: usize,
    pub branches: Vec<Branch>,
}

#[derive(Clone, Debug)]
pub struct Branch {
    /// When this branch is taken: its own test, and that no earlier branch of
    /// the chain was. An `#else` is just the negation of everything before it.
    pub effective: Cond,
    pub nodes: Vec<Node>,
}

pub struct Parsed {
    pub nodes: Vec<Node>,
    pub module_name: Option<String>,
}

/// Numeric comparisons the engine's pipelines are known to provide as flags.
///
/// Bevy 0.20 replaced `#if AVAILABLE_STORAGE_BUFFER_BINDINGS >= 3` with a flag
/// its Rust side sets, named `..._GE_3`. Any other comparison gets a flag of the
/// same shape and a warning, because nothing sets it until a pipeline does.
const PROVIDED_COMPARISONS: &[&str] = &[
    "AVAILABLE_STORAGE_BUFFER_BINDINGS__GE_3",
    "AVAILABLE_STORAGE_BUFFER_BINDINGS__GE_6",
];

struct Open {
    id: usize,
    line: usize,
    branches: Vec<Branch>,
    /// Negation of every branch test so far, for the next branch's `effective`.
    not_before: Cond,
    /// The nodes of the branch being filled.
    current: Vec<Node>,
    current_effective: Cond,
    has_else: bool,
}

/// A `#define` seen so far: the name, and when it is defined (the conditions
/// of the branches it sits in).
struct Define {
    name: String,
    when: Cond,
    has_value: bool,
}

pub fn parse(source: &str, diagnostics: &mut Vec<Diagnostic>) -> Parsed {
    let mut root: Vec<Node> = Vec::new();
    let mut stack: Vec<Open> = Vec::new();
    let mut module_name = None;
    let mut next_id = 0usize;
    let mut defines: Vec<Define> = Vec::new();

    let lines: Vec<&str> = source.split_inclusive('\n').collect();
    let mut i = 0;
    while i < lines.len() {
        let raw = lines[i];
        let line_no = i + 1;
        let trimmed = raw.trim_start();
        let directive = directive_word(trimmed);
        let enclosing = Cond::and(stack.iter().map(|o| o.current_effective.clone()));
        let target: &mut Vec<Node> = match stack.last_mut() {
            Some(open) => &mut open.current,
            None => &mut root,
        };

        match directive {
            None => {
                let text = substitute_defs(raw, &defines);
                push_code(target, text, line_no);
                i += 1;
            }
            Some("define") => {
                // naga_oil let a shader define its own defs. A bare one is a
                // flag the rest of the file can test; one with a value was
                // spliced in textually, which a `const` does as well.
                let body = strip_comment(&trimmed["#define".len()..]).trim();
                let (name, value) = match body.split_once(char::is_whitespace) {
                    Some((n, v)) => (n.trim(), Some(v.trim())),
                    None => (body, None),
                };
                if !is_ident(name) {
                    diagnostics.push(Diagnostic::error(line_no, format!("cannot read `#define {body}`")));
                } else {
                    if let Some(value) = value {
                        push_code(target, format!("const {name} = {value};\n"), line_no);
                    }
                    defines.push(Define { name: name.to_string(), when: enclosing, has_value: value.is_some() });
                }
                i += 1;
            }
            Some("define_import_path") => {
                module_name = Some(strip_comment(&trimmed["#define_import_path".len()..]).trim().to_string());
                i += 1;
            }
            Some("import") => {
                // An import's brace list may run over several lines.
                let mut text = strip_comment(&trimmed["#import".len()..]).trim().to_string();
                let mut depth = brace_balance(&text);
                i += 1;
                while depth > 0 && i < lines.len() {
                    let more = strip_comment(lines[i]);
                    depth += brace_balance(more);
                    text.push(' ');
                    text.push_str(more.trim());
                    i += 1;
                }
                target.push(Node::Import {
                    text: text.trim().to_string(),
                    line: line_no,
                });
            }
            Some("ifdef") | Some("ifndef") | Some("if") => {
                let test = branch_test(trimmed, line_no, &defines, diagnostics);
                let effective = test.clone();
                stack.push(Open {
                    id: next_id,
                    line: line_no,
                    branches: Vec::new(),
                    not_before: Cond::negate(test),
                    current: Vec::new(),
                    current_effective: effective,
                    has_else: false,
                });
                next_id += 1;
                i += 1;
            }
            Some("else") => {
                let Some(open) = stack.last_mut() else {
                    diagnostics.push(Diagnostic::error(line_no, "`#else` without a matching `#ifdef`"));
                    i += 1;
                    continue;
                };
                if open.has_else {
                    diagnostics.push(Diagnostic::error(line_no, "a second `#else` in the same `#ifdef`"));
                }
                // `#else`, or `#else ifdef X` / `#else ifndef X` / `#else if X`,
                // also written `#else #ifdef X`. Anything else after `#else`
                // was ignored by naga_oil, so it is a comment here too.
                let rest = strip_comment(trimmed["#else".len()..].trim_start());
                let rest = rest.trim().trim_start_matches('#');
                let chained = ["ifdef", "ifndef", "if"].iter().any(|d| {
                    rest.strip_prefix(d)
                        .is_some_and(|after| after.is_empty() || after.starts_with(char::is_whitespace))
                });
                let test = if chained {
                    branch_test(&format!("#{rest}"), line_no, &defines, diagnostics)
                } else {
                    open.has_else = true;
                    Cond::True
                };
                let finished = Branch {
                    effective: open.current_effective.clone(),
                    nodes: std::mem::take(&mut open.current),
                };
                open.branches.push(finished);
                open.current_effective = Cond::and([open.not_before.clone(), test.clone()]);
                open.not_before = Cond::and([open.not_before.clone(), Cond::negate(test)]);
                i += 1;
            }
            Some("endif") => {
                let Some(mut open) = stack.pop() else {
                    diagnostics.push(Diagnostic::error(line_no, "`#endif` without a matching `#ifdef`"));
                    i += 1;
                    continue;
                };
                open.branches.push(Branch {
                    effective: open.current_effective.clone(),
                    nodes: std::mem::take(&mut open.current),
                });
                let chain = Node::Chain(Chain {
                    id: open.id,
                    line: open.line,
                    end_line: line_no,
                    branches: open.branches,
                });
                match stack.last_mut() {
                    Some(parent) => parent.current.push(chain),
                    None => root.push(chain),
                }
                i += 1;
            }
            Some(other) => {
                diagnostics.push(Diagnostic::error(
                    line_no,
                    format!("`#{other}` is not a directive this translator knows; the line was dropped"),
                ));
                i += 1;
            }
        }
    }

    // Close anything left open, so a missing `#endif` costs a diagnostic rather
    // than the rest of the file.
    while let Some(mut open) = stack.pop() {
        diagnostics.push(Diagnostic::error(open.line, "this `#ifdef` has no `#endif`"));
        open.branches.push(Branch {
            effective: open.current_effective.clone(),
            nodes: std::mem::take(&mut open.current),
        });
        let chain = Node::Chain(Chain {
            id: open.id,
            line: open.line,
            end_line: lines.len(),
            branches: open.branches,
        });
        match stack.last_mut() {
            Some(parent) => parent.current.push(chain),
            None => root.push(chain),
        }
    }

    prune_dead(&mut root);
    Parsed { nodes: root, module_name }
}

fn push_code(target: &mut Vec<Node>, text: String, line: usize) {
    match target.last_mut() {
        Some(Node::Code { text: t, .. }) => t.push_str(&text),
        _ => target.push(Node::Code { text, line }),
    }
}

/// Is this line a preprocessor directive, and which one?
pub(crate) fn directive_word(trimmed: &str) -> Option<&str> {
    let rest = trimmed.strip_prefix('#')?;
    // `#{NAME}` at the start of a line is a substitution, not a directive.
    if rest.starts_with('{') {
        return None;
    }
    let end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    let word = &rest[..end];
    (!word.is_empty()).then_some(word)
}

/// The test an `#ifdef` / `#ifndef` / `#if` line makes.
fn branch_test(trimmed: &str, line: usize, defines: &[Define], diagnostics: &mut Vec<Diagnostic>) -> Cond {
    let text = strip_comment(trimmed).trim();
    // A def is set when the pipeline sets it, or when the file `#define`d it
    // earlier on a path that reaches here. `#ifdef 0` names a def nobody can
    // define: Bevy's own tonemapping shader used it to switch a block off.
    let def = |name: &str| {
        let name = name.trim();
        if name.starts_with(|c: char| c.is_ascii_digit()) {
            return Cond::False;
        }
        let local = defines.iter().filter(|d| d.name == name).map(|d| d.when.clone());
        Cond::or(std::iter::once(Cond::flag(name)).chain(local))
    };
    if let Some(name) = text.strip_prefix("#ifdef") {
        return def(name);
    }
    if let Some(name) = text.strip_prefix("#ifndef") {
        return Cond::negate(def(name));
    }
    let Some(expr) = text.strip_prefix("#if") else {
        diagnostics.push(Diagnostic::error(line, format!("cannot read the condition `{text}`")));
        return Cond::False;
    };
    let expr = expr.trim();
    // `#if 0` is how a block gets commented out.
    match expr {
        "0" | "false" => return Cond::False,
        "1" | "true" => return Cond::True,
        _ => {}
    }
    for op in ["==", "!=", ">=", "<=", ">", "<"] {
        let Some((name, value)) = expr.split_once(op) else {
            continue;
        };
        let (name, value) = (name.trim(), value.trim());
        if !is_ident(name) {
            break;
        }
        match (op, value) {
            ("==", "true") | ("!=", "false") => return Cond::flag(name),
            ("==", "false") | ("!=", "true") => return Cond::negate(Cond::flag(name)),
            _ => {}
        }
        if value.parse::<i64>().is_err() {
            break;
        }
        // Bevy's convention for a comparison it turned into a flag.
        let (flag, negate) = match op {
            ">=" => (format!("{name}__GE_{value}"), false),
            ">" => (format!("{name}__GT_{value}"), false),
            "==" => (format!("{name}__EQ_{value}"), false),
            "<" => (format!("{name}__GE_{value}"), true),
            "<=" => (format!("{name}__GT_{value}"), true),
            _ => (format!("{name}__EQ_{value}"), true),
        };
        if !PROVIDED_COMPARISONS.contains(&flag.as_str()) {
            diagnostics.push(Diagnostic::warning(
                line,
                format!(
                    "`{name} {op} {value}` became the flag `{flag}`. WESL cannot compare shader \
                     defs, so the pipeline has to set that flag for this branch to be taken"
                ),
            ));
        }
        let c = Cond::Flag(flag);
        return if negate { Cond::negate(c) } else { c };
    }
    if is_ident(expr) {
        return def(expr);
    }
    diagnostics.push(Diagnostic::error(line, format!("cannot read the condition `#if {expr}`")));
    Cond::False
}

fn is_ident(s: &str) -> bool {
    !s.is_empty()
        && s.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn strip_comment(s: &str) -> &str {
    s.find("//").map_or(s, |at| &s[..at])
}

fn brace_balance(s: &str) -> i32 {
    s.matches('{').count() as i32 - s.matches('}').count() as i32
}

/// `#{NAME}` and the older brace-less `#NAME` become `constants::NAME`, which
/// is where Bevy 0.20 puts the value of a numeric shader def. A name the file
/// `#define`d with a value is the `const` that define became.
fn substitute_defs(line: &str, defines: &[Define]) -> String {
    // A `//` comment is prose: "#define HORIZONTAL" there is a sentence about
    // the old directive, not a substitution.
    if let Some(at) = line.find("//") {
        let (code, comment) = line.split_at(at);
        return substitute_defs(code, defines) + comment;
    }
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(at) = rest.find('#') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let (name, consumed) = match after.strip_prefix('{') {
            Some(inner) => match inner.find('}') {
                Some(end) => (inner[..end].trim(), end + 2),
                None => ("", 0),
            },
            None => {
                let end = after
                    .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                    .unwrap_or(after.len());
                (&after[..end], end)
            }
        };
        if is_ident(name) {
            let local = defines.iter().any(|d| d.name == name && d.has_value);
            let qualified = if local { name.to_string() } else { format!("constants::{name}") };
            let tail = &after[consumed..];
            // `#{N}u` was text substitution, so it read as the literal `4u`.
            // `constants::Nu` would name something else entirely: the suffix
            // becomes a conversion instead.
            match literal_suffix(tail) {
                Some((ty, len)) => {
                    out.push_str(&format!("{ty}({qualified})"));
                    rest = &tail[len..];
                }
                None => {
                    out.push_str(&qualified);
                    rest = tail;
                }
            }
        } else {
            out.push('#');
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// A WGSL numeric-literal suffix standing on its own at the start of `tail`,
/// as the type it converts to and its length.
fn literal_suffix(tail: &str) -> Option<(&'static str, usize)> {
    let ty = match tail.chars().next()? {
        'u' => "u32",
        'i' => "i32",
        'f' => "f32",
        'h' => "f16",
        _ => return None,
    };
    let joined = tail[1..].chars().next().is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
    (!joined).then_some((ty, 1))
}

/// Empty every branch that can never be taken (`#if 0`, `#ifdef A` inside
/// `#ifndef A`). WESL parses code under `@if(false)` all the same, and dead
/// branches are where old shaders keep things that do not parse: the GLSL a
/// function was ported from, a half-finished experiment.
fn prune_dead(nodes: &mut [Node]) {
    for node in nodes {
        if let Node::Chain(chain) = node {
            for b in &mut chain.branches {
                if b.effective == Cond::False {
                    b.nodes.clear();
                } else {
                    prune_dead(&mut b.nodes);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(src: &str) -> Parsed {
        let mut d = Vec::new();
        let p = parse(src, &mut d);
        assert!(d.is_empty(), "{d:?}");
        p
    }

    #[test]
    fn else_branches_negate_everything_before_them() {
        let p = parse_ok("#ifdef A\na\n#else ifdef B\nb\n#else\nc\n#endif\n");
        let Node::Chain(chain) = &p.nodes[0] else { panic!() };
        let conds: Vec<String> = chain.branches.iter().map(|b| b.effective.to_string()).collect();
        assert_eq!(conds, ["A", "!A && B", "!A && !B"]);
    }

    #[test]
    fn substitutions_and_module_names() {
        let p = parse_ok("#define_import_path my::mod\n@group(#{MATERIAL_BIND_GROUP}) var x: f32;\n");
        assert_eq!(p.module_name.as_deref(), Some("my::mod"));
        let Node::Code { text, .. } = &p.nodes[0] else { panic!() };
        assert_eq!(text, "@group(constants::MATERIAL_BIND_GROUP) var x: f32;\n");
    }

    #[test]
    fn a_literal_suffix_after_a_substitution_becomes_a_conversion() {
        let p = parse_ok("let n = #{COUNT}u + #{BIAS}f;\nlet m = #{COUNT}u32;\n");
        let Node::Code { text, .. } = &p.nodes[0] else { panic!() };
        assert_eq!(
            text,
            "let n = u32(constants::COUNT) + f32(constants::BIAS);\nlet m = constants::COUNTu32;\n"
        );
    }

    #[test]
    fn a_directive_named_in_a_comment_is_left_alone() {
        let p = parse_ok("let x = #{N}; // set by #define HORIZONTAL\n");
        let Node::Code { text, .. } = &p.nodes[0] else { panic!() };
        assert_eq!(text, "let x = constants::N; // set by #define HORIZONTAL\n");
    }

    #[test]
    fn multi_line_imports_are_joined() {
        let p = parse_ok("#import bevy_pbr::{\n  a,\n  b,\n}\nfn f() {}\n");
        let Node::Import { text, .. } = &p.nodes[0] else { panic!() };
        assert_eq!(text, "bevy_pbr::{ a, b, }");
    }

    #[test]
    fn numeric_comparisons_follow_bevys_flag_names() {
        let mut d = Vec::new();
        let p = parse("#if AVAILABLE_STORAGE_BUFFER_BINDINGS >= 3\na\n#endif\n#if N < 4\nb\n#endif\n", &mut d);
        let Node::Chain(c) = &p.nodes[0] else { panic!() };
        assert_eq!(c.branches[0].effective.to_string(), "AVAILABLE_STORAGE_BUFFER_BINDINGS__GE_3");
        let Node::Chain(c) = &p.nodes[1] else { panic!() };
        assert_eq!(c.branches[0].effective.to_string(), "!N__GE_4");
        // Only the comparison no pipeline provides is worth a warning.
        assert_eq!(d.len(), 1);
    }
}
