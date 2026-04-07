//! Crash reporting and error handling
//!
//! Provides a panic hook that captures crash information, saves it to disk,
//! and shows a native dialog. On next startup, the editor displays the
//! previous crash report in an egui window.

use std::panic;
use std::sync::Mutex;
use std::backtrace::Backtrace;

use bevy::prelude::*;

/// Global crash report storage
static CRASH_REPORT: Mutex<Option<CrashReport>> = Mutex::new(None);

/// Crash report information
#[derive(Clone, Debug)]
pub struct CrashReport {
    pub message: String,
    pub location: String,
    pub backtrace: String,
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

/// Install the custom panic hook. Call this before `app.run()`.
pub fn install_panic_hook() {
    let default_hook = panic::take_hook();

    panic::set_hook(Box::new(move |panic_info| {
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
        let backtrace_str = format!("{backtrace}");

        let timestamp = chrono_lite_timestamp();

        let report = CrashReport {
            message,
            location,
            backtrace: backtrace_str,
            timestamp,
        };

        if let Ok(mut guard) = CRASH_REPORT.lock() {
            *guard = Some(report.clone());
        }

        let _ = save_crash_report(&report);

        #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
        show_crash_dialog(&report);

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

    let days = secs / 86400;
    let remaining = secs % 86400;
    let hours = remaining / 3600;
    let minutes = (remaining % 3600) / 60;
    let seconds = remaining % 60;

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
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(home) = dirs::home_dir() {
        return home.join(".renzora").join("crashes");
    }
    std::path::PathBuf::from(".renzora/crashes")
}

/// Check if there's a crash report from a previous session
pub fn check_previous_crash() -> Option<CrashReport> {
    let crash_file = get_crash_dir().join("last_crash.txt");

    if !crash_file.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&crash_file).ok()?;

    let mut message = String::new();
    let mut location = String::new();
    let mut backtrace = String::new();
    let mut timestamp = String::new();
    let mut in_backtrace = false;

    for line in content.lines() {
        if let Some(ts) = line.strip_prefix("Timestamp: ") {
            timestamp = ts.to_string();
        } else if let Some(err) = line.strip_prefix("Error: ") {
            message = err.to_string();
        } else if let Some(loc) = line.strip_prefix("Location: ") {
            location = loc.to_string();
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

    if message.is_empty() {
        return None;
    }

    Some(CrashReport {
        message,
        location,
        backtrace,
        timestamp,
    })
}

/// Show a native crash dialog using rfd, with option to copy to clipboard
#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn show_crash_dialog(report: &CrashReport) {
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

    let result = rfd::MessageDialog::new()
        .set_title("Renzora Engine - Crash Report")
        .set_description(&short_message)
        .set_level(rfd::MessageLevel::Error)
        .set_buttons(rfd::MessageButtons::YesNo)
        .show();

    if result == rfd::MessageDialogResult::Yes {
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(report.format());
        }
    }
}

// =============================================================================
// Editor-only: egui window for previous session crash reports
// =============================================================================

/// State for the crash report window UI
#[derive(Resource, Default)]
pub struct CrashReportWindowState {
    pub show_window: bool,
    pub report: Option<CrashReport>,
}

/// Plugin that installs crash reporting.
/// - Always installs the panic hook and checks for previous crashes.
/// - When the `editor` feature is enabled, also renders an egui crash report window.
pub struct CrashReportPlugin;

impl Plugin for CrashReportPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] CrashReportPlugin");
        app.init_resource::<CrashReportWindowState>()
            .add_systems(Startup, check_for_previous_crash_system);

        #[cfg(feature = "editor")]
        {
            app.add_systems(bevy_egui::EguiPrimaryContextPass, render_crash_report_window);
        }
    }
}

/// System to check for previous crash on startup
fn check_for_previous_crash_system(mut state: ResMut<CrashReportWindowState>) {
    if let Some(report) = check_previous_crash() {
        warn!("Previous session crashed: {}", report.message);
        state.report = Some(report);
        state.show_window = true;
    }
}

/// System to render the crash report window (editor only)
#[cfg(feature = "editor")]
fn render_crash_report_window(
    mut contexts: bevy_egui::EguiContexts,
    mut state: ResMut<CrashReportWindowState>,
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
    let mut close_clicked = false;

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

                ui.label("Error:");
                ui.label(
                    egui::RichText::new(&report.message)
                        .color(egui::Color32::from_rgb(255, 100, 100)),
                );

                ui.label("Location:");
                ui.monospace(&report.location);

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
                                .interactive(false),
                        );
                    });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if ui.button("Copy to Clipboard").clicked() {
                        ui.ctx().copy_text(report.format());
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            close_clicked = true;
                        }
                    });
                });
            });
        });

    state.show_window = open && !close_clicked;
}
