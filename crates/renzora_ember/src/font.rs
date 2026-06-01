//! Fonts + text/icon helpers shared by every ember component and the editor
//! chrome. Noto Sans (proportional) is embedded; the Phosphor icon font is
//! reused from `renzora_hui` (folded into ember later).

use bevy::prelude::*;
use bevy::text::Font;

use crate::theme::rgb;

/// Noto Sans, embedded so it's available regardless of the running app's
/// asset-root.
const NOTO_SANS: &[u8] = include_bytes!("../embedded/NotoSans-Regular.ttf");

/// Slight global down-scale so text matches the editor's size.
const TEXT_SCALE: f32 = 0.92;

/// The fonts ember renders with. `ui` = Noto (proportional); `phosphor` = the
/// Phosphor icon font. Inserted once both are ready.
#[derive(Resource)]
pub struct EmberFonts {
    pub ui: Handle<Font>,
    pub phosphor: Handle<Font>,
}

/// Build [`EmberFonts`] once the Phosphor font (loaded by HUI) is available.
pub(crate) fn load_fonts(
    mut commands: Commands,
    existing: Option<Res<EmberFonts>>,
    mut fonts: ResMut<Assets<Font>>,
    phosphor: Option<Res<renzora_hui::icons::PhosphorFont>>,
) {
    if existing.is_some() {
        return;
    }
    let Some(phosphor) = phosphor else {
        return;
    };
    let ui = match Font::try_from_bytes(NOTO_SANS.to_vec()) {
        Ok(font) => fonts.add(font),
        Err(e) => {
            error!("[ember] failed to load embedded Noto Sans: {e:?}");
            Handle::default()
        }
    };
    commands.insert_resource(EmberFonts {
        ui,
        phosphor: phosphor.0.clone(),
    });
}

/// A `TextFont` in the UI font at the given (pre-scale) size.
pub fn ui_font(font: &Handle<Font>, size: f32) -> TextFont {
    TextFont {
        font: font.clone(),
        font_size: size * TEXT_SCALE,
        ..default()
    }
}

/// An inline Phosphor glyph resolved immediately (real glyph + Phosphor font),
/// so rebuilding the entity doesn't flash a blank frame like the deferred
/// `Icon` component would.
pub fn icon_text(
    commands: &mut Commands,
    phosphor: &Handle<Font>,
    name: &str,
    color: (u8, u8, u8),
    size: f32,
) -> Entity {
    let ch = renzora_hui::phosphor_map::icon_glyph(name).unwrap_or('\u{E4C6}');
    commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                ..default()
            },
            Text::new(ch.to_string()),
            TextFont {
                font: phosphor.clone(),
                font_size: size,
                ..default()
            },
            TextColor(rgb(color)),
            Name::new(format!("icon:{name}")),
        ))
        .id()
}

/// A deferred Phosphor glyph (resolved a frame later by HUI's `apply_icons`) —
/// fine for chrome built once.
pub fn glyph(commands: &mut Commands, name: &str, color: (u8, u8, u8), size: f32) -> Entity {
    commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                ..default()
            },
            renzora_hui::icons::Icon::new(name.to_string(), size, Some(rgb(color))),
            Name::new(format!("glyph:{name}")),
        ))
        .id()
}

/// A padded deferred Phosphor icon button (e.g. chrome action buttons).
pub fn icon_item(commands: &mut Commands, name: &str, color: (u8, u8, u8), size: f32) -> Entity {
    commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                align_items: AlignItems::Center,
                ..default()
            },
            renzora_hui::icons::Icon::new(name.to_string(), size, Some(rgb(color))),
            Name::new(format!("icon:{name}")),
        ))
        .id()
}
