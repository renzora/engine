//! The animated input hint row under each step's text: large Phosphor mouse /
//! keyboard glyphs plus keycap "chips", animated so a right-click + key-press
//! reads at a glance. Glyphs either pulse (press/click) or slide (drag), driven
//! by [`tick_hints`] reading `Res<Time>` — the same per-frame pattern ember's
//! `spinner_anim` uses.

use bevy::prelude::*;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::theme::{accent, header_bg, rgb, text_primary};

use crate::steps::{Hint, HintAnim};

/// Tags a glyph node so [`tick_hints`] animates it.
#[derive(Component)]
pub struct HintGlyph {
    anim: HintAnim,
    phase: f32,
}

/// Build the hint row as a child of `parent`.
pub fn build_hint(commands: &mut Commands, fonts: &EmberFonts, parent: Entity, hint: &Hint) {
    let row = commands
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            margin: UiRect::top(Val::Px(6.0)),
            ..default()
        })
        .id();

    let mut phase = 0.0f32;
    for name in hint.icons {
        let glyph = icon_text(commands, &fonts.phosphor, name, accent(), 34.0);
        commands.entity(glyph).insert((
            // Relative so a drag's horizontal offset doesn't reflow siblings.
            Node {
                position_type: PositionType::Relative,
                align_items: AlignItems::Center,
                ..default()
            },
            HintGlyph { anim: hint.anim, phase },
        ));
        commands.entity(row).add_child(glyph);
        phase += 0.7;
    }

    for key in hint.keys {
        let chip = key_chip(commands, fonts, key);
        commands.entity(row).add_child(chip);
    }

    commands.entity(parent).add_child(row);
}

/// A single keycap chip (e.g. `W`, `Shift`, `drag`).
fn key_chip(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    let chip = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(9.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                min_width: Val::Px(20.0),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(rgb(header_bg())),
            BorderColor::all(rgb(accent())),
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    commands.entity(chip).add_child(text);
    chip
}

/// Pulse (alpha breathe) or drag (horizontal slide) every live hint glyph.
pub fn tick_hints(time: Res<Time>, mut glyphs: Query<(&HintGlyph, &mut Node, Option<&mut TextColor>)>) {
    let t = time.elapsed_secs();
    let (ar, ag, ab) = accent();
    for (g, mut node, color) in &mut glyphs {
        match g.anim {
            HintAnim::Pulse => {
                if let Some(mut c) = color {
                    let a = 0.45 + 0.55 * (t * 4.0 + g.phase).sin().abs();
                    c.0 = Color::srgba(ar as f32 / 255.0, ag as f32 / 255.0, ab as f32 / 255.0, a);
                }
            }
            HintAnim::Drag => {
                node.left = Val::Px((t * 3.0 + g.phase).sin() * 6.0);
            }
        }
    }
}
