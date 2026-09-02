//! Overlay lifecycle and shell — the scrim, the panel, the title bar, and the
//! spawn/despawn/rebuild loop.
//!
//! The overlay is a separate root from the editor chrome, so it does not pick up
//! a theme or language change on its own; [`manage_overlay`] compares the theme
//! name and the translation revision it was built at against the live ones and
//! rebuilds when either moves.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;

use renzora::CurrentProject;
use renzora_editor_framework::{EditorSettings, SettingsTab};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::Rx;
use renzora_ember::settings_sections::SettingsSectionRegistry;
use renzora_ember::theme::*;
use renzora_ember::widgets::scroll_view_bar;
use renzora_theme::ThemeManager;
use renzora_input::InputMap;
use renzora_viewport::settings::ViewportSettings;

use crate::lang::tr;
use crate::sidebar::build_sidebar;
use crate::state::{InputTabData, InputUi, OverlayState, PANEL_H, PANEL_W};
use crate::tabs::build_tab_content;

#[derive(Component)]
pub(crate) struct OverlayRoot;

#[derive(Component)]
pub(crate) struct CloseBtn;

// ── Lifecycle: spawn / despawn / rebuild on tab change ───────────────────────

pub(crate) fn manage_overlay(world: &mut World) {
    let (show, tab) = world
        .get_resource::<EditorSettings>()
        .map(|s| (s.show_settings, s.settings_tab))
        .unwrap_or((false, SettingsTab::default()));
    let open = show;
    let theme_name = world
        .get_resource::<ThemeManager>()
        .map(|t| t.active_theme_name.clone());

    let lang_rev = renzora::lang::revision();
    let st = world.resource::<OverlayState>();
    // Rebuild when the active theme switches so the overlay re-spawns with the
    // new palette (it's a separate root from the chrome).
    let theme_changed = st.built_theme != theme_name;
    // …and when the language changes, so its own picker re-localizes live.
    let lang_changed = st.built_lang_rev != lang_rev;
    let plugin_changed = st.built_sub != st.active_sub;
    let active_sub = st.active_sub.clone();
    let (root, built, dirty) = (
        st.root,
        st.built_tab,
        st.dirty || theme_changed || lang_changed || plugin_changed,
    );

    if !open {
        if let Some(r) = root {
            if let Ok(e) = world.get_entity_mut(r) {
                e.despawn();
            }
            let mut st = world.resource_mut::<OverlayState>();
            st.root = None;
            st.built_tab = None;
            st.dirty = false;
        }
        return;
    }

    // Already built for this tab/plugin and nothing structural changed → skip.
    if root.is_some() && built == Some(tab) && !dirty {
        return;
    }
    // Tab switch, first open, or a dirty rebuild → tear down + rebuild.
    if let Some(r) = root {
        if let Ok(e) = world.get_entity_mut(r) {
            e.despawn();
        }
    }

    let Some(new_root) = build_overlay(world, tab, active_sub.as_deref()) else {
        // Fonts not ready yet — retry next frame.
        return;
    };
    let mut st = world.resource_mut::<OverlayState>();
    st.root = Some(new_root);
    st.built_tab = Some(tab);
    st.dirty = false;
    st.built_theme = theme_name;
    st.built_lang_rev = lang_rev;
    st.built_sub = active_sub;
}

fn build_overlay(world: &mut World, tab: SettingsTab, active_sub: Option<&str>) -> Option<Entity> {
    let fonts = world.get_resource::<EmberFonts>().cloned()?;
    let settings = world.get_resource::<EditorSettings>()?.clone();
    let viewport = world.get_resource::<ViewportSettings>().cloned().unwrap_or_default();
    // Project-folder fonts come from the live registry, so the dropdowns
    // auto-populate as fonts are dropped into `<project>/fonts/`.
    let custom = world
        .get_resource::<renzora_ember::font::FontRegistry>()
        .map(|r| r.project_names())
        .unwrap_or_default();
    let themes = world
        .get_resource::<ThemeManager>()
        .map(|tm| tm.available_themes.clone())
        .unwrap_or_default();
    let has_project = world.get_resource::<CurrentProject>().is_some();
    let scenes = scan_scenes(&Rx::new(&*world));
    let input = InputTabData {
        actions: world
            .get_resource::<InputMap>()
            .map(|m| m.actions.clone())
            .unwrap_or_default(),
        selected: world.get_resource::<InputUi>().and_then(|u| u.selected),
        listening: world
            .get_resource::<InputUi>()
            .map(|u| u.listening)
            .unwrap_or(false),
    };

    let mut queue = bevy::ecs::world::CommandQueue::default();
    let root = {
        let sections = world.get_resource::<SettingsSectionRegistry>();
        let mut commands = Commands::new(&mut queue, world);
        spawn_overlay(
            &mut commands,
            &fonts,
            tab,
            &settings,
            &viewport,
            &custom,
            &themes,
            &scenes,
            has_project,
            &input,
            sections,
            active_sub,
        )
    };
    queue.apply(world);
    Some(root)
}

/// Scan `<project>/scenes/` for the boot-scene / autoload pickers.
///
/// `.bsn` is the scene format; `.ron` is still accepted because projects
/// predating the switch have scenes in it and the exporter still packs both.
/// This looked for `.ron` alone, so every dropdown built from it came up empty
/// for any project written since — including the boot-scene picker, leaving no
/// way to change which scene a game starts on short of editing `project.toml`.
fn scan_scenes(world: &Rx) -> Vec<String> {
    let Some(cp) = world.get_resource::<CurrentProject>() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(cp.path.join("scenes")) {
        for entry in rd.flatten() {
            let p = entry.path();
            if !matches!(
                p.extension().and_then(|s| s.to_str()),
                Some("bsn") | Some("ron")
            ) {
                continue;
            }
            if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                out.push(format!("scenes/{name}"));
            }
        }
    }
    out.sort();
    out
}

// ── Overlay shell ────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn spawn_overlay(
    commands: &mut Commands,
    fonts: &EmberFonts,
    tab: SettingsTab,
    settings: &EditorSettings,
    viewport: &ViewportSettings,
    custom: &[String],
    themes: &[String],
    scenes: &[String],
    has_project: bool,
    input: &InputTabData,
    sections: Option<&SettingsSectionRegistry>,
    active_sub: Option<&str>,
) -> Entity {
    // Full-screen scrim: blocks clicks behind the modal + dims slightly.
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35)),
            // Below the ember popups' own global layers (dropdown menu = 500,
            // color panel = 700) so those open *above* the modal, but above the
            // default-z chrome so the scrim covers the dock/top bar/status bar —
            // and above the global bottom panel, which claims a tier of its own
            // and used to paint over the modal from the same depth.
            GlobalZIndex(renzora_ember::stacking::MODAL_SCRIM_Z),
            FocusPolicy::Block,
            Interaction::default(),
            // Capture the wheel so scrolling doesn't bleed to the dock behind.
            renzora_ember::widgets::ModalSurface,
            // Editor chrome — keep this whole overlay tree out of scene saves
            // (auto-save can fire while Settings is open; without this its nodes
            // get serialized into the scene as leaked UI).
            renzora::HideInHierarchy,
            OverlayRoot,
            Name::new("settings-overlay"),
        ))
        .id();

    let panel = commands
        .spawn((
            Node {
                width: Val::Px(PANEL_W),
                height: Val::Px(PANEL_H),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::clip(),
                border_radius: BorderRadius {
                    top_left: Val::Px(6.0),
                    ..default()
                },
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::window_bg())),
            FocusPolicy::Block,
            Name::new("settings-panel"),
        ))
        .id();
    commands.entity(root).add_child(panel);

    let title = build_title_bar(commands, fonts);
    let body = build_body(
        commands, fonts, tab, settings, viewport, custom, themes, scenes, has_project, input,
        sections, active_sub,
    );
    commands.entity(panel).add_children(&[title, body]);
    root
}

fn build_title_bar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let label = commands
        .spawn((
            Text::new(tr("common.settings")),
            ui_font(&fonts.ui, 14.0),
            TextColor(rgb(text_primary())),
            Node {
                flex_grow: 1.0,
                ..default()
            },
        ))
        .id();
    // Themed ember icon button (Styled IconButton) — editable under "Icon Button".
    let close = renzora_ember::widgets::icon_button(commands, fonts, "x");
    commands
        .entity(close)
        .insert((FocusPolicy::Block, CloseBtn));

    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(36.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(header_bg())),
            Name::new("settings-titlebar"),
        ))
        .id();
    commands.entity(bar).add_children(&[label, close]);
    bar
}

#[allow(clippy::too_many_arguments)]
fn build_body(
    commands: &mut Commands,
    fonts: &EmberFonts,
    tab: SettingsTab,
    settings: &EditorSettings,
    viewport: &ViewportSettings,
    custom: &[String],
    themes: &[String],
    scenes: &[String],
    has_project: bool,
    input: &InputTabData,
    sections: Option<&SettingsSectionRegistry>,
    active_sub: Option<&str>,
) -> Entity {
    let sidebar = build_sidebar(commands, fonts, tab, sections, active_sub);

    let content_col = build_tab_content(
        commands, fonts, tab, settings, viewport, custom, themes, scenes, has_project, input,
        sections, active_sub,
    );
    let scroller = scroll_view_bar(commands, content_col);

    let content_pane = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                height: Val::Percent(100.0),
                min_width: Val::Px(0.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            Name::new("settings-content"),
        ))
        .id();
    commands.entity(content_pane).add_child(scroller);

    let body = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Row,
                min_height: Val::Px(0.0),
                ..default()
            },
            Name::new("settings-body"),
        ))
        .id();
    commands.entity(body).add_children(&[sidebar, content_pane]);
    body
}

pub(crate) fn settings_close_click(
    btns: Query<&Interaction, (Changed<Interaction>, With<CloseBtn>)>,
    mut settings: ResMut<EditorSettings>,
) {
    for interaction in &btns {
        if *interaction == Interaction::Pressed {
            settings.show_settings = false;
        }
    }
}
