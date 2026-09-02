//! The Viewport page — grid, entity labels, performance, camera, and gizmos.
//!
//! Three sidebar categories share this page (`viewport`, `camera`, `gizmos`)
//! and its sections are keyed to them via `focus_hide`. Most rows write
//! [`ViewportSettings`], which is the *editor's* viewport and is stripped from
//! an export — the shipped game's equivalents live under Project → Rendering.

use bevy::prelude::*;

use renzora_editor_framework::{EditorSettings, SelectionGranularity};
use renzora_ember::font::EmberFonts;
use renzora_ember::inspector::color_field;
use renzora_ember::widgets::section;
use renzora_viewport::settings::{
    CollisionGizmoVisibility, EditorCameraSource, GraphicsQuality, LabelScope, ViewportSettings,
};

use crate::lang::{loc_opt, tr};
use crate::rows::{ctl_drag, ctl_dropdown, ctl_toggle, focus_hide, note_row, settings_row};
use crate::state::{A_GREEN, A_PURPLE, A_TEAL};

pub(crate) fn tab_viewport(
    commands: &mut Commands,
    fonts: &EmberFonts,
    col: Entity,
    vp: &ViewportSettings,
    focus: Option<&str>,
) {
    let (sec, body) = section(commands, fonts, "grid-four", &tr("settings.cat.grid"), A_GREEN);
    commands.entity(col).add_child(sec);
    focus_hide(commands, sec, focus, "viewport");
    let t = ctl_toggle(
        commands,
        vp.show_grid,
        |w| w.resource::<ViewportSettings>().show_grid,
        |w, &v| w.resource_mut::<ViewportSettings>().show_grid = v,
    );
    settings_row(commands, fonts, body, 0, &tr("settings.row.show_grid"), t);
    let t = ctl_toggle(
        commands,
        vp.show_subgrid,
        |w| w.resource::<ViewportSettings>().show_subgrid,
        |w, &v| w.resource_mut::<ViewportSettings>().show_subgrid = v,
    );
    settings_row(commands, fonts, body, 1, &tr("settings.row.show_subgrid"), t);
    let t = ctl_toggle(
        commands,
        vp.show_axis_gizmo,
        |w| w.resource::<ViewportSettings>().show_axis_gizmo,
        |w, &v| w.resource_mut::<ViewportSettings>().show_axis_gizmo = v,
    );
    settings_row(commands, fonts, body, 2, &tr("settings.row.axis_gizmo"), t);
    let cf = color_field(
        commands,
        |w| {
            let c = w.resource::<ViewportSettings>().grid_color_2d;
            [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0]
        },
        |w, rgb| {
            let mut vp = w.resource_mut::<ViewportSettings>();
            let a = vp.grid_color_2d[3];
            vp.grid_color_2d = [
                (rgb[0] * 255.0).round() as u8,
                (rgb[1] * 255.0).round() as u8,
                (rgb[2] * 255.0).round() as u8,
                a,
            ];
        },
    );
    settings_row(commands, fonts, body, 3, &tr("settings.row.grid_color_2d"), cf);
    // The 2D view's status-bar cursor-coordinate readout.
    let t = ctl_toggle(
        commands,
        vp.show_cursor_coords_2d,
        |w| w.resource::<ViewportSettings>().show_cursor_coords_2d,
        |w, &v| w.resource_mut::<ViewportSettings>().show_cursor_coords_2d = v,
    );
    settings_row(commands, fonts, body, 4, &tr("settings.row.cursor_coords_2d"), t);

    // Entity name labels (Bevy 0.19 stroke-font text gizmos).
    let (sec, body) = section(commands, fonts, "text-aa", &tr("settings.cat.labels"), A_GREEN);
    commands.entity(col).add_child(sec);
    focus_hide(commands, sec, focus, "viewport");
    let t = ctl_toggle(
        commands,
        vp.show_labels,
        |w| w.resource::<ViewportSettings>().show_labels,
        |w, &v| w.resource_mut::<ViewportSettings>().show_labels = v,
    );
    settings_row(commands, fonts, body, 0, &tr("settings.row.show_labels"), t);
    let scope_strs: Vec<String> = LabelScope::ALL.iter().map(|s| loc_opt(s.label())).collect();
    let scope_labels: Vec<&str> = scope_strs.iter().map(|s| s.as_str()).collect();
    let dd = ctl_dropdown(
        commands, fonts, &scope_labels,
        LabelScope::ALL
            .iter()
            .position(|s| *s == vp.label_scope)
            .unwrap_or(0),
        |w| {
            let cur = w.resource::<ViewportSettings>().label_scope;
            LabelScope::ALL.iter().position(|s| *s == cur).unwrap_or(0)
        },
        |w, &i| {
            let sc = LabelScope::ALL.get(i).copied().unwrap_or_default();
            w.resource_mut::<ViewportSettings>().label_scope = sc;
        },
    );
    settings_row(commands, fonts, body, 1, &tr("settings.row.show_on"), dd);
    let dv = ctl_drag(
        commands, fonts, vp.label_size, 0.2, 5.0, 0.05,
        |w| w.resource::<ViewportSettings>().label_size,
        |w, &v| w.resource_mut::<ViewportSettings>().label_size = v,
    );
    settings_row(commands, fonts, body, 2, &tr("settings.row.label_size"), dv);
    let cf = color_field(
        commands,
        |w| {
            let c = w.resource::<ViewportSettings>().label_color;
            [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0]
        },
        |w, rgb| {
            w.resource_mut::<ViewportSettings>().label_color = [
                (rgb[0] * 255.0).round() as u8,
                (rgb[1] * 255.0).round() as u8,
                (rgb[2] * 255.0).round() as u8,
            ];
        },
    );
    settings_row(commands, fonts, body, 3, &tr("settings.row.label_color"), cf);
    let dv = ctl_drag(
        commands, fonts, vp.label_max_distance, 1.0, 500.0, 1.0,
        |w| w.resource::<ViewportSettings>().label_max_distance,
        |w, &v| w.resource_mut::<ViewportSettings>().label_max_distance = v,
    );
    settings_row(commands, fonts, body, 4, &tr("settings.row.max_distance"), dv);

    let (sec, body) = section(commands, fonts, "gauge", &tr("settings.cat.performance"), A_TEAL);
    commands.entity(col).add_child(sec);
    focus_hide(commands, sec, focus, "viewport");
    // Graphics Quality — gates the expensive fullscreen passes (GI / auto-exposure
    // / bloom / TAA). The single biggest lever for FPS on weak / high-DPI GPUs.
    let q_strs: Vec<String> = GraphicsQuality::ALL.iter().map(|s| loc_opt(s.label())).collect();
    let q_labels: Vec<&str> = q_strs.iter().map(|s| s.as_str()).collect();
    let dd = ctl_dropdown(
        commands, fonts, &q_labels,
        GraphicsQuality::ALL
            .iter()
            .position(|s| *s == vp.graphics_quality)
            .unwrap_or(1),
        |w| {
            let cur = w.resource::<ViewportSettings>().graphics_quality;
            GraphicsQuality::ALL.iter().position(|s| *s == cur).unwrap_or(1)
        },
        |w, &i| {
            let qv = GraphicsQuality::ALL.get(i).copied().unwrap_or_default();
            w.resource_mut::<ViewportSettings>().graphics_quality = qv;
        },
    );
    settings_row(commands, fonts, body, 0, &tr("settings.row.graphics_quality"), dd);
    let t = ctl_toggle(
        commands,
        vp.vsync,
        |w| w.resource::<ViewportSettings>().vsync,
        |w, &v| w.resource_mut::<ViewportSettings>().vsync = v,
    );
    settings_row(commands, fonts, body, 1, &tr("settings.row.vsync"), t);

    let (sec, body) = section(commands, fonts, "video-camera", &tr("settings.category.camera"), A_PURPLE);
    commands.entity(col).add_child(sec);
    focus_hide(commands, sec, focus, "camera");
    let cam = &vp.camera;
    let dv = ctl_drag(
        commands, fonts, cam.move_speed, 1.0, 50.0, 0.5,
        |w| w.resource::<ViewportSettings>().camera.move_speed,
        |w, &v| w.resource_mut::<ViewportSettings>().camera.move_speed = v,
    );
    settings_row(commands, fonts, body, 0, &tr("settings.row.move_speed"), dv);
    let dv = ctl_drag(
        commands, fonts, cam.look_sensitivity, 0.05, 2.0, 0.01,
        |w| w.resource::<ViewportSettings>().camera.look_sensitivity,
        |w, &v| w.resource_mut::<ViewportSettings>().camera.look_sensitivity = v,
    );
    settings_row(commands, fonts, body, 1, &tr("settings.row.look_sensitivity"), dv);
    let dv = ctl_drag(
        commands, fonts, cam.orbit_sensitivity, 0.05, 2.0, 0.01,
        |w| w.resource::<ViewportSettings>().camera.orbit_sensitivity,
        |w, &v| w.resource_mut::<ViewportSettings>().camera.orbit_sensitivity = v,
    );
    settings_row(commands, fonts, body, 2, &tr("settings.row.orbit_sensitivity"), dv);
    let dv = ctl_drag(
        commands, fonts, cam.pan_sensitivity, 0.1, 5.0, 0.01,
        |w| w.resource::<ViewportSettings>().camera.pan_sensitivity,
        |w, &v| w.resource_mut::<ViewportSettings>().camera.pan_sensitivity = v,
    );
    settings_row(commands, fonts, body, 3, &tr("settings.row.pan_sensitivity"), dv);
    let dv = ctl_drag(
        commands, fonts, cam.zoom_sensitivity, 0.1, 5.0, 0.01,
        |w| w.resource::<ViewportSettings>().camera.zoom_sensitivity,
        |w, &v| w.resource_mut::<ViewportSettings>().camera.zoom_sensitivity = v,
    );
    settings_row(commands, fonts, body, 4, &tr("settings.row.zoom_sensitivity"), dv);
    let t = ctl_toggle(
        commands, cam.invert_y,
        |w| w.resource::<ViewportSettings>().camera.invert_y,
        |w, &v| w.resource_mut::<ViewportSettings>().camera.invert_y = v,
    );
    settings_row(commands, fonts, body, 5, &tr("settings.row.invert_y"), t);
    let t = ctl_toggle(
        commands, cam.distance_relative_speed,
        |w| w.resource::<ViewportSettings>().camera.distance_relative_speed,
        |w, &v| w.resource_mut::<ViewportSettings>().camera.distance_relative_speed = v,
    );
    settings_row(commands, fonts, body, 6, &tr("settings.row.distance_speed"), t);
    let src_strs: Vec<String> = EditorCameraSource::ALL.iter().map(|s| loc_opt(s.label())).collect();
    let src_labels: Vec<&str> = src_strs.iter().map(|s| s.as_str()).collect();
    let dd = ctl_dropdown(
        commands, fonts, &src_labels,
        EditorCameraSource::ALL
            .iter()
            .position(|s| *s == cam.editor_camera_source)
            .unwrap_or(0),
        |w| {
            let cur = w.resource::<ViewportSettings>().camera.editor_camera_source;
            EditorCameraSource::ALL.iter().position(|s| *s == cur).unwrap_or(0)
        },
        |w, &i| {
            let src = EditorCameraSource::ALL.get(i).copied().unwrap_or_default();
            w.resource_mut::<ViewportSettings>().camera.editor_camera_source = src;
        },
    );
    settings_row(commands, fonts, body, 7, &tr("settings.row.editor_camera"), dd);
    // Editor-level (not per-viewport) play behaviour, but it's about the viewport,
    // so it lives here rather than under Scripting.
    let t = ctl_toggle(
        commands, true,
        |w| w.resource::<EditorSettings>().maximize_viewport_on_play,
        |w, &v| w.resource_mut::<EditorSettings>().maximize_viewport_on_play = v,
    );
    settings_row(commands, fonts, body, 8, &tr("settings.row.maximize_on_play"), t);

    let (sec, body) = section(commands, fonts, "gauge", &tr("settings.cat.gizmos"), A_TEAL);
    commands.entity(col).add_child(sec);
    focus_hide(commands, sec, focus, "gizmos");
    // Three states, in enum order — `Off` is the same one the viewport toolbar's
    // Gizmos dropdown reaches with its Colliders switch.
    let coll_opts = [
        tr("common.off"),
        tr("settings.opt.selected_only"),
        tr("common.always"),
    ];
    let coll_refs: Vec<&str> = coll_opts.iter().map(|s| s.as_str()).collect();
    let coll_index = |v: CollisionGizmoVisibility| match v {
        CollisionGizmoVisibility::Off => 0,
        CollisionGizmoVisibility::SelectedOnly => 1,
        CollisionGizmoVisibility::Always => 2,
    };
    let dd = ctl_dropdown(
        commands, fonts, &coll_refs,
        coll_index(vp.collision_gizmo_visibility),
        move |w| coll_index(w.resource::<ViewportSettings>().collision_gizmo_visibility),
        |w, &i| {
            w.resource_mut::<ViewportSettings>().collision_gizmo_visibility = match i {
                0 => CollisionGizmoVisibility::Off,
                2 => CollisionGizmoVisibility::Always,
                _ => CollisionGizmoVisibility::SelectedOnly,
            };
        },
    );
    settings_row(commands, fonts, body, 0, &tr("settings.row.colliders"), dd);
    // The per-gizmo switches — the same four the viewport toolbar's Gizmos
    // dropdown carries, mirrored here so Settings stays a complete view of
    // what the editor draws.
    let t = ctl_toggle(
        commands, vp.show_selection_box,
        |w| w.resource::<ViewportSettings>().show_selection_box,
        |w, &v| w.resource_mut::<ViewportSettings>().show_selection_box = v,
    );
    settings_row(commands, fonts, body, 1, &tr("viewport.gizmos.bounding_box"), t);
    let t = ctl_toggle(
        commands, vp.show_skeleton_gizmos,
        |w| w.resource::<ViewportSettings>().show_skeleton_gizmos,
        |w, &v| w.resource_mut::<ViewportSettings>().show_skeleton_gizmos = v,
    );
    settings_row(commands, fonts, body, 2, &tr("viewport.gizmos.skeleton"), t);
    let t = ctl_toggle(
        commands, vp.show_light_gizmos,
        |w| w.resource::<ViewportSettings>().show_light_gizmos,
        |w, &v| w.resource_mut::<ViewportSettings>().show_light_gizmos = v,
    );
    settings_row(commands, fonts, body, 3, &tr("viewport.gizmos.lights"), t);
    let t = ctl_toggle(
        commands, vp.show_camera_gizmos,
        |w| w.resource::<ViewportSettings>().show_camera_gizmos,
        |w, &v| w.resource_mut::<ViewportSettings>().show_camera_gizmos = v,
    );
    settings_row(commands, fonts, body, 4, &tr("viewport.gizmos.cameras"), t);
    // (The "Selection highlight" row was removed with `bevy_mod_outline` — the
    // wireframe bounding box is now the only highlight, so there is nothing to
    // choose. `settings.row.boundary_on_top` below still applies to it.)
    let gran_strs: Vec<String> = SelectionGranularity::ALL.iter().map(|g| loc_opt(g.label())).collect();
    let gran_labels: Vec<&str> = gran_strs.iter().map(|g| g.as_str()).collect();
    let dd = ctl_dropdown(
        commands, fonts, &gran_labels,
        // Seed with the default; reseeded from the resource by bind_2way.
        SelectionGranularity::ALL
            .iter()
            .position(|g| *g == SelectionGranularity::default())
            .unwrap_or(0),
        |w| {
            let cur = w.resource::<EditorSettings>().selection_granularity;
            SelectionGranularity::ALL.iter().position(|g| *g == cur).unwrap_or(0)
        },
        |w, &i| {
            let g = SelectionGranularity::ALL.get(i).copied().unwrap_or_default();
            w.resource_mut::<EditorSettings>().selection_granularity = g;
        },
    );
    settings_row(commands, fonts, body, 5, &tr("settings.row.click_selects"), dd);
    let boundary_opts = [tr("settings.opt.on_top"), tr("settings.opt.depth_tested")];
    let boundary_refs: Vec<&str> = boundary_opts.iter().map(|s| s.as_str()).collect();
    let dd = ctl_dropdown(
        commands, fonts, &boundary_refs, 0,
        |w| {
            if w.resource::<EditorSettings>().selection_boundary_on_top {
                0
            } else {
                1
            }
        },
        |w, &i| w.resource_mut::<EditorSettings>().selection_boundary_on_top = i == 0,
    );
    settings_row(commands, fonts, body, 6, &tr("settings.row.boundary"), dd);
    let dv = ctl_drag(
        commands, fonts, vp.gizmo_drag_opacity, 0.0, 1.0, 0.05,
        |w| w.resource::<ViewportSettings>().gizmo_drag_opacity,
        |w, &v| w.resource_mut::<ViewportSettings>().gizmo_drag_opacity = v.clamp(0.0, 1.0),
    );
    settings_row(commands, fonts, body, 7, &tr("settings.row.drag_opacity"), dv);
    note_row(commands, fonts, body, &tr("settings.hint.drag_opacity"));
    // Show the transform gizmo + selection outline in every viewport at once
    // (default: only the viewport the cursor is in).
    let t = ctl_toggle(
        commands,
        vp.gizmos_all_viewports,
        |w| w.resource::<ViewportSettings>().gizmos_all_viewports,
        |w, &v| w.resource_mut::<ViewportSettings>().gizmos_all_viewports = v,
    );
    settings_row(commands, fonts, body, 8, &tr("settings.row.gizmos_all_viewports"), t);
    note_row(commands, fonts, body, &tr("settings.hint.gizmos_all_viewports"));

    // Anchor the transform gizmo on the base of the selection rather than the
    // middle of its bounding box.
    let t = ctl_toggle(
        commands,
        vp.gizmo_pivot_bottom,
        |w| w.resource::<ViewportSettings>().gizmo_pivot_bottom,
        |w, &v| w.resource_mut::<ViewportSettings>().gizmo_pivot_bottom = v,
    );
    settings_row(commands, fonts, body, 8, &tr("settings.row.gizmo_pivot_bottom"), t);
    note_row(commands, fonts, body, &tr("settings.hint.gizmo_pivot_bottom"));
}
