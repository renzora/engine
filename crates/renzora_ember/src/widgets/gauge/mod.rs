//! Gauge — a circular arc dial (and the shared arc material used by the knob),
//! painted by a [`UiMaterial`] SDF shader.

use bevy::asset::Asset;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial};
use bevy::ui_render::UiMaterialPlugin;

use crate::font::{ui_font, EmberFonts};
use crate::theme::*;

/// 135° start, 270° sweep — a dial open at the bottom.
const A0: f32 = 2.356_194_5;
const SWEEP: f32 = 4.712_389;
const THICK: f32 = 0.2;

/// Registers the arc material + shader and the attach/sync systems.
pub(crate) struct GaugePlugin;

impl Plugin for GaugePlugin {
    fn build(&self, app: &mut App) {
        // Idempotent: both `WidgetsPlugin` (editor) and the markup
        // `vector::plugin` (shipped game) register this. Re-adding a
        // `UiMaterialPlugin` for the same material panics in bevy, and we'd also
        // double-add the attach/sync systems — so guard the whole plugin on the
        // material-plugin's presence.
        if app.is_plugin_added::<UiMaterialPlugin<ArcMaterial>>() {
            return;
        }
        bevy::asset::embedded_asset!(app, "gauge.wgsl");
        app.add_plugins(UiMaterialPlugin::<ArcMaterial>::default());
        app.add_systems(Update, (gauge_attach, arc_sync));
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(crate) struct ArcMaterial {
    #[uniform(0)]
    pub(crate) track: Vec4,
    #[uniform(0)]
    pub(crate) fill: Vec4,
    #[uniform(0)]
    pub(crate) params: Vec4,
}

impl UiMaterial for ArcMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_ember/widgets/gauge/gauge.wgsl".into()
    }
}

/// The 0..1 value an arc (gauge/knob) shows; turned into / kept in sync with a
/// material by [`gauge_attach`] / [`arc_sync`].
#[derive(Component)]
pub(crate) struct ArcData {
    pub(crate) value: f32,
}

fn make_arc(value: f32) -> ArcMaterial {
    let track = rgb(card_bg()).to_linear();
    let fill = rgb(accent()).to_linear();
    ArcMaterial {
        track: Vec4::new(track.red, track.green, track.blue, 1.0),
        fill: Vec4::new(fill.red, fill.green, fill.blue, 1.0),
        params: Vec4::new(value.clamp(0.0, 1.0), A0, SWEEP, THICK),
    }
}

/// Build an [`ArcMaterial`] with caller-supplied geometry + colors. Used by the
/// markup `vector="arc|gauge|ring"` / `speedometer` bridge to drive the dial
/// from a `VectorSpec` (arbitrary start/sweep/thickness/track/fill).
///
/// - `value` is the 0..1 fill fraction.
/// - `start`/`sweep` are in **radians**.
/// - `thick_frac` is the band thickness as a fraction of the radius.
pub(crate) fn make_arc_params(
    value: f32,
    start: f32,
    sweep: f32,
    thick_frac: f32,
    track: Color,
    fill: Color,
) -> ArcMaterial {
    let t = track.to_linear();
    let f = fill.to_linear();
    ArcMaterial {
        track: Vec4::new(t.red, t.green, t.blue, t.alpha),
        fill: Vec4::new(f.red, f.green, f.blue, f.alpha),
        params: Vec4::new(value.clamp(0.0, 1.0), start, sweep, thick_frac),
    }
}

pub(crate) fn gauge_attach(
    mut commands: Commands,
    mut materials: ResMut<Assets<ArcMaterial>>,
    arcs: Query<(Entity, &ArcData), Without<MaterialNode<ArcMaterial>>>,
) {
    for (e, d) in &arcs {
        // try_insert: the gauge entity may be despawned this same frame (panel teardown).
        commands
            .entity(e)
            .try_insert(MaterialNode(materials.add(make_arc(d.value))));
    }
}

pub(crate) fn arc_sync(
    mut materials: ResMut<Assets<ArcMaterial>>,
    arcs: Query<(&ArcData, &MaterialNode<ArcMaterial>), Changed<ArcData>>,
) {
    for (d, mat) in &arcs {
        if let Some(m) = materials.get_mut(&mat.0) {
            m.params.x = d.value.clamp(0.0, 1.0);
        }
    }
}

/// A circular gauge dial showing `value` (0..1) with a centered percent label.
pub fn gauge(commands: &mut Commands, fonts: &EmberFonts, value: f32) -> Entity {
    let g = commands
        .spawn((
            Node {
                width: Val::Px(86.0),
                height: Val::Px(86.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            ArcData {
                value: value.clamp(0.0, 1.0),
            },
            Pickable::IGNORE,
            Name::new("gauge"),
        ))
        .id();
    let label = commands
        .spawn((
            Text::new(format!("{}%", (value.clamp(0.0, 1.0) * 100.0) as i32)),
            ui_font(&fonts.ui, 14.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    commands.entity(g).add_child(label);
    g
}
