//! Single-line labels that end in "…" when they do not fit, instead of being
//! cut through the middle of a glyph by their parent's clip.
//!
//! bevy_text 0.20 has no elision of its own (parley lays a no-wrap line out at
//! its full width, and the node's clip rect is all that stops it), so this
//! shortens the string itself. It is built for tree rows, where a model's
//! generated names (`alex_minecraft_furry_3d_model_...`) are routinely wider
//! than the panel, and where there can be hundreds of rows on screen.
//!
//! **Why the node gets a fixed flex basis.** A no-wrap text node is as wide as
//! its text. Once the text is shortened, the node shrinks with it, and nothing
//! in the layout says how much room there is any more, so widening the panel
//! could never bring the full name back. After the full string is measured once,
//! the node's `flex_basis` is pinned to that width with `min_width: 0`: the node
//! is then `min(full width, room)` whatever it currently shows, and its
//! `ComputedNode` is the room to fit into.
//!
//! That holds only in a **row**. A flex basis is a size along the parent's main
//! axis, and in a column the main axis is height: the Inspector header's name,
//! stacked over its subtitle, was given its text's width as its *height*, and
//! the header opened up into a tall empty block. In a column the node takes the
//! column's full width instead, which is the room just the same.
//!
//! **Cost.** The system is filtered on `Changed<ComputedNode>` and
//! `Changed<TextLayoutInfo>`, which bevy_ui only touches when a value actually
//! moves, so a still panel costs nothing. A resize shortens each visible label
//! from a per-character width table taken from the one full-width layout; the
//! measured result of that guess is then checked against the room and corrected
//! a character at a time, which a good guess makes zero or one extra frame.

use bevy::prelude::*;
use bevy::text::TextLayoutInfo;
use bevy::ui::ComputedNode;

use super::HoverTooltip;

/// What a shortened label ends with. A single character, so it costs one
/// glyph's width rather than three dots' spacing.
const ELLIPSIS: char = '\u{2026}';

/// Slack (physical px) before a label counts as overflowing: layout and the
/// glyph table round independently, and a sub-pixel disagreement must not trim
/// a name that visibly fits.
const SLACK: f32 = 0.5;

/// Marks a single-line `Text` as one to shorten with "…" when it does not fit.
/// Insert with [`EllipsisText::new`] on the text entity, which should carry
/// `TextLayout::no_wrap()`; [`ellipsize`] does both.
#[derive(Component)]
pub struct EllipsisText {
    /// The whole string. Anything else that writes this entity's `Text` is
    /// taken as a new full string (the label was renamed in place).
    full: String,
    /// The string this component last wrote, to tell its own writes from
    /// someone else's.
    written: String,
    /// The full string's measured width (physical px) at `scale`, once known.
    full_w: Option<f32>,
    scale: f32,
    /// Width (physical px) of the first `i` characters of `full`, for
    /// `i in 0..=char_count`. Empty when the glyphs could not be matched to the
    /// characters, in which case widths are estimated proportionally.
    prefix: Vec<f32>,
    /// Characters of `full` shown before the ellipsis; `None` shows it whole.
    shown: Option<usize>,
    /// The smallest count found to overflow at `room`: the search never climbs
    /// back to it, which is what stops it alternating between two lengths.
    too_long: usize,
    /// The room (physical px) the current `shown` was fitted to.
    room: f32,
    /// An entity with `UiInteraction` to carry the full name as a
    /// [`HoverTooltip`] while the label is shortened.
    tooltip_host: Option<Entity>,
}

impl EllipsisText {
    pub fn new(full: impl Into<String>) -> Self {
        let full = full.into();
        Self {
            written: full.clone(),
            full,
            full_w: None,
            scale: 0.0,
            prefix: Vec::new(),
            shown: None,
            too_long: usize::MAX,
            room: -1.0,
            tooltip_host: None,
        }
    }

    /// Show the full name as a tooltip on `host` while the label is shortened.
    /// `host` must track `UiInteraction` (a row's hit area, or the label itself
    /// when it is interactive).
    pub fn with_tooltip_on(mut self, host: Entity) -> Self {
        self.tooltip_host = Some(host);
        self
    }
}

/// Make `text` a label that shortens with "…": no wrap, allowed to shrink below
/// its text's width, and marked for [`ellipsize_text`]. `full` is the string
/// the entity was spawned with; `tooltip_host`, see
/// [`EllipsisText::with_tooltip_on`].
pub fn ellipsize(
    commands: &mut Commands,
    text: Entity,
    full: impl Into<String>,
    tooltip_host: Option<Entity>,
) {
    let mut e = EllipsisText::new(full);
    e.tooltip_host = tooltip_host;
    commands.entity(text).insert((e, bevy::text::TextLayout::no_wrap()));
    commands.entity(text).entry::<Node>().or_default().and_modify(|mut n| {
        n.min_width = Val::Px(0.0);
        n.flex_shrink = 1.0;
        n.overflow = Overflow::clip();
    });
}

/// Width table for `full` from its full-width layout: the right edge of each
/// character's glyph. Glyphs are matched to characters one to one, or to the
/// non-whitespace characters when the shaper emitted no glyph for spaces; any
/// other mismatch (ligatures, combining marks) returns an empty table and the
/// caller estimates instead.
fn prefix_widths(full: &str, layout: &TextLayoutInfo) -> Vec<f32> {
    let chars: Vec<char> = full.chars().collect();
    let right = |g: &bevy::text::PositionedGlyph| g.position.x + g.atlas_info.rect.width() * 0.5;
    let glyphs = &layout.glyphs;
    let all = glyphs.len() == chars.len();
    let non_ws = glyphs.len() == chars.iter().filter(|c| !c.is_whitespace()).count();
    if !all && !non_ws {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(chars.len() + 1);
    out.push(0.0);
    let (mut gi, mut edge) = (0usize, 0.0f32);
    for c in &chars {
        if all || !c.is_whitespace() {
            if let Some(g) = glyphs.get(gi) {
                edge = edge.max(right(g));
            }
            gi += 1;
        }
        out.push(edge);
    }
    out
}

/// The most characters of `full` that fit in `room` alongside the ellipsis.
fn fit_count(e: &EllipsisText, full_w: f32, n: usize, room: f32) -> usize {
    let avg = full_w / n.max(1) as f32;
    // The ellipsis is about a wide letter's width; a wrong guess is corrected
    // by the overflow check once the shortened text has been laid out.
    let budget = room - avg * 1.2;
    if budget <= 0.0 {
        return 0;
    }
    if e.prefix.len() == n + 1 {
        e.prefix.iter().rposition(|w| *w <= budget).unwrap_or(0)
    } else {
        ((budget / avg).floor() as usize).min(n)
    }
}

fn shortened(full: &str, k: usize) -> String {
    let head: String = full.chars().take(k).collect();
    let mut s = head.trim_end().to_string();
    s.push(ELLIPSIS);
    s
}

/// Fit every [`EllipsisText`] whose room or layout changed. See the module doc.
pub(crate) fn ellipsize_text(
    mut commands: Commands,
    mut labels: Query<
        (&mut EllipsisText, &mut Text, &mut Node, &ComputedNode, &TextLayoutInfo, Option<&ChildOf>),
        Or<(Changed<ComputedNode>, Changed<TextLayoutInfo>, Changed<Text>)>,
    >,
    parents: Query<&Node, Without<EllipsisText>>,
) {
    for (mut e, mut text, mut node, computed, layout, child_of) in &mut labels {
        let in_column = child_of
            .and_then(|c| parents.get(c.parent()).ok())
            .is_some_and(|p| matches!(p.flex_direction, FlexDirection::Column | FlexDirection::ColumnReverse));
        // Someone else wrote the label: that is the new full string.
        if text.0 != e.written {
            let full = text.0.clone();
            let host = e.tooltip_host;
            *e = EllipsisText::new(full);
            e.tooltip_host = host;
            if node.flex_basis != Val::Auto {
                node.flex_basis = Val::Auto;
            }
            if let Some(h) = host {
                commands.entity(h).try_remove::<HoverTooltip>();
            }
            continue;
        }
        // Measure the full string once (again after a DPI change), with the
        // full string showing and the basis back on auto.
        if e.full_w.is_none() || (layout.scale_factor - e.scale).abs() > 1e-3 {
            if e.shown.is_some() || node.flex_basis != Val::Auto {
                e.shown = None;
                e.full_w = None;
                e.written = e.full.clone();
                text.0 = e.full.clone();
                node.flex_basis = Val::Auto;
                continue;
            }
            if layout.size.x <= 0.0 && !e.full.is_empty() {
                continue;
            }
            e.full_w = Some(layout.size.x);
            e.scale = layout.scale_factor;
            e.prefix = prefix_widths(&e.full, layout);
            if in_column {
                // See the module doc: a basis would be a height here.
                if node.width != Val::Percent(100.0) {
                    node.width = Val::Percent(100.0);
                }
            } else {
                let logical = layout.size.x / layout.scale_factor.max(1e-3);
                node.flex_basis = Val::Px(logical.ceil());
            }
            // Fit now, against the room this frame's layout already gave.
            // Waiting for the next size change instead left a label that is
            // capped by its own `max_width` (a breadcrumb segment) unfitted
            // for good: its size was already final, so no change ever came,
            // and the full name spilled over the next segment.
        }
        let Some(full_w) = e.full_w else { continue };
        let room = computed.size().x;
        let n = e.full.chars().count();

        let want = if room + SLACK >= full_w {
            None
        } else if (room - e.room).abs() > SLACK || e.shown.is_none() {
            // The room moved: start a fresh search from the table's guess.
            e.room = room;
            e.too_long = usize::MAX;
            Some(fit_count(&e, full_w, n, room))
        } else if let Some(k) = e.shown {
            // Same room, new layout of our own shortened text: correct the
            // guess against what it actually measured.
            if layout.size.x > room + SLACK && k > 0 {
                e.too_long = e.too_long.min(k);
                Some(k - 1)
            } else if k + 1 < e.too_long && k + 1 < n {
                let next_w = if e.prefix.len() == n + 1 {
                    layout.size.x + (e.prefix[k + 1] - e.prefix[k])
                } else {
                    layout.size.x + full_w / n.max(1) as f32
                };
                if next_w + SLACK <= room { Some(k + 1) } else { Some(k) }
            } else {
                Some(k)
            }
        } else {
            None
        };
        if want == e.shown {
            continue;
        }
        let was_short = e.shown.is_some();
        e.shown = want;
        let s = match want {
            Some(k) => shortened(&e.full, k),
            None => e.full.clone(),
        };
        e.written = s.clone();
        text.0 = s;
        if let Some(h) = e.tooltip_host {
            match (was_short, want.is_some()) {
                (false, true) => {
                    commands.entity(h).try_insert(HoverTooltip::new(e.full.clone()));
                }
                (true, false) => {
                    commands.entity(h).try_remove::<HoverTooltip>();
                }
                _ => {}
            }
        }
    }
}
