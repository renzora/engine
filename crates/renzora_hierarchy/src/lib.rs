//! Hierarchy panel — shows the scene entity tree.

mod cache;
mod state;
mod tree;

use std::sync::RwLock;

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora_editor::{
    search_overlay, AppEditorExt, EditorCommands, EditorPanel, EditorSelection,
    HierarchyOrder, InspectorRegistry, OverlayAction, OverlayEntry, PanelLocation,
    SceneStarter, SceneStarterRegistry, SpawnRegistry,
};
use renzora::core::ShapeRegistry;
use renzora_theme::ThemeManager;
use renzora_undo::{self, CompoundCmd, RenameCmd, ReparentCmd, SetHierarchyOrderCmd, SpawnEntityCmd, SpawnEntityKind, SpawnShapeCmd, UndoCommand, UndoContext};

use cache::{HierarchyDirty, HierarchyTreeCache};
use state::{filter_tree, filter_tree_by_type, HierarchyState};

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
        // Drain any external expand requests (e.g. from spawn_widget) so newly
        // spawned children are revealed under their parent.
        if let Some(requests) = world.get_resource::<renzora_editor::HierarchyExpandRequests>() {
            for entity in requests.drain() {
                state.expanded.insert(entity);
            }
        }

        // Consume any keyboard-triggered rename request.
        if let Some(req) = world
            .get_resource::<RenameRequest>()
            .and_then(|r| r.0)
        {
            // The name buffer seed is filled in by the panel when rename
            // actually begins — just set the target entity here.
            state.renaming_entity = Some(req);
            state.rename_buffer = world
                .get::<Name>(req)
                .map(|n| n.as_str().to_string())
                .unwrap_or_default();
            state.rename_focus_set = false;
            commands.push(|w: &mut World| {
                if let Some(mut r) = w.get_resource_mut::<RenameRequest>() {
                    r.0 = None;
                }
            });
        }

        let current: Vec<Entity> = selection.get_all();
        if current != state.last_selection {
            for &entity in &current {
                let mut cur = entity;
                while let Some(child_of) = world.get::<ChildOf>(cur) {
                    let parent = child_of.parent();
                    let named = world.get::<Name>(parent).is_some();
                    let hidden = world.get::<renzora_editor::HideInHierarchy>(parent).is_some();
                    if named && !hidden {
                        state.expanded.insert(parent);
                    }
                    cur = parent;
                }
            }
            // Reveal the new primary selection on this render — `Align::None`
            // means it only nudges the scroll if the row would otherwise be
            // clipped, so visible rows don't cause the viewport to jump.
            state.pending_reveal = current.first().copied();
            state.last_selection = current;
        }

        // Check for CreateNode shortcut (Ctrl+A)
        if world.get_resource::<renzora::core::CreateNodeRequested>().is_some() {
            state.show_add_overlay = true;
            state.add_search.clear();
            // Consume the resource via deferred command
            commands.push(|w: &mut World| { w.remove_resource::<renzora::core::CreateNodeRequested>(); });
        }

        // Collect distinct type entries from the icon registry up front so
        // the popup closure can use them without holding a `world` borrow
        // alongside the `state` borrow.
        let type_filter_entries: Vec<(&'static str, &'static str, [u8; 3])> = {
            let mut out: Vec<(&'static str, &'static str, [u8; 3])> = Vec::new();
            if let Some(registry) = world.get_resource::<renzora_editor::ComponentIconRegistry>() {
                for e in registry.iter() {
                    if !out.iter().any(|(n, _, _)| *n == e.name) {
                        out.push((e.name, e.icon, e.color));
                    }
                }
            }
            out.sort_by_key(|(n, _, _)| *n);
            out
        };

        // Search bar + "Filter" + "Add Entity" buttons
        ui.add_space(4.0);
        let row_height = ui.spacing().interact_size.y;
        let popup_id = egui::Id::new("hierarchy_type_filter_popup");
        let mut filter_resp: Option<egui::Response> = None;
        // Top-align so a shorter Add button sits flush with the top of the
        // search box rather than being vertically centered (which makes it
        // look offset downward).
        ui.with_layout(
            egui::Layout::left_to_right(egui::Align::Min),
            |ui| {
            ui.add_space(4.0);
            let add_width = 80.0;
            let add_height = (row_height - 2.0).max(16.0);
            let filter_width = row_height; // square icon button
            let spacing = ui.spacing().item_spacing.x;
            let right_margin = 8.0;
            let search_width =
                ui.available_width() - add_width - filter_width - spacing * 2.0 - right_margin;

            // Filter-by-type button — left of the search box. Highlighted
            // when a filter is active. Frameless so it reads as an icon, not
            // a button.
            let active = !state.type_filter.is_empty();
            let icon_color = if active {
                theme.semantic.accent.to_color32()
            } else {
                theme.text.secondary.to_color32()
            };
            let filter_btn = egui::Button::new(
                egui::RichText::new(regular::FUNNEL)
                    .color(icon_color)
                    .size(13.0),
            )
            .frame(false);
            let resp = ui
                .add_sized([filter_width, row_height], filter_btn)
                .on_hover_text("Filter by type");
            if resp.clicked() {
                ui.memory_mut(|m| m.toggle_popup(popup_id));
            }
            filter_resp = Some(resp);

            // Search box — `add_sized` keeps its height aligned with the
            // flanking icon buttons; plain `ui.add` would size to the
            // text-edit's intrinsic height and look offset.
            let search_resp = ui.add_sized(
                [search_width, row_height],
                egui::TextEdit::singleline(&mut state.search)
                    .hint_text(format!("{} Search entities...", regular::MAGNIFYING_GLASS)),
            );
            if search_resp.changed() {
                if state.search.is_empty() {
                    // Cleared the filter — bring the selected entity back
                    // into view so the user lands where they left off.
                    state.pending_reveal = selection.get();
                    state.pending_scroll_top = false;
                } else {
                    // Typing — show results from the top.
                    state.pending_scroll_top = true;
                }
            }

            let btn = egui::Button::new(
                egui::RichText::new(format!("{} Add Entity", regular::PLUS))
                    .color(theme.text.secondary.to_color32())
                    .size(11.0),
            );
            if ui.add_sized([add_width, add_height], btn).clicked() {
                state.show_add_overlay = true;
                state.add_search.clear();
            }
        });
        ui.add_space(4.0);

        if let Some(resp) = &filter_resp {
            egui::popup_below_widget(
                ui,
                popup_id,
                resp,
                egui::PopupCloseBehavior::CloseOnClickOutside,
                |ui| {
                    ui.set_min_width(180.0);
                    ui.label(
                        egui::RichText::new("Filter by type")
                            .size(11.0)
                            .color(theme.text.muted.to_color32()),
                    );
                    ui.separator();

                    for (name, icon, [r, g, b]) in &type_filter_entries {
                        let mut on = state.type_filter.contains(name);
                        let color = egui::Color32::from_rgb(*r, *g, *b);
                        let label = egui::RichText::new(format!("{}  {}", icon, name))
                            .color(color)
                            .size(12.0);
                        if ui.checkbox(&mut on, label).changed() {
                            if on {
                                state.type_filter.insert(*name);
                            } else {
                                state.type_filter.remove(*name);
                            }
                            state.pending_scroll_top = true;
                        }
                    }

                    // "Other" — entities that didn't match any registered type.
                    let mut other_on = state.type_filter.contains("__other__");
                    let other_label = egui::RichText::new(format!("{}  Other", regular::CIRCLE))
                        .color(theme.text.secondary.to_color32())
                        .size(12.0);
                    if ui.checkbox(&mut other_on, other_label).changed() {
                        if other_on {
                            state.type_filter.insert("__other__");
                        } else {
                            state.type_filter.remove("__other__");
                        }
                        state.pending_scroll_top = true;
                    }

                    ui.separator();
                    if ui.button("Clear").clicked() {
                        state.type_filter.clear();
                        state.pending_scroll_top = true;
                    }
                },
            );
        }

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
            // Gate entries by active workspace: Scene hides "ui" presets,
            // UI hides everything except "ui" presets. Other layouts show all.
            let active_layout = world
                .get_resource::<renzora_editor::LayoutManager>()
                .map(|lm| lm.active_name().to_string())
                .unwrap_or_default();
            let is_ui_layout = active_layout == "UI";
            let is_scene_layout = active_layout == "Scene";

            let mut entries: Vec<OverlayEntry> = Vec::new();

            // Add SpawnRegistry presets (lights, cameras, ui widgets, etc.)
            if let Some(registry) = world.get_resource::<SpawnRegistry>() {
                for p in registry.iter() {
                    let is_ui_preset = p.category == "ui";
                    let include = if is_ui_layout {
                        is_ui_preset
                    } else if is_scene_layout {
                        !is_ui_preset
                    } else {
                        true
                    };
                    if include {
                        entries.push(OverlayEntry {
                            id: p.id,
                            label: p.display_name,
                            icon: p.icon,
                            category: p.category,
                        });
                    }
                }
            }

            // Add ShapeRegistry shapes (meshes) — not relevant in UI layout.
            if !is_ui_layout {
                if let Some(shape_reg) = world.get_resource::<ShapeRegistry>() {
                    entries.extend(shape_reg.iter().map(|s| OverlayEntry {
                        id: s.id,
                        label: s.name,
                        icon: s.icon,
                        category: s.category,
                    }));
                }
            }

            // Add components from InspectorRegistry (post-processing, rendering,
            // effects, audio) — not relevant in UI layout.
            if !is_ui_layout {
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

        // Batch Rename dialog
        if state.batch_rename_active {
            let count = state.batch_rename_entities.len();
            let mut open = true;
            egui::Window::new("Batch Rename")
                .collapsible(false)
                .resizable(false)
                .open(&mut open)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ui.ctx(), |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Base name:");
                        ui.text_edit_singleline(&mut state.batch_rename_base);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Start at:");
                        ui.add(egui::DragValue::new(&mut state.batch_rename_start).range(0..=9999));
                    });
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "Preview: {}_{:02}, {}_{:02}, … ({} entities)",
                            state.batch_rename_base,
                            state.batch_rename_start,
                            state.batch_rename_base,
                            state.batch_rename_start + 1,
                            count,
                        ))
                        .size(11.0)
                        .color(theme.text.muted.to_color32()),
                    );
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.button("Rename").clicked() && !state.batch_rename_base.is_empty() {
                            let entities = state.batch_rename_entities.clone();
                            let base = state.batch_rename_base.clone();
                            let start = state.batch_rename_start;
                            commands.push(move |world: &mut World| {
                                let mut cmds: Vec<Box<dyn UndoCommand>> = Vec::new();
                                for (i, entity) in entities.iter().enumerate() {
                                    let old = world
                                        .get::<Name>(*entity)
                                        .map(|n| n.as_str().to_string())
                                        .unwrap_or_default();
                                    let new = format!("{}_{:02}", base, start as usize + i);
                                    cmds.push(Box::new(RenameCmd { entity: *entity, old, new }));
                                }
                                renzora_undo::execute(
                                    world,
                                    UndoContext::Scene,
                                    Box::new(CompoundCmd { label: "Batch Rename".to_string(), cmds }),
                                );
                            });
                            state.batch_rename_active = false;
                        }
                        if ui.button("Cancel").clicked() {
                            state.batch_rename_active = false;
                        }
                    });
                });
            if !open {
                state.batch_rename_active = false;
            }
        }

        // Read the cached entity tree. Rebuilt by `update_hierarchy_cache`
        // (Update schedule) only when the tree actually changed. When no
        // search or type filter is active we read directly from the cache
        // (no clone); otherwise we create a filtered owned copy.
        let cache_ref = world.get_resource::<HierarchyTreeCache>();
        let search_active = !state.search.trim().is_empty();
        let type_filter_active = !state.type_filter.is_empty();
        let filtered_nodes: Option<Vec<state::EntityNode>> = if !search_active && !type_filter_active {
            None
        } else {
            cache_ref.map(|c| {
                let mut nodes = c.nodes.clone();
                if type_filter_active {
                    nodes = filter_tree_by_type(nodes, &state.type_filter);
                }
                if search_active {
                    nodes = filter_tree(nodes, state.search.trim());
                }
                nodes
            })
        };
        let nodes: &[state::EntityNode] = match filtered_nodes.as_ref() {
            Some(v) => v.as_slice(),
            None => cache_ref.map_or(&[][..], |c| c.nodes.as_slice()),
        };

        if nodes.is_empty() {
            render_scene_starter_picker(ui, world, &theme);
            return;
        }

        // Reset drop target each frame
        state.drop_target = None;

        // Render the tree
        let state = &mut *state;
        let mut tree_scroll = egui::ScrollArea::vertical()
            .id_salt("hierarchy_tree")
            .auto_shrink([false, false]);
        if state.pending_scroll_top {
            tree_scroll = tree_scroll.vertical_scroll_offset(0.0);
            state.pending_scroll_top = false;
        }
        tree_scroll
            .show(ui, |ui| {
                ui.style_mut().spacing.item_spacing.y = 0.0;
                tree::render_tree(
                    ui,
                    nodes,
                    state,
                    selection,
                    commands,
                    &theme,
                );

                // Sticky-parent overlay: when the user scrolls down inside an
                // expanded subtree, paint the chain of expanded ancestors as
                // pinned headers at the top of the viewport so the user
                // always knows which group they're inside.
                paint_sticky_parents(ui, state, &theme);

                // Marquee drag selection — fill remaining visible space below
                // the tree rows so the user can click/drag from the empty area.
                let content_bottom = ui.cursor().top();
                let visible_bottom = ui.clip_rect().max.y;
                let remaining = (visible_bottom - content_bottom).max(40.0);
                let (_, empty_resp) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), remaining),
                    egui::Sense::click_and_drag(),
                );

                if empty_resp.clicked() && state.marquee_origin.is_none() {
                    selection.clear();
                }

                if empty_resp.drag_started() {
                    if let Some(pos) = ui.ctx().pointer_interact_pos() {
                        state.marquee_origin = Some(pos);
                    }
                }

                if let Some(origin) = state.marquee_origin {
                    if ui.ctx().input(|i| i.pointer.any_down()) {
                        if let Some(current) = ui.ctx().pointer_latest_pos() {
                            let marquee_rect = egui::Rect::from_two_pos(origin, current);

                            let mut selected: Vec<Entity> = Vec::new();
                            for &(entity, row_rect) in &state.row_rects {
                                if marquee_rect.intersects(row_rect) {
                                    selected.push(entity);
                                }
                            }

                            if ui.ctx().input(|i| i.modifiers.ctrl || i.modifiers.command) {
                                let mut existing = selection.get_all();
                                for e in &selected {
                                    if !existing.contains(e) {
                                        existing.push(*e);
                                    }
                                }
                                if existing != selection.get_all() {
                                    selection.set_multiple(existing);
                                }
                            } else if selected != selection.get_all() {
                                selection.set_multiple(selected);
                            }

                            let accent = theme.semantic.accent.to_color32();
                            let fill = egui::Color32::from_rgba_unmultiplied(
                                accent.r(), accent.g(), accent.b(), 30,
                            );
                            let fg = ui.ctx().layer_painter(egui::LayerId::new(
                                egui::Order::Foreground,
                                egui::Id::new("hierarchy_marquee"),
                            ));
                            fg.rect_filled(marquee_rect, 2.0, fill);
                            fg.rect_stroke(
                                marquee_rect,
                                2.0,
                                egui::Stroke::new(1.0, accent),
                                egui::StrokeKind::Inside,
                            );

                            ui.ctx().request_repaint();
                        }
                    } else {
                        state.marquee_origin = None;
                    }
                }
            });

        // Swap visible entity order for next frame's range selection
        std::mem::swap(&mut state.visible_entity_order, &mut state.building_entity_order);

        // Handle drag release → apply reparent
        if !state.drag_entities.is_empty() && !ui.ctx().input(|i| i.pointer.any_down()) {
            if let Some((target, zone)) = state.drop_target.take() {
                let drag_entities = std::mem::take(&mut state.drag_entities);
                commands.push(move |world: &mut World| {
                    use renzora_editor::TreeDropZone;
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
                    let target_name = find_node_name(nodes, target_entity)
                        .unwrap_or_else(|| format!("{:?}", target_entity));
                    match zone {
                        renzora_editor::TreeDropZone::Before => format!("Move above {}", target_name),
                        renzora_editor::TreeDropZone::After => format!("Move below {}", target_name),
                        renzora_editor::TreeDropZone::AsChild => format!("Move into {}", target_name),
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

        // ── Scene-instance drop: drag a `.ron` from the asset panel onto
        //    the hierarchy to spawn a SceneInstance at the scene root.
        let panel_rect = ui.max_rect();
        if let Some(payload) = world.get_resource::<renzora_ui::asset_drag::AssetDragPayload>() {
            if payload.is_detached && payload.matches_extensions(&["ron"]) {
                let pointer = ui.ctx().pointer_latest_pos();
                let pointer_in_panel = pointer.map_or(false, |p| panel_rect.contains(p));
                if pointer_in_panel {
                    // Highlight the panel edge to show it's a valid drop target.
                    ui.painter().rect_stroke(
                        panel_rect,
                        4.0,
                        egui::Stroke::new(2.0, theme.semantic.accent.to_color32()),
                        egui::StrokeKind::Inside,
                    );
                    // On release, spawn the instance.
                    if !ui.ctx().input(|i| i.pointer.any_down()) {
                        let path = payload.path.clone();
                        commands.push(move |world: &mut World| {
                            let host_abs = world
                                .get_resource::<renzora::core::CurrentProject>()
                                .and_then(|p| {
                                    world.get_resource::<renzora_ui::DocumentTabState>()
                                        .and_then(|t| t.tabs.get(t.active_tab)
                                            .and_then(|tab| tab.scene_path.clone()))
                                        .map(|rel| p.resolve_path(&rel))
                                });
                            if let (Some(host_abs), Some(project_root)) = (
                                host_abs,
                                world.get_resource::<renzora::core::CurrentProject>()
                                    .map(|p| p.path.clone()),
                            ) {
                                let mut cache = world
                                    .remove_resource::<renzora_engine::scene_io::SceneReferenceCache>()
                                    .unwrap_or_default();
                                let cycle = renzora_engine::scene_io::would_create_reference_cycle(
                                    &mut cache, &project_root, &host_abs, &path,
                                );
                                world.insert_resource(cache);
                                if cycle {
                                    if let Some(mut toasts) = world.get_resource_mut::<renzora_ui::Toasts>() {
                                        toasts.warning("You cannot add a scene to itself");
                                    }
                                    return;
                                }
                            }
                            let entity = renzora_engine::scene_io::spawn_scene_instance(
                                world,
                                &path,
                                None,
                                Transform::default(),
                            );
                            if let Some(entity) = entity {
                                if let Some(sel) = world.get_resource::<EditorSelection>() {
                                    sel.set(Some(entity));
                                }
                            }
                        });
                    }
                }
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
        app.init_resource::<RenameRequest>();
        app.init_resource::<HierarchyTreeCache>();
        app.init_resource::<HierarchyDirty>();
        app.add_systems(bevy::prelude::Update, (
            detect_selection_keybindings,
            cache::mark_hierarchy_dirty,
            cache::update_hierarchy_cache.after(cache::mark_hierarchy_dirty),
        ));

        // Spawn presets are now self-registered by their owning crates:
        // - Bevy types (Empty, lights, camera): renzora_editor::bevy_inspectors
        // - Physics: renzora_physics::inspector (editor feature)
        // - Terrain: renzora_terrain (editor feature)
        // - World Environment/Sun: renzora_level_presets
        app.init_resource::<SpawnRegistry>();

        // Scene starters shown on the empty-hierarchy picker. Feature-specific
        // starters (Environment, UI Canvas, Physics Arena) are registered by
        // their owning crates.
        app.register_scene_starter(SceneStarter {
            id: "empty_scene",
            title: "Empty Scene",
            description: "Start with just a camera",
            icon: egui_phosphor::regular::CIRCLE_DASHED,
            spawn_fn: |world: &mut World| {
                use renzora::core::SceneCamera;
                world.spawn((
                    Name::new("Camera"),
                    SceneCamera,
                    Camera3d::default(),
                    Camera { is_active: false, ..default() },
                    Transform::from_xyz(5.0, 4.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
                ));
            },
        });
    }
}

/// Paint pinned "sticky" headers for the stack of expanded ancestors above
/// the topmost visible row. Mirrors how a file-explorer keeps the current
/// folder path visible as you scroll deeper into a tree.
///
/// Strategy: walk the row metadata captured during `tree::render_tree`,
/// find the topmost row currently in the viewport, then walk back through
/// rows with strictly decreasing depth. Each ancestor whose own row has
/// scrolled above the clip rect's top becomes a sticky header, painted at
/// the top of the viewport via a foreground layer so it covers the live
/// rows below.
fn paint_sticky_parents(
    ui: &mut egui::Ui,
    state: &state::HierarchyState,
    theme: &renzora_theme::Theme,
) {
    use egui_phosphor::regular::{CARET_DOWN, CARET_RIGHT};
    use renzora_ui::widgets::tree::{INDENT_SIZE, ROW_HEIGHT};

    if state.row_meta.is_empty() {
        return;
    }
    let clip = ui.clip_rect();
    let clip_top = clip.min.y;

    // Topmost visible row index — first row whose bottom is below clip_top.
    let Some(top_idx) = state
        .row_meta
        .iter()
        .position(|m| m.rect.max.y > clip_top)
    else {
        return;
    };

    // Walk back through rows with strictly decreasing depth — those are the
    // ancestors of the topmost visible row in the rendered tree.
    let mut sticky: Vec<&state::StickyRowMeta> = Vec::new();
    let mut current_depth = state.row_meta[top_idx].depth;
    if current_depth == 0 {
        // Top row is a root — nothing to pin.
        return;
    }
    for i in (0..top_idx).rev() {
        let meta = &state.row_meta[i];
        if meta.depth < current_depth {
            // Only pin parents whose own row has scrolled off the top.
            if meta.rect.max.y <= clip_top {
                sticky.push(meta);
            }
            current_depth = meta.depth;
            if meta.depth == 0 {
                break;
            }
        }
    }
    if sticky.is_empty() {
        return;
    }
    // Reverse so the outermost ancestor sits on top.
    sticky.reverse();

    let layer_id = egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("hierarchy_sticky"),
    );
    let painter = ui.ctx().layer_painter(layer_id);

    let row_w = clip.width();
    let bg = theme.surfaces.panel.to_color32();
    let border = egui::Color32::from_rgba_unmultiplied(0, 0, 0, 60);

    for (i, meta) in sticky.iter().enumerate() {
        let rect = egui::Rect::from_min_size(
            egui::pos2(clip.min.x, clip_top + i as f32 * ROW_HEIGHT),
            egui::vec2(row_w, ROW_HEIGHT),
        );

        // Click target — scrolls the live row into view so the user lands on
        // the actual parent in the tree. Allocated before painting so the
        // hovered/active state can tint the background, and registered after
        // tree rendering so this overlay wins overlap arbitration.
        let resp = ui.interact(
            rect,
            egui::Id::new(("hierarchy_sticky_row", meta.entity)),
            egui::Sense::click(),
        );
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if resp.clicked() {
            ui.scroll_to_rect_animation(
                meta.rect,
                Some(egui::Align::TOP),
                egui::style::ScrollAnimation::none(),
            );
        }

        // Solid background — overdraws the live row underneath.
        painter.rect_filled(rect, 0.0, bg);
        if resp.hovered() {
            let [r, g, b, _] = theme.widgets.hovered_bg.to_color32().to_array();
            painter.rect_filled(
                rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(r, g, b, 50),
            );
        }

        // Optional left stripe if the row had a label color, mirroring the
        // tree row's accent strip.
        if let Some([r, g, b]) = meta.label_color {
            let stripe = egui::Rect::from_min_max(
                rect.min,
                egui::pos2(rect.min.x + 3.0, rect.max.y),
            );
            painter.rect_filled(stripe, 0.0, egui::Color32::from_rgb(r, g, b));
        }

        let base_x = rect.min.x + 4.0;
        let center_y = rect.center().y;
        let content_x = base_x + (meta.depth as f32 * INDENT_SIZE);

        // Caret (parents always have children, but check anyway).
        if meta.has_children {
            let caret = if meta.is_expanded { CARET_DOWN } else { CARET_RIGHT };
            let caret_color = egui::Color32::from_rgb(150, 150, 160);
            painter.text(
                egui::pos2(content_x + 8.0, center_y),
                egui::Align2::CENTER_CENTER,
                caret,
                egui::FontId::proportional(12.0),
                caret_color,
            );
        }

        // Icon
        let icon_x = content_x + 16.0;
        painter.text(
            egui::pos2(icon_x + 6.0, center_y),
            egui::Align2::LEFT_CENTER,
            meta.icon,
            egui::FontId::proportional(14.0),
            meta.icon_color,
        );

        // Label
        let label_x = icon_x + 22.0 + 4.0;
        painter.text(
            egui::pos2(label_x, center_y),
            egui::Align2::LEFT_CENTER,
            &meta.name,
            egui::FontId::proportional(13.0),
            theme.text.primary.to_color32(),
        );
    }

    // Hairline at the bottom of the sticky stack to separate from the live
    // rows scrolling underneath.
    let last_y = clip_top + sticky.len() as f32 * ROW_HEIGHT;
    painter.line_segment(
        [
            egui::pos2(clip.min.x, last_y),
            egui::pos2(clip.max.x, last_y),
        ],
        egui::Stroke::new(1.0, border),
    );
}

/// A request from the keybinding system to start renaming an entity in the
/// hierarchy panel. Consumed by the panel UI next frame.
#[derive(Resource, Default)]
pub struct RenameRequest(pub Option<Entity>);

/// Watches the new editor keybindings (SelectAll / Rename / Hide / Isolate)
/// and applies their effects.
fn detect_selection_keybindings(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Res<renzora::core::keybindings::KeyBindings>,
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    input_focus: Res<renzora::core::InputFocusState>,
    selection: Res<EditorSelection>,
    entities_q: Query<(Entity, Option<&bevy::prelude::Name>, Option<&renzora::core::HideInHierarchy>)>,
    mut vis_q: Query<&mut bevy::prelude::Visibility>,
    mut rename_req: ResMut<RenameRequest>,
) {
    use renzora::core::keybindings::EditorAction;
    if play_mode.as_ref().map_or(false, |pm| pm.is_in_play_mode()) { return; }
    if keybindings.rebinding.is_some() { return; }
    if input_focus.egui_wants_keyboard { return; }

    // SelectAll: pick every named, non-hidden-in-hierarchy entity.
    if keybindings.just_pressed(EditorAction::SelectAll, &keyboard) {
        let mut all = Vec::new();
        for (e, name, hide) in entities_q.iter() {
            if name.is_some() && hide.is_none() {
                all.push(e);
            }
        }
        selection.set_multiple(all);
    }

    // Rename: start rename on the current primary selection.
    if keybindings.just_pressed(EditorAction::Rename, &keyboard) {
        if let Some(e) = selection.get() {
            rename_req.0 = Some(e);
        }
    }

    // HideSelected: toggle Visibility::Hidden on every selected entity.
    if keybindings.just_pressed(EditorAction::HideSelected, &keyboard) {
        for e in selection.get_all() {
            if let Ok(mut v) = vis_q.get_mut(e) {
                *v = match *v {
                    Visibility::Hidden => Visibility::Visible,
                    _ => Visibility::Hidden,
                };
            }
        }
    }

    // IsolateSelected: hide everything except the current selection (and its
    // ancestors, so the tree stays navigable).
    if keybindings.just_pressed(EditorAction::IsolateSelected, &keyboard) {
        let sel: std::collections::HashSet<Entity> =
            selection.get_all().into_iter().collect();
        if !sel.is_empty() {
            for (e, name, hide) in entities_q.iter() {
                if name.is_none() || hide.is_some() {
                    continue;
                }
                if let Ok(mut v) = vis_q.get_mut(e) {
                    *v = if sel.contains(&e) {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    };
                }
            }
        }
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

/// Empty-state UI for the hierarchy: a "New" picker with one clickable card
/// per registered [`SceneStarter`]. Each card invokes that starter's
/// `spawn_fn` via `EditorCommands`.
fn render_scene_starter_picker(
    ui: &mut egui::Ui,
    world: &World,
    theme: &renzora_theme::Theme,
) {
    let registry = match world.get_resource::<SceneStarterRegistry>() {
        Some(r) => r,
        None => return,
    };
    let starters: Vec<_> = registry.iter().collect();

    let text_primary = theme.text.primary.to_color32();
    let text_muted = theme.text.muted.to_color32();
    let border = theme.widgets.border.to_color32();
    let row_bg = theme.panels.item_bg.to_color32();
    // Simple hover: brighten item_bg slightly.
    let row_hover = row_bg.gamma_multiply(1.4);

    ui.add_space(16.0);
    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new("This scene is empty")
                .size(14.0)
                .strong()
                .color(text_primary),
        );
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new("Pick a starter, or just add entities manually.")
                .size(11.0)
                .color(text_muted),
        );
    });
    ui.add_space(14.0);

    if starters.is_empty() {
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new("No starters registered.")
                    .size(11.0)
                    .color(text_muted),
            );
        });
        return;
    }

    egui::ScrollArea::vertical()
        .id_salt("scene_starter_picker")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.style_mut().spacing.item_spacing.y = 6.0;
            for starter in &starters {
                render_starter_card(ui, starter, row_bg, row_hover, border, text_primary, text_muted, world);
            }
        });
}

fn render_starter_card(
    ui: &mut egui::Ui,
    starter: &SceneStarter,
    bg: egui::Color32,
    bg_hover: egui::Color32,
    border: egui::Color32,
    text_primary: egui::Color32,
    text_muted: egui::Color32,
    world: &World,
) {
    let margin = 6.0;
    let width = ui.available_width() - margin * 2.0;

    ui.horizontal(|ui| {
        ui.add_space(margin);
        let (rect, resp) = ui.allocate_exact_size(
            egui::vec2(width, 52.0),
            egui::Sense::click(),
        );
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let fill = if resp.hovered() { bg_hover } else { bg };
        ui.painter().rect_filled(rect, egui::CornerRadius::same(6), fill);
        ui.painter().rect_stroke(
            rect,
            egui::CornerRadius::same(6),
            egui::Stroke::new(1.0, border),
            egui::StrokeKind::Inside,
        );

        let icon_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left() + 12.0, rect.top() + 10.0),
            egui::vec2(32.0, 32.0),
        );
        ui.painter().text(
            icon_rect.center(),
            egui::Align2::CENTER_CENTER,
            starter.icon,
            egui::FontId::proportional(22.0),
            text_primary,
        );

        let text_x = icon_rect.right() + 10.0;
        ui.painter().text(
            egui::pos2(text_x, rect.top() + 10.0),
            egui::Align2::LEFT_TOP,
            starter.title,
            egui::FontId::proportional(13.0),
            text_primary,
        );
        ui.painter().text(
            egui::pos2(text_x, rect.top() + 28.0),
            egui::Align2::LEFT_TOP,
            starter.description,
            egui::FontId::proportional(10.5),
            text_muted,
        );

        if resp.clicked() {
            if let Some(cmds) = world.get_resource::<EditorCommands>() {
                let id = starter.id;
                cmds.push(move |world: &mut World| {
                    let spawn = world
                        .get_resource::<SceneStarterRegistry>()
                        .and_then(|r| r.get(id))
                        .map(|s| s.spawn_fn);
                    if let Some(f) = spawn {
                        f(world);
                    }
                });
            }
        }
    });
}

