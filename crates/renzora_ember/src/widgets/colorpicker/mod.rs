//! HSV color picker — a saturation/value square + hue strip (painted by a
//! `UiMaterial`) with draggable handles and a live preview swatch.

use bevy::asset::Asset;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui::RelativeCursorPosition;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial};
use bevy::ui_render::UiMaterialPlugin;
use bevy::window::SystemCursorIcon;

use crate::theme::rgb;

pub(crate) struct ColorPickerPlugin;

impl Plugin for ColorPickerPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "colorpicker.wgsl");
        app.add_plugins(UiMaterialPlugin::<PickerMaterial>::default());
        app.add_systems(Update, (picker_attach, picker_sync, hsv_drag));
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(crate) struct PickerMaterial {
    #[uniform(0)]
    params: Vec4,
}

impl UiMaterial for PickerMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_ember/widgets/colorpicker/colorpicker.wgsl".into()
    }
}

#[derive(Component)]
pub(crate) struct PickerData {
    mode: f32,
    hue: f32,
}

#[derive(Component)]
pub(crate) struct PickerSquare {
    root: Entity,
}

#[derive(Component)]
pub(crate) struct PickerHue {
    root: Entity,
}

#[derive(Component)]
pub(crate) struct HsvPicker {
    hue: f32,
    s: f32,
    v: f32,
    sv: Entity,
    sv_handle: Entity,
    hue_handle: Entity,
    preview: Entity,
}

/// An HSV color picker (sat/val square + hue strip + preview). `hue/s/v` in 0..1.
pub fn hsv_picker(commands: &mut Commands, hue: f32, s: f32, v: f32) -> Entity {
    let root = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::FlexStart,
                column_gap: Val::Px(8.0),
                ..default()
            },
            Name::new("hsv-picker"),
        ))
        .id();
    let sv = commands
        .spawn((
            Node {
                width: Val::Px(120.0),
                height: Val::Px(120.0),
                position_type: PositionType::Relative,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            PickerData { mode: 0.0, hue },
            PickerSquare { root },
            Interaction::default(),
            RelativeCursorPosition::default(),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Crosshair),
            Name::new("sv-square"),
        ))
        .id();
    let sv_handle = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(s * 100.0),
                top: Val::Percent((1.0 - v) * 100.0),
                margin: UiRect::all(Val::Px(-5.0)),
                width: Val::Px(10.0),
                height: Val::Px(10.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BorderColor::all(rgb((250, 250, 250))),
            bevy::ui::FocusPolicy::Pass,
            Name::new("sv-handle"),
        ))
        .id();
    commands.entity(sv).add_child(sv_handle);
    let hue_node = commands
        .spawn((
            Node {
                width: Val::Px(16.0),
                height: Val::Px(120.0),
                position_type: PositionType::Relative,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            PickerData { mode: 1.0, hue: 0.0 },
            PickerHue { root },
            Interaction::default(),
            RelativeCursorPosition::default(),
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Crosshair),
            Name::new("hue-strip"),
        ))
        .id();
    let hue_handle = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Percent(hue * 100.0),
                margin: UiRect::top(Val::Px(-1.5)),
                width: Val::Percent(100.0),
                height: Val::Px(3.0),
                ..default()
            },
            BackgroundColor(rgb((250, 250, 250))),
            bevy::ui::FocusPolicy::Pass,
            Name::new("hue-handle"),
        ))
        .id();
    commands.entity(hue_node).add_child(hue_handle);
    let preview = commands
        .spawn((
            Node {
                width: Val::Px(28.0),
                height: Val::Px(28.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(Color::hsv(hue * 360.0, s, v)),
            BorderColor::all(rgb((70, 70, 82))),
            Name::new("color-preview"),
        ))
        .id();
    commands.entity(root).add_children(&[sv, hue_node, preview]);
    commands.entity(root).insert(HsvPicker {
        hue,
        s,
        v,
        sv,
        sv_handle,
        hue_handle,
        preview,
    });
    root
}

fn picker_attach(
    mut commands: Commands,
    mut materials: ResMut<Assets<PickerMaterial>>,
    surfaces: Query<(Entity, &PickerData), Without<MaterialNode<PickerMaterial>>>,
) {
    for (e, d) in &surfaces {
        let handle = materials.add(PickerMaterial {
            params: Vec4::new(d.mode, d.hue, 0.0, 0.0),
        });
        commands.entity(e).insert(MaterialNode(handle));
    }
}

fn picker_sync(
    mut materials: ResMut<Assets<PickerMaterial>>,
    surfaces: Query<(&PickerData, &MaterialNode<PickerMaterial>), Changed<PickerData>>,
) {
    for (d, mat) in &surfaces {
        if let Some(m) = materials.get_mut(&mat.0) {
            m.params = Vec4::new(d.mode, d.hue, 0.0, 0.0);
        }
    }
}

fn apply_picker(
    p: &HsvPicker,
    nodes: &mut Query<&mut Node>,
    bgs: &mut Query<&mut BackgroundColor>,
) {
    if let Ok(mut n) = nodes.get_mut(p.sv_handle) {
        n.left = Val::Percent(p.s * 100.0);
        n.top = Val::Percent((1.0 - p.v) * 100.0);
    }
    if let Ok(mut n) = nodes.get_mut(p.hue_handle) {
        n.top = Val::Percent(p.hue * 100.0);
    }
    if let Ok(mut bg) = bgs.get_mut(p.preview) {
        bg.0 = Color::hsv(p.hue * 360.0, p.s, p.v);
    }
}

fn hsv_drag(
    squares: Query<(&Interaction, &RelativeCursorPosition, &PickerSquare)>,
    hues: Query<(&Interaction, &RelativeCursorPosition, &PickerHue)>,
    mut pickers: Query<&mut HsvPicker>,
    mut datas: Query<&mut PickerData>,
    mut nodes: Query<&mut Node>,
    mut bgs: Query<&mut BackgroundColor>,
) {
    for (interaction, rcp, sq) in &squares {
        if *interaction == Interaction::Pressed {
            if let (Some(n), Ok(mut p)) = (rcp.normalized, pickers.get_mut(sq.root)) {
                p.s = (n.x + 0.5).clamp(0.0, 1.0);
                p.v = (0.5 - n.y).clamp(0.0, 1.0);
                apply_picker(&p, &mut nodes, &mut bgs);
            }
        }
    }
    for (interaction, rcp, hu) in &hues {
        if *interaction == Interaction::Pressed {
            if let (Some(n), Ok(mut p)) = (rcp.normalized, pickers.get_mut(hu.root)) {
                p.hue = (n.y + 0.5).clamp(0.0, 1.0);
                if let Ok(mut d) = datas.get_mut(p.sv) {
                    d.hue = p.hue;
                }
                apply_picker(&p, &mut nodes, &mut bgs);
            }
        }
    }
}
