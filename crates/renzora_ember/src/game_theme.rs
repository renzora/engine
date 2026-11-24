//! A game's UI theme: the project's `ui_theme`, applied to every ember widget
//! the game draws.
//!
//! A theme is the same file the editor's own theme menu uses (`Dark`, `Light`,
//! or `themes/<Name>.toml` / `themes/<Name>/theme.toml` in the project), mapped
//! through the same [`palette_from_theme`], so a theme a user builds in the
//! editor's Theme tab looks the same on their game's menus.
//!
//! Two jobs, kept apart because the editor needs one and not the other:
//!
//! - [`GameThemes`] lists the themes a game can switch to. Kept in the editor
//!   too, so a `<dropdown setting="ui_theme" />` in a template it is showing
//!   offers the same list the game will.
//! - [`apply_game_theme`] applies `ui_theme`, **in a game only**. The palette is
//!   one process-wide value and the editor's chrome is drawn from it, so in the
//!   editor it belongs to the editor's own theme; templates there show the
//!   editor's colours.
//!
//! Widgets take their colours when they are spawned, so a switch rebuilds the
//! game's markup UI. Widgets a script built itself keep the colours they were
//! built with until the script builds them again.

use bevy::prelude::*;
use renzora_project_api::{CurrentProject, EditorSession, ProjectThemeSources};
use renzora_theme::Theme;

use crate::theme::{palette_from_theme, set_palette, set_syntax_palette, syntax_from_theme};

/// The built-in themes, which have no file.
const BUILT_IN: [&str; 2] = ["Dark", "Light"];

/// Every theme the game can be drawn in, by name: the built-ins first, then the
/// project's own, sorted.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct GameThemes {
    pub names: Vec<String>,
}

/// One project theme file: its name and its TOML.
struct ThemeFile {
    name: String,
    toml: String,
}

/// The project's theme files: from the archive when the game was packed,
/// otherwise from `<project>/themes/`.
fn theme_files(project: &CurrentProject, packed: Option<&ProjectThemeSources>) -> Vec<ThemeFile> {
    if let Some(packed) = packed.filter(|p| !p.files.is_empty()) {
        return packed
            .files
            .iter()
            .filter_map(|(key, toml)| Some(ThemeFile { name: name_from_key(key)?, toml: toml.clone() }))
            .collect();
    }
    disk_theme_files(&project.path.join("themes"))
}

/// A theme's name from its archive key: `themes/Name.toml` or
/// `themes/Name/theme.toml`.
fn name_from_key(key: &str) -> Option<String> {
    let rest = key.strip_prefix("themes/")?;
    match rest.split_once('/') {
        Some((folder, "theme.toml")) => Some(folder.to_string()),
        Some(_) => None,
        None => rest.strip_suffix(".toml").map(str::to_string),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn disk_theme_files(dir: &std::path::Path) -> Vec<ThemeFile> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<ThemeFile> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let (name, file) = if path.is_dir() {
            (path.file_name().and_then(|n| n.to_str()).map(str::to_string), path.join("theme.toml"))
        } else if path.extension().and_then(|e| e.to_str()) == Some("toml") {
            (path.file_stem().and_then(|n| n.to_str()).map(str::to_string), path.clone())
        } else {
            continue;
        };
        let (Some(name), Ok(toml)) = (name, std::fs::read_to_string(&file)) else {
            continue;
        };
        // A folder wins over a flat file of the same name, as in the editor.
        match out.iter_mut().find(|f| f.name == name) {
            Some(existing) if path.is_dir() => existing.toml = toml,
            Some(_) => {}
            None => out.push(ThemeFile { name, toml }),
        }
    }
    out
}

#[cfg(target_arch = "wasm32")]
fn disk_theme_files(_dir: &std::path::Path) -> Vec<ThemeFile> {
    Vec::new()
}

/// The list, from the files.
fn names(files: &[ThemeFile]) -> Vec<String> {
    let mut own: Vec<String> = files
        .iter()
        .map(|f| f.name.clone())
        .filter(|n| !BUILT_IN.contains(&n.as_str()))
        .collect();
    own.sort();
    own.dedup();
    BUILT_IN.iter().map(|s| s.to_string()).chain(own).collect()
}

/// A theme by name, with its per-widget style sections (`None` for a built-in).
fn resolve(name: &str, files: &[ThemeFile]) -> Option<(Theme, Option<String>)> {
    match name {
        "Dark" => Some((Theme::dark(), None)),
        "Light" => Some((Theme::light(), None)),
        _ => {
            let file = files.iter().find(|f| f.name == name)?;
            match toml::from_str::<Theme>(&file.toml) {
                Ok(theme) => Some((theme, Some(file.toml.clone()))),
                Err(e) => {
                    warn!("[theme] `{name}` could not be read: {e}");
                    None
                }
            }
        }
    }
}

fn refresh_game_themes(world: &mut World) {
    let Some(project) = world.get_resource::<CurrentProject>() else { return };
    let files = theme_files(project, world.get_resource::<ProjectThemeSources>());
    let list = GameThemes { names: names(&files) };
    if world.get_resource::<GameThemes>() != Some(&list) {
        world.insert_resource(list);
    }
}

/// Apply `ui_theme` when it changes, in a game. The first application happens
/// before or just after the UI builds, and a later one is a switch; either way
/// anything already built is rebuilt in the new colours.
fn apply_game_theme(world: &mut World, mut applied: Local<Option<Option<String>>>) {
    if world.get_resource::<EditorSession>().is_some_and(|s| s.is_editor()) {
        return;
    }
    let Some(project) = world.get_resource::<CurrentProject>() else { return };
    let wanted = project.config.ui_theme.clone();
    if applied.as_ref() == Some(&wanted) {
        return;
    }
    // Unset on the first run: ember's default already is the dark theme.
    let first = applied.is_none();
    *applied = Some(wanted.clone());
    if first && wanted.is_none() {
        return;
    }

    let name = wanted.unwrap_or_else(|| "Dark".to_string());
    let files = theme_files(project, world.get_resource::<ProjectThemeSources>());
    let Some((theme, styles)) = resolve(&name, &files) else {
        warn!("[theme] the game has no theme called `{name}`; keeping the current one");
        return;
    };
    set_palette(palette_from_theme(&theme));
    set_syntax_palette(syntax_from_theme(&theme));
    let style = styles
        .and_then(|toml| crate::style::Theme::from_toml(&toml).ok())
        .unwrap_or_default();
    world.insert_resource(style);
    #[cfg(feature = "game_ui")]
    world.insert_resource(ui_theme_from(&name, &theme));
    info!("[theme] the game's UI theme is `{name}`");

    #[cfg(feature = "game_ui")]
    crate::markup::template::rebuild_all(world);
}

/// The game widget kit's `UiTheme` (health bars, `UiThemed` nodes) from the
/// same theme file, so both kits a game can draw with agree. Colours a theme
/// file has no word for (a health bar's, a modal's backdrop) keep the base's.
#[cfg(feature = "game_ui")]
fn ui_theme_from(name: &str, t: &Theme) -> crate::game_ui::components::UiTheme {
    use crate::game_ui::components::UiTheme;
    use crate::theme::{mix, rgb};
    let c = |tc: &renzora_theme::ThemeColor| {
        let [r, g, b, a] = tc.0;
        Color::srgba_u8(r, g, b, a)
    };
    let three = |tc: &renzora_theme::ThemeColor| {
        let [r, g, b, _] = tc.0;
        (r, g, b)
    };
    // A theme file does not say whether it is light; its panel does.
    let (r, g, b) = three(&t.surfaces.panel);
    let light = u32::from(r) + u32::from(g) + u32::from(b) > 3 * 128;
    let base = if light { UiTheme::light() } else { UiTheme::dark() };
    let accent = three(&t.semantic.accent);
    UiTheme {
        name: name.to_string(),
        surface: c(&t.surfaces.panel),
        surface_raised: c(&t.panels.item_bg),
        surface_overlay: c(&t.surfaces.popup),
        text_primary: c(&t.text.primary),
        text_secondary: c(&t.text.secondary),
        text_muted: c(&t.text.muted),
        text_on_accent: c(&t.widgets.active_fg),
        accent: rgb(accent),
        accent_hovered: mix(accent, three(&t.text.primary), 0.15),
        accent_pressed: mix(accent, three(&t.surfaces.panel), 0.25),
        success: c(&t.semantic.success),
        warning: c(&t.semantic.warning),
        error: c(&t.semantic.error),
        info: rgb(accent),
        border: c(&t.widgets.border_light),
        track: c(&t.surfaces.faint),
        thumb: c(&t.text.primary),
        progress_fill: rgb(accent),
        toggle_on: rgb(accent),
        toggle_off: c(&t.widgets.border_light),
        scrollbar: c(&t.panels.tree_line),
        tooltip_bg: c(&t.surfaces.popup),
        title_bar: c(&t.surfaces.extreme),
        ..base
    }
}

pub(crate) fn plugin(app: &mut App) {
    app.init_resource::<GameThemes>().add_systems(
        Update,
        (
            refresh_game_themes.run_if(resource_exists_and_changed::<CurrentProject>),
            apply_game_theme.run_if(resource_exists_and_changed::<CurrentProject>),
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str) -> ThemeFile {
        ThemeFile { name: name.into(), toml: String::new() }
    }

    #[test]
    fn archive_keys_name_their_theme() {
        assert_eq!(name_from_key("themes/Neon.toml").as_deref(), Some("Neon"));
        assert_eq!(name_from_key("themes/Neon/theme.toml").as_deref(), Some("Neon"));
        assert_eq!(name_from_key("themes/Neon/rain.wgsl"), None);
        assert_eq!(name_from_key("ui/menu.toml"), None);
    }

    #[test]
    fn built_ins_come_first_and_are_never_repeated() {
        let list = names(&[file("Zebra"), file("Dark"), file("Aqua")]);
        assert_eq!(list, ["Dark", "Light", "Aqua", "Zebra"]);
    }

    #[test]
    fn built_ins_resolve_without_a_file_and_a_missing_theme_does_not() {
        assert!(resolve("Light", &[]).is_some_and(|(_, styles)| styles.is_none()));
        assert!(resolve("Nowhere", &[]).is_none());
    }
}
