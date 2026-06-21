//! Fonts + text/icon helpers shared by every ember component and the editor
//! chrome. Noto Sans (proportional) is embedded; the Phosphor icon font is
//! reused from `renzora_hui` (folded into ember later).

use bevy::prelude::*;
use bevy::text::{Font, FontSize, FontSource};

use crate::theme::rgb;

/// Noto Sans, embedded so it's available regardless of the running app's
/// asset-root.
const NOTO_SANS: &[u8] = include_bytes!("../embedded/NotoSans-Regular.ttf");

/// JetBrains Mono — the monospace font for the code editor.
const JETBRAINS_MONO: &[u8] = include_bytes!("../embedded/JetBrainsMono-Regular.ttf");

/// Slight global down-scale so text matches the editor's size.
const TEXT_SCALE: f32 = 0.92;

/// The fonts ember renders with. `ui` = the proportional UI font (Noto by
/// default, user-changeable via the theme); `mono` = the monospace font;
/// `phosphor` = the icon font.
///
/// `ui`/`mono` are [`FontSource`]s (not raw handles) so the UI font can be a
/// loaded asset (`Handle`), a system/loaded family by name (`Family`), or a
/// generic category (`SansSerif`, `SystemUi`, …) — Bevy 0.19's Parley backend
/// resolves all three. The default embedded Noto/JetBrains handles are kept in
/// `default_ui`/`default_mono` so "reset to default" never needs to reload them
/// and so the live font-swap can tell UI text apart from icon/mono text.
#[derive(Resource, Clone)]
pub struct EmberFonts {
    pub ui: FontSource,
    pub phosphor: Handle<Font>,
    pub mono: FontSource,
    pub default_ui: FontSource,
    pub default_mono: FontSource,
}

/// Build [`EmberFonts`] once the Phosphor font (loaded by HUI) is available.
pub(crate) fn load_fonts(
    mut commands: Commands,
    existing: Option<Res<EmberFonts>>,
    mut fonts: ResMut<Assets<Font>>,
    phosphor: Option<Res<crate::icons::PhosphorFont>>,
) {
    if existing.is_some() {
        return;
    }
    let Some(phosphor) = phosphor else {
        return;
    };
    // 0.19/Parley: `Font::from_bytes` (Result) → `Font::from_bytes` (infallible).
    let ui = FontSource::Handle(fonts.add(Font::from_bytes(NOTO_SANS.to_vec())));
    let mono = FontSource::Handle(fonts.add(Font::from_bytes(JETBRAINS_MONO.to_vec())));
    commands.insert_resource(EmberFonts {
        ui: ui.clone(),
        phosphor: phosphor.0.clone(),
        mono: mono.clone(),
        default_ui: ui,
        default_mono: mono,
    });
}

/// A `TextFont` in the given font source at the given (pre-scale) size.
///
/// Takes a [`FontSource`] so callers pass `&fonts.ui` / `&fonts.mono` (now
/// source-typed) and the UI font can be swapped to any family/generic at
/// runtime. Existing call sites are unchanged — they already passed `&fonts.ui`.
pub fn ui_font(font: &FontSource, size: f32) -> TextFont {
    TextFont {
        font: font.clone(),
        font_size: FontSize::Px(size * TEXT_SCALE),
        ..default()
    }
}

/// Resolve a Phosphor icon name (e.g. `"caret-down"`) to its glyph char, for
/// binding an icon that changes at runtime. Returns `None` for unknown names.
pub use crate::phosphor_map::icon_glyph;

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
    let ch = crate::phosphor_map::icon_glyph(name).unwrap_or('\u{E4C6}');
    commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                ..default()
            },
            Text::new(ch.to_string()),
            TextFont {
                font: bevy::text::FontSource::Handle(phosphor.clone()),
                font_size: bevy::text::FontSize::Px(size),
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
            crate::icons::Icon::new(name.to_string(), size, Some(rgb(color))),
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
            crate::icons::Icon::new(name.to_string(), size, Some(rgb(color))),
            Name::new(format!("icon:{name}")),
        ))
        .id()
}
