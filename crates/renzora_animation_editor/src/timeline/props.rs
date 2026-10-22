//! Property tracks — the deferred world ops behind them.
//!
//! Reading or writing a property goes through reflection, which needs `&World`,
//! so the header buttons and toolbar do not act directly: they queue a request
//! in [`TimelineOps`] and one exclusive system applies the lot.

use bevy::prelude::*;

use renzora::reflection::list_animatable_fields;
use renzora::{PropertyKey, PropertyTrack, TrackValue};
use renzora_animation::property_playback::read_track_value;
use renzora_ember::font::EmberFonts;
use renzora_ember::widgets::{menu_item, screen_menu};
use renzora_ember::UiInteraction;

use crate::AnimationEditorState;

use super::clip::{clip_entity, cur_clip, SelKey, SelectedKey, SelectedKeys, TimelineClip};
use super::layout::{RowKind, TrackLayout};
use super::snapshots::title_case;
use super::{AddKeyTrackBtn, AddTrackBtn, DeletePropTrack, PropTrackCombo};

/// Deferred world ops requested by toolbar buttons / track headers that need
/// `&World` access (reflection enumeration / live-value reads), applied by an
/// exclusive system.
#[derive(Resource, Default)]
pub(crate) struct TimelineOps {
    /// Append a new empty property track (the user picks its property after).
    pub(super) add_empty_track: bool,
    /// Insert a key at the playhead on every property track.
    pub(super) add_key: bool,
    /// Open the per-track property picker: (track index, menu screen position).
    pub(super) open_property_menu: Option<(usize, Vec2)>,
    /// Delete the property track at this index.
    pub(super) delete_track: Option<usize>,
    /// Insert a key at the playhead on just this one property track.
    pub(super) add_key_track: Option<usize>,
    /// Delete every selected keyframe.
    pub(super) delete_selected_key: bool,
    /// Copy the selected keyframes to the timeline clipboard.
    pub(super) copy_keys: bool,
    /// Paste the clipboard at the playhead.
    pub(super) paste_keys: bool,
    /// Empty-state "Key a property": once the new clip has loaded, add a track
    /// and open its property picker here.
    pub(super) pending_property_pick: Option<Vec2>,
    /// Empty-state "Record": once the new clip has loaded, add Transform tracks
    /// and arm record.
    pub(super) pending_record_setup: bool,
    /// "+ Track" with an effect shown: open the menu offering a property
    /// track and the effect's parameters, at this window position.
    pub(super) open_add_track_menu: Option<Vec2>,
}

/// Upsert a keyframe at `time` on a property track (replace if one already sits
/// within epsilon, else insert + re-sort).
pub(super) fn upsert_key(pt: &mut PropertyTrack, time: f32, value: TrackValue) {
    const EPS: f32 = 1e-4;
    if let Some(k) = pt.keys.iter_mut().find(|k| (k.time - time).abs() < EPS) {
        k.value = value;
    } else {
        pt.keys.push(PropertyKey { time, value, interp: renzora::Interp::Linear });
        pt.keys.sort_by(|a, b| a.time.total_cmp(&b.time));
    }
}

/// Read a property track's live value and key it at `time` (auto-extending the
/// clip if `time` is past the end). Used by right-click "Add keyframe here".
pub(super) fn add_key_at(world: &mut World, entity: Option<Entity>, track: usize, time: f32) {
    let Some(entity) = entity else { return };
    let track_data = world
        .get_resource::<TimelineClip>()
        .and_then(|c| c.clip.as_ref())
        .and_then(|c| c.property_tracks.get(track))
        .cloned();
    let Some(track_data) = track_data else { return };
    let Some(val) = read_track_value(world, entity, &track_data) else {
        warn!("[prop-anim] Add Key (right-click): could not read live value for {}.{}", track_data.component, track_data.field);
        return;
    };
    info!(
        "[prop-anim] Add Key (right-click): {}.{} @ t={:.3} from {:?} -> {:?}",
        track_data.component, track_data.field, time, entity, val
    );
    if let Some(mut cache) = world.get_resource_mut::<TimelineClip>() {
        let mut dirty = false;
        if let Some(clip) = cache.clip.as_mut() {
            if time > clip.duration {
                clip.duration = time;
            }
            if let Some(pt) = clip.property_tracks.get_mut(track) {
                upsert_key(pt, time, val);
                dirty = true;
            }
        }
        if dirty {
            cache.dirty = true;
        }
    }
}

/// Set an existing keyframe's value to the entity's current live value — the
/// foolproof "pose the object, then set this key to it" action.
pub(super) fn set_key_to_live(world: &mut World, entity: Option<Entity>, track: usize, idx: usize) {
    let Some(entity) = entity else { return };
    let track_data = world
        .get_resource::<TimelineClip>()
        .and_then(|c| c.clip.as_ref())
        .and_then(|c| c.property_tracks.get(track))
        .cloned();
    let Some(track_data) = track_data else { return };
    let Some(val) = read_track_value(world, entity, &track_data) else {
        warn!("[prop-anim] Set Key: could not read live value for {}.{}", track_data.component, track_data.field);
        return;
    };
    if let Some(mut cache) = world.get_resource_mut::<TimelineClip>() {
        if let Some(key) = cache
            .clip
            .as_mut()
            .and_then(|c| c.property_tracks.get_mut(track))
            .and_then(|pt| pt.keys.get_mut(idx))
        {
            info!(
                "[prop-anim] Set Key {} of {}.{} -> {:?}",
                idx, track_data.component, track_data.field, val
            );
            key.value = val;
        }
        cache.dirty = true;
    }
}

/// Set a property key's interpolation curve and mark the clip dirty (so the
/// scrub preview and the auto-save reflect it immediately).
pub(super) fn set_key_interp(world: &mut World, track: usize, idx: usize, interp: renzora::Interp) {
    if let Some(mut cache) = world.get_resource_mut::<TimelineClip>() {
        if let Some(key) = cache
            .clip
            .as_mut()
            .and_then(|c| c.property_tracks.get_mut(track))
            .and_then(|pt| pt.keys.get_mut(idx))
        {
            key.interp = interp;
        }
        cache.dirty = true;
    }
}

/// The interpolation choices offered in a property key's right-click menu, as
/// `(icon, label, interp)`. A curated subset of Bevy's [`EaseFunction`](bevy::curve::EaseFunction)
/// set — the ones that read clearly on a float/Vec3/Color dopesheet — plus the
/// two non-eased modes. Per-key authoring writes one of these into `PropertyKey`.
pub(super) fn interp_menu_choices() -> [(&'static str, &'static str, renzora::Interp); 9] {
    use bevy::curve::EaseFunction as E;
    use renzora::Interp;
    [
        ("minus", "Linear", Interp::Linear),
        ("stairs", "Stepped (hold)", Interp::Stepped),
        ("chart-line", "Smooth (in-out)", Interp::Eased(E::SmoothStep)),
        ("trend-up", "Ease In", Interp::Eased(E::QuadraticIn)),
        ("trend-down", "Ease Out", Interp::Eased(E::QuadraticOut)),
        ("activity", "Ease In-Out", Interp::Eased(E::CubicInOut)),
        ("arrow-u-up-left", "Back Out (overshoot)", Interp::Eased(E::BackOut)),
        ("circle", "Bounce Out", Interp::Eased(E::BounceOut)),
        ("waves", "Elastic Out", Interp::Eased(E::ElasticOut)),
    ]
}

/// Localized display string for an interpolation choice's English label (the
/// English label stays the identity in `interp_menu_choices`; the `Interp` enum
/// value is what's actually written, so only the menu text is translated).
pub(super) fn interp_label_tr(label: &str) -> String {
    renzora::lang::t(match label {
        "Linear" => "animation.interp_linear",
        "Stepped (hold)" => "animation.interp_stepped",
        "Smooth (in-out)" => "animation.interp_smooth",
        "Ease In" => "animation.interp_ease_in",
        "Ease Out" => "animation.interp_ease_out",
        "Ease In-Out" => "animation.interp_ease_in_out",
        "Back Out (overshoot)" => "animation.interp_back_out",
        "Bounce Out" => "animation.interp_bounce_out",
        "Elastic Out" => "animation.interp_elastic_out",
        _ => return label.to_string(),
    })
}

/// Detect clicks on a property track's dropdown / delete button (built inside
/// the keyed header list) and queue the corresponding world op.
pub(super) fn prop_header_click(
    combos: Query<(&UiInteraction, &bevy::ui::RelativeCursorPosition, &bevy::ui::ComputedNode, &PropTrackCombo), Changed<UiInteraction>>,
    dels: Query<(&UiInteraction, &DeletePropTrack), Changed<UiInteraction>>,
    addks: Query<(&UiInteraction, &AddKeyTrackBtn), Changed<UiInteraction>>,
    add_track: Query<&UiInteraction, (With<AddTrackBtn>, Changed<UiInteraction>)>,
    windows: Query<&Window>,
    cache: Res<TimelineClip>,
    mut ops: ResMut<TimelineOps>,
) {
    if add_track.iter().any(|i| *i == UiInteraction::Pressed) {
        // With an effect in the panel "+ Track" is a choice (property track
        // or effect parameter); without one it stays the one-click add.
        if cache.fx.is_some() {
            let cursor = windows.iter().find_map(|w| w.cursor_position()).unwrap_or(Vec2::splat(200.0));
            ops.open_add_track_menu = Some(cursor + Vec2::new(0.0, 12.0));
        } else {
            ops.add_empty_track = true;
        }
    }
    for (interaction, rcp, cn, combo) in &combos {
        if *interaction != UiInteraction::Pressed {
            continue;
        }
        let cursor = windows.iter().find_map(|w| w.cursor_position()).unwrap_or(Vec2::splat(200.0));
        // Anchor the menu just below the combo.
        let size = cn.size() * cn.inverse_scale_factor();
        let top_left = cursor - (rcp.normalized.unwrap_or(Vec2::ZERO) + Vec2::splat(0.5)) * size;
        ops.open_property_menu = Some((combo.0, Vec2::new(top_left.x, top_left.y + size.y + 2.0)));
    }
    for (interaction, del) in &dels {
        if *interaction == UiInteraction::Pressed {
            ops.delete_track = Some(del.0);
        }
    }
    for (interaction, addk) in &addks {
        if *interaction == UiInteraction::Pressed {
            ops.add_key_track = Some(addk.0);
        }
    }
}

/// Apply deferred ops that need full world access (reflection): add/delete a
/// property track, open a track's property picker, or key all tracks.
pub(super) fn apply_timeline_ops(world: &mut World) {
    apply_pending_setup(world);
    if let Some(pos) = world.get_resource_mut::<TimelineOps>().and_then(|mut o| o.open_add_track_menu.take()) {
        super::fx::open_add_track_menu(world, pos);
    }
    let (add_empty, add_key, open_menu, delete, add_key_track, delete_key, copy, paste) = {
        let Some(o) = world.get_resource::<TimelineOps>() else { return };
        (o.add_empty_track, o.add_key, o.open_property_menu, o.delete_track, o.add_key_track, o.delete_selected_key, o.copy_keys, o.paste_keys)
    };
    if !add_empty && !add_key && open_menu.is_none() && delete.is_none() && add_key_track.is_none() && !delete_key && !copy && !paste {
        return;
    }
    if let Some(mut o) = world.get_resource_mut::<TimelineOps>() {
        o.add_empty_track = false;
        o.add_key = false;
        o.open_property_menu = None;
        o.delete_track = None;
        o.add_key_track = None;
        o.delete_selected_key = false;
        o.copy_keys = false;
        o.paste_keys = false;
    }

    if copy {
        super::popover::copy_selected_keys(world);
    }
    if paste {
        super::popover::paste_keys(world);
    }
    if delete_key {
        delete_selected_keys(world);
    }

    if add_empty {
        add_empty_property_track(world);
    }
    if let Some(track) = delete {
        delete_property_track(world, track);
    }
    if add_key {
        let entity = clip_entity(world);
        let scrub = world.get_resource::<AnimationEditorState>().map(|s| s.scrub_time).unwrap_or(0.0);
        if let Some(entity) = entity {
            insert_key_all_tracks(world, entity, scrub);
        }
        if let Some(mut cache) = world.get_resource_mut::<TimelineClip>() {
            super::fx::key_all(&mut cache, scrub);
        }
    }
    if let Some(track) = add_key_track {
        let entity = clip_entity(world);
        let scrub = world.get_resource::<AnimationEditorState>().map(|s| s.scrub_time).unwrap_or(0.0);
        add_key_at(world, entity, track, scrub);
    }
    if let Some((track, pos)) = open_menu {
        if let Some(entity) = clip_entity(world) {
            open_property_menu_for_track(world, entity, track, pos);
        }
    }
}

/// Finish an empty-state action once the clip it created has loaded (the
/// clip is read from disk the frame after it is written, so the action cannot
/// do this itself).
fn apply_pending_setup(world: &mut World) {
    let (pick, record) = match world.get_resource::<TimelineOps>() {
        Some(o) if o.pending_property_pick.is_some() || o.pending_record_setup => (o.pending_property_pick, o.pending_record_setup),
        _ => return,
    };
    if cur_clip(world).is_none() {
        return;
    }
    let Some(entity) = clip_entity(world) else { return };
    if let Some(mut o) = world.get_resource_mut::<TimelineOps>() {
        o.pending_property_pick = None;
        o.pending_record_setup = false;
    }
    if let Some(pos) = pick {
        add_empty_property_track(world);
        let idx = cur_clip(world).map(|c| c.property_tracks.len().saturating_sub(1)).unwrap_or(0);
        open_property_menu_for_track(world, entity, idx, pos);
    }
    if record {
        // Record only captures tracked fields, so give a fresh clip the
        // Transform's three to capture into.
        let fields = list_animatable_fields(world, entity);
        let wanted = ["translation", "rotation", "scale"];
        let picked: Vec<_> = fields
            .into_iter()
            .filter(|f| renzora::norm(&f.component) == renzora::norm("Transform") && wanted.contains(&f.field.as_str()))
            .collect();
        if let Some(mut cache) = world.get_resource_mut::<TimelineClip>() {
            if let Some(clip) = cache.clip.as_mut() {
                for f in picked {
                    let exists = clip.property_tracks.iter().any(|t| t.component == f.component && t.field == f.field);
                    if !exists {
                        clip.property_tracks.push(PropertyTrack { target: "self".into(), component: f.component, field: f.field, keys: Vec::new(), muted: false });
                    }
                }
            }
            cache.dirty = true;
        }
        if let Some(mut s) = world.get_resource_mut::<AnimationEditorState>() {
            s.record_enabled = true;
        }
    }
}

/// Delete every selected key (the primary included). Indices are removed
/// highest-first per lane, so earlier removals don't shift later ones.
pub(super) fn delete_selected_keys(world: &mut World) {
    let mut keys: Vec<SelKey> = world.get_resource::<SelectedKeys>().map(|s| s.keys.clone()).unwrap_or_default();
    if let Some(p) = world.get_resource::<SelectedKey>().and_then(|s| s.0) {
        if !keys.contains(&p) {
            keys.push(p);
        }
    }
    if keys.is_empty() {
        return;
    }
    keys.sort_by_key(|k| std::cmp::Reverse(k.index));
    if let Some(mut cache) = world.get_resource_mut::<TimelineClip>() {
        for k in &keys {
            if let Some(mut chan) = cache.lane_times(k.lane) {
                chan.remove(k.index);
            }
        }
        cache.dirty = true;
    }
    if let Some(mut s) = world.get_resource_mut::<SelectedKey>() {
        s.0 = None;
    }
    if let Some(mut s) = world.get_resource_mut::<SelectedKeys>() {
        s.clear();
    }
}

/// Mirror which entity the open clip animates into the shared
/// [`renzora::ActiveTimeline`] so the inspector can show per-property keyframe
/// buttons without linking this crate. Uses `cache.key` (not `selected_entity`)
/// so the published entity stays consistent with the loaded track buffer during
/// the one frame they differ on a selection change. Writes only on change, to
/// avoid needless change-detection churn.
pub(super) fn publish_active_timeline(
    clip: Res<TimelineClip>,
    mut active: ResMut<renzora::ActiveTimeline>,
) {
    // A clip must be loaded for the timeline to count as "active". An effect
    // shown on its own has no clip a property key could be saved into, so the
    // inspector's keyframe buttons stay hidden for it.
    let entity = clip.clip.as_ref().and(clip.key.as_ref()).filter(|_| !clip.is_effect_only()).map(|(e, _)| *e);
    if active.entity != entity {
        active.entity = entity;
    }
}

/// Drain inspector-posted keyframe requests: for each, find (or create) the
/// track for `(component, field)` on the open clip and key the entity's current
/// live value at the playhead. Exclusive because reading a live property value
/// goes through reflection (`read_track_value`, inside `add_key_at`).
pub(super) fn apply_keyframe_requests(world: &mut World) {
    let reqs = match world.get_resource_mut::<renzora::KeyframeRequests>() {
        Some(mut r) if !r.is_empty() => r.drain(),
        _ => return,
    };
    if cur_clip(world).is_none() || world.get_resource::<TimelineClip>().is_some_and(|c| c.is_effect_only()) {
        return;
    }
    let entity = clip_entity(world);
    let scrub = world
        .get_resource::<AnimationEditorState>()
        .map(|s| s.scrub_time)
        .unwrap_or(0.0);
    for req in reqs {
        add_key_for_field(world, entity, &req.component, &req.field, scrub);
    }
}

/// Key `(component, field)` at `time` from `entity`'s live value, creating the
/// track first if the clip doesn't have one yet — the inspector "keyframe this
/// field" action. An existing track is matched loosely (the inspector guesses
/// paths from its own identifiers), then keyed via its own canonical strings. A
/// *new* track is bound to the canonical `(component, field)` resolved from
/// reflection ([`list_animatable_fields`]) so the sampler can actually read it —
/// the inspector's guess only has to name the field, not drive reflection.
fn add_key_for_field(world: &mut World, entity: Option<Entity>, component: &str, field: &str, time: f32) {
    let Some(entity) = entity else { return };
    // Existing track? Key it (add_key_at reads via the track's own path).
    let existing = world
        .get_resource::<TimelineClip>()
        .and_then(|c| c.clip.as_ref())
        .and_then(|c| {
            c.property_tracks.iter().position(|t| {
                renzora::norm(&t.component) == renzora::norm(component)
                    && renzora::norm(&t.field) == renzora::norm(field)
            })
        });
    if let Some(idx) = existing {
        add_key_at(world, Some(entity), idx, time);
        return;
    }
    // No track yet — resolve the canonical animatable field and create one.
    let canon = list_animatable_fields(world, entity).into_iter().find(|f| {
        renzora::norm(&f.component) == renzora::norm(component)
            && renzora::norm(&f.field) == renzora::norm(field)
    });
    let Some(canon) = canon else {
        warn!("[prop-anim] Inspector keyframe: {}.{} is not animatable", component, field);
        return;
    };
    let idx = {
        let Some(mut cache) = world.get_resource_mut::<TimelineClip>() else { return };
        let Some(clip) = cache.clip.as_mut() else { return };
        clip.property_tracks.push(PropertyTrack {
            target: "self".into(),
            component: canon.component,
            field: canon.field,
            keys: Vec::new(),
            muted: false,
        });
        let idx = clip.property_tracks.len() - 1;
        cache.dirty = true;
        idx
    };
    add_key_at(world, Some(entity), idx, time);
}

/// Read the live value of every property track and key it at `time`.
fn insert_key_all_tracks(world: &mut World, entity: Entity, time: f32) {
    let tracks: Vec<PropertyTrack> =
        match world.get_resource::<TimelineClip>().and_then(|c| c.clip.as_ref()) {
            Some(clip) if !clip.property_tracks.is_empty() => clip.property_tracks.clone(),
            _ => return,
        };
    let values: Vec<Option<TrackValue>> =
        tracks.iter().map(|t| read_track_value(world, entity, t)).collect();
    for (pi, (t, v)) in tracks.iter().zip(&values).enumerate() {
        info!(
            "[prop-anim] Add Key: track {} {}.{} @ t={:.3} from {:?} -> {:?}",
            pi, t.component, t.field, time, entity, v
        );
    }
    if let Some(mut cache) = world.get_resource_mut::<TimelineClip>() {
        if let Some(clip) = cache.clip.as_mut() {
            let mut any = false;
            for (pi, val) in values.into_iter().enumerate() {
                let Some(val) = val else { continue };
                if let Some(pt) = clip.property_tracks.get_mut(pi) {
                    upsert_key(pt, time, val);
                    any = true;
                }
            }
            cache.dirty = cache.dirty || any;
        }
    }
}

/// Append a new empty property track (the user picks its property via the
/// in-row dropdown afterward).
fn add_empty_property_track(world: &mut World) {
    let Some(mut cache) = world.get_resource_mut::<TimelineClip>() else { return };
    // The stand-in clip of an effect shown alone is never saved; a property
    // track on it would be lost. New Clip makes a real one to key into.
    if cache.is_effect_only() {
        return;
    }
    let Some(clip) = cache.clip.as_mut() else { return };
    clip.property_tracks.push(PropertyTrack {
        target: "self".into(),
        component: String::new(),
        field: String::new(),
        keys: Vec::new(),
        muted: false,
    });
    cache.dirty = true;
}

/// Remove the property track at `track`, clearing any selection on it.
fn delete_property_track(world: &mut World, track: usize) {
    delete_track(world, RowKind::Prop(track));
}

/// Remove a bone or property track from the clip buffer (the track list's
/// right-click "Delete track", and a property row's x button). It goes through
/// the buffer like any other edit, so the undo observer snapshots it and the
/// autosave writes it.
///
/// Every selection is dropped, not only the deleted track's: selections are
/// stored by track index, and removing a track renumbers every track after it,
/// so a kept selection would silently point at a neighbour.
pub(super) fn delete_track(world: &mut World, kind: RowKind) {
    let removed = {
        let Some(mut cache) = world.get_resource_mut::<TimelineClip>() else { return };
        let Some(clip) = cache.clip.as_mut() else { return };
        let removed = match kind {
            RowKind::Bone(i) if i < clip.tracks.len() => {
                clip.tracks.remove(i);
                true
            }
            RowKind::Prop(i) if i < clip.property_tracks.len() => {
                clip.property_tracks.remove(i);
                true
            }
            _ => false,
        };
        if removed {
            cache.dirty = true;
        }
        removed
    };
    if !removed {
        return;
    }
    if let Some(mut sel) = world.get_resource_mut::<SelectedKey>() {
        sel.0 = None;
    }
    if let Some(mut all) = world.get_resource_mut::<SelectedKeys>() {
        all.clear();
    }
    if let Some(mut layout) = world.get_resource_mut::<TrackLayout>() {
        if layout.selected.is_some_and(|s| s != RowKind::Summary) {
            layout.selected = None;
        }
    }
}

/// Disable (or re-enable) a track: it stays in the clip with its keys, and
/// every sampler skips it, so the clip plays as if it were not there. A clip
/// edit like any other, so it is undoable and autosaved.
pub(super) fn set_track_muted(world: &mut World, kind: RowKind, muted: bool) {
    let Some(mut cache) = world.get_resource_mut::<TimelineClip>() else { return };
    let Some(clip) = cache.clip.as_mut() else { return };
    let flag = match kind {
        RowKind::Bone(i) => clip.tracks.get_mut(i).map(|t| &mut t.muted),
        RowKind::Prop(i) => clip.property_tracks.get_mut(i).map(|p| &mut p.muted),
        RowKind::Summary | RowKind::FxGroup | RowKind::FxEmitter(..) | RowKind::FxTrack(_) => None,
    };
    let Some(flag) = flag else { return };
    if *flag != muted {
        *flag = muted;
        cache.dirty = true;
    }
}

/// Bind a property track to `component.field` (deduped). Clears existing keys
/// since the value type may change.
fn set_property_track(world: &mut World, track: usize, component: &str, field: &str) {
    let Some(mut cache) = world.get_resource_mut::<TimelineClip>() else { return };
    let Some(clip) = cache.clip.as_mut() else { return };
    let dup = clip
        .property_tracks
        .iter()
        .enumerate()
        .any(|(i, t)| i != track && t.component == component && t.field == field);
    if dup {
        return;
    }
    if let Some(pt) = clip.property_tracks.get_mut(track) {
        pt.component = component.to_string();
        pt.field = field.to_string();
        pt.keys.clear();
    }
    cache.dirty = true;
}

/// Open the per-track property picker, listing the entity's animatable fields
/// minus those already bound on other tracks (no duplicates).
fn open_property_menu_for_track(world: &mut World, entity: Entity, track: usize, pos: Vec2) {
    let fields = list_animatable_fields(world, entity);
    if fields.is_empty() {
        return;
    }
    // Fields already bound on OTHER tracks are excluded.
    let used: std::collections::HashSet<(String, String)> = world
        .get_resource::<TimelineClip>()
        .and_then(|c| c.clip.as_ref())
        .map(|clip| {
            clip.property_tracks
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != track)
                .map(|(_, t)| (t.component.clone(), t.field.clone()))
                .collect()
        })
        .unwrap_or_default();
    let avail: Vec<_> = fields
        .into_iter()
        .filter(|f| !used.contains(&(f.component.clone(), f.field.clone())))
        .collect();
    if avail.is_empty() {
        return;
    }

    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
    let mut queue = bevy::ecs::world::CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        let menu = screen_menu(&mut commands, pos.x, pos.y);
        let kids: Vec<Entity> = avail
            .into_iter()
            .map(|f| {
                let label = format!("{} · {}", title_case(&f.component), f.label);
                let component = f.component;
                let field = f.field;
                menu_item(&mut commands, &fonts, "sliders-horizontal", &label, move |w| {
                    set_property_track(w, track, &component, &field);
                })
            })
            .collect();
        commands.entity(menu).add_children(&kids);
    }
    queue.apply(world);
}
