//! Widget Palette panel — tile grid of UI widget types, matching the shapes library design.

use std::sync::RwLock;

use bevy::prelude::*;
use bevy_egui::egui::{self, CursorIcon, Vec2};
use egui_phosphor::regular;
use renzora_editor_framework::{EditorCommands, EditorPanel, EditorSelection, PanelLocation};
use renzora_theme::ThemeManager;

use crate::components::{UiCanvas, UiWidgetType};

/// Active widget drag state — inserted as a resource when a palette drag begins,
/// removed on drop or cancel.
#[derive(Resource, Clone, Debug)]
pub struct WidgetDragPayload {
    /// The widget type being dragged.
    pub widget_type: UiWidgetType,
    /// Screen position where the drag started.
    pub origin: egui::Pos2,
    /// True once the pointer moves >5px from origin.
    pub is_detached: bool,
}

/// Draw a floating ghost showing the dragged widget type following the cursor.
pub fn draw_widget_drag_ghost(
    ctx: &egui::Context,
    payload: &WidgetDragPayload,
    pointer: egui::Pos2,
    theme: &renzora_theme::Theme,
) {
    let offset = Vec2::new(14.0, -10.0);
    let pos = pointer + offset;

    let accent = theme.semantic.accent.to_color32();

    egui::Area::new(egui::Id::new("widget_drag_ghost"))
        .fixed_pos(pos)
        .order(egui::Order::Tooltip)
        .interactable(false)
        .show(ctx, |ui| {
            let frame = egui::Frame::NONE
                .fill(theme.surfaces.panel.to_color32())
                .stroke(egui::Stroke::new(1.5, accent))
                .inner_margin(egui::Margin::symmetric(8, 5))
                .corner_radius(egui::CornerRadius::same(6));
            frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.label(
                        egui::RichText::new(payload.widget_type.icon())
                            .font(egui::FontId::proportional(14.0))
                            .color(accent),
                    );
                    ui.label(
                        egui::RichText::new(payload.widget_type.label())
                            .font(egui::FontId::proportional(11.0))
                            .color(theme.text.primary.to_color32()),
                    );
                });
            });
        });
}

/// Palette panel state.
#[derive(Default)]
struct PaletteState {
    search: String,
    active_category: Option<usize>,
}

/// Widget library panel — shows available widget types as a responsive tile grid.
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
    ("HUD", regular::CROSSHAIR, &[
        UiWidgetType::Crosshair,
        UiWidgetType::AmmoCounter,
        UiWidgetType::Compass,
        UiWidgetType::StatusEffectBar,
        UiWidgetType::NotificationFeed,
        UiWidgetType::RadialMenu,
        UiWidgetType::Minimap,
    ]),
    ("Menu", regular::LIST_BULLETS, &[
        UiWidgetType::InventoryGrid,
        UiWidgetType::DialogBox,
        UiWidgetType::ObjectiveTracker,
        UiWidgetType::LoadingScreen,
        UiWidgetType::KeybindRow,
        UiWidgetType::SettingsRow,
    ]),
    ("Extra", regular::PUZZLE_PIECE, &[
        UiWidgetType::Separator,
        UiWidgetType::NumberInput,
        UiWidgetType::VerticalSlider,
        UiWidgetType::Scrollbar,
        UiWidgetType::List,
    ]),
    ("Shapes", regular::SHAPES, &[
        UiWidgetType::Circle,
        UiWidgetType::Arc,
        UiWidgetType::RadialProgress,
        UiWidgetType::Line,
        UiWidgetType::Triangle,
        UiWidgetType::Polygon,
        UiWidgetType::Wedge,
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

        let accent = theme.semantic.accent.to_color32();
        let text_primary = theme.text.primary.to_color32();
        let text_muted = theme.text.muted.to_color32();

        let panel_padding = 6.0;
        let spacing = 4.0;

        ui.add_space(panel_padding);

        // Search bar
        ui.horizontal(|ui| {
            ui.add_space(panel_padding);
            let available = ui.available_width() - panel_padding;
            ui.add_sized(
                [available, 20.0],
                egui::TextEdit::singleline(&mut state.search)
                    .hint_text(format!("{} Search widgets...", regular::MAGNIFYING_GLASS)),
            );
        });

        ui.add_space(spacing);

        // "Add Canvas" button
        ui.horizontal(|ui| {
            ui.add_space(panel_padding);
            let btn = ui.add(
                egui::Button::new(
                    egui::RichText::new(format!("{} Add Canvas", regular::PLUS))
                        .size(12.0)
                        .color(text_primary),
                )
                .fill(theme.surfaces.panel.to_color32())
                .min_size(egui::vec2(ui.available_width() - panel_padding, 28.0)),
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

        ui.add_space(spacing);

        // Category filter tabs
        ui.horizontal(|ui| {
            ui.add_space(panel_padding);
            ui.spacing_mut().item_spacing.x = 2.0;

            if ui
                .selectable_label(state.active_category.is_none(), "All")
                .clicked()
            {
                state.active_category = None;
            }
            for (i, &(name, _icon, _types)) in CATEGORIES.iter().enumerate() {
                let selected = state.active_category == Some(i);
                if ui.selectable_label(selected, name).clicked() {
                    state.active_category = Some(i);
                }
            }
        });

        ui.add_space(spacing);

        // Filter widgets
        let search_lower = state.search.to_lowercase();
        let widgets: Vec<&UiWidgetType> = CATEGORIES
            .iter()
            .enumerate()
            .filter(|(i, _)| state.active_category.is_none() || state.active_category == Some(*i))
            .flat_map(|(_, &(_name, _icon, types))| types.iter())
            .filter(|wt| {
                search_lower.is_empty() || wt.label().to_lowercase().contains(&search_lower)
            })
            .collect();

        // Responsive tile grid
        egui::ScrollArea::vertical().show(ui, |ui| {
            if widgets.is_empty() {
                ui.add_space(8.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("No matching widgets.")
                            .size(11.0)
                            .color(text_muted),
                    );
                });
                return;
            }

            let available_width = ui.available_width() - panel_padding * 2.0;
            let min_tile = 56.0;
            let max_tile = 80.0;
            let cols =
                ((available_width + spacing) / (min_tile + spacing)).floor().max(1.0) as usize;
            let tile_size = ((available_width - spacing * (cols as f32 - 1.0)) / cols as f32)
                .clamp(min_tile, max_tile);
            let label_height = 16.0;

            ui.add_space(2.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = spacing;

                let rows = (widgets.len() + cols - 1) / cols;
                for row in 0..rows {
                    ui.horizontal(|ui| {
                        ui.add_space(panel_padding);
                        ui.spacing_mut().item_spacing.x = spacing;
                        for col in 0..cols {
                            let idx = row * cols + col;
                            if idx >= widgets.len() {
                                break;
                            }
                            let wtype = widgets[idx];
                            let total_height = tile_size + label_height;
                            let (rect, response) = ui.allocate_exact_size(
                                Vec2::new(tile_size, total_height),
                                egui::Sense::click_and_drag(),
                            );

                            let hovered = response.hovered();
                            let bg = if hovered {
                                theme.widgets.hovered_bg.to_color32()
                            } else {
                                theme.surfaces.faint.to_color32()
                            };

                            ui.painter().rect_filled(rect, 6.0, bg);

                            if hovered {
                                ui.painter().rect_stroke(
                                    rect,
                                    6.0,
                                    egui::Stroke::new(1.0, accent.linear_multiply(0.6)),
                                    egui::StrokeKind::Outside,
                                );
                                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                            }

                            // Icon
                            let icon_rect = egui::Rect::from_min_size(
                                rect.min,
                                Vec2::new(tile_size, tile_size),
                            );
                            ui.painter().text(
                                icon_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                wtype.icon(),
                                egui::FontId::proportional(28.0),
                                if hovered { accent } else { text_primary },
                            );

                            // Label
                            ui.painter().text(
                                egui::pos2(rect.center().x, rect.max.y - label_height / 2.0),
                                egui::Align2::CENTER_CENTER,
                                wtype.label(),
                                egui::FontId::proportional(9.5),
                                text_muted,
                            );

                            response
                                .clone()
                                .on_hover_text(format!("Drag onto canvas or click to add a {} widget", wtype.label()));

                            // Drag to canvas: start drag payload
                            if response.drag_started() {
                                if let Some(pos) = ui.ctx().pointer_latest_pos() {
                                    let wt = wtype.clone();
                                    commands.push(move |world: &mut World| {
                                        world.insert_resource(WidgetDragPayload {
                                            widget_type: wt,
                                            origin: pos,
                                            is_detached: false,
                                        });
                                    });
                                }
                            }

                            // Click fallback: add at default position
                            if response.clicked() {
                                let wt = wtype.clone();
                                commands.push(move |world: &mut World| {
                                    crate::spawn::spawn_widget(world, &wt, None);
                                });
                            }
                        }
                    });
                }
            });
            ui.add_space(panel_padding);
        });
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Left
    }
}
