//! Bevy-native (ember) Gamepad debug panel — a faithful bevy_ui port of the egui
//! panel: per-controller analog sticks (drawn by a `UiMaterial`), trigger bars,
//! a button grid, and the non-zero raw-axes list.
//!
//! Structure is rebuilt only when the connected-gamepad count changes; the live
//! values (stick position, trigger fill, button highlight, axis readouts) are
//! updated in place each frame from `GamepadDebugState`.

use bevy::asset::Asset;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial};
use bevy::ui_render::UiMaterialPlugin;

use renzora_ember::dock::{tab_pane, DockLeaf, TabPane};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::theme::rgb;

use crate::state::{GamepadButtonState, GamepadDebugState};

const PANEL_ID: &str = "gamepad";

fn gray(v: u8) -> Color {
    rgb((v, v, v))
}

// ── Stick material (UiMaterial) ──────────────────────────────────────────────

#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(crate) struct StickMaterial {
    #[uniform(0)]
    params: Vec4,
}

impl UiMaterial for StickMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://renzora_gamepad/stick.wgsl".into()
    }
}

#[derive(Component, Default)]
pub(crate) struct StickData {
    x: f32,
    y: f32,
}

fn stick_attach(
    mut commands: Commands,
    mut materials: ResMut<Assets<StickMaterial>>,
    sticks: Query<(Entity, &StickData), Without<MaterialNode<StickMaterial>>>,
) {
    for (e, d) in &sticks {
        let h = materials.add(StickMaterial {
            params: Vec4::new(d.x, d.y, 0.0, 0.0),
        });
        commands.entity(e).insert(MaterialNode(h));
    }
}

fn stick_sync(
    mut materials: ResMut<Assets<StickMaterial>>,
    sticks: Query<(&StickData, &MaterialNode<StickMaterial>), Changed<StickData>>,
) {
    for (d, mat) in &sticks {
        if let Some(m) = materials.get_mut(&mat.0) {
            m.params = Vec4::new(d.x, d.y, 0.0, 0.0);
        }
    }
}

// ── Components ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Side {
    Left,
    Right,
}

#[derive(Clone, Copy)]
enum GpBtn {
    South,
    East,
    West,
    North,
    Lb,
    Rb,
    Lt2,
    Rt2,
    Up,
    Down,
    Left,
    Right,
    Start,
    Select,
    L3,
    R3,
}

impl GpBtn {
    fn pressed(self, b: &GamepadButtonState) -> bool {
        match self {
            GpBtn::South => b.south,
            GpBtn::East => b.east,
            GpBtn::West => b.west,
            GpBtn::North => b.north,
            GpBtn::Lb => b.left_trigger,
            GpBtn::Rb => b.right_trigger,
            GpBtn::Lt2 => b.left_trigger2,
            GpBtn::Rt2 => b.right_trigger2,
            GpBtn::Up => b.dpad_up,
            GpBtn::Down => b.dpad_down,
            GpBtn::Left => b.dpad_left,
            GpBtn::Right => b.dpad_right,
            GpBtn::Start => b.start,
            GpBtn::Select => b.select,
            GpBtn::L3 => b.left_thumb,
            GpBtn::R3 => b.right_thumb,
        }
    }
}

/// On the list container: connected-gamepad count last built (rebuild on change).
#[derive(Component)]
pub(crate) struct GamepadRoot {
    count: usize,
}

#[derive(Component)]
pub(crate) struct GpStick {
    pad: usize,
    side: Side,
}
#[derive(Component)]
pub(crate) struct GpStickLabel {
    pad: usize,
    side: Side,
}
#[derive(Component)]
pub(crate) struct GpTriggerFill {
    pad: usize,
    side: Side,
}
#[derive(Component)]
pub(crate) struct GpTriggerLabel {
    pad: usize,
    side: Side,
}
#[derive(Component)]
pub(crate) struct GpButton {
    pad: usize,
    btn: GpBtn,
    label: Entity,
}
#[derive(Component)]
pub(crate) struct GpRawAxes {
    pad: usize,
    last: Vec<String>,
}

// ── Build ───────────────────────────────────────────────────────────────────

fn text(commands: &mut Commands, fonts: &EmberFonts, s: &str, size: f32, color: Color) -> Entity {
    commands
        .spawn((Text::new(s), ui_font(&fonts.ui, size), TextColor(color)))
        .id()
}

fn col(commands: &mut Commands, gap: f32) -> Entity {
    commands
        .spawn((Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(gap),
            ..default()
        },))
        .id()
}

fn row(commands: &mut Commands, gap: f32) -> Entity {
    commands
        .spawn((Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::FlexStart,
            column_gap: Val::Px(gap),
            ..default()
        },))
        .id()
}

fn stick_widget(commands: &mut Commands, fonts: &EmberFonts, pad: usize, side: Side, name: &str) -> Entity {
    let c = col(commands, 4.0);
    let label = text(commands, fonts, name, 11.0, gray(150));
    let stick = commands
        .spawn((
            Node {
                width: Val::Px(80.0),
                height: Val::Px(80.0),
                ..default()
            },
            StickData::default(),
            GpStick { pad, side },
            Name::new("gp-stick"),
        ))
        .id();
    let value = commands
        .spawn((
            Text::new("X: 0.00  Y: 0.00"),
            ui_font(&fonts.ui, 10.0),
            TextColor(gray(120)),
            GpStickLabel { pad, side },
        ))
        .id();
    commands.entity(c).add_children(&[label, stick, value]);
    c
}

fn trigger_widget(commands: &mut Commands, fonts: &EmberFonts, pad: usize, side: Side, name: &str) -> Entity {
    let c = col(commands, 4.0);
    let label = text(commands, fonts, name, 10.0, gray(220));
    let bar = commands
        .spawn((
            Node {
                width: Val::Px(30.0),
                height: Val::Px(60.0),
                position_type: PositionType::Relative,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(2.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BorderColor::all(gray(60)),
            Name::new("gp-trigger"),
        ))
        .id();
    let fill = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(1.0),
                right: Val::Px(1.0),
                bottom: Val::Px(1.0),
                height: Val::Px(0.0),
                ..default()
            },
            BackgroundColor(gray(80)),
            GpTriggerFill { pad, side },
            Name::new("gp-trigger-fill"),
        ))
        .id();
    commands.entity(bar).add_child(fill);
    let value = commands
        .spawn((
            Text::new("0.00"),
            ui_font(&fonts.ui, 10.0),
            TextColor(gray(120)),
            GpTriggerLabel { pad, side },
        ))
        .id();
    commands.entity(c).add_children(&[label, bar, value]);
    c
}

fn button(commands: &mut Commands, fonts: &EmberFonts, pad: usize, label: &str, btn: GpBtn) -> Entity {
    let b = commands
        .spawn((
            Node {
                width: Val::Px(60.0),
                height: Val::Px(24.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb((50, 52, 58))),
            BorderColor::all(gray(70)),
            Name::new("gp-button"),
        ))
        .id();
    let tx = text(commands, fonts, label, 10.0, gray(120));
    commands
        .entity(b)
        .insert(GpButton { pad, btn, label: tx });
    commands.entity(b).add_child(tx);
    b
}

const BUTTON_ROWS: [&[(&str, GpBtn)]; 4] = [
    &[
        ("A/Cross", GpBtn::South),
        ("B/Circle", GpBtn::East),
        ("X/Square", GpBtn::West),
        ("Y/Triangle", GpBtn::North),
    ],
    &[
        ("LB", GpBtn::Lb),
        ("RB", GpBtn::Rb),
        ("LT", GpBtn::Lt2),
        ("RT", GpBtn::Rt2),
    ],
    &[
        ("Up", GpBtn::Up),
        ("Down", GpBtn::Down),
        ("Left", GpBtn::Left),
        ("Right", GpBtn::Right),
    ],
    &[
        ("Start", GpBtn::Start),
        ("Select", GpBtn::Select),
        ("L3", GpBtn::L3),
        ("R3", GpBtn::R3),
    ],
];

fn hsep(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(1.0),
                margin: UiRect::vertical(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(gray(60)),
        ))
        .id()
}

fn spacer(commands: &mut Commands, h: f32) -> Entity {
    commands
        .spawn((Node {
            height: Val::Px(h),
            ..default()
        },))
        .id()
}

fn build_gamepad(commands: &mut Commands, fonts: &EmberFonts, pad: usize) -> Entity {
    let root = col(commands, 0.0);
    let mut kids: Vec<Entity> = Vec::new();

    kids.push(text(commands, fonts, &format!("Gamepad {}", pad + 1), 13.0, gray(230)));
    kids.push(spacer(commands, 8.0));

    // Sticks.
    let sticks = row(commands, 20.0);
    let l = stick_widget(commands, fonts, pad, Side::Left, "Left Stick");
    let r = stick_widget(commands, fonts, pad, Side::Right, "Right Stick");
    commands.entity(sticks).add_children(&[l, r]);
    kids.push(sticks);
    kids.push(spacer(commands, 12.0));

    // Triggers.
    kids.push(text(commands, fonts, "Triggers", 11.0, gray(150)));
    let triggers = row(commands, 10.0);
    let lt = trigger_widget(commands, fonts, pad, Side::Left, "LT");
    let rt = trigger_widget(commands, fonts, pad, Side::Right, "RT");
    commands.entity(triggers).add_children(&[lt, rt]);
    kids.push(triggers);
    kids.push(spacer(commands, 12.0));

    // Buttons.
    kids.push(text(commands, fonts, "Buttons", 11.0, gray(150)));
    let btn_col = col(commands, 4.0);
    let mut btn_rows: Vec<Entity> = Vec::new();
    for spec in BUTTON_ROWS {
        let br = row(commands, 4.0);
        let cells: Vec<Entity> = spec
            .iter()
            .map(|(label, btn)| button(commands, fonts, pad, label, *btn))
            .collect();
        commands.entity(br).add_children(&cells);
        btn_rows.push(br);
    }
    commands.entity(btn_col).add_children(&btn_rows);
    kids.push(btn_col);

    // Raw axes (filled each frame).
    let raw = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                margin: UiRect::top(Val::Px(12.0)),
                ..default()
            },
            GpRawAxes { pad, last: Vec::new() },
            Name::new("gp-raw-axes"),
        ))
        .id();
    kids.push(raw);

    commands.entity(root).add_children(&kids);
    root
}

fn empty_state(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            ..default()
        },))
        .id();
    let s1 = spacer(commands, 40.0);
    let t1 = text(commands, fonts, "No gamepad detected", 14.0, gray(120));
    let s2 = spacer(commands, 8.0);
    let t2 = text(commands, fonts, "Connect a controller to see input", 12.0, gray(80));
    commands.entity(root).add_children(&[s1, t1, s2, t2]);
    root
}

// ── Registration ────────────────────────────────────────────────────────────

pub fn register_native_gamepad(app: &mut App) {
    use renzora::NativePanelExt;
    use renzora_editor::SplashState;
    bevy::asset::embedded_asset!(app, "stick.wgsl");
    app.add_plugins(UiMaterialPlugin::<StickMaterial>::default());
    app.register_native_panel(PANEL_ID);
    app.add_systems(
        Update,
        (
            (gamepad_content_system, gamepad_structure).chain(),
            stick_attach,
            stick_sync,
            gamepad_sticks,
            gamepad_triggers,
            gamepad_buttons,
            gamepad_raw_axes,
        )
            .run_if(in_state(SplashState::Editor)),
    );
}

// ── Systems ───────────────────────────────────────────────────────────────

/// Build the gamepad list pane once (lazily) when its tab is first activated.
pub(crate) fn gamepad_content_system(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    leaves: Query<&DockLeaf>,
    children: Query<&Children>,
    panes: Query<&TabPane>,
) {
    let Some(_fonts) = fonts else {
        return;
    };
    for leaf in &leaves {
        if leaf.active != PANEL_ID {
            continue;
        }
        let exists = children.get(leaf.content).is_ok_and(|kids| {
            kids.iter()
                .any(|c| panes.get(c).is_ok_and(|p| p.id == PANEL_ID))
        });
        if exists {
            continue;
        }
        let list = commands
            .spawn((
                Node {
                    width: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    flex_shrink: 0.0,
                    padding: UiRect::all(Val::Px(8.0)),
                    ..default()
                },
                GamepadRoot { count: usize::MAX },
                Name::new("gamepad-list"),
            ))
            .id();
        let pane = tab_pane(&mut commands, PANEL_ID, list, true);
        commands.entity(leaf.content).add_child(pane);
    }
}

/// Rebuild the per-gamepad structure when the connected count changes.
pub(crate) fn gamepad_structure(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    state: Option<Res<GamepadDebugState>>,
    mut roots: Query<(Entity, &mut GamepadRoot)>,
    children: Query<&Children>,
) {
    let (Some(fonts), Some(state)) = (fonts, state) else {
        return;
    };
    let count = state.gamepads.len();
    for (list, mut root) in &mut roots {
        if root.count == count {
            continue;
        }
        root.count = count;
        if let Ok(kids) = children.get(list) {
            for k in kids.iter() {
                commands.entity(k).despawn();
            }
        }
        if count == 0 {
            let e = empty_state(&mut commands, &fonts);
            commands.entity(list).add_child(e);
            continue;
        }
        let mut kids: Vec<Entity> = Vec::new();
        for pad in 0..count {
            if pad > 0 {
                kids.push(spacer(&mut commands, 16.0));
                kids.push(hsep(&mut commands));
            }
            kids.push(build_gamepad(&mut commands, &fonts, pad));
        }
        commands.entity(list).add_children(&kids);
    }
}

/// Update stick positions + value labels each frame.
pub(crate) fn gamepad_sticks(
    state: Option<Res<GamepadDebugState>>,
    mut sticks: Query<(&GpStick, &mut StickData)>,
    mut labels: Query<(&GpStickLabel, &mut Text)>,
) {
    let Some(state) = state else {
        return;
    };
    for (s, mut data) in &mut sticks {
        if let Some(g) = state.gamepads.get(s.pad) {
            let v = if s.side == Side::Left {
                g.left_stick
            } else {
                g.right_stick
            };
            if data.x != v.x || data.y != v.y {
                data.x = v.x;
                data.y = v.y;
            }
        }
    }
    for (l, mut t) in &mut labels {
        if let Some(g) = state.gamepads.get(l.pad) {
            let v = if l.side == Side::Left {
                g.left_stick
            } else {
                g.right_stick
            };
            *t = Text::new(format!("X: {:.2}  Y: {:.2}", v.x, v.y));
        }
    }
}

/// Update trigger fills + value labels each frame.
pub(crate) fn gamepad_triggers(
    state: Option<Res<GamepadDebugState>>,
    mut fills: Query<(&GpTriggerFill, &mut Node, &mut BackgroundColor)>,
    mut labels: Query<(&GpTriggerLabel, &mut Text)>,
) {
    let Some(state) = state else {
        return;
    };
    for (f, mut node, mut bg) in &mut fills {
        if let Some(g) = state.gamepads.get(f.pad) {
            let v = if f.side == Side::Left {
                g.left_trigger
            } else {
                g.right_trigger
            };
            // Pixel height off the bar's inner extent (60px bar − 2px border);
            // percent height on an absolutely-positioned node is unreliable.
            let h = Val::Px(v.clamp(0.0, 1.0) * 58.0);
            if node.height != h {
                node.height = h;
            }
            let target = if v > 0.1 {
                rgb((100, 200, 100))
            } else {
                gray(80)
            };
            if bg.0 != target {
                bg.0 = target;
            }
        }
    }
    for (l, mut t) in &mut labels {
        if let Some(g) = state.gamepads.get(l.pad) {
            let v = if l.side == Side::Left {
                g.left_trigger
            } else {
                g.right_trigger
            };
            *t = Text::new(format!("{:.2}", v));
        }
    }
}

/// Highlight pressed buttons each frame (node background + label color).
pub(crate) fn gamepad_buttons(
    state: Option<Res<GamepadDebugState>>,
    buttons: Query<(Entity, &GpButton)>,
    mut bgs: Query<&mut BackgroundColor>,
    mut colors: Query<&mut TextColor>,
) {
    let Some(state) = state else {
        return;
    };
    for (node, b) in &buttons {
        let Some(g) = state.gamepads.get(b.pad) else {
            continue;
        };
        let pressed = b.btn.pressed(&g.buttons);
        let (bg, fg) = if pressed {
            (rgb((80, 160, 80)), Color::WHITE)
        } else {
            (rgb((50, 52, 58)), gray(120))
        };
        if let Ok(mut c) = bgs.get_mut(node) {
            if c.0 != bg {
                c.0 = bg;
            }
        }
        if let Ok(mut c) = colors.get_mut(b.label) {
            if c.0 != fg {
                c.0 = fg;
            }
        }
    }
}

/// Rebuild the non-zero raw-axes list when its formatted text changes.
pub(crate) fn gamepad_raw_axes(
    mut commands: Commands,
    fonts: Option<Res<EmberFonts>>,
    state: Option<Res<GamepadDebugState>>,
    mut areas: Query<(Entity, &mut GpRawAxes)>,
    children: Query<&Children>,
) {
    let (Some(fonts), Some(state)) = (fonts, state) else {
        return;
    };
    for (container, mut area) in &mut areas {
        let Some(g) = state.gamepads.get(area.pad) else {
            continue;
        };
        let lines: Vec<String> = if g.raw_axes.is_empty() {
            Vec::new()
        } else {
            std::iter::once("Raw Axes (non-zero)".to_string())
                .chain(g.raw_axes.iter().map(|(n, v)| format!("{}: {:.3}", n, v)))
                .collect()
        };
        if area.last == lines {
            continue;
        }
        area.last = lines.clone();
        if let Ok(kids) = children.get(container) {
            for k in kids.iter() {
                commands.entity(k).despawn();
            }
        }
        let mut rows: Vec<Entity> = Vec::new();
        for (i, line) in lines.iter().enumerate() {
            let (size, color) = if i == 0 {
                (11.0, gray(150))
            } else {
                (10.0, gray(120))
            };
            rows.push(text(&mut commands, &fonts, line, size, color));
        }
        commands.entity(container).add_children(&rows);
    }
}
