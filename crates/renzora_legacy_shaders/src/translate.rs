//! Turn the directive tree into WESL: find what each `#ifdef` wraps, and put
//! an `@if` on it.
//!
//! WESL's `@if` gates one *unit*: a module-scope declaration, a statement, a
//! struct member or a function parameter. naga_oil's `#ifdef` gated lines, and
//! lines need not line up with units. So the work is in two passes.
//!
//! The first scans the code as units, and checks that every `#ifdef` branch
//! starts and ends on a unit boundary. That is nearly every real shader, and
//! for those each unit inside a branch simply gets the branch's condition.
//! A block of three statements becomes three annotated statements rather than
//! one `@if { }` block, because braces would open a scope and hide a `let` the
//! code after it reads.
//!
//! A chain that cuts through a unit (half an argument list, one arm of an
//! `else`) has no WESL spelling. [`find_misaligned`] reports the lines of the
//! unit around it, and `expand` rewrites those lines once per value of the
//! chain's flags before this pass runs again. That keeps the copying to the
//! one statement involved, where expanding the file would multiply by every
//! split chain in it.

use std::collections::BTreeMap;

use crate::cond::Cond;
use crate::directives::Node;
use crate::imports::{self, Resolver};
use crate::lex::{self, Kind, Token};
use crate::Diagnostic;

/// A module-scope declaration, with the comments and blank lines before it.
#[derive(Clone, Debug)]
pub struct Item {
    pub cond: Cond,
    pub lead: String,
    pub body: String,
}

/// A chain that cuts through a unit, and the source lines of the smallest run
/// of whole units around it.
#[derive(Clone, Debug)]
pub struct Misaligned {
    pub chain: usize,
    pub first_line: usize,
    pub last_line: usize,
}

pub fn translate_tree(nodes: &[Node], resolver: &Resolver, diagnostics: &mut Vec<Diagnostic>) -> Vec<Item> {
    let mut flat = Flat::default();
    flatten(nodes, &mut flat);
    imports::rewrite_paths(&mut flat.toks, resolver);
    crate::api::rewrite_calls(&mut flat.toks, &flat.lines, diagnostics);
    let next = next_significant(&flat);
    let scan = scan(&flat, &next);
    // Expansion runs first and removes these; one left over means it gave up
    // on it, and has already said so.
    if let Some(m) = scan.misaligned.first() {
        diagnostics.push(Diagnostic::error(
            flat.chain_lines[&m.0],
            "this `#ifdef` splits a statement in a way that could not be untangled; \
             the output gates the pieces separately and will not compile",
        ));
    }
    emit(&flat, &scan.units)
}

/// The first chain that cuts through a unit, if any.
pub fn find_misaligned(nodes: &[Node]) -> Option<Misaligned> {
    let mut flat = Flat::default();
    flatten(nodes, &mut flat);
    let next = next_significant(&flat);
    let scan = scan(&flat, &next);
    let &(chain, unit_start) = scan.misaligned.first()?;
    let (chain_first, chain_last) = (flat.chain_lines[&chain], flat.chain_end_lines[&chain]);
    let unit = unit_start.and_then(|s| scan.units.iter().find(|u| u.start == s));
    let first_line = unit_start.map_or(chain_first, |s| flat.lines[s].min(chain_first));
    let last_line = unit.map_or(chain_last, |u| flat.lines[u.end].max(chain_last));
    Some(Misaligned { chain, first_line, last_line })
}

#[derive(Clone, Copy, Debug)]
enum Ev {
    Tok(usize),
    ChainStart(usize),
    Branch(usize, usize),
    ChainEnd(usize),
}

#[derive(Default)]
struct Flat {
    toks: Vec<Token>,
    /// Source line of each token.
    lines: Vec<usize>,
    evs: Vec<Ev>,
    chain_conds: BTreeMap<usize, Vec<Cond>>,
    chain_lines: BTreeMap<usize, usize>,
    chain_end_lines: BTreeMap<usize, usize>,
}

fn flatten(nodes: &[Node], flat: &mut Flat) {
    for node in nodes {
        match node {
            Node::Code { text, line } => {
                let mut line = *line;
                for tok in lex::tokenize(text) {
                    flat.evs.push(Ev::Tok(flat.toks.len()));
                    flat.lines.push(line);
                    line += tok.text.matches('\n').count();
                    flat.toks.push(tok);
                }
            }
            Node::Import { .. } => {}
            Node::Chain(chain) => {
                flat.chain_conds
                    .insert(chain.id, chain.branches.iter().map(|b| b.effective.clone()).collect());
                flat.chain_lines.insert(chain.id, chain.line);
                flat.chain_end_lines.insert(chain.id, chain.end_line);
                flat.evs.push(Ev::ChainStart(chain.id));
                for (k, branch) in chain.branches.iter().enumerate() {
                    flat.evs.push(Ev::Branch(chain.id, k));
                    flatten(&branch.nodes, flat);
                }
                flat.evs.push(Ev::ChainEnd(chain.id));
            }
        }
    }
}

/// For each event, the text of the next significant token after it.
fn next_significant(flat: &Flat) -> Vec<Option<String>> {
    let mut out = vec![None; flat.evs.len()];
    let mut next: Option<String> = None;
    for (e, ev) in flat.evs.iter().enumerate().rev() {
        out[e] = next.clone();
        if let Ev::Tok(i) = ev {
            if flat.toks[*i].kind != Kind::Trivia {
                next = Some(flat.toks[*i].text.clone());
            }
        }
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum FrameKind {
    Top,
    Struct,
    Params,
    Block,
    Switch,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Head {
    Unknown,
    Fn,
    Struct,
    If,
    Loop,
    Switch,
    Case,
    Compound,
    Simple,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Await {
    No,
    /// An `if` block just closed; an `else` may continue the statement.
    Else,
    /// A struct just closed; a `;` may follow it.
    Semi,
}

#[derive(Clone, Debug)]
struct Pending {
    start: usize,
    head: Head,
    params_opened: bool,
    cond: Cond,
}

#[derive(Clone, Debug)]
struct Frame {
    kind: FrameKind,
    unit: Option<Pending>,
    depth: u32,
    wait: Await,
    tentative_end: usize,
}

impl Frame {
    fn new(kind: FrameKind) -> Self {
        Frame { kind, unit: None, depth: 0, wait: Await::No, tentative_end: 0 }
    }
}

#[derive(Clone, Debug)]
pub struct Unit {
    start: usize,
    end: usize,
    frame_depth: usize,
    cond: Cond,
}

struct Scan {
    units: Vec<Unit>,
    /// Misaligned chains, with the first token of the unit each one cuts
    /// through (`None` when the chain itself starts that unit).
    misaligned: Vec<(usize, Option<usize>)>,
}

struct Scanner<'a> {
    toks: &'a [Token],
    frames: Vec<Frame>,
    units: Vec<Unit>,
    /// Open `#ifdef` branches: the frame depth they sit at, and their condition.
    open: Vec<(usize, Cond)>,
    last_sig: Option<usize>,
}

struct ChainCtx {
    id: usize,
    snapshot: Vec<Frame>,
    last_sig: Option<usize>,
    misaligned: bool,
    /// Once a chain is known to be misaligned, the scan carries on through the
    /// branch it was in and skips the rest. Continuing from that branch's
    /// state (rather than rewinding) is what finds where the cut unit ends.
    skipping: bool,
}

/// The start of the unit open at the chain's own level, for the report.
fn open_unit_at(s: &Scanner, snapshot: &[Frame]) -> Option<usize> {
    s.frames.get(snapshot.len().saturating_sub(1))?.unit.as_ref().map(|u| u.start)
}

fn scan(flat: &Flat, next: &[Option<String>]) -> Scan {
    let mut s = Scanner {
        toks: &flat.toks,
        frames: vec![Frame::new(FrameKind::Top)],
        units: Vec::new(),
        open: Vec::new(),
        last_sig: None,
    };
    let mut ctxs: Vec<ChainCtx> = Vec::new();
    let mut misaligned: Vec<(usize, Option<usize>)> = Vec::new();

    for (e, ev) in flat.evs.iter().enumerate() {
        match *ev {
            Ev::Tok(i) => {
                if ctxs.iter().any(|c| c.skipping) {
                    continue;
                }
                s.token(i);
            }
            Ev::ChainStart(id) => {
                if ctxs.iter().any(|c| c.skipping) {
                    ctxs.push(ChainCtx { id, snapshot: Vec::new(), last_sig: None, misaligned: true, skipping: true });
                    continue;
                }
                // What could continue a statement left open before the chain:
                // the first token of any branch, or the first token after it.
                let mut followers: Vec<Option<String>> = Vec::new();
                let mut depth = 0;
                for (k, later) in flat.evs.iter().enumerate().skip(e + 1) {
                    match *later {
                        Ev::ChainStart(_) => depth += 1,
                        Ev::ChainEnd(c) if depth == 0 && c == id => {
                            followers.push(next[k].clone());
                            break;
                        }
                        Ev::ChainEnd(_) => depth -= 1,
                        Ev::Branch(c, _) if depth == 0 && c == id => followers.push(next[k].clone()),
                        _ => {}
                    }
                }
                let aligned = s.settle_before_chain(&followers);
                if !aligned {
                    misaligned.push((id, s.frames.last().and_then(|f| f.unit.as_ref()).map(|u| u.start)));
                }
                ctxs.push(ChainCtx {
                    id,
                    snapshot: s.frames.clone(),
                    last_sig: s.last_sig,
                    misaligned: !aligned,
                    skipping: false,
                });
            }
            Ev::Branch(id, k) => {
                let Some(ctx) = ctxs.last_mut() else { continue };
                if ctx.id != id || ctx.snapshot.is_empty() || ctx.skipping {
                    continue;
                }
                if k > 0 {
                    s.open.pop();
                    if !ctx.misaligned && !s.branch_closed_cleanly(&ctx.snapshot, next[e].as_deref()) {
                        ctx.misaligned = true;
                        misaligned.push((id, open_unit_at(&s, &ctx.snapshot)));
                    }
                    if ctx.misaligned {
                        ctx.skipping = true;
                        continue;
                    }
                    s.frames = ctx.snapshot.clone();
                    s.last_sig = ctx.last_sig;
                }
                let cond = flat.chain_conds[&id][k].clone();
                s.open.push((s.frames.len(), cond));
            }
            Ev::ChainEnd(id) => {
                let Some(ctx) = ctxs.pop() else { continue };
                if ctx.snapshot.is_empty() || ctx.skipping {
                    continue;
                }
                s.open.pop();
                if ctx.misaligned {
                    continue;
                }
                if !s.branch_closed_cleanly(&ctx.snapshot, next[e].as_deref()) {
                    misaligned.push((id, open_unit_at(&s, &ctx.snapshot)));
                    continue;
                }
                s.frames = ctx.snapshot;
                s.last_sig = ctx.last_sig;
            }
        }
    }
    // A statement still waiting on a possible `else` at the end of the file.
    s.settle(None);
    while s.frames.len() > 1 {
        s.frames.pop();
    }
    Scan { units: s.units, misaligned }
}

impl Scanner<'_> {
    fn top(&mut self) -> &mut Frame {
        self.frames.last_mut().expect("the top-level frame is never popped")
    }

    fn cond_here(&self) -> Cond {
        let depth = self.frames.len();
        Cond::and(self.open.iter().filter(|(d, _)| *d == depth).map(|(_, c)| c.clone()))
    }

    fn finish(&mut self, end: usize) {
        let frame_depth = self.frames.len();
        let top = self.top();
        top.wait = Await::No;
        if let Some(p) = top.unit.take() {
            self.units.push(Unit { start: p.start, end, frame_depth, cond: p.cond });
        }
    }

    fn finish_unless_empty(&mut self, i: usize) {
        if self.top().unit.as_ref().is_some_and(|u| u.start == i) {
            self.top().unit = None;
        } else {
            self.finish(i);
        }
    }

    /// Close a statement that was only waiting to see whether `else` (or a
    /// struct's `;`) follows, given what can come next.
    fn settle(&mut self, next: Option<&str>) -> bool {
        let top = self.top();
        match top.wait {
            Await::Else if next == Some("else") => false,
            Await::Semi if next == Some(";") => false,
            Await::Else | Await::Semi => {
                let end = top.tentative_end;
                self.finish(end);
                true
            }
            Await::No => true,
        }
    }

    /// At an `#ifdef`: is the code between units, so the chain can be
    /// annotated unit by unit?
    fn settle_before_chain(&mut self, followers: &[Option<String>]) -> bool {
        let wait = self.top().wait;
        if wait != Await::No {
            let blocked = followers.iter().any(|f| match wait {
                Await::Else => f.as_deref() == Some("else"),
                Await::Semi => f.as_deref() == Some(";"),
                Await::No => false,
            });
            if blocked {
                return false;
            }
            self.settle(None);
        }
        let top = self.top();
        top.unit.is_none() && top.depth == 0
    }

    /// At the end of a branch: did it leave the structure as it found it?
    fn branch_closed_cleanly(&mut self, snapshot: &[Frame], next: Option<&str>) -> bool {
        if !self.settle(next) {
            return false;
        }
        // The last struct member before `}` needs no comma, so one that runs
        // to the end of a branch (`_padding: f32` then `#endif`) is complete.
        let top = self.top();
        if matches!(top.kind, FrameKind::Struct | FrameKind::Params) && top.depth == 0 && top.unit.is_some() {
            if let Some(last) = self.last_sig {
                self.finish(last);
            }
        }
        self.frames.len() == snapshot.len()
            && self.frames.iter().zip(snapshot).all(|(a, b)| a.kind == b.kind)
            && self.frames.last().is_some_and(|f| f.unit.is_none() && f.depth == 0)
    }

    fn token(&mut self, i: usize) {
        let tok = &self.toks[i];
        if tok.kind == Kind::Trivia {
            return;
        }
        let text = tok.text.as_str();
        let prev = self.last_sig;

        match self.top().wait {
            Await::Else if text == "else" => {
                self.top().wait = Await::No;
                self.last_sig = Some(i);
                return;
            }
            Await::Semi if text == ";" => {
                self.finish(i);
                self.last_sig = Some(i);
                return;
            }
            Await::Else | Await::Semi => {
                let end = self.top().tentative_end;
                self.finish(end);
            }
            Await::No => {}
        }

        let kind = self.top().kind;
        if self.top().unit.is_none() {
            let cond = self.cond_here();
            let head = if text == "{" && kind == FrameKind::Block { Head::Compound } else { Head::Unknown };
            self.top().unit = Some(Pending { start: i, head, params_opened: false, cond });
        }

        // The keyword that says what kind of unit this is: the first name at
        // depth 0 that is not an attribute's.
        let depth = self.top().depth;
        let after_at = prev.is_some_and(|p| self.toks[p].text == "@");
        if let Some(unit) = self.top().unit.as_mut() {
            if unit.head == Head::Unknown && tok.kind == Kind::Ident && depth == 0 && !(after_at && unit.start < i) {
                unit.head = match text {
                    "fn" => Head::Fn,
                    "struct" => Head::Struct,
                    "if" => Head::If,
                    "for" | "while" | "loop" | "continuing" => Head::Loop,
                    "switch" => Head::Switch,
                    "case" | "default" => Head::Case,
                    _ => Head::Simple,
                };
            }
        }

        let head = self.top().unit.as_ref().map_or(Head::Unknown, |u| u.head);
        // Struct members and parameters are only types and attributes, so a
        // `<` there always opens a template list (`array<T, 2>`), and the
        // comma inside it is not the end of the member. Elsewhere `<` may be a
        // comparison, and no separator that matters can appear inside one.
        let types_only = matches!(kind, FrameKind::Struct | FrameKind::Params);
        match text {
            "<" if types_only => self.top().depth += 1,
            ">" if types_only => {
                let top = self.top();
                top.depth = top.depth.saturating_sub(1);
            }
            "(" | "[" => {
                let opens_params = text == "("
                    && kind == FrameKind::Top
                    && head == Head::Fn
                    && depth == 0
                    && self.top().unit.as_ref().is_some_and(|u| !u.params_opened);
                if opens_params {
                    if let Some(u) = self.top().unit.as_mut() {
                        u.params_opened = true;
                    }
                    self.frames.push(Frame::new(FrameKind::Params));
                } else {
                    self.top().depth += 1;
                }
            }
            ")" | "]" => {
                if kind == FrameKind::Params && depth == 0 && text == ")" {
                    if self.top().unit.as_ref().is_some_and(|u| u.start != i) {
                        self.finish(prev.unwrap_or(i));
                    } else {
                        self.top().unit = None;
                    }
                    self.frames.pop();
                } else {
                    let top = self.top();
                    top.depth = top.depth.saturating_sub(1);
                }
            }
            "{" if depth == 0 => {
                let child = match (kind, head) {
                    (FrameKind::Top, Head::Fn) => Some(FrameKind::Block),
                    (FrameKind::Top, Head::Struct) => Some(FrameKind::Struct),
                    (FrameKind::Block, Head::Switch) => Some(FrameKind::Switch),
                    (FrameKind::Block, _) | (FrameKind::Switch, _) => Some(FrameKind::Block),
                    _ => None,
                };
                match child {
                    Some(k) => self.frames.push(Frame::new(k)),
                    None => self.top().depth += 1,
                }
            }
            "}" if depth == 0 && self.frames.len() > 1 => {
                // The `}` started a unit of its own above; it belongs to the
                // frame it closes, so drop that.
                if self.top().unit.as_ref().is_some_and(|u| u.start == i) {
                    self.top().unit = None;
                } else if self.top().unit.is_some() {
                    self.finish(prev.unwrap_or(i));
                }
                self.frames.pop();
                let parent = self.top();
                let parent_head = parent.unit.as_ref().map_or(Head::Unknown, |u| u.head);
                match (parent.kind, parent_head) {
                    (FrameKind::Top, Head::Struct) => {
                        parent.wait = Await::Semi;
                        parent.tentative_end = i;
                    }
                    (_, Head::If) => {
                        parent.wait = Await::Else;
                        parent.tentative_end = i;
                    }
                    _ => self.finish(i),
                }
            }
            "{" | "}" => {
                let top = self.top();
                if text == "{" {
                    top.depth += 1;
                } else {
                    top.depth = top.depth.saturating_sub(1);
                }
            }
            // A separator that starts a unit is an empty one (`;;`, a stray
            // `,`): nothing to gate, and `@if(X) ;` is not valid WESL.
            ";" if depth == 0 && matches!(kind, FrameKind::Top | FrameKind::Block) => self.finish_unless_empty(i),
            "," if depth == 0 && matches!(kind, FrameKind::Struct | FrameKind::Params) => self.finish_unless_empty(i),
            _ => {}
        }
        self.last_sig = Some(i);
    }
}

fn emit(flat: &Flat, units: &[Unit]) -> Vec<Item> {
    let mut insert: BTreeMap<usize, String> = BTreeMap::new();
    let mut tops: Vec<&Unit> = Vec::new();
    for unit in units {
        if unit.frame_depth == 1 {
            tops.push(unit);
        } else if !unit.cond.is_true() {
            insert.insert(unit.start, format!("@if({}) ", unit.cond));
        }
    }
    tops.sort_by_key(|u| u.start);

    let text = |from: usize, to: usize| -> String {
        let mut s = String::new();
        for i in from..to {
            if let Some(ins) = insert.get(&i) {
                s.push_str(ins);
            }
            s.push_str(&flat.toks[i].text);
        }
        s
    };

    let mut items = Vec::new();
    let mut at = 0;
    for unit in tops {
        if unit.start < at {
            continue;
        }
        items.push(Item {
            cond: unit.cond.clone(),
            lead: text(at, unit.start),
            body: text(unit.start, unit.end + 1),
        });
        at = unit.end + 1;
    }
    if at < flat.toks.len() {
        items.push(Item { cond: Cond::True, lead: text(at, flat.toks.len()), body: String::new() });
    }
    items
}
