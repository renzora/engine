//! `renzora_shell` — the bevy_ui-native editor shell.
//!
//! The editor's chrome (menu bar, ribbon, document tabs, status bar) plus the
//! wiring that drives the reusable [`renzora_ember`] dock. The dock itself —
//! splits, tabs, drag-docking — lives in `renzora_ember::dock`; the shell just
//! supplies the layout, the dock area, and editor-specific behavior.
//!
//! What stays in this file is the parts that own the *whole* shell: the plugin
//! and its system wiring, the workspace layout list, the one function that
//! spawns the chrome tree, and the persistence that writes it back to disk.
//! Each region of the chrome lives in its own module:
//!
//! - [`top_bar`] / [`top_menu`] — the top bar and its hamburger menu
//! - [`ribbon`] — the workspace switcher in the middle of that bar
//! - [`doc_tabs`] — the document strip (and its compact dropdown form)
//! - [`bottom_dock`] / [`panel_sets`] — the global bottom panel
//! - [`status_bar`] — the bottom bar and its theme/language dropups
//! - [`window_chrome`] / [`save_prompts`] — borderless-window controls and the
//!   two "unsaved changes" confirmations
//! - [`theme_bridge`] — theme → ember palette / shaders / fonts
//! - [`panels`] — panel titles, icons and content dispatch

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora_ember::dock::{Dock, DockArea, DockDirty};
use renzora_ember::font::{icon_text, EmberFonts};
use renzora_ember::theme::{divider, header_bg, panel_bg, rgb, text_muted, window_bg};
use renzora_ember::EmberPlugin;

pub mod dock;

mod about;
mod bottom_dock;
mod doc_tabs;
mod panel_sets;
mod panels;
mod play_controls;
mod plugin_install;
mod ribbon;
mod save_prompts;
mod status_bar;
mod theme_bridge;
mod top_bar;
mod top_menu;
mod window_chrome;

use dock::DockTree;

use bottom_dock::{
    animate_bottom_dock, bottom_dock_close_click, bottom_dock_drag_reveal, bottom_dock_grip_press,
    bottom_dock_mode_click, bottom_dock_resize_drag, clamp_bottom_dock_on_load,
    clear_bottom_dock_hover_on_hide, collapsed_bottom_bar_drag, collapsed_bottom_open_click,
    collapsed_bottom_tab_click, collapsed_bottom_tab_hover, sync_bottom_dock_mode_btn,
    sync_bottom_dock_node, sync_collapsed_bottom_bar, toggle_bottom_panel, BottomDock,
    BottomDockBtn, BottomDockCloseBtn, BottomDockDragHide, BottomDockGrip, BottomDockModeBtn,
    BottomDockResize, CollapsedBottomBar, DockAreaWrap, BOTTOM_DOCK_GRIP_H, BOTTOM_DOCK_Z,
};
use doc_tabs::{
    build_doc_tabs, doc_add_click, doc_focus_rename, doc_rename_commit, doc_tab_click,
    doc_tab_close, doc_tab_drag, doc_tab_menu_row_click, doc_tabs_follow_asset_path,
    sync_active_doc_to_workspace, sync_workspace_to_active_doc, DocTabDrag, DocTabMru, DocTabRename,
};
use panel_sets::{
    bottom_set_drag, bottom_set_focus_rename, bottom_set_menu_click, bottom_set_rename_commit,
    build_bottom_set_menu, default_panel_set_name, sync_bottom_set_menu, BottomPanelSets,
    BottomSetDrag, BottomSetRename,
};
use panels::{apply_panel_meta, content_dispatch, seed_panel_meta};
use play_controls::{
    play_btn_click, play_target_option_click, track_global_scene_cameras, update_play_button,
    update_play_target_menu, vr_active_overlay, GlobalSceneHasCamera,
};
use ribbon::{
    ribbon_context_menu, ribbon_focus_rename, ribbon_interact, ribbon_rename_commit,
    workspace_add_click, workspace_drop_to_new, RibbonDrag, RibbonRename,
};
use save_prompts::{
    close_tab_prompt_buttons, exit_prompt_buttons, pending_close_after_save, pending_exit_after_save,
    process_exit_request, process_tab_close_request,
};
use status_bar::{apply_chrome_style, build_status_bar, ThemeMenuOpen};
use theme_bridge::{apply_theme_effects, palette_from_theme, sync_theme_menu_open, theme_bridge};
use top_bar::{build_top_bar, palette_btn_click, settings_btn_click, shell_action_press};
use window_chrome::{
    build_resize_zones, update_maximize_icon, window_btn_click, window_drag, window_resize_start,
};

#[cfg(target_arch = "wasm32")]
use window_chrome::{sync_web_fullscreen_icon, web_fullscreen_click};

#[derive(Default)]
pub struct ShellPlugin;

impl Plugin for ShellPlugin {
    fn build(&self, app: &mut App) {
        info!("[editor] ShellPlugin (bevy_ui editor shell)");
        app.add_plugins(EmberPlugin);
        // Restore the persisted per-workspace dock layout if one exists, else use
        // the built-in defaults. When restoring, append any *new* built-in
        // workspace whose name the saved file predates — so engine updates that
        // add a default workspace still surface it instead of being shadowed by an
        // older saved set. (A workspace the user deliberately removed reappearing
        // is the accepted trade-off; the alternative hides new defaults.)
        let defaults = dock::workspace_layouts();
        let restored = dock::load_dock_layouts();
        // Whether there was a layout file at all. It is the difference between
        // the two reasons the bottom panel can arrive empty below — nothing
        // saved yet, versus something saved by a build that kept the panel
        // inside the workspace trees.
        let fresh = restored.is_none();
        let (mut layouts, active, floating, closed_bottoms, saved_bottom) = match restored {
            Some((mut saved, active, floating, closed, bottom)) => {
                for (name, tree) in defaults {
                    if !saved.iter().any(|(n, _)| n == &name) {
                        saved.push((name, tree));
                    }
                }
                let active = active.min(saved.len().saturating_sub(1));
                (saved, active, floating, closed, bottom)
            }
            None => (defaults, 0, Vec::new(), Default::default(), None),
        };
        // The bottom panel is global: one tree, shared by every workspace,
        // living beside the workspace layouts rather than inside one of them.
        // No shipped default puts it in a workspace tree, so there are three
        // cases and they must not be confused for one another:
        //
        // - a layout file that already has one — use it;
        // - no layout file at all — the shipped default (`default_bottom_dock`);
        // - a layout file *without* one — written by a build that kept a bottom
        //   strip in each workspace tree (and possibly a closed stash per
        //   workspace), so fold them all together once. See `migrate_bottom_dock`
        //   for why that fold deduplicates and why skipping it loses panels.
        //   Substituting the default here instead would silently discard the
        //   panels that user had arranged.
        let bottom_dock = match saved_bottom {
            Some(b) => b,
            None if fresh => dock::default_bottom_dock(),
            None => dock::migrate_bottom_dock(&mut layouts, &closed_bottoms),
        };
        // The panel's named tab-sets. A layout file that predates them (or one
        // just synthesized by the migration above) has none, which means the
        // single set it does have is whatever `tree` holds.
        let mut sets: Vec<(String, DockTree)> = bottom_dock
            .sets
            .iter()
            .map(|s| (s.name.clone(), s.tree.clone()))
            .collect();
        if sets.is_empty() {
            sets.push((default_panel_set_name(), bottom_dock.tree.clone()));
        }
        let bottom_active = bottom_dock.active.min(sets.len() - 1);
        app.insert_resource(renzora_ember::dock::FixedDock {
            // The active set's tree, not `bottom_dock.tree` — they agree in
            // every file this build writes, but a file whose `tree` was left
            // behind by an older build must not win over the sets.
            tree: sets[bottom_active].1.clone(),
            area: None,
            // Nothing to build until the shell chrome spawns the area node;
            // `track_fixed_dock_area` arms this when it does.
            dirty: false,
        });
        app.insert_resource(BottomPanelSets {
            sets,
            active: bottom_active,
        });
        app.insert_resource(BottomDock {
            height: bottom_dock.height,
            open: bottom_dock.open,
            mode: bottom_dock.mode,
            // Start at whichever end of the travel the restored state names —
            // animating a panel into place on the first frame of a session
            // reads as the editor still loading, not as a transition.
            slide: if bottom_dock.open { 1.0 } else { 0.0 },
        });
        // Empty on purpose. These marked which panels identified a collapsible
        // bottom strip *inside* a workspace tree, so ember could give that leaf
        // a collapse chevron and a snap-closed divider gesture. The bottom panel
        // is its own dock area now, with its own collapse button and its own
        // resize handle owned by the shell — a marker here would put a second,
        // dead chevron on whichever leaf happened to tab `console`, wired to a
        // `BottomSnapRequest` that nothing consumes any more.
        app.insert_resource(renzora_ember::dock::BottomStripMarkers(Vec::new()));
        // The dock starts on the active workspace (overrides DockPlugin's empty).
        app.insert_resource(Dock {
            tree: layouts[active].1.clone(),
        });
        app.insert_resource(ShellLayouts { layouts, active });
        // Workspaces a plugin registered through `register_workspace`. Drained
        // here rather than in `renzora_editor_framework` because THIS is the
        // dock the editor draws: `renzora_ui::LayoutManager` is the older egui
        // model, and a workspace installed into that one never reaches the
        // ribbon. No tree conversion either, since this dock already speaks
        // ember's `DockTree` and that is the type a plugin registers with.
        app.init_resource::<renzora_ember::workspace::PendingWorkspaces>()
            .add_systems(Update, install_plugin_workspaces);
        // Reopen persisted floating dock windows. The spawn system queues the
        // requests until ember's fonts are ready, so pushing them this early is
        // safe. (Inserted after `EmberPlugin` above, so `DockPlugin`'s
        // `init_resource` won't overwrite it.)
        if !floating.is_empty() {
            app.insert_resource(renzora_ember::dock::DockWindowRequests(
                floating
                    .into_iter()
                    .filter(|f| !matches!(f.tree, DockTree::Empty))
                    .map(|f| renzora_ember::dock::DockWindowRequest {
                        tree: f.tree,
                        position: f.position.map(|(x, y)| IVec2::new(x, y)),
                        size: UVec2::new(f.size.0.max(160), f.size.1.max(120)),
                        grab: false,
                    })
                    .collect(),
            ));
        }
        app.init_resource::<renzora::ShellPanelRegistry>();
        app.init_resource::<renzora::ShellStatusRegistry>();
        seed_panel_meta(app);
        // Without this both bottom-panel grip systems fail parameter validation
        // every frame ("Resource does not exist") and silently never run, which
        // reads exactly like a resize handle that isn't being hit.
        app.init_resource::<BottomDockResize>();
        app.init_resource::<BottomDockDragHide>();
        app.init_resource::<RibbonDrag>();
        app.init_resource::<RibbonRename>();
        app.init_resource::<BottomSetRename>();
        app.init_resource::<BottomSetDrag>();
        app.init_resource::<DocTabDrag>();
        app.init_resource::<DocTabRename>();
        app.init_resource::<DocTabMru>();
        app.init_resource::<GlobalSceneHasCamera>();
        app.add_systems(Update, track_global_scene_cameras);
        app.add_observer(doc_tabs_follow_asset_path);
        // The hamburger owns its own resource + systems; none of the four needs
        // ordering against anything here.
        top_menu::register(app);
        app.init_resource::<ThemeMenuOpen>();
        app.add_systems(
            Update,
            (
                manage_shell_root,
                apply_panel_meta,
                ribbon_interact,
                ribbon_context_menu,
                ribbon_focus_rename,
                ribbon_rename_commit,
                content_dispatch,
                (play_btn_click, update_play_button, vr_active_overlay),
                plugin_install::install_buttons,
                palette_btn_click,
                (theme_bridge, sync_theme_menu_open),
                apply_chrome_style,
                doc_add_click,
                (doc_tab_click, doc_tab_menu_row_click),
                (
                    doc_tab_drag,
                    doc_focus_rename,
                    doc_rename_commit,
                    doc_tab_close,
                    process_tab_close_request,
                    close_tab_prompt_buttons,
                    pending_close_after_save,
                ),
                (
                    sync_workspace_to_active_doc,
                    sync_active_doc_to_workspace,
                    persist_dock_layout,
                ),
                (workspace_add_click, workspace_drop_to_new),
                (window_btn_click, window_drag, window_resize_start, update_maximize_icon),
                (process_exit_request, exit_prompt_buttons, pending_exit_after_save),
            ),
        );
        // Kept as its own `add_systems` call: the tuple above is already at the
        // 20-element limit for system tuples.
        app.add_systems(
            Update,
            (
                about::process_about_request,
                about::about_credit_click,
                about::about_credit_hover,
                // Web-only: the fullscreen toggle that stands in for the
                // window controls.
                #[cfg(target_arch = "wasm32")]
                (web_fullscreen_click, sync_web_fullscreen_icon),
                relocalize_on_language_change,
                (settings_btn_click, shell_action_press),
                sync_hierarchy_filter_to_workspace,
                (play_target_option_click, update_play_target_menu),
                toggle_bottom_panel,
                (
                    // First: the one-shot load cap settles the restored height
                    // before anything else this frame reads or writes it.
                    clamp_bottom_dock_on_load,
                    sync_collapsed_bottom_bar,
                    collapsed_bottom_tab_click,
                    collapsed_bottom_open_click,
                    collapsed_bottom_bar_drag,
                    collapsed_bottom_tab_hover,
                    bottom_dock_close_click,
                    bottom_dock_mode_click,
                    // Act, then rebuild the menu, so a switch or a new set is
                    // reflected in the same frame it was asked for. The rename
                    // commit runs first for the same reason: its result is one
                    // of the things the rebuild has to pick up.
                    bottom_set_rename_commit,
                    // Before the click system and the rebuild: this is what
                    // decides whether a release was a pick or a reorder, and
                    // both change the list the rebuild reads.
                    bottom_set_drag,
                    bottom_set_menu_click,
                    sync_bottom_set_menu,
                    bottom_set_focus_rename,
                    // Press before drag, so a grip press and the first motion
                    // of the same gesture can't be processed out of order.
                    bottom_dock_grip_press,
                    bottom_dock_resize_drag,
                    // After everything that can flip `open`, before the node
                    // sync that draws the result: the auto-hide decides what
                    // `open` should be for this frame, then the animation
                    // advances toward it.
                    bottom_dock_drag_reveal,
                    animate_bottom_dock,
                    // Last: apply whatever the above decided this frame, so the
                    // panel never renders a frame at a stale height or mode.
                    // Nested so the outer tuple stays inside the 20-system
                    // limit, and chained because both touch `Interaction`.
                    (sync_bottom_dock_node, clear_bottom_dock_hover_on_hide).chain(),
                    sync_bottom_dock_mode_btn,
                )
                    .chain(),
            ),
        );
    }
}

renzora::add!(ShellPlugin, Editor);

/// The ribbon's workspace layouts and which one is active. Switching saves the
/// current dock tree back into the active slot (so per-layout edits persist)
/// and loads the chosen one into the ember [`Dock`].
#[derive(Resource)]
pub(crate) struct ShellLayouts {
    pub(crate) layouts: Vec<(String, DockTree)>,
    pub(crate) active: usize,
}

/// Install workspaces registered through
/// [`renzora_ember::workspace::RegisterWorkspace`].
///
/// Drained every frame rather than once at startup, because a native plugin
/// loads (and reloads, whenever its source moves) long after this crate's
/// `build` has run. A workspace from a plugin that arrives mid-session shows up
/// in the ribbon on the next frame.
///
/// Replaces by name. A reloading plugin re-registers on every rebuild, so
/// appending would grow the ribbon one duplicate per rebuild. Replacing also
/// means a saved layout for that name is overwritten by the plugin's, which is
/// the correct way round: the plugin's tree is the definition, and the saved one
/// may arrange panels that no longer exist.
fn install_plugin_workspaces(
    mut pending: ResMut<renzora_ember::workspace::PendingWorkspaces>,
    mut layouts: ResMut<ShellLayouts>,
) {
    if pending.0.is_empty() {
        return;
    }
    for request in pending.drain() {
        match layouts.layouts.iter().position(|(n, _)| *n == request.name) {
            Some(i) => layouts.layouts[i].1 = request.tree,
            None => layouts.layouts.push((request.name.clone(), request.tree)),
        }
        info!("[shell] workspace `{}` registered", request.name);
    }
}

/// Marks the shell's root UI entity so it can be despawned when the backend
/// switches back to egui.
#[derive(Component)]
pub(crate) struct ShellRoot;

/// Live re-localization: when the active language changes, rebuild the chrome so
/// every widget re-reads `renzora::lang::t(...)` in the new language — the same
/// despawn-`ShellRoot` path a theme switch uses; `manage_shell_root` respawns the
/// chrome and re-arms `DockDirty`, so the dock rebuilds too (panel contents
/// re-localize, not just the bars).
///
/// Driven off the lock-free `revision()` counter rather than the `LanguageChanged`
/// message, so it needs no message registration and works regardless of plugin
/// load order. The counter is bumped by every pack registration, so it ticks
/// several times during startup; we skip the first observed value (and any tick
/// while the chrome isn't up yet) to avoid a spurious rebuild before the editor
/// has even spawned — only a genuine *change* after that triggers a rebuild.
fn relocalize_on_language_change(
    mut last_rev: Local<u64>,
    mut seen_once: Local<bool>,
    roots: Query<Entity, With<ShellRoot>>,
    mut dirty: ResMut<DockDirty>,
    mut commands: Commands,
) {
    let rev = renzora::lang::revision();
    if *last_rev == rev {
        return;
    }
    *last_rev = rev;
    // Swallow the first transition (startup registrations) and any change while
    // the chrome is absent — nothing to rebuild yet.
    if !*seen_once {
        *seen_once = true;
        return;
    }
    if roots.is_empty() {
        return;
    }
    for e in &roots {
        commands.entity(e).try_despawn();
    }
    // Cancel (don't request) any pending dock rebuild — the dock tree dies with
    // the chrome despawned above, and a same-frame `rebuild_dock` against it
    // panics on the dead entities (see `theme_bridge` for the full story).
    // `manage_shell_root` re-arms `DockDirty` when it respawns the chrome.
    dirty.0 = false;
}

// ── Systems ─────────────────────────────────────────────────────────────────

/// Spawn the chrome + dock area (and trigger the ember dock to build into it).
fn manage_shell_root(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    tm: Option<Res<renzora_theme::ThemeManager>>,
    theme_menu_open: Res<ThemeMenuOpen>,
    asset_server: Res<AssetServer>,
    mut dirty: ResMut<DockDirty>,
    roots: Query<Entity, With<ShellRoot>>,
) {
    let want = true;
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
            apply_theme_effects(
                &tm.active_theme,
                &tm.active_theme_name,
                tm.active_theme_dir(),
                &asset_server,
            );
            (tm.available_themes.clone(), tm.active_theme_name.clone())
        } else {
            (Vec::new(), String::new())
        };
        spawn_shell(&mut commands, &fonts, &themes, &active, theme_menu_open.0);
        // Build the dock into the freshly-spawned `DockArea` (ember rebuilds it
        // from the persisted `Dock.tree`).
        dirty.0 = true;
    } else if !want && have {
        for e in &roots {
            commands.entity(e).try_despawn();
        }
    }
}

// ── Chrome ──────────────────────────────────────────────────────────────────

fn spawn_shell(
    commands: &mut Commands,
    fonts: &EmberFonts,
    themes: &[String],
    active: &str,
    theme_menu_open: bool,
) {
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

    // Top bar, then the document tabs, then the dock. The tabs are back in this
    // column after a spell inside the viewport panel (see [`build_doc_tabs`]):
    // as shell chrome they are on screen in every workspace, including the
    // viewport-less asset layouts an open material routes the editor into.
    let top_bar = build_top_bar(commands, font, fonts);
    let doc_tabs = build_doc_tabs(commands);

    // Wrapper holding the workspace dock and the global bottom panel overlaid
    // on it. The bottom panel CANNOT be a child of the `DockArea`: ember's
    // `rebuild_area` despawns every child of the area it rebuilds, so the
    // overlay would be destroyed on the next tab drag. As a sibling inside a
    // relatively-positioned wrapper, `bottom: 0` anchors it to the bottom of
    // the dock region — above the collapsed strip and status bar, which are
    // rows below this wrapper in the shell column.
    let dock_wrap = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                flex_basis: Val::Px(0.0),
                position_type: PositionType::Relative,
                // A column so the bottom panel can join the flow in Layout
                // mode and take height off the dock area above it. In Overlay
                // mode the panel is absolute and the dock area is the only
                // in-flow child, so the direction makes no difference there.
                flex_direction: FlexDirection::Column,
                overflow: Overflow::clip(),
                ..default()
            },
            DockAreaWrap,
            Name::new("dock-wrap"),
        ))
        .id();

    // Dock area — ember reconciles the dock into this (tagged `DockArea`).
    let dock_area = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
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

    // The global bottom panel: one dock area shared by every workspace,
    // overlaid on the bottom of the dock region. `sync_bottom_dock_node` drives
    // its height and visibility from [`BottomDock`]; ember fills it from
    // [`renzora_ember::dock::FixedDock`].
    //
    // Absolute, so growing it covers the workspace panels instead of
    // compressing them — the workspace's own split ratios are never touched by
    // a bottom-panel resize, which is the behaviour the old in-tree strip could
    // not give (there, dragging its divider *was* an edit to the workspace
    // layout).
    let bottom_dock_area = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                bottom: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Px(dock::BOTTOM_DOCK_HEIGHT),
                min_width: Val::Px(0.0),
                min_height: Val::Px(0.0),
                // Only bites in Layout mode, where the panel is an in-flow row:
                // without it flexbox would shrink the panel instead of the dock
                // area above, and the height we just set would be a suggestion.
                flex_shrink: 0.0,
                border: UiRect::top(Val::Px(1.0)),
                overflow: Overflow::clip(),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(panel_bg())),
            BorderColor::all(rgb(divider())),
            GlobalZIndex(BOTTOM_DOCK_Z),
            DockArea,
            renzora_ember::dock::FixedDockArea,
            // It floats over the workspace panels, so it has to swallow pointer
            // events like any other overlay surface — without this a click or
            // scroll inside it bleeds through to the viewport behind.
            //
            // The `RelativeCursorPosition` is not optional decoration:
            // `update_pointer_over_overlay` queries `(&RelativeCursorPosition,
            // &Node)` filtered on `OverlaySurface`, so a surface without one
            // never matches the query and the marker silently does nothing.
            renzora_ember::widgets::OverlaySurface,
            RelativeCursorPosition::default(),
            Name::new("bottom-dock-area"),
        ))
        .id();

    // Top-edge resize grip for the bottom panel.
    //
    // A sibling of the panel, not a child of it, for the same reason the panel
    // is a sibling of the dock area: `rebuild_area` despawns every child of the
    // area it rebuilds, so a grip parented to the panel would vanish the first
    // time a tab moved inside it. `sync_bottom_dock_node` keeps its `bottom`
    // offset on the panel's top edge.
    let bottom_dock_grip = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                bottom: Val::Px(dock::BOTTOM_DOCK_HEIGHT - BOTTOM_DOCK_GRIP_H * 0.5),
                width: Val::Percent(100.0),
                height: Val::Px(BOTTOM_DOCK_GRIP_H),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::NONE),
            // One tier above the panel: sibling order is not enough once the
            // panel itself is on a `GlobalZIndex` tier, and the band overhangs
            // the dock area above, whose graph panels hoist themselves to the
            // root order too.
            GlobalZIndex(BOTTOM_DOCK_Z + 1),
            Interaction::default(),
            // Not decoration: the marker's insertion hook forces
            // `FocusPolicy::Block`. `Node`'s own required `FocusPolicy` defaults
            // to `Pass` in Bevy 0.19, so without this the press falls straight
            // through to the panel underneath and the grip does nothing —
            // which is exactly how the first cut of this failed.
            renzora_ember::resize::ResizeHandle,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::NsResize),
            BottomDockGrip,
            Name::new("bottom-dock-grip"),
        ))
        .id();

    // Close (collapse) button, top-right of the open panel. A sibling for the
    // same reason as the grip: `rebuild_area` despawns the panel's children.
    let bottom_dock_close = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(6.0),
                bottom: Val::Px(dock::BOTTOM_DOCK_HEIGHT - 26.0),
                width: Val::Px(22.0),
                height: Val::Px(22.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::NONE),
            // Above the panel's own dock content, which sits in the panel's
            // stacking context one tier below.
            GlobalZIndex(BOTTOM_DOCK_Z + 2),
            Interaction::default(),
            // Blocks for the same reason as the set dropdown beside it: the
            // header filler underneath is a resize surface, and hover leaking
            // to it puts an ns-resize cursor on this button.
            bevy::ui::FocusPolicy::Block,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            BottomDockCloseBtn,
            BottomDockBtn,
            Name::new("bottom-dock-close"),
        ))
        .id();
    let close_icon = icon_text(
        commands,
        &fonts.phosphor,
        "caret-down",
        text_muted(),
        14.0,
    );
    commands.entity(bottom_dock_close).add_child(close_icon);

    // Overlay/Layout mode button, immediately left of the collapse button.
    // Spawned with the `Overlay` glyph and tooltip; `sync_bottom_dock_mode_btn`
    // swaps both whenever the mode changes (including the first frame after a
    // saved `Layout` is restored, since the resource reads as changed then).
    let bottom_dock_mode = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(30.0),
                bottom: Val::Px(dock::BOTTOM_DOCK_HEIGHT - 26.0),
                width: Val::Px(22.0),
                height: Val::Px(22.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::NONE),
            GlobalZIndex(BOTTOM_DOCK_Z + 2),
            Interaction::default(),
            // See the collapse button: without this the resize filler under the
            // corner controls takes the hover, and the cursor.
            bevy::ui::FocusPolicy::Block,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            renzora_ember::widgets::HoverTooltip::new(renzora::lang::t_or(
                "shell.bottom_dock.mode_overlay",
                "Overlay — floats over the workspace",
            )),
            BottomDockModeBtn,
            BottomDockBtn,
            Name::new("bottom-dock-mode"),
        ))
        .id();
    let mode_icon = icon_text(commands, &fonts.phosphor, "stack", text_muted(), 14.0);
    commands.entity(bottom_dock_mode).add_child(mode_icon);

    let bottom_dock_sets = build_bottom_set_menu(commands, fonts);

    commands.entity(dock_wrap).add_children(&[
        dock_area,
        bottom_dock_area,
        bottom_dock_grip,
        bottom_dock_sets,
        bottom_dock_mode,
        bottom_dock_close,
    ]);

    // Collapsed bottom-panel strip: the closed bottom panel's header stays
    // visible (tabs included); [`sync_collapsed_bottom_bar`] shows it and fills
    // the tabs whenever the bottom panel is closed. Sized/styled to match a
    // dock leaf's tab bar.
    let collapsed_bottom = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(28.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(2.0),
                flex_shrink: 0.0,
                border: UiRect::top(Val::Px(1.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(header_bg())),
            BorderColor::all(rgb(divider())),
            // Interactive: dragging the strip's background upward reopens the
            // panel and resizes it in one gesture (see
            // [`collapsed_bottom_bar_drag`]).
            //
            // No `HoverCursor` here — it lives on the strip's filler instead.
            // `apply_cursor_icon` takes the first hovered entity carrying one
            // and does no topmost resolution, so a resize cursor on the bar
            // competes with the tabs and the chevron nested inside it and wins
            // often enough to show ns-resize while hovering a tab.
            Interaction::default(),
            // It sits over whatever the panels below leave exposed, so it must
            // swallow pointer events like any floating surface — without this a
            // click or scroll on the strip bleeds into the viewport behind it.
            // The `RelativeCursorPosition` is required: `update_pointer_over_overlay`
            // queries for it, so a surface without one never matches and the
            // marker does nothing.
            renzora_ember::widgets::OverlaySurface,
            RelativeCursorPosition::default(),
            CollapsedBottomBar,
            Name::new("collapsed-bottom-bar"),
        ))
        .id();

    let statusbar = build_status_bar(commands, fonts, themes, active, theme_menu_open);

    commands
        .entity(root)
        .add_children(&[top_bar, doc_tabs, dock_wrap, collapsed_bottom, statusbar]);

    // Borderless-window edge/corner resize grips, overlaid on the perimeter.
    let grips = build_resize_zones(commands);
    commands.entity(root).add_children(&grips);
}

/// Narrow the hierarchy — and Add Entity — to UI canvases while the UI editor is
/// the surface you are working on.
///
/// **Which surface is visible, not which workspace you are in.** Panels move:
/// the UI editor can be docked into any workspace, the UI workspace can be
/// renamed, and a workspace can hold both surfaces at once. Reading the panels
/// answers the question directly instead of guessing from a label.
///
/// The rule is the obvious one. UI editor showing and no viewport → the tree is
/// about UI. Viewport showing → it is about the scene, whether or not the UI
/// editor is up beside it, because with both on screen you are working across
/// them and hiding half the scene is the wrong answer. Neither → unchanged.
///
/// Only canvases, not their contents: a canvas's widgets come from its `.html`
/// and are rebuilt from that file on every load, so the tree would be offering
/// rows whose edits the next rebuild discards. They are already excluded — see
/// the `HideInHierarchy` note in `markup/loader.rs`.
///
/// `HierarchyFilter` has existed for this since before the UI workspace did —
/// its own doc gives "UI workspace only shows cameras + canvases" as the
/// example. Nothing had ever set it. `SpawnCategoryScope` is its companion for
/// Add Entity: a tree showing only UI canvases must not offer to spawn a point
/// light, because the thing you spawned would not appear in the list you
/// spawned it from.
fn sync_hierarchy_filter_to_workspace(
    dock: Option<Res<renzora_ember::dock::Dock>>,
    fixed: Option<Res<renzora_ember::dock::FixedDock>>,
    wins: Option<Res<renzora_ember::dock::DockWindows>>,
    filter: Option<ResMut<renzora_editor_framework::HierarchyFilter>>,
    spawn_scope: Option<ResMut<renzora::SpawnCategoryScope>>,
) {
    let visible = |id: &str| {
        renzora_ember::dock::panel_visible_anywhere(
            id,
            dock.as_deref(),
            fixed.as_deref(),
            wins.as_deref(),
        )
    };
    let is_ui = visible("ui_canvas") && !visible("viewport");
    if let Some(mut filter) = filter {
        let desired = if is_ui {
            renzora_editor_framework::HierarchyFilter::OnlyWithComponents(vec!["UiCanvas"])
        } else {
            renzora_editor_framework::HierarchyFilter::All
        };
        if *filter != desired {
            *filter = desired;
        }
    }
    if let Some(mut scope) = spawn_scope {
        let desired = is_ui.then(|| vec!["ui"]);
        if scope.0 != desired {
            scope.0 = desired;
        }
    }
}

// `sync_viewport_view_to_workspace` lived here: it put the viewport into
// `ViewportView::Ui` on entering the UI workspace and gave the previous view
// back on the way out. It was the right fix while the UI editor was mounted
// inside the viewport panel — and it stopped being needed the moment the editor
// became the `ui_canvas` panel, which is what the UI workspace docks. The
// workspace no longer has anything to say about what the viewport is looking at.

/// Persist the per-workspace dock layout to `~/.renzora/layout.json` whenever it
/// settles after a change, so split ratios / panel placement / active tabs come
/// back on the next launch.
///
/// Triggers on a real change to either the live [`Dock`] (a divider/tab drag, a
/// tab switch) or [`ShellLayouts`] (workspace add/remove/rename/reorder), but the
/// write is **debounced** two ways: it waits until the left mouse button is
/// released (a divider drag mutates the tree every frame — we want one write at
/// the end, not hundreds mid-drag), and it skips the disk write when the
/// serialized layout is byte-identical to what was last written (so the system
/// never churns the file). The live active workspace's tree is synced into its
/// slot for the snapshot without mutating [`ShellLayouts`] — mutating it here
/// would re-trigger change detection and spin the system every frame.
#[allow(clippy::too_many_arguments)]
fn persist_dock_layout(
    dock: Res<Dock>,
    layouts: Res<ShellLayouts>,
    bottom: Res<BottomDock>,
    bottom_sets: Res<BottomPanelSets>,
    fixed: Res<renzora_ember::dock::FixedDock>,
    floats: Res<renzora_ember::dock::DockWindows>,
    windows: Query<&Window>,
    mut moved: MessageReader<bevy::window::WindowMoved>,
    mut resized: MessageReader<bevy::window::WindowResized>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut pending: Local<bool>,
    mut last_saved: Local<Option<String>>,
) {
    // Floating-window geometry changes arrive as window events, not resource
    // mutations — fold them into the trigger so an OS move/resize of a tear-off
    // window persists once it settles.
    let float_geo_changed = moved
        .read()
        .map(|m| m.window)
        .chain(resized.read().map(|r| r.window))
        .any(|w| floats.0.iter().any(|s| s.window == w));
    if dock.is_changed()
        || layouts.is_changed()
        || bottom.is_changed()
        || bottom_sets.is_changed()
        || fixed.is_changed()
        || floats.is_changed()
        || float_geo_changed
    {
        *pending = true;
    }
    if !*pending || mouse.pressed(MouseButton::Left) {
        return;
    }
    *pending = false;

    // Snapshot all workspaces with the active slot's tree taken from the live
    // dock (the in-resource copy is only synced on workspace switch).
    let mut snapshot = layouts.layouts.clone();
    if let Some(slot) = snapshot.get_mut(layouts.active) {
        slot.1 = dock.tree.clone();
    }
    // Snapshot every floating dock window's tree + client geometry.
    let floating: Vec<dock::FloatingLayout> = floats
        .0
        .iter()
        .filter_map(|st| {
            let win = windows.get(st.window).ok()?;
            let position = match win.position {
                bevy::window::WindowPosition::At(p) => Some((p.x, p.y)),
                _ => None,
            };
            Some(dock::FloatingLayout {
                tree: st.tree.clone(),
                position,
                size: (
                    win.resolution.physical_width(),
                    win.resolution.physical_height(),
                ),
            })
        })
        .collect();
    // Same trick as the workspace snapshot above: the live set's copy in the
    // resource is stale between switches, so take that one from `FixedDock`.
    let sets: Vec<dock::BottomPanelSet> = bottom_sets
        .sets
        .iter()
        .enumerate()
        .map(|(i, (name, tree))| dock::BottomPanelSet {
            name: name.clone(),
            tree: if i == bottom_sets.active {
                fixed.tree.clone()
            } else {
                tree.clone()
            },
        })
        .collect();
    let bottom_dock = dock::BottomDockLayout {
        tree: fixed.tree.clone(),
        height: bottom.height,
        open: bottom.open,
        mode: bottom.mode,
        sets,
        active: bottom_sets.active,
    };
    let Some(json) = dock::layout_json(&snapshot, layouts.active, &floating, &bottom_dock) else {
        return;
    };
    if last_saved.as_deref() == Some(json.as_str()) {
        return;
    }
    match dock::write_layout(&json) {
        Ok(()) => *last_saved = Some(json),
        Err(e) => warn!("[shell] failed to persist dock layout: {e}"),
    }
}

/// Open `url` in the user's default browser (cross-platform).
pub(crate) fn open_url(url: &str) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}
