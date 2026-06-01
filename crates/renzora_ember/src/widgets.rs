//! `renzora_ember` UI components — the start of a reusable bevy_ui widget set
//! used by the editor and games. These are plain entity builders + the systems
//! that animate their interaction states. [`build_gallery`] showcases them.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::{icon_text, ui_font, EmberFonts};
use crate::style::{Role, Styled, WidgetState};
use crate::theme::{
    rgb, ACCENT_BLUE, CLOSE_RED, HEADER_BG, PLAY_GREEN, TAB_ACTIVE_BG, TAB_HOVER_BG, TEXT_MUTED,
    TEXT_PRIMARY, WARN_AMBER,
};

/// Registers the widget interaction systems.
pub struct WidgetsPlugin;

impl Plugin for WidgetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                button_interact,
                toggle_interact,
                slider_drag,
                checkbox_interact,
                radio_interact,
                segmented_interact,
                stepper_interact,
                dropdown_toggle,
                dropdown_select,
                dropdown_option_hover,
                text_input_focus,
                text_input_type,
                drag_value_drag,
                color_picker_sync,
                knob_drag,
                fader_drag,
                xy_pad_drag,
            ),
        );
    }
}

// ── Button ───────────────────────────────────────────────────────────────────

#[derive(Component)]
struct EmberButton;

/// A clickable button with hover/press color states.
pub fn button(commands: &mut Commands, font: &Handle<Font>, label: &str) -> Entity {
    let b = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(TAB_ACTIVE_BG)),
            Interaction::default(),
            EmberButton,
            Styled::new(Role::Button),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("button"),
        ))
        .id();
    let t = commands
        .spawn((
            Text::new(label),
            ui_font(font, 12.0),
            TextColor(rgb(TEXT_PRIMARY)),
        ))
        .id();
    commands.entity(b).add_child(t);
    b
}

fn button_interact(
    mut q: Query<(&Interaction, &mut Styled), (With<EmberButton>, Changed<Interaction>)>,
) {
    for (interaction, mut styled) in &mut q {
        styled.state = match interaction {
            Interaction::Pressed => WidgetState::Pressed,
            Interaction::Hovered => WidgetState::Hover,
            Interaction::None => WidgetState::Normal,
        };
    }
}

// ── Toggle ───────────────────────────────────────────────────────────────────

#[derive(Component)]
struct EmberToggle {
    on: bool,
}

/// An on/off switch — click to flip.
pub fn toggle(commands: &mut Commands, on: bool) -> Entity {
    let track = commands
        .spawn((
            Node {
                width: Val::Px(38.0),
                height: Val::Px(20.0),
                padding: UiRect::all(Val::Px(2.0)),
                align_items: AlignItems::Center,
                justify_content: if on {
                    JustifyContent::FlexEnd
                } else {
                    JustifyContent::FlexStart
                },
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(if on { rgb(ACCENT_BLUE) } else { rgb((60, 60, 70)) }),
            Interaction::default(),
            EmberToggle { on },
            Styled::with_state(
                Role::Toggle,
                if on {
                    WidgetState::Active
                } else {
                    WidgetState::Normal
                },
            ),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("toggle"),
        ))
        .id();
    let knob = commands
        .spawn((
            Node {
                width: Val::Px(16.0),
                height: Val::Px(16.0),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb((240, 240, 245))),
            Name::new("toggle-knob"),
        ))
        .id();
    commands.entity(track).add_child(knob);
    track
}

fn toggle_interact(
    mut q: Query<(&Interaction, &mut EmberToggle, &mut Styled, &mut Node), Changed<Interaction>>,
) {
    for (interaction, mut tog, mut styled, mut node) in &mut q {
        if *interaction == Interaction::Pressed {
            tog.on = !tog.on;
            styled.state = if tog.on {
                WidgetState::Active
            } else {
                WidgetState::Normal
            };
            node.justify_content = if tog.on {
                JustifyContent::FlexEnd
            } else {
                JustifyContent::FlexStart
            };
        }
    }
}

// ── Slider ───────────────────────────────────────────────────────────────────

#[derive(Component)]
struct EmberSlider {
    value: f32,
    fill: Entity,
    thumb: Entity,
}

/// A draggable slider with `value` in 0..1. Click/drag anywhere on it to set
/// the value.
pub fn slider(commands: &mut Commands, value: f32) -> Entity {
    let v = value.clamp(0.0, 1.0);
    // 18px-tall hit area so it's easy to grab; the visual track is 6px.
    let row = commands
        .spawn((
            Node {
                width: Val::Px(160.0),
                height: Val::Px(18.0),
                position_type: PositionType::Relative,
                align_items: AlignItems::Center,
                ..default()
            },
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("slider"),
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
            BackgroundColor(rgb((55, 55, 66))),
            bevy::ui::FocusPolicy::Pass,
            Name::new("slider-track"),
        ))
        .id();
    let fill = commands
        .spawn((
            Node {
                width: Val::Percent(v * 100.0),
                height: Val::Percent(100.0),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(ACCENT_BLUE)),
            Name::new("slider-fill"),
        ))
        .id();
    let thumb = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(v * 100.0),
                margin: UiRect::left(Val::Px(-7.0)),
                width: Val::Px(14.0),
                height: Val::Px(14.0),
                border_radius: BorderRadius::all(Val::Px(7.0)),
                ..default()
            },
            BackgroundColor(rgb((240, 240, 245))),
            bevy::ui::FocusPolicy::Pass,
            Name::new("slider-thumb"),
        ))
        .id();
    commands.entity(track).add_child(fill);
    commands.entity(row).add_children(&[track, thumb]);
    commands.entity(row).insert(EmberSlider { value: v, fill, thumb });
    row
}

fn slider_drag(
    mut sliders: Query<(&Interaction, &bevy::ui::RelativeCursorPosition, &mut EmberSlider)>,
    mut nodes: Query<&mut Node>,
) {
    for (interaction, rcp, mut s) in &mut sliders {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(n) = rcp.normalized else {
            continue;
        };
        // `normalized` is centered (-0.5..0.5); shift to 0..1.
        let v = (n.x + 0.5).clamp(0.0, 1.0);
        if (v - s.value).abs() < 0.001 {
            continue;
        }
        s.value = v;
        if let Ok(mut fnode) = nodes.get_mut(s.fill) {
            fnode.width = Val::Percent(v * 100.0);
        }
        if let Ok(mut tnode) = nodes.get_mut(s.thumb) {
            tnode.left = Val::Percent(v * 100.0);
        }
    }
}

// ── Checkbox ─────────────────────────────────────────────────────────────────

#[derive(Component)]
struct EmberCheckbox {
    checked: bool,
    mark: Entity,
}

/// A checkbox — click to toggle.
pub fn checkbox(commands: &mut Commands, checked: bool) -> Entity {
    let box_e = commands
        .spawn((
            Node {
                width: Val::Px(18.0),
                height: Val::Px(18.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(if checked {
                rgb(ACCENT_BLUE)
            } else {
                Color::NONE
            }),
            BorderColor::all(rgb((92, 92, 104))),
            Interaction::default(),
            Styled::with_state(
                Role::Checkbox,
                if checked {
                    WidgetState::Active
                } else {
                    WidgetState::Normal
                },
            ),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("checkbox"),
        ))
        .id();
    // Inner check mark (a small white square), shown only when checked.
    let mark = commands
        .spawn((
            Node {
                width: Val::Px(8.0),
                height: Val::Px(8.0),
                border_radius: BorderRadius::all(Val::Px(2.0)),
                display: if checked {
                    Display::Flex
                } else {
                    Display::None
                },
                ..default()
            },
            BackgroundColor(rgb((245, 245, 250))),
            bevy::ui::FocusPolicy::Pass,
            Name::new("check"),
        ))
        .id();
    commands.entity(box_e).insert(EmberCheckbox { checked, mark });
    commands.entity(box_e).add_child(mark);
    box_e
}

fn checkbox_interact(
    mut boxes: Query<(&Interaction, &mut EmberCheckbox, &mut Styled), Changed<Interaction>>,
    mut nodes: Query<&mut Node>,
) {
    for (interaction, mut cb, mut styled) in &mut boxes {
        if *interaction != Interaction::Pressed {
            continue;
        }
        cb.checked = !cb.checked;
        styled.state = if cb.checked {
            WidgetState::Active
        } else {
            WidgetState::Normal
        };
        if let Ok(mut n) = nodes.get_mut(cb.mark) {
            n.display = if cb.checked {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
}

// ── Radio group ──────────────────────────────────────────────────────────────

#[derive(Component)]
struct EmberRadio {
    group: Entity,
    value: usize,
    dot: Entity,
}

/// A vertical radio group; returns the group container. Exactly one option is
/// selected at a time.
pub fn radio_group(
    commands: &mut Commands,
    font: &Handle<Font>,
    options: &[&str],
    selected: usize,
) -> Entity {
    let group = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(14.0),
                ..default()
            },
            Name::new("radio-group"),
        ))
        .id();
    let mut kids = Vec::new();
    for (i, label) in options.iter().enumerate() {
        let on = i == selected;
        let opt = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(5.0),
                    ..default()
                },
                Interaction::default(),
                renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
                Name::new("radio"),
            ))
            .id();
        let ring = commands
            .spawn((
                Node {
                    width: Val::Px(16.0),
                    height: Val::Px(16.0),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BorderColor::all(rgb((92, 92, 104))),
                bevy::ui::FocusPolicy::Pass,
                Name::new("radio-ring"),
            ))
            .id();
        let dot = commands
            .spawn((
                Node {
                    width: Val::Px(8.0),
                    height: Val::Px(8.0),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    display: if on { Display::Flex } else { Display::None },
                    ..default()
                },
                BackgroundColor(rgb(ACCENT_BLUE)),
                bevy::ui::FocusPolicy::Pass,
                Name::new("radio-dot"),
            ))
            .id();
        let text = commands
            .spawn((
                Text::new(*label),
                ui_font(font, 12.0),
                TextColor(rgb(TEXT_PRIMARY)),
            ))
            .id();
        commands.entity(ring).add_child(dot);
        commands.entity(opt).insert(EmberRadio {
            group,
            value: i,
            dot,
        });
        commands.entity(opt).add_children(&[ring, text]);
        kids.push(opt);
    }
    commands.entity(group).add_children(&kids);
    group
}

fn radio_interact(
    pressed: Query<(&Interaction, &EmberRadio), Changed<Interaction>>,
    radios: Query<&EmberRadio>,
    mut nodes: Query<&mut Node>,
) {
    let mut chosen: Option<(Entity, usize)> = None;
    for (interaction, r) in &pressed {
        if *interaction == Interaction::Pressed {
            chosen = Some((r.group, r.value));
            break;
        }
    }
    let Some((group, value)) = chosen else {
        return;
    };
    for r in &radios {
        if r.group != group {
            continue;
        }
        if let Ok(mut n) = nodes.get_mut(r.dot) {
            n.display = if r.value == value {
                Display::Flex
            } else {
                Display::None
            };
        }
    }
}

// ── Segmented control ────────────────────────────────────────────────────────

#[derive(Component)]
struct EmberSegment {
    group: Entity,
    value: usize,
}

/// A segmented control (a row of buttons, one selected). Returns the container.
pub fn segmented(
    commands: &mut Commands,
    font: &Handle<Font>,
    options: &[&str],
    selected: usize,
) -> Entity {
    let group = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb((40, 40, 48))),
            Name::new("segmented"),
        ))
        .id();
    let mut kids = Vec::new();
    for (i, label) in options.iter().enumerate() {
        let on = i == selected;
        let seg = commands
            .spawn((
                Node {
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(5.0)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(if on {
                    rgb(ACCENT_BLUE)
                } else {
                    Color::NONE
                }),
                Interaction::default(),
                EmberSegment { group, value: i },
                Styled::with_state(
                    Role::Segment,
                    if on {
                        WidgetState::Active
                    } else {
                        WidgetState::Normal
                    },
                ),
                renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
                Name::new("segment"),
            ))
            .with_children(|p| {
                p.spawn((
                    Text::new(*label),
                    ui_font(font, 12.0),
                    TextColor(rgb(TEXT_PRIMARY)),
                ));
            })
            .id();
        kids.push(seg);
    }
    commands.entity(group).add_children(&kids);
    group
}

fn segmented_interact(
    pressed: Query<(&Interaction, &EmberSegment), Changed<Interaction>>,
    mut segments: Query<(&EmberSegment, &mut Styled)>,
) {
    let mut chosen: Option<(Entity, usize)> = None;
    for (interaction, s) in &pressed {
        if *interaction == Interaction::Pressed {
            chosen = Some((s.group, s.value));
            break;
        }
    }
    let Some((group, value)) = chosen else {
        return;
    };
    for (s, mut styled) in &mut segments {
        if s.group != group {
            continue;
        }
        styled.state = if s.value == value {
            WidgetState::Active
        } else {
            WidgetState::Normal
        };
    }
}

// ── Number stepper ───────────────────────────────────────────────────────────

#[derive(Component)]
struct EmberStepper {
    value: f32,
    step: f32,
    display: Entity,
}

#[derive(Component)]
struct EmberStepButton {
    stepper: Entity,
    dir: f32,
}

/// A number stepper: `[−] value [+]`. Returns the container.
pub fn number_stepper(commands: &mut Commands, font: &Handle<Font>, value: f32, step: f32) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
            Name::new("stepper"),
        ))
        .id();
    let display = commands
        .spawn((
            Text::new(format_num(value)),
            ui_font(font, 12.0),
            TextColor(rgb(TEXT_PRIMARY)),
            Node {
                min_width: Val::Px(32.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .id();
    commands.entity(row).insert(EmberStepper {
        value,
        step,
        display,
    });
    let minus = step_button(commands, font, row, "−", -1.0);
    let plus = step_button(commands, font, row, "+", 1.0);
    commands.entity(row).add_children(&[minus, display, plus]);
    row
}

fn step_button(
    commands: &mut Commands,
    font: &Handle<Font>,
    stepper: Entity,
    label: &str,
    dir: f32,
) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Px(22.0),
                height: Val::Px(22.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(TAB_ACTIVE_BG)),
            Interaction::default(),
            EmberButton,
            Styled::new(Role::IconButton),
            EmberStepButton { stepper, dir },
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("step-button"),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(label),
                ui_font(font, 14.0),
                TextColor(rgb(TEXT_PRIMARY)),
            ));
        })
        .id()
}

fn stepper_interact(
    pressed: Query<(&Interaction, &EmberStepButton), Changed<Interaction>>,
    mut steppers: Query<&mut EmberStepper>,
    mut texts: Query<&mut Text>,
) {
    for (interaction, btn) in &pressed {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Ok(mut s) = steppers.get_mut(btn.stepper) {
            s.value += btn.dir * s.step;
            let (display, value) = (s.display, s.value);
            if let Ok(mut t) = texts.get_mut(display) {
                *t = Text::new(format_num(value));
            }
        }
    }
}

fn format_num(v: f32) -> String {
    if v.fract().abs() < 0.001 {
        format!("{}", v as i32)
    } else {
        format!("{v:.1}")
    }
}

// ── Dropdown / combobox ──────────────────────────────────────────────────────

#[derive(Component)]
struct EmberDropdown {
    selected: usize,
    open: bool,
    menu: Entity,
    label: Entity,
    options: Vec<String>,
}

#[derive(Component)]
struct EmberDropdownOption {
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
    commands.entity(box_e).insert(EmberDropdown {
        selected: sel,
        open: false,
        menu,
        label,
        options: options.iter().map(|s| s.to_string()).collect(),
    });
    commands.entity(box_e).add_children(&[label, caret, menu]);
    box_e
}

fn dropdown_toggle(
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

fn dropdown_select(
    options: Query<(&Interaction, &EmberDropdownOption), Changed<Interaction>>,
    mut dropdowns: Query<&mut EmberDropdown>,
    mut nodes: Query<&mut Node>,
    mut texts: Query<&mut Text>,
) {
    for (interaction, opt) in &options {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Ok(mut dd) = dropdowns.get_mut(opt.dropdown) {
            dd.selected = opt.value;
            dd.open = false;
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

fn dropdown_option_hover(
    mut options: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<EmberDropdownOption>)>,
) {
    for (interaction, mut bg) in &mut options {
        bg.0 = match *interaction {
            Interaction::Hovered | Interaction::Pressed => rgb(TAB_HOVER_BG),
            Interaction::None => Color::NONE,
        };
    }
}

// ── Text input ───────────────────────────────────────────────────────────────

#[derive(Component)]
struct EmberTextInput {
    value: String,
    focused: bool,
    text_entity: Entity,
    placeholder: String,
}

/// A single-line text input. Click to focus, type to edit (basic: character
/// entry + backspace; no cursor/selection yet).
pub fn text_input(
    commands: &mut Commands,
    font: &Handle<Font>,
    placeholder: &str,
    value: &str,
) -> Entity {
    let empty = value.is_empty();
    let box_e = commands
        .spawn((
            Node {
                min_width: Val::Px(180.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb((28, 28, 34))),
            BorderColor::all(rgb((70, 70, 82))),
            Interaction::default(),
            Styled::new(Role::Input),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Text),
            Name::new("text-input"),
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(if empty { placeholder } else { value }),
            ui_font(font, 12.0),
            TextColor(rgb(if empty { TEXT_MUTED } else { TEXT_PRIMARY })),
        ))
        .id();
    commands.entity(box_e).insert(EmberTextInput {
        value: value.to_string(),
        focused: false,
        text_entity: text,
        placeholder: placeholder.to_string(),
    });
    commands.entity(box_e).add_child(text);
    box_e
}

fn text_input_focus(
    pressed: Query<(Entity, &Interaction), (With<EmberTextInput>, Changed<Interaction>)>,
    mut inputs: Query<(Entity, &mut EmberTextInput, &mut Styled)>,
) {
    let mut clicked = None;
    for (e, interaction) in &pressed {
        if *interaction == Interaction::Pressed {
            clicked = Some(e);
            break;
        }
    }
    let Some(clicked) = clicked else {
        return;
    };
    for (e, mut inp, mut styled) in &mut inputs {
        let focus = e == clicked;
        if inp.focused != focus {
            inp.focused = focus;
            styled.state = if focus {
                WidgetState::Active
            } else {
                WidgetState::Normal
            };
        }
    }
}

fn text_input_type(
    mut events: MessageReader<KeyboardInput>,
    mut inputs: Query<&mut EmberTextInput>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
) {
    for ev in events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        for mut inp in &mut inputs {
            if !inp.focused {
                continue;
            }
            match &ev.logical_key {
                Key::Character(s) => inp.value.push_str(s),
                Key::Space => inp.value.push(' '),
                Key::Backspace => {
                    inp.value.pop();
                }
                _ => {}
            }
            let (text_e, val, ph) = (inp.text_entity, inp.value.clone(), inp.placeholder.clone());
            if let Ok((mut t, mut c)) = texts.get_mut(text_e) {
                if val.is_empty() {
                    *t = Text::new(ph);
                    c.0 = rgb(TEXT_MUTED);
                } else {
                    *t = Text::new(val);
                    c.0 = rgb(TEXT_PRIMARY);
                }
            }
            break;
        }
    }
}

// ── Field row helper ─────────────────────────────────────────────────────────

/// A labeled row: a fixed-width label on the left, a control on the right.
pub fn field(commands: &mut Commands, font: &Handle<Font>, label: &str, control: Entity) -> Entity {
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
            TextColor(rgb(TEXT_MUTED)),
            Node {
                min_width: Val::Px(90.0),
                ..default()
            },
        ))
        .id();
    commands.entity(row).add_children(&[lbl, control]);
    row
}

// ── Swatch + helpers ─────────────────────────────────────────────────────────

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
pub fn heading(commands: &mut Commands, font: &Handle<Font>, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(font, 13.0),
            TextColor(rgb(TEXT_MUTED)),
        ))
        .id()
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

// ── Typography ───────────────────────────────────────────────────────────────

fn text_node(commands: &mut Commands, font: &Handle<Font>, text: &str, size: f32, color: (u8, u8, u8)) -> Entity {
    commands
        .spawn((Text::new(text), ui_font(font, size), TextColor(rgb(color))))
        .id()
}

/// Display heading, level 1 (largest).
pub fn h1(commands: &mut Commands, font: &Handle<Font>, text: &str) -> Entity {
    text_node(commands, font, text, 26.0, TEXT_PRIMARY)
}
/// Heading, level 2.
pub fn h2(commands: &mut Commands, font: &Handle<Font>, text: &str) -> Entity {
    text_node(commands, font, text, 21.0, TEXT_PRIMARY)
}
/// Heading, level 3.
pub fn h3(commands: &mut Commands, font: &Handle<Font>, text: &str) -> Entity {
    text_node(commands, font, text, 17.0, TEXT_PRIMARY)
}
/// Heading, level 4 (smallest).
pub fn h4(commands: &mut Commands, font: &Handle<Font>, text: &str) -> Entity {
    text_node(commands, font, text, 14.0, TEXT_PRIMARY)
}
/// Body paragraph text.
pub fn paragraph(commands: &mut Commands, font: &Handle<Font>, text: &str) -> Entity {
    text_node(commands, font, text, 13.0, TEXT_PRIMARY)
}
/// Small, muted caption.
pub fn caption(commands: &mut Commands, font: &Handle<Font>, text: &str) -> Entity {
    text_node(commands, font, text, 11.0, TEXT_MUTED)
}
/// A muted form/field label.
pub fn label(commands: &mut Commands, font: &Handle<Font>, text: &str) -> Entity {
    text_node(commands, font, text, 12.0, TEXT_MUTED)
}
/// An accent-colored hyperlink (pointer cursor; click handling is the caller's).
pub fn link(commands: &mut Commands, font: &Handle<Font>, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(font, 12.0),
            TextColor(rgb(ACCENT_BLUE)),
            Interaction::default(),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("link"),
        ))
        .id()
}
/// Inline code — a subtle chip around monospaced-looking text.
pub fn code(commands: &mut Commands, font: &Handle<Font>, text: &str) -> Entity {
    commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb((28, 28, 34))),
            Name::new("code"),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(text),
                ui_font(font, 12.0),
                TextColor(rgb((200, 210, 235))),
            ));
        })
        .id()
}

// ── Feedback ─────────────────────────────────────────────────────────────────

/// Semantic tone for badges / alerts / toasts.
#[derive(Clone, Copy)]
pub enum Tone {
    Neutral,
    Info,
    Success,
    Warn,
    Error,
}

impl Tone {
    fn color(self) -> (u8, u8, u8) {
        match self {
            Tone::Neutral => (120, 120, 134),
            Tone::Info => ACCENT_BLUE,
            Tone::Success => PLAY_GREEN,
            Tone::Warn => WARN_AMBER,
            Tone::Error => CLOSE_RED,
        }
    }
    fn icon(self) -> &'static str {
        match self {
            Tone::Neutral => "info",
            Tone::Info => "info",
            Tone::Success => "check-circle",
            Tone::Warn => "warning",
            Tone::Error => "x-circle",
        }
    }
}

/// A small pill badge in a semantic tone.
pub fn badge(commands: &mut Commands, font: &Handle<Font>, text: &str, tone: Tone) -> Entity {
    commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)),
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(9.0)),
                ..default()
            },
            BackgroundColor(rgb(tone.color())),
            Name::new("badge"),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(text),
                ui_font(font, 11.0),
                TextColor(rgb((255, 255, 255))),
            ));
        })
        .id()
}

/// An inline alert box (themed container + tone icon + title/body).
pub fn alert(
    commands: &mut Commands,
    fonts: &EmberFonts,
    tone: Tone,
    title: &str,
    body: &str,
) -> Entity {
    let box_e = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(10.0),
                align_items: AlignItems::FlexStart,
                min_width: Val::Px(240.0),
                ..default()
            },
            BackgroundColor(rgb((38, 38, 48))),
            BorderColor::all(rgb((60, 60, 74))),
            Styled::new(Role::Alert),
            Name::new("alert"),
        ))
        .id();
    let icon = icon_text(commands, &fonts.phosphor, tone.icon(), tone.color(), 16.0);
    let col = commands
        .spawn((Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        },))
        .id();
    let t = text_node(commands, &fonts.ui, title, 13.0, TEXT_PRIMARY);
    let b = text_node(commands, &fonts.ui, body, 12.0, TEXT_MUTED);
    commands.entity(col).add_children(&[t, b]);
    commands.entity(box_e).add_children(&[icon, col]);
    box_e
}

/// A toast notification (themed card + tone icon + message + close ×).
pub fn toast(commands: &mut Commands, fonts: &EmberFonts, tone: Tone, message: &str) -> Entity {
    let box_e = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(8.0),
                align_items: AlignItems::Center,
                min_width: Val::Px(220.0),
                ..default()
            },
            BackgroundColor(rgb((44, 44, 55))),
            BorderColor::all(rgb((64, 64, 78))),
            Styled::new(Role::Toast),
            Name::new("toast"),
        ))
        .id();
    let icon = icon_text(commands, &fonts.phosphor, tone.icon(), tone.color(), 14.0);
    let msg = commands
        .spawn((
            Text::new(message),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(TEXT_PRIMARY)),
            Node {
                flex_grow: 1.0,
                ..default()
            },
        ))
        .id();
    let close = icon_text(commands, &fonts.phosphor, "x", TEXT_MUTED, 12.0);
    commands.entity(box_e).add_children(&[icon, msg, close]);
    box_e
}

/// A determinate progress bar (`value` 0..1).
pub fn progress(commands: &mut Commands, value: f32) -> Entity {
    let v = value.clamp(0.0, 1.0);
    let track = commands
        .spawn((
            Node {
                width: Val::Px(180.0),
                height: Val::Px(8.0),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb((40, 40, 48))),
            Name::new("progress"),
        ))
        .id();
    let fill = commands
        .spawn((
            Node {
                width: Val::Percent(v * 100.0),
                height: Val::Percent(100.0),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(ACCENT_BLUE)),
            Name::new("progress-fill"),
        ))
        .id();
    commands.entity(track).add_child(fill);
    track
}

/// A skeleton placeholder block (loading state; shimmer animation comes later).
pub fn skeleton(commands: &mut Commands, width: f32, height: f32) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Px(width),
                height: Val::Px(height),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb((48, 48, 58))),
            Name::new("skeleton"),
        ))
        .id()
}

// ── Inspector controls ──────────────────────────────────────────────────────

/// An inspector property row: a muted label on the left, a control pushed to
/// the right.
pub fn property_row(
    commands: &mut Commands,
    font: &Handle<Font>,
    label: &str,
    control: Entity,
) -> Entity {
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: Val::Px(10.0),
                ..default()
            },
            Name::new("property-row"),
        ))
        .id();
    let lbl = text_node(commands, font, label, 12.0, TEXT_MUTED);
    commands.entity(row).add_children(&[lbl, control]);
    row
}

/// A scrubbable numeric field (drag horizontally to change). `axis` is an
/// optional colored prefix (e.g. "X").
#[derive(Component)]
struct EmberDragValue {
    value: f32,
    step: f32,
    text: Entity,
    last_x: Option<f32>,
}

pub fn drag_value(
    commands: &mut Commands,
    font: &Handle<Font>,
    axis: &str,
    axis_color: (u8, u8, u8),
    value: f32,
    step: f32,
) -> Entity {
    let box_e = commands
        .spawn((
            Node {
                min_width: Val::Px(58.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(5.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb((28, 28, 34))),
            BorderColor::all(rgb((70, 70, 82))),
            Styled::new(Role::Input),
            Interaction::default(),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::EwResize),
            Name::new("drag-value"),
        ))
        .id();
    let text = text_node(commands, font, &format_num(value), 12.0, TEXT_PRIMARY);
    let mut kids = Vec::new();
    if !axis.is_empty() {
        kids.push(text_node(commands, font, axis, 11.0, axis_color));
    }
    kids.push(text);
    commands.entity(box_e).insert(EmberDragValue {
        value,
        step,
        text,
        last_x: None,
    });
    commands.entity(box_e).add_children(&kids);
    box_e
}

fn drag_value_drag(
    windows: Query<&Window>,
    mut values: Query<(&Interaction, &mut EmberDragValue)>,
    mut texts: Query<&mut Text>,
) {
    let cursor_x = windows.single().ok().and_then(|w| w.cursor_position()).map(|p| p.x);
    for (interaction, mut dv) in &mut values {
        if *interaction == Interaction::Pressed {
            if let (Some(cx), Some(last)) = (cursor_x, dv.last_x) {
                let delta = cx - last;
                if delta != 0.0 {
                    dv.value += delta * dv.step;
                    let (t, v) = (dv.text, dv.value);
                    if let Ok(mut text) = texts.get_mut(t) {
                        *text = Text::new(format_num(v));
                    }
                }
            }
            dv.last_x = cursor_x;
        } else if dv.last_x.is_some() {
            dv.last_x = None;
        }
    }
}

/// Three colored drag-value fields (X/Y/Z) for editing a vector.
pub fn vec3_edit(commands: &mut Commands, font: &Handle<Font>, x: f32, y: f32, z: f32) -> Entity {
    let fields = [
        drag_value(commands, font, "X", (224, 110, 110), x, 0.05),
        drag_value(commands, font, "Y", (130, 200, 130), y, 0.05),
        drag_value(commands, font, "Z", (120, 150, 240), z, 0.05),
    ];
    hstack(commands, 6.0, &fields)
}

/// A color picker: a live preview swatch driven by R/G/B sliders.
#[derive(Component)]
struct EmberColorPicker {
    r: Entity,
    g: Entity,
    b: Entity,
    preview: Entity,
}

pub fn color_picker(commands: &mut Commands, color: (u8, u8, u8)) -> Entity {
    let (r0, g0, b0) = color;
    let root = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                ..default()
            },
            Name::new("color-picker"),
        ))
        .id();
    let preview = commands
        .spawn((
            Node {
                width: Val::Px(36.0),
                height: Val::Px(36.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(color)),
            BorderColor::all(rgb((70, 70, 82))),
            Name::new("color-preview"),
        ))
        .id();
    let r = slider(commands, r0 as f32 / 255.0);
    let g = slider(commands, g0 as f32 / 255.0);
    let b = slider(commands, b0 as f32 / 255.0);
    let sliders = commands
        .spawn((Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        },))
        .id();
    commands.entity(sliders).add_children(&[r, g, b]);
    commands.entity(root).add_children(&[preview, sliders]);
    commands
        .entity(root)
        .insert(EmberColorPicker { r, g, b, preview });
    root
}

fn color_picker_sync(
    pickers: Query<&EmberColorPicker>,
    sliders: Query<&EmberSlider>,
    mut bgs: Query<&mut BackgroundColor>,
) {
    for p in &pickers {
        let (Ok(r), Ok(g), Ok(b)) = (sliders.get(p.r), sliders.get(p.g), sliders.get(p.b)) else {
            continue;
        };
        let col = Color::srgb(r.value, g.value, b.value);
        if let Ok(mut bg) = bgs.get_mut(p.preview) {
            if bg.0 != col {
                bg.0 = col;
            }
        }
    }
}

/// A rotary knob (drag vertically to change `value` 0..1).
#[derive(Component)]
struct EmberKnob {
    value: f32,
    indicator: Entity,
}

/// Top-left offset of the knob indicator dot for a given value (270° sweep).
fn knob_offset(value: f32) -> (f32, f32) {
    let theta = (-135.0 + value * 270.0_f32).to_radians();
    let cx = 22.0 + 14.0 * theta.sin();
    let cy = 22.0 - 14.0 * theta.cos();
    (cx - 3.0, cy - 3.0)
}

pub fn knob(commands: &mut Commands, value: f32) -> Entity {
    let v = value.clamp(0.0, 1.0);
    let body = commands
        .spawn((
            Node {
                width: Val::Px(44.0),
                height: Val::Px(44.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(22.0)),
                position_type: PositionType::Relative,
                ..default()
            },
            BackgroundColor(rgb((40, 40, 48))),
            BorderColor::all(rgb((70, 70, 82))),
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::NsResize),
            Name::new("knob"),
        ))
        .id();
    let (lx, ty) = knob_offset(v);
    let indicator = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(lx),
                top: Val::Px(ty),
                width: Val::Px(6.0),
                height: Val::Px(6.0),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(ACCENT_BLUE)),
            bevy::ui::FocusPolicy::Pass,
            Name::new("knob-indicator"),
        ))
        .id();
    commands.entity(body).add_child(indicator);
    commands.entity(body).insert(EmberKnob { value: v, indicator });
    body
}

fn knob_drag(
    mut knobs: Query<(&Interaction, &bevy::ui::RelativeCursorPosition, &mut EmberKnob)>,
    mut nodes: Query<&mut Node>,
) {
    for (interaction, rcp, mut k) in &mut knobs {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(n) = rcp.normalized else {
            continue;
        };
        let v = (0.5 - n.y).clamp(0.0, 1.0);
        if (v - k.value).abs() < 0.001 {
            continue;
        }
        k.value = v;
        let (lx, ty) = knob_offset(v);
        if let Ok(mut node) = nodes.get_mut(k.indicator) {
            node.left = Val::Px(lx);
            node.top = Val::Px(ty);
        }
    }
}

/// A vertical fader (drag to change `value` 0..1).
#[derive(Component)]
struct EmberFader {
    value: f32,
    fill: Entity,
    thumb: Entity,
}

pub fn fader(commands: &mut Commands, value: f32) -> Entity {
    let v = value.clamp(0.0, 1.0);
    let col = commands
        .spawn((
            Node {
                width: Val::Px(24.0),
                height: Val::Px(120.0),
                position_type: PositionType::Relative,
                ..default()
            },
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::NsResize),
            Name::new("fader"),
        ))
        .id();
    let track = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(9.0),
                width: Val::Px(6.0),
                height: Val::Percent(100.0),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb((55, 55, 66))),
            bevy::ui::FocusPolicy::Pass,
            Name::new("fader-track"),
        ))
        .id();
    let fill = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(9.0),
                bottom: Val::Px(0.0),
                width: Val::Px(6.0),
                height: Val::Percent(v * 100.0),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(ACCENT_BLUE)),
            bevy::ui::FocusPolicy::Pass,
            Name::new("fader-fill"),
        ))
        .id();
    let thumb = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(3.0),
                bottom: Val::Percent(v * 100.0),
                margin: UiRect::bottom(Val::Px(-5.0)),
                width: Val::Px(18.0),
                height: Val::Px(10.0),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb((240, 240, 245))),
            bevy::ui::FocusPolicy::Pass,
            Name::new("fader-thumb"),
        ))
        .id();
    commands.entity(col).add_children(&[track, fill, thumb]);
    commands.entity(col).insert(EmberFader { value: v, fill, thumb });
    col
}

fn fader_drag(
    mut faders: Query<(&Interaction, &bevy::ui::RelativeCursorPosition, &mut EmberFader)>,
    mut nodes: Query<&mut Node>,
) {
    for (interaction, rcp, mut f) in &mut faders {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(n) = rcp.normalized else {
            continue;
        };
        let v = (0.5 - n.y).clamp(0.0, 1.0);
        if (v - f.value).abs() < 0.001 {
            continue;
        }
        f.value = v;
        if let Ok(mut node) = nodes.get_mut(f.fill) {
            node.height = Val::Percent(v * 100.0);
        }
        if let Ok(mut node) = nodes.get_mut(f.thumb) {
            node.bottom = Val::Percent(v * 100.0);
        }
    }
}

/// A 2D XY pad (drag the handle; `x`/`y` are 0..1, y up).
#[derive(Component)]
struct EmberXyPad {
    handle: Entity,
}

pub fn xy_pad(commands: &mut Commands, x: f32, y: f32) -> Entity {
    let px = x.clamp(0.0, 1.0);
    let py = y.clamp(0.0, 1.0);
    let pad = commands
        .spawn((
            Node {
                width: Val::Px(120.0),
                height: Val::Px(120.0),
                position_type: PositionType::Relative,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb((30, 30, 38))),
            BorderColor::all(rgb((60, 60, 72))),
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Move),
            Name::new("xy-pad"),
        ))
        .id();
    let handle = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(px * 100.0),
                top: Val::Percent((1.0 - py) * 100.0),
                margin: UiRect::new(Val::Px(-6.0), Val::Px(0.0), Val::Px(-6.0), Val::Px(0.0)),
                width: Val::Px(12.0),
                height: Val::Px(12.0),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(ACCENT_BLUE)),
            bevy::ui::FocusPolicy::Pass,
            Name::new("xy-handle"),
        ))
        .id();
    commands.entity(pad).add_child(handle);
    commands.entity(pad).insert(EmberXyPad { handle });
    pad
}

fn xy_pad_drag(
    pads: Query<(&Interaction, &bevy::ui::RelativeCursorPosition, &EmberXyPad)>,
    mut nodes: Query<&mut Node>,
) {
    for (interaction, rcp, pad) in &pads {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(n) = rcp.normalized else {
            continue;
        };
        let nx = (n.x + 0.5).clamp(0.0, 1.0);
        let ny = (n.y + 0.5).clamp(0.0, 1.0);
        if let Ok(mut node) = nodes.get_mut(pad.handle) {
            node.left = Val::Percent(nx * 100.0);
            node.top = Val::Percent(ny * 100.0);
        }
    }
}

// ── Gallery ──────────────────────────────────────────────────────────────────

/// A titled panel column — the shell of each gallery category panel.
fn panel_column(
    commands: &mut Commands,
    font: &Handle<Font>,
    title: &str,
    rows: Vec<Entity>,
) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexStart,
                padding: UiRect::all(Val::Px(16.0)),
                row_gap: Val::Px(12.0),
                overflow: Overflow::clip(),
                ..default()
            },
            Name::new("gallery-panel"),
        ))
        .id();
    let heading = commands
        .spawn((
            Text::new(title),
            ui_font(font, 15.0),
            TextColor(rgb(TEXT_PRIMARY)),
        ))
        .id();
    commands.entity(root).add_child(heading);
    commands.entity(root).add_children(&rows);
    root
}

/// Gallery panel: buttons & toggles.
pub fn gallery_buttons(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;
    let btns = [
        button(commands, font, "Primary"),
        button(commands, font, "Secondary"),
        button(commands, font, "Cancel"),
    ];
    let buttons = hstack(commands, 8.0, &btns);
    let f_buttons = field(commands, font, "Buttons", buttons);

    let togs = [toggle(commands, true), toggle(commands, false)];
    let toggles = hstack(commands, 10.0, &togs);
    let f_toggle = field(commands, font, "Toggles", toggles);

    panel_column(commands, font, "Buttons & Toggles", vec![f_buttons, f_toggle])
}

/// Gallery panel: text / numeric / list inputs.
pub fn gallery_inputs(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;
    let ti = text_input(commands, font, "Type here…", "");
    let f_text = field(commands, font, "Text", ti);

    let dd = dropdown(commands, fonts, &["Forward", "Deferred", "Mobile"], 0);
    let f_dropdown = field(commands, font, "Dropdown", dd);

    let sld = slider(commands, 0.6);
    let f_slider = field(commands, font, "Slider", sld);

    let step = number_stepper(commands, font, 12.0, 1.0);
    let f_step = field(commands, font, "Stepper", step);

    panel_column(
        commands,
        font,
        "Inputs",
        vec![f_text, f_dropdown, f_slider, f_step],
    )
}

/// Gallery panel: selection controls.
pub fn gallery_selection(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;
    let cbs = [checkbox(commands, true), checkbox(commands, false)];
    let checks = hstack(commands, 10.0, &cbs);
    let f_check = field(commands, font, "Checkbox", checks);

    let radios = radio_group(commands, font, &["A", "B", "C"], 0);
    let f_radio = field(commands, font, "Radio", radios);

    let seg = segmented(commands, font, &["One", "Two", "Three"], 1);
    let f_seg = field(commands, font, "Segmented", seg);

    panel_column(commands, font, "Selection", vec![f_check, f_radio, f_seg])
}

/// Gallery panel: typography scale.
pub fn gallery_typography(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;
    let rows = vec![
        h1(commands, font, "Heading 1"),
        h2(commands, font, "Heading 2"),
        h3(commands, font, "Heading 3"),
        h4(commands, font, "Heading 4"),
        paragraph(commands, font, "Body paragraph in the UI font."),
        caption(commands, font, "Caption — small and muted."),
        link(commands, font, "A hyperlink"),
        code(commands, font, "inline_code()"),
    ];
    panel_column(commands, font, "Typography", rows)
}

/// Gallery panel: feedback components.
pub fn gallery_feedback(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;
    let badges = [
        badge(commands, font, "Info", Tone::Info),
        badge(commands, font, "OK", Tone::Success),
        badge(commands, font, "Warn", Tone::Warn),
        badge(commands, font, "Error", Tone::Error),
    ];
    let badge_row = hstack(commands, 6.0, &badges);
    let f_badge = field(commands, font, "Badge", badge_row);

    let al = alert(
        commands,
        fonts,
        Tone::Info,
        "Heads up",
        "This is an inline alert message.",
    );
    let to = toast(commands, fonts, Tone::Success, "Saved successfully");

    let pr = progress(commands, 0.7);
    let f_prog = field(commands, font, "Progress", pr);

    let sk = skeleton(commands, 180.0, 12.0);
    let f_skel = field(commands, font, "Skeleton", sk);

    panel_column(
        commands,
        font,
        "Feedback",
        vec![f_badge, al, to, f_prog, f_skel],
    )
}

/// Gallery panel: inspector value editors.
pub fn gallery_inspector(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;

    let pos = vec3_edit(commands, font, 0.0, 1.0, 0.0);
    let r_pos = property_row(commands, font, "Position", pos);

    let dv = drag_value(commands, font, "", TEXT_MUTED, 1.0, 0.05);
    let r_scale = property_row(commands, font, "Scale", dv);

    let cp = color_picker(commands, (80, 140, 255));
    let r_color = property_row(commands, font, "Color", cp);

    let knobs = [knob(commands, 0.3), knob(commands, 0.7)];
    let knob_row = hstack(commands, 12.0, &knobs);
    let r_knob = property_row(commands, font, "Knobs", knob_row);

    let pads = [fader(commands, 0.6), xy_pad(commands, 0.5, 0.5)];
    let pad_row = hstack(commands, 16.0, &pads);
    let r_pads = property_row(commands, font, "Fader / XY", pad_row);

    panel_column(
        commands,
        font,
        "Inspector",
        vec![r_pos, r_scale, r_color, r_knob, r_pads],
    )
}

/// Gallery panel: color swatches.
pub fn gallery_colors(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;
    let chips = [
        swatch(commands, ACCENT_BLUE),
        swatch(commands, PLAY_GREEN),
        swatch(commands, CLOSE_RED),
        swatch(commands, TAB_HOVER_BG),
        swatch(commands, HEADER_BG),
    ];
    let swatches = hstack(commands, 8.0, &chips);
    let f_swatch = field(commands, font, "Swatches", swatches);
    panel_column(commands, font, "Colors", vec![f_swatch])
}
