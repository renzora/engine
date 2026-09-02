//! The Input page — the project's own input actions and their bindings.
//!
//! Unlike every other tab this one is *structural*: adding an action, expanding
//! a row or entering listen mode changes what the page contains, not just a
//! value. Those handlers therefore set `OverlayState.dirty` (via [`mark_dirty`])
//! to force a rebuild, rather than relying on a two-way binding.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use bevy::window::SystemCursorIcon;

use renzora::CurrentProject;
use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::theme::*;
use renzora_ember::widgets::{bind_text_input, section, text_input};
use renzora_input::{ActionKind, InputAction, InputBinding, InputMap};

use crate::lang::tr;
use crate::rows::{ctl_drag, hrow, note_row, settings_row, text_button};
use crate::state::{InputTabData, InputUi, OverlayState, A_BLUE, A_GREEN, A_PURPLE};
use crate::tabs::shortcuts::is_modifier_key;

#[derive(Component)]
pub(crate) struct AddActionBtn {
    axis: bool,
}
#[derive(Component)]
pub(crate) struct DeleteActionBtn(usize);
#[derive(Component)]
pub(crate) struct ExpandActionBtn(usize);
#[derive(Component)]
pub(crate) struct AddBindingBtn(usize);
#[derive(Component)]
pub(crate) struct CancelListenBtn;
#[derive(Component)]
pub(crate) struct RemoveBindingBtn {
    action: usize,
    binding: usize,
}
/// Add a WASD/Arrows composite to an Axis2D action.
#[derive(Component)]
pub(crate) struct CompositeBtn {
    action: usize,
    arrows: bool,
}
#[derive(Component)]
pub(crate) struct NewActionInput;

fn kind_label(k: ActionKind) -> String {
    tr(match k {
        ActionKind::Button => "settings.opt.button",
        ActionKind::Axis1D => "settings.opt.axis1d",
        ActionKind::Axis2D => "settings.opt.axis2d",
    })
}

fn format_binding(b: &InputBinding) -> String {
    match b {
        InputBinding::Key(s) => s.clone(),
        InputBinding::MouseButton(s) => format!("Mouse {s}"),
        InputBinding::GamepadButton(s) => format!("Pad {s}"),
        InputBinding::GamepadAxis(s) => format!("Axis {s}"),
        InputBinding::Composite2D {
            up,
            down,
            left,
            right,
        } => format!("{up} {left} {down} {right}"),
    }
}

pub(crate) fn tab_input(
    commands: &mut Commands,
    fonts: &EmberFonts,
    col: Entity,
    input: &InputTabData,
) {
    // About.
    let (sec, body) = section(commands, fonts, "info", &tr("settings.section.about_input"), A_BLUE);
    commands.entity(col).add_child(sec);
    note_row(commands, fonts, body, &tr("settings.hint.input_actions"));

    // Add Action.
    let (sec, body) = section(commands, fonts, "list-plus", &tr("settings.section.add_action"), A_GREEN);
    commands.entity(col).add_child(sec);
    let ti = text_input(commands, &fonts.ui, &tr("settings.input.action_name_placeholder"), "");
    commands.entity(ti).insert(NewActionInput);
    bind_text_input(
        commands,
        ti,
        |w| w.resource::<InputUi>().new_name.clone(),
        |w, s| w.resource_mut::<InputUi>().new_name = s,
    );
    let btn_b = text_button(commands, fonts, &tr("settings.opt.button"), AddActionBtn { axis: false });
    let btn_a = text_button(commands, fonts, &tr("settings.opt.axis2d"), AddActionBtn { axis: true });
    let row = hrow(commands, &[ti, btn_b, btn_a]);
    commands.entity(body).add_child(row);

    // Input Actions list.
    let (sec, body) = section(commands, fonts, "game-controller", &tr("settings.section.input_actions"), A_PURPLE);
    commands.entity(col).add_child(sec);
    for (i, action) in input.actions.iter().enumerate() {
        let expanded = input.selected == Some(i);
        build_action_row(commands, fonts, body, i, action, expanded, input.listening);
    }
}

fn build_action_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    body: Entity,
    i: usize,
    action: &InputAction,
    expanded: bool,
    listening: bool,
) {
    // Header row: caret + name + kind + delete.
    let caret = icon_text(
        commands,
        &fonts.phosphor,
        if expanded { "caret-down" } else { "caret-right" },
        text_muted(),
        12.0,
    );
    commands
        .entity(caret)
        .insert((Interaction::default(), ExpandActionBtn(i), HoverCursor(SystemCursorIcon::Pointer)));
    let name = commands
        .spawn((
            Text::new(action.name.clone()),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
            Node {
                flex_grow: 1.0,
                ..default()
            },
            Interaction::default(),
            ExpandActionBtn(i),
        ))
        .id();
    let kind = commands
        .spawn((
            Text::new(kind_label(action.kind)),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let del = icon_text(commands, &fonts.phosphor, "trash", text_muted(), 13.0);
    commands.entity(del).insert((
        Interaction::default(),
        FocusPolicy::Block,
        DeleteActionBtn(i),
        HoverCursor(SystemCursorIcon::Pointer),
    ));
    let header = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::row_odd())),
        ))
        .id();
    commands.entity(header).add_children(&[caret, name, kind, del]);
    commands.entity(body).add_child(header);

    if !expanded {
        return;
    }

    // Expanded panel.
    let panel = commands
        .spawn((Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            padding: UiRect {
                left: Val::Px(18.0),
                top: Val::Px(2.0),
                bottom: Val::Px(6.0),
                ..default()
            },
            ..default()
        },))
        .id();
    commands.entity(body).add_child(panel);

    if action.kind != ActionKind::Button {
        let dv = ctl_drag(
            commands,
            fonts,
            action.dead_zone,
            0.0,
            0.5,
            0.01,
            move |w| {
                w.resource::<InputMap>()
                    .actions
                    .get(i)
                    .map(|a| a.dead_zone)
                    .unwrap_or(0.0)
            },
            move |w, &v| {
                if let Some(mut m) = w.get_resource_mut::<InputMap>() {
                    if let Some(a) = m.actions.get_mut(i) {
                        a.dead_zone = v;
                    }
                }
                save_input(w);
            },
        );
        settings_row(commands, fonts, panel, 0, &tr("settings.row.dead_zone"), dv);
    }

    // Existing bindings.
    for (j, b) in action.bindings.iter().enumerate() {
        let lbl = commands
            .spawn((
                Text::new(format_binding(b)),
                ui_font(&fonts.ui, 11.0),
                TextColor(rgb(renzora_ember::theme::value_text())),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
            ))
            .id();
        let rm = icon_text(commands, &fonts.phosphor, "trash", text_muted(), 12.0);
        commands.entity(rm).insert((
            Interaction::default(),
            FocusPolicy::Block,
            RemoveBindingBtn { action: i, binding: j },
            HoverCursor(SystemCursorIcon::Pointer),
        ));
        let row = hrow(commands, &[lbl, rm]);
        commands.entity(panel).add_child(row);
    }

    // Add-binding / listen prompt.
    if listening {
        let prompt = commands
            .spawn((
                Text::new(tr("settings.input.press_any")),
                ui_font(&fonts.ui, 11.0),
                TextColor(rgb(renzora_ember::theme::warn_amber())),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
            ))
            .id();
        let cancel = text_button(commands, fonts, &tr("common.cancel"), CancelListenBtn);
        let row = hrow(commands, &[prompt, cancel]);
        commands.entity(panel).add_child(row);
    } else {
        let add = text_button(commands, fonts, &tr("settings.btn.add_binding"), AddBindingBtn(i));
        let mut kids = vec![add];
        if action.kind == ActionKind::Axis2D {
            kids.push(text_button(commands, fonts, "WASD", CompositeBtn { action: i, arrows: false }));
            kids.push(text_button(commands, fonts, &tr("settings.opt.arrows"), CompositeBtn { action: i, arrows: true }));
        }
        let row = hrow(commands, &kids);
        commands.entity(panel).add_child(row);
    }
}

fn save_input(w: &mut World) {
    let (Some(map), Some(project)) = (
        w.get_resource::<InputMap>().cloned(),
        w.get_resource::<CurrentProject>().cloned(),
    ) else {
        return;
    };
    let _ = renzora_input::save_input_map(&map, &project);
}

fn mark_dirty(w: &mut World) {
    if let Some(mut st) = w.get_resource_mut::<OverlayState>() {
        st.dirty = true;
    }
}

pub(crate) fn add_action_click(world: &mut World) {
    let mut to_add: Option<bool> = None;
    let mut q = world.query_filtered::<(&Interaction, &AddActionBtn), Changed<Interaction>>();
    for (interaction, btn) in q.iter(world) {
        if *interaction == Interaction::Pressed {
            to_add = Some(btn.axis);
        }
    }
    let Some(axis) = to_add else { return };
    let name = world.resource::<InputUi>().new_name.trim().to_string();
    if name.is_empty() {
        return;
    }
    if let Some(mut m) = world.get_resource_mut::<InputMap>() {
        let action = if axis {
            InputAction::axis_2d(name, vec![], 0.15)
        } else {
            InputAction::button(name, vec![])
        };
        m.add(action);
    }
    world.resource_mut::<InputUi>().new_name.clear();
    save_input(world);
    mark_dirty(world);
}

pub(crate) fn delete_action_click(world: &mut World) {
    let mut idx = None;
    let mut q = world.query_filtered::<(&Interaction, &DeleteActionBtn), Changed<Interaction>>();
    for (interaction, btn) in q.iter(world) {
        if *interaction == Interaction::Pressed {
            idx = Some(btn.0);
        }
    }
    let Some(i) = idx else { return };
    let name = world
        .get_resource::<InputMap>()
        .and_then(|m| m.actions.get(i).map(|a| a.name.clone()));
    if let (Some(name), Some(mut m)) = (name, world.get_resource_mut::<InputMap>()) {
        m.remove(&name);
    }
    {
        let mut ui = world.resource_mut::<InputUi>();
        if ui.selected == Some(i) {
            ui.selected = None;
        }
    }
    save_input(world);
    mark_dirty(world);
}

pub(crate) fn expand_action_click(
    btns: Query<(&Interaction, &ExpandActionBtn), Changed<Interaction>>,
    mut ui: ResMut<InputUi>,
    mut state: ResMut<OverlayState>,
) {
    for (interaction, btn) in &btns {
        if *interaction == Interaction::Pressed {
            ui.selected = if ui.selected == Some(btn.0) {
                None
            } else {
                Some(btn.0)
            };
            ui.listening = false;
            state.dirty = true;
        }
    }
}

pub(crate) fn add_binding_click(
    btns: Query<(&Interaction, &AddBindingBtn), Changed<Interaction>>,
    mut ui: ResMut<InputUi>,
    mut state: ResMut<OverlayState>,
) {
    for (interaction, btn) in &btns {
        if *interaction == Interaction::Pressed {
            ui.selected = Some(btn.0);
            ui.listening = true;
            state.dirty = true;
        }
    }
}

pub(crate) fn cancel_listen_click(
    btns: Query<&Interaction, (Changed<Interaction>, With<CancelListenBtn>)>,
    mut ui: ResMut<InputUi>,
    mut state: ResMut<OverlayState>,
) {
    for interaction in &btns {
        if *interaction == Interaction::Pressed {
            ui.listening = false;
            state.dirty = true;
        }
    }
}

pub(crate) fn remove_binding_click(world: &mut World) {
    let mut target = None;
    let mut q = world.query_filtered::<(&Interaction, &RemoveBindingBtn), Changed<Interaction>>();
    for (interaction, btn) in q.iter(world) {
        if *interaction == Interaction::Pressed {
            target = Some((btn.action, btn.binding));
        }
    }
    let Some((a, b)) = target else { return };
    if let Some(mut m) = world.get_resource_mut::<InputMap>() {
        if let Some(action) = m.actions.get_mut(a) {
            if b < action.bindings.len() {
                action.bindings.remove(b);
            }
        }
    }
    save_input(world);
    mark_dirty(world);
}

pub(crate) fn composite_click(world: &mut World) {
    let mut target = None;
    let mut q = world.query_filtered::<(&Interaction, &CompositeBtn), Changed<Interaction>>();
    for (interaction, btn) in q.iter(world) {
        if *interaction == Interaction::Pressed {
            target = Some((btn.action, btn.arrows));
        }
    }
    let Some((a, arrows)) = target else { return };
    let binding = if arrows {
        InputBinding::composite_2d(
            KeyCode::ArrowUp,
            KeyCode::ArrowDown,
            KeyCode::ArrowLeft,
            KeyCode::ArrowRight,
        )
    } else {
        InputBinding::composite_2d(KeyCode::KeyW, KeyCode::KeyS, KeyCode::KeyA, KeyCode::KeyD)
    };
    if let Some(mut m) = world.get_resource_mut::<InputMap>() {
        if let Some(action) = m.actions.get_mut(a) {
            action.bindings.push(binding);
        }
    }
    save_input(world);
    mark_dirty(world);
}

/// While the Input tab is in listen mode, capture the next key or mouse button
/// and append it to the selected action's bindings.
pub(crate) fn input_listen_capture(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut ui: ResMut<InputUi>,
    mut map: ResMut<InputMap>,
    mut state: ResMut<OverlayState>,
    project: Option<Res<CurrentProject>>,
) {
    if !ui.listening {
        return;
    }
    let Some(sel) = ui.selected else { return };
    if keys.just_pressed(KeyCode::Escape) {
        ui.listening = false;
        state.dirty = true;
        return;
    }
    let binding = if let Some(k) = keys.get_just_pressed().copied().find(|k| !is_modifier_key(*k)) {
        Some(InputBinding::key(k))
    } else if mouse.just_pressed(MouseButton::Left) {
        Some(InputBinding::mouse(MouseButton::Left))
    } else if mouse.just_pressed(MouseButton::Right) {
        Some(InputBinding::mouse(MouseButton::Right))
    } else if mouse.just_pressed(MouseButton::Middle) {
        Some(InputBinding::mouse(MouseButton::Middle))
    } else {
        None
    };
    let Some(binding) = binding else { return };
    if let Some(action) = map.actions.get_mut(sel) {
        action.bindings.push(binding);
    }
    if let Some(project) = project {
        let _ = renzora_input::save_input_map(&map, &project);
    }
    ui.listening = false;
    state.dirty = true;
}
