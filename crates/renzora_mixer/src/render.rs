//! Audio Mixer panel rendering using renzora_ui widgets.
//!
//! Delegates to `mixer_channel_strip` for each bus, with custom bus management
//! (add, rename, delete, drag-reorder).

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, Pos2, RichText, Sense, Stroke, Vec2};
use egui_phosphor::regular;
use renzora_audio::{
    BusInsertsSummary, ChannelStrip, MixerFxCommand, MixerFxOp, MixerState, PluginCatalog,
};
use renzora_editor::{
    mixer_channel_strip, EditorCommands, MixerStripConfig, MixerStripState,
};
use renzora_theme::Theme;

/// Bus accent colors
fn bus_accent(name: &str, custom_index: Option<usize>) -> Color32 {
    match name {
        "MASTER" => Color32::from_rgb(195, 197, 212),
        "SFX" => Color32::from_rgb(228, 132, 52),
        "MUSIC" => Color32::from_rgb(135, 90, 228),
        "AMBIENT" => Color32::from_rgb(48, 196, 140),
        _ => {
            let palette = [
                Color32::from_rgb(208, 75, 75),
                Color32::from_rgb(75, 162, 220),
                Color32::from_rgb(205, 192, 52),
                Color32::from_rgb(160, 78, 205),
            ];
            palette[custom_index.unwrap_or(0) % palette.len()]
        }
    }
}

/// Convert between ChannelStrip (f64, 0–1.5) and MixerStripState (f32, 0–1).
fn strip_to_widget(strip: &ChannelStrip) -> MixerStripState {
    MixerStripState {
        volume: (strip.volume as f32 / 1.5).clamp(0.0, 1.0),
        pan: strip.panning as f32,
        mute: strip.muted,
        solo: strip.soloed,
        level_l: strip.peak_level.clamp(0.0, 1.0),
        level_r: (strip.peak_level * 0.92).clamp(0.0, 1.0),
        peak_l: strip.peak_level.clamp(0.0, 1.0),
        peak_r: (strip.peak_level * 0.92).clamp(0.0, 1.0),
    }
}

fn widget_to_strip(ws: &MixerStripState, strip: &mut ChannelStrip) {
    strip.volume = (ws.volume * 1.5) as f64;
    strip.panning = ws.pan as f64;
    strip.muted = ws.mute;
    strip.soloed = ws.solo;
}

#[allow(clippy::too_many_arguments)]
fn render_bus_strip(
    ui: &mut egui::Ui,
    id_salt: &str,
    name: &str,
    bus_key: &str,
    strip: &mut ChannelStrip,
    accent: Color32,
    theme: &Theme,
    inserts: Option<&BusInsertsSummary>,
    catalog: Option<&PluginCatalog>,
    commands: Option<&EditorCommands>,
) {
    let strip_width = 64.0;
    let id = ui.id().with(id_salt);
    ui.vertical(|ui| {
        ui.set_width(strip_width);
        // Compact strip-header bar: a single themed background pill with FX
        // (insert chain) on the left and the cog (device routing) on the
        // right. Using a frame ties the two controls together visually so
        // they read as "this strip's settings" rather than two stray icons.
        egui::Frame::new()
            .fill(theme.surfaces.faint.to_color32().gamma_multiply(0.85))
            .stroke(Stroke::new(0.5, theme.widgets.border.to_color32()))
            .corner_radius(3.0)
            .inner_margin(egui::Margin::symmetric(2, 1))
            .show(ui, |ui| {
                ui.set_width(strip_width - 4.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    render_strip_fx_button(
                        ui, id.with("fx"), bus_key, accent, theme, inserts, catalog, commands,
                    );
                    // Push cog to the right edge.
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            render_strip_settings_cog(ui, id.with("cog"), strip, theme);
                        },
                    );
                });
            });

        ui.add_space(2.0);

        let mut ws = strip_to_widget(strip);
        let config = MixerStripConfig {
            name: name.to_string(),
            color: accent,
        };
        if mixer_channel_strip(ui, id, &mut ws, &config, theme) {
            widget_to_strip(&ws, strip);
        }
    });
}

/// Helper to defer firing a `MixerFxCommand`. Captures the command into a
/// closure that runs with `&mut World` so we can grab `Messages<...>` and
/// `write` into it. Falls back to a warning log if the message resource
/// isn't registered (means the audio plugin wasn't initialised).
fn fire_fx_command(commands: Option<&EditorCommands>, cmd: MixerFxCommand) {
    let Some(cmds) = commands else { return };
    cmds.push(move |world| {
        if let Some(mut messages) = world.get_resource_mut::<bevy::ecs::message::Messages<MixerFxCommand>>() {
            messages.write(cmd);
        }
    });
}

/// Small "FX" button. Tints to the accent colour when the bus has plugin
/// slots loaded, faint muted grey otherwise. Click opens a popover with the
/// chain editor + a plugin picker.
#[allow(clippy::too_many_arguments)]
fn render_strip_fx_button(
    ui: &mut egui::Ui,
    id: egui::Id,
    bus_name: &str,
    accent: Color32,
    theme: &Theme,
    inserts: Option<&BusInsertsSummary>,
    catalog: Option<&PluginCatalog>,
    commands: Option<&EditorCommands>,
) {
    let chain_len = inserts.map(|s| s.slot_count(bus_name)).unwrap_or(0);
    let active = chain_len > 0;
    let icon_color = if active { accent } else { theme.text.muted.to_color32() };

    let label = if active {
        format!("FX·{}", chain_len)
    } else {
        "FX".to_string()
    };
    let btn = egui::Button::new(RichText::new(label).size(10.0).color(icon_color)).frame(false);
    let resp = ui
        .add_sized(Vec2::new(28.0, 20.0), btn)
        .on_hover_text("Plugin inserts");

    let popup_id = id.with("popup");
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
    }

    egui::popup_above_or_below_widget(
        ui,
        popup_id,
        &resp,
        egui::AboveOrBelow::Below,
        egui::PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            ui.set_min_width(280.0);
            ui.set_max_width(360.0);
            ui.spacing_mut().item_spacing.y = 4.0;

            ui.label(
                RichText::new(format!("Inserts — {}", bus_name))
                    .size(11.0)
                    .strong()
                    .color(theme.text.primary.to_color32()),
            );

            // Existing slots ----
            if chain_len == 0 {
                ui.label(
                    RichText::new("No plugins inserted on this bus.")
                        .size(10.5)
                        .italics()
                        .color(theme.text.muted.to_color32()),
                );
            } else if let Some(inserts) = inserts {
                egui::Frame::new()
                    .fill(theme.surfaces.faint.to_color32())
                    .inner_margin(egui::Margin::symmetric(4, 4))
                    .corner_radius(3.0)
                    .show(ui, |ui| {
                        for (i, slot) in inserts.slots(bus_name).iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!("{}.", i + 1))
                                        .size(10.5)
                                        .color(theme.text.muted.to_color32()),
                                );
                                let text_color = if slot.bypass {
                                    theme.text.muted.to_color32()
                                } else {
                                    theme.text.primary.to_color32()
                                };
                                ui.label(
                                    RichText::new(&slot.display_name)
                                        .size(10.5)
                                        .color(text_color),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let local_id = slot.local_id;
                                        let bus = bus_name.to_string();
                                        // Remove
                                        let remove = egui::Button::new(
                                            RichText::new(regular::TRASH)
                                                .size(10.0)
                                                .color(theme.text.muted.to_color32()),
                                        )
                                        .frame(false);
                                        if ui.add(remove).on_hover_text("Remove").clicked() {
                                            fire_fx_command(commands, MixerFxCommand {
                                                bus: bus.clone(),
                                                op: MixerFxOp::Remove { local_id },
                                            });
                                        }
                                        // Move down
                                        let dn = egui::Button::new(
                                            RichText::new(regular::CARET_DOWN)
                                                .size(10.0)
                                                .color(theme.text.muted.to_color32()),
                                        )
                                        .frame(false);
                                        if ui.add(dn).on_hover_text("Move down").clicked() {
                                            fire_fx_command(commands, MixerFxCommand {
                                                bus: bus.clone(),
                                                op: MixerFxOp::MoveDown { local_id },
                                            });
                                        }
                                        // Move up
                                        let up = egui::Button::new(
                                            RichText::new(regular::CARET_UP)
                                                .size(10.0)
                                                .color(theme.text.muted.to_color32()),
                                        )
                                        .frame(false);
                                        if ui.add(up).on_hover_text("Move up").clicked() {
                                            fire_fx_command(commands, MixerFxCommand {
                                                bus: bus.clone(),
                                                op: MixerFxOp::MoveUp { local_id },
                                            });
                                        }
                                        // Bypass toggle
                                        let bypass_color = if slot.bypass {
                                            theme.semantic.warning.to_color32()
                                        } else {
                                            theme.text.muted.to_color32()
                                        };
                                        let by = egui::Button::new(
                                            RichText::new("∅").size(11.0).color(bypass_color),
                                        )
                                        .frame(false);
                                        if ui.add(by).on_hover_text("Bypass").clicked() {
                                            fire_fx_command(commands, MixerFxCommand {
                                                bus: bus.clone(),
                                                op: MixerFxOp::ToggleBypass { local_id },
                                            });
                                        }

                                        // Open / Close floating editor
                                        // window. CLAP plugin GUIs draw
                                        // into their own OS window — we can
                                        // only ask the host to open/close
                                        // them, never embed them in egui.
                                        let editor_color = if slot.editor_open {
                                            theme.semantic.accent.to_color32()
                                        } else if slot.instance_loaded {
                                            theme.text.primary.to_color32()
                                        } else {
                                            theme.text.disabled.to_color32()
                                        };
                                        let editor_btn = egui::Button::new(
                                            RichText::new(regular::APP_WINDOW)
                                                .size(11.0)
                                                .color(editor_color),
                                        )
                                        .frame(false);
                                        let tip = if !slot.instance_loaded {
                                            "Editor unavailable — plugin host not loaded"
                                        } else if slot.editor_open {
                                            "Close editor window"
                                        } else {
                                            "Open editor (floating window)"
                                        };
                                        let resp = ui
                                            .add_enabled(slot.instance_loaded, editor_btn)
                                            .on_hover_text(tip);
                                        if resp.clicked() {
                                            let op = if slot.editor_open {
                                                MixerFxOp::CloseEditor { local_id }
                                            } else {
                                                MixerFxOp::OpenEditor { local_id }
                                            };
                                            fire_fx_command(
                                                commands,
                                                MixerFxCommand { bus, op },
                                            );
                                        }
                                    },
                                );
                            });
                        }
                    });
            }

            ui.add_space(4.0);

            // Add-plugin section ----
            if let Some(catalog) = catalog {
                if !catalog.host_present {
                    ui.label(
                        RichText::new("Audio plugin host not loaded.")
                            .size(10.5)
                            .italics()
                            .color(theme.text.muted.to_color32()),
                    );
                } else if catalog.scanning && catalog.plugins.is_empty() {
                    ui.label(
                        RichText::new("Scanning plugins…")
                            .size(10.5)
                            .italics()
                            .color(theme.text.muted.to_color32()),
                    );
                } else if catalog.plugins.is_empty() {
                    ui.label(
                        RichText::new("No CLAP plugins found in standard paths.")
                            .size(10.5)
                            .italics()
                            .color(theme.text.muted.to_color32()),
                    );
                    if catalog.last_scan_root_count > 0 {
                        ui.label(
                            RichText::new(format!(
                                "Searched {} location(s).",
                                catalog.last_scan_root_count
                            ))
                            .size(10.0)
                            .color(theme.text.muted.to_color32()),
                        );
                    }
                } else {
                    // Inline plugin list rendered directly in the FX popup.
                    // Earlier versions used a nested ComboBox here, but its
                    // dropdown renders outside the parent popup's rect, which
                    // `PopupCloseBehavior::CloseOnClickOutside` reads as a
                    // click-outside and dismisses both popups together. An
                    // inline scrollable list avoids the nested popup entirely
                    // and feels like a proper overlay menu.
                    ui.label(
                        RichText::new("Add plugin")
                            .size(10.5)
                            .strong()
                            .color(theme.text.primary.to_color32()),
                    );
                    let row_bg = theme.surfaces.faint.to_color32();
                    let hover_bg = theme.widgets.hovered_bg.to_color32();
                    egui::Frame::new()
                        .fill(row_bg)
                        .corner_radius(3.0)
                        .inner_margin(egui::Margin::symmetric(2, 2))
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .id_salt(("fx_add_list", bus_name))
                                .max_height(220.0)
                                .auto_shrink([false, true])
                                .show(ui, |ui| {
                                    ui.spacing_mut().item_spacing.y = 1.0;
                                    for entry in &catalog.plugins {
                                        let label = if entry.vendor.is_empty() {
                                            entry.name.clone()
                                        } else {
                                            format!(
                                                "{}  ·  {}",
                                                entry.name, entry.vendor
                                            )
                                        };
                                        let row_h = 20.0;
                                        let (rect, resp) = ui.allocate_exact_size(
                                            Vec2::new(ui.available_width(), row_h),
                                            Sense::click(),
                                        );
                                        if resp.hovered() {
                                            ui.painter().rect_filled(rect, 2.0, hover_bg);
                                            ui.ctx().set_cursor_icon(
                                                egui::CursorIcon::PointingHand,
                                            );
                                        }
                                        ui.painter().text(
                                            egui::Pos2::new(
                                                rect.left() + 6.0,
                                                rect.center().y,
                                            ),
                                            egui::Align2::LEFT_CENTER,
                                            &label,
                                            egui::FontId::proportional(10.5),
                                            theme.text.primary.to_color32(),
                                        );
                                        if resp.clicked() {
                                            fire_fx_command(
                                                commands,
                                                MixerFxCommand {
                                                    bus: bus_name.to_string(),
                                                    op: MixerFxOp::Add {
                                                        plugin_catalog_id: entry
                                                            .id
                                                            .clone(),
                                                    },
                                                },
                                            );
                                            // Close the popup so the user
                                            // sees the new insert in the
                                            // strip immediately.
                                            ui.memory_mut(|m| {
                                                m.close_popup(popup_id);
                                            });
                                        }
                                    }
                                });
                        });
                }
            }
        },
    );
}

/// Small frameless cog. Click opens a popover with input + output device
/// pickers for this bus. Visually unobtrusive when nothing is bound; tints
/// to the accent colour when a device is set so the user can spot which
/// strips are wired to hardware.
fn render_strip_settings_cog(
    ui: &mut egui::Ui,
    id: egui::Id,
    strip: &mut ChannelStrip,
    theme: &Theme,
) {
    let bound = strip.input_device.is_some() || strip.output_device.is_some();
    let icon_color = if bound {
        theme.semantic.accent.to_color32()
    } else {
        theme.text.muted.to_color32()
    };
    let btn = egui::Button::new(
        RichText::new(regular::GEAR).size(13.0).color(icon_color),
    )
    .frame(false);
    let resp = ui
        .add_sized(Vec2::new(20.0, 20.0), btn)
        .on_hover_text("Bus device routing");

    let popup_id = id.with("popup");
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup_id));
        // Snapshot device lists at toggle time so the popover doesn't hit
        // the OS device enumerator every frame while it's open. cpal's
        // input/output enumeration can take tens of ms on Windows.
        let inputs = renzora_audio::list_input_devices();
        let outputs = renzora_audio::list_output_devices();
        ui.ctx().data_mut(|d| {
            d.insert_temp::<Vec<String>>(popup_id.with("inputs"), inputs);
            d.insert_temp::<Vec<String>>(popup_id.with("outputs"), outputs);
        });
    }

    egui::popup_above_or_below_widget(
        ui,
        popup_id,
        &resp,
        egui::AboveOrBelow::Below,
        egui::PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            ui.set_min_width(240.0);
            ui.set_max_width(320.0);
            ui.spacing_mut().item_spacing.y = 4.0;

            let inputs: Vec<String> = ui.ctx().data(|d| {
                d.get_temp::<Vec<String>>(popup_id.with("inputs"))
                    .unwrap_or_default()
            });
            let outputs: Vec<String> = ui.ctx().data(|d| {
                d.get_temp::<Vec<String>>(popup_id.with("outputs"))
                    .unwrap_or_default()
            });

            section_label(ui, "Input device", theme);
            ui.label(
                RichText::new("Live mic / audio input routed into this bus.")
                    .size(10.0)
                    .color(theme.text.muted.to_color32()),
            );
            ui.add_space(2.0);
            inline_device_list(ui, &mut strip.input_device, &inputs, theme);

            ui.add_space(8.0);
            section_label(ui, "Output device", theme);
            ui.label(
                RichText::new("Reserved — per-bus output routing not yet wired up.")
                    .size(10.0)
                    .color(theme.text.muted.to_color32()),
            );
            ui.add_space(2.0);
            inline_device_list(ui, &mut strip.output_device, &outputs, theme);
        },
    );
}

fn section_label(ui: &mut egui::Ui, text: &str, theme: &Theme) {
    ui.label(
        RichText::new(text)
            .size(11.0)
            .strong()
            .color(theme.text.primary.to_color32()),
    );
}

/// Inline list of selectable rows — `(none)` plus one row per device. Used
/// in place of a nested `ComboBox` so the parent popover doesn't auto-close
/// when the user clicks a row (egui treats nested-popup clicks as
/// "outside", which would close the cog popover).
fn inline_device_list(
    ui: &mut egui::Ui,
    current: &mut Option<String>,
    devices: &[String],
    theme: &Theme,
) {
    egui::Frame::new()
        .fill(theme.surfaces.faint.to_color32())
        .inner_margin(egui::Margin::symmetric(4, 4))
        .corner_radius(3.0)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            if ui
                .selectable_label(current.is_none(), "(none)")
                .clicked()
            {
                *current = None;
            }
            if devices.is_empty() {
                ui.label(
                    RichText::new("No devices detected")
                        .italics()
                        .color(theme.text.muted.to_color32()),
                );
                return;
            }
            for name in devices {
                let selected = current.as_deref() == Some(name.as_str());
                if ui.selectable_label(selected, name).clicked() {
                    *current = Some(name.clone());
                }
            }
        });
}

fn strip_divider(ui: &mut egui::Ui, h: f32) {
    ui.add_space(4.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, h), Sense::hover());
    ui.painter().rect_filled(
        rect,
        egui::CornerRadius::ZERO,
        Color32::from_rgb(38, 40, 50),
    );
    ui.add_space(4.0);
}

/// Heavier divider used after the Master strip — pro DAWs visually separate
/// the main bus from the rest. A 2px line plus extra breathing room.
fn master_divider(ui: &mut egui::Ui, h: f32, theme: &Theme) {
    ui.add_space(8.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(2.0, h), Sense::hover());
    ui.painter().rect_filled(
        rect,
        egui::CornerRadius::ZERO,
        theme.widgets.border.to_color32().gamma_multiply(1.4),
    );
    ui.add_space(8.0);
}

/// Ghost strip used as the "+ Bus" placeholder. Same dimensions as a real
/// strip with a dashed-style outline so it integrates into the strip row
/// instead of floating in dead space.
fn render_add_bus_ghost(ui: &mut egui::Ui, mixer: &mut MixerState, theme: &Theme) -> bool {
    let strip_width = 64.0;
    let mut clicked = false;
    ui.vertical(|ui| {
        ui.set_width(strip_width);

        // Spacer matching the strip-header bar above the body so heights
        // line up with neighbouring strips.
        ui.add_space(20.0);

        let outline = theme.widgets.border.to_color32().gamma_multiply(0.85);
        let body_h = ui.available_height().max(40.0);
        let (rect, resp) = ui.allocate_exact_size(
            Vec2::new(strip_width, body_h),
            Sense::click(),
        );
        let painter = ui.painter_at(rect);

        let bg = if resp.hovered() {
            theme.widgets.hovered_bg.to_color32().gamma_multiply(0.6)
        } else {
            theme.surfaces.faint.to_color32().gamma_multiply(0.4)
        };
        painter.rect_filled(rect, 4.0, bg);

        // Faux dashed outline — egui doesn't natively dash strokes, so we
        // approximate by painting short segments around the perimeter.
        let dash = 4.0;
        let gap = 3.0;
        let stroke = Stroke::new(1.0, outline);
        let mut x = rect.min.x;
        while x < rect.max.x {
            let x2 = (x + dash).min(rect.max.x);
            painter.line_segment(
                [Pos2::new(x, rect.min.y), Pos2::new(x2, rect.min.y)],
                stroke,
            );
            painter.line_segment(
                [Pos2::new(x, rect.max.y), Pos2::new(x2, rect.max.y)],
                stroke,
            );
            x = x2 + gap;
        }
        let mut y = rect.min.y;
        while y < rect.max.y {
            let y2 = (y + dash).min(rect.max.y);
            painter.line_segment(
                [Pos2::new(rect.min.x, y), Pos2::new(rect.min.x, y2)],
                stroke,
            );
            painter.line_segment(
                [Pos2::new(rect.max.x, y), Pos2::new(rect.max.x, y2)],
                stroke,
            );
            y = y2 + gap;
        }

        let text_color = if resp.hovered() {
            theme.text.primary.to_color32()
        } else {
            theme.text.muted.to_color32()
        };
        painter.text(
            rect.center() - Vec2::new(0.0, 8.0),
            egui::Align2::CENTER_CENTER,
            regular::PLUS,
            egui::FontId::proportional(18.0),
            text_color,
        );
        painter.text(
            rect.center() + Vec2::new(0.0, 12.0),
            egui::Align2::CENTER_CENTER,
            "Add bus",
            egui::FontId::proportional(10.0),
            text_color,
        );

        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if resp.clicked() {
            mixer.adding_bus = true;
            clicked = true;
        }
    });
    clicked
}

#[allow(clippy::too_many_arguments)]
pub fn render_mixer_content(
    ui: &mut egui::Ui,
    mixer: &mut MixerState,
    panel_bg: Color32,
    muted_color: Color32,
    inserts: Option<&BusInsertsSummary>,
    catalog: Option<&PluginCatalog>,
    commands: Option<&EditorCommands>,
) {
    // Get theme from egui data or use a default
    let theme = renzora_theme::Theme::default();

    // Handle rename commit/cancel from previous frame
    if let Some(idx) = mixer.renaming_bus {
        let committed = ui.ctx().data_mut(|d| {
            d.get_temp::<bool>(egui::Id::new("bus_rename_committed"))
                .unwrap_or(false)
        });
        let cancelled = ui.ctx().data_mut(|d| {
            d.get_temp::<bool>(egui::Id::new("bus_rename_cancelled"))
                .unwrap_or(false)
        });
        if committed || cancelled {
            if committed {
                let trimmed = mixer.rename_buf.trim().to_string();
                if !trimmed.is_empty() && idx < mixer.custom_buses.len() {
                    mixer.custom_buses[idx].0 = trimmed;
                }
            }
            mixer.renaming_bus = None;
            mixer.rename_buf.clear();
            ui.ctx().data_mut(|d| {
                d.insert_temp(egui::Id::new("bus_rename_committed"), false);
                d.insert_temp(egui::Id::new("bus_rename_cancelled"), false);
            });
        }
    }

    egui::Frame::NONE
        .fill(panel_bg)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            let available_h = (ui.available_height() - 2.0).max(200.0);

            egui::ScrollArea::horizontal()
                .id_salt("mixer_scroll")
                .show(ui, |ui| {
                    ui.set_height(available_h);
                    ui.horizontal(|ui| {
                        ui.set_height(available_h);

                        // Built-in buses. The first string is the display
                        // label; the second is the bus key the audio engine
                        // routes by (also the key under which inserts live).
                        render_bus_strip(
                            ui, "master", "MASTER", "Master",
                            &mut mixer.master,
                            bus_accent("MASTER", None),
                            &theme,
                            inserts, catalog, commands,
                        );
                        // Heavier divider after Master so it visually
                        // separates the main bus from the routing strips.
                        master_divider(ui, available_h, &theme);
                        render_bus_strip(
                            ui, "sfx", "SFX", "Sfx",
                            &mut mixer.sfx,
                            bus_accent("SFX", None),
                            &theme,
                            inserts, catalog, commands,
                        );
                        render_bus_strip(
                            ui, "music", "MUSIC", "Music",
                            &mut mixer.music,
                            bus_accent("MUSIC", None),
                            &theme,
                            inserts, catalog, commands,
                        );
                        render_bus_strip(
                            ui, "ambient", "AMBIENT", "Ambient",
                            &mut mixer.ambient,
                            bus_accent("AMBIENT", None),
                            &theme,
                            inserts, catalog, commands,
                        );

                        if !mixer.custom_buses.is_empty() {
                            strip_divider(ui, available_h);
                        }

                        // Custom buses
                        let mut delete_idx: Option<usize> = None;
                        let mut start_rename: Option<usize> = None;

                        for i in 0..mixer.custom_buses.len() {
                            let name = mixer.custom_buses[i].0.clone();
                            let accent = bus_accent(&name, Some(i));

                            render_bus_strip(
                                ui,
                                &format!("custom_{}", i),
                                &name,
                                &name,
                                &mut mixer.custom_buses[i].1,
                                accent,
                                &theme,
                                inserts, catalog, commands,
                            );

                            // Context menu on the strip area
                            let strip_id = egui::Id::new("bus_ctx").with(i);
                            let resp = ui.interact(
                                ui.min_rect(),
                                strip_id,
                                Sense::click(),
                            );
                            resp.context_menu(|ui| {
                                ui.set_min_width(120.0);
                                if ui.button("Rename").clicked() {
                                    start_rename = Some(i);
                                    ui.close();
                                }
                                if ui
                                    .button(
                                        RichText::new("Delete")
                                            .color(Color32::from_rgb(220, 75, 55)),
                                    )
                                    .clicked()
                                {
                                    delete_idx = Some(i);
                                    ui.close();
                                }
                            });
                        }

                        // Apply deferred rename
                        if let Some(i) = start_rename {
                            mixer.rename_buf = mixer.custom_buses[i].0.clone();
                            mixer.renaming_bus = Some(i);
                        }

                        // Delete bus
                        if let Some(idx) = delete_idx {
                            if idx < mixer.custom_buses.len() {
                                mixer.custom_buses.remove(idx);
                                if mixer.renaming_bus == Some(idx) {
                                    mixer.renaming_bus = None;
                                }
                            }
                        }

                        // Add-bus affordance: a ghost-strip placeholder
                        // sized like a real strip so it integrates into the
                        // strip row instead of floating in dead space.
                        ui.add_space(4.0);
                        if mixer.adding_bus {
                            ui.vertical(|ui| {
                                ui.set_width(120.0);
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new("Bus name")
                                        .size(10.5)
                                        .color(muted_color),
                                );
                                let resp =
                                    ui.text_edit_singleline(&mut mixer.new_bus_name);
                                if resp.lost_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Escape))
                                {
                                    mixer.adding_bus = false;
                                }
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    let ok = !mixer.new_bus_name.trim().is_empty();
                                    ui.add_enabled_ui(ok, |ui| {
                                        if ui.button("Create").clicked() {
                                            let name =
                                                mixer.new_bus_name.trim().to_string();
                                            mixer.custom_buses.push((
                                                name,
                                                ChannelStrip::default(),
                                            ));
                                            mixer.new_bus_name.clear();
                                            mixer.adding_bus = false;
                                        }
                                    });
                                    if ui.button("Cancel").clicked() {
                                        mixer.adding_bus = false;
                                        mixer.new_bus_name.clear();
                                    }
                                });
                            });
                        } else {
                            // Click sets `adding_bus = true`, which switches
                            // to the text-entry form on the next frame.
                            let _ = render_add_bus_ghost(ui, mixer, &theme);
                        }
                    });
                });
        });
}
