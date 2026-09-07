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
use renzora_ember::reactive::Rx;
use renzora_ember::theme::{accent, divider, rgb, text_muted, text_primary, window_bg};

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
        (
            top_menu_open,
            top_menu_hover,
            top_menu_sync,
            update_chip_click,
            brand_mark_click,
        ),
    );
}

/// The Renzora mark at the head of the top bar; opens the About overlay.
#[derive(Component)]
struct BrandMarkBtn;

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

/// The Renzora mark that opens the top bar: the real icon, with a glyph standing
/// in where the file is not there. Clicking it opens About.
///
/// Loaded through ember's on-disk image cache rather than the `AssetServer`,
/// because the asset root is the open *project*'s and this is the engine's own
/// mark. The icon is staged beside the executable as `resources/icon.png` (see
/// `xtask`'s staging step), so it is present in every staged and downloaded
/// build and absent from a bare `cargo run` — which is what the glyph is for.
/// The splash's title bar does the same thing for the same reasons; see
/// `renzora_splash`'s `build_mark`.
///
/// About was reachable only from Help, two hovers deep in a menu whose other
/// rows are documentation links. A product's mark is where people already look
/// for what version they are running.
pub(crate) fn brand_mark(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    const MARK: f32 = 18.0;

    let button = commands
        .spawn((
            Node {
                width: Val::Px(MARK + 10.0),
                height: Val::Px(MARK + 6.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            BrandMarkBtn,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("brand-mark"),
        ))
        .id();
    renzora_ember::reactive::tracked::bind_bg(commands, button, move |w| {
        match w.get::<Interaction>(button) {
            Some(Interaction::Hovered) | Some(Interaction::Pressed) => {
                rgb(renzora_ember::theme::hover_bg())
            }
            _ => Color::NONE,
        }
    });

    #[cfg(not(target_arch = "wasm32"))]
    if let Some(path) = brand_icon_path() {
        let img = commands
            .spawn((
                ImageNode::default(),
                Node {
                    width: Val::Px(MARK),
                    height: Val::Px(MARK),
                    // Revealed by the binding below once the decode lands, so a
                    // blank `ImageNode` never flashes as a white square.
                    display: Display::None,
                    ..default()
                },
                // Let the click through to the button, or a press on the mark
                // itself (i.e. anywhere you would aim) never reaches it.
                bevy::ui::FocusPolicy::Pass,
                bevy::picking::Pickable::IGNORE,
                renzora_ember::widgets::FileImageWanted(path.clone()),
            ))
            .id();
        renzora_ember::reactive::tracked::bind_with(
            commands,
            img,
            move |w| {
                w.get_resource::<renzora_ember::widgets::FileImages>()
                    .and_then(|c| c.get(&path))
            },
            |w, e, handle: &Option<Handle<Image>>| {
                let Some(h) = handle else { return };
                if let Some(mut n) = w.get_mut::<ImageNode>(e) {
                    if n.image != *h {
                        n.image = h.clone();
                    }
                }
                if let Some(mut node) = w.get_mut::<Node>(e) {
                    node.display = Display::Flex;
                }
            },
        );
        commands.entity(button).add_child(img);
        return button;
    }

    let fallback = icon_text(commands, &fonts.phosphor, "cube", accent(), 15.0);
    commands
        .entity(fallback)
        .insert((bevy::ui::FocusPolicy::Pass, bevy::picking::Pickable::IGNORE));
    commands.entity(button).add_child(fallback);
    button
}

/// Absolute path of the icon staged beside the executable, if this build has one.
///
/// Shared with the About overlay, which draws the same mark larger.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn brand_icon_path() -> Option<std::path::PathBuf> {
    let path = std::env::current_exe()
        .ok()?
        .parent()?
        .join("resources")
        .join("icon.png");
    path.is_file().then_some(path)
}

/// Click the mark → open About.
fn brand_mark_click(
    q: Query<&Interaction, (With<BrandMarkBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.insert_resource(crate::about::ShowAboutRequested);
    }
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

/// What a menu needs to know about the world it is being built in, read once
/// per menu-open.
///
/// The rows are plain `Text` baked at build time, so none of this can be a
/// reactive binding; gathering it in one struct is what stops each new piece
/// from adding a parameter to `spawn_top_menu`, `build_menu_items` and both
/// systems that call them.
struct MenuContext<'a> {
    /// The signed-in username (`None` = signed out). The hamburger's first row
    /// *is* the name, so the fact of being signed in is not enough.
    account: Option<&'a str>,
    /// Release tag of a pending engine update, when `renzora_update`'s
    /// background check found one, so Help names the version instead of making
    /// you open a dialog to find out.
    update_tag: Option<&'a str>,
    /// Recently-opened project roots, most recent first — File > Recent
    /// Projects. Empty in a build with no splash plugin.
    recents: &'a [std::path::PathBuf],
}

/// Spawn a top-menu dropdown anchored at `pos` and return its root.
fn spawn_top_menu(
    commands: &mut Commands,
    fonts: &EmberFonts,
    kind: TopMenuKind,
    pos: Vec2,
    ctx: &MenuContext,
) -> Entity {
    let (root, card) = renzora_ember::widgets::screen_menu_parts(commands, pos.x, pos.y);
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
        dark_card(commands, card);
    }
    let kids = build_menu_items(commands, fonts, kind, ctx);
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
    recents: Option<Res<renzora::RecentProjects>>,
    mut open: ResMut<OpenTopMenu>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else {
        return;
    };
    let account = account_name(&bridge);
    let update_tag = update.as_ref().map(|u| u.0.clone());
    let ctx = MenuContext {
        account: account.as_deref(),
        update_tag: update_tag.as_deref(),
        recents: recents.as_ref().map(|r| r.0.as_slice()).unwrap_or(&[]),
    };
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
        open.menu = Some(spawn_top_menu(&mut commands, &fonts, menu.0, pos, &ctx));
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
    recents: Option<Res<renzora::RecentProjects>>,
    mut open: ResMut<OpenTopMenu>,
    mut commands: Commands,
) {
    let Some(open_kind) = open.kind else { return };
    let Some(fonts) = fonts else { return };
    let account = account_name(&bridge);
    let update_tag = update.as_ref().map(|u| u.0.clone());
    let ctx = MenuContext {
        account: account.as_deref(),
        update_tag: update_tag.as_deref(),
        recents: recents.as_ref().map(|r| r.0.as_slice()).unwrap_or(&[]),
    };
    for (interaction, menu, rcp, cn) in &q {
        if *interaction == Interaction::Hovered && menu.0 != open_kind {
            if let Some(e) = open.menu.take() {
                commands.entity(e).try_despawn();
            }
            let Some(pos) = anchor_below(&windows, rcp, cn) else {
                open.kind = None;
                return;
            };
            open.menu = Some(spawn_top_menu(&mut commands, &fonts, menu.0, pos, &ctx));
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

/// Repaint a menu card on the top bar's own surface.
///
/// Every other menu in the editor is a context menu that opens over a *panel*,
/// so `surfaces.popup` is deliberately a step lighter than what is behind it:
/// that lift is what separates the menu from the panel it covers. The hamburger
/// drops out of the top bar instead, and the top bar is `surfaces.window`: two
/// shades apart in the Dark theme (11,11,17 against 28,28,35), so the dropdown
/// read as a grey card stuck onto near-black chrome. Painting it `window_bg`
/// makes the menu continue the bar it came out of.
///
/// The border stays: it is the only thing separating the card from the bar now
/// that the two share a fill, and it is what draws the rounded corner.
fn dark_card(commands: &mut Commands, card: Entity) {
    commands.entity(card).insert(BackgroundColor(rgb(window_bg())));
}

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
    dark_card(commands, panel);
    for kid in &kids {
        spacious(commands, *kid);
    }
    commands.entity(content).add_children(&kids);
    spacious(commands, row)
}

/// File > Recent Projects: one row per entry in [`renzora::RecentProjects`],
/// each opening that project without going near a file dialog.
///
/// The dashboard has had this list since there was a dashboard, but it is only
/// reachable by leaving the project you are in — so from inside the editor the
/// way back to yesterday's project was to remember where you put it and find it
/// in an OS picker.
///
/// Rows are labelled with the folder name rather than the full path: the path
/// is what a 230px menu cannot show anyway, and the folder *is* the project's
/// name (that is how New Project names one). The empty case gets a muted row
/// instead of no submenu at all, so the entry means the same thing on a fresh
/// install as it does later.
fn recent_projects_submenu(
    commands: &mut Commands,
    fonts: &EmberFonts,
    recents: &[std::path::PathBuf],
) -> Entity {
    use renzora_ember::widgets::{menu_item, menu_item_styled};

    let kids: Vec<Entity> = if recents.is_empty() {
        vec![menu_item_styled(
            commands,
            fonts,
            "folder-dashed",
            &renzora::lang::t("splash.no_recent"),
            text_muted(),
            text_muted(),
            |_| {},
        )]
    } else {
        recents
            .iter()
            .map(|root| {
                let label = root
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| root.to_string_lossy().to_string());
                let path = root.clone();
                menu_item(commands, fonts, "folder", &label, move |w| {
                    w.insert_resource(crate::save_prompts::ProjectSwitchRequest(
                        crate::save_prompts::ProjectSwitch::Recent(path.clone()),
                    ));
                })
            })
            .collect()
    };

    panel_submenu(
        commands,
        fonts,
        "clock-counter-clockwise",
        &renzora::lang::t("splash.recent"),
        kids,
    )
}

fn build_menu_items(
    commands: &mut Commands,
    fonts: &EmberFonts,
    kind: TopMenuKind,
    ctx: &MenuContext,
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

            rows.push(menu_account_header(commands, fonts, ctx.account));
            rows.push(menu_sep(commands));

            for (icon, label, sub) in [
                ("file", renzora::lang::t("menu.file"), TopMenuKind::File),
                ("pencil-simple", renzora::lang::t("menu.edit"), TopMenuKind::Edit),
                ("eye", renzora::lang::t("menu.view"), TopMenuKind::View),
                ("question", renzora::lang::t("menu.help"), TopMenuKind::Help),
            ] {
                let kids = build_menu_items(commands, fonts, sub, ctx);
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
            if ctx.account.is_some() {
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
            // Both of these leave the project, so they ask
            // `save_prompts::process_project_switch_request` for it rather than
            // doing it: it closes every open document, and doing that on one
            // click with unsaved edits in them is the loss the window's × has
            // always prompted about.
            menu_item(commands, fonts, "folder-plus", &renzora::lang::t("menu.file.new_project"), |w| {
                w.insert_resource(crate::save_prompts::ProjectSwitchRequest(
                    crate::save_prompts::ProjectSwitch::New,
                ));
            }),
            menu_item(commands, fonts, "folder-open", &renzora::lang::t("menu.file.open_project"), |w| {
                w.insert_resource(crate::save_prompts::ProjectSwitchRequest(
                    crate::save_prompts::ProjectSwitch::Pick,
                ));
            }),
            recent_projects_submenu(commands, fonts, ctx.recents),
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
            menu_sep(commands),
            menu_item(commands, fonts, "arrow-counter-clockwise", &renzora::lang::t_or("menu.view.reset_defaults", "Reset to Defaults"), reset_defaults_action),
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
                &match ctx.update_tag {
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

// ── Reset everything ─────────────────────────────────────────────────────────

/// What the reset prompt is currently set to reset.
///
/// A resource rather than state on the dialog's entities, because the toggles
/// bind through `bind_2way`, which reads and writes the world. It outlives the
/// dialog on purpose: reopening the prompt remembers the last set of boxes, so
/// somebody resetting one thing repeatedly does not re-tick them each time.
#[derive(Resource, Clone, Copy)]
pub(crate) struct ResetDefaultsChoice {
    workspaces: bool,
    editor_settings: bool,
    viewport: bool,
    keybindings: bool,
    plugin_settings: bool,
    tutorial: bool,
}

impl Default for ResetDefaultsChoice {
    fn default() -> Self {
        Self {
            workspaces: true,
            editor_settings: true,
            viewport: true,
            keybindings: true,
            // Off by default, like the tutorial and for a related reason: a
            // plugin's settings are not the editor's configuration, they are
            // whatever that plugin was told, and some of it (an endpoint, a
            // chosen device, a path) is work the user did once and would have to
            // do again. "Put the editor back" should not silently include it.
            plugin_settings: false,
            // Off by default, unlike the other four. Redoing the tutorial is a
            // thing you ask for, not a thing you want thrown in with "put my
            // panels back" -- and having it re-offer itself unasked after an
            // unrelated reset is exactly the behaviour onboarding gets hated for.
            tutorial: false,
        }
    }
}

impl ResetDefaultsChoice {
    fn any(self) -> bool {
        self.workspaces
            || self.editor_settings
            || self.viewport
            || self.keybindings
            || self.plugin_settings
            || self.tutorial
    }
}

/// The confirm button on the reset-to-defaults prompt.
#[derive(Component)]
pub(crate) struct ResetDefaultsConfirmBtn;

/// The overlay the confirm button has to close when it fires.
#[derive(Component)]
pub(crate) struct ResetDefaultsOverlay(Entity);

/// View ▸ Reset to Defaults. Asks first, and asks *what*, because the four
/// things it can reset are not wanted together as often as you would think.
///
/// The three resets above it in the menu each undo one thing. This is the "I
/// have made a mess of the editor" button, and it is the only one whose damage
/// is not obvious from its name, so it gets a prompt that lists what goes and
/// lets each part be left alone.
fn reset_defaults_action(w: &mut World) {
    let Some(fonts) = w.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    w.get_resource_or_insert_with(ResetDefaultsChoice::default);
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let mut commands = Commands::new(&mut queue, w);
    // `Val::Auto` height, not a fixed one: the card was 250px tall around about
    // 130px of content, so the dialog sat there with a third of itself empty
    // below the buttons. There is no scrolling here and the rows never change,
    // so the content is the right thing to size to.
    let (root, content) = renzora_ember::widgets::overlay_val(
        &mut commands,
        &fonts,
        &renzora::lang::t_or("menu.view.reset_defaults", "Reset to Defaults"),
        Val::Px(480.0),
        Val::Auto,
        true,
    );
    // `overlay_val`'s content node carries no padding of its own — every caller
    // sets its own — so without this the body starts hard against the card's
    // left edge, which is what it was doing.
    let body = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::axes(Val::Px(18.0), Val::Px(16.0)),
            row_gap: Val::Px(14.0),
            ..default()
        })
        .id();

    // The warning first, and as the largest thing in the card. This is the one
    // item in the View menu that throws work away, so the dialog leads with the
    // consequence rather than with a paragraph the eye skips.
    let warn_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .id();
    let badge = commands
        .spawn((
            Node {
                width: Val::Px(34.0),
                height: Val::Px(34.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(9.0)),
                ..default()
            },
            BackgroundColor(Color::srgba_u8(WARN.0, WARN.1, WARN.2, 38)),
        ))
        .id();
    let badge_icon = icon_text(&mut commands, &fonts.phosphor, "warning", WARN, 18.0);
    commands.entity(badge).add_child(badge_icon);
    let warn_text = commands
        .spawn((
            Text::new(renzora::lang::t_or(
                "menu.view.reset_defaults_warning",
                "Choose what to reset. This cannot be undone.",
            )),
            ui_font(&fonts.ui, 14.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    commands.entity(warn_row).add_children(&[badge, warn_text]);

    let list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    let rows = vec![
        reset_option_row(
            &mut commands,
            &fonts,
            "browsers",
            &renzora::lang::t_or("menu.view.reset_opt_workspaces", "Workspaces and panels"),
            &renzora::lang::t_or(
                "menu.view.reset_opt_workspaces_sub",
                "Every workspace layout, the floating windows and the bottom panel",
            ),
            |w: &Rx| w.get_resource::<ResetDefaultsChoice>().is_some_and(|c| c.workspaces),
            |w: &mut World, v: bool| {
                if let Some(mut c) = w.get_resource_mut::<ResetDefaultsChoice>() {
                    c.workspaces = v;
                }
            },
        ),
        reset_option_row(
            &mut commands,
            &fonts,
            "sliders",
            &renzora::lang::t_or("menu.view.reset_opt_settings", "Editor settings"),
            &renzora::lang::t_or(
                "menu.view.reset_opt_settings_sub",
                "Everything in Settings, the theme back to Dark, including saved ones",
            ),
            |w: &Rx| w.get_resource::<ResetDefaultsChoice>().is_some_and(|c| c.editor_settings),
            |w: &mut World, v: bool| {
                if let Some(mut c) = w.get_resource_mut::<ResetDefaultsChoice>() {
                    c.editor_settings = v;
                }
            },
        ),
        reset_option_row(
            &mut commands,
            &fonts,
            "video-camera",
            &renzora::lang::t_or("menu.view.reset_opt_viewport", "Viewport and camera"),
            &renzora::lang::t_or(
                "menu.view.reset_opt_viewport_sub",
                "Look, orbit, pan and zoom sensitivity, the grid, gizmos and snapping",
            ),
            |w: &Rx| w.get_resource::<ResetDefaultsChoice>().is_some_and(|c| c.viewport),
            |w: &mut World, v: bool| {
                if let Some(mut c) = w.get_resource_mut::<ResetDefaultsChoice>() {
                    c.viewport = v;
                }
            },
        ),
        reset_option_row(
            &mut commands,
            &fonts,
            "keyboard",
            &renzora::lang::t_or("menu.view.reset_opt_keys", "Keyboard shortcuts"),
            &renzora::lang::t_or(
                "menu.view.reset_opt_keys_sub",
                "Every shortcut back to its shipped key",
            ),
            |w: &Rx| w.get_resource::<ResetDefaultsChoice>().is_some_and(|c| c.keybindings),
            |w: &mut World, v: bool| {
                if let Some(mut c) = w.get_resource_mut::<ResetDefaultsChoice>() {
                    c.keybindings = v;
                }
            },
        ),
        reset_option_row(
            &mut commands,
            &fonts,
            "puzzle-piece",
            &renzora::lang::t_or("menu.view.reset_opt_plugins", "Plugin settings"),
            &renzora::lang::t_or(
                "menu.view.reset_opt_plugins_sub",
                "Everything installed plugins have saved. Which plugins are on stays as it is",
            ),
            |w: &Rx| w.get_resource::<ResetDefaultsChoice>().is_some_and(|c| c.plugin_settings),
            |w: &mut World, v: bool| {
                if let Some(mut c) = w.get_resource_mut::<ResetDefaultsChoice>() {
                    c.plugin_settings = v;
                }
            },
        ),
        reset_option_row(
            &mut commands,
            &fonts,
            "graduation-cap",
            &renzora::lang::t_or("menu.view.reset_opt_tutorial", "Tutorial progress"),
            &renzora::lang::t_or(
                "menu.view.reset_opt_tutorial_sub",
                "Forget which chapters are done, so the tutorial offers itself again",
            ),
            |w: &Rx| w.get_resource::<ResetDefaultsChoice>().is_some_and(|c| c.tutorial),
            |w: &mut World, v: bool| {
                if let Some(mut c) = w.get_resource_mut::<ResetDefaultsChoice>() {
                    c.tutorial = v;
                }
            },
        ),
    ];
    commands.entity(list).add_children(&rows);

    let rule = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(1.0), ..default() },
            BackgroundColor(rgb(divider())),
        ))
        .id();

    // What survives, said as plainly as what does not. The reassurance is half
    // the reason to open this dialog at all.
    let keep_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(9.0),
            ..default()
        })
        .id();
    let keep_icon = icon_text(&mut commands, &fonts.phosphor, "check-circle", KEEP, 14.0);
    let keep_text = commands
        .spawn((
            Text::new(renzora::lang::t_or(
                "menu.view.reset_defaults_kept",
                "Your language, which plugins are installed, and your projects are untouched.",
            )),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    commands.entity(keep_row).add_children(&[keep_icon, keep_text]);

    let buttons = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::FlexEnd,
            column_gap: Val::Px(8.0),
            margin: UiRect::top(Val::Px(2.0)),
            ..default()
        })
        .id();
    let cancel = renzora_ember::widgets::button(
        &mut commands,
        &fonts.ui,
        &renzora::lang::t("common.cancel"),
    );
    commands
        .entity(cancel)
        .insert(crate::plugin_install::DismissOverlayBtn(root));
    let confirm = destructive_button(
        &mut commands,
        &fonts,
        &renzora::lang::t_or("menu.view.reset_defaults_confirm", "Reset Selected"),
    );
    commands
        .entity(confirm)
        .insert((ResetDefaultsConfirmBtn, ResetDefaultsOverlay(root)));
    // Nothing ticked, nothing to do: dim the button rather than letting a press
    // close the dialog having silently done nothing.
    renzora_ember::reactive::tracked::bind_bg(&mut commands, confirm, |w| {
        let on = w.get_resource::<ResetDefaultsChoice>().is_some_and(|c| c.any());
        let (r, g, b) = DESTRUCTIVE;
        if on { Color::srgb_u8(r, g, b) } else { Color::srgba_u8(r, g, b, 90) }
    });
    commands.entity(buttons).add_children(&[cancel, confirm]);

    commands
        .entity(body)
        .add_children(&[warn_row, list, rule, keep_row, buttons]);
    commands.entity(content).add_child(body);
    queue.apply(w);
}

/// One tickable scope: icon, name, a line of what it covers, and a switch.
#[allow(clippy::too_many_arguments)]
fn reset_option_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    glyph: &str,
    label: &str,
    sub: &str,
    get: impl Fn(&Rx) -> bool + Send + Sync + 'static,
    set: impl Fn(&mut World, bool) + Send + Sync + 'static,
) -> Entity {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            padding: UiRect::vertical(Val::Px(5.0)),
            ..default()
        })
        .id();
    let ic = icon_text(commands, &fonts.phosphor, glyph, text_muted(), 15.0);
    let col = commands
        .spawn(Node {
            flex_grow: 1.0,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    let name = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 12.5),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let detail = commands
        .spawn((
            Text::new(sub.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    commands.entity(col).add_children(&[name, detail]);
    let sw = renzora_ember::widgets::toggle_switch(commands, true);
    renzora_ember::reactive::tracked::bind_2way(commands, sw, get, move |w, v: &bool| set(w, *v));
    commands.entity(row).add_children(&[ic, col, sw]);
    row
}

/// Amber for the warning badge, green for the "kept" line. Fixed rather than
/// themed: both are status colours carrying a meaning, and a theme that
/// recoloured them would be saying something different.
const WARN: (u8, u8, u8) = (230, 170, 60);
const KEEP: (u8, u8, u8) = (74, 200, 130);
const DESTRUCTIVE: (u8, u8, u8) = (200, 62, 62);

/// A red confirm button, for the action you would regret misclicking.
///
/// Built here rather than from `widgets::button`, which carries `EmberButton` +
/// `Styled(Role::Button)` and is repainted from the theme every frame — a
/// background set on one of those is overwritten before it is ever drawn. The
/// two buttons in this row are deliberately different weights: Cancel is the
/// ordinary one, and the destructive option should not look like the default.
fn destructive_button(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    let (r, g, b) = DESTRUCTIVE;
    let btn = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(7.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::srgb_u8(r, g, b)),
            Interaction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("destructive-button"),
        ))
        .id();
    let t = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 12.5),
            TextColor(Color::WHITE),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    commands.entity(btn).add_child(t);
    btn
}

/// Do the reset once the prompt is confirmed, for the scopes that are ticked.
///
/// **Resetting the resource is usually all it takes**, now that every preference
/// round-trips through one `~/.renzora/settings.toml` section and a debounced
/// system writes that section whenever its resource changes: `EditorSettings`
/// (`[editor]`), `ViewportSettings` (`[viewport]`) and `KeyBindings`
/// (`[keybindings]`) each save themselves within a second of being replaced
/// here.
///
/// The two that need a second half are the two that are not one resource with
/// one section. `[app]` holds preferences that have no live resource at all (and
/// identity — the language, the disabled plugins — that a reset must *not*
/// touch), so it is rewritten field by field by
/// [`reset_editor_settings_prefs`]. `AutoSaveSettings` is the mirror case: a
/// live resource whose values live in `[app]`, so it is replaced here too or the
/// running editor keeps the old interval until the next launch.
pub(crate) fn reset_defaults_buttons(
    confirm: Query<(&Interaction, &ResetDefaultsOverlay), (With<ResetDefaultsConfirmBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    for (interaction, overlay) in &confirm {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let overlay = overlay.0;
        commands.queue(move |w: &mut World| {
            let choice = w.get_resource::<ResetDefaultsChoice>().copied().unwrap_or_default();
            if !choice.any() {
                return;
            }
            if let Ok(e) = w.get_entity_mut(overlay) {
                e.despawn();
            }
            let mut done: Vec<&str> = Vec::new();
            if choice.workspaces {
                reset_workspace_action(w);
                reset_global_docks_action(w);
                done.push("workspaces");
            }
            if choice.editor_settings {
                w.insert_resource(renzora_editor_framework::EditorSettings::default());
                // Autosave is the one preference the reset owns that does *not*
                // live in `EditorSettings`: it is its own resource, seeded from
                // `[app]` at boot and written back only when its Settings row is
                // edited. Resetting the file alone left the running editor on
                // the old interval, with the Settings panel still showing it,
                // until the next launch — every other scope here takes effect
                // the moment it is confirmed.
                w.insert_resource(renzora::AutoSaveSettings::default());
                #[cfg(not(target_arch = "wasm32"))]
                if let Err(e) = renzora::core::project_config::reset_editor_settings_prefs() {
                    warn!("[editor] could not reset saved editor preferences: {e}");
                }
                // The theme is a Settings tab, so it resets with the rest of
                // them. "Dark" by name rather than `Theme::dark()` directly:
                // `load_theme` is what clears `active_theme_dir` and the
                // unsaved-changes flag alongside the palette, and a half-reset
                // theme still resolving assets out of the old theme's folder is
                // worse than not resetting it.
                if let Some(mut tm) = w.get_resource_mut::<renzora_theme::ThemeManager>() {
                    tm.load_theme("Dark");
                }
                done.push("settings");
            }
            if choice.viewport {
                w.insert_resource(renzora::core::viewport_types::ViewportSettings::default());
                done.push("viewport");
            }
            if choice.keybindings {
                w.insert_resource(renzora::core::keybindings::KeyBindings::default());
                done.push("shortcuts");
            }
            // Only the saved blobs. A plugin holds its settings in its own
            // memory and writes them back when it next changes them, so one
            // that is loaded right now keeps working with what it has until it
            // is next started — there is no host-side handle to reach into it,
            // which is the whole point of the C-ABI boundary.
            #[cfg(not(target_arch = "wasm32"))]
            if choice.plugin_settings {
                match renzora::core::settings_file::clear_all_plugin_settings() {
                    // Nothing had been saved: say nothing rather than report a
                    // reset of something that was never there.
                    Ok(false) => {}
                    Ok(true) => done.push("plugin settings"),
                    Err(e) => warn!("[editor] could not clear plugin settings: {e}"),
                }
            }
            if choice.tutorial {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let _ = renzora::save_tutorial_completed(false);
                    let _ = renzora::save_tutorial_chapters(&[]);
                }
                done.push("tutorial");
            }
            renzora::core::console_log::console_info(
                "Editor",
                format!("Reset to defaults: {}", done.join(", ")),
            );
        });
    }
}
