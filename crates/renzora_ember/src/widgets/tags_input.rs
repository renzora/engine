//! Tags input — a wrapping row of removable chips plus a text field; press Enter
//! to commit a tag.

use bevy::prelude::*;

use crate::font::EmberFonts;
use crate::theme::*;

use super::chip::chip;
use super::text_input::{text_input, EmberTextInput};

#[derive(Component)]
pub(crate) struct TagsInput {
    field: Entity,
    root: Entity,
}

/// A tags input pre-filled with `initial` tags.
pub fn tags_input(commands: &mut Commands, fonts: &EmberFonts, initial: &[&str]) -> Entity {
    let root = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                row_gap: Val::Px(4.0),
                min_width: Val::Px(220.0),
                padding: UiRect::all(Val::Px(5.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb((28, 28, 34))),
            BorderColor::all(rgb((70, 70, 82))),
            Name::new("tags-input"),
        ))
        .id();
    let mut kids: Vec<Entity> = initial.iter().map(|t| chip(commands, fonts, t)).collect();
    let field = text_input(commands, &fonts.ui, "add tag…", "");
    commands.entity(field).insert(TagsInput { field, root });
    kids.push(field);
    commands.entity(root).add_children(&kids);
    root
}

pub(crate) fn tags_commit(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    tags: Query<&TagsInput>,
    mut inputs: Query<&mut EmberTextInput>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
    children: Query<&Children>,
) {
    let Some(fonts) = fonts else {
        return;
    };
    for ti in &tags {
        let Ok(mut inp) = inputs.get_mut(ti.field) else {
            continue;
        };
        if !inp.value.contains('\n') {
            continue;
        }
        let tag = inp.value.split('\n').next().unwrap_or("").trim().to_string();
        let (text_e, ph) = (inp.text_entity, inp.placeholder.clone());
        inp.value.clear();
        if let Ok((mut t, mut c)) = texts.get_mut(text_e) {
            *t = Text::new(ph);
            c.0 = rgb(text_muted());
        }
        if tag.is_empty() {
            continue;
        }
        let chip_e = chip(&mut commands, &fonts, &tag);
        if let Ok(kids) = children.get(ti.root) {
            let idx = kids.iter().position(|c| c == ti.field).unwrap_or(kids.len());
            commands.entity(ti.root).insert_children(idx, &[chip_e]);
        } else {
            commands.entity(ti.root).add_child(chip_e);
        }
    }
}
