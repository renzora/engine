//! Hierarchy panel — shows the scene entity tree.

mod state;
mod tree;

use std::sync::RwLock;

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora_editor_framework::{
    search_overlay, AppEditorExt, EditorCommands, EditorPanel, EditorSelection,
    HierarchyOrder, InspectorRegistry, OverlayAction, OverlayEntry, PanelLocation, SpawnRegistry,
};
use renzora::core::ShapeRegistry;
use renzora_theme::ThemeManager;
use renzora_undo::{self, CompoundCmd, ReparentCmd, SetHierarchyOrderCmd, SpawnEntityCmd, SpawnEntityKind, SpawnShapeCmd, UndoCommand, UndoContext};

use state::{build_entity_tree, filter_tree, HierarchyState};

/// Label color presets: ([r, g, b], name).
pub const LABEL_COLORS: &[([u8; 3], &str)] = &[
    ([220, 70,  70],  "Red"),
    ([210, 120, 80],  "Coral"),
    ([220, 140, 60],  "Orange"),
    ([210, 175, 55],  "Amber"),
    ([210, 195, 60],  "Yellow"),
    ([160, 210, 60],  "Lime"),
    ([70,  190, 100], "Green"),
    ([55,  185, 155], "Teal"),
    ([60,  200, 200], "Cyan"),
    ([70,  170, 220], "Sky"),
    ([80,  140, 220], "Blue"),
    ([90,  100, 220], "Indigo"),
    ([155, 80,  220], "Purple"),
    ([190, 70,  200], "Violet"),
    ([220, 80,  180], "Pink"),
    ([220, 80,  120], "Rose"),
    ([160, 110, 75],  "Brown"),
    ([130, 130, 140], "Gray"),
    ([200, 200, 200], "White"),
];

/// Hierarchy panel — displays all named entities as a tree.
pub struct HierarchyPanel {
    state: RwLock<HierarchyState>,
}

impl Default for HierarchyPanel {
    fn default() -> Self {
        Self {
            state: RwLock::new(HierarchyState::default()),
        }
    }
}

impl EditorPanel for HierarchyPanel {
    fn id(&self) -> &str {
        "hierarchy"
    }

    fn title(&self) -> &str {
        "Hierarchy"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::LIST_BULLETS)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };

        let commands = match world.get_resource::<EditorCommands>() {
            Some(c) => c,
            None => return,
        };

        let selection = match world.get_resource::<EditorSelection>() {
            Some(s) => s,
            None => return,
        };

        let mut state = self.state.write().unwrap();

        // Auto-expand ancestors of newly selected entities so viewport picks
        // stay visible in the tree even when their parent groups are collapsed.
        // Only insert ancestors that actually appear in the tree (have `Name`
        // and aren't `HideInHierarchy`) — the tree reparents children across
        // unnamed intermediaries like GLTF's SceneRoot, so we must mirror that.
        let current: Vec<Entity> = selection.get_all();
        if current != state.last_selection {
            for &entity in &current {
                let mut cur = entity;
                while let Some(child_of) = world.get::<ChildOf>(cur) {
                    let parent = child_of.parent();
                    let named = world.get::<Name>(parent).is_some();
                    let hidden = world.get::<renzora_editor_framework::HideInHierarchy>(parent).is_some();
                    if named && !hidden {
                        state.expanded.insert(parent);
                    }
                    cur = parent;
                }
            }
            state.last_selection = current;
        }

        // Check for CreateNode shortcut (Ctrl+A)
        if world.get_resource::<renzora::core::CreateNodeRequested>().is_some() {
            state.show_add_overlay = true;
            state.add_search.clear();
            // Consume the resource via deferred command
            commands.push(|w: &mut World| { w.remove_resource::<renzora::core::CreateNodeRequested>(); });
        }

        // Search bar + "Add Entity" button
        ui.add_space(4.0);
        let row_height = ui.spacing().interact_size.y;
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            let add_width = 50.0;
            let spacing = ui.spacing().item_spacing.x;
            let search_width = ui.available_width() - add_width - spacing - 8.0;
            ui.add(
                egui::TextEdit::singleline(&mut state.search)
                    .desired_width(search_width)
                    .hint_text(format!("{} Search entities...", regular::MAGNIFYING_GLASS)),
            );
            let btn = egui::Button::new(
                egui::RichText::new(format!("{} Add", regular::PLUS))
                    .color(theme.semantic.accent.to_color32())
                    .size(11.0),
            );
            if ui.add_sized([add_width, row_height], btn).clicked() {
                state.show_add_overlay = true;
                state.add_search.clear();
            }
        });
        ui.add_space(4.0);

        // Collider stamp progress strip — shown while a bulk stamp is in flight.
        if let Some(queue) = world.get_resource::<renzora_physics::ColliderStampQueue>() {
            if queue.is_active() {
                let progress = queue.progress();
                let done = queue.total.saturating_sub(queue.remaining.len());
                let total = queue.total;
                ui.horizontal(|ui| {
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(format!("{} Stamping {}/{}", regular::TREE_STRUCTURE, done, total))
                            .size(10.0)
                            .color(theme.text.secondary.to_color32()),
                    );
                });
                let (_, bar_rect) = ui.allocate_space(egui::vec2(ui.available_width() - 8.0, 4.0));
                let painter = ui.painter();
                let bg = theme.surfaces.overlay.to_color32();
                let fg = egui::Color32::from_rgb(100, 200, 120);
                painter.rect_filled(bar_rect, 1.0, bg);
                let mut fill = bar_rect;
                fill.set_width(bar_rect.width() * progress);
                painter.rect_filled(fill, 1.0, fg);
                ui.add_space(4.0);
                ui.ctx().request_repaint();
            }
        }

        // Add Entity overlay
        if state.show_add_overlay {
            let mut entries: Vec<OverlayEntry> = Vec::new();

            // Add SpawnRegistry presets (lights, cameras, etc.)
            if let Some(registry) = world.get_resource::<SpawnRegistry>() {
                entries.extend(registry.iter().map(|p| OverlayEntry {
                    id: p.id,
                    label: p.display_name,
                    icon: p.icon,
                    category: p.category,
                }));
            }

            // Add ShapeRegistry shapes (meshes)
            if let Some(shape_reg) = world.get_resource::<ShapeRegistry>() {
                entries.extend(shape_reg.iter().map(|s| OverlayEntry {
                    id: s.id,
                    label: s.name,
                    icon: s.icon,
                    category: s.category,
                }));
            }

            // Add components from InspectorRegistry (post-processing, rendering, effects, audio)
            if let Some(inspector_reg) = world.get_resource::<InspectorRegistry>() {
                let component_categories = &["rendering", "post_process", "effects", "Audio"];
                for entry in inspector_reg.iter() {
                    if entry.add_fn.is_some() && component_categories.contains(&entry.category) {
                        entries.push(OverlayEntry {
                            id: entry.type_id,
                            label: entry.display_name,
                            icon: entry.icon,
                            category: entry.category,
                        });
                    }
                }
            }

            let ctx = ui.ctx().clone();
            match search_overlay(&ctx, "add_entity_overlay", "Add Entity", &entries, &mut state.add_search, &theme) {
                OverlayAction::Selected(id) => {
                    state.show_add_overlay = false;

                    // Classify id: preset, shape, or component-type.
                    let mut handled = false;
                    if world.get_resource::<SpawnRegistry>()
                        .map_or(false, |r| r.iter().any(|p| p.id == id))
                    {
                        let preset_id = id.clone();
                        commands.push(move |world: &mut World| {
                            renzora_undo::execute(world, UndoContext::Scene, Box::new(SpawnEntityCmd {
                                entity: Entity::PLACEHOLDER,
                                kind: SpawnEntityKind::Preset { id: preset_id },
                            }));
                        });
                        handled = true;
                    }
                    if !handled {
                        if let Some(entry) = world.get_resource::<ShapeRegistry>().and_then(|r| r.get(&id)) {
                            let name = entry.name.to_string();
                            let shape_id = entry.id.to_string();
                            let color = entry.default_color;
                            commands.push(move |world: &mut World| {
                                renzora_undo::execute(world, UndoContext::Scene, Box::new(SpawnShapeCmd {
                                    entity: Entity::PLACEHOLDER,
                                    shape_id, name, position: Vec3::ZERO, color,
                                }));
                            });
                            handled = true;
                        }
                    }
                    if !handled {
                        if let Some(entry) = world.get_resource::<InspectorRegistry>()
                            .and_then(|r| r.iter().find(|e| e.type_id == id))
                        {
                            if entry.add_fn.is_some() {
                                let display_name = entry.display_name.to_string();
                                let type_id = entry.type_id.to_string();
                                commands.push(move |world: &mut World| {
                                    renzora_undo::execute(world, UndoContext::Scene, Box::new(SpawnEntityCmd {
                                        entity: Entity::PLACEHOLDER,
                                        kind: SpawnEntityKind::Component { type_id, display_name },
                                    }));
                                });
                            }
                        }
                    }
                }
                OverlayAction::Closed => {
                    state.show_add_overlay = false;
                }
                OverlayAction::None => {}
            }
        }

        // Build entity tree from ECS
        let nodes = build_entity_tree(world);
        let nodes = if state.search.trim().is_empty() {
            nodes
        } else {
            filter_tree(nodes, state.search.trim())
        };

        if nodes.is_empty() {
            let text_muted = theme.text.muted.to_color32();
            ui.add_space(8.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("No entities in scene.")
                        .size(11.0)
                        .color(text_muted),
                );
            });
            return;
        }

        // Reset drop target each frame
        state.drop_target = None;

        // Render the tree
        let state = &mut *state;
        egui::ScrollArea::vertical()
            .id_salt("hierarchy_tree")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.style_mut().spacing.item_spacing.y = 0.0;
                tree::render_tree(
                    ui,
                    &nodes,
                    state,
                    selection,
                    commands,
                    &theme,
                );
            });

        // Swap visible entity order for next frame's range selection
        std::mem::swap(&mut state.visible_entity_order, &mut state.building_entity_order);

        // Handle drag release → apply reparent
        if !state.drag_entities.is_empty() && !ui.ctx().input(|i| i.pointer.any_down()) {
            if let Some((target, zone)) = state.drop_target.take() {
                let drag_entities = std::mem::take(&mut state.drag_entities);
                commands.push(move |world: &mut World| {
                    use renzora_editor_framework::TreeDropZone;
                    // Capture old parents + all root orders before mutation.
                    let old_parents: Vec<(Entity, Option<Entity>)> = drag_entities.iter()
                        .map(|e| (*e, world.get::<ChildOf>(*e).map(|c| c.parent())))
                        .collect();
                    let mut old_orders: Vec<(Entity, Option<u32>)> = Vec::new();
                    for archetype in world.archetypes().iter() {
                        for arch_entity in archetype.entities() {
                            let e = arch_entity.id();
                            if world.get::<Name>(e).is_none() { continue; }
                            if world.get::<renzora::core::HideInHierarchy>(e).is_some() { continue; }
                            let o = world.get::<HierarchyOrder>(e).map(|h| h.0);
                            old_orders.push((e, o));
                        }
                    }
                    for entity in &drag_entities {
                        if *entity == target {
                            continue;
                        }
                        match zone {
                            TreeDropZone::AsChild => {
                                world.entity_mut(*entity).set_parent_in_place(target);
                                info!("[hierarchy] Moved {:?} as child of {:?}", entity, target);
                            }
                            TreeDropZone::Before | TreeDropZone::After => {
                                let parent = world.get::<ChildOf>(target).map(|c| c.parent());
                                if let Some(p) = parent {
                                    // Read target index BEFORE detaching the dragged entity
                                    let target_idx = world
                                        .get::<Children>(p)
                                        .and_then(|children| {
                                            children.iter().position(|c| c == target)
                                        });
                                    // Now detach
                                    world.entity_mut(*entity).remove_parent_in_place();
                                    if let Some(idx) = target_idx {
                                        // Adjust index: if the dragged entity was before the target
                                        // in the same parent, removing it shifted indices down by 1
                                        let was_sibling_before = world
                                            .get::<Children>(p)
                                            .and_then(|children| {
                                                children.iter().position(|c| c == target)
                                            });
                                        let final_idx = if let Some(new_target_idx) = was_sibling_before {
                                            if matches!(zone, TreeDropZone::After) {
                                                new_target_idx + 1
                                            } else {
                                                new_target_idx
                                            }
                                        } else if matches!(zone, TreeDropZone::After) {
                                            idx + 1
                                        } else {
                                            idx
                                        };
                                        world.entity_mut(p).insert_child(final_idx, *entity);
                                        info!("[hierarchy] Inserted {:?} at index {} under {:?} ({:?} target {:?})",
                                            entity, final_idx, p, zone, target);
                                    } else {
                                        world.entity_mut(*entity).set_parent_in_place(p);
                                        info!("[hierarchy] Fallback: set_parent {:?} under {:?}", entity, p);
                                    }
                                } else {
                                    // Root-level reorder: assign HierarchyOrder values
                                    world.entity_mut(*entity).remove_parent_in_place();

                                    // Collect all root named entities with their current order
                                    let mut roots: Vec<(Entity, u32)> = Vec::new();
                                    for archetype in world.archetypes().iter() {
                                        for arch_entity in archetype.entities() {
                                            let e = arch_entity.id();
                                            if world.get::<Name>(e).is_none() { continue; }
                                            if world.get::<ChildOf>(e).is_some() { continue; }
                                            if world.get::<renzora::core::HideInHierarchy>(e).is_some() { continue; }
                                            let order = world.get::<HierarchyOrder>(e).map(|h| h.0).unwrap_or(u32::MAX);
                                            roots.push((e, order));
                                        }
                                    }
                                    roots.sort_by_key(|&(_, o)| o);

                                    // Remove the dragged entity from roots list
                                    roots.retain(|&(e, _)| e != *entity);

                                    // Find target position and insert
                                    let target_pos = roots.iter().position(|&(e, _)| e == target).unwrap_or(0);
                                    let insert_pos = if matches!(zone, TreeDropZone::After) {
                                        target_pos + 1
                                    } else {
                                        target_pos
                                    };
                                    roots.insert(insert_pos, (*entity, 0));

                                    // Reassign HierarchyOrder to all roots
                                    for (i, &(e, _)) in roots.iter().enumerate() {
                                        world.entity_mut(e).insert(HierarchyOrder(i as u32));
                                    }

                                    let names: Vec<String> = roots.iter().map(|&(e, _)| {
                                        world.get::<Name>(e)
                                            .map(|n| n.as_str().to_string())
                                            .unwrap_or_else(|| format!("{e:?}"))
                                    }).collect();
                                    info!("[hierarchy] Root reorder ({:?} target {:?}): {:?}", zone, target, names);
                                }
                            }
                        }

                        // Log final children order for debugging
                        let parent = world.get::<ChildOf>(*entity).map(|c| c.parent());
                        if let Some(p) = parent {
                            if let Some(children) = world.get::<Children>(p) {
                                let names: Vec<String> = children.into_iter().map(|c| {
                                    world.get::<Name>(*c)
                                        .map(|n| n.as_str().to_string())
                                        .unwrap_or_else(|| format!("{c:?}"))
                                }).collect();
                                info!("[hierarchy] Children order of parent {:?}: {:?}", p, names);
                            }
                        }
                    }
                    // Record parent + order changes for undo.
                    let mut cmds: Vec<Box<dyn UndoCommand>> = Vec::new();
                    for (entity, old_parent) in old_parents {
                        let new_parent = world.get::<ChildOf>(entity).map(|c| c.parent());
                        if old_parent != new_parent {
                            cmds.push(Box::new(ReparentCmd { entity, old_parent, new_parent }));
                        }
                    }
                    for (entity, old) in old_orders {
                        let new = world.get::<HierarchyOrder>(entity).map(|h| h.0);
                        if old != new {
                            cmds.push(Box::new(SetHierarchyOrderCmd { entity, old, new }));
                        }
                    }
                    if !cmds.is_empty() {
                        renzora_undo::record(world, UndoContext::Scene, Box::new(CompoundCmd {
                            label: "Reorder".into(), cmds,
                        }));
                    }
                });
            } else {
                state.drag_entities.clear();
            }
        }

        // Drag tooltip — show target info
        if !state.drag_entities.is_empty() {
            if let Some(pos) = ui.ctx().pointer_latest_pos() {
                let label = if let Some((target_entity, ref zone)) = state.drop_target {
                    // Find target name from the tree
                    let target_name = find_node_name(&nodes, target_entity)
                        .unwrap_or_else(|| format!("{:?}", target_entity));
                    match zone {
                        renzora_editor_framework::TreeDropZone::Before => format!("Move above {}", target_name),
                        renzora_editor_framework::TreeDropZone::After => format!("Move below {}", target_name),
                        renzora_editor_framework::TreeDropZone::AsChild => format!("Move into {}", target_name),
                    }
                } else {
                    let count = state.drag_entities.len();
                    if count == 1 {
                        "Moving 1 entity".to_string()
                    } else {
                        format!("Moving {} entities", count)
                    }
                };
                egui::Area::new(egui::Id::new("hierarchy_drag_tooltip"))
                    .fixed_pos(pos + egui::vec2(12.0, 4.0))
                    .interactable(false)
                    .show(ui.ctx(), |ui| {
                        egui::Frame::popup(ui.style()).show(ui, |ui| {
                            ui.set_max_width(400.0);
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(label).size(11.0));
                            });
                        });
                    });
            }
        }
    }

    fn closable(&self) -> bool {
        true
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Left
    }
}

/// Plugin that registers the `HierarchyPanel` and built-in entity presets.
#[derive(Default)]
pub struct HierarchyPanelPlugin;

impl Plugin for HierarchyPanelPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] HierarchyPanelPlugin");
        app.register_panel(HierarchyPanel::default());

        // Spawn presets are now self-registered by their owning crates:
        // - Bevy types (Empty, lights, camera): renzora_editor_framework::bevy_inspectors
        // - Physics: renzora_physics::inspector (editor feature)
        // - Terrain: renzora_terrain (editor feature)
        // - World Environment/Sun: renzora_level_presets
        app.init_resource::<SpawnRegistry>();
    }
}

fn find_node_name(nodes: &[state::EntityNode], target: Entity) -> Option<String> {
    for node in nodes {
        if node.entity == target {
            return Some(node.name.clone());
        }
        if let Some(name) = find_node_name(&node.children, target) {
            return Some(name);
        }
    }
    None
}

renzora::add!(HierarchyPanelPlugin, Editor);
