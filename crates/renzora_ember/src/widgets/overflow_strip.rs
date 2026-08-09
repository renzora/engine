//! A horizontal strip of items that folds whatever doesn't fit into a "more"
//! dropdown, instead of growing without bound.
//!
//! WHY this exists: the editor's document tabs and workspace ribbon both live in
//! the top bar, one on each side of the centered ribbon. A strip that sizes to
//! its content moves everything around it every time an item is added — open a
//! scene and the whole workspace ribbon slides sideways. Capping the strip's
//! width fixes the layout, but then the tabs past the cap would simply be
//! unreachable, so the overflow has to go somewhere: the caret button at the end
//! of the strip, which opens a menu of exactly the items that were hidden.
//!
//! WHY the fit is computed from a *cached* per-item width rather than from the
//! live layout: a hidden node measures zero, so folding an item would shrink the
//! measured content, which would make the next item look like it fits, which
//! would unfold it again — a one-frame oscillation. Each item remembers the last
//! width it had while visible ([`OverflowWidth`]), and the fit is decided
//! against those remembered widths and the strip's [`OverflowBudget`]. Nothing
//! in the decision depends on what is currently folded, so it converges in one
//! pass.
//!
//! WHY a new item is measured *invisibly*: it has no remembered width yet, and
//! the obvious answer — leave it in the flow for one frame so it can be measured
//! — is one frame of a tab visibly sitting in a strip it doesn't fit in before
//! being folded away, which is exactly what the user sees as a flicker when they
//! add a document to a full strip. Instead [`probe_new_item`] takes every new
//! item out of the flow and hides it at spawn; taffy still measures it, and
//! [`overflow_fit`] puts it back the moment it has a width. For an item whose
//! *label* the strip has measured before — every row a keyed list rebuilds when
//! its content changes — that happens in the same frame it was built, via
//! [`OverflowStrip::widths`], so a rebuild never blinks either.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::world::DeferredWorld;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::PrimaryWindow;

use crate::font::EmberFonts;
use crate::theme::{rgb, text_muted, text_primary};

use super::popup::{menu_item, menu_row_visual, screen_menu, ScreenMenu};

/// Where a strip's width budget comes from.
#[derive(Clone, Copy)]
pub enum OverflowBudget {
    /// A constant number of logical pixels. For a strip whose surroundings must
    /// not move as it grows and which has no container of its own to measure.
    Fixed(f32),
    /// Whatever `measure` was laid out to, less `reserve` for the buttons that
    /// share that container with the strip (an overflow caret, an add button).
    ///
    /// Use this wherever the strip has a container that fills its slot: nothing
    /// folds while the container still has room, which a constant cap can't
    /// promise — it either folds early on a wide window or overflows a narrow one.
    Fill { measure: Entity, reserve: f32 },
}

/// The items container of an [`overflow_strip`]: its children are folded from
/// the end until they fit inside the strip's budget.
#[derive(Component)]
pub struct OverflowStrip {
    /// The caret button revealed while anything is folded.
    more: Entity,
    budget: OverflowBudget,
    /// Space between items, in logical px. Held here as well as on the `Node`
    /// because the fold has to reserve it per item — a strip laid out flush
    /// (gap 0) that still budgeted for gaps would fold one item early.
    gap: f32,
    /// Last measured width per [`OverflowEntry::label`].
    ///
    /// This is what the per-item [`OverflowWidth`] cannot be: when a keyed list
    /// rebuilds a row (a tab going active, picking up a modified marker) the row
    /// is a *new entity*, so its remembered width is gone with the old one and it
    /// would have to be re-probed — i.e. blink out — every time. The label is the
    /// same across that rebuild, so the width found under it is still good.
    widths: HashMap<String, f32>,
}

/// What an item contributes to the overflow menu when it's folded away. Put this
/// on every direct child of the strip's item container — a child without one can
/// still be folded, it just won't be reachable from the menu (and, having no
/// label, has to be re-measured after every rebuild).
#[derive(Component, Clone)]
#[component(on_add = probe_new_item)]
pub struct OverflowEntry {
    pub icon: String,
    pub label: String,
    /// Runs when the item is picked from the overflow menu — normally the same
    /// thing clicking the item itself would do.
    pub action: Arc<dyn Fn(&mut World) + Send + Sync>,
    /// Runs when the item is *dragged* out of the overflow menu instead of
    /// clicked. Setting it also changes when `action` fires: the row waits for
    /// the button to come back up, since a press is now how a drag starts too.
    pub drag: Option<Arc<dyn Fn(&mut World) + Send + Sync>>,
}

impl OverflowEntry {
    pub fn new<F>(icon: &str, label: &str, action: F) -> Self
    where
        F: Fn(&mut World) + Send + Sync + 'static,
    {
        Self {
            icon: icon.to_string(),
            label: label.to_string(),
            action: Arc::new(action),
            drag: None,
        }
    }

    /// Let this item be dragged out of the overflow menu — for a strip whose
    /// order the user can rearrange, so a folded item isn't stuck behind the
    /// caret with no way to move it back out.
    pub fn on_drag<F>(mut self, drag: F) -> Self
    where
        F: Fn(&mut World) + Send + Sync + 'static,
    {
        self.drag = Some(Arc::new(drag));
        self
    }
}

/// Hide a freshly built strip item until [`overflow_fit`] knows where it goes.
///
/// `position: absolute` takes it out of the flow so it can't push its neighbours
/// around, and `Visibility::Hidden` keeps it off screen — but taffy still lays it
/// out, so the measurement the fit needs still happens. See the module docs for
/// why measuring it *in* the flow (the obvious alternative) is the flicker.
fn probe_new_item(mut world: DeferredWorld, ctx: HookContext) {
    let item = ctx.entity;
    if let Some(mut node) = world.get_mut::<Node>(item) {
        node.position_type = PositionType::Absolute;
    }
    world
        .commands()
        .entity(item)
        .insert((Visibility::Hidden, OverflowProbing(0)));
}

/// A strip item that hasn't been measured yet — see [`probe_new_item`]. The
/// counter is a backstop: an item that never reports a width (an empty row, one
/// inside a collapsed panel) must still be let back into the flow rather than
/// left invisible forever.
#[derive(Component)]
pub(crate) struct OverflowProbing(u8);

/// Put a probed item back into the flow, carrying the width the fit will use.
fn unprobe(commands: &mut Commands, nodes: &mut Query<&mut Node>, item: Entity, width: f32) {
    if let Ok(mut node) = nodes.get_mut(item) {
        if node.position_type != PositionType::Relative {
            node.position_type = PositionType::Relative;
        }
    }
    commands
        .entity(item)
        .try_insert((OverflowWidth(width), Visibility::Inherited))
        .remove::<OverflowProbing>();
}

/// Marks an item that must stay visible even when it would otherwise be folded
/// — the active tab / workspace. Its width is reserved before anything else, so
/// it holds its place in the strip rather than disappearing into the menu you'd
/// have to open to see where you are.
#[derive(Component)]
pub struct OverflowKeep;

/// The last width this item measured while visible (logical px). Cached because
/// a folded node measures zero — see the module docs.
#[derive(Component)]
pub(crate) struct OverflowWidth(f32);

/// The caret button at the end of a strip, tagged with the strip it folds for.
#[derive(Component)]
pub(crate) struct OverflowMore {
    items: Entity,
}

/// Build a capped strip. Returns `(row, items)`: put `row` where the strip goes
/// and add the items to `items`.
///
/// The item container always hugs its content, so a button placed after the
/// strip sits right against the last item rather than stranded at the far edge.
pub fn overflow_strip(
    commands: &mut Commands,
    budget: OverflowBudget,
    name: &str,
) -> (Entity, Entity) {
    overflow_strip_gap(commands, budget, DEFAULT_GAP, name)
}

/// Default space between a strip's items, in logical px.
const DEFAULT_GAP: f32 = 2.0;

/// [`overflow_strip`] with an explicit gap between items — `0.0` for a strip
/// whose items butt up against each other (the document tabs).
pub fn overflow_strip_gap(
    commands: &mut Commands,
    budget: OverflowBudget,
    gap: f32,
    name: &str,
) -> (Entity, Entity) {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                height: Val::Percent(100.0),
                column_gap: Val::Px(gap),
                min_width: Val::Px(0.0),
                ..default()
            },
            // Structural: clicks between items fall through to whatever the
            // strip sits on (e.g. a title bar's window-drag handle).
            bevy::ui::FocusPolicy::Pass,
            Name::new(format!("{name}-strip")),
        ))
        .id();

    let more = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(5.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                flex_shrink: 0.0,
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new(format!("{name}-overflow")),
        ))
        .id();

    let items = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                height: Val::Percent(100.0),
                column_gap: Val::Px(gap),
                min_width: Val::Px(0.0),
                max_width: match budget {
                    OverflowBudget::Fixed(cap) => Val::Px(cap),
                    OverflowBudget::Fill { .. } => Val::Auto,
                },
                // Belt and braces: the fold keeps the content inside the budget,
                // but a single item wider than the whole of it would still spill.
                overflow: Overflow::clip(),
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
            OverflowStrip { more, budget, gap, widths: HashMap::new() },
            Name::new(format!("{name}-items")),
        ))
        .id();
    commands.entity(more).insert(OverflowMore { items });

    let glyph = crate::font::glyph(commands, "caret-down", text_muted(), 12.0);
    commands.entity(more).add_child(glyph);
    crate::reactive::tracked::bind_bg(commands, more, move |w| match w.get::<Interaction>(more) {
        Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(crate::theme::hover_bg()),
        _ => Color::NONE,
    });

    commands.entity(row).add_children(&[items, more]);
    (row, items)
}

/// Measure the strip's items, then fold the ones past the budget and show/hide
/// the caret.
///
/// Ordered **after** the reactive keyed lists that fill these strips, so a row
/// built this frame is decided this frame, before the layout that would have
/// drawn it in the wrong place. Everything else runs off the previous frame's
/// layout, which is all a settled item needs.
pub(crate) fn overflow_fit(
    mut strips: Query<(&mut OverflowStrip, &Children)>,
    keep: Query<(), With<OverflowKeep>>,
    entries: Query<&OverflowEntry>,
    mut probing: Query<&mut OverflowProbing>,
    mut nodes: Query<&mut Node>,
    sizes: Query<&ComputedNode>,
    mut widths: Query<&mut OverflowWidth>,
    mut commands: Commands,
) {
    let measured = |e: Entity| {
        sizes
            .get(e)
            .map(|cn| cn.size().x * cn.inverse_scale_factor())
            .unwrap_or(0.0)
    };
    for (mut strip, children) in &mut strips {
        let cap = match strip.budget {
            OverflowBudget::Fixed(cap) => cap,
            // Zero until the container has been laid out once; folding
            // everything on that first frame would be wrong, so treat it as
            // "room for now" and settle next frame.
            OverflowBudget::Fill { measure, reserve } => match measured(measure) {
                w if w <= 0.0 => f32::MAX,
                w => (w - reserve).max(0.0),
            },
        };

        // Settle a width for every item, and let each newly built one out of its
        // probe the moment one is known. An item still probing is left out of
        // `sized` entirely — it's absolutely positioned, so it takes no space in
        // the strip and has no business in the fit either.
        let mut sized: Vec<(Entity, f32)> = Vec::with_capacity(children.len());
        let mut labels: Vec<String> = Vec::with_capacity(children.len());
        for child in children.iter() {
            let label = entries.get(child).ok().map(|e| e.label.clone());
            if let Some(label) = &label {
                labels.push(label.clone());
            }

            if let Ok(mut probe) = probing.get_mut(child) {
                // Rebuilt under a label the strip has measured before: adopt that
                // width and un-probe now, in the same frame the row was built, so
                // it never renders hidden at all.
                if let Some(w) = label.as_ref().and_then(|l| strip.widths.get(l).copied()) {
                    unprobe(&mut commands, &mut nodes, child, w);
                    sized.push((child, w));
                    continue;
                }
                let w = measured(child);
                probe.0 = probe.0.saturating_add(1);
                if w <= 0.0 && probe.0 < 3 {
                    continue;
                }
                unprobe(&mut commands, &mut nodes, child, w);
                if let Some(label) = label {
                    strip.widths.insert(label, w);
                }
                sized.push((child, w));
                continue;
            }

            // Settled: refresh the remembered width while the item is visible. A
            // zero measurement is either a folded item or one mid-relayout;
            // neither should overwrite a good reading.
            let folded = nodes.get(child).map(|n| n.display == Display::None).unwrap_or(false);
            let w = measured(child);
            if folded || w <= 0.0 {
                sized.push((child, widths.get(child).map(|c| c.0).unwrap_or(0.0)));
                continue;
            }
            match widths.get_mut(child) {
                Ok(mut cached) => {
                    if (cached.0 - w).abs() > 0.5 {
                        cached.0 = w;
                    }
                }
                Err(_) => {
                    commands.entity(child).try_insert(OverflowWidth(w));
                }
            }
            if let Some(label) = label {
                if strip.widths.get(&label).is_none_or(|c| (c - w).abs() > 0.5) {
                    strip.widths.insert(label, w);
                }
            }
            sized.push((child, w));
        }
        // Don't let the label cache outlive the strip's contents — closed tabs
        // and renamed ones would otherwise accumulate in it for the session.
        if strip.widths.len() > labels.len() {
            strip.widths.retain(|k, _| labels.iter().any(|l| l == k));
        }

        // The kept item (active tab / workspace) claims its width first, so a
        // long run of earlier items can't push it into the menu.
        let gap = strip.gap;
        let mut used: f32 = sized
            .iter()
            .filter(|(e, _)| keep.contains(*e))
            .map(|(_, w)| w + gap)
            .sum();

        let mut folded_any = false;
        let mut show: Vec<(Entity, bool)> = Vec::with_capacity(sized.len());
        for &(child, width) in &sized {
            if keep.contains(child) {
                show.push((child, true));
                continue;
            }
            let w = width + gap;
            // A zero-width item is one the probe gave up on; leave it visible
            // rather than folding something that was never really measured.
            if w <= gap || used + w <= cap {
                used += w;
                show.push((child, true));
            } else {
                folded_any = true;
                show.push((child, false));
            }
        }

        for (child, visible) in show {
            let Ok(mut node) = nodes.get_mut(child) else { continue };
            let want = if visible { Display::Flex } else { Display::None };
            if node.display != want {
                node.display = want;
            }
        }

        if let Ok(mut more) = nodes.get_mut(strip.more) {
            let want = if folded_any { Display::Flex } else { Display::None };
            if more.display != want {
                more.display = want;
            }
        }
    }
}

/// Click the `»` button → a menu of the folded items, anchored under it.
pub(crate) fn overflow_more_click(
    windows: Query<&Window, With<PrimaryWindow>>,
    buttons: Query<
        (&Interaction, &OverflowMore, &UiGlobalTransform, &ComputedNode),
        Changed<Interaction>,
    >,
    strips: Query<&Children, With<OverflowStrip>>,
    folded: Query<(&Node, &OverflowEntry)>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else { return };
    for (interaction, more, ugt, cn) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Ok(children) = strips.get(more.items) else { continue };
        let rows: Vec<OverflowEntry> = children
            .iter()
            .filter_map(|child| {
                let (node, entry) = folded.get(child).ok()?;
                (node.display == Display::None).then(|| entry.clone())
            })
            .collect();
        if rows.is_empty() {
            continue;
        }
        // The button's own box in logical window px — `UiGlobalTransform` holds
        // the node's centre (a UI node's `GlobalTransform` sits at the origin).
        let inv = cn.inverse_scale_factor();
        let half = cn.size() * 0.5 * inv;
        let mid = ugt.translation * inv;
        let win_w = windows.iter().next().map(|w| w.width()).unwrap_or(f32::MAX);
        let x = (mid.x - half.x).min((win_w - 200.0).max(0.0)).max(0.0);
        let menu = screen_menu(&mut commands, x, mid.y + half.y + 2.0);
        let kids: Vec<Entity> = rows
            .into_iter()
            .map(|entry| match entry.drag.clone() {
                Some(drag) => {
                    let row = menu_row_visual(
                        &mut commands,
                        &fonts,
                        &entry.icon,
                        &entry.label,
                        text_muted(),
                        text_primary(),
                    );
                    commands.entity(row).insert(OverflowMenuRow {
                        click: entry.action.clone(),
                        drag,
                    });
                    row
                }
                None => {
                    let action = entry.action.clone();
                    menu_item(&mut commands, &fonts, &entry.icon, &entry.label, move |w| {
                        (action)(w)
                    })
                }
            })
            .collect();
        commands.entity(menu).add_children(&kids);
    }
}

/// A row in a strip's overflow menu that can be dragged back out of it, for a
/// strip whose order the user rearranges by dragging.
#[derive(Component)]
pub(crate) struct OverflowMenuRow {
    click: Arc<dyn Fn(&mut World) + Send + Sync>,
    drag: Arc<dyn Fn(&mut World) + Send + Sync>,
}

/// Press-latch input for [`OverflowMenuRow`]: a plain click runs the item's
/// action when the button comes back up, while moving past a small threshold
/// with it held hands the item to the host's drag instead. Either way the menu
/// closes.
///
/// An ordinary menu row acts on **press** ([`super::popup::MenuAction`]), which
/// can't work here — the press is also how a drag starts, so acting on it would
/// activate the very item you were trying to move.
pub(crate) fn overflow_menu_row_input(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    rows: Query<(Entity, &Interaction, &OverflowMenuRow)>,
    menus: Query<Entity, With<ScreenMenu>>,
    mut held: Local<Option<(Entity, Vec2)>>,
    mut commands: Commands,
) {
    let cursor = windows.iter().next().and_then(|w| w.cursor_position());
    let Some((row, origin)) = *held else {
        if mouse.just_pressed(MouseButton::Left) {
            if let Some(cursor) = cursor {
                if let Some((row, _, _)) = rows.iter().find(|(_, i, _)| **i == Interaction::Pressed)
                {
                    *held = Some((row, cursor));
                }
            }
        }
        return;
    };
    // The menu can be torn down under us (an outside click dismissing it).
    let Ok((_, interaction, entry)) = rows.get(row) else {
        *held = None;
        return;
    };

    let dragged = cursor.is_some_and(|c| (c - origin).length() > 5.0);
    let released = !mouse.pressed(MouseButton::Left);
    if !dragged && !released {
        return;
    }
    *held = None;

    // A release still over the row is a plain click; one that wandered off it
    // without passing the threshold is a cancelled press and does nothing.
    let action = if dragged {
        entry.drag.clone()
    } else if matches!(interaction, Interaction::Pressed | Interaction::Hovered) {
        entry.click.clone()
    } else {
        return;
    };
    commands.queue(move |world: &mut World| (action)(world));
    for menu in &menus {
        commands.entity(menu).despawn();
    }
}
