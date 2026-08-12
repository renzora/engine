//! The animated "glow box" that frames the on-screen element the current step
//! wants the user to interact with (the workspace ribbon, the dock area, the
//! theme menu — see [`crate::steps::highlight_for`]), plus the bobbing **arrow**
//! that points at it.
//!
//! A single persistent, **click-through** box (`FocusPolicy::Pass`) is moved over
//! the target node's screen rect each frame and its border pulses. The target is
//! found by bevy_ui `Name`; its logical rect comes from `ComputedNode` (size) +
//! `GlobalTransform` (center) — the shell compares UI `GlobalTransform.translation`
//! directly against the logical cursor, so that translation is in logical px.
//!
//! The arrow is a second click-through node parked just outside that rect,
//! nudging toward it on a sine. It sits *below* the target pointing up, unless
//! the target is close enough to the bottom of the window that the arrow would
//! be clipped (the status-bar theme menu) — then it flips above and points down.
//! Both the flip and the glyph swap are written only when they actually change,
//! so an idle arrow doesn't dirty `Text` (and re-run text layout) every frame.

use bevy::prelude::*;
use bevy::ui::{CalculatedClip, ComputedNode, FocusPolicy, UiGlobalTransform};
use bevy::window::PrimaryWindow;

use renzora::core::{EditorLocked, HideInHierarchy};
use renzora_ember::game_ui::shapes::TriangleShape;
use renzora_ember::theme::{accent, play_green};

use crate::state::TutorialState;
use crate::steps::highlight_for;

/// The arrowhead's box. **Square on purpose**: `triangle.wgsl` evaluates its SDF
/// in normalised `uv * 2 - 1` space, so the triangle stretches with the node's
/// aspect ratio — a wide, short node renders a squashed sliver rather than an
/// arrowhead.
///
/// It's much bigger than the triangle it draws, because the shader only fills
/// part of its box — see [`HEAD_APEX_FRAC`]. At 44px the visible head is ~19px
/// across, which is what actually matters.
const HEAD_SIZE: f32 = 44.0;

/// Where the drawn triangle sits inside that box, as fractions of node height,
/// when rotated to point up.
///
/// Derived from `triangle.wgsl`: it evaluates iq's unit equilateral SDF — apex at
/// `y = 1/√3`, base at `y = -1/(2√3)` — on `(uv * 2 - 1) * 1.15`. Dividing by
/// 1.15 and mapping `uv.y ∈ [-1, 1]` onto the node gives base at 37.4% and apex
/// at 75.1% from the top; rotating 180° to point up mirrors those to 24.9% and
/// 62.6%. **Everything else in the box is transparent.**
///
/// Both constants are load-bearing. Without the apex offset the arrow floats a
/// quarter of its own height away from whatever it's pointing at; without the
/// base offset there's a visible gap between the head and its tail, and the two
/// read as separate marks rather than one arrow.
const HEAD_APEX_FRAC: f32 = 0.249;
const HEAD_BASE_FRAC: f32 = 0.626;

/// The tail.
const STEM_W: f32 = 6.0;
const STEM_H: f32 = 14.0;

/// How far the tail is pulled back into the head's empty lower region so the two
/// touch.
const OVERLAP: f32 = (1.0 - HEAD_BASE_FRAC) * HEAD_SIZE;
/// Transparent lead-in between the arrow box's near edge and the apex.
const LEAD_IN: f32 = HEAD_APEX_FRAC * HEAD_SIZE;
/// The arrow's laid-out box …
const ARROW_H: f32 = HEAD_SIZE + STEM_H - OVERLAP;
/// … and how much of it is actually inked, which is what the placement maths and
/// the flip-when-clipped test care about.
const ARROW_INK_H: f32 = (HEAD_BASE_FRAC - HEAD_APEX_FRAC) * HEAD_SIZE + STEM_H;

/// Rotation (degrees) to make the head point up / down.
///
/// bevy_ui's `uv.y` grows **downward**, but `sdf_triangle` is written in maths
/// orientation (y up) — so the shape lands mirrored and an unrotated
/// `TriangleShape` renders pointing *down*. These two constants are that flip,
/// named rather than left as bare 0/180 so the next person doesn't "fix" them.
const HEAD_UP: f32 = 180.0;
const HEAD_DOWN: f32 = 0.0;
/// Gap between the highlight box's edge and the arrow's tip.
const ARROW_GAP: f32 = 6.0;
/// How far the arrow travels toward the target on each bob.
const ARROW_BOB: f32 = 7.0;

/// The single reusable highlight box (hidden when the step has no target).
#[derive(Component)]
pub struct HighlightBox;

/// The single reusable pointer arrow. `pointing_down` caches which way the glyph
/// currently faces so [`update_highlight`] only rewrites `Text` on a real flip.
#[derive(Component)]
pub struct HighlightArrow {
    pointing_down: bool,
}

/// Spawn the (initially hidden) highlight box. Click-through so it never blocks
/// the element it frames.
pub fn spawn_box(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(0.0),
                height: Val::Px(0.0),
                border: UiRect::all(Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(Color::NONE),
            FocusPolicy::Pass,
            GlobalZIndex(8150),
            HideInHierarchy,
            EditorLocked,
            HighlightBox,
            Name::new("tutorial-highlight"),
        ))
        .id()
}

/// Spawn the (initially hidden) pointer arrow. Click-through for the same reason
/// the box is: it hovers over neighbouring chrome and must never eat a click.
///
/// **Not a Phosphor glyph.** The embedded `phosphor.ttf` is regular weight only —
/// its cmap has no odd-codepoint `-fill` variants — so every arrow it can draw,
/// `arrow-fat-up` included, is an *outline*. A solid arrow has to be drawn, so
/// this uses ember's `TriangleShape` GPU widget (a filled SDF triangle, rotation
/// in degrees, 0 = up) with a rectangular stem under it.
pub fn spawn_arrow(commands: &mut Commands) -> Entity {
    let arrow = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Px(HEAD_SIZE),
                height: Val::Px(ARROW_H),
                display: Display::None,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            FocusPolicy::Pass,
            GlobalZIndex(8151),
            HideInHierarchy,
            EditorLocked,
            HighlightArrow {
                pointing_down: false,
            },
            Name::new("tutorial-highlight-arrow"),
        ))
        .id();
    // Head and tail as separate children, so flipping is a column reversal (they
    // swap ends) plus one rotation — no geometry rebuild.
    let head = commands
        .spawn((
            Node {
                width: Val::Px(HEAD_SIZE),
                height: Val::Px(HEAD_SIZE),
                flex_shrink: 0.0,
                ..default()
            },
            TriangleShape {
                color: green(1.0),
                rotation: HEAD_UP,
                ..default()
            },
            FocusPolicy::Pass,
            ArrowHead,
        ))
        .id();
    let stem = commands
        .spawn((
            Node {
                width: Val::Px(STEM_W),
                height: Val::Px(STEM_H),
                flex_shrink: 0.0,
                // Tucked up into the head's transparent lower half so head and
                // tail meet. Flipped to a bottom margin when the arrow points
                // down — see `update_highlight`.
                margin: UiRect::top(Val::Px(-OVERLAP)),
                ..default()
            },
            BackgroundColor(green(1.0)),
            FocusPolicy::Pass,
            ArrowStem,
        ))
        .id();
    commands.entity(arrow).add_children(&[head, stem]);
    arrow
}

/// The arrow's filled triangular head.
#[derive(Component)]
pub struct ArrowHead;

/// The arrow's rectangular tail.
#[derive(Component)]
pub struct ArrowStem;

/// The theme's "go" green at `alpha` — the arrow reads as "do this" against the
/// accent-coloured highlight box.
fn green(alpha: f32) -> Color {
    let (r, g, b) = play_green();
    Color::srgba(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, alpha)
}

/// Track the current step's target node each frame: position the box over it and
/// pulse its border (width + alpha) for an animated glow, and park the bobbing
/// arrow just outside it. Hides both when the step has no target or the target
/// isn't on screen.
#[allow(clippy::too_many_arguments)]
pub fn update_highlight(
    time: Res<Time>,
    state: Res<TutorialState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    // UI nodes position via `UiGlobalTransform` (physical px), NOT the regular
    // `GlobalTransform` — using the latter matched nothing, so the box never
    // showed. `ComputedNode` gives size + the inverse scale factor to logical px.
    targets: Query<(
        Entity,
        &Name,
        &ComputedNode,
        &UiGlobalTransform,
        Option<&CalculatedClip>,
    )>,
    // Parent lookup, for picking the most prominent of several same-named
    // matches — see `target_rect`.
    nodes: Query<&ComputedNode>,
    parents: Query<&ChildOf>,
    mut boxes: Query<(&mut Node, &mut BorderColor), With<HighlightBox>>,
    mut arrows: Query<(&mut Node, &mut HighlightArrow), Without<HighlightBox>>,
    mut heads: Query<&mut TriangleShape, With<ArrowHead>>,
    mut stems: Query<&mut BackgroundColor, With<ArrowStem>>,
    mut stem_nodes: Query<
        &mut Node,
        (With<ArrowStem>, Without<HighlightBox>, Without<HighlightArrow>),
    >,
) {
    let Ok((mut node, mut border)) = boxes.single_mut() else {
        return;
    };

    // Resolve the step's target to a logical-px rect, or `None` if this step has
    // no target / the target isn't on screen. Both the box and the arrow key off
    // this one answer, so they can never disagree about whether to show.
    // The picker and the completion card have no target, and `step()` is `None`
    // for both — so this one expression covers every "nothing to point at" case.
    //
    // `!step_done` is part of it: once the action is performed the card swaps to
    // its success message, and a box still pulsing around the button you just
    // pressed reads as "again". The prompts stay down until Continue starts the
    // next step.
    let rect = (state.active && !state.step_done)
        .then(|| state.step())
        .flatten()
        .map(|step| highlight_for(step.kind))
        .and_then(|names| {
            names
                .iter()
                .find_map(|n| target_rect(&targets, &nodes, &parents, n))
        });

    let Some((top_left, size)) = rect else {
        node.display = Display::None;
        if let Ok((mut a_node, ..)) = arrows.single_mut() {
            a_node.display = Display::None;
        }
        return;
    };

    const PAD: f32 = 4.0;
    node.display = Display::Flex;
    node.left = Val::Px(top_left.x - PAD);
    node.top = Val::Px(top_left.y - PAD);
    node.width = Val::Px(size.x + PAD * 2.0);
    node.height = Val::Px(size.y + PAD * 2.0);

    // Animated glow: pulse border width + alpha together.
    let t = time.elapsed_secs();
    let pulse = 0.5 + 0.5 * (t * 3.5).sin();
    node.border = UiRect::all(Val::Px(2.0 + 2.5 * pulse));
    let (r, g, b) = accent();
    let glow = Color::srgba(
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        0.5 + 0.45 * pulse,
    );
    *border = BorderColor::all(glow);

    // ── The pointer arrow ────────────────────────────────────────────────────
    let Ok((mut a_node, mut arrow)) = arrows.single_mut() else {
        return;
    };
    let win_h = windows.single().map(|w| w.height()).unwrap_or(f32::MAX);

    // Prefer sitting below and pointing up; flip above when the window's bottom
    // edge would clip it (the status-bar theme menu is the case that needs this).
    let box_bottom = top_left.y + size.y + PAD;
    let point_down = box_bottom + ARROW_GAP + ARROW_INK_H + ARROW_BOB > win_h;

    // Bob *toward* the target: up when pointing up, down when pointing down.
    // Positioned by where the APEX should land, then backed off by the box's
    // transparent lead-in — placing the box directly would leave the arrow
    // floating `LEAD_IN` px short of its target.
    let bob = (t * 3.5).sin().abs() * ARROW_BOB;
    a_node.display = Display::Flex;
    a_node.left = Val::Px(top_left.x + size.x * 0.5 - HEAD_SIZE * 0.5);
    a_node.top = Val::Px(if point_down {
        // Apex points down at the target's top edge; the box extends upward.
        (top_left.y - PAD - ARROW_GAP + bob) + LEAD_IN - ARROW_H
    } else {
        (box_bottom + ARROW_GAP - bob) - LEAD_IN
    });

    // Flipping swaps head and tail (column reversal), spins the head, and moves
    // the tail's negative margin to the other end. Written only on an actual
    // flip — `sync_triangle_materials` re-uploads the material on
    // `Changed<TriangleShape>`, so the colour write below is the only per-frame
    // churn we accept.
    if arrow.pointing_down != point_down {
        arrow.pointing_down = point_down;
        a_node.flex_direction = if point_down {
            FlexDirection::ColumnReverse
        } else {
            FlexDirection::Column
        };
        for mut head in &mut heads {
            head.rotation = if point_down { HEAD_DOWN } else { HEAD_UP };
        }
        for mut stem in &mut stem_nodes {
            stem.margin = if point_down {
                UiRect::bottom(Val::Px(-OVERLAP))
            } else {
                UiRect::top(Val::Px(-OVERLAP))
            };
        }
    }

    // Breathe on the box's rhythm, in the arrow's own green so the pair read as
    // one highlight without the arrow vanishing into the accent border.
    let a_glow = green(0.6 + 0.4 * pulse);
    for mut head in &mut heads {
        if head.color != a_glow {
            head.color = a_glow;
        }
    }
    for mut stem in &mut stems {
        if stem.0 != a_glow {
            stem.0 = a_glow;
        }
    }
}

/// The named UI node's logical-px rect (top-left + size), skipping zero-sized
/// (collapsed / hidden) matches. `UiGlobalTransform.translation` and
/// `ComputedNode.size()` are both physical px, so both are scaled by the node's
/// inverse scale factor to the logical px `Val::Px` wants.
///
/// Several nodes can legitimately share a name — there is one `dock-add-panel`
/// **per dock leaf**, so a four-panel layout has four. Taking the first match
/// pointed the user at whichever leaf happened to be built first, usually a
/// narrow side panel. Instead the match whose *parent* is largest wins: the
/// parent of a tab-bar button is the tab bar, whose width is its leaf's width,
/// so this resolves to the biggest dock area on screen — the one with room to
/// show the picker and the panel that lands in it.
fn target_rect(
    targets: &Query<(
        Entity,
        &Name,
        &ComputedNode,
        &UiGlobalTransform,
        Option<&CalculatedClip>,
    )>,
    nodes: &Query<&ComputedNode>,
    parents: &Query<&ChildOf>,
    target_name: &str,
) -> Option<(Vec2, Vec2)> {
    let mut best: Option<(f32, Vec2, Vec2)> = None;
    for (entity, name, cn, ugt, clip) in targets {
        if name.as_str() != target_name {
            continue;
        }
        let isf = cn.inverse_scale_factor();
        let size = cn.size() * isf;
        if size.x < 1.0 || size.y < 1.0 {
            continue;
        }
        // Skip anything scrolled out of its container. A clipped node keeps a
        // perfectly good `UiGlobalTransform` — bevy_ui clips it at draw time
        // rather than moving or culling it — so without this the highlight
        // happily framed a "Demo Panel" row that had scrolled off the Add Panel
        // list, drawing a box and an arrow in empty space below the overlay.
        // `CalculatedClip` is physical, window-global px, the same basis as
        // `UiGlobalTransform.translation`.
        if let Some(clip) = clip {
            let c = clip.clip;
            let p = ugt.translation;
            if p.x < c.min.x || p.x > c.max.x || p.y < c.min.y || p.y > c.max.y {
                continue;
            }
        }
        let parent_area = parents
            .get(entity)
            .ok()
            .and_then(|p| nodes.get(p.parent()).ok())
            .map(|p| p.size().x * p.size().y)
            .unwrap_or(0.0);
        let top_left = (ugt.translation - cn.size() * 0.5) * isf;
        if best.as_ref().is_none_or(|(a, _, _)| parent_area > *a) {
            best = Some((parent_area, top_left, size));
        }
    }
    best.map(|(_, tl, size)| (tl, size))
}
