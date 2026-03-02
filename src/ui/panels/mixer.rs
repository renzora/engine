//! Audio Mixer panel — DAW-style redesign
//!
//! Custom rotary pan knobs, segmented LED VU meters, and draggable fader strips.

use std::f32::consts::PI;

use bevy_egui::egui::{
    self, Color32, CornerRadius, FontId, Painter, Pos2, Rect, RichText, Sense, Stroke, Vec2,
};
use renzora_theme::Theme;

use crate::audio::mixer::{ChannelStrip, MixerState};

// ── Layout ────────────────────────────────────────────────────────────────────
const STRIP_W: f32 = 88.0;
const STRIP_MARGIN: f32 = 4.0;
const ACCENT_H: f32 = 3.0;   // top accent bar
const HEADER_H: f32 = 28.0;
const KNOB_AREA_H: f32 = 50.0;
const FADER_MIN_H: f32 = 96.0;
const DB_H: f32 = 16.0;
const BTN_H: f32 = 18.0;
const PAD: f32 = 4.0;
const STRIP_MIN_H: f32 =
    ACCENT_H + HEADER_H + KNOB_AREA_H + FADER_MIN_H + DB_H + BTN_H + PAD * 7.0;

// Knob geometry: 7 o'clock → 5 o'clock (300° clockwise sweep)
const KNOB_START: f32 = PI * 2.0 / 3.0; // 120° — 7 o'clock
const KNOB_SWEEP: f32 = PI * 5.0 / 3.0; // 300°

// ── Palette ───────────────────────────────────────────────────────────────────
const STRIP_BG: Color32 = Color32::from_rgb(20, 21, 26);
const STRIP_BORDER: Color32 = Color32::from_rgb(40, 42, 52);
const KNOB_BODY: Color32 = Color32::from_rgb(34, 36, 46);
const KNOB_RIM: Color32 = Color32::from_rgb(60, 63, 78);
const KNOB_TRACK_BG: Color32 = Color32::from_rgb(25, 27, 35);
const FADER_TRACK_BG: Color32 = Color32::from_rgb(10, 11, 14);
const FADER_TRACK_BORDER: Color32 = Color32::from_rgb(35, 37, 48);
const FADER_CAP_FG: Color32 = Color32::from_rgb(205, 207, 218);
const FADER_CAP_LINE: Color32 = Color32::from_rgb(95, 98, 115);
const FADER_CAP_BORDER: Color32 = Color32::from_rgb(155, 158, 172);
const VU_SEG_OFF: Color32 = Color32::from_rgb(20, 22, 30);
const VU_GREEN: Color32 = Color32::from_rgb(24, 195, 75);
const VU_YELLOW: Color32 = Color32::from_rgb(210, 188, 25);
const VU_RED: Color32 = Color32::from_rgb(208, 52, 36);
const MUTE_ON: Color32 = Color32::from_rgb(196, 58, 38);
const SOLO_ON: Color32 = Color32::from_rgb(196, 165, 28);
const BTN_OFF: Color32 = Color32::from_rgb(36, 38, 48);
const BTN_BORDER: Color32 = Color32::from_rgb(52, 55, 68);

// ── Bus accent ────────────────────────────────────────────────────────────────
#[derive(Clone, Copy)]
enum BusColor {
    Master,
    Sfx,
    Music,
    Ambient,
    Custom(usize),
}

impl BusColor {
    fn accent(self) -> Color32 {
        match self {
            Self::Master    => Color32::from_rgb(195, 197, 212),
            Self::Sfx       => Color32::from_rgb(228, 132,  52),
            Self::Music     => Color32::from_rgb(135,  90, 228),
            Self::Ambient   => Color32::from_rgb( 48, 196, 140),
            Self::Custom(i) => {
                let p = [
                    Color32::from_rgb(208,  75,  75),
                    Color32::from_rgb( 75, 162, 220),
                    Color32::from_rgb(205, 192,  52),
                    Color32::from_rgb(160,  78, 205),
                ];
                p[i % p.len()]
            }
        }
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn render_mixer_content(ui: &mut egui::Ui, mixer: &mut MixerState, theme: &Theme) {
    let panel_bg = theme.surfaces.panel.to_color32();
    let muted_color = theme.text.muted.to_color32();

    // Handle rename commit/cancel before rendering (from previous frame)
    if let Some(idx) = mixer.renaming_bus {
        let committed = ui.ctx().data_mut(|d| {
            d.get_temp::<bool>(egui::Id::new("bus_rename_committed")).unwrap_or(false)
        });
        let cancelled = ui.ctx().data_mut(|d| {
            d.get_temp::<bool>(egui::Id::new("bus_rename_cancelled")).unwrap_or(false)
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
            let available_h = (ui.available_height() - 2.0).max(STRIP_MIN_H);

            egui::ScrollArea::horizontal()
                .id_salt("mixer_scroll")
                .show(ui, |ui| {
                    ui.set_height(available_h);
                    ui.horizontal(|ui| {
                        ui.set_height(available_h);

                        // ── Built-in buses ────────────────────────────────────────
                        render_strip(ui, "MASTER", &mut mixer.master, BusColor::Master, available_h, false, false, &mut None, false, false);
                        strip_divider(ui, available_h);
                        render_strip(ui, "SFX",     &mut mixer.sfx,     BusColor::Sfx,     available_h, false, false, &mut None, false, false);
                        render_strip(ui, "MUSIC",   &mut mixer.music,   BusColor::Music,   available_h, false, false, &mut None, false, false);
                        render_strip(ui, "AMBIENT", &mut mixer.ambient, BusColor::Ambient, available_h, false, false, &mut None, false, false);

                        if !mixer.custom_buses.is_empty() {
                            strip_divider(ui, available_h);
                        }

                        // ── Custom buses ──────────────────────────────────────────
                        let pointer_pos = ui.ctx().pointer_hover_pos();
                        let n = mixer.custom_buses.len();
                        let mut strip_rects: Vec<Rect> = Vec::with_capacity(n);
                        let mut header_rects: Vec<Rect> = Vec::with_capacity(n);
                        let mut start_rename: Option<usize> = None;
                        let mut start_drag: Option<usize> = None;
                        let mut delete_idx: Option<usize> = None;

                        for i in 0..n {
                            let name = mixer.custom_buses[i].0.clone();
                            let is_renaming = mixer.renaming_bus == Some(i);
                            let is_drag_source = mixer.dragging_bus == Some(i);

                            // Drag target highlight
                            let is_drag_target = if let (Some(src), Some(pos)) = (mixer.dragging_bus, pointer_pos) {
                                // We'll check after rendering using strip_rects
                                false && src != i && pos.x > 0.0 // placeholder
                            } else {
                                false
                            };

                            let mut rename_buf_opt = if is_renaming {
                                Some(&mut mixer.rename_buf)
                            } else {
                                None
                            };

                            let interaction = render_strip(
                                ui, &name, &mut mixer.custom_buses[i].1,
                                BusColor::Custom(i), available_h,
                                true, is_renaming, &mut rename_buf_opt,
                                is_drag_source, is_drag_target,
                            );

                            strip_rects.push(interaction.strip_rect);
                            header_rects.push(interaction.header_rect);

                            // Double-click header → rename
                            if interaction.header_double_clicked && !is_renaming {
                                start_rename = Some(i);
                            }

                            // Drag on header
                            let drag_resp = ui.interact(
                                interaction.header_rect,
                                egui::Id::new("bus_drag").with(i),
                                Sense::drag(),
                            );
                            if drag_resp.drag_started() && !is_renaming {
                                start_drag = Some(i);
                            }
                        }

                        // ── Drag target overlay + drop ───────────────────────────
                        if let Some(src) = mixer.dragging_bus {
                            let mut drop_at: Option<usize> = None;
                            if let Some(pos) = pointer_pos {
                                for (i, rect) in strip_rects.iter().enumerate() {
                                    if i != src && rect.contains(pos) {
                                        drop_at = Some(i);
                                        // Accent border on drop target
                                        ui.painter().rect_stroke(
                                            *rect, CornerRadius::same(5),
                                            Stroke::new(2.0, BusColor::Custom(i).accent()),
                                            egui::StrokeKind::Inside,
                                        );
                                        break;
                                    }
                                }
                            }
                            if ui.input(|i| i.pointer.any_released()) {
                                if let Some(dst) = drop_at {
                                    let bus = mixer.custom_buses.remove(src);
                                    let insert_at = if src < dst { dst - 1 } else { dst };
                                    mixer.custom_buses.insert(insert_at, bus);
                                    // Adjust renaming index if needed
                                    if let Some(ref mut ri) = mixer.renaming_bus {
                                        if *ri == src { *ri = insert_at; }
                                    }
                                }
                                mixer.dragging_bus = None;
                            }
                        }

                        // ── Apply deferred actions ────────────────────────────────
                        if let Some(i) = start_drag {
                            mixer.dragging_bus = Some(i);
                        }
                        if let Some(i) = start_rename {
                            mixer.rename_buf = mixer.custom_buses[i].0.clone();
                            mixer.renaming_bus = Some(i);
                        }

                        // ── Context menu (right-click on custom bus header) ──────
                        for i in 0..header_rects.len() {
                            let resp = ui.interact(
                                header_rects[i],
                                egui::Id::new("bus_ctx").with(i),
                                Sense::click(),
                            );
                            resp.context_menu(|ui| {
                                ui.set_min_width(120.0);
                                if ui.button("Rename").clicked() {
                                    start_rename = Some(i);
                                    ui.close_menu();
                                }
                                if ui.button(RichText::new("Delete").color(Color32::from_rgb(220, 75, 55))).clicked() {
                                    delete_idx = Some(i);
                                    ui.close_menu();
                                }
                            });
                        }

                        // ── Delete bus ─────────────────────────────────────────────
                        if let Some(idx) = delete_idx {
                            if idx < mixer.custom_buses.len() {
                                mixer.custom_buses.remove(idx);
                                if mixer.renaming_bus == Some(idx) {
                                    mixer.renaming_bus = None;
                                }
                                if mixer.dragging_bus == Some(idx) {
                                    mixer.dragging_bus = None;
                                }
                            }
                        }

                        // ── Add bus button ─────────────────────────────────────────
                        ui.add_space(8.0);
                        if mixer.adding_bus {
                            ui.vertical(|ui| {
                                ui.set_width(120.0);
                                ui.add_space(8.0);
                                ui.label(RichText::new("Bus name").size(10.5).color(muted_color));
                                let resp = ui.text_edit_singleline(&mut mixer.new_bus_name);
                                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                                    mixer.adding_bus = false;
                                }
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    let ok = !mixer.new_bus_name.trim().is_empty();
                                    ui.add_enabled_ui(ok, |ui| {
                                        if ui.button("Create").clicked() {
                                            let name = mixer.new_bus_name.trim().to_string();
                                            mixer.custom_buses.push((name, ChannelStrip::default()));
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
                                ui.set_width(STRIP_W);
                                ui.add_space(available_h * 0.5 - 14.0);
                                let add_btn = egui::Button::new(
                                    RichText::new("+ Bus").size(10.5).color(Color32::from_rgb(130, 133, 152)),
                                )
                                .fill(Color32::from_rgb(26, 28, 36))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(50, 53, 66)));
                                if ui.add(add_btn).on_hover_text("Add mixer bus").clicked() {
                                    mixer.adding_bus = true;
                                }
                            });
                        }
                    });
                });
        });
}

// ── Channel strip ─────────────────────────────────────────────────────────────

fn strip_divider(ui: &mut egui::Ui, h: f32) {
    ui.add_space(STRIP_MARGIN);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(1.0, h), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::ZERO, Color32::from_rgb(38, 40, 50));
    ui.add_space(STRIP_MARGIN);
}

/// Return value from render_strip for custom bus interactions
struct StripInteraction {
    header_rect: Rect,
    strip_rect: Rect,
    header_double_clicked: bool,
    header_secondary_clicked: bool,
}

fn render_strip(
    ui: &mut egui::Ui,
    label: &str,
    strip: &mut ChannelStrip,
    color: BusColor,
    available_h: f32,
    is_custom: bool,
    is_renaming: bool,
    rename_buf: &mut Option<&mut String>,
    is_drag_source: bool,
    is_drag_target: bool,
) -> StripInteraction {
    let accent = color.accent();
    let (strip_rect, strip_resp) = ui.allocate_exact_size(Vec2::new(STRIP_W, available_h), Sense::click_and_drag());
    let p = ui.painter();

    // Drag source visual: slight transparency
    let bg = if is_drag_source {
        Color32::from_rgba_premultiplied(STRIP_BG.r(), STRIP_BG.g(), STRIP_BG.b(), 140)
    } else {
        STRIP_BG
    };

    // Background + border
    p.rect_filled(strip_rect, CornerRadius::same(5), bg);
    let border = if is_drag_target {
        Stroke::new(2.0, accent)
    } else {
        Stroke::new(1.0, STRIP_BORDER)
    };
    p.rect_stroke(
        strip_rect,
        CornerRadius::same(5),
        border,
        egui::StrokeKind::Outside,
    );

    // Top accent bar (3px)
    let accent_rect = Rect::from_min_size(strip_rect.min, Vec2::new(STRIP_W, ACCENT_H));
    p.rect_filled(accent_rect, CornerRadius::same(4), accent);

    // Inner content area (below accent bar)
    let content_rect = Rect::from_min_size(
        strip_rect.min + Vec2::new(0.0, ACCENT_H),
        Vec2::new(STRIP_W, available_h - ACCENT_H),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(content_rect)
            .layout(egui::Layout::top_down(egui::Align::Center)),
    );

    // ── Header ────────────────────────────────────────────────────────────────
    let header_bg = Color32::from_rgba_premultiplied(
        (accent.r() as u32 * 28 / 255) as u8,
        (accent.g() as u32 * 28 / 255) as u8,
        (accent.b() as u32 * 28 / 255) as u8,
        255,
    );
    let (header_rect, header_resp) = child.allocate_exact_size(Vec2::new(STRIP_W, HEADER_H), Sense::click());
    child.painter().rect_filled(header_rect, CornerRadius::ZERO, header_bg);

    if is_renaming {
        // Inline rename text input
        if let Some(buf) = rename_buf.as_mut() {
            let text_rect = header_rect.shrink2(Vec2::new(4.0, 4.0));
            let mut rename_child = child.new_child(
                egui::UiBuilder::new()
                    .max_rect(text_rect)
                    .layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight)),
            );
            let resp = rename_child.add(
                egui::TextEdit::singleline(*buf)
                    .font(FontId::monospace(9.0))
                    .horizontal_align(egui::Align::Center)
                    .desired_width(text_rect.width()),
            );
            if resp.lost_focus() {
                let escaped = child.input(|i| i.key_pressed(egui::Key::Escape));
                if !escaped {
                    child.ctx().data_mut(|d| d.insert_temp(egui::Id::new("bus_rename_committed"), true));
                } else {
                    child.ctx().data_mut(|d| d.insert_temp(egui::Id::new("bus_rename_cancelled"), true));
                }
            } else if !resp.has_focus() {
                resp.request_focus();
            }
        }
    } else {
        let shortened = if label.len() > 8 { &label[..8] } else { label };
        child.painter().text(
            header_rect.center(),
            egui::Align2::CENTER_CENTER,
            shortened,
            FontId::monospace(9.5),
            accent,
        );

        // Drag cursor for custom buses
        if is_custom && header_resp.hovered() {
            child.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
    }

    child.add_space(PAD);

    // ── Pan knob ──────────────────────────────────────────────────────────────
    {
        let knob_r = 15.0_f32;
        let (knob_area, knob_resp) =
            child.allocate_exact_size(Vec2::new(STRIP_W, KNOB_AREA_H), Sense::click_and_drag());

        let knob_center = Pos2::new(knob_area.center().x, knob_area.min.y + knob_r + 6.0);

        if knob_resp.dragged() {
            let d = knob_resp.drag_delta();
            strip.panning = (strip.panning + ((d.x - d.y * 0.4) / 90.0) as f64).clamp(-1.0, 1.0);
        }
        if knob_resp.double_clicked() {
            strip.panning = 0.0;
        }

        let pan_norm = ((strip.panning + 1.0) / 2.0) as f32;
        draw_knob(child.painter(), knob_center, knob_r, pan_norm, accent, true);

        // "PAN" label above
        child.painter().text(
            Pos2::new(knob_center.x, knob_area.min.y + 2.0),
            egui::Align2::CENTER_TOP,
            "PAN",
            FontId::monospace(7.0),
            Color32::from_rgb(72, 75, 92),
        );

        // Value label below knob
        let pan_str = if strip.panning.abs() < 0.01 {
            "C".to_string()
        } else if strip.panning < 0.0 {
            format!("L{:.0}", strip.panning.abs() * 100.0)
        } else {
            format!("R{:.0}", strip.panning * 100.0)
        };
        child.painter().text(
            Pos2::new(knob_center.x, knob_area.max.y - 4.0),
            egui::Align2::CENTER_BOTTOM,
            pan_str,
            FontId::monospace(8.5),
            Color32::from_rgb(110, 113, 132),
        );
    }

    child.add_space(PAD);

    // ── Mute / Solo (compact, centered) ─────────────────────────────────────────
    {
        let bw = 28.0_f32;
        let bh = 18.0_f32;
        let gap = 3.0_f32;
        let total_w = bw * 2.0 + gap;
        let inset = (STRIP_W - total_w) * 0.5;

        child.horizontal(|ui| {
            ui.add_space(inset);

            // Mute
            let (mf, mt, ms) = if strip.muted {
                (MUTE_ON, Color32::WHITE, Stroke::new(1.0, Color32::from_rgb(225, 85, 58)))
            } else {
                (BTN_OFF, Color32::from_rgb(155, 158, 172), Stroke::new(1.0, BTN_BORDER))
            };
            if ui
                .add_sized(
                    Vec2::new(bw, bh),
                    egui::Button::new(RichText::new("M").size(9.0).strong().color(mt))
                        .fill(mf)
                        .stroke(ms)
                        .corner_radius(CornerRadius::same(3)),
                )
                .on_hover_text("Mute")
                .clicked()
            {
                strip.muted = !strip.muted;
            }

            ui.add_space(gap);

            // Solo
            let (sf, st, ss) = if strip.soloed {
                (SOLO_ON, Color32::from_rgb(18, 18, 8), Stroke::new(1.0, Color32::from_rgb(225, 195, 45)))
            } else {
                (BTN_OFF, Color32::from_rgb(155, 158, 172), Stroke::new(1.0, BTN_BORDER))
            };
            if ui
                .add_sized(
                    Vec2::new(bw, bh),
                    egui::Button::new(RichText::new("S").size(9.0).strong().color(st))
                        .fill(sf)
                        .stroke(ss)
                        .corner_radius(CornerRadius::same(3)),
                )
                .on_hover_text("Solo")
                .clicked()
            {
                strip.soloed = !strip.soloed;
            }
        });
    }

    child.add_space(PAD);

    // ── Fader & Level Meter (Side by side) ─────────────────────────────────────
    let fader_area_h = (available_h - child.min_rect().height() - DB_H - PAD * 4.0 - ACCENT_H).max(FADER_MIN_H);

    child.horizontal(|ui| {
        ui.add_space(PAD);

        // Fader column
        ui.vertical(|ui| {
            render_fader(ui, &mut strip.volume, strip.muted, accent, 48.0, fader_area_h);

            ui.add_space(2.0);

            // dB readout below fader
            let (db_rect, _) = ui.allocate_exact_size(Vec2::new(48.0, DB_H), Sense::hover());
            let (db_str, db_color) = if strip.muted {
                ("MUTED".to_string(), Color32::from_rgb(175, 75, 50))
            } else {
                let db = linear_to_db(strip.volume);
                let s = if db.is_infinite() {
                    "-\u{221E}".to_string()
                } else {
                    format!("{:+.1}", db)
                };
                (s, Color32::from_rgb(125, 128, 148))
            };
            ui.painter().text(
                db_rect.center(),
                egui::Align2::CENTER_CENTER,
                db_str,
                FontId::monospace(8.0),
                db_color,
            );
        });

        ui.add_space(4.0);

        // Level meter to the right
        let (vu_rect, _) = ui.allocate_exact_size(Vec2::new(22.0, fader_area_h), Sense::hover());
        draw_vu_segmented(ui.painter(), vu_rect, strip.peak_level, strip.muted, accent);
    });

    ui.add_space(STRIP_MARGIN);

    StripInteraction {
        header_rect,
        strip_rect,
        header_double_clicked: is_custom && header_resp.double_clicked(),
        header_secondary_clicked: is_custom && header_resp.secondary_clicked(),
    }
}

// ── Custom fader ──────────────────────────────────────────────────────────────

fn render_fader(ui: &mut egui::Ui, volume: &mut f64, muted: bool, accent: Color32, w: f32, h: f32) {
    let (area, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click_and_drag());

    // Input handling
    if resp.dragged() {
        let dy = resp.drag_delta().y;
        *volume = (*volume - (dy as f64 / h as f64) * 1.5).clamp(0.0, 1.5);
    }
    if resp.double_clicked() {
        *volume = 1.0;
    }
    if resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let norm = 1.0 - ((pos.y - area.min.y) / h).clamp(0.0, 1.0);
            *volume = (norm as f64 * 1.5).clamp(0.0, 1.5);
        }
    }

    let p = ui.painter();
    let cap_h = 12.0_f32;
    let track_w = 5.0_f32;
    let cap_w = w * 0.75;

    // Fader y-mapping: volume 0..1.5 → y max..min (accounting for cap half-height)
    let usable_h = h - cap_h;
    let vol_to_y = |v: f64| -> f32 {
        area.max.y - cap_h * 0.5 - (v / 1.5).clamp(0.0, 1.0) as f32 * usable_h
    };
    let fader_y = vol_to_y(*volume);

    // dB scale marks on the right side of the track
    let scale_x = area.center().x + track_w * 0.5 + 5.0;
    let scale_marks: &[(&str, f64)] = &[
        ("+3", 1.5),
        ("  0", 1.0),
        (" -6", 0.501),
        ("-12", 0.251),
        ("-24", 0.063),
    ];
    for (lbl, v) in scale_marks {
        let y = vol_to_y(*v);
        p.line_segment(
            [Pos2::new(scale_x, y), Pos2::new(scale_x + 4.0, y)],
            Stroke::new(1.0, Color32::from_rgb(50, 53, 66)),
        );
        p.text(
            Pos2::new(scale_x + 5.0, y),
            egui::Align2::LEFT_CENTER,
            *lbl,
            FontId::monospace(7.0),
            Color32::from_rgb(62, 65, 80),
        );
    }

    // Track groove
    let track_x = area.center().x - track_w * 0.5;
    let track_top = area.min.y + cap_h * 0.5;
    let track_bot = area.max.y - cap_h * 0.5;
    let track_rect = Rect::from_min_max(
        Pos2::new(track_x, track_top),
        Pos2::new(track_x + track_w, track_bot),
    );
    p.rect_filled(track_rect, CornerRadius::same(3), FADER_TRACK_BG);
    p.rect_stroke(
        track_rect,
        CornerRadius::same(3),
        Stroke::new(1.0, FADER_TRACK_BORDER),
        egui::StrokeKind::Outside,
    );

    // Filled portion below fader cap (accent tinted)
    let fill_color = if muted {
        Color32::from_rgb(48, 25, 22)
    } else {
        Color32::from_rgba_premultiplied(accent.r(), accent.g(), accent.b(), 70)
    };
    if fader_y < track_bot - 1.0 {
        let fill_rect = Rect::from_min_max(
            Pos2::new(track_x + 1.0, fader_y),
            Pos2::new(track_x + track_w - 1.0, track_bot),
        );
        p.rect_filled(fill_rect, CornerRadius::same(2), fill_color);
    }

    // Unity (0 dB) marker — prominent tick across track
    let unity_y = vol_to_y(1.0);
    p.line_segment(
        [
            Pos2::new(track_x - 4.0, unity_y),
            Pos2::new(track_x + track_w + 4.0, unity_y),
        ],
        Stroke::new(1.5, Color32::from_rgb(95, 98, 115)),
    );

    // Fader cap shadow
    p.rect_filled(
        Rect::from_center_size(
            Pos2::new(area.center().x, fader_y + 2.0),
            Vec2::new(cap_w, cap_h),
        ),
        CornerRadius::same(3),
        Color32::from_black_alpha(90),
    );

    // Fader cap body
    let cap_rect = Rect::from_center_size(Pos2::new(area.center().x, fader_y), Vec2::new(cap_w, cap_h));
    let cap_fill = if muted { Color32::from_rgb(95, 97, 110) } else { FADER_CAP_FG };
    p.rect_filled(cap_rect, CornerRadius::same(3), cap_fill);
    p.rect_stroke(
        cap_rect,
        CornerRadius::same(3),
        Stroke::new(1.0, FADER_CAP_BORDER),
        egui::StrokeKind::Outside,
    );

    // Center line on cap
    let cy = cap_rect.center().y;
    p.line_segment(
        [
            Pos2::new(cap_rect.min.x + 5.0, cy),
            Pos2::new(cap_rect.max.x - 5.0, cy),
        ],
        Stroke::new(1.5, FADER_CAP_LINE),
    );
    // Grip notches
    for dx in [-5.0_f32, 0.0, 5.0] {
        let nx = cap_rect.center().x + dx;
        p.line_segment(
            [Pos2::new(nx, cy - 3.0), Pos2::new(nx, cy + 3.0)],
            Stroke::new(1.0, Color32::from_rgb(135, 138, 155)),
        );
    }

    // Hover glow on cap
    if resp.hovered() {
        p.rect_stroke(
            cap_rect.expand(1.0),
            CornerRadius::same(4),
            Stroke::new(1.0, Color32::from_rgba_premultiplied(accent.r(), accent.g(), accent.b(), 100)),
            egui::StrokeKind::Outside,
        );
    }
}

// ── Rotary knob ───────────────────────────────────────────────────────────────

fn draw_knob(
    p: &Painter,
    center: Pos2,
    radius: f32,
    normalized: f32, // 0..1
    accent: Color32,
    center_detent: bool,
) {
    // Drop shadow
    p.circle_filled(center + Vec2::new(0.0, 1.5), radius, Color32::from_black_alpha(100));

    // Outer arc track (background)
    let arc_r = radius + 3.5;
    draw_arc(p, center, arc_r, KNOB_START, KNOB_START + KNOB_SWEEP, Stroke::new(2.5, KNOB_TRACK_BG));

    // Filled arc (value)
    let val_angle = KNOB_START + normalized * KNOB_SWEEP;
    let center_angle = KNOB_START + KNOB_SWEEP * 0.5; // 12 o'clock
    if center_detent {
        // Pan: fill from center toward value
        let (a0, a1) = if val_angle < center_angle {
            (val_angle, center_angle)
        } else {
            (center_angle, val_angle)
        };
        if a1 - a0 > 0.04 {
            draw_arc(p, center, arc_r, a0, a1, Stroke::new(2.5, accent));
        }
        // Center dot on arc
        let cx = center.x + arc_r * center_angle.cos();
        let cy = center.y + arc_r * center_angle.sin();
        p.circle_filled(Pos2::new(cx, cy), 1.5, Color32::from_rgb(80, 84, 100));
    } else {
        if val_angle - KNOB_START > 0.04 {
            draw_arc(p, center, arc_r, KNOB_START, val_angle, Stroke::new(2.5, accent));
        }
    }

    // Body: outer rim then filled body
    p.circle_filled(center, radius, KNOB_BODY);
    p.circle_stroke(center, radius, Stroke::new(1.5, KNOB_RIM));

    // Inner highlight ring
    p.circle_stroke(
        center - Vec2::new(0.0, radius * 0.05),
        radius * 0.78,
        Stroke::new(0.5, Color32::from_rgba_premultiplied(255, 255, 255, 12)),
    );

    // Indicator line
    let ind_inner = radius * 0.28;
    let ind_outer = radius * 0.80;
    let ix = val_angle.cos();
    let iy = val_angle.sin();
    p.line_segment(
        [
            center + Vec2::new(ix * ind_inner, iy * ind_inner),
            center + Vec2::new(ix * ind_outer, iy * ind_outer),
        ],
        Stroke::new(2.0, Color32::WHITE),
    );
    // Accent dot at indicator tip
    p.circle_filled(
        center + Vec2::new(ix * ind_outer, iy * ind_outer),
        1.8,
        accent,
    );
}

fn draw_arc(p: &Painter, center: Pos2, radius: f32, start: f32, end: f32, stroke: Stroke) {
    let steps = ((end - start).abs() * radius).ceil() as usize;
    let steps = steps.max(6).min(80);
    let pts: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let a = start + (end - start) * (i as f32 / steps as f32);
            Pos2::new(center.x + radius * a.cos(), center.y + radius * a.sin())
        })
        .collect();
    for w in pts.windows(2) {
        p.line_segment([w[0], w[1]], stroke);
    }
}

// ── Segmented VU meter ────────────────────────────────────────────────────────

fn draw_vu_segmented(p: &Painter, rect: Rect, peak: f32, muted: bool, accent: Color32) {
    p.rect_filled(rect, CornerRadius::same(3), Color32::from_rgb(12, 13, 17));

    const N: usize = 22;
    const GAP: f32 = 1.5;
    let col_gap = 2.0_f32;
    let col_w = (rect.width() - col_gap) / 2.0;
    let seg_pitch = rect.height() / N as f32;
    let seg_h = (seg_pitch - GAP).max(1.0);

    let fill = if muted { 0.0_f32 } else { (peak / 1.5).clamp(0.0, 1.0) };

    // Simulate slight L/R difference
    let fills = [fill, (fill * 0.92).min(fill)];

    for col in 0..2usize {
        let x = rect.min.x + col as f32 * (col_w + col_gap);
        let lit = (fills[col] * N as f32).round() as usize;

        for seg in 0..N {
            let y = rect.max.y - (seg + 1) as f32 * seg_pitch + GAP * 0.5;
            let sr = Rect::from_min_size(Pos2::new(x, y), Vec2::new(col_w, seg_h));

            let color = if seg >= lit {
                VU_SEG_OFF
            } else if seg < 13 {
                VU_GREEN
            } else if seg < 18 {
                VU_YELLOW
            } else {
                VU_RED
            };

            // Dimmer inactive look vs bright active
            let draw_c = if seg >= lit {
                color
            } else {
                // Slight brightness boost on lit segments
                Color32::from_rgba_premultiplied(
                    (color.r() as u16 * 220 / 255) as u8,
                    (color.g() as u16 * 220 / 255) as u8,
                    (color.b() as u16 * 220 / 255) as u8,
                    255,
                )
            };

            p.rect_filled(sr, CornerRadius::same(1), draw_c);
        }
    }

    // Unity line (0 dB at 2/3 from bottom)
    let uy = rect.max.y - rect.height() * (1.0 / 1.5);
    p.line_segment(
        [Pos2::new(rect.min.x - 2.0, uy), Pos2::new(rect.max.x + 2.0, uy)],
        Stroke::new(1.0, Color32::from_rgb(62, 65, 80)),
    );

    // Muted overlay
    if muted {
        p.rect_filled(
            rect,
            CornerRadius::same(3),
            Color32::from_rgba_premultiplied(80, 25, 18, 50),
        );
    }

    // Accent tint on active segments (subtle)
    if !muted && fill > 0.02 {
        let fill_h = rect.height() * fill * (1.0 / 1.5);
        let tint_rect = Rect::from_min_size(
            Pos2::new(rect.min.x, rect.max.y - fill_h),
            Vec2::new(rect.width(), fill_h),
        );
        p.rect_filled(
            tint_rect,
            CornerRadius::same(2),
            Color32::from_rgba_premultiplied(accent.r(), accent.g(), accent.b(), 12),
        );
    }

    p.rect_stroke(
        rect,
        CornerRadius::same(3),
        Stroke::new(1.0, Color32::from_rgb(32, 34, 44)),
        egui::StrokeKind::Outside,
    );
}

// ── Utilities ─────────────────────────────────────────────────────────────────

fn linear_to_db(linear: f64) -> f64 {
    if linear <= 0.0 {
        return f64::NEG_INFINITY;
    }
    20.0 * linear.log10()
}
