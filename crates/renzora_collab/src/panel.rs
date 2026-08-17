//! The Collaborate panel — start a session, join one, and see who is in it.

use bevy::prelude::*;

use renzora::RenzoraShellExt;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::tracked::{bind_display, bind_text, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::{bind_text_input, button, text_input};

use crate::files::{human_bytes, FileSync, SyncPhase};
use crate::session::{CollabRole, CollabSession};
use crate::sync::SyncTracker;

pub const PANEL_ID: &str = "collaborate";

const MUTED: (u8, u8, u8) = (148, 148, 160);

pub struct CollabPanel;

impl Plugin for CollabPanel {
    fn build(&self, app: &mut App) {
        app.register_shell_panel(PANEL_ID, "Collaborate", "users-three", "Session");
        app.register_panel_content(PANEL_ID, true, build)
            .systems(Update, (button_clicks, toggle_clicks));
    }
}

// ── Markers ─────────────────────────────────────────────────────────────────

#[derive(Component)]
struct HostBtn;
#[derive(Component)]
struct HostOnlineBtn;
#[derive(Component)]
struct JoinCodeBtn;
#[derive(Component)]
struct JoinBtn;
#[derive(Component)]
struct LeaveBtn;
#[derive(Component)]
struct SyncBtn;
#[derive(Component)]
struct AllowControlBtn;

// ── Build ───────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(10.0),
            padding: UiRect::all(Val::Px(10.0)),
            ..default()
        })
        .id();

    let mut kids = vec![status_block(commands, fonts)];
    kids.push(offline_block(commands, fonts));
    kids.push(active_block(commands, fonts));
    kids.push(log_block(commands, fonts));
    commands.entity(root).add_children(&kids);
    root
}

/// The one-line "what is happening" header, always visible.
fn status_block(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let icon = icon_text(commands, &fonts.phosphor, "users-three", accent(), 18.0);
    let label = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary()))))
        .id();
    bind_text(commands, label, |w: &Rx| {
        let Some(session) = w.get_resource::<CollabSession>() else {
            return String::new();
        };
        if session.status.is_empty() {
            match session.role {
                CollabRole::Offline => "Not in a session".to_string(),
                CollabRole::Hosting => "Hosting".to_string(),
                CollabRole::Guest => "In a session".to_string(),
            }
        } else {
            session.status.clone()
        }
    });
    commands.entity(row).add_children(&[icon, label]);
    row
}

/// Host / join controls — shown only while offline.
fn offline_block(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = column(commands, 10.0);
    bind_display(commands, col, |w: &Rx| {
        w.get_resource::<CollabSession>().map(|s| !s.is_active()).unwrap_or(false)
    });

    let name_label = label(commands, fonts, "Your name");
    let name = text_input(commands, &fonts.ui, "name", "");
    bind_text_input(
        commands,
        name,
        |w: &Rx| w.get_resource::<CollabSession>().map(|s| s.display_name.clone()).unwrap_or_default(),
        |world, value| {
            if let Some(mut s) = world.get_resource_mut::<CollabSession>() {
                s.display_name = value;
            }
        },
    );

    let online_title = heading(commands, fonts, "Invite a friend");
    let online_hint = hint(
        commands,
        fonts,
        "Goes through renzora.com, so it works between any two people. You get a code to send them. Requires being signed in.",
    );
    let online_btn = button(commands, &fonts.ui, "Start an online session");
    commands.entity(online_btn).insert(HostOnlineBtn);

    let code_label = label(commands, fonts, "Or join with a code");
    let code_input = text_input(commands, &fonts.ui, "ABCD2345", "");
    bind_text_input(
        commands,
        code_input,
        |w: &Rx| w.get_resource::<CollabSession>().map(|s| s.code_text.clone()).unwrap_or_default(),
        |world, value| {
            if let Some(mut s) = world.get_resource_mut::<CollabSession>() {
                s.code_text = value;
            }
        },
    );
    let code_btn = button(commands, &fonts.ui, "Join session");
    commands.entity(code_btn).insert(JoinCodeBtn);

    let host_title = heading(commands, fonts, "Or connect directly");
    let host_hint = hint(
        commands,
        fonts,
        "Opens a port on this machine. No account needed, but only reachable on your own network unless you forward the port.",
    );
    let port_label = label(commands, fonts, "Port");
    let port = text_input(commands, &fonts.ui, "7700", "");
    bind_text_input(
        commands,
        port,
        |w: &Rx| w.get_resource::<CollabSession>().map(|s| s.port_text.clone()).unwrap_or_default(),
        |world, value| {
            if let Some(mut s) = world.get_resource_mut::<CollabSession>() {
                s.port_text = value;
            }
        },
    );
    let host_btn = button(commands, &fonts.ui, "Start hosting");
    commands.entity(host_btn).insert(HostBtn);

    let join_title = heading(commands, fonts, "Join someone's session");
    let join_label = label(commands, fonts, "Host address");
    let join = text_input(commands, &fonts.ui, "192.168.1.20:7700", "");
    bind_text_input(
        commands,
        join,
        |w: &Rx| w.get_resource::<CollabSession>().map(|s| s.join_text.clone()).unwrap_or_default(),
        |world, value| {
            if let Some(mut s) = world.get_resource_mut::<CollabSession>() {
                s.join_text = value;
            }
        },
    );
    let join_btn = button(commands, &fonts.ui, "Join");
    commands.entity(join_btn).insert(JoinBtn);

    let rule_a = divider(commands);
    let rule_b = divider(commands);
    let rule_c = divider(commands);
    commands.entity(col).add_children(&[
        name_label,
        name,
        rule_a,
        online_title,
        online_hint,
        online_btn,
        code_label,
        code_input,
        code_btn,
        rule_b,
        host_title,
        host_hint,
        port_label,
        port,
        host_btn,
        rule_c,
        join_title,
        join_label,
        join,
        join_btn,
    ]);
    col
}

/// Peer list, controls and file sync — shown only while in a session.
fn active_block(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = column(commands, 8.0);
    bind_display(commands, col, |w: &Rx| {
        w.get_resource::<CollabSession>().map(|s| s.is_active()).unwrap_or(false)
    });

    // The address to read out to a guest.
    let address = commands
        .spawn((Text::new(""), ui_font(&fonts.mono, 12.0), TextColor(rgb(accent()))))
        .id();
    bind_text(commands, address, |w: &Rx| {
        let Some(session) = w.get_resource::<CollabSession>() else {
            return String::new();
        };
        // The code is what a guest actually needs, so it wins over the address
        // whenever there is one.
        if let Some(code) = &session.room_code {
            return format!("Code: {code}");
        }
        match (&session.address_hint, session.is_host()) {
            (Some(ip), true) => format!("{ip}:{}", session.port_text),
            _ => String::new(),
        }
    });

    let control = button(commands, &fonts.ui, "");
    commands.entity(control).insert(AllowControlBtn);
    bind_display(commands, control, |w: &Rx| {
        w.get_resource::<CollabSession>().map(|s| s.is_host()).unwrap_or(false)
    });
    // The button's own label is the state, so there is one thing to read and one
    // thing to click rather than a checkbox and a caption that can disagree.
    bind_button_label(commands, control, |w: &Rx| {
        let allowed =
            w.get_resource::<CollabSession>().map(|s| s.allow_control).unwrap_or(false);
        if allowed {
            "Guests can edit — click to make read-only".to_string()
        } else {
            "Guests are watching — click to let them edit".to_string()
        }
    });

    let peers_title = heading(commands, fonts, "In this session");
    let peer_list = column(commands, 4.0);
    keyed_list(commands, peer_list, |w: &Rx| {
        let session = w.get_resource::<CollabSession>();
        let rows: Vec<(String, [u8; 3], bool, usize)> = session
            .map(|s| {
                s.peers
                    .values()
                    .map(|p| (p.name.clone(), p.color, p.ready, p.leases.len()))
                    .collect()
            })
            .unwrap_or_default();
        let items = rows
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                std::hash::Hash::hash(&(&row.0, row.1, row.2, row.3), &mut hasher);
                (i as u64, std::hash::Hasher::finish(&hasher))
            })
            .collect();
        KeyedSnapshot {
            items,
            build: Box::new(move |c, fonts, i| {
                let (name, color, ready, leases) = rows[i].clone();
                let row = c
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.0),
                        ..default()
                    })
                    .id();
                let dot = c
                    .spawn((
                        Node {
                            width: Val::Px(8.0),
                            height: Val::Px(8.0),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb_u8(color[0], color[1], color[2])),
                    ))
                    .id();
                let text = c
                    .spawn((
                        Text::new(if ready {
                            if leases > 0 {
                                format!("{name} — editing {leases}")
                            } else {
                                name
                            }
                        } else {
                            format!("{name} (connecting…)")
                        }),
                        ui_font(&fonts.ui, 12.0),
                        TextColor(rgb(text_primary())),
                    ))
                    .id();
                c.entity(row).add_children(&[dot, text]);
                row
            }),
        }
    });

    // File sync.
    let files_title = heading(commands, fonts, "Project files");
    let files_status = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(MUTED))))
        .id();
    bind_text(commands, files_status, |w: &Rx| {
        let Some(sync) = w.get_resource::<FileSync>() else {
            return String::new();
        };
        if sync.message.is_empty() {
            "Waiting for the host's file list…".to_string()
        } else if sync.phase == SyncPhase::Transferring {
            format!("{} — {}", sync.message, human_bytes(sync.received_bytes))
        } else {
            sync.message.clone()
        }
    });
    let sync_btn = button(commands, &fonts.ui, "Download missing files");
    commands.entity(sync_btn).insert(SyncBtn);
    bind_display(commands, sync_btn, |w: &Rx| {
        w.get_resource::<FileSync>().map(|s| s.has_offer()).unwrap_or(false)
    });

    let stats = commands
        .spawn((Text::new(""), ui_font(&fonts.mono, 11.0), TextColor(rgb(MUTED))))
        .id();
    bind_text(commands, stats, |w: &Rx| {
        let Some(tracker) = w.get_resource::<SyncTracker>() else {
            return String::new();
        };
        format!("{} sent · {} received", tracker.sent, tracker.received)
    });

    let leave = button(commands, &fonts.ui, "Leave session");
    commands.entity(leave).insert(LeaveBtn);

    let rule_a = divider(commands);
    let rule_b = divider(commands);
    let rule_c = divider(commands);
    commands.entity(col).add_children(&[
        address,
        control,
        rule_a,
        peers_title,
        peer_list,
        rule_b,
        files_title,
        files_status,
        sync_btn,
        rule_c,
        stats,
        leave,
    ]);
    col
}

fn log_block(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = column(commands, 2.0);
    bind_display(commands, col, |w: &Rx| {
        w.get_resource::<CollabSession>().map(|s| !s.log.is_empty()).unwrap_or(false)
    });
    let title = heading(commands, fonts, "Activity");
    let list = column(commands, 2.0);
    keyed_list(commands, list, |w: &Rx| {
        // Newest first, and only the last handful: this is a glance-at-it trail,
        // not a log viewer, and the console already keeps the full history.
        let lines: Vec<String> = w
            .get_resource::<CollabSession>()
            .map(|s| s.log.iter().rev().take(8).cloned().collect())
            .unwrap_or_default();
        let items = lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                std::hash::Hash::hash(line, &mut hasher);
                (i as u64, std::hash::Hasher::finish(&hasher))
            })
            .collect();
        KeyedSnapshot {
            items,
            build: Box::new(move |c, fonts, i| {
                c.spawn((
                    Text::new(lines[i].clone()),
                    ui_font(&fonts.ui, 11.0),
                    TextColor(rgb(MUTED)),
                ))
                .id()
            }),
        }
    });
    commands.entity(col).add_children(&[title, list]);
    col
}

// ── Interaction ─────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn button_clicks(
    mut session: ResMut<CollabSession>,
    mut sync: ResMut<FileSync>,
    mut ids: ResMut<crate::identity::CollabIds>,
    mut tracker: ResMut<SyncTracker>,
    mut online: ResMut<crate::online::OnlineRequests>,
    auth: Option<Res<renzora_auth::AuthSession>>,
    project: Option<Res<renzora::core::CurrentProject>>,
    host: Query<&Interaction, (With<HostBtn>, Changed<Interaction>)>,
    host_online: Query<&Interaction, (With<HostOnlineBtn>, Changed<Interaction>)>,
    join_code: Query<&Interaction, (With<JoinCodeBtn>, Changed<Interaction>)>,
    join: Query<&Interaction, (With<JoinBtn>, Changed<Interaction>)>,
    leave: Query<&Interaction, (With<LeaveBtn>, Changed<Interaction>)>,
    sync_btn: Query<&Interaction, (With<SyncBtn>, Changed<Interaction>)>,
) {
    let pressed = |i: &Interaction| *i == Interaction::Pressed;
    let project_name = || {
        project
            .as_ref()
            .and_then(|p| p.path.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "this project".into())
    };
    // Both online paths need a signed-in account, so the same refusal serves
    // both — and says what to do about it rather than just failing.
    let token = || {
        auth.as_ref().and_then(|a| crate::online::access_token(a))
    };

    if host.iter().any(pressed) {
        // Claim the host's slice of the id space before anything can be minted
        // into it. A guest does the same when its `Welcome` tells it which slice
        // is its own.
        ids.begin(crate::session::HOST_SLOT);
        tracker.reset();
        session.start_hosting(&project_name());
    }
    if host_online.iter().any(pressed) {
        match token() {
            Some(token) => {
                ids.begin(crate::session::HOST_SLOT);
                tracker.reset();
                session.status = "Creating a session…".into();
                crate::online::request_host(&mut online, token, project_name());
            }
            None => session.status = "Sign in to renzora.com to host an online session".into(),
        }
    }
    if join_code.iter().any(pressed) {
        let code = session.code_text.trim().to_string();
        if code.is_empty() {
            session.status = "Enter the code your host gave you".into();
        } else {
            match token() {
                Some(token) => {
                    tracker.reset();
                    session.status = format!("Looking up {}…", code.to_ascii_uppercase());
                    crate::online::request_join(&mut online, token, code);
                }
                None => session.status = "Sign in to renzora.com to join a session".into(),
            }
        }
    }
    if join.iter().any(pressed) {
        tracker.reset();
        session.join();
    }
    if leave.iter().any(pressed) {
        session.leave();
        sync.reset();
        ids.clear();
        tracker.reset();
    }
    if sync_btn.iter().any(pressed) {
        crate::files::accept_offer(&mut sync, &session);
    }
}

fn toggle_clicks(
    mut session: ResMut<CollabSession>,
    control: Query<&Interaction, (With<AllowControlBtn>, Changed<Interaction>)>,
) {
    if control.iter().any(|i| *i == Interaction::Pressed) {
        session.allow_control = !session.allow_control;
        let now = session.allow_control;
        // Told, not inferred: a guest has no way to observe that its edits are
        // being dropped, so the switch is only real once it has been sent.
        session.broadcast(crate::protocol::CollabMsg::Control { allowed: now });
        session.note(if now {
            "guests may now edit the scene"
        } else {
            "guests are now read-only"
        });
    }
}

// ── Small builders ──────────────────────────────────────────────────────────

fn column(commands: &mut Commands, gap: f32) -> Entity {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(gap),
            ..default()
        })
        .id()
}

fn heading(commands: &mut Commands, fonts: &EmberFonts, s: &str) -> Entity {
    commands
        .spawn((
            Text::new(s.to_string()),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id()
}

fn label(commands: &mut Commands, fonts: &EmberFonts, s: &str) -> Entity {
    commands
        .spawn((Text::new(s.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(MUTED))))
        .id()
}

fn hint(commands: &mut Commands, fonts: &EmberFonts, s: &str) -> Entity {
    commands
        .spawn((
            Text::new(s.to_string()),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(MUTED)),
        ))
        .id()
}

fn divider(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(1.0),
                margin: UiRect::vertical(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(border())),
        ))
        .id()
}

/// Bind the text of a button's single text child.
///
/// A button owns its label rather than exposing it, so this walks to the child
/// once at build time — the alternative is every caller having to build the
/// button by parts just to make its caption reactive.
fn bind_button_label<F>(commands: &mut Commands, button: Entity, value: F)
where
    F: for<'w> Fn(&Rx<'w>) -> String + Send + Sync + 'static,
{
    renzora_ember::reactive::react_anchored(commands, button, move |world: &mut World| {
        if world.get_entity(button).is_err() {
            return false;
        }
        let Some(child) = world.get::<Children>(button).and_then(|c| c.iter().next()) else {
            return true;
        };
        let text = value(&Rx::new(&*world));
        if let Some(mut t) = world.get_mut::<Text>(child) {
            // Compared before writing: `Text` is change-detected, and a blind
            // assignment every frame would re-lay-out the button forever.
            if t.0 != text {
                t.0 = text;
            }
        }
        true
    });
}
