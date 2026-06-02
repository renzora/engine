//! Bevy-native (ember) Gamepad debug panel — a faithful bevy_ui port of the egui
//! panel: per-controller analog sticks (drawn by a `UiMaterial`), trigger bars,
//! a button grid, and the non-zero raw-axes list.
//!
//! Built once into its dock pane. The set of controllers is a reactive
//! `keyed_list` (rows added/removed as gamepads connect); every live value
//! (stick position, trigger fill, button highlight, axis readouts) is a
//! value-diffed binding, so an idle controller costs nothing.

use std::hash::{Hash, Hasher};

use bevy::asset::Asset;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial};
use bevy::ui_render::UiMaterialPlugin;

use renzora_ember::dock::{tab_pane, DockLeaf, TabPane};
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::{bind_text, bind_with, keyed_list, KeyedSnapshot};
use renzora_ember::theme::rgb;

use crate::state::{GamepadButtonState, GamepadDebugState};

const PANEL_ID: &str = "gamepad";

fn gray(v: u8) -> Color {
    rgb((v, v, v))
}

/// Read gamepad `pad` out of the debug state (or `None` if absent/disconnected).
fn with_pad<R>(world: &World, pad: usize, f: impl FnOnce(&crate::state::GamepadInfo) -> R) -> Option<R> {
    world
        .get_resource::<GamepadDebugState>()
        .and_then(|s| s.gamepads.get(pad))
        .map(f)
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

// ── Buttons ───────────────────────────────────────────────────────────────

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

fn stick_side(world: &World, pad: usize, side: Side) -> Vec2 {
    with_pad(world, pad, |g| {
        if side == Side::Left {
            g.left_stick
        } else {
            g.right_stick
        }
    })
    .unwrap_or(Vec2::ZERO)
}

fn trigger_side(world: &World, pad: usize, side: Side) -> f32 {
    with_pad(world, pad, |g| {
        if side == Side::Left {
            g.left_trigger
        } else {
            g.right_trigger
        }
    })
    .unwrap_or(0.0)
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
            Name::new("gp-stick"),
        ))
        .id();
    // Drive the stick's StickData (→ material) from the live stick position.
    bind_with(
        commands,
        stick,
        move |w| stick_side(w, pad, side),
        |w, e, v: &Vec2| {
            if let Some(mut d) = w.get_mut::<StickData>(e) {
                d.x = v.x;
                d.y = v.y;
            }
        },
    );
    let value = commands
        .spawn((
            Text::new("X: 0.00  Y: 0.00"),
            ui_font(&fonts.ui, 10.0),
            TextColor(gray(120)),
        ))
        .id();
    bind_text(commands, value, move |w| {
        let v = stick_side(w, pad, side);
        format!("X: {:.2}  Y: {:.2}", v.x, v.y)
    });
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
            Name::new("gp-trigger-fill"),
        ))
        .id();
    // Fill height + color from the live trigger value. Quantize to whole pixels
    // so tiny analog jitter doesn't churn the binding.
    bind_with(
        commands,
        fill,
        move |w| (trigger_side(w, pad, side).clamp(0.0, 1.0) * 58.0).round() as i32,
        |w, e, px: &i32| {
            // Pixel height off the bar's inner extent (60px bar − 2px border);
            // percent height on an absolutely-positioned node is unreliable.
            if let Some(mut n) = w.get_mut::<Node>(e) {
                n.height = Val::Px(*px as f32);
            }
            let target = if *px > 6 {
                rgb((100, 200, 100))
            } else {
                gray(80)
            };
            if let Some(mut bg) = w.get_mut::<BackgroundColor>(e) {
                bg.0 = target;
            }
        },
    );
    commands.entity(bar).add_child(fill);
    let value = commands
        .spawn((
            Text::new("0.00"),
            ui_font(&fonts.ui, 10.0),
            TextColor(gray(120)),
        ))
        .id();
    bind_text(commands, value, move |w| {
        format!("{:.2}", trigger_side(w, pad, side))
    });
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
    // Highlight the node + label together when the button is pressed.
    bind_with(
        commands,
        b,
        move |w| with_pad(w, pad, |g| btn.pressed(&g.buttons)).unwrap_or(false),
        move |w, node, pressed: &bool| {
            let (bg, fg) = if *pressed {
                (rgb((80, 160, 80)), Color::WHITE)
            } else {
                (rgb((50, 52, 58)), gray(120))
            };
            if let Some(mut c) = w.get_mut::<BackgroundColor>(node) {
                c.0 = bg;
            }
            if let Some(mut c) = w.get_mut::<TextColor>(tx) {
                c.0 = fg;
            }
        },
    );
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

/// The non-zero raw-axes readout for `pad` (header + one line per axis).
fn raw_axes_snapshot(pad: usize) -> impl Fn(&World) -> KeyedSnapshot + Send + Sync {
    move |world| {
        let lines: Vec<String> = with_pad(world, pad, |g| {
            if g.raw_axes.is_empty() {
                Vec::new()
            } else {
                std::iter::once("Raw Axes (non-zero)".to_string())
                    .chain(g.raw_axes.iter().map(|(n, v)| format!("{}: {:.3}", n, v)))
                    .collect()
            }
        })
        .unwrap_or_default();
        let items: Vec<(u64, u64)> = lines
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let mut h = std::collections::hash_map::DefaultHasher::new();
                l.hash(&mut h);
                (i as u64, h.finish())
            })
            .collect();
        KeyedSnapshot {
            items,
            build: Box::new(move |c, f, i| {
                let (size, color) = if i == 0 {
                    (11.0, gray(150))
                } else {
                    (10.0, gray(120))
                };
                text(c, f, &lines[i], size, color)
            }),
        }
    }
}

fn build_gamepad(commands: &mut Commands, fonts: &EmberFonts, pad: usize) -> Entity {
    let root = col(commands, 0.0);
    let mut kids: Vec<Entity> = Vec::new();

    // Separator above every gamepad after the first.
    if pad > 0 {
        kids.push(spacer(commands, 16.0));
        kids.push(hsep(commands));
    }

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

    // Raw axes — a reactive nested list (header + non-zero axes).
    let raw = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                margin: UiRect::top(Val::Px(12.0)),
                ..default()
            },
            Name::new("gp-raw-axes"),
        ))
        .id();
    keyed_list(commands, raw, raw_axes_snapshot(pad));
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

/// What the top-level list renders at a given slot.
#[derive(Clone, Copy)]
enum Slot {
    Empty,
    Pad(usize),
}

/// The keyed-list snapshot for the controller set: one row per connected pad
/// (added/removed as controllers connect), or a single empty-state row.
fn gamepad_snapshot(world: &World) -> KeyedSnapshot {
    let count = world
        .get_resource::<GamepadDebugState>()
        .map(|s| s.gamepads.len())
        .unwrap_or(0);
    let slots: Vec<Slot> = if count == 0 {
        vec![Slot::Empty]
    } else {
        (0..count).map(Slot::Pad).collect()
    };
    // Structure is value-independent (live values are bindings), so the hash is
    // constant per key; rows only appear/disappear as pads connect.
    let items: Vec<(u64, u64)> = slots
        .iter()
        .map(|s| match s {
            Slot::Empty => (u64::MAX, 0),
            Slot::Pad(p) => (*p as u64, 0),
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| match slots[i] {
            Slot::Empty => empty_state(c, f),
            Slot::Pad(p) => build_gamepad(c, f, p),
        }),
    }
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
        (gamepad_content_system, stick_attach, stick_sync).run_if(in_state(SplashState::Editor)),
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
                Name::new("gamepad-list"),
            ))
            .id();
        // Reactive keyed list drives the controller rows from here on.
        keyed_list(&mut commands, list, gamepad_snapshot);
        let pane = tab_pane(&mut commands, PANEL_ID, list, true);
        commands.entity(leaf.content).add_child(pane);
    }
}
