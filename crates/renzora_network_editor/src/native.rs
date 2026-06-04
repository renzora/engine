//! Bevy-native (ember) ports of the three networking panels — faithful to the
//! egui `NetworkMonitorPanel` / `NetworkEntitiesPanel` / `NetworkSettingsPanel`.
//! Each is a one-shot build: state-dependent views are toggled with
//! `bind_display`, live values are `bind_text`, and the variable-length lists
//! (connected clients, networked entities) are `keyed_list`s.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora::core::CurrentProject;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_display, bind_text, keyed_list, KeyedSnapshot};
use renzora_ember::theme::*;
use renzora_network::status::ConnectionState;
use renzora_network::{NetworkId, NetworkOwner, NetworkStatus, Networked, OwnerKind};

const MUTED: (u8, u8, u8) = (148, 148, 160);
const GREEN: (u8, u8, u8) = (80, 200, 120);

pub struct NativeNetworkPanels;

impl Plugin for NativeNetworkPanels {
    fn build(&self, app: &mut App) {
        app.register_panel_content("network_monitor", true, build_monitor);
        app.register_panel_content("network_entities", true, build_entities);
        app.register_panel_content("network_settings", true, build_settings);
    }
}

// ── Shared little builders ───────────────────────────────────────────────────

fn nstat<R: Default>(w: &World, f: impl FnOnce(&NetworkStatus) -> R) -> R {
    w.get_resource::<NetworkStatus>().map(f).unwrap_or_default()
}

fn is_state(w: &World, want: ConnectionState) -> bool {
    w.get_resource::<NetworkStatus>().map(|s| s.state) == Some(want)
}

/// A `label …… value` row (label left, mono value right; the value is a binding).
fn stat_row<V>(commands: &mut Commands, fonts: &EmberFonts, label: &str, value: V) -> Entity
where
    V: Fn(&World) -> String + Send + Sync + 'static,
{
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            width: Val::Percent(100.0),
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let l = commands
        .spawn((Text::new(label), ui_font(&fonts.ui, 11.0), TextColor(rgb(MUTED))))
        .id();
    let gap = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let v = commands
        .spawn((Text::new(""), ui_font(&fonts.mono, 12.0), TextColor(rgb(text_primary()))))
        .id();
    bind_text(commands, v, value);
    commands.entity(row).add_children(&[l, gap, v]);
    row
}

fn text(commands: &mut Commands, fonts: &EmberFonts, s: &str, size: f32, color: (u8, u8, u8)) -> Entity {
    commands
        .spawn((Text::new(s.to_string()), ui_font(&fonts.ui, size), TextColor(rgb(color))))
        .id()
}

fn spacer(commands: &mut Commands, h: f32) -> Entity {
    commands.spawn(Node { height: Val::Px(h), ..default() }).id()
}

/// A centered empty-state column (big phosphor icon + title + optional subtitle).
fn centered(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    title: &str,
    subtitle: Option<&str>,
) -> Entity {
    let col = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::top(Val::Px(40.0)),
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, MUTED, 32.0);
    let t = text(commands, fonts, title, 13.0, MUTED);
    let mut kids = vec![ic, t];
    if let Some(sub) = subtitle {
        let s = commands
            .spawn((
                Text::new(sub.to_string()),
                ui_font(&fonts.ui, 11.0),
                TextColor(rgb(MUTED)),
                bevy::text::TextLayout::new_with_justify(bevy::text::Justify::Center),
            ))
            .id();
        kids.push(s);
    }
    commands.entity(col).add_children(&kids);
    col
}

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

fn divider(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(1.0), margin: UiRect::vertical(Val::Px(4.0)), ..default() },
            BackgroundColor(rgb(border())),
        ))
        .id()
}

// ── Network Monitor ──────────────────────────────────────────────────────────

fn build_monitor(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = column(commands, 4.0);
    commands.entity(root).insert(Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(4.0),
        padding: UiRect::all(Val::Px(8.0)),
        ..default()
    });

    // Plugin not loaded.
    let not_loaded = centered(commands, fonts, "wifi-slash", "Network plugin not loaded", None);
    bind_display(commands, not_loaded, |w| w.get_resource::<NetworkStatus>().is_none());

    // Disconnected.
    let disconnected = centered(
        commands,
        fonts,
        "wifi-slash",
        "Disconnected",
        Some("Configure networking in Network Settings to get started."),
    );
    bind_display(commands, disconnected, |w| is_state(w, ConnectionState::Disconnected));

    // Connecting.
    let connecting = centered(commands, fonts, "wifi-medium", "Connecting...", None);
    bind_display(commands, connecting, |w| is_state(w, ConnectionState::Connecting));

    // Connected.
    let connected = column(commands, 4.0);
    bind_display(commands, connected, |w| is_state(w, ConnectionState::Connected));
    {
        let header = commands
            .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() })
            .id();
        let wifi = icon_text(commands, &fonts.phosphor, "wifi-high", GREEN, 16.0);
        let lbl = text(commands, fonts, "Connected", 13.0, GREEN);
        let server = text(commands, fonts, "(Server)", 11.0, MUTED);
        bind_display(commands, server, |w| nstat(w, |s| s.is_server));
        commands.entity(header).add_children(&[wifi, lbl, server]);

        let div = divider(commands);

        // Client stats.
        let client = column(commands, 2.0);
        bind_display(commands, client, |w| !nstat(w, |s| s.is_server));
        let rtt = stat_row(commands, fonts, "RTT", |w| format!("{:.1} ms", nstat(w, |s| s.rtt_ms)));
        let jit = stat_row(commands, fonts, "Jitter", |w| format!("{:.1} ms", nstat(w, |s| s.jitter_ms)));
        let loss = stat_row(commands, fonts, "Packet Loss", |w| format!("{:.1}%", nstat(w, |s| s.packet_loss) * 100.0));
        let cid = stat_row(commands, fonts, "Client ID", |w| nstat(w, |s| s.client_id).map(|v| v.to_string()).unwrap_or_default());
        bind_display(commands, cid, |w| nstat(w, |s| s.client_id.is_some()));
        commands.entity(client).add_children(&[rtt, jit, loss, cid]);

        // Server view.
        let server_v = column(commands, 2.0);
        bind_display(commands, server_v, |w| nstat(w, |s| s.is_server));
        let count = commands
            .spawn((Text::new(""), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary()))))
            .id();
        bind_text(commands, count, |w| format!("Clients: {}", nstat(w, |s| s.connected_clients.len())));
        let list = column(commands, 1.0);
        keyed_list(commands, list, clients_snapshot);
        let sp = spacer(commands, 2.0);
        commands.entity(server_v).add_children(&[count, sp, list]);

        commands.entity(connected).add_children(&[header, div, client, server_v]);
    }

    commands.entity(root).add_children(&[not_loaded, disconnected, connecting, connected]);
    root
}

fn clients_snapshot(world: &World) -> KeyedSnapshot {
    let clients: Vec<(u64, f32)> = nstat(world, |s| {
        s.connected_clients.iter().map(|c| (c.client_id, c.rtt_ms)).collect()
    });
    if clients.is_empty() {
        return KeyedSnapshot {
            items: vec![(u64::MAX, 0)],
            build: Box::new(|c, f, _| text(c, f, "Waiting for clients...", 11.0, MUTED)),
        };
    }
    let items: Vec<(u64, u64)> = clients
        .iter()
        .map(|(id, rtt)| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            rtt.to_bits().hash(&mut h);
            (*id, h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (id, rtt) = clients[i];
            let row = c
                .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), ..default() })
                .id();
            let ic = icon_text(c, &f.phosphor, "user", MUTED, 12.0);
            let lbl = text(c, f, &format!("ID: {id}"), 12.0, text_primary());
            let r = text(c, f, &format!("{rtt:.0}ms"), 11.0, MUTED);
            c.entity(row).add_children(&[ic, lbl, r]);
            row
        }),
    }
}

// ── Network Entities ─────────────────────────────────────────────────────────

fn build_entities(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        })
        .id();
    let count = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary()))))
        .id();
    bind_text(commands, count, |w| format!("{} networked entities", networked(w).len()));
    bind_display(commands, count, |w| !networked(w).is_empty());
    let list = column(commands, 1.0);
    keyed_list(commands, list, entities_snapshot);
    commands.entity(root).add_children(&[count, list]);
    root
}

/// (entity, name, network id, owner) for every entity with `Networked`.
fn networked(world: &World) -> Vec<(Entity, Option<String>, Option<u64>, Option<OwnerKind>)> {
    let mut out = Vec::new();
    for archetype in world.archetypes().iter() {
        for arch_entity in archetype.entities() {
            let e = arch_entity.id();
            if world.get::<Networked>(e).is_some() {
                let name = world.get::<Name>(e).map(|n| n.as_str().to_string());
                let nid = world.get::<NetworkId>(e).map(|n| n.0);
                let owner = world.get::<NetworkOwner>(e).map(|o| o.0);
                out.push((e, name, nid, owner));
            }
        }
    }
    out
}

fn entities_snapshot(world: &World) -> KeyedSnapshot {
    let ents = networked(world);
    if ents.is_empty() {
        return KeyedSnapshot {
            items: vec![(u64::MAX, 0)],
            build: Box::new(|c, f, _| {
                centered(
                    c,
                    f,
                    "share-network",
                    "No networked entities",
                    Some("Add a Networked component to entities to see them listed here."),
                )
            }),
        };
    }
    let items: Vec<(u64, u64)> = ents
        .iter()
        .map(|(e, name, nid, owner)| {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (name, nid, owner.map(|o| matches!(o, OwnerKind::Server))).hash(&mut h);
            (e.to_bits(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (e, name, nid, owner) = &ents[i];
            let row = c
                .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(4.0), ..default() })
                .id();
            let ic = icon_text(c, &f.phosphor, "cube", MUTED, 12.0);
            let fallback = format!("{e:?}");
            let name_e = text(c, f, name.as_deref().unwrap_or(&fallback), 12.0, text_primary());
            let mut kids = vec![ic, name_e];
            if let Some(n) = nid {
                kids.push(text(c, f, &format!("#{n}"), 10.0, MUTED));
            }
            if let Some(own) = owner {
                let s = match own {
                    OwnerKind::Server => "Server".to_string(),
                    OwnerKind::Client(id) => format!("Client {id}"),
                };
                kids.push(text(c, f, &s, 10.0, MUTED));
            }
            c.entity(row).add_children(&kids);
            row
        }),
    }
}

// ── Network Settings ─────────────────────────────────────────────────────────

fn net_cfg<R: Default>(
    w: &World,
    f: impl FnOnce(&renzora::core::NetworkProjectConfig) -> R,
) -> R {
    w.get_resource::<CurrentProject>()
        .and_then(|p| p.config.network.as_ref())
        .map(f)
        .unwrap_or_default()
}

fn has_cfg(w: &World) -> bool {
    w.get_resource::<CurrentProject>()
        .map(|p| p.config.network.is_some())
        .unwrap_or(false)
}

fn build_settings(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            padding: UiRect::all(Val::Px(8.0)),
            ..default()
        })
        .id();

    // Configured view.
    let cfg = column(commands, 4.0);
    bind_display(commands, cfg, has_cfg);
    let title = text(commands, fonts, "Network Configuration", 13.0, text_primary());
    let div = divider(commands);
    let grid = column(commands, 6.0);
    let rows = [
        stat_row(commands, fonts, "Server Address", |w| net_cfg(w, |c| c.server_addr.clone())),
        stat_row(commands, fonts, "Port", |w| net_cfg(w, |c| c.port.to_string())),
        stat_row(commands, fonts, "Transport", |w| net_cfg(w, |c| c.transport.clone())),
        stat_row(commands, fonts, "Tick Rate", |w| format!("{} Hz", net_cfg(w, |c| c.tick_rate))),
        stat_row(commands, fonts, "Max Clients", |w| net_cfg(w, |c| c.max_clients.to_string())),
    ];
    commands.entity(grid).add_children(&rows);
    let hint = text(commands, fonts, "Edit [network] in project.toml to change settings.", 10.0, MUTED);
    let sp = spacer(commands, 8.0);
    commands.entity(cfg).add_children(&[title, div, grid, sp, hint]);

    // Not-configured view.
    let none = centered(
        commands,
        fonts,
        "cloud-slash",
        "Networking not configured",
        Some("Add [network] to project.toml to configure server mode, transport, and connection settings."),
    );
    bind_display(commands, none, |w| !has_cfg(w));

    commands.entity(root).add_children(&[cfg, none]);
    root
}
