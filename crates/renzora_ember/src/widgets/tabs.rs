//! Tabs — a standalone tab strip over a content area.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::ui_font;
use crate::style::{Role, Styled, WidgetState};
use crate::theme::*;

#[derive(Component)]
pub(crate) struct EmberTab {
    bar: Entity,
    index: usize,
}

#[derive(Component)]
pub(crate) struct EmberTabBar {
    panels: Vec<Entity>,
}

/// A standalone tab strip over a content area: `panels[i]` shows for tab `i`.
/// `labels` and `panels` must be the same length.
pub fn tabs(
    commands: &mut Commands,
    font: &Handle<Font>,
    labels: &[&str],
    panels: Vec<Entity>,
) -> Entity {
    let root = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Name::new("tabs"),
        ))
        .id();
    let bar = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(2.0),
                ..default()
            },
            EmberTabBar {
                panels: panels.clone(),
            },
            Name::new("tab-strip"),
        ))
        .id();
    let mut handles = Vec::new();
    for (i, label) in labels.iter().enumerate() {
        let handle = commands
            .spawn((
                Node {
                    padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(if i == 0 {
                    rgb(tab_active())
                } else {
                    rgb(header_bg())
                }),
                Interaction::default(),
                EmberTab { bar, index: i },
                Styled::with_state(
                    Role::Tab,
                    if i == 0 {
                        WidgetState::Active
                    } else {
                        WidgetState::Normal
                    },
                ),
                crate::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
                Name::new("tab"),
            ))
            .with_children(|p| {
                p.spawn((
                    Text::new(*label),
                    ui_font(font, 12.0),
                    TextColor(rgb(text_primary())),
                ));
            })
            .id();
        handles.push(handle);
    }
    commands.entity(bar).add_children(&handles);
    let content = commands
        .spawn((
            Node {
                padding: UiRect::all(Val::Px(10.0)),
                ..default()
            },
            Name::new("tab-content"),
        ))
        .id();
    // Show only the first panel.
    for (i, &panel) in panels.iter().enumerate() {
        commands.entity(panel).insert(Node {
            display: if i == 0 {
                Display::Flex
            } else {
                Display::None
            },
            ..default()
        });
    }
    commands.entity(content).add_children(&panels);
    commands.entity(root).add_children(&[bar, content]);
    root
}

pub(crate) fn tab_select(
    pressed: Query<(&Interaction, &EmberTab), Changed<Interaction>>,
    mut tabs_q: Query<(&EmberTab, &mut Styled)>,
    bars: Query<&EmberTabBar>,
    mut nodes: Query<&mut Node>,
) {
    let mut chosen: Option<(Entity, usize)> = None;
    for (interaction, tab) in &pressed {
        if *interaction == Interaction::Pressed {
            chosen = Some((tab.bar, tab.index));
            break;
        }
    }
    let Some((bar, index)) = chosen else {
        return;
    };
    // Restyle the bar's handles.
    for (tab, mut styled) in &mut tabs_q {
        if tab.bar != bar {
            continue;
        }
        styled.state = if tab.index == index {
            WidgetState::Active
        } else {
            WidgetState::Normal
        };
    }
    // Show the chosen panel, hide the rest.
    if let Ok(bar_data) = bars.get(bar) {
        for (i, &panel) in bar_data.panels.iter().enumerate() {
            if let Ok(mut n) = nodes.get_mut(panel) {
                n.display = if i == index {
                    Display::Flex
                } else {
                    Display::None
                };
            }
        }
    }
}
