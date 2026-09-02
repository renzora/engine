//! What is drawn, and what is drawn *over* it.
//!
//! Two dropdowns that look alike and mean different things, deliberately kept
//! apart: **Display** decides what the renderer produces (visualization mode,
//! mesh / texture / lighting / shadow flags, the floor grid), while **Gizmos**
//! decides what the editor draws on top of it. The 2D view gets its own
//! **Overlays** dropdown, since almost nothing in the other two applies to it.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use renzora::core::viewport_types::{
    CollisionGizmoVisibility, ViewportSettings, VisualizationMode,
};
use renzora_editor_framework::EditorCommands;
use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::bind_2way;
use renzora_ember::reactive::Rx;
use renzora_ember::theme::{hover_bg, rgb, tab_active, text_primary, value_text};
use renzora_ember::widgets::{
    drag_value, icon_popup_trigger, popup_anchor, popup_panel, toggle_switch, DragRange, Popup,
};
use renzora_theme::ThemeManager;

use super::rows::{check_row, option_row, section_label, separator_row, toggle_row};
use super::{col, loc_opt};

/// Marks the Display dropdown's icon trigger (for hover / open background).
#[derive(Component)]
pub(super) struct DisplayTrigger;

/// A click-to-select option inside the Display popup.
#[derive(Component, Clone, Copy)]
pub(super) enum DisplayOption {
    /// Visualization mode by index into `VisualizationMode::ALL`.
    Viz(usize),
    /// Collision gizmo visibility — `true` = Selected Only, `false` = Always.
    Collision(bool),
}

pub(super) fn build_display_dropdown(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let mut kids: Vec<Entity> = Vec::new();

    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.display.visualization")));
    for (i, m) in VisualizationMode::ALL.iter().enumerate() {
        kids.push(option_row(commands, fonts, DisplayOption::Viz(i), &loc_opt(m.label())));
    }

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.display.render")));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.display.mesh"), render_toggles.mesh));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.display.textures"), render_toggles.textures));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.shading.wireframe"), render_toggles.wireframe));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.display.lighting"), render_toggles.lighting));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.display.shadows"), render_toggles.shadows));

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.display.overlays")));
    kids.push(grid_divisions_row(commands, fonts));
    // Subgrid's switch used to sit here, but it never touched this grid — the
    // flag only splits the *2D* editor's grid into major/minor lines. Dividing
    // the 3D grid is what the -/+ above does, and the 2D flag keeps its home in
    // Settings → Viewport.
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.display.axis_gizmo"), show_axis_gizmo));
    // Sits with the axis gizmo rather than in Gizmos: both are corner chrome
    // that belongs to the *view*, where everything in Gizmos is drawn per
    // scene object.
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.display.stats"), show_stats));
    // Scene Icons / Labels and the whole collision-gizmo picker used to live
    // here; they moved to the Gizmos dropdown (`build_gizmos_dropdown`) so all
    // the "what's drawn over the scene" switches sit in one place. Display keeps
    // what the renderer produces — viz modes, render flags, and the grid.

    let panel = popup_panel(commands, &kids);
    let trigger = icon_popup_trigger(commands, fonts, "eye", panel);
    commands.entity(trigger).insert(DisplayTrigger);
    popup_anchor(commands, trigger, panel)
}

/// The **Gizmos** dropdown — one switch per editor overlay drawn *over* the
/// scene, grouped by what it belongs to (selection / scene objects / rigging /
/// physics).
///
/// Separate from Display on purpose: Display decides what the renderer
/// produces (visualization mode, mesh/texture/lighting/shadow flags, the
/// grid), while these decide what the *editor* draws on top of it. Several of
/// these gizmos had no switch at all before — the skeleton bones, the light
/// falloff wireframes and the camera frustum were unconditional — so this is
/// the first way to get a dense rig or a wall of collider boxes out of the
/// view without deselecting.
///
/// Lives in the shared toolbar rather than a viewport's own strip because
/// `ViewportSettings` is one global resource: a per-slot placement would
/// promise per-viewport control that doesn't exist.
pub(super) fn build_gizmos_dropdown(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let mut kids: Vec<Entity> = Vec::new();

    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.gizmos.selection")));
    kids.push(toggle_row!(
        commands, fonts,
        &renzora::lang::t("viewport.gizmos.bounding_box"),
        show_selection_box
    ));

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.gizmos.scene")));
    kids.push(toggle_row!(
        commands, fonts,
        &renzora::lang::t("viewport.gizmos.lights"),
        show_light_gizmos
    ));
    kids.push(toggle_row!(
        commands, fonts,
        &renzora::lang::t("viewport.gizmos.cameras"),
        show_camera_gizmos
    ));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.display.scene_icons"), show_scene_icons));
    kids.push(toggle_row!(commands, fonts, &renzora::lang::t("viewport.display.labels"), show_labels));
    // The game's own UI, not an editor overlay — but it belongs here because
    // from the viewport's point of view it is one more thing drawn over the
    // scene, and this is where you come to turn those off.
    kids.push(toggle_row!(
        commands, fonts,
        &renzora::lang::t_or("viewport.gizmos.game_ui", "Game UI"),
        show_game_ui
    ));

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.gizmos.rigging")));
    kids.push(toggle_row!(
        commands, fonts,
        &renzora::lang::t("viewport.gizmos.skeleton"),
        show_skeleton_gizmos
    ));

    kids.push(separator_row(commands));
    kids.push(section_label(commands, fonts, &renzora::lang::t("viewport.gizmos.physics")));
    // The on/off half of `collision_gizmo_visibility`. Turning it back on
    // restores the remembered Selected Only / Always choice rather than always
    // snapping to Selected Only.
    kids.push(check_row(
        commands,
        fonts,
        &renzora::lang::t("viewport.gizmos.colliders"),
        |w: &Rx| {
            w.get_resource::<ViewportSettings>()
                .map(|s| s.collision_gizmo_visibility != CollisionGizmoVisibility::Off)
                .unwrap_or(false)
        },
        |w: &mut World, v: bool| {
            let restore = w
                .get_resource::<ColliderGizmoMemory>()
                .map(|m| m.0)
                .unwrap_or(CollisionGizmoVisibility::SelectedOnly);
            if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                s.collision_gizmo_visibility = if v {
                    restore
                } else {
                    CollisionGizmoVisibility::Off
                };
            }
        },
    ));
    // The two mode rows also act as an "on" — picking either while the switch
    // is off turns colliders back on in that mode, which is what a click on a
    // visible row is expected to do.
    kids.push(option_row(
        commands,
        fonts,
        DisplayOption::Collision(true),
        &renzora::lang::t("viewport.display.selected_only"),
    ));
    kids.push(option_row(
        commands,
        fonts,
        DisplayOption::Collision(false),
        &renzora::lang::t("common.always"),
    ));

    let panel = popup_panel(commands, &kids);
    let trigger = icon_popup_trigger(commands, fonts, "bounding-box", panel);
    commands.entity(trigger).insert(DisplayTrigger);
    popup_anchor(commands, trigger, panel)
}

/// Remembers the last non-`Off` collider-gizmo mode so the Physics →
/// "Colliders" switch can return to Selected Only *or* Always — whichever was
/// in use. Editor UI state only: never persisted (a fresh session starts from
/// whatever `collision_gizmo_visibility` loaded as) and never crosses the
/// plugin boundary, so it stays here instead of in `ViewportSettings`.
#[derive(Resource)]
pub(super) struct ColliderGizmoMemory(CollisionGizmoVisibility);

impl Default for ColliderGizmoMemory {
    fn default() -> Self {
        Self(CollisionGizmoVisibility::SelectedOnly)
    }
}

/// The 2D **Overlays** dropdown — the 2D counterpart of the 3D Display
/// dropdown. Three switches bound to the 2D `ViewportSettings` flags: the
/// behind-sprites Grid, the ruler bars, and the editor Gizmos (the light
/// markers + selected-light/occluder outlines). The icon trigger reuses
/// `DisplayTrigger` so it themes identically to the other icon dropdowns and
/// needs no extra visuals system; the caller adds `TwoDOnly` so it shows only
/// in 2D view (the 3D Display dropdown is the mirror-image `ThreeDOnly`).
pub(super) fn build_overlay_2d_dropdown(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let kids = vec![
        section_label(commands, fonts, &renzora::lang::t("viewport.display.overlays")),
        grid_row(commands, fonts),
        toggle_row!(commands, fonts, &renzora::lang::t("viewport.rulers"), show_rulers_2d),
        toggle_row!(
            commands,
            fonts,
            &renzora::lang::t_or("viewport.gizmos", "Gizmos"),
            show_gizmos_2d
        ),
    ];
    let panel = popup_panel(commands, &kids);
    let trigger = icon_popup_trigger(commands, fonts, "eye", panel);
    commands.entity(trigger).insert(DisplayTrigger);
    popup_anchor(commands, trigger, panel)
}

/// The Grid row of the 2D Overlays dropdown: label, the cell-size input, then
/// the on/off switch. Unlike the other rows (plain `toggle_row!`), the grid
/// carries its size setting inline — a boxed ember [`drag_value`] editing
/// `ViewportSettings.grid_size_2d` in whole world units — sitting to the LEFT of
/// the switch. Its own setting, deliberately NOT the translate-snap step (tying
/// them together made the snap pill silently restyle the grid).
fn grid_row(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // On/off switch → `show_grid_2d`.
    let sw = toggle_switch(commands, false);
    bind_2way(
        commands,
        sw,
        |w: &Rx| {
            w.get_resource::<ViewportSettings>()
                .map(|s| s.show_grid_2d)
                .unwrap_or(false)
        },
        |w: &mut World, v: &bool| {
            if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                s.show_grid_2d = *v;
            }
        },
    );

    // Cell-size input — a boxed ember numeric field (click to type, drag to
    // scrub). Whole world units: a fractional grid size is never what a tile
    // artist wants, and int snapping keeps the readout stable.
    let dv = drag_value(commands, &fonts.ui, "", value_text(), 16.0, 0.2);
    commands.entity(dv).insert((
        DragRange { min: 1.0, max: 1024.0 },
        renzora_ember::widgets::DragSnap(1.0),
    ));
    bind_2way(
        commands,
        dv,
        |w: &Rx| {
            w.get_resource::<ViewportSettings>()
                .map(|s| s.grid_size_2d)
                .unwrap_or(16.0)
        },
        |w: &mut World, v: &f32| {
            if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                let want = v.round().max(1.0);
                if s.grid_size_2d != want {
                    s.grid_size_2d = want;
                }
            }
        },
    );

    let lbl = commands
        .spawn((
            Text::new(renzora::lang::t("viewport.grid")),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(value_text())),
        ))
        .id();
    let spacer = commands
        .spawn(Node {
            flex_grow: 1.0,
            ..default()
        })
        .id();
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
            Name::new("vp-2d-grid-row"),
        ))
        .id();
    commands.entity(row).add_children(&[lbl, spacer, dv, sw]);
    row
}

/// The Display dropdown's **Grid** row: the on/off switch, plus `-` / `+` that
/// subdivide the floor grid.
///
/// Each press halves (or doubles) the squares — the grid is infinite and
/// unitless, so a subdivision count is the only thing that means anything here;
/// a cell size in world units would be a number with nothing to measure against.
/// The readout is the divisor: `1` is the base grid, `4` is sixteenth-squares.
fn grid_divisions_row(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let sw = toggle_switch(commands, false);
    bind_2way(
        commands,
        sw,
        |w: &Rx| {
            w.get_resource::<ViewportSettings>()
                .map(|s| s.show_grid)
                .unwrap_or(false)
        },
        |w: &mut World, v: &bool| {
            if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                s.show_grid = *v;
            }
        },
    );

    let lbl = commands
        .spawn((
            Text::new(renzora::lang::t("viewport.grid")),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(value_text())),
        ))
        .id();
    let spacer = commands
        .spawn(Node {
            flex_grow: 1.0,
            ..default()
        })
        .id();

    let minus = grid_div_btn(commands, fonts, "minus", false);
    let count = commands
        .spawn((
            Text::new("1"),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
            Name::new("vp-grid-divisions"),
        ))
        .id();
    renzora_ember::reactive::tracked::bind_text(commands, count, |w| {
        w.get_resource::<ViewportSettings>()
            .map(|s| s.grid_divisions.to_string())
            .unwrap_or_else(|| "1".into())
    });
    let plus = grid_div_btn(commands, fonts, "plus", true);

    let stepper = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
            Name::new("vp-grid-stepper"),
        ))
        .id();
    commands.entity(stepper).add_children(&[minus, count, plus]);

    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
            Name::new("vp-grid-row"),
        ))
        .id();
    commands.entity(row).add_children(&[lbl, spacer, stepper, sw]);
    row
}

/// One end of the grid stepper. `up` doubles the divisions, otherwise halves —
/// powers of two, so the finer lines always land on the coarser ones.
#[derive(Component, Clone, Copy)]
pub(super) struct GridDivBtn(bool);

fn grid_div_btn(commands: &mut Commands, fonts: &EmberFonts, icon: &str, up: bool) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(18.0),
                height: Val::Px(18.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(tab_active())),
            Interaction::default(),
            HoverCursor(SystemCursorIcon::Pointer),
            GridDivBtn(up),
            Name::new(if up { "vp-grid-div-up" } else { "vp-grid-div-down" }),
        ))
        .id();
    let g = icon_text(commands, &fonts.phosphor, icon, renzora_ember::theme::text_muted(), 10.0);
    commands.entity(g).insert(bevy::ui::FocusPolicy::Pass);
    commands.entity(btn).add_child(g);
    renzora_ember::reactive::tracked::bind_bg(commands, btn, move |w| {
        match w.get::<Interaction>(btn) {
            Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(hover_bg()),
            _ => rgb(tab_active()),
        }
    });
    btn
}

/// `-` / `+` on the grid row → halve or double the subdivision, clamped to a
/// range where the lines stay distinguishable at a sane camera height.
pub(super) fn grid_div_click(
    buttons: Query<(&Interaction, &GridDivBtn), Changed<Interaction>>,
    settings: Option<ResMut<ViewportSettings>>,
) {
    let Some(mut settings) = settings else { return };
    for (interaction, btn) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let want = if btn.0 {
            settings.grid_divisions.saturating_mul(2)
        } else {
            settings.grid_divisions / 2
        }
        .clamp(1, 64);
        if settings.grid_divisions != want {
            settings.grid_divisions = want;
        }
    }
}

pub(super) fn display_option_click(
    options: Query<(&Interaction, &DisplayOption), Changed<Interaction>>,
    cmds: Option<Res<EditorCommands>>,
) {
    let Some(cmds) = cmds else { return };
    for (interaction, opt) in &options {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *opt {
            DisplayOption::Viz(i) => cmds.push(move |w: &mut World| {
                if let (Some(mut s), Some(m)) = (
                    w.get_resource_mut::<ViewportSettings>(),
                    VisualizationMode::ALL.get(i).copied(),
                ) {
                    s.visualization_mode = m;
                }
            }),
            // Picking a mode also turns colliders on if they were `Off` — the
            // row is right there under the switch, so a click on it meaning
            // "show me this" is the only sensible reading.
            DisplayOption::Collision(selected_only) => cmds.push(move |w: &mut World| {
                let mode = if selected_only {
                    CollisionGizmoVisibility::SelectedOnly
                } else {
                    CollisionGizmoVisibility::Always
                };
                if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
                    s.collision_gizmo_visibility = mode;
                }
            }),
        }
    }
}

pub(super) fn update_display_visuals(
    settings: Option<Res<ViewportSettings>>,
    mut collider_memory: ResMut<ColliderGizmoMemory>,
    theme: Option<Res<ThemeManager>>,
    triggers: Query<(&Interaction, &Popup, &mut BackgroundColor), With<DisplayTrigger>>,
    options: Query<(&DisplayOption, &Interaction, &mut BackgroundColor), Without<DisplayTrigger>>,
) {
    let (Some(settings), Some(theme)) = (settings, theme) else {
        return;
    };
    let t = &theme.active_theme;
    let accent = col(t.semantic.accent);
    let inactive = col(t.widgets.inactive_bg);
    let hovered = col(t.widgets.hovered_bg);

    for (interaction, toggle, mut bg) in triggers {
        let want = if toggle.open || *interaction == Interaction::Hovered {
            hovered
        } else {
            inactive
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }

    let viz_idx = VisualizationMode::ALL
        .iter()
        .position(|m| *m == settings.visualization_mode);
    // Keep the "turn colliders back on into what?" memory current. Doing it
    // here (rather than in the switch's setter) also catches changes made from
    // the Settings panel's Colliders dropdown.
    let collision = settings.collision_gizmo_visibility;
    if collision != CollisionGizmoVisibility::Off && collider_memory.0 != collision {
        collider_memory.0 = collision;
    }

    for (opt, interaction, mut bg) in options {
        let is_current = match *opt {
            DisplayOption::Viz(i) => viz_idx == Some(i),
            // Neither mode row is "current" while colliders are off — the
            // switch above them is the thing that reads as off.
            DisplayOption::Collision(sel) => match collision {
                CollisionGizmoVisibility::Off => false,
                CollisionGizmoVisibility::SelectedOnly => sel,
                CollisionGizmoVisibility::Always => !sel,
            },
        };
        let want = if is_current {
            accent
        } else if *interaction == Interaction::Hovered {
            hovered
        } else {
            Color::NONE
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}
