//! Icon library — `<icon name="gear" font_size="20" font_color="#fff">`.
//!
//! Backed by the Phosphor icon font (MIT), embedded in the binary so icons
//! work in any project with no asset setup. The loader stamps an [`Icon`] on
//! `<icon>` elements; [`apply_icons`] resolves the name to a glyph and renders
//! it with the Phosphor font once it's loaded.
//!
//! Names are kebab-case (`arrow-down`, `magnifying-glass`); `_` is accepted as
//! a separator too. See `phosphor_map.rs` for the full list (~1500 icons).

use bevy::prelude::*;
use bevy::text::Font;

use crate::phosphor_map::icon_glyph;

/// The embedded Phosphor TTF (regular weight).
const PHOSPHOR_TTF: &[u8] = include_bytes!("../embedded/phosphor.ttf");

/// Handle to the loaded Phosphor font, set at startup.
#[derive(Resource)]
pub struct PhosphorFont(pub Handle<Font>);

/// Stamped by the loader on `<icon>` elements. Resolved by [`apply_icons`].
#[derive(Component)]
pub struct Icon {
    pub name: String,
    pub size: f32,
    pub color: Option<Color>,
    /// Set once the glyph + font have been applied.
    pub resolved: bool,
}

impl Icon {
    pub fn new(name: String, size: f32, color: Option<Color>) -> Self {
        Self {
            name,
            size,
            color,
            resolved: false,
        }
    }
}

fn load_phosphor(mut fonts: ResMut<Assets<Font>>, mut commands: Commands) {
    match Font::try_from_bytes(PHOSPHOR_TTF.to_vec()) {
        Ok(font) => {
            let handle = fonts.add(font);
            commands.insert_resource(PhosphorFont(handle));
        }
        Err(e) => warn!("renzora_hui: failed to load embedded Phosphor font: {e}"),
    }
}

/// Resolve `<icon>` entities: look up the glyph, render it in the Phosphor
/// font. Runs until each icon is resolved (waits for the font to load).
fn apply_icons(
    font: Option<Res<PhosphorFont>>,
    mut icons: Query<(Entity, &mut Icon)>,
    mut commands: Commands,
) {
    let Some(font) = font else { return };
    for (entity, mut icon) in &mut icons {
        if icon.resolved {
            continue;
        }
        let glyph = icon_glyph(&icon.name).unwrap_or('\u{E4C6}'); // fallback: "question"
        let mut ec = commands.entity(entity);
        ec.insert(Text::new(glyph.to_string()));
        ec.insert(TextFont {
            font: font.0.clone(),
            font_size: icon.size,
            ..default()
        });
        if let Some(c) = icon.color {
            ec.insert(TextColor(c));
        }
        icon.resolved = true;
    }
}

pub fn plugin(app: &mut App) {
    app.add_systems(Startup, load_phosphor)
        .add_systems(Update, apply_icons);
}
