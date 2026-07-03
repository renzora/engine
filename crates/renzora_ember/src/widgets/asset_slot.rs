//! Drag-and-drop + asset slot — drag a "file" (e.g. from an assets panel) onto a
//! slot field, like an inspector's texture/mesh reference.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;
use bevy::window::SystemCursorIcon;

use crate::font::{icon_text, ui_font, EmberFonts};
use crate::theme::*;

const DRAG_THRESHOLD: f32 = 5.0;

/// A draggable source carrying a string payload.
#[derive(Component)]
pub(crate) struct Draggable {
    payload: String,
}

/// A drop target that shows the dropped payload's name.
#[derive(Component)]
pub(crate) struct AssetSlot {
    label: Entity,
}

#[derive(Resource, Default)]
pub(crate) struct Dnd {
    payload: Option<String>,
    ghost: Option<Entity>,
    started: bool,
    start: Vec2,
}

/// Registers the drag-and-drop resource + system.
pub(crate) struct DndPlugin;

impl Plugin for DndPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Dnd>().add_systems(Update, dnd_system);
    }
}

/// A draggable "file" chip (drag it onto an [`asset_slot`]).
pub fn draggable_file(commands: &mut Commands, fonts: &EmberFonts, name: &str) -> Entity {
    let chip = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
            Interaction::default(),
            Draggable {
                payload: name.to_string(),
            },
            crate::cursor_icon::HoverCursor(SystemCursorIcon::Grab),
            Name::new("draggable-file"),
        ))
        .id();
    let icon = icon_text(commands, &fonts.phosphor, "file", text_muted(), 13.0);
    let label = commands
        .spawn((
            Text::new(name),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    commands.entity(chip).add_children(&[icon, label]);
    chip
}

/// An asset reference slot — a drop target showing the current value.
pub fn asset_slot(commands: &mut Commands, fonts: &EmberFonts, value: &str) -> Entity {
    let empty = value.is_empty();
    let slot = commands
        .spawn((
            Node {
                min_width: Val::Px(170.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            RelativeCursorPosition::default(),
            Name::new("asset-slot"),
        ))
        .id();
    let icon = icon_text(commands, &fonts.phosphor, "image", text_muted(), 14.0);
    let label = commands
        .spawn((
            Text::new(if empty { "(drop a file)" } else { value }),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(if empty { text_muted() } else { text_primary() })),
        ))
        .id();
    commands.entity(slot).insert(AssetSlot { label });
    commands.entity(slot).add_children(&[icon, label]);
    slot
}

pub(crate) fn dnd_system(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    mut dnd: ResMut<Dnd>,
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    draggables: Query<(&Interaction, &Draggable)>,
    targets: Query<(&RelativeCursorPosition, &AssetSlot)>,
    mut nodes: Query<&mut Node>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
) {
    let cursor = windows.iter().find_map(|w| w.cursor_position());

    // Begin a potential drag.
    if dnd.payload.is_none() && mouse.just_pressed(MouseButton::Left) {
        if let Some(c) = cursor {
            for (interaction, d) in &draggables {
                if *interaction == Interaction::Pressed {
                    dnd.payload = Some(d.payload.clone());
                    dnd.start = c;
                    dnd.started = false;
                    break;
                }
            }
        }
    }

    // Release → drop or cancel.
    if mouse.just_released(MouseButton::Left) {
        let payload = dnd.payload.take();
        if let Some(ghost) = dnd.ghost.take() {
            commands.entity(ghost).despawn();
        }
        if dnd.started {
            if let (Some(payload), Some(_)) = (payload, cursor) {
                for (rcp, slot) in &targets {
                    if rcp.cursor_over {
                        if let Ok((mut t, mut c)) = texts.get_mut(slot.label) {
                            *t = Text::new(payload.clone());
                            c.0 = rgb(text_primary());
                        }
                        break;
                    }
                }
            }
        }
        dnd.started = false;
        return;
    }

    let Some(c) = cursor else {
        return;
    };
    if dnd.payload.is_none() {
        return;
    }
    // Pass the threshold → spawn the ghost.
    if !dnd.started {
        if c.distance(dnd.start) <= DRAG_THRESHOLD {
            return;
        }
        dnd.started = true;
        if let Some(fonts) = &fonts {
            let payload = dnd.payload.clone().unwrap_or_default();
            let ghost = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(c.x + 12.0),
                        top: Val::Px(c.y + 12.0),
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(rgb(accent()).with_alpha(0.9)),
                    GlobalZIndex(2000),
                    bevy::ui::FocusPolicy::Pass,
                    Name::new("dnd-ghost"),
                ))
                .with_children(|p| {
                    p.spawn((
                        Text::new(payload),
                        ui_font(&fonts.ui, 11.0),
                        TextColor(rgb((255, 255, 255))),
                    ));
                })
                .id();
            dnd.ghost = Some(ghost);
        }
    }
    if let Some(ghost) = dnd.ghost {
        if let Ok(mut n) = nodes.get_mut(ghost) {
            n.left = Val::Px(c.x + 12.0);
            n.top = Val::Px(c.y + 12.0);
        }
    }
}
