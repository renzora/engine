//! Crash reporting and error handling
//!
//! Provides a panic hook that captures crash information, saves it to disk,
//! and shows a native dialog. On next startup, the editor displays the
//! previous crash report in an egui window.

use std::backtrace::Backtrace;
use std::panic;
use std::sync::Mutex;

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
            self.timestamp, self.message, self.location, self.backtrace
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

        #[cfg(all(
            feature = "editor",
            not(any(target_arch = "wasm32", target_os = "android", target_os = "ios"))
        ))]
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
        year,
        month.min(12),
        day.min(31),
        hours,
        minutes,
        seconds
    )
}

/// Save the crash report to a file.
///
/// In the editor build the report is overwritten in `~/.renzora/crashes/last_crash.txt`
/// so the next editor launch can pick it up and show its dialog. In the runtime
/// build the report is appended to `<exe_dir>/crash.log` next to the shipped
/// binary, where players can find it without hunting through their home dir.
fn save_crash_report(report: &CrashReport) -> std::io::Result<()> {
    let crash_dir = get_crash_dir();
    std::fs::create_dir_all(&crash_dir)?;

    #[cfg(feature = "editor")]
    {
        let crash_file = crash_dir.join("last_crash.txt");
        std::fs::write(&crash_file, report.format())?;
    }

    #[cfg(not(feature = "editor"))]
    {
        use std::io::Write as _;
        let crash_file = crash_dir.join("crash.log");
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&crash_file)?;
        writeln!(f, "{}\n", report.format())?;
    }

    Ok(())
}

/// Get the crash report directory.
///
/// Editor builds keep history under the user's home dir; runtime builds drop
/// the file next to the executable so it ships with the game directory.
fn get_crash_dir() -> std::path::PathBuf {
    #[cfg(all(feature = "editor", not(target_arch = "wasm32")))]
    {
        if let Some(home) = dirs::home_dir() {
            return home.join(".renzora").join("crashes");
        }
        std::path::PathBuf::from(".renzora/crashes")
    }

    #[cfg(all(not(feature = "editor"), not(target_arch = "wasm32")))]
    {
        if let Some(exe_dir) = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        {
            return exe_dir;
        }
        std::path::PathBuf::from(".")
    }

    #[cfg(target_arch = "wasm32")]
    {
        std::path::PathBuf::from(".renzora/crashes")
    }
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

/// Show a native crash dialog using rfd, with option to copy to clipboard.
/// Editor-only — shipped runtime builds write `crash.log` silently instead.
#[cfg(all(
    feature = "editor",
    not(any(target_arch = "wasm32", target_os = "android", target_os = "ios"))
))]
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
// Editor-only: native (ember / bevy_ui) window for previous-session crash reports
// =============================================================================

/// State for the crash report window UI
#[derive(Resource, Default)]
pub struct CrashReportWindowState {
    pub show_window: bool,
    pub report: Option<CrashReport>,
}

/// Plugin that installs crash reporting.
/// - Always installs the panic hook and checks for previous crashes.
/// - With the `editor` feature, renders a native bevy_ui (ember) crash window.
pub struct CrashReportPlugin;

impl Plugin for CrashReportPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] CrashReportPlugin");
        app.init_resource::<CrashReportWindowState>()
            .add_systems(Startup, check_for_previous_crash_system);

        #[cfg(feature = "editor")]
        {
            use renzora_editor::SplashState;
            app.add_systems(
                Update,
                (
                    overlay_ui::manage_crash_overlay,
                    overlay_ui::crash_overlay_buttons,
                )
                    .run_if(in_state(SplashState::Editor)),
            );
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

/// Native bevy_ui (ember) crash report overlay — the replacement for the old
/// egui window. A dimmed backdrop + centered panel showing the previous
/// session's error / location / backtrace, with Copy-to-clipboard and Close.
#[cfg(feature = "editor")]
mod overlay_ui {
    use super::{CrashReport, CrashReportWindowState};

    use bevy::ecs::world::CommandQueue;
    use bevy::prelude::*;
    use bevy::ui::FocusPolicy;

    use renzora_ember::font::{ui_font, EmberFonts};
    use renzora_ember::theme::{border, popup_bg, rgb, text_muted, text_primary};
    use renzora_ember::widgets::{button, scroll_area, OverlaySurface};

    #[derive(Component)]
    pub(super) struct CrashOverlayRoot;
    #[derive(Component)]
    pub(super) struct CrashCloseButton;
    #[derive(Component)]
    pub(super) struct CrashCopyButton;

    /// Spawn the overlay when a previous crash is flagged; tear it down when cleared.
    pub(super) fn manage_crash_overlay(world: &mut World) {
        let show = world
            .get_resource::<CrashReportWindowState>()
            .is_some_and(|s| s.show_window && s.report.is_some());
        let mut q = world.query_filtered::<Entity, With<CrashOverlayRoot>>();
        let existing: Vec<Entity> = q.iter(world).collect();

        if show && existing.is_empty() {
            let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
                return;
            };
            let report = world
                .resource::<CrashReportWindowState>()
                .report
                .clone()
                .unwrap();
            let mut queue = CommandQueue::default();
            {
                let mut commands = Commands::new(&mut queue, world);
                spawn_overlay(&mut commands, &fonts, &report);
            }
            queue.apply(world);
        } else if !show && !existing.is_empty() {
            for e in existing {
                world.entity_mut(e).despawn();
            }
        }
    }

    fn line(
        commands: &mut Commands,
        font: &Handle<Font>,
        text: &str,
        color: (u8, u8, u8),
        size: f32,
    ) -> Entity {
        commands
            .spawn((Text::new(text), ui_font(font, size), TextColor(rgb(color))))
            .id()
    }

    fn spawn_overlay(commands: &mut Commands, fonts: &EmberFonts, report: &CrashReport) {
        let backdrop = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(0.0),
                    right: Val::Px(0.0),
                    bottom: Val::Px(0.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
                GlobalZIndex(9800),
                FocusPolicy::Block,
                Interaction::default(),
                OverlaySurface,
                CrashOverlayRoot,
                Name::new("crash-overlay"),
            ))
            .id();

        let panel = commands
            .spawn((
                Node {
                    width: Val::Px(640.0),
                    max_width: Val::Percent(94.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    padding: UiRect::all(Val::Px(14.0)),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundColor(rgb(popup_bg())),
                BorderColor::all(rgb(border())),
                FocusPolicy::Block,
                Name::new("crash-panel"),
            ))
            .id();

        let heading = line(
            commands,
            &fonts.ui,
            "The application crashed in the previous session",
            text_primary(),
            15.0,
        );
        let ts = line(
            commands,
            &fonts.ui,
            &format!("Timestamp: {}", report.timestamp),
            text_muted(),
            12.0,
        );
        let err_label = line(commands, &fonts.ui, "Error:", text_muted(), 12.0);
        let err = line(commands, &fonts.ui, &report.message, (235, 110, 110), 13.0);
        let loc_label = line(commands, &fonts.ui, "Location:", text_muted(), 12.0);
        let loc = line(commands, &fonts.ui, &report.location, text_primary(), 12.0);
        let bt_label = line(commands, &fonts.ui, "Backtrace:", text_muted(), 12.0);

        // Scrollable backtrace.
        let bt_content = commands
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(6.0)),
                ..default()
            })
            .id();
        let bt_text = commands
            .spawn((
                Text::new(report.backtrace.clone()),
                ui_font(&fonts.ui, 11.0),
                TextColor(rgb(text_muted())),
            ))
            .id();
        commands.entity(bt_content).add_child(bt_text);
        let bt_scroll = scroll_area(commands, bt_content, 240.0);

        // Button row.
        let copy_btn = button(commands, &fonts.ui, "Copy to Clipboard");
        commands.entity(copy_btn).insert(CrashCopyButton);
        let close_btn = button(commands, &fonts.ui, "Close");
        commands.entity(close_btn).insert(CrashCloseButton);
        let btn_row = commands
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: Val::Px(8.0),
                margin: UiRect::top(Val::Px(4.0)),
                ..default()
            })
            .id();
        commands.entity(btn_row).add_children(&[copy_btn, close_btn]);

        commands.entity(panel).add_children(&[
            heading, ts, err_label, err, loc_label, loc, bt_label, bt_scroll, btn_row,
        ]);
        commands.entity(backdrop).add_child(panel);
    }

    /// Handle Copy / Close clicks.
    pub(super) fn crash_overlay_buttons(
        mut state: ResMut<CrashReportWindowState>,
        close_q: Query<&Interaction, (Changed<Interaction>, With<CrashCloseButton>)>,
        copy_q: Query<&Interaction, (Changed<Interaction>, With<CrashCopyButton>)>,
    ) {
        if close_q.iter().any(|i| *i == Interaction::Pressed) {
            state.show_window = false;
        }
        if copy_q.iter().any(|i| *i == Interaction::Pressed) {
            if let Some(report) = state.report.clone() {
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    let _ = cb.set_text(report.format());
                }
            }
        }
    }
}
