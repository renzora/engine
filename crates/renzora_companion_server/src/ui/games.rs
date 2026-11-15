//! The Games screen: what can be profiled, and saved captures to open.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use eframe::egui::{self, RichText};

use super::{send, section, GOOD, MUTED, WARN};
use crate::capture::Capture;
use crate::games::ConnState;
use crate::view::{Command, ProfilingView};

/// How often the saved captures folder is looked at again while this screen
/// shows.
const RESCAN: Duration = Duration::from_secs(2);
/// Saved captures listed, newest first.
const CAPTURES_SHOWN: usize = 12;

#[derive(Default)]
pub(super) struct Games {
    captures: Vec<PathBuf>,
    scanned: Option<Instant>,
}

impl Games {
    /// Draw the screen. True when something was opened or connected, so the
    /// window moves to the Profiler.
    pub(super) fn ui(&mut self, ui: &mut egui::Ui, profiling: &ProfilingView) -> bool {
        if self.scanned.is_none_or(|t| t.elapsed() >= RESCAN) {
            self.captures = saved_captures();
            self.scanned = Some(Instant::now());
        }
        ui.ctx().request_repaint_after(RESCAN);
        let mut go = false;
        egui::Panel::right("games-help").resizable(false).exact_size(330.0).show(ui, |ui| {
            section(ui, "How games find you", |ui| {
                ui.label(RichText::new(
                    "1. Export with Allow profiling on. The build carries this editor's pairing key.\n\n\
                     2. Run it on any device on the same network. It announces itself and appears here.\n\n\
                     3. Connect. Only a companion with the matching key can, so nothing on the device needs setting up.\n\n\
                     With Allow profiling off, a game never announces itself or opens a port.",
                ).color(MUTED));
            });
        });
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            section(ui, "Games you can profile", |ui| {
                ui.label(
                    RichText::new("The editor, Play on this PC, and games exported with profiling allowed on your network.")
                        .color(MUTED),
                );
                ui.add_space(6.0);
                if profiling.games.is_empty() {
                    ui.weak("Nothing found yet. The editor appears within a second; press Play to add your game.");
                }
                for game in &profiling.games {
                    let this = profiling.game.as_ref().is_some_and(|g| g.address == game.address);
                    egui::Frame::new().fill(super::FAINT).inner_margin(8.0).corner_radius(6.0).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(RichText::new(&game.name).strong());
                                ui.monospace(
                                    RichText::new(format!("{}  ·  {}  ·  pid {}", game.place(), game.build, game.pid))
                                        .color(MUTED),
                                );
                            });
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if this {
                                    if ui.button("Disconnect").clicked() {
                                        send(Command::Disconnect);
                                    }
                                } else if ui.button("Connect").clicked() {
                                    send(Command::Connect { game: game.clone() });
                                    go = true;
                                }
                                let state = match (&profiling.state, this) {
                                    (ConnState::Connected, true) => Some(("Profiling", GOOD)),
                                    (ConnState::Connecting, true) => Some(("Connecting", MUTED)),
                                    (ConnState::Failed(_), true) => Some(("Could not connect", WARN)),
                                    _ => None,
                                };
                                if let Some((label, color)) = state {
                                    ui.colored_label(color, label);
                                }
                            });
                        });
                    });
                    ui.add_space(4.0);
                }
                if let (ConnState::Failed(why), Some(game)) = (&profiling.state, &profiling.game) {
                    ui.colored_label(WARN, format!("{}: {why}", game.name));
                }
            });

            section(ui, "Saved captures", |ui| {
                if self.captures.is_empty() {
                    ui.weak("None yet. Save one from the header while profiling.");
                }
                for path in &self.captures {
                    ui.horizontal(|ui| {
                        ui.monospace(path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Open").clicked() {
                                send(Command::OpenCapture { path: path.display().to_string() });
                                go = true;
                            }
                        });
                    });
                }
            });
        });
        go
    }
}

/// The newest saved captures in `~/.renzora/captures`.
fn saved_captures() -> Vec<PathBuf> {
    let mut found: Vec<(PathBuf, std::time::SystemTime)> = Capture::folder()
        .and_then(|dir| std::fs::read_dir(dir).ok())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "rzprof"))
        .filter_map(|e| Some((e.path(), e.metadata().ok()?.modified().ok()?)))
        .collect();
    found.sort_by_key(|(_, modified)| std::cmp::Reverse(*modified));
    found.truncate(CAPTURES_SHOWN);
    found.into_iter().map(|(p, _)| p).collect()
}
