//! The top bar's hamburger menu and the update chip beside it.
//!
//! One button opens a single dropdown whose rows are the File / Edit / View /
//! Help submenus, so the four titles that used to sit in the bar cost one slot
//! between them. The dropdown is built as a small *panel* rather than a context
//! menu — see [`spacious`] for why the rows are ember's ordinary menu rows with
//! their padding opened up instead of a second set of widgets.
//!
//! The three `reset_*_action` handlers live here because the View submenu is the
//! only thing that calls them.

use bevy::prelude::*;

use renzora_ember::dock::{Dock, DockDirty};
use renzora_ember::font::{glyph, icon_text, ui_font, EmberFonts};
use renzora_ember::theme::{rgb, text_muted, text_primary};

use crate::bottom_dock::BottomDock;
use crate::dock;
use crate::panel_sets::{default_panel_set_name, BottomPanelSets};
use crate::{open_url, ShellLayouts};

/// Register the hamburger's systems.
///
/// Kept here rather than in [`crate::ShellPlugin`] so the menu owns its own
/// wiring: none of the four needs ordering against anything outside this module.
pub(crate) fn register(app: &mut App) {
    app.init_resource::<OpenTopMenu>();
    app.add_systems(
        Update,
        (top_menu_open, top_menu_hover, top_menu_sync, update_chip_click),
    );
}

/// The top bar's "Update available" chip. Shown only while
/// [`renzora::core::UpdateAvailable`] is present; opens the Software Update
/// overlay.
#[derive(Component)]
struct UpdateChipBtn;

// ── Top-bar menus (hamburger → File / Edit / View / Help) ────────────────────

#[derive(Clone, Copy, PartialEq)]
enum TopMenuKind {
    /// The hamburger: one dropdown whose rows are the File/Edit/View/Help
    /// submenus. The four kinds below are no longer top-bar titles of their own
    /// — they only name the item list each submenu is filled with.
    Main,
    File,
    Edit,
    View,
    Help,
}

#[derive(Component)]
struct TopMenu(TopMenuKind);

/// The currently-open top menu (so hovering a sibling switches to it, and a
/// re-click toggles it closed). Cleared by [`top_menu_sync`] once dismissed.
#[derive(Resource, Default)]
struct OpenTopMenu {
    menu: Option<Entity>,
    kind: Option<TopMenuKind>,
}

/// The hamburger that replaced the File/Edit/View/Help titles: one top-bar
/// button opening a single dropdown, with those four now submenu rows inside it.
///
/// It carries a **Menu** label. It was icon-only to give the left zone back to
/// the account name and the notification bell — both of which went with the
/// social features, so the width it was saving is no longer wanted by anything,
/// and four menus collapsed behind a wordless glyph is a lot to ask a new user
/// to guess.
pub(crate) fn hamburger_menu_item(commands: &mut Commands, font: &bevy::text::FontSource) -> Entity {
    let item = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            TopMenu(TopMenuKind::Main),
            Name::new("menu:main"),
        ))
        .id();
    renzora_ember::reactive::tracked::bind_bg(commands, item, move |w| match w.get::<Interaction>(item) {
        Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(renzora_ember::theme::hover_bg()),
        _ => Color::NONE,
    });
    let icon = glyph(commands, "list", text_muted(), 15.0);
    commands.entity(icon).insert(bevy::ui::FocusPolicy::Pass);
    let label = commands
        .spawn((
            Text::new(renzora::lang::t_or("menu.label", "Menu")),
            ui_font(font, 11.0),
            TextColor(rgb(text_muted())),
            bevy::ui::FocusPolicy::Pass,
            bevy::text::TextLayout::no_wrap(),
        ))
        .id();
    commands.entity(item).add_children(&[icon, label]);
    item
}

/// The top bar's "Update available" chip: an accent-tinted pill that appears
/// when an engine update is waiting and opens the Software Update overlay.
///
/// Built unconditionally and hidden reactively rather than spawned on demand:
/// the top bar is assembled once, and a `bind_display` costs nothing next to
/// rebuilding the bar whenever a background check finishes.
pub(crate) fn build_update_chip(commands: &mut Commands, font: &bevy::text::FontSource) -> Entity {
    let chip = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            UpdateChipBtn,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("update-chip"),
        ))
        .id();
    renzora_ember::reactive::tracked::bind_display(commands, chip, |w| {
        w.get_resource::<renzora::core::UpdateAvailable>().is_some()
    });
    renzora_ember::reactive::tracked::bind_bg(commands, chip, move |w| {
        match w.get::<Interaction>(chip) {
            Some(Interaction::Hovered) | Some(Interaction::Pressed) => {
                Color::srgba(0.36, 0.65, 1.0, 0.34)
            }
            _ => Color::srgba(0.36, 0.65, 1.0, 0.20),
        }
    });
    let ic = glyph(commands, "arrow-circle-up", text_primary(), 13.0);
    commands.entity(ic).insert(bevy::ui::FocusPolicy::Pass);
    let label = commands
        .spawn((
            Text::new(String::new()),
            ui_font(font, 11.0),
            TextColor(rgb(text_primary())),
            bevy::ui::FocusPolicy::Pass,
            bevy::text::TextLayout::no_wrap(),
        ))
        .id();
    // Deliberately does not name the version: a bare tag in the top bar reads as
    // the version you're *running*, not one you could move to. The overlay the
    // chip opens spells out which release it is.
    renzora_ember::reactive::tracked::bind_text(commands, label, |w| {
        match w.get_resource::<renzora::core::UpdateAvailable>() {
            Some(_) => renzora::lang::t("menu.help.update_new"),
            None => String::new(),
        }
    });
    commands.entity(chip).add_children(&[ic, label]);
    chip
}

/// Click the update chip → open the Software Update overlay.
fn update_chip_click(
    q: Query<&Interaction, (With<UpdateChipBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.insert_resource(renzora::core::UpdateRequested);
    }
}

/// Spawn a top-menu dropdown anchored at `pos` and return its root.
fn spawn_top_menu(
    commands: &mut Commands,
    fonts: &EmberFonts,
    kind: TopMenuKind,
    pos: Vec2,
    account: Option<&str>,
    update_tag: Option<&str>,
) -> Entity {
    let root = renzora_ember::widgets::screen_menu(commands, pos.x, pos.y);
    // The hamburger's dropdown is a panel, not a context menu: 184px is right
    // for a list of verbs and far too narrow for an identity block with a name
    // and a line of description under it. Only this one menu is widened —
    // `screen_menu`'s default is what every other menu in the editor wants.
    if matches!(kind, TopMenuKind::Main) {
        commands.entity(root).entry::<Node>().and_modify(|mut n| {
            n.min_width = Val::Px(264.0);
            n.padding = UiRect::all(Val::Px(6.0));
            n.border_radius = BorderRadius::all(Val::Px(10.0));
        });
    }
    let kids = build_menu_items(commands, fonts, kind, account, update_tag);
    commands.entity(root).add_children(&kids);
    root
}

/// The signed-in username, if any — read per menu-open so the hamburger's
/// account row shows the current name without a reactive binding.
fn account_name(bridge: &Option<Res<renzora::core::AuthBridge>>) -> Option<String> {
    bridge.as_ref().and_then(|b| b.signed_in_username.clone())
}

/// Click a top-bar title → open its dropdown (anchored under the button), or
/// re-click the open one to close it.
fn top_menu_open(
    q: Query<
        (
            &Interaction,
            &TopMenu,
            &bevy::ui::RelativeCursorPosition,
            &bevy::ui::ComputedNode,
        ),
        Changed<Interaction>,
    >,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    fonts: Option<Res<EmberFonts>>,
    bridge: Option<Res<renzora::core::AuthBridge>>,
    update: Option<Res<renzora::core::UpdateAvailable>>,
    mut open: ResMut<OpenTopMenu>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else {
        return;
    };
    let account = account_name(&bridge);
    let update_tag = update.as_ref().map(|u| u.0.clone());
    for (interaction, menu, rcp, cn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(e) = open.menu.take() {
            commands.entity(e).try_despawn();
        }
        // Re-clicking the already-open menu just closes it.
        if open.kind == Some(menu.0) {
            open.kind = None;
            continue;
        }
        let Some(pos) = anchor_below(&windows, rcp, cn) else {
            open.kind = None;
            continue;
        };
        open.menu = Some(spawn_top_menu(&mut commands, &fonts, menu.0, pos, account.as_deref(), update_tag.as_deref()));
        open.kind = Some(menu.0);
    }
}

/// While a top menu is open, hovering a *different* title switches to it without
/// a click — standard menu-bar behavior.
fn top_menu_hover(
    q: Query<(
        &Interaction,
        &TopMenu,
        &bevy::ui::RelativeCursorPosition,
        &bevy::ui::ComputedNode,
    )>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    fonts: Option<Res<EmberFonts>>,
    bridge: Option<Res<renzora::core::AuthBridge>>,
    update: Option<Res<renzora::core::UpdateAvailable>>,
    mut open: ResMut<OpenTopMenu>,
    mut commands: Commands,
) {
    let Some(open_kind) = open.kind else { return };
    let Some(fonts) = fonts else { return };
    let account = account_name(&bridge);
    let update_tag = update.as_ref().map(|u| u.0.clone());
    for (interaction, menu, rcp, cn) in &q {
        if *interaction == Interaction::Hovered && menu.0 != open_kind {
            if let Some(e) = open.menu.take() {
                commands.entity(e).try_despawn();
            }
            let Some(pos) = anchor_below(&windows, rcp, cn) else {
                open.kind = None;
                return;
            };
            open.menu = Some(spawn_top_menu(&mut commands, &fonts, menu.0, pos, account.as_deref(), update_tag.as_deref()));
            open.kind = Some(menu.0);
            return;
        }
    }
}

/// Forget the open menu once it's been dismissed (click-outside / item click,
/// handled by ember), so the next hover/click starts fresh.
fn top_menu_sync(
    menus: Query<(), With<renzora_ember::widgets::ScreenMenu>>,
    mut open: ResMut<OpenTopMenu>,
) {
    if let Some(e) = open.menu {
        if menus.get(e).is_err() {
            open.menu = None;
            open.kind = None;
        }
    }
}

/// The bottom-left of a node in logical window px, derived from the cursor + the
/// node's normalized cursor position (scale-invariant; avoids UI `GlobalTransform`
/// coordinate ambiguity). Used to anchor button dropdowns just under the button.
fn anchor_below(
    windows: &Query<&Window, With<bevy::window::PrimaryWindow>>,
    rcp: &bevy::ui::RelativeCursorPosition,
    cn: &bevy::ui::ComputedNode,
) -> Option<Vec2> {
    let cursor = windows.iter().next().and_then(|w| w.cursor_position())?;
    let size = cn.size() * cn.inverse_scale_factor();
    let norm = rcp.normalized.unwrap_or(Vec2::ZERO);
    let top_left = cursor - (norm + Vec2::splat(0.5)) * size;
    Some(Vec2::new(top_left.x, top_left.y + size.y + 2.0))
}

/// Build one menu's rows. `account` is the signed-in username (`None` = signed
/// out) — the menu needs the name itself now, not just the fact of being signed
/// in, because the hamburger's first row *is* the username.
/// Open a menu row's padding out to panel proportions.
///
/// The hamburger's dropdown is the app's front door and wants air; every other
/// menu in the editor is a context menu, where tight rows are right and a list
/// of twenty verbs has to fit on screen. So the metrics stay where they are in
/// `renzora_ember` and this widens the handful of rows that want it, rather than
/// fattening every context menu in the editor to change one.
///
/// Separators are skipped rather than forbidden. A `menu_sep` is a 1px-tall
/// node and vertical padding would turn it into a bar, but the lists this walks
/// are built elsewhere and hand back rows and separators mixed together — so the
/// check lives here, where it cannot be forgotten, instead of at each call site.
fn spacious(commands: &mut Commands, row: Entity) -> Entity {
    commands.entity(row).entry::<Node>().and_modify(|mut n| {
        if n.height == Val::Px(1.0) {
            // A separator: give it more air around it, nothing inside it.
            n.margin = UiRect::vertical(Val::Px(5.0));
            return;
        }
        n.padding = UiRect::axes(Val::Px(10.0), Val::Px(7.0));
        n.column_gap = Val::Px(10.0);
        n.border_radius = BorderRadius::all(Val::Px(6.0));
    });
    row
}

/// The identity block at the top of the hamburger menu: a round avatar chip, the
/// account name, and what the account *is* underneath it.
///
/// Not a row — it has no action and no hover. Signing in is a row further down,
/// where it belongs with the other verbs; a header that is sometimes a button is
/// a header you have to test to understand.
fn menu_account_header(
    commands: &mut Commands,
    fonts: &EmberFonts,
    account: Option<&str>,
) -> Entity {
    let block = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(9.0)),
                ..default()
            },
            Name::new("menu-account-header"),
        ))
        .id();

    // A circle with a glyph in it, not an image: the shell has no avatar cache
    // — that lives with the marketplace plugin, which the shell must not depend
    // on. A filled circle reads as an avatar slot either way.
    let avatar = commands
        .spawn((
            Node {
                width: Val::Px(34.0),
                height: Val::Px(34.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(17.0)),
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::hover_bg())),
        ))
        .id();
    let glyph_name = if account.is_some() { "user" } else { "user-circle-dashed" };
    let av_ic = icon_text(commands, &fonts.phosphor, glyph_name, text_muted(), 17.0);
    commands.entity(avatar).add_child(av_ic);

    let text_col = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            row_gap: Val::Px(1.0),
            ..default()
        })
        .id();
    let (title, subtitle) = match account {
        Some(name) => (name.to_string(), "renzora.com account".to_string()),
        None => (
            renzora::lang::t_or("auth.signed_out", "Not signed in"),
            renzora::lang::t_or("auth.signed_out_hint", "Sign in to buy and publish").to_string(),
        ),
    };
    let t = commands
        .spawn((
            Text::new(title),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::no_wrap(),
        ))
        .id();
    let s = commands
        .spawn((
            Text::new(subtitle),
            ui_font(&fonts.ui, 10.5),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::no_wrap(),
        ))
        .id();
    commands.entity(text_col).add_children(&[t, s]);
    commands.entity(block).add_children(&[avatar, text_col]);
    block
}

/// A submenu row for the hamburger's panel, with its floating panel styled to
/// match the dropdown it hangs off.
///
/// Without this the second level was a plain context menu: 184px wide, 6px
/// corners, rows at context-menu pitch, opening off a 264px panel with 10px
/// corners and rows at twice the height. One menu in two visual languages,
/// depending how deep you had gone.
fn panel_submenu(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
    kids: Vec<Entity>,
) -> Entity {
    let (row, content, panel) =
        renzora_ember::widgets::menu_submenu_parts(commands, fonts, icon, label, text_muted());
    commands.entity(panel).entry::<Node>().and_modify(|mut n| {
        n.min_width = Val::Px(230.0);
        n.padding = UiRect::all(Val::Px(6.0));
        n.border_radius = BorderRadius::all(Val::Px(10.0));
    });
    for kid in &kids {
        spacious(commands, *kid);
    }
    commands.entity(content).add_children(&kids);
    spacious(commands, row)
}

fn build_menu_items(
    commands: &mut Commands,
    fonts: &EmberFonts,
    kind: TopMenuKind,
    account: Option<&str>,
    // Release tag of a pending engine update, when `renzora_update`'s background
    // check found one. Read per menu-open like `account`, so Help names the
    // version instead of making you go and look.
    update_tag: Option<&str>,
) -> Vec<Entity> {
    use renzora_ember::widgets::{menu_item, menu_sep};
    match kind {
        // The hamburger's own dropdown: the account, then four submenu rows,
        // each filled by recursing into the item list that used to be its own
        // top-bar title.
        // ── The hamburger's own dropdown ─────────────────────────────────────
        //
        // Built as a small **panel**, not a context menu: an identity block, a
        // way in to search, then the four submenus, then the two things reached
        // often enough to be top-level, then the account action. A context menu
        // is a list of verbs for the thing you right-clicked; this is the app's
        // front door, and it was reading as the former — a wall of nine tight
        // rows with the account buried among them.
        //
        // The rows are ember's ordinary menu rows with their padding opened up
        // (`spacious`), rather than a second set of widgets. Everything about
        // them — hover, click-to-close, submenu hover-open — is behaviour this
        // menu wants unchanged; only the rhythm is different.
        TopMenuKind::Main => {
            let mut rows: Vec<Entity> = Vec::new();

            rows.push(menu_account_header(commands, fonts, account));
            rows.push(menu_sep(commands));

            for (icon, label, sub) in [
                ("file", renzora::lang::t("menu.file"), TopMenuKind::File),
                ("pencil-simple", renzora::lang::t("menu.edit"), TopMenuKind::Edit),
                ("eye", renzora::lang::t("menu.view"), TopMenuKind::View),
                ("question", renzora::lang::t("menu.help"), TopMenuKind::Help),
            ] {
                let kids = build_menu_items(commands, fonts, sub, account, update_tag);
                rows.push(panel_submenu(commands, fonts, icon, &label, kids));
            }

            // Import, Export and Settings are top-level rather than buried at
            // the bottom of File. Settings took the gear button's place when
            // that left the top bar; the other two bracket a project's life and
            // are reached far too often to sit two hovers deep.
            rows.push(menu_sep(commands));
            // Import is a submenu because no OS dialog picks files and folders
            // in one pass — the same reason the Assets panel's Import button
            // opens a two-row menu instead of a picker. See
            // `renzora::core::ImportPick`.
            let import_kids = vec![
                menu_item(commands, fonts, "file", &renzora::lang::t("assets.import_files"), |w| {
                    w.insert_resource(renzora::core::ImportRequested(renzora::core::ImportPick::Files));
                }),
                menu_item(commands, fonts, "folder-open", &renzora::lang::t("assets.import_folder"), |w| {
                    w.insert_resource(renzora::core::ImportRequested(renzora::core::ImportPick::Folder));
                }),
            ];
            rows.push(panel_submenu(
                commands,
                fonts,
                "download-simple",
                &renzora::lang::t("assets.import"),
                import_kids,
            ));
            let export = menu_item(
                commands,
                fonts,
                "package",
                &renzora::lang::t("menu.file.export_project"),
                |w| {
                    w.insert_resource(renzora::core::ExportRequested);
                },
            );
            rows.push(spacious(commands, export));
            let settings = menu_item(
                commands,
                fonts,
                "gear",
                &renzora::lang::t("common.settings"),
                |w| {
                    if let Some(mut s) =
                        w.get_resource_mut::<renzora_editor_framework::EditorSettings>()
                    {
                        s.show_settings = !s.show_settings;
                    }
                },
            );
            rows.push(spacious(commands, settings));

            // The account actions last, on their own — the reference's Log Out
            // position, and the right one: they are the only rows here that end
            // a session rather than start a task.
            rows.push(menu_sep(commands));
            if account.is_some() {
                let library = menu_item(commands, fonts, "books", &renzora::lang::t("menu.account.my_library"), |w| {
                    if let Some(mut dock) = w.get_resource_mut::<Dock>() {
                        dock.tree.focus_or_add_panel("hub_library");
                    }
                    if let Some(mut d) = w.get_resource_mut::<DockDirty>() {
                        d.0 = true;
                    }
                });
                rows.push(spacious(commands, library));
                let out = menu_item(commands, fonts, "sign-out", &renzora::lang::t("auth.sign_out"), |w| {
                    w.insert_resource(renzora::core::AuthSignOutRequest);
                });
                rows.push(spacious(commands, out));
            } else {
                let sign_in = menu_item(commands, fonts, "sign-in", &renzora::lang::t("auth.sign_in"), |w| {
                    w.insert_resource(renzora::core::AuthToggleWindowRequest);
                });
                rows.push(spacious(commands, sign_in));
            }
            rows
        }
        TopMenuKind::File => vec![
            menu_item(commands, fonts, "folder-plus", &renzora::lang::t("menu.file.new_project"), |w| {
                renzora_editor_framework::handle_new_project(w)
            }),
            menu_item(commands, fonts, "folder-open", &renzora::lang::t("menu.file.open_project"), |w| {
                renzora_editor_framework::handle_open_project(w)
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "file-plus", &renzora::lang::t("menu.file.new_scene"), |w| {
                w.insert_resource(renzora::core::NewSceneRequested);
            }),
            menu_item(commands, fonts, "file", &renzora::lang::t("menu.file.open_scene"), |w| {
                w.insert_resource(renzora::core::OpenSceneRequested);
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "floppy-disk", &renzora::lang::t("common.save"), |w| {
                w.insert_resource(renzora::core::SaveSceneRequested);
            }),
            menu_item(commands, fonts, "floppy-disk-back", &renzora::lang::t_or("menu.file.save_as", "Save As…"), |w| {
                w.insert_resource(renzora::core::SaveAsSceneRequested);
            }),
            menu_sep(commands),
            // Same request the asset panel's Import button fires; renzora_import_ui
            // picks it up and opens the matching picker, then the import overlay.
            // No ImportTargetDir here, so assets land in the importer's default
            // folder. Two rows because no OS dialog picks files and folders at
            // once — see `renzora::core::ImportPick`.
            menu_item(commands, fonts, "file", &renzora::lang::t("assets.import_files"), |w| {
                w.insert_resource(renzora::core::ImportRequested(renzora::core::ImportPick::Files));
            }),
            menu_item(commands, fonts, "folder-open", &renzora::lang::t("assets.import_folder"), |w| {
                w.insert_resource(renzora::core::ImportRequested(renzora::core::ImportPick::Folder));
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "plug", &renzora::lang::t_or("menu.file.install_plugin", "Install Plugin…"), |w| {
                crate::plugin_install::open_install_dialog(w)
            }),
        ],
        TopMenuKind::Edit => vec![
            menu_item(commands, fonts, "arrow-u-up-left", &renzora::lang::t("common.undo"), |w| {
                let f = w.get_resource::<renzora_editor_framework::EditorActionHooks>().and_then(|h| h.undo);
                if let Some(f) = f {
                    f(w);
                }
            }),
            menu_item(commands, fonts, "arrow-u-up-right", &renzora::lang::t("common.redo"), |w| {
                let f = w.get_resource::<renzora_editor_framework::EditorActionHooks>().and_then(|h| h.redo);
                if let Some(f) = f {
                    f(w);
                }
            }),
        ],
        TopMenuKind::View => vec![
            menu_item(commands, fonts, "magnifying-glass-plus", &renzora::lang::t_or("menu.view.zoom_in", "Zoom In"), |w| {
                w.insert_resource(renzora::core::CameraViewRequest::ZoomIn);
            }),
            menu_item(commands, fonts, "magnifying-glass-minus", &renzora::lang::t_or("menu.view.zoom_out", "Zoom Out"), |w| {
                w.insert_resource(renzora::core::CameraViewRequest::ZoomOut);
            }),
            menu_item(commands, fonts, "magnifying-glass", &renzora::lang::t_or("menu.view.reset_zoom", "Reset Zoom"), |w| {
                w.insert_resource(renzora::core::CameraViewRequest::ResetZoom);
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "corners-out", &renzora::lang::t_or("menu.view.fit_all", "Fit All"), |w| {
                w.insert_resource(renzora::core::CameraViewRequest::FrameAll);
            }),
            menu_item(commands, fonts, "eye", &renzora::lang::t_or("menu.view.isolation_mode", "Isolation Mode"), |w| {
                let mut iso = w
                    .remove_resource::<renzora::core::IsolationMode>()
                    .unwrap_or_default();
                iso.active = !iso.active;
                w.insert_resource(iso);
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "layout", &renzora::lang::t("menu.window.reset_layout"), reset_layout_action),
            menu_item(commands, fonts, "browsers", &renzora::lang::t_or("menu.view.reset_workspace", "Reset Workspace"), reset_workspace_action),
            menu_item(commands, fonts, "rows", &renzora::lang::t_or("menu.view.reset_global_docks", "Reset Global Docks"), reset_global_docks_action),
        ],
        TopMenuKind::Help => vec![
            menu_item(commands, fonts, "graduation-cap", &renzora::lang::t_or("menu.help.tutorial", "Getting Started Tutorial"), |w| {
                w.insert_resource(renzora::core::TutorialRequested);
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "book-open", &renzora::lang::t("menu.help.documentation"), |_| {
                open_url("https://renzora.com/docs")
            }),
            menu_item(commands, fonts, "youtube-logo", &renzora::lang::t("menu.help.youtube"), |_| {
                open_url("https://youtube.com/@renzoragame")
            }),
            menu_item(commands, fonts, "discord-logo", &renzora::lang::t("menu.help.discord"), |_| {
                open_url("https://discord.gg/9UHUGUyDJv")
            }),
            menu_item(commands, fonts, "github-logo", &renzora::lang::t_or("menu.help.github", "GitHub"), |_| {
                open_url("https://github.com/renzora/engine")
            }),
            menu_sep(commands),
            // Names the pending version when there is one, so "am I out of
            // date?" is answered by the menu rather than by opening a dialog to
            // find out.
            menu_item(
                commands,
                fonts,
                "download-simple",
                &match update_tag {
                    Some(tag) => format!("{} {tag}", renzora::lang::t("menu.help.update_to")),
                    None => renzora::lang::t("menu.help.check_updates"),
                },
                |w| {
                    w.insert_resource(renzora::core::UpdateRequested);
                },
            ),
            menu_item(commands, fonts, "info", &renzora::lang::t_or("menu.help.about_engine", "About Renzora Engine"), |w| {
                w.insert_resource(crate::about::ShowAboutRequested);
            }),
        ],
    }
}

/// Reset the active workspace's dock tree to the **engine default** for that
/// workspace. The stored `ShellLayouts` entry holds the user's *edited* layout
/// (persisted to `~/.renzora/layout.json`), so resetting to it was a no-op —
/// we pull the pristine tree from [`dock::workspace_layouts`] instead, matched
/// by the active workspace's name, and overwrite both the live dock and the
/// stored layout so the reset sticks (and gets persisted).
///
/// Deliberately leaves the global bottom panel alone. It is not part of any
/// workspace ([`dock::scene_layout`]), so resetting a workspace has nothing to
/// say about it — see [`reset_global_docks_action`], which is the only thing
/// that does.
fn reset_layout_action(w: &mut World) {
    let active_name = w
        .get_resource::<ShellLayouts>()
        .and_then(|l| l.layouts.get(l.active).map(|(name, _)| name.clone()));
    let Some(active_name) = active_name else {
        return;
    };
    let Some(default_tree) = dock::workspace_layouts()
        .into_iter()
        .find(|(name, _)| *name == active_name)
        .map(|(_, t)| t)
    else {
        return;
    };
    if let Some(mut layouts) = w.get_resource_mut::<ShellLayouts>() {
        let active = layouts.active;
        if let Some(slot) = layouts.layouts.get_mut(active) {
            slot.1 = default_tree.clone();
        }
    }
    if let Some(mut dock) = w.get_resource_mut::<Dock>() {
        dock.tree = default_tree;
    }
    if let Some(mut d) = w.get_resource_mut::<DockDirty>() {
        d.0 = true;
    }
}

/// Reset the entire workspace ribbon to the engine defaults: discard any
/// user-added / removed / renamed / reordered workspaces and restore each
/// default workspace's pristine dock tree. Where [`reset_layout_action`] resets
/// only the active workspace's layout, this rebuilds the whole set (active back
/// to the first default), then flags a rebuild so the change persists.
///
/// The global bottom panel survives untouched, tab sets and all. It belongs to
/// the editor, not to a workspace, so someone restoring the shipped Scene /
/// Scripting / Debug arrangement has not asked to lose the panel set they built
/// alongside it. [`reset_global_docks_action`] is the separate, explicit way to
/// reset that.
fn reset_workspace_action(w: &mut World) {
    let defaults = dock::workspace_layouts();
    let Some(active_tree) = defaults.first().map(|(_, t)| t.clone()) else {
        return;
    };
    if let Some(mut layouts) = w.get_resource_mut::<ShellLayouts>() {
        layouts.layouts = defaults;
        layouts.active = 0;
    }
    if let Some(mut dock) = w.get_resource_mut::<Dock>() {
        dock.tree = active_tree;
    }
    if let Some(mut d) = w.get_resource_mut::<DockDirty>() {
        d.0 = true;
    }
}

/// Reset the global bottom panel: one set, named the default, holding
/// [`dock::DEFAULT_BOTTOM_TABS`], at the default height, opened.
///
/// This is the counterpart to the two workspace resets above — the panel is
/// global, so neither of them touches it and it needs a way back of its own.
/// It is also the escape hatch when the panel has been emptied *and* collapsed:
/// the collapsed strip stands in that state now (see
/// [`crate::sync_collapsed_bottom_bar`]), but a user who has already lost it on
/// an older build needs one menu item that puts everything back.
///
/// Every set goes, not just the live one. "Reset" that left three
/// user-made sets in place would be a partial reset in the one direction that
/// matters: the panels the user is complaining about not seeing may be in any
/// of them. It opens the panel too, so the reset is visible rather than
/// something that has happened behind a closed strip.
fn reset_global_docks_action(w: &mut World) {
    let tree = dock::default_bottom_tree();
    if let Some(mut fixed) = w.get_resource_mut::<renzora_ember::dock::FixedDock>() {
        fixed.tree = tree.clone();
        fixed.dirty = true;
    }
    if let Some(mut sets) = w.get_resource_mut::<BottomPanelSets>() {
        sets.sets = vec![(default_panel_set_name(), tree)];
        sets.active = 0;
    }
    if let Some(mut bottom) = w.get_resource_mut::<BottomDock>() {
        bottom.height = dock::BOTTOM_DOCK_HEIGHT;
        bottom.mode = dock::BottomDockMode::default();
        bottom.open = true;
    }
}
