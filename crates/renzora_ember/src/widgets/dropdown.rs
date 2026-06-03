//! Dropdown / combobox — a box that opens a popup option menu.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::{icon_text, ui_font, EmberFonts};
use crate::reactive::Bound;
use crate::theme::{rgb, TAB_ACTIVE_BG, TAB_HOVER_BG, TEXT_MUTED, TEXT_PRIMARY};

#[derive(Component)]
pub(crate) struct EmberDropdown {
    selected: usize,
    open: bool,
    menu: Entity,
    label: Entity,
    options: Vec<String>,
}

#[derive(Component)]
pub(crate) struct EmberDropdownOption {
    dropdown: Entity,
    value: usize,
}

/// A dropdown / combobox: a box showing the current option; click to open a
/// menu of options below it, click an option to select.
pub fn dropdown(
    commands: &mut Commands,
    fonts: &EmberFonts,
    options: &[&str],
    selected: usize,
) -> Entity {
    let sel = selected.min(options.len().saturating_sub(1));
    let box_e = commands
        .spawn((
            Node {
                min_width: Val::Px(140.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                position_type: PositionType::Relative,
                ..default()
            },
            BackgroundColor(rgb(TAB_ACTIVE_BG)),
            Interaction::default(),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("dropdown"),
        ))
        .id();
    let label = commands
        .spawn((
            Text::new(options.get(sel).copied().unwrap_or("")),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(TEXT_PRIMARY)),
            Node {
                flex_grow: 1.0,
                ..default()
            },
        ))
        .id();
    let caret = icon_text(commands, &fonts.phosphor, "caret-down", TEXT_MUTED, 12.0);
    let menu = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(100.0),
                left: Val::Px(0.0),
                min_width: Val::Px(140.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(2.0)),
                margin: UiRect::top(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::clip(),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb((30, 30, 38))),
            GlobalZIndex(500),
            Name::new("dropdown-menu"),
        ))
        .id();
    let mut rows = Vec::new();
    for (i, opt) in options.iter().enumerate() {
        let row = commands
            .spawn((
                Node {
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Interaction::default(),
                EmberDropdownOption {
                    dropdown: box_e,
                    value: i,
                },
                renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
                Name::new("dropdown-option"),
            ))
            .with_children(|p| {
                p.spawn((
                    Text::new(*opt),
                    ui_font(&fonts.ui, 12.0),
                    TextColor(rgb(TEXT_PRIMARY)),
                ));
            })
            .id();
        rows.push(row);
    }
    commands.entity(menu).add_children(&rows);
    commands.entity(box_e).insert((
        EmberDropdown {
            selected: sel,
            open: false,
            menu,
            label,
            options: options.iter().map(|s| s.to_string()).collect(),
        },
        // Carry the selection as `Bound<usize>` so `bind_2way` can drive it both
        // ways (read the model on select, push external changes to the label).
        Bound::<usize>(sel),
    ));
    commands.entity(box_e).add_children(&[label, caret, menu]);
    box_e
}

pub(crate) fn dropdown_toggle(
    mut dropdowns: Query<(&Interaction, &mut EmberDropdown), Changed<Interaction>>,
    mut nodes: Query<&mut Node>,
) {
    for (interaction, mut dd) in &mut dropdowns {
        if *interaction != Interaction::Pressed {
            continue;
        }
        dd.open = !dd.open;
        if let Ok(mut n) = nodes.get_mut(dd.menu) {
            n.display = if dd.open {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
}

pub(crate) fn dropdown_select(
    options: Query<(&Interaction, &EmberDropdownOption), Changed<Interaction>>,
    mut dropdowns: Query<(&mut EmberDropdown, &mut Bound<usize>)>,
    mut nodes: Query<&mut Node>,
    mut texts: Query<&mut Text>,
) {
    for (interaction, opt) in &options {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Ok((mut dd, mut bound)) = dropdowns.get_mut(opt.dropdown) {
            dd.selected = opt.value;
            dd.open = false;
            if bound.0 != opt.value {
                bound.0 = opt.value;
            }
            let (menu, label) = (dd.menu, dd.label);
            let text = dd.options.get(opt.value).cloned().unwrap_or_default();
            if let Ok(mut n) = nodes.get_mut(menu) {
                n.display = Display::None;
            }
            if let Ok(mut t) = texts.get_mut(label) {
                *t = Text::new(text);
            }
        }
    }
}

/// Model (`Bound<usize>`) → selection + label, when the value changes externally
/// (e.g. `bind_2way` syncing from a resource). Keeps the dropdown in sync when
/// state is edited elsewhere.
pub(crate) fn dropdown_apply(
    mut dropdowns: Query<(&mut EmberDropdown, &Bound<usize>), Changed<Bound<usize>>>,
    mut texts: Query<&mut Text>,
) {
    for (mut dd, bound) in &mut dropdowns {
        if dd.selected == bound.0 {
            continue;
        }
        dd.selected = bound.0;
        let (label, text) = (dd.label, dd.options.get(bound.0).cloned().unwrap_or_default());
        if let Ok(mut t) = texts.get_mut(label) {
            *t = Text::new(text);
        }
    }
}

pub(crate) fn dropdown_option_hover(
    mut options: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<EmberDropdownOption>),
    >,
) {
    for (interaction, mut bg) in &mut options {
        bg.0 = match *interaction {
            Interaction::Hovered | Interaction::Pressed => rgb(TAB_HOVER_BG),
            Interaction::None => Color::NONE,
        };
    }
}
