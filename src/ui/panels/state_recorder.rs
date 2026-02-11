//! State Recorder panel — record/replay physics state

use bevy_egui::egui::{self, RichText, Color32};

use crate::core::resources::state_recorder::{
    RecorderCommand, RecorderMode, StateRecorderState,
};
use crate::theming::Theme;

/// Render the state recorder panel content
pub fn render_state_recorder_content(
    ui: &mut egui::Ui,
    state: &mut StateRecorderState,
    theme: &Theme,
) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width());

                ui.label(
                    RichText::new("State Recorder")
                        .size(13.0)
                        .color(theme.text.primary.to_color32())
                        .strong(),
                );

                ui.add_space(8.0);

                // Record / Stop button
                render_record_controls(ui, state, theme);

                ui.add_space(12.0);

                // Replay controls
                if state.mode == RecorderMode::Replaying {
                    render_replay_controls(ui, state, theme);
                    ui.add_space(12.0);
                }

                // Recording list
                render_recording_list(ui, state, theme);

                ui.add_space(8.0);

                // Ghost overlay toggle
                ui.checkbox(&mut state.show_ghost, "Show Ghost Overlay");
            });
        });
}

fn render_record_controls(
    ui: &mut egui::Ui,
    state: &mut StateRecorderState,
    theme: &Theme,
) {
    ui.horizontal(|ui| {
        match state.mode {
            RecorderMode::Recording => {
                // Recording indicator
                ui.label(
                    RichText::new("\u{f111}") // filled circle
                        .size(12.0)
                        .color(Color32::from_rgb(220, 50, 50)),
                );
                ui.label(
                    RichText::new("Recording...")
                        .size(11.0)
                        .color(Color32::from_rgb(220, 50, 50)),
                );

                if let Some(ref rec) = state.active_recording {
                    ui.label(
                        RichText::new(format!("{} frames", rec.frames.len()))
                            .size(10.0)
                            .color(theme.text.muted.to_color32())
                            .monospace(),
                    );
                }

                let stop_btn = egui::Button::new(
                    RichText::new("Stop").size(11.0),
                )
                .fill(theme.semantic.error.to_color32());
                if ui.add(stop_btn).clicked() {
                    state.commands.push(RecorderCommand::StopRecording);
                }
            }
            RecorderMode::Replaying => {
                ui.label(
                    RichText::new("Replaying...")
                        .size(11.0)
                        .color(Color32::from_rgb(100, 150, 220)),
                );

                let stop_btn = egui::Button::new(
                    RichText::new("Stop Replay").size(11.0),
                );
                if ui.add(stop_btn).clicked() {
                    state.commands.push(RecorderCommand::StopReplay);
                }
            }
            RecorderMode::Idle => {
                let record_btn = egui::Button::new(
                    RichText::new("\u{f111} Record").size(12.0),
                )
                .fill(Color32::from_rgb(180, 40, 40));
                if ui.add(record_btn).clicked() {
                    state.commands.push(RecorderCommand::StartRecording);
                }
            }
        }
    });
}

fn render_replay_controls(
    ui: &mut egui::Ui,
    state: &mut StateRecorderState,
    theme: &Theme,
) {
    ui.label(
        RichText::new("Replay Controls")
            .size(12.0)
            .color(theme.text.muted.to_color32()),
    );
    ui.add_space(4.0);

    // Frame counter
    if let Some(idx) = state.replaying_index {
        if let Some(recording) = state.recordings.get(idx) {
            let total_frames = recording.frames.len();
            ui.label(
                RichText::new(format!("Frame {}/{}", state.replay_frame, total_frames))
                    .size(10.0)
                    .color(theme.text.primary.to_color32())
                    .monospace(),
            );

            // Frame scrubber
            let mut frame = state.replay_frame as f32;
            if ui.add(egui::Slider::new(&mut frame, 0.0..=(total_frames as f32 - 1.0).max(0.0)).text("frame")).changed() {
                state.replay_frame = frame as usize;
            }
        }
    }

    // Speed slider
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Speed")
                .size(10.0)
                .color(theme.text.secondary.to_color32()),
        );
        ui.add(egui::Slider::new(&mut state.replay_speed, 0.1..=5.0).step_by(0.1));
    });
}

fn render_recording_list(
    ui: &mut egui::Ui,
    state: &mut StateRecorderState,
    theme: &Theme,
) {
    ui.label(
        RichText::new("Recordings")
            .size(12.0)
            .color(theme.text.muted.to_color32()),
    );
    ui.add_space(4.0);

    if state.recordings.is_empty() {
        ui.label(
            RichText::new("No recordings yet")
                .size(9.0)
                .color(theme.text.disabled.to_color32()),
        );
        return;
    }

    let mut replay_cmd = None;
    let mut delete_cmd = None;

    for (i, recording) in state.recordings.iter().enumerate() {
        ui.horizontal(|ui| {
            // Name
            ui.label(
                RichText::new(&recording.name)
                    .size(10.0)
                    .color(theme.text.primary.to_color32()),
            );

            // Info
            ui.label(
                RichText::new(format!("{} frames ({:.1}s)", recording.frames.len(), recording.duration_secs()))
                    .size(9.0)
                    .color(theme.text.muted.to_color32())
                    .monospace(),
            );

            // Play button
            if state.mode == RecorderMode::Idle {
                if ui.small_button("\u{f04b}").clicked() { // play icon
                    replay_cmd = Some(i);
                }
            }

            // Delete button
            if state.mode == RecorderMode::Idle {
                if ui.small_button("\u{f1f8}").clicked() { // trash icon
                    delete_cmd = Some(i);
                }
            }
        });
    }

    if let Some(idx) = replay_cmd {
        state.commands.push(RecorderCommand::StartReplay(idx));
    }
    if let Some(idx) = delete_cmd {
        state.commands.push(RecorderCommand::DeleteRecording(idx));
    }
}
