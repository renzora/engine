//! **Project Components**: what a Bevy project's own code put on this entity,
//! and where that code lives.
//!
//! In a Renzora project the relationship between an entity and its behaviour is
//! visible: you attach a script, and the Scripts section shows it. In a Bevy
//! project the behaviour is a `Component` declared in a file and queried by
//! systems somewhere else, and none of that was reachable from the editor: the
//! hierarchy showed `Vehicle` as a *name* and stopped there.
//!
//! This section closes that, for the components the rest of the inspector
//! cannot: it lists what the project put on the selected entity and no editable
//! section exists for, and clicking a row opens the file that declares it at the
//! line it is declared on.
//!
//! # What it deliberately no longer holds
//!
//! It listed **every** project component to begin with, because it was the only
//! place any of them appeared. Then reflected types started getting sections of
//! their own, with their fields and their own jump to source, and every row here
//! for one of those became the same thing said twice with less in it. Clicking a
//! row showed you code you could already have reached, which is exactly how it
//! read: a list that only ever led away from the editor.
//!
//! So a component with a section is [`has_own_section`]'s to show, and what is
//! left here is what has none. Two kinds, and both are worth a row:
//!
//! * a **marker** such as `Dead`, `PlayerDriven` or `InteriorRoot`. It has no fields, so
//!   it gets no section, and "this entity is `Dead`" appears nowhere else in the
//!   inspector at all;
//! * a component whose type **never reached the type registry**, because it does
//!   not derive `Reflect`. Its name and its file are all the engine can say
//!   about it, and they are still worth saying.
//!
//! # Why this needs no `Reflect`
//!
//! It shows **identity**, not data. The engine already knows which `ComponentId`
//! belongs to which of the project's types, because the generated crate root
//! registers them (see `renzora::core::bevy_project::ProjectComponentLabels`),
//! the same table the hierarchy names entities from. Listing what an
//! entity holds, and where the type was written, needs nothing more than that.
//!
//! Reading or writing a component's **fields** does need
//! `#[derive(Reflect)] #[reflect(Component)]`, because without reflection
//! nothing in the engine can interpret the bytes. That is what promotes a
//! component out of this list and into a section of its own.

use bevy::ecs::component::ComponentId;
use bevy::ecs::reflect::ReflectComponent;
use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;
use bevy::reflect::TypeInfo;
use bevy::ui::FocusPolicy;
use bevy::window::SystemCursorIcon;

use renzora::core::bevy_project::ProjectComponentLabels;
use renzora::core::{CurrentProject, OpenCodeEditorFile};
use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::theme::{rgb, section_bg, text_muted, text_primary};

/// The `type_id` slug this section is registered under.
///
/// Not a real Rust type path: like `script_component`, this is an *inherent*
/// section rather than a drawer for one component type.
pub(crate) const TYPE_ID: &str = "project_components";

/// A row, carrying where to go when it is clicked.
#[derive(Component, Clone)]
struct ProjectComponentRow {
    file: std::path::PathBuf,
    line: u32,
}

pub(crate) fn register(app: &mut App) {
    use renzora::{AppEditorExt, InspectorEntry};

    app.register_native_inspector_ui(TYPE_ID, drawer);
    app.register_inspector(InspectorEntry {
        type_id: TYPE_ID,
        display_name: "Project Components",
        icon: "file-code",
        category: "Scripting",
        has_fn: has_project_components,
        // Nothing to add, remove or disable: the list reflects what the
        // project's code put there, and the editor does not get to overrule it.
        add_fn: None,
        remove_fn: None,
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: Vec::new(),
    });
    app.add_systems(
        Update,
        open_declaration_click
            .run_if(in_state(renzora::SplashState::Editor))
            .run_if(renzora_ember::dock::panel_active("inspector")),
    );
}

/// Does this entity hold anything the project declared?
///
/// Cheap: the table is empty in every build with no Bevy project open, so the
/// first check short-circuits for the overwhelmingly common case.
fn has_project_components(world: &World, entity: Entity) -> bool {
    let Some(labels) = world.get_resource::<ProjectComponentLabels>() else {
        return false;
    };
    if labels.is_empty() {
        return false;
    }
    world
        .get_entity(entity)
        .map(|e| {
            e.archetype()
                .components()
                .iter()
                .any(|id| labels.entry(*id).is_some() && !has_own_section(world, *id))
        })
        .unwrap_or(false)
}

/// Does this component already get an inspector section of its own?
///
/// A reflected component with fields gets a generated section (see
/// `panel::collect::append_reflected_sections`) carrying its name, its editable
/// fields and the jump to its declaration. Listing it here as well would say
/// the same thing twice, with less in it, which is what made this section feel
/// like a dead end: every row it held was either already above it or opened
/// nothing but the file.
///
/// So what is left here is what has no section to be: a marker component, and
/// one whose type never reached the type registry. Both are worth a row,
/// because "this entity is `Dead`" is real information and there is nowhere
/// else in the inspector it appears.
///
/// Approximated by "registered, and declares at least one field" rather than by
/// running the section builder. The exact test walks every field looking for one
/// that is editable, which is far too much for a predicate the collector calls
/// per registry entry per rebuild. The two disagree only for a type whose every
/// field is opaque to reflection, and there the row stays here while a section
/// also exists: a duplicate, never a disappearance.
fn has_own_section(world: &World, id: ComponentId) -> bool {
    let Some(registry) = world.get_resource::<AppTypeRegistry>() else {
        return false;
    };
    let Some(type_id) = world.components().get_info(id).and_then(|i| i.type_id()) else {
        return false;
    };
    let registry = registry.read();
    let Some(registration) = registry.get(type_id) else {
        return false;
    };
    if registration.data::<ReflectComponent>().is_none() {
        return false;
    }
    match registration.type_info() {
        TypeInfo::Struct(info) => info.field_len() > 0,
        TypeInfo::TupleStruct(info) => info.field_len() > 0,
        // An enum is always worth a section: even a fieldless one is a choice
        // between variants, which is an edit.
        TypeInfo::Enum(_) => true,
        _ => false,
    }
}

/// Build the rows.
///
/// Built once, here, rather than reconciled by a separate system like the
/// Scripts section: which components an entity holds does not change while it is
/// selected, and the inspector already rebuilds the whole drawer when the
/// selection or the archetype does.
fn drawer(world: &mut World, entity: Entity) -> Entity {
    let fonts = world.get_resource::<EmberFonts>().cloned();
    let project_root = world.get_resource::<CurrentProject>().map(|p| p.path.clone());

    // Read everything first. The spawn below needs `&mut World`, and holding a
    // borrow of a resource across it is not a thing.
    let mut rows: Vec<(String, std::path::PathBuf, u32)> = Vec::new();
    if let Some(labels) = world.get_resource::<ProjectComponentLabels>() {
        if let Ok(entity_ref) = world.get_entity(entity) {
            for id in entity_ref.archetype().components() {
                if let Some(component) = labels.entry(*id) {
                    if has_own_section(world, *id) {
                        continue;
                    }
                    rows.push((component.name.clone(), component.file.clone(), component.line));
                }
            }
        }
    }
    // Stable order. Archetype order is insertion order, which differs between
    // two entities of the same kind and would make the list jump as the
    // selection moves down the hierarchy.
    rows.sort_by(|a, b| a.0.cmp(&b.0));

    // Built through a queue rather than `world.spawn` directly, the same way the
    // Scripts drawer does, so ember's widget helpers (which take `Commands`) stay
    // usable from a `&mut World` entry point.
    let mut queue = CommandQueue::default();
    let mut commands = Commands::new(&mut queue, world);

    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            FocusPolicy::Pass,
            Name::new("project-components-root"),
        ))
        .id();

    if let Some(fonts) = fonts {
        for (name, file, line) in rows {
            // Shown project-relative. The absolute path is what the code editor
            // needs and what the row carries; it is not what a reader wants.
            let shown = project_root
                .as_ref()
                .and_then(|root| file.strip_prefix(root).ok())
                .unwrap_or(file.as_path())
                .to_string_lossy()
                .replace('\\', "/");

            let row = commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        flex_direction: FlexDirection::Row,
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(6.0),
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(rgb(section_bg())),
                    Interaction::default(),
                    // Blocks, so the press belongs to this row rather than also
                    // reaching the section header behind it. Bevy 0.19's default
                    // is `Pass`, which is how one click does two things.
                    FocusPolicy::Block,
                    HoverCursor(SystemCursorIcon::Pointer),
                    ProjectComponentRow { file, line },
                    Name::new("project-component-row"),
                ))
                .id();

            let label = commands
                .spawn((Text::new(name), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary()))))
                .id();
            let where_ = commands
                .spawn((
                    Text::new(format!("{shown}:{line}")),
                    ui_font(&fonts.ui, 10.0),
                    TextColor(rgb(text_muted())),
                ))
                .id();
            commands.entity(row).add_children(&[label, where_]);
            commands.entity(root).add_children(&[row]);
        }
    }

    queue.apply(world);
    root
}

/// Open the declaring file in the code editor.
///
/// The same route the Scripts section's open button takes, so a component and a
/// script land in the same place by the same means.
fn open_declaration_click(
    q: Query<(&Interaction, &ProjectComponentRow), Changed<Interaction>>,
    mut commands: Commands,
) {
    for (interaction, row) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let file = row.file.clone();
        let line = row.line;
        commands.queue(move |w: &mut World| {
            w.insert_resource(OpenCodeEditorFile { path: file, line: Some(line) });
            if let Some(mut dock) = w.get_resource_mut::<renzora_ember::dock::Dock>() {
                dock.tree.focus_or_add_panel("code_editor");
            }
            if let Some(mut dirty) = w.get_resource_mut::<renzora_ember::dock::DockDirty>() {
                dirty.0 = true;
            }
        });
    }
}
