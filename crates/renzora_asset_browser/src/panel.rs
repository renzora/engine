//! The panel tree itself: `build` assembles the folder-tree pane (with its list
//! switch), the splitter, and the content column of toolbar, grid and footer,
//! and wires every reactive binding between them. The breadcrumb the toolbar
//! carries is built in [`crate::breadcrumb`].

use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{
    bind_2way, bind_bg, bind_display, bind_text, bind_with, keyed_list, keyed_list_tokened,
};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::{accent, panel_bg, popup_bg, rgb, text_muted, text_primary};
use renzora_ember::virtual_scroll::virtual_scroll_versioned;
use renzora_ember::widgets::{
    icon_label_button_collapsing_parts, scroll_view, slider, text_input, EmberTextInput,
};
use renzora_ember::UiInteraction;

use crate::breadcrumb::{crumb_snapshot, CRUMB_GAP};
use crate::grid::{grid_snapshot, grid_token, list_entries};
use crate::layout::{is_compact, toolbar_action};
use crate::ops::{current_folder, project_root};
use crate::state::{
    AddMenuBtn, AssetBack, AssetGrid, AssetRoot, AssetSearch, CrumbBar, CrumbSpacer, GridArea,
    NativeAssets, NewAsset, NewAssetBtn, SortMenuBtn, Splitter, TreeAddBtn, TreeArea, TreeSearch,
    ViewToggleBtn,
};
use crate::tree::{tree_snapshot, tree_token};

/// The toolbar's height, and the height of the controls in it.
const TOOLBAR_H: f32 = 40.0;
const CONTROL_H: f32 = 28.0;

/// The footer's height.
const FOOTER_H: f32 = 24.0;

pub(crate) fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                // A column of one: `body` (the tree|splitter|grid row). Kept a
                // column rather than collapsed into `body` so the drop
                // highlight and any future panel-wide banner have somewhere to
                // sit above it.
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
    // So a press in the tree's empty space (below the last row) starts a
    // marquee, as it does in the grid.
    commands
        .entity(tree_scroll)
        .insert((TreeArea, bevy::ui::RelativeCursorPosition::default()));
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
            // The hierarchy panel's base, so the two tree lists sit on the same
            // surface.
            BackgroundColor(rgb(panel_bg())),
        ))
        .id();
    // ── Tree-pane actions ──
    // When the panel collapses to the tree-only file browser the content column
    // (with the toolbar's Import, New Folder and search) is hidden with the grid,
    // so the browser would lose all three. They come back here as one row: a
    // search box of its own and the same Import button the toolbar carries,
    // whose overlay also holds New Folder and the create list.

    // Search box: takes whatever width the Import button beside it leaves,
    // with its own marker + state field so it can't fight the hidden toolbar
    // search (see `TreeSearch`).
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
    let tree_import = import_button(commands, fonts);

    // Search + Import on one row. The header is display-gated on `narrow`, so
    // the button inherits that gate: the wide layout has the toolbar's own.
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
    commands.entity(narrow_header).add_children(&[tree_search_input, tree_import]);
    bind_display(commands, narrow_header, |w| {
        w.get_resource::<NativeAssets>().is_some_and(|s| s.narrow)
    });

    // The list label and its switch, in both layouts.
    let list_header = crate::list_switch::build_list_header(commands, fonts);

    commands
        .entity(tree_pane)
        .add_children(&[narrow_header, list_header, tree_scroll]);
    bind_with(
        commands,
        tree_pane,
        |w| {
            let s = w.get_resource::<NativeAssets>();
            (s.map(|s| s.narrow).unwrap_or(false), s.map(|s| s.tree_width).unwrap_or(240.0))
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
            UiInteraction::default(),
            Splitter,
            Name::new("assets-splitter"),
        ))
        .id();
    bind_bg(commands, splitter, move |w| {
        let dragging = w.get_resource::<NativeAssets>().is_some_and(|s| s.divider_drag.is_some());
        let hovered = matches!(
            w.get::<UiInteraction>(splitter),
            Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed)
        );
        if dragging || hovered {
            rgb(accent())
        } else {
            rgb(renzora_ember::theme::border())
        }
    });

    // ── Content (toolbar + grid + footer) ──
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

    let toolbar = build_toolbar(commands, fonts);

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
                column_gap: Val::Px(4.0),
                row_gap: Val::Px(4.0),
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

    let footer = build_footer(commands, fonts);

    commands.entity(content).add_children(&[toolbar, grid_scroll, footer]);

    // The toolbar belongs to the content column, not to the whole panel: every
    // button on it acts on the grid (import into this folder, make a folder in
    // it, search it), and the tree runs the panel's full height beside it.
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
    // Hiding the content hides the toolbar with it; the narrow layout has its
    // own header in the tree pane (`narrow_header`).
    bind_display(commands, content, |w| !w.get_resource::<NativeAssets>().is_some_and(|s| s.narrow));
    bind_display(commands, splitter, |w| !w.get_resource::<NativeAssets>().is_some_and(|s| s.narrow));

    commands.entity(root).add_child(body);

    // Drop-to-import highlight: an accent-bordered overlay shown only while an
    // OS file drag hovers the window (`FileDragHovering`, set by the importer).
    // Absolute + `Pickable::IGNORE` so it covers the panel without disturbing the
    // tree|content flex layout or intercepting clicks. `bind_display` keeps it
    // `Display::None` (and thus inert) whenever no drag is in progress.
    // A border and a label, not a wash.
    //
    // It used to tint the whole panel and centre the pill over it, which was
    // fine when the only possible target was "the project" and there was
    // nothing to aim at. Now that a drop lands wherever the cursor is, the
    // panel *is* the aiming surface: covering the tree and the breadcrumbs with
    // an accent fill hides the very folders you are trying to hit. So the fill
    // is gone, the border stays to say the panel will accept the drop, and the
    // label moves to the top edge where it never sits over a drop target.
    let drop_hl = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                align_items: AlignItems::Start,
                justify_content: JustifyContent::Center,
                padding: UiRect::top(Val::Px(6.0)),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            },
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
                padding: UiRect::axes(Val::Px(11.0), Val::Px(5.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            Pickable::IGNORE,
        ))
        .id();
    let pill_ic = icon_text(commands, &fonts.phosphor, "download-simple", accent(), 14.0);
    // Names the folder the drop will land in, rather than saying "import" and
    // leaving you to find out. It reads the same resource the importer targets
    // (`AssetBrowserCwd`), so the label cannot promise one folder and deliver
    // another -- and if a platform gives us no cursor position during an OS
    // drag, the label simply stays on the open folder, which is the truth.
    let pill_tx = commands
        .spawn((Text::new("Drop to import".to_string()), ui_font(&fonts.ui, 11.5), TextColor(rgb(text_primary()))))
        .id();
    bind_text(commands, pill_tx, |w: &Rx| {
        match w.get_resource::<renzora::core::AssetBrowserCwd>().and_then(|c| c.0.clone()) {
            None => "Drop to import".to_string(),
            Some(rel) if rel.is_empty() => "Drop into the project root".to_string(),
            Some(rel) => format!(
                "Drop into {}",
                rel.rsplit('/').next().filter(|s| !s.is_empty()).unwrap_or(&rel)
            ),
        }
    });
    commands.entity(pill).add_children(&[pill_ic, pill_tx]);
    commands.entity(drop_hl).add_child(pill);
    commands.entity(root).add_child(drop_hl);

    root
}

/// The one button for getting assets into the project: a solid accent key that
/// opens the Import overlay, which leads with Files, Folder and Marketplace and
/// lists the asset types you can make below them.
///
/// It replaced an Add button and an Import button side by side. They opened two
/// halves of one question ("where is this asset coming from?"), and the
/// overlay already answered both, so two keys only meant guessing which half
/// you wanted. Solid because it is the panel's primary action; everything else
/// on the toolbar is a ghost button.
fn import_button(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let btn = commands
        .spawn((
            Node {
                height: Val::Px(CONTROL_H),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::horizontal(Val::Px(11.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(rgb(accent())),
            UiInteraction::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            AddMenuBtn,
            TreeAddBtn,
            bevy::ui::RelativeCursorPosition::default(),
            Name::new("assets-import"),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        let lift = match w.get::<UiInteraction>(btn) {
            Some(UiInteraction::Pressed) => 0.18,
            Some(UiInteraction::Hovered) => 0.10,
            _ => 0.0,
        };
        renzora_ember::theme::mix(accent(), (255, 255, 255), lift)
    });
    let on = renzora_ember::theme::on_accent();
    let ic = icon_text(commands, &fonts.phosphor, "download-simple", on, 13.0);
    let label = commands
        .spawn((
            Text::new(renzora::lang::t("assets.import")),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(on)),
            bevy::text::TextLayout::no_wrap(),
        ))
        .id();
    for part in [ic, label] {
        commands.entity(part).insert(Pickable::IGNORE);
    }
    commands.entity(btn).add_children(&[ic, label]);
    btn
}

/// The toolbar: Import and New Folder, then the breadcrumb, then the folder
/// search, sort and view on the right.
///
/// One row, never wrapping. The breadcrumb is the one thing in it that gives
/// way when the panel narrows (by folding levels into "…", see
/// [`crate::breadcrumb`]), with New Folder and Sort dropping their labels at
/// the compact width.
fn build_toolbar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let toolbar = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                width: Val::Percent(100.0),
                height: Val::Px(TOOLBAR_H),
                column_gap: Val::Px(4.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                flex_shrink: 0.0,
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor::all(rgb(renzora_ember::theme::border())),
            Name::new("assets-toolbar"),
        ))
        .id();

    let import = import_button(commands, fonts);
    let new_folder = toolbar_action(commands, fonts, "folder-plus", &renzora::lang::t("assets.new_folder"));
    commands.entity(new_folder).insert(NewAssetBtn(NewAsset::Folder));
    ghost(commands, new_folder);

    let divider = commands
        .spawn((
            Node {
                width: Val::Px(1.0),
                height: Val::Px(18.0),
                flex_shrink: 0.0,
                margin: UiRect::horizontal(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::border())),
        ))
        .id();

    let back = commands
        .spawn((
            Node {
                width: Val::Px(26.0),
                height: Val::Px(26.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            renzora_ember::widgets::HoverTooltip::new(renzora::lang::t_or("assets.back", "Up one folder")),
            AssetBack,
            Name::new("assets-back"),
        ))
        .id();
    let back_icon = icon_text(commands, &fonts.phosphor, "caret-left", text_muted(), 14.0);
    commands.entity(back_icon).insert(Pickable::IGNORE);
    commands.entity(back).add_child(back_icon);
    bind_bg(commands, back, move |w| match w.get::<UiInteraction>(back) {
        Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(renzora_ember::theme::hover_bg()),
        _ => Color::NONE,
    });
    // Collapsed at the project root (nowhere to go up to), fully removed from
    // layout via `Display::None` rather than just hidden, so it leaves no empty
    // gap before the path.
    bind_display(commands, back, |w| current_folder(w) != project_root(w));

    // The breadcrumb: allowed to shrink (its children are not), which is what
    // makes an overflowing path measurable; see `fit_breadcrumb`.
    let crumbs = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(CRUMB_GAP),
                flex_shrink: 1.0,
                min_width: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..default()
            },
            CrumbBar,
            Name::new("assets-crumbs"),
        ))
        .id();
    keyed_list(commands, crumbs, crumb_snapshot);

    let spacer = commands
        .spawn((Node { flex_grow: 1.0, min_width: Val::Px(8.0), ..default() }, CrumbSpacer))
        .id();

    let search = search_box(commands, fonts);

    // Sort: its label is the key the folder is sorted by, so the button says
    // the current order rather than only that one can be chosen.
    let (sort_btn, sort_label) = icon_label_button_collapsing_parts(
        commands,
        fonts,
        "sort-ascending",
        &crate::state::SortMode::Name.label(),
        is_compact,
    );
    commands
        .entity(sort_btn)
        .insert((SortMenuBtn, bevy::ui::RelativeCursorPosition::default()));
    ghost(commands, sort_btn);
    bind_text(commands, sort_label, |w: &Rx| {
        w.get_resource::<NativeAssets>().map(|s| s.sort.label()).unwrap_or_default()
    });

    let grid_btn = view_button(commands, fonts, false);
    let list_btn = view_button(commands, fonts, true);

    commands.entity(toolbar).add_children(&[
        import, new_folder, divider, back, crumbs, spacer, search, sort_btn, grid_btn, list_btn,
    ]);
    toolbar
}

/// Restyle a themed button as a ghost: no fill until hovered. The toolbar has
/// one solid key (Import); filled secondary buttons beside it made four keys
/// of equal weight.
fn ghost(commands: &mut Commands, btn: Entity) {
    commands
        .entity(btn)
        .remove::<renzora_ember::style::Styled>()
        .insert(BackgroundColor(Color::NONE));
    commands.entity(btn).entry::<Node>().and_modify(|mut n| {
        n.height = Val::Px(CONTROL_H);
        n.border_radius = BorderRadius::all(Val::Px(7.0));
    });
    bind_bg(commands, btn, move |w| match w.get::<UiInteraction>(btn) {
        Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(renzora_ember::theme::hover_bg()),
        _ => Color::NONE,
    });
}

/// The search field, scoped to the folder on screen and saying so: the
/// placeholder names the folder ("Search in scripts").
///
/// A row wrapping the input rather than the input alone, so the magnifier can
/// sit inside the field: the frame moves to the wrapper and the input goes
/// transparent inside it. `AssetSearch` stays on the input itself, since
/// `interact.rs` reads the field's `EmberTextInput` through it.
fn search_box(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let field = text_input(commands, &fonts.ui, &renzora::lang::t("common.search"), "");
    commands.entity(field).insert((
        AssetSearch,
        Node {
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::NONE),
        BorderColor::all(Color::NONE),
    ));
    bind_with(
        commands,
        field,
        |w| {
            let name = current_folder(w)
                .map(|p| crate::state::file_name_of(&p))
                .unwrap_or_default();
            renzora::lang::t_or("assets.search_in", "Search in {folder}").replace("{folder}", &name)
        },
        |w, e, placeholder: &String| {
            if let Some(mut inp) = w.get_mut::<EmberTextInput>(e) {
                if inp.placeholder != *placeholder {
                    inp.placeholder = placeholder.clone();
                }
            }
        },
    );
    let icon = icon_text(commands, &fonts.phosphor, "magnifying-glass", text_muted(), 13.0);
    commands.entity(icon).insert(Pickable::IGNORE);
    let wrap = commands
        .spawn((
            Node {
                width: Val::Px(240.0),
                height: Val::Px(CONTROL_H),
                // Gives way before the breadcrumb folds everything, down to a
                // floor that still fits a few characters.
                flex_shrink: 2.0,
                min_width: Val::Px(110.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(7.0),
                padding: UiRect::horizontal(Val::Px(9.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::faint_bg())),
            BorderColor::all(rgb(renzora_ember::theme::border())),
            Name::new("assets-search"),
        ))
        .id();
    commands.entity(wrap).add_children(&[icon, field]);
    wrap
}

/// One of the two view keys (grid, list). The one in use carries a faint wash.
fn view_button(commands: &mut Commands, fonts: &EmberFonts, list: bool) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(CONTROL_H),
                height: Val::Px(CONTROL_H),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            UiInteraction::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            renzora_ember::widgets::HoverTooltip::new(if list {
                renzora::lang::t_or("assets.view.list", "List view")
            } else {
                renzora::lang::t_or("assets.view.grid", "Grid view")
            }),
            ViewToggleBtn { list },
            Name::new("assets-view"),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        let active = w.get_resource::<NativeAssets>().is_some_and(|s| s.list_view == list);
        let hovered = matches!(
            w.get::<UiInteraction>(btn),
            Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed)
        );
        if active {
            Color::srgba(1.0, 1.0, 1.0, 0.07)
        } else if hovered {
            rgb(renzora_ember::theme::hover_bg())
        } else {
            Color::NONE
        }
    });
    let icon = icon_text(commands, &fonts.phosphor, if list { "list" } else { "squares-four" }, text_muted(), 15.0);
    commands.entity(icon).insert(Pickable::IGNORE);
    bind_with(
        commands,
        icon,
        move |w| w.get_resource::<NativeAssets>().is_some_and(|s| s.list_view == list),
        |w, e, active: &bool| {
            if let Some(mut t) = w.get_mut::<TextColor>(e) {
                t.0 = rgb(if *active { text_primary() } else { text_muted() });
            }
        },
    );
    commands.entity(btn).add_child(icon);
    btn
}

/// The footer under the grid: the item count (and how many are selected) on
/// the left, the tile size slider on the right. Fixed to the bottom of the
/// content column, so neither scrolls away with the grid.
fn build_footer(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let footer = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(FOOTER_H),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect::horizontal(Val::Px(10.0)),
                border: UiRect::top(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(rgb(renzora_ember::theme::border())),
            Name::new("assets-footer"),
        ))
        .id();
    let count = commands
        .spawn((
            Text::new(String::new()),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::no_wrap(),
            Node { flex_grow: 1.0, ..default() },
        ))
        .id();
    bind_text(commands, count, |w: &Rx| {
        let n = list_entries(w).len();
        let selected = w.get_resource::<NativeAssets>().map(|s| s.selection.len()).unwrap_or(0);
        let items = if n == 1 {
            renzora::lang::t_or("assets.count.one", "1 item")
        } else {
            renzora::lang::t_or("assets.count.many", "{n} items").replace("{n}", &n.to_string())
        };
        if selected > 0 {
            let sel = renzora::lang::t_or("assets.count.selected", "{n} selected").replace("{n}", &selected.to_string());
            format!("{items} \u{00B7} {sel}")
        } else {
            items
        }
    });

    // Tile size (maps 0..1 → 0.5..1.5 tile scale).
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
        width: Val::Px(80.0),
        height: Val::Px(14.0),
        flex_shrink: 0.0,
        position_type: PositionType::Relative,
        align_items: AlignItems::Center,
        ..default()
    });
    // Size means nothing in the list view, where rows have one height.
    bind_display(commands, zoom, |w| !w.get_resource::<NativeAssets>().is_some_and(|s| s.list_view));
    commands.entity(footer).add_children(&[count, zoom]);
    footer
}
