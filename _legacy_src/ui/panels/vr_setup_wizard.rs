//! VR Setup Wizard panel
//!
//! Multi-step guided flow that walks users through verifying their VR setup.

use bevy::prelude::*;
use bevy_egui::egui::{self, Color32, RichText};
use renzora_theme::Theme;

/// Steps in the VR setup wizard
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WizardStep {
    #[default]
    Welcome,
    CheckRuntime,
    CheckHeadset,
    TestTracking,
    TestControllers,
    Complete,
}

impl WizardStep {
    const ALL: [WizardStep; 6] = [
        WizardStep::Welcome,
        WizardStep::CheckRuntime,
        WizardStep::CheckHeadset,
        WizardStep::TestTracking,
        WizardStep::TestControllers,
        WizardStep::Complete,
    ];

    #[allow(dead_code)]
    fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    fn label(self) -> &'static str {
        match self {
            WizardStep::Welcome => "Welcome",
            WizardStep::CheckRuntime => "Runtime",
            WizardStep::CheckHeadset => "Headset",
            WizardStep::TestTracking => "Tracking",
            WizardStep::TestControllers => "Controllers",
            WizardStep::Complete => "Complete",
        }
    }
}

/// Status of a wizard step check
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StepStatus {
    #[default]
    Pending,
    Checking,
    Passed,
    Failed,
}

/// VR Setup Wizard panel state
#[derive(Resource)]
pub struct VrSetupWizardState {
    pub current_step: WizardStep,
    pub step_statuses: [StepStatus; 6],
    // Live data populated by sync system
    pub runtime_detected: bool,
    pub runtime_name: String,
    pub headset_connected: bool,
    pub headset_name: String,
    pub left_tracked: bool,
    pub right_tracked: bool,
    pub session_focused: bool,
    pub refresh_rate: f32,
    pub head_position: [f32; 3],
}

impl Default for VrSetupWizardState {
    fn default() -> Self {
        Self {
            current_step: WizardStep::Welcome,
            step_statuses: [StepStatus::Pending; 6],
            runtime_detected: false,
            runtime_name: String::new(),
            headset_connected: false,
            headset_name: String::new(),
            left_tracked: false,
            right_tracked: false,
            session_focused: false,
            refresh_rate: 0.0,
            head_position: [0.0; 3],
        }
    }
}

pub fn render_vr_setup_wizard_content(
    ui: &mut egui::Ui,
    state: &mut VrSetupWizardState,
    theme: &Theme,
) {
    let muted = theme.text.muted.to_color32();
    let green = Color32::from_rgb(60, 200, 100);
    let red = Color32::from_rgb(200, 60, 60);
    let blue = Color32::from_rgb(100, 160, 255);
    let gray = Color32::from_gray(120);

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.set_width(ui.available_width());

        // ---- Step indicator ----
        ui.horizontal(|ui| {
            for (i, step) in WizardStep::ALL.iter().enumerate() {
                let color = if *step == state.current_step {
                    blue
                } else {
                    match state.step_statuses[i] {
                        StepStatus::Passed => green,
                        StepStatus::Failed => red,
                        StepStatus::Checking => Color32::from_rgb(200, 200, 60),
                        StepStatus::Pending => gray,
                    }
                };
                ui.colored_label(color, "\u{25CF}");
            }
        });
        ui.add_space(4.0);
        ui.label(RichText::new(state.current_step.label()).size(15.0).strong());
        ui.separator();
        ui.add_space(4.0);

        // ---- Step content ----
        match state.current_step {
            WizardStep::Welcome => {
                ui.label("This wizard will verify your VR setup step by step.");
                ui.add_space(8.0);
                ui.label("Make sure your headset is connected and your OpenXR runtime is installed.");
                ui.add_space(16.0);
                if ui.button("Next \u{2192}").clicked() {
                    state.current_step = WizardStep::CheckRuntime;
                    state.step_statuses[0] = StepStatus::Passed;
                }
            }

            WizardStep::CheckRuntime => {
                ui.label("Checking for OpenXR runtime...");
                ui.add_space(8.0);

                if state.runtime_detected {
                    state.step_statuses[1] = StepStatus::Passed;
                    ui.colored_label(green, format!("\u{2714} Runtime detected: {}", state.runtime_name));
                    ui.add_space(12.0);
                    if ui.button("Next \u{2192}").clicked() {
                        state.current_step = WizardStep::CheckHeadset;
                    }
                } else {
                    state.step_statuses[1] = StepStatus::Checking;
                    ui.colored_label(Color32::from_rgb(200, 200, 60), "\u{23F3} Waiting for runtime...");
                    ui.add_space(4.0);
                    ui.label(RichText::new("Ensure SteamVR or Oculus is running.").color(muted));
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.button("\u{2190} Back").clicked() {
                            state.current_step = WizardStep::Welcome;
                        }
                        if ui.button("Skip \u{2192}").clicked() {
                            state.step_statuses[1] = StepStatus::Failed;
                            state.current_step = WizardStep::CheckHeadset;
                        }
                    });
                }
            }

            WizardStep::CheckHeadset => {
                ui.label("Checking headset connection...");
                ui.add_space(8.0);

                if state.headset_connected {
                    state.step_statuses[2] = StepStatus::Passed;
                    let name = if state.headset_name.is_empty() { "Headset" } else { &state.headset_name };
                    ui.colored_label(green, format!("\u{2714} {} connected", name));
                    if state.refresh_rate > 0.0 {
                        ui.label(format!("Refresh rate: {:.0} Hz", state.refresh_rate));
                    }
                    ui.add_space(12.0);
                    if ui.button("Next \u{2192}").clicked() {
                        state.current_step = WizardStep::TestTracking;
                    }
                } else {
                    state.step_statuses[2] = StepStatus::Checking;
                    ui.colored_label(Color32::from_rgb(200, 200, 60), "\u{23F3} Waiting for headset...");
                    ui.add_space(4.0);
                    ui.label(RichText::new("Put on your headset or check USB/wireless connection.").color(muted));
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.button("\u{2190} Back").clicked() {
                            state.current_step = WizardStep::CheckRuntime;
                        }
                        if ui.button("Skip \u{2192}").clicked() {
                            state.step_statuses[2] = StepStatus::Failed;
                            state.current_step = WizardStep::TestTracking;
                        }
                    });
                }
            }

            WizardStep::TestTracking => {
                ui.label("Move your head to verify tracking is working.");
                ui.add_space(8.0);

                if state.session_focused {
                    ui.label(format!(
                        "Head position: ({:.2}, {:.2}, {:.2})",
                        state.head_position[0], state.head_position[1], state.head_position[2]
                    ));
                    ui.add_space(12.0);
                    if ui.button("Tracking OK \u{2714}").clicked() {
                        state.step_statuses[3] = StepStatus::Passed;
                        state.current_step = WizardStep::TestControllers;
                    }
                } else {
                    state.step_statuses[3] = StepStatus::Checking;
                    ui.colored_label(Color32::from_rgb(200, 200, 60), "\u{23F3} Put on headset to start tracking test...");
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("\u{2190} Back").clicked() {
                        state.current_step = WizardStep::CheckHeadset;
                    }
                    if ui.button("Skip \u{2192}").clicked() {
                        state.step_statuses[3] = StepStatus::Failed;
                        state.current_step = WizardStep::TestControllers;
                    }
                });
            }

            WizardStep::TestControllers => {
                ui.label("Pick up your controllers to verify they are tracked.");
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    let left_color = if state.left_tracked { green } else { red };
                    let left_icon = if state.left_tracked { "\u{2714}" } else { "\u{2718}" };
                    ui.colored_label(left_color, left_icon);
                    ui.label("Left controller");
                });

                ui.horizontal(|ui| {
                    let right_color = if state.right_tracked { green } else { red };
                    let right_icon = if state.right_tracked { "\u{2714}" } else { "\u{2718}" };
                    ui.colored_label(right_color, right_icon);
                    ui.label("Right controller");
                });

                let both_tracked = state.left_tracked && state.right_tracked;
                if both_tracked {
                    state.step_statuses[4] = StepStatus::Passed;
                } else {
                    state.step_statuses[4] = StepStatus::Checking;
                }

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button("\u{2190} Back").clicked() {
                        state.current_step = WizardStep::TestTracking;
                    }
                    let finish_label = if both_tracked { "Finish \u{2714}" } else { "Skip \u{2192}" };
                    if ui.button(finish_label).clicked() {
                        if !both_tracked {
                            state.step_statuses[4] = StepStatus::Failed;
                        }
                        state.current_step = WizardStep::Complete;
                    }
                });
            }

            WizardStep::Complete => {
                state.step_statuses[5] = StepStatus::Passed;
                ui.label(RichText::new("Setup verification complete!").size(14.0).strong());
                ui.add_space(8.0);

                // Summary
                for (i, step) in WizardStep::ALL.iter().enumerate() {
                    if i == 0 || i == 5 { continue; } // skip Welcome and Complete
                    let (icon, color) = match state.step_statuses[i] {
                        StepStatus::Passed => ("\u{2714}", green),
                        StepStatus::Failed => ("\u{2718}", red),
                        _ => ("\u{2014}", gray),
                    };
                    ui.horizontal(|ui| {
                        ui.colored_label(color, icon);
                        ui.label(step.label());
                    });
                }

                ui.add_space(16.0);
                if ui.button("Restart Wizard").clicked() {
                    state.current_step = WizardStep::Welcome;
                    state.step_statuses = [StepStatus::Pending; 6];
                }
            }
        }
    });
}
