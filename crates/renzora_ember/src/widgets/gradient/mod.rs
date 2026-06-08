//! Gradient editor — a horizontal color ramp painted by a `UiMaterial` with
//! draggable color stops (for particle colors, ramps, color-over-life, etc.).

use bevy::asset::Asset;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui::RelativeCursorPosition;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial};
use bevy::ui_render::UiMaterialPlugin;
use bevy::window::SystemCursorIcon;

use crate::theme::*;

const MAX_STOPS: usize = 6;

pub(crate) struct GradientEditorPlugin;

impl Plugin for GradientEditorPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "gradient.wgsl");
        app.add_plugins(UiMaterialPlugin::<GradientMaterial>::default());
        app.add_systems(Update, (gradient_attach, gradient_sync, gradient_drag).chain());
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(crate) struct GradientMaterial {
    #[uniform(0)]
    colors: [Vec4; MAX_STOPS],
    #[uniform(0)]
    params: Vec4,
}

#[derive(Clone, Copy)]
pub(crate) struct GradStop {
    pos: f32,
    color: Color,
}

#[derive(Component)]
pub(crate) struct GradientData {
    stops: Vec<GradStop>,
}

#[derive(Component)]
pub(crate) struct GradientStop {
    data: Entity,
    strip: Entity,
    idx: usize,
}

impl UiMaterial for GradientMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_ember/widgets/gradient/gradient.wgsl".into()
    }
}

fn pack(stops: &[GradStop]) -> ([Vec4; MAX_STOPS], Vec4) {
    let mut colors = [Vec4::ZERO; MAX_STOPS];
    let n = stops.len().min(MAX_STOPS);
    for (i, s) in stops.iter().take(MAX_STOPS).enumerate() {
        let c = s.color.to_srgba();
        colors[i] = Vec4::new(c.red, c.green, c.blue, s.pos);
    }
    (colors, Vec4::new(n as f32, 0.0, 0.0, 0.0))
}

/// A gradient editor with draggable stops. Each stop is `(position 0..1, rgb)`.
pub fn gradient_editor(commands: &mut Commands, stops: &[(f32, (u8, u8, u8))]) -> Entity {
    let stops: Vec<GradStop> = stops
        .iter()
        .take(MAX_STOPS)
        .map(|(p, c)| GradStop {
            pos: *p,
            color: rgb(*c),
        })
        .collect();
    let root = commands
        .spawn((
            Node {
                width: Val::Px(220.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            Name::new("gradient-editor"),
        ))
        .id();
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(26.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BorderColor::all(rgb(border())),
            GradientData {
                stops: stops.clone(),
            },
            Name::new("gradient-bar"),
        ))
        .id();
    let strip = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(14.0),
                position_type: PositionType::Relative,
                margin: UiRect::top(Val::Px(2.0)),
                ..default()
            },
            RelativeCursorPosition::default(),
            Name::new("gradient-strip"),
        ))
        .id();
    let mut handles = Vec::new();
    for (idx, s) in stops.iter().enumerate() {
        let h = commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(s.pos * 100.0),
                    top: Val::Px(0.0),
                    margin: UiRect::left(Val::Px(-5.0)),
                    width: Val::Px(10.0),
                    height: Val::Px(10.0),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(s.color),
                BorderColor::all(rgb(text_primary())),
                Interaction::default(),
                GradientStop {
                    data: bar,
                    strip,
                    idx,
                },
                renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::EwResize),
                Name::new("gradient-stop"),
            ))
            .id();
        handles.push(h);
    }
    commands.entity(strip).add_children(&handles);
    commands.entity(root).add_children(&[bar, strip]);
    root
}

fn gradient_attach(
    mut commands: Commands,
    mut materials: ResMut<Assets<GradientMaterial>>,
    bars: Query<(Entity, &GradientData), Without<MaterialNode<GradientMaterial>>>,
) {
    for (e, d) in &bars {
        let (colors, params) = pack(&d.stops);
        let handle = materials.add(GradientMaterial { colors, params });
        // try_insert: the gradient entity may be despawned this same frame (panel teardown).
        commands.entity(e).try_insert(MaterialNode(handle));
    }
}

fn gradient_sync(
    mut materials: ResMut<Assets<GradientMaterial>>,
    bars: Query<(&GradientData, &MaterialNode<GradientMaterial>), Changed<GradientData>>,
) {
    for (d, mat) in &bars {
        if let Some(m) = materials.get_mut(&mat.0) {
            let (colors, params) = pack(&d.stops);
            m.colors = colors;
            m.params = params;
        }
    }
}

fn gradient_drag(
    handles: Query<(Entity, &Interaction, &GradientStop)>,
    strips: Query<&RelativeCursorPosition>,
    mut datas: Query<&mut GradientData>,
    mut nodes: Query<&mut Node>,
) {
    for (e, interaction, stop) in &handles {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Ok(rcp) = strips.get(stop.strip) else {
            continue;
        };
        let Some(n) = rcp.normalized else {
            continue;
        };
        let pos = (n.x + 0.5).clamp(0.0, 1.0);
        if let Ok(mut d) = datas.get_mut(stop.data) {
            if let Some(s) = d.stops.get_mut(stop.idx) {
                s.pos = pos;
            }
        }
        if let Ok(mut node) = nodes.get_mut(e) {
            node.left = Val::Percent(pos * 100.0);
        }
    }
}
