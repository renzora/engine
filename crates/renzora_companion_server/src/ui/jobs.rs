//! The Jobs screen: what the companion is doing for the editor.

use eframe::egui::{self, RichText};

use super::{send, section, GOOD, WARN};
use crate::view::{Command, View};

pub(super) fn ui(ui: &mut egui::Ui, view: &View) {
    // Only when a job is actually held: shown all the time, it claimed a wait
    // nothing was in.
    if let Some(why) = &view.held {
        ui.colored_label(WARN, format!("Jobs wait for {why}, so they never compete with what you are measuring or playing."));
        ui.add_space(6.0);
    }
    if let Some(worker) = &view.render_worker {
        ui.label(RichText::new(format!("Render worker: {worker}")).weak());
        ui.add_space(4.0);
    }
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        section(ui, &format!("Running  {}", view.running.len()), |ui| {
            if view.running.is_empty() {
                ui.weak("Nothing running.");
            }
            for row in &view.running {
                ui.horizontal(|ui| {
                    ui.label(&row.label);
                    ui.monospace(RichText::new(&row.target).weak());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Cancel").clicked() {
                            send(Command::CancelJob { id: row.id });
                        }
                        ui.monospace(RichText::new(&row.note).weak());
                        let bar = match row.fraction {
                            Some(f) => egui::ProgressBar::new(f),
                            None => egui::ProgressBar::new(0.0).animate(true),
                        };
                        ui.add(bar.desired_width(160.0).desired_height(6.0));
                    });
                });
            }
        });

        section(ui, &format!("Waiting  {}", view.waiting_total), |ui| {
            for row in &view.waiting {
                ui.horizontal(|ui| {
                    ui.label(&row.label);
                    ui.monospace(RichText::new(&row.target).weak());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Cancel").clicked() {
                            send(Command::CancelJob { id: row.id });
                        }
                        if row.forceable && ui.small_button("Run anyway").clicked() {
                            send(Command::RunAnyway { id: row.id });
                        }
                        ui.weak(&row.note);
                    });
                });
            }
            if view.waiting_total > view.waiting.len() {
                ui.weak(format!("and {} more", view.waiting_total - view.waiting.len()));
            }
        });

        section(ui, "Finished", |ui| {
            if view.finished.is_empty() {
                ui.weak("Nothing yet.");
            }
            for done in &view.finished {
                ui.horizontal(|ui| {
                    ui.label(&done.label);
                    ui.monospace(RichText::new(&done.target).weak());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| match &done.result {
                        Ok(_) => {
                            ui.colored_label(GOOD, format!("done in {:.0} ms", done.took_ms));
                        }
                        Err(e) => {
                            ui.colored_label(WARN, format!("failed: {e}"));
                        }
                    });
                });
            }
        });
    });
}
