//! `renzora_ember` UI components — the start of a reusable bevy_ui widget set
//! used by the editor and games. These are plain entity builders + the systems
//! that animate their interaction states. [`build_gallery`] showcases them.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::{ui_font, EmberFonts};
use crate::theme::{
    rgb, ACCENT_BLUE, CLOSE_RED, HEADER_BG, PLAY_GREEN, TAB_ACTIVE_BG, TAB_HOVER_BG, TEXT_MUTED,
    TEXT_PRIMARY,
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
    mut q: Query<(&Interaction, &mut BackgroundColor), (With<EmberButton>, Changed<Interaction>)>,
) {
    for (interaction, mut bg) in &mut q {
        bg.0 = match interaction {
            Interaction::Pressed => rgb(ACCENT_BLUE),
            Interaction::Hovered => rgb((64, 64, 78)),
            Interaction::None => rgb(TAB_ACTIVE_BG),
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
    mut q: Query<
        (&Interaction, &mut EmberToggle, &mut BackgroundColor, &mut Node),
        Changed<Interaction>,
    >,
) {
    for (interaction, mut tog, mut bg, mut node) in &mut q {
        if *interaction == Interaction::Pressed {
            tog.on = !tog.on;
            bg.0 = if tog.on {
                rgb(ACCENT_BLUE)
            } else {
                rgb((60, 60, 70))
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
    mut boxes: Query<(&Interaction, &mut EmberCheckbox, &mut BackgroundColor), Changed<Interaction>>,
    mut nodes: Query<&mut Node>,
) {
    for (interaction, mut cb, mut bg) in &mut boxes {
        if *interaction != Interaction::Pressed {
            continue;
        }
        cb.checked = !cb.checked;
        bg.0 = if cb.checked {
            rgb(ACCENT_BLUE)
        } else {
            Color::NONE
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
    segments: Query<(Entity, &EmberSegment)>,
    mut backgrounds: Query<&mut BackgroundColor>,
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
    for (entity, s) in &segments {
        if s.group != group {
            continue;
        }
        if let Ok(mut bg) = backgrounds.get_mut(entity) {
            bg.0 = if s.value == value {
                rgb(ACCENT_BLUE)
            } else {
                Color::NONE
            };
        }
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

// ── Gallery ──────────────────────────────────────────────────────────────────

/// A scrollable showcase of the available ember widgets — the editor's first
/// real bevy_ui panel content.
pub fn build_gallery(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let font = &fonts.ui;
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexStart,
                padding: UiRect::all(Val::Px(16.0)),
                row_gap: Val::Px(12.0),
                ..default()
            },
            Name::new("widget-gallery"),
        ))
        .id();

    let title = commands
        .spawn((
            Text::new("renzora_ember — components"),
            ui_font(font, 16.0),
            TextColor(rgb(TEXT_PRIMARY)),
        ))
        .id();

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

    let cbs = [checkbox(commands, true), checkbox(commands, false)];
    let checks = hstack(commands, 10.0, &cbs);
    let f_check = field(commands, font, "Checkbox", checks);

    let radios = radio_group(commands, font, &["A", "B", "C"], 0);
    let f_radio = field(commands, font, "Radio", radios);

    let seg = segmented(commands, font, &["One", "Two", "Three"], 1);
    let f_seg = field(commands, font, "Segmented", seg);

    let sld = slider(commands, 0.6);
    let f_slider = field(commands, font, "Slider", sld);

    let step = number_stepper(commands, font, 12.0, 1.0);
    let f_step = field(commands, font, "Stepper", step);

    let chips = [
        swatch(commands, ACCENT_BLUE),
        swatch(commands, PLAY_GREEN),
        swatch(commands, CLOSE_RED),
        swatch(commands, TAB_HOVER_BG),
        swatch(commands, HEADER_BG),
    ];
    let swatches = hstack(commands, 8.0, &chips);
    let f_swatch = field(commands, font, "Swatches", swatches);

    commands.entity(root).add_children(&[
        title, f_buttons, f_toggle, f_check, f_radio, f_seg, f_slider, f_step, f_swatch,
    ]);
    root
}
