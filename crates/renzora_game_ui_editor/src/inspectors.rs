//! Native (ember) inspector drawers for `renzora_game_ui` widget components that
//! can't be a flat declarative field list (grouped controls, conditional UI,
//! dynamic lists). They live here because they need `renzora_ember`, and
//! `game_ui` itself can't depend on ember (the `ember -> hui -> game_ui` cycle).

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use renzora::{AppEditorExt, SplashState};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::inspector::{color_field_rgba, inspector_body, inspector_row, inspector_stripe};
use renzora_ember::reactive::{bind_2way, bind_bg, bind_display};
use renzora_ember::theme::{accent, rgb, text_muted, text_primary};
use renzora_ember::widgets::{bind_text_input, checkbox, drag_value, dropdown, icon_label_button, text_input, DragRange};

use renzora_ember::game_ui::components::{
    DropdownData, GradientStop, UiCursor, UiFill, UiInteractionStyle, UiStateStyle, UiStroke, UiTransition,
};

pub(crate) fn register(app: &mut App) {
    app.register_native_inspector_ui("ui_stroke", stroke_native);
    app.register_native_inspector_ui("ui_dropdown_data", dropdown_native);
    app.register_native_inspector_ui("ui_fill", fill_native);
    app.register_native_inspector_ui("ui_interaction", interaction_native);
    app.add_systems(
        Update,
        (
            stroke_side_click,
            rebuild_dropdown,
            dropdown_add_click,
            dropdown_remove_click,
            rebuild_fill,
            fill_add_stop_click,
            fill_remove_stop_click,
        )
            .run_if(in_state(SplashState::Editor)),
    );
}

// ── Border (UiStroke) ────────────────────────────────────────────────────────

#[derive(Component)]
struct StrokeSideBtn {
    entity: Entity,
    side: usize,
}

/// Native drawer for `UiStroke` — Color, Width, and a row of four side toggles
/// (Top/Right/Bottom/Left), mirroring `render_stroke_inspector`.
fn stroke_native(world: &mut World, entity: Entity) -> Entity {
    inspector_body(world, move |commands, fonts| {
        let col = commands
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(2.0)),
                ..default()
            })
            .id();

        // Color (RGBA).
        let color = color_field_rgba(
            commands,
            move |w| w.get::<UiStroke>(entity).map(|s| s.color.to_srgba().to_f32_array()).unwrap_or([0.0; 4]),
            move |w, a: [f32; 4]| {
                if let Some(mut s) = w.get_mut::<UiStroke>(entity) {
                    s.color = Color::srgba(a[0], a[1], a[2], a[3]);
                }
            },
        );
        let r_color = inspector_row(commands, &fonts.ui, "Color", color);

        // Width.
        let width = drag_value(commands, &fonts.ui, "", (210, 210, 220), 0.0, 0.5);
        commands.entity(width).insert(DragRange { min: 0.0, max: 50.0 });
        bind_2way(
            commands,
            width,
            move |w| w.get::<UiStroke>(entity).map(|s| s.width).unwrap_or(0.0),
            move |w, v: &f32| {
                if let Some(mut s) = w.get_mut::<UiStroke>(entity) {
                    s.width = *v;
                }
            },
        );
        let r_width = inspector_row(commands, &fonts.ui, "Width", width);

        // Sides — four icon toggle buttons.
        let group = commands
            .spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                flex_grow: 1.0,
                ..default()
            })
            .id();
        let btns: Vec<Entity> = [
            ("arrow-line-up", 0usize),
            ("arrow-line-right", 1),
            ("arrow-line-down", 2),
            ("arrow-line-left", 3),
        ]
        .iter()
        .map(|&(icon, side)| side_toggle(commands, fonts, entity, side, icon))
        .collect();
        commands.entity(group).add_children(&btns);
        let r_sides = inspector_row(commands, &fonts.ui, "Sides", group);

        let rows = [r_color, r_width, r_sides];
        for (i, r) in rows.iter().enumerate() {
            commands.entity(*r).insert(BackgroundColor(inspector_stripe(i)));
        }
        commands.entity(col).add_children(&rows);
        col
    })
}

fn side_toggle(commands: &mut Commands, fonts: &EmberFonts, entity: Entity, side: usize, icon: &str) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Px(24.0),
                height: Val::Px(22.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            StrokeSideBtn { entity, side },
            Name::new("stroke-side"),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 13.0);
    commands.entity(btn).add_child(ic);
    bind_bg(commands, btn, move |w| {
        let on = w.get::<UiStroke>(entity).map(|s| read_side(s, side)).unwrap_or(false);
        if on {
            rgb(accent())
        } else {
            Color::NONE
        }
    });
    btn
}

fn read_side(s: &UiStroke, side: usize) -> bool {
    match side {
        0 => s.sides.top,
        1 => s.sides.right,
        2 => s.sides.bottom,
        _ => s.sides.left,
    }
}

fn stroke_side_click(q: Query<(&Interaction, &StrokeSideBtn), Changed<Interaction>>, mut commands: Commands) {
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let (e, side) = (btn.entity, btn.side);
        commands.queue(move |w: &mut World| {
            if let Some(mut s) = w.get_mut::<UiStroke>(e) {
                match side {
                    0 => s.sides.top = !s.sides.top,
                    1 => s.sides.right = !s.sides.right,
                    2 => s.sides.bottom = !s.sides.bottom,
                    _ => s.sides.left = !s.sides.left,
                }
            }
        });
    }
}

// ── Dropdown (DropdownData options list) ─────────────────────────────────────

/// Root for the dropdown inspector; `sig` (option count) drives the rebuild so
/// Add/Remove restructures the option rows. Text edits sync via bindings.
#[derive(Component)]
struct DropdownRoot {
    entity: Entity,
    sig: Option<u64>,
}
#[derive(Component)]
struct DropdownAddBtn {
    entity: Entity,
}
#[derive(Component)]
struct DropdownRemoveBtn {
    entity: Entity,
}

fn dropdown_native(world: &mut World, entity: Entity) -> Entity {
    world
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() },
            DropdownRoot { entity, sig: None },
            Name::new("dropdown-inspector-root"),
        ))
        .id()
}

/// Rebuild the option rows + Selected dropdown when the option count changes.
fn rebuild_dropdown(world: &mut World) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
    let mut q = world.query::<(Entity, &DropdownRoot)>();
    let roots: Vec<(Entity, Entity, Option<u64>)> = q.iter(world).map(|(r, d)| (r, d.entity, d.sig)).collect();
    for (root, entity, old_sig) in roots {
        let Some(data) = world.get::<DropdownData>(entity).cloned() else { continue };
        let sig = data.options.len() as u64;
        if old_sig == Some(sig) {
            continue;
        }
        let existing: Vec<Entity> = world.get::<Children>(root).map(|c| c.iter().collect()).unwrap_or_default();
        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            for ch in existing {
                commands.entity(ch).despawn();
            }
            build_dropdown_body(&mut commands, &fonts, root, entity, &data);
        }
        queue.apply(world);
        if let Some(mut dr) = world.get_mut::<DropdownRoot>(root) {
            dr.sig = Some(sig);
        }
    }
}

fn build_dropdown_body(commands: &mut Commands, fonts: &EmberFonts, root: Entity, entity: Entity, data: &DropdownData) {
    let mut rows: Vec<Entity> = Vec::new();

    // Placeholder.
    let ph = text_input(commands, &fonts.ui, "", &data.placeholder);
    bind_text_input(
        commands,
        ph,
        move |w| w.get::<DropdownData>(entity).map(|d| d.placeholder.clone()).unwrap_or_default(),
        move |w, s: String| {
            if let Some(mut d) = w.get_mut::<DropdownData>(entity) {
                d.placeholder = s;
            }
        },
    );
    rows.push(inspector_row(commands, &fonts.ui, "Placeholder", ph));

    // Selected (dropdown of the current option labels).
    let labels: Vec<&str> = data.options.iter().map(|s| s.as_str()).collect();
    let sel = if data.selected >= 0 { data.selected as usize } else { 0 };
    let dd = dropdown(commands, fonts, &labels, sel);
    bind_2way(
        commands,
        dd,
        move |w| w.get::<DropdownData>(entity).map(|d| d.selected.max(0) as usize).unwrap_or(0),
        move |w, v: &usize| {
            if let Some(mut d) = w.get_mut::<DropdownData>(entity) {
                d.selected = *v as i32;
            }
        },
    );
    rows.push(inspector_row(commands, &fonts.ui, "Selected", dd));

    // One text input per option.
    for i in 0..data.options.len() {
        let ti = text_input(commands, &fonts.ui, "", &data.options[i]);
        bind_text_input(
            commands,
            ti,
            move |w| w.get::<DropdownData>(entity).and_then(|d| d.options.get(i).cloned()).unwrap_or_default(),
            move |w, s: String| {
                if let Some(mut d) = w.get_mut::<DropdownData>(entity) {
                    if let Some(o) = d.options.get_mut(i) {
                        *o = s;
                    }
                }
            },
        );
        rows.push(inspector_row(commands, &fonts.ui, &format!("#{}", i + 1), ti));
    }

    // Add / Remove.
    let add = icon_label_button(commands, fonts, "plus", "Add");
    commands.entity(add).insert(DropdownAddBtn { entity });
    let mut btns = vec![add];
    if data.options.len() > 1 {
        let rem = icon_label_button(commands, fonts, "minus", "Remove");
        commands.entity(rem).insert(DropdownRemoveBtn { entity });
        btns.push(rem);
    }
    let btn_group = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), margin: UiRect::top(Val::Px(4.0)), ..default() })
        .id();
    commands.entity(btn_group).add_children(&btns);
    rows.push(btn_group);

    for (i, r) in rows.iter().enumerate() {
        commands.entity(*r).insert(BackgroundColor(inspector_stripe(i)));
    }
    commands.entity(root).add_children(&rows);
}

fn dropdown_add_click(q: Query<(&Interaction, &DropdownAddBtn), Changed<Interaction>>, mut commands: Commands) {
    for (interaction, b) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let e = b.entity;
        commands.queue(move |w: &mut World| {
            if let Some(mut d) = w.get_mut::<DropdownData>(e) {
                let n = d.options.len() + 1;
                d.options.push(format!("Option {}", n));
            }
        });
    }
}

fn dropdown_remove_click(q: Query<(&Interaction, &DropdownRemoveBtn), Changed<Interaction>>, mut commands: Commands) {
    for (interaction, b) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let e = b.entity;
        commands.queue(move |w: &mut World| {
            if let Some(mut d) = w.get_mut::<DropdownData>(e) {
                if d.options.len() > 1 {
                    d.options.pop();
                }
            }
        });
    }
}

// ── Fill (UiFill enum) ───────────────────────────────────────────────────────

/// Root for the fill inspector. `sig` packs the variant discriminant + gradient
/// stop count, so switching Type or Add/Remove-ing a stop restructures the rows;
/// angle/center/color/pos edits sync via bindings (no rebuild).
#[derive(Component)]
struct FillRoot {
    entity: Entity,
    sig: Option<u64>,
}
#[derive(Component)]
struct FillAddStop {
    entity: Entity,
}
#[derive(Component)]
struct FillRemoveStop {
    entity: Entity,
}

fn fill_disc(f: &UiFill) -> usize {
    match f {
        UiFill::None => 0,
        UiFill::Solid(_) => 1,
        UiFill::LinearGradient { .. } => 2,
        UiFill::RadialGradient { .. } => 3,
    }
}

fn fill_stops_len(f: &UiFill) -> usize {
    match f {
        UiFill::LinearGradient { stops, .. } | UiFill::RadialGradient { stops, .. } => stops.len(),
        _ => 0,
    }
}

fn fill_stops_mut(f: &mut UiFill) -> Option<&mut Vec<GradientStop>> {
    match f {
        UiFill::LinearGradient { stops, .. } | UiFill::RadialGradient { stops, .. } => Some(stops),
        _ => None,
    }
}

fn fill_native(world: &mut World, entity: Entity) -> Entity {
    world
        .spawn((
            Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(3.0), ..default() },
            FillRoot { entity, sig: None },
            Name::new("fill-inspector-root"),
        ))
        .id()
}

/// Rebuild the variant-specific rows when the Type or the gradient-stop count changes.
fn rebuild_fill(world: &mut World) {
    let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else { return };
    let mut q = world.query::<(Entity, &FillRoot)>();
    let roots: Vec<(Entity, Entity, Option<u64>)> = q.iter(world).map(|(r, d)| (r, d.entity, d.sig)).collect();
    for (root, entity, old_sig) in roots {
        let Some(fill) = world.get::<UiFill>(entity).cloned() else { continue };
        let sig = (fill_disc(&fill) as u64) << 32 | fill_stops_len(&fill) as u64;
        if old_sig == Some(sig) {
            continue;
        }
        let existing: Vec<Entity> = world.get::<Children>(root).map(|c| c.iter().collect()).unwrap_or_default();
        let mut queue = CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            for ch in existing {
                commands.entity(ch).despawn();
            }
            build_fill_body(&mut commands, &fonts, root, entity, &fill);
        }
        queue.apply(world);
        if let Some(mut fr) = world.get_mut::<FillRoot>(root) {
            fr.sig = Some(sig);
        }
    }
}

fn build_fill_body(commands: &mut Commands, fonts: &EmberFonts, root: Entity, entity: Entity, fill: &UiFill) {
    let mut rows: Vec<Entity> = Vec::new();

    // Type combo.
    let type_dd = dropdown(commands, fonts, &["None", "Solid", "Linear", "Radial"], fill_disc(fill));
    bind_2way(
        commands,
        type_dd,
        move |w| w.get::<UiFill>(entity).map(fill_disc).unwrap_or(0),
        move |w, v: &usize| {
            if let Some(mut f) = w.get_mut::<UiFill>(entity) {
                *f = match v {
                    1 => UiFill::Solid(Color::srgba(0.2, 0.2, 0.2, 1.0)),
                    2 => UiFill::linear(0.0, Color::srgba(0.2, 0.2, 0.8, 1.0), Color::srgba(0.8, 0.2, 0.2, 1.0)),
                    3 => UiFill::RadialGradient {
                        center: [0.5, 0.5],
                        stops: vec![
                            GradientStop { position: 0.0, color: Color::WHITE },
                            GradientStop { position: 1.0, color: Color::BLACK },
                        ],
                    },
                    _ => UiFill::None,
                };
            }
        },
    );
    rows.push(inspector_row(commands, &fonts.ui, "Type", type_dd));

    match fill {
        UiFill::Solid(_) => {
            let color = color_field_rgba(
                commands,
                move |w| match w.get::<UiFill>(entity) {
                    Some(UiFill::Solid(c)) => c.to_srgba().to_f32_array(),
                    _ => [0.2, 0.2, 0.2, 1.0],
                },
                move |w, a: [f32; 4]| {
                    if let Some(mut f) = w.get_mut::<UiFill>(entity) {
                        *f = UiFill::Solid(Color::srgba(a[0], a[1], a[2], a[3]));
                    }
                },
            );
            rows.push(inspector_row(commands, &fonts.ui, "Color", color));
        }
        UiFill::LinearGradient { .. } => {
            let angle = drag_value(commands, &fonts.ui, "", (210, 210, 220), 0.0, 1.0);
            commands.entity(angle).insert(DragRange { min: 0.0, max: 360.0 });
            bind_2way(
                commands,
                angle,
                move |w| match w.get::<UiFill>(entity) {
                    Some(UiFill::LinearGradient { angle, .. }) => *angle,
                    _ => 0.0,
                },
                move |w, v: &f32| {
                    if let Some(mut f) = w.get_mut::<UiFill>(entity) {
                        if let UiFill::LinearGradient { angle, .. } = &mut *f {
                            *angle = *v;
                        }
                    }
                },
            );
            rows.push(inspector_row(commands, &fonts.ui, "Angle", angle));
            build_gradient_stops(commands, fonts, entity, fill, &mut rows);
        }
        UiFill::RadialGradient { .. } => {
            for (axis, label) in [(0usize, "Center X"), (1, "Center Y")] {
                let dv = drag_value(commands, &fonts.ui, "", (210, 210, 220), 0.5, 0.01);
                commands.entity(dv).insert(DragRange { min: 0.0, max: 1.0 });
                bind_2way(
                    commands,
                    dv,
                    move |w| match w.get::<UiFill>(entity) {
                        Some(UiFill::RadialGradient { center, .. }) => center[axis],
                        _ => 0.5,
                    },
                    move |w, v: &f32| {
                        if let Some(mut f) = w.get_mut::<UiFill>(entity) {
                            if let UiFill::RadialGradient { center, .. } = &mut *f {
                                center[axis] = *v;
                            }
                        }
                    },
                );
                rows.push(inspector_row(commands, &fonts.ui, label, dv));
            }
            build_gradient_stops(commands, fonts, entity, fill, &mut rows);
        }
        UiFill::None => {}
    }

    for (i, r) in rows.iter().enumerate() {
        commands.entity(*r).insert(BackgroundColor(inspector_stripe(i)));
    }
    commands.entity(root).add_children(&rows);
}

/// Per-stop Pos + Color rows and the Add Stop / Remove buttons (pushed onto `rows`).
fn build_gradient_stops(commands: &mut Commands, fonts: &EmberFonts, entity: Entity, fill: &UiFill, rows: &mut Vec<Entity>) {
    let n = fill_stops_len(fill);
    for i in 0..n {
        let pos = drag_value(commands, &fonts.ui, "", (210, 210, 220), 0.0, 0.01);
        commands.entity(pos).insert(DragRange { min: 0.0, max: 1.0 });
        bind_2way(
            commands,
            pos,
            move |w| {
                w.get::<UiFill>(entity)
                    .and_then(|f| match f {
                        UiFill::LinearGradient { stops, .. } | UiFill::RadialGradient { stops, .. } => stops.get(i).map(|s| s.position),
                        _ => None,
                    })
                    .unwrap_or(0.0)
            },
            move |w, v: &f32| {
                if let Some(mut f) = w.get_mut::<UiFill>(entity) {
                    if let Some(stops) = fill_stops_mut(&mut f) {
                        if let Some(s) = stops.get_mut(i) {
                            s.position = *v;
                        }
                    }
                }
            },
        );
        rows.push(inspector_row(commands, &fonts.ui, &format!("Stop {} Pos", i + 1), pos));

        let color = color_field_rgba(
            commands,
            move |w| {
                w.get::<UiFill>(entity)
                    .and_then(|f| match f {
                        UiFill::LinearGradient { stops, .. } | UiFill::RadialGradient { stops, .. } => {
                            stops.get(i).map(|s| s.color.to_srgba().to_f32_array())
                        }
                        _ => None,
                    })
                    .unwrap_or([0.0; 4])
            },
            move |w, a: [f32; 4]| {
                if let Some(mut f) = w.get_mut::<UiFill>(entity) {
                    if let Some(stops) = fill_stops_mut(&mut f) {
                        if let Some(s) = stops.get_mut(i) {
                            s.color = Color::srgba(a[0], a[1], a[2], a[3]);
                        }
                    }
                }
            },
        );
        rows.push(inspector_row(commands, &fonts.ui, &format!("Stop {} Color", i + 1), color));
    }

    // Add Stop / Remove.
    let add = icon_label_button(commands, fonts, "plus", "Add Stop");
    commands.entity(add).insert(FillAddStop { entity });
    let mut btns = vec![add];
    if n > 2 {
        let rem = icon_label_button(commands, fonts, "minus", "Remove");
        commands.entity(rem).insert(FillRemoveStop { entity });
        btns.push(rem);
    }
    let btn_group = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), margin: UiRect::top(Val::Px(4.0)), ..default() })
        .id();
    commands.entity(btn_group).add_children(&btns);
    rows.push(btn_group);
}

fn fill_add_stop_click(q: Query<(&Interaction, &FillAddStop), Changed<Interaction>>, mut commands: Commands) {
    for (interaction, b) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let e = b.entity;
        commands.queue(move |w: &mut World| {
            if let Some(mut f) = w.get_mut::<UiFill>(e) {
                if let Some(stops) = fill_stops_mut(&mut f) {
                    stops.push(GradientStop { position: 1.0, color: Color::WHITE });
                }
            }
        });
    }
}

fn fill_remove_stop_click(q: Query<(&Interaction, &FillRemoveStop), Changed<Interaction>>, mut commands: Commands) {
    for (interaction, b) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let e = b.entity;
        commands.queue(move |w: &mut World| {
            if let Some(mut f) = w.get_mut::<UiFill>(e) {
                if let Some(stops) = fill_stops_mut(&mut f) {
                    if stops.len() > 2 {
                        stops.pop();
                    }
                }
            }
        });
    }
}

// ── Interaction (UiInteractionStyle + UiTransition) ──────────────────────────

const CURSOR_LABELS: [&str; 10] =
    ["Default", "Pointer", "Text", "Grab", "Grabbing", "Not Allowed", "Crosshair", "EW Resize", "NS Resize", "Move"];

fn istate(is: &UiInteractionStyle, idx: usize) -> &UiStateStyle {
    match idx {
        0 => &is.normal,
        1 => &is.hovered,
        2 => &is.pressed,
        _ => &is.disabled,
    }
}

fn istate_mut(is: &mut UiInteractionStyle, idx: usize) -> &mut UiStateStyle {
    match idx {
        0 => &mut is.normal,
        1 => &mut is.hovered,
        2 => &mut is.pressed,
        _ => &mut is.disabled,
    }
}

/// Whether a state's override (by kind) is currently enabled.
fn override_present(s: &UiStateStyle, kind: u8) -> bool {
    match kind {
        0 => s.fill.is_some(),
        1 => s.stroke.is_some(),
        2 => s.opacity.is_some(),
        3 => s.text_color.is_some(),
        4 => s.text_size.is_some(),
        5 => s.cursor.is_some(),
        _ => s.scale.is_some(),
    }
}

/// Enable (with the egui default) or clear a state's override.
fn toggle_override(s: &mut UiStateStyle, kind: u8, on: bool) {
    match kind {
        0 => s.fill = on.then(|| UiFill::Solid(Color::srgba(0.3, 0.3, 0.3, 1.0))),
        1 => s.stroke = on.then(|| UiStroke::new(Color::WHITE, 1.0)),
        2 => s.opacity = on.then_some(1.0),
        3 => s.text_color = on.then_some(Color::WHITE),
        4 => s.text_size = on.then_some(14.0),
        5 => s.cursor = on.then_some(UiCursor::Pointer),
        _ => s.scale = on.then_some(1.0),
    }
}

fn cursor_to_idx(c: &UiCursor) -> usize {
    match c {
        UiCursor::Default => 0,
        UiCursor::Pointer => 1,
        UiCursor::Text => 2,
        UiCursor::Grab => 3,
        UiCursor::Grabbing => 4,
        UiCursor::NotAllowed => 5,
        UiCursor::Crosshair => 6,
        UiCursor::EwResize => 7,
        UiCursor::NsResize => 8,
        UiCursor::Move => 9,
    }
}

fn idx_to_cursor(i: usize) -> UiCursor {
    match i {
        1 => UiCursor::Pointer,
        2 => UiCursor::Text,
        3 => UiCursor::Grab,
        4 => UiCursor::Grabbing,
        5 => UiCursor::NotAllowed,
        6 => UiCursor::Crosshair,
        7 => UiCursor::EwResize,
        8 => UiCursor::NsResize,
        9 => UiCursor::Move,
        _ => UiCursor::Default,
    }
}

fn istate_f32(s: &UiStateStyle, kind: u8) -> Option<f32> {
    match kind {
        2 => s.opacity,
        4 => s.text_size,
        _ => s.scale,
    }
}

fn set_istate_f32(s: &mut UiStateStyle, kind: u8, v: f32) {
    match kind {
        2 => s.opacity = Some(v),
        4 => s.text_size = Some(v),
        _ => s.scale = Some(v),
    }
}

/// Native drawer for `UiInteractionStyle` — four state sections (Normal / Hovered
/// / Pressed / Disabled), each with per-override checkbox + value editor, plus a
/// Transition (duration) section. Mirrors `render_interaction_inspector`.
fn interaction_native(world: &mut World, entity: Entity) -> Entity {
    inspector_body(world, move |commands, fonts| {
        let col = commands
            .spawn(Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                padding: UiRect::all(Val::Px(2.0)),
                ..default()
            })
            .id();

        let mut children: Vec<Entity> = Vec::new();
        let mut stripe = 0usize;
        for (state_idx, name) in ["Normal", "Hovered", "Pressed", "Disabled"].iter().enumerate() {
            children.push(section_header(commands, fonts, name));
            for kind in 0u8..7 {
                let row = override_row(commands, fonts, entity, state_idx, kind);
                commands.entity(row).insert(BackgroundColor(inspector_stripe(stripe)));
                stripe += 1;
                children.push(row);
            }
        }

        children.push(section_header(commands, fonts, "Transition"));
        let trow = transition_row(commands, fonts, entity);
        commands.entity(trow).insert(BackgroundColor(inspector_stripe(stripe)));
        children.push(trow);

        commands.entity(col).add_children(&children);
        col
    })
}

fn section_header(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    let h = commands
        .spawn(Node { margin: UiRect { top: Val::Px(6.0), bottom: Val::Px(1.0), ..default() }, ..default() })
        .id();
    let t = commands
        .spawn((Text::new(label), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary()))))
        .id();
    commands.entity(h).add_child(t);
    h
}

fn override_checkbox(commands: &mut Commands, entity: Entity, state_idx: usize, kind: u8) -> Entity {
    let cb = checkbox(commands, false);
    bind_2way(
        commands,
        cb,
        move |w| w.get::<UiInteractionStyle>(entity).map(|is| override_present(istate(is, state_idx), kind)).unwrap_or(false),
        move |w, on: &bool| {
            if let Some(mut is) = w.get_mut::<UiInteractionStyle>(entity) {
                toggle_override(istate_mut(&mut is, state_idx), kind, *on);
            }
        },
    );
    cb
}

fn override_row(commands: &mut Commands, fonts: &EmberFonts, entity: Entity, state_idx: usize, kind: u8) -> Entity {
    let label = ["Fill", "Stroke", "Opacity", "Text Color", "Text Size", "Cursor", "Scale"][kind as usize];
    let ctrl = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), flex_grow: 1.0, ..default() })
        .id();
    let mut items = vec![override_checkbox(commands, entity, state_idx, kind)];

    match kind {
        0 => {
            // Fill: solid color editor (when Solid) + a muted "gradient" tag (when gradient).
            let color = color_field_rgba(
                commands,
                move |w| match w.get::<UiInteractionStyle>(entity).map(|is| istate(is, state_idx).fill.clone()) {
                    Some(Some(UiFill::Solid(c))) => c.to_srgba().to_f32_array(),
                    _ => [0.3, 0.3, 0.3, 1.0],
                },
                move |w, a: [f32; 4]| {
                    if let Some(mut is) = w.get_mut::<UiInteractionStyle>(entity) {
                        if let Some(UiFill::Solid(c)) = &mut istate_mut(&mut is, state_idx).fill {
                            *c = Color::srgba(a[0], a[1], a[2], a[3]);
                        }
                    }
                },
            );
            bind_display(commands, color, move |w| {
                w.get::<UiInteractionStyle>(entity).map(|is| matches!(istate(is, state_idx).fill, Some(UiFill::Solid(_)))).unwrap_or(false)
            });
            items.push(color);
            let grad = commands
                .spawn((Text::new("gradient"), ui_font(&fonts.ui, 10.0), TextColor(rgb(text_muted()))))
                .id();
            bind_display(commands, grad, move |w| {
                w.get::<UiInteractionStyle>(entity)
                    .map(|is| matches!(istate(is, state_idx).fill, Some(UiFill::LinearGradient { .. } | UiFill::RadialGradient { .. })))
                    .unwrap_or(false)
            });
            items.push(grad);
        }
        1 => {
            // Stroke: color + width.
            let color = color_field_rgba(
                commands,
                move |w| w.get::<UiInteractionStyle>(entity).and_then(|is| istate(is, state_idx).stroke.as_ref().map(|s| s.color.to_srgba().to_f32_array())).unwrap_or([1.0; 4]),
                move |w, a: [f32; 4]| {
                    if let Some(mut is) = w.get_mut::<UiInteractionStyle>(entity) {
                        if let Some(s) = &mut istate_mut(&mut is, state_idx).stroke {
                            s.color = Color::srgba(a[0], a[1], a[2], a[3]);
                        }
                    }
                },
            );
            bind_display(commands, color, move |w| w.get::<UiInteractionStyle>(entity).map(|is| istate(is, state_idx).stroke.is_some()).unwrap_or(false));
            items.push(color);
            let width = drag_value(commands, &fonts.ui, "", (210, 210, 220), 1.0, 0.5);
            commands.entity(width).insert(DragRange { min: 0.0, max: 50.0 });
            bind_2way(
                commands,
                width,
                move |w| w.get::<UiInteractionStyle>(entity).and_then(|is| istate(is, state_idx).stroke.as_ref().map(|s| s.width)).unwrap_or(1.0),
                move |w, v: &f32| {
                    if let Some(mut is) = w.get_mut::<UiInteractionStyle>(entity) {
                        if let Some(s) = &mut istate_mut(&mut is, state_idx).stroke {
                            s.width = *v;
                        }
                    }
                },
            );
            bind_display(commands, width, move |w| w.get::<UiInteractionStyle>(entity).map(|is| istate(is, state_idx).stroke.is_some()).unwrap_or(false));
            items.push(width);
        }
        3 => {
            // Text Color.
            let color = color_field_rgba(
                commands,
                move |w| w.get::<UiInteractionStyle>(entity).and_then(|is| istate(is, state_idx).text_color.map(|c| c.to_srgba().to_f32_array())).unwrap_or([1.0; 4]),
                move |w, a: [f32; 4]| {
                    if let Some(mut is) = w.get_mut::<UiInteractionStyle>(entity) {
                        let s = istate_mut(&mut is, state_idx);
                        if s.text_color.is_some() {
                            s.text_color = Some(Color::srgba(a[0], a[1], a[2], a[3]));
                        }
                    }
                },
            );
            bind_display(commands, color, move |w| w.get::<UiInteractionStyle>(entity).map(|is| istate(is, state_idx).text_color.is_some()).unwrap_or(false));
            items.push(color);
        }
        5 => {
            // Cursor.
            let dd = dropdown(commands, fonts, &CURSOR_LABELS, 0);
            bind_2way(
                commands,
                dd,
                move |w| w.get::<UiInteractionStyle>(entity).and_then(|is| istate(is, state_idx).cursor.as_ref().map(cursor_to_idx)).unwrap_or(0),
                move |w, v: &usize| {
                    if let Some(mut is) = w.get_mut::<UiInteractionStyle>(entity) {
                        let s = istate_mut(&mut is, state_idx);
                        if s.cursor.is_some() {
                            s.cursor = Some(idx_to_cursor(*v));
                        }
                    }
                },
            );
            bind_display(commands, dd, move |w| w.get::<UiInteractionStyle>(entity).map(|is| istate(is, state_idx).cursor.is_some()).unwrap_or(false));
            items.push(dd);
        }
        // Opacity (2), Text Size (4), Scale (6) — plain f32 sliders.
        _ => {
            let (init, min, max, step) = match kind {
                2 => (1.0, 0.0, 1.0, 0.01),
                4 => (14.0, 1.0, 200.0, 0.5),
                _ => (1.0, 0.1, 5.0, 0.01),
            };
            let dv = drag_value(commands, &fonts.ui, "", (210, 210, 220), init, step);
            commands.entity(dv).insert(DragRange { min, max });
            bind_2way(
                commands,
                dv,
                move |w| w.get::<UiInteractionStyle>(entity).and_then(|is| istate_f32(istate(is, state_idx), kind)).unwrap_or(init),
                move |w, v: &f32| {
                    if let Some(mut is) = w.get_mut::<UiInteractionStyle>(entity) {
                        let s = istate_mut(&mut is, state_idx);
                        if istate_f32(s, kind).is_some() {
                            set_istate_f32(s, kind, *v);
                        }
                    }
                },
            );
            bind_display(commands, dv, move |w| w.get::<UiInteractionStyle>(entity).map(|is| istate_f32(istate(is, state_idx), kind).is_some()).unwrap_or(false));
            items.push(dv);
        }
    }

    commands.entity(ctrl).add_children(&items);
    inspector_row(commands, &fonts.ui, label, ctrl)
}

/// Transition row: an enable checkbox (insert/remove `UiTransition`) + a duration slider.
fn transition_row(commands: &mut Commands, fonts: &EmberFonts, entity: Entity) -> Entity {
    let ctrl = commands
        .spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), flex_grow: 1.0, ..default() })
        .id();
    let cb = checkbox(commands, false);
    bind_2way(
        commands,
        cb,
        move |w| w.get::<UiTransition>(entity).is_some(),
        move |w, on: &bool| {
            if *on {
                if w.get::<UiTransition>(entity).is_none() {
                    w.entity_mut(entity).insert(UiTransition { duration: 0.15 });
                }
            } else {
                w.entity_mut(entity).remove::<UiTransition>();
            }
        },
    );
    let dur = drag_value(commands, &fonts.ui, "", (210, 210, 220), 0.15, 0.01);
    commands.entity(dur).insert(DragRange { min: 0.0, max: 5.0 });
    bind_2way(
        commands,
        dur,
        move |w| w.get::<UiTransition>(entity).map(|t| t.duration).unwrap_or(0.15),
        move |w, v: &f32| {
            if let Some(mut t) = w.get_mut::<UiTransition>(entity) {
                t.duration = *v;
            }
        },
    );
    bind_display(commands, dur, move |w| w.get::<UiTransition>(entity).is_some());
    commands.entity(ctrl).add_children(&[cb, dur]);
    inspector_row(commands, &fonts.ui, "Duration", ctrl)
}
