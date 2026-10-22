//! How an effect's GPU emitters fit together: each one's kernel config, the
//! effect's event buffer, and the order the render world steps them in.
//!
//! Pure, so the tests can hold it to account with no renderer; the runtime
//! ([`super::runtime`]) turns the plan into render children and buffers.
//!
//! A rule between two GPU emitters becomes an inbox on the target (in one
//! buffer for the whole effect, [`EventLayout`]) and a [`GpuRule`] on the
//! source pointing at it. Every emitter a GPU rule touches is *linked*: it
//! shares the event buffer and steps in lockstep with the others, in the
//! topological order of the rules (parents first), so a child born this step
//! raises its own birth events into an inbox that is delivered later in the
//! same step, as `EffectSim::deliver`'s rounds do. `select::link_backends`
//! has already put both ends of every rule on one backend, so a GPU source
//! never has a CPU target.

use crate::asset::{RibbonKind, VfxAsset};
use crate::sim::sub::MAX_SUB_DEPTH;
use crate::sim::EffectSim;

use super::codegen::{GpuEmitterConfig, GpuRule};
use super::layout::EventLayout;

/// The plan for every emitter slot of an effect.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EffectPlan {
    /// The kernel config of each GPU slot; `None` for a CPU one.
    pub configs: Vec<Option<GpuEmitterConfig>>,
    /// The inboxes, one per GPU target.
    pub events: EventLayout,
    /// The slot shares events with another GPU emitter.
    pub linked: Vec<bool>,
    /// Each slot's place in the topological order of the GPU rules.
    pub order: Vec<u32>,
}

impl EffectPlan {
    /// Whether any GPU emitter of the effect raises or receives events.
    pub fn has_events(&self) -> bool {
        self.events.inboxes.iter().any(Option::is_some)
    }

    /// The slots joined to `slot` by GPU rules, `slot` included.
    pub fn component(&self, slot: usize, edges: &[(usize, usize)]) -> Vec<usize> {
        let mut seen = vec![false; self.configs.len()];
        let mut stack = vec![slot];
        let mut out = Vec::new();
        while let Some(s) = stack.pop() {
            if s >= seen.len() || std::mem::replace(&mut seen[s], true) {
                continue;
            }
            out.push(s);
            for &(a, b) in edges {
                if a == s {
                    stack.push(b);
                }
                if b == s {
                    stack.push(a);
                }
            }
        }
        out.sort_unstable();
        out
    }
}

/// The sub-emitter rules between two GPU emitters of `sim`, as (source,
/// target) slots in rule order.
pub fn gpu_edges(sim: &EffectSim) -> Vec<(usize, usize)> {
    sim.emitters
        .iter()
        .enumerate()
        .filter(|(_, e)| e.is_gpu())
        .flat_map(|(from, e)| e.rules.iter().map(move |r| (from, r.target)))
        .filter(|&(_, to)| sim.emitters.get(to).is_some_and(|t| t.is_gpu()))
        .collect()
}

/// Plan the GPU emitters of `sim`, built from `asset`.
pub fn plan_effect(sim: &EffectSim, asset: &VfxAsset) -> EffectPlan {
    let n = sim.emitters.len();
    let edges = gpu_edges(sim);
    let dimension = asset.dimension;
    let targets: Vec<Option<u32>> =
        (0..n).map(|t| edges.iter().any(|&(_, to)| to == t).then(|| sim.emitters[t].capacity)).collect();
    let events = EventLayout::new(&targets);
    let linked: Vec<bool> = (0..n).map(|s| edges.iter().any(|&(a, b)| a == s || b == s)).collect();
    let order = topological_order(n, &edges);

    let configs = sim
        .emitters
        .iter()
        .enumerate()
        .map(|(slot, e)| {
            if !e.is_gpu() {
                return None;
            }
            let emitter = &asset.emitters[e.emitter_index];
            let render = &emitter.render;
            let trail = (render.draws_ribbon(dimension) && render.ribbon.kind == RibbonKind::Trail).then(|| {
                let d = render.ribbon.min_distance;
                (render.ribbon.trail_points() as u32, if d.is_finite() { d.max(0.0) } else { 0.0 })
            });
            // An emitter whose particles are all at the depth cap raises
            // nothing; one straddling it never reaches here (`select`).
            let raises = e.generations.1 < MAX_SUB_DEPTH;
            let rules = e
                .rules
                .iter()
                .enumerate()
                .filter(|_| raises)
                .filter_map(|(r, rule)| {
                    let inbox = events.inboxes.get(rule.target).copied().flatten()?;
                    let target = &sim.emitters[rule.target];
                    Some(GpuRule {
                        trigger: rule.trigger,
                        inbox,
                        // More children than the target holds would only be
                        // dropped, one thread looping over each.
                        count: rule.count.min(target.capacity),
                        inherit_velocity: rule.inherit_velocity,
                        inherit_color: rule.inherit_color,
                        target_space: target.space,
                        key: ((slot as u32) << 8) | (r as u32 & 0xFF),
                    })
                })
                .collect();
            Some(GpuEmitterConfig {
                space: e.space,
                inherit_velocity: e.inherit_velocity,
                trail,
                rules,
                target: events.inboxes.get(slot).copied().flatten().is_some(),
            })
        })
        .collect();
    EffectPlan { configs, events, linked, order }
}

/// Kahn's order over `edges` (acyclic, as `sub::accepted_rules` makes them),
/// lowest slot first among the ready, so the order is stable.
pub fn topological_order(n: usize, edges: &[(usize, usize)]) -> Vec<u32> {
    let mut incoming = vec![0usize; n];
    for &(_, b) in edges {
        if b < n {
            incoming[b] += 1;
        }
    }
    let mut order = vec![u32::MAX; n];
    let mut next = 0u32;
    let mut ready: std::collections::BTreeSet<usize> = (0..n).filter(|&s| incoming[s] == 0).collect();
    while let Some(s) = ready.pop_first() {
        order[s] = next;
        next += 1;
        for &(a, b) in edges {
            if a == s && b < n {
                incoming[b] -= 1;
                if incoming[b] == 0 {
                    ready.insert(b);
                }
            }
        }
    }
    // A cycle cannot reach here; were one to, its members go last.
    for o in order.iter_mut().filter(|o| **o == u32::MAX) {
        *o = next;
        next += 1;
    }
    order
}
