//! Sub-emitters: one emitter's particles spawning another's.
//!
//! A parent emitter records an [`Event`] when one of its particles is born,
//! dies, or passes another interval of its life, for each of its
//! [`SubEmitter`](crate::asset::SubEmitter) rules that fires on it. Events are
//! delivered at the end of the step, after every emitter has moved: the
//! children start at the parent particle's position as of that moment.
//!
//! ## Determinism
//!
//! Delivery order is fixed by the events themselves, not by where the parent
//! happened to sit in its buffer: events are sorted by source emitter, rule,
//! parent particle id and the event's index (0 for a birth or a death, `k`
//! for the `k`-th interval). Children take the target emitter's next ids in
//! that order, and every random a child reads is a hash of its id, so the
//! same parent event always gives the same children, and `simulate_to` replays
//! them exactly.
//!
//! ## Cycles and depth
//!
//! A rule that would close a loop (A spawns B spawns A, or A spawns A) is
//! refused when the effect is built, with a warning naming it: a loop would
//! fill both emitters to capacity in a few frames. The rules are taken in file
//! order, so the rule that closes the loop is the one dropped. Independently,
//! every particle carries its generation (0 for one its emitter spawned on its
//! own), and a particle [`MAX_SUB_DEPTH`] generations down spawns no more,
//! which bounds the work of a long chain however it is built.

use bevy::math::{Vec3, Vec4};

use crate::asset::{SubEmitterTrigger, VfxAsset};

/// The deepest generation that may still trigger its own sub-emitters.
pub const MAX_SUB_DEPTH: u8 = 4;

/// Most interval events one particle may raise in one step. A tiny interval
/// on a long frame would otherwise spawn without bound.
pub const MAX_INTERVAL_EVENTS_PER_STEP: u32 = 16;

/// A rule resolved against the running effect.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rule {
    pub trigger: SubEmitterTrigger,
    /// Slot of the target in `EffectSim::emitters`.
    pub target: usize,
    pub count: u32,
    pub inherit_velocity: f32,
    pub inherit_color: bool,
}

/// One firing of one rule, for one parent particle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    /// Index into the source emitter's rules.
    pub rule: usize,
    pub parent: u32,
    /// 0 for a birth or a death; `k` for the `k`-th interval.
    pub index: u32,
    /// Generation of the children: the parent's plus one.
    pub generation: u8,
    /// In the source emitter's simulation space.
    pub position: Vec3,
    pub velocity: Vec3,
    pub color: Vec4,
}

/// Where a child starts, in the target emitter's simulation space.
#[derive(Clone, Copy, Debug)]
pub struct Origin {
    pub position: Vec3,
    pub velocity: Vec3,
    /// Multiplied into the child's colour when the rule inherits colour.
    pub color: Option<Vec4>,
}

/// The rules of `asset` that are safe to run, as `(source, rule index)` in
/// asset emitter indices, and a sentence for each one refused.
///
/// A rule is refused when its target does not exist, or when adding it would
/// let the target reach the source again (a cycle; see the module docs).
pub fn accepted_rules(asset: &VfxAsset) -> (Vec<(usize, usize)>, Vec<String>) {
    let n = asset.emitters.len();
    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut accepted = Vec::new();
    let mut issues = Vec::new();
    for (source, emitter) in asset.emitters.iter().enumerate() {
        for (i, rule) in emitter.sub_emitters.iter().enumerate() {
            let target = rule.target;
            if target >= n {
                issues.push(format!(
                    "emitter `{}`: sub-emitter {} targets emitter {}, which does not exist",
                    emitter.name,
                    i + 1,
                    target + 1
                ));
                continue;
            }
            if reaches(&edges, target, source) {
                issues.push(format!(
                    "emitter `{}`: sub-emitter {} would spawn `{}`, which already leads back to it; \
                     a loop like that would fill both emitters, so it is ignored",
                    emitter.name,
                    i + 1,
                    asset.emitters[target].name
                ));
                continue;
            }
            edges[source].push(target);
            accepted.push((source, i));
        }
    }
    (accepted, issues)
}

/// Whether `to` is reachable from `from` (a node reaches itself).
fn reaches(edges: &[Vec<usize>], from: usize, to: usize) -> bool {
    let mut stack = vec![from];
    let mut seen = vec![false; edges.len()];
    while let Some(n) = stack.pop() {
        if n == to {
            return true;
        }
        if std::mem::replace(&mut seen[n], true) {
            continue;
        }
        stack.extend(edges[n].iter().copied());
    }
    false
}

/// The generations each emitter's particles can have, as `(fewest, most)`
/// hops from an emitter's own spawning, from the resolved `edges` (source
/// slot, target slot) between `n` emitters. An emitter nothing targets spawns
/// its own, generation 0; a target emits only its parents' children, one
/// generation past each parent. Saturates past [`MAX_SUB_DEPTH`], which is all
/// any reader asks about.
///
/// The GPU keeps no generation per particle (the 96-byte particle has no room
/// for one), so it decides per emitter instead: one whose particles are all
/// under the cap raises every event, one whose particles are all at or past
/// it raises none. Only a chain five emitters deep can give one emitter both,
/// and that one stays on the CPU (`gpu::select`).
pub fn generation_spans(n: usize, edges: &[(usize, usize)]) -> Vec<(u8, u8)> {
    let cap = MAX_SUB_DEPTH + 1;
    let driven: Vec<bool> = (0..n).map(|t| edges.iter().any(|&(_, to)| to == t)).collect();
    let mut spans: Vec<Option<(u8, u8)>> = (0..n).map(|i| (!driven[i]).then_some((0, 0))).collect();
    // The rules are acyclic (see `accepted_rules`), so `n` rounds of
    // relaxation reach every path's length.
    for _ in 0..=n {
        let mut next = spans.clone();
        for t in (0..n).filter(|&t| driven[t]) {
            let mut span: Option<(u8, u8)> = None;
            for &(from, _) in edges.iter().filter(|&&(_, to)| to == t) {
                if let Some((lo, hi)) = spans[from] {
                    let (lo, hi) = ((lo + 1).min(cap), (hi + 1).min(cap));
                    span = Some(match span {
                        Some((a, b)) => (a.min(lo), b.max(hi)),
                        None => (lo, hi),
                    });
                }
            }
            next[t] = span;
        }
        if next == spans {
            break;
        }
        spans = next;
    }
    // A target whose parents never spawn anything (all of them unreachable)
    // spawns nothing either; call it past the cap, which raises nothing.
    spans.into_iter().map(|s| s.unwrap_or((cap, cap))).collect()
}

/// The interval events a particle raises over a step that took its age from
/// `before` to `after`: the indices `k` with `k * interval` in `(before,
/// after]`, at most [`MAX_INTERVAL_EVENTS_PER_STEP`].
pub fn interval_events(before: f32, after: f32, interval: f32) -> std::ops::Range<u32> {
    if !interval.is_finite() || interval <= 0.0 || !before.is_finite() || !after.is_finite() || after <= before {
        return 0..0;
    }
    let first = (before / interval).floor() as i64 + 1;
    let last = (after / interval).floor() as i64;
    if last < first || first < 1 {
        return 0..0;
    }
    let last = last.min(first + MAX_INTERVAL_EVENTS_PER_STEP as i64 - 1);
    first.clamp(0, u32::MAX as i64) as u32..(last + 1).clamp(0, u32::MAX as i64) as u32
}
