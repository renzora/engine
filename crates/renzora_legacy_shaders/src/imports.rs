//! naga_oil `#import` lines, and the module paths written out in code.
//!
//! Two things changed at once. The syntax: `#import a::b` became
//! `import a::b;`. And the names: most Bevy modules moved, so the path inside
//! has to be looked up too. A module import is the awkward case, because the
//! code then says `mesh_functions::foo()` and `mesh_functions` may no longer be
//! the module's last segment (`prepass_io` is now `prepass::io`). Those
//! references are rewritten to the module's full new path, which needs no
//! import at all and cannot collide with anything.

use std::collections::{BTreeMap, BTreeSet};

use crate::lex::{Kind, Token};
use crate::{table, Diagnostic, Options};

/// One imported name, after expanding `a::{b, c::{d}}` into its leaves.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub path: Vec<String>,
    pub alias: Option<String>,
    /// For `#import "shaders/foo.wgsl"::item`, the asset path in quotes.
    pub asset: Option<String>,
}

pub fn parse(text: &str) -> Result<Vec<Entry>, String> {
    // naga_oil tolerated a WGSL-style `;` after an import, and plenty of
    // shaders (Bevy's own included) have one.
    let text = text.trim().trim_end_matches(';').trim_end();
    let mut p = Parser { s: text, at: 0 };
    let mut out = Vec::new();
    p.tree(Vec::new(), None, &mut out)?;
    p.skip_ws();
    if p.at < p.s.len() {
        return Err(format!("unexpected `{}`", &p.s[p.at..]));
    }
    Ok(out)
}

struct Parser<'a> {
    s: &'a str,
    at: usize,
}

impl Parser<'_> {
    fn skip_ws(&mut self) {
        while self.s[self.at..].starts_with(char::is_whitespace) {
            self.at += self.s[self.at..].chars().next().map_or(1, char::len_utf8);
        }
    }

    fn eat(&mut self, tok: &str) -> bool {
        self.skip_ws();
        if self.s[self.at..].starts_with(tok) {
            self.at += tok.len();
            true
        } else {
            false
        }
    }

    fn ident(&mut self) -> Option<String> {
        self.skip_ws();
        let rest = &self.s[self.at..];
        let len = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        (len > 0).then(|| {
            self.at += len;
            rest[..len].to_string()
        })
    }

    fn tree(&mut self, prefix: Vec<String>, asset: Option<String>, out: &mut Vec<Entry>) -> Result<(), String> {
        let mut path = prefix;
        let mut asset = asset;
        self.skip_ws();
        if path.is_empty() && asset.is_none() && self.eat("\"") {
            let end = self.s[self.at..].find('"').ok_or("unterminated asset path")?;
            asset = Some(self.s[self.at..self.at + end].to_string());
            self.at += end + 1;
            if !self.eat("::") {
                out.push(Entry { path, alias: self.alias(), asset });
                return Ok(());
            }
        }
        loop {
            if self.eat("{") {
                loop {
                    self.skip_ws();
                    if self.eat("}") {
                        return Ok(());
                    }
                    self.tree(path.clone(), asset.clone(), out)?;
                    if !self.eat(",") {
                        if self.eat("}") {
                            return Ok(());
                        }
                        return Err("expected `,` or `}` in an import list".into());
                    }
                }
            }
            let seg = self.ident().ok_or("expected a name")?;
            path.push(seg);
            if !self.eat("::") {
                break;
            }
        }
        out.push(Entry { path, alias: self.alias(), asset });
        Ok(())
    }

    fn alias(&mut self) -> Option<String> {
        let save = self.at;
        self.skip_ws();
        if self.s[self.at..].starts_with("as") {
            self.at += 2;
            if let Some(a) = self.ident() {
                return Some(a);
            }
        }
        self.at = save;
        None
    }
}

/// Where a legacy module path points now.
pub struct Resolver<'a> {
    pub options: &'a Options,
    /// Short name a module was imported under -> its full 0.19 path.
    pub module_aliases: BTreeMap<String, Vec<String>>,
}

impl<'a> Resolver<'a> {
    pub fn new(options: &'a Options) -> Self {
        Self { options, module_aliases: BTreeMap::new() }
    }

    fn renames(&self) -> impl Iterator<Item = (&str, &str)> {
        table::MODULES
            .iter()
            .copied()
            .chain(self.options.module_renames.iter().map(|(a, b)| (a.as_str(), b.as_str())))
    }

    /// The new path for a full legacy path (module, or module plus item), or
    /// `None` when no table knows its module.
    pub fn map(&self, path: &[String]) -> Option<Vec<String>> {
        let joined = path.join("::");
        for (old, new_module) in table::MOVED_ITEMS {
            if joined == *old {
                let item = path.last()?.clone();
                return Some(split(new_module).into_iter().chain([item]).collect());
            }
        }
        let mut best: Option<(usize, &str)> = None;
        for (old, new) in self.renames() {
            let old_segs = split(old);
            if path.len() >= old_segs.len()
                && path[..old_segs.len()] == old_segs[..]
                && best.is_none_or(|(n, _)| old_segs.len() > n)
            {
                best = Some((old_segs.len(), new));
            }
        }
        let (n, new) = best?;
        Some(split(new).into_iter().chain(path[n..].iter().cloned()).collect())
    }

    /// Is `path` exactly a module (as opposed to a module plus an item)?
    fn is_module(&self, path: &[String]) -> bool {
        let joined = path.join("::");
        self.renames().any(|(old, _)| old == joined) || self.options.known_modules.contains(&joined)
    }

    /// Is `path` a namespace above some module (`bevy_pbr`, `bevy_pbr::atmosphere`)?
    fn is_namespace(&self, path: &[String]) -> bool {
        let joined = format!("{}::", path.join("::"));
        self.renames().any(|(old, _)| old.starts_with(&joined))
    }
}

fn split(s: &str) -> Vec<String> {
    s.split("::").map(str::to_string).collect()
}

/// Translate one import entry into a WESL `import` (or nothing, for a module
/// whose uses are rewritten to full paths instead).
pub fn translate(
    entry: &Entry,
    resolver: &mut Resolver,
    used_as_module: &BTreeSet<String>,
    line: usize,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    let alias = entry.alias.as_ref().map(|a| format!(" as {a}")).unwrap_or_default();

    if let Some(asset) = &entry.asset {
        // Bevy 0.20 names an asset shader by its path from the asset root,
        // without the extension; `package::` is that root.
        let stem = asset.rsplit_once('.').map_or(asset.as_str(), |(s, _)| s);
        let segs: Vec<&str> = stem.split('/').filter(|s| !s.is_empty()).collect();
        if let Some(bad) = segs.iter().find(|s| !s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')) {
            diagnostics.push(Diagnostic::error(
                line,
                format!("the asset path `{asset}` has `{bad}` in it, which WESL cannot name as a module; rename the file"),
            ));
            return None;
        }
        let mut path: Vec<String> = segs.iter().map(|s| s.to_string()).collect();
        path.extend(entry.path.iter().cloned());
        return Some(format!("import package::{}{alias};", path.join("::")));
    }

    let path = &entry.path;
    if resolver.is_module(path) || (resolver.is_namespace(path) && entry.alias.is_none()) {
        let short = entry.alias.clone().unwrap_or_else(|| path.last().cloned().unwrap_or_default());
        if resolver.map(path).is_some() || resolver.is_namespace(path) {
            // Uses become full paths; see the module docs.
            resolver.module_aliases.insert(short, path.clone());
            return None;
        }
        // A module only the caller knows about keeps its name: the loader
        // registers legacy modules under their old `#define_import_path`.
        return Some(format!("import {}{alias};", path.join("::")));
    }

    if let Some(new) = resolver.map(path) {
        return Some(format!("import {}{alias};", new.join("::")));
    }

    // Only a path under one of Bevy's own crates is suspicious. Anything else
    // (a vendored crate's modules, a project's own) keeps its name, and the
    // loader registers it under that name.
    let root = path.first().map(String::as_str).unwrap_or("");
    let under_known = resolver
        .options
        .known_modules
        .iter()
        .any(|m| path.join("::").starts_with(&format!("{m}::")) || path.join("::") == *m);
    if table::is_bevy_root(root) && !under_known {
        diagnostics.push(Diagnostic::warning(
            line,
            format!(
                "`{}` is not a Bevy 0.19 shader module this translator knows, so the import was \
                 kept as written and may not resolve",
                path.join("::")
            ),
        ));
    }
    // An unknown path the code uses as `last::item` is a module import.
    if let Some(last) = path.last() {
        if used_as_module.contains(last) {
            return Some(format!("import {}{alias};", path.join("::")));
        }
    }
    Some(format!("import {}{alias};", path.join("::")))
}

/// Every identifier the code uses as the first segment of a path (`x::...`).
pub fn qualified_roots(tokens: &[Token]) -> BTreeSet<String> {
    let sig: Vec<&Token> = tokens.iter().filter(|t| t.kind != Kind::Trivia).collect();
    let mut out = BTreeSet::new();
    for w in sig.windows(2) {
        if w[0].kind == Kind::Ident && w[1].is("::") {
            out.insert(w[0].text.clone());
        }
    }
    out
}

/// Rewrite qualified paths in code (`mesh_functions::foo`,
/// `bevy_pbr::mesh_view_bindings::view`) to their 0.20 full paths, in place.
pub fn rewrite_paths(tokens: &mut [Token], resolver: &Resolver) {
    let sig: Vec<usize> = (0..tokens.len()).filter(|&i| tokens[i].kind != Kind::Trivia).collect();
    let mut k = 0;
    while k < sig.len() {
        let first = sig[k];
        // A path starts at an identifier that is not itself after `::`.
        let after_colons = k > 0 && tokens[sig[k - 1]].is("::");
        if tokens[first].kind != Kind::Ident || after_colons || !sig.get(k + 1).is_some_and(|&j| tokens[j].is("::")) {
            k += 1;
            continue;
        }
        // Collect `ident (:: ident)*`, as long as nothing but `::` separates
        // the segments (trivia between them is not a path).
        let mut segs = vec![tokens[first].text.clone()];
        let mut idx = vec![first];
        let mut m = k;
        while m + 2 < sig.len()
            && tokens[sig[m + 1]].is("::")
            && tokens[sig[m + 2]].kind == Kind::Ident
            && sig[m + 2] == sig[m + 1] + 1
            && sig[m + 1] == sig[m] + 1
        {
            segs.push(tokens[sig[m + 2]].text.clone());
            idx.push(sig[m + 1]);
            idx.push(sig[m + 2]);
            m += 2;
        }

        let full: Option<Vec<String>> = match resolver.module_aliases.get(&segs[0]) {
            Some(module) => {
                let legacy: Vec<String> = module.iter().cloned().chain(segs[1..].iter().cloned()).collect();
                Some(resolver.map(&legacy).unwrap_or(legacy))
            }
            None if table::is_bevy_root(&segs[0])
                || resolver.options.module_renames.iter().any(|(old, _)| old.split("::").next() == Some(&segs[0])) =>
            {
                resolver.map(&segs)
            }
            None => None,
        };
        if let Some(full) = full {
            tokens[idx[0]].text = full.join("::");
            for &i in &idx[1..] {
                tokens[i].text.clear();
                tokens[i].kind = Kind::Trivia;
            }
        }
        k = m + 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lex::tokenize;

    fn entries(s: &str) -> Vec<String> {
        parse(s)
            .unwrap()
            .into_iter()
            .map(|e| {
                let mut t = e.path.join("::");
                if let Some(a) = e.asset {
                    t = format!("\"{a}\"::{t}");
                }
                if let Some(a) = e.alias {
                    t = format!("{t} as {a}");
                }
                t
            })
            .collect()
    }

    #[test]
    fn parses_nested_lists_aliases_and_asset_paths() {
        assert_eq!(
            entries("bevy_pbr::{forward_io::VertexOutput, mesh_functions as mf, pbr_types::{A, B},}"),
            ["bevy_pbr::forward_io::VertexOutput", "bevy_pbr::mesh_functions as mf", "bevy_pbr::pbr_types::A", "bevy_pbr::pbr_types::B"]
        );
        assert_eq!(entries("\"shaders/util.wgsl\"::COLOR"), ["\"shaders/util.wgsl\"::COLOR"]);
    }

    fn run(import: &str, code: &str) -> (Vec<String>, String) {
        let options = Options::default();
        let mut resolver = Resolver::new(&options);
        let mut toks = tokenize(code);
        let used = qualified_roots(&toks);
        let mut d = Vec::new();
        let imports = parse(import)
            .unwrap()
            .iter()
            .filter_map(|e| translate(e, &mut resolver, &used, 1, &mut d))
            .collect();
        rewrite_paths(&mut toks, &resolver);
        (imports, toks.into_iter().map(|t| t.text).collect())
    }

    #[test]
    fn item_imports_follow_the_module_table() {
        let (imports, _) = run("bevy_pbr::forward_io::VertexOutput", "");
        assert_eq!(imports, ["import bevy_pbr::render::forward_io::VertexOutput;"]);
        let (imports, _) = run("bevy_pbr::utils::octahedral_encode", "");
        assert_eq!(imports, ["import bevy_render::utils::octahedral_encode;"]);
    }

    #[test]
    fn module_imports_become_full_paths_in_code() {
        let (imports, code) = run(
            "bevy_pbr::prepass_io",
            "var o: prepass_io::VertexOutput; let v = bevy_pbr::mesh_view_bindings::view;",
        );
        assert!(imports.is_empty());
        assert_eq!(
            code,
            "var o: bevy_pbr::prepass::io::VertexOutput; let v = bevy_pbr::render::mesh_view_bindings::view;"
        );
    }

    #[test]
    fn constants_and_unknown_paths_are_left_alone() {
        let (_, code) = run("bevy_pbr::mesh_functions", "@group(constants::X) var t: my_lib::Thing;");
        assert_eq!(code, "@group(constants::X) var t: my_lib::Thing;");
    }
}
