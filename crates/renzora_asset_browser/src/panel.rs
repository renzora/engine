//! The panel tree itself: `build` assembles the toolbar, the breadcrumb strip,
//! the folder-tree pane, the splitter and the grid, and wires every reactive
//! binding between them. The breadcrumb segments live here too — they are part
//! of this layout and nothing else builds them.

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{
    bind_2way, bind_bg, bind_display, bind_with, keyed_list, keyed_list_tokened,
};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::{accent, panel_bg, popup_bg, rgb, text_muted, text_primary};
use renzora_ember::virtual_scroll::virtual_scroll_versioned;
use renzora_ember::widgets::{icon_label_button_parts, scroll_view, slider, text_input};

use crate::grid::{grid_snapshot, grid_token, list_entries};
use crate::layout::{crumb_surface, header_surface, is_compact, toolbar_action};
use crate::ops::{current_folder, project_root};
use crate::state::{
    AddMenuBtn, AssetBack, AssetGrid, AssetRoot, AssetSearch, CrumbNav, GridArea, ImportBtn,
    NativeAssets, NewAsset, NewAssetBtn, SortMenuBtn, Splitter, TreeAddBtn, TreeSearch, TreeTab,
    TreeTabBtn, ViewToggleBtn,
};
use crate::tree::{tree_snapshot, tree_token};

pub(crate) fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                // A column: the toolbar spans the panel, and the
                // tree|splitter|grid row sits under it (see `body`, below).
                flex_direction: FlexDirection::Column,
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                ..default()
            },
            bevy::ui::RelativeCursorPosition::default(),
            AssetRoot,
        ))
        .id();

    // ── Folder tree (left pane, own scroll) ──
    let tree_list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.0,
            padding: UiRect::vertical(Val::Px(4.0)),
            ..default()
        })
        .id();
    keyed_list_tokened(commands, tree_list, tree_token, tree_snapshot);
    let tree_scroll = scroll_view(commands, tree_list);
    let tree_pane = commands
        .spawn((
            Node {
                width: Val::Px(180.0),
                height: Val::Percent(100.0),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                min_height: Val::Px(0.0),
                ..default()
            },
            // Match the hierarchy panel base so the shared odd/even stripe
            // (inspector_stripe) renders the exact same colors.
            BackgroundColor(rgb(panel_bg())),
        ))
        .id();
    // ── Tree-pane actions ──
    // When the panel collapses to the tree-only file browser the main toolbar
    // (which carries Add / Import / New Folder and the search box) is hidden with
    // the grid, so the browser would lose both entirely. Both are rebuilt below as
    // one header row: a search box of its own, plus a single "+ Add" dropdown (see
    // `tree_add`) folding in New Folder and Import — one control instead of a row
    // of three, so it costs no extra row and can never wrap.
    // The Project | Recent | Favs tabs stay visible in BOTH layouts, replacing the
    // old collapsible FAVORITES / RECENT sections so each list gets the full pane
    // height.

    // Search box — takes whatever width the Add button beside it leaves, with its
    // own marker + state field so it can't fight the hidden toolbar search (see
    // `TreeSearch`).
    let tree_search_input = text_input(commands, &fonts.ui, &renzora::lang::t("common.search"), "");
    commands.entity(tree_search_input).insert((
        TreeSearch,
        Node {
            // The flexible half of the header row: the button is `flex_shrink: 0`,
            // so the box absorbs the whole remainder and shrinks to nothing rather
            // than pushing the button off the pane.
            flex_grow: 1.0,
            flex_basis: Val::Px(0.0),
            min_width: Val::Px(0.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
    ));

    // Project | Recent | Favs tabs (accent underline marks the active one).
    // `TreeTab::Folders` keeps its name — the tab is the folder tree; only its
    // label reads "Project", since what it shows IS the project's asset root.
    let tree_tabs = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                width: Val::Percent(100.0),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::header_bg())),
            Name::new("assets-tree-tabs"),
        ))
        .id();
    for (tab, label) in [
        (TreeTab::Folders, "Project"),
        (TreeTab::Recent, "Recent"),
        (TreeTab::Favorites, "Favs"),
    ] {
        let btn = commands
            .spawn((
                Node {
                    flex_grow: 1.0,
                    flex_basis: Val::Px(0.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(Val::Px(6.0), Val::Px(5.0)),
                    border: UiRect::bottom(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                BorderColor::all(Color::NONE),
                Interaction::default(),
                HoverCursor(SystemCursorIcon::Pointer),
                TreeTabBtn(tab),
                Name::new("assets-tree-tab"),
            ))
            .id();
        bind_bg(commands, btn, move |w| {
            if matches!(
                w.get::<Interaction>(btn),
                Some(Interaction::Hovered) | Some(Interaction::Pressed)
            ) {
                return rgb(renzora_ember::theme::hover_bg());
            }
            Color::NONE
        });
        bind_with(
            commands,
            btn,
            move |w| w.get_resource::<NativeAssets>().is_some_and(|s| s.tree_tab == tab),
            |w, e, active: &bool| {
                let c = if *active { rgb(accent()) } else { Color::NONE };
                if let Some(mut b) = w.get_mut::<BorderColor>(e) {
                    *b = BorderColor::all(c);
                }
            },
        );
        let text = commands
            .spawn((Text::new(label), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), Pickable::IGNORE))
            .id();
        bind_with(
            commands,
            text,
            move |w| w.get_resource::<NativeAssets>().is_some_and(|s| s.tree_tab == tab),
            |w, e, active: &bool| {
                let c = rgb(if *active { text_primary() } else { text_muted() });
                if let Some(mut t) = w.get_mut::<TextColor>(e) {
                    t.0 = c;
                }
            },
        );
        commands.entity(btn).add_child(text);
        commands.entity(tree_tabs).add_child(btn);
    }

    // The tree pane's whole action vocabulary in one labelled key, sat to the
    // right of the search box: New Folder + Import + the create-asset list, as
    // one menu. Carries `AddMenuBtn` so `add_menu_open` anchors and opens it, plus
    // `TreeAddBtn` so that system knows to prepend the two file actions the narrow
    // layout has no toolbar buttons for.
    // Sized a shade under the standard button: it shares its row with the search
    // box in a pane that can be ~180px wide, so trimming the padding leaves the
    // search box more to work with. Type stays at the standard size — shrinking
    // that too read as a different, smaller class of control. `StyleOwnsPadding`
    // keeps the theme from restoring the standard padding on the first hover.
    let (tree_add, ..) =
        icon_label_button_parts(commands, fonts, "plus", &renzora::lang::t("common.add"));
    commands.entity(tree_add).insert((
        AddMenuBtn,
        TreeAddBtn,
        bevy::ui::RelativeCursorPosition::default(),
        renzora_ember::style::StyleOwnsPadding,
        Name::new("assets-tree-add"),
    ));
    commands.entity(tree_add).entry::<Node>().and_modify(|mut n| {
        n.padding = UiRect::axes(Val::Px(9.0), Val::Px(4.0));
    });

    // Search + Add on one row. The header is display-gated on `narrow`, so the
    // button inherits that gate — the wide layout keeps the toolbar's own
    // Add / Import / New Folder buttons and would only duplicate them here.
    let narrow_header = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                width: Val::Percent(100.0),
                flex_shrink: 0.0,
                padding: UiRect::all(Val::Px(6.0)),
                column_gap: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::header_bg())),
            Name::new("assets-narrow-header"),
        ))
        .id();
    commands.entity(narrow_header).add_children(&[tree_search_input, tree_add]);
    bind_display(commands, narrow_header, |w| {
        w.get_resource::<NativeAssets>().is_some_and(|s| s.narrow)
    });

    commands
        .entity(tree_pane)
        .add_children(&[narrow_header, tree_tabs, tree_scroll]);
    bind_with(
        commands,
        tree_pane,
        |w| {
            let s = w.get_resource::<NativeAssets>();
            (s.map(|s| s.narrow).unwrap_or(false), s.map(|s| s.tree_width).unwrap_or(180.0))
        },
        |w, e, (narrow, width): &(bool, f32)| {
            if let Some(mut n) = w.get_mut::<Node>(e) {
                // Tree-only mode: the tree fills the panel; otherwise its fixed width.
                n.width = if *narrow { Val::Percent(100.0) } else { Val::Px(*width) };
            }
        },
    );

    // Draggable divider (highlights on hover/drag).
    let splitter = commands
        .spawn((
            Node {
                width: Val::Px(4.0),
                height: Val::Percent(100.0),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::border())),
            Interaction::default(),
            Splitter,
            Name::new("assets-splitter"),
        ))
        .id();
    bind_bg(commands, splitter, move |w| {
        let dragging = w.get_resource::<NativeAssets>().is_some_and(|s| s.divider_drag.is_some());
        let hovered = matches!(
            w.get::<Interaction>(splitter),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        );
        if dragging || hovered {
            rgb(accent())
        } else {
            rgb(renzora_ember::theme::border())
        }
    });

    // ── Content (toolbar + search + grid + footer) ──
    let content = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::panel_bg())),
        ))
        .id();

    // Toolbar. Buttons no longer shrink (they'd deform), so wrapping is the
    // last-resort escape valve: below roughly 400px of content width even the
    // icon-only row can't fit, and a second line beats clipping controls out of
    // reach. At every normal width the row stays single-line.
    let toolbar = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                width: Val::Percent(100.0),
                column_gap: Val::Px(6.0),
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(6.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(header_surface()),
        ))
        .id();
    let back = commands
        .spawn((
            Node {
                width: Val::Px(22.0),
                height: Val::Px(20.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            AssetBack,
            Name::new("assets-back"),
        ))
        .id();
    let back_icon = icon_text(commands, &fonts.phosphor, "arrow-left", text_primary(), 13.0);
    commands.entity(back).add_child(back_icon);
    // Bare on the crumb strip, lit only on hover. A constant `hover_bg` chip
    // would be all but invisible against the lightened strip, and a permanent
    // frame here is exactly the card the row just lost.
    bind_bg(commands, back, move |w| match w.get::<Interaction>(back) {
        Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(renzora_ember::theme::selection()),
        _ => Color::NONE,
    });
    // Collapsed at the project root (nowhere to go up to) — fully removed from
    // layout via `Display::None` rather than just hidden, so it leaves no empty
    // gap before the path.
    bind_display(commands, back, |w| current_folder(w) != project_root(w));

    let new_folder = toolbar_action(commands, fonts, "folder-plus", &renzora::lang::t("assets.new_folder"));
    commands.entity(new_folder).insert(NewAssetBtn(NewAsset::Folder));
    let add = toolbar_action(commands, fonts, "plus", &renzora::lang::t("common.add"));
    commands
        .entity(add)
        .insert((AddMenuBtn, bevy::ui::RelativeCursorPosition::default()));
    let import = toolbar_action(commands, fonts, "download-simple", &renzora::lang::t("assets.import"));
    // `RelativeCursorPosition` lets `import_click` anchor its Files/Folder menu
    // to the button's own box instead of the cursor, same as the Add button.
    commands
        .entity(import)
        .insert((ImportBtn, bevy::ui::RelativeCursorPosition::default()));

    // Sort dropdown (opens a screen_menu of sort modes + direction).
    let sort_btn = toolbar_action(commands, fonts, "sort-ascending", &renzora::lang::t("assets.sort"));
    commands
        .entity(sort_btn)
        .insert((SortMenuBtn, bevy::ui::RelativeCursorPosition::default()));

    // View toggle (grid <-> list); icon reflects the view a click switches *to*.
    let view_icon = icon_text(commands, &fonts.phosphor, "list", text_primary(), 15.0);
    bind_with(
        commands,
        view_icon,
        |w| w.get_resource::<NativeAssets>().map(|s| s.list_view).unwrap_or(false),
        |w, e, list_view: &bool| {
            let name = if *list_view { "squares-four" } else { "list" };
            if let (Some(ch), Some(mut t)) = (renzora_ember::font::icon_glyph(name), w.get_mut::<Text>(e)) {
                t.0 = ch.to_string();
            }
        },
    );
    let view_btn = commands
        .spawn((
            Node {
                width: Val::Px(24.0),
                height: Val::Px(22.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::hover_bg())),
            Interaction::default(),
            ViewToggleBtn,
            Name::new("assets-view-toggle"),
        ))
        .id();
    commands.entity(view_btn).add_child(view_icon);

    // Breadcrumb path, on its own row under the action buttons with the back
    // button to its left. Deliberately unframed — it sits directly on the header
    // band rather than in an inset card, so the path reads as a location label
    // and not as another control competing with the buttons above it.
    let crumbs = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(2.0),
            flex_shrink: 1.0,
            min_width: Val::Px(0.0),
            overflow: Overflow::clip(),
            ..default()
        })
        .id();
    keyed_list(commands, crumbs, crumb_snapshot);

    let spacer = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();

    // Zoom control (maps 0..1 → 0.5..1.5 tile scale) in a small framed box with
    // a magnifier glyph.
    let zoom = slider(commands, 0.5);
    bind_2way(
        commands,
        zoom,
        |w| w.get_resource::<NativeAssets>().map(|s| (s.zoom - 0.5).clamp(0.0, 1.0)).unwrap_or(0.5),
        |w, v| {
            if let Some(mut s) = w.get_resource_mut::<NativeAssets>() {
                s.zoom = 0.5 + *v;
            }
        },
    );
    commands.entity(zoom).insert(Node {
        width: Val::Px(70.0),
        height: Val::Px(14.0),
        position_type: PositionType::Relative,
        align_items: AlignItems::Center,
        ..default()
    });
    let zoom_icon = icon_text(commands, &fonts.phosphor, "magnifying-glass", text_muted(), 11.0);
    let zoom_box = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::row_even())),
            BorderColor::all(rgb(renzora_ember::theme::border())),
            Name::new("zoom-box"),
        ))
        .id();
    commands.entity(zoom_box).add_children(&[zoom_icon, zoom]);
    // Widest control in the row, but the *only* way to resize tiles — so it
    // slims rather than disappears when space runs short: the magnifier glyph
    // goes and the track halves, keeping the slider usable.
    bind_display(commands, zoom_icon, |w| !is_compact(w));
    bind_with(commands, zoom, is_compact, |w, e, compact: &bool| {
        if let Some(mut n) = w.get_mut::<Node>(e) {
            n.width = Val::Px(if *compact { 44.0 } else { 70.0 });
        }
    });

    // Compact search field, placed just left of the zoom control.
    let search = text_input(commands, &fonts.ui, &renzora::lang::t("common.search"), "");
    commands.entity(search).insert((
        AssetSearch,
        Node {
            width: Val::Px(160.0),
            // The row's designated shock absorber: it yields width faster than
            // the breadcrumb (shrink 1) so the path stays readable as the panel
            // narrows, down to a floor that still fits a few characters.
            flex_shrink: 3.0,
            min_width: Val::Px(60.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
    ));

    // Grid (also hosts list-view rows: a 100%-wide row wraps to its own line, so
    // the same wrapping container stacks them vertically). `update_grid_layout`
    // retunes the gaps/padding per view.
    let grid = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                width: Val::Percent(100.0),
                align_items: AlignItems::FlexStart,
                align_content: AlignContent::FlexStart,
                column_gap: Val::Px(10.0),
                row_gap: Val::Px(12.0),
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            AssetGrid,
        ))
        .id();
    // Virtualized: only the tiles in (or near) the viewport are built, so a
    // folder of hundreds of split meshes stays cheap. `grid_snapshot` returns the
    // full list; the versioned form also skips re-hashing every entry on frames
    // where neither the listing nor the scroll window changed (see `grid_token`).
    virtual_scroll_versioned(commands, grid, 6, grid_token, grid_snapshot);
    let grid_scroll = scroll_view(commands, grid);
    // Mark the grid viewport so the marquee knows when a press lands in empty
    // grid space (vs. on a tile or the tree).
    commands.entity(grid_scroll).insert((GridArea, bevy::ui::RelativeCursorPosition::default()));

    // Live item count — trails the breadcrumb path on the crumb row.
    let count = commands
        .spawn((
            Text::new("0 items"),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
            Node { margin: UiRect::left(Val::Px(4.0)), flex_shrink: 0.0, ..default() },
        ))
        .id();
    bind_with(
        commands,
        count,
        |w| list_entries(w).len(),
        |w, e, n: &usize| {
            if let Some(mut t) = w.get_mut::<Text>(e) {
                let s = if *n == 1 {
                    "1 item".to_string()
                } else {
                    format!("{n} items")
                };
                if t.0 != s {
                    t.0 = s;
                }
            }
        },
    );
    // Crumb row: its own line beneath the toolbar, so the path gets the panel's
    // full width instead of competing with the actions for a share of one row.
    // Back button, path and count sit straight on the header band — no card.
    let crumb_row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                width: Val::Percent(100.0),
                column_gap: Val::Px(6.0),
                padding: UiRect::new(Val::Px(6.0), Val::Px(6.0), Val::Px(5.0), Val::Px(6.0)),
                // Hairline rules top and bottom: they're what actually separate
                // this strip from the buttons above and the grid below, since a
                // shade step on its own read as one flat header.
                border: UiRect::vertical(Val::Px(1.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(crumb_surface()),
            BorderColor::all(rgb(renzora_ember::theme::border())),
            Name::new("assets-crumb-row"),
        ))
        .id();
    commands.entity(crumb_row).add_children(&[back, crumbs, count]);

    // Toolbar row: action buttons | spacer | sort/view/search/zoom.
    commands.entity(toolbar).add_children(&[
        add,
        import,
        new_folder,
        spacer,
        sort_btn,
        view_btn,
        search,
        zoom_box,
    ]);

    commands.entity(content).add_children(&[crumb_row, grid_scroll]);

    // The toolbar spans the whole panel, above the tree as well as the grid —
    // so `root` is a column of [toolbar, body] and the tree|splitter|grid row is
    // `body`. It used to be the grid column's first child, which made the
    // panel's one bar of actions start halfway across and left a 180px notch of
    // empty header above the folder tree.
    let body = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Row,
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                ..default()
            },
            Name::new("assets-body"),
        ))
        .id();
    commands.entity(body).add_children(&[tree_pane, splitter, content]);

    // Responsive: when the panel is too narrow, hide the grid + splitter so the
    // tree fills it as a file browser (see `responsive_layout` / `narrow`).
    // The toolbar goes with them now that it is no longer inside `content` —
    // the narrow layout has its own header in the tree pane (`narrow_header`),
    // and showing both would be two search boxes and two Add buttons.
    bind_display(commands, content, |w| !w.get_resource::<NativeAssets>().is_some_and(|s| s.narrow));
    bind_display(commands, splitter, |w| !w.get_resource::<NativeAssets>().is_some_and(|s| s.narrow));
    bind_display(commands, toolbar, |w| !w.get_resource::<NativeAssets>().is_some_and(|s| s.narrow));

    commands.entity(root).add_children(&[toolbar, body]);

    // Drop-to-import highlight — an accent-bordered overlay shown only while an
    // OS file drag hovers the window (`FileDragHovering`, set by the importer).
    // Absolute + `Pickable::IGNORE` so it covers the panel without disturbing the
    // tree|content flex layout or intercepting clicks. `bind_display` keeps it
    // `Display::None` (and thus inert) whenever no drag is in progress.
    let drop_hl = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(rgb(accent()).with_alpha(0.10)),
            BorderColor::all(rgb(accent())),
            GlobalZIndex(500),
            Pickable::IGNORE,
            Name::new("assets-drop-highlight"),
        ))
        .id();
    bind_display(commands, drop_hl, |w| {
        w.get_resource::<renzora::core::FileDragHovering>().is_some_and(|h| h.0)
    });
    let pill = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::axes(Val::Px(14.0), Val::Px(9.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            Pickable::IGNORE,
        ))
        .id();
    let pill_ic = icon_text(commands, &fonts.phosphor, "download-simple", accent(), 18.0);
    let pill_tx = commands
        .spawn((Text::new("Drop to import".to_string()), ui_font(&fonts.ui, 13.0), TextColor(rgb(text_primary()))))
        .id();
    commands.entity(pill).add_children(&[pill_ic, pill_tx]);
    commands.entity(drop_hl).add_child(pill);
    commands.entity(root).add_child(drop_hl);

    root
}

/// Clickable breadcrumb segments (project root + each path component).
fn crumb_snapshot(world: &Rx) -> KeyedSnapshot {
    let Some(root) = project_root(world) else {
        return KeyedSnapshot {
            items: Vec::new(),
            build: Box::new(|_, _, _| Entity::PLACEHOLDER),
        };
    };
    let cur = current_folder(world).unwrap_or_else(|| root.clone());
    let root_name = root
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| renzora::lang::t("common.project"));
    let mut segs: Vec<(String, PathBuf)> = vec![(root_name, root.clone())];
    if let Ok(rel) = cur.strip_prefix(&root) {
        let mut acc = root.clone();
        for comp in rel.components() {
            acc = acc.join(comp);
            segs.push((comp.as_os_str().to_string_lossy().to_string(), acc.clone()));
        }
    }
    let items: Vec<(u64, u64)> = segs
        .iter()
        .enumerate()
        .map(|(i, (name, path))| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            (i, path).hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            name.hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    let last = segs.len().saturating_sub(1);
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| crumb_seg(c, f, i, &segs[i].0, &segs[i].1, i == last)),
    }
}

fn crumb_seg(
    commands: &mut Commands,
    fonts: &EmberFonts,
    idx: usize,
    name: &str,
    path: &Path,
    is_current: bool,
) -> Entity {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(2.0),
            flex_shrink: 0.0,
            ..default()
        })
        .id();
    let mut kids = Vec::new();
    if idx > 0 {
        kids.push(icon_text(commands, &fonts.phosphor, "caret-right", text_muted(), 9.0));
    }
    // Each segment is a padded, clickable chip: hand cursor + a hover wash so it
    // clearly reads as navigable. The current (last) folder is emphasized and
    // left un-lit on hover since clicking it is a no-op.
    let chip = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(5.0), Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            CrumbNav(path.to_path_buf()),
            Name::new("crumb"),
        ))
        .id();
    if !is_current {
        bind_bg(commands, chip, move |w| match w.get::<Interaction>(chip) {
            Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(renzora_ember::theme::hover_bg()),
            _ => Color::NONE,
        });
    }
    let color = if is_current { text_primary() } else { text_muted() };
    let label = commands
        .spawn((
            Text::new(name.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(color)),
            bevy::text::TextLayout::no_wrap(),
        ))
        .id();
    commands.entity(chip).add_child(label);
    kids.push(chip);
    commands.entity(row).add_children(&kids);
    row
}
