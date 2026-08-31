//! Align + distribute math over selected widgets' design-space geometry.
//! Matches the egui canvas's `compute_align` / `compute_distribute_*`.

use bevy::prelude::*;

use crate::game_ui::geometry::WidgetGeom;

#[derive(Clone, Copy)]
pub(crate) enum AlignAction {
    Left,
    CenterH,
    Right,
    Top,
    CenterV,
    Bottom,
}

impl AlignAction {
    /// Whether this action acts along the horizontal axis.
    fn horizontal(self) -> bool {
        matches!(self, Self::Left | Self::CenterH | Self::Right)
    }

    /// `start` / `center` / `end`, the shared vocabulary of `align_self` and
    /// `justify_content`.
    fn edge(self) -> &'static str {
        match self {
            Self::Left | Self::Top => "start",
            Self::CenterH | Self::CenterV => "center",
            Self::Right | Self::Bottom => "end",
        }
    }

    /// How to express this alignment on a node laid out by its parent's flex
    /// flow, given whether that parent is a row.
    ///
    /// Nudging `left`/`top` — which is what aligning does to a free node — is
    /// simply ignored on a flex child, so the buttons did nothing at all on the
    /// nodes a template is mostly made of. Flexbox splits the job in two, and
    /// which attribute applies depends on the axis:
    ///
    /// - **Cross axis** is the child's own business: `align_self` on the node.
    /// - **Main axis** is the parent's distribution: `justify_content` on the
    ///   parent, which moves *all* its children. That is flexbox's model, not a
    ///   shortcut — there is no per-child main-axis alignment.
    pub(crate) fn flow_attr(self, parent_is_row: bool) -> FlowAlign {
        if self.horizontal() == parent_is_row {
            FlowAlign::Parent("justify_content", self.edge())
        } else {
            FlowAlign::Own("align_self", self.edge())
        }
    }
}

/// Where a flow alignment has to be written.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum FlowAlign {
    /// On the node itself (`align_self`).
    Own(&'static str, &'static str),
    /// On its parent (`justify_content`), because flexbox has no per-child
    /// main-axis alignment.
    Parent(&'static str, &'static str),
}

/// New (x, y) design-space top-left for each widget.
pub(crate) fn compute_align(widgets: &[WidgetGeom], action: AlignAction) -> Vec<(Entity, f32, f32)> {
    if widgets.is_empty() {
        return vec![];
    }
    match action {
        AlignAction::Left => {
            let min_x = widgets.iter().map(|w| w.x).fold(f32::MAX, f32::min);
            widgets.iter().map(|w| (w.entity, min_x, w.y)).collect()
        }
        AlignAction::Right => {
            let max_right = widgets.iter().map(|w| w.x + w.width).fold(f32::MIN, f32::max);
            widgets.iter().map(|w| (w.entity, max_right - w.width, w.y)).collect()
        }
        AlignAction::CenterH => {
            let min_x = widgets.iter().map(|w| w.x).fold(f32::MAX, f32::min);
            let max_right = widgets.iter().map(|w| w.x + w.width).fold(f32::MIN, f32::max);
            let center = (min_x + max_right) / 2.0;
            widgets.iter().map(|w| (w.entity, center - w.width / 2.0, w.y)).collect()
        }
        AlignAction::Top => {
            let min_y = widgets.iter().map(|w| w.y).fold(f32::MAX, f32::min);
            widgets.iter().map(|w| (w.entity, w.x, min_y)).collect()
        }
        AlignAction::Bottom => {
            let max_bottom = widgets.iter().map(|w| w.y + w.height).fold(f32::MIN, f32::max);
            widgets.iter().map(|w| (w.entity, w.x, max_bottom - w.height)).collect()
        }
        AlignAction::CenterV => {
            let min_y = widgets.iter().map(|w| w.y).fold(f32::MAX, f32::min);
            let max_bottom = widgets.iter().map(|w| w.y + w.height).fold(f32::MIN, f32::max);
            let center = (min_y + max_bottom) / 2.0;
            widgets.iter().map(|w| (w.entity, w.x, center - w.height / 2.0)).collect()
        }
    }
}

/// New x for each widget, evenly spaced left→right (needs ≥3).
pub(crate) fn compute_distribute_h(widgets: &[WidgetGeom]) -> Vec<(Entity, f32)> {
    if widgets.len() < 3 {
        return vec![];
    }
    let mut sorted: Vec<&WidgetGeom> = widgets.iter().collect();
    sorted.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
    let first = sorted.first().unwrap().x;
    let last = sorted.last().unwrap().x;
    let step = (last - first) / (sorted.len() - 1) as f32;
    sorted.iter().enumerate().map(|(i, w)| (w.entity, first + step * i as f32)).collect()
}

/// New y for each widget, evenly spaced top→bottom (needs ≥3).
pub(crate) fn compute_distribute_v(widgets: &[WidgetGeom]) -> Vec<(Entity, f32)> {
    if widgets.len() < 3 {
        return vec![];
    }
    let mut sorted: Vec<&WidgetGeom> = widgets.iter().collect();
    sorted.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal));
    let first = sorted.first().unwrap().y;
    let last = sorted.last().unwrap().y;
    let step = (last - first) / (sorted.len() - 1) as f32;
    sorted.iter().enumerate().map(|(i, w)| (w.entity, first + step * i as f32)).collect()
}
