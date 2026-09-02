//! The overlay's left-hand category sidebar: the static category table, the
//! rows built from it, the live search filter, and the two click systems that
//! turn a row press into a tab / sub-selection change.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use renzora_editor_framework::{EditorSettings, SettingsTab};
use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::settings_sections::SettingsSectionRegistry;
use renzora_ember::theme::*;
use renzora_ember::widgets::{scroll_view_bar_keyed, text_input, EmberTextInput};

use crate::lang::{tr, tr_cat, tr_group};
use crate::state::{OverlayState, SIDEBAR_W};

#[derive(Component)]
pub(crate) struct TabBtn(SettingsTab, Option<String>);

/// Sidebar button for a single plugin settings section (its `SettingsSection::id`).
/// Selecting it switches to the `Plugins` tab and shows only that section.
#[derive(Component)]
pub(crate) struct PluginTabBtn(String);

/// The sidebar's search box (an `EmberTextInput`); [`filter_sidebar`] reads its
/// value to show/hide categories live.
#[derive(Component)]
pub(crate) struct SettingsSearchBox;

/// Tags a sidebar category row with its group + label so [`filter_sidebar`] can
/// match against the search query without rebuilding.
#[derive(Component)]
pub(crate) struct SettingsCatRow {
    group: String,
    label: String,
}

/// Tags a sidebar group header with its group name (hidden when the search hides
/// every row in the group).
#[derive(Component)]
pub(crate) struct SettingsGroupTag(String);

/// Sidebar categories grouped under Unreal-style section headers. Each entry is
/// `(tab, focus, icon, label)`; `focus` is the section key shown when a tab is
/// split into finer categories (`None` = the whole tab as one page). The active
/// category is `(EditorSettings.settings_tab, OverlayState.active_sub)`.
///
/// A category is a *page*, not a single section: several sections may share one
/// `focus` key and so appear stacked under one sidebar row. Window holds both
/// Window and Render Resolution; General holds Developer, Renderer and Import.
/// That is the whole point of the key — before, every section had its own key
/// and therefore its own sidebar row, which put twenty rows in the sidebar for
/// sixty-eight actual settings, six of them a lone checkbox.
type Cat = (SettingsTab, Option<&'static str>, &'static str, &'static str);
const CATS: &[(&str, &[Cat])] = &[
    (
        "PROJECT",
        &[
            (SettingsTab::Project, Some("project"), "folder-open", "Project"),
            // Window (the OS surface) + Render Resolution (what the camera
            // actually shoots at). They live together because their width/height
            // pairs are only distinguishable side by side.
            (SettingsTab::Project, Some("window"), "desktop", "Window"),
            (SettingsTab::Project, Some("rendering"), "monitor", "Rendering"),
        ],
    ),
    (
        "APPEARANCE",
        &[
            (SettingsTab::Interface, None, "layout", "Interface"),
            (SettingsTab::Theme, None, "palette", "Theme"),
        ],
    ),
    (
        "EDITOR",
        &[
            (SettingsTab::Editor, Some("general"), "wrench", "General"),
            (SettingsTab::Editor, Some("autosave"), "floppy-disk", "Auto-Save"),
            // Deliberately here rather than under PLUGINS. That group is one
            // entry per plugin's OWN settings, contributed by the plugin — a
            // list you can only reach once a plugin is loaded and working. This
            // is the editor's control over which plugins load at all, which is
            // exactly what you go looking for when one of them is the reason the
            // editor is misbehaving.
            (SettingsTab::Editor, Some("plugins"), "puzzle-piece", "Plugins"),
            (SettingsTab::Viewport, Some("viewport"), "grid-four", "Viewport"),
            (SettingsTab::Viewport, Some("camera"), "video-camera", "Camera"),
            (SettingsTab::Viewport, Some("gizmos"), "bounding-box", "Gizmos"),
            (SettingsTab::Scripting, None, "code", "Scripting"),
        ],
    ),
    (
        "CONTROLS",
        &[
            (SettingsTab::Input, None, "game-controller", "Input"),
            (SettingsTab::Shortcuts, None, "keyboard", "Shortcuts"),
        ],
    ),
    // The PLUGINS group is appended dynamically in `build_sidebar` — one entry
    // per registered plugin settings section.
];

pub(crate) fn build_sidebar(
    commands: &mut Commands,
    fonts: &EmberFonts,
    active: SettingsTab,
    sections: Option<&SettingsSectionRegistry>,
    active_sub: Option<&str>,
) -> Entity {
    // Outer fixed-width column: search box (fixed) above the scrolling list.
    let sidebar = commands
        .spawn((
            Node {
                width: Val::Px(SIDEBAR_W),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                flex_shrink: 0.0,
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::window_bg())),
            Name::new("settings-sidebar"),
        ))
        .id();
    // Search box — filters categories live (see `filter_sidebar`). The ember
    // input defaults to `min_width: 180px` (wider than the 160px sidebar, so it
    // spilled over the divider) — pin it to fill the column instead.
    let search = text_input(commands, &fonts.ui, &tr("common.search"), "");
    commands.entity(search).insert(SettingsSearchBox).queue(
        |mut e: EntityWorldMut| {
            if let Some(mut n) = e.get_mut::<Node>() {
                n.min_width = Val::Px(0.0);
                n.width = Val::Percent(100.0);
            }
        },
    );
    let search_wrap = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                flex_shrink: 0.0,
                ..default()
            },
            Name::new("settings-search"),
        ))
        .id();
    commands.entity(search_wrap).add_child(search);
    commands.entity(sidebar).add_child(search_wrap);
    // Inner scrollable list holding the rows.
    let list = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                ..default()
            },
            Name::new("settings-sidebar-list"),
        ))
        .id();
    let mut kids = Vec::new();
    for (gi, (group, cats)) in CATS.iter().enumerate() {
        // Localized once per group; the group tag + row.group share this string
        // so `filter_sidebar`'s header-matching stays consistent in any language.
        let gname = tr_group(group);
        // A little breathing room above every group but the first.
        let header = sidebar_group_header(commands, fonts, &gname, gi > 0);
        commands.entity(header).insert(SettingsGroupTag(gname.clone()));
        kids.push(header);
        for &(tab, focus, icon, label) in *cats {
            // A category is active when both its tab and its section focus
            // match the current selection.
            let selected = tab == active && active_sub == focus;
            let lname = tr_cat(label);
            let row = sidebar_tab(commands, fonts, icon, &lname, tab, focus, selected);
            commands.entity(row).insert(SettingsCatRow {
                group: gname.clone(),
                label: lname,
            });
            kids.push(row);
        }
    }
    // PLUGINS group: one sidebar category per registered plugin section.
    let plugins = sections.map(|s| s.0.as_slice()).unwrap_or_default();
    if !plugins.is_empty() {
        let pname = tr_group("PLUGINS");
        let header = sidebar_group_header(commands, fonts, &pname, true);
        commands.entity(header).insert(SettingsGroupTag(pname.clone()));
        kids.push(header);
        for entry in plugins {
            let selected = active == SettingsTab::Plugins
                && active_sub == Some(entry.id.as_str());
            let row = sidebar_plugin_tab(
                commands,
                fonts,
                &entry.icon,
                &entry.title,
                &entry.id,
                selected,
            );
            commands.entity(row).insert(SettingsCatRow {
                group: pname.clone(),
                label: entry.title.clone(),
            });
            kids.push(row);
        }
    }
    commands.entity(list).add_children(&kids);
    // Keyed so the sidebar keeps its scroll position when the overlay rebuilds
    // (selecting a category re-spawns the overlay — without this it snaps to top).
    let scroller = scroll_view_bar_keyed(commands, list, "settings-sidebar");
    commands.entity(sidebar).add_child(scroller);
    sidebar
}

/// A small uppercase muted section header that introduces a sidebar group.
fn sidebar_group_header(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    pad_top: bool,
) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::new(
                    Val::Px(8.0),
                    Val::Px(0.0),
                    Val::Px(if pad_top { 10.0 } else { 2.0 }),
                    Val::Px(2.0),
                ),
                ..default()
            },
            Name::new("settings-group-header"),
            children![(
                Text::new(label),
                ui_font(&fonts.ui, 10.0),
                TextColor(rgb(text_muted())),
            )],
        ))
        .id()
}

fn sidebar_tab(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
    tab: SettingsTab,
    focus: Option<&str>,
    active: bool,
) -> Entity {
    let icon_color = if active { accent() } else { text_muted() };
    let txt_color = if active { text_primary() } else { text_muted() };
    let ico = icon_text(commands, &fonts.phosphor, icon, icon_color, 14.0);
    let lbl = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(txt_color)),
        ))
        .id();
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(30.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            TabBtn(tab, focus.map(String::from)),
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("settings-tab"),
        ))
        .id();
    // Active → highlighted; otherwise a themed hover wash.
    renzora_ember::reactive::tracked::bind_bg(commands, row, move |w| {
        if active {
            rgb(tab_active())
        } else if matches!(
            w.get::<Interaction>(row),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        ) {
            rgb(tab_hover())
        } else {
            Color::NONE
        }
    });
    commands.entity(row).add_children(&[ico, lbl]);
    row
}

/// A sidebar row for one plugin settings section — like [`sidebar_tab`] but it
/// carries the section id and routes through [`PluginTabBtn`].
fn sidebar_plugin_tab(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
    id: &str,
    selected: bool,
) -> Entity {
    let icon_color = if selected { accent() } else { text_muted() };
    let txt_color = if selected { text_primary() } else { text_muted() };
    let ico = icon_text(commands, &fonts.phosphor, icon, icon_color, 14.0);
    let lbl = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(txt_color)),
        ))
        .id();
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(30.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            PluginTabBtn(id.to_string()),
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("settings-plugin-tab"),
        ))
        .id();
    renzora_ember::reactive::tracked::bind_bg(commands, row, move |w| {
        if selected {
            rgb(tab_active())
        } else if matches!(
            w.get::<Interaction>(row),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        ) {
            rgb(tab_hover())
        } else {
            Color::NONE
        }
    });
    commands.entity(row).add_children(&[ico, lbl]);
    row
}

/// Live-filter the sidebar categories by the search box text. Pure visibility
/// toggling (no rebuild), so the search input keeps focus while typing. A group
/// header hides when the query hides every category under it.
pub(crate) fn filter_sidebar(
    search: Query<&EmberTextInput, With<SettingsSearchBox>>,
    mut rows: Query<(&SettingsCatRow, &mut Node), Without<SettingsGroupTag>>,
    mut headers: Query<(&SettingsGroupTag, &mut Node), Without<SettingsCatRow>>,
) {
    let Ok(input) = search.single() else {
        return;
    };
    let q = input.value.trim().to_lowercase();
    let mut visible_groups: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (row, mut node) in &mut rows {
        let show = q.is_empty()
            || row.label.to_lowercase().contains(&q)
            || row.group.to_lowercase().contains(&q);
        node.display = if show { Display::Flex } else { Display::None };
        if show {
            visible_groups.insert(row.group.clone());
        }
    }
    for (tag, mut node) in &mut headers {
        let show = q.is_empty() || visible_groups.contains(&tag.0);
        node.display = if show { Display::Flex } else { Display::None };
    }
}

pub(crate) fn settings_tab_click(
    btns: Query<(&Interaction, &TabBtn), Changed<Interaction>>,
    mut settings: ResMut<EditorSettings>,
    mut state: ResMut<OverlayState>,
) {
    for (interaction, btn) in &btns {
        if *interaction == Interaction::Pressed {
            if settings.settings_tab != btn.0 {
                settings.settings_tab = btn.0;
            }
            // The button's focus key becomes the active sub-selection (a section
            // within a split tab, or `None` for a whole-tab category). This also
            // clears any previously selected plugin.
            if state.active_sub != btn.1 {
                state.active_sub = btn.1.clone();
            }
        }
    }
}

/// Selecting a plugin sidebar category switches to the `Plugins` tab and records
/// which section to show. The rebuild is driven by `active_sub` changing
/// (see `crate::overlay::manage_overlay`), so re-selecting the same plugin is a
/// no-op.
pub(crate) fn settings_plugin_click(
    btns: Query<(&Interaction, &PluginTabBtn), Changed<Interaction>>,
    mut settings: ResMut<EditorSettings>,
    mut state: ResMut<OverlayState>,
) {
    for (interaction, btn) in &btns {
        if *interaction == Interaction::Pressed {
            if settings.settings_tab != SettingsTab::Plugins {
                settings.settings_tab = SettingsTab::Plugins;
            }
            if state.active_sub.as_deref() != Some(btn.0.as_str()) {
                state.active_sub = Some(btn.0.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn text_input(value: &str) -> EmberTextInput {
        EmberTextInput {
            value: value.to_string(),
            focused: false,
            text_entity: Entity::PLACEHOLDER,
            placeholder: String::new(),
            caret: Entity::PLACEHOLDER,
            password: false,
            select_all: false,
            caret_index: 0,
            advance: 0.0,
            offsets: Vec::new(),
            sel_anchor: None,
        }
    }

    /// A sidebar with two groups, one row and one header each.
    fn sidebar(query: &str) -> (World, Entity, Entity, Entity, Entity) {
        let mut world = World::new();
        world.spawn((SettingsSearchBox, text_input(query)));

        let row_theme = world
            .spawn((
                SettingsCatRow { group: "APPEARANCE".into(), label: "Theme".into() },
                Node::default(),
            ))
            .id();
        let row_camera = world
            .spawn((
                SettingsCatRow { group: "EDITOR".into(), label: "Camera".into() },
                Node::default(),
            ))
            .id();
        let head_appearance = world
            .spawn((SettingsGroupTag("APPEARANCE".into()), Node::default()))
            .id();
        let head_editor = world
            .spawn((SettingsGroupTag("EDITOR".into()), Node::default()))
            .id();

        world.run_system_once(filter_sidebar).unwrap();
        (world, row_theme, row_camera, head_appearance, head_editor)
    }

    fn shown(world: &World, e: Entity) -> bool {
        world.get::<Node>(e).unwrap().display == Display::Flex
    }

    #[test]
    fn an_empty_query_shows_every_row_and_header() {
        let (w, theme, camera, appearance, editor) = sidebar("");
        assert!(shown(&w, theme) && shown(&w, camera));
        assert!(shown(&w, appearance) && shown(&w, editor));
    }

    #[test]
    fn a_query_hides_the_rows_that_do_not_match() {
        let (w, theme, camera, _, _) = sidebar("theme");
        assert!(shown(&w, theme));
        assert!(!shown(&w, camera));
    }

    /// A group header with no visible rows under it is a heading over empty
    /// space, which reads as a broken filter rather than as "no results".
    #[test]
    fn a_group_header_hides_once_all_its_rows_are_filtered_out() {
        let (w, _, _, appearance, editor) = sidebar("theme");
        assert!(shown(&w, appearance), "the matching row's group must stay");
        assert!(!shown(&w, editor), "an empty group's header must hide");
    }

    /// Matching the group name is what makes typing "editor" list everything
    /// under EDITOR, not only rows whose own label says "editor".
    #[test]
    fn a_query_can_match_the_group_rather_than_the_row() {
        let (w, theme, camera, _, _) = sidebar("editor");
        assert!(shown(&w, camera));
        assert!(!shown(&w, theme));
    }

    #[test]
    fn search_ignores_case_and_surrounding_space() {
        let (w, theme, camera, _, _) = sidebar("  THEME  ");
        assert!(shown(&w, theme));
        assert!(!shown(&w, camera));
    }

    #[test]
    fn a_query_matching_nothing_hides_everything() {
        let (w, theme, camera, appearance, editor) = sidebar("zzzz-no-such-setting");
        assert!(!shown(&w, theme) && !shown(&w, camera));
        assert!(!shown(&w, appearance) && !shown(&w, editor));
    }
}
