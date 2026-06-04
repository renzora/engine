//! Native hierarchy right-click context menu, built on ember's shared
//! `screen_menu` primitive (on-screen clamp, pointer-block, click-outside
//! dismiss, and action dispatch all come from ember). Each item carries a
//! closure run with `&mut World`; the menu closes itself afterward. Action
//! bodies mirror the egui panel.

use bevy::prelude::*;

use renzora_editor::{EditorSelection, EntityLabelColor};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::theme::rgb;
use renzora_ember::widgets::{menu_item, menu_item_styled, menu_sep, screen_menu, MenuAction};
use renzora_undo::{execute, DeleteShapesCmd, DeletedShape, GroupAsChildrenCmd, UndoContext};

use crate::LABEL_COLORS;

use super::components::HierRowClick;


/// Right-click a row → open a menu of actions for that entity.
pub(crate) fn hier_context_menu(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    selection: Option<Res<EditorSelection>>,
    rows: Query<(&Interaction, &HierRowClick)>,
    props: Query<(Has<Camera3d>, Has<renzora::SceneInstance>, Has<EntityLabelColor>)>,
    mut commands: Commands,
) {
    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    let Some(fonts) = fonts else {
        return;
    };
    let Some(target) = rows
        .iter()
        .find(|(i, _)| matches!(i, Interaction::Hovered | Interaction::Pressed))
        .map(|(_, r)| r.entity)
    else {
        return;
    };
    let Some(cursor) = windows.iter().next().and_then(|w| w.cursor_position()) else {
        return;
    };
    let (is_cam, is_inst, has_color) = props.get(target).unwrap_or((false, false, false));
    let multi = selection.as_ref().is_some_and(|s| s.has_multi_selection());

    let menu = screen_menu(&mut commands, cursor.x, cursor.y);
    let mut kids: Vec<Entity> = Vec::new();

    kids.push(menu_item(&mut commands, &fonts, "plus", "Add Child Entity", move |w| {
        add_child(w, target)
    }));
    kids.push(menu_item(&mut commands, &fonts, "copy", "Duplicate", move |w| {
        duplicate(w, target)
    }));
    kids.push(menu_item(&mut commands, &fonts, "arrow-square-out", "Unparent", move |w| {
        w.entity_mut(target).remove::<ChildOf>();
    }));
    if multi {
        kids.push(menu_item(&mut commands, &fonts, "folder-simple", "Group as Children", group_selection));
    }

    // Label-color (entity color-coding) section.
    kids.push(menu_sep(&mut commands));
    kids.push(color_header(&mut commands, &fonts));
    kids.push(swatch_grid(&mut commands, target));
    if has_color {
        kids.push(menu_item(&mut commands, &fonts, "x", "Clear Color", move |w| {
            w.entity_mut(target).remove::<EntityLabelColor>();
        }));
    }

    if is_cam {
        kids.push(menu_sep(&mut commands));
        kids.push(menu_item_styled(&mut commands, &fonts, "star", "Set as Default Camera", (255, 200, 80), renzora_ember::theme::text_primary(), move |w| {
            set_default_camera(w, target)
        }));
        kids.push(menu_item(&mut commands, &fonts, "frame-corners", "Snap to Viewport", move |w| {
            snap_to_viewport(w, target)
        }));
    }

    kids.push(menu_sep(&mut commands));
    kids.push(menu_item(&mut commands, &fonts, "film-strip", "Instance Scene…", move |w| {
        instance_scene(w, target)
    }));
    if is_inst {
        kids.push(menu_item(&mut commands, &fonts, "link-break", "Unpack Scene Instance", move |w| {
            w.entity_mut(target).remove::<renzora::SceneInstance>();
            renzora::core::console_log::console_info("Scene", format!("Unpacked scene instance {target:?}"));
        }));
    }

    kids.push(menu_sep(&mut commands));
    kids.push(menu_item_styled(&mut commands, &fonts, "trash", "Delete", renzora_ember::theme::close_red(), renzora_ember::theme::close_red(), move |w| {
        let entities: Vec<Entity> = match w.get_resource::<EditorSelection>() {
            Some(s) if s.is_selected(target) => s.get_all(),
            _ => vec![target],
        };
        delete_entities(w, &entities);
    }));

    commands.entity(menu).add_children(&kids);
}

/// "Label Color" header (palette glyph + label).
fn color_header(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            width: Val::Percent(100.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
            ..default()
        })
        .id();
    let ic = icon_text(commands, &fonts.phosphor, "palette", renzora_ember::theme::text_muted(), 11.0);
    let label = commands
        .spawn((
            Text::new("Label Color"),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(renzora_ember::theme::text_muted())),
        ))
        .id();
    commands.entity(row).add_children(&[ic, label]);
    row
}

/// A fixed-width wrapping grid of label-color swatches; clicking one tags the
/// entity (and closes the menu, via the swatch's `MenuAction`).
fn swatch_grid(commands: &mut Commands, target: Entity) -> Entity {
    let grid = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            width: Val::Px(196.0),
            column_gap: Val::Px(4.0),
            row_gap: Val::Px(4.0),
            padding: UiRect::new(Val::Px(8.0), Val::Px(8.0), Val::Px(2.0), Val::Px(4.0)),
            ..default()
        })
        .id();
    let swatches: Vec<Entity> = LABEL_COLORS
        .iter()
        .map(|&(color, _name)| {
            commands
                .spawn((
                    Node {
                        width: Val::Px(16.0),
                        height: Val::Px(16.0),
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb_u8(color[0], color[1], color[2])),
                    BorderColor::all(rgb(renzora_ember::theme::border())),
                    Interaction::default(),
                    MenuAction(Box::new(move |w: &mut World| {
                        w.entity_mut(target).insert(EntityLabelColor(color));
                    })),
                    Name::new("hier-color-swatch"),
                ))
                .id()
        })
        .collect();
    commands.entity(grid).add_children(&swatches);
    grid
}

// ── Action bodies (run with &mut World) ──────────────────────────────────────

fn add_child(world: &mut World, target: Entity) {
    let child = world.spawn((Name::new("New Entity"), Transform::default())).id();
    world.entity_mut(child).set_parent_in_place(target);
    if let Some(sel) = world.get_resource::<EditorSelection>() {
        sel.set(Some(child));
    }
}

fn duplicate(world: &mut World, target: Entity) {
    let parent = world.get::<ChildOf>(target).map(|c| c.parent());
    let name = world
        .get::<Name>(target)
        .map(|n| format!("{} (Copy)", n.as_str()))
        .unwrap_or_else(|| "Entity (Copy)".to_string());
    let new_entity = world.spawn((Name::new(name), Transform::default())).id();
    if let Some(p) = parent {
        world.entity_mut(new_entity).set_parent_in_place(p);
    }
    if let Some(sel) = world.get_resource::<EditorSelection>() {
        sel.set(Some(new_entity));
    }
}

fn group_selection(world: &mut World) {
    let members = world
        .get_resource::<EditorSelection>()
        .map(|s| s.get_all())
        .unwrap_or_default();
    let members: Vec<(Entity, Option<Entity>)> = members
        .iter()
        .map(|e| (*e, world.get::<ChildOf>(*e).map(|c| c.parent())))
        .collect();
    execute(
        world,
        UndoContext::Scene,
        Box::new(GroupAsChildrenCmd {
            parent: Entity::PLACEHOLDER,
            group_name: "Group".to_string(),
            members,
        }),
    );
}

fn set_default_camera(world: &mut World, target: Entity) {
    let mut to_remove = Vec::new();
    for archetype in world.archetypes().iter() {
        for arch_entity in archetype.entities() {
            let e = arch_entity.id();
            if e != target && world.get::<renzora::core::DefaultCamera>(e).is_some() {
                to_remove.push(e);
            }
        }
    }
    for e in to_remove {
        world.entity_mut(e).remove::<renzora::core::DefaultCamera>();
    }
    world.entity_mut(target).insert(renzora::core::DefaultCamera);
}

fn snap_to_viewport(world: &mut World, target: Entity) {
    let editor_transform = {
        let mut q = world.query_filtered::<&Transform, With<renzora::core::EditorCamera>>();
        q.iter(world).next().copied()
    };
    if let Some(t) = editor_transform {
        if let Some(mut transform) = world.get_mut::<Transform>(target) {
            *transform = t;
        }
    }
}

/// Pick a `.ron` scene and spawn it as a nested instance under `parent`, with a
/// reference-cycle guard (mirrors the egui "Instance Scene…").
fn instance_scene(world: &mut World, parent: Entity) {
    let scenes_dir = world
        .get_resource::<renzora::core::CurrentProject>()
        .map(|p| p.resolve_path("scenes"));

    #[cfg(not(target_arch = "wasm32"))]
    let file = {
        let mut dlg = rfd::FileDialog::new()
            .set_title("Instance Scene")
            .add_filter("Scene File", &["ron"]);
        if let Some(ref dir) = scenes_dir {
            dlg = dlg.set_directory(dir);
        }
        dlg.pick_file()
    };
    #[cfg(target_arch = "wasm32")]
    let file: Option<std::path::PathBuf> = None;

    let Some(path) = file else {
        return;
    };
    let host_abs = world
        .get_resource::<renzora::core::CurrentProject>()
        .and_then(|p| {
            world
                .get_resource::<renzora_ui::DocumentTabState>()
                .and_then(|t| t.tabs.get(t.active_tab).and_then(|tab| tab.scene_path.clone()))
                .map(|rel| p.resolve_path(&rel))
        });
    if let (Some(host_abs), Some(project_root)) = (
        host_abs,
        world
            .get_resource::<renzora::core::CurrentProject>()
            .map(|p| p.path.clone()),
    ) {
        let mut cache = world
            .remove_resource::<renzora_engine::scene_io::SceneReferenceCache>()
            .unwrap_or_default();
        let cycle = renzora_engine::scene_io::would_create_reference_cycle(
            &mut cache,
            &project_root,
            &host_abs,
            &path,
        );
        world.insert_resource(cache);
        if cycle {
            if let Some(mut toasts) = world.get_resource_mut::<renzora_ui::Toasts>() {
                toasts.warning("You cannot add a scene to itself");
            }
            return;
        }
    }
    if let Some(entity) =
        renzora_engine::scene_io::spawn_scene_instance(world, &path, Some(parent), Transform::default())
    {
        if let Some(sel) = world.get_resource::<EditorSelection>() {
            sel.set(Some(entity));
        }
    }
}

/// Delete entities — shapes go through an undoable `DeleteShapesCmd`, everything
/// else is despawned directly (mirrors the egui delete).
fn delete_entities(world: &mut World, entities: &[Entity]) {
    let mut items = Vec::new();
    let mut other = Vec::new();
    for entity in entities {
        let shape = world.get_entity(*entity).ok().and_then(|e| {
            Some(DeletedShape {
                entity: *entity,
                shape_id: e.get::<renzora::core::MeshPrimitive>()?.0.clone(),
                name: e.get::<Name>()?.as_str().to_string(),
                transform: *e.get::<Transform>()?,
                color: e.get::<renzora::core::MeshColor>()?.0,
            })
        });
        match shape {
            Some(item) => items.push(item),
            None => other.push(*entity),
        }
    }
    for e in other {
        if let Ok(em) = world.get_entity_mut(e) {
            em.despawn();
        }
    }
    if let Some(sel) = world.get_resource::<EditorSelection>() {
        sel.clear();
    }
    if !items.is_empty() {
        execute(world, UndoContext::Scene, Box::new(DeleteShapesCmd { items }));
    }
}
