//! The terrain brush's settings, mounted as a group in the viewport toolbar.
//!
//! The shelf ([`crate::shelf`]) answers *which* brush; this answers *how it
//! behaves*. Splitting them that way is the point: size and strength are the
//! two things you change constantly, often between one stroke and the next, and
//! putting them in a dock panel means every adjustment is a trip across the
//! screen and back. On the toolbar they're a few pixels from the viewport you're
//! painting in.
//!
//! The group is registered through [`renzora_ember::toolbar::register_viewport_tool_group`]
//! rather than built into the viewport crate, because `renzora_viewport` must not
//! depend on terrain. It's an ordinary arrangement group, so it can be dragged to
//! a new spot on the bar and stays there.
//!
//! Everything is context-gated. The whole group hides unless a terrain brush is
//! active, and within it each brush's own controls (`Flatten`'s target height,
//! `Noise`'s octaves, `Stamp`'s rotation…) appear only for that brush — so the
//! bar is never wider than the brush in hand actually needs.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora_editor_framework::ActiveTool;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::reactive::tracked::{bind_2way, bind_bg, bind_display, bind_text};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    drag_value_flat, dropdown_compact, slider_ranged, DragRange, DragSnap,
};

use renzora_terrain::data::{
    BrushFalloffType, BrushShape, FlattenMode, NoiseMode, StampBlendMode, TerrainBrushType,
    TerrainSettings,
};
use renzora_terrain::paint::SurfacePaintSettings;

/// Slider width in the bar. Wide enough to aim with, narrow enough that three of
/// them plus the toggles still fit one line on a typical viewport.
const SLIDER_W: f32 = 78.0;

pub fn register() {
    renzora_ember::toolbar::register_viewport_tool_group("terrain-brush", build);
}

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let group = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                flex_shrink: 0.0,
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
            Name::new("vp-terrain-brush"),
        ))
        .id();
    // Invisible unless a terrain brush is in hand. An always-present group would
    // hold its width in every other context for nothing.
    bind_display(commands, group, |w| {
        matches!(
            w.get_resource::<ActiveTool>().copied(),
            Some(ActiveTool::TerrainSculpt) | Some(ActiveTool::TerrainPaint)
        )
    });

    let kids = vec![
        sculpt_common(commands, fonts),
        paint_common(commands, fonts),
        shape_toggles(commands, fonts),
        falloff_toggles(commands, fonts),
        flatten_opts(commands, fonts),
        noise_opts(commands, fonts),
        terrace_opts(commands, fonts),
        stamp_opts(commands, fonts),
    ];
    commands.entity(group).add_children(&kids);
    group
}

// ── The always-there controls ───────────────────────────────────────────────

/// Size / strength / falloff for the sculpt brushes.
fn sculpt_common(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = cluster(commands, "sculpt-common");
    let size = labelled_slider(
        commands,
        fonts,
        "Size",
        1.0,
        200.0,
        0,
        |w| get(w, |s| s.brush_radius),
        |w, v| set(w, |s| s.brush_radius = *v),
    );
    let strength = labelled_slider(
        commands,
        fonts,
        "Strength",
        0.01,
        1.0,
        2,
        |w| get(w, |s| s.brush_strength),
        |w, v| set(w, |s| s.brush_strength = *v),
    );
    let falloff = labelled_slider(
        commands,
        fonts,
        "Falloff",
        0.0,
        1.0,
        2,
        |w| get(w, |s| s.falloff),
        |w, v| set(w, |s| s.falloff = *v),
    );
    commands.entity(row).add_children(&[size, strength, falloff]);
    only_when(commands, row, |w| tool_is(w, ActiveTool::TerrainSculpt));
    row
}

/// The paint brushes keep their own settings resource, so they get their own
/// cluster rather than sharing the sculpt one.
fn paint_common(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = cluster(commands, "paint-common");
    // Paint radius is a *fraction of the terrain*, not metres — hence 0.01..0.5
    // rather than the sculpt brush's 1..200.
    let size = labelled_slider(
        commands,
        fonts,
        "Size",
        0.01,
        0.5,
        3,
        |w| get_paint(w, |s| s.brush_radius),
        |w, v| set_paint(w, |s| s.brush_radius = *v),
    );
    let strength = labelled_slider(
        commands,
        fonts,
        "Strength",
        0.01,
        1.0,
        2,
        |w| get_paint(w, |s| s.brush_strength),
        |w, v| set_paint(w, |s| s.brush_strength = *v),
    );
    let falloff = labelled_slider(
        commands,
        fonts,
        "Falloff",
        0.0,
        1.0,
        2,
        |w| get_paint(w, |s| s.brush_falloff),
        |w, v| set_paint(w, |s| s.brush_falloff = *v),
    );
    commands.entity(row).add_children(&[size, strength, falloff]);
    only_when(commands, row, |w| tool_is(w, ActiveTool::TerrainPaint));
    row
}

const SHAPE_ICONS: [(BrushShape, &str); 3] = [
    (BrushShape::Circle, "circle"),
    (BrushShape::Square, "square"),
    (BrushShape::Diamond, "diamond"),
];

/// Circle / square / diamond. Writes whichever settings resource the active tool
/// reads, so one set of buttons serves both brushes.
fn shape_toggles(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = cluster(commands, "shape");
    let kids: Vec<Entity> = SHAPE_ICONS
    .into_iter()
    .map(|(shape, icon)| {
        let btn = toggle_button(commands);
        bind_bg(commands, btn, move |w| {
            let cur = if tool_is(w, ActiveTool::TerrainPaint) {
                w.get_resource::<SurfacePaintSettings>().map(|s| s.brush_shape)
            } else {
                w.get_resource::<TerrainSettings>().map(|s| s.brush_shape)
            };
            toggle_bg(w, btn, cur == Some(shape))
        });
        commands.entity(btn).insert(ShapeBtn(shape));
        let ic = icon_text(commands, &fonts.phosphor, icon, text_primary(), 11.0);
        commands.entity(btn).add_child(ic);
        btn
    })
    .collect();
    commands.entity(row).add_children(&kids);
    row
}

/// The five falloff curves.
///
/// Each icon is picked to *depict the curve's profile* rather than to abbreviate
/// its name — a sine for the cosine ease, a straight segment for the linear
/// ramp, an arc for the spherical one, a sharp peak for Tip, and a square wave
/// for Flat's plateau-then-cliff. A falloff is a shape, and a row of shapes is
/// readable at a glance in a way that a row of letters (S L O T F) never is;
/// the letters also collided with each other and with the shape toggles beside
/// them, since "S" could equally mean Square.
const FALLOFF_ICONS: [(BrushFalloffType, &str, &str); 5] = [
    (BrushFalloffType::Smooth, "wave-sine", "Smooth falloff — cosine ease"),
    (BrushFalloffType::Linear, "line-segment", "Linear falloff — straight ramp"),
    (BrushFalloffType::Spherical, "circle-notch", "Spherical falloff — circular arc"),
    (BrushFalloffType::Tip, "wave-triangle", "Tip falloff — sharp peak"),
    (BrushFalloffType::Flat, "wave-square", "Flat — no falloff"),
];

fn falloff_toggles(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = cluster(commands, "falloff-curve");
    let kids: Vec<Entity> = FALLOFF_ICONS
    .into_iter()
    .map(|(ft, icon, tip)| {
        let btn = toggle_button(commands);
        commands
            .entity(btn)
            .insert((FalloffBtn(ft), renzora_ember::widgets::HoverTooltip::new(tip)));
        bind_bg(commands, btn, move |w| {
            let cur = w.get_resource::<TerrainSettings>().map(|s| s.falloff_type);
            toggle_bg(w, btn, cur == Some(ft))
        });
        let ic = icon_text(commands, &fonts.phosphor, icon, text_primary(), 11.0);
        commands.entity(btn).add_child(ic);
        btn
    })
    .collect();
    commands.entity(row).add_children(&kids);
    only_when(commands, row, |w| tool_is(w, ActiveTool::TerrainSculpt));
    row
}

// ── Per-brush controls ──────────────────────────────────────────────────────

/// The order the dropdown lists flatten modes in; index into this is the
/// dropdown's model value.
const FLATTEN_MODES: [FlattenMode; 3] =
    [FlattenMode::Both, FlattenMode::Raise, FlattenMode::Lower];

fn flatten_opts(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = cluster(commands, "flatten");
    let mode = labelled_dropdown(
        commands,
        fonts,
        "Mode",
        &["Both", "Raise", "Lower"],
        64.0,
        |w| {
            let cur = get(w, |s| s.flatten_mode);
            FLATTEN_MODES.iter().position(|m| *m == cur).unwrap_or(0)
        },
        |w, i| {
            let m = FLATTEN_MODES.get(*i).copied().unwrap_or_default();
            set(w, |s| s.flatten_mode = m);
        },
    );
    let target = labelled_drag(
        commands,
        fonts,
        "Height",
        0.0,
        1.0,
        0.005,
        None,
        |w| get(w, |s| s.target_height),
        |w, v| set(w, |s| s.target_height = *v),
    );
    commands.entity(row).add_children(&[mode, target]);
    only_when_brush(commands, row, TerrainBrushType::Flatten);
    row
}

fn noise_opts(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = cluster(commands, "noise");
    let mode = labelled_dropdown(
        commands,
        fonts,
        "Noise",
        &["FBM", "Ridge", "Billow", "Warped", "Hybrid"],
        68.0,
        |w| {
            let cur = get(w, |s| s.noise_mode);
            NoiseMode::all().iter().position(|m| *m == cur).unwrap_or(0)
        },
        |w, i| {
            let m = NoiseMode::all().get(*i).copied().unwrap_or_default();
            set(w, |s| s.noise_mode = m);
        },
    );
    let scale = labelled_drag(
        commands,
        fonts,
        "Scale",
        1.0,
        500.0,
        0.5,
        None,
        |w| get(w, |s| s.noise_scale),
        |w, v| set(w, |s| s.noise_scale = *v),
    );
    let octaves = labelled_drag(
        commands,
        fonts,
        "Oct",
        1.0,
        8.0,
        0.1,
        Some(1.0),
        |w| get(w, |s| s.noise_octaves as f32),
        |w, v| set(w, |s| s.noise_octaves = v.round().clamp(1.0, 8.0) as u32),
    );
    let persistence = labelled_drag(
        commands,
        fonts,
        "Persist",
        0.1,
        0.9,
        0.01,
        None,
        |w| get(w, |s| s.noise_persistence),
        |w, v| set(w, |s| s.noise_persistence = *v),
    );
    commands
        .entity(row)
        .add_children(&[mode, scale, octaves, persistence]);
    only_when_brush(commands, row, TerrainBrushType::Noise);
    row
}

fn terrace_opts(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = cluster(commands, "terrace");
    let steps = labelled_drag(
        commands,
        fonts,
        "Steps",
        2.0,
        32.0,
        0.1,
        Some(1.0),
        |w| get(w, |s| s.terrace_steps as f32),
        |w, v| set(w, |s| s.terrace_steps = v.round().clamp(2.0, 32.0) as u32),
    );
    let sharpness = labelled_drag(
        commands,
        fonts,
        "Sharp",
        0.0,
        1.0,
        0.01,
        None,
        |w| get(w, |s| s.terrace_sharpness),
        |w, v| set(w, |s| s.terrace_sharpness = *v),
    );
    commands.entity(row).add_children(&[steps, sharpness]);
    only_when_brush(commands, row, TerrainBrushType::Terrace);
    row
}

fn stamp_opts(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = cluster(commands, "stamp");
    let blend = labelled_dropdown(
        commands,
        fonts,
        "Blend",
        &["Add", "Subtract", "Replace", "Max", "Min"],
        72.0,
        |w| {
            let cur = get(w, |s| s.stamp_blend_mode);
            StampBlendMode::all()
                .iter()
                .position(|m| *m == cur)
                .unwrap_or(0)
        },
        |w, i| {
            let m = StampBlendMode::all().get(*i).copied().unwrap_or_default();
            set(w, |s| s.stamp_blend_mode = m);
        },
    );
    // Stored in radians, shown in degrees — nobody reasons about a stamp's
    // orientation in radians.
    let rotation = labelled_drag(
        commands,
        fonts,
        "Rot",
        0.0,
        360.0,
        1.0,
        None,
        |w| get(w, |s| s.stamp_rotation).to_degrees(),
        |w, v| set(w, |s| s.stamp_rotation = v.to_radians()),
    );
    let height = labelled_drag(
        commands,
        fonts,
        "Scale",
        0.01,
        2.0,
        0.01,
        None,
        |w| get(w, |s| s.stamp_height_scale),
        |w, v| set(w, |s| s.stamp_height_scale = *v),
    );
    commands.entity(row).add_children(&[blend, rotation, height]);
    only_when_brush(commands, row, TerrainBrushType::Stamp);
    row
}

// ── Click handlers ──────────────────────────────────────────────────────────

#[derive(Component)]
pub struct ShapeBtn(BrushShape);

#[derive(Component)]
pub struct FalloffBtn(BrushFalloffType);

pub fn shape_click(
    q: Query<(&Interaction, &ShapeBtn), Changed<Interaction>>,
    mut sculpt: ResMut<TerrainSettings>,
    mut paint: ResMut<SurfacePaintSettings>,
    tool: Res<ActiveTool>,
) {
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        // One set of buttons, two owners: write whichever resource the tool in
        // hand actually reads, or the shape would appear to do nothing.
        if *tool == ActiveTool::TerrainPaint {
            if paint.brush_shape != btn.0 {
                paint.brush_shape = btn.0;
            }
        } else if sculpt.brush_shape != btn.0 {
            sculpt.brush_shape = btn.0;
        }
    }
}

pub fn falloff_click(
    q: Query<(&Interaction, &FalloffBtn), Changed<Interaction>>,
    mut settings: ResMut<TerrainSettings>,
) {
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed && settings.falloff_type != btn.0 {
            settings.falloff_type = btn.0;
        }
    }
}

// ── Builders ────────────────────────────────────────────────────────────────

/// A horizontal run of related controls.
fn cluster(commands: &mut Commands, name: &str) -> Entity {
    commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
            Name::new(format!("terrain-bar:{name}")),
        ))
        .id()
}

/// Show `row` only while `pred` holds.
fn only_when(
    commands: &mut Commands,
    row: Entity,
    pred: impl Fn(&Rx) -> bool + Send + Sync + 'static,
) {
    bind_display(commands, row, pred);
}

/// Show `row` only while `brush` is the active sculpt brush.
fn only_when_brush(commands: &mut Commands, row: Entity, brush: TerrainBrushType) {
    bind_display(commands, row, move |w| {
        tool_is(w, ActiveTool::TerrainSculpt)
            && w.get_resource::<TerrainSettings>()
                .is_some_and(|s| s.brush_type == brush)
    });
}

/// `[Label 20.0] [────●───]`. The value rides in the label rather than sitting in
/// its own box: a toolbar has no room for a third element per setting, and the
/// number is only ever glanced at.
#[allow(clippy::too_many_arguments)]
fn labelled_slider<G, S>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &'static str,
    min: f32,
    max: f32,
    decimals: usize,
    get: G,
    set: S,
) -> Entity
where
    G: Fn(&Rx) -> f32 + Send + Sync + Copy + 'static,
    S: Fn(&mut World, &f32) + Send + Sync + 'static,
{
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_text(commands, text, move |w| {
        format!("{label} {:.*}", decimals, get(w))
    });
    // Seeded at `min`; `bind_2way` corrects it from the world on its first run,
    // before the user ever sees it.
    let sld = slider_ranged(commands, min, min, max);
    commands.entity(sld).insert(Node {
        width: Val::Px(SLIDER_W),
        height: Val::Px(18.0),
        position_type: PositionType::Relative,
        align_items: AlignItems::Center,
        ..default()
    });
    bind_2way(commands, sld, get, set);
    commands.entity(row).add_children(&[text, sld]);
    row
}

/// `[Label] [12.5]` — a scrubbable number. `snap` quantizes the model for fields
/// whose setter rounds into an integer; without it the model and the rounded
/// read-back fight each other mid-drag.
#[allow(clippy::too_many_arguments)]
fn labelled_drag<G, S>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    min: f32,
    max: f32,
    step: f32,
    snap: Option<f32>,
    get: G,
    set: S,
) -> Entity
where
    G: Fn(&Rx) -> f32 + Send + Sync + 'static,
    S: Fn(&mut World, &f32) + Send + Sync + 'static,
{
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(3.0),
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let dv = drag_value_flat(commands, &fonts.ui, "", value_text(), min, step);
    commands.entity(dv).insert(DragRange { min, max });
    if let Some(s) = snap {
        commands.entity(dv).insert(DragSnap(s));
    }
    bind_2way(commands, dv, get, set);
    commands.entity(row).add_children(&[text, dv]);
    row
}

fn labelled_dropdown<G, S>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    options: &[&str],
    width: f32,
    get: G,
    set: S,
) -> Entity
where
    G: Fn(&Rx) -> usize + Send + Sync + 'static,
    S: Fn(&mut World, &usize) + Send + Sync + 'static,
{
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let dd = dropdown_compact(commands, fonts, options, 0, width);
    bind_2way(commands, dd, get, set);
    commands.entity(row).add_children(&[text, dd]);
    row
}

fn toggle_button(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Px(22.0),
                height: Val::Px(22.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            Interaction::default(),
            RelativeCursorPosition::default(),
            HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("terrain-bar-toggle"),
        ))
        .id()
}

fn toggle_bg(w: &Rx, btn: Entity, active: bool) -> Color {
    if active {
        rgb(accent())
    } else if matches!(
        w.get::<Interaction>(btn),
        Some(Interaction::Hovered) | Some(Interaction::Pressed)
    ) {
        rgb(hover_bg())
    } else {
        rgb(card_bg())
    }
}

// ── Resource accessors ──────────────────────────────────────────────────────

fn tool_is(w: &Rx, want: ActiveTool) -> bool {
    w.get_resource::<ActiveTool>().copied() == Some(want)
}

fn get<T: Default>(w: &Rx, f: impl Fn(&TerrainSettings) -> T) -> T {
    w.get_resource::<TerrainSettings>().map(f).unwrap_or_default()
}

fn set(w: &mut World, f: impl FnOnce(&mut TerrainSettings)) {
    if let Some(mut s) = w.get_resource_mut::<TerrainSettings>() {
        f(&mut s);
    }
}

fn get_paint<T: Default>(w: &Rx, f: impl Fn(&SurfacePaintSettings) -> T) -> T {
    w.get_resource::<SurfacePaintSettings>()
        .map(f)
        .unwrap_or_default()
}

fn set_paint(w: &mut World, f: impl FnOnce(&mut SurfacePaintSettings)) {
    if let Some(mut s) = w.get_resource_mut::<SurfacePaintSettings>() {
        f(&mut s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use renzora_ember::font::icon_glyph;

    /// An unknown Phosphor name doesn't fail — `tool_button` and `icon_text`
    /// fall back to rendering the *name itself*, so a typo ships as the literal
    /// text "wave-sinee" sitting in a 22px button. Catch it here instead.
    #[test]
    fn every_icon_name_resolves() {
        for (_, icon) in SHAPE_ICONS {
            assert!(icon_glyph(icon).is_some(), "unknown shape icon {icon:?}");
        }
        for (_, icon, _) in FALLOFF_ICONS {
            assert!(icon_glyph(icon).is_some(), "unknown falloff icon {icon:?}");
        }
    }

    /// Every falloff curve the engine implements needs a button, or it becomes
    /// unreachable the moment the toolbar is the only place they live.
    #[test]
    fn every_falloff_curve_has_a_button() {
        for ft in [
            BrushFalloffType::Smooth,
            BrushFalloffType::Linear,
            BrushFalloffType::Spherical,
            BrushFalloffType::Tip,
            BrushFalloffType::Flat,
        ] {
            assert!(
                FALLOFF_ICONS.iter().any(|(f, ..)| *f == ft),
                "{ft:?} has no toolbar button"
            );
        }
    }
}
