//! bevy_ui-native settings overlay — the egui → bevy_ui migration of the
//! Settings window. Mirrors the egui overlay (`super::draw_settings_overlay`):
//! a centered modal with a vertical tab sidebar and a scrollable content pane,
//! driven by [`EditorSettings::show_settings`] and only under the `BevyUi`
//! editor backend (the egui overlay is gated off there).
//!
//! Controls two-way-bind to the live resources via `bind_2way`, so edits write
//! straight back to `EditorSettings` / `ViewportSettings` the same frame — no
//! snapshot/diff round-trip like the egui version needs.

use bevy::prelude::*;
use bevy::text::TextLayout;
use bevy::ui::FocusPolicy;
use bevy::window::SystemCursorIcon;

use renzora::EditorUiBackend;
use renzora_editor::{
    CustomFonts, EditorSettings, MonoFont, SelectionHighlightMode, SettingsTab, UiFont,
};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::inspector::color_field;
use renzora_ember::reactive::bind_2way;
use renzora_ember::theme::{rgb, ACCENT_BLUE, PANEL_BG, TEXT_MUTED, TEXT_PRIMARY};
use renzora_ember::widgets::{drag_value, dropdown, scroll_view_bar, toggle_switch, DragRange};
use renzora_hui::cursor_icon::HoverCursor;
use renzora_viewport::settings::{CollisionGizmoVisibility, ViewportSettings};

const PANEL_W: f32 = 880.0;
const PANEL_H: f32 = 620.0;
const SIDEBAR_W: f32 = 160.0;
const LABEL_W: f32 = 110.0;
const EXTREME_BG: (u8, u8, u8) = (18, 18, 23);
const SECTION_HEADER_BG: (u8, u8, u8) = (40, 40, 50);
const ROW_EVEN: (u8, u8, u8) = (34, 34, 42);
const ROW_ODD: (u8, u8, u8) = (30, 30, 37);
const VALUE_COL: (u8, u8, u8) = (210, 210, 220);

// Accent colors per category — matches the egui `CategoryStyle` palette.
const A_BLUE: (u8, u8, u8) = (80, 140, 255);
const A_PURPLE: (u8, u8, u8) = (170, 130, 240);
const A_ORANGE: (u8, u8, u8) = (235, 150, 70);
const A_GREEN: (u8, u8, u8) = (110, 200, 120);
const A_TEAL: (u8, u8, u8) = (80, 200, 200);

// ── Markers / state ──────────────────────────────────────────────────────────

#[derive(Resource, Default)]
struct NativeSettingsState {
    root: Option<Entity>,
    built_tab: Option<SettingsTab>,
}

#[derive(Component)]
struct NativeSettingsRoot;

#[derive(Component)]
struct NativeSettingsTabBtn(SettingsTab);

#[derive(Component)]
struct NativeSettingsClose;

#[derive(Component)]
struct SettingsSection {
    body: Entity,
    caret: Entity,
    open: bool,
}

// ── Plugin wiring ────────────────────────────────────────────────────────────

pub(crate) fn build(app: &mut App) {
    app.init_resource::<NativeSettingsState>();
    app.add_systems(
        Update,
        (
            manage_native_settings,
            settings_tab_click,
            settings_close_click,
            settings_section_toggle,
        )
            .run_if(in_state(renzora_editor::SplashState::Editor)),
    );
}

// ── Lifecycle: spawn / despawn / rebuild on tab change ───────────────────────

fn manage_native_settings(world: &mut World) {
    let backend_bevy = world
        .get_resource::<EditorUiBackend>()
        .map(|b| b.is_bevy_ui())
        .unwrap_or(false);
    let (show, tab) = world
        .get_resource::<EditorSettings>()
        .map(|s| (s.show_settings, s.settings_tab))
        .unwrap_or((false, SettingsTab::default()));
    let open = backend_bevy && show;

    let st = world.resource::<NativeSettingsState>();
    let (root, built) = (st.root, st.built_tab);

    if !open {
        if let Some(r) = root {
            if let Ok(e) = world.get_entity_mut(r) {
                e.despawn();
            }
            let mut st = world.resource_mut::<NativeSettingsState>();
            st.root = None;
            st.built_tab = None;
        }
        return;
    }

    // Already built for this tab → nothing to do.
    if root.is_some() && built == Some(tab) {
        return;
    }
    // Tab switch or first open → tear down the old tree and rebuild.
    if let Some(r) = root {
        if let Ok(e) = world.get_entity_mut(r) {
            e.despawn();
        }
    }

    let Some(new_root) = build_overlay(world, tab) else {
        // Fonts not ready yet — retry next frame.
        return;
    };
    let mut st = world.resource_mut::<NativeSettingsState>();
    st.root = Some(new_root);
    st.built_tab = Some(tab);
}

fn build_overlay(world: &mut World, tab: SettingsTab) -> Option<Entity> {
    let fonts = world.get_resource::<EmberFonts>().cloned()?;
    let settings = world.get_resource::<EditorSettings>()?.clone();
    let viewport = world.get_resource::<ViewportSettings>().cloned().unwrap_or_default();
    let custom = world
        .get_resource::<CustomFonts>()
        .map(|c| c.names.clone())
        .unwrap_or_default();

    let mut queue = bevy::ecs::world::CommandQueue::default();
    let root = {
        let mut commands = Commands::new(&mut queue, world);
        spawn_overlay(&mut commands, &fonts, tab, &settings, &viewport, &custom)
    };
    queue.apply(world);
    Some(root)
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
            // default-z chrome so the scrim covers the dock/top bar/status bar.
            GlobalZIndex(100),
            FocusPolicy::Block,
            Interaction::default(),
            NativeSettingsRoot,
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
            BackgroundColor(rgb(EXTREME_BG)),
            FocusPolicy::Block,
            Name::new("settings-panel"),
        ))
        .id();
    commands.entity(root).add_child(panel);

    let title = build_title_bar(commands, fonts);
    let body = build_body(commands, fonts, tab, settings, viewport, custom);
    commands.entity(panel).add_children(&[title, body]);
    root
}

fn build_title_bar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let label = commands
        .spawn((
            Text::new("Settings"),
            ui_font(&fonts.ui, 14.0),
            TextColor(rgb(TEXT_PRIMARY)),
            Node {
                flex_grow: 1.0,
                ..default()
            },
        ))
        .id();
    let close = commands
        .spawn((
            Node {
                width: Val::Px(22.0),
                height: Val::Px(22.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb((38, 38, 46))),
            Interaction::default(),
            FocusPolicy::Block,
            NativeSettingsClose,
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("settings-close"),
        ))
        .id();
    let x = icon_text(commands, &fonts.phosphor, "x", TEXT_MUTED, 13.0);
    commands.entity(close).add_child(x);

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
            BackgroundColor(rgb((26, 26, 32))),
            Name::new("settings-titlebar"),
        ))
        .id();
    commands.entity(bar).add_children(&[label, close]);
    bar
}

fn build_body(
    commands: &mut Commands,
    fonts: &EmberFonts,
    tab: SettingsTab,
    settings: &EditorSettings,
    viewport: &ViewportSettings,
    custom: &[String],
) -> Entity {
    let sidebar = build_sidebar(commands, fonts, tab);

    let content_col = build_tab_content(commands, fonts, tab, settings, viewport, custom);
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
            BackgroundColor(rgb(PANEL_BG)),
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

const TABS: &[(SettingsTab, &str, &str)] = &[
    (SettingsTab::Project, "folder-open", "Project"),
    (SettingsTab::Interface, "text-aa", "Interface"),
    (SettingsTab::Editor, "wrench", "Editor"),
    (SettingsTab::Viewport, "cube", "Viewport"),
    (SettingsTab::Scripting, "code", "Scripting"),
    (SettingsTab::Assets, "desktop", "Assets"),
    (SettingsTab::Input, "game-controller", "Input"),
    (SettingsTab::Shortcuts, "keyboard", "Shortcuts"),
    (SettingsTab::Theme, "palette", "Theme"),
];

fn build_sidebar(commands: &mut Commands, fonts: &EmberFonts, active: SettingsTab) -> Entity {
    let sidebar = commands
        .spawn((
            Node {
                width: Val::Px(SIDEBAR_W),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(EXTREME_BG)),
            Name::new("settings-sidebar"),
        ))
        .id();
    let mut kids = Vec::new();
    for &(tab, icon, label) in TABS {
        kids.push(sidebar_tab(commands, fonts, icon, label, tab, tab == active));
    }
    commands.entity(sidebar).add_children(&kids);
    sidebar
}

fn sidebar_tab(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
    tab: SettingsTab,
    active: bool,
) -> Entity {
    let icon_color = if active { ACCENT_BLUE } else { TEXT_MUTED };
    let txt_color = if active { TEXT_PRIMARY } else { TEXT_MUTED };
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
            BackgroundColor(if active {
                rgb((50, 50, 62))
            } else {
                Color::NONE
            }),
            Interaction::default(),
            NativeSettingsTabBtn(tab),
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("settings-tab"),
        ))
        .id();
    commands.entity(row).add_children(&[ico, lbl]);
    row
}

// ── Section + row helpers ────────────────────────────────────────────────────

fn section(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    title: &str,
    accent: (u8, u8, u8),
) -> (Entity, Entity) {
    let body = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                padding: UiRect {
                    left: Val::Px(8.0),
                    top: Val::Px(4.0),
                    bottom: Val::Px(4.0),
                    ..default()
                },
                ..default()
            },
            Name::new("section-body"),
        ))
        .id();
    let caret = icon_text(commands, &fonts.phosphor, "caret-down", TEXT_MUTED, 12.0);
    let ico = icon_text(commands, &fonts.phosphor, icon, accent, 14.0);
    let heading = commands
        .spawn((
            Text::new(title),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(TEXT_PRIMARY)),
        ))
        .id();
    let header = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(5.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(SECTION_HEADER_BG)),
            Interaction::default(),
            SettingsSection {
                body,
                caret,
                open: true,
            },
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("section-header"),
        ))
        .id();
    commands.entity(header).add_children(&[caret, ico, heading]);

    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                margin: UiRect::bottom(Val::Px(8.0)),
                ..default()
            },
            Name::new("section"),
        ))
        .id();
    commands.entity(root).add_children(&[header, body]);
    (root, body)
}

fn settings_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    body: Entity,
    idx: usize,
    label: &str,
    control: Entity,
) {
    let bg = if idx.is_multiple_of(2) { ROW_EVEN } else { ROW_ODD };
    let lbl = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(TEXT_MUTED)),
            TextLayout::new_with_no_wrap(),
        ))
        .id();
    let label_col = commands
        .spawn((
            Node {
                width: Val::Px(LABEL_W),
                flex_shrink: 0.0,
                align_items: AlignItems::Center,
                overflow: Overflow::clip(),
                ..default()
            },
            Name::new("row-label"),
        ))
        .id();
    commands.entity(label_col).add_child(lbl);
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(bg)),
            Name::new("settings-row"),
        ))
        .id();
    commands.entity(row).add_children(&[label_col, control]);
    commands.entity(body).add_child(row);
}

/// A muted, control-less note row (the "takes effect after restart" lines).
fn note_row(commands: &mut Commands, fonts: &EmberFonts, body: Entity, text: &str) {
    let lbl = commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb((120, 120, 132))),
        ))
        .id();
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                ..default()
            },
            Name::new("note-row"),
        ))
        .id();
    commands.entity(row).add_child(lbl);
    commands.entity(body).add_child(row);
}

// Control builders — each carries its own two-way binding to live state.

fn ctl_toggle<G, S>(commands: &mut Commands, init: bool, get: G, set: S) -> Entity
where
    G: Fn(&World) -> bool + Send + Sync + 'static,
    S: Fn(&mut World, &bool) + Send + Sync + 'static,
{
    let sw = toggle_switch(commands, init);
    bind_2way(commands, sw, get, set);
    sw
}

#[allow(clippy::too_many_arguments)]
fn ctl_drag<G, S>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    init: f32,
    min: f32,
    max: f32,
    step: f32,
    get: G,
    set: S,
) -> Entity
where
    G: Fn(&World) -> f32 + Send + Sync + 'static,
    S: Fn(&mut World, &f32) + Send + Sync + 'static,
{
    let dv = drag_value(commands, &fonts.ui, "", VALUE_COL, init, step);
    if max > min {
        commands.entity(dv).insert(DragRange { min, max });
    }
    bind_2way(commands, dv, get, set);
    dv
}

fn ctl_dropdown<G, S>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    options: &[&str],
    init: usize,
    get: G,
    set: S,
) -> Entity
where
    G: Fn(&World) -> usize + Send + Sync + 'static,
    S: Fn(&mut World, &usize) + Send + Sync + 'static,
{
    let dd = dropdown(commands, fonts, options, init);
    bind_2way(commands, dd, get, set);
    dd
}

// ── Tab content dispatch ─────────────────────────────────────────────────────

fn build_tab_content(
    commands: &mut Commands,
    fonts: &EmberFonts,
    tab: SettingsTab,
    settings: &EditorSettings,
    viewport: &ViewportSettings,
    custom: &[String],
) -> Entity {
    let col = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Name::new("tab-content"),
        ))
        .id();

    match tab {
        SettingsTab::Interface => tab_interface(commands, fonts, col, settings, custom),
        SettingsTab::Editor => tab_editor(commands, fonts, col),
        SettingsTab::Viewport => tab_viewport(commands, fonts, col, viewport),
        SettingsTab::Scripting => tab_scripting(commands, fonts, col),
        SettingsTab::Assets => tab_assets(commands, fonts, col),
        // Not yet migrated to bevy_ui — placeholder so the overlay still works.
        SettingsTab::Project | SettingsTab::Input | SettingsTab::Shortcuts | SettingsTab::Theme
        | SettingsTab::Plugins => placeholder(commands, fonts, col),
    }
    col
}

fn placeholder(commands: &mut Commands, fonts: &EmberFonts, col: Entity) {
    let lbl = commands
        .spawn((
            Text::new("This tab is being migrated to bevy_ui."),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(TEXT_MUTED)),
            Node {
                margin: UiRect::all(Val::Px(12.0)),
                ..default()
            },
        ))
        .id();
    commands.entity(col).add_child(lbl);
}

// ── Interface ────────────────────────────────────────────────────────────────

fn tab_interface(
    commands: &mut Commands,
    fonts: &EmberFonts,
    col: Entity,
    settings: &EditorSettings,
    custom: &[String],
) {
    let (sec, body) = section(commands, fonts, "text-aa", "Fonts", A_BLUE);
    commands.entity(col).add_child(sec);

    // UI font: builtin labels + custom names.
    let ui_opts: Vec<String> = UiFont::BUILTIN
        .iter()
        .map(|f| f.label().to_string())
        .chain(custom.iter().cloned())
        .collect();
    let ui_refs: Vec<&str> = ui_opts.iter().map(|s| s.as_str()).collect();
    let cu = custom.to_vec();
    let cu2 = custom.to_vec();
    let dd = ctl_dropdown(
        commands,
        fonts,
        &ui_refs,
        ui_font_index(&settings.ui_font, custom),
        move |w| ui_font_index(&w.resource::<EditorSettings>().ui_font, &cu),
        move |w, &i| w.resource_mut::<EditorSettings>().ui_font = ui_font_from_index(i, &cu2),
    );
    settings_row(commands, fonts, body, 0, "UI Font", dd);

    let mono_opts: Vec<String> = MonoFont::BUILTIN
        .iter()
        .map(|f| f.label().to_string())
        .chain(custom.iter().cloned())
        .collect();
    let mono_refs: Vec<&str> = mono_opts.iter().map(|s| s.as_str()).collect();
    let cm = custom.to_vec();
    let cm2 = custom.to_vec();
    let dd = ctl_dropdown(
        commands,
        fonts,
        &mono_refs,
        mono_font_index(&settings.mono_font, custom),
        move |w| mono_font_index(&w.resource::<EditorSettings>().mono_font, &cm),
        move |w, &i| w.resource_mut::<EditorSettings>().mono_font = mono_font_from_index(i, &cm2),
    );
    settings_row(commands, fonts, body, 1, "Code Font", dd);

    let dv = ctl_drag(
        commands,
        fonts,
        settings.font_size,
        10.0,
        24.0,
        0.5,
        |w| w.resource::<EditorSettings>().font_size,
        |w, &v| w.resource_mut::<EditorSettings>().font_size = v,
    );
    settings_row(commands, fonts, body, 2, "Font Size", dv);

    let (sec, body) = section(commands, fonts, "list-bullets", "Hierarchy", A_BLUE);
    commands.entity(col).add_child(sec);
    let t = ctl_toggle(
        commands,
        settings.hierarchy_parent_stacking,
        |w| w.resource::<EditorSettings>().hierarchy_parent_stacking,
        |w, &v| w.resource_mut::<EditorSettings>().hierarchy_parent_stacking = v,
    );
    settings_row(commands, fonts, body, 0, "Parent Stacking", t);
}

fn ui_font_index(f: &UiFont, custom: &[String]) -> usize {
    match f {
        UiFont::Roboto => 0,
        UiFont::OpenSans => 1,
        UiFont::NotoSans => 2,
        UiFont::Custom(name) => {
            3 + custom.iter().position(|n| n == name).unwrap_or(0)
        }
    }
}

fn ui_font_from_index(i: usize, custom: &[String]) -> UiFont {
    match i {
        0 => UiFont::Roboto,
        1 => UiFont::OpenSans,
        2 => UiFont::NotoSans,
        n => custom
            .get(n - 3)
            .map(|s| UiFont::Custom(s.clone()))
            .unwrap_or(UiFont::NotoSans),
    }
}

fn mono_font_index(f: &MonoFont, custom: &[String]) -> usize {
    match f {
        MonoFont::JetBrainsMono => 0,
        MonoFont::FiraCode => 1,
        MonoFont::SourceCodePro => 2,
        MonoFont::Custom(name) => 3 + custom.iter().position(|n| n == name).unwrap_or(0),
    }
}

fn mono_font_from_index(i: usize, custom: &[String]) -> MonoFont {
    match i {
        0 => MonoFont::JetBrainsMono,
        1 => MonoFont::FiraCode,
        2 => MonoFont::SourceCodePro,
        n => custom
            .get(n - 3)
            .map(|s| MonoFont::Custom(s.clone()))
            .unwrap_or(MonoFont::JetBrainsMono),
    }
}

// ── Editor ───────────────────────────────────────────────────────────────────

fn tab_editor(commands: &mut Commands, fonts: &EmberFonts, col: Entity) {
    let (sec, body) = section(commands, fonts, "wrench", "Developer", A_ORANGE);
    commands.entity(col).add_child(sec);
    let t = ctl_toggle(
        commands,
        false, // corrected by bind_2way on first frame
        |w| w.resource::<EditorSettings>().dev_mode,
        |w, &v| w.resource_mut::<EditorSettings>().dev_mode = v,
    );
    settings_row(commands, fonts, body, 0, "Dev Mode", t);

    let (sec, body) = section(commands, fonts, "desktop", "UI Workspace", A_BLUE);
    commands.entity(col).add_child(sec);
    let t = ctl_toggle(
        commands,
        true,
        |w| w.resource::<EditorSettings>().ui_preview_by_default,
        |w, &v| w.resource_mut::<EditorSettings>().ui_preview_by_default = v,
    );
    settings_row(commands, fonts, body, 0, "Preview", t);

    let (sec, body) = section(commands, fonts, "monitor", "Renderer", A_BLUE);
    commands.entity(col).add_child(sec);
    let avail: Vec<renzora::RendererBackend> = renzora::RendererBackend::available().to_vec();
    let labels: Vec<&str> = avail.iter().map(|b| b.label()).collect();
    let av1 = avail.clone();
    let av2 = avail.clone();
    let dd = ctl_dropdown(
        commands,
        fonts,
        &labels,
        0, // reseeded from state by bind_2way on the first frame

        move |w| {
            let b = w.resource::<EditorSettings>().renderer_backend;
            av1.iter().position(|x| *x == b).unwrap_or(0)
        },
        move |w, &i| {
            if let Some(b) = av2.get(i).copied() {
                w.resource_mut::<EditorSettings>().renderer_backend = b;
                let _ = renzora::save_renderer_backend(b);
            }
        },
    );
    settings_row(commands, fonts, body, 0, "Graphics Backend", dd);
    note_row(commands, fonts, body, "Takes effect after restarting the editor.");
}

// ── Viewport ─────────────────────────────────────────────────────────────────

fn tab_viewport(commands: &mut Commands, fonts: &EmberFonts, col: Entity, vp: &ViewportSettings) {
    let (sec, body) = section(commands, fonts, "grid-four", "Grid", A_GREEN);
    commands.entity(col).add_child(sec);
    let t = ctl_toggle(
        commands,
        vp.show_grid,
        |w| w.resource::<ViewportSettings>().show_grid,
        |w, &v| w.resource_mut::<ViewportSettings>().show_grid = v,
    );
    settings_row(commands, fonts, body, 0, "Show Grid", t);
    let t = ctl_toggle(
        commands,
        vp.show_subgrid,
        |w| w.resource::<ViewportSettings>().show_subgrid,
        |w, &v| w.resource_mut::<ViewportSettings>().show_subgrid = v,
    );
    settings_row(commands, fonts, body, 1, "Show Subgrid", t);
    let t = ctl_toggle(
        commands,
        vp.show_axis_gizmo,
        |w| w.resource::<ViewportSettings>().show_axis_gizmo,
        |w, &v| w.resource_mut::<ViewportSettings>().show_axis_gizmo = v,
    );
    settings_row(commands, fonts, body, 2, "Axis Gizmo", t);
    let cf = color_field(
        commands,
        |w| {
            let c = w.resource::<ViewportSettings>().grid_color_2d;
            [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0]
        },
        |w, rgb| {
            let mut vp = w.resource_mut::<ViewportSettings>();
            let a = vp.grid_color_2d[3];
            vp.grid_color_2d = [
                (rgb[0] * 255.0).round() as u8,
                (rgb[1] * 255.0).round() as u8,
                (rgb[2] * 255.0).round() as u8,
                a,
            ];
        },
    );
    settings_row(commands, fonts, body, 3, "2D Grid Color", cf);

    let (sec, body) = section(commands, fonts, "gauge", "Performance", A_TEAL);
    commands.entity(col).add_child(sec);
    let t = ctl_toggle(
        commands,
        vp.vsync,
        |w| w.resource::<ViewportSettings>().vsync,
        |w, &v| w.resource_mut::<ViewportSettings>().vsync = v,
    );
    settings_row(commands, fonts, body, 0, "VSync", t);

    let (sec, body) = section(commands, fonts, "video-camera", "Camera", A_PURPLE);
    commands.entity(col).add_child(sec);
    let cam = &vp.camera;
    let dv = ctl_drag(
        commands, fonts, cam.move_speed, 1.0, 50.0, 0.5,
        |w| w.resource::<ViewportSettings>().camera.move_speed,
        |w, &v| w.resource_mut::<ViewportSettings>().camera.move_speed = v,
    );
    settings_row(commands, fonts, body, 0, "Move Speed", dv);
    let dv = ctl_drag(
        commands, fonts, cam.look_sensitivity, 0.05, 2.0, 0.01,
        |w| w.resource::<ViewportSettings>().camera.look_sensitivity,
        |w, &v| w.resource_mut::<ViewportSettings>().camera.look_sensitivity = v,
    );
    settings_row(commands, fonts, body, 1, "Look Sensitivity", dv);
    let dv = ctl_drag(
        commands, fonts, cam.orbit_sensitivity, 0.05, 2.0, 0.01,
        |w| w.resource::<ViewportSettings>().camera.orbit_sensitivity,
        |w, &v| w.resource_mut::<ViewportSettings>().camera.orbit_sensitivity = v,
    );
    settings_row(commands, fonts, body, 2, "Orbit Sensitivity", dv);
    let dv = ctl_drag(
        commands, fonts, cam.pan_sensitivity, 0.1, 5.0, 0.01,
        |w| w.resource::<ViewportSettings>().camera.pan_sensitivity,
        |w, &v| w.resource_mut::<ViewportSettings>().camera.pan_sensitivity = v,
    );
    settings_row(commands, fonts, body, 3, "Pan Sensitivity", dv);
    let dv = ctl_drag(
        commands, fonts, cam.zoom_sensitivity, 0.1, 5.0, 0.01,
        |w| w.resource::<ViewportSettings>().camera.zoom_sensitivity,
        |w, &v| w.resource_mut::<ViewportSettings>().camera.zoom_sensitivity = v,
    );
    settings_row(commands, fonts, body, 4, "Zoom Sensitivity", dv);
    let t = ctl_toggle(
        commands, cam.invert_y,
        |w| w.resource::<ViewportSettings>().camera.invert_y,
        |w, &v| w.resource_mut::<ViewportSettings>().camera.invert_y = v,
    );
    settings_row(commands, fonts, body, 5, "Invert Y", t);
    let t = ctl_toggle(
        commands, cam.distance_relative_speed,
        |w| w.resource::<ViewportSettings>().camera.distance_relative_speed,
        |w, &v| w.resource_mut::<ViewportSettings>().camera.distance_relative_speed = v,
    );
    settings_row(commands, fonts, body, 6, "Distance Speed", t);

    let (sec, body) = section(commands, fonts, "gauge", "Gizmos", A_TEAL);
    commands.entity(col).add_child(sec);
    let dd = ctl_dropdown(
        commands, fonts, &["Selected Only", "Always"],
        match vp.collision_gizmo_visibility {
            CollisionGizmoVisibility::SelectedOnly => 0,
            CollisionGizmoVisibility::Always => 1,
        },
        |w| match w.resource::<ViewportSettings>().collision_gizmo_visibility {
            CollisionGizmoVisibility::SelectedOnly => 0,
            CollisionGizmoVisibility::Always => 1,
        },
        |w, &i| {
            w.resource_mut::<ViewportSettings>().collision_gizmo_visibility = if i == 1 {
                CollisionGizmoVisibility::Always
            } else {
                CollisionGizmoVisibility::SelectedOnly
            };
        },
    );
    settings_row(commands, fonts, body, 0, "Colliders", dd);
    let dd = ctl_dropdown(
        commands, fonts, &["Outline", "Gizmo"],
        1, // Gizmo default; reseeded by bind_2way
        |w| match w.resource::<EditorSettings>().selection_highlight_mode {
            SelectionHighlightMode::Outline => 0,
            SelectionHighlightMode::Gizmo => 1,
        },
        |w, &i| {
            w.resource_mut::<EditorSettings>().selection_highlight_mode = if i == 1 {
                SelectionHighlightMode::Gizmo
            } else {
                SelectionHighlightMode::Outline
            };
        },
    );
    settings_row(commands, fonts, body, 1, "Selection", dd);
    let dd = ctl_dropdown(
        commands, fonts, &["On Top", "Depth Tested"], 0,
        |w| {
            if w.resource::<EditorSettings>().selection_boundary_on_top {
                0
            } else {
                1
            }
        },
        |w, &i| w.resource_mut::<EditorSettings>().selection_boundary_on_top = i == 0,
    );
    settings_row(commands, fonts, body, 2, "Boundary", dd);
}

// ── Scripting ────────────────────────────────────────────────────────────────

fn tab_scripting(commands: &mut Commands, fonts: &EmberFonts, col: Entity) {
    let (sec, body) = section(commands, fonts, "code", "Scripting", A_GREEN);
    commands.entity(col).add_child(sec);
    let t = ctl_toggle(
        commands, true,
        |w| w.resource::<EditorSettings>().script_rerun_on_ready_on_reload,
        |w, &v| w.resource_mut::<EditorSettings>().script_rerun_on_ready_on_reload = v,
    );
    settings_row(commands, fonts, body, 0, "Hot Reload", t);
    let t = ctl_toggle(
        commands, true,
        |w| w.resource::<EditorSettings>().scripts_use_game_camera,
        |w, &v| w.resource_mut::<EditorSettings>().scripts_use_game_camera = v,
    );
    settings_row(commands, fonts, body, 1, "Script Camera", t);
    let t = ctl_toggle(
        commands, true,
        |w| w.resource::<EditorSettings>().hide_cursor_in_play_mode,
        |w, &v| w.resource_mut::<EditorSettings>().hide_cursor_in_play_mode = v,
    );
    settings_row(commands, fonts, body, 2, "Cursor", t);
    let t = ctl_toggle(
        commands, true,
        |w| w.resource::<EditorSettings>().external_play_window,
        |w, &v| w.resource_mut::<EditorSettings>().external_play_window = v,
    );
    settings_row(commands, fonts, body, 3, "External Window", t);

    let (sec, body) = section(commands, fonts, "code", "Code Editor", A_GREEN);
    commands.entity(col).add_child(sec);
    let t = ctl_toggle(
        commands, true,
        |w| w.resource::<EditorSettings>().code_auto_close_pairs,
        |w, &v| w.resource_mut::<EditorSettings>().code_auto_close_pairs = v,
    );
    settings_row(commands, fonts, body, 0, "Auto-close pairs", t);
    let t = ctl_toggle(
        commands, true,
        |w| w.resource::<EditorSettings>().code_trim_trailing_whitespace_on_save,
        |w, &v| w.resource_mut::<EditorSettings>().code_trim_trailing_whitespace_on_save = v,
    );
    settings_row(commands, fonts, body, 1, "Trim on save", t);
    let t = ctl_toggle(
        commands, true,
        |w| w.resource::<EditorSettings>().code_show_minimap,
        |w, &v| w.resource_mut::<EditorSettings>().code_show_minimap = v,
    );
    settings_row(commands, fonts, body, 2, "Minimap", t);
    let t = ctl_toggle(
        commands, false,
        |w| w.resource::<EditorSettings>().code_show_whitespace,
        |w, &v| w.resource_mut::<EditorSettings>().code_show_whitespace = v,
    );
    settings_row(commands, fonts, body, 3, "Whitespace markers", t);
}

// ── Assets ───────────────────────────────────────────────────────────────────

fn tab_assets(commands: &mut Commands, fonts: &EmberFonts, col: Entity) {
    let (sec, body) = section(commands, fonts, "folder-open", "Import", A_BLUE);
    commands.entity(col).add_child(sec);
    let t = ctl_toggle(
        commands, true,
        |w| w.resource::<EditorSettings>().auto_import_on_drop,
        |w, &v| w.resource_mut::<EditorSettings>().auto_import_on_drop = v,
    );
    settings_row(commands, fonts, body, 0, "Drop Import", t);
}

// ── Interaction systems ──────────────────────────────────────────────────────

fn settings_tab_click(
    btns: Query<(&Interaction, &NativeSettingsTabBtn), Changed<Interaction>>,
    mut settings: ResMut<EditorSettings>,
) {
    for (interaction, btn) in &btns {
        if *interaction == Interaction::Pressed && settings.settings_tab != btn.0 {
            settings.settings_tab = btn.0;
        }
    }
}

fn settings_close_click(
    btns: Query<&Interaction, (Changed<Interaction>, With<NativeSettingsClose>)>,
    mut settings: ResMut<EditorSettings>,
) {
    for interaction in &btns {
        if *interaction == Interaction::Pressed {
            settings.show_settings = false;
        }
    }
}

fn settings_section_toggle(
    mut headers: Query<(&Interaction, &mut SettingsSection), Changed<Interaction>>,
    mut nodes: Query<&mut Node>,
    mut texts: Query<&mut Text>,
) {
    for (interaction, mut sec) in &mut headers {
        if *interaction != Interaction::Pressed {
            continue;
        }
        sec.open = !sec.open;
        if let Ok(mut n) = nodes.get_mut(sec.body) {
            n.display = if sec.open { Display::Flex } else { Display::None };
        }
        if let Ok(mut t) = texts.get_mut(sec.caret) {
            // Phosphor caret glyphs (resolved by the icon system).
            *t = Text::new(if sec.open { "\u{E136}" } else { "\u{E13A}" });
        }
    }
}
