//! Widget Palette panel — categorized list of UI widget types to add to the canvas.

use std::sync::RwLock;

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora_editor::{EditorCommands, EditorPanel, EditorSelection, PanelLocation};
use renzora_theme::ThemeManager;

use crate::components::{UiCanvas, UiWidgetType};

/// Palette panel state.
#[derive(Default)]
struct PaletteState {
    search: String,
}

/// Widget library panel — shows available widget types for drag-and-drop creation.
pub struct WidgetPalettePanel {
    state: RwLock<PaletteState>,
}

impl Default for WidgetPalettePanel {
    fn default() -> Self {
        Self {
            state: RwLock::new(PaletteState::default()),
        }
    }
}

/// All widget types available in the palette, grouped by category.
const CATEGORIES: &[(&str, &str, &[UiWidgetType])] = &[
    ("Layout", regular::LAYOUT, &[
        UiWidgetType::Container,
        UiWidgetType::Panel,
        UiWidgetType::ScrollView,
    ]),
    ("Basic", regular::CUBE, &[
        UiWidgetType::Text,
        UiWidgetType::Image,
        UiWidgetType::Button,
    ]),
    ("Input", regular::TEXTBOX, &[
        UiWidgetType::Slider,
        UiWidgetType::Checkbox,
        UiWidgetType::Toggle,
        UiWidgetType::RadioButton,
        UiWidgetType::Dropdown,
        UiWidgetType::TextInput,
    ]),
    ("Display", regular::CHART_BAR, &[
        UiWidgetType::ProgressBar,
        UiWidgetType::HealthBar,
        UiWidgetType::Spinner,
        UiWidgetType::TabBar,
    ]),
    ("Overlay", regular::STACK, &[
        UiWidgetType::Tooltip,
        UiWidgetType::Modal,
        UiWidgetType::DraggableWindow,
    ]),
];

impl EditorPanel for WidgetPalettePanel {
    fn id(&self) -> &str {
        "widget_library"
    }

    fn title(&self) -> &str {
        "Widgets"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::SQUARES_FOUR)
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

        let mut state = self.state.write().unwrap();

        let text_muted = theme.text.muted.to_color32();
        let text_primary = theme.text.primary.to_color32();
        let text_secondary = theme.text.secondary.to_color32();
        let surface = theme.surfaces.panel.to_color32();

        // Search bar
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.add(
                egui::TextEdit::singleline(&mut state.search)
                    .desired_width(ui.available_width() - 8.0)
                    .hint_text(format!("{} Search widgets...", regular::MAGNIFYING_GLASS)),
            );
        });
        ui.add_space(4.0);

        // "Add Canvas" button
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            let btn = ui.add(
                egui::Button::new(
                    egui::RichText::new(format!("{} Add Canvas", regular::PLUS))
                        .size(12.0)
                        .color(text_primary),
                )
                .fill(surface)
                .min_size(egui::vec2(ui.available_width() - 8.0, 28.0)),
            );
            if btn.clicked() {
                commands.push(move |world: &mut World| {
                    let entity = world
                        .spawn((
                            Name::new("UI Canvas"),
                            UiCanvas::default(),
                            Node {
                                width: bevy::ui::Val::Percent(100.0),
                                height: bevy::ui::Val::Percent(100.0),
                                position_type: bevy::ui::PositionType::Absolute,
                                ..default()
                            },
                        ))
                        .id();
                    if let Some(sel) = world.get_resource::<EditorSelection>() {
                        sel.set(Some(entity));
                    }
                });
            }
        });
        ui.add_space(6.0);

        ui.separator();
        ui.add_space(4.0);

        let search_lower = state.search.to_lowercase();

        // Categorized widget list
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.style_mut().spacing.item_spacing.y = 2.0;

                let mut any_visible = false;

                for &(category_name, category_icon, widget_types) in CATEGORIES {
                    // Filter widgets by search
                    let visible: Vec<_> = widget_types
                        .iter()
                        .filter(|wt| {
                            search_lower.is_empty()
                                || wt.label().to_lowercase().contains(&search_lower)
                                || category_name.to_lowercase().contains(&search_lower)
                        })
                        .collect();

                    if visible.is_empty() {
                        continue;
                    }
                    any_visible = true;

                    // Category header
                    let header_id = ui.make_persistent_id(category_name);
                    egui::collapsing_header::CollapsingState::load_with_default_open(
                        ui.ctx(),
                        header_id,
                        true,
                    )
                    .show_header(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(format!("{}  {}", category_icon, category_name))
                                    .size(11.0)
                                    .color(text_secondary),
                            );
                        });
                    })
                    .body(|ui| {
                        for wtype in &visible {
                            let icon = wtype.icon();
                            let label = wtype.label();
                            let wtype_clone = (*wtype).clone();

                            ui.horizontal(|ui| {
                                ui.add_space(8.0);
                                let btn = ui.add(
                                    egui::Button::new(
                                        egui::RichText::new(format!("{}  {}", icon, label))
                                            .size(12.0)
                                            .color(text_primary),
                                    )
                                    .fill(surface)
                                    .min_size(egui::vec2(ui.available_width() - 12.0, 30.0)),
                                );

                                if btn.clicked() {
                                    let wt = wtype_clone.clone();
                                    commands.push(move |world: &mut World| {
                                        crate::spawn::spawn_widget(world, &wt, None);
                                    });
                                }

                                btn.on_hover_text(format!("Click to add a {} widget", label));
                            });
                        }
                    });

                    ui.add_space(2.0);
                }

                // Empty state
                if !any_visible {
                    ui.add_space(8.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("No matching widgets.")
                                .size(11.0)
                                .color(text_muted),
                        );
                    });
                }
            });
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Left
    }
}
