//! Probes: debug readouts a process offers, shown in the companion.
//!
//! The editor's debug panels lived in the editor's own window, and a panel
//! that updates every frame makes Bevy lay the editor's whole UI out again
//! every frame: with the Debug workspace open, layout alone took 30 ms a
//! frame. A probe is the same readout without the UI. A crate or plugin
//! registers one with [`RegisterProbe::register_probe`]: a name, and a
//! function that builds a [`ProbeReport`] from the `World`. The companion
//! asks for the probes it is showing; the profiler's server (in the editor
//! and in any game that allows profiling) runs those, a few times a second,
//! and sends the reports. A probe nobody is watching costs nothing.
//!
//! A report is plain data in a small vocabulary ([`ProbeItem`]): figures,
//! sparklines, bars, tables, warnings and controls. The companion draws every
//! probe with one renderer, so a plugin adds a debug view without any UI code.
//!
//! # Gathering only while watched
//!
//! A probe whose data needs a per-frame system (a rolling history, a walk over
//! every mesh) gates that system with [`probe_watched`], the way it was gated
//! on its panel being the active tab.
//!
//! # Controls
//!
//! A [`ProbeItem::Toggle`] or [`ProbeItem::Button`] in a report comes back,
//! when used, as a call to the probe's `act` function with the item's `id`.

use std::collections::HashSet;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// How a value reads: the companion colours it.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub enum ProbeLevel {
    #[default]
    Normal,
    Good,
    Warn,
    Bad,
    /// Secondary detail, drawn muted.
    Muted,
}

/// One thing a report shows.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ProbeItem {
    /// A headline number: "22 fps". Drawn large.
    Figure { label: String, value: String, unit: String, level: ProbeLevel },
    /// A labelled value on one line.
    Stat { label: String, value: String, level: ProbeLevel },
    /// A history as a sparkline, oldest first.
    Series { label: String, values: Vec<f32>, unit: String },
    /// A filled bar, `fraction` 0 to 1, with its value in words.
    Bar { label: String, fraction: f32, value: String, level: ProbeLevel },
    /// Rows under column headings.
    Table { columns: Vec<String>, rows: Vec<Vec<String>> },
    /// A sentence: a warning, an explanation, an empty state.
    Note { text: String, level: ProbeLevel },
    /// An on/off control. Flipping it calls the probe's `act` with `id`.
    Toggle { id: String, label: String, on: bool },
    /// A push button. Pressing it calls the probe's `act` with `id`.
    Button { id: String, label: String },
}

impl ProbeItem {
    pub fn figure(label: impl Into<String>, value: impl Into<String>, unit: impl Into<String>, level: ProbeLevel) -> Self {
        Self::Figure { label: label.into(), value: value.into(), unit: unit.into(), level }
    }
    pub fn stat(label: impl Into<String>, value: impl ToString) -> Self {
        Self::Stat { label: label.into(), value: value.to_string(), level: ProbeLevel::Normal }
    }
    pub fn stat_at(label: impl Into<String>, value: impl ToString, level: ProbeLevel) -> Self {
        Self::Stat { label: label.into(), value: value.to_string(), level }
    }
    pub fn series(label: impl Into<String>, values: Vec<f32>, unit: impl Into<String>) -> Self {
        Self::Series { label: label.into(), values, unit: unit.into() }
    }
    pub fn bar(label: impl Into<String>, fraction: f32, value: impl Into<String>, level: ProbeLevel) -> Self {
        Self::Bar { label: label.into(), fraction: fraction.clamp(0.0, 1.0), value: value.into(), level }
    }
    pub fn table(columns: &[&str], rows: Vec<Vec<String>>) -> Self {
        Self::Table { columns: columns.iter().map(|c| c.to_string()).collect(), rows }
    }
    pub fn note(text: impl Into<String>, level: ProbeLevel) -> Self {
        Self::Note { text: text.into(), level }
    }
    pub fn toggle(id: impl Into<String>, label: impl Into<String>, on: bool) -> Self {
        Self::Toggle { id: id.into(), label: label.into(), on }
    }
    pub fn button(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::Button { id: id.into(), label: label.into() }
    }
}

/// A titled group of items.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct ProbeSection {
    pub title: String,
    pub items: Vec<ProbeItem>,
}

/// What a probe sends: its sections, in order.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct ProbeReport {
    pub sections: Vec<ProbeSection>,
}

impl ProbeReport {
    /// Start a section; items pushed with [`Self::item`] go into it.
    pub fn section(&mut self, title: impl Into<String>) -> &mut Self {
        self.sections.push(ProbeSection { title: title.into(), items: Vec::new() });
        self
    }

    /// Add `item` to the last section, starting an untitled one if there is
    /// none yet.
    pub fn item(&mut self, item: ProbeItem) -> &mut Self {
        if self.sections.is_empty() {
            self.sections.push(ProbeSection::default());
        }
        if let Some(section) = self.sections.last_mut() {
            section.items.push(item);
        }
        self
    }
}

/// One registered probe.
#[derive(Clone)]
pub struct ProbeDef {
    /// Stable id; the companion asks for probes by it.
    pub id: &'static str,
    pub title: &'static str,
    /// Phosphor icon name.
    pub icon: &'static str,
    /// Groups probes in the companion's list, e.g. "Rendering".
    pub category: &'static str,
    /// Build the report. Runs on the main thread with the whole `World`, a
    /// few times a second while watched; keep it to reading.
    pub build: fn(&mut World) -> ProbeReport,
    /// Act on a toggle or button from the report: `(world, item id, new
    /// value for a toggle)`. `None` for a probe with no controls.
    pub act: Option<fn(&mut World, &str, Option<bool>)>,
}

/// Every probe this process offers.
#[derive(Resource, Default, Clone)]
pub struct ProbeRegistry(pub Vec<ProbeDef>);

/// The probes a companion is watching right now; kept by the profiler's
/// server. Read it through [`probe_watched`].
#[derive(Resource, Default, Clone, Debug)]
pub struct WatchedProbes(pub HashSet<String>);

/// Run condition: `id` is being watched. Gate a probe's data-gathering
/// systems on it, so an unwatched probe costs nothing.
pub fn probe_watched(id: &'static str) -> impl FnMut(Option<Res<WatchedProbes>>) -> bool + Clone {
    move |watched: Option<Res<WatchedProbes>>| watched.is_some_and(|w| w.0.contains(id))
}

/// Registering a probe on the `App`.
pub trait RegisterProbe {
    fn register_probe(&mut self, probe: ProbeDef) -> &mut Self;
}

impl RegisterProbe for App {
    fn register_probe(&mut self, probe: ProbeDef) -> &mut Self {
        self.init_resource::<ProbeRegistry>().init_resource::<WatchedProbes>();
        let mut registry = self.world_mut().resource_mut::<ProbeRegistry>();
        // A plugin built twice (a reload) registers again; the later one wins.
        registry.0.retain(|p| p.id != probe.id);
        registry.0.push(probe);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(_: &mut World) -> ProbeReport {
        let mut report = ProbeReport::default();
        report
            .section("Frame")
            .item(ProbeItem::figure("Frame rate", "60", "fps", ProbeLevel::Good))
            .item(ProbeItem::series("History", vec![16.0, 17.0], "ms"));
        report
    }

    #[test]
    fn a_report_survives_the_wire() {
        let report = build(&mut World::new());
        let wire = serde_json::to_string(&report).unwrap();
        assert_eq!(serde_json::from_str::<ProbeReport>(&wire).unwrap(), report);
        assert_eq!(report.sections[0].items.len(), 2);
    }

    #[test]
    fn registering_twice_keeps_one() {
        let mut app = App::new();
        let def = ProbeDef { id: "fps", title: "FPS", icon: "gauge", category: "Frame", build, act: None };
        app.register_probe(def.clone()).register_probe(def);
        assert_eq!(app.world().resource::<ProbeRegistry>().0.len(), 1);
    }

    #[test]
    fn only_watched_probes_gather() {
        let mut app = App::new();
        app.init_resource::<WatchedProbes>();
        #[derive(Resource, Default)]
        struct Ran(u32);
        app.init_resource::<Ran>();
        app.add_systems(Update, (|mut r: ResMut<Ran>| r.0 += 1).run_if(probe_watched("fps")));
        app.update();
        assert_eq!(app.world().resource::<Ran>().0, 0);
        app.world_mut().resource_mut::<WatchedProbes>().0.insert("fps".into());
        app.update();
        assert_eq!(app.world().resource::<Ran>().0, 1);
    }
}
