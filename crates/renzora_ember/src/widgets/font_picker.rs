//! A font dropdown bound to the live [`FontRegistry`] — pick any available font
//! (the embedded default, a generic family, or a project `fonts/` file) by name.
//! Reusable anywhere a font is selected: Settings, the inspector, a per-component
//! font override, etc.

use bevy::prelude::*;

use crate::font::{EmberFonts, FontRegistry};
use crate::reactive::bind_2way;

/// Build a font picker dropdown.
///
/// - `get` returns the currently-selected font name (matched against the
///   registry; an unknown name falls back to the first entry).
/// - `set` is invoked with the chosen name.
///
/// The option list is a snapshot of the registry at build time, so rebuild the
/// picker to reflect fonts added/removed afterwards (the registry itself updates
/// live via `scan_project_fonts`).
pub fn font_picker<G, S>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    registry: &FontRegistry,
    get: G,
    set: S,
) -> Entity
where
    G: Fn(&World) -> String + Send + Sync + 'static,
    S: Fn(&mut World, String) + Send + Sync + 'static,
{
    let names: Vec<String> = registry.entries.iter().map(|e| e.name.clone()).collect();
    let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
    let dd = crate::widgets::dropdown(commands, fonts, &refs, 0);

    let get_names = names.clone();
    let set_names = names;
    bind_2way::<usize, _, _>(
        commands,
        dd,
        move |w| {
            let cur = get(w);
            get_names.iter().position(|n| *n == cur).unwrap_or(0)
        },
        move |w, &i| {
            if let Some(name) = set_names.get(i) {
                set(w, name.clone());
            }
        },
    );
    dd
}
