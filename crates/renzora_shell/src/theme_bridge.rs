//! Maps the editor's active [`renzora_theme::Theme`] into everything ember
//! renders from: the runtime palette, the code editor's syntax colours, the
//! per-surface shader/image effects, the UI font, and the per-widget style
//! cascade.
//!
//! The one subtlety worth knowing before editing anything here is *when* a
//! rebuild is allowed. Each mapping is gated on a real change (the shader apply
//! reads files off disk, so re-running it per frame would hammer the
//! filesystem), and a theme *switch* despawns the chrome — which is why
//! [`theme_bridge`] **cancels** a pending dock rebuild rather than requesting
//! one. See the comment on that branch; getting it backwards is GH issue #67.

use bevy::prelude::*;

use renzora_ember::dock::DockDirty;
use renzora_ember::widgets::Popup;

use crate::status_bar::{ThemeDropup, ThemeMenuOpen};
use crate::ShellRoot;

/// Map the active `ThemeManager` theme into ember's runtime palette, and rebuild
/// the chrome when the active theme *changes* (a switch) so widgets re-spawn with
/// the new colors. Individual color edits update the palette but don't rebuild
/// (that would close the Theme tab's color picker every frame).
pub(crate) fn theme_bridge(
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
pub(crate) fn sync_theme_menu_open(
    dropup: Query<&Popup, With<ThemeDropup>>,
    mut open: ResMut<ThemeMenuOpen>,
) {
    if let Ok(p) = dropup.single() {
        if open.0 != p.open {
            open.0 = p.open;
        }
    }
}

/// Build the ember [`renzora_ember::style::Theme`] from the active theme's
/// `themes/<name>.toml` (its per-widget style sections cascade over the
/// palette-derived defaults). Built-in themes with no file fall back to the
/// defaults.
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

pub(crate) fn palette_from_theme(t: &renzora_theme::Theme) -> renzora_ember::theme::Palette {
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
pub(crate) fn apply_theme_effects(
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

/// Map a theme's `syntax` section into the code editor's
/// [`renzora_ember::theme::SyntaxPalette`]. Token colors drop alpha (they're
/// opaque); chrome colors that overlay text keep their full RGBA.
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
