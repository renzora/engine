//! Range — a dual-thumb range slider.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::theme::*;

#[derive(Component)]
pub(crate) struct EmberRange {
    low: f32,
    high: f32,
    fill: Entity,
    low_thumb: Entity,
    high_thumb: Entity,
    active: Option<bool>,
}

fn range_thumb(commands: &mut Commands, v: f32) -> Entity {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(v * 100.0),
                top: Val::Px(2.0),
                margin: UiRect::left(Val::Px(-7.0)),
                width: Val::Px(14.0),
                height: Val::Px(14.0),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(rgb(on_accent())),
            bevy::ui::FocusPolicy::Pass,
            Name::new("range-thumb"),
        ))
        .id()
}

/// A dual-thumb range slider (`low`/`high` in 0..1).
pub fn range(commands: &mut Commands, low: f32, high: f32) -> Entity {
    let lo = low.clamp(0.0, 1.0).min(high);
    let hi = high.clamp(0.0, 1.0).max(low);
    let row = commands
        .spawn((
            Node {
                width: Val::Px(180.0),
                height: Val::Px(18.0),
                position_type: PositionType::Relative,
                align_items: AlignItems::Center,
                ..default()
            },
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("range"),
        ))
        .id();
    let track = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(6.0),
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            bevy::ui::FocusPolicy::Pass,
            Name::new("range-track"),
        ))
        .id();
    let fill = commands
        .spawn((
            Node {
                width: Val::Percent((hi - lo) * 100.0),
                height: Val::Percent(100.0),
                margin: UiRect::left(Val::Percent(lo * 100.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(accent())),
            Name::new("range-fill"),
        ))
        .id();
    commands.entity(track).add_child(fill);
    let low_thumb = range_thumb(commands, lo);
    let high_thumb = range_thumb(commands, hi);
    commands
        .entity(row)
        .add_children(&[track, low_thumb, high_thumb]);
    commands.entity(row).insert(EmberRange {
        low: lo,
        high: hi,
        fill,
        low_thumb,
        high_thumb,
        active: None,
    });
    row
}

pub(crate) fn range_drag(
    mut ranges: Query<(&Interaction, &bevy::ui::RelativeCursorPosition, &mut EmberRange)>,
    mut nodes: Query<&mut Node>,
) {
    for (interaction, rcp, mut r) in &mut ranges {
        if *interaction != Interaction::Pressed {
            r.active = None;
            continue;
        }
        let Some(n) = rcp.normalized else {
            continue;
        };
        let pos = (n.x + 0.5).clamp(0.0, 1.0);
        if r.active.is_none() {
            r.active = Some((pos - r.high).abs() < (pos - r.low).abs());
        }
        if r.active == Some(true) {
            r.high = pos.max(r.low);
        } else {
            r.low = pos.min(r.high);
        }
        let (lo, hi) = (r.low, r.high);
        if let Ok(mut f) = nodes.get_mut(r.fill) {
            f.margin.left = Val::Percent(lo * 100.0);
            f.width = Val::Percent((hi - lo) * 100.0);
        }
        if let Ok(mut t) = nodes.get_mut(r.low_thumb) {
            t.left = Val::Percent(lo * 100.0);
        }
        if let Ok(mut t) = nodes.get_mut(r.high_thumb) {
            t.left = Val::Percent(hi * 100.0);
        }
    }
}
