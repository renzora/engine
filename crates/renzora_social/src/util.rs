//! Shared helpers for the social panels.

use bevy::prelude::*;
use renzora_auth::AuthSession;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::theme::{accent, rgb, rgba, text_muted, text_primary};

// ── Area identity hues ───────────────────────────────────────────────────────
// Each Community area carries one hue, used only as tints/icons/accents over
// the theme (see `renzora_ember::widgets::accent`).

pub(crate) const HUE_FRIENDS: (u8, u8, u8) = (82, 196, 120); // green — presence, alive
pub(crate) const HUE_CHAT: (u8, u8, u8) = (91, 156, 245); // blue — conversation
pub(crate) const HUE_NOTIFY: (u8, u8, u8) = (235, 180, 80); // amber — attention
pub(crate) const HUE_FEED: (u8, u8, u8) = (240, 120, 100); // coral — showing off
pub(crate) const HUE_FORUM: (u8, u8, u8) = (167, 130, 245); // violet — discussion
pub(crate) const HUE_TEAMS: (u8, u8, u8) = (70, 190, 190); // teal — working together
pub(crate) const HUE_LEARN: (u8, u8, u8) = (86, 182, 222); // cyan — learning

/// Icon + hue for a site role, or `None` for regular users. Roles render as
/// icons, not text ("admin" → crown).
pub(crate) fn role_icon(role: &str) -> Option<(&'static str, (u8, u8, u8))> {
    match role {
        "admin" => Some(("crown-simple", (235, 180, 80))),
        "moderator" | "mod" => Some(("shield-check", (91, 156, 245))),
        _ => None,
    }
}

/// True when a user is signed in.
pub(crate) fn signed_in(w: &World) -> bool {
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

/// One neutral action chip — the shared visual for every post action (like,
/// comment, reaction, add-reaction, delete). Neutral translucent white at
/// rest; the theme accent when `active` (liked / reacted / expanded). One
/// look everywhere is what keeps the panels from turning into a hue salad.
/// Caller inserts its own marker component on the returned entity.
pub(crate) fn action_chip(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: Option<&str>,
    active: bool,
    tooltip: Option<String>,
) -> Entity {
    let (base, hover, press) = if active {
        let a = rgb(accent());
        (a.with_alpha(0.30), a.with_alpha(0.42), a.with_alpha(0.55))
    } else {
        (rgba([255, 255, 255, 10]), rgba([255, 255, 255, 22]), rgba([255, 255, 255, 34]))
    };
    let chip = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(9.0), Val::Px(4.5)),
                border_radius: BorderRadius::all(Val::Px(13.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(base),
            Interaction::default(),
            renzora_ember::widgets::HoverTint::solid(base, hover, press),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
        ))
        .id();
    if let Some(tip) = tooltip {
        commands.entity(chip).insert(renzora_ember::widgets::HoverTooltip::new(tip));
    }
    // Full text brightness even at rest — the icon IS the chip's meaning, and
    // muted-on-translucent was too faint to read (the count label stays muted).
    let ic = icon_text(
        commands,
        &fonts.phosphor,
        icon,
        if active { accent() } else { text_primary() },
        14.0,
    );
    commands.entity(chip).add_child(ic);
    if let Some(label) = label {
        let t = commands
            .spawn((
                Text::new(label.to_string()),
                ui_font(&fonts.ui, 11.0),
                TextColor(rgb(if active { text_primary() } else { text_muted() })),
            ))
            .id();
        commands.entity(chip).add_child(t);
    }
    chip
}

// ── Local profile notes (moderation) ─────────────────────────────────────────
// The site API has no notes endpoint, so notes are private to this machine:
// a JSON map `username → note` in `~/.renzora/profile_notes.json`. If the API
// grows a notes endpoint these become the offline cache.

#[cfg(not(target_arch = "wasm32"))]
fn notes_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(".renzora").join("profile_notes.json"))
}

/// All locally stored profile notes (empty when the file is absent/invalid).
pub(crate) fn load_profile_notes() -> std::collections::HashMap<String, String> {
    #[cfg(target_arch = "wasm32")]
    {
        Default::default()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        notes_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }
}

/// Save (or clear, when `note` is empty) the local note for `username`.
pub(crate) fn save_profile_note(username: &str, note: &str) {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (username, note);
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(path) = notes_path() else { return };
        let mut notes = load_profile_notes();
        if note.trim().is_empty() {
            notes.remove(username);
        } else {
            notes.insert(username.to_string(), note.trim().to_string());
        }
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string_pretty(&notes) {
            let _ = std::fs::write(path, text);
        }
    }
}

/// Whether the signed-in user has a moderator/admin site role.
pub(crate) fn is_moderator(session: &AuthSession) -> bool {
    session
        .user
        .as_ref()
        .is_some_and(|u| matches!(u.role.as_str(), "admin" | "moderator" | "mod" | "staff"))
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

// ── Timestamps ──

/// Parse the API's timestamp strings (RFC3339 `2026-07-11T18:00:00Z` or the
/// `time` crate's default `2026-07-11 18:00:00.0 +00:00:00`) into unix seconds.
/// Best-effort; assumes UTC.
pub(crate) fn parse_timestamp(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.len() < 19 {
        return None;
    }
    let date = &s[0..10];
    let time = &s[11..19];
    let mut dp = date.split('-');
    let year: i64 = dp.next()?.parse().ok()?;
    let month: i64 = dp.next()?.parse().ok()?;
    let day: i64 = dp.next()?.parse().ok()?;
    let mut tp = time.split(':');
    let hour: i64 = tp.next()?.parse().ok()?;
    let min: i64 = tp.next()?.parse().ok()?;
    let sec: i64 = tp.next()?.parse().ok()?;

    // Howard Hinnant's days_from_civil.
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;

    Some(days * 86_400 + hour * 3_600 + min * 60 + sec)
}

/// Human relative time: "just now", "5m", "3h", "2d", or "Mar 12".
pub(crate) fn relative_time(timestamp: &str) -> String {
    let Some(then) = parse_timestamp(timestamp) else {
        return String::new();
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let diff = now - then;
    if diff < 60 {
        "just now".to_string()
    } else if diff < 3_600 {
        format!("{}m", diff / 60)
    } else if diff < 86_400 {
        format!("{}h", diff / 3_600)
    } else if diff < 7 * 86_400 {
        format!("{}d", diff / 86_400)
    } else {
        // Fall back to "Mon DD" from the raw string.
        const MONTHS: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let month: usize = timestamp.get(5..7).and_then(|m| m.parse().ok()).unwrap_or(1);
        let day = timestamp.get(8..10).unwrap_or("?");
        format!("{} {}", MONTHS.get(month.saturating_sub(1)).unwrap_or(&"?"), day)
    }
}
