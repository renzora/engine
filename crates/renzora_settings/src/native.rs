//! bevy_ui-native settings overlay — a centered modal with a vertical tab
//! sidebar and a scrollable content pane, driven by
//! [`EditorSettings::show_settings`].
//!
//! Controls two-way-bind to the live resources via `bind_2way`, so edits write
//! straight back to `EditorSettings` / `ViewportSettings` the same frame.

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use bevy::window::SystemCursorIcon;

use renzora::{
    AspectMode, CurrentProject, RenderingMode, StretchMode, TextureFilter,
    WindowMode,
};
use renzora_editor::{
    CustomFonts, EditorSettings, MonoFont, SelectionHighlightMode, SettingsTab, UiFont,
};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::inspector::color_field;
use renzora_ember::reactive::{bind_2way, bind_text, bind_text_color};
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    bind_text_input, drag_value, dropdown, scroll_view_bar, section, text_input, toggle_switch,
    DragRange,
};
use renzora_hui::cursor_icon::HoverCursor;
use renzora_input::{ActionKind, InputAction, InputBinding, InputMap};
use renzora_keybindings::{EditorAction, KeyBinding, KeyBindings};
use renzora_theme::{Theme, ThemeColor, ThemeManager};
use renzora_viewport::settings::{CollisionGizmoVisibility, ViewportSettings};

const PANEL_W: f32 = 880.0;
const PANEL_H: f32 = 620.0;
const SIDEBAR_W: f32 = 160.0;

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
    /// Set by dynamic tabs (Input) to force a rebuild after a structural change
    /// (add/remove action, expand a row, enter listen mode).
    dirty: bool,
    /// Active theme name at last build — the overlay rebuilds on a theme switch
    /// so it re-spawns with the new palette (it's a separate root from the chrome
    /// and wouldn't otherwise pick up the change while open).
    built_theme: Option<String>,
}

/// Transient UI state for the Input tab (which action is expanded, whether a
/// binding capture is in progress, and the new-action name field).
#[derive(Resource, Default)]
struct NativeInputUi {
    selected: Option<usize>,
    listening: bool,
    new_name: String,
}

#[derive(Component)]
struct NativeSettingsRoot;

#[derive(Component)]
struct NativeSettingsTabBtn(SettingsTab);

#[derive(Component)]
struct NativeSettingsClose;


#[derive(Component)]
struct ThemeSaveBtn;

#[derive(Component)]
struct EmberThemeSaveBtn;

/// Snapshot of the Input tab's data, read once per (re)build.
struct InputTabData {
    actions: Vec<InputAction>,
    selected: Option<usize>,
    listening: bool,
}

#[derive(Component)]
struct RebindBtn(EditorAction);

// Input-tab markers.
#[derive(Component)]
struct AddActionBtn {
    axis: bool,
}
#[derive(Component)]
struct DeleteActionBtn(usize);
#[derive(Component)]
struct ExpandActionBtn(usize);
#[derive(Component)]
struct AddBindingBtn(usize);
#[derive(Component)]
struct CancelListenBtn;
#[derive(Component)]
struct RemoveBindingBtn {
    action: usize,
    binding: usize,
}
/// Add a WASD/Arrows composite to an Axis2D action.
#[derive(Component)]
struct CompositeBtn {
    action: usize,
    arrows: bool,
}
#[derive(Component)]
struct NewActionInput;

#[derive(Component)]
struct ResetBindingsBtn;

// ── Plugin wiring ────────────────────────────────────────────────────────────

pub(crate) fn build(app: &mut App) {
    app.init_resource::<NativeSettingsState>();
    app.init_resource::<NativeInputUi>();
    app.add_systems(
        Update,
        (
            manage_native_settings,
            settings_tab_click,
            settings_close_click,
            theme_save_click,
            ember_theme_save_click,
        )
            .run_if(in_state(renzora_editor::SplashState::Editor)),
    );
    app.add_systems(
        Update,
        (
            add_action_click,
            delete_action_click,
            expand_action_click,
            add_binding_click,
            cancel_listen_click,
            remove_binding_click,
            composite_click,
        )
            .run_if(in_state(renzora_editor::SplashState::Editor)),
    );
    // Key/mouse-rebind capture.
    app.add_systems(
        Update,
        (rebind_btn_click, rebind_capture, reset_bindings_click, input_listen_capture)
            .run_if(in_state(renzora_editor::SplashState::Editor)),
    );
}

// ── Lifecycle: spawn / despawn / rebuild on tab change ───────────────────────

fn manage_native_settings(world: &mut World) {
    let (show, tab) = world
        .get_resource::<EditorSettings>()
        .map(|s| (s.show_settings, s.settings_tab))
        .unwrap_or((false, SettingsTab::default()));
    let open = show;
    let theme_name = world
        .get_resource::<ThemeManager>()
        .map(|t| t.active_theme_name.clone());

    let st = world.resource::<NativeSettingsState>();
    // Rebuild when the active theme switches so the overlay re-spawns with the
    // new palette (it's a separate root from the chrome).
    let theme_changed = st.built_theme != theme_name;
    let (root, built, dirty) = (st.root, st.built_tab, st.dirty || theme_changed);

    if !open {
        if let Some(r) = root {
            if let Ok(e) = world.get_entity_mut(r) {
                e.despawn();
            }
            let mut st = world.resource_mut::<NativeSettingsState>();
            st.root = None;
            st.built_tab = None;
            st.dirty = false;
        }
        return;
    }

    // Already built for this tab and nothing structural changed → nothing to do.
    if root.is_some() && built == Some(tab) && !dirty {
        return;
    }
    // Tab switch, first open, or a dirty rebuild → tear down + rebuild.
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
    st.dirty = false;
    st.built_theme = theme_name;
}

fn build_overlay(world: &mut World, tab: SettingsTab) -> Option<Entity> {
    let fonts = world.get_resource::<EmberFonts>().cloned()?;
    let settings = world.get_resource::<EditorSettings>()?.clone();
    let viewport = world.get_resource::<ViewportSettings>().cloned().unwrap_or_default();
    let custom = world
        .get_resource::<CustomFonts>()
        .map(|c| c.names.clone())
        .unwrap_or_default();
    let themes = world
        .get_resource::<ThemeManager>()
        .map(|tm| tm.available_themes.clone())
        .unwrap_or_default();
    let has_project = world.get_resource::<CurrentProject>().is_some();
    let scenes = scan_scenes(world);
    let input = InputTabData {
        actions: world
            .get_resource::<InputMap>()
            .map(|m| m.actions.clone())
            .unwrap_or_default(),
        selected: world.get_resource::<NativeInputUi>().and_then(|u| u.selected),
        listening: world
            .get_resource::<NativeInputUi>()
            .map(|u| u.listening)
            .unwrap_or(false),
    };

    let mut queue = bevy::ecs::world::CommandQueue::default();
    let root = {
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
        )
    };
    queue.apply(world);
    Some(root)
}

/// Scan `<project>/scenes/*.ron` for the boot-scene / autoload pickers.
fn scan_scenes(world: &World) -> Vec<String> {
    let Some(cp) = world.get_resource::<CurrentProject>() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(cp.path.join("scenes")) {
        for entry in rd.flatten() {
            let p = entry.path();
            if p.extension().and_then(|s| s.to_str()) == Some("ron") {
                if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                    out.push(format!("scenes/{name}"));
                }
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
            // Capture the wheel so scrolling doesn't bleed to the dock behind.
            renzora_ember::widgets::ModalSurface,
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
            BackgroundColor(rgb(renzora_ember::theme::window_bg())),
            FocusPolicy::Block,
            Name::new("settings-panel"),
        ))
        .id();
    commands.entity(root).add_child(panel);

    let title = build_title_bar(commands, fonts);
    let body = build_body(
        commands, fonts, tab, settings, viewport, custom, themes, scenes, has_project, input,
    );
    commands.entity(panel).add_children(&[title, body]);
    root
}

fn build_title_bar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let label = commands
        .spawn((
            Text::new("Settings"),
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
        .insert((FocusPolicy::Block, NativeSettingsClose));

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
) -> Entity {
    let sidebar = build_sidebar(commands, fonts, tab);

    let content_col = build_tab_content(
        commands, fonts, tab, settings, viewport, custom, themes, scenes, has_project, input,
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
            BackgroundColor(rgb(renzora_ember::theme::window_bg())),
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
            NativeSettingsTabBtn(tab),
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("settings-tab"),
        ))
        .id();
    // Active → highlighted; otherwise a themed hover wash.
    renzora_ember::reactive::bind_bg(commands, row, move |w| {
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

// ── Row helpers (section is the shared ember widget) ─────────────────────────

/// A labeled, zebra-striped form row — the shared ember `inspector_row` + its
/// stripe color, parented under `body`.
fn settings_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    body: Entity,
    idx: usize,
    label: &str,
    control: Entity,
) {
    let row = renzora_ember::inspector::inspector_row(commands, &fonts.ui, label, control);
    commands
        .entity(row)
        .insert(BackgroundColor(renzora_ember::inspector::inspector_stripe(idx)));
    commands.entity(body).add_child(row);
}

/// A muted, control-less note row (the "takes effect after restart" lines).
fn note_row(commands: &mut Commands, fonts: &EmberFonts, body: Entity, text: &str) {
    let lbl = commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(renzora_ember::theme::text_muted())),
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
    let dv = drag_value(commands, &fonts.ui, "", renzora_ember::theme::value_text(), init, step);
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

#[allow(clippy::too_many_arguments)]
fn build_tab_content(
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
        SettingsTab::Project => tab_project(commands, fonts, col, scenes, has_project),
        SettingsTab::Interface => tab_interface(commands, fonts, col, settings, custom),
        SettingsTab::Editor => tab_editor(commands, fonts, col),
        SettingsTab::Viewport => tab_viewport(commands, fonts, col, viewport),
        SettingsTab::Scripting => tab_scripting(commands, fonts, col),
        SettingsTab::Assets => tab_assets(commands, fonts, col),
        SettingsTab::Theme => tab_theme(commands, fonts, col, themes),
        SettingsTab::Shortcuts => tab_shortcuts(commands, fonts, col),
        SettingsTab::Input => tab_input(commands, fonts, col, input),
        SettingsTab::Plugins => placeholder(commands, fonts, col),
    }
    col
}

fn placeholder(commands: &mut Commands, fonts: &EmberFonts, col: Entity) {
    let lbl = commands
        .spawn((
            Text::new("This tab is being migrated to bevy_ui."),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
            Node {
                margin: UiRect::all(Val::Px(12.0)),
                ..default()
            },
        ))
        .id();
    commands.entity(col).add_child(lbl);
}

// ── Project ──────────────────────────────────────────────────────────────────

fn save_project(w: &mut World) {
    if let Some(cp) = w.get_resource::<CurrentProject>() {
        let _ = cp.save_config();
    }
}

fn tab_project(
    commands: &mut Commands,
    fonts: &EmberFonts,
    col: Entity,
    scenes: &[String],
    has_project: bool,
) {
    if !has_project {
        let lbl = commands
            .spawn((
                Text::new("No project is currently loaded."),
                ui_font(&fonts.ui, 12.0),
                TextColor(rgb(text_muted())),
                Node {
                    margin: UiRect::all(Val::Px(12.0)),
                    ..default()
                },
            ))
            .id();
        commands.entity(col).add_child(lbl);
        return;
    }

    let (sec, body) = section(commands, fonts, "folder-open", "Project", A_BLUE);
    commands.entity(col).add_child(sec);
    let ti = text_input(commands, &fonts.ui, "Project name", "");
    bind_text_input(
        commands,
        ti,
        |w| {
            w.get_resource::<CurrentProject>()
                .map(|c| c.config.name.clone())
                .unwrap_or_default()
        },
        |w, s| {
            if let Some(mut cp) = w.get_resource_mut::<CurrentProject>() {
                cp.config.name = s;
            }
            save_project(w);
        },
    );
    settings_row(commands, fonts, body, 0, "Name", ti);

    let scene_opts: Vec<&str> = scenes.iter().map(|s| s.as_str()).collect();
    let sc1 = scenes.to_vec();
    let sc2 = scenes.to_vec();
    let dd = ctl_dropdown(
        commands,
        fonts,
        &scene_opts,
        0,
        move |w| {
            let cur = w
                .get_resource::<CurrentProject>()
                .map(|c| c.config.main_scene.clone())
                .unwrap_or_default();
            sc1.iter().position(|n| *n == cur).unwrap_or(0)
        },
        move |w, &i| {
            if let Some(name) = sc2.get(i).cloned() {
                if let Some(mut cp) = w.get_resource_mut::<CurrentProject>() {
                    cp.config.main_scene = name;
                }
                save_project(w);
            }
        },
    );
    settings_row(commands, fonts, body, 1, "Boot Scene", dd);

    // Rendering (3D pipeline).
    let (sec, body) = section(commands, fonts, "monitor", "Rendering", A_BLUE);
    commands.entity(col).add_child(sec);
    let dd = ctl_dropdown(
        commands,
        fonts,
        &["Auto (per platform)", "Forward", "Deferred"],
        0,
        |w| match w
            .get_resource::<CurrentProject>()
            .map(|c| c.config.rendering.mode)
            .unwrap_or_default()
        {
            RenderingMode::Auto => 0,
            RenderingMode::Forward => 1,
            RenderingMode::Deferred => 2,
        },
        |w, &i| {
            let m = match i {
                1 => RenderingMode::Forward,
                2 => RenderingMode::Deferred,
                _ => RenderingMode::Auto,
            };
            if let Some(mut cp) = w.get_resource_mut::<CurrentProject>() {
                cp.config.rendering.mode = m;
            }
            save_project(w);
        },
    );
    settings_row(commands, fonts, body, 0, "Mode", dd);
    note_row(commands, fonts, body, "Restart required for the rendering mode to take effect.");

    // Window.
    let (sec, body) = section(commands, fonts, "desktop", "Window", A_BLUE);
    commands.entity(col).add_child(sec);
    let dv = proj_u32_drag(
        commands, fonts, 320.0, 7680.0,
        |c| c.window.width,
        |c, v| c.window.width = v,
    );
    settings_row(commands, fonts, body, 0, "Width", dv);
    let dv = proj_u32_drag(
        commands, fonts, 240.0, 4320.0,
        |c| c.window.height,
        |c, v| c.window.height = v,
    );
    settings_row(commands, fonts, body, 1, "Height", dv);
    let t = ctl_toggle(
        commands,
        true,
        |w| {
            w.get_resource::<CurrentProject>()
                .map(|c| c.config.window.resizable)
                .unwrap_or(true)
        },
        |w, &v| {
            if let Some(mut cp) = w.get_resource_mut::<CurrentProject>() {
                cp.config.window.resizable = v;
            }
            save_project(w);
        },
    );
    settings_row(commands, fonts, body, 2, "Resizable", t);
    let dd = ctl_dropdown(
        commands,
        fonts,
        &["Windowed", "Fullscreen", "Borderless"],
        0,
        |w| match w
            .get_resource::<CurrentProject>()
            .map(|c| c.config.window.mode)
            .unwrap_or_default()
        {
            WindowMode::Windowed => 0,
            WindowMode::Fullscreen => 1,
            WindowMode::Borderless => 2,
        },
        |w, &i| {
            let m = match i {
                1 => WindowMode::Fullscreen,
                2 => WindowMode::Borderless,
                _ => WindowMode::Windowed,
            };
            if let Some(mut cp) = w.get_resource_mut::<CurrentProject>() {
                cp.config.window.mode = m;
            }
            save_project(w);
        },
    );
    settings_row(commands, fonts, body, 3, "Mode", dd);

    // Viewport (render resolution).
    let (sec, body) = section(commands, fonts, "video-camera", "Viewport", A_PURPLE);
    commands.entity(col).add_child(sec);
    let dd = ctl_dropdown(
        commands,
        fonts,
        &["Disabled", "Viewport"],
        0,
        |w| match w
            .get_resource::<CurrentProject>()
            .map(|c| c.config.viewport.stretch_mode)
            .unwrap_or_default()
        {
            StretchMode::Disabled => 0,
            StretchMode::Viewport => 1,
        },
        |w, &i| {
            let m = if i == 1 {
                StretchMode::Viewport
            } else {
                StretchMode::Disabled
            };
            if let Some(mut cp) = w.get_resource_mut::<CurrentProject>() {
                cp.config.viewport.stretch_mode = m;
            }
            save_project(w);
        },
    );
    settings_row(commands, fonts, body, 0, "Stretch Mode", dd);
    let dv = proj_u32_drag(
        commands, fonts, 16.0, 7680.0,
        |c| c.viewport.width,
        |c, v| c.viewport.width = v,
    );
    settings_row(commands, fonts, body, 1, "Width", dv);
    let dv = proj_u32_drag(
        commands, fonts, 16.0, 4320.0,
        |c| c.viewport.height,
        |c, v| c.viewport.height = v,
    );
    settings_row(commands, fonts, body, 2, "Height", dv);
    let dd = ctl_dropdown(
        commands,
        fonts,
        &["Keep", "Expand", "Keep Width", "Keep Height"],
        0,
        |w| match w
            .get_resource::<CurrentProject>()
            .map(|c| c.config.viewport.aspect_mode)
            .unwrap_or_default()
        {
            AspectMode::Keep => 0,
            AspectMode::Expand => 1,
            AspectMode::KeepWidth => 2,
            AspectMode::KeepHeight => 3,
        },
        |w, &i| {
            let m = match i {
                1 => AspectMode::Expand,
                2 => AspectMode::KeepWidth,
                3 => AspectMode::KeepHeight,
                _ => AspectMode::Keep,
            };
            if let Some(mut cp) = w.get_resource_mut::<CurrentProject>() {
                cp.config.viewport.aspect_mode = m;
            }
            save_project(w);
        },
    );
    settings_row(commands, fonts, body, 3, "Aspect Mode", dd);

    // Rendering 2D.
    let (sec, body) = section(commands, fonts, "image-square", "Rendering 2D", A_BLUE);
    commands.entity(col).add_child(sec);
    let dd = ctl_dropdown(
        commands,
        fonts,
        &["Nearest", "Linear"],
        0,
        |w| match w
            .get_resource::<CurrentProject>()
            .map(|c| c.config.rendering_2d.image_filter)
            .unwrap_or_default()
        {
            TextureFilter::Nearest => 0,
            TextureFilter::Linear => 1,
        },
        |w, &i| {
            let f = if i == 1 {
                TextureFilter::Linear
            } else {
                TextureFilter::Nearest
            };
            if let Some(mut cp) = w.get_resource_mut::<CurrentProject>() {
                cp.config.rendering_2d.image_filter = f;
            }
            save_project(w);
        },
    );
    settings_row(commands, fonts, body, 0, "Image Filter", dd);
}

/// A drag-value bound to a `u32` field of the current project's config,
/// saving project.toml on edit.
fn proj_u32_drag(
    commands: &mut Commands,
    fonts: &EmberFonts,
    min: f32,
    max: f32,
    get: fn(&renzora::ProjectConfig) -> u32,
    set: fn(&mut renzora::ProjectConfig, u32),
) -> Entity {
    ctl_drag(
        commands,
        fonts,
        min,
        min,
        max,
        1.0,
        move |w| {
            w.get_resource::<CurrentProject>()
                .map(|c| get(&c.config) as f32)
                .unwrap_or(0.0)
        },
        move |w, &v| {
            if let Some(mut cp) = w.get_resource_mut::<CurrentProject>() {
                set(&mut cp.config, v.round().max(0.0) as u32);
            }
            save_project(w);
        },
    )
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

// ── Theme ────────────────────────────────────────────────────────────────────

const A_VIOLET: (u8, u8, u8) = (180, 130, 230);

fn tab_theme(commands: &mut Commands, fonts: &EmberFonts, col: Entity, themes: &[String]) {
    let (sec, body) = section(commands, fonts, "palette", "Active Theme", A_VIOLET);
    commands.entity(col).add_child(sec);

    let opts: Vec<&str> = themes.iter().map(|s| s.as_str()).collect();
    let th1 = themes.to_vec();
    let th2 = themes.to_vec();
    let dd = ctl_dropdown(
        commands,
        fonts,
        &opts,
        0,
        move |w| {
            let name = &w.resource::<ThemeManager>().active_theme_name;
            th1.iter().position(|n| n == name).unwrap_or(0)
        },
        move |w, &i| {
            if let Some(name) = th2.get(i).cloned() {
                w.resource_mut::<ThemeManager>().load_theme(&name);
            }
        },
    );
    settings_row(commands, fonts, body, 0, "Theme", dd);

    // Save button row.
    let save_lbl = commands
        .spawn((
            Text::new("Save"),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let save = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(12.0), Val::Px(5.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(tab_active())),
            Interaction::default(),
            ThemeSaveBtn,
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("theme-save"),
        ))
        .id();
    commands.entity(save).add_child(save_lbl);
    settings_row(commands, fonts, body, 1, "", save);

    // Color-editing sections.
    let (sec, body) = section(commands, fonts, "palette", "Semantic Colors", A_VIOLET);
    commands.entity(col).add_child(sec);
    theme_color_row(commands, fonts, body, 0, "Accent", |t| t.semantic.accent, |t, c| t.semantic.accent = c);
    theme_color_row(commands, fonts, body, 1, "Success", |t| t.semantic.success, |t, c| t.semantic.success = c);
    theme_color_row(commands, fonts, body, 2, "Warning", |t| t.semantic.warning, |t, c| t.semantic.warning = c);
    theme_color_row(commands, fonts, body, 3, "Error", |t| t.semantic.error, |t, c| t.semantic.error = c);
    theme_color_row(commands, fonts, body, 4, "Selection", |t| t.semantic.selection, |t, c| t.semantic.selection = c);
    theme_color_row(commands, fonts, body, 5, "Sel. Stroke", |t| t.semantic.selection_stroke, |t, c| t.semantic.selection_stroke = c);

    let (sec, body) = section(commands, fonts, "palette", "Surfaces", A_VIOLET);
    commands.entity(col).add_child(sec);
    theme_color_row(commands, fonts, body, 0, "Window", |t| t.surfaces.window, |t, c| t.surfaces.window = c);
    theme_color_row(commands, fonts, body, 1, "Window Stroke", |t| t.surfaces.window_stroke, |t, c| t.surfaces.window_stroke = c);
    theme_color_row(commands, fonts, body, 2, "Panel", |t| t.surfaces.panel, |t, c| t.surfaces.panel = c);
    theme_color_row(commands, fonts, body, 3, "Popup", |t| t.surfaces.popup, |t, c| t.surfaces.popup = c);
    theme_color_row(commands, fonts, body, 4, "Faint", |t| t.surfaces.faint, |t, c| t.surfaces.faint = c);
    theme_color_row(commands, fonts, body, 5, "Extreme", |t| t.surfaces.extreme, |t, c| t.surfaces.extreme = c);

    let (sec, body) = section(commands, fonts, "palette", "Text", A_VIOLET);
    commands.entity(col).add_child(sec);
    theme_color_row(commands, fonts, body, 0, "Primary", |t| t.text.primary, |t, c| t.text.primary = c);
    theme_color_row(commands, fonts, body, 1, "Secondary", |t| t.text.secondary, |t, c| t.text.secondary = c);
    theme_color_row(commands, fonts, body, 2, "Muted", |t| t.text.muted, |t, c| t.text.muted = c);
    theme_color_row(commands, fonts, body, 3, "Heading", |t| t.text.heading, |t, c| t.text.heading = c);
    theme_color_row(commands, fonts, body, 4, "Disabled", |t| t.text.disabled, |t, c| t.text.disabled = c);
    theme_color_row(commands, fonts, body, 5, "Hyperlink", |t| t.text.hyperlink, |t, c| t.text.hyperlink = c);

    let (sec, body) = section(commands, fonts, "palette", "Widgets", A_VIOLET);
    commands.entity(col).add_child(sec);
    theme_color_row(commands, fonts, body, 0, "Inactive BG", |t| t.widgets.inactive_bg, |t, c| t.widgets.inactive_bg = c);
    theme_color_row(commands, fonts, body, 1, "Inactive FG", |t| t.widgets.inactive_fg, |t, c| t.widgets.inactive_fg = c);
    theme_color_row(commands, fonts, body, 2, "Hovered BG", |t| t.widgets.hovered_bg, |t, c| t.widgets.hovered_bg = c);
    theme_color_row(commands, fonts, body, 3, "Hovered FG", |t| t.widgets.hovered_fg, |t, c| t.widgets.hovered_fg = c);
    theme_color_row(commands, fonts, body, 4, "Active BG", |t| t.widgets.active_bg, |t, c| t.widgets.active_bg = c);
    theme_color_row(commands, fonts, body, 5, "Active FG", |t| t.widgets.active_fg, |t, c| t.widgets.active_fg = c);
    theme_color_row(commands, fonts, body, 6, "Border", |t| t.widgets.border, |t, c| t.widgets.border = c);

    let (sec, body) = section(commands, fonts, "palette", "Panels", A_VIOLET);
    commands.entity(col).add_child(sec);
    theme_color_row(commands, fonts, body, 0, "Tree Line", |t| t.panels.tree_line, |t, c| t.panels.tree_line = c);
    theme_color_row(commands, fonts, body, 1, "Drop Line", |t| t.panels.drop_line, |t, c| t.panels.drop_line = c);
    theme_color_row(commands, fonts, body, 2, "Tab Active", |t| t.panels.tab_active, |t, c| t.panels.tab_active = c);
    theme_color_row(commands, fonts, body, 3, "Tab Inactive", |t| t.panels.tab_inactive, |t, c| t.panels.tab_inactive = c);

    // ── Per-widget style editor ──────────────────────────────────────────────
    // Walk the ember `Theme` via reflection: every widget type → a section, every
    // element → a color picker (Rgba) or number field (f32), bound by reflect
    // path. Editing repaints the live `Styled` widgets immediately. Adding a
    // widget/element to the Theme makes it appear here automatically.
    let hdr = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                margin: UiRect::new(Val::Px(4.0), Val::Px(0.0), Val::Px(10.0), Val::Px(4.0)),
                ..default()
            },
            Name::new("widget-styles-header"),
        ))
        .id();
    let hdr_lbl = commands
        .spawn((
            Text::new("Widget Styles"),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
            Node {
                flex_grow: 1.0,
                ..default()
            },
        ))
        .id();
    let save_lbl = commands
        .spawn((
            Text::new("Save to theme.toml"),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let save = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(tab_active())),
            Interaction::default(),
            EmberThemeSaveBtn,
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("ember-theme-save"),
        ))
        .id();
    commands.entity(save).add_child(save_lbl);
    commands.entity(hdr).add_children(&[hdr_lbl, save]);
    commands.entity(col).add_child(hdr);

    for (widget, elems) in theme_schema() {
        let (sec, body) = section(commands, fonts, "stack", &prettify(&widget), A_VIOLET);
        commands.entity(col).add_child(sec);
        for (i, (elem, kind)) in elems.into_iter().enumerate() {
            let path = format!("{widget}.{elem}");
            let label = prettify(&elem);
            match kind {
                0 => widget_color_row(commands, fonts, body, i, &label, path),
                1 => widget_num_row(commands, fonts, body, i, &label, path),
                _ => widget_bool_row(commands, fonts, body, i, &label, path),
            }
        }
    }
}

/// `button_accent` → `Button Accent`.
fn prettify(s: &str) -> String {
    s.split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Reflect-walk the ember `Theme`: `[(widget, [(element, kind)])]` where kind is
/// 0 = color (Rgba), 1 = number (f32), 2 = bool. Read from `Theme::default()`
/// (the structure is static), so no world needed.
fn theme_schema() -> Vec<(String, Vec<(String, u8)>)> {
    use bevy::reflect::{PartialReflect, ReflectRef};
    let theme = renzora_ember::style::Theme::default();
    let mut out = Vec::new();
    let ReflectRef::Struct(s) = theme.reflect_ref() else {
        return out;
    };
    for i in 0..s.field_len() {
        let wname = s.name_at(i).unwrap_or_default().to_string();
        let Some(wfield) = s.field_at(i) else { continue };
        let ReflectRef::Struct(ws) = wfield.reflect_ref() else {
            continue;
        };
        let mut elems = Vec::new();
        for j in 0..ws.field_len() {
            let ename = ws.name_at(j).unwrap_or_default().to_string();
            let Some(ef) = ws.field_at(j) else { continue };
            let kind = if ef.try_downcast_ref::<renzora_ember::style::Rgba>().is_some() {
                0u8
            } else if ef.try_downcast_ref::<f32>().is_some() {
                1u8
            } else if ef.try_downcast_ref::<bool>().is_some() {
                2u8
            } else {
                continue;
            };
            elems.push((ename, kind));
        }
        if !elems.is_empty() {
            out.push((wname, elems));
        }
    }
    out
}

/// A toggle row bound to one ember-`Theme` bool element (e.g. dock.shadow).
fn widget_bool_row(commands: &mut Commands, fonts: &EmberFonts, body: Entity, idx: usize, label: &str, path: String) {
    use bevy::reflect::GetPath;
    use renzora_ember::style::Theme as EmberTheme;
    let p = path.clone();
    let sw = toggle_switch(commands, false);
    bind_2way(
        commands,
        sw,
        move |w| w.resource::<EmberTheme>().path::<bool>(p.as_str()).ok().copied().unwrap_or(false),
        move |w, v: &bool| {
            if let Ok(b) = w.resource_mut::<EmberTheme>().path_mut::<bool>(path.as_str()) {
                *b = *v;
            }
        },
    );
    settings_row(commands, fonts, body, idx, label, sw);
}

/// A color row bound to one ember-`Theme` element by reflect `path`
/// (e.g. `button.bg`, `node_graph.cable`). Editing repaints live.
fn widget_color_row(commands: &mut Commands, fonts: &EmberFonts, body: Entity, idx: usize, label: &str, path: String) {
    use bevy::reflect::GetPath;
    use renzora_ember::style::{Rgba, Theme as EmberTheme};
    let p = path.clone();
    let cf = color_field(
        commands,
        move |w| {
            let r = w
                .resource::<EmberTheme>()
                .path::<Rgba>(p.as_str())
                .ok()
                .copied()
                .unwrap_or(Rgba::NONE);
            [r.r as f32 / 255.0, r.g as f32 / 255.0, r.b as f32 / 255.0]
        },
        move |w, rgb| {
            let mut t = w.resource_mut::<EmberTheme>();
            if let Ok(r) = t.path_mut::<Rgba>(path.as_str()) {
                r.r = (rgb[0] * 255.0).round() as u8;
                r.g = (rgb[1] * 255.0).round() as u8;
                r.b = (rgb[2] * 255.0).round() as u8;
            }
        },
    );
    settings_row(commands, fonts, body, idx, label, cf);
}

/// A number row bound to one ember-`Theme` f32 element (radius/padding/border).
fn widget_num_row(commands: &mut Commands, fonts: &EmberFonts, body: Entity, idx: usize, label: &str, path: String) {
    use bevy::reflect::GetPath;
    use renzora_ember::style::Theme as EmberTheme;
    let p = path.clone();
    let dv = ctl_drag(
        commands,
        fonts,
        0.0,
        0.0,
        64.0,
        0.5,
        move |w| {
            w.resource::<EmberTheme>()
                .path::<f32>(p.as_str())
                .ok()
                .copied()
                .unwrap_or(0.0)
        },
        move |w, &v| {
            let mut t = w.resource_mut::<EmberTheme>();
            if let Ok(f) = t.path_mut::<f32>(path.as_str()) {
                *f = v;
            }
        },
    );
    settings_row(commands, fonts, body, idx, label, dv);
}

/// A theme color row: a swatch/picker two-way bound to one `Theme` color,
/// updating the live `ThemeManager.active_theme` (+ `mark_modified`) on edit.
fn theme_color_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    body: Entity,
    idx: usize,
    label: &str,
    get: fn(&Theme) -> ThemeColor,
    set: fn(&mut Theme, ThemeColor),
) {
    let cf = color_field(
        commands,
        move |w| {
            let [r, g, b, _] = get(&w.resource::<ThemeManager>().active_theme).0;
            [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
        },
        move |w, rgb| {
            let mut tm = w.resource_mut::<ThemeManager>();
            let a = get(&tm.active_theme).0[3];
            let col = ThemeColor::with_alpha(
                (rgb[0] * 255.0).round() as u8,
                (rgb[1] * 255.0).round() as u8,
                (rgb[2] * 255.0).round() as u8,
                a,
            );
            set(&mut tm.active_theme, col);
            tm.mark_modified();
        },
    );
    settings_row(commands, fonts, body, idx, label, cf);
}

fn theme_save_click(
    btns: Query<&Interaction, (Changed<Interaction>, With<ThemeSaveBtn>)>,
    mut tm: ResMut<ThemeManager>,
) {
    for interaction in &btns {
        if *interaction == Interaction::Pressed {
            let name = tm.active_theme_name.clone();
            tm.save_theme(&name);
        }
    }
}

/// Write the live ember `Theme`'s per-widget style sections into the active
/// theme's `themes/<name>.toml`, preserving any existing (e.g. egui color)
/// sections. The bridge / runtime reloads these via `Theme::from_toml`.
fn ember_theme_save_click(world: &mut World) {
    let pressed = {
        let mut q = world
            .query_filtered::<&Interaction, (Changed<Interaction>, With<EmberThemeSaveBtn>)>();
        q.iter(world).any(|i| *i == Interaction::Pressed)
    };
    if !pressed {
        return;
    }
    let Some(project) = world.get_resource::<CurrentProject>().cloned() else {
        return;
    };
    let name = world
        .get_resource::<ThemeManager>()
        .map(|t| t.active_theme_name.clone())
        .unwrap_or_default();
    let theme = world.resource::<renzora_ember::style::Theme>().clone();

    let dir = project.path.join("themes");
    let path = dir.join(format!("{name}.toml"));
    // Preserve existing sections; overwrite the per-widget style tables.
    let mut doc: toml::value::Table = std::fs::read_to_string(&path)
        .ok()
        .and_then(|c| toml::from_str(&c).ok())
        .unwrap_or_default();
    if let Ok(toml::Value::Table(t)) = toml::Value::try_from(&theme) {
        for (k, v) in t {
            doc.insert(k, v);
        }
    }
    if let Ok(out) = toml::to_string_pretty(&toml::Value::Table(doc)) {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(&path, out);
    }
}

// ── Input ────────────────────────────────────────────────────────────────────

fn kind_label(k: ActionKind) -> &'static str {
    match k {
        ActionKind::Button => "Button",
        ActionKind::Axis1D => "Axis1D",
        ActionKind::Axis2D => "Axis2D",
    }
}

fn format_binding(b: &InputBinding) -> String {
    match b {
        InputBinding::Key(s) => s.clone(),
        InputBinding::MouseButton(s) => format!("Mouse {s}"),
        InputBinding::GamepadButton(s) => format!("Pad {s}"),
        InputBinding::GamepadAxis(s) => format!("Axis {s}"),
        InputBinding::Composite2D {
            up,
            down,
            left,
            right,
        } => format!("{up} {left} {down} {right}"),
    }
}

/// A small text button carrying a marker component.
/// A themed ember button (Styled(Role::Button)) carrying a marker — picks up
/// Theme.button + hover/press states, editable under "Button" in the Theme tab.
fn text_button<M: Component>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    marker: M,
) -> Entity {
    let btn = renzora_ember::widgets::button(commands, &fonts.ui, label);
    commands.entity(btn).insert(marker);
    btn
}

/// A horizontal container with the given children — a row inside a section body.
fn hrow(commands: &mut Commands, kids: &[Entity]) -> Entity {
    let row = commands
        .spawn((Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            ..default()
        },))
        .id();
    commands.entity(row).add_children(kids);
    row
}

fn tab_input(commands: &mut Commands, fonts: &EmberFonts, col: Entity, input: &InputTabData) {
    // About.
    let (sec, body) = section(commands, fonts, "info", "About Input Actions", A_BLUE);
    commands.entity(col).add_child(sec);
    note_row(
        commands,
        fonts,
        body,
        "Named actions map physical inputs (keys, mouse, gamepad) to gameplay. Scripts read them by name.",
    );

    // Add Action.
    let (sec, body) = section(commands, fonts, "list-plus", "Add Action", A_GREEN);
    commands.entity(col).add_child(sec);
    let ti = text_input(commands, &fonts.ui, "Action name...", "");
    commands.entity(ti).insert(NewActionInput);
    bind_text_input(
        commands,
        ti,
        |w| w.resource::<NativeInputUi>().new_name.clone(),
        |w, s| w.resource_mut::<NativeInputUi>().new_name = s,
    );
    let btn_b = text_button(commands, fonts, "Button", AddActionBtn { axis: false });
    let btn_a = text_button(commands, fonts, "Axis2D", AddActionBtn { axis: true });
    let row = hrow(commands, &[ti, btn_b, btn_a]);
    commands.entity(body).add_child(row);

    // Input Actions list.
    let (sec, body) = section(commands, fonts, "game-controller", "Input Actions", A_PURPLE);
    commands.entity(col).add_child(sec);
    for (i, action) in input.actions.iter().enumerate() {
        let expanded = input.selected == Some(i);
        build_action_row(commands, fonts, body, i, action, expanded, input.listening);
    }
}

fn build_action_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    body: Entity,
    i: usize,
    action: &InputAction,
    expanded: bool,
    listening: bool,
) {
    // Header row: caret + name + kind + delete.
    let caret = icon_text(
        commands,
        &fonts.phosphor,
        if expanded { "caret-down" } else { "caret-right" },
        text_muted(),
        12.0,
    );
    commands
        .entity(caret)
        .insert((Interaction::default(), ExpandActionBtn(i), HoverCursor(SystemCursorIcon::Pointer)));
    let name = commands
        .spawn((
            Text::new(action.name.clone()),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
            Node {
                flex_grow: 1.0,
                ..default()
            },
            Interaction::default(),
            ExpandActionBtn(i),
        ))
        .id();
    let kind = commands
        .spawn((
            Text::new(kind_label(action.kind)),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let del = icon_text(commands, &fonts.phosphor, "trash", text_muted(), 13.0);
    commands.entity(del).insert((
        Interaction::default(),
        FocusPolicy::Block,
        DeleteActionBtn(i),
        HoverCursor(SystemCursorIcon::Pointer),
    ));
    let header = commands
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
            BackgroundColor(rgb(renzora_ember::theme::row_odd())),
        ))
        .id();
    commands.entity(header).add_children(&[caret, name, kind, del]);
    commands.entity(body).add_child(header);

    if !expanded {
        return;
    }

    // Expanded panel.
    let panel = commands
        .spawn((Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            padding: UiRect {
                left: Val::Px(18.0),
                top: Val::Px(2.0),
                bottom: Val::Px(6.0),
                ..default()
            },
            ..default()
        },))
        .id();
    commands.entity(body).add_child(panel);

    if action.kind != ActionKind::Button {
        let dv = ctl_drag(
            commands,
            fonts,
            action.dead_zone,
            0.0,
            0.5,
            0.01,
            move |w| {
                w.resource::<InputMap>()
                    .actions
                    .get(i)
                    .map(|a| a.dead_zone)
                    .unwrap_or(0.0)
            },
            move |w, &v| {
                if let Some(mut m) = w.get_resource_mut::<InputMap>() {
                    if let Some(a) = m.actions.get_mut(i) {
                        a.dead_zone = v;
                    }
                }
                save_input(w);
            },
        );
        settings_row(commands, fonts, panel, 0, "Dead Zone", dv);
    }

    // Existing bindings.
    for (j, b) in action.bindings.iter().enumerate() {
        let lbl = commands
            .spawn((
                Text::new(format_binding(b)),
                ui_font(&fonts.ui, 11.0),
                TextColor(rgb(renzora_ember::theme::value_text())),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
            ))
            .id();
        let rm = icon_text(commands, &fonts.phosphor, "trash", text_muted(), 12.0);
        commands.entity(rm).insert((
            Interaction::default(),
            FocusPolicy::Block,
            RemoveBindingBtn { action: i, binding: j },
            HoverCursor(SystemCursorIcon::Pointer),
        ));
        let row = hrow(commands, &[lbl, rm]);
        commands.entity(panel).add_child(row);
    }

    // Add-binding / listen prompt.
    if listening {
        let prompt = commands
            .spawn((
                Text::new("Press any key or mouse button..."),
                ui_font(&fonts.ui, 11.0),
                TextColor(rgb(renzora_ember::theme::warn_amber())),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
            ))
            .id();
        let cancel = text_button(commands, fonts, "Cancel", CancelListenBtn);
        let row = hrow(commands, &[prompt, cancel]);
        commands.entity(panel).add_child(row);
    } else {
        let add = text_button(commands, fonts, "+ Add Binding", AddBindingBtn(i));
        let mut kids = vec![add];
        if action.kind == ActionKind::Axis2D {
            kids.push(text_button(commands, fonts, "WASD", CompositeBtn { action: i, arrows: false }));
            kids.push(text_button(commands, fonts, "Arrows", CompositeBtn { action: i, arrows: true }));
        }
        let row = hrow(commands, &kids);
        commands.entity(panel).add_child(row);
    }
}

fn save_input(w: &mut World) {
    let (Some(map), Some(project)) = (
        w.get_resource::<InputMap>().cloned(),
        w.get_resource::<CurrentProject>().cloned(),
    ) else {
        return;
    };
    let _ = renzora_input::save_input_map(&map, &project);
}

fn mark_dirty(w: &mut World) {
    if let Some(mut st) = w.get_resource_mut::<NativeSettingsState>() {
        st.dirty = true;
    }
}

fn add_action_click(world: &mut World) {
    let mut to_add: Option<bool> = None;
    let mut q = world.query_filtered::<(&Interaction, &AddActionBtn), Changed<Interaction>>();
    for (interaction, btn) in q.iter(world) {
        if *interaction == Interaction::Pressed {
            to_add = Some(btn.axis);
        }
    }
    let Some(axis) = to_add else { return };
    let name = world.resource::<NativeInputUi>().new_name.trim().to_string();
    if name.is_empty() {
        return;
    }
    if let Some(mut m) = world.get_resource_mut::<InputMap>() {
        let action = if axis {
            InputAction::axis_2d(name, vec![], 0.15)
        } else {
            InputAction::button(name, vec![])
        };
        m.add(action);
    }
    world.resource_mut::<NativeInputUi>().new_name.clear();
    save_input(world);
    mark_dirty(world);
}

fn delete_action_click(world: &mut World) {
    let mut idx = None;
    let mut q = world.query_filtered::<(&Interaction, &DeleteActionBtn), Changed<Interaction>>();
    for (interaction, btn) in q.iter(world) {
        if *interaction == Interaction::Pressed {
            idx = Some(btn.0);
        }
    }
    let Some(i) = idx else { return };
    let name = world
        .get_resource::<InputMap>()
        .and_then(|m| m.actions.get(i).map(|a| a.name.clone()));
    if let (Some(name), Some(mut m)) = (name, world.get_resource_mut::<InputMap>()) {
        m.remove(&name);
    }
    {
        let mut ui = world.resource_mut::<NativeInputUi>();
        if ui.selected == Some(i) {
            ui.selected = None;
        }
    }
    save_input(world);
    mark_dirty(world);
}

fn expand_action_click(
    btns: Query<(&Interaction, &ExpandActionBtn), Changed<Interaction>>,
    mut ui: ResMut<NativeInputUi>,
    mut state: ResMut<NativeSettingsState>,
) {
    for (interaction, btn) in &btns {
        if *interaction == Interaction::Pressed {
            ui.selected = if ui.selected == Some(btn.0) {
                None
            } else {
                Some(btn.0)
            };
            ui.listening = false;
            state.dirty = true;
        }
    }
}

fn add_binding_click(
    btns: Query<(&Interaction, &AddBindingBtn), Changed<Interaction>>,
    mut ui: ResMut<NativeInputUi>,
    mut state: ResMut<NativeSettingsState>,
) {
    for (interaction, btn) in &btns {
        if *interaction == Interaction::Pressed {
            ui.selected = Some(btn.0);
            ui.listening = true;
            state.dirty = true;
        }
    }
}

fn cancel_listen_click(
    btns: Query<&Interaction, (Changed<Interaction>, With<CancelListenBtn>)>,
    mut ui: ResMut<NativeInputUi>,
    mut state: ResMut<NativeSettingsState>,
) {
    for interaction in &btns {
        if *interaction == Interaction::Pressed {
            ui.listening = false;
            state.dirty = true;
        }
    }
}

fn remove_binding_click(world: &mut World) {
    let mut target = None;
    let mut q = world.query_filtered::<(&Interaction, &RemoveBindingBtn), Changed<Interaction>>();
    for (interaction, btn) in q.iter(world) {
        if *interaction == Interaction::Pressed {
            target = Some((btn.action, btn.binding));
        }
    }
    let Some((a, b)) = target else { return };
    if let Some(mut m) = world.get_resource_mut::<InputMap>() {
        if let Some(action) = m.actions.get_mut(a) {
            if b < action.bindings.len() {
                action.bindings.remove(b);
            }
        }
    }
    save_input(world);
    mark_dirty(world);
}

fn composite_click(world: &mut World) {
    let mut target = None;
    let mut q = world.query_filtered::<(&Interaction, &CompositeBtn), Changed<Interaction>>();
    for (interaction, btn) in q.iter(world) {
        if *interaction == Interaction::Pressed {
            target = Some((btn.action, btn.arrows));
        }
    }
    let Some((a, arrows)) = target else { return };
    let binding = if arrows {
        InputBinding::composite_2d(
            KeyCode::ArrowUp,
            KeyCode::ArrowDown,
            KeyCode::ArrowLeft,
            KeyCode::ArrowRight,
        )
    } else {
        InputBinding::composite_2d(KeyCode::KeyW, KeyCode::KeyS, KeyCode::KeyA, KeyCode::KeyD)
    };
    if let Some(mut m) = world.get_resource_mut::<InputMap>() {
        if let Some(action) = m.actions.get_mut(a) {
            action.bindings.push(binding);
        }
    }
    save_input(world);
    mark_dirty(world);
}

/// While the Input tab is in listen mode, capture the next key or mouse button
/// and append it to the selected action's bindings.
fn input_listen_capture(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut ui: ResMut<NativeInputUi>,
    mut map: ResMut<InputMap>,
    mut state: ResMut<NativeSettingsState>,
    project: Option<Res<CurrentProject>>,
) {
    if !ui.listening {
        return;
    }
    let Some(sel) = ui.selected else { return };
    if keys.just_pressed(KeyCode::Escape) {
        ui.listening = false;
        state.dirty = true;
        return;
    }
    let binding = if let Some(k) = keys.get_just_pressed().copied().find(|k| !is_modifier_key(*k)) {
        Some(InputBinding::key(k))
    } else if mouse.just_pressed(MouseButton::Left) {
        Some(InputBinding::mouse(MouseButton::Left))
    } else if mouse.just_pressed(MouseButton::Right) {
        Some(InputBinding::mouse(MouseButton::Right))
    } else if mouse.just_pressed(MouseButton::Middle) {
        Some(InputBinding::mouse(MouseButton::Middle))
    } else {
        None
    };
    let Some(binding) = binding else { return };
    if let Some(action) = map.actions.get_mut(sel) {
        action.bindings.push(binding);
    }
    if let Some(project) = project {
        let _ = renzora_input::save_input_map(&map, &project);
    }
    ui.listening = false;
    state.dirty = true;
}

// ── Shortcuts ────────────────────────────────────────────────────────────────

const A_YELLOW: (u8, u8, u8) = (225, 200, 70);

fn tab_shortcuts(commands: &mut Commands, fonts: &EmberFonts, col: Entity) {
    // Group built-in actions by category, preserving first-seen order.
    let mut groups: Vec<(&'static str, Vec<EditorAction>)> = Vec::new();
    for a in EditorAction::all() {
        let cat = a.category();
        if let Some(g) = groups.iter_mut().find(|(c, _)| *c == cat) {
            g.1.push(a);
        } else {
            groups.push((cat, vec![a]));
        }
    }

    for (cat, actions) in groups {
        let (sec, body) = section(commands, fonts, "keyboard", cat, A_YELLOW);
        commands.entity(col).add_child(sec);
        for (i, action) in actions.into_iter().enumerate() {
            let btn = rebind_button(commands, fonts, action);
            settings_row(commands, fonts, body, i, action.display_name(), btn);
        }
    }

    // Reset-all row.
    let reset_lbl = commands
        .spawn((
            Text::new("Reset All to Defaults"),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    let reset = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(12.0), Val::Px(5.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                margin: UiRect::top(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(rgb((60, 40, 40))),
            Interaction::default(),
            ResetBindingsBtn,
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("reset-bindings"),
        ))
        .id();
    commands.entity(reset).add_child(reset_lbl);
    commands.entity(col).add_child(reset);
}

/// A rebind button whose label/colour live-track the binding + rebinding state
/// (so it shows "Press key..." while listening, without rebuilding the overlay).
fn rebind_button(commands: &mut Commands, fonts: &EmberFonts, action: EditorAction) -> Entity {
    let lbl = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_text(commands, lbl, move |w| {
        let kb = w.resource::<KeyBindings>();
        if kb.rebinding == Some(action) {
            "Press key...".to_string()
        } else {
            kb.get(action)
                .map(|b| b.display())
                .unwrap_or_else(|| "Unbound".to_string())
        }
    });
    bind_text_color(commands, lbl, move |w| {
        let kb = w.resource::<KeyBindings>();
        if kb.rebinding == Some(action) {
            rgb(renzora_ember::theme::warn_amber())
        } else if kb.get(action).is_some() {
            rgb(accent())
        } else {
            rgb(text_muted())
        }
    });
    let btn = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(section_bg())),
            Interaction::default(),
            RebindBtn(action),
            HoverCursor(SystemCursorIcon::Pointer),
            Name::new("rebind-btn"),
        ))
        .id();
    commands.entity(btn).add_child(lbl);
    btn
}

fn rebind_btn_click(
    btns: Query<(&Interaction, &RebindBtn), Changed<Interaction>>,
    mut kb: ResMut<KeyBindings>,
) {
    for (interaction, btn) in &btns {
        if *interaction == Interaction::Pressed {
            kb.rebinding = Some(btn.0);
            kb.plugin_rebinding = None;
        }
    }
}

fn reset_bindings_click(
    btns: Query<&Interaction, (Changed<Interaction>, With<ResetBindingsBtn>)>,
    mut kb: ResMut<KeyBindings>,
) {
    for interaction in &btns {
        if *interaction == Interaction::Pressed {
            *kb = KeyBindings::default();
        }
    }
}

fn is_modifier_key(k: KeyCode) -> bool {
    matches!(
        k,
        KeyCode::ControlLeft
            | KeyCode::ControlRight
            | KeyCode::ShiftLeft
            | KeyCode::ShiftRight
            | KeyCode::AltLeft
            | KeyCode::AltRight
            | KeyCode::SuperLeft
            | KeyCode::SuperRight
    )
}

/// While a (plugin) rebind is pending, capture the next non-modifier key + its
/// held modifiers and commit it. Escape cancels.
fn rebind_capture(keys: Res<ButtonInput<KeyCode>>, mut kb: ResMut<KeyBindings>) {
    let action = kb.rebinding;
    let plugin = kb.plugin_rebinding;
    if action.is_none() && plugin.is_none() {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        kb.rebinding = None;
        kb.plugin_rebinding = None;
        return;
    }
    let key = keys
        .get_just_pressed()
        .copied()
        .find(|k| !is_modifier_key(*k));
    let Some(key) = key else {
        return;
    };
    let ctrl = keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight);
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let alt = keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight);
    let mut b = KeyBinding::new(key);
    if ctrl {
        b = b.ctrl();
    }
    if shift {
        b = b.shift();
    }
    if alt {
        b = b.alt();
    }
    if let Some(a) = action {
        kb.set(a, b);
        kb.rebinding = None;
    } else if let Some(id) = plugin {
        kb.set_plugin(id, b);
        kb.plugin_rebinding = None;
    }
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

