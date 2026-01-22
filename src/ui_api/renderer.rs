//! Internal egui renderer for the UI abstraction layer.
//!
//! This module translates abstract Widget definitions into actual egui calls.
//! It is internal to the editor and not exposed to plugins.

use bevy_egui::egui;

use super::events::{UiEvent, UiEventQueue};
use super::types::{Align, SemanticColor, TextStyle};
use super::widgets::{Tab, TableColumn, TableRow, Widget};

/// Theme configuration for rendering
pub struct Theme {
    pub primary: egui::Color32,
    pub secondary: egui::Color32,
    pub accent: egui::Color32,
    pub success: egui::Color32,
    pub warning: egui::Color32,
    pub error: egui::Color32,
    pub background: egui::Color32,
    pub surface: egui::Color32,
    pub text: egui::Color32,
    pub text_muted: egui::Color32,
    pub border: egui::Color32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            primary: egui::Color32::from_rgb(100, 150, 255),
            secondary: egui::Color32::from_rgb(150, 150, 150),
            accent: egui::Color32::from_rgb(255, 180, 100),
            success: egui::Color32::from_rgb(100, 200, 100),
            warning: egui::Color32::from_rgb(255, 200, 100),
            error: egui::Color32::from_rgb(255, 100, 100),
            background: egui::Color32::from_rgb(30, 30, 30),
            surface: egui::Color32::from_rgb(45, 45, 45),
            text: egui::Color32::from_rgb(220, 220, 220),
            text_muted: egui::Color32::from_rgb(150, 150, 150),
            border: egui::Color32::from_rgb(70, 70, 70),
        }
    }
}

impl Theme {
    /// Convert semantic color to egui color
    pub fn semantic_color(&self, color: SemanticColor) -> egui::Color32 {
        match color {
            SemanticColor::Primary => self.primary,
            SemanticColor::Secondary => self.secondary,
            SemanticColor::Accent => self.accent,
            SemanticColor::Success => self.success,
            SemanticColor::Warning => self.warning,
            SemanticColor::Error => self.error,
            SemanticColor::Background => self.background,
            SemanticColor::Surface => self.surface,
            SemanticColor::Text => self.text,
            SemanticColor::TextMuted => self.text_muted,
            SemanticColor::Border => self.border,
        }
    }

    /// Get font id for text style
    pub fn text_style_font(&self, style: TextStyle) -> egui::FontId {
        match style {
            TextStyle::Body => egui::FontId::proportional(14.0),
            TextStyle::Heading1 => egui::FontId::proportional(24.0),
            TextStyle::Heading2 => egui::FontId::proportional(20.0),
            TextStyle::Heading3 => egui::FontId::proportional(16.0),
            TextStyle::Caption => egui::FontId::proportional(12.0),
            TextStyle::Code => egui::FontId::monospace(14.0),
            TextStyle::Label => egui::FontId::proportional(13.0),
        }
    }
}

/// UI Renderer that translates abstract widgets to egui
pub struct UiRenderer {
    theme: Theme,
    events: UiEventQueue,
}

impl Default for UiRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl UiRenderer {
    /// Create a new UI renderer with default theme
    pub fn new() -> Self {
        Self {
            theme: Theme::default(),
            events: UiEventQueue::new(),
        }
    }

    /// Create a UI renderer with custom theme
    pub fn with_theme(theme: Theme) -> Self {
        Self {
            theme,
            events: UiEventQueue::new(),
        }
    }

    /// Get the current theme
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Set the theme
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    /// Drain all pending events
    pub fn drain_events(&mut self) -> impl Iterator<Item = UiEvent> + '_ {
        self.events.drain()
    }

    /// Render a widget tree
    pub fn render(&mut self, ui: &mut egui::Ui, widget: &Widget) {
        self.render_widget(ui, widget);
    }

    fn render_widget(&mut self, ui: &mut egui::Ui, widget: &Widget) {
        match widget {
            Widget::Label { text, style } => {
                let font = self.theme.text_style_font(*style);
                ui.label(egui::RichText::new(text).font(font));
            }

            Widget::Button { label, id, enabled } => {
                let button = egui::Button::new(label);
                let response = ui.add_enabled(*enabled, button);
                if response.clicked() {
                    self.events.push(UiEvent::ButtonClicked(*id));
                }
            }

            Widget::IconButton {
                icon,
                tooltip,
                id,
                enabled,
            } => {
                let button = egui::Button::new(icon);
                let response = ui.add_enabled(*enabled, button).on_hover_text(tooltip);
                if response.clicked() {
                    self.events.push(UiEvent::ButtonClicked(*id));
                }
            }

            Widget::TextInput {
                value,
                placeholder,
                id,
            } => {
                let mut text = value.clone();
                let response = ui.add(
                    egui::TextEdit::singleline(&mut text).hint_text(placeholder),
                );
                if response.changed() {
                    self.events.push(UiEvent::TextInputChanged {
                        id: *id,
                        value: text.clone(),
                    });
                }
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.events.push(UiEvent::TextInputSubmitted {
                        id: *id,
                        value: text,
                    });
                }
            }

            Widget::TextEdit {
                value,
                id,
                min_lines,
                max_lines: _,
            } => {
                let mut text = value.clone();
                let response = ui.add(
                    egui::TextEdit::multiline(&mut text)
                        .desired_rows(*min_lines as usize)
                        .font(egui::FontId::monospace(14.0)),
                );
                if response.changed() {
                    self.events.push(UiEvent::TextInputChanged {
                        id: *id,
                        value: text,
                    });
                }
            }

            Widget::Checkbox { checked, label, id } => {
                let mut value = *checked;
                if ui.checkbox(&mut value, label).changed() {
                    self.events.push(UiEvent::CheckboxToggled {
                        id: *id,
                        checked: value,
                    });
                }
            }

            Widget::Slider {
                value,
                min,
                max,
                id,
                label,
            } => {
                let mut val = *value;
                let slider = egui::Slider::new(&mut val, *min..=*max);
                let slider = if let Some(label) = label {
                    slider.text(label)
                } else {
                    slider
                };
                if ui.add(slider).changed() {
                    self.events.push(UiEvent::SliderChanged { id: *id, value: val });
                }
            }

            Widget::SliderInt {
                value,
                min,
                max,
                id,
                label,
            } => {
                let mut val = *value;
                let slider = egui::Slider::new(&mut val, *min..=*max);
                let slider = if let Some(label) = label {
                    slider.text(label)
                } else {
                    slider
                };
                if ui.add(slider).changed() {
                    self.events.push(UiEvent::SliderIntChanged { id: *id, value: val });
                }
            }

            Widget::Dropdown {
                selected,
                options,
                id,
            } => {
                let current = options.get(*selected as usize).map(|s| s.as_str()).unwrap_or("");
                egui::ComboBox::from_id_salt(id.0)
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        for (i, option) in options.iter().enumerate() {
                            if ui.selectable_label(i == *selected as usize, option).clicked() {
                                self.events.push(UiEvent::DropdownSelected {
                                    id: *id,
                                    index: i as u32,
                                });
                            }
                        }
                    });
            }

            Widget::ColorPicker { color, id, alpha } => {
                let mut rgba = *color;
                let response = if *alpha {
                    ui.color_edit_button_rgba_unmultiplied(&mut rgba)
                } else {
                    let mut rgb = [rgba[0], rgba[1], rgba[2]];
                    let response = ui.color_edit_button_rgb(&mut rgb);
                    rgba = [rgb[0], rgb[1], rgb[2], 1.0];
                    response
                };
                if response.changed() {
                    self.events.push(UiEvent::ColorChanged {
                        id: *id,
                        color: rgba,
                    });
                }
            }

            Widget::ProgressBar { progress, label } => {
                let bar = egui::ProgressBar::new(*progress);
                let bar = if let Some(text) = label {
                    bar.text(text)
                } else {
                    bar
                };
                ui.add(bar);
            }

            Widget::Row {
                children,
                spacing,
                align,
            } => {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = *spacing;
                    ui.with_layout(self.align_to_layout(*align, egui::Direction::LeftToRight), |ui| {
                        for child in children {
                            self.render_widget(ui, child);
                        }
                    });
                });
            }

            Widget::Column {
                children,
                spacing,
                align,
            } => {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = *spacing;
                    ui.with_layout(self.align_to_layout(*align, egui::Direction::TopDown), |ui| {
                        for child in children {
                            self.render_widget(ui, child);
                        }
                    });
                });
            }

            Widget::Panel {
                title,
                children,
                collapsible,
                default_open,
            } => {
                if *collapsible {
                    egui::CollapsingHeader::new(title)
                        .default_open(*default_open)
                        .show(ui, |ui| {
                            for child in children {
                                self.render_widget(ui, child);
                            }
                        });
                } else {
                    ui.group(|ui| {
                        ui.heading(title);
                        for child in children {
                            self.render_widget(ui, child);
                        }
                    });
                }
            }

            Widget::ScrollArea {
                child,
                max_height,
                horizontal,
            } => {
                let mut scroll = egui::ScrollArea::vertical();
                if let Some(height) = max_height {
                    scroll = scroll.max_height(*height);
                }
                if *horizontal {
                    scroll = scroll.horizontal_scroll_offset(0.0);
                }
                scroll.show(ui, |ui| {
                    self.render_widget(ui, child);
                });
            }

            Widget::Group { children, frame } => {
                if *frame {
                    ui.group(|ui| {
                        for child in children {
                            self.render_widget(ui, child);
                        }
                    });
                } else {
                    for child in children {
                        self.render_widget(ui, child);
                    }
                }
            }

            Widget::TreeNode {
                label,
                id,
                children,
                expanded,
                leaf,
            } => {
                let response = egui::CollapsingHeader::new(label)
                    .id_salt(id.0)
                    .default_open(*expanded)
                    .show_unindented(ui, |ui| {
                        if !*leaf {
                            for child in children {
                                self.render_widget(ui, child);
                            }
                        }
                    });

                if response.header_response.clicked() {
                    self.events.push(UiEvent::TreeNodeSelected(*id));
                }
            }

            Widget::Table {
                columns,
                rows,
                id,
                striped,
            } => {
                self.render_table(ui, columns, rows, *id, *striped);
            }

            Widget::Tabs { tabs, active, id } => {
                self.render_tabs(ui, tabs, *active, *id);
            }

            Widget::Separator => {
                ui.separator();
            }

            Widget::Spacer { size } => {
                let space = match size {
                    super::types::Size::Fixed(s) => *s,
                    super::types::Size::Fill => ui.available_width(),
                    _ => 8.0,
                };
                ui.add_space(space);
            }

            Widget::Image { path, size } => {
                // TODO: Implement image loading
                if let Some([w, h]) = size {
                    ui.allocate_space(egui::vec2(*w, *h));
                }
                let _ = path;
            }

            Widget::Custom { type_id, data } => {
                // Custom widgets need to be handled by registered handlers
                ui.label(format!("Custom widget: {} ({} bytes)", type_id, data.len()));
            }

            Widget::Empty => {}
        }
    }

    fn render_table(
        &mut self,
        ui: &mut egui::Ui,
        columns: &[TableColumn],
        rows: &[TableRow],
        _id: super::types::UiId,
        _striped: bool,
    ) {
        // Simple table rendering without egui_extras
        egui::Grid::new("table")
            .num_columns(columns.len())
            .striped(true)
            .show(ui, |ui| {
                // Header row
                for col in columns {
                    ui.strong(&col.header);
                }
                ui.end_row();

                // Data rows
                for row in rows {
                    for cell in &row.cells {
                        self.render_widget(ui, cell);
                    }
                    ui.end_row();
                }
            });
    }

    fn render_tabs(&mut self, ui: &mut egui::Ui, tabs: &[Tab], active: u32, id: super::types::UiId) {
        ui.horizontal(|ui| {
            for (i, tab) in tabs.iter().enumerate() {
                let selected = i == active as usize;
                let text = if let Some(icon) = &tab.icon {
                    format!("{} {}", icon, tab.label)
                } else {
                    tab.label.clone()
                };

                if ui.selectable_label(selected, &text).clicked() && !selected {
                    self.events.push(UiEvent::TabSelected {
                        id,
                        index: i as u32,
                    });
                }

                if tab.closable {
                    if ui.small_button("×").clicked() {
                        self.events.push(UiEvent::TabClosed {
                            id,
                            index: i as u32,
                        });
                    }
                }
            }
        });

        ui.separator();

        if let Some(tab) = tabs.get(active as usize) {
            for widget in &tab.content {
                self.render_widget(ui, widget);
            }
        }
    }

    fn align_to_layout(&self, align: Align, direction: egui::Direction) -> egui::Layout {
        let cross_align = match align {
            Align::Start => egui::Align::Min,
            Align::Center => egui::Align::Center,
            Align::End => egui::Align::Max,
            Align::Stretch => egui::Align::Center,
        };
        egui::Layout::from_main_dir_and_cross_align(direction, cross_align)
    }
}
