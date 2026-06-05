//! `renzora_shell` — the bevy_ui-native editor shell.
//!
//! The editor's chrome (menu bar, ribbon, document tabs, status bar) plus the
//! wiring that drives the reusable [`renzora_ember`] dock. The dock itself —
//! splits, tabs, drag-docking — lives in `renzora_ember::dock`; the shell just
//! supplies the layout, the dock area, and editor-specific behavior.
//!
//! ## Coexistence during the migration
//! [`renzora::EditorUiBackend`] selects which editor renders — the legacy egui
//! editor (`Egui`, default) or this shell (`BevyUi`) — mutually exclusive (the
//! egui `editor_ui_system` is gated off under `BevyUi`). **F10** toggles.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora::{EditorUiBackend, NativePanelIds};
use renzora_ember::dock::{tab_pane, Dock, DockArea, DockDirty, DockLeaf, DockTab, TabPane};
use renzora_ember::font::{glyph, icon_item, icon_text, ui_font, EmberFonts};
use renzora_ember::widgets::{menu_item, menu_sep, scroll_area, screen_menu, text_input, EmberTextInput, Popup};
use renzora_ember::theme::{
    accent, divider, header_bg, placeholder, play_green, rgb, tab_active, text_muted, text_primary,
    window_bg,
};
use renzora_ember::EmberPlugin;

pub mod dock;

use dock::DockTree;

#[derive(Default)]
pub struct ShellPlugin;

impl Plugin for ShellPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ShellPlugin (bevy_ui editor shell)");
        app.add_plugins(EmberPlugin);
        app.init_resource::<EditorUiBackend>();
        let layouts = dock::workspace_layouts();
        // The dock starts on the first workspace (overrides DockPlugin's empty).
        app.insert_resource(Dock {
            tree: layouts[0].1.clone(),
        });
        app.insert_resource(ShellLayouts { layouts, active: 0 });
        app.init_resource::<renzora::ShellPanelRegistry>();
        app.init_resource::<renzora::ShellStatusRegistry>();
        app.init_resource::<RibbonDrag>();
        app.init_resource::<RibbonRename>();
        app.add_systems(
            Update,
            (
                toggle_backend,
                manage_shell_root,
                apply_panel_meta,
                ribbon_interact,
                ribbon_context_menu,
                ribbon_focus_rename,
                ribbon_rename_commit,
                content_dispatch,
                top_menu_open,
                settings_btn_click,
                palette_btn_click,
                sign_in_click,
                theme_bridge,
                apply_chrome_style,
                doc_add_click,
                doc_tab_click,
                doc_tab_close,
                sync_workspace_to_active_doc,
                workspace_add_click,
            ),
        );
    }
}

/// Map the active `ThemeManager` theme into ember's runtime palette, and rebuild
/// the chrome when the active theme *changes* (a switch) so widgets re-spawn with
/// the new colors. Individual color edits update the palette but don't rebuild
/// (that would close the Theme tab's color picker every frame).
fn theme_bridge(
    tm: Option<Res<renzora_theme::ThemeManager>>,
    backend: Res<EditorUiBackend>,
    project: Option<Res<renzora::CurrentProject>>,
    mut last_name: Local<Option<String>>,
    mut last_pal: Local<Option<renzora_ember::theme::Palette>>,
    roots: Query<Entity, With<ShellRoot>>,
    mut dirty: ResMut<DockDirty>,
    mut commands: Commands,
) {
    let Some(tm) = tm else { return };
    let pal = palette_from_theme(&tm.active_theme);
    if last_pal.as_ref() != Some(&pal) {
        renzora_ember::theme::set_palette(pal);
        *last_pal = Some(pal);
    }

    let first = last_name.is_none();
    let switched = last_name.as_deref().is_some_and(|n| n != tm.active_theme_name);
    if first || switched {
        *last_name = Some(tm.active_theme_name.clone());
        // Palette is current → build the ember Theme: palette-derived defaults
        // cascaded with the active theme file's per-widget style sections.
        let theme = build_ember_theme(project.as_deref(), &tm.active_theme_name);
        commands.insert_resource(theme);
        if switched && backend.is_bevy_ui() {
            for e in &roots {
                commands.entity(e).despawn();
            }
            dirty.0 = true;
        }
    }
}

/// Build the ember [`Theme`] from the active theme's `themes/<name>.toml` (its
/// per-widget style sections cascade over the palette-derived defaults). Built-in
/// themes with no file fall back to the defaults.
fn build_ember_theme(
    project: Option<&renzora::CurrentProject>,
    name: &str,
) -> renzora_ember::style::Theme {
    if let Some(p) = project {
        let path = p.path.join("themes").join(format!("{name}.toml"));
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(theme) = renzora_ember::style::Theme::from_toml(&content) {
                return theme;
            }
        }
    }
    renzora_ember::style::Theme::default()
}

fn palette_from_theme(t: &renzora_theme::Theme) -> renzora_ember::theme::Palette {
    fn tc(c: &renzora_theme::ThemeColor) -> (u8, u8, u8) {
        let [r, g, b, _] = c.0.to_array();
        (r, g, b)
    }
    renzora_ember::theme::Palette {
        window_bg: tc(&t.surfaces.window),
        panel_bg: tc(&t.surfaces.panel),
        header_bg: tc(&t.surfaces.extreme),
        tab_active: tc(&t.panels.tab_active),
        tab_hover: tc(&t.panels.tab_hover),
        close_red: tc(&t.semantic.error),
        divider: tc(&t.widgets.border),
        text_primary: tc(&t.text.primary),
        text_muted: tc(&t.text.muted),
        placeholder: tc(&t.text.disabled),
        play_green: tc(&t.semantic.success),
        warn_amber: tc(&t.semantic.warning),
        accent: tc(&t.semantic.accent),
        on_accent: tc(&t.widgets.active_fg),
        border: tc(&t.widgets.border_light),
        popup_bg: tc(&t.surfaces.popup),
        row_even: tc(&t.panels.inspector_row_even),
        row_odd: tc(&t.panels.inspector_row_odd),
        value_text: tc(&t.text.secondary),
        selection: tc(&t.semantic.selection),
        section_bg: tc(&t.panels.category_frame_bg),
        hover_bg: tc(&t.panels.item_hover),
        card_bg: tc(&t.panels.item_bg),
        tree_line: tc(&t.panels.tree_line),
    }
}

/// The top-bar gear button — toggles the Settings overlay.
#[derive(Component)]
struct TopBarSettingsBtn;

fn settings_btn_click(
    btns: Query<&Interaction, (Changed<Interaction>, With<TopBarSettingsBtn>)>,
    settings: Option<ResMut<renzora_editor::EditorSettings>>,
) {
    let Some(mut settings) = settings else { return };
    for interaction in &btns {
        if *interaction == Interaction::Pressed {
            settings.show_settings = !settings.show_settings;
        }
    }
}

renzora::add!(ShellPlugin, Editor);

/// The ribbon's workspace layouts and which one is active. Switching saves the
/// current dock tree back into the active slot (so per-layout edits persist)
/// and loads the chosen one into the ember [`Dock`].
#[derive(Resource)]
struct ShellLayouts {
    layouts: Vec<(String, DockTree)>,
    active: usize,
}

/// A ribbon workspace button (Scene, Blueprints, …). Carries its layout index;
/// the active highlight comes from the reactive rebuild (see `ribbon_snapshot`).
#[derive(Component)]
struct RibbonItem {
    index: usize,
}

/// The ribbon's "+" — adds a new empty workspace.
#[derive(Component)]
struct WorkspaceAddBtn;

/// The top-bar magnifier — toggles the command palette.
#[derive(Component)]
struct CommandPaletteBtn;

/// The top-bar account chip — opens the sign-in overlay.
#[derive(Component)]
struct SignInBtn;

/// In-progress ribbon drag (press-latch → reorder on release). `active` flips
/// once the cursor moves past a small threshold so a plain click still switches.
#[derive(Resource, Default)]
struct RibbonDrag(Option<RibbonDragState>);

struct RibbonDragState {
    from: usize,
    start_cursor: Vec2,
    active: bool,
}

/// The workspace currently being inline-renamed (`None` = none). Read by
/// `ribbon_snapshot` so that tab renders an edit field in place of its label.
#[derive(Resource, Default)]
struct RibbonRename(Option<usize>);

/// Marks the inline rename text field, carrying the workspace index it renames.
#[derive(Component)]
struct RibbonRenameInput(usize);

/// Marks the shell's root UI entity so it can be despawned when the backend
/// switches back to egui.
#[derive(Component)]
struct ShellRoot;

// ── Systems ─────────────────────────────────────────────────────────────────

/// F10 flips the active editor UI backend between the legacy egui editor and
/// the bevy_ui shell.
fn toggle_backend(keys: Res<ButtonInput<KeyCode>>, mut backend: ResMut<EditorUiBackend>) {
    if keys.just_pressed(KeyCode::F10) {
        *backend = match *backend {
            EditorUiBackend::Egui => EditorUiBackend::BevyUi,
            EditorUiBackend::BevyUi => EditorUiBackend::Egui,
        };
        info!("[shell] editor UI backend -> {:?}", *backend);
    }
}

/// Spawn the chrome + dock area when the backend is `BevyUi` (and trigger the
/// ember dock to build into it); tear it down when it isn't.
fn manage_shell_root(
    mut commands: Commands,
    backend: Res<EditorUiBackend>,
    fonts: Option<Res<EmberFonts>>,
    tm: Option<Res<renzora_theme::ThemeManager>>,
    mut dirty: ResMut<DockDirty>,
    roots: Query<Entity, With<ShellRoot>>,
) {
    let want = backend.is_bevy_ui();
    let have = !roots.is_empty();
    if want && !have {
        // Wait for fonts so text/icons render from the first frame.
        let Some(fonts) = fonts else {
            return;
        };
        // Set the palette from the active theme *before* spawning so the chrome
        // never builds with a stale palette (the theme_bridge's per-frame
        // set_palette can otherwise race the rebuild's spawn across a frame).
        let (themes, active) = if let Some(tm) = tm.as_ref() {
            renzora_ember::theme::set_palette(palette_from_theme(&tm.active_theme));
            (tm.available_themes.clone(), tm.active_theme_name.clone())
        } else {
            (Vec::new(), String::new())
        };
        spawn_shell(&mut commands, &fonts, &themes, &active);
        // Build the dock into the freshly-spawned `DockArea` (ember rebuilds it
        // from the persisted `Dock.tree`).
        dirty.0 = true;
    } else if !want && have {
        for e in &roots {
            commands.entity(e).despawn();
        }
    }
}

/// Apply real panel titles/icons from [`renzora::ShellPanelRegistry`] onto the
/// dock tabs (overriding ember's humanized defaults). Cheap; only writes on a
/// real change.
fn apply_panel_meta(
    registry: Res<renzora::ShellPanelRegistry>,
    tabs: Query<&DockTab>,
    mut texts: Query<&mut Text>,
) {
    if registry.panels.is_empty() {
        return;
    }
    for tab in &tabs {
        let Some(info) = registry.panels.get(&tab.id) else {
            continue;
        };
        if !info.title.is_empty() {
            if let Ok(mut t) = texts.get_mut(tab.label) {
                if t.0 != info.title {
                    t.0 = info.title.clone();
                }
            }
        }
        if !info.icon.is_empty() {
            if let Ok(mut t) = texts.get_mut(tab.icon) {
                if t.0 != info.icon {
                    t.0 = info.icon.clone();
                }
            }
        }
    }
}

/// Fill each leaf's content with the active panel's UI. Panels that registered a
/// **bevy-native** renderer (`NativePanelIds`) own their own `content` entity and
/// are skipped here. For the rest: the `gallery_*` ember showcases, and a
/// centered title placeholder for everything else. Shares the `PanelContent`
/// marker with native panels so the two never desync over one content entity.
fn content_dispatch(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    native: Option<Res<NativePanelIds>>,
    leaves: Query<&DockLeaf>,
    panes: Query<&TabPane>,
    children_q: Query<&Children>,
) {
    let Some(fonts) = fonts else {
        return;
    };
    for leaf in &leaves {
        if leaf.active.is_empty() {
            continue;
        }
        // A panel crate renders this id itself — leave its content alone.
        if native
            .as_ref()
            .is_some_and(|n| n.0.contains(&leaf.active))
        {
            continue;
        }
        // Build the active tab's pane once (lazily). If it already exists, do
        // nothing — `sync_panes` toggles its visibility on tab switch.
        let exists = children_q.get(leaf.content).is_ok_and(|kids| {
            kids.iter()
                .any(|c| panes.get(c).is_ok_and(|p| p.id == leaf.active))
        });
        if exists {
            continue;
        }
        let built = build_panel_content(&mut commands, &fonts, &leaf.active);
        let pane = tab_pane(&mut commands, &leaf.active, built, true);
        commands.entity(leaf.content).add_child(pane);
    }
}

/// Build the bevy_ui content for a panel id.
fn build_panel_content(commands: &mut Commands, fonts: &EmberFonts, id: &str) -> Entity {
    use renzora_ember::widgets;
    match id {
        "gallery_typography" => widgets::gallery_typography(commands, fonts),
        "gallery_buttons" => widgets::gallery_buttons(commands, fonts),
        "gallery_inputs" => widgets::gallery_inputs(commands, fonts),
        "gallery_selection" => widgets::gallery_selection(commands, fonts),
        "gallery_feedback" => widgets::gallery_feedback(commands, fonts),
        "gallery_inspector" => widgets::gallery_inspector(commands, fonts),
        "gallery_containers" => widgets::gallery_containers(commands, fonts),
        "gallery_nav" => widgets::gallery_nav(commands, fonts),
        "gallery_data" => widgets::gallery_data(commands, fonts),
        "gallery_forms" => widgets::gallery_forms(commands, fonts),
        "gallery_overlays" => widgets::gallery_overlays(commands, fonts),
        "gallery_menus" => widgets::gallery_menus(commands, fonts),
        "gallery_extras" => widgets::gallery_extras(commands, fonts),
        "gallery_node_graph" => widgets::gallery_node_graph(commands, fonts),
        "gallery_timeline" => widgets::gallery_timeline(commands, fonts),
        "gallery_code" => widgets::gallery_code(commands, fonts),
        "gallery_charts" => widgets::gallery_charts(commands, fonts),
        "gallery_pickers" => widgets::gallery_pickers(commands, fonts),
        "gallery_animation" => widgets::gallery_animation(commands, fonts),
        "gallery_audio" => widgets::gallery_audio(commands, fonts),
        "gallery_colors" => widgets::gallery_colors(commands, fonts),
        _ => {
            // Placeholder: the panel's name, centered.
            let container = commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    Name::new("placeholder"),
                ))
                .id();
            let text = commands
                .spawn((
                    Text::new(renzora_ember::dock::humanize(id)),
                    ui_font(&fonts.ui, 13.0),
                    TextColor(rgb(placeholder())),
                ))
                .id();
            commands.entity(container).add_child(text);
            container
        }
    }
}

/// Clicking a ribbon workspace button switches the dock layout: save the current
/// dock back into its slot, load the chosen layout into the ember [`Dock`],
/// flag a rebuild, and restyle the ribbon.
/// Press-latch ribbon interaction: a plain click switches workspace; a drag past
/// a small threshold reorders on release (mirrors the egui title-bar tabs).
#[allow(clippy::too_many_arguments)]
fn ribbon_interact(
    mut drag: ResMut<RibbonDrag>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    rename: Res<RibbonRename>,
    pressed: Query<(&RibbonItem, &Interaction)>,
    geom: Query<(&RibbonItem, &GlobalTransform)>,
    mut layouts: ResMut<ShellLayouts>,
    mut dock: ResMut<Dock>,
    mut dirty: ResMut<DockDirty>,
) {
    // Don't drag/switch while a tab is being renamed.
    if rename.0.is_some() {
        drag.0 = None;
        return;
    }
    let cursor = windows.iter().next().and_then(|w| w.cursor_position());

    if drag.0.is_none() && mouse.just_pressed(MouseButton::Left) {
        if let Some(cur) = cursor {
            for (item, interaction) in &pressed {
                if *interaction == Interaction::Pressed {
                    drag.0 = Some(RibbonDragState { from: item.index, start_cursor: cur, active: false });
                    break;
                }
            }
        }
    }

    if let (Some(state), Some(cur)) = (drag.0.as_mut(), cursor) {
        if (cur - state.start_cursor).length() > 5.0 {
            state.active = true;
        }
    }

    if mouse.just_released(MouseButton::Left) {
        if let Some(state) = drag.0.take() {
            if !state.active {
                apply_workspace(state.from, &mut layouts, &mut dock, &mut dirty);
            } else if let Some(cur) = cursor {
                // Insertion slot = first tab whose center is right of the cursor.
                let mut centers: Vec<(usize, f32)> = geom.iter().map(|(it, gt)| (it.index, gt.translation().x)).collect();
                centers.sort_by_key(|(i, _)| *i);
                let mut target = layouts.layouts.len();
                for (i, cx) in &centers {
                    if cur.x < *cx {
                        target = *i;
                        break;
                    }
                }
                let from = state.from;
                let post_to = if from < target { target.saturating_sub(1) } else { target };
                if post_to != from {
                    move_workspace(&mut layouts, &dock, from, post_to);
                }
            }
        }
    }
}

/// Move workspace `from` → `to` (remove-then-insert), saving the live dock tree
/// into the active slot first and remapping the active index to follow.
fn move_workspace(layouts: &mut ShellLayouts, dock: &Dock, from: usize, to: usize) {
    let len = layouts.layouts.len();
    if from >= len || to >= len || from == to {
        return;
    }
    let active = layouts.active;
    if let Some(slot) = layouts.layouts.get_mut(active) {
        slot.1 = dock.tree.clone();
    }
    let item = layouts.layouts.remove(from);
    layouts.layouts.insert(to, item);
    layouts.active = if active == from {
        to
    } else {
        let mut a = active;
        if from < a {
            a -= 1;
        }
        if to <= a {
            a += 1;
        }
        a
    };
}

/// Right-click a ribbon tab → context menu (Rename / Remove).
fn ribbon_context_menu(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    items: Query<(&RibbonItem, &RelativeCursorPosition)>,
    layouts: Res<ShellLayouts>,
    mut commands: Commands,
) {
    if !mouse.just_pressed(MouseButton::Right) {
        return;
    }
    let Some(fonts) = fonts else { return };
    let Some(cur) = windows.iter().next().and_then(|w| w.cursor_position()) else {
        return;
    };
    for (item, rcp) in &items {
        if !rcp.cursor_over {
            continue;
        }
        let index = item.index;
        let can_delete = layouts.layouts.len() > 1;
        let menu = screen_menu(&mut commands, cur.x, cur.y);
        let rename = menu_item(&mut commands, &fonts, "pencil-simple", "Rename", move |w| {
            if let Some(mut r) = w.get_resource_mut::<RibbonRename>() {
                r.0 = Some(index);
            }
        });
        let mut kids = vec![rename];
        if can_delete {
            let remove = menu_item(&mut commands, &fonts, "trash", "Remove", move |w| remove_workspace(w, index));
            kids.push(remove);
        }
        commands.entity(menu).add_children(&kids);
        break;
    }
}

/// Remove workspace `index`, remapping the active index (and switching the live
/// dock to the new active's tree when the active workspace itself is removed).
fn remove_workspace(world: &mut World, index: usize) {
    let (len, active) = {
        let Some(l) = world.get_resource::<ShellLayouts>() else { return };
        (l.layouts.len(), l.active)
    };
    if len <= 1 || index >= len {
        return;
    }
    let removing_active = index == active;
    {
        let mut l = world.resource_mut::<ShellLayouts>();
        l.layouts.remove(index);
        let new_len = l.layouts.len();
        l.active = if active == index {
            active.min(new_len - 1)
        } else if active > index {
            active - 1
        } else {
            active
        };
    }
    if removing_active {
        let new_tree = {
            let l = world.resource::<ShellLayouts>();
            l.layouts[l.active].1.clone()
        };
        world.resource_mut::<Dock>().tree = new_tree;
        world.resource_mut::<DockDirty>().0 = true;
    }
}

/// Auto-focus the rename field the frame it spawns.
fn ribbon_focus_rename(mut q: Query<&mut EmberTextInput, Added<RibbonRenameInput>>) {
    for mut inp in &mut q {
        inp.focused = true;
    }
}

/// Commit (Enter / blur) or cancel (Escape) the active ribbon rename.
fn ribbon_rename_commit(
    mut rename: ResMut<RibbonRename>,
    keys: Res<ButtonInput<KeyCode>>,
    inputs: Query<(&EmberTextInput, &RibbonRenameInput)>,
    mut layouts: ResMut<ShellLayouts>,
    mut had_focus: Local<bool>,
) {
    let Some(index) = rename.0 else {
        *had_focus = false;
        return;
    };
    if keys.just_pressed(KeyCode::Escape) {
        rename.0 = None;
        *had_focus = false;
        return;
    }
    let Some((inp, _)) = inputs.iter().find(|(_, r)| r.0 == index) else {
        return;
    };
    if inp.focused {
        *had_focus = true;
    }
    let enter = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter);
    let blurred = *had_focus && !inp.focused;
    if !enter && !blurred {
        return;
    }
    let new: String = inp.value.replace('\n', "").trim().to_string();
    rename.0 = None;
    *had_focus = false;
    if new.is_empty() {
        return;
    }
    if let Some(slot) = layouts.layouts.get_mut(index) {
        slot.0 = new;
    }
}

/// The top-bar magnifier → toggle the command palette (consumed by
/// `renzora_command_palette`).
fn palette_btn_click(
    q: Query<&Interaction, (With<CommandPaletteBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.insert_resource(renzora::core::ToggleCommandPaletteRequested);
    }
}

/// The account chip. Signed out → open the sign-in overlay. Signed in → a
/// dropdown (My Library / Settings / Sign Out), mirroring the egui title bar.
fn sign_in_click(
    q: Query<&Interaction, (With<SignInBtn>, Changed<Interaction>)>,
    bridge: Option<Res<renzora::core::AuthBridge>>,
    fonts: Option<Res<EmberFonts>>,
    windows: Query<&Window>,
    mut commands: Commands,
) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let signed_in = bridge.and_then(|b| b.signed_in_username.clone()).is_some();
    if !signed_in {
        commands.insert_resource(renzora::core::AuthToggleWindowRequest);
        return;
    }
    let Some(fonts) = fonts else { return };
    let Some(cursor) = windows.iter().next().and_then(|w| w.cursor_position()) else { return };

    let menu = screen_menu(&mut commands, cursor.x, cursor.y);
    let lib = menu_item(&mut commands, &fonts, "books", "My Library", |w| {
        if let Some(mut dock) = w.get_resource_mut::<Dock>() {
            dock.tree.focus_or_add_panel("hub_library");
        }
        if let Some(mut d) = w.get_resource_mut::<DockDirty>() {
            d.0 = true;
        }
    });
    let settings = menu_item(&mut commands, &fonts, "gear", "Settings", |w| {
        if let Some(mut s) = w.get_resource_mut::<renzora_editor::EditorSettings>() {
            s.show_settings = !s.show_settings;
        }
    });
    let sep = menu_sep(&mut commands);
    let out = menu_item(&mut commands, &fonts, "sign-out", "Sign Out", |w| {
        w.insert_resource(renzora::core::AuthSignOutRequest);
    });
    commands.entity(menu).add_children(&[lib, settings, sep, out]);
}

/// `+` → add a new empty workspace and switch to it.
fn workspace_add_click(
    q: Query<&Interaction, (With<WorkspaceAddBtn>, Changed<Interaction>)>,
    mut layouts: ResMut<ShellLayouts>,
    mut dock: ResMut<Dock>,
    mut dirty: ResMut<DockDirty>,
) {
    if !q.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    // Save the current layout, then append + focus a fresh empty workspace.
    let active = layouts.active;
    if let Some(slot) = layouts.layouts.get_mut(active) {
        slot.1 = dock.tree.clone();
    }
    let name = format!("Workspace {}", layouts.layouts.len() + 1);
    // A genuinely empty workspace (not a tab literally named "empty"), so the
    // dock shows its "Add Panel" button.
    layouts.layouts.push((name, DockTree::Empty));
    let idx = layouts.layouts.len() - 1;
    dock.tree = layouts.layouts[idx].1.clone();
    layouts.active = idx;
    dirty.0 = true;
}

// ── Chrome ──────────────────────────────────────────────────────────────────

fn spawn_shell(commands: &mut Commands, fonts: &EmberFonts, themes: &[String], active: &str) {
    let font = &fonts.ui;
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
            ShellRoot,
            renzora::HideInHierarchy,
            Name::new("Renzora Shell"),
        ))
        .id();

    let top_bar = build_top_bar(commands, font);
    let doctabs = build_doc_tabs(commands, font);

    // Dock area — ember reconciles the dock into this (tagged `DockArea`).
    let dock_area = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                // Zero minimum so a tall panel's content can't inflate the dock
                // area's min-content height and push it past the window (the
                // flexbox min-content trap — `overflow: clip` alone doesn't
                // override it). Without this, tall content blows up every leaf.
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                flex_basis: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..default()
            },
            DockArea,
            Name::new("dock-area"),
        ))
        .id();

    let statusbar = build_status_bar(commands, fonts, themes, active);

    commands
        .entity(root)
        .add_children(&[top_bar, doctabs, dock_area, statusbar]);
}

/// Which chrome bar an entity is, so [`apply_chrome_style`] can repaint each from
/// `Theme.chrome` (fill / height / separator edge / rounding / padding).
#[derive(Component, Clone, Copy)]
enum ChromeBar {
    Top,
    DocTabs,
    Status,
}

/// Repaint the three chrome bars (top bar, document tabs, status bar) from the
/// ember `Theme.chrome` whenever the theme changes — mirrors the dock's
/// `apply_dock_style` so the bars are theme-driven (and live-editable in the
/// Theme tab) rather than baking in palette colors. The status bar's separator
/// sits on its top edge; the top bars' on the bottom.
fn apply_chrome_style(
    theme: Res<renzora_ember::style::Theme>,
    mut q: Query<(Ref<ChromeBar>, &mut BackgroundColor, &mut BorderColor, &mut Node)>,
) {
    let repaint = theme.is_changed();
    for (kind, mut bg, mut bc, mut node) in &mut q {
        if !repaint && !kind.is_added() {
            continue;
        }
        let (s, edge_top) = match *kind {
            ChromeBar::Top => (&theme.top_bar, false),
            ChromeBar::DocTabs => (&theme.doc_tabs, false),
            ChromeBar::Status => (&theme.status_bar, true),
        };
        bg.0 = s.bg.color();
        node.height = Val::Px(s.height);
        node.border = if edge_top {
            UiRect::top(Val::Px(s.border_width))
        } else {
            UiRect::bottom(Val::Px(s.border_width))
        };
        node.border_radius = BorderRadius::all(Val::Px(s.radius));
        node.padding = UiRect::axes(Val::Px(s.pad_x), Val::Px(s.pad_y));
        *bc = BorderColor::all(s.border.color());
    }
}

/// The bottom status bar: a "Ready" label + plugin-contributed items from the
/// bevy-native `ShellStatusRegistry`, rendered via a reactive keyed list (so live
/// metrics update without rebuilding the bar).
fn build_status_bar(
    commands: &mut Commands,
    fonts: &EmberFonts,
    themes: &[String],
    active: &str,
) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(22.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                padding: UiRect::horizontal(Val::Px(10.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
            BorderColor::all(Color::NONE),
            ChromeBar::Status,
            Name::new("status-bar"),
        ))
        .id();

    // Left items (Ready + left-aligned status) fill the bar, pushing the theme
    // picker + right-aligned metrics to the right.
    let left_content = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(14.0),
                min_width: Val::Px(0.0),
                ..default()
            },
            Name::new("status-left"),
        ))
        .id();
    renzora_ember::reactive::keyed_list(commands, left_content, status_snapshot_left);

    // The theme dropup — a fixed element on the right, just before the metrics.
    let dropup = theme_dropup(commands, fonts, themes, active);

    let right_content = commands
        .spawn((
            Node {
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(14.0),
                ..default()
            },
            Name::new("status-right"),
        ))
        .id();
    renzora_ember::reactive::keyed_list(commands, right_content, status_snapshot_right);

    commands.entity(bar).add_children(&[left_content, dropup, right_content]);
    bar
}

/// The theme picker dropup in the status bar: shows the active theme and opens a
/// menu (flipped up — it's at the window bottom) of available themes; picking one
/// calls `ThemeManager::load_theme`, which the theme bridge applies + rebuilds.
fn theme_dropup(
    commands: &mut Commands,
    fonts: &EmberFonts,
    themes: &[String],
    active: &str,
) -> Entity {
    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                // Flip *up* (the bar is at the window bottom) and anchor to the
                // trigger's right edge so the menu opens up-and-left, on-screen.
                bottom: Val::Percent(100.0),
                right: Val::Px(0.0),
                margin: UiRect::bottom(Val::Px(4.0)),
                flex_direction: FlexDirection::Column,
                min_width: Val::Px(160.0),
                padding: UiRect::all(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::popup_bg())),
            BorderColor::all(rgb(divider())),
            GlobalZIndex(600),
            bevy::ui::RelativeCursorPosition::default(),
            Name::new("theme-menu"),
        ))
        .id();
    let mut rows = Vec::new();
    for name in themes {
        let n = name.clone();
        let icon = if name == active { "check" } else { "palette" };
        rows.push(menu_item(commands, fonts, icon, name, move |w| {
            if let Some(mut tm) = w.get_resource_mut::<renzora_theme::ThemeManager>() {
                tm.load_theme(&n);
            }
        }));
    }
    // Cap the height + scroll so the long theme list doesn't run off-screen.
    let content = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, ..default() })
        .id();
    commands.entity(content).add_children(&rows);
    let scroll = scroll_area(commands, content, 260.0);
    commands.entity(panel).add_child(scroll);

    let icon = icon_text(commands, &fonts.phosphor, "palette", text_muted(), 12.0);
    let label = commands
        .spawn((
            Text::new(if active.is_empty() { "Theme" } else { active }),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let caret = icon_text(commands, &fonts.phosphor, "caret-up", text_muted(), 9.0);
    let trigger = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                position_type: PositionType::Relative,
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            Popup::new(panel),
            renzora_hui::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("theme-dropup"),
        ))
        .id();
    commands.entity(trigger).add_children(&[icon, label, caret, panel]);
    trigger
}

enum StatusRow {
    Label(String, (u8, u8, u8)),
    Seg(renzora::ShellStatusSegment),
}

/// Status segments for one alignment, as keyed rows (each item's `render` is
/// recomputed every frame).
fn status_rows(world: &World, align: renzora::ShellStatusAlign) -> Vec<StatusRow> {
    let mut rows: Vec<StatusRow> = Vec::new();
    if let Some(reg) = world.get_resource::<renzora::ShellStatusRegistry>() {
        let mut items: Vec<&renzora::ShellStatusItem> =
            reg.items.iter().filter(|i| i.align == align).collect();
        items.sort_by_key(|i| i.order);
        for it in items {
            rows.extend((it.render)(world).into_iter().map(StatusRow::Seg));
        }
    }
    rows
}

/// Build a keyed snapshot from a row list.
fn rows_snapshot(rows: Vec<StatusRow>) -> renzora_ember::reactive::KeyedSnapshot {
    use std::hash::{Hash, Hasher};
    let items: Vec<(u64, u64)> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            i.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            match r {
                StatusRow::Label(t, c) => (0u8, t, c).hash(&mut h),
                StatusRow::Seg(s) => (1u8, &s.icon, &s.text, s.color).hash(&mut h),
            }
            (k.finish(), h.finish())
        })
        .collect();
    renzora_ember::reactive::KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| status_row(c, f, &rows[i])),
    }
}

/// Left side: a Ready label + left-aligned status items.
fn status_snapshot_left(world: &World) -> renzora_ember::reactive::KeyedSnapshot {
    let mut rows = vec![StatusRow::Label("Ready".to_string(), text_muted())];
    rows.extend(status_rows(world, renzora::ShellStatusAlign::Left));
    rows_snapshot(rows)
}

/// Right side: the right-aligned metrics.
fn status_snapshot_right(world: &World) -> renzora_ember::reactive::KeyedSnapshot {
    rows_snapshot(status_rows(world, renzora::ShellStatusAlign::Right))
}

fn status_row(commands: &mut Commands, fonts: &EmberFonts, row: &StatusRow) -> Entity {
    match row {
        StatusRow::Label(text, color) => commands
            .spawn((
                Text::new(text.clone()),
                ui_font(&fonts.ui, 11.0),
                TextColor(rgb(*color)),
            ))
            .id(),
        StatusRow::Seg(s) => {
            let r = commands
                .spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .id();
            let mut kids = Vec::new();
            let color = (s.color[0], s.color[1], s.color[2]);
            if !s.icon.is_empty() {
                let glyph = renzora_ember::font::icon_glyph(&s.icon)
                    .unwrap_or_else(|| s.icon.chars().next().unwrap_or(' '));
                kids.push(
                    commands
                        .spawn((
                            Text::new(glyph.to_string()),
                            TextFont {
                                font: fonts.phosphor.clone(),
                                font_size: 12.0,
                                ..default()
                            },
                            TextColor(rgb(color)),
                        ))
                        .id(),
                );
            }
            kids.push(
                commands
                    .spawn((
                        Text::new(s.text.clone()),
                        ui_font(&fonts.ui, 11.0),
                        TextColor(rgb(color)),
                    ))
                    .id(),
            );
            commands.entity(r).add_children(&kids);
            r
        }
    }
}

/// The top bar: File/Edit/View/Help on the left, the layout ribbon centered,
/// action buttons on the right.
fn build_top_bar(commands: &mut Commands, font: &Handle<Font>) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(34.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(8.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(window_bg())),
            BorderColor::all(Color::NONE),
            ChromeBar::Top,
            Name::new("top-bar"),
        ))
        .id();

    let left = zone(commands, "top-left", JustifyContent::FlexStart, 2.0, 1.0);
    let left_kids = vec![
        top_menu_item(commands, font, "File", TopMenuKind::File),
        top_menu_item(commands, font, "Edit", TopMenuKind::Edit),
        top_menu_item(commands, font, "View", TopMenuKind::View),
        top_menu_item(commands, font, "Help", TopMenuKind::Help),
    ];
    commands.entity(left).add_children(&left_kids);

    let center = zone(commands, "top-center", JustifyContent::Center, 2.0, 0.0);
    let magnifier = glyph(commands, "magnifying-glass", text_muted(), 14.0);
    // Search button — toggles the global command palette (Ctrl+P).
    commands.entity(magnifier).insert((
        Node {
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::axes(Val::Px(5.0), Val::Px(3.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        Interaction::default(),
        CommandPaletteBtn,
        renzora_hui::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
    ));
    // Reactive ribbon — one button per workspace in `ShellLayouts`.
    let ribbon = commands
        .spawn((
            Node {
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(2.0),
                ..default()
            },
            Name::new("ribbon"),
        ))
        .id();
    renzora_ember::reactive::keyed_list(commands, ribbon, ribbon_snapshot);
    let add = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                ..default()
            },
            Interaction::default(),
            WorkspaceAddBtn,
            renzora_hui::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("workspace-add"),
        ))
        .id();
    let add_label = commands
        .spawn((Text::new("+"), ui_font(font, 12.0), TextColor(rgb(text_muted()))))
        .id();
    commands.entity(add).add_child(add_label);
    commands.entity(center).add_children(&[magnifier, ribbon, add]);

    let right = zone(commands, "top-right", JustifyContent::FlexEnd, 8.0, 1.0);
    let play = icon_item(commands, "play", play_green(), 16.0);
    let code = icon_item(commands, "code", text_muted(), 16.0);
    let settings = icon_item(commands, "gear", text_muted(), 16.0);
    commands.entity(settings).insert((
        Interaction::default(),
        TopBarSettingsBtn,
        renzora_hui::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
    ));

    let account = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            Interaction::default(),
            SignInBtn,
            renzora_hui::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("account"),
        ))
        .id();
    let user = glyph(commands, "user", text_muted(), 14.0);
    let sign_in = commands
        .spawn((
            Text::new("Sign In"),
            ui_font(font, 12.0),
            TextColor(rgb(text_muted())),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    // Reflect the signed-in username (from the auth bridge) on the label.
    renzora_ember::reactive::bind_text(commands, sign_in, |w| {
        w.get_resource::<renzora::core::AuthBridge>()
            .and_then(|b| b.signed_in_username.clone())
            .unwrap_or_else(|| "Sign In".to_string())
    });
    commands.entity(account).add_children(&[user, sign_in]);

    let window = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                margin: UiRect::left(Val::Px(14.0)),
                ..default()
            },
            Name::new("window-buttons"),
        ))
        .id();
    let min = icon_item(commands, "minus", text_muted(), 14.0);
    let max = icon_item(commands, "square", text_muted(), 13.0);
    let close = icon_item(commands, "x", text_muted(), 14.0);
    commands.entity(window).add_children(&[min, max, close]);

    commands
        .entity(right)
        .add_children(&[play, code, settings, account, window]);

    commands.entity(bar).add_children(&[left, center, right]);
    bar
}

/// A top-bar ribbon entry (workspace switcher). Full height so the active
/// item's blue underline pins to the bottom edge. Clicking switches workspace
/// `index`; dragging reorders, right-click renames/removes (see [`ribbon_interact`]).
fn ribbon_item(
    commands: &mut Commands,
    font: &Handle<Font>,
    label: &str,
    index: usize,
    active: bool,
) -> Entity {
    let item = commands
        .spawn((
            Node {
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            Interaction::default(),
            RelativeCursorPosition::default(),
            renzora_hui::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new(format!("ribbon:{label}")),
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(label),
            ui_font(font, 12.0),
            TextColor(rgb(if active { text_primary() } else { text_muted() })),
        ))
        .id();
    let text_wrap = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::horizontal(Val::Px(7.0)),
                ..default()
            },
            Name::new("ribbon-label"),
        ))
        .id();
    commands.entity(text_wrap).add_child(text);
    let underline = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(2.0),
                ..default()
            },
            BackgroundColor(if active { rgb(accent()) } else { Color::NONE }),
            Name::new("ribbon-underline"),
        ))
        .id();
    commands.entity(item).insert(RibbonItem { index });
    commands.entity(item).add_children(&[text_wrap, underline]);
    item
}

/// Keyed snapshot of the workspace ribbon (one button per `ShellLayouts` entry;
/// the content hash carries the active flag so switching repaints just the two
/// affected buttons).
fn ribbon_snapshot(world: &World) -> renzora_ember::reactive::KeyedSnapshot {
    use std::hash::{Hash, Hasher};
    let empty = || renzora_ember::reactive::KeyedSnapshot {
        items: Vec::new(),
        build: Box::new(|c, _, _| c.spawn(Node::default()).id()),
    };
    let Some(layouts) = world.get_resource::<ShellLayouts>() else {
        return empty();
    };
    let active = layouts.active;
    let renaming = world.get_resource::<RibbonRename>().and_then(|r| r.0);
    let names: Vec<(usize, String)> = layouts
        .layouts
        .iter()
        .enumerate()
        .map(|(i, (n, _))| (i, n.clone()))
        .collect();
    let items: Vec<(u64, u64)> = names
        .iter()
        .map(|(i, name)| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            i.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (name, *i == active, renaming == Some(*i)).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    renzora_ember::reactive::KeyedSnapshot {
        items,
        build: Box::new(move |c, f, idx| {
            let (i, name) = &names[idx];
            if renaming == Some(*i) {
                build_ribbon_rename_field(c, &f.ui, *i, name)
            } else {
                ribbon_item(c, &f.ui, name, *i, *i == active)
            }
        }),
    }
}

/// Inline rename field for a ribbon tab (mirrors the native hierarchy's). Seeded
/// with the current name; committed by [`ribbon_rename_commit`].
fn build_ribbon_rename_field(commands: &mut Commands, font: &Handle<Font>, index: usize, name: &str) -> Entity {
    let input = text_input(commands, font, "Name", name);
    commands.entity(input).insert((
        RibbonRenameInput(index),
        Node {
            width: Val::Px(96.0),
            height: Val::Px(22.0),
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(4.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..default()
        },
    ));
    input
}

/// The document tab strip: the open documents (`DocumentTabState`, shared with
/// the egui editor) rendered reactively, plus an add-document button.
fn build_doc_tabs(commands: &mut Commands, _font: &Handle<Font>) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(30.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::horizontal(Val::Px(8.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(header_bg())),
            BorderColor::all(rgb(divider())),
            ChromeBar::DocTabs,
            Name::new("doc-tabs"),
        ))
        .id();

    // Reactive tab strip from the shared DocumentTabState.
    let tabs = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                height: Val::Percent(100.0),
                column_gap: Val::Px(2.0),
                min_width: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..default()
            },
            Name::new("doc-tab-list"),
        ))
        .id();
    renzora_ember::reactive::keyed_list(commands, tabs, doc_tab_snapshot);

    // "+" — add a new document (scene) tab.
    let plus = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(7.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            DocAddBtn,
            renzora_hui::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("doc-add"),
        ))
        .id();
    let plus_icon = glyph(commands, "plus", text_muted(), 13.0);
    commands.entity(plus).add_child(plus_icon);
    commands.entity(bar).add_children(&[tabs, plus]);
    bar
}

#[derive(Component)]
struct DocAddBtn;
#[derive(Component)]
struct DocTabClick(u64);
#[derive(Component)]
struct DocTabClose(u64);

/// Keyed snapshot of the open document tabs (id-keyed; the content hash carries
/// active/modified state so a tab repaints only when it actually changes).
fn doc_tab_snapshot(world: &World) -> renzora_ember::reactive::KeyedSnapshot {
    use std::hash::{Hash, Hasher};
    let empty = || renzora_ember::reactive::KeyedSnapshot {
        items: Vec::new(),
        build: Box::new(|c, _, _| c.spawn(Node::default()).id()),
    };
    let Some(state) = world.get_resource::<renzora_ui::DocumentTabState>() else {
        return empty();
    };
    let can_close = state.tabs.len() > 1;
    // (id, name, icon glyph, active, modified)
    let tabs: Vec<(u64, String, &'static str, bool, bool)> = state
        .tabs
        .iter()
        .enumerate()
        .map(|(i, t)| (t.id, t.name.clone(), t.kind.icon(), i == state.active_tab, t.is_modified))
        .collect();
    let items: Vec<(u64, u64)> = tabs
        .iter()
        .map(|(id, name, icon, active, modified)| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            id.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (name, icon, active, modified, can_close).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    renzora_ember::reactive::KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (id, name, icon, active, modified) = &tabs[i];
            doc_tab_row(c, f, *id, name, icon, *active, *modified, can_close)
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn doc_tab_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    id: u64,
    name: &str,
    icon: &str,
    active: bool,
    modified: bool,
    can_close: bool,
) -> Entity {
    let fg = if active { text_primary() } else { text_muted() };
    let tab = commands
        .spawn((
            Node {
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(9.0), Val::Px(4.0)),
                border: UiRect::top(Val::Px(2.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(if active { rgb(tab_active()) } else { Color::NONE }),
            BorderColor::all(if active { rgb(accent()) } else { Color::NONE }),
            Interaction::default(),
            DocTabClick(id),
            renzora_hui::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new(format!("doc:{name}")),
        ))
        .id();
    // Kind icon — the phosphor glyph is already resolved on the DocTabKind.
    let ic = commands
        .spawn((
            Text::new(icon.to_string()),
            TextFont { font: fonts.phosphor.clone(), font_size: 13.0, ..default() },
            TextColor(rgb(fg)),
        ))
        .id();
    let lbl = commands
        .spawn((
            Text::new(if modified { format!("{name}*") } else { name.to_string() }),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(fg)),
        ))
        .id();
    let mut kids = vec![ic, lbl];
    if can_close {
        let close = commands
            .spawn((
                Node {
                    align_items: AlignItems::Center,
                    padding: UiRect::left(Val::Px(2.0)),
                    ..default()
                },
                Interaction::default(),
                DocTabClose(id),
                renzora_hui::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            ))
            .id();
        let x = icon_text(commands, &fonts.phosphor, "x", text_muted(), 11.0);
        commands.entity(close).add_child(x);
        kids.push(close);
    }
    commands.entity(tab).add_children(&kids);
    tab
}

/// Swap the dock to workspace `index`, saving the current layout into the active
/// slot first. The ribbon highlight follows via the reactive rebuild (the
/// snapshot keys on `layouts.active`). Shared by the ribbon + doc-tab clicks.
fn apply_workspace(index: usize, layouts: &mut ShellLayouts, dock: &mut Dock, dirty: &mut DockDirty) {
    if index == layouts.active || index >= layouts.layouts.len() {
        return;
    }
    let active = layouts.active;
    if let Some(slot) = layouts.layouts.get_mut(active) {
        slot.1 = dock.tree.clone();
    }
    dock.tree = layouts.layouts[index].1.clone();
    layouts.active = index;
    dirty.0 = true;
}

/// `+` → add an "Untitled Scene" document and focus it.
fn doc_add_click(
    q: Query<&Interaction, (With<DocAddBtn>, Changed<Interaction>)>,
    state: Option<ResMut<renzora_ui::DocumentTabState>>,
) {
    let Some(mut state) = state else { return };
    if q.iter().any(|i| *i == Interaction::Pressed) {
        let idx = state.add_tab("Untitled Scene".into(), None);
        state.activate_tab(idx);
    }
}

/// Click a document tab → activate it + switch to the workspace its kind maps to.
/// Switch the bevy_ui workspace to match the active document tab whenever it
/// changes — including programmatic opens (double-clicking an asset, the
/// inspector's "edit" button), not just manual tab clicks. So opening a
/// `.material` / `.particle` / script switches to its editor workspace instead
/// of silently leaving the dock on the scene layout. The `Local` change-guard
/// means it only fires on a real active-tab change, so ribbon navigation while a
/// doc tab is open isn't fought (the scene entities are never touched — this is
/// purely a layout switch). Mirrors the egui editor's context-driven layout.
fn sync_workspace_to_active_doc(
    state: Option<Res<renzora_ui::DocumentTabState>>,
    mut layouts: ResMut<ShellLayouts>,
    mut dock: ResMut<Dock>,
    mut dirty: ResMut<DockDirty>,
    mut last: Local<Option<u64>>,
) {
    let Some(state) = state else { return };
    let active_id = state.active_tab_id();
    if *last == active_id {
        return;
    }
    *last = active_id;
    let Some(name) = state.active_tab().and_then(|t| t.kind.layout_name()) else {
        return;
    };
    if let Some(wi) = layouts.layouts.iter().position(|(n, _)| n == name) {
        apply_workspace(wi, &mut layouts, &mut dock, &mut dirty);
    }
}

fn doc_tab_click(
    q: Query<(&Interaction, &DocTabClick), Changed<Interaction>>,
    state: Option<ResMut<renzora_ui::DocumentTabState>>,
    mut layouts: ResMut<ShellLayouts>,
    mut dock: ResMut<Dock>,
    mut dirty: ResMut<DockDirty>,
) {
    let Some(mut state) = state else { return };
    for (interaction, click) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(idx) = state.tabs.iter().position(|t| t.id == click.0) else {
            continue;
        };
        state.activate_tab(idx);
        if let Some(name) = state.tabs[idx].kind.layout_name() {
            if let Some(wi) = layouts.layouts.iter().position(|(n, _)| n == name) {
                apply_workspace(wi, &mut layouts, &mut dock, &mut dirty);
            }
        }
    }
}

/// Click a document tab's × → close it (the model refuses to close the last
/// scene / last tab).
fn doc_tab_close(
    q: Query<(&Interaction, &DocTabClose), Changed<Interaction>>,
    state: Option<ResMut<renzora_ui::DocumentTabState>>,
) {
    let Some(mut state) = state else { return };
    for (interaction, close) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(idx) = state.tabs.iter().position(|t| t.id == close.0) {
            state.close_tab(idx);
        }
    }
}

/// A full-height flex row used as a top-bar zone (left / center / right).
fn zone(
    commands: &mut Commands,
    name: &str,
    justify: JustifyContent,
    gap: f32,
    grow: f32,
) -> Entity {
    commands
        .spawn((
            Node {
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: justify,
                column_gap: Val::Px(gap),
                flex_grow: grow,
                ..default()
            },
            Name::new(name.to_string()),
        ))
        .id()
}



// ── Top-bar menus (File / Edit / View / Help) ────────────────────────────────

#[derive(Clone, Copy)]
enum TopMenuKind {
    File,
    Edit,
    View,
    Help,
}

#[derive(Component)]
struct TopMenu(TopMenuKind);

/// An interactive top-bar menu title (File/Edit/View/Help).
fn top_menu_item(
    commands: &mut Commands,
    font: &Handle<Font>,
    label: &str,
    kind: TopMenuKind,
) -> Entity {
    let item = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            renzora_hui::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            TopMenu(kind),
            Name::new(format!("menu:{label}")),
        ))
        .id();
    renzora_ember::reactive::bind_bg(commands, item, move |w| match w.get::<Interaction>(item) {
        Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(renzora_ember::theme::hover_bg()),
        _ => Color::NONE,
    });
    commands.entity(item).with_children(|p| {
        p.spawn((
            Text::new(label),
            ui_font(font, 14.0),
            TextColor(rgb(text_muted())),
        ));
    });
    item
}

/// Click a top-bar title → open its menu via the shared ember `screen_menu`,
/// anchored to the button's bottom-left (stable, independent of cursor position).
fn top_menu_open(
    q: Query<
        (
            &Interaction,
            &TopMenu,
            &bevy::ui::RelativeCursorPosition,
            &bevy::ui::ComputedNode,
        ),
        Changed<Interaction>,
    >,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else {
        return;
    };
    for (interaction, menu, rcp, cn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(pos) = anchor_below(&windows, rcp, cn) else {
            continue;
        };
        let root = renzora_ember::widgets::screen_menu(&mut commands, pos.x, pos.y);
        let kids = build_menu_items(&mut commands, &fonts, menu.0);
        commands.entity(root).add_children(&kids);
    }
}

/// The bottom-left of a node in logical window px, derived from the cursor + the
/// node's normalized cursor position (scale-invariant; avoids UI `GlobalTransform`
/// coordinate ambiguity). Used to anchor button dropdowns just under the button.
fn anchor_below(
    windows: &Query<&Window>,
    rcp: &bevy::ui::RelativeCursorPosition,
    cn: &bevy::ui::ComputedNode,
) -> Option<Vec2> {
    let cursor = windows.iter().next().and_then(|w| w.cursor_position())?;
    let size = cn.size() * cn.inverse_scale_factor();
    let norm = rcp.normalized.unwrap_or(Vec2::ZERO);
    let top_left = cursor - (norm + Vec2::splat(0.5)) * size;
    Some(Vec2::new(top_left.x, top_left.y + size.y + 2.0))
}

fn build_menu_items(
    commands: &mut Commands,
    fonts: &EmberFonts,
    kind: TopMenuKind,
) -> Vec<Entity> {
    use renzora_ember::widgets::{menu_item, menu_sep};
    match kind {
        TopMenuKind::File => vec![
            menu_item(commands, fonts, "folder-plus", "New Project", |w| {
                renzora_editor::handle_new_project(w)
            }),
            menu_item(commands, fonts, "folder-open", "Open Project…", |w| {
                renzora_editor::handle_open_project(w)
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "file-plus", "New Scene", |w| {
                w.insert_resource(renzora::core::NewSceneRequested);
            }),
            menu_item(commands, fonts, "file", "Open Scene…", |w| {
                w.insert_resource(renzora::core::OpenSceneRequested);
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "floppy-disk", "Save", |w| {
                w.insert_resource(renzora::core::SaveSceneRequested);
            }),
            menu_item(commands, fonts, "floppy-disk-back", "Save As…", |w| {
                w.insert_resource(renzora::core::SaveAsSceneRequested);
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "package", "Export Project…", |w| {
                w.insert_resource(renzora::core::ExportRequested);
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "gear", "Settings", |w| {
                if let Some(mut s) = w.get_resource_mut::<renzora_editor::EditorSettings>() {
                    s.show_settings = !s.show_settings;
                }
            }),
        ],
        TopMenuKind::Edit => vec![
            menu_item(commands, fonts, "arrow-u-up-left", "Undo", |w| {
                let f = w.get_resource::<renzora_editor::EditorActionHooks>().and_then(|h| h.undo);
                if let Some(f) = f {
                    f(w);
                }
            }),
            menu_item(commands, fonts, "arrow-u-up-right", "Redo", |w| {
                let f = w.get_resource::<renzora_editor::EditorActionHooks>().and_then(|h| h.redo);
                if let Some(f) = f {
                    f(w);
                }
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "layout", "Reset Layout", reset_layout_action),
        ],
        TopMenuKind::View => vec![
            menu_item(commands, fonts, "magnifying-glass-plus", "Zoom In", |w| {
                w.insert_resource(renzora::core::CameraViewRequest::ZoomIn);
            }),
            menu_item(commands, fonts, "magnifying-glass-minus", "Zoom Out", |w| {
                w.insert_resource(renzora::core::CameraViewRequest::ZoomOut);
            }),
            menu_item(commands, fonts, "magnifying-glass", "Reset Zoom", |w| {
                w.insert_resource(renzora::core::CameraViewRequest::ResetZoom);
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "corners-out", "Fit All", |w| {
                w.insert_resource(renzora::core::CameraViewRequest::FrameAll);
            }),
            menu_item(commands, fonts, "eye", "Isolation Mode", |w| {
                let mut iso = w
                    .remove_resource::<renzora::core::IsolationMode>()
                    .unwrap_or_default();
                iso.active = !iso.active;
                w.insert_resource(iso);
            }),
        ],
        TopMenuKind::Help => vec![
            menu_item(commands, fonts, "graduation-cap", "Getting Started Tutorial", |w| {
                w.insert_resource(renzora::core::TutorialRequested);
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "book-open", "Documentation", |_| {
                open_url("https://renzora.com/docs")
            }),
            menu_item(commands, fonts, "youtube-logo", "YouTube", |_| {
                open_url("https://youtube.com/@renzoragame")
            }),
            menu_item(commands, fonts, "discord-logo", "Discord", |_| {
                open_url("https://discord.gg/9UHUGUyDJv")
            }),
            menu_item(commands, fonts, "github-logo", "GitHub", |_| {
                open_url("https://github.com/renzora/engine")
            }),
        ],
    }
}

/// Reset the active workspace's dock tree to its registered layout.
fn reset_layout_action(w: &mut World) {
    let tree = w
        .get_resource::<ShellLayouts>()
        .and_then(|l| l.layouts.get(l.active).map(|(_, t)| t.clone()));
    if let Some(tree) = tree {
        if let Some(mut dock) = w.get_resource_mut::<Dock>() {
            dock.tree = tree;
        }
    }
    if let Some(mut d) = w.get_resource_mut::<DockDirty>() {
        d.0 = true;
    }
}

/// Open `url` in the user's default browser (cross-platform).
fn open_url(url: &str) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}
