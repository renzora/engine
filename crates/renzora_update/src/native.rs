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
use renzora_ember::reactive::tracked::{bind_bg, bind_display, bind_text};
use renzora_ember::theme::*;
use renzora_ember::widgets::{overlay_sized, scroll_area};

use crate::UpdateState;

const GREEN: (u8, u8, u8) = (89, 191, 115);
const RED: (u8, u8, u8) = (239, 68, 68);

pub(crate) fn register(app: &mut App) {
    app.add_systems(
        Update,
        (
            manage_modal,
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

/// What the action button does right now.
#[derive(PartialEq, Eq, Clone, Copy)]
enum Action {
    Check,
    Download,
    Install,
    /// Nothing to do — checking, downloading, up to date, or a source checkout.
    None,
}

fn action_for(s: &UpdateState) -> Action {
    if s.checking || s.downloading() {
        return Action::None;
    }
    if s.staged.is_some() {
        return if s.can_install() {
            Action::Install
        } else {
            Action::None
        };
    }
    match s.result.as_ref() {
        Some(r) if r.update_available => {
            if !s.can_install() {
                Action::None
            } else if r.download_url.is_some() {
                Action::Download
            } else {
                Action::None
            }
        }
        Some(_) => Action::Check,
        None => Action::Check,
    }
}

fn action_label(s: &UpdateState) -> String {
    match action_for(s) {
        Action::Check => renzora::lang::t("update.btn.check"),
        Action::Download => renzora::lang::t("update.btn.download"),
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
        520.0,
        460.0,
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

    // ── Headline: the answer, in one line ────────────────────────────────────
    let headline = commands
        .spawn((
            Text::new(String::new()),
            ui_font(&fonts.ui, 16.0),
            TextColor(rgb(text_primary())),
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
            Some(r) if r.update_available => format!(
                "{} {}",
                renzora::lang::t("update.available"),
                r.latest_version.clone().unwrap_or_default()
            ),
            Some(_) => renzora::lang::t("update.up_to_date"),
            None => renzora::lang::t("update.checking"),
        }
    });

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
            .and_then(|s| s.result.as_ref())
            .and_then(|r| r.release_notes.clone())
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
    let notes_scroll = scroll_area(commands, notes_col, 170.0);

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

    let notes_link = pill(commands, fonts, "arrow-square-out", &renzora::lang::t("update.btn.notes"));
    commands.entity(notes_link).insert(NotesLinkBtn);
    bind_display(commands, notes_link, |w| {
        w.get_resource::<UpdateState>()
            .and_then(|s| s.result.as_ref())
            .is_some_and(|r| r.release_url.is_some())
    });

    let close = pill(commands, fonts, "x", &renzora::lang::t("update.btn.later"));
    commands.entity(close).insert(CloseBtn);

    let action = pill(commands, fonts, "download-simple", "");
    commands.entity(action).insert(ActionBtn);
    bind_display(commands, action, |w| {
        w.get_resource::<UpdateState>()
            .is_some_and(|s| action_for(s) != Action::None)
    });
    bind_bg(commands, action, |w| {
        let hot = w
            .get_resource::<UpdateState>()
            .is_some_and(|s| matches!(action_for(s), Action::Install));
        if hot {
            rgb(GREEN)
        } else {
            rgb(accent())
        }
    });

    commands
        .entity(row)
        .add_children(&[notes_link, spacer, close, action]);

    commands.entity(body).add_children(&[
        headline,
        sub,
        channels,
        rule,
        notes_scroll,
        progress,
        error,
        row,
    ]);
    commands.entity(content).add_child(body);
}

/// A small pill button: icon + label.
fn pill(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str) -> Entity {
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
    btn
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
