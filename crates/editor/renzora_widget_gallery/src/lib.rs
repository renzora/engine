//! Widget Gallery — showcases every widget in `renzora_widgets`.
//!
//! Add `WidgetGalleryPlugin` to your app to register the gallery panels.

use std::sync::RwLock;

use bevy::prelude::*;
use renzora::bevy_egui::egui::{self, Color32};
use renzora::egui_phosphor::regular;
use renzora::editor::{
    AppEditorExt, DockTree, EditorPanel, LayoutManager, PanelLocation, WorkspaceLayout,
};
use renzora::theme::ThemeManager;
// Widget re-exports come through renzora::editor (via renzora_ui)
use renzora::editor::{
    section_header, inline_property, property_row, toggle_switch,
    icon_button, empty_state, checkerboard, dim_color,
    TileGrid, TileState, split_label_two_lines,
    tree_row, TreeRowConfig,
    collapsible_section, collapsible_section_removable,
    node_graph, NodeGraphState, NodeGraphConfig, NodeDef, PinDef, PinDirection, PinShape, ConnectionDef,
};

// ── Shared gallery state ────────────────────────────────────────────────────

struct GalleryState {
    toggle_a: bool,
    toggle_b: bool,
    toggle_c: bool,
    slider_val: f32,
    text_val: String,
}

impl Default for GalleryState {
    fn default() -> Self {
        Self {
            toggle_a: true,
            toggle_b: false,
            toggle_c: true,
            slider_val: 0.65,
            text_val: "Hello, Renzora!".into(),
        }
    }
}

// ── Controls gallery state ─────────────────────────────────────────────────

// ── Toast data ─────────────────────────────────────────────────────────────

#[derive(Clone)]
enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Clone)]
struct Toast {
    id: u64,
    message: String,
    level: ToastLevel,
    created_at: f64,
    duration: f64,
}

struct ControlsState {
    // Buttons
    click_count: u32,
    // Dropdowns
    selected_blend: usize,
    selected_layer: usize,
    selected_quality: usize,
    // Checkboxes
    check_shadows: bool,
    check_bloom: bool,
    check_ssao: bool,
    check_fxaa: bool,
    // Radio
    radio_mode: usize,
    // Text inputs
    single_line: String,
    multiline: String,
    search_text: String,
    // Numeric
    drag_x: f32,
    drag_y: f32,
    drag_z: f32,
    int_val: i32,
    slider_range: f32,
    log_slider: f64,
    // Color
    color_rgb: [f32; 3],
    color_rgba: [f32; 4],
    // Progress
    progress: f32,
    progress_anim: f32,
    // Spinner
    spinner_val: f64,
    // Modal
    show_modal: bool,
    show_confirm: bool,
    show_window: bool,
    // Toasts
    toasts: Vec<Toast>,
    toast_counter: u64,
}

impl Default for ControlsState {
    fn default() -> Self {
        Self {
            click_count: 0,
            selected_blend: 0,
            selected_layer: 0,
            selected_quality: 2,
            check_shadows: true,
            check_bloom: true,
            check_ssao: false,
            check_fxaa: true,
            radio_mode: 0,
            single_line: "Entity_01".into(),
            multiline: "A multiline text field\nfor notes or descriptions.".into(),
            search_text: String::new(),
            drag_x: 0.0,
            drag_y: 1.5,
            drag_z: -3.2,
            int_val: 42,
            slider_range: 0.5,
            log_slider: 1000.0,
            color_rgb: [0.2, 0.5, 0.9],
            color_rgba: [1.0, 0.4, 0.1, 0.8],
            progress: 0.68,
            progress_anim: 0.0,
            spinner_val: 60.0,
            show_modal: false,
            show_confirm: false,
            show_window: false,
            toasts: Vec::new(),
            toast_counter: 0,
        }
    }
}

// ── Controls Gallery panel ─────────────────────────────────────────────────

pub struct ControlsGallery {
    state: RwLock<ControlsState>,
}

impl Default for ControlsGallery {
    fn default() -> Self {
        Self {
            state: RwLock::new(ControlsState::default()),
        }
    }
}

const BLEND_MODES: &[&str] = &["Normal", "Multiply", "Screen", "Overlay", "Soft Light", "Hard Light"];
const LAYER_NAMES: &[&str] = &["Default", "UI", "Transparent", "Terrain", "Water", "Skybox"];
const QUALITY_LEVELS: &[&str] = &["Low", "Medium", "High", "Ultra"];
const RENDER_MODES: &[&str] = &["Forward", "Deferred", "Ray Traced"];

impl EditorPanel for ControlsGallery {
    fn id(&self) -> &str {
        "gallery_controls"
    }

    fn title(&self) -> &str {
        "Controls"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::FADERS)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };
        let mut state = self.state.write().unwrap();

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(6.0);

            // ── Buttons ─────────────────────────────────────────────
            section_header(ui, "Buttons", &theme);

            ui.horizontal(|ui| {
                if ui.button("Default").clicked() {
                    state.click_count += 1;
                }
                if ui.add(egui::Button::new(
                    egui::RichText::new(format!("{} Save", regular::FLOPPY_DISK))
                        .color(theme.text.primary.to_color32()),
                )).clicked() {
                    state.click_count += 1;
                }
                if ui.add(egui::Button::new(
                    egui::RichText::new(format!("{} Delete", regular::TRASH))
                        .color(theme.semantic.error.to_color32()),
                )).clicked() {
                    state.click_count += 1;
                }
            });

            ui.horizontal(|ui| {
                if ui.small_button("Small").clicked() {
                    state.click_count += 1;
                }
                ui.add_space(4.0);
                ui.add_enabled(false, egui::Button::new("Disabled"));
                ui.add_space(4.0);
                if ui.add(egui::Button::new(
                    egui::RichText::new(format!("{} Build", regular::HAMMER))
                        .color(Color32::WHITE),
                ).fill(theme.semantic.accent.to_color32())).clicked() {
                    state.click_count += 1;
                }
                if ui.add(egui::Button::new(
                    egui::RichText::new(format!("{} Run", regular::PLAY))
                        .color(Color32::WHITE),
                ).fill(theme.semantic.success.to_color32())).clicked() {
                    state.click_count += 1;
                }
            });

            ui.label(
                egui::RichText::new(format!("Clicked {} time(s)", state.click_count))
                    .size(11.0)
                    .color(theme.text.muted.to_color32()),
            );

            ui.add_space(12.0);

            // ── Dropdown / ComboBox ─────────────────────────────────
            section_header(ui, "Dropdowns (ComboBox)", &theme);

            inline_property(ui, 0, "Blend Mode", &theme, |ui| {
                egui::ComboBox::from_id_salt("blend_mode")
                    .selected_text(BLEND_MODES[state.selected_blend])
                    .show_ui(ui, |ui| {
                        for (i, mode) in BLEND_MODES.iter().enumerate() {
                            ui.selectable_value(&mut state.selected_blend, i, *mode);
                        }
                    });
            });

            inline_property(ui, 1, "Layer", &theme, |ui| {
                egui::ComboBox::from_id_salt("layer")
                    .selected_text(LAYER_NAMES[state.selected_layer])
                    .show_ui(ui, |ui| {
                        for (i, name) in LAYER_NAMES.iter().enumerate() {
                            ui.selectable_value(&mut state.selected_layer, i, *name);
                        }
                    });
            });

            inline_property(ui, 2, "Quality", &theme, |ui| {
                egui::ComboBox::from_id_salt("quality")
                    .selected_text(QUALITY_LEVELS[state.selected_quality])
                    .show_ui(ui, |ui| {
                        for (i, level) in QUALITY_LEVELS.iter().enumerate() {
                            ui.selectable_value(&mut state.selected_quality, i, *level);
                        }
                    });
            });

            ui.add_space(12.0);

            // ── Checkboxes ──────────────────────────────────────────
            section_header(ui, "Checkboxes", &theme);

            ui.horizontal(|ui| {
                ui.checkbox(&mut state.check_shadows, "Shadows");
                ui.checkbox(&mut state.check_bloom, "Bloom");
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut state.check_ssao, "SSAO");
                ui.checkbox(&mut state.check_fxaa, "FXAA");
            });

            ui.add_space(12.0);

            // ── Radio Buttons ───────────────────────────────────────
            section_header(ui, "Radio Buttons", &theme);

            ui.horizontal(|ui| {
                for (i, label) in RENDER_MODES.iter().enumerate() {
                    ui.radio_value(&mut state.radio_mode, i, *label);
                }
            });
            ui.label(
                egui::RichText::new(format!("Selected: {}", RENDER_MODES[state.radio_mode]))
                    .size(11.0)
                    .color(theme.text.muted.to_color32()),
            );

            ui.add_space(12.0);

            // ── Text Inputs ─────────────────────────────────────────
            section_header(ui, "Text Inputs", &theme);

            inline_property(ui, 0, "Name", &theme, |ui| {
                ui.text_edit_singleline(&mut state.single_line);
            });

            inline_property(ui, 1, "Search", &theme, |ui| {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut state.search_text)
                        .hint_text(format!("{} Search...", regular::MAGNIFYING_GLASS)),
                );
                if response.changed() && !state.search_text.is_empty() {
                    // Visual feedback that search is active
                }
            });

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Multiline:")
                    .size(11.0)
                    .color(theme.text.secondary.to_color32()),
            );
            ui.add(
                egui::TextEdit::multiline(&mut state.multiline)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY),
            );

            ui.add_space(12.0);

            // ── Numeric Controls ────────────────────────────────────
            section_header(ui, "Numeric (DragValue / Slider)", &theme);

            inline_property(ui, 0, "Position", &theme, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("X").size(11.0).color(Color32::from_rgb(230, 89, 89)));
                    ui.add(egui::DragValue::new(&mut state.drag_x).speed(0.1).range(-100.0..=100.0));
                    ui.label(egui::RichText::new("Y").size(11.0).color(Color32::from_rgb(89, 191, 115)));
                    ui.add(egui::DragValue::new(&mut state.drag_y).speed(0.1).range(-100.0..=100.0));
                    ui.label(egui::RichText::new("Z").size(11.0).color(Color32::from_rgb(99, 178, 238)));
                    ui.add(egui::DragValue::new(&mut state.drag_z).speed(0.1).range(-100.0..=100.0));
                });
            });

            inline_property(ui, 1, "Integer", &theme, |ui| {
                ui.add(egui::DragValue::new(&mut state.int_val).speed(1).range(0..=100));
            });

            inline_property(ui, 2, "Range", &theme, |ui| {
                ui.add(egui::Slider::new(&mut state.slider_range, 0.0..=1.0).text(""));
            });

            inline_property(ui, 3, "Log Scale", &theme, |ui| {
                ui.add(
                    egui::Slider::new(&mut state.log_slider, 1.0..=100000.0)
                        .logarithmic(true)
                        .text(""),
                );
            });

            inline_property(ui, 4, "Spinner", &theme, |ui| {
                ui.add(
                    egui::DragValue::new(&mut state.spinner_val)
                        .speed(0.5)
                        .range(0.0..=360.0)
                        .suffix("\u{00B0}"),
                );
            });

            ui.add_space(12.0);

            // ── Color Pickers ───────────────────────────────────────
            section_header(ui, "Color Pickers", &theme);

            inline_property(ui, 0, "Diffuse", &theme, |ui| {
                ui.color_edit_button_rgb(&mut state.color_rgb);
                ui.label(
                    egui::RichText::new(format!(
                        "#{:02X}{:02X}{:02X}",
                        (state.color_rgb[0] * 255.0) as u8,
                        (state.color_rgb[1] * 255.0) as u8,
                        (state.color_rgb[2] * 255.0) as u8,
                    ))
                    .size(11.0)
                    .color(theme.text.muted.to_color32()),
                );
            });

            inline_property(ui, 1, "Emission", &theme, |ui| {
                ui.color_edit_button_rgba_unmultiplied(&mut state.color_rgba);
                ui.label(
                    egui::RichText::new(format!("a={:.0}%", state.color_rgba[3] * 100.0))
                        .size(11.0)
                        .color(theme.text.muted.to_color32()),
                );
            });

            ui.add_space(12.0);

            // ── Progress Bars ───────────────────────────────────────
            section_header(ui, "Progress Bars", &theme);

            inline_property(ui, 0, "Loading", &theme, |ui| {
                ui.add(egui::ProgressBar::new(state.progress).text(format!("{:.0}%", state.progress * 100.0)));
            });

            // Animated progress
            state.progress_anim += 0.003;
            if state.progress_anim > 1.0 {
                state.progress_anim = 0.0;
            }
            inline_property(ui, 1, "Building", &theme, |ui| {
                ui.add(egui::ProgressBar::new(state.progress_anim).animate(true));
            });
            ui.ctx().request_repaint();

            inline_property(ui, 2, "Adjust", &theme, |ui| {
                ui.add(egui::Slider::new(&mut state.progress, 0.0..=1.0).text(""));
            });

            ui.add_space(12.0);

            // ── Separators & Spacing ────────────────────────────────
            section_header(ui, "Separators & Spacing", &theme);

            ui.label("Content above separator");
            ui.separator();
            ui.label("Content below separator");
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Left");
                ui.separator();
                ui.label("Center");
                ui.separator();
                ui.label("Right");
            });

            ui.add_space(12.0);

            // ── Tooltips & Hyperlinks ───────────────────────────────
            section_header(ui, "Tooltips & Links", &theme);

            ui.horizontal(|ui| {
                ui.label("Hover me:").on_hover_text("This is a tooltip with extra info.");
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(format!("{} Info", regular::INFO))
                        .color(theme.semantic.accent.to_color32()),
                ).on_hover_ui(|ui| {
                    ui.label("Rich tooltip with custom content");
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(regular::CHECK_CIRCLE)
                                .color(theme.semantic.success.to_color32()),
                        );
                        ui.label("Status: OK");
                    });
                });
            });

            ui.horizontal(|ui| {
                ui.hyperlink_to("Renzora Docs", "https://renzora.dev");
                ui.label("|");
                ui.hyperlink_to("Source Code", "https://github.com/renzora");
            });

            ui.add_space(12.0);

            // ── Menus with Submenus ─────────────────────────────────
            section_header(ui, "Menus with Submenus", &theme);

            ui.horizontal(|ui| {
                egui::MenuBar::new().ui(ui, |ui: &mut egui::Ui| {
                    ui.menu_button(format!("{} File", regular::FILE), |ui| {
                        if ui.button(format!("{} New Scene", regular::FILE_PLUS)).clicked() {
                            ui.close();
                        }
                        if ui.button(format!("{} Open...", regular::FOLDER_OPEN)).clicked() {
                            ui.close();
                        }

                        egui::containers::menu::SubMenuButton::new(format!("{} Open Recent", regular::CLOCK_COUNTER_CLOCKWISE))
                            .ui(ui, |ui: &mut egui::Ui| {
                                if ui.button("main_scene.scn").clicked() { ui.close(); }
                                if ui.button("test_level.scn").clicked() { ui.close(); }
                                if ui.button("prototype.scn").clicked() { ui.close(); }
                                ui.separator();
                                if ui.button("Clear Recent").clicked() { ui.close(); }
                            });

                        ui.separator();
                        if ui.button(format!("{} Save", regular::FLOPPY_DISK)).clicked() {
                            ui.close();
                        }
                        if ui.button(format!("{} Save As...", regular::FLOPPY_DISK)).clicked() {
                            ui.close();
                        }

                        egui::containers::menu::SubMenuButton::new(format!("{} Export", regular::EXPORT))
                            .ui(ui, |ui: &mut egui::Ui| {
                                if ui.button("glTF (.glb)").clicked() { ui.close(); }
                                if ui.button("FBX (.fbx)").clicked() { ui.close(); }
                                if ui.button("OBJ (.obj)").clicked() { ui.close(); }

                                egui::containers::menu::SubMenuButton::new("Image Formats")
                                    .ui(ui, |ui: &mut egui::Ui| {
                                        if ui.button("PNG").clicked() { ui.close(); }
                                        if ui.button("JPEG").clicked() { ui.close(); }
                                        if ui.button("EXR (HDR)").clicked() { ui.close(); }
                                    });
                            });

                        ui.separator();
                        if ui.button(format!("{} Quit", regular::SIGN_OUT)).clicked() {
                            ui.close();
                        }
                    });

                    ui.menu_button(format!("{} Edit", regular::PENCIL), |ui| {
                        if ui.button(format!("{} Undo", regular::ARROW_COUNTER_CLOCKWISE)).clicked() {
                            ui.close();
                        }
                        if ui.button(format!("{} Redo", regular::ARROW_CLOCKWISE)).clicked() {
                            ui.close();
                        }
                        ui.separator();
                        if ui.button(format!("{} Cut", regular::SCISSORS)).clicked() {
                            ui.close();
                        }
                        if ui.button(format!("{} Copy", regular::COPY)).clicked() {
                            ui.close();
                        }
                        if ui.button(format!("{} Paste", regular::CLIPBOARD)).clicked() {
                            ui.close();
                        }

                        ui.separator();

                        egui::containers::menu::SubMenuButton::new(format!("{} Preferences", regular::GEAR))
                            .ui(ui, |ui: &mut egui::Ui| {
                                if ui.button("General").clicked() { ui.close(); }
                                if ui.button("Keybindings").clicked() { ui.close(); }
                                if ui.button("Theme").clicked() { ui.close(); }
                            });
                    });

                    ui.menu_button(format!("{} View", regular::EYE), |ui| {
                        egui::containers::menu::SubMenuButton::new(format!("{} Panels", regular::SQUARES_FOUR))
                            .ui(ui, |ui: &mut egui::Ui| {
                                if ui.button("Hierarchy").clicked() { ui.close(); }
                                if ui.button("Inspector").clicked() { ui.close(); }
                                if ui.button("Console").clicked() { ui.close(); }
                                if ui.button("Assets").clicked() { ui.close(); }
                            });

                        egui::containers::menu::SubMenuButton::new(format!("{} Layout", regular::LAYOUT))
                            .ui(ui, |ui: &mut egui::Ui| {
                                if ui.button("Default").clicked() { ui.close(); }
                                if ui.button("Scripting").clicked() { ui.close(); }
                                if ui.button("Debug").clicked() { ui.close(); }
                                if ui.button("Minimal").clicked() { ui.close(); }
                            });

                        ui.separator();
                        if ui.button("Toggle Fullscreen").clicked() { ui.close(); }
                    });
                });
            });

            ui.add_space(12.0);

            // ── Context Menu with Submenus ──────────────────────────
            section_header(ui, "Context Menu (right-click)", &theme);

            let (menu_rect, menu_response) = ui.allocate_exact_size(
                egui::Vec2::new(ui.available_width(), 40.0),
                egui::Sense::click(),
            );
            let is_hovered = menu_response.hovered();
            ui.painter().rect_filled(
                menu_rect,
                egui::CornerRadius::same(4),
                if is_hovered {
                    theme.widgets.hovered_bg.to_color32()
                } else {
                    theme.widgets.inactive_bg.to_color32()
                },
            );
            ui.painter().text(
                menu_rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{} Right-click for context menu with submenus", regular::CURSOR_CLICK),
                egui::FontId::proportional(11.0),
                theme.text.secondary.to_color32(),
            );

            menu_response.context_menu(|ui| {
                if ui.button(format!("{} Cut", regular::SCISSORS)).clicked() {
                    ui.close();
                }
                if ui.button(format!("{} Copy", regular::COPY)).clicked() {
                    ui.close();
                }
                if ui.button(format!("{} Paste", regular::CLIPBOARD)).clicked() {
                    ui.close();
                }
                ui.separator();

                egui::containers::menu::SubMenuButton::new(format!("{} Transform", regular::ARROWS_OUT_CARDINAL))
                    .ui(ui, |ui: &mut egui::Ui| {
                        if ui.button("Reset Position").clicked() { ui.close(); }
                        if ui.button("Reset Rotation").clicked() { ui.close(); }
                        if ui.button("Reset Scale").clicked() { ui.close(); }
                        ui.separator();
                        if ui.button("Snap to Grid").clicked() { ui.close(); }
                    });

                egui::containers::menu::SubMenuButton::new(format!("{} Add Component", regular::PLUS_CIRCLE))
                    .ui(ui, |ui: &mut egui::Ui| {
                        egui::containers::menu::SubMenuButton::new(format!("{} Physics", regular::ATOM))
                            .ui(ui, |ui: &mut egui::Ui| {
                                if ui.button("Rigid Body").clicked() { ui.close(); }
                                if ui.button("Collider").clicked() { ui.close(); }
                                if ui.button("Joint").clicked() { ui.close(); }
                            });

                        egui::containers::menu::SubMenuButton::new(format!("{} Rendering", regular::CUBE))
                            .ui(ui, |ui: &mut egui::Ui| {
                                if ui.button("Mesh Renderer").clicked() { ui.close(); }
                                if ui.button("Light").clicked() { ui.close(); }
                                if ui.button("Camera").clicked() { ui.close(); }
                            });

                        egui::containers::menu::SubMenuButton::new(format!("{} Audio", regular::SPEAKER_HIGH))
                            .ui(ui, |ui: &mut egui::Ui| {
                                if ui.button("Audio Source").clicked() { ui.close(); }
                                if ui.button("Audio Listener").clicked() { ui.close(); }
                            });
                    });

                ui.separator();
                if ui.button(format!("{} Duplicate", regular::COPY_SIMPLE)).clicked() {
                    ui.close();
                }
                if ui.button(format!("{} Delete", regular::TRASH)).clicked() {
                    ui.close();
                }
            });

            ui.add_space(12.0);

            // ── Modal / Overlay ─────────────────────────────────────
            section_header(ui, "Modal & Overlays", &theme);

            ui.horizontal(|ui| {
                if ui.button(format!("{} Info Modal", regular::INFO)).clicked() {
                    state.show_modal = true;
                }
                if ui.button(format!("{} Confirm Dialog", regular::WARNING)).clicked() {
                    state.show_confirm = true;
                }
                if ui.button(format!("{} Floating Window", regular::BROWSER)).clicked() {
                    state.show_window = !state.show_window;
                }
            });

            ui.label(
                egui::RichText::new("Modals block background input. Floating windows are draggable overlays.")
                    .size(11.0)
                    .color(theme.text.muted.to_color32()),
            );

            ui.add_space(12.0);

            // ── Toast Notifications ─────────────────────────────────
            section_header(ui, "Toast Notifications", &theme);

            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(
                    egui::RichText::new(format!("{} Info", regular::INFO))
                        .color(theme.semantic.accent.to_color32()),
                )).clicked() {
                    state.toast_counter += 1;
                    let tid = state.toast_counter;
                    state.toasts.push(Toast {
                        id: tid,
                        message: "Scene saved successfully.".into(),
                        level: ToastLevel::Info,
                        created_at: ui.ctx().input(|i| i.time),
                        duration: 3.0,
                    });
                }
                if ui.add(egui::Button::new(
                    egui::RichText::new(format!("{} Success", regular::CHECK_CIRCLE))
                        .color(theme.semantic.success.to_color32()),
                )).clicked() {
                    state.toast_counter += 1;
                    let tid = state.toast_counter;
                    state.toasts.push(Toast {
                        id: tid,
                        message: "Build completed with 0 errors.".into(),
                        level: ToastLevel::Success,
                        created_at: ui.ctx().input(|i| i.time),
                        duration: 3.0,
                    });
                }
                if ui.add(egui::Button::new(
                    egui::RichText::new(format!("{} Warning", regular::WARNING))
                        .color(theme.semantic.warning.to_color32()),
                )).clicked() {
                    state.toast_counter += 1;
                    let tid = state.toast_counter;
                    state.toasts.push(Toast {
                        id: tid,
                        message: "3 assets have missing references.".into(),
                        level: ToastLevel::Warning,
                        created_at: ui.ctx().input(|i| i.time),
                        duration: 4.0,
                    });
                }
                if ui.add(egui::Button::new(
                    egui::RichText::new(format!("{} Error", regular::X_CIRCLE))
                        .color(theme.semantic.error.to_color32()),
                )).clicked() {
                    state.toast_counter += 1;
                    let tid = state.toast_counter;
                    state.toasts.push(Toast {
                        id: tid,
                        message: "Shader compilation failed: missing uniform 'u_time'.".into(),
                        level: ToastLevel::Error,
                        created_at: ui.ctx().input(|i| i.time),
                        duration: 5.0,
                    });
                }
            });

            let active_count = {
                let now = ui.ctx().input(|i| i.time);
                state.toasts.iter().filter(|t| now - t.created_at < t.duration).count()
            };
            ui.label(
                egui::RichText::new(format!("{} active toast(s)", active_count))
                    .size(11.0)
                    .color(theme.text.muted.to_color32()),
            );

            ui.add_space(8.0);
        });

        // ── Render modals and overlays outside the scroll area ───────

        let ctx = ui.ctx().clone();

        // Info modal
        if state.show_modal {
            let modal = egui::Modal::new(egui::Id::new("gallery_info_modal"));
            let resp = modal.show(&ctx, |ui| {
                ui.set_width(300.0);
                ui.heading("About Renzora");
                ui.add_space(8.0);
                ui.label("Renzora is a modular game engine built with Bevy and egui.");
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Version 0.2.0")
                        .size(11.0)
                        .color(theme.text.muted.to_color32()),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            state.show_modal = false;
                        }
                    });
                });
            });
            if resp.should_close() {
                state.show_modal = false;
            }
        }

        // Confirm dialog modal
        if state.show_confirm {
            let modal = egui::Modal::new(egui::Id::new("gallery_confirm_modal"));
            let resp = modal.show(&ctx, |ui| {
                ui.set_width(340.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(regular::WARNING)
                            .size(20.0)
                            .color(theme.semantic.warning.to_color32()),
                    );
                    ui.heading("Unsaved Changes");
                });
                ui.add_space(8.0);
                ui.label("You have unsaved changes. Do you want to save before closing?");
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(egui::Button::new(
                            egui::RichText::new("Save").color(Color32::WHITE),
                        ).fill(theme.semantic.accent.to_color32())).clicked() {
                            state.show_confirm = false;
                            state.toast_counter += 1;
                            let tid = state.toast_counter;
                            state.toasts.push(Toast {
                                id: tid,
                                message: "Changes saved.".into(),
                                level: ToastLevel::Success,
                                created_at: ctx.input(|i| i.time),
                                duration: 2.5,
                            });
                        }
                        if ui.button("Don't Save").clicked() {
                            state.show_confirm = false;
                        }
                        if ui.button("Cancel").clicked() {
                            state.show_confirm = false;
                        }
                    });
                });
            });
            if resp.should_close() {
                state.show_confirm = false;
            }
        }

        // Floating window overlay
        if state.show_window {
            let mut open = state.show_window;
            egui::Window::new(format!("{} Quick Settings", regular::GEAR))
                .open(&mut open)
                .collapsible(true)
                .resizable(true)
                .default_width(250.0)
                .show(&ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Theme:");
                        egui::ComboBox::from_id_salt("overlay_theme")
                            .selected_text("Dark")
                            .show_ui(ui, |ui| {
                                let _ = ui.selectable_label(true, "Dark");
                                let _ = ui.selectable_label(false, "Light");
                                let _ = ui.selectable_label(false, "Monokai");
                            });
                    });
                    ui.horizontal(|ui| {
                        ui.label("Font size:");
                        ui.label("13px");
                    });
                    ui.separator();
                    ui.checkbox(&mut true.clone(), "Show grid");
                    ui.checkbox(&mut false.clone(), "Snap to grid");
                    ui.horizontal(|ui| {
                        ui.label("Grid size:");
                        ui.add(egui::DragValue::new(&mut 1.0_f32.clone()).speed(0.1).range(0.1..=10.0));
                    });
                });
            state.show_window = open;
        }

        // ── Render toast notifications ──────────────────────────────

        let now = ctx.input(|i| i.time);

        // Remove expired toasts
        state.toasts.retain(|t| now - t.created_at < t.duration);

        // Render active toasts stacked from bottom-right
        let has_toasts = !state.toasts.is_empty();
        let toast_snapshot: Vec<Toast> = state.toasts.clone();

        for (i, toast) in toast_snapshot.iter().rev().enumerate() {
            let age = now - toast.created_at;
            let remaining = toast.duration - age;

            // Fade in for first 0.2s, fade out for last 0.5s
            let alpha = if age < 0.2 {
                (age / 0.2) as f32
            } else if remaining < 0.5 {
                (remaining / 0.5) as f32
            } else {
                1.0
            };
            let alpha_u8 = (alpha * 240.0) as u8;

            let (icon, icon_color) = match toast.level {
                ToastLevel::Info => (regular::INFO, theme.semantic.accent.to_color32()),
                ToastLevel::Success => (regular::CHECK_CIRCLE, theme.semantic.success.to_color32()),
                ToastLevel::Warning => (regular::WARNING, theme.semantic.warning.to_color32()),
                ToastLevel::Error => (regular::X_CIRCLE, theme.semantic.error.to_color32()),
            };

            let y_offset = -40.0 - i as f32 * 50.0;

            egui::Area::new(egui::Id::new(("gallery_toast", toast.id)))
                .order(egui::Order::Foreground)
                .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-12.0, y_offset))
                .interactable(false)
                .show(&ctx, |ui| {
                    let bg = Color32::from_rgba_unmultiplied(35, 35, 45, alpha_u8);
                    let border = Color32::from_rgba_unmultiplied(
                        icon_color.r(), icon_color.g(), icon_color.b(),
                        (alpha * 120.0) as u8,
                    );

                    egui::Frame::NONE
                        .fill(bg)
                        .stroke(egui::Stroke::new(1.0, border))
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .corner_radius(egui::CornerRadius::same(6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(icon)
                                        .size(14.0)
                                        .color(Color32::from_rgba_unmultiplied(
                                            icon_color.r(), icon_color.g(), icon_color.b(),
                                            alpha_u8,
                                        )),
                                );
                                ui.label(
                                    egui::RichText::new(&toast.message)
                                        .size(12.0)
                                        .color(Color32::from_rgba_unmultiplied(
                                            220, 220, 220, alpha_u8,
                                        )),
                                );
                            });
                        });
                });
        }

        if has_toasts {
            ctx.request_repaint();
        }
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Center
    }
}

// ── Properties & Sections panel ─────────────────────────────────────────────

pub struct PropertiesGallery {
    state: RwLock<GalleryState>,
}

impl Default for PropertiesGallery {
    fn default() -> Self {
        Self {
            state: RwLock::new(GalleryState::default()),
        }
    }
}

impl EditorPanel for PropertiesGallery {
    fn id(&self) -> &str {
        "gallery_properties"
    }

    fn title(&self) -> &str {
        "Properties"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::SLIDERS)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };
        let mut state = self.state.write().unwrap();

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(6.0);

            // ── section_header ───────────────────────────────────
            section_header(ui, "section_header", &theme);
            ui.label(
                egui::RichText::new("Muted label used to group controls.")
                    .size(11.0)
                    .color(theme.text.disabled.to_color32()),
            );
            ui.add_space(12.0);

            // ── inline_property ──────────────────────────────────
            section_header(ui, "inline_property", &theme);

            inline_property(ui, 0, "Name", &theme, |ui| {
                ui.text_edit_singleline(&mut state.text_val);
            });
            inline_property(ui, 1, "Speed", &theme, |ui| {
                ui.add(egui::Slider::new(&mut state.slider_val, 0.0..=1.0));
            });
            inline_property(ui, 2, "Position", &theme, |ui| {
                ui.label("0.0, 0.0, 0.0");
            });
            inline_property(ui, 3, "Rotation", &theme, |ui| {
                ui.label("0.0, 0.0, 0.0");
            });
            inline_property(ui, 4, "Visible", &theme, |ui| {
                toggle_switch(ui, egui::Id::new("prop_visible"), state.toggle_a);
            });

            ui.add_space(12.0);

            // ── property_row ─────────────────────────────────────
            section_header(ui, "property_row", &theme);

            property_row(ui, 0, &theme, |ui| {
                ui.label("Full-width row with alternating background");
            });
            property_row(ui, 1, &theme, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Row with button:");
                    if ui.button("Click me").clicked() {}
                });
            });
            property_row(ui, 2, &theme, |ui| {
                ui.label("Even row (darker)");
            });
            property_row(ui, 3, &theme, |ui| {
                ui.label("Odd row (lighter)");
            });

            ui.add_space(12.0);

            // ── toggle_switch ────────────────────────────────────
            section_header(ui, "toggle_switch", &theme);

            ui.horizontal(|ui| {
                ui.label("Enabled:");
                if toggle_switch(ui, egui::Id::new("demo_toggle_a"), state.toggle_a) {
                    state.toggle_a = !state.toggle_a;
                }
                ui.add_space(16.0);
                ui.label("Disabled:");
                if toggle_switch(ui, egui::Id::new("demo_toggle_b"), state.toggle_b) {
                    state.toggle_b = !state.toggle_b;
                }
                ui.add_space(16.0);
                ui.label("Active:");
                if toggle_switch(ui, egui::Id::new("demo_toggle_c"), state.toggle_c) {
                    state.toggle_c = !state.toggle_c;
                }
            });

            ui.add_space(12.0);

            // ── icon_button ──────────────────────────────────────
            section_header(ui, "icon_button", &theme);

            ui.horizontal(|ui| {
                icon_button(ui, regular::PLAY, "Play", theme.semantic.success.to_color32());
                ui.add_space(4.0);
                icon_button(ui, regular::PAUSE, "Pause", theme.semantic.warning.to_color32());
                ui.add_space(4.0);
                icon_button(ui, regular::STOP, "Stop", theme.semantic.error.to_color32());
                ui.add_space(4.0);
                icon_button(ui, regular::ARROW_CLOCKWISE, "Refresh", theme.semantic.accent.to_color32());
                ui.add_space(4.0);
                icon_button(ui, regular::TRASH, "Delete", theme.text.muted.to_color32());
                ui.add_space(4.0);
                icon_button(ui, regular::GEAR, "Settings", theme.text.muted.to_color32());
                ui.add_space(4.0);
                icon_button(ui, regular::PLUS, "Add", theme.semantic.accent.to_color32());
            });

            ui.add_space(12.0);

            // ── empty_state ──────────────────────────────────────
            section_header(ui, "empty_state", &theme);

            egui::Frame::new()
                .fill(theme.surfaces.faint.to_color32())
                .corner_radius(egui::CornerRadius::same(6))
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    empty_state(ui, regular::FOLDER_OPEN, "No assets", "Drag files here to import.", &theme);
                });

            ui.add_space(12.0);

            // ── checkerboard ─────────────────────────────────────
            section_header(ui, "checkerboard", &theme);

            let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(200.0, 60.0), egui::Sense::hover());
            checkerboard(ui.painter(), rect);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Transparency pattern",
                egui::FontId::proportional(11.0),
                Color32::from_white_alpha(180),
            );

            ui.add_space(12.0);

            // ── dim_color ────────────────────────────────────────
            section_header(ui, "dim_color", &theme);

            ui.horizontal(|ui| {
                let base = theme.semantic.accent.to_color32();
                let factors = [1.0, 0.8, 0.6, 0.4, 0.2];
                for f in factors {
                    let c = dim_color(base, f);
                    let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(30.0, 20.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 3.0, c);
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("{:.0}%", f * 100.0),
                        egui::FontId::proportional(9.0),
                        Color32::WHITE,
                    );
                }
            });

            ui.add_space(8.0);
        });
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Left
    }
}

// ── Categories panel ────────────────────────────────────────────────────────

pub struct CategoriesGallery {
    state: RwLock<CategoriesState>,
}

struct CategoriesState {
    disabled_sections: Vec<bool>,
}

impl Default for CategoriesGallery {
    fn default() -> Self {
        Self {
            state: RwLock::new(CategoriesState {
                disabled_sections: vec![false, false, true, false],
            }),
        }
    }
}

impl EditorPanel for CategoriesGallery {
    fn id(&self) -> &str {
        "gallery_categories"
    }

    fn title(&self) -> &str {
        "Categories"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::SQUARES_FOUR)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };
        let mut state = self.state.write().unwrap();

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(6.0);

            // ── collapsible_section ──────────────────────────────
            section_header(ui, "collapsible_section", &theme);

            collapsible_section(ui, regular::CUBE, "Transform", "transform", &theme, "cs_transform", true, |ui| {
                inline_property(ui, 0, "Position", &theme, |ui| { ui.label("0.0, 0.0, 0.0"); });
                inline_property(ui, 1, "Rotation", &theme, |ui| { ui.label("0.0, 0.0, 0.0"); });
                inline_property(ui, 2, "Scale", &theme, |ui| { ui.label("1.0, 1.0, 1.0"); });
            });

            collapsible_section(ui, regular::SUN, "Lighting", "lighting", &theme, "cs_lighting", true, |ui| {
                inline_property(ui, 0, "Intensity", &theme, |ui| { ui.label("1000.0"); });
                inline_property(ui, 1, "Color", &theme, |ui| { ui.label("#FFFFFF"); });
                inline_property(ui, 2, "Shadows", &theme, |ui| { ui.label("Enabled"); });
            });

            collapsible_section(ui, regular::CAMERA, "Camera", "camera", &theme, "cs_camera", false, |ui| {
                inline_property(ui, 0, "FOV", &theme, |ui| { ui.label("60.0"); });
                inline_property(ui, 1, "Near", &theme, |ui| { ui.label("0.1"); });
                inline_property(ui, 2, "Far", &theme, |ui| { ui.label("1000.0"); });
            });

            collapsible_section(ui, regular::SCROLL, "Script", "scripting", &theme, "cs_script", false, |ui| {
                inline_property(ui, 0, "File", &theme, |ui| { ui.label("player.rhai"); });
                inline_property(ui, 1, "Active", &theme, |ui| { ui.label("true"); });
            });

            collapsible_section(ui, regular::ATOM, "Physics", "physics", &theme, "cs_physics", false, |ui| {
                inline_property(ui, 0, "Mass", &theme, |ui| { ui.label("1.0 kg"); });
                inline_property(ui, 1, "Drag", &theme, |ui| { ui.label("0.1"); });
                inline_property(ui, 2, "Gravity", &theme, |ui| { ui.label("Enabled"); });
            });

            ui.add_space(12.0);

            // ── collapsible_section_removable ─────────────────────
            section_header(ui, "collapsible_section_removable", &theme);

            let categories = [
                (regular::CUBE, "Mesh Renderer", "rendering"),
                (regular::PAINT_BRUSH, "Material", "rendering"),
                (regular::SPEAKER_HIGH, "Audio Emitter", "audio"),
                (regular::HEART, "Health", "gameplay"),
            ];

            for (i, (icon, label, cat)) in categories.iter().enumerate() {
                let is_disabled = state.disabled_sections.get(i).copied().unwrap_or(false);
                let id_src = format!("csr_{}", i);
                let action = collapsible_section_removable(
                    ui, icon, label, cat, &theme, &id_src, i < 2, true, is_disabled,
                    |ui| {
                        inline_property(ui, 0, "Setting A", &theme, |ui| { ui.label("value"); });
                        inline_property(ui, 1, "Setting B", &theme, |ui| { ui.label("value"); });
                    },
                );

                if action.toggle_clicked {
                    if let Some(d) = state.disabled_sections.get_mut(i) {
                        *d = !*d;
                    }
                }
            }

            ui.add_space(8.0);
        });
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Center
    }
}

// ── Tile Grid panel ─────────────────────────────────────────────────────────

pub struct TileGridGallery {
    state: RwLock<TileGridState>,
}

struct TileGridState {
    selected: Option<usize>,
    zoom: f32,
}

impl Default for TileGridGallery {
    fn default() -> Self {
        Self {
            state: RwLock::new(TileGridState {
                selected: None,
                zoom: 1.0,
            }),
        }
    }
}

struct TileItem {
    name: &'static str,
    icon: &'static str,
    color: Color32,
}

const TILE_ITEMS: &[TileItem] = &[
    TileItem { name: "player.glb", icon: regular::CUBE, color: Color32::from_rgb(99, 178, 238) },
    TileItem { name: "enemy.glb", icon: regular::CUBE, color: Color32::from_rgb(230, 89, 89) },
    TileItem { name: "tree.glb", icon: regular::TREE, color: Color32::from_rgb(89, 191, 115) },
    TileItem { name: "stone_wall.png", icon: regular::IMAGE, color: Color32::from_rgb(180, 160, 120) },
    TileItem { name: "grass.png", icon: regular::IMAGE, color: Color32::from_rgb(120, 200, 120) },
    TileItem { name: "metal.png", icon: regular::IMAGE, color: Color32::from_rgb(160, 170, 190) },
    TileItem { name: "footstep.wav", icon: regular::SPEAKER_HIGH, color: Color32::from_rgb(180, 100, 220) },
    TileItem { name: "ambient.ogg", icon: regular::MUSIC_NOTES, color: Color32::from_rgb(100, 180, 220) },
    TileItem { name: "explosion_fx.ron", icon: regular::SPARKLE, color: Color32::from_rgb(255, 180, 50) },
    TileItem { name: "main_scene.scn", icon: regular::FILM_SCRIPT, color: Color32::from_rgb(69, 101, 151) },
    TileItem { name: "player_ctrl.rhai", icon: regular::SCROLL, color: Color32::from_rgb(236, 154, 120) },
    TileItem { name: "water_shader.wgsl", icon: regular::MONITOR, color: Color32::from_rgb(180, 130, 255) },
];

impl EditorPanel for TileGridGallery {
    fn id(&self) -> &str {
        "gallery_tiles"
    }

    fn title(&self) -> &str {
        "Tile Grid"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::GRID_FOUR)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };
        let mut state = self.state.write().unwrap();

        ui.add_space(4.0);

        // Zoom slider
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Zoom:")
                    .size(11.0)
                    .color(theme.text.muted.to_color32()),
            );
            ui.add(egui::Slider::new(&mut state.zoom, 0.5..=2.0).show_value(false));
            ui.label(
                egui::RichText::new(format!("{:.0}%", state.zoom * 100.0))
                    .size(11.0)
                    .color(theme.text.muted.to_color32()),
            );
        });
        ui.add_space(4.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            let grid = TileGrid::new(&theme)
                .zoom(state.zoom)
                .available_width(ui.available_width());

            grid.show(ui, TILE_ITEMS.len(), |ui, index, tile| {
                let item = &TILE_ITEMS[index];
                let is_selected = state.selected == Some(index);
                let is_hovered = tile.response.hovered();

                // Draw icon
                ui.painter().text(
                    tile.icon_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    item.icon,
                    egui::FontId::proportional(tile.icon_size),
                    if is_selected || is_hovered {
                        Color32::WHITE
                    } else {
                        dim_color(item.color, 0.8)
                    },
                );

                // Draw label
                let (line1, line2) = split_label_two_lines(item.name, tile.tile_size, tile.font_size);
                let label_color = if is_selected {
                    Color32::WHITE
                } else {
                    theme.text.primary.to_color32()
                };

                ui.painter().text(
                    tile.label_line1_pos(),
                    egui::Align2::CENTER_CENTER,
                    &line1,
                    egui::FontId::proportional(tile.font_size),
                    label_color,
                );
                if !line2.is_empty() {
                    ui.painter().text(
                        tile.label_line2_pos(),
                        egui::Align2::CENTER_CENTER,
                        &line2,
                        egui::FontId::proportional(tile.font_size),
                        dim_color(label_color, 0.7),
                    );
                }

                // Handle click
                if tile.response.clicked() {
                    state.selected = Some(index);
                }

                TileState {
                    is_selected,
                    is_hovered,
                    color: Some(item.color),
                }
            });
        });
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Center
    }
}

// ── Tree Widget panel ───────────────────────────────────────────────────────

pub struct TreeGallery {
    state: RwLock<TreeState>,
}

struct TreeState {
    expanded: [bool; 5],
    selected: Option<usize>,
}

impl Default for TreeGallery {
    fn default() -> Self {
        Self {
            state: RwLock::new(TreeState {
                expanded: [true, true, false, true, false],
                selected: None,
            }),
        }
    }
}

struct TreeNode {
    label: &'static str,
    icon: &'static str,
    icon_color: Color32,
    label_color: Option<[u8; 3]>,
    children: &'static [TreeNode],
}

const SCENE_TREE: &[TreeNode] = &[
    TreeNode {
        label: "World",
        icon: regular::GLOBE,
        icon_color: Color32::from_rgb(100, 180, 220),
        label_color: None,
        children: &[
            TreeNode {
                label: "Environment",
                icon: regular::SUN,
                icon_color: Color32::from_rgb(247, 207, 100),
                label_color: Some([134, 188, 126]),
                children: &[
                    TreeNode { label: "Directional Light", icon: regular::SUN, icon_color: Color32::from_rgb(247, 207, 100), label_color: Some([247, 207, 100]), children: &[] },
                    TreeNode { label: "Skybox", icon: regular::CLOUD, icon_color: Color32::from_rgb(150, 180, 220), label_color: Some([134, 188, 126]), children: &[] },
                ],
            },
            TreeNode {
                label: "Player",
                icon: regular::USER,
                icon_color: Color32::from_rgb(99, 178, 238),
                label_color: Some([99, 178, 238]),
                children: &[
                    TreeNode { label: "Camera", icon: regular::CAMERA, icon_color: Color32::from_rgb(178, 132, 209), label_color: Some([178, 132, 209]), children: &[] },
                    TreeNode { label: "Mesh", icon: regular::CUBE, icon_color: Color32::from_rgb(99, 178, 238), label_color: None, children: &[] },
                    TreeNode { label: "Collider", icon: regular::BOUNDING_BOX, icon_color: Color32::from_rgb(120, 200, 200), label_color: Some([120, 200, 200]), children: &[] },
                ],
            },
            TreeNode {
                label: "Enemies",
                icon: regular::SKULL,
                icon_color: Color32::from_rgb(230, 89, 89),
                label_color: Some([230, 89, 89]),
                children: &[
                    TreeNode { label: "Zombie_01", icon: regular::CUBE, icon_color: Color32::from_rgb(230, 89, 89), label_color: None, children: &[] },
                    TreeNode { label: "Zombie_02", icon: regular::CUBE, icon_color: Color32::from_rgb(230, 89, 89), label_color: None, children: &[] },
                ],
            },
            TreeNode {
                label: "Props",
                icon: regular::PACKAGE,
                icon_color: Color32::from_rgb(180, 160, 120),
                label_color: None,
                children: &[
                    TreeNode { label: "Barrel", icon: regular::CYLINDER, icon_color: Color32::from_rgb(180, 140, 100), label_color: None, children: &[] },
                    TreeNode { label: "Crate", icon: regular::CUBE, icon_color: Color32::from_rgb(160, 140, 100), label_color: None, children: &[] },
                ],
            },
        ],
    },
];

impl EditorPanel for TreeGallery {
    fn id(&self) -> &str {
        "gallery_tree"
    }

    fn title(&self) -> &str {
        "Tree Widget"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::TREE_STRUCTURE)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };
        let mut state = self.state.write().unwrap();

        ui.add_space(4.0);
        section_header(ui, "Scene Hierarchy", &theme);

        egui::ScrollArea::vertical().show(ui, |ui| {
            let mut row_index = 0;
            let mut node_index = 0;
            let mut parent_lines = Vec::new();

            fn render_tree(
                ui: &mut egui::Ui,
                nodes: &[TreeNode],
                depth: usize,
                parent_lines: &mut Vec<bool>,
                row_index: &mut usize,
                node_index: &mut usize,
                state: &mut TreeState,
                theme: &renzora::theme::Theme,
            ) {
                for (i, node) in nodes.iter().enumerate() {
                    let is_last = i == nodes.len() - 1;
                    let my_index = *node_index;
                    let has_children = !node.children.is_empty();
                    let is_expanded = state
                        .expanded
                        .get(my_index)
                        .copied()
                        .unwrap_or(false);

                    let result = tree_row(
                        ui,
                        &TreeRowConfig {
                            stable_id: None,
                            depth,
                            is_last,
                            parent_lines,
                            row_index: *row_index,
                            has_children,
                            is_expanded,
                            is_selected: state.selected == Some(my_index),
                            icon: Some(node.icon),
                            icon_color: Some(node.icon_color),
                            label: node.label,
                            label_color: node.label_color,
                            theme,
                            prefix_width: 0.0,
                        },
                    );

                    if result.expand_toggled {
                        if let Some(e) = state.expanded.get_mut(my_index) {
                            *e = !*e;
                        }
                    }
                    if result.clicked {
                        state.selected = Some(my_index);
                    }

                    *row_index += 1;
                    *node_index += 1;

                    if is_expanded && has_children {
                        parent_lines.push(!is_last);
                        render_tree(
                            ui,
                            node.children,
                            depth + 1,
                            parent_lines,
                            row_index,
                            node_index,
                            state,
                            theme,
                        );
                        parent_lines.pop();
                    }
                }
            }

            render_tree(
                ui,
                SCENE_TREE,
                0,
                &mut parent_lines,
                &mut row_index,
                &mut node_index,
                &mut state,
                &theme,
            );
        });
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Left
    }
}

// ── Mixer gallery ───────────────────────────────────────────────────────────

use renzora::editor::{
    rotary_knob, KnobConfig,
    vertical_fader, FaderConfig,
    vu_meter, VuMeterConfig, VuMeterValue,
    mixer_channel_strip, MixerStripConfig, MixerStripState,
};

struct MixerGalleryState {
    knob_a: f32,
    knob_b: f32,
    knob_c: f32,
    fader_a: f32,
    fader_b: f32,
    vu_level: f32,
    vu_peak: f32,
    vu_level_r: f32,
    vu_peak_r: f32,
    strips: Vec<(MixerStripConfig, MixerStripState)>,
    time: f64,
}

impl Default for MixerGalleryState {
    fn default() -> Self {
        let names = ["Kick", "Snare", "Hi-Hat", "Bass", "Synth", "Vox"];
        let colors = [
            Color32::from_rgb(230, 89, 89),
            Color32::from_rgb(242, 166, 64),
            Color32::from_rgb(89, 191, 115),
            Color32::from_rgb(100, 180, 255),
            Color32::from_rgb(178, 132, 209),
            Color32::from_rgb(255, 180, 220),
        ];
        let volumes = [0.80, 0.65, 0.50, 0.70, 0.55, 0.60];

        let strips = names
            .iter()
            .zip(colors.iter())
            .zip(volumes.iter())
            .map(|((name, &color), &vol)| {
                let cfg = MixerStripConfig {
                    name: name.to_string(),
                    color,
                };
                let mut st = MixerStripState::default();
                st.volume = vol;
                (cfg, st)
            })
            .collect();

        Self {
            knob_a: 0.5,
            knob_b: 0.75,
            knob_c: 0.25,
            fader_a: 0.7,
            fader_b: 0.4,
            vu_level: 0.0,
            vu_peak: 0.0,
            vu_level_r: 0.0,
            vu_peak_r: 0.0,
            strips,
            time: 0.0,
        }
    }
}

pub struct MixerGallery {
    state: RwLock<MixerGalleryState>,
}

impl Default for MixerGallery {
    fn default() -> Self {
        Self {
            state: RwLock::new(MixerGalleryState::default()),
        }
    }
}

impl EditorPanel for MixerGallery {
    fn id(&self) -> &str {
        "gallery_mixer"
    }

    fn title(&self) -> &str {
        "Mixer"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::SPEAKER_HIGH)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => tm.active_theme.clone(),
            None => return,
        };
        let mut state = self.state.write().unwrap();

        // Animate VU meters
        let dt = ui.ctx().input(|i| i.stable_dt) as f64;
        state.time += dt;
        let t = state.time;

        // Animated levels for standalone VU demos
        state.vu_level = (0.5 + 0.4 * (t * 3.0).sin() as f32 + 0.1 * (t * 7.3).sin() as f32).clamp(0.0, 1.0);
        state.vu_level_r = (0.45 + 0.35 * (t * 2.7).sin() as f32 + 0.15 * (t * 5.9).sin() as f32).clamp(0.0, 1.0);
        // Peak decay
        if state.vu_level > state.vu_peak {
            state.vu_peak = state.vu_level;
        } else {
            state.vu_peak = (state.vu_peak - 0.3 * dt as f32).max(state.vu_level);
        }
        if state.vu_level_r > state.vu_peak_r {
            state.vu_peak_r = state.vu_level_r;
        } else {
            state.vu_peak_r = (state.vu_peak_r - 0.3 * dt as f32).max(state.vu_level_r);
        }

        // Animate strip VU levels
        let freqs = [4.1, 3.3, 5.7, 2.8, 4.5, 3.0];
        for (i, (_cfg, strip)) in state.strips.iter_mut().enumerate() {
            let freq = freqs.get(i).copied().unwrap_or(3.0);
            let base = strip.volume * 0.8;
            strip.level_l = (base + 0.2 * (t * freq).sin() as f32 + 0.05 * (t * freq * 2.1).sin() as f32).clamp(0.0, 1.0);
            strip.level_r = (base + 0.18 * (t * freq * 0.9).sin() as f32 + 0.06 * (t * freq * 1.7).sin() as f32).clamp(0.0, 1.0);
            if strip.level_l > strip.peak_l {
                strip.peak_l = strip.level_l;
            } else {
                strip.peak_l = (strip.peak_l - 0.3 * dt as f32).max(strip.level_l);
            }
            if strip.level_r > strip.peak_r {
                strip.peak_r = strip.level_r;
            } else {
                strip.peak_r = (strip.peak_r - 0.3 * dt as f32).max(strip.level_r);
            }
        }

        ui.ctx().request_repaint();

        let audio_accent = theme.categories.audio.accent.to_color32();

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(6.0);

            // ── Rotary Knobs ───────────────────────────────────
            section_header(ui, "Rotary Knobs", &theme);

            ui.horizontal(|ui| {
                let knob_cfg = |label: &str| KnobConfig {
                    size: 48.0,
                    min: 0.0,
                    max: 1.0,
                    color: audio_accent,
                    track_color: dim_color(audio_accent, 0.3),
                    label: Some(label.into()),
                };

                ui.add_space(8.0);
                rotary_knob(ui, egui::Id::new("mixer_knob_a"), &mut state.knob_a, &knob_cfg("Gain"));
                ui.add_space(12.0);
                rotary_knob(ui, egui::Id::new("mixer_knob_b"), &mut state.knob_b, &knob_cfg("Freq"));
                ui.add_space(12.0);
                rotary_knob(ui, egui::Id::new("mixer_knob_c"), &mut state.knob_c, &knob_cfg("Res"));
            });

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Drag up/down to change. Shift for fine control. Double-click to reset.")
                    .size(10.0)
                    .color(theme.text.muted.to_color32()),
            );

            ui.add_space(12.0);

            // ── Vertical Faders ────────────────────────────────
            section_header(ui, "Vertical Faders", &theme);

            ui.horizontal(|ui| {
                ui.add_space(8.0);
                vertical_fader(
                    ui,
                    egui::Id::new("mixer_fader_a"),
                    &mut state.fader_a,
                    &FaderConfig {
                        width: 32.0,
                        height: 120.0,
                        label: Some("Main".into()),
                        label_color: theme.text.muted.to_color32(),
                        track_color: theme.surfaces.extreme.to_color32(),
                        handle_color: theme.widgets.inactive_bg.to_color32(),
                        ..Default::default()
                    },
                );
                ui.add_space(16.0);
                vertical_fader(
                    ui,
                    egui::Id::new("mixer_fader_b"),
                    &mut state.fader_b,
                    &FaderConfig {
                        width: 32.0,
                        height: 120.0,
                        label: Some("Aux".into()),
                        label_color: theme.text.muted.to_color32(),
                        track_color: theme.surfaces.extreme.to_color32(),
                        handle_color: theme.widgets.inactive_bg.to_color32(),
                        ..Default::default()
                    },
                );
            });

            ui.add_space(12.0);

            // ── VU Meters ──────────────────────────────────────
            section_header(ui, "VU Meters", &theme);

            ui.horizontal(|ui| {
                ui.add_space(8.0);

                // Mono meter
                ui.vertical(|ui| {
                    let val = VuMeterValue {
                        level: state.vu_level,
                        peak: state.vu_peak,
                    };
                    vu_meter(ui, &val, None, &VuMeterConfig {
                        width: 12.0,
                        height: 100.0,
                        stereo: false,
                        gap: 2.0,
                    });
                    ui.label(
                        egui::RichText::new("Mono")
                            .size(9.0)
                            .color(theme.text.muted.to_color32()),
                    );
                });

                ui.add_space(16.0);

                // Stereo meter
                ui.vertical(|ui| {
                    let left = VuMeterValue {
                        level: state.vu_level,
                        peak: state.vu_peak,
                    };
                    let right = VuMeterValue {
                        level: state.vu_level_r,
                        peak: state.vu_peak_r,
                    };
                    vu_meter(ui, &left, Some(&right), &VuMeterConfig {
                        width: 10.0,
                        height: 100.0,
                        stereo: true,
                        gap: 2.0,
                    });
                    ui.label(
                        egui::RichText::new("Stereo")
                            .size(9.0)
                            .color(theme.text.muted.to_color32()),
                    );
                });
            });

            ui.add_space(12.0);

            // ── Channel Strips ─────────────────────────────────
            section_header(ui, "Channel Strips", &theme);

            ui.horizontal(|ui| {
                for (i, (cfg, strip_state)) in state.strips.iter_mut().enumerate() {
                    mixer_channel_strip(
                        ui,
                        egui::Id::new("mixer_strip").with(i),
                        strip_state,
                        cfg,
                        &theme,
                    );
                    ui.add_space(2.0);
                }
            });

            ui.add_space(8.0);
        });
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Center
    }
}

// ── Node Graph Gallery ─────────────────────────────────────────────────────

fn build_demo_graph() -> NodeGraphState {
    // Pin colors
    let flow   = Color32::from_rgb(220, 220, 220); // white-ish
    let float  = Color32::from_rgb(100, 200, 100); // green
    let vec3   = Color32::from_rgb(230, 160, 60);  // orange
    let bool_c = Color32::from_rgb(200, 60, 60);   // red
    let color  = Color32::from_rgb(160, 100, 220);  // purple

    // Header colors by category
    let event_hdr     = Color32::from_rgb(180, 60, 60);
    let math_hdr      = Color32::from_rgb(60, 140, 100);
    let logic_hdr     = Color32::from_rgb(140, 100, 180);
    let transform_hdr = Color32::from_rgb(100, 150, 220);
    let render_hdr    = Color32::from_rgb(200, 150, 120);

    let nodes = vec![
        // 1: Event / OnUpdate
        NodeDef {
            id: 1,
            title: "OnUpdate".into(),
            header_color: event_hdr,
            position: [-350.0, -120.0],
            pins: vec![
                PinDef { name: "flow_out".into(), label: "Exec".into(), color: flow, shape: PinShape::Triangle, direction: PinDirection::Output },
                PinDef { name: "delta".into(), label: "Delta".into(), color: float, shape: PinShape::Circle, direction: PinDirection::Output },
            ],
            thumbnail: None,
        },
        // 2: Branch
        NodeDef {
            id: 2,
            title: "Branch".into(),
            header_color: logic_hdr,
            position: [-100.0, -120.0],
            pins: vec![
                PinDef { name: "flow_in".into(), label: "Exec".into(), color: flow, shape: PinShape::Triangle, direction: PinDirection::Input },
                PinDef { name: "condition".into(), label: "Condition".into(), color: bool_c, shape: PinShape::Circle, direction: PinDirection::Input },
                PinDef { name: "true_out".into(), label: "True".into(), color: flow, shape: PinShape::Triangle, direction: PinDirection::Output },
                PinDef { name: "false_out".into(), label: "False".into(), color: flow, shape: PinShape::Triangle, direction: PinDirection::Output },
            ],
            thumbnail: None,
        },
        // 3: Float constant A
        NodeDef {
            id: 3,
            title: "Float".into(),
            header_color: math_hdr,
            position: [-350.0, 80.0],
            pins: vec![
                PinDef { name: "value".into(), label: "3.14".into(), color: float, shape: PinShape::Circle, direction: PinDirection::Output },
            ],
            thumbnail: None,
        },
        // 4: Float constant B
        NodeDef {
            id: 4,
            title: "Float".into(),
            header_color: math_hdr,
            position: [-350.0, 180.0],
            pins: vec![
                PinDef { name: "value".into(), label: "2.0".into(), color: float, shape: PinShape::Circle, direction: PinDirection::Output },
            ],
            thumbnail: None,
        },
        // 5: Add
        NodeDef {
            id: 5,
            title: "Add".into(),
            header_color: math_hdr,
            position: [-100.0, 100.0],
            pins: vec![
                PinDef { name: "a".into(), label: "A".into(), color: float, shape: PinShape::Circle, direction: PinDirection::Input },
                PinDef { name: "b".into(), label: "B".into(), color: float, shape: PinShape::Circle, direction: PinDirection::Input },
                PinDef { name: "result".into(), label: "Result".into(), color: float, shape: PinShape::Circle, direction: PinDirection::Output },
            ],
            thumbnail: None,
        },
        // 6: Multiply
        NodeDef {
            id: 6,
            title: "Multiply".into(),
            header_color: math_hdr,
            position: [150.0, 100.0],
            pins: vec![
                PinDef { name: "a".into(), label: "A".into(), color: float, shape: PinShape::Circle, direction: PinDirection::Input },
                PinDef { name: "b".into(), label: "B".into(), color: float, shape: PinShape::Circle, direction: PinDirection::Input },
                PinDef { name: "result".into(), label: "Result".into(), color: float, shape: PinShape::Circle, direction: PinDirection::Output },
            ],
            thumbnail: None,
        },
        // 7: GetPosition
        NodeDef {
            id: 7,
            title: "GetPosition".into(),
            header_color: transform_hdr,
            position: [-100.0, 280.0],
            pins: vec![
                PinDef { name: "position".into(), label: "XYZ".into(), color: vec3, shape: PinShape::Circle, direction: PinDirection::Output },
            ],
            thumbnail: None,
        },
        // 8: SetColor
        NodeDef {
            id: 8,
            title: "SetColor".into(),
            header_color: render_hdr,
            position: [150.0, 260.0],
            pins: vec![
                PinDef { name: "flow_in".into(), label: "Exec".into(), color: flow, shape: PinShape::Triangle, direction: PinDirection::Input },
                PinDef { name: "color_in".into(), label: "Color".into(), color: color, shape: PinShape::Circle, direction: PinDirection::Input },
                PinDef { name: "position".into(), label: "Pos".into(), color: vec3, shape: PinShape::Circle, direction: PinDirection::Input },
                PinDef { name: "flow_out".into(), label: "Exec".into(), color: flow, shape: PinShape::Triangle, direction: PinDirection::Output },
            ],
            thumbnail: None,
        },
    ];

    let connections = vec![
        // OnUpdate → Branch (flow)
        ConnectionDef { from_node: 1, from_pin: "flow_out".into(), to_node: 2, to_pin: "flow_in".into(), color: None },
        // Branch true → SetColor (flow)
        ConnectionDef { from_node: 2, from_pin: "true_out".into(), to_node: 8, to_pin: "flow_in".into(), color: None },
        // Float A → Add.a
        ConnectionDef { from_node: 3, from_pin: "value".into(), to_node: 5, to_pin: "a".into(), color: None },
        // Float B → Add.b
        ConnectionDef { from_node: 4, from_pin: "value".into(), to_node: 5, to_pin: "b".into(), color: None },
        // Add → Multiply.a
        ConnectionDef { from_node: 5, from_pin: "result".into(), to_node: 6, to_pin: "a".into(), color: None },
        // GetPosition → SetColor.position
        ConnectionDef { from_node: 7, from_pin: "position".into(), to_node: 8, to_pin: "position".into(), color: None },
    ];

    NodeGraphState {
        nodes,
        connections,
        offset: [0.0, 0.0],
        zoom: 1.0,
        selected: Vec::new(),
        dragging: None,
        connecting: None,
        box_select: None,
    }
}

struct NodeGraphGalleryState {
    graph: NodeGraphState,
}

impl Default for NodeGraphGalleryState {
    fn default() -> Self {
        Self { graph: build_demo_graph() }
    }
}

pub struct NodeGraphGallery {
    state: RwLock<NodeGraphGalleryState>,
}

impl Default for NodeGraphGallery {
    fn default() -> Self {
        Self { state: RwLock::new(NodeGraphGalleryState::default()) }
    }
}

impl EditorPanel for NodeGraphGallery {
    fn id(&self) -> &str {
        "gallery_node_graph"
    }

    fn title(&self) -> &str {
        "Node Graph"
    }

    fn icon(&self) -> Option<&str> {
        Some(regular::GRAPH)
    }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        let theme = match world.get_resource::<ThemeManager>() {
            Some(tm) => &tm.active_theme,
            None => return,
        };

        let config = NodeGraphConfig {
            canvas_bg: theme.material.canvas_bg.to_color32(),
            grid_dot: theme.material.grid_dot.to_color32(),
            node_bg: theme.material.node_bg.to_color32(),
            node_border: theme.material.node_border.to_color32(),
            selected_border: theme.material.node_selected_border.to_color32(),
            selection_fill: theme.material.selection_rect_fill.to_color32(),
            selection_stroke: theme.material.selection_rect_stroke.to_color32(),
            text_color: theme.text.primary.to_color32(),
            text_muted: theme.text.muted.to_color32(),
            ..NodeGraphConfig::default()
        };

        let mut state = self.state.write().unwrap();
        node_graph(ui, egui::Id::new("gallery_node_graph_widget"), &mut state.graph, &config);
    }

    fn default_location(&self) -> PanelLocation {
        PanelLocation::Center
    }
}

// ── Gallery layout ──────────────────────────────────────────────────────────

fn gallery_layout() -> WorkspaceLayout {
    WorkspaceLayout {
        name: "Gallery".into(),
        tree: DockTree::horizontal(
            DockTree::vertical(
                DockTree::leaf("gallery_tree"),
                DockTree::leaf("gallery_properties"),
                0.5,
            ),
            DockTree::horizontal(
                DockTree::vertical(
                    DockTree::Leaf {
                        tabs: vec!["gallery_controls".into(), "gallery_mixer".into()],
                        active_tab: 0,
                    },
                    DockTree::leaf("gallery_categories"),
                    0.5,
                ),
                DockTree::vertical(
                    DockTree::leaf("gallery_tiles"),
                    DockTree::leaf("gallery_node_graph"),
                    0.5,
                ),
                0.55,
            ),
            0.25,
        ),
    }
}

// ── Plugin ──────────────────────────────────────────────────────────────────

/// Plugin that registers the widget gallery panels and layout.
#[derive(Default)]
pub struct WidgetGalleryPlugin;

impl Plugin for WidgetGalleryPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] WidgetGalleryPlugin");
        // Register panels
        app.register_panel(ControlsGallery::default());
        app.register_panel(PropertiesGallery::default());
        app.register_panel(CategoriesGallery::default());
        app.register_panel(TileGridGallery::default());
        app.register_panel(TreeGallery::default());
        app.register_panel(MixerGallery::default());
        app.register_panel(NodeGraphGallery::default());

        // Add gallery layout
        let world = app.world_mut();
        let mut layouts = world
            .remove_resource::<LayoutManager>()
            .unwrap_or_default();

        layouts.layouts.push(gallery_layout());

        world.insert_resource(layouts);
    }
}

renzora::add!(WidgetGalleryPlugin, Editor);
