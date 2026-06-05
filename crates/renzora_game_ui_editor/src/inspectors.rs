//! Native (ember) inspector drawers for `renzora_game_ui` widget components that
//! can't be a flat declarative field list (grouped controls, conditional UI,
//! dynamic lists). They live here because they need `renzora_ember`, and
//! `game_ui` itself can't depend on ember (the `ember -> hui -> game_ui` cycle).
//! Each mirrors its egui `custom_ui_fn`; egui keeps that one.

use bevy::ecs::world::CommandQueue;
use bevy::prelude::*;

use renzora_editor::{AppEditorExt, SplashState};
use renzora_ember::font::{icon_text, EmberFonts};
use renzora_ember::inspector::{color_field_rgba, inspector_body, inspector_row, inspector_stripe};
use renzora_ember::reactive::{bind_2way, bind_bg};
use renzora_ember::theme::{accent, rgb, text_muted};
use renzora_ember::widgets::{bind_text_input, drag_value, dropdown, icon_label_button, text_input, DragRange};

use renzora_game_ui::components::{DropdownData, UiStroke};

pub(crate) fn register(app: &mut App) {
    app.register_native_inspector_ui("ui_stroke", stroke_native);
    app.register_native_inspector_ui("ui_dropdown_data", dropdown_native);
    app.add_systems(
        Update,
        (stroke_side_click, rebuild_dropdown, dropdown_add_click, dropdown_remove_click).run_if(in_state(SplashState::Editor)),
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
