//! `renzora_shell` — the bevy_ui-native editor shell.
//!
//! The editor's chrome (menu bar, ribbon, document tabs, status bar) plus the
//! wiring that drives the reusable [`renzora_ember`] dock. The dock itself —
//! splits, tabs, drag-docking — lives in `renzora_ember::dock`; the shell just
//! supplies the layout, the dock area, and editor-specific behavior.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition, UiGlobalTransform};

use renzora::NativePanelIds;
use renzora_ember::dock::{tab_pane, Dock, DockArea, DockDirty, DockLeaf, DockTab, TabPane};
use renzora_ember::font::{glyph, icon_item, icon_text, ui_font, EmberFonts};
use renzora_ember::widgets::{
    menu_item, scroll_area_keyed, screen_menu, text_input, EmberTextInput, Popup,
};
use renzora_ember::theme::{
    accent, divider, header_bg, placeholder, play_green, rgb, tab_active, text_muted, text_primary,
    window_bg,
};
use renzora_ember::EmberPlugin;

pub mod dock;
mod about;
mod plugin_install;

use dock::{ClosedBottom, DockTree};
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
        let (mut layouts, active, floating, mut closed_bottoms) = match dock::load_dock_layouts() {
            Some((mut saved, active, floating, closed)) => {
                for (name, tree) in defaults {
                    if !saved.iter().any(|(n, _)| n == &name) {
                        saved.push((name, tree));
                    }
                }
                let active = active.min(saved.len().saturating_sub(1));
                (saved, active, floating, closed)
            }
            None => (defaults, 0, Vec::new(), Default::default()),
        };
        // The bottom panel starts closed: stash each workspace's bottom strip
        // (the assets/console row) out of its tree so every launch begins
        // without it — Ctrl+Space brings it back (`toggle_bottom_panel`). A
        // workspace already closed in a previous session keeps its persisted
        // stash; one whose bottom region isn't the strip (Animation's
        // timeline) is left open. `take_bottom_strip` finds the strip whether
        // it's a root full-width region (old saved layouts) or nested under
        // the viewport column (the shipped default).
        for (name, tree) in layouts.iter_mut() {
            if closed_bottoms.contains_key(name.as_str()) {
                continue;
            }
            if let Some(stash) = dock::take_bottom_strip(tree) {
                closed_bottoms.insert(name.clone(), stash);
            }
        }
        app.insert_resource(BottomPanel {
            closed: closed_bottoms,
        });
        // Tell ember's dock which panels mark the collapsible bottom strip:
        // a leaf tabbing one of these below a vertical divider gets the
        // collapse chevron and the divider snap-closed gesture even when it
        // isn't the full-width root region (the shipped Scene default nests
        // the strip under the viewport column). Only `console` — the asset
        // browser now lives in the left column under the hierarchy, where a
        // marker would wrongly give that pane a collapse chevron.
        app.insert_resource(renzora_ember::dock::BottomStripMarkers(vec![
            "console".into(),
        ]));
        // The dock starts on the active workspace (overrides DockPlugin's empty).
        app.insert_resource(Dock {
            tree: layouts[active].1.clone(),
        });
        app.insert_resource(ShellLayouts { layouts, active });
        app.add_systems(Update, notif_bell_click);
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
        app.init_resource::<RibbonDrag>();
        app.init_resource::<RibbonRename>();
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
                (top_menu_open, top_menu_hover, top_menu_sync),
                (settings_btn_click, play_btn_click, update_play_button, vr_active_overlay),
                plugin_install::install_buttons,
                palette_btn_click,
                (theme_bridge, sync_theme_menu_open),
                apply_chrome_style,
                doc_add_click,
                doc_tab_click,
                (
                    doc_tab_close,
                    process_tab_close_request,
                    close_tab_prompt_buttons,
                    pending_close_after_save,
                ),
                (sync_workspace_to_active_doc, persist_dock_layout),
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
                relocalize_on_language_change,
                (play_target_option_click, update_play_target_menu),
                toggle_bottom_panel,
                (
                    bottom_snap_collapse,
                    sync_collapsed_bottom_bar,
                    position_collapsed_bottom_bar,
                    collapsed_bottom_tab_click,
                    collapsed_bottom_open_click,
                    collapsed_bottom_bar_drag,
                    collapsed_bottom_tab_hover,
                ),
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

/// The top-bar gear button — toggles the Settings overlay.
#[derive(Component)]
struct TopBarSettingsBtn;

fn settings_btn_click(
    btns: Query<&Interaction, (Changed<Interaction>, With<TopBarSettingsBtn>)>,
    settings: Option<ResMut<renzora_editor_framework::EditorSettings>>,
) {
    let Some(mut settings) = settings else { return };
    for interaction in &btns {
        if *interaction == Interaction::Pressed {
            settings.show_settings = !settings.show_settings;
        }
    }
}

/// The top-bar Play / Stop button (icon + text). This is the editor's single
/// play control now that the viewport toolbar's play/scripts buttons are gone.
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
                column_gap: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
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
    let icon = glyph(commands, "play", play_green(), 16.0);
    commands
        .entity(icon)
        .insert((TopBarPlayIcon, bevy::ui::FocusPolicy::Pass));
    let label = commands
        .spawn((
            Text::new(renzora::lang::t("common.play")),
            ui_font(font, 13.0),
            TextColor(rgb(play_green())),
            TopBarPlayLabel,
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    commands.entity(btn).add_children(&[icon, label]);
    btn
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
) {
    let Some(mut pm) = play_mode else { return };
    let runtime_alive = runtime.is_some_and(|r| r.is_alive());
    let has_cam = !scene_cams.is_empty();
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
/// while playing, simulating, or an external runtime is alive.
fn update_play_button(
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    runtime: Option<Res<renzora_viewport::external_runtime::ExternalRuntime>>,
    theme: Option<Res<renzora_theme::ThemeManager>>,
    scene_cams: Query<(), With<renzora::core::SceneCamera>>,
    settings: Option<Res<renzora_editor_framework::EditorSettings>>,
    mut icons: Query<&mut renzora_ember::icons::Icon, With<TopBarPlayIcon>>,
    mut labels: Query<(&mut Text, &mut TextColor), With<TopBarPlayLabel>>,
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
    let has_cam = !scene_cams.is_empty();
    let simulate = settings.is_some_and(|s| s.play_launch_simulate);

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
    } else if simulate {
        renzora::lang::t("common.simulate")
    } else {
        renzora::lang::t("common.play")
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

    let mut rows = Vec::new();
    for (choice, icon_name, label) in [
        (
            PlayLaunchChoice::Viewport,
            "frame-corners",
            renzora::lang::t_or("shell.play_target.viewport", "Viewport"),
        ),
        (
            PlayLaunchChoice::Window,
            "app-window",
            renzora::lang::t_or("shell.play_target.runtime_window", "Window"),
        ),
        (
            PlayLaunchChoice::Vr,
            "virtual-reality",
            renzora::lang::t_or("shell.play_target.vr", "VR Headset"),
        ),
        (
            PlayLaunchChoice::Simulate,
            "flask",
            renzora::lang::t_or("common.simulate", "Simulate"),
        ),
    ] {
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
        renzora_ember::reactive::bind_bg(commands, row, move |w| match w.get::<Interaction>(row) {
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
                padding: UiRect::axes(Val::Px(3.0), Val::Px(5.0)),
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
    renzora_ember::reactive::bind_bg(commands, trigger, move |w| {
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

/// Per-workspace stashes for the collapsible bottom panel, keyed by workspace
/// name. An entry means that workspace's bottom region is currently detached
/// from its tree and lives here (see [`ClosedBottom`]); no entry means the
/// panel is open (or the workspace never had one). Persisted with the layout —
/// the panels inside a closed bottom panel exist nowhere else.
#[derive(Resource, Default)]
struct BottomPanel {
    closed: std::collections::BTreeMap<String, ClosedBottom>,
}

/// Ctrl+Space ([`EditorAction::ToggleBottomPanel`]): collapse or restore the
/// active workspace's bottom panel.
///
/// Closing detaches the bottom region into [`BottomPanel`] (see
/// [`close_bottom_panel`] for the full-width / nested / legacy order); opening
/// re-attaches it at the remembered ratio, in its old place — full-width when it
/// was full-width, back under its column when it wasn't (see
/// [`reopen_bottom_panel`]). The stash round-trips the whole subtree, so tab
/// order, active tab and any splits inside the bottom region survive the toggle.
fn toggle_bottom_panel(
    keyboard: Res<ButtonInput<KeyCode>>,
    keybindings: Option<Res<KeyBindings>>,
    input_focus: Option<Res<renzora::core::InputFocusState>>,
    layouts: Res<ShellLayouts>,
    wins: Res<renzora_ember::dock::DockWindows>,
    mut bottom: ResMut<BottomPanel>,
    mut dock: ResMut<Dock>,
    mut dirty: ResMut<DockDirty>,
) {
    let Some(kb) = keybindings else { return };
    if kb.rebinding.is_some() || input_focus.is_some_and(|f| f.ui_wants_keyboard) {
        return;
    }
    if !kb.just_pressed(EditorAction::ToggleBottomPanel, &keyboard) {
        return;
    }
    let Some(name) = layouts.layouts.get(layouts.active).map(|(n, _)| n.clone()) else {
        return;
    };
    if reopen_bottom_panel(&name, &wins, &mut bottom, &mut dock, &mut dirty).is_some() {
        // reopened
    } else if let Some(stash) = close_bottom_panel(&mut dock.tree) {
        bottom.closed.insert(name, stash);
        dirty.0 = true;
    }
}

/// Detach the active workspace's bottom panel into a stash, in preference order:
///
/// 1. A full-width bottom region at the root ([`DockTree::detach_bottom`]) —
///    content-agnostic, so any workspace's root strip (Animation's timeline)
///    collapses. No anchor: it reopens full-width.
/// 2. A *nested* console strip that isn't full width — sits under one column
///    with a full-height panel beside it. Detached with an anchor
///    ([`DockTree::detach_bottom_containing`]) so it reopens in the same
///    place. Keyed on `console` only: the asset browser now docks in the left
///    column, so an `assets` marker here would collapse that pane instead.
/// 3. A legacy strip saved before the bottom region existed.
fn close_bottom_panel(tree: &mut DockTree) -> Option<ClosedBottom> {
    if let Some((bottom, ratio)) = tree.detach_bottom() {
        return Some(ClosedBottom {
            tree: bottom,
            ratio,
            anchor: Vec::new(),
        });
    }
    if let Some((bottom, ratio, anchor)) = tree.detach_bottom_containing("console") {
        return Some(ClosedBottom {
            tree: bottom,
            ratio,
            anchor,
        });
    }
    dock::take_legacy_bottom_strip(tree)
}

/// Re-attach `name`'s stashed bottom region to the dock tree. Returns the
/// path of the vertical split it re-created (empty = full-width at the root)
/// if a stash existed and was reopened, `None` otherwise — the drag-open
/// gesture hands that path to ember so the live drag adopts the right divider.
///
/// A stashed panel can re-appear while the bottom panel is closed (the tab
/// bar's "+" menu, a focus request, a tear-off window). Re-attaching it
/// anyway would put one panel id in two leaves, so those are dropped from
/// the stash — the live copy wins.
fn reopen_bottom_panel(
    name: &str,
    wins: &renzora_ember::dock::DockWindows,
    bottom: &mut BottomPanel,
    dock: &mut Dock,
    dirty: &mut DockDirty,
) -> Option<Vec<bool>> {
    let mut stash = bottom.closed.remove(name)?;
    let mut ids = Vec::new();
    stash.tree.collect_panels(&mut ids);
    for id in ids {
        if dock.tree.contains_panel(&id) || wins.0.iter().any(|s| s.tree.contains_panel(&id)) {
            stash.tree.remove_panel(&id);
        }
    }
    let path = dock
        .tree
        .attach_bottom_at(stash.tree, stash.ratio, &stash.anchor);
    dirty.0 = true;
    Some(path)
}

/// Consume ember's [`BottomSnapRequest`]: a hard drag down on a bottom
/// region's divider, or a click on its collapse chevron, snaps that region
/// closed (same stash as Ctrl+Space). A request naming a target panel came
/// from a **nested** strip — detach the region containing that panel, with
/// its anchor, so it reopens in place; no target means the root region.
/// Divider snaps carry the **pre-drag** ratio, so reopening restores the
/// height the panel had before the snap — not the squished mid-drag one the
/// live tree holds by then; chevron clicks carry none and keep the detached
/// ratio.
fn bottom_snap_collapse(
    snap: Option<ResMut<renzora_ember::dock::BottomSnapRequest>>,
    layouts: Res<ShellLayouts>,
    mut bottom: ResMut<BottomPanel>,
    mut dock: ResMut<Dock>,
    mut dirty: ResMut<DockDirty>,
) {
    let Some(mut snap) = snap else { return };
    let Some(req) = snap.0.take() else { return };
    let Some(name) = layouts.layouts.get(layouts.active).map(|(n, _)| n.clone()) else {
        return;
    };
    if bottom.closed.contains_key(&name) {
        return;
    }
    let stash = match &req.target {
        None => dock.tree.detach_bottom().map(|(tree, ratio)| ClosedBottom {
            tree,
            ratio: req.restore.unwrap_or(ratio),
            anchor: Vec::new(),
        }),
        Some(panel) => {
            dock.tree
                .detach_bottom_containing(panel)
                .map(|(tree, ratio, anchor)| ClosedBottom {
                    tree,
                    ratio: req.restore.unwrap_or(ratio),
                    anchor,
                })
        }
    };
    if let Some(stash) = stash {
        bottom.closed.insert(name, stash);
        dirty.0 = true;
    }
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

/// Keep the collapsed strip in sync with the active workspace's stash: shown
/// with one tab per stashed panel while the bottom panel is closed, hidden
/// while it's open. Tab children rebuild only when the stashed tab set (or
/// the bar entity, after a chrome respawn) changes.
fn sync_collapsed_bottom_bar(
    layouts: Res<ShellLayouts>,
    bottom: Res<BottomPanel>,
    fonts: Option<Res<EmberFonts>>,
    registry: Option<Res<renzora::core::ShellPanelRegistry>>,
    bars: Query<Entity, With<CollapsedBottomBar>>,
    mut nodes: Query<&mut Node>,
    mut commands: Commands,
    mut built: Local<Option<(Entity, Vec<String>)>>,
) {
    let (Some(fonts), Ok(bar)) = (fonts, bars.single()) else {
        return;
    };
    let Ok(mut node) = nodes.get_mut(bar) else {
        return;
    };
    let stash = layouts
        .layouts
        .get(layouts.active)
        .and_then(|(n, _)| bottom.closed.get(n.as_str()));
    let Some(stash) = stash else {
        if node.display != Display::None {
            node.display = Display::None;
        }
        return;
    };
    if node.display != Display::Flex {
        node.display = Display::Flex;
    }
    let mut ids = Vec::new();
    stash.tree.collect_panels(&mut ids);
    // Keyed on the bar entity too: a theme/language chrome respawn creates a
    // fresh (childless) bar, which must rebuild even for the same tab set.
    if built.as_ref() == Some(&(bar, ids.clone())) {
        return;
    }
    *built = Some((bar, ids.clone()));

    commands.entity(bar).despawn_related::<Children>();
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
    let strip_filler = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                height: Val::Percent(100.0),
                ..default()
            },
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

/// Align the collapsed strip with the region its stash is anchored to. The
/// bar is chrome — a row between the dock area and the status bar — so left
/// alone it spans the full window width. But a strip that was docked under
/// one column (the shipped Scene default nests it under the viewport) should
/// collapse *in place*: under that same column, leaving the side columns
/// their full height. When the active stash carries an anchor, this pulls
/// the bar out of the chrome flow (absolute, hugging the status bar's top
/// edge) and sizes it to the on-screen span of the leaves still holding the
/// anchor panels — the same panels [`reopen_bottom_panel`] re-attaches
/// under, so the closed strip sits exactly where the panel will reopen.
/// Falls back to the in-flow full-width row when the stash has no anchor or
/// the anchor panels all left the main window (mirroring the reopen
/// fallback in `attach_bottom_at`).
fn position_collapsed_bottom_bar(
    layouts: Res<ShellLayouts>,
    bottom: Res<BottomPanel>,
    areas: Query<Entity, (With<DockArea>, Without<renzora_ember::dock::FloatingDockArea>)>,
    leaves: Query<(&DockLeaf, &ComputedNode, &UiGlobalTransform)>,
    chrome: Query<(&ChromeBar, &ComputedNode)>,
    mut bars: Query<&mut Node, With<CollapsedBottomBar>>,
) {
    let Ok(mut node) = bars.single_mut() else {
        return;
    };
    let stash = layouts
        .layouts
        .get(layouts.active)
        .and_then(|(n, _)| bottom.closed.get(n.as_str()));
    // On-screen horizontal span of the anchor panels' leaves, main window
    // only — an anchor panel that migrated to a floating window can't
    // position the main chrome's bar.
    let anchored = stash.filter(|s| !s.anchor.is_empty()).and_then(|s| {
        let area = areas.single().ok()?;
        let mut left = f32::INFINITY;
        let mut right = f32::NEG_INFINITY;
        for (leaf, cn, gt) in &leaves {
            if leaf.area != area || !s.anchor.iter().any(|p| leaf.tabs.contains(p)) {
                continue;
            }
            let inv = cn.inverse_scale_factor();
            let half = cn.size().x * inv * 0.5;
            left = left.min(gt.translation.x * inv - half);
            right = right.max(gt.translation.x * inv + half);
        }
        (left < right).then_some((left, right - left))
    });
    let want = match anchored {
        Some((left, width)) => {
            // Sit flush on the status bar. Its height is theme-driven
            // (`apply_chrome_style`), so read the live computed height
            // instead of assuming one.
            let status_h = chrome
                .iter()
                .find(|(kind, _)| matches!(kind, ChromeBar::Status))
                .map(|(_, cn)| cn.size().y * cn.inverse_scale_factor())
                .unwrap_or(0.0);
            (
                PositionType::Absolute,
                Val::Px(left),
                Val::Px(width),
                Val::Px(status_h),
            )
        }
        None => (
            PositionType::Relative,
            Val::Auto,
            Val::Percent(100.0),
            Val::Auto,
        ),
    };
    // Reads go through `Deref` (no change flag); only assign on a real
    // change, since any `Node` write triggers a relayout.
    if node.position_type != want.0
        || node.left != want.1
        || node.width != want.2
        || node.bottom != want.3
    {
        node.position_type = want.0;
        node.left = want.1;
        node.width = want.2;
        node.bottom = want.3;
    }
}

/// Click a collapsed-strip tab → reopen the bottom panel (same restore path
/// as Ctrl+Space) with the clicked panel as the active tab.
fn collapsed_bottom_tab_click(
    tabs: Query<(&Interaction, &CollapsedBottomTab), Changed<Interaction>>,
    layouts: Res<ShellLayouts>,
    wins: Res<renzora_ember::dock::DockWindows>,
    mut bottom: ResMut<BottomPanel>,
    mut dock: ResMut<Dock>,
    mut dirty: ResMut<DockDirty>,
) {
    for (interaction, tab) in &tabs {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(name) = layouts.layouts.get(layouts.active).map(|(n, _)| n.clone()) else {
            return;
        };
        if reopen_bottom_panel(&name, &wins, &mut bottom, &mut dock, &mut dirty).is_some() {
            dock.tree.set_active_tab(&tab.0);
        }
        return;
    }
}

/// Click the collapsed strip's open chevron → reopen the bottom panel at its
/// remembered height.
fn collapsed_bottom_open_click(
    btns: Query<&Interaction, (With<CollapsedBottomOpenBtn>, Changed<Interaction>)>,
    layouts: Res<ShellLayouts>,
    wins: Res<renzora_ember::dock::DockWindows>,
    mut bottom: ResMut<BottomPanel>,
    mut dock: ResMut<Dock>,
    mut dirty: ResMut<DockDirty>,
) {
    for interaction in &btns {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Some(name) = layouts.layouts.get(layouts.active).map(|(n, _)| n.clone()) else {
            return;
        };
        reopen_bottom_panel(&name, &wins, &mut bottom, &mut dock, &mut dirty);
        return;
    }
}

/// Drag the collapsed strip's empty background upward → reopen the bottom
/// panel and hand the still-held cursor to the divider it re-attached under
/// ([`renzora_ember::dock::GrabRootDivider`]), so the panel sizes live from
/// the closed position — the gesture continues as a normal divider drag.
/// Tabs and the open chevron sit above the bar and capture their own
/// presses, so this only fires from the strip itself.
fn collapsed_bottom_bar_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    bars: Query<&Interaction, With<CollapsedBottomBar>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    layouts: Res<ShellLayouts>,
    wins: Res<renzora_ember::dock::DockWindows>,
    grab: Option<ResMut<renzora_ember::dock::GrabRootDivider>>,
    mut bottom: ResMut<BottomPanel>,
    mut dock: ResMut<Dock>,
    mut dirty: ResMut<DockDirty>,
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
    let Some(name) = layouts.layouts.get(layouts.active).map(|(n, _)| n.clone()) else {
        return;
    };
    if let Some(path) = reopen_bottom_panel(&name, &wins, &mut bottom, &mut dock, &mut dirty) {
        if let Some(mut grab) = grab {
            // The path of the split the strip re-attached under — ember
            // adopts that divider (not necessarily the root one) as the
            // live drag once the rebuilt tree provides it.
            grab.0 = Some(path);
        }
    }
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

/// Tags the ribbon strip + its `+` as a drop target for dock-tab drags: dropping
/// a dragged panel here spawns a new workspace from it (see [`workspace_drop_to_new`]).
#[derive(Component)]
struct WorkspaceDropZone;

/// The top-bar magnifier — toggles the command palette.
#[derive(Component)]
struct CommandPaletteBtn;

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
    toolbars: Option<Res<renzora_ember::toolbar::PanelToolbars>>,
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
        let toolbars = toolbars.map(|t| t.clone()).unwrap_or_default();
        spawn_shell(&mut commands, &fonts, &themes, &active, theme_menu_open.0, &toolbars);
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
    ("outline", "Outline", "list-dashes", "Editing"),
    ("code_editor", "Code", "code", "Editing"),
    ("console", "Console", "terminal", "Debug"),
    ("problems", "Problems", "warning-circle", "Debug"),
    ("script_variables", "Variables", "brackets-curly", "Scripting"),
    ("scripts_on_entity", "Scripts", "file-code", "Scripting"),
    // Viewports
    ("viewport", "Viewport", "perspective", "Scene"),
    ("viewport-2", "Viewport 2", "perspective", "Scene"),
    ("viewport-3", "Viewport 3", "perspective", "Scene"),
    ("viewport-4", "Viewport 4", "perspective", "Scene"),
    ("camera_preview", "Camera Preview", "video-camera", "Scene"),
    // Audio
    ("mixer", "Mixer", "faders", "Audio"),
    ("record", "Record", "record", "Audio"),
    ("daw", "Record", "record", "Audio"),
    // Animation
    ("timeline", "Timeline", "film-strip", "Animation"),
    ("sequencer", "Sequencer", "film-slate", "Animation"),
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
    // Marketplace
    ("hub_store", "Marketplace", "storefront", "Marketplace"),
    ("hub_library", "Library", "books", "Marketplace"),
    ("asset_uploader", "Publish", "upload-simple", "Marketplace"),
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
    let removed_name;
    {
        let mut l = world.resource_mut::<ShellLayouts>();
        removed_name = l.layouts.remove(index).0;
        let new_len = l.layouts.len();
        l.active = if active == index {
            active.min(new_len - 1)
        } else if active > index {
            active - 1
        } else {
            active
        };
    }
    // The workspace's panels are gone on purpose — including any stashed in a
    // closed bottom panel. (If another workspace shares the name, its stash is
    // collateral; names are effectively unique in practice.)
    if let Some(mut bp) = world.get_resource_mut::<BottomPanel>() {
        bp.closed.remove(&removed_name);
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
    mut bottom: ResMut<BottomPanel>,
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
        let old = std::mem::replace(&mut slot.0, new.clone());
        // A closed bottom-panel stash is keyed by workspace name — re-key it or
        // the rename would orphan the stash (its panels live only there).
        if old != new {
            if let Some(stash) = bottom.closed.remove(&old) {
                bottom.closed.insert(new, stash);
            }
        }
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
                    WindowResizeZone(octant),
                    renzora_ember::cursor_icon::HoverCursor(cursor),
                    Name::new("resize-zone"),
                ))
                .id();
            // Resizing makes no sense while maximized — hide the grips then.
            renzora_ember::reactive::bind_display(commands, id, |w| {
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
    toolbars: &renzora_ember::toolbar::PanelToolbars,
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

    // Shared panel-toolbar strip directly below the document tabs. Each panel
    // contributes its own toolbar items (via `register_panel_toolbar*`), and the
    // host shows the ones whose panel is the active dock tab — e.g. the viewport
    // header for the viewport, code-editor controls for the code editor, etc.
    let toolbar = renzora_ember::toolbar::build_toolbar_host(commands, fonts, toolbars);

    // Collapsed bottom-panel strip: the closed bottom panel's header stays
    // visible in place (tabs included); [`sync_collapsed_bottom_bar`] shows it
    // and fills the tabs whenever the active workspace's bottom panel is
    // closed, and [`position_collapsed_bottom_bar`] aligns it under the column
    // the stash is anchored to (full window width only for a full-width
    // stash). Sized/styled to match a dock leaf's tab bar.
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
            // panel and continues as a live divider drag (see
            // [`collapsed_bottom_bar_drag`]); the ns-resize cursor hints it.
            Interaction::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::NsResize),
            // When anchored under one column the bar overlays the bottom edge
            // of that column's panel (see [`position_collapsed_bottom_bar`]),
            // so it must swallow pointer events like any floating surface —
            // without this a click/scroll on the strip would bleed into the
            // viewport behind it.
            renzora_ember::widgets::OverlaySurface,
            CollapsedBottomBar,
            Name::new("collapsed-bottom-bar"),
        ))
        .id();

    let statusbar = build_status_bar(commands, fonts, themes, active, theme_menu_open);

    commands
        .entity(root)
        .add_children(&[top_bar, doctabs, toolbar, dock_area, collapsed_bottom, statusbar]);

    // Borderless-window edge/corner resize grips, overlaid on the perimeter.
    let grips = build_resize_zones(commands);
    commands.entity(root).add_children(&grips);
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
    renzora_ember::reactive::keyed_list(commands, left_content, status_snapshot_left);

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
    renzora_ember::reactive::keyed_list(commands, right_content, status_snapshot_right);

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

/// Left side: a Ready label + left-aligned status items. A plugin can swap the
/// "Ready" text via [`renzora::ShellReadyStatus`] (e.g. the auto-save countdown).
fn status_snapshot_left(world: &World) -> renzora_ember::reactive::KeyedSnapshot {
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
            commands.entity(r).add_children(&kids);
            r
        }
    }
}

/// The top bar: File/Edit/View/Help on the left, the layout ribbon centered,
/// action buttons on the right.
fn build_top_bar(commands: &mut Commands, font: &bevy::text::FontSource) -> Entity {
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

    let left = zone(commands, "top-left", JustifyContent::FlexStart, 2.0, 1.0);
    let left_kids = vec![
        top_menu_item(commands, font, &renzora::lang::t("menu.file"), TopMenuKind::File),
        top_menu_item(commands, font, &renzora::lang::t("menu.edit"), TopMenuKind::Edit),
        top_menu_item(commands, font, &renzora::lang::t("menu.view"), TopMenuKind::View),
        top_menu_item(commands, font, &renzora::lang::t("menu.help"), TopMenuKind::Help),
        account_menu_item(commands, font),
        notification_bell_item(commands, font),
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
        renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
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
            bevy::ui::FocusPolicy::Pass,
            WorkspaceDropZone,
            RelativeCursorPosition::default(),
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
    commands.entity(center).add_children(&[magnifier, ribbon, add]);

    let right = zone(commands, "top-right", JustifyContent::FlexEnd, 8.0, 1.0);
    let play = build_play_button(commands, font);
    // Play + its target caret grouped tightly so they read as one split button
    // (the zone's 8px gap would otherwise pull them apart).
    let play_caret = build_play_target_caret(commands, font);
    let play_group = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(1.0),
                ..default()
            },
            Name::new("play-group"),
        ))
        .id();
    commands.entity(play_group).add_children(&[play, play_caret]);
    let settings = icon_item(commands, "gear", text_muted(), 16.0);
    commands.entity(settings).insert((
        Interaction::default(),
        TopBarSettingsBtn,
        renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
    ));

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
    let mut kids: Vec<Entity> = Vec::new();
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
        renzora_ember::reactive::bind_bg(commands, btn, move |w| match w.get::<Interaction>(btn) {
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
        renzora_ember::reactive::bind_text_color(commands, g, move |w| {
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

    commands
        .entity(right)
        .add_children(&[play_group, settings, window]);

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
    commands.entity(item).add_children(&[text_wrap, underline, marker]);
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

/// The document tab strip: the open documents (`DocumentTabState`, shared with
/// the egui editor) rendered reactively, plus an add-document button.
fn build_doc_tabs(commands: &mut Commands, _font: &bevy::text::FontSource) -> Entity {
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
            renzora_ember::widgets::ThemeShaderSurface {
                surface: renzora_ember::widgets::ThemeSurface::DocTabs,
            },
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
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
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
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new(format!("doc:{name}")),
        ))
        .id();
    // Kind icon — `icon` is a phosphor *name* (kebab-case); resolve to a glyph.
    let ic = icon_text(commands, &fonts.phosphor, icon, fg, 13.0);
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
                renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
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
    bottom: Res<BottomPanel>,
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
    let Some(json) = dock::layout_json(&snapshot, layouts.active, &floating, &bottom.closed)
    else {
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
    mut commands: Commands,
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
        if let Some(name) = state.tabs[idx].kind.layout_name() {
            if let Some(wi) = layouts.layouts.iter().position(|(n, _)| n == name) {
                apply_workspace(wi, &mut layouts, &mut dock, &mut dirty);
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
            // Structural container — let clicks fall through to the bar's drag
            // handle behind it (interactive children still block on their own).
            bevy::ui::FocusPolicy::Pass,
            Name::new(name.to_string()),
        ))
        .id()
}



// ── Top-bar menus (File / Edit / View / Help) ────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum TopMenuKind {
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

/// An interactive top-bar menu title (File/Edit/View/Help).
fn top_menu_item(
    commands: &mut Commands,
    font: &bevy::text::FontSource,
    label: &str,
    kind: TopMenuKind,
) -> Entity {
    // The debug Name is a stable identifier derived from the menu kind, NOT the
    // (now localized) label — so translating the title doesn't rename the entity.
    let name_id = match kind {
        TopMenuKind::File => "file",
        TopMenuKind::Edit => "edit",
        TopMenuKind::View => "view",
        TopMenuKind::Help => "help",
        TopMenuKind::Account => "account",
    };
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
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            TopMenu(kind),
            Name::new(format!("menu:{name_id}")),
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
            bevy::ui::FocusPolicy::Pass,
        ));
    });
    item
}

/// The account menu title (left bar, after Help). Identical styling to the other
/// top menus, but its label is reactive (the signed-in username, or "Sign In").
fn account_menu_item(commands: &mut Commands, font: &bevy::text::FontSource) -> Entity {
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
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            TopMenu(TopMenuKind::Account),
            Name::new("menu:account"),
        ))
        .id();
    renzora_ember::reactive::bind_bg(commands, item, move |w| match w.get::<Interaction>(item) {
        Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(renzora_ember::theme::hover_bg()),
        _ => Color::NONE,
    });
    let label = commands
        .spawn((
            Text::new(renzora::lang::t("auth.sign_in")),
            ui_font(font, 14.0),
            TextColor(rgb(text_muted())),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    renzora_ember::reactive::bind_text(commands, label, |w| {
        w.get_resource::<renzora::core::AuthBridge>()
            .and_then(|b| b.signed_in_username.clone())
            .unwrap_or_else(|| renzora::lang::t("auth.sign_in"))
    });
    commands.entity(item).add_child(label);
    item
}

/// Marks the top-bar notification bell (right of the account item). Clicking
/// it opens the Community Notifications panel via the social bridge.
#[derive(Component)]
struct NotifBellBtn;

/// The top-bar notification bell: unread count from [`renzora::core::SocialBridge`],
/// hidden when signed out or disabled in Settings → Social & Privacy.
fn notification_bell_item(commands: &mut Commands, font: &bevy::text::FontSource) -> Entity {
    let item = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(7.0), Val::Px(4.0)),
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            bevy::ui::RelativeCursorPosition::default(),
            renzora_ember::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            NotifBellBtn,
            Name::new("menu:notifications"),
        ))
        .id();
    renzora_ember::reactive::bind_bg(commands, item, move |w| match w.get::<Interaction>(item) {
        Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(renzora_ember::theme::hover_bg()),
        _ => Color::NONE,
    });
    renzora_ember::reactive::bind_display(commands, item, |w| {
        let enabled = w
            .get_resource::<renzora::core::SocialBridge>()
            .map(|b| b.notify_button_enabled)
            .unwrap_or(false);
        let signed_in = w
            .get_resource::<renzora::core::AuthBridge>()
            .and_then(|b| b.signed_in_username.clone())
            .is_some();
        enabled && signed_in
    });
    let bell = glyph(commands, "bell", text_muted(), 13.0);
    let count = commands
        .spawn((
            Text::new(""),
            ui_font(font, 11.0),
            TextColor(renzora_ember::theme::rgba([235, 180, 80, 255])),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    renzora_ember::reactive::bind_text(commands, count, |w| {
        let n = w
            .get_resource::<renzora::core::SocialBridge>()
            .map(|b| b.unread_notifications)
            .unwrap_or(0);
        if n > 0 { n.to_string() } else { String::new() }
    });
    // Hide the count entirely at zero so the button doesn't reserve gap space.
    renzora_ember::reactive::bind_display(commands, count, |w| {
        w.get_resource::<renzora::core::SocialBridge>()
            .map(|b| b.unread_notifications > 0)
            .unwrap_or(false)
    });
    commands.entity(item).add_children(&[bell, count]);
    item
}

/// Bell click → toggle the notifications dropdown (consumed by `renzora_social`).
/// Anchored below the button via [`anchor_below`], the same way the top menus
/// position their popups.
fn notif_bell_click(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    bells: Query<
        (&Interaction, &bevy::ui::RelativeCursorPosition, &ComputedNode),
        (With<NotifBellBtn>, Changed<Interaction>),
    >,
    bridge: Option<ResMut<renzora::core::SocialBridge>>,
) {
    let Some(mut bridge) = bridge else { return };
    for (i, rcp, cn) in &bells {
        if *i == Interaction::Pressed {
            if let Some(pos) = anchor_below(&windows, rcp, cn) {
                bridge.notify_dropdown_request = Some((pos.x, pos.y));
            }
        }
    }
}

/// Spawn a top-menu dropdown anchored at `pos` and return its root.
fn spawn_top_menu(commands: &mut Commands, fonts: &EmberFonts, kind: TopMenuKind, pos: Vec2, signed_in: bool) -> Entity {
    let root = renzora_ember::widgets::screen_menu(commands, pos.x, pos.y);
    let kids = build_menu_items(commands, fonts, kind, signed_in);
    commands.entity(root).add_children(&kids);
    root
}

fn signed_in(bridge: &Option<Res<renzora::core::AuthBridge>>) -> bool {
    bridge.as_ref().and_then(|b| b.signed_in_username.clone()).is_some()
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
    mut open: ResMut<OpenTopMenu>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else {
        return;
    };
    let signed = signed_in(&bridge);
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
        open.menu = Some(spawn_top_menu(&mut commands, &fonts, menu.0, pos, signed));
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
    mut open: ResMut<OpenTopMenu>,
    mut commands: Commands,
) {
    let Some(open_kind) = open.kind else { return };
    let Some(fonts) = fonts else { return };
    let signed = signed_in(&bridge);
    for (interaction, menu, rcp, cn) in &q {
        if *interaction == Interaction::Hovered && menu.0 != open_kind {
            if let Some(e) = open.menu.take() {
                commands.entity(e).try_despawn();
            }
            let Some(pos) = anchor_below(&windows, rcp, cn) else {
                open.kind = None;
                return;
            };
            open.menu = Some(spawn_top_menu(&mut commands, &fonts, menu.0, pos, signed));
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

fn build_menu_items(
    commands: &mut Commands,
    fonts: &EmberFonts,
    kind: TopMenuKind,
    signed_in: bool,
) -> Vec<Entity> {
    use renzora_ember::widgets::{menu_item, menu_sep};
    match kind {
        TopMenuKind::Account => {
            if signed_in {
                vec![
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
            // picks it up and opens the file picker / import overlay. No
            // ImportTargetDir here, so files land in the importer's default folder.
            menu_item(commands, fonts, "download-simple", &renzora::lang::t("menu.file.import_assets"), |w| {
                w.insert_resource(renzora::core::ImportRequested);
            }),
            menu_item(commands, fonts, "package", &renzora::lang::t("menu.file.export_project"), |w| {
                w.insert_resource(renzora::core::ExportRequested);
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "plug", &renzora::lang::t_or("menu.file.install_plugin", "Install Plugin…"), |w| {
                crate::plugin_install::open_install_dialog(w)
            }),
            menu_sep(commands),
            menu_item(commands, fonts, "gear", &renzora::lang::t("common.settings"), |w| {
                if let Some(mut s) = w.get_resource_mut::<renzora_editor_framework::EditorSettings>() {
                    s.show_settings = !s.show_settings;
                }
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
    // The pristine default carries the bottom strip in-tree; a surviving stash
    // would duplicate those panels on the next Ctrl+Space toggle.
    if let Some(mut bp) = w.get_resource_mut::<BottomPanel>() {
        bp.closed.remove(&active_name);
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
fn reset_workspace_action(w: &mut World) {
    let defaults = dock::workspace_layouts();
    let Some((_, active_tree)) = defaults.first().map(|(n, t)| (n.clone(), t.clone())) else {
        return;
    };
    if let Some(mut layouts) = w.get_resource_mut::<ShellLayouts>() {
        layouts.layouts = defaults;
        layouts.active = 0;
    }
    // Every default tree carries its bottom region in-tree; surviving stashes
    // would duplicate those panels on the next Ctrl+Space toggle.
    if let Some(mut bp) = w.get_resource_mut::<BottomPanel>() {
        bp.closed.clear();
    }
    if let Some(mut dock) = w.get_resource_mut::<Dock>() {
        dock.tree = active_tree;
    }
    if let Some(mut d) = w.get_resource_mut::<DockDirty>() {
        d.0 = true;
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
