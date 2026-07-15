//! The bell dropdown — a quick glance at recent notifications without leaving
//! what you're doing. Toggled by the shell's bell button (which passes its
//! screen position through [`SocialBridge::notify_dropdown_request`]).

use bevy::prelude::*;
use renzora::core::SocialBridge;
use renzora_auth::AuthSession;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::theme::{card_bg, placeholder, popup_bg, rgb, rgba, row_even, row_odd, text_muted, text_primary};
use renzora_ember::widgets::{accent_ghost, icon_badge, tint, HoverTint};

use crate::panels::notifications::{self, NotificationsPanel};
use crate::util::{self, HUE_NOTIFY};

const WIDTH: f32 = 360.0;
const MAX_ROWS: usize = 8;

#[derive(Resource, Default)]
pub(crate) struct NotifyDropdownUi {
    root: Option<Entity>,
}

#[derive(Component)]
pub(crate) struct NddBackdrop;
#[derive(Component)]
pub(crate) struct NddRow(renzora_auth::social::NotificationRow);
#[derive(Component)]
pub(crate) struct NddMarkAllBtn;

/// Toggle the dropdown when the bell asks for it.
pub(crate) fn toggle(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    mut ui: ResMut<NotifyDropdownUi>,
    mut bridge: ResMut<SocialBridge>,
    mut panel: ResMut<NotificationsPanel>,
    session: Res<AuthSession>,
    nodes: Query<Entity, With<Node>>,
) {
    // The x is ignored — the dropdown centers itself horizontally (see below);
    // y anchors it just under the top bar.
    let Some((_x, y)) = bridge.notify_dropdown_request.take() else {
        return;
    };
    // Already open → close.
    if let Some(root) = ui.root.take() {
        if nodes.get(root).is_ok() {
            commands.entity(root).try_despawn();
            return;
        }
    }
    let Some(fonts) = fonts else { return };
    if !session.is_signed_in() {
        return;
    }
    // Make sure we have data (one-shot; the WS keeps it live afterwards).
    if !panel.loaded_once {
        notifications::refresh(&mut panel, &session);
    }

    // Full-screen backdrop that dismisses on click. A column that centers its
    // child horizontally, so the dropdown is centered regardless of window width.
    let backdrop = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            renzora_ember::cursor_icon::NoAutoCursor,
            NddBackdrop,
            GlobalZIndex(940),
            Name::new("notify_dropdown"),
        ))
        .id();

    // Centered by the backdrop's `align_items`; `margin.top` drops it under the
    // bar. A soft accent glow (double border via inset) keeps it from reading grey.
    let dropdown = commands
        .spawn((
            Node {
                width: Val::Px(WIDTH),
                margin: UiRect::top(Val::Px(y)),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(8.0)),
                row_gap: Val::Px(5.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(tint(HUE_NOTIFY, 70)),
            bevy::ui::FocusPolicy::Block,
        ))
        .id();

    // Header.
    let header = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            padding: UiRect::all(Val::Px(4.0)),
            ..default()
        })
        .id();
    let badge = icon_badge(&mut commands, &fonts, HUE_NOTIFY, "bell", 20.0);
    let title = commands
        .spawn((Text::new("Notifications"), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())), Node { flex_grow: 1.0, ..default() }))
        .id();
    // "Mark all read" lives in the header now (the old panel-opening footer is
    // gone — there's no panel). It's a no-op when everything's already read.
    let mark_all = accent_ghost(&mut commands, &fonts, HUE_NOTIFY, "Mark all read");
    commands.entity(mark_all).insert(NddMarkAllBtn);
    commands.entity(header).add_children(&[badge, title, mark_all]);
    commands.entity(dropdown).add_child(header);

    // Rows (newest first, odd/even striped).
    if panel.items.is_empty() {
        let none = commands
            .spawn((
                Text::new(if panel.loading { "Loading..." } else { "You're all caught up" }),
                ui_font(&fonts.ui, 10.5),
                TextColor(rgb(placeholder())),
                Node { margin: UiRect::all(Val::Px(8.0)), ..default() },
            ))
            .id();
        commands.entity(dropdown).add_child(none);
    }
    for (i, n) in panel.items.iter().take(MAX_ROWS).enumerate() {
        let base = rgb(if i % 2 == 0 { row_even() } else { row_odd() });
        let row = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(7.0),
                    padding: UiRect::all(Val::Px(6.0)),
                    border_radius: BorderRadius::all(Val::Px(5.0)),
                    ..default()
                },
                BackgroundColor(base),
                Interaction::default(),
                HoverTint::solid(base, rgb(card_bg()), tint(HUE_NOTIFY, 40)),
                NddRow(n.clone()),
            ))
            .id();
        // Actor avatar when known, else the type badge.
        let lead = if n.actor_avatar_url.is_some() {
            crate::avatars::avatar_image(&mut commands, &fonts, n.actor_avatar_url.as_deref(), 22.0)
        } else {
            icon_badge(&mut commands, &fonts, HUE_NOTIFY, "bell", 22.0)
        };
        let col = commands
            .spawn(Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, overflow: Overflow::clip(), ..default() })
            .id();
        let t = commands
            .spawn((
                Text::new(n.title.clone()),
                ui_font(&fonts.ui, 10.5),
                TextColor(rgb(if n.read { text_muted() } else { text_primary() })),
                bevy::text::TextLayout::no_wrap(),
            ))
            .id();
        let when = commands
            .spawn((Text::new(util::relative_time(&n.created_at)), ui_font(&fonts.ui, 8.5), TextColor(rgb(placeholder()))))
            .id();
        commands.entity(col).add_children(&[t, when]);
        let mut kids = vec![lead, col];
        if !n.read {
            kids.push(
                commands
                    .spawn((
                        Node { width: Val::Px(7.0), height: Val::Px(7.0), border_radius: BorderRadius::all(Val::Px(4.0)), flex_shrink: 0.0, ..default() },
                        BackgroundColor(tint(HUE_NOTIFY, 255)),
                    ))
                    .id(),
            );
        }
        commands.entity(row).add_children(&kids);
        commands.entity(dropdown).add_child(row);
    }

    commands.entity(backdrop).add_child(dropdown);
    ui.root = Some(backdrop);
    let _ = rgba([0, 0, 0, 0]);
}

/// Clicks inside the dropdown: row → mark read + deep-link; footer → panel;
/// backdrop → dismiss.
#[allow(clippy::too_many_arguments)]
pub(crate) fn clicks(
    mut commands: Commands,
    mut ui: ResMut<NotifyDropdownUi>,
    mut bridge: ResMut<SocialBridge>,
    mut panel: ResMut<NotificationsPanel>,
    session: Res<AuthSession>,
    rows: Query<(&Interaction, &NddRow), Changed<Interaction>>,
    mark_alls: Query<&Interaction, (With<NddMarkAllBtn>, Changed<Interaction>)>,
    backdrops: Query<&Interaction, (With<NddBackdrop>, Changed<Interaction>)>,
) {
    let mut close = false;
    // The dropdown is built imperatively (not reactive), so it can't clear its
    // unread dots in place — mark all read and close; the bell badge updates.
    for i in &mark_alls {
        if *i == Interaction::Pressed {
            notifications::mark_all_optimistic(&mut panel, &mut bridge, &session);
            close = true;
        }
    }
    for (i, row) in &rows {
        if *i != Interaction::Pressed {
            continue;
        }
        let n = &row.0;
        if !n.read {
            notifications::mark_read_optimistic(&mut panel, &mut bridge, &session, &n.id);
        }
        bridge.open_panel_request = Some(crate::routing::route_notification(n));
        close = true;
    }
    for i in &backdrops {
        if *i == Interaction::Pressed {
            close = true;
        }
    }
    if close {
        // `try_despawn`: the backdrop may have been torn down externally
        // (workspace/layout rebuild) while this resource still held its id.
        if let Some(root) = ui.root.take() {
            commands.entity(root).try_despawn();
        }
    }
}
