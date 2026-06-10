//! Runtime "Render Toggles" — debug switches that disable expensive,
//! resolution-bound render work so its cost can be isolated *without* a
//! rebuild. Its own panel ("Render Toggles", Debug category in Add Panel).
//!
//! Each toggle defaults to ON (normal rendering). Unchecking forces the
//! corresponding work off every frame; re-checking restores the saved
//! state. These are diagnostic switches, not persisted settings — nothing
//! here is written to disk.
//!
//! Why this exists: on an empty scene the editor still spends ~30ms of GPU
//! per frame for ~15 draw calls, because the cost is fullscreen passes on
//! the active camera (atmosphere + screen-space GI + auto-exposure), not
//! geometry. These toggles let us bisect which pass is responsible on a
//! given machine (notably Apple Silicon / Retina).
//!
//! Note: prepasses are intentionally NOT togglable. Their attachment layout
//! is fixed at camera spawn (re-adding trips a wgpu validation error that
//! degrades the pipeline until restart), and the depth prepass is a *win* in
//! real scenes (early-Z overdraw rejection) — stripping it slows heavy scenes
//! down. The "Shadows" checkbox drives the engine's existing viewport shadow
//! setting (`ViewportSettings.render_toggles.shadows`) rather than poking
//! lights directly — that setting already has a single authority
//! (`renzora_viewport::update_shadow_settings`), so we don't fight or flicker.

use bevy::prelude::*;

use renzora::core::viewport_types::ViewportSettings;
use renzora::{IsolatedCamera, LumenLighting, LumenQuality, RenzoraShellExt, RtLighting, SplashState};
use renzora_auto_exposure::AutoExposureSettings;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::theme::{rgb, text_muted};

use super::camera::{checkbox_row, faint_box};

/// State for the Render Toggles panel. The four `pub bool`s are checkbox
/// state (`true` = normal/ON); the `*_saved` vectors hold the original
/// component values so unchecking is fully reversible.
#[derive(Resource)]
pub struct RenderToggles {
    /// Allow offscreen preview cameras (Studio/Material/Shader/Particle,
    /// thumbnails, UI render cam — anything `IsolatedCamera`) to render.
    pub preview_cameras: bool,
    /// Screen-space global illumination (Lumen + RT lighting).
    pub gi: bool,
    /// Auto-exposure post-process (the per-frame histogram pass).
    pub auto_exposure: bool,
    // No `shadows` field: the panel's "Shadows" checkbox reads/writes the
    // engine's existing `ViewportSettings.render_toggles.shadows` directly, so
    // there's a single owner and no enforcement system of our own.
    gi_saved: Vec<(Entity, RtLighting)>,
    lumen_saved: Vec<(Entity, LumenLighting)>,
    ae_saved: Vec<(Entity, bool)>,
}

impl Default for RenderToggles {
    fn default() -> Self {
        Self {
            preview_cameras: true,
            gi: true,
            auto_exposure: true,
            gi_saved: Vec::new(),
            lumen_saved: Vec::new(),
            ae_saved: Vec::new(),
        }
    }
}

pub(super) fn register(app: &mut App) {
    app.init_resource::<RenderToggles>();
    // Standalone panel: content builder + Add-Panel menu entry (Debug group).
    app.register_panel_content("render_toggles", true, build);
    app.register_shell_panel("render_toggles", "Render Toggles", "sliders-horizontal", "Debug");
    // PostUpdate so these win over the editor's own Update-stage camera
    // activation / effect-routing systems, and before render extraction.
    app.add_systems(
        PostUpdate,
        (
            enforce_preview_cameras,
            enforce_gi,
            enforce_auto_exposure,
        )
            .run_if(in_state(SplashState::Editor)),
    );
}

// ── UI ────────────────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = super::root(commands);
    let label = super::section(commands, fonts, "Render Toggles");
    let hint = commands
        .spawn((
            Text::new("Uncheck to stop that work — isolate GPU cost (not saved)"),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
        ))
        .id();

    let box_ = faint_box(commands);
    let r1 = checkbox_row(
        commands,
        fonts,
        "Preview cameras (offscreen)",
        |w| get(w, |t| t.preview_cameras),
        |w, v| set(w, move |t| t.preview_cameras = v),
    );
    let r2 = checkbox_row(
        commands,
        fonts,
        "Global illumination (Lumen/RT)",
        |w| get(w, |t| t.gi),
        |w, v| set(w, move |t| t.gi = v),
    );
    let r3 = checkbox_row(
        commands,
        fonts,
        "Auto exposure",
        |w| get(w, |t| t.auto_exposure),
        |w, v| set(w, move |t| t.auto_exposure = v),
    );
    // Shadows / Meshes / Textures / Lighting are bound to the engine's existing
    // `ViewportSettings.render_toggles` — each already has a single authority
    // (renzora_viewport's update_shadow_settings / update_render_toggles), so we
    // keep no copy of our own and never fight or flicker. "Meshes off" discards
    // all mesh pixels → the cleanest safe way to blank the scene geometry.
    let r4 = checkbox_row(commands, fonts, "Shadows", |w| vp(w, |t| t.shadows), |w, v| vp_set(w, move |t| t.shadows = v));
    let r5 = checkbox_row(commands, fonts, "Meshes (blank scene)", |w| vp(w, |t| t.mesh), |w, v| vp_set(w, move |t| t.mesh = v));
    let r6 = checkbox_row(commands, fonts, "Textures", |w| vp(w, |t| t.textures), |w, v| vp_set(w, move |t| t.textures = v));
    let r7 = checkbox_row(commands, fonts, "Lighting (unlit)", |w| vp(w, |t| t.lighting), |w, v| vp_set(w, move |t| t.lighting = v));
    commands.entity(box_).add_children(&[r1, r2, r3, r4, r5, r6, r7]);

    commands.entity(root).add_children(&[label, hint, box_]);
    root
}

fn get(w: &World, f: impl Fn(&RenderToggles) -> bool) -> bool {
    w.get_resource::<RenderToggles>().map(f).unwrap_or(true)
}

fn set(w: &mut World, f: impl FnOnce(&mut RenderToggles)) {
    if let Some(mut t) = w.get_resource_mut::<RenderToggles>() {
        f(&mut t);
    }
}

/// Read a `ViewportSettings.render_toggles` flag (defaults to `true`/on).
fn vp(w: &World, f: impl Fn(&renzora::core::viewport_types::RenderToggles) -> bool) -> bool {
    w.get_resource::<ViewportSettings>().map(|s| f(&s.render_toggles)).unwrap_or(true)
}

/// Mutate a `ViewportSettings.render_toggles` flag (the engine applies it).
fn vp_set(w: &mut World, f: impl FnOnce(&mut renzora::core::viewport_types::RenderToggles)) {
    if let Some(mut s) = w.get_resource_mut::<ViewportSettings>() {
        f(&mut s.render_toggles);
    }
}

// ── Enforcement ─────────────────────────────────────────────────────────────

/// While off, force every offscreen preview camera inactive. While on, do
/// nothing — the owning editor systems re-activate them as panels mount.
fn enforce_preview_cameras(
    toggles: Res<RenderToggles>,
    mut cams: Query<&mut Camera, With<IsolatedCamera>>,
) {
    if toggles.preview_cameras {
        return;
    }
    for mut cam in &mut cams {
        if cam.is_active {
            cam.is_active = false;
        }
    }
}

/// Disable Lumen + RT lighting while off (saving originals on the way down,
/// restoring them on the way back up). The per-frame force loop keeps GI off
/// even if the effect-routing system re-applies it.
fn enforce_gi(
    mut toggles: ResMut<RenderToggles>,
    mut last: Local<Option<bool>>,
    mut rt: Query<(Entity, &mut RtLighting)>,
    mut lumen: Query<(Entity, &mut LumenLighting)>,
) {
    let now = toggles.gi;
    let transitioned = *last != Some(now);

    if !now {
        if transitioned {
            toggles.gi_saved.clear();
            toggles.lumen_saved.clear();
            for (e, r) in &rt {
                toggles.gi_saved.push((e, r.clone()));
            }
            for (e, l) in &lumen {
                toggles.lumen_saved.push((e, l.clone()));
            }
        }
        // Guard the writes so we don't re-trigger change detection every frame.
        for (_, mut r) in &mut rt {
            if r.enabled {
                r.enabled = false;
            }
        }
        for (_, mut l) in &mut lumen {
            if l.quality != LumenQuality::Off {
                l.quality = LumenQuality::Off;
            }
        }
    } else if transitioned {
        for (e, v) in std::mem::take(&mut toggles.gi_saved) {
            if let Ok((_, mut r)) = rt.get_mut(e) {
                *r = v;
            }
        }
        for (e, v) in std::mem::take(&mut toggles.lumen_saved) {
            if let Ok((_, mut l)) = lumen.get_mut(e) {
                *l = v;
            }
        }
    }

    *last = Some(now);
}

/// Disable the auto-exposure source(s) while off, restore on the way back up.
fn enforce_auto_exposure(
    mut toggles: ResMut<RenderToggles>,
    mut last: Local<Option<bool>>,
    mut ae: Query<(Entity, &mut AutoExposureSettings)>,
) {
    let now = toggles.auto_exposure;
    let transitioned = *last != Some(now);

    if !now {
        if transitioned {
            toggles.ae_saved.clear();
            for (e, s) in &ae {
                toggles.ae_saved.push((e, s.enabled));
            }
        }
        for (_, mut s) in &mut ae {
            if s.enabled {
                s.enabled = false;
            }
        }
    } else if transitioned {
        for (e, en) in std::mem::take(&mut toggles.ae_saved) {
            if let Ok((_, mut s)) = ae.get_mut(e) {
                s.enabled = en;
            }
        }
    }

    *last = Some(now);
}

