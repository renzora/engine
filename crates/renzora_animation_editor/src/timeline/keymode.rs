//! Bone key interpolation: a key's [`KeyMode`], its hand-set tangents, and the
//! menus and buttons that set them in the dopesheet, the key card and the
//! Curves view.
//!
//! Property keys keep their own [`Interp`] (an easing curve per key); a bone
//! key has a [`KeyMode`] instead, because a bone's motion is shaped by
//! tangents through its keys rather than by an easing between two of them.
//! Where a choice is applied to a selection holding both, each kind gets its
//! nearest equivalent, so one right-click works on a mixed selection.

use bevy::prelude::*;

use renzora::{AnimClip, Interp, KeyCurve, KeyMode};
use renzora_ember::font::EmberFonts;
use renzora_ember::widgets::menu_item;

use super::clip::{Lane, SelKey, SelectedKey, SelectedKeys, TimelineClip};

/// The choices, in menu order: mode, its icon, and the line explaining it.
pub(super) fn mode_choices() -> [(KeyMode, &'static str, &'static str); 6] {
    [
        (KeyMode::Auto, "bezier-curve", "Auto (smooth, no overshoot)"),
        (KeyMode::Linear, "line-segment", "Linear"),
        (KeyMode::Constant, "stairs", "Constant (hold)"),
        (KeyMode::Flat, "minus", "Flat (ease in and out)"),
        (KeyMode::Free, "pen-nib", "Free (one hand-set tangent)"),
        (KeyMode::Broken, "git-commit", "Broken (two hand-set tangents)"),
    ]
}

/// Localized menu text for a mode choice.
pub(super) fn mode_label_tr(mode: KeyMode, fallback: &str) -> String {
    let key = match mode {
        KeyMode::Auto => "timeline.key_auto",
        KeyMode::Linear => "timeline.key_linear",
        KeyMode::Constant => "timeline.key_constant",
        KeyMode::Flat => "timeline.key_flat",
        KeyMode::Free => "timeline.key_free",
        KeyMode::Broken => "timeline.key_broken",
    };
    renzora::lang::t_or(key, fallback)
}

/// The nearest property easing for a bone mode, for a mixed selection. The
/// hand-set modes have none: a property key has no tangents to set.
fn interp_of(mode: KeyMode) -> Option<Interp> {
    match mode {
        KeyMode::Linear => Some(Interp::Linear),
        KeyMode::Constant => Some(Interp::Stepped),
        KeyMode::Auto | KeyMode::Flat => Some(Interp::Eased(bevy::curve::EaseFunction::SmoothStep)),
        KeyMode::Free | KeyMode::Broken => None,
    }
}

/// The selection a choice acts on: every selected key, the primary included.
pub(super) fn selection(world: &World) -> Vec<SelKey> {
    let mut keys: Vec<SelKey> = world.get_resource::<SelectedKeys>().map(|s| s.keys.clone()).unwrap_or_default();
    if let Some(p) = world.get_resource::<SelectedKey>().and_then(|s| s.0) {
        if !keys.contains(&p) {
            keys.push(p);
        }
    }
    keys
}

/// A bone or blend shape key's mode, `None` for any other key.
pub(super) fn key_mode(clip: &AnimClip, k: SelKey) -> Option<KeyMode> {
    match k.lane {
        Lane::Bone { track, channel } => {
            let t = clip.tracks.get(track)?;
            (k.index < t.key_count(channel)).then(|| t.key_curve(channel, k.index).mode)
        }
        Lane::Morph { track } => {
            let t = clip.morph_tracks.get(track)?;
            (k.index < t.keys.len()).then(|| t.key_curve(k.index).mode)
        }
        Lane::Prop { .. } | Lane::Fx { .. } => None,
    }
}

/// The curve a key with curve `c` gets when switched to `mode`, `None` when
/// it already has it. `tangents` are the key's resolved tangents now.
///
/// Switching to a hand-set mode seeds the tangents with the ones the key has
/// now, so the curve does not move until a handle is dragged: choosing Free on
/// an Auto key is a way to start from Auto's shape. Leaving one clears them,
/// so a file only carries the tangents something reads.
fn switched(mut c: KeyCurve, mode: KeyMode, tangents: impl FnOnce() -> Option<(Vec4, Vec4)>) -> Option<KeyCurve> {
    if c.mode == mode {
        return None;
    }
    if mode.is_hand_set() && !c.mode.is_hand_set() {
        if let Some((a, b)) = tangents() {
            c.in_tan = a.to_array();
            c.out_tan = b.to_array();
        }
    }
    // Free reads one tangent for both sides; breaking it starts both sides
    // from that one.
    if c.mode == KeyMode::Free && mode == KeyMode::Broken {
        c.in_tan = c.out_tan;
    }
    if !mode.is_hand_set() {
        c = KeyCurve::new(mode);
    }
    c.mode = mode;
    Some(c)
}

/// Give one bone or blend shape key `mode` (see [`switched`]).
pub(super) fn set_bone_mode(clip: &mut AnimClip, k: SelKey, mode: KeyMode) {
    match k.lane {
        Lane::Bone { track, channel } => {
            let Some(t) = clip.tracks.get_mut(track) else { return };
            if k.index >= t.key_count(channel) {
                return;
            }
            let spline = || t.channel_spline(channel).tangents(k.index);
            if let Some(c) = switched(t.key_curve(channel, k.index), mode, spline) {
                t.set_key_curve(channel, k.index, c);
            }
        }
        Lane::Morph { track } => {
            let Some(t) = clip.morph_tracks.get_mut(track) else { return };
            if k.index >= t.keys.len() {
                return;
            }
            let spline = || t.spline().tangents(k.index);
            if let Some(c) = switched(t.key_curve(k.index), mode, spline) {
                t.set_key_curve(k.index, c);
            }
        }
        Lane::Prop { .. } | Lane::Fx { .. } => {}
    }
}

/// Apply a mode to `keys`: bone keys take it, property keys their nearest
/// easing (see [`interp_of`]), effect keys are left alone.
pub(super) fn apply_mode(world: &mut World, keys: &[SelKey], mode: KeyMode) {
    let Some(mut cache) = world.get_resource_mut::<TimelineClip>() else { return };
    let Some(clip) = cache.clip.as_mut() else { return };
    let mut changed = false;
    for k in keys {
        match k.lane {
            Lane::Bone { .. } | Lane::Morph { .. } => {
                set_bone_mode(clip, *k, mode);
                changed = true;
            }
            Lane::Prop { track } => {
                if let (Some(interp), Some(key)) = (interp_of(mode), clip.property_tracks.get_mut(track).and_then(|p| p.keys.get_mut(k.index))) {
                    key.interp = interp;
                    changed = true;
                }
            }
            Lane::Fx { .. } => {}
        }
    }
    if changed {
        cache.dirty = true;
    }
}

/// Menu items for every mode, each applying to `keys`, the current one ticked.
pub(super) fn mode_menu_items(commands: &mut Commands, fonts: &EmberFonts, keys: Vec<SelKey>, current: Option<KeyMode>) -> Vec<Entity> {
    mode_choices()
        .into_iter()
        .map(|(mode, icon, fallback)| {
            let keys = keys.clone();
            let shown = if current == Some(mode) { "check" } else { icon };
            menu_item(commands, fonts, shown, &mode_label_tr(mode, fallback), move |w| apply_mode(w, &keys, mode))
        })
        .collect()
}

/// What a right-click on key `picked` acts on: the whole selection when the
/// key is part of it, else that key alone (it is not selected by the click).
pub(super) fn menu_targets(world: &World, picked: SelKey) -> Vec<SelKey> {
    let sel = selection(world);
    if sel.contains(&picked) {
        sel
    } else {
        vec![picked]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora::BoneTrack;

    fn clip() -> AnimClip {
        let mut c = renzora_animation::extract::create_empty_clip("c", 2.0);
        let mut t = BoneTrack::new("arm");
        t.translations = vec![(0.0, [0.0; 3]), (1.0, [1.0, 0.0, 0.0]), (2.0, [2.0, 0.0, 0.0])];
        for i in 0..3 {
            t.set_key_curve(0, i, KeyCurve::AUTO);
        }
        c.tracks.push(t);
        c
    }

    const MID: SelKey = SelKey { lane: Lane::Bone { track: 0, channel: 0 }, index: 1 };

    #[test]
    fn going_free_keeps_the_curve_where_it_was() {
        let mut c = clip();
        let before: Vec<f32> = (0..=20).map(|i| c.tracks[0].channel_spline(0).sample(i as f32 / 10.0).x).collect();
        set_bone_mode(&mut c, MID, KeyMode::Free);
        let curve = c.tracks[0].key_curve(0, 1);
        assert_eq!(curve.mode, KeyMode::Free);
        assert!((curve.out_tan[0] - 1.0).abs() < 1e-5, "seeded from Auto: {:?}", curve.out_tan);
        let after: Vec<f32> = (0..=20).map(|i| c.tracks[0].channel_spline(0).sample(i as f32 / 10.0).x).collect();
        for (a, b) in before.iter().zip(&after) {
            assert!((a - b).abs() < 1e-5);
        }
    }

    #[test]
    fn leaving_a_hand_set_mode_drops_its_tangents() {
        let mut c = clip();
        set_bone_mode(&mut c, MID, KeyMode::Broken);
        set_bone_mode(&mut c, MID, KeyMode::Linear);
        assert_eq!(c.tracks[0].key_curve(0, 1), KeyCurve::LINEAR);
    }

    #[test]
    fn a_blend_shape_key_takes_a_mode_like_a_bone_key() {
        let mut c = clip();
        let mut m = renzora::MorphTrack::new("Face", 0, "");
        m.keys = vec![(0.0, 0.0), (1.0, 1.0)];
        c.morph_tracks.push(m);
        let k = SelKey { lane: Lane::Morph { track: 0 }, index: 0 };
        assert_eq!(key_mode(&c, k), Some(KeyMode::Linear));
        set_bone_mode(&mut c, k, KeyMode::Constant);
        assert_eq!(key_mode(&c, k), Some(KeyMode::Constant));
        assert_eq!(c.morph_tracks[0].sample(0.9), Some(0.0));
    }

    #[test]
    fn a_property_key_takes_the_nearest_easing() {
        assert_eq!(interp_of(KeyMode::Constant), Some(Interp::Stepped));
        assert_eq!(interp_of(KeyMode::Free), None);
    }
}
