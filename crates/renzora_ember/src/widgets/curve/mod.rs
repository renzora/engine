//! Curve editor — a draggable cubic-bezier easing curve (for tweening) painted by
//! a `UiMaterial`, with two control handles over a grid.

use bevy::asset::Asset;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui::RelativeCursorPosition;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial};
use bevy::ui_render::UiMaterialPlugin;
use bevy::window::SystemCursorIcon;

use crate::theme::{rgb, ACCENT_BLUE};

pub(crate) struct CurveEditorPlugin;

impl Plugin for CurveEditorPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "curve.wgsl");
        app.add_plugins(UiMaterialPlugin::<CurveMaterial>::default());
        app.add_systems(Update, (curve_attach, curve_sync, curve_drag).chain());
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(crate) struct CurveMaterial {
    #[uniform(0)]
    ab: Vec4,
    #[uniform(0)]
    cd: Vec4,
    #[uniform(0)]
    color: Vec4,
    #[uniform(0)]
    params: Vec4,
}

impl UiMaterial for CurveMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_ember/widgets/curve/curve.wgsl".into()
    }
}

/// Control points of the easing curve. `p0`/`p3` are pinned to the corners; only
/// `p1`/`p2` move. Coordinates are 0..1 with y pointing up.
#[derive(Component)]
pub(crate) struct CurveData {
    p1: Vec2,
    p2: Vec2,
}

#[derive(Component)]
pub(crate) struct CurveHandle {
    root: Entity,
    idx: u8,
}

fn handle(commands: &mut Commands, root: Entity, idx: u8, p: Vec2) -> Entity {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(p.x * 100.0),
                top: Val::Percent((1.0 - p.y) * 100.0),
                margin: UiRect::all(Val::Px(-6.0)),
                width: Val::Px(12.0),
                height: Val::Px(12.0),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb((24, 24, 30))),
            BorderColor::all(rgb(ACCENT_BLUE)),
            Interaction::default(),
            CurveHandle { root, idx },
            renzora_hui::cursor_icon::HoverCursor(SystemCursorIcon::Grab),
            Name::new("curve-handle"),
        ))
        .id()
}

/// A bezier easing-curve editor. `p1`/`p2` set the initial tangents (0..1).
pub fn curve_editor(commands: &mut Commands, p1: Vec2, p2: Vec2) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Px(180.0),
                height: Val::Px(130.0),
                position_type: PositionType::Relative,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BorderColor::all(rgb((70, 70, 82))),
            CurveData { p1, p2 },
            RelativeCursorPosition::default(),
            Name::new("curve-editor"),
        ))
        .id();
    let h1 = handle(commands, root, 0, p1);
    let h2 = handle(commands, root, 1, p2);
    commands.entity(root).add_children(&[h1, h2]);
    root
}

fn curve_attach(
    mut commands: Commands,
    mut materials: ResMut<Assets<CurveMaterial>>,
    curves: Query<(Entity, &CurveData), Without<MaterialNode<CurveMaterial>>>,
) {
    for (e, d) in &curves {
        let c = rgb(ACCENT_BLUE).to_linear();
        let handle = materials.add(CurveMaterial {
            ab: Vec4::new(0.0, 0.0, d.p1.x, d.p1.y),
            cd: Vec4::new(d.p2.x, d.p2.y, 1.0, 1.0),
            color: Vec4::new(c.red, c.green, c.blue, 1.0),
            params: Vec4::new(2.5, 0.0, 0.0, 0.0),
        });
        commands.entity(e).insert(MaterialNode(handle));
    }
}

fn curve_sync(
    mut materials: ResMut<Assets<CurveMaterial>>,
    curves: Query<(&CurveData, &MaterialNode<CurveMaterial>), Changed<CurveData>>,
) {
    for (d, mat) in &curves {
        if let Some(m) = materials.get_mut(&mat.0) {
            m.ab = Vec4::new(0.0, 0.0, d.p1.x, d.p1.y);
            m.cd = Vec4::new(d.p2.x, d.p2.y, 1.0, 1.0);
        }
    }
}

fn curve_drag(
    handles: Query<(Entity, &Interaction, &CurveHandle)>,
    roots: Query<&RelativeCursorPosition>,
    mut datas: Query<&mut CurveData>,
    mut nodes: Query<&mut Node>,
) {
    for (e, interaction, h) in &handles {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Ok(rcp) = roots.get(h.root) else {
            continue;
        };
        let Some(n) = rcp.normalized else {
            continue;
        };
        let pt = Vec2::new((n.x + 0.5).clamp(0.0, 1.0), (0.5 - n.y).clamp(0.0, 1.0));
        if let Ok(mut d) = datas.get_mut(h.root) {
            if h.idx == 0 {
                d.p1 = pt;
            } else {
                d.p2 = pt;
            }
        }
        if let Ok(mut node) = nodes.get_mut(e) {
            node.left = Val::Percent(pt.x * 100.0);
            node.top = Val::Percent((1.0 - pt.y) * 100.0);
        }
    }
}
