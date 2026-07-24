//! Native hierarchy right-click context menu, built on ember's shared
//! `screen_menu` primitive (on-screen clamp, pointer-block, click-outside
//! dismiss, and action dispatch all come from ember). Each item carries a
//! closure run with `&mut World`; the menu closes itself afterward. Action
//! bodies mirror the egui panel.
//!
//! Right-clicking the empty space under the tree gets a *different* menu: the
//! header button's whole spawn list, one ember [`menu_submenu`] row per category
//! (hover a category, click an entity), spawning at the scene root.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora::core::ShapeRegistry;
use renzora_editor_framework::{EditorSelection, EntityLabelColor, InspectorRegistry, SpawnRegistry};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::theme::rgb;
use renzora_ember::widgets::{
    category_color, menu_item, menu_item_styled, menu_sep, menu_submenu_styled, screen_menu,
    MenuAction, SearchEntry,
};
use renzora_undo::{execute, GroupAsChildrenCmd, UndoContext};

use crate::LABEL_COLORS;

use super::add_entity::spawn_entries;
use super::components::HierRowClick;

/// Marks the tree's scroll wrapper — the region a right-click can land in
/// *without* hitting a row (the empty space under the last entity). Scoped to
/// the list rather than the whole panel so right-clicking the search box still
/// gets the text field's own menu instead of this one.
#[derive(Component)]
pub(crate) struct HierListArea;

/// Registries the "Add Entity" submenu is built from — one bundle so the context
/// menu's own parameter list stays readable.
#[derive(SystemParam)]
pub(crate) struct SpawnRegistries<'w> {
    spawn: Option<Res<'w, SpawnRegistry>>,
    shape: Option<Res<'w, ShapeRegistry>>,
    inspector: Option<Res<'w, InspectorRegistry>>,
}

impl SpawnRegistries<'_> {
    fn entries(&self) -> Vec<SearchEntry> {
        spawn_entries(
            self.spawn.as_deref(),
            self.shape.as_deref(),
            self.inspector.as_deref(),
        )
    }
}

/// Right-click a row → open a menu of actions for that entity. Right-click the
/// empty space below the tree → the quick-add categories instead, so a scene can
/// be filled without going through the header button's search overlay. The two
/// are deliberately separate menus: on a row you're acting on that entity, not
/// hunting for a new one.
pub(crate) fn hier_context_menu(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    selection: Option<Res<EditorSelection>>,
    rows: Query<(&Interaction, &HierRowClick)>,
    props: Query<(Has<Camera3d>, Has<renzora::SceneInstance>, Has<EntityLabelColor>)>,
    list_area: Query<&RelativeCursorPosition, With<HierListArea>>,
    registries: SpawnRegistries,
    mut commands: Commands,
) {
    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    let Some(fonts) = fonts else {
        return;
    };
    let Some(cursor) = windows.iter().find_map(|w| w.cursor_position()) else {
        return;
    };
    let Some(target) = rows
        .iter()
        .find(|(i, _)| matches!(i, Interaction::Hovered | Interaction::Pressed))
        .map(|(_, r)| r.entity)
    else {
        // Not on a row: only the empty area below the tree gets a menu, and all
        // it offers is adding at the scene root.
        if list_area.iter().any(|rcp| rcp.cursor_over) {
            let menu = screen_menu(&mut commands, cursor.x, cursor.y);
            let rows = add_entity_rows(&mut commands, &fonts, registries.entries());
            commands.entity(menu).add_children(&rows);
        }
        return;
    };
    let (is_cam, is_inst, has_color) = props.get(target).unwrap_or((false, false, false));
    let multi = selection.as_ref().is_some_and(|s| s.has_multi_selection());

    let menu = screen_menu(&mut commands, cursor.x, cursor.y);
    let mut kids: Vec<Entity> = Vec::new();

    kids.push(menu_item(&mut commands, &fonts, "plus", &renzora::lang::t("hierarchy.context.add_child"), move |w| {
        add_child(w, target)
    }));
    kids.push(menu_item(&mut commands, &fonts, "pencil-simple", &renzora::lang::t("hierarchy.context.rename"), move |w| {
        if let Some(mut r) = w.get_resource_mut::<super::rename::HierRename>() {
            r.0 = Some(target);
        }
    }));
    kids.push(menu_item(&mut commands, &fonts, "copy", &renzora::lang::t("hierarchy.context.duplicate"), move |w| {
        duplicate(w, target)
    }));
    kids.push(menu_item(&mut commands, &fonts, "arrow-square-out", &renzora::lang::t("hierarchy.context.unparent"), move |w| {
        w.entity_mut(target).remove::<ChildOf>();
    }));
    if multi {
        kids.push(menu_item(&mut commands, &fonts, "folder-simple", &renzora::lang::t("hierarchy.context.group_as_children"), group_selection));
    }

    // Label-color (entity color-coding) section.
    kids.push(menu_sep(&mut commands));
    kids.push(color_header(&mut commands, &fonts));
    kids.push(swatch_grid(&mut commands, target));
    if has_color {
        kids.push(menu_item(&mut commands, &fonts, "x", &renzora::lang::t("hierarchy.context.clear_color"), move |w| {
            w.entity_mut(target).remove::<EntityLabelColor>();
        }));
    }

    if is_cam {
        kids.push(menu_sep(&mut commands));
        kids.push(menu_item_styled(&mut commands, &fonts, "star", &renzora::lang::t("hierarchy.context.set_default_camera"), (255, 200, 80), renzora_ember::theme::text_primary(), move |w| {
            set_default_camera(w, target)
        }));
        kids.push(menu_item(&mut commands, &fonts, "frame-corners", &renzora::lang::t("hierarchy.context.snap_to_viewport"), move |w| {
            renzora_editor_framework::camera::snap_entity_to_editor_camera(w, target);
        }));
    }

    kids.push(menu_sep(&mut commands));
    kids.push(menu_item(&mut commands, &fonts, "film-slate", &renzora::lang::t("hierarchy.context.instance_scene"), move |w| {
        instance_scene(w, target)
    }));
    if is_inst {
        kids.push(menu_item(&mut commands, &fonts, "link-break", &renzora::lang::t("hierarchy.context.unpack_scene_instance"), move |w| {
            w.entity_mut(target).remove::<renzora::SceneInstance>();
            renzora::core::console_log::console_info("Scene", format!("Unpacked scene instance {target:?}"));
        }));
    }

    kids.push(menu_sep(&mut commands));
    kids.push(menu_item_styled(&mut commands, &fonts, "trash", &renzora::lang::t("hierarchy.context.delete"), renzora_ember::theme::close_red(), renzora_ember::theme::close_red(), move |w| {
        let entities: Vec<Entity> = match w.get_resource::<EditorSelection>() {
            Some(s) if s.is_selected(target) => s.get_all(),
            _ => vec![target],
        };
        delete_entities(w, &entities);
    }));

    commands.entity(menu).add_children(&kids);
}

/// The header button's whole spawn list as hover submenus, one row per category,
/// sitting at the *top level* of the menu — no "Add Entity" row to open first,
/// so a common entity is one hover and a click away instead of a search-overlay
/// round-trip. `entries` decide where the spawn lands (see [`spawn_entries`]).
fn add_entity_rows(commands: &mut Commands, fonts: &EmberFonts, entries: Vec<SearchEntry>) -> Vec<Entity> {
    // Group by category, first-seen order — the same order (presets, shapes,
    // then component-backed entries) the search overlay lists them in.
    let mut cats: Vec<(String, Vec<SearchEntry>)> = Vec::new();
    for entry in entries {
        match cats.iter_mut().find(|(name, _)| *name == entry.category) {
            Some((_, group)) => group.push(entry),
            None => cats.push((entry.category.clone(), vec![entry])),
        }
    }

    cats.into_iter()
        .map(|(name, group)| {
            // Same accent the Add Entity overlay gives this category, so the two
            // views of one list read as the same thing — and the icons carry it
            // down into the category's own items.
            let accent = category_color(&name);
            // The first entry's glyph reads better as the category's icon than a
            // generic folder would (Lights → bulb, Shapes → cube).
            let icon = group.first().map(|e| e.icon.clone()).unwrap_or_default();
            let (cat_row, cat_content) = menu_submenu_styled(commands, fonts, &icon, &name, accent);
            let items: Vec<Entity> = group
                .into_iter()
                .map(|entry| {
                    let SearchEntry { icon, label, action, .. } = entry;
                    menu_item_styled(
                        commands,
                        fonts,
                        &icon,
                        &label,
                        accent,
                        renzora_ember::theme::text_primary(),
                        action,
                    )
                })
                .collect();
            commands.entity(cat_content).add_children(&items);
            cat_row
        })
        .collect()
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
            Text::new(renzora::lang::t("hierarchy.context.label_color")),
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

/// Pick a `.ron` scene and spawn it as a nested instance under `parent`, with a
/// reference-cycle guard (mirrors the egui "Instance Scene…").
fn instance_scene(world: &mut World, parent: Entity) {
    let scenes_dir = world
        .get_resource::<renzora::core::CurrentProject>()
        .map(|p| p.resolve_path("scenes"));

    #[cfg(not(target_arch = "wasm32"))]
    let file = {
        let mut dlg = rfd::FileDialog::new()
            .set_title(renzora::lang::t("hierarchy.dialog.instance_scene_title"))
            .add_filter(renzora::lang::t("hierarchy.dialog.scene_file"), &["bsn"]);
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
                toasts.warning(renzora::lang::t("hierarchy.toast.cannot_add_self"));
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

/// Delete entities with faithful undo — snapshots each entity's whole subtree
/// (any components + children) so Ctrl+Z restores lights, cameras, imported
/// models, 2D nodes and groups, not just default-mesh primitives.
fn delete_entities(world: &mut World, entities: &[Entity]) {
    renzora_undo::delete_entities_with_undo(world, entities);
}
