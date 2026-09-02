//! The global bottom panel: its chrome state, the slide animation, the top-edge
//! resize, the Overlay/Layout mode button, and the collapsed strip that stands
//! in for it while it is shut.
//!
//! The panel's *contents* are one dock tree in [`renzora_ember::dock::FixedDock`],
//! held outside every workspace layout. That is what makes closing it cheap:
//! nothing moves, `open` simply goes false. The named tab-sets that switch which
//! tree is live are in [`crate::panel_sets`].

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};

use renzora_ember::dock::DockTab;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::theme::{placeholder, rgb, tab_active, text_muted};

use renzora::core::keybindings::{EditorAction, KeyBindings};

use crate::dock;
use crate::panel_sets::{default_panel_set_name, BottomPanelSets};

/// The global bottom panel's chrome state: how tall it is when open, and
/// whether it is open at all. Its *contents* live in
/// [`renzora_ember::dock::FixedDock`] — this is only what the shell needs to
/// size and show the area node.
///
/// This replaced a `BTreeMap<workspace_name, ClosedBottom>`. The old model had
/// to stash the bottom region's whole subtree out of the workspace tree when
/// closed, because that was the only place those panels existed; closing was
/// therefore a destructive tree edit that had to round-trip exactly. Now the
/// tree is held out of the workspace layouts permanently, so closing is just
/// `open = false` — nothing moves, and nothing can be lost by failing to
/// restore it.
#[derive(Resource)]
pub(crate) struct BottomDock {
    /// Logical px, applied to the area node when open.
    pub(crate) height: f32,
    pub(crate) open: bool,
    /// Whether the panel floats over the workspace or takes height from it.
    pub(crate) mode: dock::BottomDockMode,
    /// How far the slide-open animation has got: 0 = fully closed, 1 = fully
    /// open at `height`. Chased toward `open` by [`animate_bottom_dock`], and
    /// the only thing [`sync_bottom_dock_node`] scales the node by — `open` is
    /// still the state everything else reads, so nothing else has to know the
    /// panel moves rather than appearing.
    ///
    /// Deliberately not persisted: a session starts at whichever end of the
    /// travel `open` says, not mid-slide.
    pub(crate) slide: f32,
}

/// Ctrl+Space ([`EditorAction::ToggleBottomPanel`]): show or hide the global
/// bottom panel.
///
/// This used to detach the bottom region out of the active workspace's tree
/// into a per-workspace stash, and re-attach it on reopen — because those
/// panels existed *only* inside that tree, closing was a destructive edit that
/// had to round-trip tab order, active tab, split ratio and an anchor path
/// exactly, drop panels that had reappeared elsewhere meanwhile, and re-key
/// itself whenever a workspace was renamed or removed.
///
/// The tree lives in [`renzora_ember::dock::FixedDock`] now, outside every
/// workspace layout, so none of that applies: hiding the panel hides a node and
/// nothing moves. The three helpers that did the round-trip
/// (`close_bottom_panel`, `reopen_bottom_panel`, `bottom_snap_collapse`) are
/// gone with it.
///
/// Opening always goes to [`default_open_height`] rather than to the height the
/// panel last had, for the same reason clicking a collapsed tab does: the
/// remembered height can be anything, including the near-minimum a drag-to-close
/// leaves behind, and a shortcut that opens the panel to a sliver reads as
/// broken. The chevron is the control that reopens at the remembered height.
pub(crate) fn toggle_bottom_panel(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Option<Res<KeyBindings>>,
    input_focus: Option<Res<renzora::core::InputFocusState>>,
    wraps: Query<&ComputedNode, With<DockAreaWrap>>,
    mut bottom: ResMut<BottomDock>,
) {
    let Some(kb) = keybindings else { return };
    if kb.rebinding.is_some() || input_focus.is_some_and(|f| f.ui_wants_keyboard) {
        return;
    }
    if !kb.just_pressed(EditorAction::ToggleBottomPanel, &keyboard) {
        return;
    }
    bottom.open = !bottom.open;
    if bottom.open {
        if let Some(h) = default_open_height(&wraps) {
            bottom.height = h;
        }
    }
}

/// Share of the dock region the bottom panel takes when it is *shown* rather
/// than restored. Enough that the panel opens onto real content — a readable
/// run of console lines, a couple of rows of asset thumbnails — without the
/// workspace above it stopping being the thing you are looking at.
const BOTTOM_DOCK_OPEN_FRACTION: f32 = 0.40;

/// [`BOTTOM_DOCK_OPEN_FRACTION`] of the dock region's height, in logical px,
/// floored at the panel's minimum — the height the bottom panel opens to when
/// something asks for it to be shown rather than restored.
///
/// `None` before the wrapper node has been laid out, which the callers read as
/// "leave the height alone" rather than falling back to a guess.
fn default_open_height(wraps: &Query<&ComputedNode, With<DockAreaWrap>>) -> Option<f32> {
    let avail = dock_region_height(wraps)?;
    Some((avail * BOTTOM_DOCK_OPEN_FRACTION).max(dock::BOTTOM_DOCK_MIN_HEIGHT))
}

/// The dock region's height in logical px — the full span from the top bar down
/// to the status bar, and so the tallest the bottom panel may be dragged.
///
/// `None` before the wrapper node has been laid out — the node exists for a few
/// frames at zero height, which is not a measurement, so a zero reads as "not
/// yet" rather than as a dock region with no room in it. Callers that only need
/// a clamp read that as "no limit yet" (`f32::INFINITY`); callers that would
/// have to *guess* a height read it as "leave it alone".
fn dock_region_height(wraps: &Query<&ComputedNode, With<DockAreaWrap>>) -> Option<f32> {
    let wrap = wraps.iter().next()?;
    let height = wrap.size().y * wrap.inverse_scale_factor();
    (height > 0.0).then_some(height)
}

/// Cap the restored bottom-panel height at [`BOTTOM_DOCK_OPEN_FRACTION`] of the
/// dock region, once, on the first frame the region has a size.
///
/// The panel can be dragged up to the top bar and that height is remembered, so
/// without this an editor that was closed with the panel pulled right up starts
/// the next session with its workspace hidden behind a full-height Assets
/// browser — the state is recoverable, but it is a poor thing to open onto, and
/// it is not what the person who dragged it there was choosing. Capping only at
/// load keeps the drag itself unrestricted: 40% is where a session *starts*, not
/// a ceiling on where it can go.
///
/// A shorter remembered height is left exactly as it was — this is a cap, not a
/// reset.
pub(crate) fn clamp_bottom_dock_on_load(
    wraps: Query<&ComputedNode, With<DockAreaWrap>>,
    mut bottom: ResMut<BottomDock>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let Some(cap) = default_open_height(&wraps) else {
        return;
    };
    *done = true;
    if bottom.height > cap {
        bottom.height = cap;
    }
}

/// Seconds the bottom panel takes to travel its full height. Short enough that
/// `Ctrl+Space` still answers instantly, long enough that the eye follows the
/// panel to where it went instead of it simply being somewhere else — which is
/// the whole point of animating a panel that covers 40% of the editor.
const BOTTOM_DOCK_SLIDE_SECS: f32 = 0.16;

/// Chase [`BottomDock::slide`] toward whatever `open` currently says.
///
/// Every path that opens or closes the panel — the shortcut, both chevrons, a
/// tab click on the collapsed strip, the snap-shut drag, the drag-away hide —
/// writes only `open`, so all of them animate without any of them knowing that
/// they do. [`sync_bottom_dock_node`] is the one place that reads `slide`.
///
/// The resource is written *only* while the value is genuinely moving:
/// [`sync_bottom_dock_mode_btn`] early-outs on `bottom.is_changed()`, and
/// touching the `ResMut` every frame would quietly turn that into no early-out
/// at all.
/// On the **real** clock, not the virtual one: this is editor chrome, and it
/// has to keep moving while play mode is paused or time-scaled.
pub(crate) fn animate_bottom_dock(
    time: Res<Time<bevy::time::Real>>,
    mut bottom: ResMut<BottomDock>,
) {
    let target = if bottom.open { 1.0 } else { 0.0 };
    if bottom.slide == target {
        return;
    }
    // Guard against a zero/absurd delta (a stalled frame, a debugger pause)
    // stretching the slide across seconds of wall clock.
    let step = (time.delta_secs() / BOTTOM_DOCK_SLIDE_SECS).clamp(0.0, 1.0);
    bottom.slide = if bottom.slide < target {
        (bottom.slide + step).min(target)
    } else {
        (bottom.slide - step).max(target)
    };
}

/// Smoothstep the linear slide parameter, so the panel eases out of rest at
/// both ends rather than starting and stopping at full speed.
fn slide_ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The bottom panel's open state as it was when the current drag began, and
/// therefore what it gets back when the drag ends. `None` when no drag is being
/// tracked — including a drag that started with the panel already closed, which
/// this never touches.
#[derive(Resource, Default)]
pub(crate) struct BottomDockDragHide {
    restore: Option<bool>,
}

/// How far above the dock region's bottom edge counts as "near the bottom", and
/// so brings an auto-hidden panel back.
///
/// Deliberately a narrow strip rather than the panel's own footprint. The space
/// a closed panel *would* occupy is a full-width band across the editor, and
/// while it is closed that band is somebody else's — a hierarchy row, an
/// inspector slot, the lower half of the viewport. Reopening the moment a drag
/// crossed into it would put the panel on top of the drop target the user was
/// heading for. Coming back has to be something you ask for by aiming at the
/// bottom of the window, not something that happens on the way past.
const BOTTOM_DOCK_REVEAL_BAND: f32 = 48.0;

/// Drag an asset out of the bottom panel and the panel gets out of your way;
/// bring the drag back to it — or anywhere near the bottom of the editor — and
/// it comes back.
///
/// Almost everything worth dropping an asset *on* is underneath this panel: the
/// viewport, the hierarchy, an inspector slot. Dragging out of the Assets tab
/// therefore starts by covering the target with the panel you dragged from, and
/// the old answer was to close the panel by hand first and lose sight of what
/// you were dragging.
///
/// It writes `open` and nothing else, so the panel *slides* out of the way
/// rather than blinking, and every other system continues to see one ordinary
/// open/closed panel. The state the drag found is restored when the drag ends,
/// wherever it was dropped — an auto-hide that outlived its gesture would just
/// be the panel closing itself for no reason the user can see.
///
/// Shape-library drags are included because that panel is a bottom-panel tab
/// too, and the gesture — drag out of the bottom panel, aim at the viewport — is
/// the identical one.
pub(crate) fn bottom_dock_drag_reveal(
    asset_drag: Option<Res<renzora_ui::AssetDragPayload>>,
    shape_drag: Option<Res<renzora_ui::ShapeDragState>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    wraps: Query<(&ComputedNode, &UiGlobalTransform), With<DockAreaWrap>>,
    mut bottom: ResMut<BottomDock>,
    mut hide: ResMut<BottomDockDragHide>,
) {
    // `is_detached` gates on the pointer having actually moved, so a plain
    // click on an asset never flickers the panel.
    let dragging = asset_drag.is_some_and(|d| d.is_detached)
        || shape_drag.is_some_and(|s| s.dragging_shape.is_some());
    if !dragging {
        if let Some(open) = hide.restore.take() {
            bottom.open = open;
        }
        return;
    }
    let Some(cursor) = windows.single().ok().and_then(|w| w.cursor_position()) else {
        return;
    };
    let Some((node, transform)) = wraps.iter().next() else {
        return;
    };
    let inv = node.inverse_scale_factor();
    let size = node.size() * inv;
    // The node exists at zero height for a few frames after it spawns, which is
    // not a measurement — see `dock_region_height`.
    if size.y <= 0.0 {
        return;
    }
    let region_bottom = transform.translation.y * inv + size.y * 0.5;

    if hide.restore.is_none() {
        // A drag that began with the panel already closed is not ours: the
        // asset came from somewhere else, and opening the panel under the
        // cursor would be a surprise rather than a convenience.
        if !bottom.open {
            return;
        }
        hide.restore = Some(true);
    }

    // The two thresholds are deliberately different, and far apart.
    //
    // Leaving is judged against the panel's *own top edge*: everything below it
    // is the panel, so a drag toward a folder tile or another tab inside it
    // never triggers a hide. Coming back is judged against a narrow strip at
    // the very bottom of the region — see [`BOTTOM_DOCK_REVEAL_BAND`] for why
    // it can't be the footprint again.
    //
    // The gap between them is also what makes this stable. A single threshold
    // put the open and closed states on either side of one line: the panel hid,
    // which moved nothing, so the cursor was still on the line, so a pixel of
    // jitter reopened it — and it flickered for as long as the drag hovered
    // there. With hysteresis there is a wide dead band in the middle where
    // neither test fires and the panel simply stays as it is.
    let open = if bottom.open {
        cursor.y >= region_bottom - dock::clamp_height(bottom.height, size.y)
    } else {
        cursor.y >= region_bottom - BOTTOM_DOCK_REVEAL_BAND
    };
    if bottom.open != open {
        bottom.open = open;
    }
}

/// A live drag of the bottom panel's top edge: `(cursor y at press, panel
/// height at press)`. `None` when no drag is in flight.
///
/// Held in a resource rather than a `Local` because the collapsed strip's
/// drag-to-open gesture arms it from a different system — opening the panel
/// and resizing it are one continuous gesture for the user.
#[derive(Resource, Default)]
pub(crate) struct BottomDockResize {
    active: Option<(f32, f32)>,
}

/// The bottom panel's top-edge resize grip.
#[derive(Component)]
pub(crate) struct BottomDockGrip;

/// The open bottom panel's collapse button — the counterpart of the collapsed
/// strip's open chevron, so the panel can be dismissed without knowing the
/// Ctrl+Space binding.
#[derive(Component)]
pub(crate) struct BottomDockCloseBtn;

/// Shared marker for the open panel's corner buttons (mode, then collapse).
/// They sit in one row and share a placement and visibility rule, so
/// [`sync_bottom_dock_node`] drives them through a single query — which also
/// keeps its `&mut Node` queries disjoint without a third `Without` filter.
#[derive(Component)]
pub(crate) struct BottomDockBtn;

/// Click the open panel's collapse button → close it.
pub(crate) fn bottom_dock_close_click(
    btns: Query<&Interaction, (With<BottomDockCloseBtn>, Changed<Interaction>)>,
    mut bottom: ResMut<BottomDock>,
) {
    if btns.iter().any(|i| matches!(i, Interaction::Pressed)) {
        bottom.open = false;
    }
}

/// The open panel's mode button, immediately left of the collapse button:
/// switch between overlaying the workspace and docking into it.
#[derive(Component)]
pub(crate) struct BottomDockModeBtn;

/// Click the mode button → flip [`BottomDock::mode`], shrinking the panel if
/// that is what it takes for the flip to be visible.
///
/// The button reports the *effective* mode, so at a height only an overlay can
/// have it reads `Overlay` even when `mode` already says `Layout`. Flipping the
/// stored value there would leave the panel looking identical and the button
/// still saying `Overlay` — the control would read as dead. Both branches
/// therefore pull the height down to what layout mode can hold, which is the
/// part of "dock into the workspace" the user is actually asking for.
pub(crate) fn bottom_dock_mode_click(
    btns: Query<&Interaction, (With<BottomDockModeBtn>, Changed<Interaction>)>,
    wraps: Query<&ComputedNode, With<DockAreaWrap>>,
    mut bottom: ResMut<BottomDock>,
) {
    if !btns.iter().any(|i| matches!(i, Interaction::Pressed)) {
        return;
    }
    let avail = dock_region_height(&wraps).unwrap_or(f32::INFINITY);
    let max_docked = dock::max_layout_height(avail);
    match bottom.mode.effective(bottom.height, avail) {
        dock::BottomDockMode::Overlay => {
            bottom.mode = dock::BottomDockMode::Layout;
            if bottom.height > max_docked {
                bottom.height = max_docked;
            }
        }
        dock::BottomDockMode::Layout => bottom.mode = dock::BottomDockMode::Overlay,
    }
}

/// Keep the mode button's glyph and tooltip on the *current* mode — the icon
/// reports what the panel is doing now, not what clicking would do, matching
/// every other stateful toggle in the chrome.
///
/// "Now" means the effective mode: a layout-mode panel dragged too tall to dock
/// is overlaying the workspace, and the button has to say so or the panel's
/// behaviour and its own label disagree. That case gets its own tooltip,
/// because "you are in Overlay" is not the useful thing to say to someone who
/// chose Layout — "drag me back down" is.
pub(crate) fn sync_bottom_dock_mode_btn(
    bottom: Res<BottomDock>,
    wraps: Query<&ComputedNode, With<DockAreaWrap>>,
    mut btns: Query<(&Children, &mut renzora_ember::widgets::HoverTooltip), With<BottomDockModeBtn>>,
    // The button is respawned whenever the chrome is (theme or language
    // switch), always carrying the `Overlay` glyph it was authored with — so a
    // fresh button has to be re-synced even though the mode itself never moved.
    spawned: Query<(), Added<BottomDockModeBtn>>,
    mut text: Query<&mut Text>,
    // Resizing the *window* can flip the effective mode without `BottomDock`
    // moving at all — the panel stands still while the room for a workspace
    // above it runs out. Cheap to measure, so it joins the early-out rather
    // than defeating it.
    mut last_avail: Local<f32>,
) {
    let avail = dock_region_height(&wraps).unwrap_or(f32::INFINITY);
    if !bottom.is_changed() && spawned.is_empty() && *last_avail == avail {
        return;
    }
    *last_avail = avail;
    let effective = bottom.mode.effective(bottom.height, avail);
    let (icon, tip) = match (effective, bottom.mode) {
        (dock::BottomDockMode::Overlay, dock::BottomDockMode::Layout) => (
            "stack",
            renzora::lang::t_or(
                "shell.bottom_dock.mode_forced_overlay",
                "Overlay — too tall to dock; drag it down to return to Layout",
            ),
        ),
        (dock::BottomDockMode::Overlay, _) => (
            "stack",
            renzora::lang::t_or(
                "shell.bottom_dock.mode_overlay",
                "Overlay — floats over the workspace",
            ),
        ),
        (dock::BottomDockMode::Layout, _) => (
            "rows",
            renzora::lang::t_or(
                "shell.bottom_dock.mode_layout",
                "Layout — docked below the workspace",
            ),
        ),
    };
    let Some(glyph) = renzora_ember::phosphor_map::icon_glyph(icon).map(|c| c.to_string()) else {
        return;
    };
    for (children, mut tooltip) in &mut btns {
        if tooltip.0 != tip {
            tooltip.0 = tip.clone();
        }
        for child in children.iter() {
            if let Ok(mut t) = text.get_mut(child) {
                if t.0 != glyph {
                    t.0 = glyph.clone();
                }
            }
        }
    }
}

/// The relatively-positioned wrapper holding the workspace dock area and the
/// bottom panel overlaid on it. Its computed height is the space a bottom-panel
/// resize is allowed to eat into.
#[derive(Component)]
pub(crate) struct DockAreaWrap;

/// Thickness of the bottom panel's top-edge resize band, logical px. Straddles
/// the border so the cursor changes slightly before and after the visible edge —
/// a 1px border is not a target anyone can hit.
pub(crate) const BOTTOM_DOCK_GRIP_H: f32 = 10.0;

/// Stacking tier for the global bottom panel.
///
/// It has to be a `GlobalZIndex` and not merely a later sibling, because
/// `GlobalZIndex` is *global*: any node carrying one is lifted out of its
/// parent's stacking context into the root order. The node-graph widget uses it
/// throughout (canvas, edges, nodes — up to 10), so the Blueprint and Material
/// graph panels were being hoisted to the root order and painting straight over
/// the bottom panel, which had no tier at all and sat in normal flow. Sibling
/// order cannot win against that; only a higher tier can.
///
/// 100 puts it above panel *content* while staying below every floating
/// surface, which must still open over it: the dock's root drop overlay (200),
/// modals and dropdowns (500), menus (700), the tab-drag ghost (1000) and
/// asset-slot drags (2000).
///
/// Winning that way cut the other way once a graph panel was docked *into* this
/// one: its parts, still at 0–10, went under this background and the canvas came
/// up blank. The graph's depths are now relative tiers rebased against whatever
/// it's mounted in (`NgTier` / `ng_rebase_z` in ember), so a graph inside this
/// panel lands at 100–110 and one outside it stays at 0–10. This tier is still
/// what keeps the outside case from painting over us.
pub(crate) const BOTTOM_DOCK_Z: i32 = 100;

/// Push [`BottomDock`] onto the panel node, its resize band and its corner
/// buttons: height, vertical placement, and whether each is displayed.
///
/// Also applies the mode. Both modes leave the panel occupying the bottom
/// `height` px of [`DockAreaWrap`], which is why the absolutely-placed grip and
/// buttons need no mode-specific arithmetic — only the panel node itself
/// changes, between an absolute overlay and an in-flow row of the dock column.
///
/// Height is clamped only to the dock region itself: the panel can be dragged
/// the whole way up to the top bar. It used to stop a fixed strip short of it,
/// on the grounds that a full-height overlay hides the very panels you would
/// click to recover — but the panel's own mode and collapse buttons ride at its
/// top edge, so they stay on screen at any height (and `Ctrl+Space` closes it
/// from anywhere). What the old clamp really protected was *layout* mode, where
/// the same drag squeezes every panel above to nothing; that case is now
/// handled by [`dock::BottomDockMode::effective`] switching the panel to an
/// overlay instead of by refusing the drag.
// The `Without` filters that keep the three `&mut Node` queries disjoint, and
// the `Or` that gathers every hideable interactive node, are both unavoidably
// wordy — a system's parameters are not an argument list a caller threads.
#[allow(clippy::type_complexity)]
pub(crate) fn sync_bottom_dock_node(
    bottom: Res<BottomDock>,
    wraps: Query<&ComputedNode, With<DockAreaWrap>>,
    mut areas: Query<
        &mut Node,
        (
            With<renzora_ember::dock::FixedDockArea>,
            Without<BottomDockGrip>,
            Without<BottomDockBtn>,
        ),
    >,
    mut grips: Query<&mut Node, (With<BottomDockGrip>, Without<BottomDockBtn>)>,
    mut btns: Query<&mut Node, With<BottomDockBtn>>,
    // Every interactive node that this system can hide. Hiding one while the
    // cursor is on it strands its `Interaction` at `Hovered`, because Bevy's
    // focus pass skips hidden entities and never writes the reset — and
    // `apply_cursor_icon` picks the *first* hovered node carrying a
    // `HoverCursor`, so one stranded entry owns the cursor for the whole app.
    // Closing the panel by clicking its own toggle hits this every time.
    //
    // Filtering on a zero `ComputedNode` size did not fix it, so the computed
    // size evidently goes stale the same way. Clearing the state explicitly is
    // the only version that doesn't depend on what Bevy updates for a hidden
    // node.
    mut hidden_interactions: Query<
        &mut Interaction,
        Or<(
            With<BottomDockGrip>,
            With<BottomDockBtn>,
            With<renzora_ember::dock::FixedAreaHeader>,
        )>,
    >,
) {
    // Shown whenever it's open, empty or not. Hiding an empty one used to be
    // the tidier choice — no bare bordered slab — but closing the panel's last
    // tab then took the whole panel away *with its corner controls*, and
    // Ctrl+Space couldn't bring it back: the toggle set `open`, this line
    // immediately re-hid it, and there was no way left to add a panel. An empty
    // one is not blank anyway; ember renders its "Add Panel" button, and the
    // panel-set dropdown sits in the corner beside it.
    //
    // Shown for the whole of the slide, not only while `open` — a closing panel
    // has to stay on screen to be seen leaving.
    let show = bottom.open || bottom.slide > 0.0;
    let avail = dock_region_height(&wraps).unwrap_or(f32::INFINITY);
    // `target` is the height the panel has when it is fully open, and the
    // height every decision below is made against; `eased` is how far along the
    // travel it currently is.
    let target = dock::clamp_height(bottom.height, avail);
    let eased = slide_ease(bottom.slide);
    let want = if show { Display::Flex } else { Display::None };
    // The grip and the corner buttons ride the panel's top edge, so mid-slide
    // they would be somewhere the panel isn't yet — and at the bottom of the
    // travel their inset arithmetic goes negative and puts them under it. They
    // appear once the panel has arrived.
    let show_controls = bottom.open && bottom.slide >= 1.0;
    let want_controls = if show_controls {
        Display::Flex
    } else {
        Display::None
    };

    // Overlay: absolute, pinned to the wrapper's bottom edge, painted over the
    // dock area. Layout: an in-flow row of the dock column, so the dock area's
    // `flex_grow` hands it the remaining height and every panel above reflows.
    // The insets are cleared in layout mode because a relatively-positioned
    // node treats them as an offset rather than an anchor.
    // The *effective* mode, not the stored one: a layout-mode panel dragged
    // past what the workspace can give up renders as an overlay for as long as
    // it stays that tall.
    // Measured against `target`: judging the effective mode by the animated
    // height would start every open in layout mode and flip to overlay partway
    // up, which reparents the panel and makes the whole workspace jump mid-slide.
    let layout_mode = bottom.mode.effective(target, avail) == dock::BottomDockMode::Layout;
    // The two modes have to animate differently, because the thing that moves
    // is different.
    //
    // **Overlay** slides: the panel keeps its full height throughout and
    // travels down past the wrapper's bottom edge, where `DockAreaWrap`'s
    // `Overflow::clip()` takes it. Its contents are laid out once, at the size
    // they will end at, so the tab bar and the panel body ride down intact.
    //
    // **Layout** can't do that — its height *is* the height the workspace above
    // gives up, and a panel translated out of view would leave the gap it was
    // occupying. So it opens as an accordion: the height itself grows, and
    // every panel above reflows into what's left, which is the same thing
    // dragging its top edge already does.
    let (height, bottom_inset) = if layout_mode {
        (target * eased, Val::Auto)
    } else {
        (target, Val::Px(-target * (1.0 - eased)))
    };
    // Cleared in layout mode because a relatively-positioned node treats an
    // inset as an offset rather than as an anchor.
    let (position_type, left_inset) = if layout_mode {
        (PositionType::Relative, Val::Auto)
    } else {
        (PositionType::Absolute, Val::Px(0.0))
    };
    if let Ok(mut node) = areas.single_mut() {
        // Reads go through `Deref` (no change flag); only assign on a real
        // change, since any `Node` write triggers a relayout.
        if node.display != want {
            node.display = want;
        }
        if node.height != Val::Px(height) {
            node.height = Val::Px(height);
        }
        if node.position_type != position_type {
            node.position_type = position_type;
        }
        if node.left != left_inset {
            node.left = left_inset;
        }
        if node.bottom != bottom_inset {
            node.bottom = bottom_inset;
        }
    }
    if let Ok(mut node) = grips.single_mut() {
        if node.display != want_controls {
            node.display = want_controls;
        }
        // Centre the band on the panel's top edge so the drag works from just
        // above it as well as just below. Placed against `target`, not the
        // animated height: it is hidden until the panel arrives there, and
        // writing a moving inset would relayout it for nothing.
        let offset = Val::Px(target - BOTTOM_DOCK_GRIP_H * 0.5);
        if node.bottom != offset {
            node.bottom = offset;
        }
    }
    // The corner buttons are only shown while the panel is open — the collapsed
    // strip carries its own chevron for the closed state, in the same corner, so
    // the toggle appears continuous as the panel opens and closes.
    for mut node in &mut btns {
        if node.display != want_controls {
            node.display = want_controls;
        }
        // Sit inside the panel, clear of the resize band above it, so a press
        // near the corner can't be ambiguous between closing and resizing.
        // Against `target` for the same reason as the grip above.
        let offset = Val::Px(target - 26.0);
        if node.bottom != offset {
            node.bottom = offset;
        }
    }
    // Nothing hidden may stay `Hovered` (see the query's comment). Keyed on the
    // controls rather than on the panel: they are the nodes this system hides,
    // and mid-slide they are hidden while the panel itself is still up.
    if !show_controls {
        for mut interaction in &mut hidden_interactions {
            if *interaction != Interaction::None {
                *interaction = Interaction::None;
            }
        }
    }
}

/// Reset the hover/press state of everything *inside* the bottom panel on the
/// frame the panel is hidden.
///
/// Same hazard [`sync_bottom_dock_node`] handles for its own corner controls,
/// but for the panel's contents, and with teeth: Bevy's focus pass skips hidden
/// entities, so an asset tile or a folder row that was under the cursor when
/// the panel went away keeps reading `Hovered` forever. The asset browser's
/// drop handler treats a hovered *folder* as "move the dragged files in here",
/// so a stranded one turns a drop into the viewport into a file move — and
/// [`bottom_dock_drag_reveal`] hides the panel mid-drag as a matter of course,
/// which is exactly the moment a folder row is likely to be the last thing the
/// cursor touched.
///
/// Only on the transition, so closing the panel doesn't cost a subtree walk
/// every frame it stays closed.
pub(crate) fn clear_bottom_dock_hover_on_hide(
    bottom: Res<BottomDock>,
    areas: Query<Entity, With<renzora_ember::dock::FixedDockArea>>,
    children: Query<&Children>,
    mut interactions: Query<&mut Interaction>,
    mut was_shown: Local<bool>,
) {
    let shown = bottom.open || bottom.slide > 0.0;
    if shown == *was_shown {
        return;
    }
    *was_shown = shown;
    if shown {
        return;
    }
    let Ok(area) = areas.single() else { return };
    let mut stack = vec![area];
    while let Some(entity) = stack.pop() {
        if let Ok(mut interaction) = interactions.get_mut(entity) {
            if *interaction != Interaction::None {
                *interaction = Interaction::None;
            }
        }
        if let Ok(kids) = children.get(entity) {
            stack.extend(kids.iter());
        }
    }
}

/// Press the grip → start a resize, recording where the cursor was and how
/// tall the panel was at that moment.
///
/// Reads the *current* `Interaction` on the `just_pressed` frame rather than
/// filtering on `Changed<Interaction>`. That mirrors ember's `divider_drag`,
/// which drives the identical gesture: a `Changed` filter only sees the frame
/// the transition is written, so any frame where the press and the focus update
/// don't line up drops the gesture entirely and the handle reads as dead.
pub(crate) fn bottom_dock_grip_press(
    mouse: Res<ButtonInput<MouseButton>>,
    grips: Query<&Interaction, With<BottomDockGrip>>,
    headers: Query<&Interaction, With<renzora_ember::dock::FixedAreaHeader>>,
    tabs: Query<&Interaction, With<DockTab>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    bottom: Res<BottomDock>,
    mut resize: ResMut<BottomDockResize>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let on_grip = grips.iter().any(|i| matches!(i, Interaction::Pressed));
    // The panel also resizes by dragging its header's empty space, which is a
    // bigger and more obvious target than a 10px edge band. The marker sits on
    // the tab bar's filler, so it spans only the gap after the tabs.
    //
    // The tab check is belt and braces: `FocusPolicy` defaults to `Pass` in
    // Bevy 0.19, so a press can be seen by more than one node, and a resize
    // starting because someone clicked a tab would be worse than a resize that
    // occasionally needs a second try.
    let on_header = headers.iter().any(|i| matches!(i, Interaction::Pressed))
        && !tabs
            .iter()
            .any(|i| matches!(i, Interaction::Pressed | Interaction::Hovered));
    if !on_grip && !on_header {
        return;
    }
    let Some(cursor_y) = windows
        .single()
        .ok()
        .and_then(|w| w.cursor_position())
        .map(|c| c.y)
    else {
        return;
    };
    resize.active = Some((cursor_y, bottom.height));
}

/// Drive a live bottom-panel resize, and snap the panel closed when dragged
/// hard down past its minimum — the counterpart of the collapsed strip's
/// drag-up-to-open, so the panel can be dismissed with the same gesture that
/// opened it.
pub(crate) fn bottom_dock_resize_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    wraps: Query<&ComputedNode, With<DockAreaWrap>>,
    mut bottom: ResMut<BottomDock>,
    mut resize: ResMut<BottomDockResize>,
) {
    if !mouse.pressed(MouseButton::Left) {
        resize.active = None;
        return;
    }
    let Some((start_y, start_h)) = resize.active else {
        return;
    };
    let Some(cursor_y) = windows
        .single()
        .ok()
        .and_then(|w| w.cursor_position())
        .map(|c| c.y)
    else {
        return;
    };
    // Cursor y grows downward, so dragging up (smaller y) grows the panel.
    let height = start_h + (start_y - cursor_y);
    if height < dock::BOTTOM_DOCK_MIN_HEIGHT * 0.5 {
        bottom.open = false;
        // Snap, don't slide. The panel has been following the cursor down for
        // the whole gesture and is already at a sliver; animating the last few
        // px would play a transition *behind* a cursor that has finished
        // moving, and read as lag rather than as the panel leaving.
        bottom.slide = 0.0;
        resize.active = None;
        return;
    }
    // Clamped to the dock region here as well as in `sync_bottom_dock_node`, so
    // the height that gets *persisted* is one the panel can actually have —
    // otherwise dragging past the top bar banks metres of overshoot that the
    // next drag downward has to unwind before the panel so much as moves.
    let avail = dock_region_height(&wraps).unwrap_or(f32::INFINITY);
    bottom.height = dock::clamp_height(height, avail);
}

/// The collapsed bottom-panel strip: a tab-bar-height row between the dock
/// area and the status bar — exactly where the closed bottom panel's header
/// would sit — showing the stashed region's tabs in a muted, closed state.
/// Hidden while the bottom panel is open (or the workspace never had one).
#[derive(Component)]
pub(crate) struct CollapsedBottomBar;

/// One tab in the collapsed strip; clicking reopens the bottom panel with
/// this panel as the active tab.
#[derive(Component)]
pub(crate) struct CollapsedBottomTab(String);

/// The open chevron at the right end of the collapsed strip; clicking
/// reopens the bottom panel (counterpart of the open panel's collapse
/// chevron).
#[derive(Component)]
pub(crate) struct CollapsedBottomOpenBtn;

/// Keep the collapsed strip in sync with the global bottom panel: shown with
/// one tab per panel in the [`renzora_ember::dock::FixedDock`] tree while the
/// panel is closed, hidden while it's open. Tab children rebuild only when the
/// tab set (or the bar entity, after a chrome respawn) changes.
///
/// It reads the same tree the open panel renders, rather than a stash of what
/// was detached — so the strip lists the panel's real contents in every
/// workspace, and a panel added to the bottom dock while it happens to be
/// closed shows up here immediately.
///
/// **An empty panel still gets a strip**, showing the active set's name in
/// place of the tabs it has none of. Hiding it was the tidier choice — no bare
/// bar under a panel with nothing in it — but it made the panel destroy itself:
/// close the last tab, then collapse, and the strip went with the panel's own
/// corner controls, leaving nothing on screen to click. Ctrl+Space still
/// reopened it, but only for someone who knew the binding; everyone else had to
/// reset the layout to get the panel back. The strip is the one thing that must
/// survive the panel being empty, because reopening is what makes ember's "Add
/// Panel" button reachable again.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_collapsed_bottom_bar(
    bottom: Res<BottomDock>,
    fixed: Res<renzora_ember::dock::FixedDock>,
    sets: Res<BottomPanelSets>,
    fonts: Option<Res<EmberFonts>>,
    registry: Option<Res<renzora::core::ShellPanelRegistry>>,
    bars: Query<Entity, With<CollapsedBottomBar>>,
    mut nodes: Query<&mut Node>,
    mut commands: Commands,
    mut built: Local<Option<(Entity, Vec<String>, String)>>,
) {
    let (Some(fonts), Ok(bar)) = (fonts, bars.single()) else {
        return;
    };
    let Ok(mut node) = nodes.get_mut(bar) else {
        return;
    };
    let mut ids = Vec::new();
    fixed.tree.collect_panels(&mut ids);
    // Nothing to collapse *to* only when the panel is already open.
    if bottom.open {
        if node.display != Display::None {
            node.display = Display::None;
        }
        return;
    }
    if node.display != Display::Flex {
        node.display = Display::Flex;
    }
    // What an empty panel labels itself with, and "" when it has tabs to show
    // instead. Part of the rebuild key so a rename of the empty set repaints,
    // without a rename repainting a strip that isn't showing the name.
    let empty_label = if ids.is_empty() {
        sets.sets
            .get(sets.active)
            .map(|(name, _)| name.clone())
            .unwrap_or_else(default_panel_set_name)
    } else {
        String::new()
    };
    // Keyed on the bar entity too: a theme/language chrome respawn creates a
    // fresh (childless) bar, which must rebuild even for the same tab set.
    if built.as_ref() == Some(&(bar, ids.clone(), empty_label.clone())) {
        return;
    }
    *built = Some((bar, ids.clone(), empty_label.clone()));

    commands.entity(bar).despawn_related::<Children>();
    if !empty_label.is_empty() {
        // Italic would be the usual "nothing here" cue, but the UI font has no
        // italic face — the muted colour and the em dash carry it instead.
        let empty = renzora::lang::t_or("shell.bottom_dock.empty_hint", "empty");
        let hint = commands
            .spawn((
                Text::new(format!("{empty_label} — {empty}")),
                ui_font(&fonts.ui, 12.0),
                TextColor(rgb(placeholder())),
                bevy::text::TextLayout::no_wrap(),
                Node {
                    margin: UiRect::horizontal(Val::Px(9.0)),
                    ..default()
                },
                Name::new("closed-bottom-empty"),
            ))
            .id();
        commands.entity(bar).add_child(hint);
    }
    for id in ids {
        let (title, icon) = registry
            .as_ref()
            .and_then(|r| r.panels.get(&id))
            .map(|info| {
                let icon = if info.icon.is_empty() {
                    "circle".to_string()
                } else {
                    info.icon.clone()
                };
                (info.title.clone(), icon)
            })
            .unwrap_or_else(|| (renzora_ember::dock::humanize(&id), "circle".to_string()));
        let tab = commands
            .spawn((
                Node {
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(5.0),
                    padding: UiRect::horizontal(Val::Px(9.0)),
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Interaction::default(),
                renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                CollapsedBottomTab(id.clone()),
                Name::new(format!("closed-bottom-tab:{id}")),
            ))
            .id();
        let ic = icon_text(&mut commands, &fonts.phosphor, &icon, text_muted(), 13.0);
        let label = commands
            .spawn((
                Text::new(title),
                ui_font(&fonts.ui, 12.0),
                TextColor(rgb(text_muted())),
                bevy::text::TextLayout::no_wrap(),
            ))
            .id();
        commands.entity(tab).add_children(&[ic, label]);
        commands.entity(bar).add_child(tab);
    }

    // Right-aligned open chevron (mirrors the open panel's collapse chevron).
    //
    // The filler carries the resize cursor, not the bar. `apply_cursor_icon`
    // takes the first hovered entity with a `HoverCursor` and does no topmost
    // resolution, so a cursor on the bar competes with the tabs and the chevron
    // nested inside it — which is why hovering a closed-strip tab showed the
    // resize cursor. On the filler it can only be hovered over empty space.
    // The bar keeps its `Interaction` so `collapsed_bottom_bar_drag` still sees
    // the press anywhere along it.
    let strip_filler = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                height: Val::Percent(100.0),
                ..default()
            },
            Interaction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::NsResize),
            Name::new("closed-bottom-filler"),
        ))
        .id();
    let chev = icon_text(&mut commands, &fonts.phosphor, "caret-up", text_muted(), 13.0);
    let open_btn = commands
        .spawn((
            Node {
                height: Val::Percent(100.0),
                width: Val::Px(24.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            CollapsedBottomOpenBtn,
            Name::new("closed-bottom-open"),
        ))
        .id();
    commands.entity(open_btn).add_child(chev);
    commands.entity(bar).add_children(&[strip_filler, open_btn]);
}

// `position_collapsed_bottom_bar` lived here. It pulled the collapsed strip out
// of the chrome flow and sized it to the on-screen span of whichever column its
// stash was anchored under, so a strip that had been nested below the viewport
// collapsed in place rather than spanning the window.
//
// The global bottom panel has no anchor to align to — it is one full-width
// region below every workspace by construction — so the strip is simply the
// full-width chrome row it always was when unanchored, and the whole
// measure-the-leaves pass is dead weight.

/// Click a collapsed-strip tab → open the bottom panel with the clicked panel
/// as the active tab.
pub(crate) fn collapsed_bottom_tab_click(
    tabs: Query<(&Interaction, &CollapsedBottomTab), Changed<Interaction>>,
    wraps: Query<&ComputedNode, With<DockAreaWrap>>,
    mut bottom: ResMut<BottomDock>,
    mut fixed: ResMut<renzora_ember::dock::FixedDock>,
) {
    for (interaction, tab) in &tabs {
        if *interaction != Interaction::Pressed {
            continue;
        }
        bottom.open = true;
        // Open to the standard share of the dock region rather than the height
        // it last had. Clicking a *tab* is a request to look at that panel, and
        // the remembered height could be anything — including the near-minimum a
        // drag-to-close leaves behind, which would reopen to a sliver of the
        // panel the click was asking to see.
        if let Some(h) = default_open_height(&wraps) {
            bottom.height = h;
        }
        fixed.tree.set_active_tab(&tab.0);
        fixed.dirty = true;
        return;
    }
}

/// Click the collapsed strip's open chevron → open the bottom panel at its
/// remembered height.
pub(crate) fn collapsed_bottom_open_click(
    btns: Query<&Interaction, (With<CollapsedBottomOpenBtn>, Changed<Interaction>)>,
    mut bottom: ResMut<BottomDock>,
) {
    for interaction in &btns {
        if *interaction != Interaction::Pressed {
            continue;
        }
        bottom.open = true;
        return;
    }
}

/// Drag the collapsed strip's empty background upward → open the bottom panel
/// and continue as a live resize of its top edge, so opening and sizing are one
/// gesture. Tabs and the open chevron sit above the bar and capture their own
/// presses, so this only fires from the strip's own background.
///
/// This used to hand the held cursor to ember via `GrabRootDivider`, because
/// the panel's height *was* a split ratio inside the workspace tree and only a
/// dock divider could drive it. The panel is an overlay with its own height
/// now, so the shell drives the drag itself and ember never hears about it —
/// which is also what keeps the resize from touching the workspace layout.
pub(crate) fn collapsed_bottom_bar_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    bars: Query<&Interaction, With<CollapsedBottomBar>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut bottom: ResMut<BottomDock>,
    mut resize: ResMut<BottomDockResize>,
    mut press_y: Local<Option<f32>>,
) {
    if !mouse.pressed(MouseButton::Left) {
        *press_y = None;
        return;
    }
    let Some(cursor_y) = windows
        .single()
        .ok()
        .and_then(|w| w.cursor_position())
        .map(|c| c.y)
    else {
        return;
    };
    if mouse.just_pressed(MouseButton::Left) {
        if bars.iter().any(|i| matches!(i, Interaction::Pressed)) {
            *press_y = Some(cursor_y);
        }
        return;
    }
    let Some(start_y) = *press_y else { return };
    // A few px of upward travel arms the gesture (a plain click does nothing
    // — the tabs and the chevron own click-to-open).
    if start_y - cursor_y < 4.0 {
        return;
    }
    *press_y = None;
    // Open at the minimum and let the drag grow it from there, so the top edge
    // tracks the cursor from where the gesture started rather than jumping to
    // the remembered height and then following.
    bottom.open = true;
    // No slide: the panel's top edge is being held by the cursor for the rest
    // of this gesture, and an animation would put it somewhere else while the
    // drag says it is here. Direct manipulation is its own transition.
    bottom.slide = 1.0;
    bottom.height = dock::BOTTOM_DOCK_MIN_HEIGHT;
    resize.active = Some((cursor_y, dock::BOTTOM_DOCK_MIN_HEIGHT));
}

/// Collapsed-strip tabs highlight on hover (they're otherwise muted —
/// reading as closed, not active).
pub(crate) fn collapsed_bottom_tab_hover(
    mut tabs: Query<
        (&Interaction, &mut BackgroundColor),
        Or<(With<CollapsedBottomTab>, With<CollapsedBottomOpenBtn>)>,
    >,
) {
    for (interaction, mut bg) in &mut tabs {
        let want = if matches!(interaction, Interaction::Hovered | Interaction::Pressed) {
            rgb(tab_active())
        } else {
            Color::NONE
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}
