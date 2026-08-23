//! Shared layout/text helpers used across the ember widgets.

use bevy::prelude::*;

use crate::font::ui_font;
use crate::theme::*;

/// A `Text` entity in the UI font at `size` and `color`.
pub(crate) fn text_node(
    commands: &mut Commands,
    font: &bevy::text::FontSource,
    text: &str,
    size: f32,
    color: (u8, u8, u8),
) -> Entity {
    commands
        .spawn((Text::new(text), ui_font(font, size), TextColor(rgb(color))))
        .id()
}

/// Format a number compactly (integers without a trailing decimal).
///
/// The decimal count scales with the magnitude, because a fixed one destroyed
/// small values outright: the import inspector's unit scale is `0.01` for a
/// centimetre source, and `{:.1}` rendered that as `0.0` — a field that read as
/// "this import will collapse to a point" while holding a perfectly correct
/// value. Anything under `0.05` had the same problem everywhere in the editor.
///
/// Trailing zeros are trimmed afterwards so widening the precision doesn't turn
/// `0.5` into `0.50`.
pub(crate) fn format_num(v: f32) -> String {
    if !v.is_finite() {
        return format!("{v}");
    }
    let m = v.abs();
    // Enough decimals to keep ~3 significant digits at any magnitude.
    let decimals = if m >= 1.0 || m == 0.0 {
        1
    } else if m >= 0.1 {
        2
    } else if m >= 0.01 {
        3
    } else if m >= 0.001 {
        4
    } else {
        6
    };
    let s = format!("{v:.decimals$}");
    let trimmed = s.trim_end_matches('0').trim_end_matches('.');
    // A value too small for even six decimals trims down to "-0"; a minus sign
    // on a displayed zero looks like a bug, so drop it.
    match trimmed {
        "" | "-" | "-0" => "0".to_string(),
        other => other.to_string(),
    }
}

/// A labeled row: a fixed-width label on the left, a control on the right.
pub fn field(commands: &mut Commands, font: &bevy::text::FontSource, label: &str, control: Entity) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(12.0),
                ..default()
            },
            Name::new("field"),
        ))
        .id();
    let lbl = commands
        .spawn((
            Text::new(label),
            ui_font(font, 12.0),
            TextColor(rgb(text_muted())),
            Node {
                min_width: Val::Px(90.0),
                ..default()
            },
        ))
        .id();
    commands.entity(row).add_children(&[lbl, control]);
    row
}

/// A horizontal row of widgets with a gap.
pub fn hstack(commands: &mut Commands, gap: f32, children: &[Entity]) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(gap),
                ..default()
            },
            Name::new("hstack"),
        ))
        .id();
    commands.entity(row).add_children(children);
    row
}

/// A small rounded color chip.
pub fn swatch(commands: &mut Commands, color: (u8, u8, u8)) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Px(28.0),
                height: Val::Px(28.0),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(color)),
            Name::new("swatch"),
        ))
        .id()
}

/// A section heading.
pub fn heading(commands: &mut Commands, font: &bevy::text::FontSource, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(font, 13.0),
            TextColor(rgb(text_muted())),
        ))
        .id()
}

// ── Layout / style utilities ──────────────────────────────────────────────────

/// A vertical column of widgets with a gap.
pub fn vstack(commands: &mut Commands, gap: f32, children: &[Entity]) -> Entity {
    let col = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(gap),
                ..default()
            },
            Name::new("vstack"),
        ))
        .id();
    commands.entity(col).add_children(children);
    col
}

/// A fixed empty box (use to push siblings apart).
pub fn spacer(commands: &mut Commands, width: f32, height: f32) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Px(width),
                height: Val::Px(height),
                ..default()
            },
            Name::new("spacer"),
        ))
        .id()
}

/// A styled container box (rounded, padded, bordered) — fill it with children.
/// The utility for "rounded corners / padding / border" in one call.
pub fn frame(commands: &mut Commands, padding: f32, radius: f32) -> Entity {
    commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(padding)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(radius)),
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            BorderColor::all(rgb(border())),
            Name::new("frame"),
        ))
        .id()
}

#[cfg(test)]
mod tests {
    use super::format_num;

    #[test]
    fn whole_numbers_lose_the_decimal() {
        assert_eq!(format_num(1.0), "1");
        assert_eq!(format_num(0.0), "0");
        assert_eq!(format_num(-4.0), "-4");
        assert_eq!(format_num(1583.0), "1583");
    }

    #[test]
    fn small_scales_survive() {
        // The bug this guards: a centimetre unit scale rendered as "0.0",
        // which reads as a scale that would collapse the model to a point.
        assert_eq!(format_num(0.01), "0.01");
        assert_eq!(format_num(0.0254), "0.025");
        assert_eq!(format_num(0.001), "0.001");
        assert_eq!(format_num(-0.01), "-0.01");
    }

    #[test]
    fn ordinary_fractions_stay_short() {
        assert_eq!(format_num(0.5), "0.5");
        assert_eq!(format_num(1.5), "1.5");
        assert_eq!(format_num(2.5), "2.5");
    }

    #[test]
    fn near_zero_does_not_print_as_negative_zero() {
        assert_eq!(format_num(-1e-9), "0");
        assert_eq!(format_num(1e-9), "0");
    }
}
