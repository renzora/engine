//! Live font application — turns the persisted [`UiFont`] / [`MonoFont`]
//! choices into renderable `FontSource`s and pushes them through every text
//! entity already on screen, so the whole editor restyles without a rebuild.

use bevy::prelude::*;

use renzora_editor_framework::{EditorSettings, MonoFont, UiFont};
use renzora_ember::font::EmberFonts;

use crate::state::OverlayState;

/// Map the persisted [`UiFont`] choice to a renderable [`FontSource`]. Built-ins
/// and custom project fonts resolve by family name via Parley's system-font
/// discovery (`system_font_discovery` feature); `NotoSans` is the embedded
/// default already loaded as a handle.
fn ui_font_source(
    choice: &UiFont,
    fonts: &EmberFonts,
    registry: &renzora_ember::font::FontRegistry,
) -> bevy::text::FontSource {
    use bevy::text::FontSource;
    match choice {
        UiFont::System => FontSource::SystemUi,
        UiFont::NotoSans => fonts.default_ui.clone(),
        UiFont::Roboto => FontSource::Family("Roboto".into()),
        UiFont::OpenSans => FontSource::Family("Open Sans".into()),
        // A project `fonts/` file: use its loaded handle; fall back to a family
        // lookup if the registry hasn't scanned it (yet) or it's a system name.
        UiFont::Custom(name) => registry
            .resolve(name)
            .unwrap_or_else(|| FontSource::Family(name.as_str().into())),
    }
}

/// As [`ui_font_source`], for the monospace/code font.
fn mono_font_source(
    choice: &MonoFont,
    fonts: &EmberFonts,
    registry: &renzora_ember::font::FontRegistry,
) -> bevy::text::FontSource {
    use bevy::text::FontSource;
    match choice {
        MonoFont::JetBrainsMono => fonts.default_mono.clone(),
        MonoFont::FiraCode => FontSource::Family("Fira Code".into()),
        MonoFont::SourceCodePro => FontSource::Family("Source Code Pro".into()),
        MonoFont::Custom(name) => registry
            .resolve(name)
            .unwrap_or_else(|| FontSource::Family(name.as_str().into())),
    }
}

/// When the font registry changes (a font added to / removed from the project
/// `fonts/` folder), mark the settings overlay dirty so an open panel rebuilds
/// and the font dropdowns re-list. Harmless when the panel is closed.
pub(crate) fn refresh_settings_on_font_change(
    registry: Res<renzora_ember::font::FontRegistry>,
    mut state: ResMut<OverlayState>,
) {
    if registry.is_changed() {
        state.dirty = true;
    }
}

/// Apply the UI/code font choices from [`EditorSettings`] to [`EmberFonts`],
/// live-rewriting every already-spawned text entity that still uses the old
/// source so the whole editor restyles without a rebuild. UI and mono text are
/// kept distinct by comparing against the *current* `EmberFonts.ui` / `.mono`,
/// so icon (phosphor) text and 3D gizmo stroke text are never touched.
pub(crate) fn apply_font_settings(
    settings: Res<EditorSettings>,
    registry: Res<renzora_ember::font::FontRegistry>,
    fonts: Option<ResMut<EmberFonts>>,
    // The theme font override applied last run, so a theme switching its font
    // on/off re-triggers the swap even when settings/registry are unchanged.
    mut last_theme_ui: Local<Option<bevy::text::FontSource>>,
    mut text_q: Query<&mut TextFont>,
) {
    let Some(mut fonts) = fonts else {
        return;
    };
    // A folder theme can override the UI font; it wins over the user's setting
    // while active. Reverts to the setting when the theme clears it (`None`).
    let theme_ui = renzora_ember::font::theme_ui_font();
    // Re-apply when the choice changes, when the registry changes (a project font
    // may have just finished loading, so the chosen name now resolves), or when
    // the theme font override flips. The no-op early-outs below keep extra runs
    // harmless.
    if !settings.is_changed() && !registry.is_changed() && *last_theme_ui == theme_ui {
        return;
    }
    *last_theme_ui = theme_ui.clone();
    // Compute both before mutating so the immutable borrow of `fonts` is done.
    let new_ui = theme_ui.unwrap_or_else(|| ui_font_source(&settings.ui_font, &fonts, &registry));
    let new_mono = mono_font_source(&settings.mono_font, &fonts, &registry);

    if new_ui != fonts.ui {
        let old = std::mem::replace(&mut fonts.ui, new_ui.clone());
        for mut tf in &mut text_q {
            if tf.font == old {
                tf.font = new_ui.clone();
            }
        }
    }
    if new_mono != fonts.mono {
        let old = std::mem::replace(&mut fonts.mono, new_mono.clone());
        for mut tf in &mut text_q {
            if tf.font == old {
                tf.font = new_mono.clone();
            }
        }
    }

    // Font Size: a global multiplier relative to the 14px design reference (the
    // size the `ui_font(..)` call sites were tuned at; the default setting is
    // 17 → ~1.21x). New text picks it up via `ui_font` (which reads the global
    // scale); existing UI/mono text is rescaled here by the ratio of the change
    // so sizes track the slider.
    let new_scale = (settings.font_size / 14.0).clamp(0.1, 4.0);
    let old_scale = renzora_ember::font::ui_font_scale();
    if (new_scale - old_scale).abs() > f32::EPSILON {
        let ratio = new_scale / old_scale;
        renzora_ember::font::set_ui_font_scale(new_scale);
        let ui_src = fonts.ui.clone();
        let mono_src = fonts.mono.clone();
        for mut tf in &mut text_q {
            // Only editor text built through `ui_font` (UI or code font) — the
            // source match excludes icon glyphs (phosphor) and 3D gizmo text.
            if tf.font == ui_src || tf.font == mono_src {
                if let bevy::text::FontSize::Px(px) = &mut tf.font_size {
                    *px *= ratio;
                }
            }
        }
    }
}
