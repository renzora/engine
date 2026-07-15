//! Become-a-Creator panel — the marketplace seller onboarding wizard. Three
//! gated steps: accept the creator agreement, connect a Stripe payout account,
//! then start selling. The whole wizard is rebuilt from `OnboardStatus` via one
//! keyed-list token, so a status change (or the agreement checkbox toggling)
//! bumps `version` and the steps re-render with the right dimming/checks.
//!
//! Money and asset uploads live on the website: connecting a payout account and
//! uploading an asset both open a hosted page in the browser. Returning from the
//! browser won't push an update back to the engine, so a Refresh action re-polls
//! status to reflect what changed there.

use bevy::prelude::*;
use crossbeam_channel::{unbounded, Receiver, Sender};
use renzora::core::RenzoraShellExt;
use renzora::SplashState;
use renzora_auth::billing::OnboardStatus;
use renzora_auth::AuthSession;
use renzora_ember::dock::panel_active;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, bind_text, keyed_list_tokened, Bound, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    accent_button, accent_ghost, accent_icon_button, checkbox, empty_state, markdown_view,
    scroll_area, tint,
};

use crate::toasts::{ToastQueue, Tone};
use crate::util::{self, hash64, session_clone};

pub(crate) const PANEL_ID: &str = "social_onboarding";

/// Emerald-teal — growth, "go". The creator area's identity hue.
const HUE_ONBOARD: (u8, u8, u8) = (64, 196, 150);

/// The agreement, summarized faithfully. Rendered as markdown inside the
/// scrollable terms box.
const AGREEMENT: &str = "\
**1. Revenue share.** You receive **80%** of the sale price of each asset you \
sell. Renzora retains a **20%** platform fee to run the marketplace and \
services.

**2. Payouts.** Earnings are paid out through Stripe Connect to the bank \
account you link. The minimum withdrawal is **500 credits ($50)**.

**3. Ownership & license.** You keep full ownership of everything you upload. \
By listing an asset you grant Renzora a non-exclusive license to host, display, \
market, and distribute it to buyers through the marketplace.

**4. Content & quality.** You are responsible for having the rights to what you \
sell. Prohibited content (stolen, infringing, malicious, or disallowed \
material) is removed. Listings must meet the marketplace quality standards.

**5. Refunds & disputes.** Buyer refunds and disputes are handled under the \
marketplace refund policy; refunded sales are deducted from your balance.

**6. Termination.** Either party may end this agreement at any time. Pending \
payouts you've already earned are still settled after termination.";

// ── Worker results ─────────────────────────────────────────────────────────

pub(crate) enum OnboardResult {
    /// Latest creator-onboarding status.
    Status(Result<OnboardStatus, String>),
    /// The agreement was accepted (server side).
    PolicyAccepted(Result<(), String>),
    /// Stripe Connect onboarding URL — open in the browser.
    ConnectUrl(Result<String, String>),
}

#[derive(Resource)]
pub(crate) struct OnboardingPanel {
    pub status: OnboardStatus,
    /// Whether the agreement checkbox is currently ticked. Mirrored from the
    /// ember checkbox so the wizard rebuild can enable/disable the accept button.
    pub policy_checked: bool,
    /// The user chose "Skip for now" on the payout step — unlocks step 3 without
    /// a connected account (paid assets still need one).
    pub skipped_connect: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub version: u64,
    pub loaded_once: bool,
    pub tx: Sender<OnboardResult>,
    rx: Receiver<OnboardResult>,
}

impl Default for OnboardingPanel {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        Self {
            status: OnboardStatus::default(),
            policy_checked: false,
            skipped_connect: false,
            loading: false,
            error: None,
            version: 0,
            loaded_once: false,
            tx,
            rx,
        }
    }
}

impl OnboardingPanel {
    pub fn bump(&mut self) {
        self.version = self.version.wrapping_add(1);
    }

    /// Fully onboarded = agreement accepted AND Stripe payout account ready.
    fn all_set(&self) -> bool {
        self.status.policy_accepted && self.status.stripe_onboarded
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_thread(f: impl FnOnce() + Send + 'static) {
    std::thread::spawn(f);
}
#[cfg(target_arch = "wasm32")]
fn spawn_thread(_f: impl FnOnce() + Send + 'static) {}

/// Open a URL in the user's default browser (Stripe / marketplace pages).
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
    app.init_resource::<OnboardingPanel>();
    app.register_shell_panel(PANEL_ID, "Become a Creator", "seal-check", "Community");
    app.register_panel_content(PANEL_ID, true, build);
    app.add_systems(
        Update,
        (poll_results, auto_load.run_if(panel_active(PANEL_ID)), clicks)
            .run_if(in_state(SplashState::Editor)),
    );
}

// ── Systems ────────────────────────────────────────────────────────────────

fn poll_results(
    mut panel: ResMut<OnboardingPanel>,
    session: Res<AuthSession>,
    mut toasts: ResMut<ToastQueue>,
) {
    let mut got = Vec::new();
    while let Ok(r) = panel.rx.try_recv() {
        got.push(r);
    }
    for r in got {
        match r {
            OnboardResult::Status(Ok(status)) => {
                panel.status = status;
                panel.loading = false;
                panel.error = None;
                panel.bump();
            }
            OnboardResult::Status(Err(e)) => {
                panel.loading = false;
                panel.error = Some(e.clone());
                toasts.push(Tone::Error, e, None);
                panel.bump();
            }
            OnboardResult::PolicyAccepted(Ok(())) => {
                toasts.push(Tone::Success, "Agreement accepted", None);
                // Re-poll so `policy_accepted` flips and step 2 unlocks.
                reload_status(&mut panel, &session);
            }
            OnboardResult::PolicyAccepted(Err(e)) => {
                toasts.push(Tone::Error, format!("Couldn't accept: {e}"), None);
            }
            OnboardResult::ConnectUrl(Ok(url)) => {
                open_url(&url);
                toasts.push(Tone::Info, "Opening Stripe onboarding in your browser…", None);
            }
            OnboardResult::ConnectUrl(Err(e)) => {
                toasts.push(Tone::Error, format!("Couldn't start onboarding: {e}"), None);
            }
        }
    }
}

/// One-shot status fetch once signed in. `loaded_once` is set at spawn time so a
/// failure can't loop; Refresh re-polls on demand.
fn auto_load(mut panel: ResMut<OnboardingPanel>, session: Res<AuthSession>) {
    if session.is_signed_in() && !panel.loaded_once {
        panel.loaded_once = true;
        panel.loading = true;
        let tx = panel.tx.clone();
        let s = session_clone(&session);
        spawn_thread(move || {
            let _ = tx.send(OnboardResult::Status(renzora_auth::billing::onboard_status(&s)));
        });
    }
}

/// Dispatch a fresh status poll on a worker.
fn reload_status(panel: &mut OnboardingPanel, session: &AuthSession) {
    panel.loading = true;
    let tx = panel.tx.clone();
    let s = session_clone(session);
    spawn_thread(move || {
        let _ = tx.send(OnboardResult::Status(renzora_auth::billing::onboard_status(&s)));
    });
}

// ── Clicks ─────────────────────────────────────────────────────────────────

#[derive(Component)]
struct PolicyCheckbox;
#[derive(Component)]
struct AcceptBtn;
#[derive(Component)]
struct ConnectBtn;
#[derive(Component)]
struct SkipBtn;
#[derive(Component)]
struct UploadBtn;
#[derive(Component)]
struct RefreshBtn;

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn clicks(
    mut panel: ResMut<OnboardingPanel>,
    session: Res<AuthSession>,
    mut toasts: ResMut<ToastQueue>,
    mut commands: Commands,
    checks: Query<&Bound<bool>, (With<PolicyCheckbox>, Changed<Bound<bool>>)>,
    accepts: Query<&Interaction, (With<AcceptBtn>, Changed<Interaction>)>,
    connects: Query<&Interaction, (With<ConnectBtn>, Changed<Interaction>)>,
    skips: Query<&Interaction, (With<SkipBtn>, Changed<Interaction>)>,
    uploads: Query<&Interaction, (With<UploadBtn>, Changed<Interaction>)>,
    refreshes: Query<&Interaction, (With<RefreshBtn>, Changed<Interaction>)>,
) {
    let pressed = |i: &Interaction| *i == Interaction::Pressed;

    // Mirror the checkbox into resource state — but only on a genuine change, or
    // the freshly-spawned checkbox after each rebuild would re-fire `Changed`
    // and bump forever (rebuild → new checkbox → Changed → bump → rebuild …).
    for b in &checks {
        if b.0 != panel.policy_checked {
            panel.policy_checked = b.0;
            panel.bump();
        }
    }

    if accepts.iter().any(pressed) {
        if !panel.policy_checked {
            toasts.push(Tone::Warn, "Please read and agree to the terms first", None);
        } else {
            let tx = panel.tx.clone();
            let s = session_clone(&session);
            spawn_thread(move || {
                let r = renzora_auth::billing::accept_policy(&s).map(|_| ());
                let _ = tx.send(OnboardResult::PolicyAccepted(r));
            });
        }
    }

    if connects.iter().any(pressed) {
        let tx = panel.tx.clone();
        let s = session_clone(&session);
        spawn_thread(move || {
            let _ = tx.send(OnboardResult::ConnectUrl(renzora_auth::billing::start_connect_onboarding(&s)));
        });
    }

    if skips.iter().any(pressed) && !panel.skipped_connect {
        panel.skipped_connect = true;
        panel.bump();
    }

    if uploads.iter().any(pressed) {
        // Open the in-editor Publish panel (the engine's own uploader), not the
        // website — mirrors the Marketplace panel's "Upload Asset" button.
        commands.queue(|world: &mut World| {
            renzora_ember::dock::open_or_focus_panel(world, "asset_uploader");
        });
    }

    if refreshes.iter().any(pressed) && session.is_signed_in() {
        reload_status(&mut panel, &session);
    }
}

// ── Build ──────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let hue = HUE_ONBOARD;
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(10.0),
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        })
        .id();

    // Signed-out gate.
    let signed_out = empty_state(
        commands,
        fonts,
        hue,
        "seal-check",
        "Sign in to become a creator",
        Some("Sell your assets on the Renzora Marketplace"),
    );
    bind_display(commands, signed_out, |w| !util::signed_in(w));

    // Signed-in body: header (title + refresh), error line, then the wizard.
    let body = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(10.0),
            ..default()
        })
        .id();
    bind_display(commands, body, util::signed_in);

    let header = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(9.0),
            ..default()
        })
        .id();
    let head_col = commands
        .spawn(Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, ..default() })
        .id();
    let title = commands
        .spawn((Text::new("Become a Creator"), ui_font(&fonts.ui, 14.5), TextColor(rgb(text_primary()))))
        .id();
    let subtitle = commands
        .spawn((
            Text::new("Complete these steps to start selling on the Renzora Marketplace."),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    commands.entity(head_col).add_children(&[title, subtitle]);
    let refresh = accent_icon_button(commands, fonts, hue, "arrow-clockwise");
    commands.entity(refresh).insert(RefreshBtn);
    commands.entity(header).add_children(&[head_col, refresh]);

    let error = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 10.5), TextColor(rgb((224, 80, 80)))))
        .id();
    bind_text(commands, error, |w| {
        w.get_resource::<OnboardingPanel>().and_then(|p| p.error.clone()).unwrap_or_default()
    });
    bind_display(commands, error, |w| {
        w.get_resource::<OnboardingPanel>().map(|p| p.error.is_some()).unwrap_or(false)
    });

    // The wizard is rebuilt from status via one token, so gating/checks stay
    // correct as the steps complete.
    let wizard = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(10.0), ..default() })
        .id();
    keyed_list_tokened(
        commands,
        wizard,
        |w| w.get_resource::<OnboardingPanel>().map(|p| p.version).unwrap_or(0),
        wizard_snapshot,
    );

    commands.entity(body).add_children(&[header, error, wizard]);
    commands.entity(root).add_children(&[signed_out, body]);
    root
}

// ── Wizard snapshot ──────────────────────────────────────────────────────────

fn wizard_snapshot(w: &World) -> KeyedSnapshot {
    let Some(panel) = w.get_resource::<OnboardingPanel>() else {
        return util::empty_snapshot();
    };
    let status = panel.status;
    let policy_checked = panel.policy_checked;
    let skipped = panel.skipped_connect;
    let all_set = panel.all_set();

    // A single item that carries the whole wizard: its content hash is every
    // input to how the steps render, so any state change rebuilds it.
    let content = hash64(&(
        status.policy_accepted,
        status.stripe_connected,
        status.stripe_onboarded,
        policy_checked,
        skipped,
        all_set,
    ));
    KeyedSnapshot {
        items: vec![(1, content)],
        build: Box::new(move |commands, fonts, _| {
            let hue = HUE_ONBOARD;
            let col = commands
                .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(10.0), ..default() })
                .id();

            // Fully onboarded → celebrate instead of showing the wizard.
            if all_set {
                let done = all_set_card(commands, fonts, hue);
                commands.entity(col).add_child(done);
                return col;
            }

            // Progress dots: filled per completed step.
            let filled = status.policy_accepted as usize
                + status.stripe_onboarded as usize
                + (status.policy_accepted && (status.stripe_onboarded || skipped)) as usize;
            let dots = progress_dots(commands, hue, filled);
            commands.entity(col).add_child(dots);

            // ── Step 1 — Creator Policy ──
            let step1_done = status.policy_accepted;
            let step1 = step_card(commands, fonts, hue, 1, "Creator Policy", false, step1_done);
            let terms_title = commands
                .spawn((
                    Text::new("Renzora Marketplace Creator Agreement"),
                    ui_font(&fonts.ui, 11.0),
                    TextColor(rgb(text_primary())),
                ))
                .id();
            let terms_md = markdown_view(commands, fonts, AGREEMENT);
            let terms_scroll = scroll_area(commands, terms_md, 160.0);
            // Chrome wrapper around the scroll area — styling the scroll
            // viewport's own Node directly would clobber the fields the scroll
            // machinery depends on (overflow, max-height, column layout).
            let terms_box = commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::all(Val::Px(8.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(7.0)),
                        ..default()
                    },
                    BackgroundColor(rgb(section_bg())),
                    BorderColor::all(rgba([255, 255, 255, 10])),
                ))
                .id();
            commands.entity(terms_box).add_child(terms_scroll);
            commands.entity(step1).add_children(&[terms_title, terms_box]);
            if step1_done {
                let ok = done_row(commands, fonts, hue, "Accepted");
                commands.entity(step1).add_child(ok);
            } else {
                let agree_row = commands
                    .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(7.0), ..default() })
                    .id();
                let cb = checkbox(commands, policy_checked);
                commands.entity(cb).insert(PolicyCheckbox);
                let cb_label = commands
                    .spawn((
                        Text::new("I have read and agree to the Renzora Marketplace Creator Agreement"),
                        ui_font(&fonts.ui, 10.0),
                        TextColor(rgb(text_muted())),
                    ))
                    .id();
                commands.entity(agree_row).add_children(&[cb, cb_label]);
                // Enabled look only when checked (the click handler also guards).
                let accept = if policy_checked {
                    accent_button(commands, fonts, hue, "Accept & Continue")
                } else {
                    let b = accent_button(commands, fonts, hue, "Accept & Continue");
                    commands.entity(b).insert(BackgroundColor(tint(hue, 70)));
                    b
                };
                commands.entity(accept).insert(AcceptBtn);
                commands.entity(step1).add_children(&[agree_row, accept]);
            }
            commands.entity(col).add_child(step1);

            // ── Step 2 — Connect Payment Account (dimmed until step 1 done) ──
            let step2_enabled = status.policy_accepted;
            let step2_done = status.stripe_onboarded;
            let step2 = step_card(commands, fonts, hue, 2, "Connect Payment Account", !step2_enabled, step2_done);
            let step2_note = commands
                .spawn((
                    Text::new("Connect your bank through Stripe to receive payouts when your assets sell."),
                    ui_font(&fonts.ui, 10.0),
                    TextColor(rgb(text_muted())),
                ))
                .id();
            commands.entity(step2).add_child(step2_note);
            if step2_done {
                let ok = done_row(commands, fonts, hue, "Connected");
                commands.entity(step2).add_child(ok);
            } else if step2_enabled {
                let btn_row = commands
                    .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() })
                    .id();
                let connect = accent_button(commands, fonts, hue, "Connect with Stripe");
                commands.entity(connect).insert(ConnectBtn);
                let skip = accent_ghost(commands, fonts, hue, "Skip for now");
                commands.entity(skip).insert(SkipBtn);
                commands.entity(btn_row).add_children(&[connect, skip]);
                let skip_note = commands
                    .spawn((Text::new("Required for paid assets"), ui_font(&fonts.ui, 9.0), TextColor(rgb(placeholder()))))
                    .id();
                commands.entity(step2).add_children(&[btn_row, skip_note]);
            }
            commands.entity(col).add_child(step2);

            // ── Step 3 — Start Selling (dimmed until reached) ──
            let step3_enabled = status.policy_accepted && (status.stripe_onboarded || skipped);
            let step3 = step_card(commands, fonts, hue, 3, "Start Selling", !step3_enabled, false);
            let step3_note = commands
                .spawn((
                    Text::new("You're ready to upload your first asset!"),
                    ui_font(&fonts.ui, 10.0),
                    TextColor(rgb(text_muted())),
                ))
                .id();
            commands.entity(step3).add_child(step3_note);
            if step3_enabled {
                let upload = accent_button(commands, fonts, hue, "Upload an asset");
                commands.entity(upload).insert(UploadBtn);
                commands.entity(step3).add_child(upload);
            }
            commands.entity(col).add_child(step3);

            col
        }),
    }
}

// ── Widgets ──────────────────────────────────────────────────────────────────

/// A row of three progress dots; the first `filled` are lit in the hue.
fn progress_dots(commands: &mut Commands, hue: (u8, u8, u8), filled: usize) -> Entity {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    for i in 0..3 {
        let dot = commands
            .spawn((
                Node {
                    width: Val::Px(9.0),
                    height: Val::Px(9.0),
                    border_radius: BorderRadius::all(Val::Px(4.5)),
                    ..default()
                },
                BackgroundColor(if i < filled { tint(hue, 255) } else { tint(hue, 45) }),
            ))
            .id();
        commands.entity(row).add_child(dot);
    }
    row
}

/// A wizard step card: numbered/checked badge, title, optional dimming when the
/// step isn't reachable yet. Children (body) are appended by the caller.
fn step_card(
    commands: &mut Commands,
    fonts: &EmberFonts,
    hue: (u8, u8, u8),
    number: u32,
    title: &str,
    dimmed: bool,
    done: bool,
) -> Entity {
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
            BackgroundColor(rgb(card_bg())),
            BorderColor::all(if done { tint(hue, 70) } else { rgba([255, 255, 255, 10]) }),
        ))
        .id();
    let head = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() })
        .id();
    // Badge: check when done, else the step number.
    let badge = commands
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
            BackgroundColor(tint(hue, if done || !dimmed { 200 } else { 45 })),
        ))
        .id();
    if done {
        let ic = icon_text(commands, &fonts.phosphor, "check", (30, 30, 34), 12.0);
        commands.entity(badge).add_child(ic);
    } else {
        let n = commands
            .spawn((
                Text::new(format!("{number}")),
                ui_font(&fonts.ui, 11.0),
                TextColor(rgb(if dimmed { text_muted() } else { (30, 30, 34) })),
            ))
            .id();
        commands.entity(badge).add_child(n);
    }
    let step_label = commands
        .spawn((Text::new(format!("Step {number}")), ui_font(&fonts.ui, 8.5), TextColor(rgb(placeholder()))))
        .id();
    let step_title = commands
        .spawn((
            Text::new(title.to_string()),
            ui_font(&fonts.ui, 12.5),
            TextColor(rgb(if dimmed { text_muted() } else { text_primary() })),
        ))
        .id();
    let title_col = commands
        .spawn(Node { flex_direction: FlexDirection::Column, ..default() })
        .id();
    commands.entity(title_col).add_children(&[step_label, step_title]);
    commands.entity(head).add_children(&[badge, title_col]);
    commands.entity(card).add_child(head);
    card
}

/// A green "done" row: check icon + label, for accepted/connected states.
fn done_row(commands: &mut Commands, fonts: &EmberFonts, hue: (u8, u8, u8), label: &str) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(9.0), Val::Px(5.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                align_self: AlignSelf::FlexStart,
                ..default()
            },
            BackgroundColor(tint(hue, 34)),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, "check-circle", hue, 13.0);
    let t = commands
        .spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 10.5), TextColor(rgb(hue))))
        .id();
    commands.entity(row).add_children(&[ic, t]);
    row
}

/// The all-set celebration shown once the creator is fully onboarded.
fn all_set_card(commands: &mut Commands, fonts: &EmberFonts, hue: (u8, u8, u8)) -> Entity {
    let card = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(10.0),
                padding: UiRect::all(Val::Px(20.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(12.0)),
                ..default()
            },
            BackgroundColor(tint(hue, 24)),
            BorderColor::all(tint(hue, 60)),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, "seal-check", hue, 34.0);
    let title = commands
        .spawn((Text::new("You're all set!"), ui_font(&fonts.ui, 14.5), TextColor(rgb(text_primary()))))
        .id();
    let sub = commands
        .spawn((
            Text::new("Your creator account is ready — upload assets and start selling."),
            ui_font(&fonts.ui, 10.5),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let upload = accent_button(commands, fonts, hue, "Upload an asset");
    commands.entity(upload).insert(UploadBtn);
    commands.entity(card).add_children(&[ic, title, sub, upload]);
    card
}
