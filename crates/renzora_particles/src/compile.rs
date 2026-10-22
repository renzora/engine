//! Graph to IR.
//!
//! The compiler walks backwards from the `output/particle` node, one context at
//! a time, lowering each node an output pin needs into [`crate::ir`]
//! instructions. Four properties are guaranteed, and the tests hold it to each:
//!
//! - **Deterministic.** The same graph gives the same program, instruction for
//!   instruction. Hash maps are used for lookup only; every walk is in sink-pin
//!   order or node-list order.
//! - **Shared subexpressions.** Instructions are hash-consed: emitting one that
//!   already exists returns the existing register. Two outputs of one shape
//!   node, or two nodes reading the same input, cost one evaluation.
//! - **Cycles are rejected**, anywhere in the graph, reachable or not, so the
//!   editor can highlight one before it is wired to anything.
//! - **Unconnected pins read their inline value**, then their catalog default.
//!   An attribute that means nothing in the context being compiled (a particle's
//!   age at birth) reads as zero; see [`Attribute::available_in`].
//!
//! Every error carries the id of the node it is about, so the editor can put the
//! red outline on the right box. Validation and type checking cover the whole
//! graph and report every problem they find, not just the first, for the same
//! reason.

use std::collections::{HashMap, HashSet};
use std::f32::consts::{PI, TAU};

use crate::asset::{NodeId, PinValue, VfxGraph, VfxNode};
use crate::blackbody;
use crate::curve::{CurveTable, GradientTable};
use crate::ir::{
    Attribute, BinaryOp, ColliderIr, ColliderShape, CompiledEmitter, Context, Instr, Op, Program, Reg, SinkPin,
    TernaryOp, UnaryOp, ValueType,
};
use crate::nodes::{node_def, NodeDef, PinDefault, PinKind, PropKind, OUTPUT_NODE, WIND_WOBBLE_HZ};

/// What went wrong, and on which node.
#[derive(Clone, Debug, PartialEq)]
pub struct CompileError {
    /// The node to highlight. `None` only for graph-wide problems (no output).
    pub node: Option<NodeId>,
    pub kind: CompileErrorKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CompileErrorKind {
    /// No `output/particle` node.
    MissingOutput,
    /// More than one `output/particle` node; the error names the extra one.
    MultipleOutputs,
    DuplicateNodeId,
    UnknownNodeType(String),
    /// A connection names a node that is not in the graph.
    DanglingConnection { missing: NodeId },
    UnknownPin(String),
    /// Two connections into one input.
    MultipleConnections(String),
    /// The node is part of a cycle.
    Cycle,
    TypeMismatch {
        pin: String,
        expected: String,
        found: ValueType,
    },
    /// An inline value of the wrong shape for its pin or property.
    BadValue(String),
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(node) = self.node {
            write!(f, "node {node}: ")?;
        }
        match &self.kind {
            CompileErrorKind::MissingOutput => write!(f, "the graph has no Particle Output node"),
            CompileErrorKind::MultipleOutputs => {
                write!(f, "a graph has exactly one Particle Output node; this is an extra one")
            }
            CompileErrorKind::DuplicateNodeId => write!(f, "another node has the same id"),
            CompileErrorKind::UnknownNodeType(t) => write!(f, "unknown node type `{t}`"),
            CompileErrorKind::DanglingConnection { missing } => {
                write!(f, "connected to node {missing}, which does not exist")
            }
            CompileErrorKind::UnknownPin(p) => write!(f, "has no pin named `{p}`"),
            CompileErrorKind::MultipleConnections(p) => {
                write!(f, "input `{p}` has more than one connection")
            }
            CompileErrorKind::Cycle => write!(f, "is part of a cycle"),
            CompileErrorKind::TypeMismatch {
                pin,
                expected,
                found,
            } => write!(
                f,
                "input `{pin}` expects {expected} but is given {}",
                found.name()
            ),
            CompileErrorKind::BadValue(p) => write!(f, "the value of `{p}` has the wrong type"),
        }
    }
}

impl std::error::Error for CompileError {}

/// "No limit" for a cap built from a pin whose zero means off. Large enough
/// that no real force reaches it, small enough that multiplying it by a
/// distance or a step stays finite in f32.
const UNLIMITED: f32 = 1.0e30;

/// A Gravity Well's inputs, lowered once and sampled at more than one place.
struct Well {
    point: Reg,
    strength: Reg,
    /// Softening squared.
    e2: Reg,
    /// Clamped to 0 and up; 0 is no cap.
    max_force: Reg,
}

fn err(node: NodeId, kind: CompileErrorKind) -> CompileError {
    CompileError {
        node: Some(node),
        kind,
    }
}

/// Compile one emitter's graph.
pub fn compile_graph(graph: &VfxGraph) -> Result<CompiledEmitter, Vec<CompileError>> {
    compile_graph_with_params(graph, &[])
}

/// Compile one emitter's graph with `params` (node, pin) read from parameter
/// slots, slot `i` for `params[i]`, instead of from their inline values: the
/// pins an effect's tracks key (see [`crate::params`]). A wired pin keeps its
/// wire, and a pin the graph does not have is ignored.
pub fn compile_graph_with_params(
    graph: &VfxGraph,
    params: &[(NodeId, String)],
) -> Result<CompiledEmitter, Vec<CompileError>> {
    let mut index = GraphIndex::build(graph)?;
    index.params = params.iter().enumerate().map(|(i, (n, p))| ((*n, p.clone()), i as u32)).collect();

    // Type-check every node, not just the reachable ones, so a broken node on
    // the canvas is reported before it is wired in. One builder in the update
    // context: types do not depend on the context.
    let mut tables = Tables::default();
    {
        let mut errors: Vec<CompileError> = Vec::new();
        let mut checker = Builder::new(&index, Context::Update, &mut tables);
        for node in &graph.nodes {
            let Some(def) = node_def(&node.node_type) else {
                continue;
            };
            if def.node_type == OUTPUT_NODE {
                for pin in SinkPin::ALL {
                    if let Err(e) = checker.input(node, def, pin.name()) {
                        if !errors.contains(&e) {
                            errors.push(e);
                        }
                    }
                }
            }
            // A collision node has no outputs to check through, so its
            // inputs are checked directly, the way the sink's are.
            if def.is_collider() {
                if let Err(e) = checker.collider(node, def) {
                    if !errors.contains(&e) {
                        errors.push(e);
                    }
                }
            }
            for out in def.outputs {
                if let Err(e) = checker.output(node.id, out.name) {
                    if !errors.contains(&e) {
                        errors.push(e);
                    }
                }
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
    }
    // The checker interned tables for every node; start the real build clean so
    // a table only the checker touched is not shipped.
    let mut tables = Tables::default();

    let sink = index.output_node;
    let mut programs = Vec::with_capacity(3);
    let mut colliders = Vec::new();
    for ctx in [Context::Spawn, Context::Init, Context::Update] {
        let mut b = Builder::new(&index, ctx, &mut tables);
        let (node, def) = index.nodes[&sink];
        let mut outputs = Vec::new();
        for pin in SinkPin::ALL.into_iter().filter(|p| p.context() == ctx) {
            let reg = b.input(node, def, pin.name()).map_err(|e| vec![e])?;
            outputs.push((pin, reg));
        }
        // Collision nodes are update-context sinks, found in node-list order
        // so the order they are applied in is the order the file lists them.
        if ctx == Context::Update {
            for node in &graph.nodes {
                let Some(&(node, def)) = index.nodes.get(&node.id) else { continue };
                if def.is_collider() {
                    colliders.push(b.collider(node, def).map_err(|e| vec![e])?);
                }
            }
        }
        programs.push(Program {
            instrs: b.instrs,
            outputs,
        });
    }
    let update = programs.pop().unwrap_or_default();
    let init = programs.pop().unwrap_or_default();
    let spawn = programs.pop().unwrap_or_default();
    Ok(CompiledEmitter {
        spawn,
        init,
        update,
        curves: tables.curves,
        gradients: tables.gradients,
        colliders,
    })
}

// ── Graph index and validation ──────────────────────────────────────────────

struct GraphIndex<'a> {
    nodes: HashMap<NodeId, (&'a VfxNode, &'static NodeDef)>,
    /// (to_node, to_pin) -> (from_node, from_pin)
    incoming: HashMap<(NodeId, &'a str), (NodeId, &'a str)>,
    output_node: NodeId,
    /// Keyed pins and their parameter slots.
    params: HashMap<(NodeId, String), u32>,
}

impl<'a> GraphIndex<'a> {
    fn build(graph: &'a VfxGraph) -> Result<Self, Vec<CompileError>> {
        let mut errors = Vec::new();
        let mut nodes = HashMap::new();
        let mut outputs = Vec::new();
        for node in &graph.nodes {
            let Some(def) = node_def(&node.node_type) else {
                errors.push(err(
                    node.id,
                    CompileErrorKind::UnknownNodeType(node.node_type.clone()),
                ));
                continue;
            };
            if nodes.insert(node.id, (node, def)).is_some() {
                errors.push(err(node.id, CompileErrorKind::DuplicateNodeId));
                continue;
            }
            if def.node_type == OUTPUT_NODE {
                outputs.push(node.id);
            }
            for (name, value) in &node.input_values {
                check_value_shape(node.id, def, name, value, &mut errors);
            }
        }
        if outputs.is_empty() {
            errors.push(CompileError {
                node: None,
                kind: CompileErrorKind::MissingOutput,
            });
        }
        for extra in outputs.iter().skip(1) {
            errors.push(err(*extra, CompileErrorKind::MultipleOutputs));
        }

        let mut incoming = HashMap::new();
        for c in &graph.connections {
            let from = nodes.get(&c.from_node);
            let to = nodes.get(&c.to_node);
            match (from, to) {
                (None, None) => continue,
                (None, Some(_)) => {
                    errors.push(err(
                        c.to_node,
                        CompileErrorKind::DanglingConnection {
                            missing: c.from_node,
                        },
                    ));
                    continue;
                }
                (Some(_), None) => {
                    errors.push(err(
                        c.from_node,
                        CompileErrorKind::DanglingConnection { missing: c.to_node },
                    ));
                    continue;
                }
                (Some((_, from_def)), Some((_, to_def))) => {
                    if from_def.output(&c.from_pin).is_none() {
                        errors.push(err(
                            c.from_node,
                            CompileErrorKind::UnknownPin(c.from_pin.clone()),
                        ));
                        continue;
                    }
                    if to_def.input(&c.to_pin).is_none() {
                        errors.push(err(c.to_node, CompileErrorKind::UnknownPin(c.to_pin.clone())));
                        continue;
                    }
                }
            }
            let key = (c.to_node, c.to_pin.as_str());
            if incoming
                .insert(key, (c.from_node, c.from_pin.as_str()))
                .is_some()
            {
                errors.push(err(
                    c.to_node,
                    CompileErrorKind::MultipleConnections(c.to_pin.clone()),
                ));
            }
        }

        if errors.is_empty() {
            if let Some(node) = find_cycle(graph, &incoming) {
                errors.push(err(node, CompileErrorKind::Cycle));
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(Self {
            nodes,
            incoming,
            output_node: outputs[0],
            params: HashMap::new(),
        })
    }
}

/// An inline value or property of the wrong kind for its slot. Values for
/// names the node does not have are ignored: a file from a newer engine with a
/// pin this one lacks should still load.
fn check_value_shape(
    node: NodeId,
    def: &NodeDef,
    name: &str,
    value: &PinValue,
    errors: &mut Vec<CompileError>,
) {
    if let Some(prop) = def.prop(name) {
        let ok = matches!(
            (prop.kind, value),
            (PropKind::Bool(_), PinValue::Bool(_))
                | (PropKind::Curve, PinValue::Curve(_))
                | (PropKind::Gradient, PinValue::Gradient(_))
                | (PropKind::Text(_), PinValue::Text(_))
        );
        if !ok {
            errors.push(err(node, CompileErrorKind::BadValue(name.to_string())));
        }
    } else if let Some(pin) = def.input(name) {
        if inline_value(value, pin.kind).is_none() {
            errors.push(err(node, CompileErrorKind::BadValue(name.to_string())));
        }
    }
}

/// Depth-first search for a cycle over the dependency edges. Iterates nodes in
/// file order so the node it names is deterministic.
fn find_cycle(graph: &VfxGraph, incoming: &HashMap<(NodeId, &str), (NodeId, &str)>) -> Option<NodeId> {
    let mut deps: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    for ((to, _), (from, _)) in incoming {
        deps.entry(*to).or_default().push(*from);
    }
    for list in deps.values_mut() {
        list.sort_unstable();
    }
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        Visiting,
        Done,
    }
    let mut marks: HashMap<NodeId, Mark> = HashMap::new();
    for start in graph.nodes.iter().map(|n| n.id) {
        if marks.contains_key(&start) {
            continue;
        }
        // Explicit stack: (node, next dependency index).
        let mut stack: Vec<(NodeId, usize)> = vec![(start, 0)];
        marks.insert(start, Mark::Visiting);
        while let Some((node, i)) = stack.pop() {
            let next = deps.get(&node).and_then(|d| d.get(i)).copied();
            match next {
                Some(dep) => {
                    stack.push((node, i + 1));
                    match marks.get(&dep) {
                        Some(Mark::Visiting) => return Some(dep),
                        Some(Mark::Done) => {}
                        None => {
                            marks.insert(dep, Mark::Visiting);
                            stack.push((dep, 0));
                        }
                    }
                }
                None => {
                    marks.insert(node, Mark::Done);
                }
            }
        }
    }
    None
}

/// An inline value as `(type, lanes)` for a pin of `kind`, or `None` when it
/// cannot be one. A Float fits any fixed pin (splat); a Vec3 fits a Color pin
/// with alpha 1, because hand-written colours are usually three numbers.
fn inline_value(value: &PinValue, kind: PinKind) -> Option<(ValueType, [f32; 4])> {
    let (ty, lanes) = match value {
        PinValue::Float(v) => (ValueType::Float, [*v; 4]),
        PinValue::Vec2([x, y]) => (ValueType::Vec2, [*x, *y, 0.0, 0.0]),
        PinValue::Vec3([x, y, z]) => (ValueType::Vec3, [*x, *y, *z, 0.0]),
        PinValue::Color(c) => (ValueType::Color, *c),
        _ => return None,
    };
    match kind {
        PinKind::Generic => Some((ty, lanes)),
        PinKind::Fixed(want) if want == ty => Some((ty, lanes)),
        PinKind::Fixed(want) if ty == ValueType::Float => Some((want, lanes)),
        PinKind::Fixed(ValueType::Color) if ty == ValueType::Vec3 => {
            Some((ValueType::Color, [lanes[0], lanes[1], lanes[2], 1.0]))
        }
        PinKind::Fixed(_) => None,
    }
}

// ── Builder ─────────────────────────────────────────────────────────────────

/// Curve and gradient tables, interned by node so a curve read from two
/// contexts is one table.
#[derive(Default)]
struct Tables {
    curves: Vec<CurveTable>,
    curve_index: HashMap<NodeId, u32>,
    gradients: Vec<GradientTable>,
    gradient_index: HashMap<NodeId, u32>,
}

struct Builder<'g, 't> {
    graph: &'g GraphIndex<'g>,
    ctx: Context,
    tables: &'t mut Tables,
    instrs: Vec<Instr>,
    interned: HashMap<Instr, Reg>,
    memo: HashMap<(NodeId, &'static str), Reg>,
    /// Defensive: the global cycle check already ran, but a lowering bug that
    /// recursed forever would be a stack overflow rather than an error.
    visiting: HashSet<NodeId>,
}

impl<'g, 't> Builder<'g, 't> {
    fn new(graph: &'g GraphIndex<'g>, ctx: Context, tables: &'t mut Tables) -> Self {
        Self {
            graph,
            ctx,
            tables,
            instrs: Vec::new(),
            interned: HashMap::new(),
            memo: HashMap::new(),
            visiting: HashSet::new(),
        }
    }

    fn ty(&self, r: Reg) -> ValueType {
        self.instrs[r.0 as usize].ty
    }

    fn emit(&mut self, op: Op, ty: ValueType) -> Reg {
        let instr = Instr { op, ty };
        if let Some(r) = self.interned.get(&instr) {
            return *r;
        }
        let r = Reg(self.instrs.len() as u32);
        self.instrs.push(instr);
        self.interned.insert(instr, r);
        r
    }

    fn constant(&mut self, ty: ValueType, lanes: [f32; 4]) -> Reg {
        // Canonical lanes, so the same value always interns to one register.
        let mut v = [0.0; 4];
        v[..ty.width()].copy_from_slice(&lanes[..ty.width()]);
        self.emit(Op::constant(v), ty)
    }

    fn float(&mut self, v: f32) -> Reg {
        self.constant(ValueType::Float, [v, 0.0, 0.0, 0.0])
    }

    fn attr(&mut self, a: Attribute) -> Reg {
        if a.available_in(self.ctx) {
            self.emit(Op::Attr(a), a.value_type())
        } else {
            self.constant(a.value_type(), [0.0; 4])
        }
    }

    fn unary(&mut self, op: UnaryOp, a: Reg) -> Reg {
        let ty = match op {
            UnaryOp::Length => ValueType::Float,
            UnaryOp::UnitSphere | UnaryOp::UnitCircle | UnaryOp::Curl => ValueType::Vec3,
            _ => self.ty(a),
        };
        self.emit(Op::Unary(op, a), ty)
    }

    /// Binary op on two registers already of one type (or Float with anything
    /// for Mul/Div, which the interpreter treats lane-wise after a splat).
    fn binary(&mut self, op: BinaryOp, a: Reg, b: Reg) -> Reg {
        let (a, b, ty) = self.unify2(a, b);
        let ty = match op {
            BinaryOp::Dot => ValueType::Float,
            _ => ty,
        };
        self.emit(Op::Binary(op, a, b), ty)
    }

    fn extract(&mut self, a: Reg, lane: u8) -> Reg {
        self.emit(Op::Extract(a, lane), ValueType::Float)
    }

    fn compose3(&mut self, x: Reg, y: Reg, z: Reg) -> Reg {
        self.emit(Op::Compose3(x, y, z), ValueType::Vec3)
    }

    fn splat(&mut self, a: Reg, to: ValueType) -> Reg {
        if self.ty(a) == to {
            a
        } else {
            self.emit(Op::Unary(UnaryOp::Splat, a), to)
        }
    }

    /// Internal unify for operands the lowering itself produced, where a
    /// mismatch would be a bug in this file rather than in the user's graph:
    /// splat a Float side, otherwise keep the left type.
    fn unify2(&mut self, a: Reg, b: Reg) -> (Reg, Reg, ValueType) {
        let (ta, tb) = (self.ty(a), self.ty(b));
        let ty = ta.unify(tb).unwrap_or(ta);
        (self.splat(a, ty), self.splat(b, ty), ty)
    }

    /// Unify a node's generic inputs, reporting a mismatch against the node.
    fn unify_inputs(&mut self, node: NodeId, pins: &[(&str, Reg)]) -> Result<(ValueType, Vec<Reg>), CompileError> {
        let mut ty = ValueType::Float;
        for (pin, r) in pins {
            let found = self.ty(*r);
            ty = match ty.unify(found) {
                Some(u) => u,
                None => {
                    return Err(err(
                        node,
                        CompileErrorKind::TypeMismatch {
                            pin: pin.to_string(),
                            expected: format!("{} (or Float)", ty.name()),
                            found,
                        },
                    ));
                }
            };
        }
        let regs = pins.iter().map(|(_, r)| self.splat(*r, ty)).collect();
        Ok((ty, regs))
    }

    /// An input pin's value, coerced to the pin's type for a fixed pin.
    fn input(&mut self, node: &VfxNode, def: &NodeDef, pin: &str) -> Result<Reg, CompileError> {
        let Some(pin_def) = def.input(pin) else {
            return Err(err(node.id, CompileErrorKind::UnknownPin(pin.to_string())));
        };
        if let Some(&(from, from_pin)) = self.graph.incoming.get(&(node.id, pin)) {
            let src = self.output(from, from_pin)?;
            return match pin_def.kind {
                PinKind::Generic => Ok(src),
                PinKind::Fixed(want) => {
                    let found = self.ty(src);
                    if found == want {
                        Ok(src)
                    } else if found == ValueType::Float {
                        Ok(self.splat(src, want))
                    } else {
                        Err(err(
                            node.id,
                            CompileErrorKind::TypeMismatch {
                                pin: pin.to_string(),
                                expected: want.name().to_string(),
                                found,
                            },
                        ))
                    }
                }
            };
        }
        // A keyed pin reads its slot. Its type is the one the inline value
        // would have had, so keying a pin never changes what it type-checks
        // as: the tracks only change the number.
        if let Some(&slot) = self.graph.params.get(&(node.id, pin.to_string())) {
            let ty = node
                .input_values
                .get(pin)
                .and_then(|v| inline_value(v, pin_def.kind))
                .map(|(ty, _)| ty)
                .unwrap_or(match pin_def.kind {
                    PinKind::Fixed(ty) => ty,
                    PinKind::Generic => ValueType::Float,
                });
            return Ok(self.emit(Op::Param(slot), ty));
        }
        if let Some(value) = node.input_values.get(pin) {
            return match inline_value(value, pin_def.kind) {
                Some((ty, lanes)) => Ok(self.constant(ty, lanes)),
                None => Err(err(node.id, CompileErrorKind::BadValue(pin.to_string()))),
            };
        }
        Ok(match (pin_def.default, pin_def.kind) {
            (PinDefault::Attr(a), _) => self.attr(a),
            (PinDefault::Value(v), PinKind::Fixed(ty)) => self.constant(ty, v),
            (PinDefault::Value(v), PinKind::Generic) => self.float(v[0]),
        })
    }

    fn bool_prop(node: &VfxNode, def: &NodeDef, name: &str) -> bool {
        match node.input_values.get(name) {
            Some(PinValue::Bool(b)) => *b,
            _ => match def.prop(name).map(|p| p.kind) {
                Some(PropKind::Bool(b)) => b,
                _ => false,
            },
        }
    }

    /// Lower a collision node's inputs; see [`ColliderIr`]. A zero plane
    /// normal is left for the simulation to skip rather than normalised to
    /// NaN here, since it may come from a wire and differ per particle.
    fn collider(&mut self, node: &VfxNode, def: &NodeDef) -> Result<ColliderIr, CompileError> {
        let shape = match def.node_type {
            "collide/plane" => ColliderShape::Plane,
            "collide/sphere" => ColliderShape::Sphere,
            "collide/box" => ColliderShape::Box,
            "collide/scene" => ColliderShape::Scene,
            other => return Err(err(node.id, CompileErrorKind::UnknownNodeType(other.to_string()))),
        };
        let (a, b) = match shape {
            ColliderShape::Plane => (self.input(node, def, "point")?, self.input(node, def, "normal")?),
            ColliderShape::Sphere => (self.input(node, def, "center")?, self.input(node, def, "radius")?),
            ColliderShape::Box => (self.input(node, def, "center")?, self.input(node, def, "size")?),
            // The scene has no geometry of its own to place; only the
            // particle's size travels.
            ColliderShape::Scene => (self.constant(ValueType::Vec3, [0.0; 4]), self.input(node, def, "radius")?),
        };
        Ok(ColliderIr {
            node: node.id,
            shape,
            contain: matches!(shape, ColliderShape::Sphere | ColliderShape::Box) && Self::bool_prop(node, def, "inside"),
            kill: Self::bool_prop(node, def, "kill"),
            // The scene's colliders are where they are in the world, whatever
            // the flag says; the node has none to set.
            world: shape == ColliderShape::Scene || Self::bool_prop(node, def, "world"),
            a,
            b,
            bounce: self.input(node, def, "bounce")?,
            friction: self.input(node, def, "friction")?,
        })
    }

    /// A per-node random stream, never 0 (which `input/particle_random` owns,
    /// so that two of those with one salt agree, which is the point of a salt).
    fn stream(node: NodeId) -> u32 {
        let mut h = (node as u32) ^ ((node >> 32) as u32) ^ 0x6A09_E667;
        h ^= h >> 16;
        h = h.wrapping_mul(0x7FEB_352D);
        h ^= h >> 15;
        h | 1
    }

    fn random(&mut self, salt: Reg, stream: u32) -> Reg {
        self.emit(Op::Random { salt, stream }, ValueType::Color)
    }

    /// The world wind at a particle, scaled, with its turbulent wobble.
    ///
    /// The wobble is a cross-wind (horizontal, perpendicular to the wind) and a
    /// smaller vertical push, each a sum of two sines of the particle's age at
    /// incommensurate rates with phases from the particle's own random. That
    /// is deterministic per particle and time, costs a handful of ops, and
    /// replays exactly under a seek; a noise field would cost a curl per
    /// particle and add nothing a viewer can tell apart from it. Its size is
    /// the turbulence times the wind speed, so a calm day stays calm.
    ///
    /// One fixed random stream for every wind node, so two of them in one
    /// graph agree on a particle's gusts. Age is zero at birth, which makes the
    /// init-context value a fixed per-particle offset; that is the right
    /// reading of "the wind it was born into".
    fn wind_velocity(&mut self, scale: Reg, hz: Reg) -> Reg {
        const WIND_STREAM: u32 = 0x5749_4E45;
        let base = self.attr(Attribute::Wind);
        let base = self.binary(BinaryOp::Mul, base, scale);
        let turbulence = self.attr(Attribute::WindTurbulence);
        let speed = self.unary(UnaryOp::Length, base);
        let up = self.constant(ValueType::Vec3, [0.0, 1.0, 0.0, 0.0]);
        // up x wind: horizontal, at right angles to the wind, as long as it.
        let side = self.binary(BinaryOp::Cross, up, base);
        let zero = self.float(0.0);
        let rand = self.random(zero, WIND_STREAM);
        let age = self.attr(Attribute::Age);
        let tau = self.float(TAU);
        let wave = |b: &mut Self, rate: f32, lane: u8| {
            let rate = b.float(rate);
            let f = b.binary(BinaryOp::Mul, hz, rate);
            let cycles = b.binary(BinaryOp::Mul, f, age);
            let phase = b.extract(rand, lane);
            let turns = b.binary(BinaryOp::Add, cycles, phase);
            let angle = b.binary(BinaryOp::Mul, turns, tau);
            b.unary(UnaryOp::Sin, angle)
        };
        let s1 = wave(self, 1.0, 0);
        let s2 = wave(self, 2.37, 1);
        let s3 = wave(self, 1.61, 2);
        let w1 = self.float(0.65);
        let w2 = self.float(0.35);
        let a = self.binary(BinaryOp::Mul, s1, w1);
        let b = self.binary(BinaryOp::Mul, s2, w2);
        let lateral = self.binary(BinaryOp::Add, a, b);
        let lateral = self.binary(BinaryOp::Mul, lateral, turbulence);
        let side = self.binary(BinaryOp::Mul, side, lateral);
        let lift = self.float(0.3);
        let vertical = self.binary(BinaryOp::Mul, s3, lift);
        let vertical = self.binary(BinaryOp::Mul, vertical, turbulence);
        let vertical = self.binary(BinaryOp::Mul, vertical, speed);
        let vertical = self.binary(BinaryOp::Mul, up, vertical);
        let gusty = self.binary(BinaryOp::Add, base, side);
        self.binary(BinaryOp::Add, gusty, vertical)
    }

    /// `k / (1 + k dt)`: the rate that makes an explicit step of a relaxation
    /// `dv/dt = -k v` land where the implicit one would. Shared by every
    /// drag-like force so none of them overshoots on a long frame.
    fn implicit_rate(&mut self, k: Reg) -> Reg {
        let dt = self.attr(Attribute::DeltaTime);
        let kdt = self.binary(BinaryOp::Mul, k, dt);
        let one = self.float(1.0);
        let den = self.binary(BinaryOp::Add, one, kdt);
        self.binary(BinaryOp::Div, k, den)
    }

    /// `1 / dt²`: the stiffest pull `k * d` toward a point `d` away that one
    /// step can apply without carrying a particle past the point. The step
    /// adds `k d dt` to the velocity and then moves by it for `dt`, so it
    /// closes `k d dt²` of the `d`; capped here it closes at most all of it.
    /// That is what keeps an attractor from slingshotting a particle through
    /// its centre on a long frame: past the cap the pull stops growing with
    /// stiffness, and below it the explicit step is the stable side of
    /// `k dt² < 4`, whatever the frame rate.
    ///
    /// A context with no step (the init context, where `dt` is 0) has nothing
    /// to overshoot, so the cap is lifted there rather than collapsing to the
    /// zero a division by zero gives.
    fn stiffness_cap(&mut self) -> Reg {
        let dt = self.attr(Attribute::DeltaTime);
        let dt2 = self.binary(BinaryOp::Mul, dt, dt);
        self.per_step_cap(dt2)
    }

    /// `1 / per_step`, or no limit when `per_step` is 0 (no step at all).
    fn per_step_cap(&mut self, per_step: Reg) -> Reg {
        let one = self.float(1.0);
        let cap = self.binary(BinaryOp::Div, one, per_step);
        let tiny = self.float(1e-12);
        let stepping = self.emit(Op::Ternary(TernaryOp::Smoothstep, tiny, tiny, per_step), ValueType::Float);
        let idle = self.binary(BinaryOp::Sub, one, stepping);
        let unlimited = self.float(UNLIMITED);
        let lift = self.binary(BinaryOp::Mul, idle, unlimited);
        self.binary(BinaryOp::Add, cap, lift)
    }

    /// A Gravity Well's pull at `at`: Plummer's softened gravity,
    /// `strength * d / (r² + e²)^1.5`. It is the inverse square far out and
    /// falls to zero at the centre instead of rising to infinity, which is
    /// the singularity that flings a particle out of a plain `1 / r²`. Worked
    /// as a stiffness `k` (pull per metre of offset) so both caps are one
    /// clamp on one number: Max Force (`|k| r <= F`) and, for a pull, the step
    /// cap ([`Builder::stiffness_cap`]); a push cannot overshoot.
    fn well_pull(&mut self, w: &Well, at: Reg) -> Reg {
        let d = self.binary(BinaryOp::Sub, w.point, at);
        let r2 = self.binary(BinaryOp::Dot, d, d);
        let s = self.binary(BinaryOp::Add, r2, w.e2);
        let root = self.unary(UnaryOp::Sqrt, s);
        let s15 = self.binary(BinaryOp::Mul, s, root);
        let k = self.binary(BinaryOp::Div, w.strength, s15);
        // 0 is no cap.
        let r = self.unary(UnaryOp::Sqrt, r2);
        let per_metre = self.binary(BinaryOp::Div, w.max_force, r);
        let on = self.is_on(w.max_force);
        let one = self.float(1.0);
        let off = self.binary(BinaryOp::Sub, one, on);
        let unlimited = self.float(UNLIMITED);
        let lift = self.binary(BinaryOp::Mul, off, unlimited);
        let k_force = self.binary(BinaryOp::Add, per_metre, lift);
        let cap = self.stiffness_cap();
        let hi = self.binary(BinaryOp::Min, k_force, cap);
        let lo = self.unary(UnaryOp::Neg, k_force);
        let k = self.emit(Op::Ternary(TernaryOp::Clamp, k, lo, hi), ValueType::Float);
        self.binary(BinaryOp::Mul, d, k)
    }

    /// The well's potential at `at`, `-strength / sqrt(r² + e²)`: the energy
    /// [`Builder::well_pull`]'s pull is the slope of.
    fn well_potential(&mut self, w: &Well, at: Reg) -> Reg {
        let d = self.binary(BinaryOp::Sub, w.point, at);
        let r2 = self.binary(BinaryOp::Dot, d, d);
        let s = self.binary(BinaryOp::Add, r2, w.e2);
        let root = self.unary(UnaryOp::Sqrt, s);
        let phi = self.binary(BinaryOp::Div, w.strength, root);
        self.unary(UnaryOp::Neg, phi)
    }

    /// `1` where `v` is at least a hair above zero, else `0`: how a pin whose
    /// zero means "off" (a Max Force, a ring Radius) switches without a
    /// branch. The IR's smoothstep with equal edges is exactly that step.
    fn is_on(&mut self, v: Reg) -> Reg {
        let tiny = self.float(1e-6);
        self.emit(Op::Ternary(TernaryOp::Smoothstep, tiny, tiny, v), ValueType::Float)
    }

    /// The register for one output pin of a node.
    fn output(&mut self, node_id: NodeId, pin: &str) -> Result<Reg, CompileError> {
        let Some(&(node, def)) = self.graph.nodes.get(&node_id) else {
            return Err(err(node_id, CompileErrorKind::UnknownPin(pin.to_string())));
        };
        let Some(out) = def.output(pin) else {
            return Err(err(node_id, CompileErrorKind::UnknownPin(pin.to_string())));
        };
        if let Some(r) = self.memo.get(&(node_id, out.name)) {
            return Ok(*r);
        }
        if !self.visiting.insert(node_id) {
            return Err(err(node_id, CompileErrorKind::Cycle));
        }
        let result = self.lower(node, def, out.name);
        self.visiting.remove(&node_id);
        let r = result?;
        self.memo.insert((node_id, out.name), r);
        Ok(r)
    }

    /// What each node computes. One arm per catalog entry; the catalog test
    /// checks that every entry lowers, so a node added there and forgotten here
    /// fails loudly.
    fn lower(&mut self, node: &VfxNode, def: &NodeDef, out: &str) -> Result<Reg, CompileError> {
        let id = node.id;
        macro_rules! inp {
            ($name:expr) => {
                self.input(node, def, $name)?
            };
        }
        let r = match def.node_type {
            OUTPUT_NODE => return Err(err(id, CompileErrorKind::UnknownPin(out.to_string()))),

            // ── Shapes ──
            "shape/point" => {
                let zero = self.float(0.0);
                let rand = self.random(zero, Self::stream(id));
                match out {
                    "position" => self.constant(ValueType::Vec3, [0.0; 4]),
                    _ => self.unary(UnaryOp::UnitSphere, rand),
                }
            }
            "shape/sphere" => {
                let radius = inp!("radius");
                let zero = self.float(0.0);
                let rand = self.random(zero, Self::stream(id));
                let dir = self.unary(UnaryOp::UnitSphere, rand);
                if out == "direction" {
                    dir
                } else if Self::bool_prop(node, def, "surface") {
                    self.binary(BinaryOp::Mul, dir, radius)
                } else {
                    // Cube root, so points are uniform over the volume rather
                    // than crowded at the centre.
                    let u = self.extract(rand, 2);
                    let c = self.unary(UnaryOp::Cbrt, u);
                    let scale = self.binary(BinaryOp::Mul, radius, c);
                    self.binary(BinaryOp::Mul, dir, scale)
                }
            }
            "shape/box" => {
                let size = inp!("size");
                let zero = self.float(0.0);
                let rand = self.random(zero, Self::stream(id));
                let (x, y, z) = (self.extract(rand, 0), self.extract(rand, 1), self.extract(rand, 2));
                let r3 = self.compose3(x, y, z);
                let half = self.float(0.5);
                let centred = self.binary(BinaryOp::Sub, r3, half);
                let pos = self.binary(BinaryOp::Mul, centred, size);
                match out {
                    "position" => pos,
                    _ => self.unary(UnaryOp::Normalize, pos),
                }
            }
            "shape/cone" => {
                let zero = self.float(0.0);
                let rand = self.random(zero, Self::stream(id));
                if out == "direction" {
                    let angle = inp!("angle");
                    let to_rad = self.float(PI / 180.0);
                    let rad = self.binary(BinaryOp::Mul, angle, to_rad);
                    let (u, v) = (self.extract(rand, 0), self.extract(rand, 1));
                    self.emit(
                        Op::Ternary(TernaryOp::ConeDirection, rad, u, v),
                        ValueType::Vec3,
                    )
                } else {
                    let radius = inp!("radius");
                    let a = self.extract(rand, 2);
                    let circle = self.unary(UnaryOp::UnitCircle, a);
                    let d = self.extract(rand, 3);
                    let sq = self.unary(UnaryOp::Sqrt, d);
                    let dist = self.binary(BinaryOp::Mul, radius, sq);
                    let cx = self.extract(circle, 0);
                    let cy = self.extract(circle, 1);
                    let x = self.binary(BinaryOp::Mul, cx, dist);
                    let z = self.binary(BinaryOp::Mul, cy, dist);
                    let y = self.float(0.0);
                    self.compose3(x, y, z)
                }
            }
            "shape/circle" => {
                let radius = inp!("radius");
                let zero = self.float(0.0);
                let rand = self.random(zero, Self::stream(id));
                let a = self.extract(rand, 0);
                let circle = self.unary(UnaryOp::UnitCircle, a);
                if out == "direction" {
                    circle
                } else if Self::bool_prop(node, def, "edge") {
                    self.binary(BinaryOp::Mul, circle, radius)
                } else {
                    // Square root: uniform over the disc's area.
                    let u = self.extract(rand, 1);
                    let sq = self.unary(UnaryOp::Sqrt, u);
                    let dist = self.binary(BinaryOp::Mul, radius, sq);
                    self.binary(BinaryOp::Mul, circle, dist)
                }
            }
            "shape/rect" => {
                let size = inp!("size");
                let zero = self.float(0.0);
                let rand = self.random(zero, Self::stream(id));
                let half = self.float(0.5);
                let rx = self.extract(rand, 0);
                let ry = self.extract(rand, 1);
                let cx = self.binary(BinaryOp::Sub, rx, half);
                let cy = self.binary(BinaryOp::Sub, ry, half);
                let sx = self.extract(size, 0);
                let sy = self.extract(size, 1);
                let x = self.binary(BinaryOp::Mul, cx, sx);
                let y = self.binary(BinaryOp::Mul, cy, sy);
                let pos = self.compose3(x, y, zero);
                match out {
                    "position" => pos,
                    _ => self.unary(UnaryOp::Normalize, pos),
                }
            }

            // ── Inputs ──
            "input/age" => self.attr(Attribute::Age),
            "input/life" => self.attr(Attribute::Life),
            "input/lifetime" => self.attr(Attribute::Lifetime),
            "input/position" => self.attr(Attribute::Position),
            "input/velocity" => self.attr(Attribute::Velocity),
            "input/index" => self.attr(Attribute::Index),
            "input/emitter_time" => self.attr(Attribute::EmitterTime),
            "input/delta_time" => self.attr(Attribute::DeltaTime),
            "input/emitter_velocity" => self.attr(Attribute::EmitterVelocity),
            "input/wind" => {
                let (scale, hz) = (inp!("scale"), inp!("frequency"));
                if out == "strength" {
                    let base = self.attr(Attribute::Wind);
                    let scaled = self.binary(BinaryOp::Mul, base, scale);
                    let speed = self.unary(UnaryOp::Length, scaled);
                    let per_ref = self.float(1.0 / renzora::wind::REFERENCE_WIND_SPEED);
                    self.binary(BinaryOp::Mul, speed, per_ref)
                } else {
                    self.wind_velocity(scale, hz)
                }
            }
            // Its value is its own `value` pin, which `crate::params` makes a
            // parameter slot of: the script's value when one has been set,
            // else the inline one. Lowered as a plain input so a graph
            // compiled without parameters (a test, the editor's checker) still
            // reads the inline value.
            "input/parameter" => inp!("value"),
            "input/particle_random" => {
                let salt = inp!("salt");
                let rand = self.random(salt, 0);
                self.extract(rand, 0)
            }

            // ── Random ──
            "random/range" => {
                let (min, max, salt) = (inp!("min"), inp!("max"), inp!("salt"));
                let rand = self.random(salt, Self::stream(id));
                let t = self.extract(rand, 0);
                self.emit(Op::Ternary(TernaryOp::Lerp, min, max, t), ValueType::Float)
            }
            "random/vec3_range" => {
                let (min, max, salt) = (inp!("min"), inp!("max"), inp!("salt"));
                let rand = self.random(salt, Self::stream(id));
                let (x, y, z) = (self.extract(rand, 0), self.extract(rand, 1), self.extract(rand, 2));
                let t = self.compose3(x, y, z);
                self.emit(Op::Ternary(TernaryOp::Lerp, min, max, t), ValueType::Vec3)
            }
            "random/unit_vector" => {
                let salt = inp!("salt");
                let rand = self.random(salt, Self::stream(id));
                self.unary(UnaryOp::UnitSphere, rand)
            }
            "random/color_between" => {
                let (a, b, salt) = (inp!("a"), inp!("b"), inp!("salt"));
                let rand = self.random(salt, Self::stream(id));
                let t = self.extract(rand, 0);
                let t = self.splat(t, ValueType::Color);
                self.emit(Op::Ternary(TernaryOp::Lerp, a, b, t), ValueType::Color)
            }

            // ── Math ──
            "math/float" | "math/vec3" | "math/color" => inp!("value"),
            "math/add" | "math/subtract" | "math/multiply" | "math/divide" | "math/min"
            | "math/max" => {
                let (a, b) = (inp!("a"), inp!("b"));
                let (ty, regs) = self.unify_inputs(id, &[("a", a), ("b", b)])?;
                let op = match def.node_type {
                    "math/add" => BinaryOp::Add,
                    "math/subtract" => BinaryOp::Sub,
                    "math/multiply" => BinaryOp::Mul,
                    "math/divide" => BinaryOp::Div,
                    "math/min" => BinaryOp::Min,
                    _ => BinaryOp::Max,
                };
                self.emit(Op::Binary(op, regs[0], regs[1]), ty)
            }
            "math/dot" => {
                let (a, b) = (inp!("a"), inp!("b"));
                let (_, regs) = self.unify_inputs(id, &[("a", a), ("b", b)])?;
                self.emit(Op::Binary(BinaryOp::Dot, regs[0], regs[1]), ValueType::Float)
            }
            "math/lerp" => {
                let (a, b, t) = (inp!("a"), inp!("b"), inp!("t"));
                let (ty, r) = self.unify_inputs(id, &[("a", a), ("b", b), ("t", t)])?;
                self.emit(Op::Ternary(TernaryOp::Lerp, r[0], r[1], r[2]), ty)
            }
            "math/clamp" => {
                let (v, lo, hi) = (inp!("value"), inp!("min"), inp!("max"));
                let (ty, r) = self.unify_inputs(id, &[("value", v), ("min", lo), ("max", hi)])?;
                self.emit(Op::Ternary(TernaryOp::Clamp, r[0], r[1], r[2]), ty)
            }
            "math/smoothstep" => {
                let (e0, e1, v) = (inp!("edge0"), inp!("edge1"), inp!("value"));
                let (ty, r) =
                    self.unify_inputs(id, &[("edge0", e0), ("edge1", e1), ("value", v)])?;
                self.emit(Op::Ternary(TernaryOp::Smoothstep, r[0], r[1], r[2]), ty)
            }
            "math/remap" => {
                let pins = [
                    ("value", inp!("value")),
                    ("in_min", inp!("in_min")),
                    ("in_max", inp!("in_max")),
                    ("out_min", inp!("out_min")),
                    ("out_max", inp!("out_max")),
                ];
                let (ty, r) = self.unify_inputs(id, &pins)?;
                let num = self.emit(Op::Binary(BinaryOp::Sub, r[0], r[1]), ty);
                let den = self.emit(Op::Binary(BinaryOp::Sub, r[2], r[1]), ty);
                let t = self.emit(Op::Binary(BinaryOp::Div, num, den), ty);
                self.emit(Op::Ternary(TernaryOp::Lerp, r[3], r[4], t), ty)
            }
            "math/abs" | "math/negate" | "math/sin" | "math/cos" | "math/normalize"
            | "math/length" => {
                let v = inp!("value");
                let op = match def.node_type {
                    "math/abs" => UnaryOp::Abs,
                    "math/negate" => UnaryOp::Neg,
                    "math/sin" => UnaryOp::Sin,
                    "math/cos" => UnaryOp::Cos,
                    "math/normalize" => UnaryOp::Normalize,
                    _ => UnaryOp::Length,
                };
                self.unary(op, v)
            }
            "math/compose_vec3" => {
                let (x, y, z) = (inp!("x"), inp!("y"), inp!("z"));
                self.compose3(x, y, z)
            }
            "math/split_vec3" => {
                let v = inp!("value");
                let lane = match out {
                    "x" => 0,
                    "y" => 1,
                    _ => 2,
                };
                self.extract(v, lane)
            }

            // ── Curves ──
            "curve/curve" => {
                let t = inp!("t");
                let index = match self.tables.curve_index.get(&id) {
                    Some(i) => *i,
                    None => {
                        let points = match node.input_values.get("points") {
                            Some(PinValue::Curve(p)) => p.as_slice(),
                            _ => &[],
                        };
                        let i = self.tables.curves.len() as u32;
                        self.tables.curves.push(CurveTable::new(points));
                        self.tables.curve_index.insert(id, i);
                        i
                    }
                };
                self.emit(Op::Curve { index, t }, ValueType::Float)
            }
            "curve/gradient" => {
                let t = inp!("t");
                let index = match self.tables.gradient_index.get(&id) {
                    Some(i) => *i,
                    None => {
                        let stops = match node.input_values.get("stops") {
                            Some(PinValue::Gradient(s)) => s.as_slice(),
                            _ => &[],
                        };
                        let i = self.tables.gradients.len() as u32;
                        self.tables.gradients.push(GradientTable::new(stops));
                        self.tables.gradient_index.insert(id, i);
                        i
                    }
                };
                self.emit(Op::Gradient { index, t }, ValueType::Color)
            }

            // ── Forces ──
            "force/curl_noise" => {
                let (p, strength, freq, scroll) =
                    (inp!("position"), inp!("strength"), inp!("frequency"), inp!("scroll"));
                let time = self.attr(Attribute::EmitterTime);
                let offset = self.binary(BinaryOp::Mul, scroll, time);
                let scaled = self.binary(BinaryOp::Mul, p, freq);
                let sample = self.binary(BinaryOp::Add, scaled, offset);
                let octaves = inp!("octaves");
                // One plain octave stays the single-curl op it always was, so
                // every effect written before octaves existed compiles to the
                // very same program.
                let one = Op::constant([1.0, 0.0, 0.0, 0.0]);
                let curl = if self.instrs[octaves.0 as usize].op == one {
                    self.unary(UnaryOp::Curl, sample)
                } else {
                    let (lac, pers) = (inp!("lacunarity"), inp!("persistence"));
                    let shape = self.emit(Op::Compose2(lac, pers), ValueType::Vec2);
                    self.emit(Op::Ternary(TernaryOp::CurlFbm, sample, octaves, shape), ValueType::Vec3)
                };
                self.binary(BinaryOp::Mul, curl, strength)
            }
            "force/speed_limit" => {
                let (v, limit, rate) = (inp!("velocity"), inp!("limit"), inp!("rate"));
                // Brake only the excess: max(|v| - limit, 0) along -v, at the
                // implicit rate k / (1 + k dt), so a firm rate settles onto
                // the limit in one step instead of overshooting below it.
                let zero = self.float(0.0);
                let speed = self.unary(UnaryOp::Length, v);
                let limit = self.binary(BinaryOp::Max, limit, zero);
                let excess = self.binary(BinaryOp::Sub, speed, limit);
                let excess = self.binary(BinaryOp::Max, excess, zero);
                let dir = self.unary(UnaryOp::Normalize, v);
                let k = self.binary(BinaryOp::Max, rate, zero);
                let k_eff = self.implicit_rate(k);
                let mag = self.binary(BinaryOp::Mul, excess, k_eff);
                let brake = self.binary(BinaryOp::Mul, dir, mag);
                self.unary(UnaryOp::Neg, brake)
            }
            "force/drag" => {
                let (v, linear, quadratic) = (inp!("velocity"), inp!("linear"), inp!("quadratic"));
                // k = linear + quadratic |v|, applied implicitly like the
                // output's drag: quadratic drag is stiff at speed, and an
                // explicit step of it flings a fast particle backwards.
                let zero = self.float(0.0);
                let speed = self.unary(UnaryOp::Length, v);
                let linear = self.binary(BinaryOp::Max, linear, zero);
                let quadratic = self.binary(BinaryOp::Max, quadratic, zero);
                let q = self.binary(BinaryOp::Mul, quadratic, speed);
                let k = self.binary(BinaryOp::Add, linear, q);
                let k_eff = self.implicit_rate(k);
                let pull = self.binary(BinaryOp::Mul, v, k_eff);
                self.unary(UnaryOp::Neg, pull)
            }
            "force/attractor" => {
                let (p, point, strength, falloff) =
                    (inp!("position"), inp!("point"), inp!("strength"), inp!("falloff"));
                let d = self.binary(BinaryOp::Sub, point, p);
                let dist2 = self.binary(BinaryOp::Dot, d, d);
                let dir = self.unary(UnaryOp::Normalize, d);
                let one = self.float(1.0);
                let weak = self.binary(BinaryOp::Mul, falloff, dist2);
                let den = self.binary(BinaryOp::Add, one, weak);
                let mag = self.binary(BinaryOp::Div, strength, den);
                // A pull of constant size toward a point overshoots it on any
                // step that would close the distance, and then flips every
                // step after: the jitter a particle used to sit in at the
                // centre. Capped at `dist / dt²` (see `stiffness_cap`) it
                // lands on the point instead. Only the pull is capped: a push
                // (negative strength) cannot overshoot, and must still move a
                // particle born on the point.
                let dist = self.unary(UnaryOp::Sqrt, dist2);
                let cap = self.stiffness_cap();
                let reach = self.binary(BinaryOp::Mul, dist, cap);
                let mag = self.binary(BinaryOp::Min, mag, reach);
                self.binary(BinaryOp::Mul, dir, mag)
            }
            "force/gravity_well" => {
                let (p, v, point, strength, softening, max_force) =
                    (inp!("position"), inp!("velocity"), inp!("point"), inp!("strength"), inp!("softening"), inp!("max_force"));
                let dt = self.attr(Attribute::DeltaTime);
                let e2 = self.binary(BinaryOp::Mul, softening, softening);
                let zero = self.float(0.0);
                let max_force = self.binary(BinaryOp::Max, max_force, zero);
                let well = Well { point, strength, e2, max_force };
                // The output's integration (velocity first, then position) is
                // symplectic: on motion the step resolves, a particle in the
                // well keeps its energy and swings out as far as it fell, pass
                // after pass. What it cannot resolve is a core crossed in less
                // than a step. There the pull is sampled once beside the
                // centre on the way in and once further out on the way back,
                // and the difference is energy from nowhere: the slingshot.
                let a = self.well_pull(&well, p);
                // So on such a step, and only there, the step may not leave
                // the particle with more energy in the well (speed squared
                // over two plus the well's potential) than it started with:
                // the new velocity is shortened to the speed that energy
                // allows at the new place. A particle let go at rest can then
                // never climb out past where it started. On resolved steps
                // the limit is off, because clipping the small rise every
                // symplectic step makes on half its swing would bleed a
                // well-behaved orbit dry.
                let dv = self.binary(BinaryOp::Mul, a, dt);
                let v1 = self.binary(BinaryOp::Add, v, dv);
                let step1 = self.binary(BinaryOp::Mul, v1, dt);
                let p1 = self.binary(BinaryOp::Add, p, step1);
                let phi0 = self.well_potential(&well, p);
                let phi1 = self.well_potential(&well, p1);
                let fall = self.binary(BinaryOp::Sub, phi0, phi1);
                let two = self.float(2.0);
                let fall = self.binary(BinaryOp::Mul, fall, two);
                let v0_2 = self.binary(BinaryOp::Dot, v, v);
                let allowed = self.binary(BinaryOp::Add, v0_2, fall);
                let allowed = self.binary(BinaryOp::Max, allowed, zero);
                let v1_2 = self.binary(BinaryOp::Dot, v1, v1);
                let ratio = self.binary(BinaryOp::Div, allowed, v1_2);
                let ratio = self.unary(UnaryOp::Sqrt, ratio);
                let one = self.float(1.0);
                let shrink = self.binary(BinaryOp::Min, ratio, one);
                let v_ok = self.binary(BinaryOp::Mul, v1, shrink);
                let change = self.binary(BinaryOp::Sub, v_ok, v);
                let limited = self.binary(BinaryOp::Div, change, dt);
                // Unresolved: the step's reach is half the softening or more
                // (fully at the softening). A zero softening is a core no
                // step resolves.
                let speed = self.unary(UnaryOp::Length, v1);
                let reach = self.binary(BinaryOp::Mul, speed, dt);
                let hair = self.float(1e-6);
                let core = self.binary(BinaryOp::Max, softening, zero);
                let core = self.binary(BinaryOp::Add, core, hair);
                let ratio = self.binary(BinaryOp::Div, reach, core);
                let half = self.float(0.5);
                let unresolved = self.emit(Op::Ternary(TernaryOp::Smoothstep, half, one, ratio), ValueType::Float);
                // With no step (the init context) there is no motion to
                // limit, and the division would read as no pull at all.
                let tiny = self.float(1e-12);
                let stepping = self.emit(Op::Ternary(TernaryOp::Smoothstep, tiny, tiny, dt), ValueType::Float);
                let weight = self.binary(BinaryOp::Mul, unresolved, stepping);
                let weight = self.splat(weight, ValueType::Vec3);
                self.emit(Op::Ternary(TernaryOp::Lerp, a, limited, weight), ValueType::Vec3)
            }
            "force/orbit" => {
                let (p, v, center, axis) = (inp!("position"), inp!("velocity"), inp!("center"), inp!("axis"));
                let (speed, grip, ring, pull) = (inp!("speed"), inp!("grip"), inp!("radius"), inp!("radial_pull"));
                // The particle's place around the axis: its offset in the
                // plane at right angles to it, how far out that is, and the
                // way round.
                let a = self.unary(UnaryOp::Normalize, axis);
                let rel = self.binary(BinaryOp::Sub, p, center);
                let height = self.binary(BinaryOp::Dot, rel, a);
                let along = self.binary(BinaryOp::Mul, a, height);
                let radial = self.binary(BinaryOp::Sub, rel, along);
                let r = self.unary(UnaryOp::Length, radial);
                let out_dir = self.unary(UnaryOp::Normalize, radial);
                let tangent = self.binary(BinaryOp::Cross, a, out_dir);
                let zero = self.float(0.0);
                let one = self.float(1.0);
                let dt = self.attr(Attribute::DeltaTime);
                // Round the axis: the speed along the tangent relaxes toward
                // `speed * r` at the implicit rate, so a high grip locks on in
                // a step without overshooting and a low one spins up.
                let vt = self.binary(BinaryOp::Dot, v, tangent);
                let vr = self.binary(BinaryOp::Dot, v, out_dir);
                let target = self.binary(BinaryOp::Mul, speed, r);
                let gap = self.binary(BinaryOp::Sub, target, vt);
                let grip = self.binary(BinaryOp::Max, grip, zero);
                let k_grip = self.implicit_rate(grip);
                let gain = self.binary(BinaryOp::Mul, gap, k_grip);
                let gain_dt = self.binary(BinaryOp::Mul, gain, dt);
                let vt1 = self.binary(BinaryOp::Add, vt, gain_dt);
                // The distance the step must end at: where it is, or with a
                // Radius, part of the way to the ring. That part is the
                // implicit first-order step of rate `sqrt(radial_pull)`,
                // which closes the gap on any frame and never crosses it.
                let pull = self.binary(BinaryOp::Max, pull, zero);
                let rate = self.unary(UnaryOp::Sqrt, pull);
                let rdt = self.binary(BinaryOp::Mul, rate, dt);
                let den = self.binary(BinaryOp::Add, one, rdt);
                let share = self.binary(BinaryOp::Div, rdt, den);
                let ring_gap = self.binary(BinaryOp::Sub, ring, r);
                let closed = self.binary(BinaryOp::Mul, ring_gap, share);
                let toward_ring = self.binary(BinaryOp::Add, r, closed);
                let on = self.is_on(ring);
                let r_end = self.emit(Op::Ternary(TernaryOp::Lerp, r, toward_ring, on), ValueType::Float);
                // The step moves the particle `s` along the tangent and `u`
                // out along the radius; it ends `r_end` from the axis when
                // `(r + u)² + s² = r_end²`. So the radial speed is solved for,
                // not integrated: the particle lands at that distance every
                // step whatever the frame rate or speed, which is what keeps
                // a stepped circle from spiralling out (or a corrected one
                // in). A step cannot go further round than the circle is
                // wide, so `s` is held to `r_end`.
                let s = self.binary(BinaryOp::Mul, vt1, dt);
                let s_lo = self.unary(UnaryOp::Neg, r_end);
                let s = self.emit(Op::Ternary(TernaryOp::Clamp, s, s_lo, r_end), ValueType::Float);
                let r_end2 = self.binary(BinaryOp::Mul, r_end, r_end);
                let s2 = self.binary(BinaryOp::Mul, s, s);
                let room = self.binary(BinaryOp::Sub, r_end2, s2);
                let reach = self.unary(UnaryOp::Sqrt, room);
                let u = self.binary(BinaryOp::Sub, reach, r);
                let vt1 = self.binary(BinaryOp::Div, s, dt);
                let vr1 = self.binary(BinaryOp::Div, u, dt);
                let dvt = self.binary(BinaryOp::Sub, vt1, vt);
                let dvr = self.binary(BinaryOp::Sub, vr1, vr);
                let at = self.binary(BinaryOp::Div, dvt, dt);
                let ar = self.binary(BinaryOp::Div, dvr, dt);
                let spin = self.binary(BinaryOp::Mul, tangent, at);
                let hold = self.binary(BinaryOp::Mul, out_dir, ar);
                let stepped = self.binary(BinaryOp::Add, spin, hold);
                // With no step (the init context) there is nothing to land:
                // the pull round the axis, as the grip applies it.
                let around = self.binary(BinaryOp::Mul, tangent, gain);
                let tiny = self.float(1e-12);
                let stepping = self.emit(Op::Ternary(TernaryOp::Smoothstep, tiny, tiny, dt), ValueType::Float);
                let stepping = self.splat(stepping, ValueType::Vec3);
                self.emit(Op::Ternary(TernaryOp::Lerp, around, stepped, stepping), ValueType::Vec3)
            }
            "force/conform_sphere" => {
                let (p, v, center, radius) = (inp!("position"), inp!("velocity"), inp!("center"), inp!("radius"));
                let (influence, attraction, max_speed) = (inp!("influence"), inp!("attraction"), inp!("max_speed"));
                let (sticky, shell) = (inp!("sticky"), inp!("shell"));
                // Hanabi's `ConformToSphereModifier`, line for line, with its
                // velocity change turned into the acceleration that makes the
                // same change over this step (`v += a * dt` is how the output
                // integrates). Signed distance to the surface, negative
                // inside; the radial speed toward the centre; the speed it
                // should have, easing to zero in the shell so a particle on
                // the surface rests there instead of oscillating through it.
                let rel = self.binary(BinaryOp::Sub, center, p);
                let dist = self.unary(UnaryOp::Length, rel);
                let dir = self.unary(UnaryOp::Normalize, rel);
                let sd = self.binary(BinaryOp::Sub, dist, radius);
                let zero = self.float(0.0);
                let current = self.binary(BinaryOp::Dot, v, dir);
                let depth = self.unary(UnaryOp::Abs, sd);
                let shell_f = self.emit(Op::Ternary(TernaryOp::Smoothstep, zero, shell, depth), ValueType::Float);
                // sign(sd): +1 outside (toward the centre), -1 inside (out).
                let outside = self.emit(Op::Ternary(TernaryOp::Smoothstep, zero, zero, sd), ValueType::Float);
                let two = self.float(2.0);
                let one = self.float(1.0);
                let side = self.binary(BinaryOp::Mul, outside, two);
                let side = self.binary(BinaryOp::Sub, side, one);
                let want = self.binary(BinaryOp::Mul, side, shell_f);
                let want = self.binary(BinaryOp::Mul, want, max_speed);
                let delta = self.binary(BinaryOp::Sub, want, current);
                // Stickier in the shell: `mix(sticky, plain, shell_factor)`.
                let attraction = self.binary(BinaryOp::Max, attraction, zero);
                let sticky_accel = self.binary(BinaryOp::Mul, attraction, sticky);
                let accel = self.emit(Op::Ternary(TernaryOp::Lerp, sticky_accel, attraction, shell_f), ValueType::Float);
                let dt = self.attr(Attribute::DeltaTime);
                let budget = self.binary(BinaryOp::Mul, accel, dt);
                let budget = self.binary(BinaryOp::Max, budget, zero);
                let floor = self.unary(UnaryOp::Neg, budget);
                let change = self.emit(Op::Ternary(TernaryOp::Clamp, delta, floor, budget), ValueType::Float);
                let rate = self.binary(BinaryOp::Div, change, dt);
                // Untouched beyond the influence distance from the surface
                // (inside the sphere always counts): `influence >= sd`.
                let within = self.emit(Op::Ternary(TernaryOp::Smoothstep, sd, sd, influence), ValueType::Float);
                let rate = self.binary(BinaryOp::Mul, rate, within);
                self.binary(BinaryOp::Mul, dir, rate)
            }
            "force/vortex" => {
                let (p, center, axis, strength) =
                    (inp!("position"), inp!("center"), inp!("axis"), inp!("strength"));
                let a = self.unary(UnaryOp::Normalize, axis);
                let r = self.binary(BinaryOp::Sub, p, center);
                let tangent = self.binary(BinaryOp::Cross, a, r);
                self.binary(BinaryOp::Mul, tangent, strength)
            }
            "force/wind" => {
                let (p, v, coupling, falloff) =
                    (inp!("position"), inp!("velocity"), inp!("coupling"), inp!("height_falloff"));
                // Unwired, the wind pin means the world wind with its
                // per-particle wobble, not the bare attribute: the bare value
                // is one vector for the whole emitter, and a plume that all
                // leans in lockstep reads as a sheet, not as smoke.
                let explicit = self.graph.incoming.contains_key(&(id, "wind"))
                    || node.input_values.contains_key("wind");
                let wind = if explicit {
                    inp!("wind")
                } else {
                    let one = self.float(1.0);
                    let hz = self.float(WIND_WOBBLE_HZ);
                    self.wind_velocity(one, hz)
                };
                // Sheltered near the ground: smoothstep(0, falloff, max(y, 0)).
                // With falloff 0 both edges are 0 and the IR's smoothstep is 1
                // for any input at or above them, which the max guarantees, so
                // 0 is "off" without a branch.
                let y = self.extract(p, 1);
                let zero = self.float(0.0);
                let y = self.binary(BinaryOp::Max, y, zero);
                let shelter = self.emit(Op::Ternary(TernaryOp::Smoothstep, zero, falloff, y), ValueType::Float);
                let target = self.binary(BinaryOp::Mul, wind, shelter);
                let rel = self.binary(BinaryOp::Sub, target, v);
                // k / (1 + k dt) instead of k: the implicit-Euler form of the
                // relaxation, so the step can never overshoot the wind however
                // high the coupling or long the frame. Plain k flings a
                // particle past the wind and back once k * dt passes 2.
                let k = self.binary(BinaryOp::Max, coupling, zero);
                let k_eff = self.implicit_rate(k);
                self.binary(BinaryOp::Mul, rel, k_eff)
            }

            // ── Colour ──
            "color/blackbody" => {
                let (kelvin, intensity) = (inp!("temperature"), inp!("intensity"));
                // Position in the mired-uniform table; see `crate::blackbody`.
                let one = self.float(1.0);
                let kelvin = self.binary(BinaryOp::Max, kelvin, one);
                let million = self.float(1.0e6);
                let mired = self.binary(BinaryOp::Div, million, kelvin);
                let top = self.float(blackbody::MIRED_MAX);
                let num = self.binary(BinaryOp::Sub, top, mired);
                let span = self.float(1.0 / (blackbody::MIRED_MAX - blackbody::MIRED_MIN));
                let t = self.binary(BinaryOp::Mul, num, span);
                let index = match self.tables.gradient_index.get(&id) {
                    Some(i) => *i,
                    None => {
                        let i = self.tables.gradients.len() as u32;
                        self.tables.gradients.push(GradientTable::new(blackbody::table()));
                        self.tables.gradient_index.insert(id, i);
                        i
                    }
                };
                let rgb = self.emit(Op::Gradient { index, t }, ValueType::Color);
                // Scale rgb by the intensity but keep alpha 1: a colour's alpha
                // is opacity (or a flame's heat), and an HDR glow of 4 must not
                // become an opacity of 4. lerp((0,0,0,1), (1,1,1,1), i) is
                // (i, i, i, 1).
                let black = self.constant(ValueType::Color, [0.0, 0.0, 0.0, 1.0]);
                let white = self.constant(ValueType::Color, [1.0; 4]);
                let i4 = self.splat(intensity, ValueType::Color);
                let scale = self.emit(Op::Ternary(TernaryOp::Lerp, black, white, i4), ValueType::Color);
                self.emit(Op::Binary(BinaryOp::Mul, rgb, scale), ValueType::Color)
            }

            other => return Err(err(id, CompileErrorKind::UnknownNodeType(other.to_string()))),
        };
        Ok(r)
    }
}
