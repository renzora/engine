//! The Debug screen: the probes the connected game or editor offers, as cards.
//!
//! Probes (`renzora::probes`) are the editor's old debug panels without their
//! UI: the game builds a report from its `World`, and this screen draws it.
//! Every probe is drawn by the one renderer here, from the report's small
//! vocabulary of figures, sparklines, bars, tables, notes and controls, so a
//! plugin's probe needs no companion code.
//!
//! Only probes shown here are watched, and the game builds reports only for
//! watched probes, so an unopened probe costs the game nothing.
//!
//! The cards sit in columns as many as the screen allows, each card in the
//! shortest column so far, scrolling down. The
//! probes ticked are kept in `~/.renzora/companion_window.json`: the window is
//! a process that starts afresh each time it opens, so without the file it
//! forgot them every time it was closed.

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

use eframe::egui::{self, Color32, RichText};
use serde::{Deserialize, Serialize};

use renzora_companion_proto::probes::{ProbeItem, ProbeLevel, ProbeReport};

use super::{send, ACCENT, BAD, FAINT, GOOD, MUTED, PANEL, WARN};
use crate::capture::Capture;
use crate::log;
use crate::view::Command;

/// Rows of a table drawn on a card; a longer table is the probe's to trim.
const TABLE_ROWS: usize = 40;
/// Bars in a sparkline.
const SPARK_BARS: usize = 90;
/// The narrowest a card is drawn; the grid fits as many columns of at least
/// this as the screen has room for, and shares out what is left.
const CARD_MIN_W: f32 = 360.0;
/// Space between cards, across and down.
const CARD_GAP: f32 = 10.0;

pub(super) struct Debug {
    /// The probes ticked. Performance by default, so the screen is never
    /// empty the first time it is opened.
    watching: BTreeSet<String>,
    /// What the companion was last told, and whether a game was connected
    /// then: a game that just connected knows nothing of it.
    sent: Option<(Vec<String>, bool)>,
    /// Each card's height when last drawn, which decides its column next
    /// time (see [`place`]).
    heights: HashMap<String, f32>,
}

impl Default for Debug {
    fn default() -> Self {
        let watching = Settings::load()
            .map(|s| s.debug_probes.into_iter().collect())
            .unwrap_or_else(|| BTreeSet::from(["performance".to_string()]));
        Self { watching, sent: None, heights: HashMap::new() }
    }
}

/// What the window remembers between openings.
#[derive(Serialize, Deserialize, Default)]
struct Settings {
    /// The probes ticked on the Debug screen.
    #[serde(default)]
    debug_probes: Vec<String>,
}

impl Settings {
    fn path() -> Option<PathBuf> {
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(|home| PathBuf::from(home).join(".renzora").join("companion_window.json"))
    }

    /// The saved settings, or `None` when there are none yet (or they cannot
    /// be read, in which case the defaults stand and the next save replaces
    /// them).
    fn load() -> Option<Self> {
        let text = std::fs::read_to_string(Self::path()?).ok()?;
        serde_json::from_str(&text).ok()
    }

    fn save(&self) {
        let Some(path) = Self::path() else { return };
        let written = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| serde_json::to_string_pretty(self).map_err(std::io::Error::other))
            .and_then(|text| std::fs::write(&path, text));
        if let Err(e) = written {
            log::warn(format!("could not save the window's settings to {}: {e}", path.display()));
        }
    }
}

impl Debug {
    /// Tell the companion which probes to watch: the ticked ones while this
    /// screen shows, none otherwise (nothing would draw their reports).
    pub(super) fn sync_watch(&mut self, showing: bool, connected: bool) {
        let wanted: Vec<String> = if showing { self.watching.iter().cloned().collect() } else { Vec::new() };
        let now = (wanted, connected);
        if self.sent.as_ref() == Some(&now) {
            return;
        }
        if connected {
            send(Command::Watch { probes: now.0.clone() });
        }
        self.sent = Some(now);
    }

    pub(super) fn ui(&mut self, ui: &mut egui::Ui, capture: &Capture) {
        egui::Panel::left("probe-list").resizable(false).exact_size(230.0).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.label(RichText::new("Probes").strong());
                if capture.probes.is_empty() {
                    ui.weak("None yet: connect on the Games screen.");
                    return;
                }
                let mut changed = false;
                let mut category = "";
                for probe in sorted_probes(capture) {
                    if probe.category != category {
                        category = &probe.category;
                        ui.add_space(6.0);
                        ui.label(RichText::new(category.to_uppercase()).small().color(MUTED));
                    }
                    let mut on = self.watching.contains(&probe.id);
                    if ui.checkbox(&mut on, &probe.title).changed() {
                        if on {
                            self.watching.insert(probe.id.clone());
                        } else {
                            self.watching.remove(&probe.id);
                        }
                        changed = true;
                    }
                }
                if changed {
                    Settings { debug_probes: self.watching.iter().cloned().collect() }.save();
                }
            });
        });

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if self.watching.is_empty() {
                ui.weak("Pick probes on the left. Connect to the editor or a game on the Games screen first.");
                return;
            }
            // In the list's order, so a card keeps its place in the grid;
            // probes this game does not offer (ticked for another) last.
            let mut shown: Vec<(&str, &str)> = sorted_probes(capture)
                .into_iter()
                .filter(|p| self.watching.contains(&p.id))
                .map(|p| (p.id.as_str(), p.title.as_str()))
                .collect();
            for id in &self.watching {
                if !shown.iter().any(|(shown_id, _)| shown_id == id) {
                    shown.push((id.as_str(), id.as_str()));
                }
            }
            let (columns, width) = grid(ui.available_width());
            let placed = place(&shown, columns, &self.heights);
            let heights = &mut self.heights;
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = CARD_GAP;
                for column in placed {
                    ui.allocate_ui_with_layout(egui::vec2(width, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                        ui.spacing_mut().item_spacing.y = CARD_GAP;
                        for (id, title) in column {
                            let height = card(ui, width, id, title, capture);
                            heights.insert(id.to_string(), height);
                        }
                    });
                }
            });
        });
    }
}

/// A card's height before it has been drawn once: about a short report's.
const UNMEASURED_H: f32 = 120.0;

/// Share `cards` out over `columns`, each into the one that is shortest so
/// far, by the heights the cards had when last drawn. Laid out in rows
/// instead, each row was as tall as its tallest card and a short card left a
/// gap the height of its neighbour under it.
fn place<'a>(cards: &[(&'a str, &'a str)], columns: usize, heights: &HashMap<String, f32>) -> Vec<Vec<(&'a str, &'a str)>> {
    let mut placed: Vec<Vec<(&str, &str)>> = vec![Vec::new(); columns.max(1)];
    let mut filled = vec![0.0f32; placed.len()];
    for &(id, title) in cards {
        // The first of the shortest, so equal columns fill left to right.
        let shortest = (0..filled.len()).fold(0, |best, i| if filled[i] < filled[best] { i } else { best });
        placed[shortest].push((id, title));
        filled[shortest] += heights.get(id).copied().unwrap_or(UNMEASURED_H) + CARD_GAP;
    }
    placed
}

/// The probes the game offers, by category and then title.
fn sorted_probes(capture: &Capture) -> Vec<&renzora_companion_proto::wire::ProbeInfo> {
    let mut probes: Vec<_> = capture.probes.iter().collect();
    probes.sort_by(|a, b| (a.category.as_str(), a.title.as_str()).cmp(&(b.category.as_str(), b.title.as_str())));
    probes
}

/// How many columns of cards fit `available` points across, and how wide
/// each is: as many as are at least [`CARD_MIN_W`], sharing out the rest.
fn grid(available: f32) -> (usize, f32) {
    let columns = (((available + CARD_GAP) / (CARD_MIN_W + CARD_GAP)).floor() as usize).max(1);
    let width = ((available - CARD_GAP * (columns - 1) as f32) / columns as f32).max(1.0);
    (columns, width)
}

/// One probe's card, `width` points wide outside its border. Returns how tall
/// it came out.
fn card(ui: &mut egui::Ui, width: f32, id: &str, title: &str, capture: &Capture) -> f32 {
    ui.allocate_ui_with_layout(egui::vec2(width, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
        egui::Frame::group(ui.style()).fill(PANEL).inner_margin(10.0).show(ui, |ui| {
            // A card reads top to bottom, filling its column.
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).strong().size(15.0));
            match capture.reports.get(id) {
                Some((report, _, _)) => draw_report(ui, id, report),
                None => {
                    ui.weak("Waiting for the first report.");
                }
            }
        });
    })
    .response
    .rect
    .height()
}

fn level_color(level: ProbeLevel) -> Color32 {
    match level {
        ProbeLevel::Normal => Color32::from_gray(230),
        ProbeLevel::Good => GOOD,
        ProbeLevel::Warn => WARN,
        ProbeLevel::Bad => BAD,
        ProbeLevel::Muted => MUTED,
    }
}

fn draw_report(ui: &mut egui::Ui, probe: &str, report: &ProbeReport) {
    for section in &report.sections {
        if !section.title.is_empty() {
            ui.add_space(6.0);
            ui.label(RichText::new(section.title.to_uppercase()).small().color(MUTED));
        }
        for item in &section.items {
            draw_item(ui, probe, item);
        }
    }
}

/// A label on the left and something on the right of one line.
fn row(ui: &mut egui::Ui, label: &str, right: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(MUTED));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), right);
    });
}

fn draw_item(ui: &mut egui::Ui, probe: &str, item: &ProbeItem) {
    match item {
        ProbeItem::Figure { label, value, unit, level } => {
            ui.label(RichText::new(label).small().color(MUTED));
            ui.horizontal(|ui| {
                ui.label(RichText::new(value).size(24.0).color(level_color(*level)));
                ui.label(RichText::new(unit).color(MUTED));
            });
        }
        ProbeItem::Stat { label, value, level } => {
            row(ui, label, |ui| {
                ui.monospace(RichText::new(value).color(level_color(*level)));
            });
        }
        ProbeItem::Series { label, values, unit } => sparkline(ui, label, values, unit),
        ProbeItem::Bar { label, fraction, value, level } => {
            row(ui, label, |ui| {
                ui.monospace(RichText::new(value).color(MUTED));
            });
            let fill = if *level == ProbeLevel::Normal { ACCENT } else { level_color(*level) };
            let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 5.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 3.0, FAINT);
            let mut filled = rect;
            filled.set_width(rect.width() * fraction.clamp(0.0, 1.0));
            ui.painter().rect_filled(filled, 3.0, fill);
        }
        ProbeItem::Table { columns, rows } => {
            egui::Grid::new(("probe-table", probe, columns.join("|"))).striped(true).show(ui, |ui| {
                for column in columns {
                    ui.label(RichText::new(column).small().color(MUTED));
                }
                ui.end_row();
                for cells in rows.iter().take(TABLE_ROWS) {
                    for cell in cells {
                        ui.label(RichText::new(cell).small().monospace());
                    }
                    ui.end_row();
                }
            });
            if rows.len() > TABLE_ROWS {
                ui.weak(format!("{} more rows", rows.len() - TABLE_ROWS));
            }
        }
        ProbeItem::Note { text, level } => {
            ui.label(RichText::new(text).color(level_color(*level)));
        }
        ProbeItem::Toggle { id, label, on } => {
            row(ui, label, |ui| {
                if ui.button(if *on { "On" } else { "Off" }).clicked() {
                    send(Command::ProbeAct { probe: probe.to_string(), item: id.clone(), value: Some(!on) });
                }
            });
        }
        ProbeItem::Button { id, label } => {
            if ui.button(label).clicked() {
                send(Command::ProbeAct { probe: probe.to_string(), item: id.clone(), value: None });
            }
        }
    }
}

/// A history as thin bars, newest on the right, scaled to its own peak; the
/// latest value and the peak read out beside it.
fn sparkline(ui: &mut egui::Ui, label: &str, values: &[f32], unit: &str) {
    let latest = values.last().copied().unwrap_or(0.0);
    let peak = values.iter().copied().fold(0.0f32, f32::max);
    row(ui, label, |ui| {
        ui.monospace(RichText::new(format!("{latest:.2} {unit}  (peak {peak:.2})")).small().color(MUTED));
    });
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 40.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, FAINT);
    let shown = &values[values.len().saturating_sub(SPARK_BARS)..];
    if shown.is_empty() || peak <= 0.0 {
        return;
    }
    let step = rect.width() / shown.len() as f32;
    for (i, value) in shown.iter().enumerate() {
        let h = (value / peak * rect.height()).max(1.0);
        let x = rect.left() + i as f32 * step;
        painter.rect_filled(
            egui::Rect::from_min_max(egui::pos2(x, rect.bottom() - h), egui::pos2(x + (step - 1.0).max(1.0), rect.bottom())),
            0.0,
            ACCENT,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grid_fits_as_many_columns_as_there_is_room_for() {
        assert_eq!(grid(300.0).0, 1, "narrower than one card still shows one");
        assert_eq!(grid(CARD_MIN_W * 2.0 + CARD_GAP).0, 2);
        assert_eq!(grid(CARD_MIN_W * 2.0 + CARD_GAP - 1.0).0, 1);
        let (columns, width) = grid(1200.0);
        assert_eq!(columns, 3);
        assert!((width * 3.0 + CARD_GAP * 2.0 - 1200.0).abs() < 0.01, "the columns fill the width");
    }

    /// A card goes under the shortest column, so a short card's neighbour
    /// does not leave a gap under it; unmeasured cards fill left to right.
    #[test]
    fn cards_go_into_the_shortest_column() {
        let cards = [("a", "A"), ("b", "B"), ("c", "C"), ("d", "D")];
        let heights = HashMap::from([("a".to_string(), 50.0), ("b".to_string(), 900.0), ("c".to_string(), 300.0)]);
        fn ids<'a>(column: &[(&'a str, &'a str)]) -> Vec<&'a str> {
            column.iter().map(|(id, _)| *id).collect()
        }
        let placed = place(&cards, 2, &heights);
        assert_eq!(ids(&placed[0]), ["a", "c", "d"]);
        assert_eq!(ids(&placed[1]), ["b"]);
        let fresh = place(&cards, 2, &HashMap::new());
        assert_eq!(ids(&fresh[0]), ["a", "c"]);
        assert_eq!(ids(&fresh[1]), ["b", "d"]);
    }

    /// Settings written by one opening read back in the next, and a file
    /// from before a setting existed still reads.
    #[test]
    fn settings_read_back_and_tolerate_missing_fields() {
        let saved = Settings { debug_probes: vec!["memory".into(), "performance".into()] };
        let text = serde_json::to_string(&saved).unwrap();
        let read: Settings = serde_json::from_str(&text).unwrap();
        assert_eq!(read.debug_probes, ["memory", "performance"]);
        let older: Settings = serde_json::from_str("{}").unwrap();
        assert!(older.debug_probes.is_empty());
    }
}
