//! The bottom panel's named tab-sets: the dropdown in the panel's top-right
//! corner that switches between them, adds one, drops one, renames one, and
//! reorders them by drag.
//!
//! A "set" is a whole dock tree the panel can be showing. The *live* one is in
//! [`renzora_ember::dock::FixedDock`]; the copies here are only refreshed when
//! the user switches away from one, which is why [`activate_panel_set`] parks
//! the live tree before handing ember the new one.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora_ember::font::{glyph, ui_font, EmberFonts};
use renzora_ember::theme::{accent, divider, play_green, rgb, text_muted, text_primary};

use crate::bottom_dock::{BottomDock, BottomDockBtn, BOTTOM_DOCK_Z};
use crate::dock::{self, DockTree};

/// The bottom panel's named tab-sets, and which one is live.
///
/// Mirrors how [`crate::ShellLayouts`] relates to [`renzora_ember::dock::Dock`]:
/// the *live* tree is the one in [`renzora_ember::dock::FixedDock`], and
/// `sets[active].1` is only refreshed when the user switches away from it (or
/// when the layout is saved). Reading the active slot's tree straight out of
/// here therefore gives you the panel as it was when it last went out of view,
/// not as it is now.
#[derive(Resource)]
pub(crate) struct BottomPanelSets {
    pub(crate) sets: Vec<(String, DockTree)>,
    pub(crate) active: usize,
}

/// The name a bottom panel gets when it has never had a second set — the case
/// for every layout written before sets existed.
pub(crate) fn default_panel_set_name() -> String {
    renzora::lang::t_or("shell.bottom_dock.set_default", "Default")
}

/// `Default 2`, `Default 3`, … — the first numbered name the panel isn't already
/// using, so removing set 2 and adding another gives back `Default 2` rather
/// than climbing forever.
fn next_panel_set_name(taken: &[(String, DockTree)]) -> String {
    let base = default_panel_set_name();
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|name| !taken.iter().any(|(n, _)| n == name))
        .unwrap_or(base)
}

/// Make `index` the live set: park the tree the panel is showing back in the
/// slot it came from, then hand ember the new one.
///
/// The park is what makes switching lossless — the live tree has been edited in
/// `FixedDock` (tabs dragged, panels closed) and the copy in `sets` is stale by
/// exactly those edits.
fn activate_panel_set(
    sets: &mut BottomPanelSets,
    fixed: &mut renzora_ember::dock::FixedDock,
    index: usize,
) {
    if index >= sets.sets.len() {
        return;
    }
    let live = fixed.tree.clone();
    if let Some(slot) = sets.sets.get_mut(sets.active) {
        slot.1 = live;
    }
    sets.active = index;
    fixed.tree = sets.sets[index].1.clone();
    // The area node exists by the time any of this is reachable (the menu that
    // calls it lives in the same chrome), so a rebuild is always wanted.
    fixed.dirty = true;
}

/// The panel-set dropdown's trigger — a name + caret in the panel's top-right
/// corner, left of the Overlay/Layout button.
#[derive(Component)]
pub(crate) struct BottomSetTrigger;
/// The trigger's label, kept on the active set's name.
#[derive(Component)]
pub(crate) struct BottomSetLabel;
/// The dropdown's panel. Its rows are rebuilt from [`BottomPanelSets`] rather
/// than spawned once, because the set list changes while the chrome stands.
#[derive(Component)]
pub(crate) struct BottomSetMenu;
/// The "New Panel Set" row.
#[derive(Component)]
pub(crate) struct BottomSetNew;
/// The "Remove This Set" row. Present only while more than one set exists —
/// removing the last one would leave the panel with nowhere to put a tab.
#[derive(Component)]
pub(crate) struct BottomSetRemove;
/// The pencil at a set row's right edge, carrying the set it renames.
#[derive(Component)]
pub(crate) struct BottomSetRenameBtn(usize);
/// The set currently being inline-renamed (`None` = none), read by
/// [`sync_bottom_set_menu`] so that row renders a text field in place of its
/// name. Mirrors [`crate::RibbonRename`], which does the same for workspaces.
#[derive(Resource, Default)]
pub(crate) struct BottomSetRename(Option<usize>);
/// Marks the inline rename field, carrying the set index it renames.
#[derive(Component)]
pub(crate) struct BottomSetRenameInput(usize);

/// A draggable set row: which set it is, and the insertion bar drawn at its top
/// edge while a reorder drag points at that slot. The vertical twin of
/// [`crate::DocTabItem`].
#[derive(Component)]
pub(crate) struct BottomSetItem {
    index: usize,
    marker: Entity,
}

/// The in-flight reorder of the panel sets, if any.
#[derive(Resource, Default)]
pub(crate) struct BottomSetDrag(Option<BottomSetDragState>);

struct BottomSetDragState {
    /// The set being carried, by index at the time of the press.
    index: usize,
    start_cursor: Vec2,
    /// Cleared until the cursor has moved far enough to call this a drag rather
    /// than a click — which is what lets one gesture mean both.
    active: bool,
    /// Insertion slot in the *pre-removal* list, as [`reorder_panel_sets`] takes
    /// it.
    target: usize,
}

/// Move set `from` to insertion slot `to`, keeping `active` pointed at the same
/// set it was before.
///
/// `to` is a slot in the list *as it stands*, so both the set's own slot and the
/// one just past it mean "don't move" — the same convention
/// `DocumentTabState::reorder` uses, and the reason the caller can hand over a
/// marker position without adjusting for the removal itself.
///
/// Only the names and trees move. The set the panel is *showing* lives in
/// `FixedDock`, not in `sets[active].1`, so a reorder never has to touch the
/// live tree — it only has to keep `active` on the right slot.
fn reorder_panel_sets(sets: &mut BottomPanelSets, from: usize, to: usize) {
    if from >= sets.sets.len() || to > sets.sets.len() || to == from || to == from + 1 {
        return;
    }
    let set = sets.sets.remove(from);
    let at = if to > from { to - 1 } else { to };
    let at = at.min(sets.sets.len());
    sets.sets.insert(at, set);
    if sets.active == from {
        sets.active = at;
    } else if from < sets.active && sets.active <= at {
        sets.active -= 1;
    } else if at <= sets.active && sets.active < from {
        sets.active += 1;
    }
}

/// Build the panel-set dropdown: trigger + (empty) menu panel.
///
/// The menu is a child of the trigger so it anchors to it, and which way it
/// opens is decided per-open by ember's `popup_position`: **down into the panel
/// when the panel is tall enough to hold it, up over the workspace when it
/// isn't.** The trigger rides the top edge of a panel whose height is the
/// user's to choose, so neither direction is the safe one to hard-code — a
/// short panel has no room below (the dock wrapper clips at the status bar),
/// and a panel dragged up to the top bar has none above.
///
/// Authored downward, which is what it gets on the first frame of an open,
/// before the menu has a measured height to flip on.
pub(crate) fn build_bottom_set_menu(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(100.0),
                right: Val::Px(0.0),
                margin: UiRect::top(Val::Px(4.0)),
                min_width: Val::Px(180.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                padding: UiRect::all(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::popup_bg())),
            BorderColor::all(rgb(divider())),
            GlobalZIndex(BOTTOM_DOCK_Z + 3),
            // Not decoration: without both of these the menu is invisible to
            // `correct_pointer_state`, so a click on one of its rows *also*
            // lands in whatever panel is behind it.
            renzora_ember::widgets::OverlaySurface,
            RelativeCursorPosition::default(),
            // Same reason as the trigger: the panel's own background hangs over
            // the dock header's resize filler.
            bevy::ui::FocusPolicy::Block,
            BottomSetMenu,
            Name::new("bottom-set-menu"),
        ))
        .id();

    let label = commands
        .spawn((
            Text::new(default_panel_set_name()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::no_wrap(),
            Node {
                min_width: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
            BottomSetLabel,
        ))
        .id();
    let caret = glyph(commands, "caret-down", text_muted(), 10.0);
    commands.entity(caret).insert(bevy::ui::FocusPolicy::Pass);

    let trigger = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                // Clear of the mode button at 30 and the collapse button at 6.
                right: Val::Px(54.0),
                bottom: Val::Px(dock::BOTTOM_DOCK_HEIGHT - 26.0),
                height: Val::Px(22.0),
                max_width: Val::Px(160.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::horizontal(Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                // No `Overflow::clip()` here — the menu is a child of this node,
                // and a clipping parent clips absolutely-positioned descendants
                // too. The label below carries the clip instead.
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::NONE),
            GlobalZIndex(BOTTOM_DOCK_Z + 2),
            Interaction::default(),
            // Not optional. `Node`'s required `FocusPolicy` is `Pass` in Bevy
            // 0.19, so hover falls *through* this button to the dock header's
            // filler underneath — which is the panel's resize surface and
            // carries an ns-resize `HoverCursor`. `apply_cursor_icon` takes the
            // first hovered entity with a cursor and does no topmost
            // resolution, so the filler won and the dropdown showed a resize
            // cursor. Blocking keeps the hover here, where it belongs.
            bevy::ui::FocusPolicy::Block,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            renzora_ember::widgets::Popup::new(panel),
            // No tooltip: the control already reads as what it is (a named set
            // plus a caret), and a bubble over the panel's own top edge covers
            // the tabs it's about to switch.
            BottomSetTrigger,
            // Shown/hidden and vertically placed with the panel's other corner
            // controls by `sync_bottom_dock_node`.
            BottomDockBtn,
            Name::new("bottom-set-trigger"),
        ))
        .id();
    renzora_ember::reactive::tracked::bind_bg(commands, trigger, move |w| {
        match w.get::<Interaction>(trigger) {
            Some(Interaction::Hovered) | Some(Interaction::Pressed) => {
                Color::srgba(1.0, 1.0, 1.0, 0.09)
            }
            _ => Color::NONE,
        }
    });
    commands
        .entity(trigger)
        .add_children(&[label, caret, panel]);
    trigger
}

/// One row of the panel-set menu: a leading glyph and a label.
fn bottom_set_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    icon_color: (u8, u8, u8),
    label: String,
) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            // Block, like the trigger: a row sits over the panel's header
            // filler, which is a resize surface (see the trigger's comment).
            bevy::ui::FocusPolicy::Block,
            // The reorder drag hit-tests in the cursor's own space rather than
            // against node centres, which drift under UI scaling — the lesson
            // `ribbon_interact` and the document tabs both learned the hard way.
            RelativeCursorPosition::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("bottom-set-row"),
        ))
        .id();
    renzora_ember::reactive::tracked::bind_bg(commands, row, move |w| {
        match w.get::<Interaction>(row) {
            Some(Interaction::Hovered) | Some(Interaction::Pressed) => {
                rgb(renzora_ember::theme::hover_bg())
            }
            _ => Color::NONE,
        }
    });
    let ic = glyph(commands, icon, icon_color, 12.0);
    commands.entity(ic).insert(bevy::ui::FocusPolicy::Pass);
    let text = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::no_wrap(),
            // Takes the slack, so a trailing button (the rename pencil) sits at
            // the row's right edge rather than against the name.
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    commands.entity(row).add_children(&[ic, text]);
    row
}

/// The inline rename field for panel set `index`, styled like the ribbon's.
fn build_bottom_set_rename_field(
    commands: &mut Commands,
    font: &bevy::text::FontSource,
    index: usize,
    name: &str,
) -> Entity {
    let input = renzora_ember::widgets::text_input(commands, font, "Name", name);
    commands.entity(input).insert((
        BottomSetRenameInput(index),
        Node {
            width: Val::Percent(100.0),
            height: Val::Px(22.0),
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(4.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..default()
        },
    ));
    input
}

/// What [`sync_bottom_set_menu`] compares against to decide the menu it built
/// is still the right one: menu entity, the set names, the live set, and the
/// set being renamed.
type BottomSetMenuKey = (Entity, Vec<String>, usize, Option<usize>);

/// Fill the panel-set menu from [`BottomPanelSets`], and keep the trigger's
/// label on the active set's name.
///
/// Rebuilt on change rather than reconciled: the list is a handful of rows and
/// only moves when the user opens the menu and acts on it, so the churn the
/// reactive lists were built to avoid doesn't arise. Keyed on the menu entity
/// as well as the contents, because a theme or language change respawns the
/// chrome and hands us a fresh, childless panel.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_bottom_set_menu(
    sets: Res<BottomPanelSets>,
    rename: Res<BottomSetRename>,
    fonts: Option<Res<EmberFonts>>,
    theme: Option<Res<renzora_theme::ThemeManager>>,
    menus: Query<Entity, With<BottomSetMenu>>,
    mut labels: Query<&mut Text, With<BottomSetLabel>>,
    mut commands: Commands,
    mut built: Local<Option<BottomSetMenuKey>>,
) {
    let (Some(fonts), Ok(menu)) = (fonts, menus.single()) else {
        return;
    };
    let names: Vec<String> = sets.sets.iter().map(|(n, _)| n.clone()).collect();
    // `rename.0` is part of the key: entering and leaving rename mode swaps one
    // row between a label and a text field, and nothing else about the set list
    // changes when it does.
    if built.as_ref() == Some(&(menu, names.clone(), sets.active, rename.0)) {
        return;
    }
    *built = Some((menu, names.clone(), sets.active, rename.0));

    for mut text in &mut labels {
        let want = names.get(sets.active).cloned().unwrap_or_default();
        if text.0 != want {
            text.0 = want;
        }
    }

    let green = theme
        .map(|t| {
            let [r, g, b, _] = t.active_theme.semantic.success.to_array();
            (r, g, b)
        })
        .unwrap_or_else(play_green);

    commands.entity(menu).despawn_related::<Children>();
    let mut rows = Vec::new();
    for (i, name) in names.iter().enumerate() {
        // The row being renamed is the field, not a label — it can't also be a
        // click target, or typing in it would keep re-activating the set.
        if rename.0 == Some(i) {
            let holder = commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                        ..default()
                    },
                    Name::new("bottom-set-rename-row"),
                ))
                .id();
            let field = build_bottom_set_rename_field(&mut commands, &fonts.ui, i, name);
            commands.entity(holder).add_child(field);
            rows.push(holder);
            continue;
        }
        // Check on the live set, the set's own glyph on the others — the same
        // check-or-icon slot the theme and play-target menus use.
        let (icon, color) = if i == sets.active {
            ("check", green)
        } else {
            ("squares-four", text_muted())
        };
        let row = bottom_set_row(&mut commands, &fonts, icon, color, name.clone());
        // Insertion bar for a reorder drag: a hairline of accent across the
        // row's top edge, hidden until the drag points at this slot. Absolute,
        // so it costs the row no height and can't shift the menu as it moves.
        let marker = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(-1.0),
                    width: Val::Percent(100.0),
                    height: Val::Px(2.0),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(rgb(accent())),
                bevy::ui::FocusPolicy::Pass,
                Name::new("bottom-set-insert-marker"),
            ))
            .id();
        commands.entity(row).add_child(marker);
        // `BottomSetItem` is both the click target and the drag handle — the
        // row's index is the one thing either needs, so there's no separate
        // marker component for "this row picks a set".
        commands.entity(row).insert(BottomSetItem { index: i, marker });
        // Rename pencil at the row's right edge. `Block`, or the press also
        // reaches the row and switches to that set on the way into the field —
        // `Node`'s required `FocusPolicy` is `Pass` in Bevy 0.19.
        let pencil = commands
            .spawn((
                Node {
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    width: Val::Px(16.0),
                    height: Val::Px(16.0),
                    flex_shrink: 0.0,
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Interaction::default(),
                bevy::ui::FocusPolicy::Block,
                BottomSetRenameBtn(i),
                renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                Name::new("bottom-set-rename"),
            ))
            .id();
        let pencil_icon = glyph(&mut commands, "pencil-simple", text_muted(), 11.0);
        commands.entity(pencil_icon).insert(bevy::ui::FocusPolicy::Pass);
        commands.entity(pencil).add_child(pencil_icon);
        commands.entity(row).add_child(pencil);
        rows.push(row);
    }
    let sep = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(1.0),
                margin: UiRect::vertical(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(divider())),
        ))
        .id();
    rows.push(sep);
    let new_row = bottom_set_row(
        &mut commands,
        &fonts,
        "plus",
        text_muted(),
        renzora::lang::t_or("shell.bottom_dock.set_new", "New Panel Set"),
    );
    commands.entity(new_row).insert(BottomSetNew);
    rows.push(new_row);
    if names.len() > 1 {
        let remove_row = bottom_set_row(
            &mut commands,
            &fonts,
            "trash",
            text_muted(),
            renzora::lang::t_or("shell.bottom_dock.set_remove", "Remove This Set"),
        );
        commands.entity(remove_row).insert(BottomSetRemove);
        rows.push(remove_row);
    }
    commands.entity(menu).add_children(&rows);
}

/// Drive the panel-set menu: pick a set, add one, drop the live one, or start
/// renaming one.
///
/// Every branch but the rename closes the menu, so the result is visible
/// immediately rather than behind a popup the user still has to dismiss —
/// rename is the exception because the field it opens *is* in the menu.
#[allow(clippy::too_many_arguments)]
pub(crate) fn bottom_set_menu_click(
    new_rows: Query<&Interaction, (With<BottomSetNew>, Changed<Interaction>)>,
    remove_rows: Query<&Interaction, (With<BottomSetRemove>, Changed<Interaction>)>,
    pencils: Query<(&Interaction, &BottomSetRenameBtn), Changed<Interaction>>,
    mut sets: ResMut<BottomPanelSets>,
    mut rename: ResMut<BottomSetRename>,
    mut fixed: ResMut<renzora_ember::dock::FixedDock>,
    mut bottom: ResMut<BottomDock>,
    triggers: Query<Entity, With<BottomSetTrigger>>,
    mut commands: Commands,
) {
    for (interaction, pencil) in &pencils {
        if *interaction == Interaction::Pressed {
            rename.0 = Some(pencil.0);
        }
    }
    let mut acted = false;
    // Picking a set isn't here: it fires on *release*, in [`bottom_set_drag`],
    // because the same press can be the start of a reorder. Acting on the press
    // would switch sets and close the menu out from under the drag.
    for interaction in &new_rows {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let name = next_panel_set_name(&sets.sets);
        // Empty on purpose: ember renders an "Add Panel" button for an empty
        // tree, which is exactly the right first step in a brand new set.
        sets.sets.push((name, DockTree::Empty));
        let last = sets.sets.len() - 1;
        activate_panel_set(&mut sets, &mut fixed, last);
        // A new set is a request to work in the panel, so make sure it's up.
        bottom.open = true;
        acted = true;
    }
    for interaction in &remove_rows {
        // Guarded here as well as in the builder: the row is only spawned while
        // there's more than one set, but the menu it lives in can outlive that
        // by a frame.
        if *interaction != Interaction::Pressed || sets.sets.len() < 2 {
            continue;
        }
        let gone = sets.active;
        sets.sets.remove(gone);
        // Land on the neighbour that kept its position, so removing the last
        // set doesn't jump to the front.
        let next = gone.min(sets.sets.len() - 1);
        // Not `activate_panel_set`: parking the live tree would write it into
        // whichever set slid into this index.
        sets.active = next;
        fixed.tree = sets.sets[next].1.clone();
        fixed.dirty = true;
        acted = true;
    }
    if acted {
        // Whatever the menu was in the middle of no longer refers to the set
        // list it was opened against.
        rename.0 = None;
        for trigger in &triggers {
            renzora_ember::widgets::close_popup(&mut commands, trigger);
        }
    }
}

/// Press-latch reorder for the panel-set rows, plus the plain click that picks
/// a set: drag a row past a small threshold to move it in [`BottomPanelSets`],
/// or release without moving to switch to it. The vertical twin of
/// [`crate::doc_tab_drag`].
///
/// Both halves of the gesture live here for the reason that split is usually
/// made: only the code that watched the press *and* the motion knows which of
/// the two it was. Switching sets used to happen on the press, in
/// [`bottom_set_menu_click`], which closed the menu — so a drag ended before it
/// started.
///
/// The reorder is applied once, on release, rather than live as the cursor
/// crosses each neighbour: the insertion bar is what shows where the row will
/// land, and a live swap would rebuild the whole menu (and so respawn the row
/// under the cursor) on every crossing.
#[allow(clippy::too_many_arguments)]
pub(crate) fn bottom_set_drag(
    mut drag: ResMut<BottomSetDrag>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    rename: Res<BottomSetRename>,
    pressed: Query<(&BottomSetItem, &Interaction)>,
    items: Query<(&BottomSetItem, &RelativeCursorPosition)>,
    mut nodes: Query<&mut Node>,
    mut sets: ResMut<BottomPanelSets>,
    mut fixed: ResMut<renzora_ember::dock::FixedDock>,
    triggers: Query<Entity, With<BottomSetTrigger>>,
    mut commands: Commands,
) {
    let hide_markers = |items: &Query<(&BottomSetItem, &RelativeCursorPosition)>,
                        nodes: &mut Query<&mut Node>| {
        for (it, _) in items {
            if let Ok(mut n) = nodes.get_mut(it.marker) {
                if n.display != Display::None {
                    n.display = Display::None;
                }
            }
        }
    };

    // A row being renamed is a text field, not a handle.
    if rename.0.is_some() {
        drag.0 = None;
        hide_markers(&items, &mut nodes);
        return;
    }
    let cursor = windows.iter().next().and_then(|w| w.cursor_position());

    if drag.0.is_none() && mouse.just_pressed(MouseButton::Left) {
        if let Some(cur) = cursor {
            for (item, interaction) in &pressed {
                if *interaction == Interaction::Pressed {
                    drag.0 = Some(BottomSetDragState {
                        index: item.index,
                        start_cursor: cur,
                        active: false,
                        target: item.index,
                    });
                    break;
                }
            }
        }
    }

    if let (Some(st), Some(cur)) = (drag.0.as_mut(), cursor) {
        if (cur - st.start_cursor).length() > 5.0 {
            st.active = true;
        }
    }

    // Which slot the cursor is pointing at, and the marker that says so: the
    // top half of a row inserts above it, the bottom half below.
    match drag.0.as_mut() {
        Some(st) if st.active => {
            let mut shown: Option<(Entity, bool)> = None;
            for (it, rcp) in &items {
                if !rcp.cursor_over {
                    continue;
                }
                let before = rcp.normalized.is_none_or(|n| n.y < 0.0);
                st.target = if before { it.index } else { it.index + 1 };
                shown = Some((it.marker, !before));
                break;
            }
            hide_markers(&items, &mut nodes);
            if let Some((marker, below)) = shown {
                if let Ok(mut n) = nodes.get_mut(marker) {
                    n.display = Display::Flex;
                    if below {
                        n.top = Val::Auto;
                        n.bottom = Val::Px(-1.0);
                    } else {
                        n.top = Val::Px(-1.0);
                        n.bottom = Val::Auto;
                    }
                }
            }
        }
        _ => hide_markers(&items, &mut nodes),
    }

    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    hide_markers(&items, &mut nodes);
    let Some(st) = drag.0.take() else { return };
    if !st.active {
        // A click: switch to that set and dismiss, which is what this row did
        // when the press handled it.
        if st.index != sets.active {
            activate_panel_set(&mut sets, &mut fixed, st.index);
        }
        for trigger in &triggers {
            renzora_ember::widgets::close_popup(&mut commands, trigger);
        }
        return;
    }
    // A reorder leaves the menu open: you are arranging a list, and having it
    // shut on every move would make ordering three sets three round trips.
    let to = st.target.min(sets.sets.len());
    reorder_panel_sets(&mut sets, st.index, to);
}

/// Focus the panel-set rename field the frame it spawns, so the pencil puts the
/// caret in the name rather than only drawing a box.
pub(crate) fn bottom_set_focus_rename(
    mut fields: Query<&mut renzora_ember::widgets::EmberTextInput, Added<BottomSetRenameInput>>,
) {
    for mut input in &mut fields {
        input.focused = true;
    }
}

/// Commit (Enter / blur) or cancel (Escape) a panel-set rename. The twin of
/// [`crate::ribbon_rename_commit`], which does this for workspaces.
pub(crate) fn bottom_set_rename_commit(
    mut rename: ResMut<BottomSetRename>,
    keys: Res<ButtonInput<KeyCode>>,
    fields: Query<(
        &renzora_ember::widgets::EmberTextInput,
        &BottomSetRenameInput,
    )>,
    mut sets: ResMut<BottomPanelSets>,
    mut had_focus: Local<bool>,
) {
    let Some(index) = rename.0 else {
        *had_focus = false;
        return;
    };
    if keys.just_pressed(KeyCode::Escape) {
        rename.0 = None;
        *had_focus = false;
        return;
    }
    let Some((input, _)) = fields.iter().find(|(_, r)| r.0 == index) else {
        return;
    };
    // The blur test needs a frame where the field *was* focused: it spawns
    // unfocused and is focused by `bottom_set_focus_rename`, so without this a
    // rename would commit-and-close on its very first frame.
    if input.focused {
        *had_focus = true;
    }
    let enter = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter);
    let blurred = *had_focus && !input.focused;
    if !enter && !blurred {
        return;
    }
    let new: String = input.value.replace('\n', "").trim().to_string();
    rename.0 = None;
    *had_focus = false;
    // An empty name would leave a row you can't read or aim at, so a cleared
    // field cancels instead.
    if new.is_empty() {
        return;
    }
    if let Some(slot) = sets.sets.get_mut(index) {
        slot.0 = new;
    }
}
