//! Native (ember) "Modeling" panel — mode switching, select-mode buttons,
//! the topology-op buttons that don't have a natural viewport gesture
//! (subdivide, bisect, mirror, array, merges), their tunables, and the
//! sculpt brush picker.
//!
//! Buttons don't run operators themselves: they push [`ModelingOp`]s into
//! [`PendingOps`], the same funnel the keyboard shortcuts use, so undo and
//! selection bookkeeping stay in one place (`apply_pending_ops`).

use bevy::prelude::*;
use renzora::core::viewport_types::{ViewportMode, ViewportSettings};
use renzora_editor_framework::SplashState;
use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{bind_2way, bind_bg, bind_display, bind_text_color};
use renzora_ember::theme::*;
use renzora_ember::widgets::{drag_value, slider, DragRange};

use crate::sculpt::{BrushKind, SculptBrush};
use crate::selection::{MeshSelection, SelectMode};
use crate::tools::{ModelingOp, ModelingSettings, PendingOps};

const LABEL_W: f32 = 88.0;

pub struct NativeModeling;

impl Plugin for NativeModeling {
    fn build(&self, app: &mut App) {
        renzora::RenzoraShellExt::register_shell_panel(app, "modeling", "Modeling", "cube", "3D");
        app.register_panel_content("modeling", true, build);
        app.add_systems(
            Update,
            (
                mode_btn_click,
                sel_mode_btn_click,
                op_btn_click,
                brush_btn_click,
                toggle_btn_click,
            )
                .run_if(in_state(SplashState::Editor)),
        );
    }
}

// ── Marker components for click systems ────────────────────────────────────

#[derive(Component)]
struct ModeBtn(ViewportMode);

#[derive(Component)]
struct SelModeBtn(SelectMode);

#[derive(Component)]
struct OpBtn(ModelingOp);

#[derive(Component)]
struct BrushBtn(BrushKind);

#[derive(Component, Clone, Copy, PartialEq)]
enum ToggleBtn {
    SymmetryX,
    ArrayRelative,
}

// ── Click systems ──────────────────────────────────────────────────────────

fn mode_btn_click(
    q: Query<(&Interaction, &ModeBtn), Changed<Interaction>>,
    mut settings: Option<ResMut<ViewportSettings>>,
) {
    let Some(settings) = settings.as_mut() else {
        return;
    };
    for (i, btn) in &q {
        if *i == Interaction::Pressed {
            settings.viewport_mode = btn.0;
        }
    }
}

fn sel_mode_btn_click(
    q: Query<(&Interaction, &SelModeBtn), Changed<Interaction>>,
    mut commands: Commands,
) {
    for (i, btn) in &q {
        if *i == Interaction::Pressed {
            let mode = btn.0;
            // Deferred: the flush needs &mut World (reads the target's EditMesh).
            commands.queue(move |world: &mut World| {
                crate::systems::set_select_mode(world, mode);
            });
        }
    }
}

fn op_btn_click(
    q: Query<(&Interaction, &OpBtn), Changed<Interaction>>,
    mut pending: ResMut<PendingOps>,
) {
    for (i, btn) in &q {
        if *i == Interaction::Pressed {
            pending.0.push(btn.0);
        }
    }
}

fn brush_btn_click(
    q: Query<(&Interaction, &BrushBtn), Changed<Interaction>>,
    mut brush: ResMut<SculptBrush>,
) {
    for (i, btn) in &q {
        if *i == Interaction::Pressed {
            brush.kind = btn.0;
        }
    }
}

fn toggle_btn_click(
    q: Query<(&Interaction, &ToggleBtn), Changed<Interaction>>,
    mut settings: ResMut<ModelingSettings>,
) {
    for (i, btn) in &q {
        if *i != Interaction::Pressed {
            continue;
        }
        match btn {
            ToggleBtn::SymmetryX => settings.symmetry_x = !settings.symmetry_x,
            ToggleBtn::ArrayRelative => settings.array_relative = !settings.array_relative,
        }
    }
}

// ── Panel root ─────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(6.0)),
                ..default()
            },
            Name::new("native-modeling"),
        ))
        .id();

    // Mode switcher.
    let mode_row = button_row(commands);
    for mode in [ViewportMode::Scene, ViewportMode::Edit, ViewportMode::Sculpt] {
        let btn = pill_button(commands, fonts, mode.label(), move |w| {
            w.get_resource::<ViewportSettings>()
                .map(|s| s.viewport_mode == mode)
                .unwrap_or(false)
        });
        commands.entity(btn).insert(ModeBtn(mode));
        commands.entity(mode_row).add_child(btn);
    }
    let mode_hint = caption(
        commands,
        fonts,
        "Tab toggles Edit mode on the selected mesh",
        text_muted(),
    );
    commands.entity(root).add_children(&[mode_row, mode_hint]);

    // ── Edit-mode section ──
    let edit_section = section_col(commands);
    bind_display(commands, edit_section, |w| {
        w.get_resource::<ViewportSettings>()
            .map(|s| s.viewport_mode == ViewportMode::Edit)
            .unwrap_or(false)
    });
    build_edit_section(commands, fonts, edit_section);
    commands.entity(root).add_child(edit_section);

    // ── Sculpt-mode section ──
    let sculpt_section = section_col(commands);
    bind_display(commands, sculpt_section, |w| {
        w.get_resource::<ViewportSettings>()
            .map(|s| s.viewport_mode == ViewportMode::Sculpt)
            .unwrap_or(false)
    });
    build_sculpt_section(commands, fonts, sculpt_section);
    commands.entity(root).add_child(sculpt_section);

    root
}

fn build_edit_section(commands: &mut Commands, fonts: &EmberFonts, section: Entity) {
    // Select mode.
    let title = caption(commands, fonts, "Select", text_primary());
    let sel_row = button_row(commands);
    for (label, mode) in [
        ("Vertex", SelectMode::Vertex),
        ("Edge", SelectMode::Edge),
        ("Face", SelectMode::Face),
    ] {
        let btn = pill_button(commands, fonts, label, move |w| {
            w.get_resource::<MeshSelection>()
                .map(|s| s.mode == mode)
                .unwrap_or(false)
        });
        commands.entity(btn).insert(SelModeBtn(mode));
        commands.entity(sel_row).add_child(btn);
    }
    commands.entity(section).add_children(&[title, sel_row]);

    // Symmetry toggle.
    let sym = pill_button(commands, fonts, "X Symmetry", |w| {
        w.get_resource::<ModelingSettings>()
            .map(|s| s.symmetry_x)
            .unwrap_or(false)
    });
    commands.entity(sym).insert(ToggleBtn::SymmetryX);
    commands.entity(section).add_child(sym);

    // Ops.
    let ops_title = caption(commands, fonts, "Operations", text_primary());
    commands.entity(section).add_child(ops_title);
    for (label, op) in [
        ("Subdivide", ModelingOp::Subdivide),
        ("Inset Faces (I)", ModelingOp::Inset),
        ("Merge at Center (M)", ModelingOp::MergeAtCenter),
        ("Merge by Distance", ModelingOp::RemoveDoubles),
        ("Delete (X)", ModelingOp::Delete),
        ("Dissolve (Ctrl+X)", ModelingOp::Dissolve),
        ("Array", ModelingOp::Array),
    ] {
        let btn = pill_button(commands, fonts, label, |_| false);
        commands.entity(btn).insert(OpBtn(op));
        commands.entity(section).add_child(btn);
    }
    // Axis op rows.
    for (label, make) in [
        ("Bisect", ModelingOp::Bisect as fn(usize) -> ModelingOp),
        ("Mirror", ModelingOp::Mirror as fn(usize) -> ModelingOp),
    ] {
        let row = field_row(commands, fonts, label);
        for (axis_label, axis) in [("X", 0usize), ("Y", 1), ("Z", 2)] {
            let btn = pill_button(commands, fonts, axis_label, |_| false);
            commands.entity(btn).insert(OpBtn(make(axis)));
            commands.entity(row).add_child(btn);
        }
        commands.entity(section).add_child(row);
    }

    // Settings.
    let settings_title = caption(commands, fonts, "Settings", text_primary());
    let inset = labelled_slider(
        commands,
        fonts,
        "Inset",
        0.05,
        0.9,
        |w| {
            w.get_resource::<ModelingSettings>()
                .map(|s| s.inset_amount)
                .unwrap_or(0.25)
        },
        |w, v| set_modeling(w, |s| s.inset_amount = *v),
    );
    let weld = labelled_drag(
        commands,
        fonts,
        "Weld Dist",
        0.0001,
        0.5,
        0.001,
        |w| {
            w.get_resource::<ModelingSettings>()
                .map(|s| s.weld_dist)
                .unwrap_or(0.001)
        },
        |w, v| set_modeling(w, |s| s.weld_dist = v.max(0.0001)),
    );
    let count = labelled_drag(
        commands,
        fonts,
        "Array Count",
        2.0,
        16.0,
        0.1,
        |w| {
            w.get_resource::<ModelingSettings>()
                .map(|s| s.array_count as f32)
                .unwrap_or(2.0)
        },
        |w, v| set_modeling(w, |s| s.array_count = v.round().clamp(2.0, 64.0) as u32),
    );
    commands
        .entity(section)
        .add_children(&[settings_title, inset, weld, count]);

    // Array offset XYZ.
    let offset_row = field_row(commands, fonts, "Array Offset");
    for (axis, axis_label, color) in [
        (0usize, "X", (214u8, 84u8, 84u8)),
        (1, "Y", (120, 190, 84)),
        (2, "Z", (84, 130, 214)),
    ] {
        let dv = drag_value(commands, &fonts.ui, axis_label, color, 1.0, 0.05);
        commands.entity(dv).insert(DragRange {
            min: -100.0,
            max: 100.0,
        });
        bind_2way(
            commands,
            dv,
            move |w: &World| {
                w.get_resource::<ModelingSettings>()
                    .map(|s| s.array_offset[axis])
                    .unwrap_or(0.0)
            },
            move |w: &mut World, v: &f32| set_modeling(w, |s| s.array_offset[axis] = *v),
        );
        commands.entity(offset_row).add_child(dv);
    }
    let rel = pill_button(commands, fonts, "Relative Offset", |w| {
        w.get_resource::<ModelingSettings>()
            .map(|s| s.array_relative)
            .unwrap_or(true)
    });
    commands.entity(rel).insert(ToggleBtn::ArrayRelative);
    commands.entity(section).add_children(&[offset_row, rel]);

    // Shortcut cheatsheet.
    let keys_title = caption(commands, fonts, "Shortcuts", text_primary());
    commands.entity(section).add_child(keys_title);
    for line in [
        "1/2/3 — vertex / edge / face",
        "A select all · Shift+click add",
        "Alt+click — edge loop",
        "G grab (X/Y/Z lock) · E extrude",
        "Ctrl+R loop cut (scroll = cuts)",
        "Ctrl+J (Scene) — join meshes",
    ] {
        let hint = caption(commands, fonts, line, text_muted());
        commands.entity(section).add_child(hint);
    }
}

fn build_sculpt_section(commands: &mut Commands, fonts: &EmberFonts, section: Entity) {
    let title = caption(commands, fonts, "Brush", text_primary());
    commands.entity(section).add_child(title);

    // Two rows of three brush buttons.
    for row_kinds in BrushKind::ALL.chunks(3) {
        let row = button_row(commands);
        for kind in row_kinds {
            let kind = *kind;
            let btn = pill_button(commands, fonts, kind.label(), move |w| {
                w.get_resource::<SculptBrush>()
                    .map(|b| b.kind == kind)
                    .unwrap_or(false)
            });
            commands.entity(btn).insert(BrushBtn(kind));
            commands.entity(row).add_child(btn);
        }
        commands.entity(section).add_child(row);
    }

    let radius = labelled_slider(
        commands,
        fonts,
        "Radius",
        0.02,
        3.0,
        |w| {
            w.get_resource::<SculptBrush>()
                .map(|b| b.radius)
                .unwrap_or(0.35)
        },
        |w, v| {
            if let Some(mut b) = w.get_resource_mut::<SculptBrush>() {
                b.radius = *v;
            }
        },
    );
    let strength = labelled_slider(
        commands,
        fonts,
        "Strength",
        0.01,
        1.0,
        |w| {
            w.get_resource::<SculptBrush>()
                .map(|b| b.strength)
                .unwrap_or(0.5)
        },
        |w, v| {
            if let Some(mut b) = w.get_resource_mut::<SculptBrush>() {
                b.strength = *v;
            }
        },
    );
    commands.entity(section).add_children(&[radius, strength]);

    let sym = pill_button(commands, fonts, "X Symmetry", |w| {
        w.get_resource::<ModelingSettings>()
            .map(|s| s.symmetry_x)
            .unwrap_or(false)
    });
    commands.entity(sym).insert(ToggleBtn::SymmetryX);
    commands.entity(section).add_child(sym);

    for line in [
        "Ctrl — invert brush",
        "Shift — temporary smooth",
        "[ / ] — brush size",
    ] {
        let hint = caption(commands, fonts, line, text_muted());
        commands.entity(section).add_child(hint);
    }
}

// ── Small builders (same idiom as the terrain panel) ───────────────────────

fn set_modeling(w: &mut World, f: impl FnOnce(&mut ModelingSettings)) {
    if let Some(mut s) = w.get_resource_mut::<ModelingSettings>() {
        f(&mut s);
    }
}

fn section_col(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            margin: UiRect::top(Val::Px(2.0)),
            ..default()
        })
        .id()
}

fn button_row(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .id()
}

fn caption(commands: &mut Commands, fonts: &EmberFonts, text: &str, color: (u8, u8, u8)) -> Entity {
    commands
        .spawn((
            Text::new(text.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(color)),
            Node {
                margin: UiRect::vertical(Val::Px(1.0)),
                ..default()
            },
        ))
        .id()
}

fn field_row(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            margin: UiRect::bottom(Val::Px(2.0)),
            ..default()
        })
        .id();
    let lbl = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node {
                width: Val::Px(LABEL_W),
                flex_shrink: 0.0,
                ..default()
            },
        ))
        .id();
    commands.entity(row).add_child(lbl);
    row
}

/// A flexible pill button that highlights via `active` (and hover).
fn pill_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    active: impl Fn(&World) -> bool + Send + Sync + 'static,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                height: Val::Px(24.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                padding: UiRect::horizontal(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            Interaction::default(),
            HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new(format!("modeling-btn-{label}")),
        ))
        .id();
    let active = std::sync::Arc::new(active);
    let active_bg = active.clone();
    bind_bg(commands, btn, move |w| {
        if active_bg(w) {
            rgb(accent())
        } else if matches!(
            w.get::<Interaction>(btn),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        ) {
            rgb(hover_bg())
        } else {
            rgb(card_bg())
        }
    });
    let text = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    bind_text_color(commands, text, move |w| {
        if active(w) {
            Color::WHITE
        } else {
            rgb(text_primary())
        }
    });
    commands.entity(btn).add_child(text);
    btn
}

/// A labelled scrubbable numeric field.
#[allow(clippy::too_many_arguments)]
fn labelled_drag<G, S>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    min: f32,
    max: f32,
    step: f32,
    get: G,
    set: S,
) -> Entity
where
    G: Fn(&World) -> f32 + Send + Sync + 'static,
    S: Fn(&mut World, &f32) + Send + Sync + 'static,
{
    let row = field_row(commands, fonts, label);
    let dv = drag_value(commands, &fonts.ui, "", value_text(), min, step);
    if max > min {
        commands.entity(dv).insert(DragRange { min, max });
    }
    bind_2way(commands, dv, get, set);
    commands.entity(row).add_child(dv);
    row
}

/// A labelled slider mapped from the widget's 0..1 model to `min..max`.
fn labelled_slider<G, S>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    min: f32,
    max: f32,
    get: G,
    set: S,
) -> Entity
where
    G: Fn(&World) -> f32 + Send + Sync + 'static,
    S: Fn(&mut World, &f32) + Send + Sync + 'static,
{
    let row = field_row(commands, fonts, label);
    let span = (max - min).max(1e-6);
    let sld = slider(commands, 0.0);
    commands.entity(sld).insert(Node {
        flex_grow: 1.0,
        min_width: Val::Px(0.0),
        ..default()
    });
    let get_n = move |w: &World| ((get(w) - min) / span).clamp(0.0, 1.0);
    let set_n = move |w: &mut World, v: &f32| {
        let real = min + v.clamp(0.0, 1.0) * span;
        set(w, &real);
    };
    bind_2way(commands, sld, get_n, set_n);
    commands.entity(row).add_child(sld);
    row
}
