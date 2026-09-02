//! The Project page — everything stored in `project.toml`.
//!
//! Every control here writes `CurrentProject.config` and immediately calls
//! [`save_project`], so `project.toml` is always current; there is no apply
//! button. Three sidebar categories share this page (`project`, `window`,
//! `rendering`) and its sections are keyed to them via `focus_hide`.

use bevy::prelude::*;

use renzora::{AspectMode, CurrentProject, RenderingMode, StretchMode, TextureFilter, WindowMode};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::theme::*;
use renzora_ember::widgets::{bind_text_input, section, text_input};
use renzora_viewport::settings::GraphicsQuality;

use crate::lang::tr;
use crate::rows::{ctl_drag, ctl_dropdown, ctl_toggle, focus_hide, note_row, settings_row};
use crate::state::{A_BLUE, A_PURPLE};

fn save_project(w: &mut World) {
    if let Some(cp) = w.get_resource::<CurrentProject>() {
        let _ = cp.save_config();
    }
}

pub(crate) fn tab_project(
    commands: &mut Commands,
    fonts: &EmberFonts,
    col: Entity,
    scenes: &[String],
    custom: &[String],
    has_project: bool,
    focus: Option<&str>,
) {
    if !has_project {
        let lbl = commands
            .spawn((
                Text::new(tr("settings.hint.no_project")),
                ui_font(&fonts.ui, 12.0),
                TextColor(rgb(text_muted())),
                Node {
                    margin: UiRect::all(Val::Px(12.0)),
                    ..default()
                },
            ))
            .id();
        commands.entity(col).add_child(lbl);
        return;
    }

    let (sec, body) = section(commands, fonts, "folder-open", &tr("common.project"), A_BLUE);
    commands.entity(col).add_child(sec);
    focus_hide(commands, sec, focus, "project");
    let ti = text_input(commands, &fonts.ui, &tr("settings.input.project_name_placeholder"), "");
    bind_text_input(
        commands,
        ti,
        |w| {
            w.get_resource::<CurrentProject>()
                .map(|c| c.config.name.clone())
                .unwrap_or_default()
        },
        |w, s| {
            if let Some(mut cp) = w.get_resource_mut::<CurrentProject>() {
                cp.config.name = s;
            }
            save_project(w);
        },
    );
    settings_row(commands, fonts, body, 0, &tr("common.name"), ti);

    let scene_opts: Vec<&str> = scenes.iter().map(|s| s.as_str()).collect();
    let sc1 = scenes.to_vec();
    let sc2 = scenes.to_vec();
    let dd = ctl_dropdown(
        commands,
        fonts,
        &scene_opts,
        0,
        move |w| {
            let cur = w
                .get_resource::<CurrentProject>()
                .map(|c| c.config.main_scene.clone())
                .unwrap_or_default();
            sc1.iter().position(|n| *n == cur).unwrap_or(0)
        },
        move |w, &i| {
            if let Some(name) = sc2.get(i).cloned() {
                if let Some(mut cp) = w.get_resource_mut::<CurrentProject>() {
                    cp.config.main_scene = name;
                }
                save_project(w);
            }
        },
    );
    settings_row(commands, fonts, body, 1, &tr("settings.row.boot_scene"), dd);

    // Default UI font for the shipped game (ProjectConfig.ui_font). "Default"
    // keeps the embedded font; other entries are generics + project fonts.
    let mut font_opts: Vec<String> = vec![
        tr("common.default"),
        tr("settings.opt.system_ui"),
        tr("settings.opt.sans_serif"),
        tr("settings.opt.serif"),
        tr("settings.opt.monospace"),
    ];
    font_opts.extend(custom.iter().cloned());
    let font_refs: Vec<&str> = font_opts.iter().map(|s| s.as_str()).collect();
    let fo1 = font_opts.clone();
    let fo2 = font_opts.clone();
    let dd = ctl_dropdown(
        commands,
        fonts,
        &font_refs,
        0,
        move |w| match w
            .get_resource::<CurrentProject>()
            .and_then(|c| c.config.ui_font.clone())
        {
            Some(name) => fo1.iter().position(|n| *n == name).unwrap_or(0),
            None => 0,
        },
        move |w, &i| {
            // Index 0 = "Default" → None (embedded font).
            let val = if i == 0 { None } else { fo2.get(i).cloned() };
            if let Some(mut cp) = w.get_resource_mut::<CurrentProject>() {
                cp.config.ui_font = val;
            }
            save_project(w);
        },
    );
    settings_row(commands, fonts, body, 2, &tr("settings.row.game_ui_font"), dd);

    // Global scenes — the `autoload` list. Each scene toggled on here loads
    // before the boot scene and every entity it spawns is tagged `Persistent`,
    // so subsequent scene loads skip it. That's how a project keeps one UI
    // scene, one music scene and one networking scene alive across every
    // transition instead of respawning them per level.
    //
    // A toggle per scene rather than an add/remove list: the set of candidates
    // is just `scenes/`, and "which of my scenes are global" is the question
    // being answered. Order follows `scan_scenes` (directory order), which
    // matters only if two global scenes race to touch the same thing at boot.
    let (sec, body) = section(
        commands,
        fonts,
        "layers",
        &tr("settings.cat.global_scenes"),
        A_BLUE,
    );
    commands.entity(col).add_child(sec);
    // Keyed to "project", not a key of its own: `global_scenes` never had a
    // sidebar entry, so every Project category hid it and the toggles could not
    // be reached at all. Same fix as the Language picker under Interface.
    focus_hide(commands, sec, focus, "project");
    if scenes.is_empty() {
        let lbl = commands
            .spawn((
                Text::new(tr("settings.hint.no_scenes")),
                ui_font(&fonts.ui, 12.0),
                TextColor(rgb(text_muted())),
                Node {
                    margin: UiRect::all(Val::Px(12.0)),
                    ..default()
                },
            ))
            .id();
        commands.entity(body).add_child(lbl);
    }
    for (i, scene) in scenes.iter().enumerate() {
        let get_name = scene.clone();
        let set_name = scene.clone();
        let t = ctl_toggle(
            commands,
            false,
            move |w| {
                w.get_resource::<CurrentProject>()
                    .is_some_and(|c| c.config.autoload.contains(&get_name))
            },
            move |w, on| {
                if let Some(mut cp) = w.get_resource_mut::<CurrentProject>() {
                    let list = &mut cp.config.autoload;
                    match (*on, list.iter().position(|a| *a == set_name)) {
                        // Guard against a double-add: the entry is the load
                        // instruction, so a duplicate spawns the scene twice.
                        (true, None) => list.push(set_name.clone()),
                        (false, Some(idx)) => {
                            list.remove(idx);
                        }
                        _ => {}
                    }
                }
                save_project(w);
            },
        );
        settings_row(commands, fonts, body, i, scene, t);
    }

    // Rendering (3D pipeline).
    let (sec, body) = section(commands, fonts, "monitor", &tr("settings.section.rendering_3d"), A_BLUE);
    commands.entity(col).add_child(sec);
    focus_hide(commands, sec, focus, "rendering");
    let rmode_opts = [
        tr("settings.opt.auto_per_platform"),
        tr("settings.opt.forward"),
        tr("settings.opt.deferred"),
    ];
    let rmode_refs: Vec<&str> = rmode_opts.iter().map(|s| s.as_str()).collect();
    let dd = ctl_dropdown(
        commands,
        fonts,
        &rmode_refs,
        0,
        |w| match w
            .get_resource::<CurrentProject>()
            .map(|c| c.config.rendering.mode)
            .unwrap_or_default()
        {
            RenderingMode::Auto => 0,
            RenderingMode::Forward => 1,
            RenderingMode::Deferred => 2,
        },
        |w, &i| {
            let m = match i {
                1 => RenderingMode::Forward,
                2 => RenderingMode::Deferred,
                _ => RenderingMode::Auto,
            };
            if let Some(mut cp) = w.get_resource_mut::<CurrentProject>() {
                cp.config.rendering.mode = m;
            }
            save_project(w);
        },
    );
    settings_row(commands, fonts, body, 0, &tr("common.mode"), dd);

    // Graphics quality for the SHIPPED GAME, and the same caveat as VSync below:
    // the identically-named row in Settings → Viewport → Performance writes
    // `ViewportSettings`, which is the editor's own viewport and is stripped from
    // an export. `[rendering] graphics_quality` is the one the runtime resolves
    // onto the play camera, and it had no control at all — so it sat on its
    // `Medium` default however the editor was configured.
    let gq_opts = [tr("common.low"), tr("common.medium"), tr("common.high")];
    let gq_refs: Vec<&str> = gq_opts.iter().map(|s| s.as_str()).collect();
    let dd = ctl_dropdown(
        commands,
        fonts,
        &gq_refs,
        1,
        |w| match w
            .get_resource::<CurrentProject>()
            .map(|c| c.config.rendering.graphics_quality)
            .unwrap_or_default()
        {
            GraphicsQuality::Low => 0,
            GraphicsQuality::Medium => 1,
            GraphicsQuality::High => 2,
        },
        |w, &i| {
            let q = match i {
                0 => GraphicsQuality::Low,
                2 => GraphicsQuality::High,
                _ => GraphicsQuality::Medium,
            };
            if let Some(mut cp) = w.get_resource_mut::<CurrentProject>() {
                cp.config.rendering.graphics_quality = q;
            }
            save_project(w);
        },
    );
    settings_row(commands, fonts, body, 1, &tr("settings.row.game_graphics_quality"), dd);

    // 3D render scale for the shipped game. Runtime-only by design — the editor
    // uses per-camera `CameraRenderResolution` — which is precisely why it needs
    // a control here: nothing in the editor would ever set it as a side effect.
    let dv = ctl_drag(
        commands,
        fonts,
        1.0,
        0.25,
        2.0,
        0.05,
        |w| {
            w.get_resource::<CurrentProject>()
                .map(|c| c.config.rendering.render_scale)
                .unwrap_or(1.0)
        },
        |w, &v| {
            if let Some(mut cp) = w.get_resource_mut::<CurrentProject>() {
                cp.config.rendering.render_scale = v.clamp(0.25, 2.0);
            }
            save_project(w);
        },
    );
    settings_row(commands, fonts, body, 2, &tr("settings.row.render_scale"), dv);

    note_row(commands, fonts, body, &tr("settings.hint.restart_rendering"));

    // Window.
    let (sec, body) = section(commands, fonts, "desktop", &tr("settings.cat.window"), A_BLUE);
    commands.entity(col).add_child(sec);
    focus_hide(commands, sec, focus, "window");
    let dv = proj_u32_drag(
        commands, fonts, 320.0, 7680.0,
        |c| c.window.width,
        |c, v| c.window.width = v,
    );
    settings_row(commands, fonts, body, 0, &tr("common.width"), dv);
    let dv = proj_u32_drag(
        commands, fonts, 240.0, 4320.0,
        |c| c.window.height,
        |c, v| c.window.height = v,
    );
    settings_row(commands, fonts, body, 1, &tr("common.height"), dv);
    let t = ctl_toggle(
        commands,
        true,
        |w| {
            w.get_resource::<CurrentProject>()
                .map(|c| c.config.window.resizable)
                .unwrap_or(true)
        },
        |w, &v| {
            if let Some(mut cp) = w.get_resource_mut::<CurrentProject>() {
                cp.config.window.resizable = v;
            }
            save_project(w);
        },
    );
    settings_row(commands, fonts, body, 2, &tr("settings.row.resizable"), t);
    let wmode_opts = [
        tr("settings.opt.windowed"),
        tr("settings.opt.fullscreen"),
        tr("settings.opt.borderless"),
    ];
    let wmode_refs: Vec<&str> = wmode_opts.iter().map(|s| s.as_str()).collect();
    let dd = ctl_dropdown(
        commands,
        fonts,
        &wmode_refs,
        0,
        |w| match w
            .get_resource::<CurrentProject>()
            .map(|c| c.config.window.mode)
            .unwrap_or_default()
        {
            WindowMode::Windowed => 0,
            WindowMode::Fullscreen => 1,
            WindowMode::Borderless => 2,
        },
        |w, &i| {
            let m = match i {
                1 => WindowMode::Fullscreen,
                2 => WindowMode::Borderless,
                _ => WindowMode::Windowed,
            };
            if let Some(mut cp) = w.get_resource_mut::<CurrentProject>() {
                cp.config.window.mode = m;
            }
            save_project(w);
        },
    );
    settings_row(commands, fonts, body, 3, &tr("common.mode"), dd);
    // VSync for the SHIPPED GAME. There is a second control with this name in
    // Settings → Viewport → Performance, and it governs the editor's own
    // viewport — `[editor.viewport] vsync` — which is why this one has to exist
    // separately rather than being folded into it. Without it the game field was
    // reachable only by hand-editing `project.toml`: it defaults to `true`, so an
    // export came out locked to the monitor's refresh while the editor, whose
    // vsync the user *had* turned off, ran uncapped. The two readings disagreeing
    // looked like a frame limiter in the runtime.
    let t = ctl_toggle(
        commands,
        true,
        |w| {
            w.get_resource::<CurrentProject>()
                .map(|c| c.config.window.vsync)
                .unwrap_or(true)
        },
        |w, &v| {
            if let Some(mut cp) = w.get_resource_mut::<CurrentProject>() {
                cp.config.window.vsync = v;
            }
            save_project(w);
        },
    );
    settings_row(commands, fonts, body, 4, &tr("settings.row.game_vsync"), t);

    // Render Resolution. Shares the "window" key so it sits directly under the
    // Window section: both carry a width/height pair, and the only thing that
    // tells them apart is that this one is the resolution the camera renders at
    // (honoured only when Stretch Mode is Viewport) while the window is the OS
    // surface it gets scaled onto. Calling it "Viewport" — its old name, and
    // still the `[viewport]` key in project.toml — made that unguessable.
    let (sec, body) = section(
        commands,
        fonts,
        "video-camera",
        &tr("settings.section.render_resolution"),
        A_PURPLE,
    );
    commands.entity(col).add_child(sec);
    focus_hide(commands, sec, focus, "window");
    let stretch_opts = [tr("common.disabled"), tr("settings.tab.viewport")];
    let stretch_refs: Vec<&str> = stretch_opts.iter().map(|s| s.as_str()).collect();
    let dd = ctl_dropdown(
        commands,
        fonts,
        &stretch_refs,
        0,
        |w| match w
            .get_resource::<CurrentProject>()
            .map(|c| c.config.viewport.stretch_mode)
            .unwrap_or_default()
        {
            StretchMode::Disabled => 0,
            StretchMode::Viewport => 1,
        },
        |w, &i| {
            let m = if i == 1 {
                StretchMode::Viewport
            } else {
                StretchMode::Disabled
            };
            if let Some(mut cp) = w.get_resource_mut::<CurrentProject>() {
                cp.config.viewport.stretch_mode = m;
            }
            save_project(w);
        },
    );
    settings_row(commands, fonts, body, 0, &tr("settings.row.stretch_mode"), dd);
    let dv = proj_u32_drag(
        commands, fonts, 16.0, 7680.0,
        |c| c.viewport.width,
        |c, v| c.viewport.width = v,
    );
    settings_row(commands, fonts, body, 1, &tr("common.width"), dv);
    let dv = proj_u32_drag(
        commands, fonts, 16.0, 4320.0,
        |c| c.viewport.height,
        |c, v| c.viewport.height = v,
    );
    settings_row(commands, fonts, body, 2, &tr("common.height"), dv);
    let aspect_opts = [
        tr("settings.opt.keep"),
        tr("settings.opt.expand"),
        tr("settings.opt.keep_width"),
        tr("settings.opt.keep_height"),
    ];
    let aspect_refs: Vec<&str> = aspect_opts.iter().map(|s| s.as_str()).collect();
    let dd = ctl_dropdown(
        commands,
        fonts,
        &aspect_refs,
        0,
        |w| match w
            .get_resource::<CurrentProject>()
            .map(|c| c.config.viewport.aspect_mode)
            .unwrap_or_default()
        {
            AspectMode::Keep => 0,
            AspectMode::Expand => 1,
            AspectMode::KeepWidth => 2,
            AspectMode::KeepHeight => 3,
        },
        |w, &i| {
            let m = match i {
                1 => AspectMode::Expand,
                2 => AspectMode::KeepWidth,
                3 => AspectMode::KeepHeight,
                _ => AspectMode::Keep,
            };
            if let Some(mut cp) = w.get_resource_mut::<CurrentProject>() {
                cp.config.viewport.aspect_mode = m;
            }
            save_project(w);
        },
    );
    settings_row(commands, fonts, body, 3, &tr("settings.row.aspect_mode"), dd);

    // Rendering 2D — a single dropdown, so it rides along under Rendering
    // rather than owning a sidebar row of its own.
    let (sec, body) = section(commands, fonts, "image-square", &tr("settings.section.rendering_2d"), A_BLUE);
    commands.entity(col).add_child(sec);
    focus_hide(commands, sec, focus, "rendering");
    let filter_opts = [tr("settings.opt.nearest"), tr("settings.opt.linear")];
    let filter_refs: Vec<&str> = filter_opts.iter().map(|s| s.as_str()).collect();
    let dd = ctl_dropdown(
        commands,
        fonts,
        &filter_refs,
        0,
        |w| match w
            .get_resource::<CurrentProject>()
            .map(|c| c.config.rendering_2d.image_filter)
            .unwrap_or_default()
        {
            TextureFilter::Nearest => 0,
            TextureFilter::Linear => 1,
        },
        |w, &i| {
            let f = if i == 1 {
                TextureFilter::Linear
            } else {
                TextureFilter::Nearest
            };
            if let Some(mut cp) = w.get_resource_mut::<CurrentProject>() {
                cp.config.rendering_2d.image_filter = f;
            }
            save_project(w);
        },
    );
    settings_row(commands, fonts, body, 0, &tr("settings.row.image_filter"), dd);
}

/// A drag-value bound to a `u32` field of the current project's config,
/// saving project.toml on edit.
fn proj_u32_drag(
    commands: &mut Commands,
    fonts: &EmberFonts,
    min: f32,
    max: f32,
    get: fn(&renzora::ProjectConfig) -> u32,
    set: fn(&mut renzora::ProjectConfig, u32),
) -> Entity {
    ctl_drag(
        commands,
        fonts,
        min,
        min,
        max,
        1.0,
        move |w| {
            w.get_resource::<CurrentProject>()
                .map(|c| get(&c.config) as f32)
                .unwrap_or(0.0)
        },
        move |w, &v| {
            if let Some(mut cp) = w.get_resource_mut::<CurrentProject>() {
                set(&mut cp.config, v.round().max(0.0) as u32);
            }
            save_project(w);
        },
    )
}
