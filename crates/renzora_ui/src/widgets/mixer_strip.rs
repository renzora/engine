//! Mixer channel strip — composite layout: name, pan knob, fader + VU, mute/solo, dB readout.

use bevy_egui::egui::{self, Color32, Sense, Vec2};
use renzora_theme::Theme;

use super::colors::dim_color;
use super::fader::{vertical_fader, FaderConfig};
use super::knob::{rotary_knob, KnobConfig};
use super::vu_meter::{vu_meter, VuMeterConfig, VuMeterValue};

/// Configuration for a mixer channel strip.
pub struct MixerStripConfig {
    /// Channel name displayed at the top.
    pub name: String,
    /// Accent color for the channel.
    pub color: Color32,
}

/// Mutable state for a mixer channel strip.
pub struct MixerStripState {
    /// Volume level (0.0–1.0).
    pub volume: f32,
    /// Pan position (-1.0 left to 1.0 right).
    pub pan: f32,
    /// Muted flag.
    pub mute: bool,
    /// Soloed flag.
    pub solo: bool,
    /// Current left channel level for the VU meter.
    pub level_l: f32,
    /// Current right channel level for the VU meter.
    pub level_r: f32,
    /// Peak hold left.
    pub peak_l: f32,
    /// Peak hold right.
    pub peak_r: f32,
}

impl Default for MixerStripState {
    fn default() -> Self {
        Self {
            volume: 0.75,
            pan: 0.0,
            mute: false,
            solo: false,
            level_l: 0.0,
            level_r: 0.0,
            peak_l: 0.0,
            peak_r: 0.0,
        }
    }
}

/// Paint an interactive mixer channel strip. Returns `true` if any value changed.
pub fn mixer_channel_strip(
    ui: &mut egui::Ui,
    id: egui::Id,
    state: &mut MixerStripState,
    config: &MixerStripConfig,
    theme: &Theme,
) -> bool {
    let mut changed = false;
    let strip_width = 64.0;

    ui.allocate_ui(Vec2::new(strip_width, ui.available_height()), |ui| {
        let bg = theme.surfaces.faint.to_color32();
        let frame = egui::Frame::new()
            .inner_margin(egui::Margin::same(4))
            .fill(bg)
            .corner_radius(4.0)
            .stroke(egui::Stroke::new(1.0, theme.widgets.border.to_color32()));

        frame.show(ui, |ui| {
            ui.set_width(strip_width - 10.0);
            ui.vertical_centered(|ui| {
                // ── Channel name ───────────────────────────────
                ui.label(
                    egui::RichText::new(&config.name)
                        .size(10.0)
                        .strong()
                        .color(config.color),
                );
                ui.add_space(4.0);

                // ── Pan knob ───────────────────────────────────
                let pan_knob_config = KnobConfig {
                    size: 32.0,
                    min: -1.0,
                    max: 1.0,
                    color: config.color,
                    track_color: dim_color(config.color, 0.25),
                    label: Some("Pan".into()),
                };
                if rotary_knob(ui, id.with("pan"), &mut state.pan, &pan_knob_config) {
                    changed = true;
                }

                ui.add_space(6.0);

                // ── Fader + VU side by side ────────────────────
                ui.horizontal(|ui| {
                    let fader_cfg = FaderConfig {
                        width: 24.0,
                        height: 100.0,
                        min: 0.0,
                        max: 1.0,
                        track_color: theme.surfaces.extreme.to_color32(),
                        handle_color: theme.widgets.inactive_bg.to_color32(),
                        label: None,
                        label_color: theme.text.muted.to_color32(),
                    };
                    if vertical_fader(ui, id.with("vol"), &mut state.volume, &fader_cfg) {
                        changed = true;
                    }

                    let vu_left = VuMeterValue {
                        level: state.level_l,
                        peak: state.peak_l,
                    };
                    let vu_right = VuMeterValue {
                        level: state.level_r,
                        peak: state.peak_r,
                    };
                    let vu_cfg = VuMeterConfig {
                        width: 6.0,
                        height: 100.0,
                        stereo: true,
                        gap: 2.0,
                    };
                    vu_meter(ui, &vu_left, Some(&vu_right), &vu_cfg);
                });

                ui.add_space(4.0);

                // ── Mute / Solo buttons ────────────────────────
                ui.horizontal(|ui| {
                    let mute_color = if state.mute {
                        theme.semantic.error.to_color32()
                    } else {
                        theme.widgets.inactive_bg.to_color32()
                    };
                    let solo_color = if state.solo {
                        theme.semantic.warning.to_color32()
                    } else {
                        theme.widgets.inactive_bg.to_color32()
                    };

                    let btn_size = Vec2::new(22.0, 16.0);

                    // Mute button
                    let (m_rect, m_resp) = ui.allocate_exact_size(btn_size, Sense::click());
                    if m_resp.clicked() {
                        state.mute = !state.mute;
                        changed = true;
                    }
                    let m_bg = if m_resp.hovered() {
                        dim_color(mute_color, 1.2)
                    } else {
                        mute_color
                    };
                    ui.painter().rect_filled(m_rect, 3.0, m_bg);
                    ui.painter().text(
                        m_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "M",
                        egui::FontId::proportional(9.0),
                        Color32::WHITE,
                    );

                    // Solo button
                    let (s_rect, s_resp) = ui.allocate_exact_size(btn_size, Sense::click());
                    if s_resp.clicked() {
                        state.solo = !state.solo;
                        changed = true;
                    }
                    let s_bg = if s_resp.hovered() {
                        dim_color(solo_color, 1.2)
                    } else {
                        solo_color
                    };
                    ui.painter().rect_filled(s_rect, 3.0, s_bg);
                    ui.painter().text(
                        s_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "S",
                        egui::FontId::proportional(9.0),
                        Color32::WHITE,
                    );
                });

                ui.add_space(2.0);

                // ── dB readout ─────────────────────────────────
                let db = if state.volume > 0.0 {
                    20.0_f32 * state.volume.log10()
                } else {
                    -60.0
                };
                ui.label(
                    egui::RichText::new(format!("{:.1} dB", db))
                        .size(9.0)
                        .color(theme.text.muted.to_color32()),
                );
            });
        });
    });

    changed
}
