//! The rail's footer — the renzora.com account and the language picker — and the
//! status strip along the bottom of the window.
//!
//! # Signing in without depending on the account
//!
//! The session, the sign-in form and the API client all live in
//! `renzora_marketplace`, which this crate must not depend on (see
//! [`super::sections`] for why). It does not need to: the contract crate carries
//! the whole boundary already — [`renzora::core::AuthBridge`] to read who is
//! signed in, and the two request markers to ask for the modal or a sign-out.
//! The modal that answers is the *same* one the editor's title bar opens, and its
//! systems are ungated, so it renders over the dashboard as readily as over the
//! editor and a sign-in here is a sign-in there.
//!
//! The account row hides itself when `AuthBridge` is absent. That is not
//! defensive coding — it is the honest reading of a build with no account plugin
//! in it, where a Sign In button would be a control that cannot do anything.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use bevy::window::SystemCursorIcon;

use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_bg, bind_display, bind_text, bind_text_color};
use renzora_ember::reactive::Rx;
use renzora_ember::widgets::{menu_item, scroll_area_keyed, HoverTooltip, Popup};

use super::chrome::{SplashUrl, DISCORD_URL, GITHUB_URL, WEBSITE_URL, YOUTUBE_URL};
use super::style::*;
use crate::github::{format_count, GithubStats};

#[derive(Component)]
struct SignInBtn;
#[derive(Component)]
struct SignOutBtn;

pub(crate) fn systems(app: &mut App) {
    app.add_systems(Update, (sign_in_click, sign_out_click));
}

// ── Account row ──────────────────────────────────────────────────────────────

/// The rail's account block: a Sign In button, or the signed-in username with a
/// sign-out control beside it.
///
/// Both faces are built and one is hidden, rather than rebuilding the row when
/// the session changes: it is two small nodes, and a `bind_display` pair has no
/// teardown to get wrong halfway through a sign-in.
pub(crate) fn build_account_row(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let block = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                margin: UiRect::bottom(Val::Px(4.0)),
                ..default()
            },
            FocusPolicy::Pass,
            Name::new("splash-account"),
        ))
        .id();
    // No account plugin in this build → no account controls. See the module doc.
    bind_display(commands, block, |w| w.get_resource::<renzora::core::AuthBridge>().is_some());

    let signed_out = sign_in_button(commands, fonts);
    bind_display(commands, signed_out, |w| !signed_in(w));

    let signed_in_row = signed_in_block(commands, fonts);
    bind_display(commands, signed_in_row, signed_in);

    commands.entity(block).add_children(&[signed_out, signed_in_row]);
    block
}

fn signed_in(w: &Rx) -> bool {
    w.get_resource::<renzora::core::AuthBridge>()
        .is_some_and(|b| b.signed_in_username.is_some())
}

fn username(w: &Rx) -> String {
    w.get_resource::<renzora::core::AuthBridge>()
        .and_then(|b| b.signed_in_username.clone())
        .unwrap_or_default()
}

fn sign_in_button(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(34.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(7.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(btn_dark()),
            BorderColor::all(border_soft()),
            Interaction::default(),
            FocusPolicy::Block,
            SignInBtn,
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("splash-sign-in"),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        if is_hovered(w, btn) { btn_dark_hover() } else { btn_dark() }
    });
    let ic = icon_text(commands, &fonts.phosphor, "sign-in", ICON_ACCENT, 14.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let label = commands
        .spawn((
            Text::new("Sign in".to_string()),
            ui_font(&fonts.ui, 12.5),
            TextColor(text()),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(btn).add_children(&[ic, label]);
    btn
}

fn signed_in_block(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(38.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(ca(16, 18, 28, 220)),
            BorderColor::all(border_soft()),
            FocusPolicy::Block,
        ))
        .id();

    // A monogram, not an avatar: the avatar cache lives with the account plugin
    // and this crate cannot reach it. The first letter of the username is enough
    // to make the row read as an identity rather than as a line of text.
    let badge = commands
        .spawn((
            Node {
                width: Val::Px(24.0),
                height: Val::Px(24.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(ca(110, 150, 255, 44)),
            FocusPolicy::Pass,
        ))
        .id();
    let initial = commands
        .spawn((
            Text::new(String::new()),
            ui_font(&fonts.ui, 12.0),
            TextColor(accent()),
            FocusPolicy::Pass,
        ))
        .id();
    bind_text(commands, initial, |w| {
        username(w).chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default()
    });
    commands.entity(badge).add_child(initial);

    let col = commands
        .spawn((
            Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, ..default() },
            FocusPolicy::Pass,
        ))
        .id();
    let name = commands
        .spawn((
            Text::new(String::new()),
            ui_font(&fonts.ui, 12.0),
            TextColor(text()),
            FocusPolicy::Pass,
        ))
        .id();
    bind_text(commands, name, |w| elide(&username(w), 16));
    let sub = commands
        .spawn((
            Text::new("renzora.com".to_string()),
            ui_font(&fonts.ui, 9.5),
            TextColor(text_muted()),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(col).add_children(&[name, sub]);

    let out = commands
        .spawn((
            Node {
                width: Val::Px(24.0),
                height: Val::Px(24.0),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            FocusPolicy::Block,
            SignOutBtn,
            HoverTooltip::new("Sign out"),
            HoverCursor(SystemCursorIcon::Pointer),
        ))
        .id();
    bind_bg(commands, out, move |w| {
        if is_hovered(w, out) { ca(239, 68, 68, 40) } else { Color::NONE }
    });
    let out_icon = icon_text(commands, &fonts.phosphor, "sign-out", ICON_MUTED, 13.0);
    commands.entity(out_icon).insert(FocusPolicy::Pass);
    bind_text_color(commands, out_icon, move |w| {
        if is_hovered(w, out) { error_color() } else { text_muted() }
    });
    commands.entity(out).add_child(out_icon);

    commands.entity(row).add_children(&[badge, col, out]);
    row
}

fn sign_in_click(
    q: Query<&Interaction, (With<SignInBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.insert_resource(renzora::core::AuthToggleWindowRequest);
    }
}

fn sign_out_click(
    q: Query<&Interaction, (With<SignOutBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.insert_resource(renzora::core::AuthSignOutRequest);
    }
}

// ── Language picker ──────────────────────────────────────────────────────────

/// Compact language picker for the rail footer: a globe + the active language's
/// native name that opens a dropdown of every registered language (built-in
/// packs + any external `languages/*.toml`). Picking one applies and persists it
/// (`set_active` + `save_language`), so the choice is already in effect when the
/// editor opens. Uses the shared ember `Popup`/`menu_item` widgets — their toggle
/// systems run in every state, including Splash.
pub(crate) fn build_language_picker(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let langs = renzora::lang::available();
    let active = renzora::lang::active_code();

    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                // Opens upward: the trigger is the last row in the rail, so a
                // downward menu would fall off the bottom of the window.
                bottom: Val::Percent(100.0),
                left: Val::Px(0.0),
                margin: UiRect::bottom(Val::Px(4.0)),
                flex_direction: FlexDirection::Column,
                min_width: Val::Px(178.0),
                padding: UiRect::all(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(c(22, 24, 30)),
            BorderColor::all(border_soft()),
            GlobalZIndex(700),
            bevy::ui::RelativeCursorPosition::default(),
            Name::new("splash-language-menu"),
        ))
        .id();

    let mut rows = Vec::new();
    for m in &langs {
        let code = m.code.clone();
        let label = if m.name.is_empty() { m.code.clone() } else { m.name.clone() };
        let icon = if m.code == active { "check" } else { "globe" };
        rows.push(menu_item(commands, fonts, icon, &label, move |_w| {
            renzora::lang::set_active(&code);
            let _ = renzora::save_language(&code);
        }));
    }
    let content = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() })
        .id();
    commands.entity(content).add_children(&rows);
    let scroll = scroll_area_keyed(commands, content, 280.0, "splash-language-menu");
    commands.entity(panel).add_child(scroll);

    let active_name = langs
        .iter()
        .find(|m| m.code == active)
        .map(|m| if m.name.is_empty() { m.code.clone() } else { m.name.clone() })
        .unwrap_or_else(|| {
            if active.is_empty() { "Language".to_string() } else { active.clone() }
        });

    let icon = icon_text(commands, &fonts.phosphor, "globe", ICON_MUTED, 13.0);
    commands.entity(icon).insert(FocusPolicy::Pass);
    let label = commands
        .spawn((
            Text::new(active_name),
            ui_font(&fonts.ui, 11.5),
            TextColor(text_muted()),
            FocusPolicy::Pass,
        ))
        .id();
    let caret = icon_text(commands, &fonts.phosphor, "caret-up", ICON_MUTED, 9.0);
    commands.entity(caret).insert(FocusPolicy::Pass);
    let trigger = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(28.0),
                position_type: PositionType::Relative,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            FocusPolicy::Block,
            Popup { panel, open: false },
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("splash-language-picker"),
        ))
        .id();
    bind_bg(commands, trigger, move |w| {
        if is_hovered(w, trigger) { panel_hover() } else { Color::NONE }
    });
    commands.entity(trigger).add_children(&[icon, label, caret, panel]);
    trigger
}

// ── Status strip ─────────────────────────────────────────────────────────────

/// The strip along the bottom: render health and build identity on the left,
/// the project's public links on the right.
pub(crate) fn build_status_bar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(STATUSBAR_H),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::horizontal(Val::Px(12.0)),
                border: UiRect::top(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(rail_bg()),
            BorderColor::all(border_soft()),
            FocusPolicy::Block,
            Name::new("splash-status-bar"),
        ))
        .id();

    let left = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    // Frame rate and nothing else.
    //
    // The version moved to the title bar (`chrome::build_title_bar`) — it is
    // identity, not telemetry. The ABI hash went entirely: it linked to the
    // commit that froze it, which is a question one plugin author asks once, and
    // it was spending the strip's whole left side on a hex string that reads as
    // an error code to everyone else. `renzora::version::display()` in the title
    // bar identifies the build; the release page carries the rest.
    let fps = build_fps(commands, fonts);
    commands.entity(left).add_child(fps);

    let right = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .id();
    let website = social_button(commands, fonts, "globe", "Website", WEBSITE_URL, false);
    let youtube = social_button(commands, fonts, "youtube-logo", "YouTube", YOUTUBE_URL, false);
    let discord = social_button(commands, fonts, "discord-logo", "Discord", DISCORD_URL, false);
    let star = social_button(commands, fonts, "star", "Star us on GitHub", GITHUB_URL, true);
    commands.entity(right).add_children(&[website, youtube, discord, star]);

    commands.entity(bar).add_children(&[left, right]);
    bar
}

/// Status-strip type sizes. A pair of constants because the frame rate and the
/// link labels read as one line of text however far apart they sit, and they
/// were previously loose `10.5`s that would have drifted the first time one of
/// them was adjusted.
const STATUS_TEXT: f32 = 12.0;
const STATUS_MONO: f32 = 11.5;

/// FPS readout for the status strip — a quick render-health baseline. The splash
/// is GPU-light, so this is "is the app/window itself smooth?", to compare
/// against the editor's much heavier per-frame cost. Color-coded green/amber/red.
fn build_fps(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let label = commands
        .spawn((
            Text::new(String::new()),
            ui_font(&fonts.mono, STATUS_MONO),
            TextColor(text_muted()),
            FocusPolicy::Pass,
            Name::new("splash-fps"),
        ))
        .id();
    bind_text(commands, label, |w| {
        let fps = w.get_resource::<super::SplashFps>().map(|f| f.0).unwrap_or(0.0);
        format!("{fps:.0} FPS")
    });
    bind_text_color(commands, label, |w| {
        let fps = w.get_resource::<super::SplashFps>().map(|f| f.0).unwrap_or(0.0);
        if fps >= 58.0 {
            c(100, 200, 100)
        } else if fps >= 30.0 {
            c(200, 200, 100)
        } else {
            c(200, 100, 100)
        }
    });
    label
}

fn social_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    txt: &str,
    url: &str,
    starred: bool,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                height: Val::Px(24.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::horizontal(Val::Px(9.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            FocusPolicy::Block,
            SplashUrl(url.to_string()),
            HoverCursor(SystemCursorIcon::Pointer),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        if is_hovered(w, btn) { btn_dark_hover() } else { Color::NONE }
    });
    let col = if starred { (235, 195, 80) } else { ICON_MUTED };
    let ic = icon_text(commands, &fonts.phosphor, icon, col, 13.5);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let t = commands
        .spawn((
            Text::new(txt.to_string()),
            ui_font(&fonts.ui, STATUS_TEXT),
            TextColor(if starred { c(235, 195, 80) } else { text_muted() }),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(btn).add_children(&[ic, t]);
    if starred {
        bind_text(commands, t, |w| {
            let stars = w.get_resource::<GithubStats>().and_then(|s| s.stars);
            match stars {
                Some(n) => format!("Star us on GitHub  ({})", format_count(n)),
                None => "Star us on GitHub".to_string(),
            }
        });
    }
    btn
}
