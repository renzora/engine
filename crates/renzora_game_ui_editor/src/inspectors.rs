//! Native (ember) inspector drawers for `renzora_game_ui` widget components that
//! can't be a flat declarative field list (grouped controls, conditional UI,
//! dynamic lists). They live here because they need `renzora_ember`, and
//! `game_ui` itself can't depend on ember (the `ember -> hui -> game_ui` cycle).
//! Each mirrors its egui `custom_ui_fn`; egui keeps that one.

use bevy::prelude::*;

use renzora_editor::{AppEditorExt, SplashState};
use renzora_ember::font::{icon_text, EmberFonts};
use renzora_ember::inspector::{color_field_rgba, inspector_body, inspector_row, inspector_stripe};
use renzora_ember::reactive::{bind_2way, bind_bg};
use renzora_ember::theme::{accent, rgb, text_muted};
use renzora_ember::widgets::{drag_value, DragRange};

use renzora_game_ui::components::UiStroke;

pub(crate) fn register(app: &mut App) {
    app.register_native_inspector_ui("ui_stroke", stroke_native);
    app.add_systems(Update, stroke_side_click.run_if(in_state(SplashState::Editor)));
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
