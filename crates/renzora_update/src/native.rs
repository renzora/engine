//! The Software Update dialog (ember / bevy_ui).
//!
//! A single centered modal: what you're running, what's available, the release
//! notes, and one action button whose meaning moves through the update — Check →
//! Download → Install & Restart. The previous updater's dialog was egui and did
//! not survive the move to bevy_ui; this is a fresh one built on the same
//! `overlay_sized` card the About modal uses.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_bg, bind_display, bind_text, bind_text_color, bind_with};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::widgets::{overlay_sized, scroll_area};

use crate::UpdateState;

const GREEN: (u8, u8, u8) = (89, 191, 115);
const AMBER: (u8, u8, u8) = (242, 166, 64);
const RED: (u8, u8, u8) = (239, 68, 68);

pub(crate) fn register(app: &mut App) {
    app.add_systems(
        Update,
        (
            manage_modal,
            rebuild_version_list,
            version_row_click,
            action_label_sync,
            action_click,
            channel_click,
            notes_link_click,
            close_click,
        ),
    );
}

// ── Markers ──────────────────────────────────────────────────────────────────

#[derive(Component)]
struct UpdateRoot;
/// The one button whose meaning depends on where you are in the update.
#[derive(Component)]
struct ActionBtn;
#[derive(Component)]
struct NotesLinkBtn;
#[derive(Component, Clone, Copy)]
struct ChannelBtn(&'static str);
#[derive(Component)]
struct CloseBtn;
/// The scrollable version list; `sig` is what it was last built from.
#[derive(Component)]
struct VersionList {
    sig: u64,
}
/// One row in that list, carrying the tag it selects.
#[derive(Component)]
struct VersionRow(String);

/// Colour with an explicit alpha. Ember's theme exposes only opaque `rgb`,
/// and these tints have to sit over whatever the card behind them is.
fn ca(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color::srgba(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0)
}

/// Reactive `BorderColor`. Ember has `bind_bg` but no border equivalent.
fn bind_border<F>(commands: &mut Commands, target: Entity, value: F)
where
    F: for<'w> Fn(&Rx<'w>) -> Color + Send + Sync + 'static,
{
    bind_with(commands, target, value, |world, e, c| {
        if let Ok(mut ent) = world.get_entity_mut(e) {
            ent.insert(BorderColor::all(*c));
        }
    });
}

/// Reactive icon: swap the phosphor glyph on an existing icon entity.
///
/// `icon_text` resolves a name to a glyph once at spawn. The action button's
/// meaning changes as the update progresses, so its icon has to follow — an
/// unchanging download arrow on a button that now says "Confirm" is worse
/// than no icon.
fn bind_icon<F>(commands: &mut Commands, target: Entity, name: F)
where
    F: for<'w> Fn(&Rx<'w>) -> &'static str + Send + Sync + 'static,
{
    bind_with(commands, target, name, |world, e, n| {
        let ch = renzora_ember::phosphor_map::icon_glyph(n).unwrap_or('\u{E4C6}');
        if let Some(mut t) = world.get_mut::<Text>(e) {
            t.0 = ch.to_string();
        }
    });
}

/// Settled, checked, and genuinely nothing to install.
///
/// Excludes `no_platform_builds`: a green tick beside "No builds for your
/// platform" would be reassuring about the wrong thing.
fn up_to_date(w: &Rx<'_>) -> bool {
    w.get_resource::<UpdateState>().is_some_and(|s| {
        !s.checking
            && !s.downloading()
            && s.staged.is_none()
            && s.result
                .as_ref()
                .is_some_and(|r| !r.update_available && !r.no_platform_builds)
    })
}

/// Phosphor icon for whatever the action button currently means.
fn action_icon(a: Action, checkout: bool) -> &'static str {
    match a {
        Action::Check => "arrows-clockwise",
        Action::Download => "download-simple",
        Action::ConfirmOverwrite => "warning-circle",
        // Restarting into the new build is the actual effect, so say that
        // rather than reusing the download arrow.
        Action::Install if checkout => "warning-circle",
        Action::Install => "arrow-clockwise",
        Action::None => "check-circle",
    }
}

/// What the action button does right now.
#[derive(PartialEq, Eq, Clone, Copy)]
enum Action {
    Check,
    Download,
    /// Arm the overwrite confirmation. Only reachable from a source checkout,
    /// where installing replaces the `dist/` tree the build stages into — one
    /// click says what will happen, the next one does it.
    ConfirmOverwrite,
    Install,
    /// Nothing to do — checking, downloading, or up to date.
    None,
}

fn action_for(s: &UpdateState) -> Action {
    if s.checking || s.downloading() {
        return Action::None;
    }
    if !s.can_install() {
        // Layout detection failed, so there is nowhere to install to. The error
        // line already says why.
        return Action::None;
    }
    if s.staged.is_some() {
        return if s.is_source_checkout() && !s.overwrite_armed {
            Action::ConfirmOverwrite
        } else {
            Action::Install
        };
    }
    match s.target() {
        // Downloading is always safe: it writes to ~/.renzora/updates, never to
        // the install. Only the install itself needs confirming.
        //
        // Keyed off the SELECTED entry, not "is there an update", because the
        // version list can point at an older release — rolling back is a
        // download like any other. The one thing never worth downloading is the
        // build already running.
        Some(e) if e.download_url.is_some() && !e.is_current => Action::Download,
        _ => Action::Check,
    }
}

fn action_label(s: &UpdateState) -> String {
    match action_for(s) {
        Action::Check => renzora::lang::t("update.btn.check"),
        Action::Download => renzora::lang::t("update.btn.download"),
        Action::ConfirmOverwrite => renzora::lang::t("update.btn.overwrite"),
        Action::Install if s.is_source_checkout() => {
            renzora::lang::t("update.btn.confirm_overwrite")
        }
        Action::Install => renzora::lang::t("update.btn.install"),
        Action::None => String::new(),
    }
}

// ── Lifecycle ────────────────────────────────────────────────────────────────

/// Spawn/despawn the modal to match `UpdateState::visible`.
///
/// `spawned` is what stops the dialog resurrecting itself. Escape, a backdrop
/// click and the card's × are all handled by ember's generic `overlay_dismiss`,
/// which despawns the root without knowing anything about this crate — so
/// "visible, but no root" is ambiguous: either we have not spawned it yet, or
/// the user just dismissed it. Without the flag the second case looks like the
/// first and the modal reopens on the very next frame, every frame.
fn manage_modal(world: &mut World, mut spawned: Local<bool>) {
    let visible = world
        .get_resource::<UpdateState>()
        .is_some_and(|s| s.visible);
    let mut q = world.query_filtered::<Entity, With<UpdateRoot>>();
    let existing: Vec<Entity> = q.iter(world).collect();

    match (visible, existing.is_empty(), *spawned) {
        (true, true, false) => {
            let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
                return;
            };
            let mut queue = CommandQueue::default();
            {
                let mut commands = Commands::new(&mut queue, world);
                spawn_modal(&mut commands, &fonts);
            }
            queue.apply(world);
            *spawned = true;
        }
        // Dismissed by ember: follow it rather than fight it.
        (true, true, true) => {
            *spawned = false;
            if let Some(mut s) = world.get_resource_mut::<UpdateState>() {
                s.visible = false;
            }
        }
        (false, false, _) => {
            for e in existing {
                world.entity_mut(e).despawn();
            }
            *spawned = false;
        }
        _ => {}
    }
}

fn spawn_modal(commands: &mut Commands, fonts: &EmberFonts) {
    let (root, content) = overlay_sized(
        commands,
        fonts,
        &renzora::lang::t("update.title"),
        560.0,
        620.0,
        true,
    );
    commands.entity(root).insert(UpdateRoot);

    let body = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            row_gap: Val::Px(10.0),
            padding: UiRect::all(Val::Px(18.0)),
            ..default()
        })
        .id();

    // ── Headline: the answer, in one line, in a card that carries the mood ───
    // Green with a tick when up to date, accent-tinted while there is something
    // to do. A one-word state change is much easier to read as a colour than as
    // a sentence you have to finish.
    let card = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            BorderColor::all(rgb(divider())),
        ))
        .id();
    bind_bg(commands, card, |w| {
        if up_to_date(w) {
            ca(GREEN.0, GREEN.1, GREEN.2, 28)
        } else {
            ca(255, 255, 255, 10)
        }
    });
    bind_border(commands, card, |w| {
        if up_to_date(w) {
            ca(GREEN.0, GREEN.1, GREEN.2, 120)
        } else {
            rgb(divider())
        }
    });

    let status_icon = icon_text(commands, &fonts.phosphor, "check-circle", GREEN, 20.0);
    commands.entity(status_icon).insert(FocusPolicy::Pass);
    bind_icon(commands, status_icon, |w| {
        if up_to_date(w) {
            "check-circle"
        } else {
            "download-simple"
        }
    });
    bind_text_color(commands, status_icon, |w| {
        if up_to_date(w) {
            rgb(GREEN)
        } else {
            rgb(accent())
        }
    });

    let headline = commands
        .spawn((
            Text::new(String::new()),
            ui_font(&fonts.ui, 16.0),
            TextColor(rgb(text_primary())),
            FocusPolicy::Pass,
        ))
        .id();
    bind_text(commands, headline, |w| {
        let Some(s) = w.get_resource::<UpdateState>() else {
            return String::new();
        };
        if s.checking {
            return renzora::lang::t("update.checking");
        }
        if s.downloading() {
            return renzora::lang::t("update.downloading");
        }
        if s.staged.is_some() {
            return renzora::lang::t("update.ready");
        }
        match s.result.as_ref() {
            Some(r) if r.no_platform_builds => {
                renzora::lang::t("update.no_platform_build")
            }
            Some(r) if r.update_available => format!(
                "{} {}",
                renzora::lang::t("update.available"),
                r.latest_version.clone().unwrap_or_default()
            ),
            Some(_) => renzora::lang::t("update.up_to_date"),
            None => renzora::lang::t("update.checking"),
        }
    });

    commands.entity(card).add_children(&[status_icon, headline]);

    // Current version + channel, always visible so the dialog answers "what am
    // I even running" without a second trip to About.
    let sub = commands
        .spawn((
            Text::new(String::new()),
            ui_font(&fonts.ui, 11.5),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_text(commands, sub, |w| {
        let Some(s) = w.get_resource::<UpdateState>() else {
            return String::new();
        };
        let installed = renzora::version::display();
        match s.layout.as_ref() {
            Some(l) if l.is_source_checkout => format!(
                "{installed} — {}",
                renzora::lang::t("update.source_checkout")
            ),
            _ => format!("{} {installed}", renzora::lang::t("update.installed")),
        }
    });

    // ── Channel picker ───────────────────────────────────────────────────────
    let channels = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    let clabel = commands
        .spawn((
            Text::new(renzora::lang::t("update.channel")),
            ui_font(&fonts.ui, 11.5),
            TextColor(rgb(text_muted())),
            Node {
                margin: UiRect::right(Val::Px(4.0)),
                ..default()
            },
        ))
        .id();
    commands.entity(channels).add_child(clabel);
    for (pref, key) in [
        ("auto", "update.channel.auto"),
        ("stable", "update.channel.stable"),
        ("nightly", "update.channel.nightly"),
    ] {
        let chip = channel_chip(commands, fonts, pref, &renzora::lang::t(key));
        commands.entity(channels).add_child(chip);
    }

    let rule = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(1.0),
                ..default()
            },
            BackgroundColor(rgb(divider())),
        ))
        .id();

    // ── Release notes ────────────────────────────────────────────────────────
    // ── Version list ─────────────────────────────────────────────────────────
    // Every version the channel offers, newest first, with the running one
    // marked. Picking an older one is allowed on purpose: rolling back is a
    // legitimate thing to want, and the check already fetched the whole list.
    let list_label = commands
        .spawn((
            Text::new(renzora::lang::t("update.versions")),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let list_col = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            VersionList { sig: 0 },
        ))
        .id();
    let list_scroll = scroll_area(commands, list_col, 130.0);

    let notes_col = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .id();
    let notes = commands
        .spawn((
            Text::new(String::new()),
            ui_font(&fonts.ui, 11.5),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_text(commands, notes, |w| {
        w.get_resource::<UpdateState>()
            .and_then(|s| s.target().and_then(|e| e.notes.clone()))
            .map(|n| {
                // Plain text in a plain label: the notes are markdown and this
                // is not a markdown renderer. Trimming the length keeps one
                // verbose release from pushing the buttons off the card.
                let n = n.replace("\r\n", "\n");
                if n.chars().count() > 1200 {
                    let cut: String = n.chars().take(1200).collect();
                    format!("{cut}…")
                } else {
                    n
                }
            })
            .unwrap_or_default()
    });
    commands.entity(notes_col).add_child(notes);
    let notes_scroll = scroll_area(commands, notes_col, 130.0);

    // ── Progress + error ─────────────────────────────────────────────────────
    let progress = commands
        .spawn((
            Text::new(String::new()),
            ui_font(&fonts.ui, 11.5),
            TextColor(rgb(accent())),
        ))
        .id();
    bind_text(commands, progress, |w| {
        let Some(s) = w.get_resource::<UpdateState>() else {
            return String::new();
        };
        match s.progress {
            Some((got, total)) if total > 0 => format!(
                "{:.0}%  ({:.1} / {:.1} MB)",
                got as f64 / total as f64 * 100.0,
                got as f64 / 1_000_000.0,
                total as f64 / 1_000_000.0
            ),
            Some((got, _)) => format!("{:.1} MB", got as f64 / 1_000_000.0),
            None => String::new(),
        }
    });
    bind_display(commands, progress, |w| {
        w.get_resource::<UpdateState>()
            .is_some_and(|s| s.progress.is_some())
    });

    let error = commands
        .spawn((
            Text::new(String::new()),
            ui_font(&fonts.ui, 11.5),
            TextColor(rgb(RED)),
        ))
        .id();
    bind_text(commands, error, |w| {
        w.get_resource::<UpdateState>()
            .and_then(|s| s.error.clone())
            .map(|e| format!("⚠ {e}"))
            .unwrap_or_default()
    });
    bind_display(commands, error, |w| {
        w.get_resource::<UpdateState>().is_some_and(|s| s.error.is_some())
    });

    let warning = overwrite_warning(commands, fonts);

    // ── Buttons ──────────────────────────────────────────────────────────────
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        })
        .id();
    let spacer = commands
        .spawn(Node {
            flex_grow: 1.0,
            ..default()
        })
        .id();

    let (notes_link, _) = pill(commands, fonts, "arrow-square-out", &renzora::lang::t("update.btn.notes"));
    commands.entity(notes_link).insert(NotesLinkBtn);
    bind_display(commands, notes_link, |w| {
        w.get_resource::<UpdateState>()
            .and_then(|s| s.result.as_ref())
            .is_some_and(|r| r.release_url.is_some())
    });

    let (close, _) = pill(commands, fonts, "x", &renzora::lang::t("update.btn.later"));
    commands.entity(close).insert(CloseBtn);

    let (action, action_ic) = pill(commands, fonts, "download-simple", "");
    commands.entity(action).insert(ActionBtn);
    bind_icon(commands, action_ic, |w| match w.get_resource::<UpdateState>() {
        Some(s) => action_icon(action_for(s), s.is_source_checkout()),
        None => "download-simple",
    });
    bind_display(commands, action, |w| {
        w.get_resource::<UpdateState>()
            .is_some_and(|s| action_for(s) != Action::None)
    });
    // The action button carries its own weight: green for a normal install,
    // amber to arm an overwrite, red once armed. A destructive button that looks
    // like every other button is how you get a stray click.
    bind_bg(commands, action, |w| {
        let Some(s) = w.get_resource::<UpdateState>() else {
            return rgb(accent());
        };
        match action_for(s) {
            Action::ConfirmOverwrite => rgb(AMBER),
            Action::Install if s.is_source_checkout() => rgb(RED),
            Action::Install => rgb(GREEN),
            _ => rgb(accent()),
        }
    });

    commands
        .entity(row)
        .add_children(&[notes_link, spacer, close, action]);

    commands.entity(body).add_children(&[
        card,
        sub,
        channels,
        rule,
        list_label,
        list_scroll,
        notes_scroll,
        progress,
        error,
        warning,
        row,
    ]);
    commands.entity(content).add_child(body);
}

/// A small pill button: icon + label. Returns `(button, icon)` — the icon is
/// handed back because the action button's glyph changes with its meaning and
/// binding needs the entity that owns the `Text`.
fn pill(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str) -> (Entity, Entity) {
    let btn = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
            Interaction::default(),
            FocusPolicy::Block,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_primary(), 12.0);
    commands.entity(ic).insert(FocusPolicy::Pass);
    let t = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.5),
            TextColor(rgb(text_primary())),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(btn).add_children(&[ic, t]);
    (btn, ic)
}

/// Open a URL in the user's browser.
///
/// Local rather than shared: every crate that needs this has its own three-line
/// copy (the shell, the hub, the splash, the palette), and adding an eighth is
/// less disruptive than promoting one of them mid-feature.
fn open_url(url: &str) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

fn channel_chip(
    commands: &mut Commands,
    fonts: &EmberFonts,
    pref: &'static str,
    label: &str,
) -> Entity {
    let chip = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(9.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            FocusPolicy::Block,
            ChannelBtn(pref),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    bind_bg(commands, chip, move |w| {
        let selected = w
            .get_resource::<UpdateState>()
            .is_some_and(|s| s.channel_pref == pref);
        if selected {
            rgb(accent())
        } else {
            rgb(section_bg())
        }
    });
    let t = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
            FocusPolicy::Pass,
        ))
        .id();
    commands.entity(chip).add_child(t);
    chip
}

// ── Version list ─────────────────────────────────────────────────────────────

/// Rebuild the list only when what it shows actually changed.
///
/// A content signature rather than a per-frame rebuild: respawning rows every
/// frame is how a reactive UI ends up allocating hundreds of entities a frame
/// and losing scroll position. The signature covers the tags, which one is
/// selected, and whether something is staged — everything a row renders.
fn version_list_sig(s: &UpdateState) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100_0000_01b3);
        }
    };
    if let Some(r) = s.result.as_ref() {
        for e in &r.releases {
            feed(e.tag.as_bytes());
            feed(&[e.is_current as u8, e.is_newer as u8]);
        }
    }
    feed(s.selected_tag.as_deref().unwrap_or("-").as_bytes());
    feed(&[s.staged.is_some() as u8, s.checking as u8]);
    h
}

fn rebuild_version_list(world: &mut World) {
    let Some(state) = world.get_resource::<UpdateState>() else {
        return;
    };
    let sig = version_list_sig(state);

    let mut q = world.query::<(Entity, &VersionList)>();
    let Some((list, current)) = q.iter(world).next().map(|(e, v)| (e, v.sig)) else {
        return;
    };
    if current == sig {
        return;
    }

    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
        return;
    };
    let rows: Vec<(String, bool, bool, bool)> = world
        .get_resource::<UpdateState>()
        .and_then(|s| {
            let selected = s.target().map(|e| e.tag.clone());
            s.result.as_ref().map(|r| {
                r.releases
                    .iter()
                    .map(|e| {
                        (
                            e.tag.clone(),
                            e.is_nightly,
                            e.is_current,
                            selected.as_deref() == Some(e.tag.as_str()),
                        )
                    })
                    .collect()
            })
        })
        .unwrap_or_default();

    // Despawn the old rows before building the new ones. `try_despawn` because a
    // row may already be gone if the overlay closed in the same frame.
    let old: Vec<Entity> = world
        .get::<Children>(list)
        .map(|c| c.iter().collect())
        .unwrap_or_default();

    let mut queue = bevy::ecs::world::CommandQueue::default();
    {
        let mut commands = Commands::new(&mut queue, world);
        for e in old {
            commands.entity(e).try_despawn();
        }
        let mut kids = Vec::new();
        for (tag, is_nightly, is_current, selected) in rows {
            kids.push(version_row(
                &mut commands,
                &fonts,
                &tag,
                is_nightly,
                is_current,
                selected,
            ));
        }
        commands.entity(list).replace_children(&kids);
        commands.entity(list).insert(VersionList { sig });
    }
    queue.apply(world);
}

fn version_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    tag: &str,
    is_nightly: bool,
    is_current: bool,
    selected: bool,
) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(if selected {
                ca(255, 255, 255, 18)
            } else {
                Color::NONE
            }),
            Interaction::default(),
            FocusPolicy::Block,
            VersionRow(tag.to_string()),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();

    // A nightly and a release are different kinds of thing, not different
    // shades of one — give them different glyphs rather than only a colour.
    let icon_name = if is_nightly { "moon-stars" } else { "seal-check" };
    let icon_colour = if is_nightly { text_muted() } else { GREEN };
    let ic = icon_text(commands, &fonts.phosphor, icon_name, icon_colour, 13.0);
    commands.entity(ic).insert(FocusPolicy::Pass);

    let label = commands
        .spawn((
            Text::new(tag.to_string()),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
            FocusPolicy::Pass,
            Node {
                flex_grow: 1.0,
                ..default()
            },
            bevy::text::TextLayout::no_wrap(),
        ))
        .id();

    // Every listed entry is installable now, so the only trailing note left is
    // "this is what you are running".
    let note = if is_current {
        Some((renzora::lang::t("update.row.current"), accent()))
    } else {
        None
    };
    let mut kids = vec![ic, label];
    if let Some((text, colour)) = note {
        let n = commands
            .spawn((
                Text::new(text),
                ui_font(&fonts.ui, 10.5),
                TextColor(rgb(colour)),
                FocusPolicy::Pass,
            ))
            .id();
        kids.push(n);
    }
    commands.entity(row).add_children(&kids);
    row
}

fn version_row_click(
    q: Query<(&Interaction, &VersionRow), Changed<Interaction>>,
    mut commands: Commands,
) {
    let Some(tag) = q
        .iter()
        .find(|(i, _)| **i == Interaction::Pressed)
        .map(|(_, r)| r.0.clone())
    else {
        return;
    };
    commands.queue(move |w: &mut World| {
        if let Some(mut s) = w.get_resource_mut::<UpdateState>() {
            s.select(&tag);
        }
    });
}

// ── Interaction ──────────────────────────────────────────────────────────────

/// Keep the action button's label in step with what it will do.
///
/// A separate system rather than a `bind_text` at spawn time because the label
/// lives on a child of the button, and the button is what carries the marker.
fn action_label_sync(
    q: Query<&Children, With<ActionBtn>>,
    mut texts: Query<&mut Text>,
    state: Res<UpdateState>,
) {
    let want = action_label(&state);
    for children in &q {
        for child in children.iter() {
            if let Ok(mut text) = texts.get_mut(child) {
                if text.0 != want {
                    text.0 = want.clone();
                }
            }
        }
    }
}

/// A line spelling out exactly what an armed overwrite is about to replace.
///
/// Shown only while armed, and it names the real path — "this will overwrite
/// your build output" is much easier to act on when you can see which directory.
fn overwrite_warning(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let warn = commands
        .spawn((
            Text::new(String::new()),
            ui_font(&fonts.ui, 11.5),
            TextColor(rgb(AMBER)),
        ))
        .id();
    bind_text(commands, warn, |w| {
        let Some(s) = w.get_resource::<UpdateState>() else {
            return String::new();
        };
        match s.layout.as_ref() {
            Some(l) => format!(
                "{} {}",
                renzora::lang::t("update.overwrite_warning"),
                l.target.display()
            ),
            None => String::new(),
        }
    });
    bind_display(commands, warn, |w| {
        w.get_resource::<UpdateState>()
            .is_some_and(|s| s.overwrite_armed && s.is_source_checkout())
    });
    warn
}

fn action_click(
    q: Query<&Interaction, (With<ActionBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    commands.queue(|w: &mut World| {
        let Some(mut state) = w.get_resource_mut::<UpdateState>() else {
            return;
        };
        match action_for(&state) {
            Action::Check => state.start_check(),
            Action::Download => crate::start_download(&mut state),
            // First click only arms it; the button then re-labels itself and the
            // warning line names the directory about to be replaced.
            Action::ConfirmOverwrite => state.overwrite_armed = true,
            // Does not return when it succeeds — the process exits so the
            // sidecar can replace the files it is running from.
            Action::Install => crate::install_and_restart(&mut state),
            Action::None => {}
        }
    });
}

fn channel_click(
    q: Query<(&Interaction, &ChannelBtn), Changed<Interaction>>,
    mut commands: Commands,
) {
    let Some(pref) = q
        .iter()
        .find(|(i, _)| **i == Interaction::Pressed)
        .map(|(_, c)| c.0)
    else {
        return;
    };
    commands.queue(move |w: &mut World| {
        if let Some(mut state) = w.get_resource_mut::<UpdateState>() {
            state.set_channel(pref);
        }
    });
}

fn notes_link_click(q: Query<&Interaction, (With<NotesLinkBtn>, Changed<Interaction>)>, state: Res<UpdateState>) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    if let Some(url) = state.result.as_ref().and_then(|r| r.release_url.clone()) {
        open_url(&url);
    }
}

fn close_click(q: Query<&Interaction, (With<CloseBtn>, Changed<Interaction>)>, mut state: ResMut<UpdateState>) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        state.visible = false;
    }
}
