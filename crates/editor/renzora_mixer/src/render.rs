//! Audio Mixer panel rendering using renzora_ui widgets.
//!
//! Delegates to `mixer_channel_strip` for each bus, with custom bus management
//! (add, rename, delete, drag-reorder).

use bevy_egui::egui::{self, Color32, RichText, Sense, Stroke, Vec2};
use renzora_audio::{ChannelStrip, MixerState};
use renzora_editor::{
    mixer_channel_strip, MixerStripConfig, MixerStripState,
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

fn render_bus_strip(
    ui: &mut egui::Ui,
    id_salt: &str,
    name: &str,
    strip: &mut ChannelStrip,
    accent: Color32,
    theme: &Theme,
) {
    let mut ws = strip_to_widget(strip);
    let config = MixerStripConfig {
        name: name.to_string(),
        color: accent,
    };
    let id = ui.id().with(id_salt);
    if mixer_channel_strip(ui, id, &mut ws, &config, theme) {
        widget_to_strip(&ws, strip);
    }
    // Always sync peak levels back (they're read-only from the widget's perspective)
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

pub fn render_mixer_content(
    ui: &mut egui::Ui,
    mixer: &mut MixerState,
    panel_bg: Color32,
    muted_color: Color32,
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

                        // Built-in buses
                        render_bus_strip(
                            ui,
                            "master",
                            "MASTER",
                            &mut mixer.master,
                            bus_accent("MASTER", None),
                            &theme,
                        );
                        strip_divider(ui, available_h);
                        render_bus_strip(
                            ui,
                            "sfx",
                            "SFX",
                            &mut mixer.sfx,
                            bus_accent("SFX", None),
                            &theme,
                        );
                        render_bus_strip(
                            ui,
                            "music",
                            "MUSIC",
                            &mut mixer.music,
                            bus_accent("MUSIC", None),
                            &theme,
                        );
                        render_bus_strip(
                            ui,
                            "ambient",
                            "AMBIENT",
                            &mut mixer.ambient,
                            bus_accent("AMBIENT", None),
                            &theme,
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
                                &mut mixer.custom_buses[i].1,
                                accent,
                                &theme,
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

                        // Add bus button
                        ui.add_space(8.0);
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
                            ui.vertical(|ui| {
                                ui.set_width(64.0);
                                ui.add_space(available_h * 0.5 - 14.0);
                                let add_btn = egui::Button::new(
                                    RichText::new("+ Bus")
                                        .size(10.5)
                                        .color(Color32::from_rgb(130, 133, 152)),
                                )
                                .fill(Color32::from_rgb(26, 28, 36))
                                .stroke(Stroke::new(
                                    1.0,
                                    Color32::from_rgb(50, 53, 66),
                                ));
                                if ui
                                    .add(add_btn)
                                    .on_hover_text("Add mixer bus")
                                    .clicked()
                                {
                                    mixer.adding_bus = true;
                                }
                            });
                        }
                    });
                });
        });
}
