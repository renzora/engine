//! The runtime weight controller: crossfades, blend trees and layers.
//!
//! Bevy's `AnimationTransitions` knows one "main" animation and fades the rest
//! out, setting the main one's weight every frame. That cannot drive a blend
//! tree (several clips main at once, at weights that change every frame) or a
//! layer (clips that must not be faded out when the base changes), and with it
//! on the player any weight set here was overwritten in `PostUpdate`. So the
//! animator owns its weights: a [`BaseController`] for the base motion, one
//! [`LayerController`] per layer, and [`drive_animators`] writing the result
//! onto the clip nodes each frame.
//!
//! The base crossfade keeps Bevy's "greedy" rule, so a crossfade looks the way
//! it did: the newest fading motion takes what it still holds, older ones
//! share what is left, and the current motion gets the remainder.

use std::collections::{HashMap, HashSet};

use bevy::animation::RepeatAnimation;
use bevy::prelude::*;

use crate::blend_tree::BlendTree;
use crate::component::{AnimatorComponent, AnimatorState};
use crate::graph_builder::{resolve_tree, TreeWeights};
use crate::layers::LayerBlendMode;
use crate::state_machine::AnimParams;

/// What the base plays: one clip, or a blend tree.
#[derive(Clone, Debug, PartialEq)]
pub enum Motion {
    Clip(String),
    Tree(BlendTree),
}

impl Motion {
    pub fn clips(&self) -> Vec<String> {
        match self {
            Motion::Clip(name) => vec![name.clone()],
            Motion::Tree(tree) => {
                let mut out = Vec::new();
                tree.collect_clips(&mut out);
                out
            }
        }
    }

    pub fn weights(&self, params: &AnimParams) -> TreeWeights {
        match self {
            Motion::Clip(name) => TreeWeights { blend: vec![(name.clone(), 1.0)], additive: Vec::new() },
            Motion::Tree(tree) => resolve_tree(tree, params),
        }
    }
}

/// The motion the base is playing or fading into.
#[derive(Clone, Debug)]
pub struct PlayingMotion {
    pub motion: Motion,
    pub looping: bool,
    pub speed: f32,
    /// Its clips still need starting on the player.
    pub needs_start: bool,
}

#[derive(Clone, Debug)]
struct Fading {
    motion: Motion,
    weight: f32,
    decline_per_sec: f32,
}

/// One frame of base weights.
#[derive(Clone, Debug, Default)]
pub struct BaseWeights {
    /// Normally blended clips (all motions together).
    pub blend: HashMap<String, f32>,
    /// Difference clips added by blend tree overlays.
    pub additive: HashMap<String, f32>,
    /// The current motion's own blend weights, before its crossfade share.
    pub current: Vec<(String, f32)>,
    /// Every clip any motion still references.
    pub referenced: HashSet<String>,
}

/// The base motion and whatever it is fading out of.
#[derive(Clone, Debug)]
pub struct BaseController {
    pub current: Option<PlayingMotion>,
    fading: Vec<Fading>,
    current_weight: f32,
}

impl Default for BaseController {
    fn default() -> Self {
        Self { current: None, fading: Vec::new(), current_weight: 1.0 }
    }
}

impl BaseController {
    /// Start `motion`, fading the current one out over `blend` seconds.
    pub fn play(&mut self, motion: Motion, blend: f32, looping: bool, speed: f32) {
        if let Some(old) = self.current.take() {
            if blend > 0.0 && old.motion != motion {
                self.fading.push(Fading { motion: old.motion, weight: self.current_weight, decline_per_sec: 1.0 / blend });
            }
        }
        if blend <= 0.0 {
            self.fading.clear();
        }
        // A motion faded back in stops fading, or its fade ending would stop it.
        self.fading.retain(|f| f.motion != motion);
        self.current = Some(PlayingMotion { motion, looping, speed, needs_start: true });
    }

    pub fn is_idle(&self) -> bool {
        self.current.is_none() && self.fading.is_empty()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Whether the current motion is exactly this one clip.
    pub fn current_is_clip(&self, name: &str) -> bool {
        matches!(&self.current, Some(PlayingMotion { motion: Motion::Clip(n), .. }) if n == name)
    }

    pub fn set_speed(&mut self, speed: f32) {
        if let Some(current) = &mut self.current {
            current.speed = speed;
        }
    }

    /// Fade out what is fading, by `dt` seconds.
    pub fn advance(&mut self, dt: f32) {
        for f in &mut self.fading {
            f.weight = (f.weight - f.decline_per_sec * dt).max(0.0);
        }
        self.fading.retain(|f| f.weight > 0.0);
    }

    /// The weight of every clip this frame.
    pub fn weights(&mut self, params: &AnimParams) -> BaseWeights {
        let mut out = BaseWeights::default();
        let add = |motion: &Motion, share: f32, out: &mut BaseWeights| {
            let w = motion.weights(params);
            for (name, x) in &w.blend {
                *out.blend.entry(name.clone()).or_default() += x * share;
            }
            for (name, x) in &w.additive {
                *out.additive.entry(name.clone()).or_default() += x * share;
            }
            out.referenced.extend(motion.clips());
            w
        };
        let mut remaining = 1.0f32;
        for f in self.fading.iter().rev() {
            let share = f.weight * remaining;
            remaining -= share;
            add(&f.motion, share, &mut out);
        }
        if let Some(current) = &self.current {
            out.current = add(&current.motion, remaining, &mut out).blend;
        }
        self.current_weight = remaining;
        out
    }
}

/// A layer's crossfade between the clips played on it.
#[derive(Clone, Debug, Default)]
pub struct LayerController {
    fades: Vec<(String, f32)>,
}

impl LayerController {
    /// Move toward `target` by `dt`, and return each clip's share of the
    /// layer (summing to 1 while anything plays).
    pub fn update(&mut self, target: Option<&str>, dt: f32, blend: f32) -> Vec<(String, f32)> {
        if let Some(t) = target {
            if !self.fades.iter().any(|(n, _)| n == t) {
                // Onto an empty layer the clip arrives at once: the layer's
                // own weight is the fade-in there, not this crossfade.
                let start = if self.fades.iter().all(|(_, w)| *w <= 0.0) { 1.0 } else { 0.0 };
                self.fades.push((t.to_string(), start));
            }
        }
        let rate = if blend > 0.0 { dt / blend } else { f32::INFINITY };
        for (name, w) in &mut self.fades {
            *w = if Some(name.as_str()) == target { (*w + rate).min(1.0) } else { (*w - rate).max(0.0) };
        }
        self.fades.retain(|(n, w)| *w > 0.0 || Some(n.as_str()) == target);
        let total: f32 = self.fades.iter().map(|(_, w)| *w).sum();
        if total <= 0.0 {
            return Vec::new();
        }
        self.fades.iter().map(|(n, w)| (n.clone(), w / total)).collect()
    }
}

/// Highest override weight used, so the lerp factor `w / (1 - w)` stays finite.
const MAX_OVERRIDE_WEIGHT: f32 = 0.9999;

/// The playback weight a layer's clips get at layer weight `weight`.
///
/// An override layer's clips sit beside the stack below (weight 1) in a blend
/// node, which normalises, so `w / (1 - w)` makes the result a lerp by `w`.
/// An additive layer's clips are added at the weight itself.
pub fn layer_scale(mode: LayerBlendMode, weight: f32) -> f32 {
    let w = if weight.is_finite() { weight } else { 0.0 };
    match mode {
        LayerBlendMode::Override => {
            let w = w.clamp(0.0, MAX_OVERRIDE_WEIGHT);
            w / (1.0 - w)
        }
        LayerBlendMode::Additive => w.clamp(0.0, 1.0),
    }
}

fn repeat_of(looping: bool) -> RepeatAnimation {
    if looping { RepeatAnimation::Forever } else { RepeatAnimation::Never }
}

/// Write every animator's weights onto its clip nodes.
///
/// Gameplay only: in the editor the timeline preview owns the player, and an
/// idle base controller touches nothing, so a preview is never fought over.
pub fn drive_animators(
    time: Res<Time>,
    clips: Res<Assets<AnimationClip>>,
    mut animators: Query<(&AnimatorComponent, &mut AnimatorState)>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let dt = time.delta_secs();
    for (animator, mut state) in animators.iter_mut() {
        if !state.initialized || state.is_paused {
            continue;
        }
        let Some(player_entity) = state.player_entity else { continue };
        let Ok(mut player) = players.get_mut(player_entity) else { continue };
        let state = &mut *state;

        if !state.base.is_idle() {
            drive_base(state, &mut player, &clips, dt);
        }
        drive_layers(animator, state, &mut player, dt);
    }
}

fn drive_base(state: &mut AnimatorState, player: &mut AnimationPlayer, clips: &Assets<AnimationClip>, dt: f32) {
    state.base.advance(dt);
    let weights = state.base.weights(&state.params);
    let Some(current) = state.base.current.clone() else {
        apply_base_weights(state, player, &weights, None);
        return;
    };

    if current.needs_start {
        let restart = matches!(current.motion, Motion::Clip(_));
        let start = |node: AnimationNodeIndex, player: &mut AnimationPlayer| {
            let anim = if restart || !player.is_playing_animation(node) { player.start(node) } else { player.play(node) };
            anim.set_repeat(repeat_of(current.looping)).set_speed(current.speed);
        };
        for clip in current.motion.clips() {
            if let Some(&node) = state.layout.base.get(&clip) {
                start(node, player);
            }
            if let Some(&node) = state.layout.overlay.get(&clip) {
                start(node, player);
            }
        }
        if let Some(c) = &mut state.base.current {
            c.needs_start = false;
        }
    }
    apply_base_weights(state, player, &weights, Some(&current));

    // Blend tree clips of different lengths are kept in step: each runs at the
    // speed that makes it last as long as the weighted mix, so a walk and a
    // run blended together plant their feet together.
    if let Motion::Tree(_) = current.motion {
        let durations: Vec<(AnimationNodeIndex, f32, f32)> = weights
            .current
            .iter()
            .filter_map(|(name, w)| {
                let node = *state.layout.base.get(name)?;
                let dur = state.clip_handles.get(name).and_then(|h| clips.get(h)).map(|c| c.duration())?;
                Some((node, dur, *w))
            })
            .collect();
        let total: f32 = durations.iter().map(|(_, _, w)| w).sum();
        let mixed: f32 = durations.iter().map(|(_, d, w)| d * w).sum::<f32>() / total.max(f32::EPSILON);
        if mixed > f32::EPSILON {
            for (node, dur, _) in &durations {
                if let Some(anim) = player.animation_mut(*node) {
                    anim.set_speed(current.speed * dur / mixed);
                }
            }
        }
        let dominant = TreeWeights { blend: weights.current.clone(), additive: Vec::new() }.dominant().map(str::to_string);
        // Only the heaviest clip of a tree fires markers, so a walk and a run
        // blended together do not each play a footstep.
        state.marker_suppressed = weights
            .current
            .iter()
            .map(|(n, _)| n.clone())
            .filter(|n| Some(n) != dominant.as_ref())
            .collect();
        if let Some(d) = dominant {
            state.current_clip = Some(d);
        }
    } else {
        state.marker_suppressed.clear();
    }
}

fn apply_base_weights(
    state: &AnimatorState,
    player: &mut AnimationPlayer,
    weights: &BaseWeights,
    current: Option<&PlayingMotion>,
) {
    for (map, values) in [(&state.layout.base, &weights.blend), (&state.layout.overlay, &weights.additive)] {
        for (slot, &node) in map {
            if weights.referenced.contains(slot) {
                let w = values.get(slot).copied().unwrap_or(0.0);
                if !player.is_playing_animation(node) {
                    let (looping, speed) = current.map_or((true, 1.0), |c| (c.looping, c.speed));
                    player.play(node).set_repeat(repeat_of(looping)).set_speed(speed);
                }
                if let Some(anim) = player.animation_mut(node) {
                    anim.set_weight(w);
                }
            } else if player.is_playing_animation(node) {
                player.stop(node);
            }
        }
    }
}

fn drive_layers(animator: &AnimatorComponent, state: &mut AnimatorState, player: &mut AnimationPlayer, dt: f32) {
    state.layer_controllers.resize_with(animator.layers.len(), LayerController::default);
    for (k, layer) in animator.layers.iter().enumerate() {
        let Some(nodes) = state.layout.layers.get(k) else { continue };
        let shares = state.layer_controllers[k].update(layer.current_clip.as_deref(), dt, animator.blend_duration);
        let scale = layer_scale(nodes.mode, layer.weight);
        for (slot, &node) in &nodes.nodes {
            let share = shares.iter().find(|(n, _)| n == slot).map(|(_, w)| *w);
            match share {
                Some(share) => {
                    if !player.is_playing_animation(node) {
                        let slot_def = animator.get_slot(slot);
                        player
                            .play(node)
                            .set_repeat(repeat_of(slot_def.is_none_or(|s| s.looping)))
                            .set_speed(slot_def.map_or(1.0, |s| s.speed));
                    }
                    if let Some(anim) = player.animation_mut(node) {
                        // Zero is skipped by Bevy's evaluation, keeping the clip's
                        // time running while the layer is faded out.
                        anim.set_weight(share * scale);
                    }
                }
                None if player.is_playing_animation(node) => {
                    player.stop(node);
                }
                None => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(n: &str) -> Motion {
        Motion::Clip(n.into())
    }

    fn total(w: &BaseWeights) -> f32 {
        w.blend.values().sum()
    }

    /// A crossfade always splits exactly one unit of weight, and ends with
    /// the new motion alone.
    #[test]
    fn a_crossfade_partitions_weight_and_completes() {
        let mut c = BaseController::default();
        let params = AnimParams::default();
        c.play(clip("idle"), 0.0, true, 1.0);
        assert_eq!(c.weights(&params).blend.get("idle"), Some(&1.0));
        c.play(clip("walk"), 0.5, true, 1.0);
        for _ in 0..4 {
            c.advance(0.1);
            let w = c.weights(&params);
            assert!((total(&w) - 1.0).abs() < 1e-5);
            assert!(w.blend["walk"] > 0.0 && w.blend["idle"] > 0.0);
        }
        c.advance(0.2);
        let w = c.weights(&params);
        assert_eq!(w.blend.get("walk"), Some(&1.0));
        assert!(!w.referenced.contains("idle"), "a finished fade releases its clip");
    }

    /// Going back to a motion that is still fading out takes it back rather
    /// than leaving its fade to stop it later.
    #[test]
    fn replaying_a_fading_motion_cancels_its_fade() {
        let mut c = BaseController::default();
        let params = AnimParams::default();
        c.play(clip("idle"), 0.0, true, 1.0);
        c.play(clip("walk"), 1.0, true, 1.0);
        c.advance(0.3);
        c.weights(&params);
        c.play(clip("idle"), 1.0, true, 1.0);
        for _ in 0..20 {
            c.advance(0.1);
        }
        let w = c.weights(&params);
        assert_eq!(w.blend.get("idle"), Some(&1.0));
        assert!(!w.referenced.contains("walk"));
    }

    /// A blend tree motion spreads its share over its clips by parameter.
    #[test]
    fn a_tree_motion_follows_its_parameter() {
        use crate::blend_tree::Blend1DEntry;
        let tree = BlendTree::Blend1D {
            param: "speed".into(),
            entries: vec![
                Blend1DEntry { clip: "idle".into(), threshold: 0.0 },
                Blend1DEntry { clip: "walk".into(), threshold: 2.0 },
                Blend1DEntry { clip: "run".into(), threshold: 5.0 },
            ],
        };
        let mut c = BaseController::default();
        c.play(Motion::Tree(tree), 0.0, true, 1.0);
        let mut params = AnimParams::default();
        params.set_float("speed", 3.5);
        let w = c.weights(&params);
        assert!((w.blend["walk"] - 0.5).abs() < 1e-5);
        assert!((w.blend["run"] - 0.5).abs() < 1e-5);
        assert_eq!(w.blend.get("idle").copied().unwrap_or(0.0), 0.0);
        assert!(w.referenced.contains("idle"), "every clip of a playing tree stays referenced");
    }

    /// An override layer at weight `w` must be a lerp by `w` against a stack
    /// below of weight 1, under a normalising blend.
    #[test]
    fn override_layer_scale_is_a_lerp_after_normalising() {
        for w in [0.0f32, 0.25, 0.5, 0.9] {
            let x = layer_scale(LayerBlendMode::Override, w);
            assert!((x / (1.0 + x) - w).abs() < 1e-5, "w={w}");
        }
        assert!(layer_scale(LayerBlendMode::Override, 1.0).is_finite());
        assert_eq!(layer_scale(LayerBlendMode::Additive, 0.4), 0.4);
        assert_eq!(layer_scale(LayerBlendMode::Additive, f32::NAN), 0.0);
    }

    #[test]
    fn a_layer_crossfades_between_its_clips() {
        let mut l = LayerController::default();
        let w = l.update(Some("wave"), 0.016, 0.2);
        assert_eq!(w, vec![("wave".to_string(), 1.0)], "onto an empty layer the clip arrives at once");
        l.update(Some("point"), 0.1, 0.2);
        let w = l.update(Some("point"), 0.0, 0.2);
        let total: f32 = w.iter().map(|(_, x)| x).sum();
        assert!((total - 1.0).abs() < 1e-5);
        assert!(w.iter().all(|(_, x)| *x > 0.0));
        let w = l.update(Some("point"), 0.2, 0.2);
        assert_eq!(w, vec![("point".to_string(), 1.0)]);
        assert!(l.update(None, 1.0, 0.2).is_empty());
    }
}
