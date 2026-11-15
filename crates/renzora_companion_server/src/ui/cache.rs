//! The Cache screen: the open project as the companion sees it, and the
//! thumbnail cache it keeps in step with it.

use eframe::egui;

use super::{bytes, send, section, WARN};
use crate::view::{CacheView, Command};

pub(super) fn ui(ui: &mut egui::Ui, cache: &CacheView) {
    section(ui, "Project", |ui| {
        let Some(root) = &cache.root else {
            ui.weak("No project open yet.");
            return;
        };
        ui.monospace(root);
        if cache.walking {
            ui.weak("listing its files...");
            return;
        }
        ui.monospace(format!("{} files, {}", cache.files, bytes(cache.bytes)));
        ui.monospace(format!("{} models, {} materials", cache.models, cache.materials));
        if let Some(why) = &cache.watch_error {
            ui.colored_label(WARN, format!("Not watching for changes: {why}"));
        }
    });
    section(ui, "Thumbnails", |ui| {
        if cache.render_ahead {
            ui.weak("Every model and material without a current thumbnail gets one, a few at a time while nothing else is queued. A deleted model's or material's thumbnail is deleted with it.");
        } else {
            ui.weak("The engine is not beside the companion, so thumbnails are not rendered ahead of time: the editor draws one when a folder needs it. A deleted model's or material's thumbnail is deleted with it.");
        }
        ui.add_space(4.0);
        ui.monospace(match cache.thumbnails {
            Some((count, size)) => format!("{count} cached, {}", bytes(size)),
            None if cache.root.is_some() => "measuring...".to_string(),
            None => String::new(),
        });
        ui.monospace(format!(
            "This session: {} rendered ahead, {} removed with their file",
            cache.prewarmed, cache.pruned
        ));
        if cache.backlog > 0 {
            ui.monospace(format!("{} files still to look at", cache.backlog));
        }
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let missing = ui.add_enabled(cache.root.is_some() && cache.render_ahead, egui::Button::new("Render missing thumbnails"));
            if missing.clicked() {
                send(Command::RenderMissing);
            }
            let clear = ui.add_enabled(cache.root.is_some(), egui::Button::new("Clear thumbnail cache"));
            if clear.clicked() {
                send(Command::ClearThumbnails);
            }
        });
    });
}
