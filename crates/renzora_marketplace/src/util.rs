//! Shared helpers for the marketplace panels.
//!
//! This file used to serve the Community panels and was mostly about them: a
//! per-area accent hue for each (friends green, chat blue, forum violet…), a
//! neutral `action_chip` for post likes and reactions, relative timestamps for
//! feed entries, local moderation notes kept in `~/.renzora/profile_notes.json`,
//! and role icons for forum posts. All of it went when the panels did. What is
//! left is the handful of things the store, library, uploader and wallet use.

use crate::auth::AuthSession;
use bevy::prelude::*;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::rgb;

/// True when a user is signed in.
pub(crate) fn signed_in(w: &Rx) -> bool {
    w.get_resource::<AuthSession>()
        .map(|s| s.is_signed_in())
        .unwrap_or(false)
}

/// A small rounded text button; insert your own marker component on the result.
pub(crate) fn pill_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    bg: (u8, u8, u8),
    fg: (u8, u8, u8),
) -> Entity {
    let lighten = |(r, g, b): (u8, u8, u8), n: u8| {
        (r.saturating_add(n), g.saturating_add(n), b.saturating_add(n))
    };
    let btn = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(5.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(bg)),
            Interaction::default(),
            renzora_ember::widgets::HoverTint::solid(
                rgb(bg),
                rgb(lighten(bg, 22)),
                rgb(lighten(bg, 40)),
            ),
        ))
        .id();
    let t = commands
        .spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 10.5), TextColor(rgb(fg))))
        .id();
    commands.entity(btn).add_child(t);
    btn
}

/// Clone just the tokens of a session for use on a worker thread.
pub(crate) fn session_clone(session: &AuthSession) -> AuthSession {
    AuthSession {
        user: session.user.clone(),
        access_token: session.access_token.clone(),
        refresh_token: None,
    }
}

/// Stable 64-bit hash for keyed-list keys / content hashes.
pub(crate) fn hash64<T: std::hash::Hash + ?Sized>(t: &T) -> u64 {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    t.hash(&mut h);
    h.finish()
}

/// A keyed snapshot with no rows.
pub(crate) fn empty_snapshot() -> renzora_ember::reactive::KeyedSnapshot {
    renzora_ember::reactive::KeyedSnapshot {
        items: Vec::new(),
        build: Box::new(|commands, _fonts, _i| commands.spawn(Node::default()).id()),
    }
}
