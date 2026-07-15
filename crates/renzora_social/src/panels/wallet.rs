//! Wallet panel — the community's credits hub: live balance, buying credit
//! packs (via Stripe Checkout in the browser), donating to keep the platform
//! running, a donor leaderboard, and the donor-badge tiers.
//!
//! Money never touches the app: both buying and (the payout side of) donating
//! hand off to a hosted Stripe page opened in the browser. The donation total
//! and leaderboard are public endpoints, so those sections stay populated even
//! when signed out — only donating itself needs a session.

use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver, Sender};
use renzora::core::RenzoraShellExt;
use renzora::SplashState;
use renzora_auth::billing::{DonateResponse, DonationLeader, CREDIT_USD_CENTS};
use renzora_auth::AuthSession;
use renzora_ember::dock::panel_active;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, bind_text, keyed_list_tokened, Bound, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    accent_chip, accent_ghost, checkbox, elevation, gradient_badge, gradient_button,
    gradient_diag, gradient_icon_tile, icon_badge, text_input, tint, EmberTextInput, HoverTint,
};

use crate::avatars::avatar_image;
use crate::toasts::{ToastQueue, Tone};
use crate::util::{self, hash64, session_clone};

pub(crate) const PANEL_ID: &str = "social_wallet";

/// Warm gold — value, generosity. The wallet's identity hue (an amber distinct
/// from the notifications amber, leaning richer/gold).
const HUE_WALLET: (u8, u8, u8) = (238, 184, 82);

/// Credit packs offered for purchase: `(credits, optional tag)`.
const PACKS: [(i64, Option<&str>); 4] = [
    (50, None),
    (100, None),
    (250, Some("Popular")),
    (500, Some("Best value")),
];

/// Quick-fill donation amounts.
const DONATE_PRESETS: [i64; 4] = [10, 50, 100, 500];

/// Donor-badge tiers: `(name, credit threshold, icon)`.
const TIERS: [(&str, i64, &str); 4] = [
    ("Bronze", 100, "medal"),
    ("Silver", 500, "medal"),
    ("Gold", 1000, "medal"),
    ("Platinum", 5000, "crown-simple"),
];

/// Rank medal colors for the top three leaderboard spots (gold/silver/bronze).
const RANK_COLORS: [(u8, u8, u8); 3] = [(235, 190, 90), (200, 205, 215), (205, 145, 95)];

// ── Worker results ─────────────────────────────────────────────────────────

pub(crate) enum WalletResult {
    /// Total credits donated to the platform (public).
    Total(Result<i64, String>),
    /// Top donors (public).
    Leaderboard(Result<Vec<DonationLeader>, String>),
    /// Stripe Checkout URL for a credit top-up — open in the browser.
    Checkout(Result<String, String>),
    /// A donation finished.
    Donated(Result<DonateResponse, String>),
}

#[derive(Resource)]
pub(crate) struct WalletPanel {
    pub donation_total: i64,
    pub leaderboard: Vec<DonationLeader>,
    /// Whether the leaderboard fetch has landed at least once — distinguishes
    /// "still loading" from a genuinely empty board.
    pub leaderboard_loaded: bool,
    pub version: u64,
    pub loaded_once: bool,
    pub tx: Sender<WalletResult>,
    rx: Receiver<WalletResult>,
}

impl Default for WalletPanel {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        Self {
            donation_total: 0,
            leaderboard: Vec::new(),
            leaderboard_loaded: false,
            version: 0,
            loaded_once: false,
            tx,
            rx,
        }
    }
}

impl WalletPanel {
    pub fn bump(&mut self) {
        self.version = self.version.wrapping_add(1);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_thread(f: impl FnOnce() + Send + 'static) {
    std::thread::spawn(f);
}
#[cfg(target_arch = "wasm32")]
fn spawn_thread(_f: impl FnOnce() + Send + 'static) {}

/// Open a URL in the user's default browser (Stripe hosted pages).
#[cfg(not(target_arch = "wasm32"))]
fn open_url(url: &str) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

pub(crate) fn register(app: &mut App) {
    app.init_resource::<WalletPanel>();
    app.register_shell_panel(PANEL_ID, "Wallet", "wallet", "Community");
    app.register_panel_content(PANEL_ID, true, build);
    app.add_systems(
        Update,
        (
            poll_results,
            auto_load.run_if(panel_active(PANEL_ID)),
            buy_clicks,
            donate_clicks,
        )
            .run_if(in_state(SplashState::Editor)),
    );
}

// ── Systems ────────────────────────────────────────────────────────────────

fn poll_results(
    mut panel: ResMut<WalletPanel>,
    mut session: ResMut<AuthSession>,
    mut toasts: ResMut<ToastQueue>,
) {
    let mut got = Vec::new();
    while let Ok(r) = panel.rx.try_recv() {
        got.push(r);
    }
    for r in got {
        match r {
            WalletResult::Total(Ok(total)) => {
                panel.donation_total = total;
                panel.bump();
            }
            WalletResult::Total(Err(_)) => {}
            WalletResult::Leaderboard(res) => {
                panel.leaderboard_loaded = true;
                if let Ok(list) = res {
                    panel.leaderboard = list;
                }
                panel.bump();
            }
            WalletResult::Checkout(Ok(url)) => {
                open_url(&url);
                toasts.push(Tone::Info, "Opening Stripe checkout in your browser…", None);
            }
            WalletResult::Checkout(Err(e)) => {
                toasts.push(Tone::Error, format!("Checkout failed: {e}"), None);
            }
            WalletResult::Donated(Ok(resp)) => {
                toasts.push(Tone::Success, format!("Thank you! You donated {} credits.", resp.amount), None);
                // Reflect the spend in the live balance header immediately — the
                // donate response carries no new balance, and there's no cheap
                // session refetch, so decrement optimistically (clamped at 0).
                if let Some(u) = session.user.as_mut() {
                    u.credit_balance = (u.credit_balance - resp.amount).max(0);
                }
                panel.donation_total = resp.total_donated;
                panel.bump();
                // Pull the fresh public total + leaderboard so the board reflects
                // the new standing.
                refresh_public(&mut panel);
            }
            WalletResult::Donated(Err(e)) => {
                toasts.push(Tone::Error, format!("Donation failed: {e}"), None);
            }
        }
    }
}

/// One-shot load of the public donation total + leaderboard. `loaded_once` is
/// set at spawn time so a failure can't loop; these endpoints need no session,
/// so this runs signed out too.
fn auto_load(mut panel: ResMut<WalletPanel>) {
    if !panel.loaded_once {
        panel.loaded_once = true;
        refresh_public(&mut panel);
    }
}

/// Re-fetch the public donation total + leaderboard on their own workers.
fn refresh_public(panel: &mut WalletPanel) {
    let tx = panel.tx.clone();
    spawn_thread(move || {
        let _ = tx.send(WalletResult::Total(
            renzora_auth::billing::donation_total().map(|t| t.total),
        ));
    });
    let tx = panel.tx.clone();
    spawn_thread(move || {
        let _ = tx.send(WalletResult::Leaderboard(renzora_auth::billing::donation_leaderboard()));
    });
}

/// Kick off a Stripe Checkout for `amount` credits, or nudge the user to sign
/// in first (the top-up endpoint needs a session).
fn start_topup(session: &AuthSession, toasts: &mut ToastQueue, tx: &Sender<WalletResult>, amount: i64) {
    if !session.is_signed_in() {
        toasts.push(Tone::Warn, "Sign in to buy credits.", None);
        return;
    }
    let tx = tx.clone();
    let s = session_clone(session);
    spawn_thread(move || {
        let _ = tx.send(WalletResult::Checkout(renzora_auth::billing::topup_checkout_url(&s, amount)));
    });
}

// ── Clicks ─────────────────────────────────────────────────────────────────

#[derive(Component)]
struct BuyPackBtn(i64);
#[derive(Component)]
struct BuyCustomBtn;
#[derive(Component)]
struct BuyAmountInput;
#[derive(Component)]
struct DonatePresetBtn(i64);
#[derive(Component)]
struct DonateAmountInput;
#[derive(Component)]
struct DonateMessageInput;
#[derive(Component)]
struct DonateAnonCheckbox;
#[derive(Component)]
struct DonateBtn;

fn buy_clicks(
    panel: Res<WalletPanel>,
    session: Res<AuthSession>,
    mut toasts: ResMut<ToastQueue>,
    packs: Query<(&Interaction, &BuyPackBtn), Changed<Interaction>>,
    custom: Query<&Interaction, (With<BuyCustomBtn>, Changed<Interaction>)>,
    amount: Query<&EmberTextInput, With<BuyAmountInput>>,
) {
    let pressed = |i: &Interaction| *i == Interaction::Pressed;
    for (i, b) in &packs {
        if pressed(i) {
            start_topup(&session, &mut toasts, &panel.tx, b.0);
        }
    }
    // "Buy" acts on the custom-amount field.
    if custom.iter().any(pressed) {
        let raw = amount.iter().next().map(|a| a.value.trim().to_string()).unwrap_or_default();
        match raw.parse::<i64>() {
            Ok(n) if n >= 50 => start_topup(&session, &mut toasts, &panel.tx, n),
            Ok(_) => toasts.push(Tone::Warn, "Minimum top-up is 50 credits", None),
            Err(_) => toasts.push(Tone::Warn, "Pick a pack, or enter a custom amount (min 50) to top up", None),
        }
    }
}

fn donate_clicks(
    panel: Res<WalletPanel>,
    session: Res<AuthSession>,
    mut toasts: ResMut<ToastQueue>,
    presets: Query<(&Interaction, &DonatePresetBtn), Changed<Interaction>>,
    donate: Query<&Interaction, (With<DonateBtn>, Changed<Interaction>)>,
    mut inputs: Query<(&mut EmberTextInput, Option<&DonateAmountInput>, Option<&DonateMessageInput>)>,
    anon: Query<&Bound<bool>, With<DonateAnonCheckbox>>,
) {
    let pressed = |i: &Interaction| *i == Interaction::Pressed;

    // Preset chips just fill the amount field (no state behind a keyed list).
    for (i, b) in &presets {
        if pressed(i) {
            for (mut input, is_amount, _) in &mut inputs {
                if is_amount.is_some() {
                    input.value = b.0.to_string();
                }
            }
        }
    }

    if !donate.iter().any(pressed) {
        return;
    }
    if !session.is_signed_in() {
        toasts.push(Tone::Warn, "Sign in to donate.", None);
        return;
    }
    let mut amount_str = String::new();
    let mut message = String::new();
    for (input, is_amount, is_message) in &inputs {
        if is_amount.is_some() {
            amount_str = input.value.trim().to_string();
        }
        if is_message.is_some() {
            message = input.value.trim().to_string();
        }
    }
    let amount: i64 = match amount_str.parse() {
        Ok(n) if n >= 1 => n,
        _ => {
            toasts.push(Tone::Warn, "Enter an amount of at least 1 credit", None);
            return;
        }
    };
    let anonymous = anon.iter().next().map(|b| b.0).unwrap_or(false);
    let message = if message.is_empty() { None } else { Some(message) };

    // Clear the composer so the donation reads as sent.
    for (mut input, is_amount, is_message) in &mut inputs {
        if is_amount.is_some() || is_message.is_some() {
            input.value.clear();
        }
    }

    let tx = panel.tx.clone();
    let s = session_clone(&session);
    spawn_thread(move || {
        let r = renzora_auth::billing::donate(&s, amount, message.as_deref(), anonymous);
        let _ = tx.send(WalletResult::Donated(r));
    });
}

// ── Build ──────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let hue = HUE_WALLET;
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::all(Val::Px(10.0)),
            ..default()
        })
        .id();
    // Keep the content a tight, centered column — the design is a narrow
    // storefront (like the website), so it shouldn't sprawl across a wide dock.
    let inner = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            max_width: Val::Px(820.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(14.0),
            ..default()
        })
        .id();
    commands.entity(root).add_child(inner);

    // ── Balance header — a lit card: gradient tile + live balance + USD value. ──
    let bal_card = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(12.0),
                padding: UiRect::all(Val::Px(16.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(14.0)),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
            // A soft diagonal sheen — the hue catches the top-left corner and
            // falls off to the card color, so it reads as a lit surface rather
            // than the old harsh top-to-bottom gold band.
            gradient_diag(tint(hue, 34), rgb(section_bg())),
            BorderColor::all(tint(hue, 40)),
            elevation(2.0, 14.0),
        ))
        .id();
    let tile = gradient_icon_tile(commands, fonts, hue, "wallet", 46.0);
    let bal_col = commands
        .spawn(Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, row_gap: Val::Px(1.0), ..default() })
        .id();
    let bal_row = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Baseline, column_gap: Val::Px(6.0), ..default() })
        .id();
    let bal_num = commands
        .spawn((Text::new("0"), ui_font(&fonts.ui, 26.0), TextColor(rgb(text_primary()))))
        .id();
    bind_text(commands, bal_num, |w| {
        let bal = w
            .get_resource::<AuthSession>()
            .and_then(|s| s.user.as_ref().map(|u| u.credit_balance))
            .unwrap_or(0);
        format!("{bal}")
    });
    let bal_lbl = commands
        .spawn((Text::new("credits"), ui_font(&fonts.ui, 12.5), TextColor(rgb(hue))))
        .id();
    commands.entity(bal_row).add_children(&[bal_num, bal_lbl]);
    let bal_sub = commands
        .spawn((Text::new("1 credit = $0.10"), ui_font(&fonts.ui, 9.5), TextColor(rgb(text_muted()))))
        .id();
    commands.entity(bal_col).add_children(&[bal_row, bal_sub]);
    // Right side: the balance's dollar value (fills the space the Top up button
    // used to occupy — the Buy section below is the real top-up flow).
    let usd_col = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            row_gap: Val::Px(1.0),
            ..default()
        })
        .id();
    let usd_num = commands
        .spawn((Text::new("$0.00"), ui_font(&fonts.ui, 17.0), TextColor(rgb(hue))))
        .id();
    bind_text(commands, usd_num, |w| {
        let bal = w
            .get_resource::<AuthSession>()
            .and_then(|s| s.user.as_ref().map(|u| u.credit_balance))
            .unwrap_or(0);
        format!("${:.2}", bal as f64 * 0.10)
    });
    let usd_lbl = commands
        .spawn((Text::new("value"), ui_font(&fonts.ui, 9.5), TextColor(rgb(text_muted()))))
        .id();
    commands.entity(usd_col).add_children(&[usd_num, usd_lbl]);
    commands.entity(bal_card).add_children(&[tile, bal_col, usd_col]);

    // ── Buy credits ──
    let (buy_card, buy_body) = section_card(commands, fonts, hue, "shopping-cart-simple", "Buy credits");
    let buy_note = muted_note(
        commands,
        fonts,
        "Select a pack — you'll be redirected to Stripe for secure payment.",
    );
    let packs_row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(8.0),
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    for (credits, tag) in PACKS {
        let pack = pack_button(commands, fonts, hue, credits, tag);
        commands.entity(pack).insert(BuyPackBtn(credits));
        commands.entity(packs_row).add_child(pack);
    }
    let custom_row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let buy_amount = text_input(commands, &fonts.ui, "Custom amount (min 50)", "");
    commands.entity(buy_amount).insert((BuyAmountInput, Node { flex_grow: 1.0, ..default() }));
    let buy_btn = gradient_button(commands, fonts, hue, "Buy");
    commands.entity(buy_btn).insert(BuyCustomBtn);
    commands.entity(custom_row).add_children(&[buy_amount, buy_btn]);
    commands.entity(buy_body).add_children(&[buy_note, packs_row, custom_row]);

    // ── Support Renzora (donate) ──
    let (support_card, support_body) = section_card(commands, fonts, hue, "heart", "Support Renzora");
    let don_note = muted_note(
        commands,
        fonts,
        "Your donations help keep the platform running and fund new features.",
    );
    let total_line = commands
        .spawn((Text::new("0 credits donated"), ui_font(&fonts.ui, 11.0), TextColor(rgb(hue))))
        .id();
    bind_text(commands, total_line, |w| {
        let total = w.get_resource::<WalletPanel>().map(|p| p.donation_total).unwrap_or(0);
        format!("{total} credits donated so far")
    });

    // Signed-in donation form (presets → amount → message → anonymous → donate).
    let form = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    bind_display(commands, form, util::signed_in);
    let presets_row = commands
        .spawn(Node { flex_direction: FlexDirection::Row, column_gap: Val::Px(5.0), ..default() })
        .id();
    for amount in DONATE_PRESETS {
        let chip = accent_ghost(commands, fonts, hue, &amount.to_string());
        commands.entity(chip).insert(DonatePresetBtn(amount));
        commands.entity(presets_row).add_child(chip);
    }
    let donate_amount = text_input(commands, &fonts.ui, "Amount (credits)", "");
    commands.entity(donate_amount).insert((DonateAmountInput, Node { width: Val::Percent(100.0), ..default() }));
    let donate_message = text_input(commands, &fonts.ui, "Message (optional)", "");
    commands.entity(donate_message).insert((DonateMessageInput, Node { width: Val::Percent(100.0), ..default() }));
    let anon_row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(7.0),
            ..default()
        })
        .id();
    let anon_box = checkbox(commands, false);
    commands.entity(anon_box).insert(DonateAnonCheckbox);
    let anon_label = commands
        .spawn((Text::new("Donate anonymously"), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_muted()))))
        .id();
    commands.entity(anon_row).add_children(&[anon_box, anon_label]);
    let donate_btn = gradient_button(commands, fonts, hue, "Donate credits");
    commands.entity(donate_btn).insert((DonateBtn, Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        padding: UiRect::axes(Val::Px(11.0), Val::Px(9.0)),
        border_radius: BorderRadius::all(Val::Px(8.0)),
        margin: UiRect::top(Val::Px(4.0)),
        ..default()
    }));
    commands
        .entity(form)
        .add_children(&[presets_row, donate_amount, donate_message, anon_row, donate_btn]);

    // Signed-out prompt in place of the form.
    let signed_out = muted_note(commands, fonts, "Sign in to donate.");
    bind_display(commands, signed_out, |w| !util::signed_in(w));

    // Donor-badge tiers live at the foot of the support card.
    let tiers_row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(6.0),
            row_gap: Val::Px(6.0),
            margin: UiRect::top(Val::Px(2.0)),
            ..default()
        })
        .id();
    for (name, threshold, icon) in TIERS {
        let chip = accent_chip(commands, fonts, hue, Some(icon), &format!("{name} · {threshold}+"));
        commands.entity(tiers_row).add_child(chip);
    }
    commands
        .entity(support_body)
        .add_children(&[don_note, total_line, form, signed_out, tiers_row]);

    // ── Leaderboard ──
    let (lb_card, lb_body) = section_card(commands, fonts, hue, "trophy", "Top donors");
    let lb_list = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(4.0), ..default() })
        .id();
    keyed_list_tokened(
        commands,
        lb_list,
        |w| w.get_resource::<WalletPanel>().map(|p| p.version).unwrap_or(0),
        leaderboard_snapshot,
    );
    commands.entity(lb_body).add_child(lb_list);

    commands.entity(inner).add_children(&[bal_card, buy_card, support_card, lb_card]);
    root
}

// ── Widgets / snapshots ──────────────────────────────────────────────────────

/// A titled section card: a subtly hue-tinted rounded surface with an icon
/// badge + heading, returning the card and its content body column.
fn section_card(
    commands: &mut Commands,
    fonts: &EmberFonts,
    hue: (u8, u8, u8),
    icon: &str,
    title: &str,
) -> (Entity, Entity) {
    let card = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(rgba([255, 255, 255, 5])),
            BorderColor::all(rgba([255, 255, 255, 12])),
        ))
        .id();
    let header = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let badge = icon_badge(commands, fonts, hue, icon, 24.0);
    let t = commands
        .spawn((Text::new(title.to_string()), ui_font(&fonts.ui, 12.5), TextColor(rgb(text_primary()))))
        .id();
    commands.entity(header).add_children(&[badge, t]);
    let body = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(8.0), ..default() })
        .id();
    commands.entity(card).add_children(&[header, body]);
    (card, body)
}

fn muted_note(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((Text::new(text.to_string()), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted()))))
        .id()
}

/// A credit pack as a tinted, clickable card: credits headline, dollar price,
/// optional highlight tag. The caller attaches the click marker.
fn pack_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    hue: (u8, u8, u8),
    credits: i64,
    tag: Option<&str>,
) -> Entity {
    let cents = credits * CREDIT_USD_CENTS;
    let tagged = tag.is_some();
    // Two-up on a narrow panel; grows wider when docked large.
    let card = commands
        .spawn((
            Node {
                // ~half-width basis so four packs lay out 2×2 (and stay 2-up on a
                // narrow panel) rather than stretching into one wide row.
                flex_grow: 1.0,
                flex_basis: Val::Percent(46.0),
                min_width: Val::Px(150.0),
                position_type: PositionType::Relative,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexStart,
                row_gap: Val::Px(2.0),
                padding: UiRect::all(Val::Px(12.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(tint(hue, if tagged { 30 } else { 16 })),
            BorderColor::all(tint(hue, if tagged { 135 } else { 42 })),
            Interaction::default(),
            HoverTint::tinted(hue, if tagged { 30 } else { 16 }, 52),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    // Highlighted packs stand out via a brighter tint + border (set above) —
    // no colored halo glow (that washed out on the dark surface).
    let credits_txt = commands
        .spawn((Text::new(format!("{credits}")), ui_font(&fonts.ui, 21.0), TextColor(rgb(text_primary()))))
        .id();
    let credits_lbl = commands
        .spawn((Text::new("credits"), ui_font(&fonts.ui, 8.5), TextColor(rgb(text_muted()))))
        .id();
    let price = commands
        .spawn((
            Text::new(format!("${}.{:02}", cents / 100, cents % 100)),
            ui_font(&fonts.ui, 12.5),
            TextColor(rgb(hue)),
        ))
        .id();
    commands.entity(card).add_children(&[credits_txt, credits_lbl, price]);
    if let Some(tag) = tag {
        // A gradient badge overlapping the top-right corner (POPULAR / BEST VALUE).
        let bright = (hue.0.saturating_add(45), hue.1.saturating_add(35), hue.2.saturating_add(20));
        let badge = gradient_badge(commands, fonts, rgb(bright), rgb(hue), &tag.to_uppercase());
        commands.entity(badge).insert(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(-8.0),
            right: Val::Px(8.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            padding: UiRect::axes(Val::Px(7.0), Val::Px(2.0)),
            border_radius: BorderRadius::all(Val::Px(8.0)),
            ..default()
        });
        commands.entity(card).add_child(badge);
    }
    card
}

fn leaderboard_snapshot(w: &World) -> KeyedSnapshot {
    let Some(panel) = w.get_resource::<WalletPanel>() else {
        return util::empty_snapshot();
    };
    if panel.leaderboard.is_empty() {
        return note(if panel.leaderboard_loaded {
            "No donations yet — be the first!"
        } else {
            "Loading top donors…"
        });
    }
    let leaders = panel.leaderboard.clone();
    let items = leaders
        .iter()
        .enumerate()
        .map(|(i, l)| (i as u64, hash64(&(&l.username, &l.avatar_url, l.total))))
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |commands, fonts, i| {
            let l = &leaders[i];
            let rank = i + 1;
            let row = commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(9.0),
                        padding: UiRect::all(Val::Px(6.0)),
                        border_radius: BorderRadius::all(Val::Px(7.0)),
                        ..default()
                    },
                    BackgroundColor(if rank <= 3 {
                        tint(RANK_COLORS[rank - 1], 26)
                    } else {
                        rgba([255, 255, 255, 6])
                    }),
                ))
                .id();
            // Rank medallion: colored circle with the position number.
            let medal_color = RANK_COLORS.get(rank - 1).copied().unwrap_or((150, 150, 160));
            let medal = commands
                .spawn((
                    Node {
                        width: Val::Px(22.0),
                        height: Val::Px(22.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border_radius: BorderRadius::all(Val::Px(11.0)),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    BackgroundColor(tint(medal_color, if rank <= 3 { 220 } else { 40 })),
                ))
                .id();
            let medal_txt = commands
                .spawn((
                    Text::new(format!("{rank}")),
                    ui_font(&fonts.ui, 10.0),
                    TextColor(rgb(if rank <= 3 { (30, 30, 34) } else { text_primary() })),
                ))
                .id();
            commands.entity(medal).add_child(medal_txt);
            // Anonymous donors arrive with an empty username.
            let name = if l.username.is_empty() { "Anonymous".to_string() } else { l.username.clone() };
            let av = avatar_image(commands, fonts, l.avatar_url.as_deref(), 26.0);
            let name_txt = commands
                .spawn((
                    Text::new(name),
                    ui_font(&fonts.ui, 11.5),
                    TextColor(rgb(text_primary())),
                    Node { flex_grow: 1.0, ..default() },
                ))
                .id();
            let credits = commands
                .spawn((Text::new(format!("{} credits", l.total)), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted()))))
                .id();
            commands.entity(row).add_children(&[medal, av, name_txt, credits]);
            row
        }),
    }
}

fn note(msg: &'static str) -> KeyedSnapshot {
    KeyedSnapshot {
        items: vec![(u64::MAX, hash64(msg))],
        build: Box::new(move |commands, fonts, _| {
            commands
                .spawn((
                    Text::new(msg),
                    ui_font(&fonts.ui, 10.5),
                    TextColor(rgb(placeholder())),
                    Node { margin: UiRect::top(Val::Px(6.0)), ..default() },
                ))
                .id()
        }),
    }
}
