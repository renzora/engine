//! World-based inspector rendering using the component registry
//!
//! This module provides inspector rendering that uses `&mut World` directly,
//! allowing iteration over registered components and calling their inspector functions.

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, CornerRadius, RichText, Vec2};

use crate::component_system::{
    AddComponentPopupState, ComponentCategory, ComponentDefinition, ComponentRegistry,
    get_category_style,
};
use crate::core::{ComponentOrder, DisabledComponents, EditorEntity, SelectionState};
use renzora_theme::Theme;
use crate::ui_api::{renderer::UiRenderer, UiEvent};
use crate::plugin_core::PluginHost;

use super::inspector::{
    set_inspector_theme, render_category, render_category_removable, CategoryHeaderAction,
};

use egui_phosphor::regular::{
    ARROWS_OUT_CARDINAL, PLUS, MAGNIFYING_GLASS, CHECK_CIRCLE,
    TAG, PENCIL_SIMPLE, PUZZLE_PIECE, PAINT_BUCKET, X,
};

use crate::core::EntityLabelColor;

/// Map component category to theme category name
fn category_to_theme_name(category: ComponentCategory) -> &'static str {
    match category {
        ComponentCategory::Rendering => "rendering",
        ComponentCategory::Lighting => "lighting",
        ComponentCategory::Physics => "physics",
        ComponentCategory::Camera => "camera",
        ComponentCategory::Audio => "audio",
        ComponentCategory::Scripting => "scripting",
        ComponentCategory::UI => "ui",
        ComponentCategory::Effects => "effects",
        ComponentCategory::PostProcess => "post_process",
        ComponentCategory::Gameplay => "gameplay",
        ComponentCategory::Animation => "animation",
    }
}

/// Information about a component to render
struct ComponentToRender {
    def: &'static ComponentDefinition,
    theme_category: &'static str,
}

/// Render inspector content using the component registry
///
/// This version takes `&mut World` directly, allowing it to call component
/// `inspector_fn` functions that require World access.
///
/// Returns (ui_events, scene_changed)
pub fn render_inspector_content_world(
    ui: &mut egui::Ui,
    world: &mut World,
    selection: &SelectionState,
    plugin_host: Option<&PluginHost>,
    ui_renderer: &mut UiRenderer,
    add_component_popup: &mut AddComponentPopupState,
    assets: &mut crate::core::AssetBrowserState,
    thumbnail_cache: &mut crate::core::ThumbnailCache,
    theme: &Theme,
) -> (Vec<UiEvent>, bool) {
    // Store theme colors in egui context for inspector components to access
    set_inspector_theme(ui.ctx(), theme);

    let mut scene_changed = false;
    let mut component_to_remove: Option<&'static str> = None;
    let mut component_to_toggle: Option<&'static str> = None;

    // Content area with padding
    egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(6, 4))
        .show(ui, |ui| {
            // Disable drag-to-scroll while the user is reordering components
            let comp_drag_active = ui.ctx()
                .data(|d| d.get_temp::<String>(egui::Id::new("inspector_comp_drag")))
                .filter(|s| !s.is_empty())
                .is_some();
            egui::ScrollArea::vertical()
                .drag_to_scroll(!comp_drag_active)
                .show(ui, |ui| {
                if let Some(selected) = selection.selected_entity {
                    // Get editor entity data (clone to release borrow)
                    let editor_entity_data = world.get::<EditorEntity>(selected).cloned();
                    let disabled_components = world.get::<DisabledComponents>(selected)
                        .cloned()
                        .unwrap_or_default();

                    if let Some(editor_entity) = editor_entity_data {
                        // Show multi-selection indicator if applicable
                        let multi_count = selection.multi_selection.len();
                        if multi_count > 1 {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(format!("{} items selected", multi_count))
                                    .color(Color32::from_rgb(140, 191, 242)));
                            });
                            ui.add_space(4.0);
                        }

                        // Clone editor entity data for editing
                        let mut current_name = editor_entity.name.clone();
                        let mut current_tag = editor_entity.tag.clone();
                        let current_visible = editor_entity.visible;
                        let current_locked = editor_entity.locked;

                        // Entity header with editable name
                        let mut name_changed = false;
                        let mut tag_changed = false;

                        ui.horizontal(|ui| {
                            // Accent bar
                            let (rect, _) = ui.allocate_exact_size(Vec2::new(4.0, 20.0), egui::Sense::hover());
                            ui.painter().rect_filled(rect, 0.0, theme.semantic.accent.to_color32());

                            ui.label(RichText::new(PENCIL_SIMPLE).size(14.0).color(theme.text.muted.to_color32()));

                            // Editable entity name
                            let name_response = ui.add(
                                egui::TextEdit::singleline(&mut current_name)
                                    .desired_width(ui.available_width() - 80.0)
                                    .font(egui::TextStyle::Heading)
                            );
                            if name_response.changed() && !current_name.is_empty() {
                                name_changed = true;
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new(CHECK_CIRCLE).color(theme.semantic.success.to_color32()));
                            });
                        });

                        ui.add_space(4.0);

                        // Tag input
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);
                            ui.label(RichText::new(TAG).size(14.0).color(theme.text.muted.to_color32()));
                            ui.label(RichText::new(crate::locale::t("inspector.tag")).size(12.0).color(theme.text.muted.to_color32()));

                            let tag_response = ui.add(
                                egui::TextEdit::singleline(&mut current_tag)
                                    .desired_width(100.0)
                                    .hint_text("Untagged")
                            );
                            if tag_response.changed() {
                                tag_changed = true;
                            }
                        });

                        // Label color picker
                        {
                            use crate::ui::panels::hierarchy::LABEL_COLORS;
                            let current_label_color = world.get::<EntityLabelColor>(selected).map(|c| c.0);
                            ui.horizontal(|ui| {
                                ui.add_space(8.0);
                                ui.label(RichText::new(PAINT_BUCKET).size(14.0).color(theme.text.muted.to_color32()));
                                ui.label(RichText::new(crate::locale::t("inspector.label")).size(12.0).color(theme.text.muted.to_color32()));
                                if current_label_color.is_some() {
                                    ui.add_space(4.0);
                                    let clear_resp = ui.add(
                                        egui::Button::new(RichText::new(X).size(10.0).color(theme.text.muted.to_color32()))
                                            .frame(false)
                                    );
                                    if clear_resp.on_hover_text("Clear label color").clicked() {
                                        if let Ok(mut entity_ref) = world.get_entity_mut(selected) {
                                            entity_ref.remove::<EntityLabelColor>();
                                        }
                                        scene_changed = true;
                                    }
                                }
                            });
                            for row in [&LABEL_COLORS[..10], &LABEL_COLORS[10..]] {
                                ui.horizontal(|ui| {
                                    ui.add_space(8.0);
                                    ui.spacing_mut().item_spacing = egui::Vec2::new(3.0, 0.0);
                                    for ([r, g, b], name) in row {
                                        let color = egui::Color32::from_rgb(*r, *g, *b);
                                        let is_current = current_label_color == Some([*r, *g, *b]);
                                        let (swatch_rect, swatch_resp) = ui.allocate_exact_size(egui::Vec2::splat(14.0), egui::Sense::click());
                                        ui.painter().rect_filled(swatch_rect.shrink(1.0), 2.0, color);
                                        if is_current {
                                            ui.painter().rect_stroke(swatch_rect, 2.0, egui::Stroke::new(1.5, egui::Color32::WHITE), egui::StrokeKind::Outside);
                                        } else if swatch_resp.hovered() {
                                            ui.painter().rect_stroke(swatch_rect, 2.0, egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 200, 200)), egui::StrokeKind::Outside);
                                        }
                                        if swatch_resp.on_hover_text(*name).clicked() {
                                            if let Ok(mut entity_ref) = world.get_entity_mut(selected) {
                                                entity_ref.insert(EntityLabelColor([*r, *g, *b]));
                                            }
                                            scene_changed = true;
                                        }
                                    }
                                });
                            }
                        }

                        // Apply changes if name or tag changed
                        if name_changed || tag_changed {
                            if let Ok(mut entity_ref) = world.get_entity_mut(selected) {
                                entity_ref.insert(EditorEntity {
                                    name: current_name,
                                    tag: current_tag,
                                    visible: current_visible,
                                    locked: current_locked,
                                });
                            }
                            scene_changed = true;
                        }

                        ui.add_space(8.0);

                        // Collect component info before rendering (to avoid borrow conflicts)
                        let (components_to_render, add_menu_data) = {
                            let component_registry = world.resource::<ComponentRegistry>();

                            // Components present on the entity
                            let present: Vec<ComponentToRender> = component_registry
                                .all()
                                .filter(|def| (def.has_fn)(world, selected))
                                .map(|def| ComponentToRender {
                                    def,
                                    theme_category: category_to_theme_name(def.category),
                                })
                                .collect();

                            // Data for Add Component menu
                            let menu_data: Vec<(ComponentCategory, Vec<&'static ComponentDefinition>)> =
                                ComponentCategory::all_in_order()
                                    .iter()
                                    .filter_map(|category| {
                                        let in_cat: Vec<_> = component_registry
                                            .all()
                                            .filter(|d| d.category == *category)
                                            .collect();
                                        if in_cat.is_empty() {
                                            None
                                        } else {
                                            Some((*category, in_cat))
                                        }
                                    })
                                    .collect();

                            (present, menu_data)
                        };

                        // Sort non-transform components by stored ComponentOrder
                        let component_order = world.get::<ComponentOrder>(selected).cloned().unwrap_or_default();
                        let mut ordered_components: Vec<&ComponentToRender> = components_to_render.iter()
                            .filter(|c| c.def.type_id != "transform")
                            .collect();
                        ordered_components.sort_by_key(|c| {
                            component_order.order.iter().position(|id| id.as_str() == c.def.type_id).unwrap_or(usize::MAX)
                        });

                        // Add Component dropdown button
                        ui.vertical_centered(|ui| {
                            ui.menu_button(
                                RichText::new(format!("{} {}", PLUS, crate::locale::t("inspector.add_component"))).color(theme.text.hyperlink.to_color32()),
                                    |ui| {
                                        ui.set_min_width(220.0);

                                        for (category, defs) in &add_menu_data {
                                            let (accent, _) = get_category_style(*category);
                                            let label = format!("{} {}", category.icon(), category.display_name());

                                            ui.menu_button(RichText::new(label).color(accent), |ui| {
                                                ui.set_min_width(180.0);

                                                for def in defs {
                                                    let comp_key = format!("comp.{}.label", def.type_id);
                                                    let v = crate::locale::t(&comp_key);
                                                    let comp_name = if v == comp_key { def.display_name.to_string() } else { v };
                                                    let item_label = format!("{} {}", def.icon, comp_name);
                                                    if ui.button(RichText::new(item_label)).clicked() {
                                                        add_component_popup.pending_add = Some((selected, def.type_id));
                                                        scene_changed = true;
                                                        ui.close();
                                                    }
                                                }
                                            });
                                        }
                                    },
                                );
                            });

                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(8.0);

                        // Transform section (special handling - always first)
                        let has_transform = world.get::<Transform>(selected).is_some();
                        if has_transform {
                            render_category(
                                ui,
                                ARROWS_OUT_CARDINAL,
                                &crate::locale::t("inspector.category.transform"),
                                "transform",
                                theme,
                                "inspector_transform",
                                true,
                                |ui| {
                                    if let Some(mut transform) = world.get_mut::<Transform>(selected) {
                                        if crate::ui::inspectors::render_transform_inspector(ui, &mut transform) {
                                            scene_changed = true;
                                        }
                                    }
                                },
                            );
                        }

                        // Drag state for component reordering (persisted in egui temp memory as String,
                        // empty string = not dragging, non-empty = type_id of dragged component)
                        let drag_state_id = egui::Id::new("inspector_comp_drag");
                        let dragging_type_id: Option<String> = ui.ctx()
                            .data(|d| d.get_temp::<String>(drag_state_id))
                            .filter(|s| !s.is_empty());
                        let is_drag_active = dragging_type_id.is_some();
                        let drag_released = is_drag_active && ui.ctx().input(|i| !i.pointer.any_down());

                        // Show drag ghost near cursor
                        if let Some(ref dragging_type) = dragging_type_id {
                            if let Some(pos) = ui.ctx().pointer_hover_pos() {
                                let display_name = ordered_components.iter()
                                    .find(|c| c.def.type_id == dragging_type.as_str())
                                    .map(|c| c.def.display_name)
                                    .unwrap_or("Component");
                                let icon = ordered_components.iter()
                                    .find(|c| c.def.type_id == dragging_type.as_str())
                                    .map(|c| c.def.icon)
                                    .unwrap_or("");
                                egui::Area::new(egui::Id::new("comp_drag_ghost"))
                                    .order(egui::Order::Tooltip)
                                    .fixed_pos(pos + egui::Vec2::new(14.0, 8.0))
                                    .show(ui.ctx(), |ui| {
                                        egui::Frame::new()
                                            .fill(theme.panels.category_frame_bg.to_color32())
                                            .corner_radius(egui::CornerRadius::same(4))
                                            .inner_margin(egui::Margin::symmetric(8, 4))
                                            .show(ui, |ui| {
                                                ui.label(egui::RichText::new(format!("{} {}", icon, display_name))
                                                    .size(12.0)
                                                    .color(theme.text.primary.to_color32()));
                                            });
                                    });
                            }
                        }

                        // Render each registered component (sorted, transform already handled above)
                        let mut new_drag_type: Option<String> = None;
                        let mut component_rects: Vec<(String, f32, f32)> = Vec::new();

                        for comp in &ordered_components {
                            let comp_disabled = disabled_components.is_disabled(comp.def.type_id);
                            let is_dragged = dragging_type_id.as_deref() == Some(comp.def.type_id);

                            let top_y = ui.cursor().top();

                            let action = ui.scope(|ui| {
                                if is_dragged {
                                    ui.set_opacity(0.35);
                                }
                                render_component_inspector(
                                    ui,
                                    world,
                                    selected,
                                    comp.def,
                                    comp.theme_category,
                                    theme,
                                    comp_disabled,
                                    &mut scene_changed,
                                )
                            }).inner;

                            let bottom_y = ui.cursor().top();
                            component_rects.push((comp.def.type_id.to_string(), top_y, bottom_y));

                            if action.drag_started {
                                new_drag_type = Some(comp.def.type_id.to_string());
                            }
                            if action.remove_clicked {
                                component_to_remove = Some(comp.def.type_id);
                            }
                            if action.toggle_clicked {
                                component_to_toggle = Some(comp.def.type_id);
                            }
                        }

                        // Show drop indicator and commit reorder on release
                        if let Some(ref dragging_type) = dragging_type_id {
                            if let Some(cursor_pos) = ui.ctx().pointer_hover_pos() {
                                let drop_idx = compute_drop_idx(cursor_pos.y, &component_rects, dragging_type);

                                // Draw drop indicator line
                                let indicator_y = if drop_idx < component_rects.len() {
                                    component_rects[drop_idx].1
                                } else if let Some((_, _, bottom)) = component_rects.last() {
                                    *bottom
                                } else {
                                    0.0
                                };
                                ui.painter().hline(
                                    ui.max_rect().x_range(),
                                    indicator_y,
                                    egui::Stroke::new(2.0, egui::Color32::from_rgb(66, 135, 245)),
                                );

                                // Commit reorder when drag is released
                                if drag_released {
                                    let new_order = compute_new_order(&ordered_components, dragging_type, drop_idx);
                                    if let Ok(mut entity_ref) = world.get_entity_mut(selected) {
                                        entity_ref.insert(ComponentOrder { order: new_order });
                                    }
                                    scene_changed = true;
                                }
                            }
                        }

                        // Update drag state in egui temp memory (String, empty = cleared)
                        if drag_released {
                            ui.ctx().data_mut(|d| d.insert_temp(drag_state_id, String::new()));
                        } else if let Some(ref new_type) = new_drag_type {
                            ui.ctx().data_mut(|d| d.insert_temp(drag_state_id, new_type.clone()));
                        }

                        // Plugin-registered inspector sections (only if plugin_host is provided)
                        if let Some(plugin_host) = plugin_host {
                            let api = plugin_host.api();
                            for (type_id, inspector_def, _plugin_id) in &api.inspectors {
                                if let Some(content) = api.inspector_contents.get(type_id) {
                                    render_category(
                                        ui,
                                        PUZZLE_PIECE,
                                        &inspector_def.label,
                                        "plugin",
                                        theme,
                                        &format!("inspector_plugin_{:?}", type_id),
                                        true,
                                        |ui| {
                                            for widget in content {
                                                ui_renderer.render(ui, widget);
                                            }
                                        },
                                    );
                                }
                            }
                        }

                    }
                } else if let Some(asset_path) = &assets.selected_asset.clone() {
                    add_component_popup.is_open = false;
                    super::inspector::render_asset_inspector(ui, asset_path, thumbnail_cache, theme);
                } else {
                    add_component_popup.is_open = false;

                    ui.add_space(20.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new(MAGNIFYING_GLASS).size(32.0).color(theme.text.disabled.to_color32()));
                        ui.add_space(8.0);
                        ui.label(RichText::new(crate::locale::t("inspector.no_selection")).weak());
                        ui.add_space(8.0);
                        ui.label(RichText::new(crate::locale::t("inspector.no_selection_hint")).weak());
                    });
                }
            });
        });

    // Handle component toggle (deferred to avoid borrow conflicts)
    if let (Some(type_id), Some(selected)) = (component_to_toggle, selection.selected_entity) {
        if let Some(mut dc) = world.get_mut::<DisabledComponents>(selected) {
            dc.toggle(type_id);
        } else {
            let mut dc = DisabledComponents::default();
            dc.toggle(type_id);
            if let Ok(mut entity_ref) = world.get_entity_mut(selected) {
                entity_ref.insert(dc);
            }
        }
        scene_changed = true;
    }

    // Handle component removal (deferred to avoid borrow conflicts)
    if let (Some(type_id), Some(selected)) = (component_to_remove, selection.selected_entity) {
        // Store the removal info for later processing
        add_component_popup.pending_remove = Some((selected, type_id));
        scene_changed = true;
    }

    (ui_renderer.drain_events().collect(), scene_changed)
}

/// Render a single component's inspector section
/// Returns a CategoryHeaderAction indicating which buttons were clicked
fn render_component_inspector(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    def: &'static ComponentDefinition,
    theme_category: &str,
    theme: &Theme,
    is_disabled: bool,
    scene_changed: &mut bool,
) -> CategoryHeaderAction {
    let mut action = CategoryHeaderAction { remove_clicked: false, toggle_clicked: false, drag_started: false };

    // We need to scope the world access properly
    // First, extract the assets we need
    let comp_label_key = format!("comp.{}.label", def.type_id);
    let comp_label_val = crate::locale::t(&comp_label_key);
    let comp_label = if comp_label_val == comp_label_key { def.display_name.to_string() } else { comp_label_val };

    world.resource_scope(|world, mut meshes: Mut<Assets<Mesh>>| {
        world.resource_scope(|world, mut materials: Mut<Assets<StandardMaterial>>| {
            action = render_category_removable(
                ui,
                def.icon,
                &comp_label,
                theme_category,
                theme,
                &format!("inspector_{}", def.type_id),
                true,
                true, // can_remove
                is_disabled,
                |ui| {
                    if (def.inspector_fn)(ui, world, entity, &mut meshes, &mut materials) {
                        *scene_changed = true;
                    }
                },
            );
        });
    });

    action
}

/// Compute the index to insert the dragged component at, based on cursor Y position.
/// Returns an index into `rects` (before that item), or `rects.len()` for end.
fn compute_drop_idx(cursor_y: f32, rects: &[(String, f32, f32)], dragging: &str) -> usize {
    for (idx, (type_id, top, bottom)) in rects.iter().enumerate() {
        if type_id.as_str() == dragging {
            continue; // skip the dragged item itself
        }
        let mid = (top + bottom) / 2.0;
        if cursor_y < mid {
            return idx;
        }
    }
    rects.len()
}

/// Compute the new component order after dropping `dragging` at `drop_idx`.
fn compute_new_order(components: &[&ComponentToRender], dragging: &str, drop_idx: usize) -> Vec<String> {
    let from_idx = components.iter().position(|c| c.def.type_id == dragging).unwrap_or(0);
    let mut new_order: Vec<String> = components.iter().map(|c| c.def.type_id.to_string()).collect();
    new_order.remove(from_idx);
    let insert_idx = if drop_idx > from_idx { drop_idx - 1 } else { drop_idx }.min(new_order.len());
    new_order.insert(insert_idx, dragging.to_string());
    new_order
}
