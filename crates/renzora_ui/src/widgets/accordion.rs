//! Accordion group — sibling collapsibles with one-at-a-time opening.
//!
//! Built on top of `collapsible_section`. Pass a closure per section; the
//! group tracks which index is open in egui memory.

use bevy_egui::egui;
use renzora_theme::Theme;

pub struct AccordionSection<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub icon: &'a str,
    pub category: &'a str,
}

/// Render a set of sibling collapsible sections where at most one is open
/// at a time. Returns the index of the currently-open section.
pub fn accordion(
    ui: &mut egui::Ui,
    group_id: egui::Id,
    sections: &[AccordionSection<'_>],
    theme: &Theme,
    mut body: impl FnMut(&mut egui::Ui, usize),
) -> Option<usize> {
    let mut open = ui
        .memory(|m| m.data.get_temp::<Option<usize>>(group_id).flatten());

    for (i, s) in sections.iter().enumerate() {
        let is_open = Some(i) == open;
        let section_id = format!("{}_{}", s.id, i);
        super::category::collapsible_section(
            ui,
            s.icon,
            s.title,
            s.category,
            theme,
            &section_id,
            is_open,
            |ui| body(ui, i),
        );
        // collapsible_section persists its own open state via egui ids; we
        // piggyback: when the user toggles one section open, close others
        // by detecting a transition this frame.
        let opened_now = ui.memory(|m| {
            m.data
                .get_temp::<bool>(egui::Id::new(section_id.clone()).with("open"))
                .unwrap_or(is_open)
        });
        if opened_now && Some(i) != open {
            open = Some(i);
        } else if !opened_now && Some(i) == open {
            open = None;
        }
    }
    ui.memory_mut(|m| m.data.insert_temp::<Option<usize>>(group_id, open));
    open
}
