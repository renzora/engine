//! Export overlay UI — a modal dialog for configuring and running project exports.

use bevy::prelude::*;
use bevy_egui::egui;
use egui_phosphor::regular;
use renzora_core::CurrentProject;
use renzora_rpak::{pack_directory, RpakPacker};
use renzora_theme::ThemeManager;

use crate::templates::{Platform, TemplateManager};

/// Packaging mode for the exported build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackagingMode {
    /// Runtime binary + .rpak file side by side.
    SeparateFiles,
    /// .rpak appended to the binary — single executable.
    SingleBinary,
}

/// Window mode for the exported game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowMode {
    Windowed,
    Fullscreen,
    Borderless,
}

/// Export progress state.
#[derive(Debug, Clone, PartialEq)]
pub enum ExportProgress {
    Idle,
    Packing,
    Writing,
    Done(String),
    Error(String),
}

/// Resource holding the export overlay state.
#[derive(Resource)]
pub struct ExportOverlayState {
    pub visible: bool,
    pub platform: Platform,
    pub packaging_mode: PackagingMode,
    pub window_mode: WindowMode,
    pub window_width: u32,
    pub window_height: u32,
    pub console_logging: bool,
    pub compression_level: i32,
    pub icon_path: Option<String>,
    pub output_dir: String,
    pub progress: ExportProgress,
}

impl Default for ExportOverlayState {
    fn default() -> Self {
        Self {
            visible: false,
            platform: Platform::current().unwrap_or(Platform::WindowsX64),
            packaging_mode: PackagingMode::SeparateFiles,
            window_mode: WindowMode::Windowed,
            window_width: 1280,
            window_height: 720,
            console_logging: false,
            compression_level: 3,
            icon_path: None,
            output_dir: String::new(),
            progress: ExportProgress::Idle,
        }
    }
}

pub fn draw_export_overlay(world: &mut World, ctx: &egui::Context) {
    // Dim background
    let screen = ctx.screen_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("export_overlay_bg"),
    ));
    painter.rect_filled(screen, 0.0, egui::Color32::from_black_alpha(160));

    // Read theme
    let (panel_bg, text_primary, text_secondary, accent, error_color, border_color, surface_mid) = {
        let theme_mgr = world.resource::<ThemeManager>();
        let t = &theme_mgr.active_theme;
        (
            t.surfaces.panel.0,
            t.text.primary.0,
            t.text.secondary.0,
            t.semantic.accent.0,
            t.semantic.error.0,
            t.widgets.border.0,
            t.surfaces.faint.0,
        )
    };

    let has_project = world.get_resource::<CurrentProject>().is_some();

    let window_width = 520.0;
    let window_id = egui::Id::new("export_overlay_window");

    egui::Area::new(window_id)
        .fixed_pos(egui::pos2(
            (screen.width() - window_width) / 2.0,
            screen.height() * 0.1,
        ))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let frame = egui::Frame::new()
                .fill(panel_bg)
                .stroke(egui::Stroke::new(1.0, border_color))
                .corner_radius(8.0)
                .inner_margin(egui::Margin::same(20));

            frame.show(ui, |ui| {
                ui.set_width(window_width);

                // Header
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{} Export Project", regular::PACKAGE))
                            .size(18.0)
                            .color(text_primary),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(egui::Button::new(
                                egui::RichText::new(regular::X).size(16.0).color(text_secondary),
                            ).frame(false))
                            .clicked()
                        {
                            world.resource_mut::<ExportOverlayState>().visible = false;
                        }
                    });
                });

                ui.add_space(4.0);
                ui.separator();
                ui.add_space(8.0);

                if !has_project {
                    ui.label(
                        egui::RichText::new("No project open. Open a project before exporting.")
                            .color(error_color),
                    );
                    return;
                }

                let project_name = world
                    .resource::<CurrentProject>()
                    .config
                    .name
                    .clone();

                // --- Platform ---
                section_label(ui, regular::DESKTOP_TOWER, "Platform", text_primary);
                ui.add_space(4.0);

                let mut export_state = world.resource_mut::<ExportOverlayState>();
                let current_platform_name = export_state.platform.display_name().to_string();

                let combo_width = ui.available_width() - 8.0;
                egui::ComboBox::from_id_salt("export_platform")
                    .selected_text(&current_platform_name)
                    .width(combo_width)
                    .show_ui(ui, |ui| {
                        ui.set_min_width(combo_width);
                        for platform in Platform::ALL {
                            let selected = export_state.platform == *platform;
                            let (rect, resp) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), 36.0),
                                egui::Sense::click(),
                            );
                            if resp.clicked() {
                                export_state.platform = *platform;
                            }
                            if resp.hovered() {
                                ui.painter().rect_filled(rect, 4.0, surface_mid);
                            }
                            ui.painter().text(
                                rect.left_top() + egui::vec2(6.0, 2.0),
                                egui::Align2::LEFT_TOP,
                                platform.display_name(),
                                egui::FontId::proportional(13.0),
                                if selected { accent } else { text_primary },
                            );
                            ui.painter().text(
                                rect.left_top() + egui::vec2(6.0, 18.0),
                                egui::Align2::LEFT_TOP,
                                platform.supported_devices(),
                                egui::FontId::proportional(11.0),
                                text_secondary,
                            );
                        }
                    });

                let selected_platform = export_state.platform;
                drop(export_state);

                // Template status
                let template_installed = world.resource::<TemplateManager>().is_installed(selected_platform);
                ui.add_space(2.0);
                if template_installed {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(regular::CHECK_CIRCLE).color(egui::Color32::from_rgb(89, 191, 115)));
                        ui.label(egui::RichText::new("Template installed").size(11.0).color(text_secondary));
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(regular::WARNING).color(egui::Color32::from_rgb(242, 166, 64)));
                        ui.label(egui::RichText::new("Template not installed").size(11.0).color(text_secondary));

                        if ui.add(egui::Button::new(
                            egui::RichText::new(format!("{} Install from file...", regular::FOLDER_OPEN)).size(11.0),
                        ).fill(surface_mid)).clicked() {
                            // Open file dialog to pick a template binary
                            let platform = selected_platform;
                            if let Some(path) = rfd::FileDialog::new()
                                .set_title("Select runtime template binary")
                                .pick_file()
                            {
                                let mut mgr = world.resource_mut::<TemplateManager>();
                                if let Err(e) = mgr.install_from_file(platform, &path) {
                                    warn!("Failed to install template: {}", e);
                                }
                            }
                        }
                    });
                }

                ui.add_space(12.0);

                let is_desktop = matches!(selected_platform,
                    Platform::WindowsX64 | Platform::LinuxX64 | Platform::MacOSX64 | Platform::MacOSArm64
                );

                // --- Packaging (desktop only) ---
                if is_desktop {
                    section_label(ui, regular::FILE_ARCHIVE, "Packaging", text_primary);
                    ui.add_space(4.0);

                    let mut export_state = world.resource_mut::<ExportOverlayState>();

                    ui.horizontal(|ui| {
                        ui.radio_value(
                            &mut export_state.packaging_mode,
                            PackagingMode::SeparateFiles,
                            egui::RichText::new("Binary + .rpak").color(text_primary),
                        );
                        ui.radio_value(
                            &mut export_state.packaging_mode,
                            PackagingMode::SingleBinary,
                            egui::RichText::new("Single executable").color(text_primary),
                        );
                    });

                    drop(export_state);
                }

                // --- Compression ---
                {
                    if !is_desktop {
                        section_label(ui, regular::FILE_ARCHIVE, "Packaging", text_primary);
                        ui.add_space(4.0);
                    } else {
                        ui.add_space(4.0);
                    }
                    let mut export_state = world.resource_mut::<ExportOverlayState>();
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Compression:").size(12.0).color(text_secondary));
                        ui.add(
                            egui::Slider::new(&mut export_state.compression_level, 1..=19)
                                .text("zstd level"),
                        );
                    });
                    drop(export_state);
                }

                ui.add_space(12.0);

                // --- Window Settings (desktop only) ---
                if is_desktop {
                    section_label(ui, regular::MONITOR, "Window", text_primary);
                    ui.add_space(4.0);

                    let mut export_state = world.resource_mut::<ExportOverlayState>();

                    ui.horizontal(|ui| {
                        ui.radio_value(
                            &mut export_state.window_mode,
                            WindowMode::Windowed,
                            egui::RichText::new("Windowed").color(text_primary),
                        );
                        ui.radio_value(
                            &mut export_state.window_mode,
                            WindowMode::Fullscreen,
                            egui::RichText::new("Fullscreen").color(text_primary),
                        );
                        ui.radio_value(
                            &mut export_state.window_mode,
                            WindowMode::Borderless,
                            egui::RichText::new("Borderless").color(text_primary),
                        );
                    });

                    if export_state.window_mode == WindowMode::Windowed {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Size:").size(12.0).color(text_secondary));
                            ui.add(egui::DragValue::new(&mut export_state.window_width).speed(1).range(320..=7680).suffix("w"));
                            ui.label(egui::RichText::new("x").color(text_secondary));
                            ui.add(egui::DragValue::new(&mut export_state.window_height).speed(1).range(240..=4320).suffix("h"));
                        });
                    }

                    drop(export_state);
                    ui.add_space(12.0);
                }

                // --- Options ---
                section_label(ui, regular::GEAR, "Options", text_primary);
                ui.add_space(4.0);

                let mut export_state = world.resource_mut::<ExportOverlayState>();

                ui.checkbox(
                    &mut export_state.console_logging,
                    egui::RichText::new("Console logging").color(text_primary),
                );

                // Icon
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Icon:").size(12.0).color(text_secondary));
                    if let Some(ref icon) = export_state.icon_path {
                        ui.label(egui::RichText::new(icon.as_str()).size(11.0).color(text_primary));
                        if ui.add(egui::Button::new(
                            egui::RichText::new(regular::X).size(12.0),
                        ).frame(false)).clicked() {
                            export_state.icon_path = None;
                        }
                    } else {
                        ui.label(egui::RichText::new("None").size(11.0).color(text_secondary));
                    }
                    let icon_path_mut = &mut export_state.icon_path;
                    if ui.add(egui::Button::new(
                        egui::RichText::new(format!("{} Browse", regular::IMAGE)).size(11.0),
                    ).fill(surface_mid)).clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Select icon")
                            .add_filter("Images", &["png", "ico", "svg"])
                            .pick_file()
                        {
                            *icon_path_mut = Some(path.to_string_lossy().to_string());
                        }
                    }
                });

                drop(export_state);
                ui.add_space(12.0);

                // --- Output ---
                section_label(ui, regular::FOLDER_OPEN, "Output", text_primary);
                ui.add_space(4.0);

                let mut export_state = world.resource_mut::<ExportOverlayState>();

                ui.horizontal(|ui| {
                    let text_edit = egui::TextEdit::singleline(&mut export_state.output_dir)
                        .hint_text("Export directory...")
                        .desired_width(ui.available_width() - 80.0);
                    ui.add(text_edit);

                    let output_dir_mut = &mut export_state.output_dir;
                    if ui.add(egui::Button::new(
                        egui::RichText::new(format!("{} Browse", regular::FOLDER)).size(11.0),
                    ).fill(surface_mid)).clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Select output directory")
                            .pick_folder()
                        {
                            *output_dir_mut = path.to_string_lossy().to_string();
                        }
                    }
                });

                let progress = export_state.progress.clone();
                let can_export = template_installed
                    && !export_state.output_dir.is_empty()
                    && matches!(progress, ExportProgress::Idle | ExportProgress::Done(_) | ExportProgress::Error(_));

                drop(export_state);

                ui.add_space(16.0);

                // --- Progress / status ---
                match &progress {
                    ExportProgress::Idle => {}
                    ExportProgress::Packing => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(egui::RichText::new("Packing assets...").color(text_secondary));
                        });
                    }
                    ExportProgress::Writing => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(egui::RichText::new("Writing output...").color(text_secondary));
                        });
                    }
                    ExportProgress::Done(msg) => {
                        ui.label(egui::RichText::new(format!("{} {}", regular::CHECK_CIRCLE, msg)).color(egui::Color32::from_rgb(89, 191, 115)));
                    }
                    ExportProgress::Error(msg) => {
                        ui.label(egui::RichText::new(format!("{} {}", regular::WARNING, msg)).color(error_color));
                    }
                }

                ui.add_space(8.0);

                // --- Export button ---
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let button = egui::Button::new(
                            egui::RichText::new(format!("{} Export", regular::ROCKET_LAUNCH))
                                .size(14.0)
                                .color(egui::Color32::WHITE),
                        )
                        .fill(if can_export { accent } else { surface_mid })
                        .min_size(egui::vec2(100.0, 32.0));

                        if ui.add_enabled(can_export, button).clicked() {
                            run_export(world, &project_name);
                        }
                    });
                });
            });
        });
}

/// Export for Android: copy the template APK and inject the rpak into its assets/ folder.
fn export_android_apk(
    template_path: &std::path::Path,
    output_dir: &std::path::Path,
    binary_name: &str,
    packer: RpakPacker,
    compression_level: i32,
) -> std::io::Result<()> {
    use std::io::{Read as _, Write as _};

    let rpak_bytes = packer.finish(compression_level)?;

    let apk_dest = output_dir.join(binary_name);

    // Read the template APK
    let template_data = std::fs::read(template_path)?;
    let cursor = std::io::Cursor::new(&template_data);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    // Create the output APK, copying all existing entries and adding the rpak
    let out_file = std::fs::File::create(&apk_dest)?;
    let mut writer = zip::ZipWriter::new(out_file);

    // Copy all existing entries from template
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let name = entry.name().to_string();

        // Android requires resources.arsc and native libs to be stored
        // uncompressed with 4-byte alignment (R+ / API 30+)
        let must_store = name == "resources.arsc"
            || name.ends_with(".so");

        let options = if must_store {
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored)
                .unix_permissions(entry.unix_mode().unwrap_or(0o644))
                .with_alignment(16384)
        } else {
            zip::write::SimpleFileOptions::default()
                .compression_method(entry.compression())
                .unix_permissions(entry.unix_mode().unwrap_or(0o644))
        };

        writer.start_file(name, options)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        writer.write_all(&buf)?;
    }

    // Add the rpak as assets/game.rpak
    let rpak_options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored);
    writer.start_file("assets/game.rpak", rpak_options)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    writer.write_all(&rpak_bytes)?;

    writer.finish()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    Ok(())
}

fn section_label(ui: &mut egui::Ui, icon: &str, label: &str, color: egui::Color32) {
    ui.label(
        egui::RichText::new(format!("{} {}", icon, label))
            .size(13.0)
            .color(color)
            .strong(),
    );
}

fn run_export(world: &mut World, project_name: &str) {
    world.resource_mut::<ExportOverlayState>().progress = ExportProgress::Packing;

    let project = world.resource::<CurrentProject>().clone();
    let export_state = world.resource::<ExportOverlayState>();
    let platform = export_state.platform;
    let packaging_mode = export_state.packaging_mode;
    let compression_level = export_state.compression_level;
    let output_dir = std::path::PathBuf::from(&export_state.output_dir);
    let window_mode = export_state.window_mode;
    let window_width = export_state.window_width;
    let window_height = export_state.window_height;
    let _console_logging = export_state.console_logging;
    let project_name = project_name.to_string();

    // Get template path
    let template_path = match world.resource::<TemplateManager>().get(platform) {
        Some(t) => t.path.clone(),
        None => {
            world.resource_mut::<ExportOverlayState>().progress =
                ExportProgress::Error("No template installed for this platform".to_string());
            return;
        }
    };

    // Pack assets
    let packer = match pack_directory(&project.path) {
        Ok(p) => p,
        Err(e) => {
            world.resource_mut::<ExportOverlayState>().progress =
                ExportProgress::Error(format!("Failed to pack assets: {}", e));
            return;
        }
    };

    let file_count = packer.len();

    world.resource_mut::<ExportOverlayState>().progress = ExportProgress::Writing;

    // Write export config into the rpak (override project.toml with export settings)
    let mut export_config = project.config.clone();
    export_config.window.width = window_width;
    export_config.window.height = window_height;
    export_config.window.fullscreen = matches!(window_mode, WindowMode::Fullscreen);
    export_config.window.resizable = matches!(window_mode, WindowMode::Windowed);

    // Create output directory
    if let Err(e) = std::fs::create_dir_all(&output_dir) {
        world.resource_mut::<ExportOverlayState>().progress =
            ExportProgress::Error(format!("Failed to create output dir: {}", e));
        return;
    }

    let binary_name = platform.binary_name(&project_name);
    let is_android = matches!(platform, Platform::AndroidArm64 | Platform::AndroidX86_64 | Platform::FireTVArm64);
    let result = if is_android {
        export_android_apk(&template_path, &output_dir, &binary_name, packer, compression_level)
            .and_then(|_| {
                let apk_path = output_dir.join(&binary_name);
                crate::apk_signer::sign_apk(&apk_path)
            })
    } else {
        match packaging_mode {
            PackagingMode::SeparateFiles => {
                let rpak_path = output_dir.join(format!("{}.rpak", project_name));
                let binary_dest = output_dir.join(&binary_name);

                packer
                    .write_to_file(&rpak_path, compression_level)
                    .and_then(|_| std::fs::copy(&template_path, &binary_dest).map(|_| ()))
                    .and_then(|_| {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            let perms = std::fs::Permissions::from_mode(0o755);
                            std::fs::set_permissions(&binary_dest, perms)?;
                        }
                        Ok(())
                    })
            }
            PackagingMode::SingleBinary => {
                let binary_dest = output_dir.join(&binary_name);
                packer.append_to_binary(&template_path, &binary_dest, compression_level)
                    .and_then(|_| {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            let perms = std::fs::Permissions::from_mode(0o755);
                            std::fs::set_permissions(&binary_dest, perms)?;
                        }
                        Ok(())
                    })
            }
        }
    };

    match result {
        Ok(()) => {
            world.resource_mut::<ExportOverlayState>().progress = ExportProgress::Done(
                format!("Exported {} files to {}", file_count, output_dir.display()),
            );
        }
        Err(e) => {
            world.resource_mut::<ExportOverlayState>().progress =
                ExportProgress::Error(format!("Export failed: {}", e));
        }
    }
}
