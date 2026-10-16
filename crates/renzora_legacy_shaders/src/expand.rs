//! Untangle `#ifdef`s that cut through a statement, by copying the statement.
//!
//! `#ifdef` gated lines, so a legacy shader could switch half an argument
//! list, or give one function two different headers. WESL has no way to gate
//! part of a statement. What it can gate is a whole one, so the statement (or
//! declaration) the chain cuts through is written out once per value of the
//! chain's flags, each copy wrapped in plain `#ifdef` lines and holding only
//! its own branch. Every copy is a whole unit again, and the normal pass
//! annotates it like any other.
//!
//! This works on the source text and runs before everything else, so the rest
//! of the translator never sees a split chain. The cost of a split is two
//! copies of one statement, not of the file.

use std::collections::BTreeSet;

use crate::directives::{self, Chain, Node};
use crate::{translate, Diagnostic};

/// Above this many flags in one split chain, the copies stop being readable.
const MAX_FLAGS: usize = 6;
/// A guard against a split the rewrite cannot remove: each round removes one.
const MAX_ROUNDS: usize = 256;

pub struct Expanded {
    pub source: String,
    /// For each line of `source`, the 1-based line of the original it came
    /// from, so diagnostics point at what the author wrote.
    pub line_map: Vec<usize>,
}

pub fn expand(source: &str, diagnostics: &mut Vec<Diagnostic>) -> Expanded {
    let mut text = source.to_string();
    let mut map: Vec<usize> = (1..=text.split_inclusive('\n').count().max(1)).collect();
    let mut reported: BTreeSet<usize> = BTreeSet::new();

    for _ in 0..MAX_ROUNDS {
        let mut scratch = Vec::new();
        let parsed = directives::parse(&text, &mut scratch);
        let Some(split) = translate::find_misaligned(&parsed.nodes) else { break };
        let Some(chain) = find_chain(&parsed.nodes, split.chain) else { break };
        let original = |line: usize| map.get(line.saturating_sub(1)).copied().unwrap_or(line);

        let mut flags = BTreeSet::new();
        for b in &chain.branches {
            b.effective.flags(&mut flags);
        }
        let flags: Vec<String> = flags.into_iter().collect();
        if flags.len() > MAX_FLAGS {
            diagnostics.push(Diagnostic::error(
                original(chain.line),
                format!(
                    "this `#ifdef` splits a statement and depends on {} flags, too many to write \
                     the statement out once per combination",
                    flags.len()
                ),
            ));
            break;
        }
        if reported.insert(original(chain.line)) {
            diagnostics.push(Diagnostic::info(
                original(chain.line),
                "this `#ifdef` splits a statement, so the statement was written out once per \
                 value of its flags",
            ));
        }

        let lines: Vec<&str> = text.split_inclusive('\n').collect();
        let pairs = pair_directives(&lines);
        let (first, last) = balance(&pairs, split.first_line, split.last_line.min(lines.len()));
        let starts = branch_starts(&lines, chain.line, chain.end_line);
        let branch_of = |l: usize| starts.iter().filter(|&&s| s < l).count().checked_sub(1);

        let mut out = String::with_capacity(text.len() * 2);
        let mut out_map = Vec::with_capacity(map.len() * 2);
        let mut push = |s: &str, from: usize, out: &mut String| {
            out.push_str(s);
            if !s.ends_with('\n') {
                out.push('\n');
            }
            out_map.push(from);
        };
        for l in 1..first {
            push(lines[l - 1], original(l), &mut out);
        }
        for sigma in 0..(1u32 << flags.len()) {
            let on = |f: &str| flags.iter().position(|x| x == f).is_some_and(|i| sigma & (1 << i) != 0);
            let taken = chain.branches.iter().position(|b| b.effective.eval(&on));
            for (i, f) in flags.iter().enumerate() {
                let d = if sigma & (1 << i) != 0 { "#ifdef" } else { "#ifndef" };
                push(&format!("{d} {f}\n"), original(first), &mut out);
            }
            for l in first..=last {
                if starts.contains(&l) || l == chain.end_line {
                    continue;
                }
                if l > chain.line && l < chain.end_line && branch_of(l) != taken {
                    continue;
                }
                push(lines[l - 1], original(l), &mut out);
            }
            for _ in &flags {
                push("#endif\n", original(last), &mut out);
            }
        }
        for l in (last + 1)..=lines.len() {
            push(lines[l - 1], original(l), &mut out);
        }
        text = out;
        map = out_map;
    }
    Expanded { source: text, line_map: map }
}

fn find_chain(nodes: &[Node], id: usize) -> Option<&Chain> {
    for node in nodes {
        if let Node::Chain(chain) = node {
            if chain.id == id {
                return Some(chain);
            }
            for b in &chain.branches {
                if let Some(c) = find_chain(&b.nodes, id) {
                    return Some(c);
                }
            }
        }
    }
    None
}

/// For every directive line of a conditional, the first and last line of the
/// whole chain it belongs to.
fn pair_directives(lines: &[&str]) -> Vec<Option<(usize, usize)>> {
    let mut out = vec![None; lines.len() + 1];
    let mut stack: Vec<(usize, Vec<usize>)> = Vec::new();
    for (i, raw) in lines.iter().enumerate() {
        let l = i + 1;
        match directives::directive_word(raw.trim_start()) {
            Some("ifdef" | "ifndef" | "if") => stack.push((l, vec![l])),
            Some("else") => {
                if let Some((_, members)) = stack.last_mut() {
                    members.push(l);
                }
            }
            Some("endif") => {
                if let Some((start, mut members)) = stack.pop() {
                    members.push(l);
                    for m in members {
                        out[m] = Some((start, l));
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// Widen `first..=last` until every conditional with a directive inside it
/// lies wholly inside it, so each copy has balanced `#ifdef`/`#endif`s.
fn balance(pairs: &[Option<(usize, usize)>], first: usize, last: usize) -> (usize, usize) {
    let (mut first, mut last) = (first, last);
    loop {
        // Widening can pull in more directives, so go again until nothing moves.
        let (mut lo, mut hi) = (first, last);
        for (s, e) in (first..=last).filter_map(|l| pairs.get(l).copied().flatten()) {
            lo = lo.min(s);
            hi = hi.max(e);
        }
        if (lo, hi) == (first, last) {
            return (first, last);
        }
        (first, last) = (lo, hi);
    }
}

/// The lines that begin each branch of the chain on `start..=end`: its
/// `#ifdef` and each of its own `#else`s, not those of chains nested in it.
fn branch_starts(lines: &[&str], start: usize, end: usize) -> Vec<usize> {
    let mut out = vec![start];
    let mut depth = 0usize;
    for l in (start + 1)..end {
        match directives::directive_word(lines[l - 1].trim_start()) {
            Some("ifdef" | "ifndef" | "if") => depth += 1,
            Some("endif") => depth = depth.saturating_sub(1),
            Some("else") if depth == 0 => out.push(l),
            _ => {}
        }
    }
    out
}
