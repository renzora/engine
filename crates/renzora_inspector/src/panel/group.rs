//! Foldable field groups inside a section (`FieldType::Group`): a header row
//! with a caret and the group's name and, for a feature, its on/off switch,
//! over its fields.
//!
//! The rows are the card's own rows, not children of a box under the header:
//! a group adds them to the section body right after its header and tags each
//! with [`InGroup`]. Inside a box of their own they were siblings only of each
//! other, so `fit_label_columns` sized their label column on its own and their
//! controls and reset buttons sat out of line with the rest of the card.
//!
//! A folded group builds **nothing**, for the reason a folded section builds
//! nothing (see [`super::section`]): `bevy_ui` lays out a hidden subtree every
//! frame regardless, so hiding rows would save the scrolling and none of the
//! cost. Unfolding builds the rows, folding throws them away, and the recipe
//! for building them rides on the header.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use renzora_ember::font::{icon_glyph, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::bind_bg;
use renzora_ember::theme::{hover_bg, rgb, text_primary};
use renzora_ember::UiInteraction;

use super::fields::{build_field_row, build_field_row_with_label};
use super::spec::{field_label_loc, FieldKind, FieldSpec};

/// Which groups are unfolded, per component type and group name.
///
/// Per type, like [`super::InspectorSectionsOpen`]: opening the Sun's Contact
/// Shadows on one scene's sun should leave it open on the next. Groups start
/// folded, since the point of one is a shorter card.
#[derive(Resource, Default)]
pub(crate) struct InspectorGroupsOpen(std::collections::HashMap<(&'static str, &'static str), bool>);

impl InspectorGroupsOpen {
    pub(crate) fn is_open(&self, type_id: &'static str, name: &'static str) -> bool {
        self.0.get(&(type_id, name)).copied().unwrap_or(false)
    }

    fn set(&mut self, type_id: &'static str, name: &'static str, open: bool) {
        self.0.insert((type_id, name), open);
    }
}

/// On a group's header row: how to fill it, and the label whose caret flips.
#[derive(Component)]
pub(super) struct GroupHeader {
    label: Entity,
    open: bool,
    type_id: &'static str,
    name: &'static str,
    entity: Entity,
    fields: Vec<FieldSpec>,
}

/// On a row a group added: the group's header. Folding despawns every row
/// that leads back to the header, through nested groups too.
#[derive(Component)]
pub(super) struct InGroup(Entity);

/// Build `fields` as rows, a header (and, if unfolded, its rows) for each
/// group. Returned in order and unparented, for the caller to place: a
/// section adds them to its body, an unfolding group splices them in after
/// its header. `owner` is the group they belong to, if any.
pub(super) fn build_rows(
    commands: &mut Commands,
    fonts: &EmberFonts,
    fields: &[FieldSpec],
    entity: Entity,
    type_id: &'static str,
    owner: Option<Entity>,
) -> Vec<Entity> {
    let mut out = Vec::new();
    for field in fields {
        let row = match field.kind {
            FieldKind::Group { switch, open } => {
                let header = build_header(commands, fonts, field, entity, type_id, switch, open);
                out.push(header);
                if open {
                    out.extend(build_rows(commands, fonts, &field.children, entity, type_id, Some(header)));
                }
                header
            }
            _ => {
                let row = build_field_row(commands, fonts, field, entity, type_id);
                out.push(row);
                row
            }
        };
        if let Some(owner) = owner {
            commands.entity(row).insert(InGroup(owner));
        }
    }
    out
}

/// Bring every group's open flag in `fields` up to date with what the user has
/// since folded or unfolded. A section's field list is collected once and
/// reused when its body is rebuilt (scrolled back on screen), so without this
/// a group opened after the section was built would come back folded.
pub(super) fn refresh_open(fields: &mut [FieldSpec], groups: &InspectorGroupsOpen, type_id: &'static str) {
    for field in fields {
        if let FieldKind::Group { ref mut open, .. } = field.kind {
            *open = groups.is_open(type_id, field.name);
            refresh_open(&mut field.children, groups, type_id);
        }
    }
}

fn caret(open: bool) -> String {
    icon_glyph(if open { "caret-down" } else { "caret-right" })
        .map(|g| g.to_string())
        .unwrap_or_default()
}

/// A group's header: a row like any other, so its switch and reset button sit
/// in the card's columns, with the caret in front of the name in the label.
/// A switch group's header is its `Bool` row; a plain group's has no control.
fn build_header(
    commands: &mut Commands,
    fonts: &EmberFonts,
    field: &FieldSpec,
    entity: Entity,
    type_id: &'static str,
    switch: bool,
    open: bool,
) -> Entity {
    // The caret is the label's first span, so the label column is fitted to
    // the caret and the name together and clips neither.
    let label = commands
        .spawn((
            Text::new(caret(open)),
            ui_font(&bevy::text::FontSource::Handle(fonts.phosphor.clone()), 9.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let name = commands
        .spawn((
            TextSpan::new(format!("  {}", field_label_loc(field.name))),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    commands.entity(label).add_child(name);

    let mut as_row = field.clone();
    if switch {
        as_row.kind = FieldKind::Bool;
    }
    let row = build_field_row_with_label(commands, fonts, &as_row, entity, type_id, Some(label));
    commands.entity(row).insert((
        BackgroundColor(Color::NONE),
        UiInteraction::default(),
        renzora_ember::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
        GroupHeader {
            label,
            open,
            type_id,
            name: field.name,
            entity,
            fields: field.children.clone(),
        },
        Name::new("inspector-group-header"),
    ));
    commands.entity(row).entry::<Node>().and_modify(|mut n| {
        n.border_radius = BorderRadius::all(Val::Px(4.0));
    });
    bind_bg(commands, row, move |w| match w.get::<UiInteraction>(row) {
        Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(hover_bg()),
        _ => Color::NONE,
    });
    row
}

/// Fold or unfold a group on a press of its header: splice its rows in after
/// the header or throw them away, and remember the choice for this component
/// type.
#[allow(clippy::too_many_arguments)]
pub(super) fn group_header_click(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    mut groups: ResMut<InspectorGroupsOpen>,
    mut headers: Query<(Entity, &UiInteraction, &mut GroupHeader), Changed<UiInteraction>>,
    members: Query<(Entity, &InGroup)>,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    mut texts: Query<&mut Text>,
) {
    let Some(fonts) = fonts else { return };
    for (header_entity, interaction, mut header) in &mut headers {
        if *interaction != UiInteraction::Pressed {
            continue;
        }
        header.open = !header.open;
        groups.set(header.type_id, header.name, header.open);
        if let Ok(mut text) = texts.get_mut(header.label) {
            text.0 = caret(header.open);
        }

        if !header.open {
            for row in rows_of(header_entity, &members) {
                commands.entity(row).try_despawn();
            }
            continue;
        }

        let Ok(parent) = parents.get(header_entity).map(ChildOf::parent) else {
            continue;
        };
        let mut fields = header.fields.clone();
        refresh_open(&mut fields, &groups, header.type_id);
        let rows = build_rows(
            &mut commands,
            &fonts,
            &fields,
            header.entity,
            header.type_id,
            Some(header_entity),
        );
        // The whole order at once: `insert_children` at an index has panicked
        // in Bevy when moving an existing child to the end of the list.
        let mut ordered: Vec<Entity> = Vec::new();
        for kid in children.get(parent).map(|c| c.iter().collect::<Vec<_>>()).unwrap_or_default() {
            ordered.push(kid);
            if kid == header_entity {
                ordered.extend(rows.iter().copied());
            }
        }
        commands.entity(parent).replace_children(&ordered);
    }
}

/// Every row under `header`, through nested groups.
fn rows_of(header: Entity, members: &Query<(Entity, &InGroup)>) -> Vec<Entity> {
    let mut owners = vec![header];
    let mut rows = Vec::new();
    while let Some(owner) = owners.pop() {
        for (row, in_group) in members {
            if in_group.0 == owner {
                rows.push(row);
                owners.push(row);
            }
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_start_folded_and_remember_per_type() {
        let mut g = InspectorGroupsOpen::default();
        assert!(!g.is_open("sun", "Contact Shadows"));
        g.set("sun", "Contact Shadows", true);
        assert!(g.is_open("sun", "Contact Shadows"));
        assert!(!g.is_open("spot_light", "Contact Shadows"));
    }

    /// Folding a group takes its rows and those of any group inside it, and
    /// nothing else.
    #[test]
    fn folding_finds_rows_through_nested_groups() {
        let mut world = World::new();
        let outer = world.spawn_empty().id();
        let a = world.spawn(InGroup(outer)).id();
        let inner = world.spawn(InGroup(outer)).id();
        let b = world.spawn(InGroup(inner)).id();
        let elsewhere = world.spawn_empty().id();
        let not_ours = world.spawn(InGroup(elsewhere)).id();
        let mut state: bevy::ecs::system::SystemState<Query<(Entity, &InGroup)>> =
            bevy::ecs::system::SystemState::new(&mut world);
        let members = state.get(&world).unwrap();
        let mut rows = rows_of(outer, &members);
        rows.sort();
        let mut want = vec![a, inner, b];
        want.sort();
        assert_eq!(rows, want);
        assert!(!rows.contains(&not_ours));
    }
}
