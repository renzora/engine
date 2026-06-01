//! Charts — a GPU-painted line/area chart (via [`UiMaterial`]), a pure-bevy_ui
//! bar chart, and a compact sparkline. Debug-graph style.

use bevy::asset::Asset;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial};
use bevy::ui_render::UiMaterialPlugin;

use crate::font::{ui_font, EmberFonts};
use crate::theme::{rgb, ACCENT_BLUE, TEXT_MUTED};

const MAX_SAMPLES: usize = 32;

/// Registers the chart material + shader and the attach system.
pub(crate) struct ChartPlugin;

impl Plugin for ChartPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "chart.wgsl");
        app.add_plugins(UiMaterialPlugin::<ChartMaterial>::default());
        app.add_systems(Update, chart_attach);
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

/// Holds the raw chart samples until [`chart_attach`] turns them into a material.
#[derive(Component)]
pub(crate) struct ChartData {
    values: Vec<f32>,
}

fn chart_node(commands: &mut Commands, values: &[f32], width: f32, height: f32) -> Entity {
    let outer = commands
        .spawn((
            Node {
                width: Val::Px(width),
                height: Val::Px(height),
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
            ChartData {
                values: values.to_vec(),
            },
            Pickable::IGNORE,
            Name::new("chart-plot"),
        ))
        .id();
    commands.entity(outer).add_child(plot);
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
        if cd.values.len() < 2 {
            continue;
        }
        let min = cd.values.iter().cloned().fold(f32::MAX, f32::min);
        let max = cd.values.iter().cloned().fold(f32::MIN, f32::max);
        let range = (max - min).max(1e-4);
        let n = cd.values.len().min(MAX_SAMPLES);
        let mut flat = [0.0f32; MAX_SAMPLES];
        for (i, v) in cd.values.iter().take(MAX_SAMPLES).enumerate() {
            flat[i] = (v - min) / range;
        }
        let mut data = [Vec4::ZERO; 8];
        for (g, slot) in data.iter_mut().enumerate() {
            *slot = Vec4::new(flat[g * 4], flat[g * 4 + 1], flat[g * 4 + 2], flat[g * 4 + 3]);
        }
        let accent = rgb(ACCENT_BLUE).to_linear();
        let material = ChartMaterial {
            data,
            color: Vec4::new(accent.red, accent.green, accent.blue, 1.0),
            params: Vec4::new(n as f32, 2.0, 0.22, 0.0),
        };
        commands
            .entity(e)
            .insert(MaterialNode(materials.add(material)));
    }
}
