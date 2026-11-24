//! Native hierarchy right-click context menu, built on ember's shared
//! `screen_menu` primitive (on-screen clamp, pointer-block, click-outside
//! dismiss, and action dispatch all come from ember). Each item carries a
//! closure run with `&mut World`; the menu closes itself afterward.
//!
//! A row's menu is a short list of submenus: **Add** (an empty child, or any
//! spawnable kind, placed under the entity), **Edit**, **Attach**, **Prefab**,
//! **Camera** (cameras only) and **Label Color**, then whatever other crates
//! registered for the entity, then Delete.
//!
//! The "Attach ▸" submenu (make a new script / material / particle / … for this
//! entity) lives in [`super::create_asset`]: it owns an overlay as well as the
//! rows, and this file is long enough already.
//!
//! Right-clicking the empty space under the tree gets a *different* menu: the
//! header button's whole spawn list, one ember [`menu_submenu`] row per category
//! (hover a category, click an entity), spawning at the scene root.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use renzora_ember::UiCursor;

use renzora_scene_api::ShapeRegistry;
use renzora_editor_framework::{EditorSelection, EntityLabelColor, InspectorRegistry, SpawnRegistry};
use renzora_ember::font::EmberFonts;
use renzora_ember::theme::rgb;
use renzora_ember::widgets::{
    category_color, menu_item, menu_item_styled, menu_sep, menu_submenu, menu_submenu_styled,
    screen_menu, MenuAction, SearchEntry,
};
use renzora_ember::UiInteraction;
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
    scope: Option<Res<'w, renzora_editor_api::SpawnCategoryScope>>,
}

impl SpawnRegistries<'_> {
    pub(crate) fn entries(&self) -> Vec<SearchEntry> {
        spawn_entries(
            self.spawn.as_deref(),
            self.shape.as_deref(),
            self.inspector.as_deref(),
            self.scope.as_deref(),
        )
    }
}

/// What decides whether "Separate Parts" is offered: a part of a placed model
/// (not the model itself) that draws meshes, or holds parts that do.
#[derive(SystemParam)]
pub(crate) struct ModelParts<'w, 's> {
    meshes: Query<'w, 's, (), With<bevy::gltf::GltfMeshName>>,
    models: Query<'w, 's, (), With<renzora_scene_api::MeshInstanceData>>,
    children: Query<'w, 's, &'static Children>,
    parents: Query<'w, 's, &'static ChildOf>,
}

impl ModelParts<'_, '_> {
    fn separable(&self, entity: Entity) -> bool {
        let in_model = self.parents.iter_ancestors(entity).any(|a| self.models.contains(a));
        in_model
            && !self.models.contains(entity)
            && (self.meshes.contains(entity) || self.children.iter_descendants(entity).any(|d| self.meshes.contains(d)))
    }
}

/// Right-click a row → open a menu of actions for that entity. Right-click the
/// empty space below the tree → the quick-add categories instead, so a scene can
/// be filled without going through the header button's search overlay. The two
/// are deliberately separate menus: on a row you're acting on that entity, not
/// hunting for a new one.
pub(crate) fn hier_context_menu(
    mouse: Res<ButtonInput<MouseButton>>,
    create_menu: Option<Res<renzora_shell_api::CreateMenuRegistry>>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    selection: Option<Res<EditorSelection>>,
    rows: Query<(&UiInteraction, &HierRowClick)>,
    props: Query<(Has<Camera3d>, Has<renzora_scene_api::SceneInstance>, Has<EntityLabelColor>)>,
    unit_props: Query<(Has<renzora_scene_api::PickAsUnit>, Has<renzora_scene_api::SelectionStop>, Has<Children>)>,
    ui: Query<
        (),
        Or<(
            With<renzora_ember::game_ui::UiCanvas>,
            With<renzora_ember::game_ui::UiWidget>,
        )>,
    >,
    list_area: Query<&UiCursor, With<HierListArea>>,
    registries: SpawnRegistries,
    model_parts: ModelParts,
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
        .find(|(i, _)| matches!(i, UiInteraction::Hovered | UiInteraction::Pressed))
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
    // A UI canvas or widget. Its contents come from a `.html` and are rebuilt
    // from that file, so the rows that restructure a *scene* subtree — add a
    // child, unparent, instance a scene into it — either do nothing that
    // survives or actively break the canvas. They are left out rather than
    // shown-and-ignored.
    let is_ui = ui.get(target).is_ok();

    // One submenu per kind of thing to do, so the top level is a short list of
    // verbs instead of every action the entity supports. It had grown to twenty
    // rows plus a colour grid, and the delete at the bottom of it was past the
    // fold on a laptop screen.
    let menu = screen_menu(&mut commands, cursor.x, cursor.y);
    let mut kids: Vec<Entity> = Vec::new();

    // ── Add ▸ ── an empty child, then every spawnable kind, all placed under
    // this entity at its origin.
    if !is_ui {
        let (row, content) = menu_submenu(&mut commands, &fonts, "plus", &renzora_lang_api::t("hierarchy.context.add"));
        let mut items = vec![
            menu_item(&mut commands, &fonts, "plus-square", &renzora_lang_api::t("hierarchy.context.add_empty"), move |w| {
                add_child(w, target)
            }),
            menu_sep(&mut commands),
        ];
        items.extend(add_entity_rows(&mut commands, &fonts, child_entries(registries.entries(), target)));
        commands.entity(content).add_children(&items);
        kids.push(row);
    }

    // ── Edit ▸ ──
    {
        let (row, content) = menu_submenu(&mut commands, &fonts, "pencil-simple", &renzora_lang_api::t("hierarchy.context.edit"));
        let mut items = vec![
            menu_item(&mut commands, &fonts, "pencil-simple", &renzora_lang_api::t("hierarchy.context.rename"), move |w| {
                if let Some(mut r) = w.get_resource_mut::<super::rename::HierRename>() {
                    r.0 = Some(target);
                }
            }),
            menu_item(&mut commands, &fonts, "copy", &renzora_lang_api::t("hierarchy.context.duplicate"), move |w| {
                duplicate(w, target)
            }),
        ];
        if !is_ui {
            items.push(menu_item(&mut commands, &fonts, "arrow-square-out", &renzora_lang_api::t("hierarchy.context.unparent"), move |w| {
                w.entity_mut(target).remove::<ChildOf>();
            }));
        }
        if multi {
            items.push(menu_item(&mut commands, &fonts, "folder-simple", &renzora_lang_api::t("hierarchy.context.group_as_children"), group_selection));
        }
        // Whether a viewport click on any part of this selects the whole thing
        // (`renzora_scene_api::PickAsUnit`, read by `renzora_gizmo::picking`).
        // Offered on anything with children, which is anything it can make a
        // difference to; not on an entity whose code already makes it pick as
        // one (`SelectionStop`, a terrain), which the toggle could not undo.
        let (is_unit, is_stop, has_children) = unit_props.get(target).unwrap_or((false, false, false));
        if !is_ui && has_children && !is_stop {
            let (icon, key) = if is_unit {
                ("squares-four", "hierarchy.context.select_parts")
            } else {
                ("cube", "hierarchy.context.select_as_one")
            };
            items.push(menu_item(&mut commands, &fonts, icon, &renzora_lang_api::t(key), move |w| {
                let Ok(mut e) = w.get_entity_mut(target) else { return };
                if is_unit {
                    e.remove::<renzora_scene_api::PickAsUnit>();
                } else {
                    e.insert(renzora_scene_api::PickAsUnit);
                }
            }));
        }
        // Split merged pieces of a model part (both wheels of an axle) into
        // nodes of their own. A rewrite of the model file, done by the rig tools.
        if !is_ui && model_parts.separable(target) {
            items.push(menu_item(&mut commands, &fonts, "scissors", &renzora_lang_api::t("hierarchy.context.separate_parts"), move |w| {
                w.trigger(renzora_shell_api::SeparateModelParts { entity: target });
            }));
        }
        commands.entity(content).add_children(&items);
        kids.push(row);
    }

    // ── Attach ▸ ── make a new project asset (and, for scripts, attach it here).
    kids.push(super::create_asset::create_submenu(
        &mut commands,
        &fonts,
        target,
        is_ui,
        create_menu.as_deref(),
    ));

    // ── Prefab ▸ ──
    if !is_ui {
        let (row, content) = menu_submenu(&mut commands, &fonts, "package", &renzora_lang_api::t("hierarchy.context.prefab"));
        let mut items = Vec::new();
        if is_inst {
            items.push(menu_item(&mut commands, &fonts, "upload-simple", &renzora_lang_api::t("hierarchy.context.apply_prefab"), move |w| {
                renzora_undo::apply_prefab(w, target)
            }));
            items.push(menu_item(&mut commands, &fonts, "arrow-counter-clockwise", &renzora_lang_api::t("hierarchy.context.revert_prefab"), move |w| {
                renzora_undo::revert_prefab_with_undo(w, target)
            }));
            items.push(menu_item(&mut commands, &fonts, "link-break", &renzora_lang_api::t("hierarchy.context.unpack_scene_instance"), move |w| {
                w.entity_mut(target).remove::<renzora_scene_api::SceneInstance>();
                renzora_log_api::console_info("Scene", format!("Unpacked scene instance {target:?}"));
            }));
        } else {
            items.push(menu_item(&mut commands, &fonts, "package", &renzora_lang_api::t("hierarchy.context.create_prefab"), move |w| {
                renzora_undo::create_prefab_with_undo(w, target)
            }));
        }
        items.push(menu_sep(&mut commands));
        items.push(menu_item(&mut commands, &fonts, "film-slate", &renzora_lang_api::t("hierarchy.context.instance_scene"), move |w| {
            instance_scene(w, target)
        }));
        commands.entity(content).add_children(&items);
        kids.push(row);
    }

    // ── Camera ▸ ──
    if is_cam {
        let (row, content) = menu_submenu(&mut commands, &fonts, "video-camera", &renzora_lang_api::t("hierarchy.context.camera"));
        let items = [
            menu_item_styled(&mut commands, &fonts, "star", &renzora_lang_api::t("hierarchy.context.set_default_camera"), (255, 200, 80), renzora_ember::theme::text_primary(), move |w| {
                set_default_camera(w, target)
            }),
            menu_item(&mut commands, &fonts, "frame-corners", &renzora_lang_api::t("hierarchy.context.snap_to_viewport"), move |w| {
                renzora_editor_framework::camera::snap_entity_to_editor_camera(w, target);
            }),
        ];
        commands.entity(content).add_children(&items);
        kids.push(row);
    }

    // ── Label Color ▸ ──
    {
        let (row, content) = menu_submenu(&mut commands, &fonts, "palette", &renzora_lang_api::t("hierarchy.context.label_color"));
        let mut items = vec![swatch_grid(&mut commands, target)];
        if has_color {
            items.push(menu_item(&mut commands, &fonts, "x", &renzora_lang_api::t("hierarchy.context.clear_color"), move |w| {
                w.entity_mut(target).remove::<EntityLabelColor>();
            }));
        }
        commands.entity(content).add_children(&items);
        kids.push(row);
    }

    // What other crates registered for this entity (world partition's
    // "Partition into cells"), at the top level: each is one specific action.
    if !is_ui {
        kids.push(entity_action_slot(&mut commands, &fonts, target));
    }

    kids.push(menu_sep(&mut commands));
    kids.push(menu_item_styled(&mut commands, &fonts, "trash", &renzora_lang_api::t("hierarchy.context.delete"), renzora_ember::theme::close_red(), renzora_ember::theme::close_red(), move |w| {
        let entities: Vec<Entity> = match w.get_resource::<EditorSelection>() {
            Some(s) if s.is_selected(target) => s.get_all(),
            _ => vec![target],
        };
        delete_entities(w, &entities);
    }));

    commands.entity(menu).add_children(&kids);
}

/// The rows other crates registered for `target`
/// ([`renzora_shell_api::EntityActionRegistry`]), such as world partition's
/// "Partition into cells".
///
/// Whether a row applies is a question about the world, which this system
/// cannot ask, so it reserves an empty slot in the menu's order and fills it
/// from a command that can. The slot stays collapsed when nothing applies.
fn entity_action_slot(commands: &mut Commands, fonts: &EmberFonts, target: Entity) -> Entity {
    let slot = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .id();
    let fonts = fonts.clone();
    commands.queue(move |world: &mut World| {
        let rows: Vec<(String, String, fn(&mut World, Entity))> = {
            let Some(registry) = world.get_resource::<renzora_shell_api::EntityActionRegistry>() else {
                return;
            };
            registry
                .for_entity(world, target)
                .map(|a| (a.icon.clone(), a.label_text(), a.run))
                .collect()
        };
        if rows.is_empty() {
            return;
        }
        let mut queue = bevy::ecs::world::CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            let mut kids = vec![menu_sep(&mut commands)];
            for (icon, label, run) in rows {
                kids.push(menu_item(&mut commands, &fonts, &icon, &label, move |w| run(w, target)));
            }
            if let Ok(mut slot) = commands.get_entity(slot) {
                slot.add_children(&kids);
            }
        }
        queue.apply(world);
    });
    slot
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

/// The spawn list again, with each action putting what it spawns under
/// `parent` at the parent's origin, for a row's **Add** submenu.
///
/// The spawn commands select what they spawn, which is how the wrapper finds
/// it; nothing new selected means nothing was spawned. The reparent is outside
/// the spawn's undo entry, and does not need to be in it: undoing the spawn
/// removes the entity wherever it is.
fn child_entries(entries: Vec<SearchEntry>, parent: Entity) -> Vec<SearchEntry> {
    entries
        .into_iter()
        .map(|mut entry| {
            let spawn = std::mem::replace(&mut entry.action, Box::new(|_: &mut World| {}));
            entry.action = Box::new(move |w: &mut World| {
                let before = w.get_resource::<EditorSelection>().and_then(|s| s.get());
                spawn(w);
                let after = w.get_resource::<EditorSelection>().and_then(|s| s.get());
                if let Some(child) = after.filter(|a| Some(*a) != before && *a != parent) {
                    if let Ok(mut e) = w.get_entity_mut(child) {
                        e.insert(ChildOf(parent));
                    }
                }
            });
            entry
        })
        .collect()
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
                    UiInteraction::default(),
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

/// Deep-clones `target` through Bevy's `clone_and_spawn`, the same path the
/// Ctrl+D shortcut uses (`renzora_gizmo::shortcuts::duplicate_entities`), so
/// the menu and keyboard produce the same object. This used to spawn only a
/// `Name` and a default `Transform`: the "copy" had no mesh, collider or
/// gameplay components and sat at the origin.
fn duplicate(world: &mut World, target: Entity) {
    let Ok(mut src) = world.get_entity_mut(target) else {
        return;
    };
    let new_entity = src.clone_and_spawn();
    let name = world
        .get::<Name>(new_entity)
        .map(|n| format!("{} (Copy)", n.as_str()))
        .unwrap_or_else(|| "Entity (Copy)".to_string());
    world.entity_mut(new_entity).insert(Name::new(name));
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
            if e != target && world.get::<renzora_scene_api::DefaultCamera>(e).is_some() {
                to_remove.push(e);
            }
        }
    }
    for e in to_remove {
        world.entity_mut(e).remove::<renzora_scene_api::DefaultCamera>();
    }
    world.entity_mut(target).insert(renzora_scene_api::DefaultCamera);
}

/// Pick a `.ron` scene and spawn it as a nested instance under `parent`, with a
/// reference-cycle guard (mirrors the egui "Instance Scene…").
fn instance_scene(world: &mut World, parent: Entity) {
    let scenes_dir = world
        .get_resource::<renzora_project_api::CurrentProject>()
        .map(|p| p.resolve_path("scenes"));

    #[cfg(not(target_arch = "wasm32"))]
    let file = {
        let mut dlg = rfd::FileDialog::new()
            .set_title(renzora_lang_api::t("hierarchy.dialog.instance_scene_title"))
            .add_filter(renzora_lang_api::t("hierarchy.dialog.scene_file"), &["bsn"]);
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
        .get_resource::<renzora_project_api::CurrentProject>()
        .and_then(|p| {
            world
                .get_resource::<renzora_ui::DocumentTabState>()
                .and_then(|t| t.tabs.get(t.active_tab).and_then(|tab| tab.scene_path.clone()))
                .map(|rel| p.resolve_path(&rel))
        });
    if let (Some(host_abs), Some(project_root)) = (
        host_abs,
        world
            .get_resource::<renzora_project_api::CurrentProject>()
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
                toasts.warning(renzora_lang_api::t("hierarchy.toast.cannot_add_self"));
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
    // Reselect the nearest surviving neighbour after deleting so the viewport +
    // inspector don't go blank (shared with the Delete-key path in the gizmo).
    renzora_undo::delete_entities_with_undo_reselect(world, entities);
}
