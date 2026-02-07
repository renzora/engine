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
use crate::core::{DisabledComponents, EditorEntity, SelectionState};
use crate::theming::Theme;
use crate::ui_api::{renderer::UiRenderer, UiEvent};
use crate::plugin_core::PluginHost;

use super::inspector::{
    set_inspector_theme, render_category, render_category_removable, CategoryHeaderAction,
};

use egui_phosphor::regular::{
    ARROWS_OUT_CARDINAL, PLUS, MAGNIFYING_GLASS, CHECK_CIRCLE,
    TAG, PENCIL_SIMPLE, PUZZLE_PIECE,
};

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
            egui::ScrollArea::vertical().show(ui, |ui| {
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
                            ui.label(RichText::new("Tag").size(12.0).color(theme.text.muted.to_color32()));

                            let tag_response = ui.add(
                                egui::TextEdit::singleline(&mut current_tag)
                                    .desired_width(100.0)
                                    .hint_text("Untagged")
                            );
                            if tag_response.changed() {
                                tag_changed = true;
                            }
                        });

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

                        // Add Component dropdown button
                        egui::Frame::default()
                            .fill(theme.widgets.inactive_bg.to_color32())
                            .corner_radius(CornerRadius::same(4))
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width() - 12.0);
                                ui.menu_button(
                                    RichText::new(format!("{} Add Component", PLUS)).color(theme.text.hyperlink.to_color32()),
                                    |ui| {
                                        ui.set_min_width(220.0);

                                        for (category, defs) in &add_menu_data {
                                            let (accent, _) = get_category_style(*category);
                                            let label = format!("{} {}", category.icon(), category.display_name());

                                            ui.menu_button(RichText::new(label).color(accent), |ui| {
                                                ui.set_min_width(180.0);

                                                for def in defs {
                                                    let item_label = format!("{} {}", def.icon, def.display_name);
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
                                "Transform",
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

                        // Render each registered component
                        for comp in &components_to_render {
                            // Skip Transform - handled above
                            if comp.def.type_id == "transform" {
                                continue;
                            }

                            let comp_disabled = disabled_components.is_disabled(comp.def.type_id);

                            // Use a closure that captures world mutably for each component
                            let action = render_component_inspector(
                                ui,
                                world,
                                selected,
                                comp.def,
                                comp.theme_category,
                                theme,
                                comp_disabled,
                                &mut scene_changed,
                            );

                            if action.remove_clicked {
                                component_to_remove = Some(comp.def.type_id);
                            }
                            if action.toggle_clicked {
                                component_to_toggle = Some(comp.def.type_id);
                            }
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
                        ui.label(RichText::new("No Selection").weak());
                        ui.add_space(8.0);
                        ui.label(RichText::new("Select an entity in the Hierarchy\nor click on an object in the Scene.").weak());
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
    let mut action = CategoryHeaderAction { remove_clicked: false, toggle_clicked: false };

    // We need to scope the world access properly
    // First, extract the assets we need
    world.resource_scope(|world, mut meshes: Mut<Assets<Mesh>>| {
        world.resource_scope(|world, mut materials: Mut<Assets<StandardMaterial>>| {
            action = render_category_removable(
                ui,
                def.icon,
                def.display_name,
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
