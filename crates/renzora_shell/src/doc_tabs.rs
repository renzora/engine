//! The document tabs: the strip under the top bar, the compact dropdown that can
//! stand in for it, and everything that opens, reorders, renames and closes a
//! document.
//!
//! **Where the strip lives.** A row of the shell's own column, so it is on
//! screen in every workspace. It spent a while inside the primary viewport
//! panel, which meant it existed only where a `viewport` panel did — and an open
//! material routes the editor *to* a workspace that has none, so the bar holding
//! that material's tab vanished the moment you clicked it. See [`build_doc_tabs`].
//!
//! Scenes and assets share the one list, in both presentations.

use bevy::prelude::*;
use bevy::ui::{BackgroundGradient, ColorStop, LinearGradient, RelativeCursorPosition};

use renzora_ember::dock::{Dock, DockDirty};
use renzora_ember::font::{glyph, icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::{
    accent, border, divider, header_bg, mix, panel_bg, rgb, text_muted, text_primary,
};
use renzora_ember::widgets::{text_input, EmberTextInput, Popup};

use crate::ribbon::{apply_workspace, select_workspace};
use crate::save_prompts::{CloseTabPromptRoot, TabCloseRequest};
use crate::ShellLayouts;

/// Longest document-tab label kept intact, in characters.
const DOC_TAB_CHARS: usize = 18;

#[derive(Component)]
pub(crate) struct DocAddBtn;
#[derive(Component)]
pub(crate) struct DocTabClick(u64);
#[derive(Component)]
pub(crate) struct DocTabClose(u64);

/// A document tab in the strip, carrying its id and the insertion marker shown
/// at its edge during a reorder drag (mirrors `RibbonItem`).
#[derive(Component)]
pub(crate) struct DocTabItem {
    id: u64,
    marker: Entity,
}

/// In-progress document-tab reorder — the same press-latch shape as
/// `RibbonDrag`, so a plain click still activates the tab.
#[derive(Resource, Default)]
pub(crate) struct DocTabDrag(Option<DocTabDragState>);

struct DocTabDragState {
    /// The tab being carried, by **id**: a reorder shifts every index around it,
    /// and a tab can be closed from elsewhere mid-drag.
    id: u64,
    start_cursor: Vec2,
    /// Flips once the cursor has moved past a small threshold, so a click that
    /// happens to wobble a pixel doesn't reorder anything. A drag started from
    /// the overflow menu is born active — there was no click to tell it apart
    /// from in the first place.
    active: bool,
    /// Insertion slot in `DocumentTabState::tabs` (`0..=len`) under the live
    /// cursor; applied on release.
    target: usize,
}

/// The document tab currently being inline-renamed (`None` = none). Read by
/// [`doc_tab_snapshot`] so that tab renders an edit field in place of itself.
#[derive(Resource, Default)]
pub(crate) struct DocTabRename(Option<u64>);

/// Marks the inline rename text field, carrying the tab id it renames.
#[derive(Component)]
pub(crate) struct DocTabRenameInput(u64);

/// The trigger button of the document-tab dropdown, so its popup can be closed
/// from a row inside it.
#[derive(Component)]
pub(crate) struct DocTabMenuTrigger;

/// A row in that dropdown. The row also carries [`DocTabClick`] (or
/// [`DocAddBtn`]), which the strip's own systems handle — this marker exists
/// only to close the menu behind the click.
#[derive(Component)]
pub(crate) struct DocTabMenuRow;

/// Most-recently-active document tab ids, oldest first, so a workspace switch
/// can return to the document you were last in *there* — see
/// [`sync_active_doc_to_workspace`].
///
/// Kept here rather than in [`renzora_ui::DocumentTabState`] because it's a
/// property of this session's navigation, not of the document set: nothing
/// persists it, and every activation route (a tab click, an asset browser
/// double-click, the inspector's edit button) is observed the same way — by
/// [`sync_workspace_to_active_doc`] noticing the active tab changed.
#[derive(Resource, Default)]
pub(crate) struct DocTabMru(Vec<u64>);

/// The document tab bar: every open document (`DocumentTabState`, shared with
/// the egui editor) rendered reactively, plus an add-document button.
///
/// **Where it lives.** A row of the shell's own column, directly under the top
/// bar and above the dock — so it is on screen in every workspace. It spent a
/// while inside the primary viewport panel, mounted through
/// [`renzora_ember::toolbar::register_viewport_top_strip`], on the argument that
/// tabs belong with the thing they switch between. What that actually bought
/// was a tab bar that existed only where a `viewport` panel did: five of the
/// nine default workspaces (Blueprints, Materials, Particles, Animation, Hub)
/// have none, and an open material routes the editor *to* one of those — so the
/// bar holding that material's tab vanished the moment you clicked it, leaving
/// the document unreachable and uncloseable from its own editor.
///
/// Scenes and assets share the one bar. They are one list in the model, they
/// are one Ctrl+Tab's worth of "things I have open" to the user, and splitting
/// them into two bars only asks which half a given file is in.
///
/// The primary viewport's Maximize button still rides along at the right-hand
/// end, as it did when this bar lived in the panel: it was the full-width row
/// there and it is the full-width row here.
///
/// The bar spans the window, so nothing folds until the tabs genuinely fill it.
/// Inside it the tab list hugs its content, so the `+` button sits directly
/// after the last tab and travels right as tabs are added; once they fill the
/// bar the surplus folds into the caret menu and `+` stops moving.
pub(crate) fn build_doc_tabs(commands: &mut Commands) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(30.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                flex_shrink: 0.0,
                min_width: Val::Px(0.0),
                padding: UiRect::horizontal(Val::Px(6.0)),
                overflow: Overflow::clip(),
                // Closed off underneath, against the dock. Dark rather than the
                // toolbar's own separator colour: this edge is where the window
                // chrome stops and the workspace begins, which is a harder
                // boundary than the ones *inside* the chrome.
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            // A half-step off `panel` toward the theme's contrasting surface —
            // just enough to read as its own band rather than more toolbar.
            // Mixing toward a second *theme* colour rather than toward white
            // keeps it differentiated on light themes too, where "lighter" would
            // walk it into the background instead of away from it.
            //
            // Graded rather than flat, and the direction matters: lit at the top
            // where it meets the top bar, settling back toward `panel` at the
            // bottom where the dark rule closes it off against the dock. The
            // band therefore reads as catching light from above rather than as a
            // slab someone dropped between two darker things.
            BackgroundColor(mix(panel_bg(), header_bg(), 0.55)),
            BackgroundGradient::from(LinearGradient::to_bottom(vec![
                ColorStop::auto(mix(panel_bg(), header_bg(), 0.85)),
                ColorStop::auto(mix(panel_bg(), header_bg(), 0.20)),
            ])),
            BorderColor::all(rgb(divider())),
            // No `OverlaySurface` here any more: it needed one while it sat over
            // the viewport's picking area, where a click landing between two tabs
            // would otherwise fall through and deselect whatever was in the
            // scene. As a row of the shell's column it has nothing behind it.
            Name::new("doc-tabs"),
        ))
        .id();

    // Reactive tab strip from the shared DocumentTabState. The budget is the
    // bar's own measured width, less room for the caret and `+` that share it.
    // Gap 0: the tabs butt against each other, so the run of inactive ones reads
    // as a single band with the active tab cut out of it.
    let (strip, tabs) = renzora_ember::widgets::overflow_strip_gap(
        commands,
        renzora_ember::widgets::OverflowBudget::Fill { measure: bar, reserve: 66.0 },
        0.0,
        "doc-tab",
    );
    renzora_ember::reactive::tracked::keyed_list(commands, tabs, doc_tab_snapshot);

    // "+" — add a new document (scene) tab.
    let plus = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(7.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            DocAddBtn,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("doc-add"),
        ))
        .id();
    let plus_icon = glyph(commands, "plus", text_muted(), 13.0);
    commands.entity(plus).add_child(plus_icon);

    // Nothing after the `+`: the viewport's Maximize used to be parked at the far
    // end of this bar, and went back to the viewport's own toolbar when the bar
    // stopped being part of that panel.
    commands.entity(bar).add_children(&[strip, plus]);
    // Hidden — and costing no height, since it's a row of the shell's column —
    // while Settings has the tabs set to Dropdown.
    renzora_ember::reactive::tracked::bind_display(commands, bar, |w: &Rx| !doc_tabs_dropdown(w));
    bar
}

/// Whether the document tabs are set to the top-bar dropdown rather than the
/// strip. Read through the `Rx` so both presentations' `bind_display`s react to
/// the setting changing; false (the strip) when there's no `EditorSettings` yet.
fn doc_tabs_dropdown(w: &Rx) -> bool {
    w.get_resource::<renzora_editor_framework::EditorSettings>()
        .is_some_and(|s| s.doc_tabs_dropdown)
}

/// The document tabs as a dropdown in the top bar, beside Play, plus the
/// primary viewport's Maximize — the other presentation of [`build_doc_tabs`],
/// chosen in Settings → Interface → Document tabs.
///
/// The trade it offers is a row of the window: the strip is easier to move
/// between (every document is one click, and you can see what's open without
/// asking), and this is smaller. It shows the active document — icon, name, and
/// the `*` when it has unsaved edits — and opens onto all of them.
///
/// Maximize comes along because it lives at the end of the strip, and the strip
/// is what's hidden. Both presentations build their own, tagged
/// `MaximizeSlot(0)`; the driver systems find them by component and only one is
/// ever on screen, so a hidden duplicate costs nothing.
pub(crate) fn build_doc_tab_menu_group(
    commands: &mut Commands,
    fonts: &EmberFonts,
    font: &bevy::text::FontSource,
) -> Entity {
    // One row per open document, reactive off the same state the strip renders.
    let list = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    renzora_ember::reactive::tracked::keyed_list(commands, list, doc_tab_menu_snapshot);

    // Ember's own popup surface rather than a hand-rolled node: only this one is
    // known to `correct_pointer_state`, and without that a click on a row lands
    // in the viewport behind the menu as well as on the row — the menu hangs
    // over the scene, so that would select whatever was under it.
    let panel = renzora_ember::widgets::popup_panel_aligned(
        commands,
        &[list],
        renzora_ember::widgets::PopupAlign::Left,
    );
    commands.entity(panel).insert(Name::new("doc-tab-menu"));
    // Tightened rather than rebuilt: the surface's own layout (absolute, edge
    // alignment, the `OverlaySurface` that makes clicks stop here) stays
    // ember's, and only the metrics change. A toolbar-sized control shouldn't
    // open a panel with the padding of a settings popover.
    commands
        .entity(panel)
        .entry::<Node>()
        .and_modify(|mut n| {
            n.min_width = Val::Px(170.0);
            n.padding = UiRect::all(Val::Px(4.0));
            n.row_gap = Val::Px(1.0);
        });

    let trigger = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                // 22px and tight, like the toolbar's compact dropdowns — this
                // sits between the Play pill and the ribbon, where a control
                // that names a document can eat the bar if it's let to.
                height: Val::Px(22.0),
                padding: UiRect::horizontal(Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                position_type: PositionType::Relative,
                max_width: Val::Px(150.0),
                // NOT `Overflow::clip()`, however much a too-long name wants it:
                // the menu is a *child* of this node, and a clipping parent
                // clips absolutely-positioned descendants too — so the panel
                // opened, correctly, inside a 190×20 box and was never seen.
                // The clip belongs on the label, which is the thing that can
                // overflow.
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            Popup { panel, open: false },
            DocTabMenuTrigger,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("doc-tab-menu-trigger"),
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
    // Kind glyph + name of the active document, both following it.
    let icon = icon_text(
        commands,
        &fonts.phosphor,
        "film-slate",
        renzora_ui::DocTabKind::Scene.color(),
        12.0,
    );
    commands.entity(icon).insert(bevy::ui::FocusPolicy::Pass);
    renzora_ember::reactive::tracked::bind_text(commands, icon, |w: &Rx| {
        let name = active_doc(w).map(|t| t.kind.icon()).unwrap_or("film-slate");
        renzora_ember::phosphor_map::icon_glyph(name)
            .unwrap_or('\u{E4C6}')
            .to_string()
    });
    // The glyph carries the active document's type color, same as its tab does
    // in the strip — this trigger is that tab, in the compact layout that has no
    // room for the strip.
    renzora_ember::reactive::tracked::bind_text_color(commands, icon, |w: &Rx| {
        rgb(active_doc(w)
            .map(|t| t.kind.color())
            .unwrap_or_else(|| renzora_ui::DocTabKind::Scene.color()))
    });
    let label = commands
        .spawn((
            Text::new(String::new()),
            ui_font(font, 11.0),
            TextColor(rgb(text_primary())),
            // The trigger's width cap is enforced here rather than on the
            // trigger, which has the menu among its children (see above).
            bevy::text::TextLayout::no_wrap(),
            Node {
                min_width: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    renzora_ember::reactive::tracked::bind_text(commands, label, |w: &Rx| {
        active_doc(w)
            .map(|t| {
                let shown = elide(&t.name, DOC_TAB_CHARS);
                if t.is_modified {
                    format!("{shown}*")
                } else {
                    shown
                }
            })
            .unwrap_or_default()
    });
    let caret = glyph(commands, "caret-down", text_muted(), 10.0);
    commands.entity(caret).insert(bevy::ui::FocusPolicy::Pass);
    commands
        .entity(trigger)
        .add_children(&[icon, label, caret, panel]);

    // The strip's `+`, in the same place relative to the documents: immediately
    // to their right. `DocAddBtn` is what `doc_add_click` handles, wherever it
    // sits, so this is the strip's button in a second spot rather than a second
    // implementation of it.
    let plus = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(5.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            DocAddBtn,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            renzora_ember::widgets::HoverTooltip::new(renzora::lang::t("menu.file.new_scene")),
            Name::new("doc-tab-menu-add"),
        ))
        .id();
    renzora_ember::reactive::tracked::bind_bg(commands, plus, move |w| {
        match w.get::<Interaction>(plus) {
            Some(Interaction::Hovered) | Some(Interaction::Pressed) => {
                Color::srgba(1.0, 1.0, 1.0, 0.09)
            }
            _ => Color::NONE,
        }
    });
    let plus_icon = glyph(commands, "plus", text_muted(), 13.0);
    commands.entity(plus).add_child(plus_icon);

    let group = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(2.0),
                margin: UiRect::left(Val::Px(8.0)),
                display: Display::None,
                ..default()
            },
            Name::new("doc-tab-menu-group"),
        ))
        .id();
    renzora_ember::reactive::tracked::bind_display(commands, group, doc_tabs_dropdown);
    commands.entity(group).add_children(&[trigger, plus]);
    group
}

/// The active document tab, read through the `Rx` so a binding on it reacts.
fn active_doc<'w>(w: &Rx<'w>) -> Option<&'w renzora_ui::DocumentTab> {
    w.get_resource::<renzora_ui::DocumentTabState>()
        .and_then(|s| s.tabs.get(s.active_tab))
}

/// A hoverable row of the document-tab dropdown, without its contents.
fn doc_tab_menu_row_node(commands: &mut Commands, name: &'static str) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            DocTabMenuRow,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new(name),
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
    row
}

/// The dropdown's rows: every open document, active one accented. Keyed by id
/// like the strip's, so a row repaints only when its own content changes.
fn doc_tab_menu_snapshot(world: &Rx) -> renzora_ember::reactive::KeyedSnapshot {
    use std::hash::{Hash, Hasher};
    let empty = || renzora_ember::reactive::KeyedSnapshot {
        items: Vec::new(),
        build: Box::new(|c, _, _| c.spawn(Node::default()).id()),
    };
    let Some(state) = world.get_resource::<renzora_ui::DocumentTabState>() else {
        return empty();
    };
    // Closable follows the strip's rule exactly (see `doc_tab_snapshot`): the
    // model refuses the last tab and the last *scene*, so a ✕ that only some of
    // these rows can honour would be worse than none.
    let scenes = state.tabs.iter().filter(|t| !t.kind.is_asset()).count();
    let rows: Vec<(u64, String, renzora_ui::DocTabKind, bool, bool, bool)> = state
        .tabs
        .iter()
        .enumerate()
        .map(|(i, t)| {
            (
                t.id,
                t.name.clone(),
                t.kind,
                i == state.active_tab,
                t.is_modified,
                state.tabs.len() > 1 && (t.kind.is_asset() || scenes > 1),
            )
        })
        .collect();
    let items: Vec<(u64, u64)> = rows
        .iter()
        .map(|(id, name, kind, active, modified, can_close)| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            id.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (name, kind, active, modified, can_close).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    renzora_ember::reactive::KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (id, name, kind, active, modified, can_close) = &rows[i];
            let row = doc_tab_menu_row_node(c, "doc-tab-menu-row");
            c.entity(row).insert(DocTabClick(*id));
            // Type color on every row, matching the strip this dropdown stands
            // in for: whichever of the two the user has chosen, a document is
            // named by the same glyph in the same color.
            let ic = icon_text(c, &f.phosphor, kind.icon(), kind.color(), 11.0);
            let label = c
                .spawn((
                    Text::new(if *modified {
                        format!("{name}*")
                    } else {
                        name.clone()
                    }),
                    ui_font(&f.ui, 11.0),
                    TextColor(rgb(if *active { text_primary() } else { text_muted() })),
                    bevy::text::TextLayout::no_wrap(),
                    // Takes the slack so the ✕ sits at the row's right edge
                    // rather than trailing the name.
                    Node {
                        flex_grow: 1.0,
                        min_width: Val::Px(0.0),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    bevy::ui::FocusPolicy::Pass,
                ))
                .id();
            let mut kids = vec![ic, label];
            // Every closable row carries one, not just the active row: in the
            // strip you can click the tab you want to close first, but here the
            // menu is the only way at a document that isn't the current one, so
            // a click-then-✕ would mean switching to a document just to shut it.
            if *can_close {
                let close = c
                    .spawn((
                        Node {
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            width: Val::Px(14.0),
                            height: Val::Px(14.0),
                            flex_shrink: 0.0,
                            border_radius: BorderRadius::all(Val::Px(3.0)),
                            ..default()
                        },
                        BackgroundColor(Color::NONE),
                        Interaction::default(),
                        DocTabClose(*id),
                        // Block, or the press also reaches the row's
                        // `DocTabClick` — closing a document by way of first
                        // switching the editor to it. `Node`'s required
                        // `FocusPolicy` is `Pass` in Bevy 0.19, so this is not
                        // the default.
                        bevy::ui::FocusPolicy::Block,
                        renzora_ember::cursor_icon::HoverCursor(
                            bevy::window::SystemCursorIcon::Pointer,
                        ),
                        Name::new("doc-tab-menu-close"),
                    ))
                    .id();
                let x = icon_text(c, &f.phosphor, "x", text_muted(), 10.0);
                c.entity(close).add_child(x);
                kids.push(close);
            }
            c.entity(row).add_children(&kids);
            row
        }),
    }
}

/// Close the document dropdown behind a click on any of its rows. The row's own
/// job — activating that tab, or adding a scene — is done by the systems that
/// own [`DocTabClick`] / [`DocAddBtn`], which don't know or care that they were
/// pressed inside a menu.
pub(crate) fn doc_tab_menu_row_click(
    rows: Query<(&Interaction, &DocTabMenuRow), Changed<Interaction>>,
    triggers: Query<Entity, (With<DocTabMenuTrigger>, With<Popup>)>,
    mut commands: Commands,
) {
    if !rows.iter().any(|(i, _)| *i == Interaction::Pressed) {
        return;
    }
    for trigger in &triggers {
        renzora_ember::widgets::close_popup(&mut commands, trigger);
    }
}

/// Keyed snapshot of the open document tabs (id-keyed; the content hash carries
/// active/modified state so a tab repaints only when it actually changes).
fn doc_tab_snapshot(world: &Rx) -> renzora_ember::reactive::KeyedSnapshot {
    use std::hash::{Hash, Hasher};
    let empty = || renzora_ember::reactive::KeyedSnapshot {
        items: Vec::new(),
        build: Box::new(|c, _, _| c.spawn(Node::default()).id()),
    };
    let Some(state) = world.get_resource::<renzora_ui::DocumentTabState>() else {
        return empty();
    };
    // Closable is per-tab, because the model's two refusals aren't the same
    // rule: `close_tab` declines the last tab overall *and* the last scene tab,
    // the latter so Asset mode always has a scene to return to. Counting tabs
    // rather than scenes put a ✕ on the last scene as soon as a material was
    // open beside it — one that `close_tab` then quietly declined.
    let scenes = state.tabs.iter().filter(|t| !t.kind.is_asset()).count();
    let renaming = world.get_resource::<DocTabRename>().and_then(|r| r.0);
    let last = state.tabs.len().saturating_sub(1);
    // (id, name, kind, active, modified, renaming, trailing seam, closable)
    //
    // The *kind* travels rather than the glyph it resolves to, because the tab
    // now takes two things from it — the icon and that icon's type color — and
    // one of them in the snapshot would leave the other to be looked up twice.
    //
    // The seam belongs to the *boundary*, not to either tab, so exactly one of
    // the pair draws it: the left one. Every tab but the last, including either
    // side of the active one — with no fill on any tab there is nothing else
    // marking where one ends and the next begins.
    let tabs: Vec<(u64, String, renzora_ui::DocTabKind, bool, bool, bool, bool, bool)> = state
        .tabs
        .iter()
        .enumerate()
        .map(|(i, t)| {
            (
                t.id,
                t.name.clone(),
                t.kind,
                i == state.active_tab,
                t.is_modified,
                renaming == Some(t.id),
                i != last,
                state.tabs.len() > 1 && (t.kind.is_asset() || scenes > 1),
            )
        })
        .collect();
    let items: Vec<(u64, u64)> = tabs
        .iter()
        .map(|(id, name, kind, active, modified, editing, seam, can_close)| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            id.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (name, kind, active, modified, editing, seam, can_close).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    renzora_ember::reactive::KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (id, name, kind, active, modified, editing, seam, can_close) = &tabs[i];
            if *editing {
                build_doc_rename_field(c, &f.ui, *id, name)
            } else {
                doc_tab_row(c, f, *id, name, *kind, *active, *modified, *can_close, *seam)
            }
        }),
    }
}

/// Inline rename field for a document tab, replacing the tab itself for as long
/// as the edit is live (the same swap `ribbon_snapshot` does). Seeded with the
/// current name — which for a saved document is its file stem, extension
/// excluded; [`rename_doc_tab`] puts the extension back.
fn build_doc_rename_field(
    commands: &mut Commands,
    font: &bevy::text::FontSource,
    id: u64,
    name: &str,
) -> Entity {
    let input = text_input(commands, font, &renzora::lang::t("common.name"), name);
    commands.entity(input).insert((
        DocTabRenameInput(id),
        Node {
            width: Val::Px(140.0),
            height: Val::Px(22.0),
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.0)),
            // Square, like the tab it stands in for.
            flex_shrink: 0.0,
            ..default()
        },
        // Folding the tab you're in the middle of renaming into the caret menu
        // would take the field you're typing in off screen with it.
        renzora_ember::widgets::OverflowKeep,
    ));
    input
}

#[allow(clippy::too_many_arguments)]
fn doc_tab_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    id: u64,
    name: &str,
    kind: renzora_ui::DocTabKind,
    active: bool,
    modified: bool,
    can_close: bool,
    seam: bool,
) -> Entity {
    let fg = if active { text_primary() } else { text_muted() };
    let icon = kind.icon();
    let tab = commands
        .spawn((
            Node {
                // Full-height, square, and unfilled: with no background of
                // its own, a tab is its icon and its name, and the padding is
                // the only thing separating one from the next.
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(11.0), Val::Px(0.0)),
                // Bottom edge, pointing at the scene the tab selects, and on
                // EVERY tab rather than only the active one — the border eats
                // into the content box, so handing it to one state and not the
                // other would shift the label the moment you clicked. Inactive
                // tabs simply paint theirs transparent.
                border: UiRect::bottom(Val::Px(2.0)),
                flex_shrink: 0.0,
                ..default()
            },
            // No fill in either state. Fills and gradients both tried to say
            // "these are separate objects", and each added more chrome to a
            // strip whose job is to name six things. Marking the active tab is
            // left to the accent rule under it and its brighter label — the same
            // pairing, and the same token, as the workspace ribbon's underline.
            BackgroundColor(Color::NONE),
            BorderColor::all(if active { rgb(accent()) } else { Color::NONE }),
            Interaction::default(),
            DocTabClick(id),
            // The reorder drag hit-tests in the cursor's own space rather than
            // against node centres, which drift under UI scaling — see
            // `ribbon_interact`, which learned this the hard way.
            RelativeCursorPosition::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            // How the tab appears in the strip's `»` menu once it folds. The
            // active tab is pinned visible — folding the one you're editing into
            // a menu is exactly the tab you can least afford to lose sight of.
            // Dragging the row moves the tab instead of activating it, so a
            // folded tab isn't stranded at the end of the strip with no way back.
            renzora_ember::widgets::OverflowEntry::new(icon, name, move |w| activate_doc_tab(w, id))
                .on_drag(move |w| start_doc_tab_drag(w, id))
                .icon_color(kind.color()),
            Name::new(format!("doc:{name}")),
        ))
        .id();
    // Insertion marker: a thin accent bar at the tab's edge, hidden until a
    // reorder drag points at this slot. Absolutely positioned, so it never
    // affects the strip's layout (or its width budget).
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
            Name::new("doc-insert-marker"),
        ))
        .id();
    commands.entity(tab).insert(DocTabItem { id, marker });
    if active {
        commands.entity(tab).insert(renzora_ember::widgets::OverflowKeep);
    }
    // Kind icon: scene vs material vs script at a glance, without reading the
    // name. It was dropped while the strip was the top bar's fixed-width left
    // zone, where every glyph cost a tab off the visible end; spanning the whole
    // viewport there's room for it again.
    //
    // The type color, in every state — the same green the asset browser gives a
    // material, on the active tab and the inactive ones alike. Graying the
    // inactive ones made the strip say "current tab" twice, once with the accent
    // rule and again with six identical gray glyphs, while throwing away the one
    // thing the icon is there for: which tab holds which kind of thing. Active
    // state is the underline and the brighter label; the icon is type identity.
    let kind_icon = icon_text(commands, &fonts.phosphor, icon, kind.color(), 12.0);
    // Elide the *name*, then add the modified marker — eliding afterwards would
    // eat the asterisk on exactly the tabs that most need it.
    let shown = elide(name, DOC_TAB_CHARS);
    if shown != name {
        commands
            .entity(tab)
            .insert(renzora_ember::widgets::HoverTooltip::new(name));
    }
    // Semibold, not regular: the tab labels are the one place in the chrome
    // that names what you're editing, and at this size the weight is what
    // carries the active tab now that its fill is the same as the bar's.
    let mut label_font = ui_font(&fonts.ui, 12.0);
    label_font.weight = bevy::text::FontWeight::SEMIBOLD;
    let lbl = commands
        .spawn((
            Text::new(if modified { format!("{shown}*") } else { shown }),
            label_font,
            TextColor(rgb(fg)),
        ))
        .id();
    let mut kids = vec![kind_icon, lbl];
    // Only the active tab carries a ✕. On every tab it was six close buttons
    // competing for the eye, and it made the strip a near-copy of the dock's own
    // panel tab bar directly above — same chips, same ✕, same trailing `+` — for
    // two entirely different ideas. Closing an inactive scene is now click-then-✕,
    // which is one extra click on the thing you were about to look at anyway.
    if can_close && active {
        let close = commands
            .spawn((
                Node {
                    align_items: AlignItems::Center,
                    padding: UiRect::left(Val::Px(1.0)),
                    ..default()
                },
                Interaction::default(),
                DocTabClose(id),
                renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            ))
            .id();
        let x = icon_text(commands, &fonts.phosphor, "x", text_muted(), 10.0);
        commands.entity(close).add_child(x);
        kids.push(close);
    }
    // The boundary between two tabs: a short hairline centred on the trailing
    // edge, not a full-height rule. Edge-to-edge lines on flush tabs read as a
    // picket fence — the eye follows the verticals instead of the names.
    // Absolutely positioned, so it costs the tab no width.
    if seam {
        let line = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(0.0),
                    top: Val::Percent(30.0),
                    height: Val::Percent(40.0),
                    width: Val::Px(1.0),
                    ..default()
                },
                BackgroundColor(doc_tab_divider()),
                bevy::ui::FocusPolicy::Pass,
                Name::new("doc-tab-seam"),
            ))
            .id();
        kids.push(line);
    }
    kids.push(marker);
    commands.entity(tab).add_children(&kids);
    tab
}

/// The hairline between two scene tabs.
///
/// The same token the viewport toolbar's own separators use (`border`, which the
/// palette takes from the theme's `border_light`), so the two rows of chrome
/// divide their contents the same way. It is deliberately NOT `divider`: that is
/// the darker token, and it belongs to the hard edge under the whole strip, not
/// to the soft boundaries between names inside it.
fn doc_tab_divider() -> Color {
    rgb(border())
}

/// Shorten `s` to `max` characters, ending in an ellipsis when it doesn't fit.
///
/// Done on the string because bevy_ui has no `text-overflow: ellipsis` — a `Text`
/// wider than its node either wraps or spills, and neither is what a tab wants.
/// Counting *characters* rather than measuring the laid-out width is
/// approximate for a proportional font, but it's stable, costs nothing, and an
/// elided tab carries the full name in a hover tooltip.
fn elide(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let kept: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", kept.trim_end())
}

/// Activate document tab `id` from a `&mut World` context — the same work
/// [`doc_tab_click`] does for a click on the tab itself, for the tabs that have
/// folded into the strip's overflow menu.
fn activate_doc_tab(w: &mut World, id: u64) {
    let Some(mut state) = w.get_resource_mut::<renzora_ui::DocumentTabState>() else {
        return;
    };
    let Some(idx) = state.tabs.iter().position(|t| t.id == id) else {
        return;
    };
    let switch = state.activate_tab(idx);
    let layout = state.tabs[idx].kind.layout_name().map(|n| n.to_string());
    if let Some((old_tab_id, new_tab_id)) = switch {
        w.insert_resource(renzora::TabSwitchRequest { old_tab_id, new_tab_id });
    }
    let Some(layout) = layout else { return };
    let index = w
        .get_resource::<ShellLayouts>()
        .and_then(|l| l.layouts.iter().position(|(n, _)| *n == layout));
    if let Some(index) = index {
        select_workspace(w, index);
    }
}

/// `+` → add an "Untitled Scene" document and focus it.
pub(crate) fn doc_add_click(
    mut commands: Commands,
    q: Query<&Interaction, (With<DocAddBtn>, Changed<Interaction>)>,
    state: Option<ResMut<renzora_ui::DocumentTabState>>,
) {
    let Some(mut state) = state else { return };
    if q.iter().any(|i| *i == Interaction::Pressed) {
        let idx = state.add_tab("Untitled Scene".into(), None);
        // Cache the leaving scene + load the new (empty) tab's scene. The new
        // tab has no buffer, so `handle_tab_switch` resets to a fresh empty
        // scene — what "New Scene" should show, instead of the current scene.
        if let Some((old_id, new_id)) = state.activate_tab(idx) {
            commands.insert_resource(renzora::TabSwitchRequest {
                old_tab_id: old_id,
                new_tab_id: new_id,
            });
        }
    }
}

/// Whether a tab of `kind` belongs to the workspace called `workspace`.
///
/// Both layout tables count. `layout_name` is the direct answer for most kinds,
/// but shaders name a `Shaders` workspace that doesn't exist — the layout that
/// actually opens a `.wgsl` is the code editor's, which its *asset* layout
/// (`Scripting-Asset`) names. Stripping the `-Asset` suffix reads that mapping
/// off the data instead of hard-coding the exception, and it keeps the
/// `scene_layout_names_are_unique` invariant those tables are tested against.
fn kind_in_workspace(kind: renzora_ui::DocTabKind, workspace: &str) -> bool {
    kind.layout_name() == Some(workspace)
        || kind
            .asset_layout_name()
            .and_then(|l| l.strip_suffix("-Asset"))
            == Some(workspace)
}

/// Follow the active document tab: point [`renzora_ui::EditorContext`] at it and
/// switch the workspace its kind maps to.
///
/// This runs for *every* activation — a tab click, a programmatic open
/// (double-clicking an asset, the inspector's "edit" button), a close that
/// promotes its neighbour — because it watches the active id rather than any one
/// route. The `EditorContext` half is what makes clicking a second material tab
/// swap the graph: every asset panel loads from the context's path, so without
/// it the dock switched to the Materials workspace and left the *previous*
/// material in it. `open_asset_tab` sets the context when it opens a document;
/// nothing else did when you moved between two already-open ones.
///
/// The `Local` change-guard means it only fires on a real active-tab change, so
/// ribbon navigation while a doc tab is open isn't fought (the scene entities
/// are never touched — this is purely a layout switch).
#[allow(clippy::too_many_arguments)]
pub(crate) fn sync_workspace_to_active_doc(
    state: Option<Res<renzora_ui::DocumentTabState>>,
    mut layouts: ResMut<ShellLayouts>,
    mut dock: ResMut<Dock>,
    mut dirty: ResMut<DockDirty>,
    context: Option<ResMut<renzora_ui::EditorContext>>,
    project: Option<Res<renzora::CurrentProject>>,
    mut mru: ResMut<DocTabMru>,
    mut commands: Commands,
    mut last: Local<Option<u64>>,
) {
    let Some(state) = state else { return };
    let active_id = state.active_tab_id();
    if *last == active_id {
        return;
    }
    *last = active_id;
    let Some(tab) = state.active_tab() else { return };

    // Newest last. Dropping ids of closed tabs here keeps the stack from growing
    // for a session's worth of opens and closes.
    mru.0.retain(|id| *id != tab.id && state.tabs.iter().any(|t| t.id == *id));
    mru.0.push(tab.id);

    // Asset panels read their file straight off this, so it has to move with the
    // tab. Written only when it actually differs: it's a change-detected
    // resource, and several panels reload on any change to it.
    if let Some(mut context) = context {
        let next = renzora_ui::EditorContext::from_tab(tab);
        if *context != next {
            *context = next;
        }
    }

    // The kind's own workspace, or — for a kind naming one that doesn't exist —
    // the workspace its asset layout is derived from, which is the same fallback
    // [`kind_in_workspace`] accepts. Shaders are the case: `Shaders` is not a
    // workspace, but `Scripting-Asset` says the code editor's is where a `.wgsl`
    // belongs.
    let wi = [
        tab.kind.layout_name(),
        tab.kind
            .asset_layout_name()
            .and_then(|l| l.strip_suffix("-Asset")),
    ]
    .into_iter()
    .flatten()
    .find_map(|name| layouts.layouts.iter().position(|(n, _)| n == name));
    if let Some(wi) = wi {
        apply_workspace(wi, &mut layouts, &mut dock, &mut dirty);
    }

    // A script or shader has no panel that reads `EditorContext` — the code
    // editor keeps its own list of open files and only ever hears about one
    // through `OpenCodeEditorFile`. Asking again for a file it already holds
    // just focuses that tab, so this is the same move `open_asset_tab` makes,
    // on the route it doesn't cover: moving between two documents already open.
    //
    // Revealing the panel belongs here rather than at the asset browser's
    // double-click, because it has to happen *after* the workspace switch
    // above — done there, the code editor was added to the layout we were on
    // the way out of.
    match tab.kind {
        renzora_ui::DocTabKind::Script | renzora_ui::DocTabKind::Shader => {
            if let (Some(rel), Some(project)) = (tab.scene_path.as_ref(), project) {
                commands.insert_resource(renzora::core::OpenCodeEditorFile {
                    path: project.resolve_path(rel),
                });
            }
            // Dirty either way: `focus_or_add_panel` returns false when the
            // panel was already there, but it still moved that leaf's active
            // tab, and the dock only repaints when flagged.
            dock.tree.focus_or_add_panel("code_editor");
            dirty.0 = true;
        }
        // A UI template's document *is* a canvas, so returning to its tab
        // re-selects that canvas and reveals the panel showing it — the same
        // move, one panel over.
        renzora_ui::DocTabKind::Ui => {
            if let (Some(rel), Some(project)) = (tab.scene_path.as_ref(), project) {
                commands.insert_resource(renzora::core::OpenUiTemplateFile {
                    path: project.resolve_path(rel),
                });
            }
            dock.tree.focus_or_add_panel("ui_canvas");
            dirty.0 = true;
        }
        _ => {}
    }
}

/// The other direction: switching workspace brings that workspace's document
/// forward. Pick the Materials workspace off the ribbon and the material you
/// were last editing is the active tab again, with its graph loaded — rather
/// than the Materials layout sitting there showing whatever the scene tab you
/// were on happens to select.
///
/// Nothing happens when the active tab already belongs to the workspace being
/// switched to, which is what keeps this from fighting
/// [`sync_workspace_to_active_doc`]: a tab click switches the workspace, this
/// system sees that change, finds the tab that caused it already in place, and
/// stops. That check is on the *active tab* rather than on the MRU stack
/// deliberately — it holds whichever of the two systems runs first in a frame,
/// where "is the MRU top fresh yet" would not.
///
/// A workspace no open document maps to (Debug, Hub, Animation) leaves the
/// active tab alone: there is nothing there to bring forward, and stealing the
/// tab strip's selection to show something unrelated would be worse than
/// leaving it.
pub(crate) fn sync_active_doc_to_workspace(
    layouts: Res<ShellLayouts>,
    state: Option<ResMut<renzora_ui::DocumentTabState>>,
    mru: Res<DocTabMru>,
    mut last: Local<Option<usize>>,
    mut commands: Commands,
) {
    let Some(mut state) = state else { return };
    if *last == Some(layouts.active) {
        return;
    }
    *last = Some(layouts.active);
    let Some((name, _)) = layouts.layouts.get(layouts.active) else {
        return;
    };
    if state
        .active_tab()
        .is_some_and(|t| kind_in_workspace(t.kind, name))
    {
        return;
    }
    // Most recent first; falling back to display order for a workspace you have
    // documents in but have never been to this session (restored tabs, say).
    let idx = mru
        .0
        .iter()
        .rev()
        .find_map(|id| {
            state
                .tabs
                .iter()
                .position(|t| t.id == *id && kind_in_workspace(t.kind, name))
        })
        .or_else(|| {
            state
                .tabs
                .iter()
                .position(|t| kind_in_workspace(t.kind, name))
        });
    let Some(idx) = idx else { return };
    // Through `activate_tab` + `TabSwitchRequest` like every other switch, so a
    // scene→scene move still swaps the live scene for the incoming tab's buffer.
    if let Some((old_id, new_id)) = state.activate_tab(idx) {
        commands.insert_resource(renzora::TabSwitchRequest {
            old_tab_id: old_id,
            new_tab_id: new_id,
        });
    }
}

pub(crate) fn doc_tab_click(
    mut commands: Commands,
    q: Query<(&Interaction, &DocTabClick), Changed<Interaction>>,
    state: Option<ResMut<renzora_ui::DocumentTabState>>,
    rename: Res<DocTabRename>,
) {
    let Some(mut state) = state else { return };
    for (interaction, click) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        // While this tab is being renamed its edit field owns clicks — a press
        // really landing in there must not re-activate the tab underneath.
        if rename.0 == Some(click.0) {
            continue;
        }
        let Some(idx) = state.tabs.iter().position(|t| t.id == click.0) else {
            continue;
        };
        // Activate the tab AND swap the scene/asset content it owns. Without
        // the `TabSwitchRequest`, clicking a tab only switched the dock layout
        // (a no-op for scene→scene) and the viewport kept the old scene —
        // `handle_tab_switch` is what caches the leaving tab + restores this
        // tab's buffered scene.
        if let Some((old_id, new_id)) = state.activate_tab(idx) {
            commands.insert_resource(renzora::TabSwitchRequest {
                old_tab_id: old_id,
                new_tab_id: new_id,
            });
        }
        // The workspace, the editor context and the code editor's focus all
        // follow from the active tab having changed, and
        // [`sync_workspace_to_active_doc`] does that for every route into a
        // document — this one, an asset-browser double-click, a close promoting
        // its neighbour. A copy of the layout switch lived here too and had
        // already drifted: it knew only `layout_name`, so a shader tab clicked
        // here went looking for a `Shaders` workspace that doesn't exist.
    }
}

/// Press-latch reorder for the document tabs, plus the double-click that opens an
/// inline rename: dragging a tab past a small threshold moves it in
/// [`renzora_ui::DocumentTabState`] on release, while two quick clicks that
/// *didn't* drag start a rename. Mirrors `ribbon_interact`.
///
/// The reorder is applied **once, on release** rather than live as the cursor
/// crosses each neighbour: every mutation of `DocumentTabState` is a project.toml
/// write (`persist_open_tabs`), and a live reorder would spend one per tab
/// crossed. The insertion marker is what makes that deferral invisible.
///
/// The double-click lives here rather than in [`doc_tab_click`] for the same
/// reason it keys off the *release*: this is the only place that knows whether
/// the press in between turned into a drag. Arming the rename from presses alone
/// would fire it on the click that follows a reorder.
#[allow(clippy::too_many_arguments)]
pub(crate) fn doc_tab_drag(
    mut drag: ResMut<DocTabDrag>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    time: Res<Time>,
    mut rename: ResMut<DocTabRename>,
    pressed: Query<(&DocTabItem, &Interaction)>,
    items: Query<(&DocTabItem, &RelativeCursorPosition, &Visibility)>,
    mut nodes: Query<&mut Node>,
    state: Option<ResMut<renzora_ui::DocumentTabState>>,
    mut last_click: Local<Option<(u64, f64)>>,
) {
    let hide_markers = |items: &Query<(&DocTabItem, &RelativeCursorPosition, &Visibility)>,
                        nodes: &mut Query<&mut Node>| {
        for (it, _, _) in items {
            if let Ok(mut n) = nodes.get_mut(it.marker) {
                if n.display != Display::None {
                    n.display = Display::None;
                }
            }
        }
    };

    // Don't drag while a tab is being renamed — the press belongs to the field.
    if rename.0.is_some() {
        drag.0 = None;
        hide_markers(&items, &mut nodes);
        return;
    }
    let Some(mut state) = state else {
        drag.0 = None;
        hide_markers(&items, &mut nodes);
        return;
    };
    let cursor = windows.iter().next().and_then(|w| w.cursor_position());

    if drag.0.is_none() && mouse.just_pressed(MouseButton::Left) {
        if let Some(cur) = cursor {
            for (item, interaction) in &pressed {
                if *interaction == Interaction::Pressed {
                    let from = state.tabs.iter().position(|t| t.id == item.id).unwrap_or(0);
                    drag.0 = Some(DocTabDragState {
                        id: item.id,
                        start_cursor: cur,
                        active: false,
                        target: from,
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

    // Track the insertion slot under the cursor and show the matching edge
    // marker: the cursor in a tab's left half inserts before it, right half
    // after it. Folded tabs never report `cursor_over`, so they're skipped for
    // free — a drag can only land among the tabs actually on screen.
    match drag.0.as_mut() {
        Some(st) if st.active => {
            let mut shown: Option<(Entity, bool)> = None;
            for (it, rcp, vis) in &items {
                // A tab still being measured out of the flow sits at its static
                // position and would hit-test over a real one — see the strip's
                // `probe_new_item`. It isn't on screen; it can't be a drop target.
                if !rcp.cursor_over || *vis == Visibility::Hidden {
                    continue;
                }
                let Some(idx) = state.tabs.iter().position(|t| t.id == it.id) else {
                    continue;
                };
                let before = rcp.normalized.is_none_or(|n| n.x < 0.0);
                st.target = if before { idx } else { idx + 1 };
                shown = Some((it.marker, !before));
                break;
            }
            hide_markers(&items, &mut nodes);
            if let Some((marker, right)) = shown {
                if let Ok(mut n) = nodes.get_mut(marker) {
                    n.display = Display::Flex;
                    if right {
                        n.left = Val::Auto;
                        n.right = Val::Px(-2.0);
                    } else {
                        n.left = Val::Px(-2.0);
                        n.right = Val::Auto;
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
        // A click, not a drag: the second one within the double-click window
        // opens the inline rename. The tab is already active from the press.
        let now = time.elapsed_secs_f64();
        if last_click.is_some_and(|(id, t)| id == st.id && now - t < 0.4) {
            *last_click = None;
            rename.0 = Some(st.id);
        } else {
            *last_click = Some((st.id, now));
        }
        return;
    }
    // A reorder invalidates the click that started it, so the next click on the
    // tab you just moved isn't read as the second half of a double-click.
    *last_click = None;
    let Some(from) = state.tabs.iter().position(|t| t.id == st.id) else {
        return;
    };
    // `reorder` takes an insertion slot in the *pre-removal* list, so both the
    // tab's own slot and the one just past it are no-ops.
    let to = st.target.min(state.tabs.len());
    if to != from && to != from + 1 {
        state.reorder(from, to);
    }
}

/// Start carrying a document tab that has folded into the strip's `»` menu,
/// from the drag the menu row hands over. Born active: the press that started it
/// was inside the menu, so there's no click/drag ambiguity left to resolve, and
/// no strip position to measure the threshold from.
fn start_doc_tab_drag(world: &mut World, id: u64) {
    let from = world
        .get_resource::<renzora_ui::DocumentTabState>()
        .and_then(|s| s.tabs.iter().position(|t| t.id == id))
        .unwrap_or(0);
    if let Some(mut drag) = world.get_resource_mut::<DocTabDrag>() {
        drag.0 = Some(DocTabDragState {
            id,
            start_cursor: Vec2::ZERO,
            active: true,
            target: from,
        });
    }
}

/// Auto-focus the document-tab rename field the frame it spawns, with the whole
/// name selected the way an OS rename does — a double-click means "replace this",
/// not "put a caret somewhere in it".
pub(crate) fn doc_focus_rename(mut q: Query<&mut EmberTextInput, Added<DocTabRenameInput>>) {
    for mut inp in &mut q {
        inp.focused = true;
        inp.select_all = true;
    }
}

/// Commit (Enter / click-away) or cancel (Escape) the active document-tab rename.
///
/// Commit-on-blur waits until the field has actually held focus: it's spawned by
/// the keyed-list rebuild a frame or two after [`DocTabRename`] is set, so "no
/// field yet" must not read as "gone", and the double-click that opened the
/// rename must not immediately close it again.
pub(crate) fn doc_rename_commit(
    mut rename: ResMut<DocTabRename>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut inputs: Query<(
        &mut EmberTextInput,
        &RelativeCursorPosition,
        &DocTabRenameInput,
    )>,
    mut commands: Commands,
    mut had_focus: Local<bool>,
) {
    let Some(id) = rename.0 else {
        *had_focus = false;
        return;
    };
    if keys.just_pressed(KeyCode::Escape) {
        rename.0 = None;
        *had_focus = false;
        return;
    }
    let Some((mut inp, rcp, _)) = inputs.iter_mut().find(|(_, _, r)| r.0 == id) else {
        return;
    };
    // A click inside the field (to move the caret) must keep it editing; the
    // strip's own click handling can otherwise steal focus the instant you click.
    if mouse.just_pressed(MouseButton::Left) && rcp.cursor_over && !inp.focused {
        inp.focused = true;
    }
    if inp.focused {
        *had_focus = true;
    }
    if !*had_focus {
        return;
    }
    let enter = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter);
    let clicked_away = mouse.just_pressed(MouseButton::Left) && !rcp.cursor_over;
    if !enter && !clicked_away {
        return;
    }
    let new: String = inp.value.replace('\n', "").trim().to_string();
    rename.0 = None;
    *had_focus = false;
    if new.is_empty() {
        return;
    }
    commands.queue(move |world: &mut World| rename_doc_tab(world, id, &new));
}

/// Apply a document-tab rename.
///
/// A tab with a file behind it is renamed **on disk**. Its label is that file's
/// stem and nothing else — `editor_open_tabs` persists only paths and kinds, and
/// a reopened tab takes its name from the path again — so a label-only rename
/// would silently undo itself on the next project load. The move is announced
/// via [`renzora::AssetPathChanged`], the same event the asset browser fires, so
/// every holder of the old path (this tab included, through
/// [`doc_tabs_follow_asset_path`]) is patched by one code path.
///
/// An unsaved tab has no file, so there the label really is all there is.
fn rename_doc_tab(world: &mut World, id: u64, new_name: &str) {
    let old_rel = world
        .get_resource::<renzora_ui::DocumentTabState>()
        .and_then(|s| s.tabs.iter().find(|t| t.id == id))
        .map(|t| t.scene_path.clone());
    let Some(old_rel) = old_rel else { return };
    let Some(old_rel) = old_rel else {
        // No path yet — this is a `+` tab ("Untitled Scene"). Naming it used to
        // relabel the tab and nothing else, so the scene the user had just built
        // and named still existed nowhere on disk, with no prompt to say so.
        // Naming an untitled scene now creates it, which also matches what
        // renaming a *saved* tab does: the tab label IS the file name.
        name_untitled_scene(world, id, new_name);
        return;
    };

    let Some(old_abs) = world
        .get_resource::<renzora::CurrentProject>()
        .map(|p| p.resolve_path(&old_rel))
    else {
        return;
    };
    // Keep the extension: the label the user edited never had one.
    let file_name = match old_abs.extension().and_then(|e| e.to_str()) {
        Some(ext) => format!("{new_name}.{ext}"),
        None => new_name.to_string(),
    };
    let new_abs = old_abs.with_file_name(&file_name);
    if new_abs == old_abs {
        return;
    }
    if new_abs.exists() {
        warn!("[tabs] rename refused — '{}' already exists", new_abs.display());
        return;
    }
    if let Err(e) = std::fs::rename(&old_abs, &new_abs) {
        warn!("[tabs] failed to rename '{}': {e}", old_abs.display());
        return;
    }
    // Derived from the stored path rather than re-deriving it from the new
    // absolute one: `make_relative` canonicalizes, and the tab's path is already
    // project-relative with forward slashes.
    let new_rel = match old_rel.rfind('/') {
        Some(i) => format!("{}/{}", &old_rel[..i], file_name),
        None => file_name,
    };
    world.trigger(renzora::AssetPathChanged {
        old: old_rel,
        new: new_rel,
        is_dir: false,
    });
}

/// Give a never-saved scene tab a name, and create the file to match.
///
/// Only the **active** tab writes a file: an inactive tab's contents live in
/// `SceneTabBuffers`, not in the world, so saving here would write whatever
/// scene happens to be open into somebody else's file. An inactive tab just
/// takes the label and stays untitled until it is focused and saved.
fn name_untitled_scene(world: &mut World, id: u64, new_name: &str) {
    let file_stem: String = new_name
        .trim()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    if file_stem.is_empty() {
        return;
    }

    let (is_active_scene, _) = world
        .get_resource::<renzora_ui::DocumentTabState>()
        .and_then(|s| {
            let active_id = s.tabs.get(s.active_tab).map(|t| t.id);
            s.tabs
                .iter()
                .find(|t| t.id == id)
                .map(|t| {
                    (
                        active_id == Some(id) && t.kind == renzora_ui::DocTabKind::Scene,
                        (),
                    )
                })
        })
        .unwrap_or((false, ()));

    // Relabel regardless; only the active scene tab also gains a file.
    if let Some(mut state) = world.get_resource_mut::<renzora_ui::DocumentTabState>() {
        if let Some(tab) = state.tabs.iter_mut().find(|t| t.id == id) {
            tab.name = new_name.to_string();
        }
    }
    if !is_active_scene {
        return;
    }

    let rel = format!("scenes/{file_stem}.bsn");
    let abs = match world.get_resource::<renzora::CurrentProject>() {
        Some(p) => p.resolve_path(&rel),
        None => return,
    };
    if abs.exists() {
        warn!("[tabs] '{}' already exists — scene not created", abs.display());
        renzora::core::console_log::console_error(
            "Scene",
            format!("A scene named '{file_stem}' already exists"),
        );
        return;
    }
    if let Some(dir) = abs.parent() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            warn!("[tabs] failed to create {}: {e}", dir.display());
            return;
        }
    }

    // Point the tab at the new path, then let the normal save path write it —
    // `save_scene_system` sees a scene tab WITH a path and targets exactly this
    // file, so there is one scene-writing code path rather than two.
    if let Some(mut state) = world.get_resource_mut::<renzora_ui::DocumentTabState>() {
        if let Some(tab) = state.tabs.iter_mut().find(|t| t.id == id) {
            tab.scene_path = Some(rel.clone());
        }
    }
    world.insert_resource(renzora::core::SaveSceneRequested);
    renzora::core::console_log::console_success("Scene", format!("Created {rel}"));
}

/// Follow a renamed or moved asset in the open document tabs, so a rename from
/// anywhere — this strip, the asset browser, a folder move — leaves every open
/// tab pointing at the file it actually has open rather than at a dead path.
pub(crate) fn doc_tabs_follow_asset_path(
    trigger: On<renzora::AssetPathChanged>,
    state: Option<ResMut<renzora_ui::DocumentTabState>>,
    context: Option<ResMut<renzora_ui::EditorContext>>,
) {
    let ev = trigger.event();
    if let Some(mut state) = state {
        for tab in state.tabs.iter_mut() {
            let Some(new_path) = tab.scene_path.as_ref().and_then(|p| ev.rewrite(p)) else {
                continue;
            };
            if let Some(stem) = std::path::Path::new(&new_path)
                .file_stem()
                .and_then(|s| s.to_str())
            {
                tab.name = stem.to_string();
            }
            tab.scene_path = Some(new_path);
        }
    }
    // Asset-mode panels load straight from this path, so it has to move too.
    if let Some(mut context) = context {
        if let renzora_ui::EditorContext::Asset { path, .. } = &mut *context {
            if let Some(new_path) = ev.rewrite(path) {
                *path = new_path;
            }
        }
    }
}

/// Close a document tab by id and, if it was the active tab, fire a
/// [`renzora::TabSwitchRequest`] so the viewport follows to the newly-active
/// tab. `close_tab` only moves the active index — it never swaps scene content —
/// so without this the old scene would linger under a different active tab.
pub(crate) fn close_doc_tab_by_id(
    state: &mut renzora_ui::DocumentTabState,
    id: u64,
    commands: &mut Commands,
) {
    let Some(idx) = state.tabs.iter().position(|t| t.id == id) else {
        return;
    };
    let was_active = state.active_tab == idx;
    // The active tab's id before the close — used as `old` for the switch so
    // `handle_tab_switch` despawns the current scene before loading the next.
    let prev_active_id = state.active_tab_id();
    if state.close_tab(idx).is_some() && was_active {
        if let (Some(old), Some(new)) = (prev_active_id, state.active_tab_id()) {
            if old != new {
                commands.insert_resource(renzora::TabSwitchRequest {
                    old_tab_id: old,
                    new_tab_id: new,
                });
            }
        }
    }
}

/// Click a document tab's × → close it. A tab with unsaved changes opens a
/// save-confirmation prompt instead of closing outright (see
/// [`crate::save_prompts::process_tab_close_request`]); clean tabs close
/// immediately. The model refuses to close the last scene / last tab regardless.
pub(crate) fn doc_tab_close(
    q: Query<(&Interaction, &DocTabClose), Changed<Interaction>>,
    state: Option<ResMut<renzora_ui::DocumentTabState>>,
    prompt_open: Query<(), With<CloseTabPromptRoot>>,
    mut commands: Commands,
) {
    let Some(mut state) = state else { return };
    // A prompt is already up — ignore clicks until it's resolved.
    if !prompt_open.is_empty() {
        return;
    }
    for (interaction, close) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(idx) = state.tabs.iter().position(|t| t.id == close.0) else {
            continue;
        };
        if state.tabs[idx].is_modified {
            // Defer to the prompt flow; it activates the tab and asks the user.
            commands.insert_resource(TabCloseRequest { id: close.0 });
        } else {
            close_doc_tab_by_id(&mut state, close.0, &mut commands);
        }
    }
}
