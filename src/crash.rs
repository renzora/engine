//! Crash reporting and error handling
//!
//! Provides a panic hook that captures crash information and displays it
//! in a user-friendly window with a copy button.

use std::panic::{self, PanicHookInfo};
use std::sync::{Arc, Mutex};
use std::backtrace::Backtrace;

/// Global crash report storage
static CRASH_REPORT: Mutex<Option<CrashReport>> = Mutex::new(None);

/// Crash report information
#[derive(Clone, Debug)]
pub struct CrashReport {
    /// The panic message
    pub message: String,
    /// The location where the panic occurred
    pub location: String,
    /// The backtrace at the time of panic
    pub backtrace: String,
    /// Timestamp of the crash
    pub timestamp: String,
}

impl CrashReport {
    /// Format the crash report for display/copying
    pub fn format(&self) -> String {
        format!(
            "=== CRASH REPORT ===\n\
             Timestamp: {}\n\n\
             Error: {}\n\
             Location: {}\n\n\
             === BACKTRACE ===\n\
             {}\n\
             === END CRASH REPORT ===",
            self.timestamp,
            self.message,
            self.location,
            self.backtrace
        )
    }
}

/// Install the custom panic hook
pub fn install_panic_hook() {
    let default_hook = panic::take_hook();

    panic::set_hook(Box::new(move |panic_info: &PanicHookInfo<'_>| {
        // Capture crash information
        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic".to_string()
        };

        let location = if let Some(loc) = panic_info.location() {
            format!("{}:{}:{}", loc.file(), loc.line(), loc.column())
        } else {
            "Unknown location".to_string()
        };

        let backtrace = Backtrace::force_capture();
        let backtrace_str = format!("{}", backtrace);

        let timestamp = chrono_lite_timestamp();

        let report = CrashReport {
            message,
            location,
            backtrace: backtrace_str,
            timestamp,
        };

        // Store the crash report
        if let Ok(mut guard) = CRASH_REPORT.lock() {
            *guard = Some(report.clone());
        }

        // Save to file for persistence
        let _ = save_crash_report(&report);

        // Show the crash window
        show_crash_window(&report);

        // Call the default hook (this will print to stderr)
        default_hook(panic_info);
    }));
}

/// Simple timestamp without external dependency
fn chrono_lite_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    let secs = duration.as_secs();

    // Simple UTC time calculation
    let days = secs / 86400;
    let remaining = secs % 86400;
    let hours = remaining / 3600;
    let minutes = (remaining % 3600) / 60;
    let seconds = remaining % 60;

    // Approximate date (good enough for crash reports)
    let years_since_1970 = days / 365;
    let year = 1970 + years_since_1970;
    let day_of_year = days % 365;
    let month = (day_of_year / 30) + 1;
    let day = (day_of_year % 30) + 1;

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
        year, month.min(12), day.min(31), hours, minutes, seconds
    )
}

/// Save the crash report to a file
fn save_crash_report(report: &CrashReport) -> std::io::Result<()> {
    let crash_dir = get_crash_dir();
    std::fs::create_dir_all(&crash_dir)?;

    let crash_file = crash_dir.join("last_crash.txt");
    std::fs::write(&crash_file, report.format())?;

    Ok(())
}

/// Get the crash report directory
fn get_crash_dir() -> std::path::PathBuf {
    if let Some(home) = dirs::home_dir() {
        home.join(".renzora").join("crashes")
    } else {
        std::path::PathBuf::from(".renzora/crashes")
    }
}

/// Check if there's a crash report from a previous session
pub fn check_previous_crash() -> Option<CrashReport> {
    let crash_file = get_crash_dir().join("last_crash.txt");

    if crash_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&crash_file) {
            // Parse the crash report
            let mut message = String::new();
            let mut location = String::new();
            let mut backtrace = String::new();
            let mut timestamp = String::new();
            let mut in_backtrace = false;

            for line in content.lines() {
                if line.starts_with("Timestamp: ") {
                    timestamp = line.strip_prefix("Timestamp: ").unwrap_or("").to_string();
                } else if line.starts_with("Error: ") {
                    message = line.strip_prefix("Error: ").unwrap_or("").to_string();
                } else if line.starts_with("Location: ") {
                    location = line.strip_prefix("Location: ").unwrap_or("").to_string();
                } else if line == "=== BACKTRACE ===" {
                    in_backtrace = true;
                } else if line == "=== END CRASH REPORT ===" {
                    in_backtrace = false;
                } else if in_backtrace {
                    backtrace.push_str(line);
                    backtrace.push('\n');
                }
            }

            // Delete the crash file after reading
            let _ = std::fs::remove_file(&crash_file);

            if !message.is_empty() {
                return Some(CrashReport {
                    message,
                    location,
                    backtrace,
                    timestamp,
                });
            }
        }
    }

    None
}

/// Show a native crash window using rfd (same library used for file dialogs)
fn show_crash_window(report: &CrashReport) {
    use rfd::MessageDialog;
    use rfd::MessageLevel;
    use rfd::MessageButtons;
    use rfd::MessageDialogResult;

    let short_message = format!(
        "The application has crashed.\n\n\
         Error: {}\n\
         Location: {}\n\n\
         A crash report has been saved to:\n\
         {}\n\n\
         Would you like to copy the full crash report to clipboard?",
        report.message,
        report.location,
        get_crash_dir().join("last_crash.txt").display()
    );

    let result = MessageDialog::new()
        .set_title("Renzora Engine - Crash Report")
        .set_description(&short_message)
        .set_level(MessageLevel::Error)
        .set_buttons(MessageButtons::YesNo)
        .show();

    if result == MessageDialogResult::Yes {
        // Copy to clipboard
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(report.format());
        }
    }
}

/// Resource to track if we should show the previous crash report
#[derive(bevy::prelude::Resource, Default)]
pub struct PreviousCrashReport(pub Option<CrashReport>);

/// State for the crash report window UI
#[derive(bevy::prelude::Resource, Default)]
pub struct CrashReportWindowState {
    pub show_window: bool,
    pub report: Option<CrashReport>,
    pub copied: bool,
}

/// System to render the crash report window (for previous session crashes)
pub fn render_crash_report_window(
    mut contexts: bevy_egui::EguiContexts,
    mut state: bevy::prelude::ResMut<CrashReportWindowState>,
) {
    use bevy_egui::egui;

    if !state.show_window {
        return;
    }

    let Some(report) = state.report.clone() else {
        state.show_window = false;
        return;
    };

    let Ok(ctx) = contexts.ctx_mut() else { return };

    let mut open = state.show_window;
    let mut copy_clicked = false;
    let mut close_clicked = false;
    let was_copied = state.copied;

    egui::Window::new("Previous Session Crash Report")
        .open(&mut open)
        .default_size([600.0, 400.0])
        .resizable(true)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.heading("The application crashed in the previous session");
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.label("Timestamp:");
                    ui.monospace(&report.timestamp);
                });

                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label("Error:");
                    ui.colored_label(egui::Color32::from_rgb(255, 100, 100), &report.message);
                });

                ui.horizontal(|ui| {
                    ui.label("Location:");
                    ui.monospace(&report.location);
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                ui.label("Backtrace:");
                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut report.backtrace.as_str())
                                .font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY)
                                .interactive(false)
                        );
                    });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if ui.button("Copy to Clipboard").clicked() {
                        copy_clicked = true;
                    }

                    if was_copied {
                        ui.colored_label(egui::Color32::from_rgb(100, 200, 100), "Copied!");
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            close_clicked = true;
                        }
                    });
                });
            });
        });

    // Apply state changes after the UI is drawn
    if copy_clicked {
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(report.format());
            state.copied = true;
        }
    }

    // Close if either the X button was clicked (open becomes false) or the Close button was clicked
    state.show_window = open && !close_clicked;
}

/// System to check for previous crash on startup
pub fn check_for_previous_crash(
    mut state: bevy::prelude::ResMut<CrashReportWindowState>,
) {
    if let Some(report) = check_previous_crash() {
        state.report = Some(report);
        state.show_window = true;
    }
}
