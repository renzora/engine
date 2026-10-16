//! Legacy shader support: translate shaders written for Bevy's old naga_oil
//! preprocessor into WESL.
//!
//! Bevy 0.20 replaced naga_oil with WESL. A `.wgsl` file is now handed to the
//! GPU as it is, so a shader still written with `#import`, `#ifdef` or
//! `#{MATERIAL_BIND_GROUP}` fails on its first line. Shaders like that are not
//! only the engine's own: theme shaders, `.shader` files in projects and the
//! shaders inside marketplace plugins were all written that way, and their
//! authors should not have to port them by hand to keep them working.
//!
//! [`translate`] does that port. What it covers:
//!
//! - `#import` lines become `import ...;` at the top of the file, with Bevy's
//!   module paths updated to where 0.20 put them ([`table`]).
//! - `#ifdef` / `#ifndef` / `#if` / `#else` / `#endif` become `@if(...)` on
//!   each declaration, statement, struct member or parameter they wrap.
//! - `#{NAME}` becomes `constants::NAME`.
//! - `#define_import_path` is dropped from the source and returned, so the
//!   loader can keep registering the module under its old name and the
//!   shaders importing it keep resolving.
//!
//! What it cannot fix is reported in [`Translation::diagnostics`] with a line
//! number rather than failing silently: a numeric `#if` comparison no pipeline
//! provides a flag for, a module path it does not know, or a call to a Bevy
//! shader function that 0.20 removed (which surfaces when the result compiles).

mod api;
mod cond;
mod directives;
mod expand;
mod imports;
mod lex;
pub mod table;
mod translate;

pub use cond::Cond;

use directives::Node;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// Something was translated in an unusual way, and still works.
    Info,
    /// The result compiles but may not behave as before.
    Warning,
    /// Part of the shader could not be translated.
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// 1-based line in the legacy source.
    pub line: usize,
    pub severity: Severity,
    pub message: String,
}

impl Diagnostic {
    pub(crate) fn error(line: usize, message: impl Into<String>) -> Self {
        Self { line, severity: Severity::Error, message: message.into() }
    }

    pub(crate) fn warning(line: usize, message: impl Into<String>) -> Self {
        Self { line, severity: Severity::Warning, message: message.into() }
    }

    pub(crate) fn info(line: usize, message: impl Into<String>) -> Self {
        Self { line, severity: Severity::Info, message: message.into() }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Options {
    /// Extra module renames on top of Bevy's, as `(old, new)`: for the
    /// engine's own modules, whose 0.20 names depend on where they are loaded.
    pub module_renames: Vec<(String, String)>,
    /// Legacy module names that exist and keep their name, typically every
    /// other shader's `#define_import_path` in the same project. Tells a
    /// module import (`#import my::lib`) apart from an item import
    /// (`#import my::Thing`).
    pub known_modules: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Translation {
    /// The shader as WESL.
    pub source: String,
    /// The `#define_import_path` the legacy source declared, if any.
    pub module_name: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Translation {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == Severity::Error)
    }
}

/// Does this source use anything only naga_oil understood?
///
/// Plain WGSL is valid WESL, so a shader that answers `false` needs no
/// translation and should be passed through untouched.
pub fn is_legacy(source: &str) -> bool {
    source.lines().any(|line| {
        let t = line.trim_start();
        [
            "#import",
            "#ifdef",
            "#ifndef",
            "#if ",
            "#else",
            "#endif",
            "#define_import_path",
        ]
        .iter()
        .any(|d| t.starts_with(d))
            || has_substitution(line)
    })
}

/// `#{NAME}`, or the older `#NAME`, outside a comment.
fn has_substitution(line: &str) -> bool {
    let code = line.split("//").next().unwrap_or("");
    code.match_indices('#').any(|(at, _)| {
        code[at + 1..]
            .chars()
            .next()
            .is_some_and(|c| c == '{' || c == '_' || c.is_ascii_alphabetic())
    })
}

/// Translate a naga_oil-era shader into WESL.
pub fn translate(source: &str, options: &Options) -> Translation {
    // Split statements first, on the text, so nothing after this sees one.
    let mut expand_diagnostics = Vec::new();
    let expanded = expand::expand(source, &mut expand_diagnostics);

    let mut diagnostics = Vec::new();
    let mut parsed = directives::parse(&expanded.source, &mut diagnostics);
    rename_defs(&mut parsed.nodes);

    // Imports first: they decide how qualified paths in the code are read.
    let mut found = Vec::new();
    collect_imports(&parsed.nodes, &Cond::True, &mut found);
    let mut code_tokens = Vec::new();
    collect_code(&parsed.nodes, &mut code_tokens);
    let used_as_module = imports::qualified_roots(&code_tokens);
    let declared_here = module_scope_names(&code_tokens);

    let mut resolver = imports::Resolver::new(options);
    let mut import_lines: Vec<(Cond, String)> = Vec::new();
    for (cond, text, line) in found {
        match imports::parse(&text) {
            Ok(entries) => {
                for entry in entries {
                    // naga_oil let a file's own declaration shadow an import of
                    // the same name; WESL calls the pair a duplicate.
                    let bound = entry.alias.as_ref().or(entry.path.last());
                    if let Some(name) = bound.filter(|n| declared_here.contains(*n)) {
                        diagnostics.push(Diagnostic::info(
                            line,
                            format!("`{name}` is imported and also declared in this file; the file's own declaration wins, as it did before"),
                        ));
                        continue;
                    }
                    if let Some(out) = imports::translate(&entry, &mut resolver, &used_as_module, line, &mut diagnostics) {
                        if !import_lines.iter().any(|(c, t)| *c == cond && *t == out) {
                            import_lines.push((cond.clone(), out));
                        }
                    }
                }
            }
            Err(e) => diagnostics.push(Diagnostic::error(line, format!("cannot read `#import {text}`: {e}"))),
        }
    }

    let mut items = translate::translate_tree(&parsed.nodes, &resolver, &mut diagnostics);
    for d in &mut diagnostics {
        d.line = expanded.line_map.get(d.line.saturating_sub(1)).copied().unwrap_or(d.line);
    }
    diagnostics.extend(expand_diagnostics);

    // A licence or doc comment at the top stays at the top; the imports go
    // after it, and must come before any declaration.
    let mut out = String::new();
    if let Some(first) = items.first_mut() {
        let lead = std::mem::take(&mut first.lead);
        let keep_on_top = lead.trim_end().len();
        out.push_str(lead[..keep_on_top].trim_start_matches('\n'));
        if keep_on_top > 0 {
            out.push('\n');
        }
        first.lead = lead[keep_on_top..].to_string();
    }
    if !import_lines.is_empty() {
        if !out.is_empty() {
            out.push('\n');
        }
        for (cond, line) in &import_lines {
            if !cond.is_true() {
                out.push_str(&format!("@if({cond}) "));
            }
            out.push_str(line);
            out.push('\n');
        }
    }
    for item in &items {
        out.push_str(&item.lead);
        if !item.cond.is_true() && !item.body.is_empty() {
            out.push_str(&format!("@if({})\n", item.cond));
        }
        out.push_str(&item.body);
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }

    diagnostics.sort_by_key(|d| d.line);
    diagnostics.dedup();
    Translation { source: out, module_name: parsed.module_name, diagnostics }
}

/// Names declared at module scope: `fn`, `struct`, `const`, `var`, `alias`
/// and `override` outside any braces.
fn module_scope_names(tokens: &[lex::Token]) -> std::collections::BTreeSet<String> {
    let sig: Vec<&lex::Token> = tokens.iter().filter(|t| t.kind != lex::Kind::Trivia).collect();
    let mut out = std::collections::BTreeSet::new();
    let mut depth = 0i32;
    let mut i = 0;
    while i < sig.len() {
        match sig[i].text.as_str() {
            "{" => depth += 1,
            "}" => depth -= 1,
            "fn" | "struct" | "const" | "var" | "alias" | "override" if depth == 0 => {
                let mut j = i + 1;
                // `var<uniform> name`.
                if sig.get(j).is_some_and(|t| t.text == "<") {
                    while j < sig.len() && sig[j].text != ">" {
                        j += 1;
                    }
                    j += 1;
                }
                if let Some(name) = sig.get(j).filter(|t| t.kind == lex::Kind::Ident) {
                    out.insert(name.text.clone());
                }
            }
            _ => {}
        }
        i += 1;
    }
    out
}

fn rename_defs(nodes: &mut [Node]) {
    let map = |f: &str| {
        table::DEFS
            .iter()
            .find(|(old, _)| *old == f)
            .map(|(_, new)| new.to_string())
    };
    for node in nodes {
        if let Node::Chain(chain) = node {
            for b in &mut chain.branches {
                b.effective = b.effective.rename(&map);
                rename_defs(&mut b.nodes);
            }
        }
    }
}

fn collect_imports(nodes: &[Node], cond: &Cond, out: &mut Vec<(Cond, String, usize)>) {
    for node in nodes {
        match node {
            Node::Import { text, line } => out.push((cond.clone(), text.clone(), *line)),
            Node::Chain(chain) => {
                for b in &chain.branches {
                    collect_imports(&b.nodes, &Cond::and([cond.clone(), b.effective.clone()]), out);
                }
            }
            Node::Code { .. } => {}
        }
    }
}

fn collect_code(nodes: &[Node], out: &mut Vec<lex::Token>) {
    for node in nodes {
        match node {
            Node::Code { text, .. } => out.extend(lex::tokenize(text)),
            Node::Chain(chain) => chain.branches.iter().for_each(|b| collect_code(&b.nodes, out)),
            Node::Import { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests;
