//! `renzora_shell` — the bevy_ui-native editor shell.
//!
//! The editor's chrome (menu bar, ribbon, document tabs, status bar) plus the
//! wiring that drives the reusable [`renzora_ember`] dock. The dock itself —
//! splits, tabs, drag-docking — lives in `renzora_ember::dock`; the shell just
//! supplies the layout, the dock area, and editor-specific behavior.

use bevy::prelude::*;
use renzora_ember::reactive::Rx;
use bevy::ui::{ComputedNode, RelativeCursorPosition, UiGlobalTransform};

use renzora::NativePanelIds;
use renzora_ember::dock::{tab_pane, Dock, DockArea, DockDirty, DockLeaf, DockTab, TabPane};
use renzora_ember::font::{glyph, icon_text, ui_font, EmberFonts};
use renzora_ember::widgets::{
    menu_item, scroll_area_keyed, screen_menu, text_input, EmberTextInput, Popup,
};
use bevy::ui::{BackgroundGradient, ColorStop, LinearGradient};
use renzora_ember::theme::{
    accent, border, divider, header_bg, mix, panel_bg, placeholder, play_green, rgb, tab_active,
    text_muted, text_primary, window_bg,
};
use renzora_ember::EmberPlugin;

pub mod dock;
mod about;
mod plugin_install;

use dock::DockTree;
use renzora::core::keybindings::{EditorAction, KeyBindings};

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
        app.init_resource::<OpenTopMenu>();
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
                (top_menu_open, top_menu_hover, top_menu_sync, update_chip_click),
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

/// Map the active `ThemeManager` theme into ember's runtime palette, and rebuild
/// the chrome when the active theme *changes* (a switch) so widgets re-spawn with
/// the new colors. Individual color edits update the palette but don't rebuild
/// (that would close the Theme tab's color picker every frame).
fn theme_bridge(
    tm: Option<Res<renzora_theme::ThemeManager>>,
    project: Option<Res<renzora::CurrentProject>>,
    asset_server: Res<AssetServer>,
    mut last_name: Local<Option<String>>,
    mut last_pal: Local<Option<renzora_ember::theme::Palette>>,
    mut last_syntax: Local<Option<renzora_ember::theme::SyntaxPalette>>,
    mut last_effects: Local<Option<String>>,
    mut last_themes: Local<Option<Vec<String>>>,
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

    // Chrome shader effects (matrix rain, …). Gated on a real change — the apply
    // reads the theme folder's `.wgsl` off disk, so re-running it every frame
    // would hammer the filesystem. The fingerprint folds in the theme name (so a
    // switch re-applies) and the effect fields (so a live edit does too).
    // Fold every referenced shader file's mtime in too, so editing a theme's
    // `.wgsl` and saving hot-reloads the effect without reselecting the theme.
    let eff = &tm.active_theme.effects;
    let imgs = &tm.active_theme.images;
    let files = [
        &eff.top_bar, &eff.doc_tabs, &eff.status_bar, &eff.panel, &eff.panel_header,
        &imgs.top_bar, &imgs.doc_tabs, &imgs.status_bar, &imgs.panel, &imgs.panel_header,
    ];
    let mut shader_mtime: u64 = 0;
    if let Some(dir) = tm.active_theme_dir() {
        for f in files {
            if f.is_empty() {
                continue;
            }
            if let Some(secs) = std::fs::metadata(dir.join(f))
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
            {
                shader_mtime = shader_mtime.max(secs);
            }
        }
    }
    let eff_fp = format!(
        "{}\u{1}{}|{}|{}|{}|{}\u{1}{}|{}|{}|{}|{}\u{1}{}\u{1}{}",
        tm.active_theme_name,
        eff.top_bar, eff.doc_tabs, eff.status_bar, eff.panel, eff.panel_header,
        imgs.top_bar, imgs.doc_tabs, imgs.status_bar, imgs.panel, imgs.panel_header,
        tm.active_theme.fonts.ui,
        shader_mtime,
    );
    if last_effects.as_deref() != Some(eff_fp.as_str()) {
        apply_theme_effects(
            &tm.active_theme,
            &tm.active_theme_name,
            tm.active_theme_dir(),
            &asset_server,
        );
        apply_theme_fonts(
            &tm.active_theme,
            &tm.active_theme_name,
            tm.active_theme_dir(),
            &asset_server,
        );
        *last_effects = Some(eff_fp);
    }

    // Map the theme's syntax colors into the code editor's palette. Tracked
    // separately so a syntax-only edit pushes through without churning `pal`.
    let syn = syntax_palette_from_theme(&tm.active_theme);
    if last_syntax.as_ref() != Some(&syn) {
        renzora_ember::theme::set_syntax_palette(syn);
        *last_syntax = Some(syn);
    }

    // The status-bar theme dropup is built once (when the chrome spawns) from a
    // snapshot of `available_themes`. The chrome can spawn *before* the project's
    // `themes/*.toml` are scanned (fonts may load during Splash/Loading, while the
    // scan only runs on `OnEnter(Editor)`), so that snapshot can be the bare
    // Dark/Light built-ins. When the list later changes, rebuild the chrome so the
    // dropup re-spawns with the full set. (Guarded to a real change so it doesn't
    // churn.)
    let themes_changed = last_themes.as_deref().is_some_and(|t| t != tm.available_themes.as_slice());
    if last_themes.as_deref() != Some(tm.available_themes.as_slice()) {
        *last_themes = Some(tm.available_themes.clone());
    }

    let first = last_name.is_none();
    let switched = last_name.as_deref().is_some_and(|n| n != tm.active_theme_name);
    if first || switched {
        *last_name = Some(tm.active_theme_name.clone());
        // Palette is current → build the ember Theme: palette-derived defaults
        // cascaded with the active theme file's per-widget style sections.
        let theme = build_ember_theme(project.as_deref(), &tm.active_theme_name);
        commands.insert_resource(theme);
    }
    // A theme switch rebuilds for the new palette; a theme-list change rebuilds the
    // (already-built) chrome so the dropup re-spawns with the full set. If the
    // chrome isn't up yet, the list change needs no rebuild — `manage_shell_root`
    // will spawn it fresh from the current list.
    //
    // CANCEL any pending dock rebuild rather than requesting one: the `DockArea`
    // lives under `ShellRoot`, so the despawn queued here dooms the whole dock
    // tree. If `DockDirty` were set now and ember's `rebuild_dock` ran later this
    // same frame (their relative order is unconstrained), it would queue detach/
    // despawn/reparent commands against that doomed tree — and since our despawn
    // applies first, those commands would hit dead entities and panic with
    // "Entity despawned" (GH issue #67: instant crash on theme change; whether it
    // fired depended on the schedule order the binary happened to build).
    // `manage_shell_root` sets `DockDirty` when it respawns the chrome, which is
    // the only correct moment to rebuild the dock.
    if switched || (themes_changed && !roots.is_empty()) {
        for e in &roots {
            commands.entity(e).try_despawn();
        }
        dirty.0 = false;
    }
}

/// Mirror the theme dropup's live open state into [`ThemeMenuOpen`] so it
/// survives the chrome rebuild a theme switch triggers. The generic `popup_toggle`
/// / `popup_dismiss` systems own the `Popup.open` flag (trigger toggles it,
/// outside-click clears it); this just copies it out to the persistent resource,
/// and is a no-op while the dropup is absent (e.g. mid-rebuild).
fn sync_theme_menu_open(
    dropup: Query<&Popup, With<ThemeDropup>>,
    mut open: ResMut<ThemeMenuOpen>,
) {
    if let Ok(p) = dropup.single() {
        if open.0 != p.open {
            open.0 = p.open;
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
        let [r, g, b, _] = c.0;
        (r, g, b)
    }
    renzora_ember::theme::Palette {
        window_bg: tc(&t.surfaces.window),
        panel_bg: tc(&t.surfaces.panel),
        faint_bg: tc(&t.surfaces.faint),
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

/// The theme-relative shader file a surface paints with (empty = none).
fn surface_shader_rel(eff: &renzora_theme::ThemeEffects, s: renzora_ember::widgets::ThemeSurface) -> &str {
    use renzora_ember::widgets::ThemeSurface as S;
    match s {
        S::TopBar => &eff.top_bar,
        S::DocTabs => &eff.doc_tabs,
        S::StatusBar => &eff.status_bar,
        S::Panel => &eff.panel,
        S::PanelHeader => &eff.panel_header,
    }
}

/// The theme-relative image file a surface paints with (empty = none).
fn surface_image_rel(img: &renzora_theme::ThemeImages, s: renzora_ember::widgets::ThemeSurface) -> &str {
    use renzora_ember::widgets::ThemeSurface as S;
    match s {
        S::TopBar => &img.top_bar,
        S::DocTabs => &img.doc_tabs,
        S::StatusBar => &img.status_bar,
        S::Panel => &img.panel,
        S::PanelHeader => &img.panel_header,
    }
}

/// Push a theme's `[effects]` + `[images]` into ember **per surface**. Each
/// surface resolves to: its own shader (if `[effects]` names one), else the
/// built-in image display (if `[images]` names one), else off — and, in all
/// cases, binds the surface's image so a custom shader can sample it. Shaders are
/// read off disk; images load via the asset server (project-relative path, like
/// fonts). Callers gate on a real change.
fn apply_theme_effects(
    theme: &renzora_theme::Theme,
    theme_name: &str,
    theme_dir: Option<&std::path::Path>,
    asset_server: &AssetServer,
) {
    use renzora_ember::widgets::{set_surface_image, set_surface_shader, shader_key, SurfaceSource, ThemeSurface};

    for surface in ThemeSurface::ALL {
        let shader_rel = surface_shader_rel(&theme.effects, surface);
        let image_rel = surface_image_rel(&theme.images, surface);
        let has_shader = !shader_rel.is_empty();
        let has_image = !image_rel.is_empty();

        // Bind the surface's image (or clear → default white texture).
        if has_image {
            let rel = format!("themes/{}/{}", theme_name, image_rel.replace('\\', "/"));
            set_surface_image(surface, Some(asset_server.load::<bevy::image::Image>(rel)));
        } else {
            set_surface_image(surface, None);
        }

        // Resolve the shader source for this surface.
        if !has_shader && !has_image {
            set_surface_shader(surface, None); // off
            continue;
        }
        let req = if has_shader {
            match theme_dir.map(|d| d.join(shader_rel)) {
                Some(path) => match std::fs::read_to_string(&path) {
                    Ok(src) => (shader_key(&src), SurfaceSource::Custom(src)),
                    Err(e) => {
                        warn!("[theme] effect shader {:?} unreadable: {e} — using built-in", path);
                        (0, SurfaceSource::Builtin)
                    }
                },
                None => (0, SurfaceSource::Builtin),
            }
        } else {
            // Image only → built-in image display shader.
            (1, SurfaceSource::Image)
        };
        set_surface_shader(surface, Some(req));
    }
}

/// Load a theme's `[fonts]` from its own folder and set the editor's UI-font
/// override (cleared when the theme ships no font). Loaded by project-relative
/// path (`themes/<Name>/<file>`) — the same basis the font scanner uses — so the
/// asset server dedupes it and a shipped game can pack it. `AssetServer::load` is
/// async/cheap, but callers still gate this on an actual theme change.
fn apply_theme_fonts(
    theme: &renzora_theme::Theme,
    theme_name: &str,
    theme_dir: Option<&std::path::Path>,
    asset_server: &AssetServer,
) {
    use bevy::text::{Font, FontSource};
    let src = match (theme.fonts.ui.is_empty(), theme_dir) {
        (false, Some(_)) => {
            let rel = format!("themes/{}/{}", theme_name, theme.fonts.ui.replace('\\', "/"));
            Some(FontSource::Handle(asset_server.load::<Font>(rel)))
        }
        _ => None, // no font shipped (or unknown folder) → user's font setting
    };
    renzora_ember::font::set_theme_ui_font(src);
}

/// Map a theme's `syntax` section into the code editor's [`SyntaxPalette`].
/// Token colors drop alpha (they're opaque); chrome colors that overlay text
/// keep their full RGBA.
fn syntax_palette_from_theme(t: &renzora_theme::Theme) -> renzora_ember::theme::SyntaxPalette {
    fn tc(c: &renzora_theme::ThemeColor) -> (u8, u8, u8) {
        let [r, g, b, _] = c.0;
        (r, g, b)
    }
    let s = &t.syntax;
    renzora_ember::theme::SyntaxPalette {
        normal: tc(&s.normal),
        keyword: tc(&s.keyword),
        type_: tc(&s.r#type),
        function: tc(&s.function),
        number: tc(&s.number),
        string: tc(&s.string),
        comment: tc(&s.comment),
        operator: tc(&s.operator),
        constant: tc(&s.constant),
        punctuation: tc(&s.punctuation),
        line_number: tc(&s.line_number),
        line_number_active: tc(&s.line_number_active),
        current_line: s.current_line.0,
        selection: s.selection.0,
        cursor: tc(&s.cursor),
        indent_guide: s.indent_guide.0,
        bracket_match: s.bracket_match.0,
        find_match: s.find_match.0,
    }
}


/// Play + its target caret as one tight split button, in the top bar's left
/// zone. Kept as its own group so the zone's item spacing doesn't pull the caret
/// away from the pill it belongs to; a left margin sets it off from the session
/// actions before it. It has previously lived at the trailing end of the
/// viewport's own tool strip — the top bar wins because running the game is not
/// a viewport action, and this bar is on screen in every workspace.
fn build_play_group(commands: &mut Commands, font: &bevy::text::FontSource) -> Entity {
    let play = build_play_button(commands, font);
    let caret = build_play_target_caret(commands, font);
    let group = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(1.0),
                margin: UiRect::left(Val::Px(8.0)),
                ..default()
            },
            Name::new("play-group"),
        ))
        .id();
    commands.entity(group).add_children(&[play, caret]);
    group
}

/// The Play / Stop button (icon + text). This is the editor's single play
/// control now that the viewport toolbar's play/scripts buttons are gone.
#[derive(Component)]
struct TopBarPlayBtn;
/// The play button's phosphor glyph (swaps play ↔ stop with state).
#[derive(Component)]
struct TopBarPlayIcon;
/// The play button's "Play" / "Stop" text label.
#[derive(Component)]
struct TopBarPlayLabel;

/// Build the top-bar Play button: a phosphor glyph + a "Play" label in one
/// clickable pill. The glyph + label live as `FocusPolicy::Pass` children so the
/// hover/click lands on the parent (where `Interaction` lives).
fn build_play_button(commands: &mut Commands, font: &bevy::text::FontSource) -> Entity {
    let btn = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            TopBarPlayBtn,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("top-bar-play"),
        ))
        .id();
    let icon = glyph(commands, "play", play_green(), 13.0);
    commands
        .entity(icon)
        .insert((TopBarPlayIcon, bevy::ui::FocusPolicy::Pass));
    let label = commands
        .spawn((
            Text::new(renzora::lang::t("common.play")),
            ui_font(font, 11.0),
            TextColor(rgb(play_green())),
            TopBarPlayLabel,
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    commands.entity(btn).add_children(&[icon, label]);
    btn
}

/// Whether any global (autoload) scene supplies a camera.
///
/// The Play gate asks "is there a scene camera", and until now asked it of the
/// *live world*. Global scenes don't load until Play, so a project whose only
/// camera lives in one could never start: the camera isn't there to open the
/// gate, and the gate is what would load it.
///
/// Answered from the scene files rather than the world, since that is the only
/// place the information exists while editing.
#[derive(Resource, Default)]
struct GlobalSceneHasCamera(bool);

/// Recompute [`GlobalSceneHasCamera`] when the autoload list changes.
///
/// A substring test for the component's type name, not a parse: the answer only
/// gates a button, both scene formats spell the type the same way, and a wrong
/// answer degrades safely — a false positive lets Play start and
/// `enter_play_mode` reports "no scene camera found" as it already does for an
/// empty scene.
fn track_global_scene_cameras(
    project: Option<Res<renzora::CurrentProject>>,
    mut state: ResMut<GlobalSceneHasCamera>,
    mut last: Local<Option<Vec<String>>>,
) {
    let Some(project) = project else { return };
    if last.as_ref() == Some(&project.config.autoload) {
        return;
    }
    *last = Some(project.config.autoload.clone());
    state.0 = project.config.autoload.iter().any(|rel| {
        std::fs::read_to_string(project.resolve_path(rel))
            .map(|text| text.contains("SceneCamera"))
            .unwrap_or(false)
    });
}

/// Click the top-bar Play button → launch the mode picked in the play-target
/// dropdown (full play, or Simulate when that's the selection) from Editing
/// with a scene camera; or stop (while playing, simulating, or while an
/// external runtime is alive).
fn play_btn_click(
    btns: Query<&Interaction, (Changed<Interaction>, With<TopBarPlayBtn>)>,
    play_mode: Option<ResMut<renzora::core::PlayModeState>>,
    runtime: Option<Res<renzora_viewport::external_runtime::ExternalRuntime>>,
    scene_cams: Query<(), With<renzora::core::SceneCamera>>,
    settings: Option<Res<renzora_editor_framework::EditorSettings>>,
    global_cam: Option<Res<GlobalSceneHasCamera>>,
) {
    let Some(mut pm) = play_mode else { return };
    let runtime_alive = runtime.is_some_and(|r| r.is_alive());
    // A camera in a global scene counts even though it isn't loaded yet.
    let has_cam = !scene_cams.is_empty() || global_cam.is_some_and(|g| g.0);
    let simulate = settings.is_some_and(|s| s.play_launch_simulate);
    for interaction in &btns {
        if *interaction != Interaction::Pressed {
            continue;
        }
        // `is_in_play_mode` deliberately EXCLUDES Simulating, so cover it too.
        if runtime_alive || pm.is_in_play_mode() || pm.is_simulating() {
            pm.request_stop = true;
        } else if pm.is_editing() && has_cam {
            if simulate {
                pm.request_simulate = true;
            } else {
                pm.request_play = true;
            }
        }
    }
}

/// Fullscreen takeover shown while an in-process VR session renders to the
/// headset. The editor's offscreen cameras are suspended meanwhile (see
/// `renzora_viewport::sync_viewport_camera_activation`), so without this the
/// panels would sit on a frozen stale frame; instead the whole window reads
/// unambiguously as "the headset owns the session". Stop (or taking the
/// session down from the headset) removes it.
#[derive(Component)]
struct VrActiveOverlay;

fn vr_active_overlay(
    mut commands: Commands,
    vr: Option<Res<renzora::VrPlayState>>,
    existing: Query<Entity, With<VrActiveOverlay>>,
    fonts: Option<Res<EmberFonts>>,
) {
    let active = vr.as_ref().is_some_and(|v| v.active);
    match (active, existing.iter().next()) {
        (true, None) => {
            let Some(fonts) = fonts else { return };
            let root = commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        row_gap: Val::Px(14.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.02, 0.02, 0.03, 0.96)),
                    GlobalZIndex(5000),
                    bevy::ui::FocusPolicy::Block,
                    // Registers with ember's pointer-blocking pass so clicks
                    // can't reach panels underneath (see overlay conventions).
                    renzora_ember::widgets::OverlaySurface,
                    VrActiveOverlay,
                    Name::new("vr-active-overlay"),
                ))
                .id();
            let icon = glyph(&mut commands, "virtual-reality", (140, 160, 220), 56.0);
            let title = commands
                .spawn((
                    Text::new(renzora::lang::t_or("shell.vr_active", "VR Mode Active")),
                    ui_font(&fonts.ui, 22.0),
                    TextColor(Color::srgb(0.92, 0.94, 1.0)),
                ))
                .id();
            let hint = commands
                .spawn((
                    Text::new(renzora::lang::t_or(
                        "shell.vr_active_hint",
                        "The scene is playing in the headset. Press Stop to return.",
                    )),
                    ui_font(&fonts.ui, 13.0),
                    TextColor(Color::srgb(0.55, 0.58, 0.68)),
                ))
                .id();
            commands.entity(root).add_children(&[icon, title, hint]);
        }
        (false, Some(entity)) => {
            commands.entity(entity).try_despawn();
        }
        _ => {}
    }
}

/// Drive the Play button's glyph + label + color from play state and the
/// selected launch mode: green "Play" (or blue flask "Simulate" when that mode
/// is picked) when editing — muted if there's no scene camera — and red "Stop"
/// while playing, simulating, or an external runtime is alive. The idle label
/// also names the play target ("Play Viewport", "Play VR"; see
/// [`PlayLaunchChoice::play_label`]), so the caret menu's selection is visible
/// on the button itself.
fn update_play_button(
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    runtime: Option<Res<renzora_viewport::external_runtime::ExternalRuntime>>,
    theme: Option<Res<renzora_theme::ThemeManager>>,
    scene_cams: Query<(), With<renzora::core::SceneCamera>>,
    settings: Option<Res<renzora_editor_framework::EditorSettings>>,
    global_cam: Option<Res<GlobalSceneHasCamera>>,
    mut icons: Query<&mut renzora_ember::icons::Icon, With<TopBarPlayIcon>>,
    mut labels: Query<(&mut Text, &mut TextColor), With<TopBarPlayLabel>>,
    mut fills: Query<(&mut BackgroundColor, &Interaction), With<TopBarPlayBtn>>,
) {
    let Some(theme) = theme else { return };
    let t = &theme.active_theme;
    let tc = |c: renzora_theme::ThemeColor| {
        let [r, g, b, _] = c.to_array();
        Color::srgb_u8(r, g, b)
    };
    let green = tc(t.semantic.success);
    let red = tc(t.semantic.error);
    let muted = tc(t.text.muted);

    let active = runtime.is_some_and(|r| r.is_alive())
        || play_mode
            .as_ref()
            .is_some_and(|p| p.is_in_play_mode() || p.is_simulating());
    // Matches `play_btn_click`: a global scene's camera counts, so the button
    // doesn't read as disabled while the click handler would accept it.
    let has_cam = !scene_cams.is_empty() || global_cam.is_some_and(|g| g.0);
    let choice = settings
        .as_deref()
        .map(PlayLaunchChoice::current)
        .unwrap_or(PlayLaunchChoice::Viewport);
    let simulate = choice == PlayLaunchChoice::Simulate;

    // `icon_name` is a phosphor glyph name (not localized); the label IS localized.
    let (icon_name, color, playing) = if active {
        ("stop", red, true)
    } else {
        let (idle_icon, idle_color) = if simulate {
            ("flask", rgb(SIM_BLUE))
        } else {
            ("play", green)
        };
        (idle_icon, if has_cam { idle_color } else { muted }, false)
    };
    let label_text = if playing {
        renzora::lang::t("common.stop")
    } else {
        choice.play_label()
    };

    for mut icon in &mut icons {
        if icon.name != icon_name {
            icon.name = icon_name.to_string();
            icon.resolved = false; // force `apply_icons` to re-render the glyph
        }
        if icon.color != Some(color) {
            icon.color = Some(color);
            icon.resolved = false;
        }
    }
    for (mut text, mut tcolor) in &mut labels {
        if text.0 != label_text {
            text.0 = label_text.clone();
        }
        if tcolor.0 != color {
            tcolor.0 = color;
        }
    }
    // A tinted fill so the control reads as a *button*, not as green text on the
    // toolbar. Derived from the same state color the icon and label use, so Play
    // / Simulate / Stop each wash the pill in their own hue, and dimmed along
    // with them when there's no scene camera to play through.
    for (mut bg, interaction) in &mut fills {
        let alpha = match interaction {
            Interaction::Pressed => 0.34,
            Interaction::Hovered => 0.26,
            Interaction::None => 0.16,
        };
        let want = color.with_alpha(alpha);
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

/// The slim caret beside the Play pill that opens the play-target menu.
#[derive(Component)]
struct PlayTargetCaret;

/// What the Play button launches — the selection made in the play-target menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlayLaunchChoice {
    /// Full play inside the editor viewport panel.
    Viewport,
    /// Full play in its own OS runtime window (project window settings).
    Window,
    /// Full play in a VR headset: the external runtime process launched with
    /// `--vr` (OpenXR stereo rendering + a desktop mirror window).
    Vr,
    /// Simulate: scripts + physics tick while the editor stays live.
    Simulate,
}

impl PlayLaunchChoice {
    /// The mode currently selected, resolved from [`EditorSettings`].
    fn current(s: &renzora_editor_framework::EditorSettings) -> Self {
        if s.play_launch_simulate {
            Self::Simulate
        } else if s.play_launch_vr {
            Self::Vr
        } else if s.external_play_window {
            Self::Window
        } else {
            Self::Viewport
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Viewport => "frame-corners",
            Self::Window => "app-window",
            Self::Vr => "virtual-reality",
            Self::Simulate => "flask",
        }
    }

    /// What the Play button reads while idle. Window stays the plain "Play" —
    /// launching the game in its own window is what a play button ordinarily
    /// means — while the targets that put the game somewhere else name
    /// themselves, so the button says where the next Play will run without
    /// having to open the caret menu to check.
    fn play_label(self) -> String {
        match self {
            Self::Viewport => renzora::lang::t_or("shell.play_button.viewport", "Play Viewport"),
            Self::Window => renzora::lang::t("common.play"),
            Self::Vr => renzora::lang::t_or("shell.play_button.vr", "Play VR"),
            Self::Simulate => renzora::lang::t("common.simulate"),
        }
    }
}

/// A row in the play-target menu; picking it makes the Play button launch that
/// mode.
#[derive(Component)]
struct PlayTargetOption {
    choice: PlayLaunchChoice,
}
/// The leading glyph of a play-target row — a check on the selected row, the
/// option's own icon on the others (mirrors the theme menu's check-or-icon slot).
#[derive(Component)]
struct PlayTargetOptionIcon {
    choice: PlayLaunchChoice,
}

/// Build the play-target dropdown: a caret beside the Play pill opening a menu
/// that picks where Play runs — inside the editor viewport, or in an actual
/// runtime window using the project's window settings (title, resolution,
/// window mode, resizable). Picking an option writes
/// `EditorSettings.external_play_window` and persists it per-user, so the
/// choice sticks across sessions; the next Play uses it.
fn build_play_target_caret(commands: &mut Commands, font: &bevy::text::FontSource) -> Entity {
    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(100.0),
                right: Val::Px(0.0),
                margin: UiRect::top(Val::Px(2.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                min_width: Val::Px(120.0),
                padding: UiRect::all(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::popup_bg())),
            BorderColor::all(rgb(divider())),
            GlobalZIndex(600),
            RelativeCursorPosition::default(),
            Name::new("play-target-menu"),
        ))
        .id();

    // Window and VR are the two targets that need something outside this
    // process: Window spawns `<exe_dir>/renzora` as a child process (see
    // `renzora_viewport::external_runtime`) and VR needs an OpenXR device. A
    // browser tab has neither, so the web editor doesn't offer them.
    //
    // Viewport and Simulate both run in-process and work unchanged — which is
    // the whole reason play mode needed no porting for the web build.
    let mut choices = vec![(
        PlayLaunchChoice::Viewport,
        "frame-corners",
        renzora::lang::t_or("shell.play_target.viewport", "Viewport"),
    )];
    #[cfg(not(target_arch = "wasm32"))]
    {
        choices.push((
            PlayLaunchChoice::Window,
            "app-window",
            renzora::lang::t_or("shell.play_target.runtime_window", "Window"),
        ));
        choices.push((
            PlayLaunchChoice::Vr,
            "virtual-reality",
            renzora::lang::t_or("shell.play_target.vr", "VR Headset"),
        ));
    }
    choices.push((
        PlayLaunchChoice::Simulate,
        "flask",
        renzora::lang::t_or("common.simulate", "Simulate"),
    ));

    let mut rows = Vec::new();
    for (choice, icon_name, label) in choices {
        let row = commands
            .spawn((
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(8.0),
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Interaction::default(),
                PlayTargetOption { choice },
                renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                Name::new("play-target-option"),
            ))
            .id();
        renzora_ember::reactive::tracked::bind_bg(commands, row, move |w| match w.get::<Interaction>(row) {
            Some(Interaction::Hovered) | Some(Interaction::Pressed) => {
                rgb(renzora_ember::theme::hover_bg())
            }
            _ => Color::NONE,
        });
        let ic = glyph(commands, icon_name, text_muted(), 12.0);
        commands.entity(ic).insert((
            PlayTargetOptionIcon { choice },
            bevy::ui::FocusPolicy::Pass,
        ));
        let t = commands
            .spawn((
                Text::new(label),
                ui_font(font, 12.0),
                TextColor(rgb(text_primary())),
                bevy::ui::FocusPolicy::Pass,
            ))
            .id();
        commands.entity(row).add_children(&[ic, t]);
        rows.push(row);
    }
    commands.entity(panel).add_children(&rows);

    let trigger = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(2.0), Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                position_type: PositionType::Relative,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            Popup { panel, open: false },
            PlayTargetCaret,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("play-target-caret"),
        ))
        .id();
    renzora_ember::reactive::tracked::bind_bg(commands, trigger, move |w| {
        match w.get::<Interaction>(trigger) {
            Some(Interaction::Hovered) | Some(Interaction::Pressed) => {
                Color::srgba(1.0, 1.0, 1.0, 0.09)
            }
            _ => Color::NONE,
        }
    });
    let caret = glyph(commands, "caret-down", text_muted(), 10.0);
    commands.entity(caret).insert(bevy::ui::FocusPolicy::Pass);
    commands.entity(trigger).add_children(&[caret, panel]);
    trigger
}

/// Pick a play-target row → write the launch mode, persist the viewport/window
/// half of it, close the menu. Simulate is a session-only choice layered on
/// top: it doesn't touch the persisted viewport-vs-window preference, so
/// dropping back out of Simulate restores whichever of the two was saved.
fn play_target_option_click(
    opts: Query<(&Interaction, &PlayTargetOption), Changed<Interaction>>,
    mut settings: Option<ResMut<renzora_editor_framework::EditorSettings>>,
    carets: Query<Entity, (With<PlayTargetCaret>, With<Popup>)>,
    mut commands: Commands,
) {
    for (interaction, opt) in &opts {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(s) = settings.as_mut() {
            match opt.choice {
                PlayLaunchChoice::Simulate => s.play_launch_simulate = true,
                PlayLaunchChoice::Vr => {
                    s.play_launch_simulate = false;
                    s.play_launch_vr = true;
                    let _ = renzora::save_play_vr(true);
                }
                PlayLaunchChoice::Viewport | PlayLaunchChoice::Window => {
                    s.play_launch_simulate = false;
                    s.play_launch_vr = false;
                    let _ = renzora::save_play_vr(false);
                    let runtime_window = opt.choice == PlayLaunchChoice::Window;
                    s.external_play_window = runtime_window;
                    let _ = renzora::save_play_runtime_window(runtime_window);
                }
            }
        }
        for caret in &carets {
            renzora_ember::widgets::close_popup(&mut commands, caret);
        }
    }
}

/// Keep each play-target row's leading glyph in sync with the current launch
/// mode: the selected row shows a green check, the others show their own icons.
fn update_play_target_menu(
    settings: Option<Res<renzora_editor_framework::EditorSettings>>,
    theme: Option<Res<renzora_theme::ThemeManager>>,
    mut icons: Query<(&mut renzora_ember::icons::Icon, &PlayTargetOptionIcon)>,
) {
    let Some(settings) = settings else { return };
    let current = PlayLaunchChoice::current(&settings);
    let green = theme
        .map(|t| {
            let [r, g, b, _] = t.active_theme.semantic.success.to_array();
            Color::srgb_u8(r, g, b)
        })
        .unwrap_or_else(|| rgb(play_green()));
    for (mut icon, opt) in &mut icons {
        let (name, color) = if opt.choice == current {
            ("check", green)
        } else {
            (opt.choice.icon(), rgb(text_muted()))
        };
        if icon.name != name {
            icon.name = name.to_string();
            icon.resolved = false;
        }
        if icon.color != Some(color) {
            icon.color = Some(color);
            icon.resolved = false;
        }
    }
}

/// Simulate's accent colour (blue) — distinct from Play's green so the two
/// launch modes read apart at a glance on the Play button.
const SIM_BLUE: (u8, u8, u8) = (86, 169, 247);

renzora::add!(ShellPlugin, Editor);

/// The ribbon's workspace layouts and which one is active. Switching saves the
/// current dock tree back into the active slot (so per-layout edits persist)
/// and loads the chosen one into the ember [`Dock`].
#[derive(Resource)]
struct ShellLayouts {
    layouts: Vec<(String, DockTree)>,
    active: usize,
}

/// The global bottom panel's chrome state: how tall it is when open, and
/// whether it is open at all. Its *contents* live in
/// [`renzora_ember::dock::FixedDock`] — this is only what the shell needs to
/// size and show the area node.
///
/// This replaced a `BTreeMap<workspace_name, ClosedBottom>`. The old model had
/// to stash the bottom region's whole subtree out of the workspace tree when
/// closed, because that was the only place those panels existed; closing was
/// therefore a destructive tree edit that had to round-trip exactly. Now the
/// tree is held out of the workspace layouts permanently, so closing is just
/// `open = false` — nothing moves, and nothing can be lost by failing to
/// restore it.
#[derive(Resource)]
struct BottomDock {
    /// Logical px, applied to the area node when open.
    height: f32,
    open: bool,
    /// Whether the panel floats over the workspace or takes height from it.
    mode: dock::BottomDockMode,
    /// How far the slide-open animation has got: 0 = fully closed, 1 = fully
    /// open at `height`. Chased toward `open` by [`animate_bottom_dock`], and
    /// the only thing [`sync_bottom_dock_node`] scales the node by — `open` is
    /// still the state everything else reads, so nothing else has to know the
    /// panel moves rather than appearing.
    ///
    /// Deliberately not persisted: a session starts at whichever end of the
    /// travel `open` says, not mid-slide.
    slide: f32,
}

/// The bottom panel's named tab-sets, and which one is live.
///
/// Mirrors how [`ShellLayouts`] relates to [`Dock`]: the *live* tree is the one
/// in [`renzora_ember::dock::FixedDock`], and `sets[active].1` is only refreshed
/// when the user switches away from it (or when the layout is saved). Reading
/// the active slot's tree straight out of here therefore gives you the panel as
/// it was when it last went out of view, not as it is now.
#[derive(Resource)]
struct BottomPanelSets {
    sets: Vec<(String, DockTree)>,
    active: usize,
}

/// The name a bottom panel gets when it has never had a second set — the case
/// for every layout written before sets existed.
fn default_panel_set_name() -> String {
    renzora::lang::t_or("shell.bottom_dock.set_default", "Default")
}

/// `Default 2`, `Default 3`, … — the first numbered name the panel isn't already
/// using, so removing set 2 and adding another gives back `Default 2` rather
/// than climbing forever.
fn next_panel_set_name(taken: &[(String, DockTree)]) -> String {
    let base = default_panel_set_name();
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|name| !taken.iter().any(|(n, _)| n == name))
        .unwrap_or(base)
}

/// Make `index` the live set: park the tree the panel is showing back in the
/// slot it came from, then hand ember the new one.
///
/// The park is what makes switching lossless — the live tree has been edited in
/// `FixedDock` (tabs dragged, panels closed) and the copy in `sets` is stale by
/// exactly those edits.
fn activate_panel_set(
    sets: &mut BottomPanelSets,
    fixed: &mut renzora_ember::dock::FixedDock,
    index: usize,
) {
    if index >= sets.sets.len() {
        return;
    }
    let live = fixed.tree.clone();
    if let Some(slot) = sets.sets.get_mut(sets.active) {
        slot.1 = live;
    }
    sets.active = index;
    fixed.tree = sets.sets[index].1.clone();
    // The area node exists by the time any of this is reachable (the menu that
    // calls it lives in the same chrome), so a rebuild is always wanted.
    fixed.dirty = true;
}

/// The panel-set dropdown's trigger — a name + caret in the panel's top-right
/// corner, left of the Overlay/Layout button.
#[derive(Component)]
struct BottomSetTrigger;
/// The trigger's label, kept on the active set's name.
#[derive(Component)]
struct BottomSetLabel;
/// The dropdown's panel. Its rows are rebuilt from [`BottomPanelSets`] rather
/// than spawned once, because the set list changes while the chrome stands.
#[derive(Component)]
struct BottomSetMenu;
/// The "New Panel Set" row.
#[derive(Component)]
struct BottomSetNew;
/// The "Remove This Set" row. Present only while more than one set exists —
/// removing the last one would leave the panel with nowhere to put a tab.
#[derive(Component)]
struct BottomSetRemove;
/// The pencil at a set row's right edge, carrying the set it renames.
#[derive(Component)]
struct BottomSetRenameBtn(usize);
/// The set currently being inline-renamed (`None` = none), read by
/// [`sync_bottom_set_menu`] so that row renders a text field in place of its
/// name. Mirrors [`RibbonRename`], which does the same for workspaces.
#[derive(Resource, Default)]
struct BottomSetRename(Option<usize>);
/// Marks the inline rename field, carrying the set index it renames.
#[derive(Component)]
struct BottomSetRenameInput(usize);

/// A draggable set row: which set it is, and the insertion bar drawn at its top
/// edge while a reorder drag points at that slot. The vertical twin of
/// [`DocTabItem`].
#[derive(Component)]
struct BottomSetItem {
    index: usize,
    marker: Entity,
}

/// The in-flight reorder of the panel sets, if any.
#[derive(Resource, Default)]
struct BottomSetDrag(Option<BottomSetDragState>);

struct BottomSetDragState {
    /// The set being carried, by index at the time of the press.
    index: usize,
    start_cursor: Vec2,
    /// Cleared until the cursor has moved far enough to call this a drag rather
    /// than a click — which is what lets one gesture mean both.
    active: bool,
    /// Insertion slot in the *pre-removal* list, as [`reorder_panel_sets`] takes
    /// it.
    target: usize,
}

/// Move set `from` to insertion slot `to`, keeping `active` pointed at the same
/// set it was before.
///
/// `to` is a slot in the list *as it stands*, so both the set's own slot and the
/// one just past it mean "don't move" — the same convention
/// `DocumentTabState::reorder` uses, and the reason the caller can hand over a
/// marker position without adjusting for the removal itself.
///
/// Only the names and trees move. The set the panel is *showing* lives in
/// `FixedDock`, not in `sets[active].1`, so a reorder never has to touch the
/// live tree — it only has to keep `active` on the right slot.
fn reorder_panel_sets(sets: &mut BottomPanelSets, from: usize, to: usize) {
    if from >= sets.sets.len() || to > sets.sets.len() || to == from || to == from + 1 {
        return;
    }
    let set = sets.sets.remove(from);
    let at = if to > from { to - 1 } else { to };
    let at = at.min(sets.sets.len());
    sets.sets.insert(at, set);
    if sets.active == from {
        sets.active = at;
    } else if from < sets.active && sets.active <= at {
        sets.active -= 1;
    } else if at <= sets.active && sets.active < from {
        sets.active += 1;
    }
}

/// Build the panel-set dropdown: trigger + (empty) menu panel.
///
/// The menu is a child of the trigger so it anchors to it, and which way it
/// opens is decided per-open by ember's `popup_position`: **down into the panel
/// when the panel is tall enough to hold it, up over the workspace when it
/// isn't.** The trigger rides the top edge of a panel whose height is the
/// user's to choose, so neither direction is the safe one to hard-code — a
/// short panel has no room below (the dock wrapper clips at the status bar),
/// and a panel dragged up to the top bar has none above.
///
/// Authored downward, which is what it gets on the first frame of an open,
/// before the menu has a measured height to flip on.
fn build_bottom_set_menu(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(100.0),
                right: Val::Px(0.0),
                margin: UiRect::top(Val::Px(4.0)),
                min_width: Val::Px(180.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                padding: UiRect::all(Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(renzora_ember::theme::popup_bg())),
            BorderColor::all(rgb(divider())),
            GlobalZIndex(BOTTOM_DOCK_Z + 3),
            // Not decoration: without both of these the menu is invisible to
            // `correct_pointer_state`, so a click on one of its rows *also*
            // lands in whatever panel is behind it.
            renzora_ember::widgets::OverlaySurface,
            RelativeCursorPosition::default(),
            // Same reason as the trigger: the panel's own background hangs over
            // the dock header's resize filler.
            bevy::ui::FocusPolicy::Block,
            BottomSetMenu,
            Name::new("bottom-set-menu"),
        ))
        .id();

    let label = commands
        .spawn((
            Text::new(default_panel_set_name()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::no_wrap(),
            Node {
                min_width: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
            BottomSetLabel,
        ))
        .id();
    let caret = glyph(commands, "caret-down", text_muted(), 10.0);
    commands.entity(caret).insert(bevy::ui::FocusPolicy::Pass);

    let trigger = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                // Clear of the mode button at 30 and the collapse button at 6.
                right: Val::Px(54.0),
                bottom: Val::Px(dock::BOTTOM_DOCK_HEIGHT - 26.0),
                height: Val::Px(22.0),
                max_width: Val::Px(160.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                padding: UiRect::horizontal(Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                // No `Overflow::clip()` here — the menu is a child of this node,
                // and a clipping parent clips absolutely-positioned descendants
                // too. The label below carries the clip instead.
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::NONE),
            GlobalZIndex(BOTTOM_DOCK_Z + 2),
            Interaction::default(),
            // Not optional. `Node`'s required `FocusPolicy` is `Pass` in Bevy
            // 0.19, so hover falls *through* this button to the dock header's
            // filler underneath — which is the panel's resize surface and
            // carries an ns-resize `HoverCursor`. `apply_cursor_icon` takes the
            // first hovered entity with a cursor and does no topmost
            // resolution, so the filler won and the dropdown showed a resize
            // cursor. Blocking keeps the hover here, where it belongs.
            bevy::ui::FocusPolicy::Block,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            renzora_ember::widgets::Popup::new(panel),
            // No tooltip: the control already reads as what it is (a named set
            // plus a caret), and a bubble over the panel's own top edge covers
            // the tabs it's about to switch.
            BottomSetTrigger,
            // Shown/hidden and vertically placed with the panel's other corner
            // controls by `sync_bottom_dock_node`.
            BottomDockBtn,
            Name::new("bottom-set-trigger"),
        ))
        .id();
    renzora_ember::reactive::tracked::bind_bg(commands, trigger, move |w| {
        match w.get::<Interaction>(trigger) {
            Some(Interaction::Hovered) | Some(Interaction::Pressed) => {
                Color::srgba(1.0, 1.0, 1.0, 0.09)
            }
            _ => Color::NONE,
        }
    });
    commands
        .entity(trigger)
        .add_children(&[label, caret, panel]);
    trigger
}

/// One row of the panel-set menu: a leading glyph and a label.
fn bottom_set_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    icon_color: (u8, u8, u8),
    label: String,
) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            // Block, like the trigger: a row sits over the panel's header
            // filler, which is a resize surface (see the trigger's comment).
            bevy::ui::FocusPolicy::Block,
            // The reorder drag hit-tests in the cursor's own space rather than
            // against node centres, which drift under UI scaling — the lesson
            // `ribbon_interact` and the document tabs both learned the hard way.
            RelativeCursorPosition::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("bottom-set-row"),
        ))
        .id();
    renzora_ember::reactive::tracked::bind_bg(commands, row, move |w| {
        match w.get::<Interaction>(row) {
            Some(Interaction::Hovered) | Some(Interaction::Pressed) => {
                rgb(renzora_ember::theme::hover_bg())
            }
            _ => Color::NONE,
        }
    });
    let ic = glyph(commands, icon, icon_color, 12.0);
    commands.entity(ic).insert(bevy::ui::FocusPolicy::Pass);
    let text = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::no_wrap(),
            // Takes the slack, so a trailing button (the rename pencil) sits at
            // the row's right edge rather than against the name.
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    commands.entity(row).add_children(&[ic, text]);
    row
}

/// The inline rename field for panel set `index`, styled like the ribbon's.
fn build_bottom_set_rename_field(
    commands: &mut Commands,
    font: &bevy::text::FontSource,
    index: usize,
    name: &str,
) -> Entity {
    let input = renzora_ember::widgets::text_input(commands, font, "Name", name);
    commands.entity(input).insert((
        BottomSetRenameInput(index),
        Node {
            width: Val::Percent(100.0),
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

/// What [`sync_bottom_set_menu`] compares against to decide the menu it built
/// is still the right one: menu entity, the set names, the live set, and the
/// set being renamed.
type BottomSetMenuKey = (Entity, Vec<String>, usize, Option<usize>);

/// Fill the panel-set menu from [`BottomPanelSets`], and keep the trigger's
/// label on the active set's name.
///
/// Rebuilt on change rather than reconciled: the list is a handful of rows and
/// only moves when the user opens the menu and acts on it, so the churn the
/// reactive lists were built to avoid doesn't arise. Keyed on the menu entity
/// as well as the contents, because a theme or language change respawns the
/// chrome and hands us a fresh, childless panel.
#[allow(clippy::too_many_arguments)]
fn sync_bottom_set_menu(
    sets: Res<BottomPanelSets>,
    rename: Res<BottomSetRename>,
    fonts: Option<Res<EmberFonts>>,
    theme: Option<Res<renzora_theme::ThemeManager>>,
    menus: Query<Entity, With<BottomSetMenu>>,
    mut labels: Query<&mut Text, With<BottomSetLabel>>,
    mut commands: Commands,
    mut built: Local<Option<BottomSetMenuKey>>,
) {
    let (Some(fonts), Ok(menu)) = (fonts, menus.single()) else {
        return;
    };
    let names: Vec<String> = sets.sets.iter().map(|(n, _)| n.clone()).collect();
    // `rename.0` is part of the key: entering and leaving rename mode swaps one
    // row between a label and a text field, and nothing else about the set list
    // changes when it does.
    if built.as_ref() == Some(&(menu, names.clone(), sets.active, rename.0)) {
        return;
    }
    *built = Some((menu, names.clone(), sets.active, rename.0));

    for mut text in &mut labels {
        let want = names.get(sets.active).cloned().unwrap_or_default();
        if text.0 != want {
            text.0 = want;
        }
    }

    let green = theme
        .map(|t| {
            let [r, g, b, _] = t.active_theme.semantic.success.to_array();
            (r, g, b)
        })
        .unwrap_or_else(play_green);

    commands.entity(menu).despawn_related::<Children>();
    let mut rows = Vec::new();
    for (i, name) in names.iter().enumerate() {
        // The row being renamed is the field, not a label — it can't also be a
        // click target, or typing in it would keep re-activating the set.
        if rename.0 == Some(i) {
            let holder = commands
                .spawn((
                    Node {
                        width: Val::Percent(100.0),
                        padding: UiRect::axes(Val::Px(4.0), Val::Px(2.0)),
                        ..default()
                    },
                    Name::new("bottom-set-rename-row"),
                ))
                .id();
            let field = build_bottom_set_rename_field(&mut commands, &fonts.ui, i, name);
            commands.entity(holder).add_child(field);
            rows.push(holder);
            continue;
        }
        // Check on the live set, the set's own glyph on the others — the same
        // check-or-icon slot the theme and play-target menus use.
        let (icon, color) = if i == sets.active {
            ("check", green)
        } else {
            ("squares-four", text_muted())
        };
        let row = bottom_set_row(&mut commands, &fonts, icon, color, name.clone());
        // Insertion bar for a reorder drag: a hairline of accent across the
        // row's top edge, hidden until the drag points at this slot. Absolute,
        // so it costs the row no height and can't shift the menu as it moves.
        let marker = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(0.0),
                    top: Val::Px(-1.0),
                    width: Val::Percent(100.0),
                    height: Val::Px(2.0),
                    display: Display::None,
                    ..default()
                },
                BackgroundColor(rgb(accent())),
                bevy::ui::FocusPolicy::Pass,
                Name::new("bottom-set-insert-marker"),
            ))
            .id();
        commands.entity(row).add_child(marker);
        // `BottomSetItem` is both the click target and the drag handle — the
        // row's index is the one thing either needs, so there's no separate
        // marker component for "this row picks a set".
        commands.entity(row).insert(BottomSetItem { index: i, marker });
        // Rename pencil at the row's right edge. `Block`, or the press also
        // reaches the row and switches to that set on the way into the field —
        // `Node`'s required `FocusPolicy` is `Pass` in Bevy 0.19.
        let pencil = commands
            .spawn((
                Node {
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    width: Val::Px(16.0),
                    height: Val::Px(16.0),
                    flex_shrink: 0.0,
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Interaction::default(),
                bevy::ui::FocusPolicy::Block,
                BottomSetRenameBtn(i),
                renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                Name::new("bottom-set-rename"),
            ))
            .id();
        let pencil_icon = glyph(&mut commands, "pencil-simple", text_muted(), 11.0);
        commands.entity(pencil_icon).insert(bevy::ui::FocusPolicy::Pass);
        commands.entity(pencil).add_child(pencil_icon);
        commands.entity(row).add_child(pencil);
        rows.push(row);
    }
    let sep = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(1.0),
                margin: UiRect::vertical(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(divider())),
        ))
        .id();
    rows.push(sep);
    let new_row = bottom_set_row(
        &mut commands,
        &fonts,
        "plus",
        text_muted(),
        renzora::lang::t_or("shell.bottom_dock.set_new", "New Panel Set"),
    );
    commands.entity(new_row).insert(BottomSetNew);
    rows.push(new_row);
    if names.len() > 1 {
        let remove_row = bottom_set_row(
            &mut commands,
            &fonts,
            "trash",
            text_muted(),
            renzora::lang::t_or("shell.bottom_dock.set_remove", "Remove This Set"),
        );
        commands.entity(remove_row).insert(BottomSetRemove);
        rows.push(remove_row);
    }
    commands.entity(menu).add_children(&rows);
}

/// Drive the panel-set menu: pick a set, add one, drop the live one, or start
/// renaming one.
///
/// Every branch but the rename closes the menu, so the result is visible
/// immediately rather than behind a popup the user still has to dismiss —
/// rename is the exception because the field it opens *is* in the menu.
#[allow(clippy::too_many_arguments)]
fn bottom_set_menu_click(
    new_rows: Query<&Interaction, (With<BottomSetNew>, Changed<Interaction>)>,
    remove_rows: Query<&Interaction, (With<BottomSetRemove>, Changed<Interaction>)>,
    pencils: Query<(&Interaction, &BottomSetRenameBtn), Changed<Interaction>>,
    mut sets: ResMut<BottomPanelSets>,
    mut rename: ResMut<BottomSetRename>,
    mut fixed: ResMut<renzora_ember::dock::FixedDock>,
    mut bottom: ResMut<BottomDock>,
    triggers: Query<Entity, With<BottomSetTrigger>>,
    mut commands: Commands,
) {
    for (interaction, pencil) in &pencils {
        if *interaction == Interaction::Pressed {
            rename.0 = Some(pencil.0);
        }
    }
    let mut acted = false;
    // Picking a set isn't here: it fires on *release*, in [`bottom_set_drag`],
    // because the same press can be the start of a reorder. Acting on the press
    // would switch sets and close the menu out from under the drag.
    for interaction in &new_rows {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let name = next_panel_set_name(&sets.sets);
        // Empty on purpose: ember renders an "Add Panel" button for an empty
        // tree, which is exactly the right first step in a brand new set.
        sets.sets.push((name, DockTree::Empty));
        let last = sets.sets.len() - 1;
        activate_panel_set(&mut sets, &mut fixed, last);
        // A new set is a request to work in the panel, so make sure it's up.
        bottom.open = true;
        acted = true;
    }
    for interaction in &remove_rows {
        // Guarded here as well as in the builder: the row is only spawned while
        // there's more than one set, but the menu it lives in can outlive that
        // by a frame.
        if *interaction != Interaction::Pressed || sets.sets.len() < 2 {
            continue;
        }
        let gone = sets.active;
        sets.sets.remove(gone);
        // Land on the neighbour that kept its position, so removing the last
        // set doesn't jump to the front.
        let next = gone.min(sets.sets.len() - 1);
        // Not `activate_panel_set`: parking the live tree would write it into
        // whichever set slid into this index.
        sets.active = next;
        fixed.tree = sets.sets[next].1.clone();
        fixed.dirty = true;
        acted = true;
    }
    if acted {
        // Whatever the menu was in the middle of no longer refers to the set
        // list it was opened against.
        rename.0 = None;
        for trigger in &triggers {
            renzora_ember::widgets::close_popup(&mut commands, trigger);
        }
    }
}

/// Press-latch reorder for the panel-set rows, plus the plain click that picks
/// a set: drag a row past a small threshold to move it in [`BottomPanelSets`],
/// or release without moving to switch to it. The vertical twin of
/// [`doc_tab_drag`].
///
/// Both halves of the gesture live here for the reason that split is usually
/// made: only the code that watched the press *and* the motion knows which of
/// the two it was. Switching sets used to happen on the press, in
/// [`bottom_set_menu_click`], which closed the menu — so a drag ended before it
/// started.
///
/// The reorder is applied once, on release, rather than live as the cursor
/// crosses each neighbour: the insertion bar is what shows where the row will
/// land, and a live swap would rebuild the whole menu (and so respawn the row
/// under the cursor) on every crossing.
#[allow(clippy::too_many_arguments)]
fn bottom_set_drag(
    mut drag: ResMut<BottomSetDrag>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    rename: Res<BottomSetRename>,
    pressed: Query<(&BottomSetItem, &Interaction)>,
    items: Query<(&BottomSetItem, &RelativeCursorPosition)>,
    mut nodes: Query<&mut Node>,
    mut sets: ResMut<BottomPanelSets>,
    mut fixed: ResMut<renzora_ember::dock::FixedDock>,
    triggers: Query<Entity, With<BottomSetTrigger>>,
    mut commands: Commands,
) {
    let hide_markers = |items: &Query<(&BottomSetItem, &RelativeCursorPosition)>,
                        nodes: &mut Query<&mut Node>| {
        for (it, _) in items {
            if let Ok(mut n) = nodes.get_mut(it.marker) {
                if n.display != Display::None {
                    n.display = Display::None;
                }
            }
        }
    };

    // A row being renamed is a text field, not a handle.
    if rename.0.is_some() {
        drag.0 = None;
        hide_markers(&items, &mut nodes);
        return;
    }
    let cursor = windows.iter().next().and_then(|w| w.cursor_position());

    if drag.0.is_none() && mouse.just_pressed(MouseButton::Left) {
        if let Some(cur) = cursor {
            for (item, interaction) in &pressed {
                if *interaction == Interaction::Pressed {
                    drag.0 = Some(BottomSetDragState {
                        index: item.index,
                        start_cursor: cur,
                        active: false,
                        target: item.index,
                    });
                    break;
                }
            }
        }
    }

    if let (Some(st), Some(cur)) = (drag.0.as_mut(), cursor) {
        if (cur - st.start_cursor).length() > 5.0 {
            st.active = true;
        }
    }

    // Which slot the cursor is pointing at, and the marker that says so: the
    // top half of a row inserts above it, the bottom half below.
    match drag.0.as_mut() {
        Some(st) if st.active => {
            let mut shown: Option<(Entity, bool)> = None;
            for (it, rcp) in &items {
                if !rcp.cursor_over {
                    continue;
                }
                let before = rcp.normalized.is_none_or(|n| n.y < 0.0);
                st.target = if before { it.index } else { it.index + 1 };
                shown = Some((it.marker, !before));
                break;
            }
            hide_markers(&items, &mut nodes);
            if let Some((marker, below)) = shown {
                if let Ok(mut n) = nodes.get_mut(marker) {
                    n.display = Display::Flex;
                    if below {
                        n.top = Val::Auto;
                        n.bottom = Val::Px(-1.0);
                    } else {
                        n.top = Val::Px(-1.0);
                        n.bottom = Val::Auto;
                    }
                }
            }
        }
        _ => hide_markers(&items, &mut nodes),
    }

    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    hide_markers(&items, &mut nodes);
    let Some(st) = drag.0.take() else { return };
    if !st.active {
        // A click: switch to that set and dismiss, which is what this row did
        // when the press handled it.
        if st.index != sets.active {
            activate_panel_set(&mut sets, &mut fixed, st.index);
        }
        for trigger in &triggers {
            renzora_ember::widgets::close_popup(&mut commands, trigger);
        }
        return;
    }
    // A reorder leaves the menu open: you are arranging a list, and having it
    // shut on every move would make ordering three sets three round trips.
    let to = st.target.min(sets.sets.len());
    reorder_panel_sets(&mut sets, st.index, to);
}

/// Focus the panel-set rename field the frame it spawns, so the pencil puts the
/// caret in the name rather than only drawing a box.
fn bottom_set_focus_rename(
    mut fields: Query<&mut renzora_ember::widgets::EmberTextInput, Added<BottomSetRenameInput>>,
) {
    for mut input in &mut fields {
        input.focused = true;
    }
}

/// Commit (Enter / blur) or cancel (Escape) a panel-set rename. The twin of
/// [`ribbon_rename_commit`], which does this for workspaces.
fn bottom_set_rename_commit(
    mut rename: ResMut<BottomSetRename>,
    keys: Res<ButtonInput<KeyCode>>,
    fields: Query<(
        &renzora_ember::widgets::EmberTextInput,
        &BottomSetRenameInput,
    )>,
    mut sets: ResMut<BottomPanelSets>,
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
    let Some((input, _)) = fields.iter().find(|(_, r)| r.0 == index) else {
        return;
    };
    // The blur test needs a frame where the field *was* focused: it spawns
    // unfocused and is focused by `bottom_set_focus_rename`, so without this a
    // rename would commit-and-close on its very first frame.
    if input.focused {
        *had_focus = true;
    }
    let enter = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter);
    let blurred = *had_focus && !input.focused;
    if !enter && !blurred {
        return;
    }
    let new: String = input.value.replace('\n', "").trim().to_string();
    rename.0 = None;
    *had_focus = false;
    // An empty name would leave a row you can't read or aim at, so a cleared
    // field cancels instead.
    if new.is_empty() {
        return;
    }
    if let Some(slot) = sets.sets.get_mut(index) {
        slot.0 = new;
    }
}

/// Ctrl+Space ([`EditorAction::ToggleBottomPanel`]): show or hide the global
/// bottom panel.
///
/// This used to detach the bottom region out of the active workspace's tree
/// into a per-workspace stash, and re-attach it on reopen — because those
/// panels existed *only* inside that tree, closing was a destructive edit that
/// had to round-trip tab order, active tab, split ratio and an anchor path
/// exactly, drop panels that had reappeared elsewhere meanwhile, and re-key
/// itself whenever a workspace was renamed or removed.
///
/// The tree lives in [`renzora_ember::dock::FixedDock`] now, outside every
/// workspace layout, so none of that applies: hiding the panel hides a node and
/// nothing moves. The three helpers that did the round-trip
/// (`close_bottom_panel`, `reopen_bottom_panel`, `bottom_snap_collapse`) are
/// gone with it.
///
/// Opening always goes to [`default_open_height`] rather than to the height the
/// panel last had, for the same reason clicking a collapsed tab does: the
/// remembered height can be anything, including the near-minimum a drag-to-close
/// leaves behind, and a shortcut that opens the panel to a sliver reads as
/// broken. The chevron is the control that reopens at the remembered height.
fn toggle_bottom_panel(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Option<Res<KeyBindings>>,
    input_focus: Option<Res<renzora::core::InputFocusState>>,
    wraps: Query<&ComputedNode, With<DockAreaWrap>>,
    mut bottom: ResMut<BottomDock>,
) {
    let Some(kb) = keybindings else { return };
    if kb.rebinding.is_some() || input_focus.is_some_and(|f| f.ui_wants_keyboard) {
        return;
    }
    if !kb.just_pressed(EditorAction::ToggleBottomPanel, &keyboard) {
        return;
    }
    bottom.open = !bottom.open;
    if bottom.open {
        if let Some(h) = default_open_height(&wraps) {
            bottom.height = h;
        }
    }
}

/// Share of the dock region the bottom panel takes when it is *shown* rather
/// than restored. Enough that the panel opens onto real content — a readable
/// run of console lines, a couple of rows of asset thumbnails — without the
/// workspace above it stopping being the thing you are looking at.
const BOTTOM_DOCK_OPEN_FRACTION: f32 = 0.40;

/// [`BOTTOM_DOCK_OPEN_FRACTION`] of the dock region's height, in logical px,
/// floored at the panel's minimum — the height the bottom panel opens to when
/// something asks for it to be shown rather than restored.
///
/// `None` before the wrapper node has been laid out, which the callers read as
/// "leave the height alone" rather than falling back to a guess.
fn default_open_height(wraps: &Query<&ComputedNode, With<DockAreaWrap>>) -> Option<f32> {
    let avail = dock_region_height(wraps)?;
    Some((avail * BOTTOM_DOCK_OPEN_FRACTION).max(dock::BOTTOM_DOCK_MIN_HEIGHT))
}

/// The dock region's height in logical px — the full span from the top bar down
/// to the status bar, and so the tallest the bottom panel may be dragged.
///
/// `None` before the wrapper node has been laid out — the node exists for a few
/// frames at zero height, which is not a measurement, so a zero reads as "not
/// yet" rather than as a dock region with no room in it. Callers that only need
/// a clamp read that as "no limit yet" (`f32::INFINITY`); callers that would
/// have to *guess* a height read it as "leave it alone".
fn dock_region_height(wraps: &Query<&ComputedNode, With<DockAreaWrap>>) -> Option<f32> {
    let wrap = wraps.iter().next()?;
    let height = wrap.size().y * wrap.inverse_scale_factor();
    (height > 0.0).then_some(height)
}

/// Cap the restored bottom-panel height at [`BOTTOM_DOCK_OPEN_FRACTION`] of the
/// dock region, once, on the first frame the region has a size.
///
/// The panel can be dragged up to the top bar and that height is remembered, so
/// without this an editor that was closed with the panel pulled right up starts
/// the next session with its workspace hidden behind a full-height Assets
/// browser — the state is recoverable, but it is a poor thing to open onto, and
/// it is not what the person who dragged it there was choosing. Capping only at
/// load keeps the drag itself unrestricted: 40% is where a session *starts*, not
/// a ceiling on where it can go.
///
/// A shorter remembered height is left exactly as it was — this is a cap, not a
/// reset.
fn clamp_bottom_dock_on_load(
    wraps: Query<&ComputedNode, With<DockAreaWrap>>,
    mut bottom: ResMut<BottomDock>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let Some(cap) = default_open_height(&wraps) else {
        return;
    };
    *done = true;
    if bottom.height > cap {
        bottom.height = cap;
    }
}

/// Seconds the bottom panel takes to travel its full height. Short enough that
/// `Ctrl+Space` still answers instantly, long enough that the eye follows the
/// panel to where it went instead of it simply being somewhere else — which is
/// the whole point of animating a panel that covers 40% of the editor.
const BOTTOM_DOCK_SLIDE_SECS: f32 = 0.16;

/// Chase [`BottomDock::slide`] toward whatever `open` currently says.
///
/// Every path that opens or closes the panel — the shortcut, both chevrons, a
/// tab click on the collapsed strip, the snap-shut drag, the drag-away hide —
/// writes only `open`, so all of them animate without any of them knowing that
/// they do. [`sync_bottom_dock_node`] is the one place that reads `slide`.
///
/// The resource is written *only* while the value is genuinely moving:
/// [`sync_bottom_dock_mode_btn`] early-outs on `bottom.is_changed()`, and
/// touching the `ResMut` every frame would quietly turn that into no early-out
/// at all.
/// On the **real** clock, not the virtual one: this is editor chrome, and it
/// has to keep moving while play mode is paused or time-scaled.
fn animate_bottom_dock(time: Res<Time<bevy::time::Real>>, mut bottom: ResMut<BottomDock>) {
    let target = if bottom.open { 1.0 } else { 0.0 };
    if bottom.slide == target {
        return;
    }
    // Guard against a zero/absurd delta (a stalled frame, a debugger pause)
    // stretching the slide across seconds of wall clock.
    let step = (time.delta_secs() / BOTTOM_DOCK_SLIDE_SECS).clamp(0.0, 1.0);
    bottom.slide = if bottom.slide < target {
        (bottom.slide + step).min(target)
    } else {
        (bottom.slide - step).max(target)
    };
}

/// Smoothstep the linear slide parameter, so the panel eases out of rest at
/// both ends rather than starting and stopping at full speed.
fn slide_ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The bottom panel's open state as it was when the current drag began, and
/// therefore what it gets back when the drag ends. `None` when no drag is being
/// tracked — including a drag that started with the panel already closed, which
/// this never touches.
#[derive(Resource, Default)]
struct BottomDockDragHide {
    restore: Option<bool>,
}

/// How far above the dock region's bottom edge counts as "near the bottom", and
/// so brings an auto-hidden panel back.
///
/// Deliberately a narrow strip rather than the panel's own footprint. The space
/// a closed panel *would* occupy is a full-width band across the editor, and
/// while it is closed that band is somebody else's — a hierarchy row, an
/// inspector slot, the lower half of the viewport. Reopening the moment a drag
/// crossed into it would put the panel on top of the drop target the user was
/// heading for. Coming back has to be something you ask for by aiming at the
/// bottom of the window, not something that happens on the way past.
const BOTTOM_DOCK_REVEAL_BAND: f32 = 48.0;

/// Drag an asset out of the bottom panel and the panel gets out of your way;
/// bring the drag back to it — or anywhere near the bottom of the editor — and
/// it comes back.
///
/// Almost everything worth dropping an asset *on* is underneath this panel: the
/// viewport, the hierarchy, an inspector slot. Dragging out of the Assets tab
/// therefore starts by covering the target with the panel you dragged from, and
/// the old answer was to close the panel by hand first and lose sight of what
/// you were dragging.
///
/// It writes `open` and nothing else, so the panel *slides* out of the way
/// rather than blinking, and every other system continues to see one ordinary
/// open/closed panel. The state the drag found is restored when the drag ends,
/// wherever it was dropped — an auto-hide that outlived its gesture would just
/// be the panel closing itself for no reason the user can see.
///
/// Shape-library drags are included because that panel is a bottom-panel tab
/// too, and the gesture — drag out of the bottom panel, aim at the viewport — is
/// the identical one.
fn bottom_dock_drag_reveal(
    asset_drag: Option<Res<renzora_ui::AssetDragPayload>>,
    shape_drag: Option<Res<renzora_ui::ShapeDragState>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    wraps: Query<(&ComputedNode, &UiGlobalTransform), With<DockAreaWrap>>,
    mut bottom: ResMut<BottomDock>,
    mut hide: ResMut<BottomDockDragHide>,
) {
    // `is_detached` gates on the pointer having actually moved, so a plain
    // click on an asset never flickers the panel.
    let dragging = asset_drag.is_some_and(|d| d.is_detached)
        || shape_drag.is_some_and(|s| s.dragging_shape.is_some());
    if !dragging {
        if let Some(open) = hide.restore.take() {
            bottom.open = open;
        }
        return;
    }
    let Some(cursor) = windows.single().ok().and_then(|w| w.cursor_position()) else {
        return;
    };
    let Some((node, transform)) = wraps.iter().next() else {
        return;
    };
    let inv = node.inverse_scale_factor();
    let size = node.size() * inv;
    // The node exists at zero height for a few frames after it spawns, which is
    // not a measurement — see `dock_region_height`.
    if size.y <= 0.0 {
        return;
    }
    let region_bottom = transform.translation.y * inv + size.y * 0.5;

    if hide.restore.is_none() {
        // A drag that began with the panel already closed is not ours: the
        // asset came from somewhere else, and opening the panel under the
        // cursor would be a surprise rather than a convenience.
        if !bottom.open {
            return;
        }
        hide.restore = Some(true);
    }

    // The two thresholds are deliberately different, and far apart.
    //
    // Leaving is judged against the panel's *own top edge*: everything below it
    // is the panel, so a drag toward a folder tile or another tab inside it
    // never triggers a hide. Coming back is judged against a narrow strip at
    // the very bottom of the region — see [`BOTTOM_DOCK_REVEAL_BAND`] for why
    // it can't be the footprint again.
    //
    // The gap between them is also what makes this stable. A single threshold
    // put the open and closed states on either side of one line: the panel hid,
    // which moved nothing, so the cursor was still on the line, so a pixel of
    // jitter reopened it — and it flickered for as long as the drag hovered
    // there. With hysteresis there is a wide dead band in the middle where
    // neither test fires and the panel simply stays as it is.
    let open = if bottom.open {
        cursor.y >= region_bottom - dock::clamp_height(bottom.height, size.y)
    } else {
        cursor.y >= region_bottom - BOTTOM_DOCK_REVEAL_BAND
    };
    if bottom.open != open {
        bottom.open = open;
    }
}

/// A live drag of the bottom panel's top edge: `(cursor y at press, panel
/// height at press)`. `None` when no drag is in flight.
///
/// Held in a resource rather than a `Local` because the collapsed strip's
/// drag-to-open gesture arms it from a different system — opening the panel
/// and resizing it are one continuous gesture for the user.
#[derive(Resource, Default)]
struct BottomDockResize {
    active: Option<(f32, f32)>,
}

/// The bottom panel's top-edge resize grip.
#[derive(Component)]
struct BottomDockGrip;

/// The open bottom panel's collapse button — the counterpart of the collapsed
/// strip's open chevron, so the panel can be dismissed without knowing the
/// Ctrl+Space binding.
#[derive(Component)]
struct BottomDockCloseBtn;

/// Shared marker for the open panel's corner buttons (mode, then collapse).
/// They sit in one row and share a placement and visibility rule, so
/// [`sync_bottom_dock_node`] drives them through a single query — which also
/// keeps its `&mut Node` queries disjoint without a third `Without` filter.
#[derive(Component)]
struct BottomDockBtn;

/// Click the open panel's collapse button → close it.
fn bottom_dock_close_click(
    btns: Query<&Interaction, (With<BottomDockCloseBtn>, Changed<Interaction>)>,
    mut bottom: ResMut<BottomDock>,
) {
    if btns.iter().any(|i| matches!(i, Interaction::Pressed)) {
        bottom.open = false;
    }
}

/// The open panel's mode button, immediately left of the collapse button:
/// switch between overlaying the workspace and docking into it.
#[derive(Component)]
struct BottomDockModeBtn;

/// Click the mode button → flip [`BottomDock::mode`], shrinking the panel if
/// that is what it takes for the flip to be visible.
///
/// The button reports the *effective* mode, so at a height only an overlay can
/// have it reads `Overlay` even when `mode` already says `Layout`. Flipping the
/// stored value there would leave the panel looking identical and the button
/// still saying `Overlay` — the control would read as dead. Both branches
/// therefore pull the height down to what layout mode can hold, which is the
/// part of "dock into the workspace" the user is actually asking for.
fn bottom_dock_mode_click(
    btns: Query<&Interaction, (With<BottomDockModeBtn>, Changed<Interaction>)>,
    wraps: Query<&ComputedNode, With<DockAreaWrap>>,
    mut bottom: ResMut<BottomDock>,
) {
    if !btns.iter().any(|i| matches!(i, Interaction::Pressed)) {
        return;
    }
    let avail = dock_region_height(&wraps).unwrap_or(f32::INFINITY);
    let max_docked = dock::max_layout_height(avail);
    match bottom.mode.effective(bottom.height, avail) {
        dock::BottomDockMode::Overlay => {
            bottom.mode = dock::BottomDockMode::Layout;
            if bottom.height > max_docked {
                bottom.height = max_docked;
            }
        }
        dock::BottomDockMode::Layout => bottom.mode = dock::BottomDockMode::Overlay,
    }
}

/// Keep the mode button's glyph and tooltip on the *current* mode — the icon
/// reports what the panel is doing now, not what clicking would do, matching
/// every other stateful toggle in the chrome.
///
/// "Now" means the effective mode: a layout-mode panel dragged too tall to dock
/// is overlaying the workspace, and the button has to say so or the panel's
/// behaviour and its own label disagree. That case gets its own tooltip,
/// because "you are in Overlay" is not the useful thing to say to someone who
/// chose Layout — "drag me back down" is.
fn sync_bottom_dock_mode_btn(
    bottom: Res<BottomDock>,
    wraps: Query<&ComputedNode, With<DockAreaWrap>>,
    mut btns: Query<(&Children, &mut renzora_ember::widgets::HoverTooltip), With<BottomDockModeBtn>>,
    // The button is respawned whenever the chrome is (theme or language
    // switch), always carrying the `Overlay` glyph it was authored with — so a
    // fresh button has to be re-synced even though the mode itself never moved.
    spawned: Query<(), Added<BottomDockModeBtn>>,
    mut text: Query<&mut Text>,
    // Resizing the *window* can flip the effective mode without `BottomDock`
    // moving at all — the panel stands still while the room for a workspace
    // above it runs out. Cheap to measure, so it joins the early-out rather
    // than defeating it.
    mut last_avail: Local<f32>,
) {
    let avail = dock_region_height(&wraps).unwrap_or(f32::INFINITY);
    if !bottom.is_changed() && spawned.is_empty() && *last_avail == avail {
        return;
    }
    *last_avail = avail;
    let effective = bottom.mode.effective(bottom.height, avail);
    let (icon, tip) = match (effective, bottom.mode) {
        (dock::BottomDockMode::Overlay, dock::BottomDockMode::Layout) => (
            "stack",
            renzora::lang::t_or(
                "shell.bottom_dock.mode_forced_overlay",
                "Overlay — too tall to dock; drag it down to return to Layout",
            ),
        ),
        (dock::BottomDockMode::Overlay, _) => (
            "stack",
            renzora::lang::t_or(
                "shell.bottom_dock.mode_overlay",
                "Overlay — floats over the workspace",
            ),
        ),
        (dock::BottomDockMode::Layout, _) => (
            "rows",
            renzora::lang::t_or(
                "shell.bottom_dock.mode_layout",
                "Layout — docked below the workspace",
            ),
        ),
    };
    let Some(glyph) = renzora_ember::phosphor_map::icon_glyph(icon).map(|c| c.to_string()) else {
        return;
    };
    for (children, mut tooltip) in &mut btns {
        if tooltip.0 != tip {
            tooltip.0 = tip.clone();
        }
        for child in children.iter() {
            if let Ok(mut t) = text.get_mut(child) {
                if t.0 != glyph {
                    t.0 = glyph.clone();
                }
            }
        }
    }
}

/// The relatively-positioned wrapper holding the workspace dock area and the
/// bottom panel overlaid on it. Its computed height is the space a bottom-panel
/// resize is allowed to eat into.
#[derive(Component)]
struct DockAreaWrap;

/// Thickness of the bottom panel's top-edge resize band, logical px. Straddles
/// the border so the cursor changes slightly before and after the visible edge —
/// a 1px border is not a target anyone can hit.
const BOTTOM_DOCK_GRIP_H: f32 = 10.0;

/// Stacking tier for the global bottom panel.
///
/// It has to be a `GlobalZIndex` and not merely a later sibling, because
/// `GlobalZIndex` is *global*: any node carrying one is lifted out of its
/// parent's stacking context into the root order. The node-graph widget uses it
/// throughout (canvas, edges, nodes — up to 10), so the Blueprint and Material
/// graph panels were being hoisted to the root order and painting straight over
/// the bottom panel, which had no tier at all and sat in normal flow. Sibling
/// order cannot win against that; only a higher tier can.
///
/// 100 puts it above panel *content* while staying below every floating
/// surface, which must still open over it: the dock's root drop overlay (200),
/// modals and dropdowns (500), menus (700), the tab-drag ghost (1000) and
/// asset-slot drags (2000).
///
/// Winning that way cut the other way once a graph panel was docked *into* this
/// one: its parts, still at 0–10, went under this background and the canvas came
/// up blank. The graph's depths are now relative tiers rebased against whatever
/// it's mounted in (`NgTier` / `ng_rebase_z` in ember), so a graph inside this
/// panel lands at 100–110 and one outside it stays at 0–10. This tier is still
/// what keeps the outside case from painting over us.
const BOTTOM_DOCK_Z: i32 = 100;

/// Push [`BottomDock`] onto the panel node, its resize band and its corner
/// buttons: height, vertical placement, and whether each is displayed.
///
/// Also applies the mode. Both modes leave the panel occupying the bottom
/// `height` px of [`DockAreaWrap`], which is why the absolutely-placed grip and
/// buttons need no mode-specific arithmetic — only the panel node itself
/// changes, between an absolute overlay and an in-flow row of the dock column.
///
/// Height is clamped only to the dock region itself: the panel can be dragged
/// the whole way up to the top bar. It used to stop a fixed strip short of it,
/// on the grounds that a full-height overlay hides the very panels you would
/// click to recover — but the panel's own mode and collapse buttons ride at its
/// top edge, so they stay on screen at any height (and `Ctrl+Space` closes it
/// from anywhere). What the old clamp really protected was *layout* mode, where
/// the same drag squeezes every panel above to nothing; that case is now
/// handled by [`dock::BottomDockMode::effective`] switching the panel to an
/// overlay instead of by refusing the drag.
// The `Without` filters that keep the three `&mut Node` queries disjoint, and
// the `Or` that gathers every hideable interactive node, are both unavoidably
// wordy — a system's parameters are not an argument list a caller threads.
#[allow(clippy::type_complexity)]
fn sync_bottom_dock_node(
    bottom: Res<BottomDock>,
    wraps: Query<&ComputedNode, With<DockAreaWrap>>,
    mut areas: Query<
        &mut Node,
        (
            With<renzora_ember::dock::FixedDockArea>,
            Without<BottomDockGrip>,
            Without<BottomDockBtn>,
        ),
    >,
    mut grips: Query<&mut Node, (With<BottomDockGrip>, Without<BottomDockBtn>)>,
    mut btns: Query<&mut Node, With<BottomDockBtn>>,
    // Every interactive node that this system can hide. Hiding one while the
    // cursor is on it strands its `Interaction` at `Hovered`, because Bevy's
    // focus pass skips hidden entities and never writes the reset — and
    // `apply_cursor_icon` picks the *first* hovered node carrying a
    // `HoverCursor`, so one stranded entry owns the cursor for the whole app.
    // Closing the panel by clicking its own toggle hits this every time.
    //
    // Filtering on a zero `ComputedNode` size did not fix it, so the computed
    // size evidently goes stale the same way. Clearing the state explicitly is
    // the only version that doesn't depend on what Bevy updates for a hidden
    // node.
    mut hidden_interactions: Query<
        &mut Interaction,
        Or<(
            With<BottomDockGrip>,
            With<BottomDockBtn>,
            With<renzora_ember::dock::FixedAreaHeader>,
        )>,
    >,
) {
    // Shown whenever it's open, empty or not. Hiding an empty one used to be
    // the tidier choice — no bare bordered slab — but closing the panel's last
    // tab then took the whole panel away *with its corner controls*, and
    // Ctrl+Space couldn't bring it back: the toggle set `open`, this line
    // immediately re-hid it, and there was no way left to add a panel. An empty
    // one is not blank anyway; ember renders its "Add Panel" button, and the
    // panel-set dropdown sits in the corner beside it.
    //
    // Shown for the whole of the slide, not only while `open` — a closing panel
    // has to stay on screen to be seen leaving.
    let show = bottom.open || bottom.slide > 0.0;
    let avail = dock_region_height(&wraps).unwrap_or(f32::INFINITY);
    // `target` is the height the panel has when it is fully open, and the
    // height every decision below is made against; `eased` is how far along the
    // travel it currently is.
    let target = dock::clamp_height(bottom.height, avail);
    let eased = slide_ease(bottom.slide);
    let want = if show { Display::Flex } else { Display::None };
    // The grip and the corner buttons ride the panel's top edge, so mid-slide
    // they would be somewhere the panel isn't yet — and at the bottom of the
    // travel their inset arithmetic goes negative and puts them under it. They
    // appear once the panel has arrived.
    let show_controls = bottom.open && bottom.slide >= 1.0;
    let want_controls = if show_controls {
        Display::Flex
    } else {
        Display::None
    };

    // Overlay: absolute, pinned to the wrapper's bottom edge, painted over the
    // dock area. Layout: an in-flow row of the dock column, so the dock area's
    // `flex_grow` hands it the remaining height and every panel above reflows.
    // The insets are cleared in layout mode because a relatively-positioned
    // node treats them as an offset rather than an anchor.
    // The *effective* mode, not the stored one: a layout-mode panel dragged
    // past what the workspace can give up renders as an overlay for as long as
    // it stays that tall.
    // Measured against `target`: judging the effective mode by the animated
    // height would start every open in layout mode and flip to overlay partway
    // up, which reparents the panel and makes the whole workspace jump mid-slide.
    let layout_mode = bottom.mode.effective(target, avail) == dock::BottomDockMode::Layout;
    // The two modes have to animate differently, because the thing that moves
    // is different.
    //
    // **Overlay** slides: the panel keeps its full height throughout and
    // travels down past the wrapper's bottom edge, where `DockAreaWrap`'s
    // `Overflow::clip()` takes it. Its contents are laid out once, at the size
    // they will end at, so the tab bar and the panel body ride down intact.
    //
    // **Layout** can't do that — its height *is* the height the workspace above
    // gives up, and a panel translated out of view would leave the gap it was
    // occupying. So it opens as an accordion: the height itself grows, and
    // every panel above reflows into what's left, which is the same thing
    // dragging its top edge already does.
    let (height, bottom_inset) = if layout_mode {
        (target * eased, Val::Auto)
    } else {
        (target, Val::Px(-target * (1.0 - eased)))
    };
    // Cleared in layout mode because a relatively-positioned node treats an
    // inset as an offset rather than as an anchor.
    let (position_type, left_inset) = if layout_mode {
        (PositionType::Relative, Val::Auto)
    } else {
        (PositionType::Absolute, Val::Px(0.0))
    };
    if let Ok(mut node) = areas.single_mut() {
        // Reads go through `Deref` (no change flag); only assign on a real
        // change, since any `Node` write triggers a relayout.
        if node.display != want {
            node.display = want;
        }
        if node.height != Val::Px(height) {
            node.height = Val::Px(height);
        }
        if node.position_type != position_type {
            node.position_type = position_type;
        }
        if node.left != left_inset {
            node.left = left_inset;
        }
        if node.bottom != bottom_inset {
            node.bottom = bottom_inset;
        }
    }
    if let Ok(mut node) = grips.single_mut() {
        if node.display != want_controls {
            node.display = want_controls;
        }
        // Centre the band on the panel's top edge so the drag works from just
        // above it as well as just below. Placed against `target`, not the
        // animated height: it is hidden until the panel arrives there, and
        // writing a moving inset would relayout it for nothing.
        let offset = Val::Px(target - BOTTOM_DOCK_GRIP_H * 0.5);
        if node.bottom != offset {
            node.bottom = offset;
        }
    }
    // The corner buttons are only shown while the panel is open — the collapsed
    // strip carries its own chevron for the closed state, in the same corner, so
    // the toggle appears continuous as the panel opens and closes.
    for mut node in &mut btns {
        if node.display != want_controls {
            node.display = want_controls;
        }
        // Sit inside the panel, clear of the resize band above it, so a press
        // near the corner can't be ambiguous between closing and resizing.
        // Against `target` for the same reason as the grip above.
        let offset = Val::Px(target - 26.0);
        if node.bottom != offset {
            node.bottom = offset;
        }
    }
    // Nothing hidden may stay `Hovered` (see the query's comment). Keyed on the
    // controls rather than on the panel: they are the nodes this system hides,
    // and mid-slide they are hidden while the panel itself is still up.
    if !show_controls {
        for mut interaction in &mut hidden_interactions {
            if *interaction != Interaction::None {
                *interaction = Interaction::None;
            }
        }
    }
}

/// Reset the hover/press state of everything *inside* the bottom panel on the
/// frame the panel is hidden.
///
/// Same hazard [`sync_bottom_dock_node`] handles for its own corner controls,
/// but for the panel's contents, and with teeth: Bevy's focus pass skips hidden
/// entities, so an asset tile or a folder row that was under the cursor when
/// the panel went away keeps reading `Hovered` forever. The asset browser's
/// drop handler treats a hovered *folder* as "move the dragged files in here",
/// so a stranded one turns a drop into the viewport into a file move — and
/// [`bottom_dock_drag_reveal`] hides the panel mid-drag as a matter of course,
/// which is exactly the moment a folder row is likely to be the last thing the
/// cursor touched.
///
/// Only on the transition, so closing the panel doesn't cost a subtree walk
/// every frame it stays closed.
fn clear_bottom_dock_hover_on_hide(
    bottom: Res<BottomDock>,
    areas: Query<Entity, With<renzora_ember::dock::FixedDockArea>>,
    children: Query<&Children>,
    mut interactions: Query<&mut Interaction>,
    mut was_shown: Local<bool>,
) {
    let shown = bottom.open || bottom.slide > 0.0;
    if shown == *was_shown {
        return;
    }
    *was_shown = shown;
    if shown {
        return;
    }
    let Ok(area) = areas.single() else { return };
    let mut stack = vec![area];
    while let Some(entity) = stack.pop() {
        if let Ok(mut interaction) = interactions.get_mut(entity) {
            if *interaction != Interaction::None {
                *interaction = Interaction::None;
            }
        }
        if let Ok(kids) = children.get(entity) {
            stack.extend(kids.iter());
        }
    }
}

/// Press the grip → start a resize, recording where the cursor was and how
/// tall the panel was at that moment.
///
/// Reads the *current* `Interaction` on the `just_pressed` frame rather than
/// filtering on `Changed<Interaction>`. That mirrors ember's `divider_drag`,
/// which drives the identical gesture: a `Changed` filter only sees the frame
/// the transition is written, so any frame where the press and the focus update
/// don't line up drops the gesture entirely and the handle reads as dead.
fn bottom_dock_grip_press(
    mouse: Res<ButtonInput<MouseButton>>,
    grips: Query<&Interaction, With<BottomDockGrip>>,
    headers: Query<&Interaction, With<renzora_ember::dock::FixedAreaHeader>>,
    tabs: Query<&Interaction, With<DockTab>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    bottom: Res<BottomDock>,
    mut resize: ResMut<BottomDockResize>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let on_grip = grips.iter().any(|i| matches!(i, Interaction::Pressed));
    // The panel also resizes by dragging its header's empty space, which is a
    // bigger and more obvious target than a 10px edge band. The marker sits on
    // the tab bar's filler, so it spans only the gap after the tabs.
    //
    // The tab check is belt and braces: `FocusPolicy` defaults to `Pass` in
    // Bevy 0.19, so a press can be seen by more than one node, and a resize
    // starting because someone clicked a tab would be worse than a resize that
    // occasionally needs a second try.
    let on_header = headers.iter().any(|i| matches!(i, Interaction::Pressed))
        && !tabs
            .iter()
            .any(|i| matches!(i, Interaction::Pressed | Interaction::Hovered));
    if !on_grip && !on_header {
        return;
    }
    let Some(cursor_y) = windows
        .single()
        .ok()
        .and_then(|w| w.cursor_position())
        .map(|c| c.y)
    else {
        return;
    };
    resize.active = Some((cursor_y, bottom.height));
}

/// Drive a live bottom-panel resize, and snap the panel closed when dragged
/// hard down past its minimum — the counterpart of the collapsed strip's
/// drag-up-to-open, so the panel can be dismissed with the same gesture that
/// opened it.
fn bottom_dock_resize_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    wraps: Query<&ComputedNode, With<DockAreaWrap>>,
    mut bottom: ResMut<BottomDock>,
    mut resize: ResMut<BottomDockResize>,
) {
    if !mouse.pressed(MouseButton::Left) {
        resize.active = None;
        return;
    }
    let Some((start_y, start_h)) = resize.active else {
        return;
    };
    let Some(cursor_y) = windows
        .single()
        .ok()
        .and_then(|w| w.cursor_position())
        .map(|c| c.y)
    else {
        return;
    };
    // Cursor y grows downward, so dragging up (smaller y) grows the panel.
    let height = start_h + (start_y - cursor_y);
    if height < dock::BOTTOM_DOCK_MIN_HEIGHT * 0.5 {
        bottom.open = false;
        // Snap, don't slide. The panel has been following the cursor down for
        // the whole gesture and is already at a sliver; animating the last few
        // px would play a transition *behind* a cursor that has finished
        // moving, and read as lag rather than as the panel leaving.
        bottom.slide = 0.0;
        resize.active = None;
        return;
    }
    // Clamped to the dock region here as well as in `sync_bottom_dock_node`, so
    // the height that gets *persisted* is one the panel can actually have —
    // otherwise dragging past the top bar banks metres of overshoot that the
    // next drag downward has to unwind before the panel so much as moves.
    let avail = dock_region_height(&wraps).unwrap_or(f32::INFINITY);
    bottom.height = dock::clamp_height(height, avail);
}

/// The collapsed bottom-panel strip: a tab-bar-height row between the dock
/// area and the status bar — exactly where the closed bottom panel's header
/// would sit — showing the stashed region's tabs in a muted, closed state.
/// Hidden while the bottom panel is open (or the workspace never had one).
#[derive(Component)]
struct CollapsedBottomBar;

/// One tab in the collapsed strip; clicking reopens the bottom panel with
/// this panel as the active tab.
#[derive(Component)]
struct CollapsedBottomTab(String);

/// The open chevron at the right end of the collapsed strip; clicking
/// reopens the bottom panel (counterpart of the open panel's collapse
/// chevron).
#[derive(Component)]
struct CollapsedBottomOpenBtn;

/// Keep the collapsed strip in sync with the global bottom panel: shown with
/// one tab per panel in the [`renzora_ember::dock::FixedDock`] tree while the
/// panel is closed, hidden while it's open. Tab children rebuild only when the
/// tab set (or the bar entity, after a chrome respawn) changes.
///
/// It reads the same tree the open panel renders, rather than a stash of what
/// was detached — so the strip lists the panel's real contents in every
/// workspace, and a panel added to the bottom dock while it happens to be
/// closed shows up here immediately.
///
/// **An empty panel still gets a strip**, showing the active set's name in
/// place of the tabs it has none of. Hiding it was the tidier choice — no bare
/// bar under a panel with nothing in it — but it made the panel destroy itself:
/// close the last tab, then collapse, and the strip went with the panel's own
/// corner controls, leaving nothing on screen to click. Ctrl+Space still
/// reopened it, but only for someone who knew the binding; everyone else had to
/// reset the layout to get the panel back. The strip is the one thing that must
/// survive the panel being empty, because reopening is what makes ember's "Add
/// Panel" button reachable again.
#[allow(clippy::too_many_arguments)]
fn sync_collapsed_bottom_bar(
    bottom: Res<BottomDock>,
    fixed: Res<renzora_ember::dock::FixedDock>,
    sets: Res<BottomPanelSets>,
    fonts: Option<Res<EmberFonts>>,
    registry: Option<Res<renzora::core::ShellPanelRegistry>>,
    bars: Query<Entity, With<CollapsedBottomBar>>,
    mut nodes: Query<&mut Node>,
    mut commands: Commands,
    mut built: Local<Option<(Entity, Vec<String>, String)>>,
) {
    let (Some(fonts), Ok(bar)) = (fonts, bars.single()) else {
        return;
    };
    let Ok(mut node) = nodes.get_mut(bar) else {
        return;
    };
    let mut ids = Vec::new();
    fixed.tree.collect_panels(&mut ids);
    // Nothing to collapse *to* only when the panel is already open.
    if bottom.open {
        if node.display != Display::None {
            node.display = Display::None;
        }
        return;
    }
    if node.display != Display::Flex {
        node.display = Display::Flex;
    }
    // What an empty panel labels itself with, and "" when it has tabs to show
    // instead. Part of the rebuild key so a rename of the empty set repaints,
    // without a rename repainting a strip that isn't showing the name.
    let empty_label = if ids.is_empty() {
        sets.sets
            .get(sets.active)
            .map(|(name, _)| name.clone())
            .unwrap_or_else(default_panel_set_name)
    } else {
        String::new()
    };
    // Keyed on the bar entity too: a theme/language chrome respawn creates a
    // fresh (childless) bar, which must rebuild even for the same tab set.
    if built.as_ref() == Some(&(bar, ids.clone(), empty_label.clone())) {
        return;
    }
    *built = Some((bar, ids.clone(), empty_label.clone()));

    commands.entity(bar).despawn_related::<Children>();
    if !empty_label.is_empty() {
        // Italic would be the usual "nothing here" cue, but the UI font has no
        // italic face — the muted colour and the em dash carry it instead.
        let empty = renzora::lang::t_or("shell.bottom_dock.empty_hint", "empty");
        let hint = commands
            .spawn((
                Text::new(format!("{empty_label} — {empty}")),
                ui_font(&fonts.ui, 12.0),
                TextColor(rgb(placeholder())),
                bevy::text::TextLayout::no_wrap(),
                Node {
                    margin: UiRect::horizontal(Val::Px(9.0)),
                    ..default()
                },
                Name::new("closed-bottom-empty"),
            ))
            .id();
        commands.entity(bar).add_child(hint);
    }
    for id in ids {
        let (title, icon) = registry
            .as_ref()
            .and_then(|r| r.panels.get(&id))
            .map(|info| {
                let icon = if info.icon.is_empty() {
                    "circle".to_string()
                } else {
                    info.icon.clone()
                };
                (info.title.clone(), icon)
            })
            .unwrap_or_else(|| (renzora_ember::dock::humanize(&id), "circle".to_string()));
        let tab = commands
            .spawn((
                Node {
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(5.0),
                    padding: UiRect::horizontal(Val::Px(9.0)),
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Interaction::default(),
                renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
                CollapsedBottomTab(id.clone()),
                Name::new(format!("closed-bottom-tab:{id}")),
            ))
            .id();
        let ic = icon_text(&mut commands, &fonts.phosphor, &icon, text_muted(), 13.0);
        let label = commands
            .spawn((
                Text::new(title),
                ui_font(&fonts.ui, 12.0),
                TextColor(rgb(text_muted())),
                bevy::text::TextLayout::no_wrap(),
            ))
            .id();
        commands.entity(tab).add_children(&[ic, label]);
        commands.entity(bar).add_child(tab);
    }

    // Right-aligned open chevron (mirrors the open panel's collapse chevron).
    //
    // The filler carries the resize cursor, not the bar. `apply_cursor_icon`
    // takes the first hovered entity with a `HoverCursor` and does no topmost
    // resolution, so a cursor on the bar competes with the tabs and the chevron
    // nested inside it — which is why hovering a closed-strip tab showed the
    // resize cursor. On the filler it can only be hovered over empty space.
    // The bar keeps its `Interaction` so `collapsed_bottom_bar_drag` still sees
    // the press anywhere along it.
    let strip_filler = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                height: Val::Percent(100.0),
                ..default()
            },
            Interaction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::NsResize),
            Name::new("closed-bottom-filler"),
        ))
        .id();
    let chev = icon_text(&mut commands, &fonts.phosphor, "caret-up", text_muted(), 13.0);
    let open_btn = commands
        .spawn((
            Node {
                height: Val::Percent(100.0),
                width: Val::Px(24.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            CollapsedBottomOpenBtn,
            Name::new("closed-bottom-open"),
        ))
        .id();
    commands.entity(open_btn).add_child(chev);
    commands.entity(bar).add_children(&[strip_filler, open_btn]);
}

// `position_collapsed_bottom_bar` lived here. It pulled the collapsed strip out
// of the chrome flow and sized it to the on-screen span of whichever column its
// stash was anchored under, so a strip that had been nested below the viewport
// collapsed in place rather than spanning the window.
//
// The global bottom panel has no anchor to align to — it is one full-width
// region below every workspace by construction — so the strip is simply the
// full-width chrome row it always was when unanchored, and the whole
// measure-the-leaves pass is dead weight.

/// Click a collapsed-strip tab → open the bottom panel with the clicked panel
/// as the active tab.
fn collapsed_bottom_tab_click(
    tabs: Query<(&Interaction, &CollapsedBottomTab), Changed<Interaction>>,
    wraps: Query<&ComputedNode, With<DockAreaWrap>>,
    mut bottom: ResMut<BottomDock>,
    mut fixed: ResMut<renzora_ember::dock::FixedDock>,
) {
    for (interaction, tab) in &tabs {
        if *interaction != Interaction::Pressed {
            continue;
        }
        bottom.open = true;
        // Open to the standard share of the dock region rather than the height
        // it last had. Clicking a *tab* is a request to look at that panel, and
        // the remembered height could be anything — including the near-minimum a
        // drag-to-close leaves behind, which would reopen to a sliver of the
        // panel the click was asking to see.
        if let Some(h) = default_open_height(&wraps) {
            bottom.height = h;
        }
        fixed.tree.set_active_tab(&tab.0);
        fixed.dirty = true;
        return;
    }
}

/// Click the collapsed strip's open chevron → open the bottom panel at its
/// remembered height.
fn collapsed_bottom_open_click(
    btns: Query<&Interaction, (With<CollapsedBottomOpenBtn>, Changed<Interaction>)>,
    mut bottom: ResMut<BottomDock>,
) {
    for interaction in &btns {
        if *interaction != Interaction::Pressed {
            continue;
        }
        bottom.open = true;
        return;
    }
}

/// Drag the collapsed strip's empty background upward → open the bottom panel
/// and continue as a live resize of its top edge, so opening and sizing are one
/// gesture. Tabs and the open chevron sit above the bar and capture their own
/// presses, so this only fires from the strip's own background.
///
/// This used to hand the held cursor to ember via `GrabRootDivider`, because
/// the panel's height *was* a split ratio inside the workspace tree and only a
/// dock divider could drive it. The panel is an overlay with its own height
/// now, so the shell drives the drag itself and ember never hears about it —
/// which is also what keeps the resize from touching the workspace layout.
fn collapsed_bottom_bar_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    bars: Query<&Interaction, With<CollapsedBottomBar>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut bottom: ResMut<BottomDock>,
    mut resize: ResMut<BottomDockResize>,
    mut press_y: Local<Option<f32>>,
) {
    if !mouse.pressed(MouseButton::Left) {
        *press_y = None;
        return;
    }
    let Some(cursor_y) = windows
        .single()
        .ok()
        .and_then(|w| w.cursor_position())
        .map(|c| c.y)
    else {
        return;
    };
    if mouse.just_pressed(MouseButton::Left) {
        if bars.iter().any(|i| matches!(i, Interaction::Pressed)) {
            *press_y = Some(cursor_y);
        }
        return;
    }
    let Some(start_y) = *press_y else { return };
    // A few px of upward travel arms the gesture (a plain click does nothing
    // — the tabs and the chevron own click-to-open).
    if start_y - cursor_y < 4.0 {
        return;
    }
    *press_y = None;
    // Open at the minimum and let the drag grow it from there, so the top edge
    // tracks the cursor from where the gesture started rather than jumping to
    // the remembered height and then following.
    bottom.open = true;
    // No slide: the panel's top edge is being held by the cursor for the rest
    // of this gesture, and an animation would put it somewhere else while the
    // drag says it is here. Direct manipulation is its own transition.
    bottom.slide = 1.0;
    bottom.height = dock::BOTTOM_DOCK_MIN_HEIGHT;
    resize.active = Some((cursor_y, dock::BOTTOM_DOCK_MIN_HEIGHT));
}

/// Collapsed-strip tabs highlight on hover (they're otherwise muted —
/// reading as closed, not active).
fn collapsed_bottom_tab_hover(
    mut tabs: Query<
        (&Interaction, &mut BackgroundColor),
        Or<(With<CollapsedBottomTab>, With<CollapsedBottomOpenBtn>)>,
    >,
) {
    for (interaction, mut bg) in &mut tabs {
        let want = if matches!(interaction, Interaction::Hovered | Interaction::Pressed) {
            rgb(tab_active())
        } else {
            Color::NONE
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

/// A ribbon workspace button (Scene, Blueprints, …). Carries its layout index;
/// the active highlight comes from the reactive rebuild (see `ribbon_snapshot`).
#[derive(Component)]
struct RibbonItem {
    index: usize,
    /// Vertical insertion marker shown at this tab's left/right edge during a
    /// reorder drag (mirrors the dock tab-drag preview). Toggled in
    /// [`ribbon_interact`].
    marker: Entity,
}

/// The ribbon's "+" — adds a new empty workspace.
#[derive(Component)]
struct WorkspaceAddBtn;

/// The top bar's "Update available" chip. Shown only while
/// [`renzora::core::UpdateAvailable`] is present; opens the Software Update
/// overlay.
#[derive(Component)]
struct UpdateChipBtn;

/// Tags the ribbon strip + its `+` as a drop target for dock-tab drags: dropping
/// a dragged panel here spawns a new workspace from it (see [`workspace_drop_to_new`]).
#[derive(Component)]
struct WorkspaceDropZone;

/// The top-bar magnifier — toggles the command palette.
#[derive(Component)]
struct CommandPaletteBtn;

/// The top-bar gear — toggles the Settings panel.
#[derive(Component)]
struct SettingsBtn;

/// In-progress ribbon drag (press-latch → reorder on release). `active` flips
/// once the cursor moves past a small threshold so a plain click still switches.
#[derive(Resource, Default)]
struct RibbonDrag(Option<RibbonDragState>);

struct RibbonDragState {
    from: usize,
    start_cursor: Vec2,
    active: bool,
    /// Insertion slot (0..=len) under the live cursor; applied on release.
    target: usize,
}

/// The workspace currently being inline-renamed (`None` = none). Read by
/// `ribbon_snapshot` so that tab renders an edit field in place of its label.
#[derive(Resource, Default)]
struct RibbonRename(Option<usize>);

/// Marks the inline rename text field, carrying the workspace index it renames.
#[derive(Component)]
struct RibbonRenameInput(usize);

/// In-progress document-tab reorder — the same press-latch shape as
/// [`RibbonDrag`], so a plain click still activates the tab.
#[derive(Resource, Default)]
struct DocTabDrag(Option<DocTabDragState>);

struct DocTabDragState {
    /// The tab being carried, by **id**: a reorder shifts every index around it,
    /// and a tab can be closed from elsewhere mid-drag.
    id: u64,
    start_cursor: Vec2,
    /// Flips once the cursor has moved past a small threshold, so a click that
    /// happens to wobble a pixel doesn't reorder anything. A drag started from
    /// the overflow menu is born active — there was no click to tell it apart
    /// from in the first place.
    active: bool,
    /// Insertion slot in `DocumentTabState::tabs` (`0..=len`) under the live
    /// cursor; applied on release.
    target: usize,
}

/// The document tab currently being inline-renamed (`None` = none). Read by
/// [`doc_tab_snapshot`] so that tab renders an edit field in place of itself.
#[derive(Resource, Default)]
struct DocTabRename(Option<u64>);

/// Marks the inline rename text field, carrying the tab id it renames.
#[derive(Component)]
struct DocTabRenameInput(u64);

/// Marks the shell's root UI entity so it can be despawned when the backend
/// switches back to egui.
#[derive(Component)]
struct ShellRoot;

/// Marks the status-bar theme picker's trigger so its open/closed state can be
/// mirrored into [`ThemeMenuOpen`] each frame.
#[derive(Component)]
struct ThemeDropup;

/// Whether the status-bar theme dropup is open, persisted *across* chrome
/// rebuilds. Picking a theme switches `active_theme_name`, which makes
/// `theme_bridge` despawn and respawn the whole chrome — without this the rebuilt
/// dropup would always come back closed, so the menu would vanish the instant you
/// clicked a theme inside it. Holding the open state here lets the rebuilt dropup
/// re-open, so the menu only closes on a real outside click (or toggling the
/// trigger).
#[derive(Resource, Default)]
struct ThemeMenuOpen(bool);

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

/// Chrome metadata (id, title, icon, category) for every editor panel, seeded
/// into [`renzora::ShellPanelRegistry`] at startup. Without this the dock tabs
/// fall back to ember's generic circle glyph and the Add-Panel `+` picker is
/// empty (nothing else populates the registry). Icons are kebab-case Phosphor
/// names, resolved to glyphs via `renzora_ember::font::icon_glyph`.
const PANEL_META: &[(&str, &str, &str, &str)] = &[
    // Scene / editing
    ("hierarchy", "Hierarchy", "tree-structure", "Scene"),
    ("inspector", "Inspector", "sliders-horizontal", "Editing"),
    ("scenes", "Scenes", "film-slate", "Scene"),
    ("scene_diagnostics", "Scene Diagnostics", "first-aid", "Debug"),
    ("streaming_debug", "Streaming", "broadcast", "Debug"),
    ("assets", "Assets", "folder-open", "Assets"),
    ("shape_library", "Shapes", "shapes", "Assets"),
    ("level_presets", "Level Presets", "globe", "Scene"),
    ("history", "History", "clock-counter-clockwise", "Editing"),
    ("code_editor", "Code", "code", "Editing"),
    ("console", "Console", "terminal", "Debug"),
    ("problems", "Problems", "warning-circle", "Debug"),
    // Viewports
    ("viewport", "Viewport", "perspective", "Scene"),
    ("viewport-2", "Viewport 2", "perspective", "Scene"),
    ("viewport-3", "Viewport 3", "perspective", "Scene"),
    ("viewport-4", "Viewport 4", "perspective", "Scene"),
    ("camera_preview", "Camera Preview", "video-camera", "Scene"),
    // Audio
    ("mixer", "Mixer", "faders", "Audio"),
    // Animation
    ("timeline", "Timeline", "film-strip", "Animation"),
    ("animation", "Animation", "play-circle", "Animation"),
    ("studio_preview", "Studio Preview", "person", "Animation"),
    ("animator_state_machine", "State Machine", "graph", "Animation"),
    ("animator_params", "Parameters", "sliders-horizontal", "Animation"),
    // Material
    ("material_preview", "Material Preview", "sphere", "Material"),
    ("material_inspector", "Material", "palette", "Material"),
    ("material_graph", "Material Graph", "graph", "Material"),
    // Particle
    ("particle_preview", "Particle Preview", "sparkle", "Particle"),
    ("particle_editor", "Particle Editor", "sparkle", "Particle"),
    ("particle_graph", "Particle Graph", "graph", "Particle"),
    // Shader
    ("shader_properties", "Shader", "graphics-card", "Shader"),
    ("shader_preview", "Shader Preview", "image", "Shader"),
    ("shader_compiler_log", "Compiler Log", "list-dashes", "Shader"),
    // Blueprint
    ("blueprint_graph", "Blueprint", "blueprint", "Blueprint"),
    ("blueprint_properties", "Blueprint Properties", "sliders-horizontal", "Blueprint"),
    // The Marketplace panels (store, library, publish) and the wallet are NOT
    // listed here. They belong to the `marketplace` plugin, which registers its
    // own shell metadata the way any plugin does — so an install without it
    // shows no Marketplace category at all, rather than three panels that open
    // empty.
    // Network
    ("network_monitor", "Network", "broadcast", "Network"),
    ("network_entities", "Net Entities", "users-three", "Network"),
    ("network_settings", "Net Settings", "gear", "Network"),
    // Terrain / foliage / navigation
    ("terrain_tools", "Terrain", "mountains", "Terrain"),
    ("foliage_painting", "Foliage", "tree", "Terrain"),
    ("navmesh", "Navmesh", "path", "Navigation"),
    // Input
    ("gamepad", "Gamepad", "game-controller", "Input"),
    // Debug / profiling
    ("performance", "Performance", "gauge", "Debug"),
    ("render_stats", "Render Stats", "chart-bar", "Debug"),
    ("ecs_stats", "ECS Stats", "list-numbers", "Debug"),
    ("memory_profiler", "Memory", "memory", "Debug"),
    ("system_profiler", "System", "cpu", "Debug"),
    ("physics_debug", "Physics Debug", "atom", "Debug"),
    ("camera_debug", "Camera Debug", "video-camera", "Debug"),
    ("culling_debug", "Culling", "scissors", "Debug"),
    ("material_resolver_diag", "Material Diag", "palette", "Debug"),
    ("lumen_diag", "Lumen Diag", "lightbulb", "Debug"),
    ("scripting_diag", "Scripting Diag", "bug", "Debug"),
    ("ui_reactivity", "UI Reactivity", "lightning", "Debug"),
    ("ui_layout", "UI Layout", "layout", "Debug"),
    ("resources", "Resources", "database", "Debug"),
    // Plugins
    ("plugin_resources", "Plugin Resources", "puzzle-piece", "Tools"),
];

/// Seed [`renzora::ShellPanelRegistry`] from [`PANEL_META`] (as defaults — a
/// plugin that already called `register_shell_panel` for an id wins).
fn seed_panel_meta(app: &mut App) {
    use renzora::ShellPanelInfo;
    let mut reg = app.world_mut().resource_mut::<renzora::ShellPanelRegistry>();
    for &(id, title, icon, category) in PANEL_META {
        reg.panels.entry(id.to_string()).or_insert_with(|| ShellPanelInfo {
            title: title.to_string(),
            icon: icon.to_string(),
            category: category.to_string(),
        });
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
            // Localize the tab title via `panel.<id>`, falling back to the
            // registry's English title. Compared against the *localized* value so
            // it doesn't thrash, and re-localizes live (this system runs each frame).
            let title = renzora::lang::t_or(&format!("panel.{}", tab.id), &info.title);
            if let Ok(mut t) = texts.get_mut(tab.label) {
                if t.0 != title {
                    t.0 = title;
                }
            }
        }
        if !info.icon.is_empty() {
            // `info.icon` is a kebab-case Phosphor NAME; resolve it to the glyph
            // the tab's phosphor-font Text expects (mirrors the status bar).
            let glyph = renzora_ember::font::icon_glyph(&info.icon)
                .unwrap_or('\u{E4C6}')
                .to_string();
            if let Ok(mut t) = texts.get_mut(tab.icon) {
                if t.0 != glyph {
                    t.0 = glyph;
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
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    rename: Res<RibbonRename>,
    pressed: Query<(&RibbonItem, &Interaction)>,
    items: Query<(&RibbonItem, &RelativeCursorPosition)>,
    mut nodes: Query<&mut Node>,
    mut layouts: ResMut<ShellLayouts>,
    mut dock: ResMut<Dock>,
    mut dirty: ResMut<DockDirty>,
) {
    // Hide every insertion marker (no drag live, or before re-showing one slot).
    let hide_markers = |items: &Query<(&RibbonItem, &RelativeCursorPosition)>, nodes: &mut Query<&mut Node>| {
        for (it, _) in items {
            if let Ok(mut n) = nodes.get_mut(it.marker) {
                if n.display != Display::None {
                    n.display = Display::None;
                }
            }
        }
    };

    // Don't drag/switch while a tab is being renamed.
    if rename.0.is_some() {
        drag.0 = None;
        hide_markers(&items, &mut nodes);
        return;
    }
    let cursor = windows.iter().next().and_then(|w| w.cursor_position());

    if drag.0.is_none() && mouse.just_pressed(MouseButton::Left) {
        if let Some(cur) = cursor {
            for (item, interaction) in &pressed {
                if *interaction == Interaction::Pressed {
                    drag.0 = Some(RibbonDragState { from: item.index, start_cursor: cur, active: false, target: item.index });
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

    // While actively dragging, track the insertion slot under the cursor and show
    // the matching edge marker. Using each tab's RelativeCursorPosition (not a
    // GlobalTransform center compared against the cursor, which drifts under UI
    // scaling) keeps the hit-test in the cursor's own space — fixing both the
    // missing divider and the wrong drop position.
    if let Some(state) = drag.0.as_mut() {
        if state.active {
            // (marker, right-edge): cursor in a tab's left half inserts before it,
            // right half after it.
            let mut shown: Option<(Entity, bool)> = None;
            for (it, rcp) in &items {
                if rcp.cursor_over {
                    let before = rcp.normalized.is_none_or(|n| n.x < 0.0);
                    state.target = if before { it.index } else { it.index + 1 };
                    shown = Some((it.marker, !before));
                    break;
                }
            }
            hide_markers(&items, &mut nodes);
            if let Some((marker, right)) = shown {
                if let Ok(mut n) = nodes.get_mut(marker) {
                    n.display = Display::Flex;
                    if right {
                        n.left = Val::Auto;
                        n.right = Val::Px(-2.0);
                    } else {
                        n.left = Val::Px(-2.0);
                        n.right = Val::Auto;
                    }
                }
            }
        }
    } else {
        hide_markers(&items, &mut nodes);
    }

    if mouse.just_released(MouseButton::Left) {
        hide_markers(&items, &mut nodes);
        if let Some(state) = drag.0.take() {
            if !state.active {
                apply_workspace(state.from, &mut layouts, &mut dock, &mut dirty);
            } else {
                let from = state.from;
                let target = state.target.min(layouts.layouts.len());
                // Removing `from` first shifts later slots left by one.
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
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
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
    // The bottom panel is no longer part of any workspace, so removing one
    // leaves it alone — there is nothing keyed by `removed_name` to clean up.
    // This used to drop that workspace's stash, which also meant deleting a
    // workspace silently deleted whatever panels were sitting in its closed
    // bottom strip.
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
        // Renaming used to have to re-key the bottom-panel stash, which was
        // keyed by workspace name and held the only copy of its panels — miss
        // the re-key and the rename orphaned them. The bottom panel is global
        // now and knows nothing about workspace names, so a rename is just a
        // rename.
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

// ── Window controls (borderless chrome) ──────────────────────────────────────

use bevy::window::SystemCursorIcon;
use renzora_ui::window_chrome::{WindowAction, WindowActionQueue};

/// A window-control button (minimize / maximize / close).
#[derive(Component)]
struct WindowBtn(WindowAction);

/// The web's stand-in for the window controls: a fullscreen toggle.
///
/// Browser fullscreen is the only "window" state a page can actually change,
/// and it is the one worth having — it takes the tab strip and address bar away
/// and gives the editor the whole display, which is much closer to how the
/// desktop build is used.
#[cfg(target_arch = "wasm32")]
#[derive(Component)]
struct WebFullscreenBtn;

/// Toggle browser fullscreen, and report whether the page is now fullscreen.
///
/// Must run from a click: browsers only grant `requestFullscreen` in response
/// to a user gesture, and refuse it silently otherwise.
#[cfg(target_arch = "wasm32")]
fn toggle_web_fullscreen() {
    let Some(doc) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    if doc.fullscreen_element().is_some() {
        doc.exit_fullscreen();
    } else if let Some(el) = doc.document_element() {
        // Fullscreen the whole page rather than the canvas: the canvas is sized
        // from its parent (`fit_canvas_to_parent`), so promoting the root keeps
        // that relationship and lets Bevy resize into the new viewport on its
        // own.
        let _ = el.request_fullscreen();
    }
}

#[cfg(target_arch = "wasm32")]
fn web_fullscreen_click(q: Query<&Interaction, (With<WebFullscreenBtn>, Changed<Interaction>)>) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        toggle_web_fullscreen();
    }
}

/// Swap the glyph between "expand" and "collapse" as fullscreen changes.
///
/// Polled rather than driven by the `fullscreenchange` event: the state can
/// also change by Esc or F11, which no click of ours would hear about, and a
/// cheap per-frame read of `document.fullscreenElement` covers every route.
#[cfg(target_arch = "wasm32")]
fn sync_web_fullscreen_icon(q: Query<&Children, With<WebFullscreenBtn>>, mut text: Query<&mut Text>) {
    let is_fs = web_sys::window()
        .and_then(|w| w.document())
        .is_some_and(|d| d.fullscreen_element().is_some());
    let Some(want) =
        renzora_ember::phosphor_map::icon_glyph(if is_fs { "corners-in" } else { "corners-out" })
    else {
        return;
    };
    let want = want.to_string();
    for children in &q {
        for child in children.iter() {
            if let Ok(mut t) = text.get_mut(child) {
                if t.0 != want {
                    t.0 = want.clone();
                }
            }
        }
    }
}

/// An empty top-bar region that initiates an OS window-move on press (and, when
/// maximized, restores first — Windows aero-snap then handles half/maximize).
#[derive(Component)]
struct WindowDragHandle;

/// A perimeter hit zone that initiates an OS edge/corner resize on press.
#[derive(Component)]
struct WindowResizeZone(bevy::math::CompassOctant);

/// The maximize button's icon — swapped between maximize/restore glyphs.
#[derive(Component)]
struct MaximizeIcon;

/// Keep the maximize button's glyph in sync with the window's maximized state.
fn update_maximize_icon(
    queue: Option<Res<WindowActionQueue>>,
    mut q: Query<&mut renzora_ember::icons::Icon, With<MaximizeIcon>>,
) {
    let maximized = queue.is_some_and(|q| q.maximized);
    let want = if maximized { "arrows-in-simple" } else { "square" };
    for mut icon in &mut q {
        if icon.name != want {
            icon.name = want.to_string();
            icon.resolved = false; // force `apply_icons` to re-render the glyph
        }
    }
}

fn window_btn_click(
    q: Query<(&Interaction, &WindowBtn), Changed<Interaction>>,
    queue: Option<ResMut<WindowActionQueue>>,
    mut commands: Commands,
) {
    let Some(mut queue) = queue else { return };
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        // Close is routed through the exit flow (which may prompt to save
        // unsaved changes first); everything else applies immediately.
        if matches!(btn.0, WindowAction::Close) {
            commands.insert_resource(ExitRequest);
        } else {
            queue.push(btn.0);
        }
    }
}

// ── Save-before-exit flow ────────────────────────────────────────────────────

/// Set when the user asks to close the window (the × button). Consumed by
/// [`process_exit_request`], which either exits straight away or — if any
/// document has unsaved changes — opens the [`ExitPromptRoot`] overlay.
#[derive(Resource)]
struct ExitRequest;

/// Set while we've asked the scene-save system to run and are waiting for it to
/// finish before exiting (see [`pending_exit_after_save`]).
#[derive(Resource)]
struct PendingExitAfterSave;

/// The backdrop root of the "unsaved changes" overlay.
#[derive(Component)]
struct ExitPromptRoot;

/// The overlay's three actions.
#[derive(Component)]
struct ExitPromptSave;
#[derive(Component)]
struct ExitPromptDiscard;
#[derive(Component)]
struct ExitPromptCancel;

/// Are there any documents with unsaved edits?
fn any_unsaved(tabs: &renzora_ui::DocumentTabState) -> bool {
    tabs.tabs.iter().any(|t| t.is_modified)
}

/// Handle a pending [`ExitRequest`]: exit immediately when nothing is dirty,
/// otherwise open the save-confirmation overlay.
fn process_exit_request(
    req: Option<Res<ExitRequest>>,
    tabs: Option<Res<renzora_ui::DocumentTabState>>,
    fonts: Option<Res<EmberFonts>>,
    mut exit: MessageWriter<AppExit>,
    open: Query<(), With<ExitPromptRoot>>,
    mut commands: Commands,
) {
    if req.is_none() {
        return;
    }
    commands.remove_resource::<ExitRequest>();
    // A prompt is already up — ignore repeat clicks.
    if !open.is_empty() {
        return;
    }

    let dirty = tabs.as_ref().is_some_and(|t| any_unsaved(t));
    // Nothing unsaved (or we can't render the prompt) → exit straight away.
    // Write `AppExit` here in `Update` (not via the `WindowAction::Close` queue,
    // which `apply_window_actions` drains in `Last`) so the exit event already
    // exists by the time the `Last`-scheduled `kill_on_app_exit` reads it —
    // otherwise the two `Last` systems race and the fast-exit is missed,
    // falling back to the slow World teardown.
    if !dirty || fonts.is_none() {
        exit.write(AppExit::Success);
        return;
    }
    let fonts = fonts.unwrap();
    let count = tabs
        .map(|t| t.tabs.iter().filter(|x| x.is_modified).count())
        .unwrap_or(0);
    spawn_exit_prompt(&mut commands, &fonts, count);
}

/// Build the centered "unsaved changes" confirmation overlay.
fn spawn_exit_prompt(commands: &mut Commands, fonts: &EmberFonts, count: usize) {
    let (root, content) =
        renzora_ember::widgets::overlay_sized(commands, fonts, "Unsaved Changes", 440.0, 188.0, true);
    commands.entity(root).insert(ExitPromptRoot);

    let body = if count == 1 {
        "You have unsaved changes. Save before closing?".to_string()
    } else {
        format!("You have unsaved changes in {count} documents. Save before closing?")
    };

    // Pad the content and lay out the message above a right-aligned button row.
    commands.entity(content).insert(Node {
        width: Val::Percent(100.0),
        flex_grow: 1.0,
        min_height: Val::Px(0.0),
        flex_direction: FlexDirection::Column,
        justify_content: JustifyContent::SpaceBetween,
        padding: UiRect::all(Val::Px(16.0)),
        ..default()
    });

    let message = commands
        .spawn((
            Text::new(body),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(text_muted())),
        ))
        .id();

    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::FlexEnd,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();

    let cancel = renzora_ember::widgets::button(commands, &fonts.ui, "Cancel");
    commands.entity(cancel).insert(ExitPromptCancel);
    let discard = renzora_ember::widgets::button(commands, &fonts.ui, "Don't Save");
    commands.entity(discard).insert(ExitPromptDiscard);
    let save = renzora_ember::widgets::button(commands, &fonts.ui, "Save & Close");
    // Tag it as the accent (primary) action so `apply_theme` paints it the
    // highlight color instead of the plain button color.
    commands.entity(save).insert((
        ExitPromptSave,
        renzora_ember::style::Styled::new(renzora_ember::style::Role::ButtonAccent),
    ));

    commands.entity(row).add_children(&[cancel, discard, save]);
    commands.entity(content).add_children(&[message, row]);
}

/// Drive the overlay's buttons. (Escape / backdrop click / the title × are
/// handled by ember's generic `overlay_dismiss`, which despawns the root — i.e.
/// the same as Cancel.)
fn exit_prompt_buttons(
    save: Query<&Interaction, (Changed<Interaction>, With<ExitPromptSave>)>,
    discard: Query<&Interaction, (Changed<Interaction>, With<ExitPromptDiscard>)>,
    cancel: Query<&Interaction, (Changed<Interaction>, With<ExitPromptCancel>)>,
    roots: Query<Entity, With<ExitPromptRoot>>,
    mut exit: MessageWriter<AppExit>,
    mut commands: Commands,
) {
    let save = save.iter().any(|i| *i == Interaction::Pressed);
    let discard = discard.iter().any(|i| *i == Interaction::Pressed);
    let cancel = cancel.iter().any(|i| *i == Interaction::Pressed);

    if !(save || discard || cancel) {
        return;
    }

    // Either way the prompt goes away.
    for r in &roots {
        commands.entity(r).despawn();
    }

    if save {
        // Run the same Save the title bar uses, then exit once it lands.
        commands.insert_resource(renzora::core::SaveSceneRequested);
        commands.insert_resource(PendingExitAfterSave);
    } else if discard {
        exit.write(AppExit::Success);
    }
    // cancel → nothing else; the close is abandoned.
}

/// After "Save & Close", wait for the scene-save to complete, then exit. If the
/// save was redirected to a Save-As dialog the user cancelled (changes remain
/// unsaved), abort the exit instead of losing work.
fn pending_exit_after_save(
    pending: Option<Res<PendingExitAfterSave>>,
    save_req: Option<Res<renzora::core::SaveSceneRequested>>,
    save_as_req: Option<Res<renzora::core::SaveAsSceneRequested>>,
    tabs: Option<Res<renzora_ui::DocumentTabState>>,
    mut exit: MessageWriter<AppExit>,
    mut commands: Commands,
) {
    if pending.is_none() {
        return;
    }
    // Still saving (or prompting for a path) — keep waiting.
    if save_req.is_some() || save_as_req.is_some() {
        return;
    }
    commands.remove_resource::<PendingExitAfterSave>();

    let still_dirty = tabs.is_some_and(|t| any_unsaved(&t));
    if !still_dirty {
        exit.write(AppExit::Success);
    }
    // else: save failed or Save-As was cancelled → stay open, don't lose work.
}

// ── Close-tab save prompt ─────────────────────────────────────────────────────

/// Set by [`doc_tab_close`] when the × is clicked on a tab with unsaved changes.
/// Consumed by [`process_tab_close_request`], which foregrounds the tab and
/// opens the save-confirmation prompt.
#[derive(Resource)]
struct TabCloseRequest {
    id: u64,
}

/// Set after "Save & Close" while we wait for the scene-save to land before
/// closing the tab (see [`pending_close_after_save`]). Carries the tab id.
#[derive(Resource)]
struct PendingCloseAfterSave {
    id: u64,
}

/// Backdrop root of the "unsaved changes" prompt for a single tab. Stores the
/// id of the tab whose close is pending so the buttons know what to act on.
#[derive(Component)]
struct CloseTabPromptRoot(u64);

/// The prompt's three actions.
#[derive(Component)]
struct CloseTabPromptSave;
#[derive(Component)]
struct CloseTabPromptDiscard;
#[derive(Component)]
struct CloseTabPromptCancel;

/// Handle a pending [`TabCloseRequest`]: foreground the target tab (so what the
/// user decides about is what they see, and so a subsequent Save targets this
/// tab's live scene) and open the save-confirmation prompt. If the tab turned
/// out clean in the meantime, just close it.
fn process_tab_close_request(
    req: Option<Res<TabCloseRequest>>,
    state: Option<ResMut<renzora_ui::DocumentTabState>>,
    fonts: Option<Res<EmberFonts>>,
    open: Query<(), With<CloseTabPromptRoot>>,
    mut commands: Commands,
) {
    let Some(req) = req else { return };
    // A prompt is already up — leave the request until it's resolved.
    if !open.is_empty() {
        return;
    }
    let id = req.id;
    commands.remove_resource::<TabCloseRequest>();

    let (Some(mut state), Some(fonts)) = (state, fonts) else { return };
    let Some(idx) = state.tabs.iter().position(|t| t.id == id) else { return };
    // Not dirty anymore (saved elsewhere since the click) → close outright.
    if !state.tabs[idx].is_modified {
        close_doc_tab_by_id(&mut state, id, &mut commands);
        return;
    }
    let name = state.tabs[idx].name.clone();
    // Bring the tab forward if it's in the background.
    if state.active_tab != idx {
        if let Some((old_id, new_id)) = state.activate_tab(idx) {
            commands.insert_resource(renzora::TabSwitchRequest {
                old_tab_id: old_id,
                new_tab_id: new_id,
            });
        }
    }
    spawn_close_tab_prompt(&mut commands, &fonts, id, &name);
}

/// Build the centered "unsaved changes" prompt for closing a single tab.
fn spawn_close_tab_prompt(commands: &mut Commands, fonts: &EmberFonts, id: u64, name: &str) {
    let (root, content) =
        renzora_ember::widgets::overlay_sized(commands, fonts, "Unsaved Changes", 440.0, 188.0, true);
    commands.entity(root).insert(CloseTabPromptRoot(id));

    let body = format!("\"{name}\" has unsaved changes. Save before closing?");

    // Pad the content and lay out the message above a right-aligned button row.
    commands.entity(content).insert(Node {
        width: Val::Percent(100.0),
        flex_grow: 1.0,
        min_height: Val::Px(0.0),
        flex_direction: FlexDirection::Column,
        justify_content: JustifyContent::SpaceBetween,
        padding: UiRect::all(Val::Px(16.0)),
        ..default()
    });

    let message = commands
        .spawn((
            Text::new(body),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(text_muted())),
        ))
        .id();

    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::FlexEnd,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();

    let cancel = renzora_ember::widgets::button(commands, &fonts.ui, "Cancel");
    commands.entity(cancel).insert(CloseTabPromptCancel);
    let discard = renzora_ember::widgets::button(commands, &fonts.ui, "Don't Save");
    commands.entity(discard).insert(CloseTabPromptDiscard);
    let save = renzora_ember::widgets::button(commands, &fonts.ui, "Save & Close");
    commands.entity(save).insert((
        CloseTabPromptSave,
        renzora_ember::style::Styled::new(renzora_ember::style::Role::ButtonAccent),
    ));

    commands.entity(row).add_children(&[cancel, discard, save]);
    commands.entity(content).add_children(&[message, row]);
}

/// Drive the close prompt's buttons. (Escape / backdrop click / the title × are
/// handled by ember's generic `overlay_dismiss`, which despawns the root — same
/// as Cancel: the tab stays open.)
fn close_tab_prompt_buttons(
    save: Query<&Interaction, (Changed<Interaction>, With<CloseTabPromptSave>)>,
    discard: Query<&Interaction, (Changed<Interaction>, With<CloseTabPromptDiscard>)>,
    cancel: Query<&Interaction, (Changed<Interaction>, With<CloseTabPromptCancel>)>,
    roots: Query<(Entity, &CloseTabPromptRoot)>,
    state: Option<ResMut<renzora_ui::DocumentTabState>>,
    mut commands: Commands,
) {
    let save = save.iter().any(|i| *i == Interaction::Pressed);
    let discard = discard.iter().any(|i| *i == Interaction::Pressed);
    let cancel = cancel.iter().any(|i| *i == Interaction::Pressed);

    if !(save || discard || cancel) {
        return;
    }

    // The target tab id lives on the root; capture it before despawning.
    let target = roots.iter().next().map(|(_, r)| r.0);
    for (e, _) in &roots {
        commands.entity(e).despawn();
    }
    let Some(id) = target else { return };

    if save {
        // Save the now-foregrounded tab, then close it once the save lands.
        commands.insert_resource(renzora::core::SaveSceneRequested);
        commands.insert_resource(PendingCloseAfterSave { id });
    } else if discard {
        if let Some(mut state) = state {
            close_doc_tab_by_id(&mut state, id, &mut commands);
        }
    }
    // cancel → nothing; the close is abandoned.
}

/// After "Save & Close", wait for the scene-save to complete, then close the
/// tab. If the save was redirected to a Save-As dialog the user cancelled (the
/// tab is still dirty), abort the close instead of losing work.
fn pending_close_after_save(
    pending: Option<Res<PendingCloseAfterSave>>,
    save_req: Option<Res<renzora::core::SaveSceneRequested>>,
    save_as_req: Option<Res<renzora::core::SaveAsSceneRequested>>,
    state: Option<ResMut<renzora_ui::DocumentTabState>>,
    mut commands: Commands,
) {
    let Some(pending) = pending else { return };
    // Still saving (or prompting for a path) — keep waiting.
    if save_req.is_some() || save_as_req.is_some() {
        return;
    }
    let id = pending.id;
    commands.remove_resource::<PendingCloseAfterSave>();

    let Some(mut state) = state else { return };
    let Some(idx) = state.tabs.iter().position(|t| t.id == id) else { return };
    // Clean now → the save succeeded; close it. Still dirty → Save-As was
    // cancelled, so keep the tab open and don't lose the edits.
    if !state.tabs[idx].is_modified {
        close_doc_tab_by_id(&mut state, id, &mut commands);
    }
}

/// Click-timing for the drag handle: distinguishes a single press (window move)
/// from a double-click (toggle maximize).
#[derive(Default)]
struct DragClickState {
    last: f32,
    /// Whether the previous press restored a maximized window (so a double-click
    /// on a maximized window restores rather than re-maximizing).
    restored_on_press: bool,
}

/// Press an empty top-bar area → start an OS window-move; double-click → toggle
/// maximize/restore (the OS then handles aero-snap when you drag to an edge).
fn window_drag(
    bar: Query<&Interaction, (With<WindowDragHandle>, Changed<Interaction>)>,
    others: Query<&Interaction, Without<WindowDragHandle>>,
    queue: Option<ResMut<WindowActionQueue>>,
    time: Res<Time>,
    mut state: Local<DragClickState>,
) {
    let Some(mut queue) = queue else { return };
    if !bar.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    // If any other widget is hovered/pressed, the press landed on a menu/button —
    // not the empty bar — so don't drag (belt-and-braces over focus blocking).
    if others.iter().any(|i| *i != Interaction::None) {
        return;
    }
    let now = time.elapsed_secs();
    if now - state.last < 0.4 {
        // Double-click. If the first press already restored a maximized window
        // (via StartDrag), don't re-maximize — leave it restored.
        state.last = 0.0;
        if !state.restored_on_press {
            queue.push(WindowAction::ToggleMaximize);
        }
    } else {
        state.last = now;
        state.restored_on_press = queue.maximized;
        queue.push(WindowAction::StartDrag);
    }
}

fn window_resize_start(
    q: Query<(&Interaction, &WindowResizeZone), Changed<Interaction>>,
    queue: Option<ResMut<WindowActionQueue>>,
) {
    let Some(mut queue) = queue else { return };
    for (interaction, zone) in &q {
        if *interaction == Interaction::Pressed {
            queue.push(WindowAction::StartResize(zone.0));
        }
    }
}

/// Build the 8 invisible edge/corner resize zones overlaid on the window border.
/// Returns them so the caller parents them under the shell root.
fn build_resize_zones(commands: &mut Commands) -> Vec<Entity> {
    use bevy::math::CompassOctant as O;
    const T: f32 = 5.0; // edge thickness
    const C: f32 = 12.0; // corner size
    let px = Val::Px;
    // (octant, cursor, node)
    // The top edge is the title bar (drag area) — only the corners resize there,
    // so dragging the bar doesn't clash with a top-edge resize.
    let zones: [(O, SystemCursorIcon, Node); 7] = [
        (O::South, SystemCursorIcon::SResize, Node { position_type: PositionType::Absolute, bottom: px(0.0), left: px(C), right: px(C), height: px(T), ..default() }),
        (O::West, SystemCursorIcon::WResize, Node { position_type: PositionType::Absolute, left: px(0.0), top: px(C), bottom: px(C), width: px(T), ..default() }),
        (O::East, SystemCursorIcon::EResize, Node { position_type: PositionType::Absolute, right: px(0.0), top: px(C), bottom: px(C), width: px(T), ..default() }),
        (O::NorthWest, SystemCursorIcon::NwResize, Node { position_type: PositionType::Absolute, top: px(0.0), left: px(0.0), width: px(C), height: px(C), ..default() }),
        (O::NorthEast, SystemCursorIcon::NeResize, Node { position_type: PositionType::Absolute, top: px(0.0), right: px(0.0), width: px(C), height: px(C), ..default() }),
        (O::SouthWest, SystemCursorIcon::SwResize, Node { position_type: PositionType::Absolute, bottom: px(0.0), left: px(0.0), width: px(C), height: px(C), ..default() }),
        (O::SouthEast, SystemCursorIcon::SeResize, Node { position_type: PositionType::Absolute, bottom: px(0.0), right: px(0.0), width: px(C), height: px(C), ..default() }),
    ];
    zones
        .into_iter()
        .map(|(octant, cursor, node)| {
            let id = commands
                .spawn((
                    node,
                    BackgroundColor(Color::NONE),
                    GlobalZIndex(60),
                    Interaction::default(),
                    // Overlaid on the window perimeter, so it covers the edge of
                    // whatever panel is docked against it: the press is this
                    // zone's alone, and panels can see the gesture is in flight
                    // rather than reading it as a press on their content.
                    renzora_ember::resize::ResizeHandle,
                    WindowResizeZone(octant),
                    renzora_ember::cursor_icon::HoverCursor(cursor),
                    Name::new("resize-zone"),
                ))
                .id();
            // Resizing makes no sense while maximized — hide the grips then.
            renzora_ember::reactive::tracked::bind_display(commands, id, |w| {
                !w.get_resource::<WindowActionQueue>().map(|q| q.maximized).unwrap_or(false)
            });
            id
        })
        .collect()
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

/// Drag any dock tab onto the workspace ribbon (the strip or its `+`) and drop it
/// to spawn a NEW workspace containing only that panel — the panel is *moved* out
/// of the workspace it came from.
///
/// The ember dock publishes the in-flight drag through [`renzora_ember::dock::DockDragWatch`]:
/// `dragging` is the panel id, and setting `claim` tells the dock to leave the
/// drop to us (so it neither re-docks nor tab-switches). We claim while the cursor
/// is over the ribbon, then build the workspace on release.
///
/// The release is handled from `Local` state captured on earlier frames because
/// the dock clears its own watch on release and may run before us that frame — so
/// `watch.dragging` can already be `None` by the time we see the mouse-up.
fn workspace_drop_to_new(
    watch: Option<ResMut<renzora_ember::dock::DockDragWatch>>,
    mouse: Res<ButtonInput<MouseButton>>,
    zones: Query<&RelativeCursorPosition, With<WorkspaceDropZone>>,
    mut add_bg: Query<&mut BackgroundColor, With<WorkspaceAddBtn>>,
    mut layouts: ResMut<ShellLayouts>,
    mut dock: ResMut<Dock>,
    mut dirty: ResMut<DockDirty>,
    mut dragged_id: Local<Option<String>>,
    mut over_zone: Local<bool>,
) {
    let Some(mut watch) = watch else {
        return;
    };

    // Resolve the drop using the prior frames' captured state, then reset.
    if mouse.just_released(MouseButton::Left) {
        if *over_zone {
            if let Some(id) = dragged_id.take() {
                make_workspace_from_panel(&id, &mut layouts, &mut dock, &mut dirty);
            }
        }
        *over_zone = false;
        *dragged_id = None;
        if let Ok(mut bg) = add_bg.single_mut() {
            bg.0 = Color::NONE;
        }
        // Deliberately leave `watch.claim`/`watch.dragging` for the dock to clear:
        // if the dock's `tab_drag` runs after us this frame it must still see the
        // claim so it skips its own re-dock. It clears both on release regardless.
        return;
    }

    // Track the in-flight drag and claim the drop while over the ribbon.
    if let Some(id) = &watch.dragging {
        if dragged_id.as_deref() != Some(id.as_str()) {
            *dragged_id = Some(id.clone());
        }
    }
    let hovering = watch.dragging.is_some() && zones.iter().any(|rcp| rcp.cursor_over);
    if watch.claim != hovering {
        watch.claim = hovering;
    }
    if *over_zone != hovering {
        *over_zone = hovering;
        if let Ok(mut bg) = add_bg.single_mut() {
            bg.0 = if hovering { rgb(accent()) } else { Color::NONE };
        }
    }
}

/// Move panel `id` into a brand-new workspace of its own and switch to it. The
/// panel is removed from the current (active) tree first so this is a move, not a
/// copy; the emptied current workspace is saved back into its slot.
fn make_workspace_from_panel(
    id: &str,
    layouts: &mut ShellLayouts,
    dock: &mut Dock,
    dirty: &mut DockDirty,
) {
    dock.tree.remove_panel(id);
    let active = layouts.active;
    if let Some(slot) = layouts.layouts.get_mut(active) {
        slot.1 = dock.tree.clone();
    }
    let name = renzora_ember::dock::humanize(id);
    layouts.layouts.push((name, DockTree::leaf(id.to_string())));
    let idx = layouts.layouts.len() - 1;
    dock.tree = layouts.layouts[idx].1.clone();
    layouts.active = idx;
    dirty.0 = true;
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

/// Which chrome bar an entity is, so [`apply_chrome_style`] can repaint each from
/// `Theme.chrome` (fill / height / separator edge / rounding / padding).
///
/// There's no `DocTabs` variant, even though the document tabs are a shell bar
/// again: [`build_doc_tabs`] paints its own band from the palette (a `mix` of
/// `panel` toward `header`) rather than from `Theme.chrome`, so there is nothing
/// here to repaint. `Theme.chrome.doc_tabs` still exists — themes on disk set
/// it, and the dock's own tab strips read it.
#[derive(Component, Clone, Copy)]
enum ChromeBar {
    Top,
    Status,
}

/// Repaint the chrome bars (top bar, status bar) from the ember `Theme.chrome`
/// whenever the theme changes — mirrors the dock's `apply_dock_style` so the
/// bars are theme-driven (and live-editable in the Theme tab) rather than baking
/// in palette colors. The status bar's separator sits on its top edge; the top
/// bar's on the bottom.
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
    theme_menu_open: bool,
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
            renzora_ember::widgets::ThemeShaderSurface {
                surface: renzora_ember::widgets::ThemeSurface::StatusBar,
            },
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
    renzora_ember::reactive::tracked::keyed_list(commands, left_content, status_snapshot_left);

    // The language + theme dropups — fixed elements on the right, before metrics.
    let lang_picker = language_dropup(commands, fonts);
    let dropup = theme_dropup(commands, fonts, themes, active, theme_menu_open);

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
    renzora_ember::reactive::tracked::keyed_list(commands, right_content, status_snapshot_right);

    commands.entity(bar).add_children(&[left_content, lang_picker, dropup, right_content]);
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
    open: bool,
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
                // Start open when a prior chrome instance had it open (a theme
                // switch rebuilds the chrome — see `ThemeMenuOpen`).
                display: if open { Display::Flex } else { Display::None },
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
    // Keyed so the list keeps its scroll position when picking a theme rebuilds
    // the whole chrome (see `ThemeMenuOpen`).
    let scroll = scroll_area_keyed(commands, content, 260.0, "status-theme-menu");
    commands.entity(panel).add_child(scroll);

    let icon = icon_text(commands, &fonts.phosphor, "palette", text_muted(), 12.0);
    let theme_label = if active.is_empty() {
        renzora::lang::t_or("status.theme", "Theme")
    } else {
        active.to_string()
    };
    let label = commands
        .spawn((
            Text::new(theme_label),
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
            Popup { panel, open },
            ThemeDropup,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("theme-dropup"),
        ))
        .id();
    commands.entity(trigger).add_children(&[icon, label, caret, panel]);
    trigger
}

/// Marks the status-bar language picker trigger.
#[derive(Component)]
struct LanguageDropup;

/// The language picker dropup in the status bar: shows the active language and
/// opens a menu (flipped up) of every registered language — built-in packs plus
/// any external `languages/*.toml`. Picking one calls `renzora::lang::set_active`
/// and persists it via `renzora::save_language`; the resulting `LanguageChanged`
/// drives whatever live relocalization is wired up. Mirrors `theme_dropup`.
fn language_dropup(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let langs = renzora::lang::available();
    let active = renzora::lang::active_code();

    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                // Flip up (bar is at the window bottom), anchored to the right.
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
            Name::new("language-menu"),
        ))
        .id();

    let mut rows = Vec::new();
    for m in &langs {
        let code = m.code.clone();
        let label = if m.name.is_empty() {
            m.code.clone()
        } else {
            m.name.clone()
        };
        let icon = if m.code == active { "check" } else { "globe" };
        rows.push(menu_item(commands, fonts, icon, &label, move |_w| {
            renzora::lang::set_active(&code);
            let _ = renzora::save_language(&code);
        }));
    }
    let content = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .id();
    commands.entity(content).add_children(&rows);
    let scroll = scroll_area_keyed(commands, content, 260.0, "status-language-menu");
    commands.entity(panel).add_child(scroll);

    // Trigger shows the active language's native name (falls back to its code).
    let active_name = langs
        .iter()
        .find(|m| m.code == active)
        .map(|m| {
            if m.name.is_empty() {
                m.code.clone()
            } else {
                m.name.clone()
            }
        })
        .unwrap_or_else(|| {
            if active.is_empty() {
                renzora::lang::t("settings.row.language")
            } else {
                active.clone()
            }
        });

    let icon = icon_text(commands, &fonts.phosphor, "globe", text_muted(), 12.0);
    let label = commands
        .spawn((
            Text::new(active_name),
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
            Popup { panel, open: false },
            LanguageDropup,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("language-dropup"),
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
fn status_rows(world: &Rx, align: renzora::ShellStatusAlign) -> Vec<StatusRow> {
    let mut rows: Vec<StatusRow> = Vec::new();
    if let Some(reg) = world.get_resource::<renzora::ShellStatusRegistry>() {
        let mut items: Vec<&renzora::ShellStatusItem> =
            reg.items.iter().filter(|i| i.align == align).collect();
        items.sort_by_key(|i| i.order);
        for it in items {
            rows.extend((it.render)(world.untracked()).into_iter().map(StatusRow::Seg));
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
                // The bar's *fraction* is quantized to whole percent before it
                // reaches the hash. It changes continuously, and every change
                // rebuilds the row — a raw f32 would despawn and respawn the
                // segment on most frames of a long job. `Busy` is not hashed at
                // all beyond its kind: it animates in place (see
                // `progress_indeterminate`), so it never needs a rebuild.
                StatusRow::Seg(s) => {
                    let bar = match s.bar {
                        None => (0u8, 0u8),
                        Some(renzora::ShellStatusBar::Busy) => (1u8, 0u8),
                        Some(renzora::ShellStatusBar::Fraction(f)) => {
                            (2u8, (f.clamp(0.0, 1.0) * 100.0) as u8)
                        }
                    };
                    (1u8, &s.icon, &s.text, s.color, bar).hash(&mut h)
                }
            }
            (k.finish(), h.finish())
        })
        .collect();
    renzora_ember::reactive::KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| status_row(c, f, &rows[i])),
    }
}

/// Left side: a Ready label + left-aligned status items. A plugin can swap the
/// "Ready" text via [`renzora::ShellReadyStatus`] (e.g. the auto-save countdown).
fn status_snapshot_left(world: &Rx) -> renzora_ember::reactive::KeyedSnapshot {
    let (label, color) = world
        .get_resource::<renzora::ShellReadyStatus>()
        .and_then(|r| r.label.clone().map(|t| (t, r.color)))
        .map(|(t, c)| (t, c.map(|c| (c[0], c[1], c[2])).unwrap_or_else(text_muted)))
        .unwrap_or_else(|| (renzora::lang::t_or("status.ready", "Ready"), text_muted()));
    let mut rows = vec![StatusRow::Label(label, color)];
    rows.extend(status_rows(world, renzora::ShellStatusAlign::Left));
    rows_snapshot(rows)
}

/// Right side: the right-aligned metrics.
fn status_snapshot_right(world: &Rx) -> renzora_ember::reactive::KeyedSnapshot {
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
                                font: bevy::text::FontSource::Handle(fonts.phosphor.clone()),
                                font_size: bevy::text::FontSize::Px(12.0),
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
            // Sized for a 22px-tall bar: wide enough to read as progress,
            // narrow enough that a background job doesn't push the rest of the
            // status bar around.
            match s.bar {
                None => {}
                Some(renzora::ShellStatusBar::Busy) => kids.push(
                    renzora_ember::widgets::progress_indeterminate(commands, 70.0, 4.0),
                ),
                Some(renzora::ShellStatusBar::Fraction(f)) => {
                    kids.push(renzora_ember::widgets::progress_sized(commands, f, 70.0, 4.0))
                }
            }
            commands.entity(r).add_children(&kids);
            r
        }
    }
}

/// The top bar: File/Edit/View/Help on the left, the layout ribbon centered,
/// action buttons on the right.
fn build_top_bar(commands: &mut Commands, font: &bevy::text::FontSource, fonts: &EmberFonts) -> Entity {
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
            // Host for a themeable shader effect (matrix rain, …). The driver in
            // ember paints it as this node's background when the active theme sets
            // `effects.top_bar`; the menus/buttons render on top.
            renzora_ember::widgets::ThemeShaderSurface {
                surface: renzora_ember::widgets::ThemeSurface::TopBar,
            },
            // The bar is the window drag handle; empty areas (zones pass through)
            // reach it, while interactive children (menus/buttons) block it.
            Interaction::default(),
            WindowDragHandle,
            Name::new("top-bar"),
        ))
        .id();

    // `clip: false` — the Play button's target dropdown is a child of its caret,
    // absolutely positioned below the bar, and bevy_ui clips absolutely
    // positioned descendants like everything else (the trap that eats tooltips
    // and submenu panels). A growing zone clips by default so its contents can't
    // spill over the centered ribbon, which mattered while the document tabs
    // lived here and could be arbitrarily wide; what's left is a fixed handful
    // of buttons that will never reach half the window.
    let left = zone(commands, "top-left", JustifyContent::FlexStart, 2.0, 1.0, false);
    // Everything that acts on the *session* rather than on a panel: the
    // hamburger, Settings, undo / redo / save, and Play. All of them used to be
    // somewhere in the viewport's tool strip or its menus, which meant they were
    // missing from any workspace without a viewport — and none of them is a
    // viewport action. This bar is on screen in every workspace. The document
    // tabs used to fill the rest of this zone; they now sit at the top of the
    // viewport panel (see [`build_doc_tabs`]).
    let hamburger = hamburger_menu_item(commands);
    let session = renzora_viewport::native_header::build_session_actions(commands, fonts);
    let settings = settings_button(commands);
    // Plugin-contributed icon buttons, right of the gear. The Marketplace's is
    // the first: it is a place you go, not a panel you dock, so it wants a door
    // in the chrome rather than a tab in a workspace.
    let actions = build_shell_actions(commands);
    let play = build_play_group(commands, font);
    // The document tabs, for anyone who'd rather not spend a row of the window
    // on them — hidden unless Settings has them set to Dropdown, in which case
    // the strip under this bar is the one that's hidden instead.
    let docs = build_doc_tab_menu_group(commands, fonts, font);
    commands
        .entity(left)
        .add_children(&[hamburger, session, settings, actions, play, docs]);

    let center = zone(commands, "top-center", JustifyContent::Center, 2.0, 0.0, false);
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
        renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
    ));
    // Reactive ribbon — one button per workspace in `ShellLayouts`. Capped the
    // same way the document tabs are, so a project with a dozen workspaces folds
    // the tail into a `»` menu instead of crowding out the bar's two ends.
    let (ribbon_strip, ribbon) = renzora_ember::widgets::overflow_strip(
        commands,
        renzora_ember::widgets::OverflowBudget::Fixed(RIBBON_W),
        "ribbon",
    );
    commands
        .entity(ribbon)
        .insert((WorkspaceDropZone, RelativeCursorPosition::default()));
    renzora_ember::reactive::tracked::keyed_list(commands, ribbon, ribbon_snapshot);
    let add = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            WorkspaceAddBtn,
            WorkspaceDropZone,
            RelativeCursorPosition::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("workspace-add"),
        ))
        .id();
    let add_label = commands
        .spawn((Text::new("+"), ui_font(font, 12.0), TextColor(rgb(text_muted()))))
        .id();
    commands.entity(add).add_child(add_label);
    commands.entity(center).add_children(&[magnifier, ribbon_strip, add]);

    // The right zone is window controls only now: Play moved to the toolbar
    // strip's trailing edge (see [`build_play_group`]) and the gear moved into
    // the hamburger menu as its own top-level Settings row.
    let right = zone(commands, "top-right", JustifyContent::FlexEnd, 8.0, 1.0, true);

    // Window controls: a fixed-size button with the glyph as a *child* so
    // `align_items`/`justify_content: Center` truly center it (text placed
    // directly on a node is NOT vertically centered by Bevy — it rides the top
    // of the box). The buttons are shorter than the bar and centered in it, so
    // their glyphs line up with the play/code/gear icons to their left.
    let window = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(2.0),
                margin: UiRect::left(Val::Px(6.0)),
                ..default()
            },
            Name::new("window-buttons"),
        ))
        .id();
    #[allow(unused_mut)]
    let mut kids: Vec<Entity> = Vec::new();

    // Web: one fullscreen toggle instead of the three window controls.
    //
    // A browser tab has no OS window to minimize, maximize or close —
    // `set_minimized` is a no-op and a tab cannot close itself unless a script
    // opened it. Fullscreen is the one window state a page CAN change, and it
    // is the one worth having: it hides the tab strip and address bar and gives
    // the editor the whole display.
    #[cfg(target_arch = "wasm32")]
    {
        let btn = commands
            .spawn((
                Node {
                    width: Val::Px(32.0),
                    height: Val::Px(24.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Interaction::default(),
                WebFullscreenBtn,
                renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            ))
            .id();
        let g = glyph(commands, "corners-out", text_muted(), 14.0);
        commands.entity(g).insert(bevy::ui::FocusPolicy::Pass);
        commands.entity(btn).add_child(g);
        // Same hover treatment as the desktop minimize/maximize buttons — a
        // faint wash, no red (nothing here is destructive).
        renzora_ember::reactive::tracked::bind_bg(commands, btn, move |w| {
            match w.get::<Interaction>(btn) {
                Some(Interaction::Hovered) | Some(Interaction::Pressed) => {
                    Color::srgba(1.0, 1.0, 1.0, 0.09)
                }
                _ => Color::NONE,
            }
        });
        renzora_ember::reactive::tracked::bind_text_color(commands, g, move |w| {
            match w.get::<Interaction>(btn) {
                Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(text_primary()),
                _ => rgb(text_muted()),
            }
        });
        kids.push(btn);
    }

    #[cfg(not(target_arch = "wasm32"))]
    for (name, action, is_close) in [
        ("minus", WindowAction::Minimize, false),
        ("square", WindowAction::ToggleMaximize, false),
        ("x", WindowAction::Close, true),
    ] {
        let btn = commands
            .spawn((
                Node {
                    width: Val::Px(32.0),
                    height: Val::Px(24.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                Interaction::default(),
                WindowBtn(action),
                renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            ))
            .id();
        // The glyph is a child; `FocusPolicy::Pass` lets the hover/click land on
        // the button (so the bindings below see the parent's `Interaction`).
        let g = glyph(commands, name, text_muted(), 14.0);
        commands.entity(g).insert(bevy::ui::FocusPolicy::Pass);
        if matches!(action, WindowAction::ToggleMaximize) {
            // The maximize glyph reflects window state (square ↔ restore).
            commands.entity(g).insert(MaximizeIcon);
        }
        commands.entity(btn).add_child(g);

        // Hover fill on the button: minimize/maximize get a faint wash; close
        // goes the standard Windows close-red.
        renzora_ember::reactive::tracked::bind_bg(commands, btn, move |w| match w.get::<Interaction>(btn) {
            Some(Interaction::Hovered) | Some(Interaction::Pressed) => {
                if is_close {
                    Color::srgb_u8(232, 17, 35)
                } else {
                    Color::srgba(1.0, 1.0, 1.0, 0.09)
                }
            }
            _ => Color::NONE,
        });
        // Glyph color tracks the parent button's hover: the close × turns white
        // on its red fill; the other two brighten from muted to primary.
        renzora_ember::reactive::tracked::bind_text_color(commands, g, move |w| {
            match w.get::<Interaction>(btn) {
                Some(Interaction::Hovered) | Some(Interaction::Pressed) => {
                    if is_close {
                        Color::WHITE
                    } else {
                        rgb(text_primary())
                    }
                }
                _ => rgb(text_muted()),
            }
        });
        kids.push(btn);
    }
    commands.entity(window).add_children(&kids);

    // ── "Update available" chip ──────────────────────────────────────────────
    // Present only while `renzora_update`'s background check has something to
    // offer; the resource is removed again when a later check disagrees, and the
    // chip goes with it. Clicking opens the same overlay as Help ▸ Check for
    // Updates — this is a shortcut to it, not a second way of doing it.
    let update_chip = build_update_chip(commands, font);

    commands.entity(right).add_children(&[update_chip, window]);

    commands.entity(bar).add_children(&[left, center, right]);
    bar
}

/// A top-bar ribbon entry (workspace switcher). Full height so the active
/// item's blue underline pins to the bottom edge. Clicking switches workspace
/// `index`; dragging reorders, right-click renames/removes (see [`ribbon_interact`]).
fn ribbon_item(
    commands: &mut Commands,
    font: &bevy::text::FontSource,
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
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new(format!("ribbon:{label}")),
        ))
        .id();
    // Localize the *display* of built-in workspace names (Scene, Scripting, …)
    // via `layout.<slug>`; the stored `label` stays the workspace's identity (it's
    // the persisted key + the entity Name). A user-renamed/added workspace has no
    // matching key, so `t_or` falls back to its raw name.
    let display = renzora::lang::t_or(&format!("layout.{}", label.to_lowercase()), label);
    let text = commands
        .spawn((
            Text::new(display),
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
    // Insertion marker: a thin accent bar pinned to the item's edge, hidden until
    // a reorder drag points at this slot (see [`ribbon_interact`]). Absolutely
    // positioned so it never affects the ribbon's layout.
    let marker = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(-2.0),
                top: Val::Px(0.0),
                height: Val::Percent(100.0),
                width: Val::Px(2.0),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(accent())),
            bevy::ui::FocusPolicy::Pass,
            Name::new("ribbon-insert-marker"),
        ))
        .id();
    commands.entity(item).insert(RibbonItem { index, marker });
    // What this workspace looks like in the ribbon's `»` menu once it folds —
    // and, while it's the active one, the guarantee that it never folds at all.
    commands.entity(item).insert(renzora_ember::widgets::OverflowEntry::new(
        "browsers",
        &renzora::lang::t_or(&format!("layout.{}", label.to_lowercase()), label),
        move |w| select_workspace(w, index),
    ));
    if active {
        commands.entity(item).insert(renzora_ember::widgets::OverflowKeep);
    }
    commands.entity(item).add_children(&[text_wrap, underline, marker]);
    item
}

/// Switch to workspace `index` from a `&mut World` context (the ribbon's
/// overflow menu, which has no system params of its own). The three resources
/// [`apply_workspace`] mutates can't be borrowed at once, hence the nesting.
fn select_workspace(w: &mut World, index: usize) {
    w.resource_scope(|w, mut layouts: Mut<ShellLayouts>| {
        w.resource_scope(|w, mut dock: Mut<Dock>| {
            let mut dirty = w.resource_mut::<DockDirty>();
            apply_workspace(index, &mut layouts, &mut dock, &mut dirty);
        });
    });
}

/// Activate document tab `id` from a `&mut World` context — the same work
/// [`doc_tab_click`] does for a click on the tab itself, for the tabs that have
/// folded into the strip's overflow menu.
fn activate_doc_tab(w: &mut World, id: u64) {
    let Some(mut state) = w.get_resource_mut::<renzora_ui::DocumentTabState>() else {
        return;
    };
    let Some(idx) = state.tabs.iter().position(|t| t.id == id) else {
        return;
    };
    let switch = state.activate_tab(idx);
    let layout = state.tabs[idx].kind.layout_name().map(|n| n.to_string());
    if let Some((old_tab_id, new_tab_id)) = switch {
        w.insert_resource(renzora::TabSwitchRequest { old_tab_id, new_tab_id });
    }
    let Some(layout) = layout else { return };
    let index = w
        .get_resource::<ShellLayouts>()
        .and_then(|l| l.layouts.iter().position(|(n, _)| *n == layout));
    if let Some(index) = index {
        select_workspace(w, index);
    }
}

/// Keyed snapshot of the workspace ribbon (one button per `ShellLayouts` entry;
/// the content hash carries the active flag so switching repaints just the two
/// affected buttons).
fn ribbon_snapshot(world: &Rx) -> renzora_ember::reactive::KeyedSnapshot {
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
fn build_ribbon_rename_field(commands: &mut Commands, font: &bevy::text::FontSource, index: usize, name: &str) -> Entity {
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

/// The document tab bar: every open document (`DocumentTabState`, shared with
/// the egui editor) rendered reactively, plus an add-document button.
///
/// **Where it lives.** A row of the shell's own column, directly under the top
/// bar and above the dock — so it is on screen in every workspace. It spent a
/// while inside the primary viewport panel, mounted through
/// [`renzora_ember::toolbar::register_viewport_top_strip`], on the argument that
/// tabs belong with the thing they switch between. What that actually bought
/// was a tab bar that existed only where a `viewport` panel did: five of the
/// nine default workspaces (Blueprints, Materials, Particles, Animation, Hub)
/// have none, and an open material routes the editor *to* one of those — so the
/// bar holding that material's tab vanished the moment you clicked it, leaving
/// the document unreachable and uncloseable from its own editor.
///
/// Scenes and assets share the one bar. They are one list in the model, they
/// are one Ctrl+Tab's worth of "things I have open" to the user, and splitting
/// them into two bars only asks which half a given file is in.
///
/// The primary viewport's Maximize button still rides along at the right-hand
/// end, as it did when this bar lived in the panel: it was the full-width row
/// there and it is the full-width row here.
///
/// The bar spans the window, so nothing folds until the tabs genuinely fill it.
/// Inside it the tab list hugs its content, so the `+` button sits directly
/// after the last tab and travels right as tabs are added; once they fill the
/// bar the surplus folds into the caret menu and `+` stops moving.
fn build_doc_tabs(commands: &mut Commands) -> Entity {
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(30.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                flex_shrink: 0.0,
                min_width: Val::Px(0.0),
                padding: UiRect::horizontal(Val::Px(6.0)),
                overflow: Overflow::clip(),
                // Closed off underneath, against the dock. Dark rather than the
                // toolbar's own separator colour: this edge is where the window
                // chrome stops and the workspace begins, which is a harder
                // boundary than the ones *inside* the chrome.
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            // A half-step off `panel` toward the theme's contrasting surface —
            // just enough to read as its own band rather than more toolbar.
            // Mixing toward a second *theme* colour rather than toward white
            // keeps it differentiated on light themes too, where "lighter" would
            // walk it into the background instead of away from it.
            //
            // Graded rather than flat, and the direction matters: lit at the top
            // where it meets the top bar, settling back toward `panel` at the
            // bottom where the dark rule closes it off against the dock. The
            // band therefore reads as catching light from above rather than as a
            // slab someone dropped between two darker things.
            BackgroundColor(mix(panel_bg(), header_bg(), 0.55)),
            BackgroundGradient::from(LinearGradient::to_bottom(vec![
                ColorStop::auto(mix(panel_bg(), header_bg(), 0.85)),
                ColorStop::auto(mix(panel_bg(), header_bg(), 0.20)),
            ])),
            BorderColor::all(rgb(divider())),
            // No `OverlaySurface` here any more: it needed one while it sat over
            // the viewport's picking area, where a click landing between two tabs
            // would otherwise fall through and deselect whatever was in the
            // scene. As a row of the shell's column it has nothing behind it.
            Name::new("doc-tabs"),
        ))
        .id();

    // Reactive tab strip from the shared DocumentTabState. The budget is the
    // bar's own measured width, less room for the caret and `+` that share it.
    // Gap 0: the tabs butt against each other, so the run of inactive ones reads
    // as a single band with the active tab cut out of it.
    let (strip, tabs) = renzora_ember::widgets::overflow_strip_gap(
        commands,
        renzora_ember::widgets::OverflowBudget::Fill { measure: bar, reserve: 66.0 },
        0.0,
        "doc-tab",
    );
    renzora_ember::reactive::tracked::keyed_list(commands, tabs, doc_tab_snapshot);

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
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("doc-add"),
        ))
        .id();
    let plus_icon = glyph(commands, "plus", text_muted(), 13.0);
    commands.entity(plus).add_child(plus_icon);

    // Nothing after the `+`: the viewport's Maximize used to be parked at the far
    // end of this bar, and went back to the viewport's own toolbar when the bar
    // stopped being part of that panel.
    commands.entity(bar).add_children(&[strip, plus]);
    // Hidden — and costing no height, since it's a row of the shell's column —
    // while Settings has the tabs set to Dropdown.
    renzora_ember::reactive::tracked::bind_display(commands, bar, |w: &Rx| !doc_tabs_dropdown(w));
    bar
}

/// Whether the document tabs are set to the top-bar dropdown rather than the
/// strip. Read through the `Rx` so both presentations' `bind_display`s react to
/// the setting changing; false (the strip) when there's no `EditorSettings` yet.
fn doc_tabs_dropdown(w: &Rx) -> bool {
    w.get_resource::<renzora_editor_framework::EditorSettings>()
        .is_some_and(|s| s.doc_tabs_dropdown)
}

/// The trigger button of the document-tab dropdown, so its popup can be closed
/// from a row inside it.
#[derive(Component)]
struct DocTabMenuTrigger;

/// A row in that dropdown. The row also carries [`DocTabClick`] (or
/// [`DocAddBtn`]), which the strip's own systems handle — this marker exists
/// only to close the menu behind the click.
#[derive(Component)]
struct DocTabMenuRow;

/// The document tabs as a dropdown in the top bar, beside Play, plus the
/// primary viewport's Maximize — the other presentation of [`build_doc_tabs`],
/// chosen in Settings → Interface → Document tabs.
///
/// The trade it offers is a row of the window: the strip is easier to move
/// between (every document is one click, and you can see what's open without
/// asking), and this is smaller. It shows the active document — icon, name, and
/// the `*` when it has unsaved edits — and opens onto all of them.
///
/// Maximize comes along because it lives at the end of the strip, and the strip
/// is what's hidden. Both presentations build their own, tagged
/// `MaximizeSlot(0)`; the driver systems find them by component and only one is
/// ever on screen, so a hidden duplicate costs nothing.
fn build_doc_tab_menu_group(
    commands: &mut Commands,
    fonts: &EmberFonts,
    font: &bevy::text::FontSource,
) -> Entity {
    // One row per open document, reactive off the same state the strip renders.
    let list = commands
        .spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    renzora_ember::reactive::tracked::keyed_list(commands, list, doc_tab_menu_snapshot);

    // Ember's own popup surface rather than a hand-rolled node: only this one is
    // known to `correct_pointer_state`, and without that a click on a row lands
    // in the viewport behind the menu as well as on the row — the menu hangs
    // over the scene, so that would select whatever was under it.
    let panel = renzora_ember::widgets::popup_panel_aligned(
        commands,
        &[list],
        renzora_ember::widgets::PopupAlign::Left,
    );
    commands.entity(panel).insert(Name::new("doc-tab-menu"));
    // Tightened rather than rebuilt: the surface's own layout (absolute, edge
    // alignment, the `OverlaySurface` that makes clicks stop here) stays
    // ember's, and only the metrics change. A toolbar-sized control shouldn't
    // open a panel with the padding of a settings popover.
    commands
        .entity(panel)
        .entry::<Node>()
        .and_modify(|mut n| {
            n.min_width = Val::Px(170.0);
            n.padding = UiRect::all(Val::Px(4.0));
            n.row_gap = Val::Px(1.0);
        });

    let trigger = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                // 22px and tight, like the toolbar's compact dropdowns — this
                // sits between the Play pill and the ribbon, where a control
                // that names a document can eat the bar if it's let to.
                height: Val::Px(22.0),
                padding: UiRect::horizontal(Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                position_type: PositionType::Relative,
                max_width: Val::Px(150.0),
                // NOT `Overflow::clip()`, however much a too-long name wants it:
                // the menu is a *child* of this node, and a clipping parent
                // clips absolutely-positioned descendants too — so the panel
                // opened, correctly, inside a 190×20 box and was never seen.
                // The clip belongs on the label, which is the thing that can
                // overflow.
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            Popup { panel, open: false },
            DocTabMenuTrigger,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("doc-tab-menu-trigger"),
        ))
        .id();
    renzora_ember::reactive::tracked::bind_bg(commands, trigger, move |w| {
        match w.get::<Interaction>(trigger) {
            Some(Interaction::Hovered) | Some(Interaction::Pressed) => {
                Color::srgba(1.0, 1.0, 1.0, 0.09)
            }
            _ => Color::NONE,
        }
    });
    // Kind glyph + name of the active document, both following it.
    let icon = icon_text(
        commands,
        &fonts.phosphor,
        "film-slate",
        renzora_ui::DocTabKind::Scene.color(),
        12.0,
    );
    commands.entity(icon).insert(bevy::ui::FocusPolicy::Pass);
    renzora_ember::reactive::tracked::bind_text(commands, icon, |w: &Rx| {
        let name = active_doc(w).map(|t| t.kind.icon()).unwrap_or("film-slate");
        renzora_ember::phosphor_map::icon_glyph(name)
            .unwrap_or('\u{E4C6}')
            .to_string()
    });
    // The glyph carries the active document's type color, same as its tab does
    // in the strip — this trigger is that tab, in the compact layout that has no
    // room for the strip.
    renzora_ember::reactive::tracked::bind_text_color(commands, icon, |w: &Rx| {
        rgb(active_doc(w)
            .map(|t| t.kind.color())
            .unwrap_or_else(|| renzora_ui::DocTabKind::Scene.color()))
    });
    let label = commands
        .spawn((
            Text::new(String::new()),
            ui_font(font, 11.0),
            TextColor(rgb(text_primary())),
            // The trigger's width cap is enforced here rather than on the
            // trigger, which has the menu among its children (see above).
            bevy::text::TextLayout::no_wrap(),
            Node {
                min_width: Val::Px(0.0),
                overflow: Overflow::clip(),
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    renzora_ember::reactive::tracked::bind_text(commands, label, |w: &Rx| {
        active_doc(w)
            .map(|t| {
                let shown = elide(&t.name, DOC_TAB_CHARS);
                if t.is_modified {
                    format!("{shown}*")
                } else {
                    shown
                }
            })
            .unwrap_or_default()
    });
    let caret = glyph(commands, "caret-down", text_muted(), 10.0);
    commands.entity(caret).insert(bevy::ui::FocusPolicy::Pass);
    commands
        .entity(trigger)
        .add_children(&[icon, label, caret, panel]);

    // The strip's `+`, in the same place relative to the documents: immediately
    // to their right. `DocAddBtn` is what `doc_add_click` handles, wherever it
    // sits, so this is the strip's button in a second spot rather than a second
    // implementation of it.
    let plus = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(Val::Px(5.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            DocAddBtn,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            renzora_ember::widgets::HoverTooltip::new(renzora::lang::t("menu.file.new_scene")),
            Name::new("doc-tab-menu-add"),
        ))
        .id();
    renzora_ember::reactive::tracked::bind_bg(commands, plus, move |w| {
        match w.get::<Interaction>(plus) {
            Some(Interaction::Hovered) | Some(Interaction::Pressed) => {
                Color::srgba(1.0, 1.0, 1.0, 0.09)
            }
            _ => Color::NONE,
        }
    });
    let plus_icon = glyph(commands, "plus", text_muted(), 13.0);
    commands.entity(plus).add_child(plus_icon);

    let group = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(2.0),
                margin: UiRect::left(Val::Px(8.0)),
                display: Display::None,
                ..default()
            },
            Name::new("doc-tab-menu-group"),
        ))
        .id();
    renzora_ember::reactive::tracked::bind_display(commands, group, doc_tabs_dropdown);
    commands.entity(group).add_children(&[trigger, plus]);
    group
}

/// The active document tab, read through the `Rx` so a binding on it reacts.
fn active_doc<'w>(w: &Rx<'w>) -> Option<&'w renzora_ui::DocumentTab> {
    w.get_resource::<renzora_ui::DocumentTabState>()
        .and_then(|s| s.tabs.get(s.active_tab))
}

/// A hoverable row of the document-tab dropdown, without its contents.
fn doc_tab_menu_row_node(commands: &mut Commands, name: &'static str) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            DocTabMenuRow,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new(name),
        ))
        .id();
    renzora_ember::reactive::tracked::bind_bg(commands, row, move |w| {
        match w.get::<Interaction>(row) {
            Some(Interaction::Hovered) | Some(Interaction::Pressed) => {
                rgb(renzora_ember::theme::hover_bg())
            }
            _ => Color::NONE,
        }
    });
    row
}

/// The dropdown's rows: every open document, active one accented. Keyed by id
/// like the strip's, so a row repaints only when its own content changes.
fn doc_tab_menu_snapshot(world: &Rx) -> renzora_ember::reactive::KeyedSnapshot {
    use std::hash::{Hash, Hasher};
    let empty = || renzora_ember::reactive::KeyedSnapshot {
        items: Vec::new(),
        build: Box::new(|c, _, _| c.spawn(Node::default()).id()),
    };
    let Some(state) = world.get_resource::<renzora_ui::DocumentTabState>() else {
        return empty();
    };
    // Closable follows the strip's rule exactly (see `doc_tab_snapshot`): the
    // model refuses the last tab and the last *scene*, so a ✕ that only some of
    // these rows can honour would be worse than none.
    let scenes = state.tabs.iter().filter(|t| !t.kind.is_asset()).count();
    let rows: Vec<(u64, String, renzora_ui::DocTabKind, bool, bool, bool)> = state
        .tabs
        .iter()
        .enumerate()
        .map(|(i, t)| {
            (
                t.id,
                t.name.clone(),
                t.kind,
                i == state.active_tab,
                t.is_modified,
                state.tabs.len() > 1 && (t.kind.is_asset() || scenes > 1),
            )
        })
        .collect();
    let items: Vec<(u64, u64)> = rows
        .iter()
        .map(|(id, name, kind, active, modified, can_close)| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            id.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (name, kind, active, modified, can_close).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    renzora_ember::reactive::KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (id, name, kind, active, modified, can_close) = &rows[i];
            let row = doc_tab_menu_row_node(c, "doc-tab-menu-row");
            c.entity(row).insert(DocTabClick(*id));
            // Type color on every row, matching the strip this dropdown stands
            // in for: whichever of the two the user has chosen, a document is
            // named by the same glyph in the same color.
            let ic = icon_text(c, &f.phosphor, kind.icon(), kind.color(), 11.0);
            let label = c
                .spawn((
                    Text::new(if *modified {
                        format!("{name}*")
                    } else {
                        name.clone()
                    }),
                    ui_font(&f.ui, 11.0),
                    TextColor(rgb(if *active { text_primary() } else { text_muted() })),
                    bevy::text::TextLayout::no_wrap(),
                    // Takes the slack so the ✕ sits at the row's right edge
                    // rather than trailing the name.
                    Node {
                        flex_grow: 1.0,
                        min_width: Val::Px(0.0),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    bevy::ui::FocusPolicy::Pass,
                ))
                .id();
            let mut kids = vec![ic, label];
            // Every closable row carries one, not just the active row: in the
            // strip you can click the tab you want to close first, but here the
            // menu is the only way at a document that isn't the current one, so
            // a click-then-✕ would mean switching to a document just to shut it.
            if *can_close {
                let close = c
                    .spawn((
                        Node {
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            width: Val::Px(14.0),
                            height: Val::Px(14.0),
                            flex_shrink: 0.0,
                            border_radius: BorderRadius::all(Val::Px(3.0)),
                            ..default()
                        },
                        BackgroundColor(Color::NONE),
                        Interaction::default(),
                        DocTabClose(*id),
                        // Block, or the press also reaches the row's
                        // `DocTabClick` — closing a document by way of first
                        // switching the editor to it. `Node`'s required
                        // `FocusPolicy` is `Pass` in Bevy 0.19, so this is not
                        // the default.
                        bevy::ui::FocusPolicy::Block,
                        renzora_ember::cursor_icon::HoverCursor(
                            bevy::window::SystemCursorIcon::Pointer,
                        ),
                        Name::new("doc-tab-menu-close"),
                    ))
                    .id();
                let x = icon_text(c, &f.phosphor, "x", text_muted(), 10.0);
                c.entity(close).add_child(x);
                kids.push(close);
            }
            c.entity(row).add_children(&kids);
            row
        }),
    }
}

/// Close the document dropdown behind a click on any of its rows. The row's own
/// job — activating that tab, or adding a scene — is done by the systems that
/// own [`DocTabClick`] / [`DocAddBtn`], which don't know or care that they were
/// pressed inside a menu.
fn doc_tab_menu_row_click(
    rows: Query<(&Interaction, &DocTabMenuRow), Changed<Interaction>>,
    triggers: Query<Entity, (With<DocTabMenuTrigger>, With<Popup>)>,
    mut commands: Commands,
) {
    if !rows.iter().any(|(i, _)| *i == Interaction::Pressed) {
        return;
    }
    for trigger in &triggers {
        renzora_ember::widgets::close_popup(&mut commands, trigger);
    }
}

/// Width budget for the workspace ribbon before workspaces start folding. Unlike
/// the document tabs there's no container to measure — the ribbon is centered
/// and content-sized, so it grows symmetrically out of the middle of the bar and
/// a constant is what keeps it from meeting the two ends. Sized for the eight
/// built-in workspaces plus a few of your own.
const RIBBON_W: f32 = 700.0;

#[derive(Component)]
struct DocAddBtn;
#[derive(Component)]
struct DocTabClick(u64);
#[derive(Component)]
struct DocTabClose(u64);

/// A document tab in the strip, carrying its id and the insertion marker shown
/// at its edge during a reorder drag (mirrors [`RibbonItem`]).
#[derive(Component)]
struct DocTabItem {
    id: u64,
    marker: Entity,
}

/// Keyed snapshot of the open document tabs (id-keyed; the content hash carries
/// active/modified state so a tab repaints only when it actually changes).
fn doc_tab_snapshot(world: &Rx) -> renzora_ember::reactive::KeyedSnapshot {
    use std::hash::{Hash, Hasher};
    let empty = || renzora_ember::reactive::KeyedSnapshot {
        items: Vec::new(),
        build: Box::new(|c, _, _| c.spawn(Node::default()).id()),
    };
    let Some(state) = world.get_resource::<renzora_ui::DocumentTabState>() else {
        return empty();
    };
    // Closable is per-tab, because the model's two refusals aren't the same
    // rule: `close_tab` declines the last tab overall *and* the last scene tab,
    // the latter so Asset mode always has a scene to return to. Counting tabs
    // rather than scenes put a ✕ on the last scene as soon as a material was
    // open beside it — one that `close_tab` then quietly declined.
    let scenes = state.tabs.iter().filter(|t| !t.kind.is_asset()).count();
    let renaming = world.get_resource::<DocTabRename>().and_then(|r| r.0);
    let last = state.tabs.len().saturating_sub(1);
    // (id, name, kind, active, modified, renaming, trailing seam, closable)
    //
    // The *kind* travels rather than the glyph it resolves to, because the tab
    // now takes two things from it — the icon and that icon's type color — and
    // one of them in the snapshot would leave the other to be looked up twice.
    //
    // The seam belongs to the *boundary*, not to either tab, so exactly one of
    // the pair draws it: the left one. Every tab but the last, including either
    // side of the active one — with no fill on any tab there is nothing else
    // marking where one ends and the next begins.
    let tabs: Vec<(u64, String, renzora_ui::DocTabKind, bool, bool, bool, bool, bool)> = state
        .tabs
        .iter()
        .enumerate()
        .map(|(i, t)| {
            (
                t.id,
                t.name.clone(),
                t.kind,
                i == state.active_tab,
                t.is_modified,
                renaming == Some(t.id),
                i != last,
                state.tabs.len() > 1 && (t.kind.is_asset() || scenes > 1),
            )
        })
        .collect();
    let items: Vec<(u64, u64)> = tabs
        .iter()
        .map(|(id, name, kind, active, modified, editing, seam, can_close)| {
            let mut k = std::collections::hash_map::DefaultHasher::new();
            id.hash(&mut k);
            let mut h = std::collections::hash_map::DefaultHasher::new();
            (name, kind, active, modified, editing, seam, can_close).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    renzora_ember::reactive::KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| {
            let (id, name, kind, active, modified, editing, seam, can_close) = &tabs[i];
            if *editing {
                build_doc_rename_field(c, &f.ui, *id, name)
            } else {
                doc_tab_row(c, f, *id, name, *kind, *active, *modified, *can_close, *seam)
            }
        }),
    }
}

/// Inline rename field for a document tab, replacing the tab itself for as long
/// as the edit is live (the same swap `ribbon_snapshot` does). Seeded with the
/// current name — which for a saved document is its file stem, extension
/// excluded; [`rename_doc_tab`] puts the extension back.
fn build_doc_rename_field(
    commands: &mut Commands,
    font: &bevy::text::FontSource,
    id: u64,
    name: &str,
) -> Entity {
    let input = text_input(commands, font, &renzora::lang::t("common.name"), name);
    commands.entity(input).insert((
        DocTabRenameInput(id),
        Node {
            width: Val::Px(140.0),
            height: Val::Px(22.0),
            align_items: AlignItems::Center,
            padding: UiRect::horizontal(Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.0)),
            // Square, like the tab it stands in for.
            flex_shrink: 0.0,
            ..default()
        },
        // Folding the tab you're in the middle of renaming into the caret menu
        // would take the field you're typing in off screen with it.
        renzora_ember::widgets::OverflowKeep,
    ));
    input
}

#[allow(clippy::too_many_arguments)]
fn doc_tab_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    id: u64,
    name: &str,
    kind: renzora_ui::DocTabKind,
    active: bool,
    modified: bool,
    can_close: bool,
    seam: bool,
) -> Entity {
    let fg = if active { text_primary() } else { text_muted() };
    let icon = kind.icon();
    let tab = commands
        .spawn((
            Node {
                // Full-height, square, and unfilled: with no background of
                // its own, a tab is its icon and its name, and the padding is
                // the only thing separating one from the next.
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(11.0), Val::Px(0.0)),
                // Bottom edge, pointing at the scene the tab selects, and on
                // EVERY tab rather than only the active one — the border eats
                // into the content box, so handing it to one state and not the
                // other would shift the label the moment you clicked. Inactive
                // tabs simply paint theirs transparent.
                border: UiRect::bottom(Val::Px(2.0)),
                flex_shrink: 0.0,
                ..default()
            },
            // No fill in either state. Fills and gradients both tried to say
            // "these are separate objects", and each added more chrome to a
            // strip whose job is to name six things. Marking the active tab is
            // left to the accent rule under it and its brighter label — the same
            // pairing, and the same token, as the workspace ribbon's underline.
            BackgroundColor(Color::NONE),
            BorderColor::all(if active { rgb(accent()) } else { Color::NONE }),
            Interaction::default(),
            DocTabClick(id),
            // The reorder drag hit-tests in the cursor's own space rather than
            // against node centres, which drift under UI scaling — see
            // `ribbon_interact`, which learned this the hard way.
            RelativeCursorPosition::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            // How the tab appears in the strip's `»` menu once it folds. The
            // active tab is pinned visible — folding the one you're editing into
            // a menu is exactly the tab you can least afford to lose sight of.
            // Dragging the row moves the tab instead of activating it, so a
            // folded tab isn't stranded at the end of the strip with no way back.
            renzora_ember::widgets::OverflowEntry::new(icon, name, move |w| activate_doc_tab(w, id))
                .on_drag(move |w| start_doc_tab_drag(w, id))
                .icon_color(kind.color()),
            Name::new(format!("doc:{name}")),
        ))
        .id();
    // Insertion marker: a thin accent bar at the tab's edge, hidden until a
    // reorder drag points at this slot. Absolutely positioned, so it never
    // affects the strip's layout (or its width budget).
    let marker = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(-2.0),
                top: Val::Px(0.0),
                height: Val::Percent(100.0),
                width: Val::Px(2.0),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(accent())),
            bevy::ui::FocusPolicy::Pass,
            Name::new("doc-insert-marker"),
        ))
        .id();
    commands.entity(tab).insert(DocTabItem { id, marker });
    if active {
        commands.entity(tab).insert(renzora_ember::widgets::OverflowKeep);
    }
    // Kind icon: scene vs material vs script at a glance, without reading the
    // name. It was dropped while the strip was the top bar's fixed-width left
    // zone, where every glyph cost a tab off the visible end; spanning the whole
    // viewport there's room for it again.
    //
    // The type color, in every state — the same green the asset browser gives a
    // material, on the active tab and the inactive ones alike. Graying the
    // inactive ones made the strip say "current tab" twice, once with the accent
    // rule and again with six identical gray glyphs, while throwing away the one
    // thing the icon is there for: which tab holds which kind of thing. Active
    // state is the underline and the brighter label; the icon is type identity.
    let kind_icon = icon_text(commands, &fonts.phosphor, icon, kind.color(), 12.0);
    // Elide the *name*, then add the modified marker — eliding afterwards would
    // eat the asterisk on exactly the tabs that most need it.
    let shown = elide(name, DOC_TAB_CHARS);
    if shown != name {
        commands
            .entity(tab)
            .insert(renzora_ember::widgets::HoverTooltip::new(name));
    }
    // Semibold, not regular: the tab labels are the one place in the chrome
    // that names what you're editing, and at this size the weight is what
    // carries the active tab now that its fill is the same as the bar's.
    let mut label_font = ui_font(&fonts.ui, 12.0);
    label_font.weight = bevy::text::FontWeight::SEMIBOLD;
    let lbl = commands
        .spawn((
            Text::new(if modified { format!("{shown}*") } else { shown }),
            label_font,
            TextColor(rgb(fg)),
        ))
        .id();
    let mut kids = vec![kind_icon, lbl];
    // Only the active tab carries a ✕. On every tab it was six close buttons
    // competing for the eye, and it made the strip a near-copy of the dock's own
    // panel tab bar directly above — same chips, same ✕, same trailing `+` — for
    // two entirely different ideas. Closing an inactive scene is now click-then-✕,
    // which is one extra click on the thing you were about to look at anyway.
    if can_close && active {
        let close = commands
            .spawn((
                Node {
                    align_items: AlignItems::Center,
                    padding: UiRect::left(Val::Px(1.0)),
                    ..default()
                },
                Interaction::default(),
                DocTabClose(id),
                renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            ))
            .id();
        let x = icon_text(commands, &fonts.phosphor, "x", text_muted(), 10.0);
        commands.entity(close).add_child(x);
        kids.push(close);
    }
    // The boundary between two tabs: a short hairline centred on the trailing
    // edge, not a full-height rule. Edge-to-edge lines on flush tabs read as a
    // picket fence — the eye follows the verticals instead of the names.
    // Absolutely positioned, so it costs the tab no width.
    if seam {
        let line = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(0.0),
                    top: Val::Percent(30.0),
                    height: Val::Percent(40.0),
                    width: Val::Px(1.0),
                    ..default()
                },
                BackgroundColor(doc_tab_divider()),
                bevy::ui::FocusPolicy::Pass,
                Name::new("doc-tab-seam"),
            ))
            .id();
        kids.push(line);
    }
    kids.push(marker);
    commands.entity(tab).add_children(&kids);
    tab
}

/// Longest document-tab label kept intact, in characters.
const DOC_TAB_CHARS: usize = 18;

/// The hairline between two scene tabs.
///
/// The same token the viewport toolbar's own separators use (`border`, which the
/// palette takes from the theme's `border_light`), so the two rows of chrome
/// divide their contents the same way. It is deliberately NOT `divider`: that is
/// the darker token, and it belongs to the hard edge under the whole strip, not
/// to the soft boundaries between names inside it.
fn doc_tab_divider() -> Color {
    rgb(border())
}

/// Shorten `s` to `max` characters, ending in an ellipsis when it doesn't fit.
///
/// Done on the string because bevy_ui has no `text-overflow: ellipsis` — a `Text`
/// wider than its node either wraps or spills, and neither is what a tab wants.
/// Counting *characters* rather than measuring the laid-out width is
/// approximate for a proportional font, but it's stable, costs nothing, and an
/// elided tab carries the full name in a hover tooltip.
fn elide(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let kept: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", kept.trim_end())
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

/// `+` → add an "Untitled Scene" document and focus it.
fn doc_add_click(
    mut commands: Commands,
    q: Query<&Interaction, (With<DocAddBtn>, Changed<Interaction>)>,
    state: Option<ResMut<renzora_ui::DocumentTabState>>,
) {
    let Some(mut state) = state else { return };
    if q.iter().any(|i| *i == Interaction::Pressed) {
        let idx = state.add_tab("Untitled Scene".into(), None);
        // Cache the leaving scene + load the new (empty) tab's scene. The new
        // tab has no buffer, so `handle_tab_switch` resets to a fresh empty
        // scene — what "New Scene" should show, instead of the current scene.
        if let Some((old_id, new_id)) = state.activate_tab(idx) {
            commands.insert_resource(renzora::TabSwitchRequest {
                old_tab_id: old_id,
                new_tab_id: new_id,
            });
        }
    }
}

/// Most-recently-active document tab ids, oldest first, so a workspace switch
/// can return to the document you were last in *there* — see
/// [`sync_active_doc_to_workspace`].
///
/// Kept here rather than in [`renzora_ui::DocumentTabState`] because it's a
/// property of this session's navigation, not of the document set: nothing
/// persists it, and every activation route (a tab click, an asset browser
/// double-click, the inspector's edit button) is observed the same way — by
/// [`sync_workspace_to_active_doc`] noticing the active tab changed.
#[derive(Resource, Default)]
struct DocTabMru(Vec<u64>);

/// Whether a tab of `kind` belongs to the workspace called `workspace`.
///
/// Both layout tables count. `layout_name` is the direct answer for most kinds,
/// but shaders name a `Shaders` workspace that doesn't exist — the layout that
/// actually opens a `.wgsl` is the code editor's, which its *asset* layout
/// (`Scripting-Asset`) names. Stripping the `-Asset` suffix reads that mapping
/// off the data instead of hard-coding the exception, and it keeps the
/// `scene_layout_names_are_unique` invariant those tables are tested against.
fn kind_in_workspace(kind: renzora_ui::DocTabKind, workspace: &str) -> bool {
    kind.layout_name() == Some(workspace)
        || kind
            .asset_layout_name()
            .and_then(|l| l.strip_suffix("-Asset"))
            == Some(workspace)
}

/// Follow the active document tab: point [`renzora_ui::EditorContext`] at it and
/// switch the workspace its kind maps to.
///
/// This runs for *every* activation — a tab click, a programmatic open
/// (double-clicking an asset, the inspector's "edit" button), a close that
/// promotes its neighbour — because it watches the active id rather than any one
/// route. The `EditorContext` half is what makes clicking a second material tab
/// swap the graph: every asset panel loads from the context's path, so without
/// it the dock switched to the Materials workspace and left the *previous*
/// material in it. `open_asset_tab` sets the context when it opens a document;
/// nothing else did when you moved between two already-open ones.
///
/// The `Local` change-guard means it only fires on a real active-tab change, so
/// ribbon navigation while a doc tab is open isn't fought (the scene entities
/// are never touched — this is purely a layout switch).
#[allow(clippy::too_many_arguments)]
fn sync_workspace_to_active_doc(
    state: Option<Res<renzora_ui::DocumentTabState>>,
    mut layouts: ResMut<ShellLayouts>,
    mut dock: ResMut<Dock>,
    mut dirty: ResMut<DockDirty>,
    context: Option<ResMut<renzora_ui::EditorContext>>,
    project: Option<Res<renzora::CurrentProject>>,
    mut mru: ResMut<DocTabMru>,
    mut commands: Commands,
    mut last: Local<Option<u64>>,
) {
    let Some(state) = state else { return };
    let active_id = state.active_tab_id();
    if *last == active_id {
        return;
    }
    *last = active_id;
    let Some(tab) = state.active_tab() else { return };

    // Newest last. Dropping ids of closed tabs here keeps the stack from growing
    // for a session's worth of opens and closes.
    mru.0.retain(|id| *id != tab.id && state.tabs.iter().any(|t| t.id == *id));
    mru.0.push(tab.id);

    // Asset panels read their file straight off this, so it has to move with the
    // tab. Written only when it actually differs: it's a change-detected
    // resource, and several panels reload on any change to it.
    if let Some(mut context) = context {
        let next = renzora_ui::EditorContext::from_tab(tab);
        if *context != next {
            *context = next;
        }
    }

    // The kind's own workspace, or — for a kind naming one that doesn't exist —
    // the workspace its asset layout is derived from, which is the same fallback
    // [`kind_in_workspace`] accepts. Shaders are the case: `Shaders` is not a
    // workspace, but `Scripting-Asset` says the code editor's is where a `.wgsl`
    // belongs.
    let wi = [
        tab.kind.layout_name(),
        tab.kind
            .asset_layout_name()
            .and_then(|l| l.strip_suffix("-Asset")),
    ]
    .into_iter()
    .flatten()
    .find_map(|name| layouts.layouts.iter().position(|(n, _)| n == name));
    if let Some(wi) = wi {
        apply_workspace(wi, &mut layouts, &mut dock, &mut dirty);
    }

    // A script or shader has no panel that reads `EditorContext` — the code
    // editor keeps its own list of open files and only ever hears about one
    // through `OpenCodeEditorFile`. Asking again for a file it already holds
    // just focuses that tab, so this is the same move `open_asset_tab` makes,
    // on the route it doesn't cover: moving between two documents already open.
    //
    // Revealing the panel belongs here rather than at the asset browser's
    // double-click, because it has to happen *after* the workspace switch
    // above — done there, the code editor was added to the layout we were on
    // the way out of.
    if matches!(
        tab.kind,
        renzora_ui::DocTabKind::Script | renzora_ui::DocTabKind::Shader
    ) {
        if let (Some(rel), Some(project)) = (tab.scene_path.as_ref(), project) {
            commands.insert_resource(renzora::core::OpenCodeEditorFile {
                path: project.resolve_path(rel),
            });
        }
        // Dirty either way: `focus_or_add_panel` returns false when the panel
        // was already there, but it still moved that leaf's active tab, and the
        // dock only repaints when flagged.
        dock.tree.focus_or_add_panel("code_editor");
        dirty.0 = true;
    }
}

/// The other direction: switching workspace brings that workspace's document
/// forward. Pick the Materials workspace off the ribbon and the material you
/// were last editing is the active tab again, with its graph loaded — rather
/// than the Materials layout sitting there showing whatever the scene tab you
/// were on happens to select.
///
/// Nothing happens when the active tab already belongs to the workspace being
/// switched to, which is what keeps this from fighting
/// [`sync_workspace_to_active_doc`]: a tab click switches the workspace, this
/// system sees that change, finds the tab that caused it already in place, and
/// stops. That check is on the *active tab* rather than on the MRU stack
/// deliberately — it holds whichever of the two systems runs first in a frame,
/// where "is the MRU top fresh yet" would not.
///
/// A workspace no open document maps to (Debug, Hub, Animation) leaves the
/// active tab alone: there is nothing there to bring forward, and stealing the
/// tab strip's selection to show something unrelated would be worse than
/// leaving it.
fn sync_active_doc_to_workspace(
    layouts: Res<ShellLayouts>,
    state: Option<ResMut<renzora_ui::DocumentTabState>>,
    mru: Res<DocTabMru>,
    mut last: Local<Option<usize>>,
    mut commands: Commands,
) {
    let Some(mut state) = state else { return };
    if *last == Some(layouts.active) {
        return;
    }
    *last = Some(layouts.active);
    let Some((name, _)) = layouts.layouts.get(layouts.active) else {
        return;
    };
    if state
        .active_tab()
        .is_some_and(|t| kind_in_workspace(t.kind, name))
    {
        return;
    }
    // Most recent first; falling back to display order for a workspace you have
    // documents in but have never been to this session (restored tabs, say).
    let idx = mru
        .0
        .iter()
        .rev()
        .find_map(|id| {
            state
                .tabs
                .iter()
                .position(|t| t.id == *id && kind_in_workspace(t.kind, name))
        })
        .or_else(|| {
            state
                .tabs
                .iter()
                .position(|t| kind_in_workspace(t.kind, name))
        });
    let Some(idx) = idx else { return };
    // Through `activate_tab` + `TabSwitchRequest` like every other switch, so a
    // scene→scene move still swaps the live scene for the incoming tab's buffer.
    if let Some((old_id, new_id)) = state.activate_tab(idx) {
        commands.insert_resource(renzora::TabSwitchRequest {
            old_tab_id: old_id,
            new_tab_id: new_id,
        });
    }
}

fn doc_tab_click(
    mut commands: Commands,
    q: Query<(&Interaction, &DocTabClick), Changed<Interaction>>,
    state: Option<ResMut<renzora_ui::DocumentTabState>>,
    rename: Res<DocTabRename>,
) {
    let Some(mut state) = state else { return };
    for (interaction, click) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        // While this tab is being renamed its edit field owns clicks — a press
        // really landing in there must not re-activate the tab underneath.
        if rename.0 == Some(click.0) {
            continue;
        }
        let Some(idx) = state.tabs.iter().position(|t| t.id == click.0) else {
            continue;
        };
        // Activate the tab AND swap the scene/asset content it owns. Without
        // the `TabSwitchRequest`, clicking a tab only switched the dock layout
        // (a no-op for scene→scene) and the viewport kept the old scene —
        // `handle_tab_switch` is what caches the leaving tab + restores this
        // tab's buffered scene.
        if let Some((old_id, new_id)) = state.activate_tab(idx) {
            commands.insert_resource(renzora::TabSwitchRequest {
                old_tab_id: old_id,
                new_tab_id: new_id,
            });
        }
        // The workspace, the editor context and the code editor's focus all
        // follow from the active tab having changed, and
        // [`sync_workspace_to_active_doc`] does that for every route into a
        // document — this one, an asset-browser double-click, a close promoting
        // its neighbour. A copy of the layout switch lived here too and had
        // already drifted: it knew only `layout_name`, so a shader tab clicked
        // here went looking for a `Shaders` workspace that doesn't exist.
    }
}

/// Press-latch reorder for the document tabs, plus the double-click that opens an
/// inline rename: dragging a tab past a small threshold moves it in
/// [`DocumentTabState`] on release, while two quick clicks that *didn't* drag
/// start a rename. Mirrors [`ribbon_interact`].
///
/// The reorder is applied **once, on release** rather than live as the cursor
/// crosses each neighbour: every mutation of `DocumentTabState` is a project.toml
/// write (`persist_open_tabs`), and a live reorder would spend one per tab
/// crossed. The insertion marker is what makes that deferral invisible.
///
/// The double-click lives here rather than in [`doc_tab_click`] for the same
/// reason it keys off the *release*: this is the only place that knows whether
/// the press in between turned into a drag. Arming the rename from presses alone
/// would fire it on the click that follows a reorder.
#[allow(clippy::too_many_arguments)]
fn doc_tab_drag(
    mut drag: ResMut<DocTabDrag>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    time: Res<Time>,
    mut rename: ResMut<DocTabRename>,
    pressed: Query<(&DocTabItem, &Interaction)>,
    items: Query<(&DocTabItem, &RelativeCursorPosition, &Visibility)>,
    mut nodes: Query<&mut Node>,
    state: Option<ResMut<renzora_ui::DocumentTabState>>,
    mut last_click: Local<Option<(u64, f64)>>,
) {
    let hide_markers = |items: &Query<(&DocTabItem, &RelativeCursorPosition, &Visibility)>,
                        nodes: &mut Query<&mut Node>| {
        for (it, _, _) in items {
            if let Ok(mut n) = nodes.get_mut(it.marker) {
                if n.display != Display::None {
                    n.display = Display::None;
                }
            }
        }
    };

    // Don't drag while a tab is being renamed — the press belongs to the field.
    if rename.0.is_some() {
        drag.0 = None;
        hide_markers(&items, &mut nodes);
        return;
    }
    let Some(mut state) = state else {
        drag.0 = None;
        hide_markers(&items, &mut nodes);
        return;
    };
    let cursor = windows.iter().next().and_then(|w| w.cursor_position());

    if drag.0.is_none() && mouse.just_pressed(MouseButton::Left) {
        if let Some(cur) = cursor {
            for (item, interaction) in &pressed {
                if *interaction == Interaction::Pressed {
                    let from = state.tabs.iter().position(|t| t.id == item.id).unwrap_or(0);
                    drag.0 = Some(DocTabDragState {
                        id: item.id,
                        start_cursor: cur,
                        active: false,
                        target: from,
                    });
                    break;
                }
            }
        }
    }

    if let (Some(st), Some(cur)) = (drag.0.as_mut(), cursor) {
        if (cur - st.start_cursor).length() > 5.0 {
            st.active = true;
        }
    }

    // Track the insertion slot under the cursor and show the matching edge
    // marker: the cursor in a tab's left half inserts before it, right half
    // after it. Folded tabs never report `cursor_over`, so they're skipped for
    // free — a drag can only land among the tabs actually on screen.
    match drag.0.as_mut() {
        Some(st) if st.active => {
            let mut shown: Option<(Entity, bool)> = None;
            for (it, rcp, vis) in &items {
                // A tab still being measured out of the flow sits at its static
                // position and would hit-test over a real one — see the strip's
                // `probe_new_item`. It isn't on screen; it can't be a drop target.
                if !rcp.cursor_over || *vis == Visibility::Hidden {
                    continue;
                }
                let Some(idx) = state.tabs.iter().position(|t| t.id == it.id) else {
                    continue;
                };
                let before = rcp.normalized.is_none_or(|n| n.x < 0.0);
                st.target = if before { idx } else { idx + 1 };
                shown = Some((it.marker, !before));
                break;
            }
            hide_markers(&items, &mut nodes);
            if let Some((marker, right)) = shown {
                if let Ok(mut n) = nodes.get_mut(marker) {
                    n.display = Display::Flex;
                    if right {
                        n.left = Val::Auto;
                        n.right = Val::Px(-2.0);
                    } else {
                        n.left = Val::Px(-2.0);
                        n.right = Val::Auto;
                    }
                }
            }
        }
        _ => hide_markers(&items, &mut nodes),
    }

    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    hide_markers(&items, &mut nodes);
    let Some(st) = drag.0.take() else { return };
    if !st.active {
        // A click, not a drag: the second one within the double-click window
        // opens the inline rename. The tab is already active from the press.
        let now = time.elapsed_secs_f64();
        if last_click.is_some_and(|(id, t)| id == st.id && now - t < 0.4) {
            *last_click = None;
            rename.0 = Some(st.id);
        } else {
            *last_click = Some((st.id, now));
        }
        return;
    }
    // A reorder invalidates the click that started it, so the next click on the
    // tab you just moved isn't read as the second half of a double-click.
    *last_click = None;
    let Some(from) = state.tabs.iter().position(|t| t.id == st.id) else {
        return;
    };
    // `reorder` takes an insertion slot in the *pre-removal* list, so both the
    // tab's own slot and the one just past it are no-ops.
    let to = st.target.min(state.tabs.len());
    if to != from && to != from + 1 {
        state.reorder(from, to);
    }
}

/// Start carrying a document tab that has folded into the strip's `»` menu,
/// from the drag the menu row hands over. Born active: the press that started it
/// was inside the menu, so there's no click/drag ambiguity left to resolve, and
/// no strip position to measure the threshold from.
fn start_doc_tab_drag(world: &mut World, id: u64) {
    let from = world
        .get_resource::<renzora_ui::DocumentTabState>()
        .and_then(|s| s.tabs.iter().position(|t| t.id == id))
        .unwrap_or(0);
    if let Some(mut drag) = world.get_resource_mut::<DocTabDrag>() {
        drag.0 = Some(DocTabDragState {
            id,
            start_cursor: Vec2::ZERO,
            active: true,
            target: from,
        });
    }
}

/// Auto-focus the document-tab rename field the frame it spawns, with the whole
/// name selected the way an OS rename does — a double-click means "replace this",
/// not "put a caret somewhere in it".
fn doc_focus_rename(mut q: Query<&mut EmberTextInput, Added<DocTabRenameInput>>) {
    for mut inp in &mut q {
        inp.focused = true;
        inp.select_all = true;
    }
}

/// Commit (Enter / click-away) or cancel (Escape) the active document-tab rename.
///
/// Commit-on-blur waits until the field has actually held focus: it's spawned by
/// the keyed-list rebuild a frame or two after [`DocTabRename`] is set, so "no
/// field yet" must not read as "gone", and the double-click that opened the
/// rename must not immediately close it again.
fn doc_rename_commit(
    mut rename: ResMut<DocTabRename>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut inputs: Query<(
        &mut EmberTextInput,
        &RelativeCursorPosition,
        &DocTabRenameInput,
    )>,
    mut commands: Commands,
    mut had_focus: Local<bool>,
) {
    let Some(id) = rename.0 else {
        *had_focus = false;
        return;
    };
    if keys.just_pressed(KeyCode::Escape) {
        rename.0 = None;
        *had_focus = false;
        return;
    }
    let Some((mut inp, rcp, _)) = inputs.iter_mut().find(|(_, _, r)| r.0 == id) else {
        return;
    };
    // A click inside the field (to move the caret) must keep it editing; the
    // strip's own click handling can otherwise steal focus the instant you click.
    if mouse.just_pressed(MouseButton::Left) && rcp.cursor_over && !inp.focused {
        inp.focused = true;
    }
    if inp.focused {
        *had_focus = true;
    }
    if !*had_focus {
        return;
    }
    let enter = keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter);
    let clicked_away = mouse.just_pressed(MouseButton::Left) && !rcp.cursor_over;
    if !enter && !clicked_away {
        return;
    }
    let new: String = inp.value.replace('\n', "").trim().to_string();
    rename.0 = None;
    *had_focus = false;
    if new.is_empty() {
        return;
    }
    commands.queue(move |world: &mut World| rename_doc_tab(world, id, &new));
}

/// Apply a document-tab rename.
///
/// A tab with a file behind it is renamed **on disk**. Its label is that file's
/// stem and nothing else — `editor_open_tabs` persists only paths and kinds, and
/// a reopened tab takes its name from the path again — so a label-only rename
/// would silently undo itself on the next project load. The move is announced
/// via [`renzora::AssetPathChanged`], the same event the asset browser fires, so
/// every holder of the old path (this tab included, through
/// [`doc_tabs_follow_asset_path`]) is patched by one code path.
///
/// An unsaved tab has no file, so there the label really is all there is.
fn rename_doc_tab(world: &mut World, id: u64, new_name: &str) {
    let old_rel = world
        .get_resource::<renzora_ui::DocumentTabState>()
        .and_then(|s| s.tabs.iter().find(|t| t.id == id))
        .map(|t| t.scene_path.clone());
    let Some(old_rel) = old_rel else { return };
    let Some(old_rel) = old_rel else {
        // No path yet — this is a `+` tab ("Untitled Scene"). Naming it used to
        // relabel the tab and nothing else, so the scene the user had just built
        // and named still existed nowhere on disk, with no prompt to say so.
        // Naming an untitled scene now creates it, which also matches what
        // renaming a *saved* tab does: the tab label IS the file name.
        name_untitled_scene(world, id, new_name);
        return;
    };

    let Some(old_abs) = world
        .get_resource::<renzora::CurrentProject>()
        .map(|p| p.resolve_path(&old_rel))
    else {
        return;
    };
    // Keep the extension: the label the user edited never had one.
    let file_name = match old_abs.extension().and_then(|e| e.to_str()) {
        Some(ext) => format!("{new_name}.{ext}"),
        None => new_name.to_string(),
    };
    let new_abs = old_abs.with_file_name(&file_name);
    if new_abs == old_abs {
        return;
    }
    if new_abs.exists() {
        warn!("[tabs] rename refused — '{}' already exists", new_abs.display());
        return;
    }
    if let Err(e) = std::fs::rename(&old_abs, &new_abs) {
        warn!("[tabs] failed to rename '{}': {e}", old_abs.display());
        return;
    }
    // Derived from the stored path rather than re-deriving it from the new
    // absolute one: `make_relative` canonicalizes, and the tab's path is already
    // project-relative with forward slashes.
    let new_rel = match old_rel.rfind('/') {
        Some(i) => format!("{}/{}", &old_rel[..i], file_name),
        None => file_name,
    };
    world.trigger(renzora::AssetPathChanged {
        old: old_rel,
        new: new_rel,
        is_dir: false,
    });
}

/// Give a never-saved scene tab a name, and create the file to match.
///
/// Only the **active** tab writes a file: an inactive tab's contents live in
/// `SceneTabBuffers`, not in the world, so saving here would write whatever
/// scene happens to be open into somebody else's file. An inactive tab just
/// takes the label and stays untitled until it is focused and saved.
fn name_untitled_scene(world: &mut World, id: u64, new_name: &str) {
    let file_stem: String = new_name
        .trim()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    if file_stem.is_empty() {
        return;
    }

    let (is_active_scene, _) = world
        .get_resource::<renzora_ui::DocumentTabState>()
        .and_then(|s| {
            let active_id = s.tabs.get(s.active_tab).map(|t| t.id);
            s.tabs
                .iter()
                .find(|t| t.id == id)
                .map(|t| {
                    (
                        active_id == Some(id) && t.kind == renzora_ui::DocTabKind::Scene,
                        (),
                    )
                })
        })
        .unwrap_or((false, ()));

    // Relabel regardless; only the active scene tab also gains a file.
    if let Some(mut state) = world.get_resource_mut::<renzora_ui::DocumentTabState>() {
        if let Some(tab) = state.tabs.iter_mut().find(|t| t.id == id) {
            tab.name = new_name.to_string();
        }
    }
    if !is_active_scene {
        return;
    }

    let rel = format!("scenes/{file_stem}.bsn");
    let abs = match world.get_resource::<renzora::CurrentProject>() {
        Some(p) => p.resolve_path(&rel),
        None => return,
    };
    if abs.exists() {
        warn!("[tabs] '{}' already exists — scene not created", abs.display());
        renzora::core::console_log::console_error(
            "Scene",
            format!("A scene named '{file_stem}' already exists"),
        );
        return;
    }
    if let Some(dir) = abs.parent() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            warn!("[tabs] failed to create {}: {e}", dir.display());
            return;
        }
    }

    // Point the tab at the new path, then let the normal save path write it —
    // `save_scene_system` sees a scene tab WITH a path and targets exactly this
    // file, so there is one scene-writing code path rather than two.
    if let Some(mut state) = world.get_resource_mut::<renzora_ui::DocumentTabState>() {
        if let Some(tab) = state.tabs.iter_mut().find(|t| t.id == id) {
            tab.scene_path = Some(rel.clone());
        }
    }
    world.insert_resource(renzora::core::SaveSceneRequested);
    renzora::core::console_log::console_success("Scene", format!("Created {rel}"));
}

/// Follow a renamed or moved asset in the open document tabs, so a rename from
/// anywhere — this strip, the asset browser, a folder move — leaves every open
/// tab pointing at the file it actually has open rather than at a dead path.
fn doc_tabs_follow_asset_path(
    trigger: On<renzora::AssetPathChanged>,
    state: Option<ResMut<renzora_ui::DocumentTabState>>,
    context: Option<ResMut<renzora_ui::EditorContext>>,
) {
    let ev = trigger.event();
    if let Some(mut state) = state {
        for tab in state.tabs.iter_mut() {
            let Some(new_path) = tab.scene_path.as_ref().and_then(|p| ev.rewrite(p)) else {
                continue;
            };
            if let Some(stem) = std::path::Path::new(&new_path)
                .file_stem()
                .and_then(|s| s.to_str())
            {
                tab.name = stem.to_string();
            }
            tab.scene_path = Some(new_path);
        }
    }
    // Asset-mode panels load straight from this path, so it has to move too.
    if let Some(mut context) = context {
        if let renzora_ui::EditorContext::Asset { path, .. } = &mut *context {
            if let Some(new_path) = ev.rewrite(path) {
                *path = new_path;
            }
        }
    }
}

/// Close a document tab by id and, if it was the active tab, fire a
/// [`TabSwitchRequest`] so the viewport follows to the newly-active tab.
/// `close_tab` only moves the active index — it never swaps scene content — so
/// without this the old scene would linger under a different active tab.
fn close_doc_tab_by_id(
    state: &mut renzora_ui::DocumentTabState,
    id: u64,
    commands: &mut Commands,
) {
    let Some(idx) = state.tabs.iter().position(|t| t.id == id) else {
        return;
    };
    let was_active = state.active_tab == idx;
    // The active tab's id before the close — used as `old` for the switch so
    // `handle_tab_switch` despawns the current scene before loading the next.
    let prev_active_id = state.active_tab_id();
    if state.close_tab(idx).is_some() && was_active {
        if let (Some(old), Some(new)) = (prev_active_id, state.active_tab_id()) {
            if old != new {
                commands.insert_resource(renzora::TabSwitchRequest {
                    old_tab_id: old,
                    new_tab_id: new,
                });
            }
        }
    }
}

/// Click a document tab's × → close it. A tab with unsaved changes opens a
/// save-confirmation prompt instead of closing outright (see
/// [`process_tab_close_request`]); clean tabs close immediately. The model
/// refuses to close the last scene / last tab regardless.
fn doc_tab_close(
    q: Query<(&Interaction, &DocTabClose), Changed<Interaction>>,
    state: Option<ResMut<renzora_ui::DocumentTabState>>,
    prompt_open: Query<(), With<CloseTabPromptRoot>>,
    mut commands: Commands,
) {
    let Some(mut state) = state else { return };
    // A prompt is already up — ignore clicks until it's resolved.
    if !prompt_open.is_empty() {
        return;
    }
    for (interaction, close) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(idx) = state.tabs.iter().position(|t| t.id == close.0) else {
            continue;
        };
        if state.tabs[idx].is_modified {
            // Defer to the prompt flow; it activates the tab and asks the user.
            commands.insert_resource(TabCloseRequest { id: close.0 });
        } else {
            close_doc_tab_by_id(&mut state, close.0, &mut commands);
        }
    }
}

/// The top bar's gear — opens (or closes) the Settings panel.
///
/// The bar carried a gear button once before; it was dropped when the menus were
/// folded into the hamburger, which left the hamburger's own **Settings** row as
/// the only way in. That row stays — this is the one-click path back, for the
/// thing the menu's own comment admits is "reached far too often".
fn settings_button(commands: &mut Commands) -> Entity {
    let gear = glyph(commands, "gear", text_muted(), 14.0);
    commands.entity(gear).insert((
        Node {
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            padding: UiRect::axes(Val::Px(5.0), Val::Px(3.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        Interaction::default(),
        SettingsBtn,
        renzora_ember::widgets::HoverTooltip::new(renzora::lang::t("common.settings")),
        renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
    ));
    gear
}

/// Marks a plugin-contributed top-bar button with the id it reports when
/// pressed.
#[derive(Component)]
struct ShellActionBtn(&'static str);

/// The row of plugin-contributed top-bar buttons.
///
/// Built once with the chrome, from whatever is in
/// [`renzora::ShellActionRegistry`] at that moment — which is every plugin's
/// registration, since plugins are added during `App` assembly and the chrome is
/// built after the splash. A plugin that registers later gets its button on the
/// next chrome rebuild (a theme or layout change), which is the same deal
/// status-bar items and panels get.
fn build_shell_actions(commands: &mut Commands) -> Entity {
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(2.0),
                ..default()
            },
            Name::new("top-bar-actions"),
        ))
        .id();
    commands.queue(move |world: &mut World| {
        let items: Vec<(&'static str, &'static str, String, i32)> = world
            .get_resource::<renzora::ShellActionRegistry>()
            .map(|reg| {
                let mut v: Vec<_> = reg
                    .items
                    .iter()
                    .map(|i| (i.id, i.icon, (i.tooltip)(), i.order))
                    .collect();
                v.sort_by_key(|i| i.3);
                v
            })
            .unwrap_or_default();
        if items.is_empty() {
            return;
        }
        let Some(fonts) = world.get_resource::<EmberFonts>().cloned() else {
            return;
        };
        let mut queue = bevy::ecs::world::CommandQueue::default();
        {
            let mut commands = Commands::new(&mut queue, world);
            let kids: Vec<Entity> = items
                .into_iter()
                .map(|(id, icon, tooltip, _)| {
                    let btn = renzora_ember::font::icon_text(
                        &mut commands,
                        &fonts.phosphor,
                        icon,
                        text_muted(),
                        14.0,
                    );
                    commands.entity(btn).insert((
                        Node {
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            padding: UiRect::axes(Val::Px(5.0), Val::Px(3.0)),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                            ..default()
                        },
                        Interaction::default(),
                        ShellActionBtn(id),
                        renzora_ember::widgets::HoverTooltip::new(tooltip),
                        renzora_ember::cursor_icon::HoverCursor(
                            bevy::window::SystemCursorIcon::Pointer,
                        ),
                        Name::new(format!("top-action:{id}")),
                    ));
                    btn
                })
                .collect();
            commands.entity(row).add_children(&kids);
        }
        queue.apply(world);
    });
    row
}

/// Turn a press into a [`renzora::ShellActionInvoked`] for whoever registered
/// the id.
fn shell_action_press(
    q: Query<(&Interaction, &ShellActionBtn), Changed<Interaction>>,
    mut invoked: MessageWriter<renzora::ShellActionInvoked>,
) {
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed {
            invoked.write(renzora::ShellActionInvoked(btn.0));
        }
    }
}

/// Gear → toggle the Settings panel. Same toggle the hamburger's Settings row
/// runs, so clicking either while it's open closes it.
fn settings_btn_click(
    q: Query<&Interaction, (With<SettingsBtn>, Changed<Interaction>)>,
    settings: Option<ResMut<renzora_editor_framework::EditorSettings>>,
) {
    let Some(mut settings) = settings else { return };
    if q.iter().any(|i| *i == Interaction::Pressed) {
        settings.show_settings = !settings.show_settings;
    }
}

/// A full-height flex row used as a top-bar zone (left / center / right).
///
/// A growing zone gets `flex_basis: 0` — without it flexbox hands out only the
/// *leftover* space equally, so the two side zones end up as wide as their own
/// content plus a share, and the "centered" middle zone sits wherever the
/// heavier side pushes it. From a zero basis both sides are dealt identical
/// widths whatever they hold, which is what actually centers the ribbon in the
/// window. They shrink rather than grow past that half.
///
/// `clip` is separate from `grow` because the two wants can conflict: clipping
/// is what stops a zone's contents spilling over the ribbon, but it also cuts
/// off anything a child hangs *outside* the bar — a dropdown panel, a tooltip.
/// A zone holding a fixed, small set of buttons has nothing to contain and
/// should not clip.
fn zone(
    commands: &mut Commands,
    name: &str,
    justify: JustifyContent,
    gap: f32,
    grow: f32,
    clip: bool,
) -> Entity {
    let growing = grow > 0.0;
    commands
        .spawn((
            Node {
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: justify,
                column_gap: Val::Px(gap),
                flex_grow: grow,
                flex_basis: if growing { Val::Px(0.0) } else { Val::Auto },
                min_width: Val::Px(0.0),
                overflow: if clip { Overflow::clip() } else { Overflow::visible() },
                ..default()
            },
            // Structural container — let clicks fall through to the bar's drag
            // handle behind it (interactive children still block on their own).
            bevy::ui::FocusPolicy::Pass,
            Name::new(name.to_string()),
        ))
        .id()
}



// ── Top-bar menus (hamburger → File / Edit / View / Help) ────────────────────

#[derive(Clone, Copy, PartialEq)]
enum TopMenuKind {
    /// The hamburger: one dropdown whose rows are the File/Edit/View/Help
    /// submenus. The four kinds below are no longer top-bar titles of their own
    /// — they only name the item list each submenu is filled with.
    Main,
    File,
    Edit,
    View,
    Help,
    Account,
}

#[derive(Component)]
struct TopMenu(TopMenuKind);

/// The currently-open top menu (so hovering a sibling switches to it, and a
/// re-click toggles it closed). Cleared by [`top_menu_sync`] once dismissed.
#[derive(Resource, Default)]
struct OpenTopMenu {
    menu: Option<Entity>,
    kind: Option<TopMenuKind>,
}

/// The hamburger that replaced the File/Edit/View/Help titles: one top-bar
/// button opening a single dropdown, with those four now submenu rows inside it.
///
/// It's icon-only on purpose — the point of collapsing four titles into one was
/// to give the left zone back to the account name and the bell, so a label here
/// would spend most of the width we just reclaimed.
fn hamburger_menu_item(commands: &mut Commands) -> Entity {
    let item = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(7.0), Val::Px(4.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            TopMenu(TopMenuKind::Main),
            Name::new("menu:main"),
        ))
        .id();
    renzora_ember::reactive::tracked::bind_bg(commands, item, move |w| match w.get::<Interaction>(item) {
        Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(renzora_ember::theme::hover_bg()),
        _ => Color::NONE,
    });
    let icon = glyph(commands, "list", text_muted(), 15.0);
    commands.entity(item).add_child(icon);
    item
}

/// The top bar's "Update available" chip: an accent-tinted pill that appears
/// when an engine update is waiting and opens the Software Update overlay.
///
/// Built unconditionally and hidden reactively rather than spawned on demand:
/// the top bar is assembled once, and a `bind_display` costs nothing next to
/// rebuilding the bar whenever a background check finishes.
fn build_update_chip(commands: &mut Commands, font: &bevy::text::FontSource) -> Entity {
    let chip = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                border_radius: BorderRadius::all(Val::Px(10.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            UpdateChipBtn,
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("update-chip"),
        ))
        .id();
    renzora_ember::reactive::tracked::bind_display(commands, chip, |w| {
        w.get_resource::<renzora::core::UpdateAvailable>().is_some()
    });
    renzora_ember::reactive::tracked::bind_bg(commands, chip, move |w| {
        match w.get::<Interaction>(chip) {
            Some(Interaction::Hovered) | Some(Interaction::Pressed) => {
                Color::srgba(0.36, 0.65, 1.0, 0.34)
            }
            _ => Color::srgba(0.36, 0.65, 1.0, 0.20),
        }
    });
    let ic = glyph(commands, "arrow-circle-up", text_primary(), 13.0);
    commands.entity(ic).insert(bevy::ui::FocusPolicy::Pass);
    let label = commands
        .spawn((
            Text::new(String::new()),
            ui_font(font, 11.0),
            TextColor(rgb(text_primary())),
            bevy::ui::FocusPolicy::Pass,
            bevy::text::TextLayout::no_wrap(),
        ))
        .id();
    // Deliberately does not name the version: a bare tag in the top bar reads as
    // the version you're *running*, not one you could move to. The overlay the
    // chip opens spells out which release it is.
    renzora_ember::reactive::tracked::bind_text(commands, label, |w| {
        match w.get_resource::<renzora::core::UpdateAvailable>() {
            Some(_) => renzora::lang::t("menu.help.update_new"),
            None => String::new(),
        }
    });
    commands.entity(chip).add_children(&[ic, label]);
    chip
}

/// Click the update chip → open the Software Update overlay.
fn update_chip_click(
    q: Query<&Interaction, (With<UpdateChipBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.insert_resource(renzora::core::UpdateRequested);
    }
}

/// Spawn a top-menu dropdown anchored at `pos` and return its root.
fn spawn_top_menu(
    commands: &mut Commands,
    fonts: &EmberFonts,
    kind: TopMenuKind,
    pos: Vec2,
    account: Option<&str>,
    update_tag: Option<&str>,
) -> Entity {
    let root = renzora_ember::widgets::screen_menu(commands, pos.x, pos.y);
    let kids = build_menu_items(commands, fonts, kind, account, update_tag);
    commands.entity(root).add_children(&kids);
    root
}

/// The signed-in username, if any — read per menu-open so the hamburger's
/// account row shows the current name without a reactive binding.
fn account_name(bridge: &Option<Res<renzora::core::AuthBridge>>) -> Option<String> {
    bridge.as_ref().and_then(|b| b.signed_in_username.clone())
}

/// Click a top-bar title → open its dropdown (anchored under the button), or
/// re-click the open one to close it.
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
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    fonts: Option<Res<EmberFonts>>,
    bridge: Option<Res<renzora::core::AuthBridge>>,
    update: Option<Res<renzora::core::UpdateAvailable>>,
    mut open: ResMut<OpenTopMenu>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else {
        return;
    };
    let account = account_name(&bridge);
    let update_tag = update.as_ref().map(|u| u.0.clone());
    for (interaction, menu, rcp, cn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Some(e) = open.menu.take() {
            commands.entity(e).try_despawn();
        }
        // Re-clicking the already-open menu just closes it.
        if open.kind == Some(menu.0) {
            open.kind = None;
            continue;
        }
        let Some(pos) = anchor_below(&windows, rcp, cn) else {
            open.kind = None;
            continue;
        };
        open.menu = Some(spawn_top_menu(&mut commands, &fonts, menu.0, pos, account.as_deref(), update_tag.as_deref()));
        open.kind = Some(menu.0);
    }
}

/// While a top menu is open, hovering a *different* title switches to it without
/// a click — standard menu-bar behavior.
fn top_menu_hover(
    q: Query<(
        &Interaction,
        &TopMenu,
        &bevy::ui::RelativeCursorPosition,
        &bevy::ui::ComputedNode,
    )>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    fonts: Option<Res<EmberFonts>>,
    bridge: Option<Res<renzora::core::AuthBridge>>,
    update: Option<Res<renzora::core::UpdateAvailable>>,
    mut open: ResMut<OpenTopMenu>,
    mut commands: Commands,
) {
    let Some(open_kind) = open.kind else { return };
    let Some(fonts) = fonts else { return };
    let account = account_name(&bridge);
    let update_tag = update.as_ref().map(|u| u.0.clone());
    for (interaction, menu, rcp, cn) in &q {
        if *interaction == Interaction::Hovered && menu.0 != open_kind {
            if let Some(e) = open.menu.take() {
                commands.entity(e).try_despawn();
            }
            let Some(pos) = anchor_below(&windows, rcp, cn) else {
                open.kind = None;
                return;
            };
            open.menu = Some(spawn_top_menu(&mut commands, &fonts, menu.0, pos, account.as_deref(), update_tag.as_deref()));
            open.kind = Some(menu.0);
            return;
        }
    }
}

/// Forget the open menu once it's been dismissed (click-outside / item click,
/// handled by ember), so the next hover/click starts fresh.
fn top_menu_sync(
    menus: Query<(), With<renzora_ember::widgets::ScreenMenu>>,
    mut open: ResMut<OpenTopMenu>,
) {
    if let Some(e) = open.menu {
        if menus.get(e).is_err() {
            open.menu = None;
            open.kind = None;
        }
    }
}

/// The bottom-left of a node in logical window px, derived from the cursor + the
/// node's normalized cursor position (scale-invariant; avoids UI `GlobalTransform`
/// coordinate ambiguity). Used to anchor button dropdowns just under the button.
fn anchor_below(
    windows: &Query<&Window, With<bevy::window::PrimaryWindow>>,
    rcp: &bevy::ui::RelativeCursorPosition,
    cn: &bevy::ui::ComputedNode,
) -> Option<Vec2> {
    let cursor = windows.iter().next().and_then(|w| w.cursor_position())?;
    let size = cn.size() * cn.inverse_scale_factor();
    let norm = rcp.normalized.unwrap_or(Vec2::ZERO);
    let top_left = cursor - (norm + Vec2::splat(0.5)) * size;
    Some(Vec2::new(top_left.x, top_left.y + size.y + 2.0))
}

/// Build one menu's rows. `account` is the signed-in username (`None` = signed
/// out) — the menu needs the name itself now, not just the fact of being signed
/// in, because the hamburger's first row *is* the username.
fn build_menu_items(
    commands: &mut Commands,
    fonts: &EmberFonts,
    kind: TopMenuKind,
    account: Option<&str>,
    // Release tag of a pending engine update, when `renzora_update`'s background
    // check found one. Read per menu-open like `account`, so Help names the
    // version instead of making you go and look.
    update_tag: Option<&str>,
) -> Vec<Entity> {
    use renzora_ember::widgets::{menu_item, menu_sep, menu_submenu};
    match kind {
        // The hamburger's own dropdown: the account, then four submenu rows,
        // each filled by recursing into the item list that used to be its own
        // top-bar title.
        TopMenuKind::Main => {
            let mut rows: Vec<Entity> = Vec::new();
            // The signed-in username, which lost its top-bar slot to the
            // document tabs. Signed out there's nothing to nest, so it's a plain
            // "Sign In" row rather than a submenu holding one item. No reactive
            // binding needed either way: the menu is rebuilt on every open, so
            // the label is read fresh each time.
            if let Some(name) = account {
                let (row, content) = menu_submenu(commands, fonts, "user", name);
                let kids = build_menu_items(commands, fonts, TopMenuKind::Account, account, update_tag);
                commands.entity(content).add_children(&kids);
                rows.push(row);
            } else {
                rows.extend(build_menu_items(commands, fonts, TopMenuKind::Account, account, update_tag));
            }
            rows.push(menu_sep(commands));
            rows.extend(
                [
                    ("file", renzora::lang::t("menu.file"), TopMenuKind::File),
                    ("pencil-simple", renzora::lang::t("menu.edit"), TopMenuKind::Edit),
                    ("eye", renzora::lang::t("menu.view"), TopMenuKind::View),
                    ("question", renzora::lang::t("menu.help"), TopMenuKind::Help),
                ]
                .into_iter()
                .map(|(icon, label, sub)| {
                    let (row, content) = menu_submenu(commands, fonts, icon, &label);
                    let kids = build_menu_items(commands, fonts, sub, account, update_tag);
                    commands.entity(content).add_children(&kids);
                    row
                }),
            );
            // Export and Settings are top-level rather than buried at the bottom
            // of File. Settings took the gear button's place when that left the
            // top bar, and both are reached far too often to sit two hovers deep
            // — exporting is the end of every project, not a File housekeeping
            // chore.
            rows.push(menu_sep(commands));
            rows.push(menu_item(
                commands,
                fonts,
                "package",
                &renzora::lang::t("menu.file.export_project"),
                |w| {
                    w.insert_resource(renzora::core::ExportRequested);
                },
            ));
            rows.push(menu_item(
                commands,
                fonts,
                "gear",
                &renzora::lang::t("common.settings"),
                |w| {
                    if let Some(mut s) =
                        w.get_resource_mut::<renzora_editor_framework::EditorSettings>()
                    {
                        s.show_settings = !s.show_settings;
                    }
                },
            ));
            rows
        }
        TopMenuKind::Account => {
            if account.is_some() {
                vec![
                    // No Notifications row: notifications existed to announce
                    // feed, message and friend activity, all of which is gone.
                    menu_item(commands, fonts, "books", &renzora::lang::t("menu.account.my_library"), |w| {
                        if let Some(mut dock) = w.get_resource_mut::<Dock>() {
                            dock.tree.focus_or_add_panel("hub_library");
                        }
                        if let Some(mut d) = w.get_resource_mut::<DockDirty>() {
                            d.0 = true;
                        }
                    }),
                    menu_sep(commands),
                    menu_item(commands, fonts, "sign-out", &renzora::lang::t("auth.sign_out"), |w| {
                        w.insert_resource(renzora::core::AuthSignOutRequest);
                    }),
                ]
            } else {
                vec![menu_item(commands, fonts, "sign-in", &renzora::lang::t("auth.sign_in"), |w| {
                    w.insert_resource(renzora::core::AuthToggleWindowRequest);
                })]
            }
        }
        TopMenuKind::File => vec![
            menu_item(commands, fonts, "folder-plus", &renzora::lang::t("menu.file.new_project"), |w| {
                renzora_editor_framework::handle_new_project(w)
            }),
            menu_item(commands, fonts, "folder-open", &renzora::lang::t("menu.file.open_project"), |w| {
                renzora_editor_framework::handle_open_project(w)
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "file-plus", &renzora::lang::t("menu.file.new_scene"), |w| {
                w.insert_resource(renzora::core::NewSceneRequested);
            }),
            menu_item(commands, fonts, "file", &renzora::lang::t("menu.file.open_scene"), |w| {
                w.insert_resource(renzora::core::OpenSceneRequested);
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "floppy-disk", &renzora::lang::t("common.save"), |w| {
                w.insert_resource(renzora::core::SaveSceneRequested);
            }),
            menu_item(commands, fonts, "floppy-disk-back", &renzora::lang::t_or("menu.file.save_as", "Save As…"), |w| {
                w.insert_resource(renzora::core::SaveAsSceneRequested);
            }),
            menu_sep(commands),
            // Same request the asset panel's Import button fires; renzora_import_ui
            // picks it up and opens the matching picker, then the import overlay.
            // No ImportTargetDir here, so assets land in the importer's default
            // folder. Two rows because no OS dialog picks files and folders at
            // once — see `renzora::core::ImportPick`.
            menu_item(commands, fonts, "file", &renzora::lang::t("assets.import_files"), |w| {
                w.insert_resource(renzora::core::ImportRequested(renzora::core::ImportPick::Files));
            }),
            menu_item(commands, fonts, "folder-open", &renzora::lang::t("assets.import_folder"), |w| {
                w.insert_resource(renzora::core::ImportRequested(renzora::core::ImportPick::Folder));
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "plug", &renzora::lang::t_or("menu.file.install_plugin", "Install Plugin…"), |w| {
                crate::plugin_install::open_install_dialog(w)
            }),
        ],
        TopMenuKind::Edit => vec![
            menu_item(commands, fonts, "arrow-u-up-left", &renzora::lang::t("common.undo"), |w| {
                let f = w.get_resource::<renzora_editor_framework::EditorActionHooks>().and_then(|h| h.undo);
                if let Some(f) = f {
                    f(w);
                }
            }),
            menu_item(commands, fonts, "arrow-u-up-right", &renzora::lang::t("common.redo"), |w| {
                let f = w.get_resource::<renzora_editor_framework::EditorActionHooks>().and_then(|h| h.redo);
                if let Some(f) = f {
                    f(w);
                }
            }),
        ],
        TopMenuKind::View => vec![
            menu_item(commands, fonts, "magnifying-glass-plus", &renzora::lang::t_or("menu.view.zoom_in", "Zoom In"), |w| {
                w.insert_resource(renzora::core::CameraViewRequest::ZoomIn);
            }),
            menu_item(commands, fonts, "magnifying-glass-minus", &renzora::lang::t_or("menu.view.zoom_out", "Zoom Out"), |w| {
                w.insert_resource(renzora::core::CameraViewRequest::ZoomOut);
            }),
            menu_item(commands, fonts, "magnifying-glass", &renzora::lang::t_or("menu.view.reset_zoom", "Reset Zoom"), |w| {
                w.insert_resource(renzora::core::CameraViewRequest::ResetZoom);
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "corners-out", &renzora::lang::t_or("menu.view.fit_all", "Fit All"), |w| {
                w.insert_resource(renzora::core::CameraViewRequest::FrameAll);
            }),
            menu_item(commands, fonts, "eye", &renzora::lang::t_or("menu.view.isolation_mode", "Isolation Mode"), |w| {
                let mut iso = w
                    .remove_resource::<renzora::core::IsolationMode>()
                    .unwrap_or_default();
                iso.active = !iso.active;
                w.insert_resource(iso);
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "layout", &renzora::lang::t("menu.window.reset_layout"), reset_layout_action),
            menu_item(commands, fonts, "browsers", &renzora::lang::t_or("menu.view.reset_workspace", "Reset Workspace"), reset_workspace_action),
            menu_item(commands, fonts, "rows", &renzora::lang::t_or("menu.view.reset_global_docks", "Reset Global Docks"), reset_global_docks_action),
        ],
        TopMenuKind::Help => vec![
            menu_item(commands, fonts, "graduation-cap", &renzora::lang::t_or("menu.help.tutorial", "Getting Started Tutorial"), |w| {
                w.insert_resource(renzora::core::TutorialRequested);
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "book-open", &renzora::lang::t("menu.help.documentation"), |_| {
                open_url("https://renzora.com/docs")
            }),
            menu_item(commands, fonts, "youtube-logo", &renzora::lang::t("menu.help.youtube"), |_| {
                open_url("https://youtube.com/@renzoragame")
            }),
            menu_item(commands, fonts, "discord-logo", &renzora::lang::t("menu.help.discord"), |_| {
                open_url("https://discord.gg/9UHUGUyDJv")
            }),
            menu_item(commands, fonts, "github-logo", &renzora::lang::t_or("menu.help.github", "GitHub"), |_| {
                open_url("https://github.com/renzora/engine")
            }),
            menu_sep(commands),
            // Names the pending version when there is one, so "am I out of
            // date?" is answered by the menu rather than by opening a dialog to
            // find out.
            menu_item(
                commands,
                fonts,
                "download-simple",
                &match update_tag {
                    Some(tag) => format!("{} {tag}", renzora::lang::t("menu.help.update_to")),
                    None => renzora::lang::t("menu.help.check_updates"),
                },
                |w| {
                    w.insert_resource(renzora::core::UpdateRequested);
                },
            ),
            menu_item(commands, fonts, "info", &renzora::lang::t_or("menu.help.about_engine", "About Renzora Engine"), |w| {
                w.insert_resource(crate::about::ShowAboutRequested);
            }),
        ],
    }
}

/// Reset the active workspace's dock tree to the **engine default** for that
/// workspace. The stored `ShellLayouts` entry holds the user's *edited* layout
/// (persisted to `~/.renzora/layout.json`), so resetting to it was a no-op —
/// we pull the pristine tree from [`dock::workspace_layouts`] instead, matched
/// by the active workspace's name, and overwrite both the live dock and the
/// stored layout so the reset sticks (and gets persisted).
///
/// Deliberately leaves the global bottom panel alone. It is not part of any
/// workspace ([`dock::scene_layout`]), so resetting a workspace has nothing to
/// say about it — see [`reset_global_docks_action`], which is the only thing
/// that does.
fn reset_layout_action(w: &mut World) {
    let active_name = w
        .get_resource::<ShellLayouts>()
        .and_then(|l| l.layouts.get(l.active).map(|(name, _)| name.clone()));
    let Some(active_name) = active_name else {
        return;
    };
    let Some(default_tree) = dock::workspace_layouts()
        .into_iter()
        .find(|(name, _)| *name == active_name)
        .map(|(_, t)| t)
    else {
        return;
    };
    if let Some(mut layouts) = w.get_resource_mut::<ShellLayouts>() {
        let active = layouts.active;
        if let Some(slot) = layouts.layouts.get_mut(active) {
            slot.1 = default_tree.clone();
        }
    }
    if let Some(mut dock) = w.get_resource_mut::<Dock>() {
        dock.tree = default_tree;
    }
    if let Some(mut d) = w.get_resource_mut::<DockDirty>() {
        d.0 = true;
    }
}

/// Reset the entire workspace ribbon to the engine defaults: discard any
/// user-added / removed / renamed / reordered workspaces and restore each
/// default workspace's pristine dock tree. Where [`reset_layout_action`] resets
/// only the active workspace's layout, this rebuilds the whole set (active back
/// to the first default), then flags a rebuild so the change persists.
///
/// The global bottom panel survives untouched, tab sets and all. It belongs to
/// the editor, not to a workspace, so someone restoring the shipped Scene /
/// Scripting / Debug arrangement has not asked to lose the panel set they built
/// alongside it. [`reset_global_docks_action`] is the separate, explicit way to
/// reset that.
fn reset_workspace_action(w: &mut World) {
    let defaults = dock::workspace_layouts();
    let Some(active_tree) = defaults.first().map(|(_, t)| t.clone()) else {
        return;
    };
    if let Some(mut layouts) = w.get_resource_mut::<ShellLayouts>() {
        layouts.layouts = defaults;
        layouts.active = 0;
    }
    if let Some(mut dock) = w.get_resource_mut::<Dock>() {
        dock.tree = active_tree;
    }
    if let Some(mut d) = w.get_resource_mut::<DockDirty>() {
        d.0 = true;
    }
}

/// Reset the global bottom panel: one set, named the default, holding
/// [`dock::DEFAULT_BOTTOM_TABS`], at the default height, opened.
///
/// This is the counterpart to the two workspace resets above — the panel is
/// global, so neither of them touches it and it needs a way back of its own.
/// It is also the escape hatch when the panel has been emptied *and* collapsed:
/// the collapsed strip stands in that state now (see
/// [`sync_collapsed_bottom_bar`]), but a user who has already lost it on an
/// older build needs one menu item that puts everything back.
///
/// Every set goes, not just the live one. "Reset" that left three
/// user-made sets in place would be a partial reset in the one direction that
/// matters: the panels the user is complaining about not seeing may be in any
/// of them. It opens the panel too, so the reset is visible rather than
/// something that has happened behind a closed strip.
fn reset_global_docks_action(w: &mut World) {
    let tree = dock::default_bottom_tree();
    if let Some(mut fixed) = w.get_resource_mut::<renzora_ember::dock::FixedDock>() {
        fixed.tree = tree.clone();
        fixed.dirty = true;
    }
    if let Some(mut sets) = w.get_resource_mut::<BottomPanelSets>() {
        sets.sets = vec![(default_panel_set_name(), tree)];
        sets.active = 0;
    }
    if let Some(mut bottom) = w.get_resource_mut::<BottomDock>() {
        bottom.height = dock::BOTTOM_DOCK_HEIGHT;
        bottom.mode = dock::BottomDockMode::default();
        bottom.open = true;
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
