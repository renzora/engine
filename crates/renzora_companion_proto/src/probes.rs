//! What a probe reports: plain data in a small vocabulary.
//!
//! Registering and running probes needs the `World`, and stays in
//! `renzora::probes`, which re-exports these. The report itself is data the
//! companion draws, so it is defined here, where the companion can read it
//! without linking Bevy.

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_survives_the_wire() {
        let mut report = ProbeReport::default();
        report
            .section("Frame")
            .item(ProbeItem::figure("Frame rate", "60", "fps", ProbeLevel::Good))
            .item(ProbeItem::series("History", vec![16.0, 17.0], "ms"));
        let wire = serde_json::to_string(&report).unwrap();
        assert_eq!(serde_json::from_str::<ProbeReport>(&wire).unwrap(), report);
        assert_eq!(report.sections[0].items.len(), 2);
    }
}
