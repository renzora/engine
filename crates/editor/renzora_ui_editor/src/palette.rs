//! Widget Palette panel — draggable list of UI widget types to add to the canvas.

use std::sync::RwLock;

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora_editor::{EditorCommands, EditorPanel, EditorSelection, PanelLocation};
use renzora_theme::ThemeManager;

use crate::components::{UiCanvas, UiWidget, UiWidgetType};

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

/// All widget types available in the palette.
const WIDGET_TYPES: &[UiWidgetType] = &[
    UiWidgetType::Container,
    UiWidgetType::Panel,
    UiWidgetType::Text,
    UiWidgetType::Image,
    UiWidgetType::Button,
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
        let _accent = theme.semantic.accent.to_color32();
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

        // Widget type cards
        let search_lower = state.search.to_lowercase();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.style_mut().spacing.item_spacing.y = 2.0;

                for wtype in WIDGET_TYPES {
                    let label = wtype.label();
                    if !search_lower.is_empty() && !label.to_lowercase().contains(&search_lower) {
                        continue;
                    }

                    let icon = wtype.icon();
                    let wtype_clone = wtype.clone();

                    ui.horizontal(|ui| {
                        ui.add_space(4.0);
                        let btn = ui.add(
                            egui::Button::new(
                                egui::RichText::new(format!("{}  {}", icon, label))
                                    .size(12.0)
                                    .color(text_primary),
                            )
                            .fill(surface)
                            .min_size(egui::vec2(ui.available_width() - 8.0, 32.0)),
                        );

                        if btn.clicked() {
                            let wt = wtype_clone.clone();
                            commands.push(move |world: &mut World| {
                                spawn_widget(world, &wt, None);
                            });
                        }

                        // Tooltip
                        btn.on_hover_text(format!("Click to add a {} widget", label));
                    });
                }

                // Empty state
                if WIDGET_TYPES.iter().all(|w| {
                    !search_lower.is_empty() && !w.label().to_lowercase().contains(&search_lower)
                }) {
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

/// Spawn a UI widget entity and optionally parent it to a canvas.
pub fn spawn_widget(world: &mut World, widget_type: &UiWidgetType, parent: Option<Entity>) {
    let name = widget_type.label();

    // Find first canvas before spawning (need &mut World for query)
    let first_canvas = {
        let mut canvas_query = world.query_filtered::<Entity, With<UiCanvas>>();
        canvas_query.iter(world).next()
    };

    let entity_cmds = match widget_type {
        UiWidgetType::Container => {
            world.spawn((
                Name::new(name),
                UiWidget {
                    widget_type: widget_type.clone(),
                    locked: false,
                },
                Node {
                    width: bevy::ui::Val::Px(200.0),
                    height: bevy::ui::Val::Px(100.0),
                    ..default()
                },
            ))
        }
        UiWidgetType::Panel => {
            world.spawn((
                Name::new(name),
                UiWidget {
                    widget_type: widget_type.clone(),
                    locked: false,
                },
                Node {
                    width: bevy::ui::Val::Px(300.0),
                    height: bevy::ui::Val::Px(200.0),
                    padding: bevy::ui::UiRect::all(bevy::ui::Val::Px(8.0)),
                    border_radius: BorderRadius::all(bevy::ui::Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.15, 0.15, 0.18, 0.9)),
                BorderColor::all(Color::srgba(0.3, 0.3, 0.35, 1.0)),
            ))
        }
        UiWidgetType::Text => {
            world.spawn((
                Name::new(name),
                UiWidget {
                    widget_type: widget_type.clone(),
                    locked: false,
                },
                Node::default(),
                bevy::ui::widget::Text::new("Hello World"),
                TextColor(Color::WHITE),
                TextFont {
                    font_size: 16.0,
                    ..default()
                },
            ))
        }
        UiWidgetType::Image => {
            world.spawn((
                Name::new(name),
                UiWidget {
                    widget_type: widget_type.clone(),
                    locked: false,
                },
                Node {
                    width: bevy::ui::Val::Px(128.0),
                    height: bevy::ui::Val::Px(128.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.3, 0.3, 0.3, 1.0)),
            ))
        }
        UiWidgetType::Button => {
            world.spawn((
                Name::new(name),
                UiWidget {
                    widget_type: widget_type.clone(),
                    locked: false,
                },
                Node {
                    width: bevy::ui::Val::Px(150.0),
                    height: bevy::ui::Val::Px(40.0),
                    justify_content: bevy::ui::JustifyContent::Center,
                    align_items: bevy::ui::AlignItems::Center,
                    border_radius: BorderRadius::all(bevy::ui::Val::Px(4.0)),
                    ..default()
                },
                Button,
                BackgroundColor(Color::srgba(0.25, 0.25, 0.8, 1.0)),
                BorderColor::all(Color::srgba(0.4, 0.4, 0.9, 1.0)),
            ))
        }
    };

    let entity = entity_cmds.id();

    // Parent to canvas if specified
    if let Some(parent_entity) = parent {
        world.entity_mut(entity).set_parent_in_place(parent_entity);
    } else if let Some(canvas) = first_canvas {
        world.entity_mut(entity).set_parent_in_place(canvas);
    }

    if let Some(sel) = world.get_resource::<EditorSelection>() {
        sel.set(Some(entity));
    }
}
