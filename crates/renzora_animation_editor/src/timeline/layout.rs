//! Which rows the dopesheet shows, in what order, and how far they are
//! scrolled: the track list's nesting, collapse and filter, resolved once into
//! a flat list every other module indexes into.
//!
//! Before this, a lane's vertical position *was* its track index. That made a
//! 60-bone rig a 60-row wall with no way to fold a leg away, no filter, and no
//! vertical scroll (rows past the panel's bottom were simply unreachable).
//! Resolving the visible rows here once, when the clip / collapse / filter
//! actually change, lets the header list, the lanes, picking and box-select all
//! agree on one `row -> y` mapping that does not cost a walk of the clip per
//! frame.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use bevy::ui::ComputedNode;

use renzora_ember::widgets::EmberTextInput;

use crate::AnimationEditorState;

use super::clip::TimelineClip;
use super::{KeyLane, TrackFilterField};

/// What one visible row shows.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum RowKind {
    /// The "All tracks" row at the top: every track's keys merged.
    Summary,
    /// A skeletal bone track (its T/R/S channels share the row).
    Bone(usize),
    /// A property track.
    Prop(usize),
    /// The Particles group's head: "Particles · Campfire".
    FxGroup,
    /// An effect emitter, drawn as its on/off bar. The second field is its
    /// `Enabled` track, whose keys are the bar's switches, once it has one.
    FxEmitter(usize, Option<usize>),
    /// One of the effect's other tracks (a keyed parameter or bursts), by
    /// index into its `VfxAsset::tracks`.
    FxTrack(usize),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Row {
    pub(crate) kind: RowKind,
    /// Nesting depth in the track list (bones only; 0 for the rest).
    pub(crate) depth: u8,
    /// A bone with child bones in the clip, so it shows a caret.
    pub(crate) has_children: bool,
    pub(crate) collapsed: bool,
}

/// The resolved row list plus the scroll and size of the lane area.
#[derive(Resource, Default)]
pub(crate) struct TrackLayout {
    pub(crate) rows: Vec<Row>,
    index: HashMap<RowKind, usize>,
    /// Per bone track, the bone track it nests under.
    bone_parent: Vec<Option<usize>>,
    /// The clip the parent table was resolved for.
    hierarchy_for: Option<(Entity, String, usize)>,
    /// Collapsed bones, by bone name (survives clip reloads and reorders).
    pub(crate) collapsed: HashSet<String>,
    pub(crate) filter: String,
    /// The row picked in the track list (also the curves view's subject).
    pub(crate) selected: Option<RowKind>,
    /// Vertical scroll of the rows, px.
    pub(crate) vscroll: f32,
    /// Logical size of the lane area, px.
    pub(crate) view: Vec2,
    /// Bumped whenever `rows` is rebuilt; part of every draw token.
    pub(crate) gen: u64,
    sig: u64,
}

impl TrackLayout {
    pub(crate) fn row_of(&self, kind: RowKind) -> Option<usize> {
        self.index.get(&kind).copied()
    }

    /// The first and one-past-last row index intersecting the visible area.
    pub(crate) fn visible_range(&self, th: f32) -> (usize, usize) {
        let th = th.max(1.0);
        let first = (self.vscroll / th).floor().max(0.0) as usize;
        let last = ((self.vscroll + self.view.y.max(th)) / th).ceil() as usize + 1;
        (first.min(self.rows.len()), last.min(self.rows.len()))
    }

    /// Top of row `row` in lane space.
    pub(crate) fn row_top(&self, row: usize, th: f32) -> f32 {
        row as f32 * th - self.vscroll
    }

    /// Content height plus room for the horizontal scrollbar, so the last
    /// row can scroll clear of it.
    pub(crate) fn content_h(&self, th: f32) -> f32 {
        self.rows.len() as f32 * th + super::scrollbars::BAR
    }

    pub(crate) fn max_vscroll(&self, th: f32) -> f32 {
        (self.content_h(th) - self.view.y).max(0.0)
    }

    /// The row under a lane-space y, if any.
    pub(crate) fn row_at(&self, y: f32, th: f32) -> Option<usize> {
        let r = ((y + self.vscroll) / th.max(1.0)).floor();
        (r >= 0.0 && (r as usize) < self.rows.len()).then_some(r as usize)
    }
}

/// Resolve which bone track nests under which, by walking the animated
/// entity's own hierarchy: a track's parent is the nearest ancestor entity
/// whose `Name` matches another track's bone. Clips carry only bone names, so
/// the scene is the only place the shape of the rig is written down.
fn resolve_parents(
    root: Entity,
    names: &[String],
    children: &Query<&Children>,
    name_q: &Query<&Name>,
) -> Vec<Option<usize>> {
    let by_name: HashMap<&str, usize> =
        names.iter().enumerate().map(|(i, n)| (n.as_str(), i)).collect();
    let mut out = vec![None; names.len()];
    // DFS carrying the nearest track-bearing ancestor.
    let mut stack: Vec<(Entity, Option<usize>)> = vec![(root, None)];
    let mut guard = 0usize;
    while let Some((e, above)) = stack.pop() {
        guard += 1;
        if guard > 100_000 {
            break;
        }
        let mine = name_q.get(e).ok().and_then(|n| by_name.get(n.as_str()).copied());
        if let Some(i) = mine {
            if out[i].is_none() {
                out[i] = above.filter(|p| *p != i);
            }
        }
        let next_above = mine.or(above);
        if let Ok(kids) = children.get(e) {
            for c in kids.iter() {
                stack.push((c, next_above));
            }
        }
    }
    out
}

/// Rebuild [`TrackLayout::rows`] when the clip, the collapsed set or the
/// filter changed, and keep the lane-area size and scroll clamp current.
#[allow(clippy::too_many_arguments)]
pub(super) fn rebuild_layout(
    cache: Res<TimelineClip>,
    state: Option<Res<AnimationEditorState>>,
    mut layout: ResMut<TrackLayout>,
    filter: Query<&EmberTextInput, With<TrackFilterField>>,
    lane: Query<&ComputedNode, With<KeyLane>>,
    children: Query<&Children>,
    names: Query<&Name>,
) {
    let th = state.as_ref().map(|s| s.track_height).unwrap_or(super::ROW_DEFAULT).max(1.0);
    if let Some(cn) = lane.iter().next() {
        let size = cn.size() * cn.inverse_scale_factor();
        if (size - layout.view).abs().max_element() > 0.5 {
            layout.view = size;
        }
    }
    let f = filter.iter().next().map(|f| f.value.trim().to_lowercase()).unwrap_or_default();
    if f != layout.filter {
        layout.filter = f;
    }

    let Some(clip) = cache.clip.as_ref() else {
        if !layout.rows.is_empty() {
            layout.rows.clear();
            layout.index.clear();
            layout.gen = layout.gen.wrapping_add(1);
            layout.sig = 0;
        }
        return;
    };

    // Parent table: only when the clip identity or its track count changes.
    let hkey = cache.key.as_ref().map(|(e, n)| (*e, n.clone(), clip.tracks.len()));
    if hkey != layout.hierarchy_for {
        let bone_names: Vec<String> = clip.tracks.iter().map(|t| t.bone_name.clone()).collect();
        layout.bone_parent = match hkey.as_ref() {
            Some((root, _, _)) => resolve_parents(*root, &bone_names, &children, &names),
            None => vec![None; bone_names.len()],
        };
        layout.hierarchy_for = hkey;
    }

    let mut h = std::collections::hash_map::DefaultHasher::new();
    cache.key.hash(&mut h);
    clip.tracks.len().hash(&mut h);
    clip.property_tracks.len().hash(&mut h);
    for t in &clip.tracks {
        t.bone_name.hash(&mut h);
    }
    let mut collapsed: Vec<&String> = layout.collapsed.iter().collect();
    collapsed.sort();
    collapsed.hash(&mut h);
    layout.filter.hash(&mut h);
    layout.bone_parent.hash(&mut h);
    if let Some(fx) = cache.fx.as_ref() {
        super::fx::structure_hash(fx, &mut h);
    }
    let sig = h.finish();

    if sig != layout.sig {
        let mut rows = build_rows(clip, &layout.bone_parent, &layout.collapsed, &layout.filter);
        if let Some(fx) = cache.fx.as_ref() {
            push_fx_rows(&mut rows, fx, &layout.filter);
        }
        layout.index = rows.iter().enumerate().map(|(i, r)| (r.kind, i)).collect();
        layout.rows = rows;
        layout.sig = sig;
        layout.gen = layout.gen.wrapping_add(1);
        if layout.selected.is_some_and(|k| !layout.index.contains_key(&k)) {
            layout.selected = None;
        }
    }

    let max = layout.max_vscroll(th);
    if layout.vscroll > max {
        layout.vscroll = max;
    }
}

/// The Particles group under the clip's tracks: its head, then per emitter
/// its on/off row and its other tracks nested one level in. A filter keeps
/// the rows whose name matches (and the head, when any does), flat like the
/// clip's own filtered rows.
fn push_fx_rows(rows: &mut Vec<Row>, fx: &super::fx::FxDoc, filter: &str) {
    let flat = |kind, depth| Row { kind, depth, has_children: false, collapsed: false };
    let mut fx_rows: Vec<Row> = Vec::new();
    for (e, emitter) in fx.doc.emitters.iter().enumerate() {
        let matches = |label: &str| filter.is_empty() || label.to_lowercase().contains(filter);
        if matches(&emitter.name) {
            fx_rows.push(flat(RowKind::FxEmitter(e, fx.enabled_track(e)), 0));
        }
        for (i, t) in fx.doc.tracks.iter().enumerate() {
            if t.emitter != e || t.target == renzora_particles::params::ParamTarget::Enabled {
                continue;
            }
            if matches(&renzora_particles::params::track_label(&fx.doc, t)) {
                fx_rows.push(flat(RowKind::FxTrack(i), if filter.is_empty() { 1 } else { 0 }));
            }
        }
    }
    if filter.is_empty() || !fx_rows.is_empty() {
        rows.push(flat(RowKind::FxGroup, 0));
        rows.extend(fx_rows);
    }
}

fn build_rows(
    clip: &renzora_animation::AnimClip,
    parent: &[Option<usize>],
    collapsed: &HashSet<String>,
    filter: &str,
) -> Vec<Row> {
    let mut rows = vec![Row { kind: RowKind::Summary, depth: 0, has_children: false, collapsed: false }];
    let n = clip.tracks.len();
    if !filter.is_empty() {
        // A filtered list is flat: nesting only gets in the way of finding a
        // bone by name, and a match's collapsed ancestor must not hide it.
        for (i, t) in clip.tracks.iter().enumerate() {
            if t.bone_name.to_lowercase().contains(filter) {
                rows.push(Row { kind: RowKind::Bone(i), depth: 0, has_children: false, collapsed: false });
            }
        }
        for (i, p) in clip.property_tracks.iter().enumerate() {
            let label = format!("{} {}", p.component, p.field).to_lowercase();
            if label.contains(filter) {
                rows.push(Row { kind: RowKind::Prop(i), depth: 0, has_children: false, collapsed: false });
            }
        }
        return rows;
    }
    let mut kids: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut roots: Vec<usize> = Vec::new();
    for i in 0..n {
        match parent.get(i).copied().flatten() {
            Some(p) if p < n => kids[p].push(i),
            _ => roots.push(i),
        }
    }
    // Iterative DFS in clip order; a cycle (shouldn't happen) is cut by `seen`.
    let mut seen = vec![false; n];
    let mut stack: Vec<(usize, u8)> = roots.into_iter().rev().map(|r| (r, 0)).collect();
    while let Some((i, d)) = stack.pop() {
        if seen[i] {
            continue;
        }
        seen[i] = true;
        let has_children = !kids[i].is_empty();
        let is_collapsed = has_children && collapsed.contains(&clip.tracks[i].bone_name);
        rows.push(Row { kind: RowKind::Bone(i), depth: d.min(12), has_children, collapsed: is_collapsed });
        if is_collapsed {
            // Hidden, not lost: mark the subtree seen so the cycle fallback
            // below does not hand it rows of its own.
            let mut hide: Vec<usize> = kids[i].clone();
            while let Some(c) = hide.pop() {
                if !seen[c] {
                    seen[c] = true;
                    hide.extend(kids[c].iter().copied());
                }
            }
        } else {
            for &c in kids[i].iter().rev() {
                stack.push((c, d.saturating_add(1)));
            }
        }
    }
    // Any bone left unreached (a cycle) still gets a row.
    for (i, s) in seen.iter().enumerate() {
        if !s {
            rows.push(Row { kind: RowKind::Bone(i), depth: 0, has_children: false, collapsed: false });
        }
    }
    for i in 0..clip.property_tracks.len() {
        rows.push(Row { kind: RowKind::Prop(i), depth: 0, has_children: false, collapsed: false });
    }
    rows
}
