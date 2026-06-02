//! Charts — a GPU-painted line/area chart (via [`UiMaterial`]), a pure-bevy_ui
//! bar chart, and a compact sparkline. Debug-graph style.
//!
//! [`line_chart_live`] is the reactive variant: it re-samples a closure each
//! frame and updates the plot in place (fixed range, target line, custom color),
//! which is what the debug/diagnostic panels use for rolling FPS/GPU/entity
//! graphs.

use bevy::asset::Asset;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial};
use bevy::ui_render::UiMaterialPlugin;

use crate::font::{ui_font, EmberFonts};
use crate::reactive::bind_with;
use crate::theme::{rgb, ACCENT_BLUE, TEXT_MUTED};

const MAX_SAMPLES: usize = 32;

/// Registers the chart material + shader and the attach/sync systems.
pub(crate) struct ChartPlugin;

impl Plugin for ChartPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "chart.wgsl");
        app.add_plugins(UiMaterialPlugin::<ChartMaterial>::default());
        app.add_systems(Update, (chart_attach, chart_sync));
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(crate) struct ChartMaterial {
    #[uniform(0)]
    data: [Vec4; 8],
    #[uniform(0)]
    color: Vec4,
    #[uniform(0)]
    params: Vec4,
}

impl UiMaterial for ChartMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_ember/widgets/chart/chart.wgsl".into()
    }
}

/// Holds the raw chart samples + style; [`chart_attach`]/[`chart_sync`] turn it
/// into the material uniforms.
#[derive(Component)]
pub(crate) struct ChartData {
    values: Vec<f32>,
    /// Fixed range; `None` auto-fits to the visible samples.
    min: Option<f32>,
    max: Option<f32>,
    color: Color,
    /// Raw value at which to draw the target line (`None` = no line).
    target: Option<f32>,
}

impl ChartData {
    fn solid(values: Vec<f32>) -> Self {
        Self {
            values,
            min: None,
            max: None,
            color: rgb(ACCENT_BLUE),
            target: None,
        }
    }
}

/// Build the material uniforms for the current samples + style, or `None` if
/// there isn't enough data to draw a line yet.
fn chart_material(cd: &ChartData) -> Option<ChartMaterial> {
    if cd.values.len() < 2 {
        return None;
    }
    // Show the most recent MAX_SAMPLES (a rolling window for time series).
    let start = cd.values.len().saturating_sub(MAX_SAMPLES);
    let slice = &cd.values[start..];

    let auto_min = slice.iter().cloned().fold(f32::MAX, f32::min);
    let auto_max = slice.iter().cloned().fold(f32::MIN, f32::max);
    let min = cd.min.unwrap_or(auto_min);
    let max = cd.max.unwrap_or(auto_max);
    let range = (max - min).max(1e-4);

    let n = slice.len().min(MAX_SAMPLES);
    let mut flat = [0.0f32; MAX_SAMPLES];
    for (i, v) in slice.iter().take(MAX_SAMPLES).enumerate() {
        flat[i] = ((v - min) / range).clamp(0.0, 1.0);
    }
    let mut data = [Vec4::ZERO; 8];
    for (g, slot) in data.iter_mut().enumerate() {
        *slot = Vec4::new(flat[g * 4], flat[g * 4 + 1], flat[g * 4 + 2], flat[g * 4 + 3]);
    }
    let lin = cd.color.to_linear();
    let target = cd
        .target
        .map(|t| (t - min) / range)
        .filter(|v| (0.0..=1.0).contains(v))
        .unwrap_or(-1.0);
    Some(ChartMaterial {
        data,
        color: Vec4::new(lin.red, lin.green, lin.blue, 1.0),
        params: Vec4::new(n as f32, 2.0, 0.22, target),
    })
}

/// The chart frame (border + clip) and its absolutely-filled plot node. Returns
/// `(outer, plot)` — bind the plot's [`ChartData`] to drive it live.
fn chart_shell(commands: &mut Commands, width: Val, height: Val, data: ChartData) -> (Entity, Entity) {
    let outer = commands
        .spawn((
            Node {
                width,
                height,
                position_type: PositionType::Relative,
                overflow: Overflow::clip(),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb((24, 24, 30))),
            BorderColor::all(rgb((48, 48, 58))),
            Name::new("chart"),
        ))
        .id();
    let plot = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            data,
            Pickable::IGNORE,
            Name::new("chart-plot"),
        ))
        .id();
    commands.entity(outer).add_child(plot);
    (outer, plot)
}

fn chart_node(commands: &mut Commands, values: &[f32], width: f32, height: f32) -> Entity {
    let (outer, _) = chart_shell(
        commands,
        Val::Px(width),
        Val::Px(height),
        ChartData::solid(values.to_vec()),
    );
    outer
}

fn axis_label(commands: &mut Commands, font: &Handle<Font>, text: &str, top: bool) -> Entity {
    commands
        .spawn((
            Text::new(text),
            ui_font(font, 9.0),
            TextColor(rgb(TEXT_MUTED)),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(3.0),
                top: if top { Val::Px(2.0) } else { Val::Auto },
                bottom: if top { Val::Auto } else { Val::Px(2.0) },
                ..default()
            },
        ))
        .id()
}

/// A line + area chart of `values` (any range; auto-normalized) with grid +
/// min/max value labels.
pub fn line_chart(commands: &mut Commands, fonts: &EmberFonts, values: &[f32]) -> Entity {
    let node = chart_node(commands, values, 200.0, 80.0);
    if values.len() >= 2 {
        let max = values.iter().cloned().fold(f32::MIN, f32::max);
        let min = values.iter().cloned().fold(f32::MAX, f32::min);
        let top = axis_label(commands, &fonts.ui, &format!("{max:.1}"), true);
        let bot = axis_label(commands, &fonts.ui, &format!("{min:.1}"), false);
        commands.entity(node).add_children(&[top, bot]);
    }
    node
}

/// A compact inline sparkline.
pub fn sparkline(commands: &mut Commands, values: &[f32]) -> Entity {
    chart_node(commands, values, 110.0, 28.0)
}

/// Styling for a [`line_chart_live`].
pub struct ChartStyle {
    pub color: Color,
    /// Fixed value range; leave `None` to auto-fit the visible window.
    pub min: Option<f32>,
    pub max: Option<f32>,
    /// Optional target line (e.g. the 60-FPS / 16.67-ms budget line).
    pub target: Option<f32>,
    pub height: f32,
}

impl Default for ChartStyle {
    fn default() -> Self {
        Self {
            color: rgb(ACCENT_BLUE),
            min: None,
            max: None,
            target: None,
            height: 48.0,
        }
    }
}

/// A full-width, reactive line/area chart: `sampler` is re-read each frame and
/// the plot is updated in place (value-diffed — no work when the data is
/// unchanged). Use for rolling debug graphs.
pub fn line_chart_live<F>(commands: &mut Commands, style: ChartStyle, sampler: F) -> Entity
where
    F: Fn(&World) -> Vec<f32> + Send + Sync + 'static,
{
    let (outer, plot) = chart_shell(
        commands,
        Val::Percent(100.0),
        Val::Px(style.height),
        ChartData {
            values: Vec::new(),
            min: style.min,
            max: style.max,
            color: style.color,
            target: style.target,
        },
    );
    bind_with(commands, plot, sampler, |w, e, v: &Vec<f32>| {
        if let Some(mut cd) = w.get_mut::<ChartData>(e) {
            cd.values.clone_from(v);
        }
    });
    outer
}

/// A simple vertical bar chart (pure bevy_ui rectangles).
pub fn bar_chart(commands: &mut Commands, values: &[f32]) -> Entity {
    let outer = commands
        .spawn((
            Node {
                width: Val::Px(200.0),
                height: Val::Px(80.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::FlexEnd,
                column_gap: Val::Px(3.0),
                padding: UiRect::all(Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb((24, 24, 30))),
            BorderColor::all(rgb((48, 48, 58))),
            Name::new("bar-chart"),
        ))
        .id();
    let max = values.iter().cloned().fold(f32::MIN, f32::max).max(1e-4);
    let bars: Vec<Entity> = values
        .iter()
        .map(|v| {
            let h = (v / max).clamp(0.0, 1.0);
            commands
                .spawn((
                    Node {
                        flex_grow: 1.0,
                        min_width: Val::Px(3.0),
                        height: Val::Percent(h * 100.0),
                        border_radius: BorderRadius::top(Val::Px(2.0)),
                        ..default()
                    },
                    BackgroundColor(rgb(ACCENT_BLUE)),
                    Name::new("bar"),
                ))
                .id()
        })
        .collect();
    commands.entity(outer).add_children(&bars);
    outer
}

fn chart_attach(
    mut commands: Commands,
    mut materials: ResMut<Assets<ChartMaterial>>,
    charts: Query<(Entity, &ChartData), Without<MaterialNode<ChartMaterial>>>,
) {
    for (e, cd) in &charts {
        if let Some(material) = chart_material(cd) {
            commands
                .entity(e)
                .insert(MaterialNode(materials.add(material)));
        }
    }
}

fn chart_sync(
    mut materials: ResMut<Assets<ChartMaterial>>,
    charts: Query<(&ChartData, &MaterialNode<ChartMaterial>), Changed<ChartData>>,
) {
    for (cd, node) in &charts {
        if let Some(material) = chart_material(cd) {
            if let Some(slot) = materials.get_mut(&node.0) {
                *slot = material;
            }
        }
    }
}
