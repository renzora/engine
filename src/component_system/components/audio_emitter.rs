//! Audio Player component definition
//!
//! Represents a sound-producing entity with full kira-backed playback control.

use bevy::prelude::*;
use bevy_egui::egui;
use serde::{Deserialize, Serialize};

use crate::component_system::{
    ComponentCategory, ComponentRegistry, PropertyValue, PropertyValueType,
};
use crate::core::InspectorPanelRenderState;
use crate::project::CurrentProject;
use crate::register_component;

use crate::ui::{inline_property, property_row};

use egui_phosphor::regular::{SPEAKER_HIGH, MUSIC_NOTE, MAGNIFYING_GLASS, FILE_AUDIO, TRASH, PLAY, STOP};

// ============================================================================
// Data Types
// ============================================================================

/// Distance rolloff curve for spatial audio
#[derive(Clone, Debug, Default, Reflect, Serialize, Deserialize, PartialEq)]
pub enum RolloffType {
    #[default]
    Logarithmic,
    Linear,
}

impl std::fmt::Display for RolloffType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RolloffType::Logarithmic => write!(f, "Logarithmic"),
            RolloffType::Linear => write!(f, "Linear"),
        }
    }
}

/// Full audio emitter component with kira-backed playback
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct AudioEmitterData {
    pub clip: String,
    pub volume: f32,
    pub pitch: f32,
    pub panning: f32,
    pub looping: bool,
    pub loop_start: f64,
    pub loop_end: f64,
    pub autoplay: bool,
    pub fade_in: f32,
    pub bus: String,
    pub spatial: bool,
    pub spatial_min_distance: f32,
    pub spatial_max_distance: f32,
    pub spatial_rolloff: RolloffType,
    pub reverb_send: f32,
    pub delay_send: f32,
}

impl Default for AudioEmitterData {
    fn default() -> Self {
        Self {
            clip: String::new(),
            volume: 1.0,
            pitch: 1.0,
            panning: 0.0,
            looping: false,
            loop_start: 0.0,
            loop_end: 0.0,
            autoplay: false,
            fade_in: 0.0,
            bus: "Sfx".to_string(),
            spatial: false,
            spatial_min_distance: 1.0,
            spatial_max_distance: 50.0,
            spatial_rolloff: RolloffType::Logarithmic,
            reverb_send: 0.0,
            delay_send: 0.0,
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

const AUDIO_EXTENSIONS: &[&str] = &["wav", "ogg", "mp3", "flac", "opus"];

fn is_audio_ext(ext: &str) -> bool {
    AUDIO_EXTENSIONS.iter().any(|&e| e.eq_ignore_ascii_case(ext))
}

/// Recursively scan a folder for audio files, returning (display_name, path) pairs.
fn scan_audio_folder(folder: &std::path::Path) -> Vec<(String, std::path::PathBuf)> {
    let mut files = Vec::new();
    scan_audio_folder_recursive(folder, &mut files);
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

fn scan_audio_folder_recursive(dir: &std::path::Path, out: &mut Vec<(String, std::path::PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_audio_folder_recursive(&path, out);
        } else {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if is_audio_ext(ext) {
                let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();
                out.push((name, path));
            }
        }
    }
}

// ============================================================================
// Inspector
// ============================================================================

fn inspect_audio_emitter(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    // ── Read immutable state before mutable borrows ──────────────────────
    let dragging_path = world.get_resource::<InspectorPanelRenderState>()
        .and_then(|rs| rs.dragging_asset_path.clone());

    let project_path = world.get_resource::<CurrentProject>()
        .map(|p| p.path.clone());

    // Scan project for audio files
    let all_audio_files: Vec<(String, std::path::PathBuf)> = project_path.as_ref()
        .map(|p| scan_audio_folder(p))
        .unwrap_or_default();

    // Collect OS dropped audio files
    let os_dropped_audio: Vec<std::path::PathBuf> = ui.ctx().input(|i| {
        i.raw.dropped_files.iter()
            .filter_map(|f| f.path.clone())
            .filter(|p| {
                let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
                is_audio_ext(ext)
            })
            .collect()
    });

    // Read available buses before mutable access
    let mut available_buses = vec!["Master".to_string(), "Sfx".to_string(), "Music".to_string(), "Ambient".to_string()];
    if let Some(mixer) = world.get_resource::<crate::audio::MixerState>() {
        for (name, _) in &mixer.custom_buses {
            available_buses.push(name.clone());
        }
    }

    // Check if preview is currently playing for this entity
    let is_previewing = world.get_resource::<crate::audio::AudioPreviewState>()
        .map(|p| p.is_playing_entity(entity))
        .unwrap_or(false);

    let theme_colors = crate::ui::get_inspector_theme(ui.ctx());
    let available_width = ui.available_width();

    let mut changed = false;
    let mut should_clear_drag = false;
    let mut row = 0;

    // ── Clip (current value + preview/stop + clear) ──────────────────────
    let clip = world.get::<AudioEmitterData>(entity).map(|d| d.clip.clone()).unwrap_or_default();
    let is_empty = clip.is_empty();

    property_row(ui, row, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(egui::RichText::new(MUSIC_NOTE).size(13.0).color(theme_colors.semantic_accent));
            if is_empty {
                ui.label(egui::RichText::new("No clip selected").size(11.0).color(theme_colors.text_muted));
            } else {
                let display = clip.rsplit('/').next().or_else(|| clip.rsplit('\\').next()).unwrap_or(&clip);
                ui.label(egui::RichText::new(display).size(11.0));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Clear button
                if !is_empty {
                    if ui.add(
                        egui::Button::new(egui::RichText::new(TRASH).size(12.0).color(theme_colors.semantic_error))
                            .frame(false)
                    ).on_hover_text("Clear clip").clicked() {
                        if let Some(mut data) = world.get_mut::<AudioEmitterData>(entity) {
                            data.clip = String::new();
                            changed = true;
                        }
                    }
                }
                // Play / Stop toggle button
                if !is_empty {
                    let (icon, tooltip, color) = if is_previewing {
                        (STOP, "Stop", theme_colors.semantic_error)
                    } else {
                        (PLAY, "Preview", theme_colors.semantic_accent)
                    };
                    if ui.add(
                        egui::Button::new(egui::RichText::new(icon).size(12.0).color(color)).frame(false)
                    ).on_hover_text(tooltip).clicked() {
                        if is_previewing {
                            // Stop
                            world.resource_scope(|_world, mut preview: bevy::ecs::world::Mut<crate::audio::AudioPreviewState>| {
                                preview.stop();
                            });
                        } else {
                            // Play
                            let path = clip.clone();
                            let bus = world.get::<AudioEmitterData>(entity).map(|d| d.bus.clone()).unwrap_or_else(|| "Sfx".to_string());
                            world.resource_scope(|world, mut preview: bevy::ecs::world::Mut<crate::audio::AudioPreviewState>| {
                                world.resource_scope(|world, mixer: bevy::ecs::world::Mut<crate::audio::MixerState>| {
                                    if let Some(mut manager) = world.get_non_send_resource_mut::<crate::audio::KiraAudioManager>() {
                                        preview.play(&mut manager, &path, &bus, &mixer, entity);
                                    }
                                });
                            });
                        }
                    }
                }
            });
        });
    });
    row += 1;

    // ── Clip Search ──────────────────────────────────────────────────────
    let search_id = ui.id().with("audio_clip_search");
    let mut search_text = ui.ctx().data_mut(|d| d.get_temp::<String>(search_id).unwrap_or_default());

    property_row(ui, row, |ui| {
        let search_resp = ui.add(
            egui::TextEdit::singleline(&mut search_text)
                .hint_text(format!("{} Search audio clips...", MAGNIFYING_GLASS))
                .desired_width(f32::INFINITY),
        );
        if search_resp.changed() {
            ui.ctx().data_mut(|d| d.insert_temp(search_id, search_text.clone()));
        }

        // ── Search Results Popup ─────────────────────────────────────────
        if !search_text.is_empty() && (search_resp.has_focus() || search_resp.lost_focus()) {
            let query = search_text.to_lowercase();
            let matching: Vec<_> = all_audio_files.iter()
                .filter(|(n, _)| n.to_lowercase().contains(&query))
                .collect();

            let popup_id = search_id.with("popup");
            let popup_pos = search_resp.rect.left_bottom() + egui::vec2(0.0, 2.0);
            let popup_width = search_resp.rect.width();

            let inactive_bg = theme_colors.widget_inactive_bg;
            let hovered_bg = theme_colors.widget_hovered_bg;
            let border_color = theme_colors.widget_border;
            let muted_color = theme_colors.text_muted;
            let project_path_ref = project_path.clone();

            let area_resp = egui::Area::new(popup_id)
                .order(egui::Order::Foreground)
                .fixed_pos(popup_pos)
                .show(ui.ctx(), |ui| -> Option<String> {
                    let mut clicked: Option<String> = None;
                    ui.set_width(popup_width);

                    egui::Frame::new()
                        .fill(inactive_bg)
                        .stroke(egui::Stroke::new(1.0, border_color))
                        .corner_radius(egui::CornerRadius::same(4))
                        .inner_margin(egui::Margin::same(4))
                        .show(ui, |ui| {
                            if matching.is_empty() {
                                ui.label(egui::RichText::new("No audio files found").size(11.0).color(muted_color));
                                return;
                            }

                            egui::ScrollArea::vertical()
                                .max_height(160.0)
                                .id_salt("audio_search_results")
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    for (name, path) in &matching {
                                        let resp = ui.add(
                                            egui::Button::new(
                                                egui::RichText::new(format!("{} {}", MUSIC_NOTE, name)).size(11.0)
                                            )
                                            .frame(false)
                                            .min_size(egui::Vec2::new(ui.available_width(), 20.0)),
                                        );
                                        if resp.hovered() {
                                            let [r, g, b, _] = hovered_bg.to_array();
                                            ui.painter().rect_filled(resp.rect, 2.0, egui::Color32::from_rgba_unmultiplied(r, g, b, 80));
                                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                        }
                                        if resp.clicked() {
                                            clicked = Some(if let Some(ref proj) = project_path_ref {
                                                path.strip_prefix(proj).unwrap_or(path).to_string_lossy().to_string()
                                            } else {
                                                path.to_string_lossy().to_string()
                                            });
                                        }
                                    }
                                });
                        });

                    clicked
                });

            if let Some(rel_path) = area_resp.inner {
                if let Some(mut data) = world.get_mut::<AudioEmitterData>(entity) {
                    data.clip = rel_path;
                    changed = true;
                }
                ui.ctx().data_mut(|d| d.insert_temp::<String>(search_id, String::new()));
            }

            // Eat scroll events inside popup
            if area_resp.response.rect.contains(ui.ctx().pointer_hover_pos().unwrap_or_default()) {
                ui.ctx().input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
            }
        }
    });
    row += 1;

    // ── Drop Zone ────────────────────────────────────────────────────────
    property_row(ui, row, |ui| {
        let drop_zone_height = 36.0;
        let drop_width = ui.available_width();
        let (rect, _response) = ui.allocate_exact_size(
            egui::Vec2::new(drop_width, drop_zone_height),
            egui::Sense::click_and_drag(),
        );

        let pointer_pos = ui.ctx().pointer_hover_pos();
        let pointer_in_zone = pointer_pos.map_or(false, |p| rect.contains(p));

        ui.painter().rect_filled(rect, 4.0, theme_colors.widget_inactive_bg);

        let is_dragging_audio = dragging_path.as_ref().map_or(false, |p: &std::path::PathBuf| {
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
            is_audio_ext(ext)
        });

        if is_dragging_audio && pointer_in_zone {
            ui.painter().rect_stroke(
                rect, 4.0,
                egui::Stroke::new(2.0, theme_colors.semantic_accent),
                egui::StrokeKind::Inside,
            );
        } else {
            ui.painter().rect_stroke(
                rect, 4.0,
                egui::Stroke::new(1.0, theme_colors.widget_border),
                egui::StrokeKind::Outside,
            );
        }

        let center = rect.center();
        ui.painter().text(
            egui::pos2(center.x, center.y - 6.0),
            egui::Align2::CENTER_CENTER,
            FILE_AUDIO,
            egui::FontId::proportional(14.0),
            theme_colors.text_muted,
        );
        ui.painter().text(
            egui::pos2(center.x, center.y + 8.0),
            egui::Align2::CENTER_CENTER,
            "Drop audio file here",
            egui::FontId::proportional(10.0),
            theme_colors.text_muted,
        );

        if is_dragging_audio && pointer_in_zone && ui.ctx().input(|i| i.pointer.any_released()) {
            if let Some(ref asset_path) = dragging_path {
                let rel_path = if let Some(ref proj) = project_path {
                    asset_path.strip_prefix(proj).unwrap_or(asset_path).to_string_lossy().to_string()
                } else {
                    asset_path.to_string_lossy().to_string()
                };
                if let Some(mut data) = world.get_mut::<AudioEmitterData>(entity) {
                    data.clip = rel_path;
                    changed = true;
                }
                should_clear_drag = true;
            }
        }

        if let Some(dropped) = os_dropped_audio.first() {
            let rel_path = if let Some(ref proj) = project_path {
                dropped.strip_prefix(proj).unwrap_or(dropped).to_string_lossy().to_string()
            } else {
                dropped.to_string_lossy().to_string()
            };
            if let Some(mut data) = world.get_mut::<AudioEmitterData>(entity) {
                data.clip = rel_path;
                changed = true;
            }
        }
    });
    row += 1;

    if should_clear_drag {
        if let Some(mut rs) = world.get_resource_mut::<InspectorPanelRenderState>() {
            rs.dragging_asset_path = None;
        }
    }

    // ── Volume ────────────────────────────────────────────────────────────
    {
        let mut volume = world.get::<AudioEmitterData>(entity).map(|d| d.volume).unwrap_or(1.0);
        let resp = inline_property(ui, row, "Volume", |ui| {
            ui.add(egui::Slider::new(&mut volume, 0.0..=2.0)).changed()
        });
        if resp {
            if let Some(mut data) = world.get_mut::<AudioEmitterData>(entity) {
                data.volume = volume;
                changed = true;
            }
        }
    }
    row += 1;

    // ── Pitch ─────────────────────────────────────────────────────────────
    {
        let mut pitch = world.get::<AudioEmitterData>(entity).map(|d| d.pitch).unwrap_or(1.0);
        let resp = inline_property(ui, row, "Pitch", |ui| {
            ui.add(egui::Slider::new(&mut pitch, 0.1..=4.0)).changed()
        });
        if resp {
            if let Some(mut data) = world.get_mut::<AudioEmitterData>(entity) {
                data.pitch = pitch;
                changed = true;
            }
        }
    }
    row += 1;

    // ── Panning ──────────────────────────────────────────────────────────
    {
        let mut panning = world.get::<AudioEmitterData>(entity).map(|d| d.panning).unwrap_or(0.0);
        let resp = inline_property(ui, row, "Panning", |ui| {
            ui.add(egui::Slider::new(&mut panning, -1.0..=1.0)).changed()
        });
        if resp {
            if let Some(mut data) = world.get_mut::<AudioEmitterData>(entity) {
                data.panning = panning;
                changed = true;
            }
        }
    }
    row += 1;

    // ── Bus ──────────────────────────────────────────────────────────────
    {
        let bus = world.get::<AudioEmitterData>(entity).map(|d| d.bus.clone()).unwrap_or_else(|| "Sfx".to_string());
        property_row(ui, row, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.add_sized(
                    [80.0, 16.0],
                    egui::Label::new(egui::RichText::new("Bus").size(11.0)).truncate(),
                );
                egui::ComboBox::from_id_salt("audio_bus")
                    .selected_text(&bus)
                    .show_ui(ui, |ui| {
                        for variant in &available_buses {
                            let selected = *variant == bus;
                            if ui.selectable_label(selected, variant).clicked() {
                                if let Some(mut data) = world.get_mut::<AudioEmitterData>(entity) {
                                    data.bus = variant.clone();
                                    changed = true;
                                }
                            }
                        }
                    });
            });
        });
    }
    row += 1;

    // ── Looping ──────────────────────────────────────────────────────────
    {
        let mut looping = world.get::<AudioEmitterData>(entity).map(|d| d.looping).unwrap_or(false);
        let resp = inline_property(ui, row, "Looping", |ui| {
            ui.checkbox(&mut looping, "").changed()
        });
        if resp {
            if let Some(mut data) = world.get_mut::<AudioEmitterData>(entity) {
                data.looping = looping;
                changed = true;
            }
        }
    }
    row += 1;

    // ── Loop Region (conditional) ────────────────────────────────────────
    let looping = world.get::<AudioEmitterData>(entity).map(|d| d.looping).unwrap_or(false);
    if looping {
        {
            let mut loop_start = world.get::<AudioEmitterData>(entity).map(|d| d.loop_start).unwrap_or(0.0);
            let resp = inline_property(ui, row, "Loop Start", |ui| {
                ui.add(egui::DragValue::new(&mut loop_start).speed(0.01).range(0.0..=f64::MAX).suffix("s")).changed()
            });
            if resp {
                if let Some(mut data) = world.get_mut::<AudioEmitterData>(entity) {
                    data.loop_start = loop_start;
                    changed = true;
                }
            }
        }
        row += 1;

        {
            let mut loop_end = world.get::<AudioEmitterData>(entity).map(|d| d.loop_end).unwrap_or(0.0);
            let resp = inline_property(ui, row, "Loop End", |ui| {
                ui.add(egui::DragValue::new(&mut loop_end).speed(0.01).range(0.0..=f64::MAX).suffix("s")).changed()
            });
            if resp {
                if let Some(mut data) = world.get_mut::<AudioEmitterData>(entity) {
                    data.loop_end = loop_end;
                    changed = true;
                }
            }
        }
        row += 1;
    }

    // ── Autoplay ─────────────────────────────────────────────────────────
    {
        let mut autoplay = world.get::<AudioEmitterData>(entity).map(|d| d.autoplay).unwrap_or(false);
        let resp = inline_property(ui, row, "Autoplay", |ui| {
            ui.checkbox(&mut autoplay, "").changed()
        });
        if resp {
            if let Some(mut data) = world.get_mut::<AudioEmitterData>(entity) {
                data.autoplay = autoplay;
                changed = true;
            }
        }
    }
    row += 1;

    // ── Fade In ──────────────────────────────────────────────────────────
    {
        let mut fade_in = world.get::<AudioEmitterData>(entity).map(|d| d.fade_in).unwrap_or(0.0);
        let resp = inline_property(ui, row, "Fade In", |ui| {
            ui.add(egui::DragValue::new(&mut fade_in).speed(0.05).range(0.0..=10.0).suffix("s")).changed()
        });
        if resp {
            if let Some(mut data) = world.get_mut::<AudioEmitterData>(entity) {
                data.fade_in = fade_in;
                changed = true;
            }
        }
    }
    row += 1;

    // ── Spatial Audio ────────────────────────────────────────────────────
    {
        let mut spatial = world.get::<AudioEmitterData>(entity).map(|d| d.spatial).unwrap_or(false);
        let resp = inline_property(ui, row, "Spatial", |ui| {
            ui.checkbox(&mut spatial, "").changed()
        });
        if resp {
            if let Some(mut data) = world.get_mut::<AudioEmitterData>(entity) {
                data.spatial = spatial;
                changed = true;
            }
        }
    }
    row += 1;

    let spatial = world.get::<AudioEmitterData>(entity).map(|d| d.spatial).unwrap_or(false);
    if spatial {
        {
            let mut min_dist = world.get::<AudioEmitterData>(entity).map(|d| d.spatial_min_distance).unwrap_or(1.0);
            let resp = inline_property(ui, row, "  Min Distance", |ui| {
                ui.add(egui::DragValue::new(&mut min_dist).speed(0.1).range(0.01..=1000.0)).changed()
            });
            if resp {
                if let Some(mut data) = world.get_mut::<AudioEmitterData>(entity) {
                    data.spatial_min_distance = min_dist;
                    changed = true;
                }
            }
        }
        row += 1;

        {
            let mut max_dist = world.get::<AudioEmitterData>(entity).map(|d| d.spatial_max_distance).unwrap_or(50.0);
            let resp = inline_property(ui, row, "  Max Distance", |ui| {
                ui.add(egui::DragValue::new(&mut max_dist).speed(0.5).range(0.1..=10000.0)).changed()
            });
            if resp {
                if let Some(mut data) = world.get_mut::<AudioEmitterData>(entity) {
                    data.spatial_max_distance = max_dist;
                    changed = true;
                }
            }
        }
        row += 1;

        {
            let rolloff = world.get::<AudioEmitterData>(entity).map(|d| d.spatial_rolloff.clone()).unwrap_or_default();
            property_row(ui, row, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    ui.add_sized(
                        [80.0, 16.0],
                        egui::Label::new(egui::RichText::new("  Rolloff").size(11.0)).truncate(),
                    );
                    egui::ComboBox::from_id_salt("rolloff_type")
                        .selected_text(format!("{}", rolloff))
                        .show_ui(ui, |ui| {
                            for variant in [RolloffType::Logarithmic, RolloffType::Linear] {
                                let selected = variant == rolloff;
                                if ui.selectable_label(selected, format!("{}", variant)).clicked() {
                                    if let Some(mut data) = world.get_mut::<AudioEmitterData>(entity) {
                                        data.spatial_rolloff = variant;
                                        changed = true;
                                    }
                                }
                            }
                        });
                });
            });
        }
        row += 1;
    }

    // ── Reverb Send ─────────────────────────────────────────────────────
    {
        let mut reverb = world.get::<AudioEmitterData>(entity).map(|d| d.reverb_send).unwrap_or(0.0);
        let resp = inline_property(ui, row, "Reverb Send", |ui| {
            ui.add(egui::Slider::new(&mut reverb, 0.0..=1.0)).changed()
        });
        if resp {
            if let Some(mut data) = world.get_mut::<AudioEmitterData>(entity) {
                data.reverb_send = reverb;
                changed = true;
            }
        }
    }
    row += 1;

    // ── Delay Send ──────────────────────────────────────────────────────
    {
        let mut delay = world.get::<AudioEmitterData>(entity).map(|d| d.delay_send).unwrap_or(0.0);
        let resp = inline_property(ui, row, "Delay Send", |ui| {
            ui.add(egui::Slider::new(&mut delay, 0.0..=1.0)).changed()
        });
        if resp {
            if let Some(mut data) = world.get_mut::<AudioEmitterData>(entity) {
                data.delay_send = delay;
                changed = true;
            }
        }
    }

    changed
}

// ============================================================================
// Scripting Integration
// ============================================================================

fn get_script_properties(world: &World, entity: Entity) -> Vec<(&'static str, PropertyValue)> {
    let Some(data) = world.get::<AudioEmitterData>(entity) else {
        return vec![];
    };
    vec![
        ("clip", PropertyValue::String(data.clip.clone())),
        ("volume", PropertyValue::Float(data.volume)),
        ("pitch", PropertyValue::Float(data.pitch)),
        ("panning", PropertyValue::Float(data.panning)),
        ("looping", PropertyValue::Bool(data.looping)),
        ("loop_start", PropertyValue::Float(data.loop_start as f32)),
        ("loop_end", PropertyValue::Float(data.loop_end as f32)),
        ("autoplay", PropertyValue::Bool(data.autoplay)),
        ("fade_in", PropertyValue::Float(data.fade_in)),
        ("bus", PropertyValue::String(data.bus.clone())),
        ("spatial", PropertyValue::Bool(data.spatial)),
        ("spatial_min_distance", PropertyValue::Float(data.spatial_min_distance)),
        ("spatial_max_distance", PropertyValue::Float(data.spatial_max_distance)),
        ("spatial_rolloff", PropertyValue::String(format!("{}", data.spatial_rolloff))),
        ("reverb_send", PropertyValue::Float(data.reverb_send)),
        ("delay_send", PropertyValue::Float(data.delay_send)),
    ]
}

fn set_script_property(world: &mut World, entity: Entity, prop: &str, val: &PropertyValue) -> bool {
    let Some(mut data) = world.get_mut::<AudioEmitterData>(entity) else {
        return false;
    };
    match prop {
        "clip" => { if let PropertyValue::String(v) = val { data.clip = v.clone(); true } else { false } }
        "volume" => { if let PropertyValue::Float(v) = val { data.volume = *v; true } else { false } }
        "pitch" => { if let PropertyValue::Float(v) = val { data.pitch = *v; true } else { false } }
        "panning" => { if let PropertyValue::Float(v) = val { data.panning = *v; true } else { false } }
        "looping" => { if let PropertyValue::Bool(v) = val { data.looping = *v; true } else { false } }
        "loop_start" => { if let PropertyValue::Float(v) = val { data.loop_start = *v as f64; true } else { false } }
        "loop_end" => { if let PropertyValue::Float(v) = val { data.loop_end = *v as f64; true } else { false } }
        "autoplay" => { if let PropertyValue::Bool(v) = val { data.autoplay = *v; true } else { false } }
        "fade_in" => { if let PropertyValue::Float(v) = val { data.fade_in = *v; true } else { false } }
        "bus" => { if let PropertyValue::String(v) = val { data.bus = v.clone(); true } else { false } }
        "spatial" => { if let PropertyValue::Bool(v) = val { data.spatial = *v; true } else { false } }
        "spatial_min_distance" => { if let PropertyValue::Float(v) = val { data.spatial_min_distance = *v; true } else { false } }
        "spatial_max_distance" => { if let PropertyValue::Float(v) = val { data.spatial_max_distance = *v; true } else { false } }
        "spatial_rolloff" => {
            if let PropertyValue::String(v) = val {
                data.spatial_rolloff = match v.as_str() {
                    "Linear" => RolloffType::Linear,
                    _ => RolloffType::Logarithmic,
                };
                true
            } else { false }
        }
        "reverb_send" => { if let PropertyValue::Float(v) = val { data.reverb_send = *v; true } else { false } }
        "delay_send" => { if let PropertyValue::Float(v) = val { data.delay_send = *v; true } else { false } }
        _ => false,
    }
}

fn script_property_meta() -> Vec<(&'static str, PropertyValueType)> {
    vec![
        ("clip", PropertyValueType::String),
        ("volume", PropertyValueType::Float),
        ("pitch", PropertyValueType::Float),
        ("panning", PropertyValueType::Float),
        ("looping", PropertyValueType::Bool),
        ("loop_start", PropertyValueType::Float),
        ("loop_end", PropertyValueType::Float),
        ("autoplay", PropertyValueType::Bool),
        ("fade_in", PropertyValueType::Float),
        ("bus", PropertyValueType::String),
        ("spatial", PropertyValueType::Bool),
        ("spatial_min_distance", PropertyValueType::Float),
        ("spatial_max_distance", PropertyValueType::Float),
        ("spatial_rolloff", PropertyValueType::String),
        ("reverb_send", PropertyValueType::Float),
        ("delay_send", PropertyValueType::Float),
    ]
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(AudioEmitterData {
        type_id: "audio_emitter",
        display_name: "Audio Player",
        category: ComponentCategory::Audio,
        icon: SPEAKER_HIGH,
        custom_inspector: inspect_audio_emitter,
        custom_script_properties: get_script_properties,
        custom_script_set: set_script_property,
        custom_script_meta: script_property_meta,
    }));
}
