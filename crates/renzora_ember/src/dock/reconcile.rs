//! Turning a [`DockTree`] into bevy_ui entities, and keeping them themed.
//!
//! The delicate part is **content preservation**. A rebuild despawns the area's
//! whole subtree, so each leaf's content node is detached to the root first and
//! re-parented by `build_tree` — otherwise reordering a tab would recreate the
//! panel and lose its state. But a content node that is detached *and then
//! despawned in the same frame* corrupts bevy_ui's taffy tree, so only contents
//! the new tree will actually reuse may be detached. See [`rebuild_area`], which
//! carries the full reasoning at each of its three guards.

use std::collections::HashMap;

use bevy::prelude::*;

use crate::font::{icon_text, ui_font, EmberFonts};
use crate::theme::{accent, border, divider, header_bg, panel_bg, rgb, tab_active, text_muted, text_primary};

use crate::dock::components::{
    BottomCollapseBtn, BottomStripMarkers, Divider, DockLeaf, DockTab, DropOverlay, FixedAreaHeader,
    InsertMarker, LeafGrip, ParentSplit, RootDropOverlay, TabBarOf, TabClose,
};
use crate::dock::tree::{DockTree, SplitDirection};
use crate::dock::windows::{DockWindows, FloatingDockArea};
use crate::dock::{tab_meta, Dock, DockArea, DockDirty, FixedDock};

/// (Re)build each dirty dock: the primary [`DockArea`] when [`DockDirty`], and
/// every floating dock window whose per-window flag is set.
// A system's parameters are not an argument list a caller has to thread — the
// same allow `rebuild_area` below carries, and for the same reason.
#[allow(clippy::too_many_arguments)]
pub(crate) fn rebuild_dock(
    mut dirty: ResMut<DockDirty>,
    mut wins: ResMut<DockWindows>,
    mut fixed: ResMut<FixedDock>,
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    dock: Res<Dock>,
    markers: Res<BottomStripMarkers>,
    areas: Query<(Entity, Option<&Children>, Option<&FloatingDockArea>), With<DockArea>>,
    leaves: Query<&DockLeaf>,
    // Liveness probe for `DockLeaf::content`. A leaf stores its content node as a
    // bare `Entity`, which nothing invalidates when that node is despawned — see
    // `rebuild_area` for why a stale one has to be filtered out rather than
    // re-parented.
    alive: Query<Entity>,
) {
    let Some(fonts) = fonts else {
        return;
    };
    // The primary is "the non-floating area that isn't the fixed one". It used
    // to be just "the non-floating one", which was unambiguous while the fixed
    // area didn't exist; with it spawned, whichever the query happened to yield
    // first would get the primary's tree built into it.
    let fixed_area = fixed.area;
    if dirty.0 {
        if let Some((area_entity, children, _)) = areas
            .iter()
            .find(|(e, _, f)| f.is_none() && Some(*e) != fixed_area)
        {
            rebuild_area(
                &mut commands, &fonts, &markers.0, &dock.tree, area_entity, children, &leaves,
                &alive, false, true,
            );
            dirty.0 = false;
        }
    }
    if fixed.dirty {
        // Same retry-next-frame rule as a floating window: the consumer's node
        // is spawned with commands, so it may not exist the frame it is asked
        // for. Leaving the flag set is what makes a chrome respawn refill it.
        if let Some((area_entity, children, _)) = fixed_area.and_then(|a| areas.get(a).ok()) {
            rebuild_area(
                &mut commands, &fonts, &markers.0, &fixed.tree, area_entity, children, &leaves,
                &alive, false, false,
            );
            fixed.dirty = false;
        }
    }
    for st in wins.0.iter_mut().filter(|s| s.dirty) {
        // The area may not exist yet the frame the window spawns (commands
        // apply at the end of the frame) — leave it dirty and retry next frame.
        if let Ok((area_entity, children, _)) = areas.get(st.area) {
            rebuild_area(
                &mut commands, &fonts, &markers.0, &st.tree, area_entity, children, &leaves,
                &alive, true, true,
            );
            st.dirty = false;
        }
    }
}

/// Rebuild one dock area's subtree from `tree`. Shared by the primary dock and
/// floating dock windows; `floating` leaves are chromeless (no tab bar — the
/// window's own title bar plays that role).
#[allow(clippy::too_many_arguments)]
fn rebuild_area(
    commands: &mut Commands,
    fonts: &EmberFonts,
    markers: &[String],
    tree: &DockTree,
    area_entity: Entity,
    children: Option<&Children>,
    leaves: &Query<&DockLeaf>,
    alive: &Query<Entity>,
    floating: bool,
    // Whether whole leaves in this area may be dragged elsewhere. `false` drops
    // the leaf grip from every tab bar — for an area pinned to a fixed slot,
    // where moving the leaf is not a thing that can happen.
    movable: bool,
) {
    // Preserve each leaf's content entity (keyed by its active panel) and detach
    // it from the hierarchy so the despawn below doesn't take it — `build_tree`
    // re-parents it, so reordering/moving tabs keeps the panel (and its state)
    // instead of recreating it.
    //
    // Only detach contents the *new* tree will reuse. A content node that is
    // detached-to-root and then despawned in the same frame (e.g. every non-
    // viewport panel when maximizing) corrupts bevy_ui's taffy tree: it gets
    // reparented to the implicit-viewport root and its slotmap key freed, while
    // its old leaf still lists it as a child — so taffy panics with
    // `invalid SlotMap key` when removing that leaf. Leaving non-reused contents
    // attached lets them despawn safely with their old leaf instead.
    //
    // Scoped to THIS area's leaves: with floating dock windows, another window
    // can legitimately show the same panel id — detaching its content here
    // would steal a live panel out of that window.
    let mut reusable = std::collections::HashSet::new();
    tree.active_tab_ids(&mut reusable);
    let mut preserved: HashMap<String, Entity> = HashMap::new();
    //
    // `alive` is not belt-and-braces. `DockLeaf::content` is a bare `Entity`,
    // and nothing invalidates it when that node is despawned — `sync_panes`
    // despawns every non-active pane, a panel can tear its own content down, and
    // a chrome rebuild can race this one. The despawn loop just below already
    // says exactly that, and uses `try_despawn` for it; the same reasoning was
    // never applied up here. A stale `leaf.content` got preserved, handed to
    // `build_tree`, and re-parented with `add_child` — which, unlike
    // `try_despawn`, has no fallible variant, so it surfaced as a burst of
    // "entity is invalid; its index now has generation N" warnings naming a
    // command the log could not name.
    //
    // Skipping a dead one is also the *correct* outcome, not just a quiet one:
    // `build_tree` builds fresh content for any panel id missing from
    // `preserved`, so the panel comes back rather than vanishing.
    //
    // `preserved` is keyed by panel id, so only the FIRST leaf showing a given
    // id may be detached. Two leaves in one area can legitimately show the same
    // panel — drag a second `hierarchy` tab out of its leaf and both are active
    // in their own leaves — and detaching both then inserting both left the
    // first entity detached-to-root *and* evicted from the map by the second
    // insert. Nothing could reach it after that: the child sweep below walks the
    // area's children and it is no longer anyone's child, and the `drain()`
    // cleanup at the end no longer lists it. It stayed alive and parentless,
    // rendering as a root-level node with no tab bar that the user could not
    // close — and came back on every rebuild (a tab drag) until a restart built
    // the tree from scratch.
    //
    // Leaving the duplicate attached is both the safe and the correct outcome:
    // it despawns with its old leaf (the taffy rule the block above depends on),
    // and `build_tree` builds it fresh, because only one leaf can take the
    // single preserved entity out of the map anyway (`preserved.remove`).
    for leaf in leaves.iter().filter(|l| l.area == area_entity) {
        if !leaf.active.is_empty()
            && reusable.contains(&leaf.active)
            && alive.get(leaf.content).is_ok()
            && !preserved.contains_key(&leaf.active)
        {
            preserved.insert(leaf.active.clone(), leaf.content);
            commands.entity(leaf.content).remove::<ChildOf>();
        }
    }

    // `try_despawn`: a child may already have been despawned this frame by
    // another system (e.g. a panel tearing down its own content, or a chrome
    // rebuild racing this one) — a plain `despawn` on that stale handle panics.
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).try_despawn();
        }
    }
    let tree_root = build_tree(
        commands,
        &fonts.ui,
        &fonts.phosphor,
        markers,
        area_entity,
        floating,
        movable,
        None,
        Vec::new(),
        tree,
        &mut preserved,
    );
    commands.entity(area_entity).add_child(tree_root);

    // Root drop overlay + cursor tracking for edge/corner docking. Added after
    // the tree so the overlay draws on top; recreated with every rebuild (the
    // child-despawn above took the previous one).
    commands
        .entity(area_entity)
        .insert(bevy::ui::RelativeCursorPosition::default());
    let root_overlay = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                border: UiRect::all(Val::Px(2.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(rgb(accent())),
            bevy::ui::FocusPolicy::Pass,
            RootDropOverlay { area: area_entity },
            Name::new("root-drop-overlay"),
        ))
        .id();
    commands.entity(area_entity).add_child(root_overlay);

    // Any preserved content not reused (its panel no longer exists) → despawn.
    // `try_despawn` for the same stale-handle reason as the child sweep above.
    for (_, content) in preserved.drain() {
        commands.entity(content).try_despawn();
    }
}

#[allow(clippy::too_many_arguments)]
fn build_tree(
    commands: &mut Commands,
    font: &bevy::text::FontSource,
    phosphor: &Handle<Font>,
    markers: &[String],
    area: Entity,
    floating: bool,
    movable: bool,
    parent: Option<ParentSplit>,
    path: Vec<bool>,
    tree: &DockTree,
    preserved: &mut HashMap<String, Entity>,
) -> Entity {
    match tree {
        DockTree::Split {
            direction,
            ratio,
            first,
            second,
        } => {
            let row = matches!(direction, SplitDirection::Horizontal);
            let container = commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        min_width: Val::Px(0.0),
                        min_height: Val::Px(0.0),
                        flex_direction: if row {
                            FlexDirection::Row
                        } else {
                            FlexDirection::Column
                        },
                        position_type: PositionType::Relative,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    Name::new("split"),
                ))
                .id();

            let pct = ratio.clamp(0.1, 0.9) * 100.0;

            let mut wa = Node {
                overflow: Overflow::clip(),
                flex_shrink: 0.0,
                ..default()
            };
            if row {
                wa.width = Val::Percent(pct);
                wa.height = Val::Percent(100.0);
            } else {
                wa.height = Val::Percent(pct);
                wa.width = Val::Percent(100.0);
            }
            let wrap_a = commands.spawn((wa, Name::new("split-first"))).id();
            let mut path_a = path.clone();
            path_a.push(false);
            let info_a = ParentSplit {
                container,
                first_wrap: wrap_a,
                horizontal: row,
                is_second: false,
                path: path.clone(),
            };
            let child_a = build_tree(
                commands, font, phosphor, markers, area, floating, movable, Some(info_a), path_a,
                first, preserved,
            );
            commands.entity(wrap_a).add_child(child_a);

            // The divider is a flush 1px line laid out as a flex sibling *between*
            // the two panes, so the panels stay flush (no gap) and the line sits
            // exactly on the boundary at any dock position / nesting / UI scale.
            //
            // The grab area is a wider, transparent, absolutely-positioned child of
            // the line. Centering it uses only negative *px* insets (no percent +
            // margin combo, which taffy doesn't honor on abs-pos nodes — the bug
            // that made the old handle drift off the line). It overhangs both panes
            // without taking layout space; a `ZIndex` lift puts the line + handle
            // above the neighbouring pane so the whole grab width is clickable.
            const GRAB: f32 = 11.0;
            let mut line = Node {
                flex_shrink: 0.0,
                position_type: PositionType::Relative,
                ..default()
            };
            if row {
                line.width = Val::Px(1.0);
                line.height = Val::Percent(100.0);
            } else {
                line.height = Val::Px(1.0);
                line.width = Val::Percent(100.0);
            }
            let divider = commands
                .spawn((
                    line,
                    BackgroundColor(rgb(divider())),
                    bevy::ui::ZIndex(1),
                    DockPart::Divider,
                    Name::new("divider"),
                ))
                .id();

            let cursor = crate::cursor_icon::parse_cursor(if row {
                "ew-resize"
            } else {
                "ns-resize"
            })
            .unwrap();
            let mut hit = Node {
                position_type: PositionType::Absolute,
                ..default()
            };
            if row {
                hit.left = Val::Px(-(GRAB - 1.0) / 2.0);
                hit.width = Val::Px(GRAB);
                hit.top = Val::Px(0.0);
                hit.height = Val::Percent(100.0);
            } else {
                hit.top = Val::Px(-(GRAB - 1.0) / 2.0);
                hit.height = Val::Px(GRAB);
                hit.left = Val::Px(0.0);
                hit.width = Val::Percent(100.0);
            }
            // A vertical split whose bottom pane is a leaf tabbing one of the
            // bottom-strip marker panels is a collapsible region: tag its
            // divider so overshooting it downward snap-closes the strip (the
            // root divider does this content-agnostically via its empty path).
            let strip = if !row && !floating {
                match &**second {
                    DockTree::Leaf { tabs, .. } => {
                        tabs.iter().find(|t| markers.contains(*t)).cloned()
                    }
                    _ => None,
                }
            } else {
                None
            };
            let handle = commands
                .spawn((
                    hit,
                    Interaction::default(),
                    // The strip overhangs `GRAB/2` into both panes, so without
                    // this the press it owns also lands on whatever it covers
                    // (GH #81).
                    crate::resize::ResizeHandle,
                    crate::cursor_icon::HoverCursor(cursor),
                    Divider {
                        container,
                        first_wrap: wrap_a,
                        horizontal: row,
                        path: path.clone(),
                        area,
                        floating,
                        strip,
                    },
                    Name::new("divider-handle"),
                ))
                .id();
            commands.entity(divider).add_child(handle);

            let mut wb = Node {
                overflow: Overflow::clip(),
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                // Without a zero minimum, this flex child's automatic min-size is
                // its (tall) content, so it inflates instead of shrinking to the
                // remaining space — tall panels then clip at the bottom with no
                // scroll. (The first child is capped by its explicit pct size.)
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                ..default()
            };
            if row {
                wb.height = Val::Percent(100.0);
            } else {
                wb.width = Val::Percent(100.0);
            }
            let wrap_b = commands.spawn((wb, Name::new("split-second"))).id();
            let mut path_b = path.clone();
            path_b.push(true);
            let info_b = ParentSplit {
                container,
                first_wrap: wrap_a,
                horizontal: row,
                is_second: true,
                path: path.clone(),
            };
            let child_b = build_tree(
                commands, font, phosphor, markers, area, floating, movable, Some(info_b), path_b,
                second, preserved,
            );
            commands.entity(wrap_b).add_child(child_b);

            commands
                .entity(container)
                .add_children(&[wrap_a, divider, wrap_b]);
            container
        }
        DockTree::Leaf { tabs, active_tab } => build_leaf(
            commands, font, phosphor, markers, area, floating, movable, parent, tabs, *active_tab,
            preserved,
        ),
        DockTree::Empty => {
            let container = commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    Name::new("empty"),
                ))
                .id();
            let btn = commands
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.0),
                        padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(6.0)),
                        ..default()
                    },
                    BackgroundColor(rgb(tab_active())),
                    BorderColor::all(rgb(border())),
                    Interaction::default(),
                    crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                    // No leaf yet — picking sets the tree's root leaf.
                    AddPanelButton {
                        leaf: Entity::PLACEHOLDER,
                        area,
                    },
                    Name::new("empty-add-panel"),
                ))
                .id();
            let ic = icon_text(commands, phosphor, "plus", text_muted(), 14.0);
            let t = commands
                .spawn((
                    Text::new(renzora::lang::t("menu.add_panel")),
                    ui_font(font, 13.0),
                    TextColor(rgb(text_muted())),
                ))
                .id();
            commands.entity(btn).add_children(&[ic, t]);
            commands.entity(container).add_child(btn);
            container
        }
    }
}

/// Which dock element a node styles from [`crate::style::DockStyle`].
#[derive(Component, Clone, Copy)]
pub(crate) enum DockPart {
    Leaf,
    TabBar,
    Divider,
    /// A tab button — geometry (radius/padding) from the style; its bg + text
    /// colors are state-driven by `tab_hover`.
    Tab,
}

/// Paint every [`DockPart`] from the live `Theme.dock` on theme change / spawn —
/// the dock's panel chrome (leaf bg/border/radius/padding + drop shadow, tab bar,
/// dividers) follows the theme and is editable in the Theme tab.
pub(crate) fn apply_dock_style(
    theme: Res<crate::style::Theme>,
    mut commands: Commands,
    mut q: Query<(
        Entity,
        Ref<DockPart>,
        &mut BackgroundColor,
        Option<&mut BorderColor>,
        &mut Node,
    )>,
) {
    let repaint = theme.is_changed();
    let d = &theme.dock;
    for (e, part, mut bg, border, mut node) in &mut q {
        if !repaint && !part.is_added() {
            continue;
        }
        match *part {
            DockPart::Leaf => {
                bg.0 = d.leaf_bg.color();
                node.border = UiRect::all(Val::Px(d.leaf_border_width));
                node.border_radius = BorderRadius::all(Val::Px(d.leaf_radius));
                node.padding = UiRect::all(Val::Px(d.leaf_padding));
                node.margin = UiRect::all(Val::Px(d.leaf_margin));
                if let Some(mut bc) = border {
                    *bc = BorderColor::all(d.leaf_border.color());
                }
                if d.shadow {
                    commands.entity(e).insert(BoxShadow::new(
                        d.shadow_color.color().with_alpha(d.shadow_alpha),
                        Val::Px(d.shadow_x),
                        Val::Px(d.shadow_y),
                        Val::Px(d.shadow_spread),
                        Val::Px(d.shadow_blur),
                    ));
                } else {
                    commands.entity(e).remove::<BoxShadow>();
                }
            }
            DockPart::TabBar => {
                bg.0 = d.tabbar_bg.color();
                node.border = UiRect::bottom(Val::Px(d.header_border_width));
                // Top corners only. The tab bar is the top slice of the leaf, so
                // its upper corners *are* the leaf's — and bevy_ui's clip is a
                // plain `Rect`, so a rounded leaf does not round its children;
                // each node has to round itself or it paints square over the
                // corner. Rounding all four would put a curve halfway down the
                // panel where the bar meets its content.
                node.border_radius = BorderRadius {
                    top_left: Val::Px(d.header_radius),
                    top_right: Val::Px(d.header_radius),
                    bottom_left: Val::Px(0.0),
                    bottom_right: Val::Px(0.0),
                };
                node.padding = UiRect::axes(Val::Px(d.header_pad_x), Val::Px(d.header_pad_y));
                if let Some(mut bc) = border {
                    *bc = BorderColor::all(d.header_border.color());
                }
            }
            DockPart::Tab => {
                node.border_radius = BorderRadius::all(Val::Px(d.tab_radius));
                node.padding = UiRect::axes(Val::Px(d.tab_pad_x), Val::Px(d.tab_pad_y));
            }
            DockPart::Divider => bg.0 = d.divider.color(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_leaf(
    commands: &mut Commands,
    font: &bevy::text::FontSource,
    phosphor: &Handle<Font>,
    markers: &[String],
    area: Entity,
    floating: bool,
    movable: bool,
    parent: Option<ParentSplit>,
    tabs: &[String],
    active: usize,
    preserved: &mut HashMap<String, Entity>,
) -> Entity {
    let leaf = commands
        .spawn((
            Node {
                // Flex-fill the cell (equivalent to 100%/100% when margin is 0)
                // so a `leaf_margin` insets the panel cleanly instead of
                // overflowing the way a fixed 100% size + margin would.
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                align_self: AlignSelf::Stretch,
                // Force a zero minimum so tall panel content can't push the
                // leaf's content-based min-size up and disturb sibling splits.
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            BorderColor::all(Color::NONE),
            DockPart::Leaf,
            crate::widgets::ThemeShaderSurface {
                surface: crate::widgets::ThemeSurface::Panel,
            },
            bevy::ui::RelativeCursorPosition::default(),
            Name::new("leaf"),
        ))
        .id();
    populate_leaf(
        commands, font, phosphor, markers, area, floating, movable, parent, leaf, tabs, active,
        preserved,
    );
    leaf
}

/// The tab bar's `+` button — opens the Add-Panel picker for its leaf.
#[derive(Component)]
pub(crate) struct AddPanelButton {
    leaf: Entity,
    /// The dock area the leaf lives in — routes the add to the right tree.
    area: Entity,
}

/// The horizontally-clipping container holding a leaf's tabs. When the tabs
/// overflow the leaf width it clips them (`Overflow::scroll_x`) and the wheel
/// pans it ([`tab_strip_wheel`]) — there is no visible scrollbar, so it reads
/// as tabs that quietly slide under the pinned `+`/collapse controls. Carries
/// its leaf so the in-place tab reorder (`tab_drag`) can re-sort *the strip's*
/// children (the tabs) rather than the tab bar's — the bar also holds the grip,
/// `+`, and collapse chevron, which a wholesale reorder would drop.
#[derive(Component)]
pub(crate) struct TabScrollStrip {
    pub(crate) leaf: Entity,
}

/// The undock handle at the left of a tab: press it to tear that panel off
/// into a floating window — no Ctrl needed. Overlaid absolutely on the tab's
/// left padding so it takes NO layout space (no gap in unhovered tabs), and
/// kept `Display::None` until the tab is hovered so the hidden handle can't
/// intercept clicks either. `FocusPolicy::Block` so pressing it undocks
/// instead of switching/dragging the tab (the same trick the close × uses).
#[derive(Component)]
pub(crate) struct TabGrip {
    /// The tab this handle sits in (drives hover visibility).
    pub(crate) tab: Entity,
    /// Panel id to undock.
    pub(crate) id: String,
    pub(crate) leaf: Entity,
    pub(crate) area: Entity,
}

/// Click the tab bar `+` → open the shared search overlay of panels not already
/// in that leaf; selecting one adds it as a tab (mirrors the egui panel picker).
pub(crate) fn add_panel_click(
    q: Query<(&Interaction, &AddPanelButton), Changed<Interaction>>,
    leaves: Query<&DockLeaf>,
    fonts: Option<Res<EmberFonts>>,
    registry: Option<Res<renzora::core::ShellPanelRegistry>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else {
        return;
    };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let leaf = btn.leaf;
        let area = btn.area;
        let existing: std::collections::HashSet<String> = leaves
            .get(leaf)
            .map(|l| l.tabs.iter().cloned().collect())
            .unwrap_or_default();
        let mut entries: Vec<crate::widgets::SearchEntry> = Vec::new();
        if let Some(reg) = &registry {
            let mut items: Vec<(&String, &renzora::core::ShellPanelInfo)> =
                reg.panels.iter().collect();
            items.sort_by(|a, b| a.1.title.cmp(&b.1.title));
            for (id, info) in items {
                if existing.contains(id.as_str()) {
                    continue;
                }
                let id = id.clone();
                let icon = if info.icon.is_empty() {
                    "circle".to_string()
                } else {
                    info.icon.clone()
                };
                let category_en = if info.category.is_empty() {
                    "General".to_string()
                } else {
                    info.category.clone()
                };
                // Localized category header. The slug (lowercased, non-alphanumerics
                // → '_') keys `panel.cat.<slug>`; grouping still keys off this string,
                // so all entries in one English category share one localized header.
                let slug: String = category_en
                    .trim()
                    .to_lowercase()
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                    .collect();
                let category = renzora::lang::t_or(&format!("panel.cat.{}", slug), &category_en);
                // Localized panel display name — reuses the `panel.<id>` keys seeded
                // by the chrome pass; falls back to the registry's English title.
                let label = renzora::lang::t_or(&format!("panel.{}", id), &info.title);
                entries.push(crate::widgets::SearchEntry::new(
                    icon,
                    label,
                    category,
                    move |w: &mut World| {
                        let sibling = w.get::<DockLeaf>(leaf).and_then(|l| l.tabs.first().cloned());
                        // Route to the tree owning this leaf's area — the fixed
                        // (global bottom) dock's, else a floating dock window's,
                        // else the primary dock's.
                        let add = |tree: &mut DockTree| match &sibling {
                            Some(sib) => {
                                tree.add_tab(sib, id.clone());
                            }
                            None => *tree = DockTree::leaf(id.clone()),
                        };
                        // The fixed area has to be checked first, exactly as
                        // `area_tree_mut` does. It wasn't, so picking a panel
                        // from the global bottom panel's `+` fell through to the
                        // primary tree and added the tab to the workspace hidden
                        // behind the overlay — the bottom panel never changed, so
                        // the button read as dead.
                        if w.get_resource::<FixedDock>().is_some_and(|f| f.area == Some(area)) {
                            let mut fixed = w.resource_mut::<FixedDock>();
                            add(&mut fixed.tree);
                            fixed.dirty = true;
                            return;
                        }
                        let floating = w
                            .get_resource::<DockWindows>()
                            .and_then(|ws| ws.0.iter().position(|s| s.area == area));
                        match floating {
                            Some(idx) => {
                                if let Some(mut ws) = w.get_resource_mut::<DockWindows>() {
                                    let st = &mut ws.0[idx];
                                    add(&mut st.tree);
                                    st.dirty = true;
                                }
                            }
                            None => {
                                if let Some(mut dock) = w.get_resource_mut::<Dock>() {
                                    add(&mut dock.tree);
                                }
                                if let Some(mut d) = w.get_resource_mut::<DockDirty>() {
                                    d.0 = true;
                                }
                            }
                        }
                    },
                ));
            }
        }
        let title = renzora::lang::t("menu.add_panel");
        crate::widgets::grid_overlay(&mut commands, &fonts, &title, entries);
    }
}

/// Wheel over a tab strip pans it horizontally. The strip clips overflowing
/// tabs but draws no scrollbar, so without this the tabs pushed past the leaf's
/// right edge would be unreachable. Vertical wheel maps to horizontal because a
/// 28px-tall strip has no vertical range; a trackpad's horizontal wheel passes
/// through. Bevy clamps `ScrollPosition` to the scrollable range during layout,
/// so we needn't clamp the far edge ourselves — only guard against going
/// negative past the first tab.
pub(crate) fn tab_strip_wheel(
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    mut strips: Query<
        (&bevy::ui::RelativeCursorPosition, &mut bevy::ui::ScrollPosition),
        With<TabScrollStrip>,
    >,
) {
    use bevy::input::mouse::MouseScrollUnit;
    let mut delta = 0.0;
    for ev in wheel.read() {
        // Line events are ~1 notch each; scale to about one tab's width. Pixel
        // events (precision trackpads) are already in logical px.
        let unit = if matches!(ev.unit, MouseScrollUnit::Line) { 24.0 } else { 1.0 };
        // Wheel down (negative y) reveals tabs to the right; horizontal wheel
        // adds directly.
        delta += (-ev.y + ev.x) * unit;
    }
    if delta == 0.0 {
        return;
    }
    for (rcp, mut scroll) in &mut strips {
        if rcp.cursor_over {
            scroll.x = (scroll.x + delta).max(0.0);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn populate_leaf(
    commands: &mut Commands,
    font: &bevy::text::FontSource,
    phosphor: &Handle<Font>,
    markers: &[String],
    area: Entity,
    floating: bool,
    movable: bool,
    parent: Option<ParentSplit>,
    leaf: Entity,
    tabs: &[String],
    active: usize,
    preserved: &mut HashMap<String, Entity>,
) {
    // Floating windows are chromeless single-panel hosts: the OS window's
    // own title bar plays the tab bar's role, so no tabs, no add-panel
    // button, no tab-bar resize filler.
    let tabbar = if floating {
        None
    } else {
        let tabbar = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Px(28.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(2.0),
                    // Snug: tabs sit flush to the leaf edges (no outer padding).
                    flex_shrink: 0.0,
                    // Hairline separator under the header.
                    border: UiRect::bottom(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(rgb(header_bg())),
                BorderColor::all(rgb(divider())),
                // Subtle drop shadow so the header reads as lifted above the content.
                BoxShadow::new(
                    Color::srgba(0.0, 0.0, 0.0, 0.30),
                    Val::Px(0.0),
                    Val::Px(2.0),
                    Val::Px(0.0),
                    Val::Px(4.0),
                ),
                bevy::ui::RelativeCursorPosition::default(),
                TabBarOf(leaf),
                DockPart::TabBar,
                crate::widgets::ThemeShaderSurface {
                    surface: crate::widgets::ThemeSurface::PanelHeader,
                },
                Name::new("tabbar"),
            ))
            .id();

        let mut bar_kids: Vec<Entity> = Vec::new();
        // Whole-leaf drag handle at the far left of the bar: grab it to move
        // the leaf's entire tab set as one unit (see [`LeafGrip`]).
        //
        // Omitted in an area that isn't movable — the editor's global bottom
        // panel is pinned to its slot, so offering a handle whose whole purpose
        // is relocating the leaf would advertise something that cannot happen.
        // Individual tabs still drag in and out; it is the *leaf* that is fixed.
        if movable {
            let leaf_grip_icon =
                icon_text(commands, phosphor, "dots-six-vertical", text_muted(), 12.0);
            let leaf_grip = commands
                .spawn((
                    Node {
                        height: Val::Percent(100.0),
                        width: Val::Px(14.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        flex_shrink: 0.0,
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    Interaction::default(),
                    crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Grab),
                    LeafGrip { leaf },
                    Name::new("leaf-grip"),
                ))
                .id();
            commands.entity(leaf_grip).add_child(leaf_grip_icon);
            bar_kids.push(leaf_grip);
        }
        // Tabs live inside a horizontally-clipping strip so a leaf with more
        // tabs than fit never pushes the "+"/collapse controls off the bar:
        // the overflowing tabs scroll instead (see [`tab_strip_wheel`]). The
        // strip shows no scrollbar — the wheel is the only affordance, hence
        // "invisible scroll".
        let mut tab_kids: Vec<Entity> = Vec::new();
        for (i, id) in tabs.iter().enumerate() {
            let is_active = i == active;
            let fg = if is_active { text_primary() } else { text_muted() };
            let (title, icon) = tab_meta(id);
            let tab = commands
                .spawn((
                    Node {
                        // Fill the full bar height so the active tab is snug to the
                        // top (content stays vertically centered via align_items).
                        height: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(5.0),
                        padding: UiRect::horizontal(Val::Px(9.0)),
                        position_type: PositionType::Relative,
                        flex_shrink: 0.0,
                        ..default()
                    },
                    BackgroundColor(if is_active {
                        rgb(tab_active())
                    } else {
                        Color::NONE
                    }),
                    Interaction::default(),
                    bevy::ui::RelativeCursorPosition::default(),
                    DockPart::Tab,
                    crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                    Name::new(format!("tab:{id}")),
                ))
                .id();
            // Undock handle overlaid on the tab's left padding — hidden (and
            // non-hit-testable) until the tab is hovered (`tab_grip_hover`);
            // press to tear the panel off into a floating window
            // (`tab_grip_interact`). Absolute so unhovered tabs keep their
            // exact size — no reserved gap.
            let grip_icon = icon_text(commands, phosphor, "dots-six-vertical", text_muted(), 10.0);
            let grip = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: Val::Px(10.0),
                        height: Val::Percent(100.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        display: Display::None,
                        ..default()
                    },
                    Interaction::default(),
                    crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Grab),
                    // Block so pressing the handle undocks instead of
                    // selecting/dragging the tab (mirrors the close ×).
                    bevy::ui::FocusPolicy::Block,
                    Name::new("tab-undock-grip"),
                ))
                .id();
            commands.entity(grip).add_child(grip_icon);
            let tab_icon = icon_text(commands, phosphor, icon, fg, 13.0);
            let tab_label = commands
                .spawn((
                    Text::new(title),
                    ui_font(font, 12.0),
                    TextColor(rgb(fg)),
                    bevy::text::TextLayout::no_wrap(),
                ))
                .id();
            let close = icon_text(commands, phosphor, "x", text_muted(), 11.0);
            commands.entity(close).insert((
                TabClose,
                bevy::ui::RelativeCursorPosition::default(),
                // Block so clicking the × closes the tab instead of selecting it.
                bevy::ui::FocusPolicy::Block,
            ));
            let marker = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(-2.0),
                        top: Val::Px(0.0),
                        height: Val::Percent(100.0),
                        width: Val::Px(2.0),
                        display: Display::None,
                        ..default()
                    },
                    BackgroundColor(rgb(accent())),
                    bevy::ui::FocusPolicy::Pass,
                    InsertMarker,
                    Name::new("insert-marker"),
                ))
                .id();
            commands.entity(tab).insert(DockTab {
                id: id.clone(),
                leaf,
                label: tab_label,
                icon: tab_icon,
                marker,
            });
            commands.entity(grip).insert(TabGrip {
                tab,
                id: id.clone(),
                leaf,
                area,
            });
            commands
                .entity(tab)
                .add_children(&[grip, tab_icon, tab_label, close, marker]);
            tab_kids.push(tab);
        }
        let tab_strip = commands
            .spawn((
                Node {
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(2.0),
                    // Allowed to shrink below the tabs' natural width and clip
                    // the overflow (Overflow::scroll_x), rather than letting the
                    // tabs spill past the "+" and collapse controls.
                    min_width: Val::Px(0.0),
                    flex_shrink: 1.0,
                    overflow: Overflow::scroll_x(),
                    ..default()
                },
                bevy::ui::ScrollPosition::default(),
                bevy::ui::RelativeCursorPosition::default(),
                TabScrollStrip { leaf },
                Name::new("tab-strip"),
            ))
            .id();
        commands.entity(tab_strip).add_children(&tab_kids);
        bar_kids.push(tab_strip);
        // "+" add-panel button, pinned to the bar *outside* the scroll strip so
        // it stays put and clickable no matter how many tabs the leaf holds.
        let add_icon = icon_text(commands, phosphor, "plus", text_muted(), 13.0);
        let add_btn = commands
            .spawn((
                Node {
                    height: Val::Percent(100.0),
                    width: Val::Px(22.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Interaction::default(),
                crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                AddPanelButton { leaf, area },
                Name::new("dock-add-panel"),
            ))
            .id();
        commands.entity(add_btn).add_child(add_icon);
        bar_kids.push(add_btn);
        let filler = commands
            .spawn((
                Node {
                    flex_grow: 1.0,
                    height: Val::Percent(100.0),
                    ..default()
                },
                Name::new("tabbar-filler"),
            ))
            .id();
        // In a pinned area the header's *empty* space is a drag surface for the
        // consumer (the editor's bottom panel resizes from it). Tagged on the
        // filler rather than the bar: `apply_cursor_icon` picks the first
        // hovered entity carrying a `HoverCursor` with no topmost resolution, so
        // a cursor on the bar competes with every tab inside it and wins often
        // enough that hovering a tab showed the resize cursor.
        if !movable {
            commands.entity(filler).insert((
                Interaction::default(),
                FixedAreaHeader,
                // It resizes the area, so it is a handle like any other: it
                // owns its press (nothing behind it may also see it), and it
                // raises `ResizeBusy` for the consumers that resolve a press
                // geometrically — the viewport among them, which would
                // otherwise read a drag from the header as a click in the
                // scene.
                crate::resize::ResizeHandle,
                crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::NsResize),
            ));
        }
        // Is this leaf a collapsible bottom region? Either the primary dock's
        // whole bottom region (root vertical split, second child — content-
        // agnostic), or a nested bottom strip: the bottom child of any
        // vertical split whose tabs include a bottom-strip marker panel. Both
        // get a collapse chevron at the bar's right end — the click mirror of
        // Ctrl+Space / the divider snap — so the toggle survives the strip
        // being docked under one column instead of full-width.
        let is_root_bottom = !floating
            && parent
                .as_ref()
                .is_some_and(|p| p.path.is_empty() && !p.horizontal && p.is_second);
        let strip = if !floating
            && !is_root_bottom
            && parent.as_ref().is_some_and(|p| !p.horizontal && p.is_second)
        {
            tabs.iter().find(|t| markers.contains(*t)).cloned()
        } else {
            None
        };
        if let Some(p) = parent.filter(|p| p.aligned()) {
            // Always vertical now — `aligned()` only accepts the split whose
            // divider lies along this bar, and that divider is horizontal.
            commands.entity(filler).insert((
                Interaction::default(),
                crate::resize::ResizeHandle,
                crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::NsResize),
                Divider {
                    container: p.container,
                    first_wrap: p.first_wrap,
                    horizontal: p.horizontal,
                    path: p.path,
                    area,
                    floating,
                    // The filler doubles as the parent split's resize handle,
                    // so a strip leaf's filler snap-closes like its divider.
                    strip: strip.clone(),
                },
            ));
        }
        bar_kids.push(filler);
        if is_root_bottom || strip.is_some() {
            let chev = icon_text(commands, phosphor, "caret-down", text_muted(), 13.0);
            let btn = commands
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
                    crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                    BottomCollapseBtn {
                        // Root region collapses content-agnostically; a nested
                        // strip is found again by one of its panels.
                        target: if is_root_bottom { None } else { strip },
                    },
                    Name::new("bottom-collapse"),
                ))
                .id();
            commands.entity(btn).add_child(chev);
            bar_kids.push(btn);
        }
        commands.entity(tabbar).add_children(&bar_kids);
        Some(tabbar)
    };

    // Content region. Reuse the active panel's preserved content entity (kept
    // across this rebuild) so reordering/moving tabs doesn't recreate the panel;
    // otherwise spawn an empty node for the consumer to fill.
    let active_id = tabs.get(active).cloned().unwrap_or_default();
    let content = preserved.remove(&active_id).unwrap_or_else(|| {
        commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    // Zero minimum so the active panel's content size never
                    // reserves space in the leaf's column layout.
                    min_width: Val::Px(0.0),
                    min_height: Val::Px(0.0),
                    flex_basis: Val::Px(0.0),
                    overflow: Overflow::clip(),
                    ..default()
                },
                Name::new("content"),
            ))
            .id()
    });

    let overlay = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                border: UiRect::all(Val::Px(2.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(rgb(accent())),
            bevy::ui::FocusPolicy::Pass,
            DropOverlay,
            Name::new("drop-overlay"),
        ))
        .id();

    commands.entity(leaf).insert(DockLeaf {
        tabs: tabs.to_vec(),
        content,
        active: active_id,
        area,
        overlay,
    });
    match tabbar {
        Some(tabbar) => {
            commands
                .entity(leaf)
                .add_children(&[tabbar, content, overlay]);
        }
        None => {
            commands.entity(leaf).add_children(&[content, overlay]);
        }
    }
}
