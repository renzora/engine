//! Camera debug panel

use bevy::prelude::{Entity, World};
use bevy_egui::egui::{self, Color32, CursorIcon, RichText};
use renzora_theme::Theme;

use crate::state::{CameraDebugState, CameraProjectionType};

/// Width above which we switch to a two-column layout (list on the
/// left, selected-camera details on the right). Below this, stays
/// single column with details under the list — useful for narrow
/// side-docked layouts.
const TWO_COLUMN_BREAKPOINT: f32 = 560.0;

pub fn render_camera_debug_content(
    ui: &mut egui::Ui,
    state: &mut CameraDebugState,
    theme: &Theme,
    world: &World,
) {
    egui::Frame::NONE
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let two_col = ui.available_width() >= TWO_COLUMN_BREAKPOINT
                && state.selected_camera.is_some();

            if two_col {
                egui::ScrollArea::vertical()
                    .id_salt("camera_debug_root_scroll_two_col")
                    .show(ui, |ui| {
                        render_camera_count_header(ui, state, theme);
                        ui.add_space(12.0);

                        let total = ui.available_width();
                        let gap = 12.0;
                        let left_w = (total - gap) * 0.45;
                        let right_w = total - gap - left_w;

                        ui.horizontal_top(|ui| {
                            ui.allocate_ui_with_layout(
                                egui::vec2(left_w, ui.available_height()),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    ui.set_width(left_w);
                                    render_camera_list(ui, state, theme);
                                    ui.add_space(12.0);
                                    render_gizmo_toggles(ui, state, theme);
                                },
                            );
                            ui.add_space(gap);
                            ui.allocate_ui_with_layout(
                                egui::vec2(right_w, ui.available_height()),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    ui.set_width(right_w);
                                    render_selected_camera_details(
                                        ui, state, theme, world,
                                    );
                                },
                            );
                        });
                    });
            } else {
                egui::ScrollArea::vertical()
                    .id_salt("camera_debug_root_scroll_single")
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());

                        render_camera_count_header(ui, state, theme);
                        ui.add_space(12.0);
                        render_camera_list(ui, state, theme);
                        ui.add_space(16.0);

                        if state.selected_camera.is_some() {
                            render_selected_camera_details(ui, state, theme, world);
                            ui.add_space(16.0);
                        }

                        render_gizmo_toggles(ui, state, theme);
                    });
            }
        });
}

fn render_camera_count_header(ui: &mut egui::Ui, state: &CameraDebugState, theme: &Theme) {
    let count = state.scene_camera_count();
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{}", count))
                .size(28.0)
                .color(theme.text.primary.to_color32())
                .strong(),
        );
        ui.label(
            RichText::new("cameras")
                .size(12.0)
                .color(theme.text.muted.to_color32()),
        );
    });
}

fn render_camera_list(ui: &mut egui::Ui, state: &mut CameraDebugState, theme: &Theme) {
    ui.label(
        RichText::new("Cameras")
            .size(12.0)
            .color(theme.text.muted.to_color32()),
    );
    ui.add_space(4.0);

    if state.cameras.is_empty() {
        ui.label(
            RichText::new("No cameras in scene")
                .size(11.0)
                .color(theme.text.muted.to_color32()),
        );
        return;
    }

    egui::Frame::NONE
        .fill(theme.surfaces.faint.to_color32())
        .corner_radius(4.0)
        .inner_margin(egui::Margin::same(4))
        .show(ui, |ui| {
            // Collect entity decisions in a separate pass so we don't
            // hold `&state.cameras` while pushing to `state.pending_toggles`.
            let mut clicked_select: Option<Entity> = None;
            let mut clicked_toggle: Option<Entity> = None;

            for camera in &state.cameras {
                let is_selected = state.selected_camera == Some(camera.entity);
                let bg_color = if is_selected {
                    Color32::from_rgb(60, 80, 120)
                } else {
                    Color32::TRANSPARENT
                };

                egui::Frame::NONE
                    .fill(bg_color)
                    .corner_radius(2.0)
                    .inner_margin(egui::Margin::symmetric(6, 4))
                    .show(ui, |ui| {
                        let response = ui.horizontal(|ui| {
                            let status_color = if camera.is_active {
                                Color32::from_rgb(100, 200, 100)
                            } else {
                                Color32::from_rgb(120, 120, 130)
                            };
                            ui.label(RichText::new("\u{25cf}").size(8.0).color(status_color));
                            ui.label(
                                RichText::new(&camera.name)
                                    .size(11.0)
                                    .color(theme.text.primary.to_color32()),
                            );

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    // Toggle pill — flips `Camera::is_active`
                                    // on click via the debug bridge. Sized
                                    // small so it fits the existing row.
                                    let (label, fg, bg) = if camera.is_active {
                                        ("ON", Color32::from_rgb(20, 40, 20), Color32::from_rgb(120, 210, 120))
                                    } else {
                                        ("OFF", Color32::from_rgb(220, 220, 220), Color32::from_rgb(70, 70, 78))
                                    };
                                    let btn = egui::Button::new(
                                        RichText::new(label).size(9.0).color(fg).monospace().strong(),
                                    )
                                    .fill(bg)
                                    .corner_radius(2.0)
                                    .min_size(egui::vec2(28.0, 14.0));
                                    let resp = ui.add(btn);
                                    if resp.hovered() {
                                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                                    }
                                    if resp.clicked() {
                                        clicked_toggle = Some(camera.entity);
                                    }

                                    let proj_text = match camera.projection_type {
                                        CameraProjectionType::Perspective => "P",
                                        CameraProjectionType::Orthographic => "O",
                                    };
                                    ui.label(
                                        RichText::new(proj_text)
                                            .size(9.0)
                                            .color(theme.text.muted.to_color32())
                                            .monospace(),
                                    );
                                    ui.label(
                                        RichText::new(format!("#{}", camera.order))
                                            .size(9.0)
                                            .color(theme.text.muted.to_color32()),
                                    );
                                },
                            );
                        });

                        let interact = response.response.interact(egui::Sense::click());
                        if interact.hovered() {
                            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        }
                        if interact.clicked() {
                            clicked_select = Some(camera.entity);
                        }
                    });
            }

            if let Some(entity) = clicked_select {
                state.selected_camera = Some(entity);
            }
            if let Some(entity) = clicked_toggle {
                state.pending_toggles.push(entity);
            }
        });
}

fn render_selected_camera_details(
    ui: &mut egui::Ui,
    state: &CameraDebugState,
    theme: &Theme,
    world: &World,
) {
    let Some(camera) = state.selected_camera_info() else {
        return;
    };

    ui.label(
        RichText::new("Selected Camera")
            .size(12.0)
            .color(theme.text.muted.to_color32()),
    );
    ui.add_space(4.0);

    egui::Frame::NONE
        .fill(theme.surfaces.faint.to_color32())
        .corner_radius(4.0)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(&camera.name)
                        .size(15.0)
                        .color(theme.text.primary.to_color32())
                        .strong(),
                );
                ui.label(
                    RichText::new(format!("({:?})", camera.entity))
                        .size(11.0)
                        .color(theme.text.muted.to_color32())
                        .monospace(),
                );
                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        // Copy-to-clipboard: dumps everything visible
                        // for this camera (props + transform + full
                        // component list + values) as plain text so
                        // diagnostic info is one click away from a
                        // bug report or commit message.
                        let copy_btn = ui.add(
                            egui::Button::new(
                                RichText::new("Copy")
                                    .size(11.0)
                                    .monospace(),
                            )
                            .min_size(egui::vec2(40.0, 18.0)),
                        );
                        if copy_btn.hovered() {
                            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                        }
                        if copy_btn.clicked() {
                            let dump = format_camera_dump(camera, world);
                            ui.ctx().copy_text(dump);
                        }
                    },
                );
            });

            ui.separator();

            // ── Component values ──────────────────────────────
            // Render this first because it's where the diagnostic
            // gold is — e.g. "VoxelCacheView.inject_active = false"
            // immediately explains why SDF GI isn't appearing even
            // though the component is attached.
            let values = collect_component_values(world, camera.entity);
            if !values.is_empty() {
                ui.label(
                    RichText::new("Component values")
                        .size(12.0)
                        .color(theme.text.secondary.to_color32()),
                );
                ui.add_space(4.0);
                egui::Grid::new("camera_component_values_grid")
                    .num_columns(2)
                    .spacing([12.0, 3.0])
                    .show(ui, |ui| {
                        for (key, value) in &values {
                            ui.label(
                                RichText::new(key)
                                    .size(11.0)
                                    .color(theme.text.muted.to_color32())
                                    .monospace(),
                            );
                            ui.label(
                                RichText::new(value)
                                    .size(11.0)
                                    .color(theme.text.primary.to_color32())
                                    .monospace(),
                            );
                            ui.end_row();
                        }
                    });
                ui.add_space(10.0);
            }

            ui.label(
                RichText::new("Projection")
                    .size(12.0)
                    .color(theme.text.secondary.to_color32()),
            );
            egui::Grid::new("camera_projection_grid")
                .num_columns(2)
                .spacing([12.0, 2.0])
                .show(ui, |ui| {
                    grid_row(ui, "Type", &format!("{}", camera.projection_type), theme);
                    if let Some(fov) = camera.fov_degrees {
                        grid_row(ui, "FOV", &format!("{:.1}\u{00b0}", fov), theme);
                    }
                    if let Some(scale) = camera.ortho_scale {
                        grid_row(ui, "Scale", &format!("{:.2}", scale), theme);
                    }
                    grid_row(ui, "Near", &format!("{:.3}", camera.near), theme);
                    grid_row(ui, "Far", &format!("{:.1}", camera.far), theme);
                    grid_row(ui, "Aspect", &format!("{:.2}", camera.aspect_ratio), theme);
                });

            ui.add_space(8.0);
            ui.label(
                RichText::new("Transform")
                    .size(12.0)
                    .color(theme.text.secondary.to_color32()),
            );
            egui::Grid::new("camera_transform_grid")
                .num_columns(2)
                .spacing([12.0, 2.0])
                .show(ui, |ui| {
                    grid_row(ui, "Position", &format_vec3(camera.position), theme);
                    grid_row(ui, "Rotation", &format_vec3(camera.rotation_degrees), theme);
                    grid_row(ui, "Forward", &format_vec3(camera.forward), theme);
                });

            if let Some(color) = camera.clear_color {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Clear:")
                            .size(10.0)
                            .color(theme.text.secondary.to_color32()),
                    );
                    let rgba = color.to_srgba();
                    let preview = Color32::from_rgba_unmultiplied(
                        (rgba.red * 255.0) as u8,
                        (rgba.green * 255.0) as u8,
                        (rgba.blue * 255.0) as u8,
                        (rgba.alpha * 255.0) as u8,
                    );
                    let (rect, _) =
                        ui.allocate_exact_size(egui::Vec2::splat(12.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 2.0, preview);
                    ui.painter().rect_stroke(
                        rect,
                        2.0,
                        egui::Stroke::new(1.0, Color32::from_gray(80)),
                        egui::StrokeKind::Inside,
                    );
                });
            }

            if let Some(viewport) = camera.viewport {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Viewport:")
                            .size(10.0)
                            .color(theme.text.secondary.to_color32()),
                    );
                    ui.label(
                        RichText::new(format!(
                            "[{:.0}, {:.0}, {:.0}x{:.0}]",
                            viewport[0], viewport[1], viewport[2], viewport[3]
                        ))
                        .size(9.0)
                        .color(theme.text.muted.to_color32())
                        .monospace(),
                    );
                });
            }

            // ── Live component list ──────────────────────────────
            // Enumerate everything attached to the selected camera
            // straight from its archetype. Live every frame; if a
            // routing/cleanup system adds or removes a component,
            // it shows up here within one frame. Built for the
            // common diagnostic question "which camera actually
            // has the LumenLighting / SsrSettings / etc. right now?"
            ui.add_space(12.0);
            ui.label(
                RichText::new("Components")
                    .size(12.0)
                    .color(theme.text.secondary.to_color32()),
            );
            ui.add_space(2.0);
            render_component_list(ui, theme, world, camera.entity);
        });
}

/// Pull the full component name list off the entity's archetype,
/// shorten each to its last `::` segment, group by inferred category,
/// and render. Live source — no caching, recomputed each frame.
fn render_component_list(ui: &mut egui::Ui, theme: &Theme, world: &World, entity: Entity) {
    let Ok(entity_ref) = world.get_entity(entity) else {
        ui.label(
            RichText::new("(entity not found)")
                .size(10.0)
                .color(theme.text.muted.to_color32())
                .italics(),
        );
        return;
    };

    let archetype = entity_ref.archetype();
    let mut names: Vec<String> = archetype
        .components()
        .iter()
        .filter_map(|id| world.components().get_info(*id))
        .map(|info| info.name().to_string())
        .collect();
    names.sort();

    // Categorise by substring match on the full type path. Anything
    // unrecognised lands in "Other".
    let mut prepass = Vec::new();
    let mut lighting = Vec::new();
    let mut post_process = Vec::new();
    let mut transform = Vec::new();
    let mut camera_core = Vec::new();
    let mut markers = Vec::new();
    let mut other = Vec::new();

    for full_name in &names {
        let short = full_name
            .rsplit("::")
            .next()
            .unwrap_or(full_name.as_str())
            .to_string();
        let bucket = if full_name.contains("Prepass") {
            &mut prepass
        } else if full_name.contains("LumenLighting")
            || full_name.contains("RtLighting")
            || full_name.contains("VoxelCacheView")
            || full_name.contains("LumenSkyCubemap")
            || full_name.contains("EnvironmentMap")
            || full_name.contains("Atmosphere")
            || full_name.contains("Skybox")
            || full_name.contains("NightStars")
            || full_name.contains("Clouds")
            || full_name.contains("DirectionalLight")
            || full_name.contains("PointLight")
            || full_name.contains("SpotLight")
        {
            &mut lighting
        } else if full_name.contains("Bloom")
            || full_name.contains("Ssao")
            || full_name.contains("Ssr")
            || full_name.contains("MotionBlur")
            || full_name.contains("DepthOfField")
            || full_name.contains("Taa")
            || full_name.contains("Cas")
            || full_name.contains("Tonemapping")
            || full_name.contains("DebandDither")
            || full_name.contains("AutoExposure")
            || full_name.contains("VolumetricFog")
            || full_name.contains("DistanceFog")
            || full_name.contains("ScreenReflection")
            || full_name.contains("LumenTrace")
        {
            &mut post_process
        } else if full_name.contains("Transform")
            || full_name.contains("Visibility")
            || full_name.contains("ChildOf")
            || full_name.contains("Children")
            || full_name.contains("VisibleEntities")
        {
            &mut transform
        } else if full_name.ends_with("Camera")
            || full_name.ends_with("Camera2d")
            || full_name.ends_with("Camera3d")
            || full_name.contains("RenderTarget")
            || full_name.contains("Projection")
            || full_name.contains("Frustum")
            || full_name.contains("Hdr")
            || full_name.contains("Msaa")
            || full_name.contains("ClusterConfig")
            || full_name.contains("OrbitCameraState")
            || full_name.contains("ViewUniform")
        {
            &mut camera_core
        } else if full_name.contains("EditorCamera")
            || full_name.contains("SceneCamera")
            || full_name.contains("DefaultCamera")
            || full_name.contains("PlayModeCamera")
            || full_name.contains("HideInHierarchy")
            || full_name.contains("EditorLocked")
            || full_name.contains("IsolatedCamera")
            || full_name.contains("Name")
        {
            &mut markers
        } else {
            &mut other
        };
        bucket.push(short);
    }

    render_component_category(ui, theme, "Markers", &markers, Color32::from_rgb(180, 160, 220));
    render_component_category(
        ui,
        theme,
        "Camera core",
        &camera_core,
        Color32::from_rgb(140, 180, 220),
    );
    render_component_category(ui, theme, "Prepass", &prepass, Color32::from_rgb(220, 180, 140));
    render_component_category(ui, theme, "Lighting / GI", &lighting, Color32::from_rgb(220, 220, 140));
    render_component_category(
        ui,
        theme,
        "Post-process",
        &post_process,
        Color32::from_rgb(140, 220, 180),
    );
    render_component_category(
        ui,
        theme,
        "Transform / hierarchy",
        &transform,
        Color32::from_rgb(180, 180, 180),
    );
    render_component_category(ui, theme, "Other", &other, Color32::from_rgb(160, 160, 160));
}

fn render_component_category(
    ui: &mut egui::Ui,
    theme: &Theme,
    title: &str,
    names: &[String],
    title_color: Color32,
) {
    if names.is_empty() {
        return;
    }
    ui.add_space(6.0);
    ui.label(
        RichText::new(format!("{title}  ({})", names.len()))
            .size(11.0)
            .color(title_color)
            .strong(),
    );
    for name in names {
        ui.label(
            RichText::new(format!("  {name}"))
                .size(11.0)
                .color(theme.text.primary.to_color32())
                .monospace(),
        );
    }
}

/// Look up known component types on `entity` and format their fields as
/// `name: value` lines. The whitelist is small on purpose — we want
/// diagnostic gold for the GI/render path (LumenLighting quality,
/// VoxelCacheView.inject_active, Camera.is_active/order/target,
/// EnvironmentMapLight.intensity) without becoming a general inspector.
/// Unknown component types still show as plain names in the categorised
/// list above this section.
fn collect_component_values(world: &World, entity: Entity) -> Vec<(String, String)> {
    let mut out = Vec::new();

    if let Some(cam) = world.get::<bevy::camera::Camera>(entity) {
        out.push((
            "Camera.is_active".into(),
            cam.is_active.to_string(),
        ));
        out.push(("Camera.order".into(), cam.order.to_string()));
    }

    if let Some(rt) = world.get::<bevy::camera::RenderTarget>(entity) {
        out.push((
            "RenderTarget".into(),
            match rt {
                bevy::camera::RenderTarget::Window(w) => format!("Window({:?})", w),
                bevy::camera::RenderTarget::Image(_) => "Image(<offscreen>)".into(),
                bevy::camera::RenderTarget::TextureView(_) => "TextureView".into(),
                _ => "<other>".into(),
            },
        ));
    }

    if let Some(lumen) = world.get::<renzora_lumen::LumenLighting>(entity) {
        out.push((
            "LumenLighting.quality".into(),
            format!("{:?}", lumen.quality),
        ));
        out.push((
            "LumenLighting.intensity".into(),
            format!("{:.3}", lumen.intensity),
        ));
        out.push((
            "LumenLighting.specular_intensity".into(),
            format!("{:.3}", lumen.specular_intensity),
        ));
        out.push((
            "LumenLighting.debug".into(),
            format!("{:?}", lumen.debug),
        ));
    }

    if let Some(rt) = world.get::<renzora_rt::RtLighting>(entity) {
        out.push(("RtLighting.enabled".into(), rt.enabled.to_string()));
        out.push((
            "RtLighting.intensity".into(),
            format!("{:.3}", rt.intensity),
        ));
        out.push(("RtLighting.debug".into(), format!("{:?}", rt.debug)));
    }

    if let Some(em) = world.get::<bevy::light::EnvironmentMapLight>(entity) {
        out.push((
            "EnvironmentMapLight.intensity".into(),
            format!("{:.3}", em.intensity),
        ));
        out.push((
            "EnvironmentMapLight.affects_lightmapped_mesh_diffuse".into(),
            em.affects_lightmapped_mesh_diffuse.to_string(),
        ));
    }

    if let Some(msaa) = world.get::<bevy::render::view::Msaa>(entity) {
        out.push(("Msaa".into(), format!("{:?}", msaa)));
    }

    if let Some(view) = world.get::<renzora_lumen::VoxelCacheView>(entity) {
        out.push((
            "VoxelCacheView.inject_active".into(),
            view.inject_active.to_string(),
        ));
        out.push((
            "VoxelCacheView.debug_active".into(),
            view.debug_active.to_string(),
        ));
    }

    out
}

fn render_gizmo_toggles(ui: &mut egui::Ui, state: &mut CameraDebugState, theme: &Theme) {
    ui.label(
        RichText::new("Debug Visualization")
            .size(12.0)
            .color(theme.text.muted.to_color32()),
    );
    ui.add_space(4.0);

    egui::Frame::NONE
        .fill(theme.surfaces.faint.to_color32())
        .corner_radius(4.0)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.checkbox(&mut state.show_frustum_gizmos, "Show Frustum (selected)");
            ui.checkbox(&mut state.show_camera_axes, "Show Camera Axes");
            ui.checkbox(&mut state.show_all_frustums, "Show All Frustums");

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Frustum Color:")
                        .size(10.0)
                        .color(theme.text.secondary.to_color32()),
                );
                let rgba = state.frustum_color.to_srgba();
                let mut color = [rgba.red, rgba.green, rgba.blue, rgba.alpha];
                if ui.color_edit_button_rgba_unmultiplied(&mut color).changed() {
                    state.frustum_color =
                        bevy::prelude::Color::srgba(color[0], color[1], color[2], color[3]);
                }
            });
        });
}

fn grid_row(ui: &mut egui::Ui, label: &str, value: &str, theme: &Theme) {
    ui.label(
        RichText::new(label)
            .size(11.0)
            .color(theme.text.muted.to_color32()),
    );
    ui.label(
        RichText::new(value)
            .size(11.0)
            .color(theme.text.primary.to_color32())
            .monospace(),
    );
    ui.end_row();
}

fn format_vec3(v: bevy::prelude::Vec3) -> String {
    format!("({:.2}, {:.2}, {:.2})", v.x, v.y, v.z)
}

/// Plain-text dump of everything visible for the selected camera, for
/// the "Copy" button. Format is line-oriented so it pastes cleanly
/// into commit messages or bug reports.
fn format_camera_dump(
    camera: &crate::state::CameraInfo,
    world: &World,
) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let _ = writeln!(s, "Camera: {} ({:?})", camera.name, camera.entity);
    let _ = writeln!(s, "  is_active: {}", camera.is_active);
    let _ = writeln!(s, "  order: {}", camera.order);
    let _ = writeln!(
        s,
        "  projection: {} (near {:.3}, far {:.1})",
        camera.projection_type, camera.near, camera.far
    );
    if let Some(fov) = camera.fov_degrees {
        let _ = writeln!(s, "  fov: {:.1}°", fov);
    }
    let _ = writeln!(s, "  position: {}", format_vec3(camera.position));
    let _ = writeln!(s, "  rotation: {}", format_vec3(camera.rotation_degrees));
    let _ = writeln!(s, "  forward: {}", format_vec3(camera.forward));

    // Component values block
    let values = collect_component_values(world, camera.entity);
    if !values.is_empty() {
        let _ = writeln!(s, "\nComponent values:");
        for (k, v) in &values {
            let _ = writeln!(s, "  {k} = {v}");
        }
    }

    // Full component list (just names)
    if let Ok(entity_ref) = world.get_entity(camera.entity) {
        let mut names: Vec<String> = entity_ref
            .archetype()
            .components()
            .iter()
            .filter_map(|id| world.components().get_info(*id))
            .map(|info| {
                let full = info.name().to_string();
                full.rsplit("::")
                    .next()
                    .unwrap_or(full.as_str())
                    .to_string()
            })
            .collect();
        names.sort();
        let _ = writeln!(s, "\nComponents ({}):", names.len());
        for n in names {
            let _ = writeln!(s, "  {n}");
        }
    }
    s
}
