use renzora_plugin::prelude::*;

#[derive(Resource)]
#[repr(C)]
pub struct FlockSettings {
    pub separation: f32,
    pub cohesion: f32,
    pub radius: f32,
    pub max_speed: f32,
}

impl Default for FlockSettings {
    fn default() -> Self {
        Self {
            separation: 1.5,
            cohesion: 0.8,
            radius: 3.0,
            max_speed: 4.0,
        }
    }
}

#[derive(Component, Default)]
#[repr(C)]
pub struct Boid {
    pub vx: f32,
    pub vy: f32,
    pub vz: f32,
}

#[derive(Component)]
#[repr(C)]
pub struct Leader {
    pub bias: f32,
}

impl Default for Leader {
    fn default() -> Self {
        Self { bias: 0.9 }
    }
}

fn breathe(mut s: ResMut<FlockSettings>, time: Res<Time>) {
    s.cohesion = 0.8 + (time.elapsed_secs() * 0.4).sin() * 0.5;
}

fn flock(
    mut q: Query<(&mut Transform, &mut Boid, Option<&Leader>)>,
    s: Res<FlockSettings>,
    time: Res<Time>,
) {
    let dt = time.delta_secs().min(0.05);
    let mut points: Vec<Vec3> = Vec::with_capacity(q.len());
    let mut centre = Vec3::ZERO;
    for (t, _, _) in &q {
        points.push(t.translation);
        centre += t.translation;
    }
    if points.len() < 2 {
        return;
    }
    centre = centre / points.len() as f32;

    for (i, (t, b, leader)) in (&mut q).into_iter().enumerate() {
        let p = points[i];

        let mut steer = Vec3::ZERO;
        for (j, other) in points.iter().enumerate() {
            if j == i {
                continue;
            }
            let away = p - *other;
            let d = away.length();
            if d > 0.0001 && d < s.radius {
                steer += away / (d * d);
            }
        }
        steer = steer * s.separation;

        let pull = leader.map_or(1.0, |l| 1.0 - l.bias);
        steer += (centre - p) * s.cohesion * pull;

        let mut v = Vec3 {
            x: b.vx,
            y: b.vy,
            z: b.vz,
        } + steer * dt;
        let speed = v.length();
        if speed > s.max_speed {
            v = v / speed * s.max_speed;
        }
        b.vx = v.x;
        b.vy = v.y;
        b.vz = v.z;
        t.translation += v * dt;
    }
}

/// The panel, as markup.
///
/// Only buttons reach here. A `bind(Type.field)` widget talks to the resource
/// directly — the host knows its layout, so dragging a slider never calls into
/// this plugin at all.
fn on_action(action: Action) {
    // No captures — the handler is a plain fn, same rule a system follows, and
    // for the same reason: the host has nowhere to put a capture. The name is
    // the `action` number the button's `PanelActionId` carried.
    match action.name() {
        "1" => info("flock: reset"),
        "2" => info("flock: calm"),
        other => warn(&format!("flock: unknown action {other}")),
    }
}

pub struct FlockPlugin;

impl Plugin for FlockPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FlockSettings>()
            .register_component::<Boid>()
            .register_component::<Leader>()
            .add_panel(
                Panel::new(
                    "flock",
                    "Flock",
                    bsn! {
                        // `UiRect` is a struct, not an enum — there is no
                        // `All(..)` variant, and naming one silently drops the
                        // padding rather than failing.
                        Node {
                            flex_direction: Column,
                            row_gap: Px(6.0),
                            padding: { left: Px(4.0), right: Px(4.0), top: Px(4.0), bottom: Px(4.0) },
                        }
                        Children [
                            Text("Flocking"),
                            // Two-way bound to the resource. Dragging one writes
                            // the field and the flocking system reads the new
                            // value next frame; changing it from anywhere else
                            // moves the thumb. Neither direction calls into this
                            // plugin — the resource lives host-side, so the host
                            // syncs both ends itself.
                            //
                            // `min`/`max` are in the field's own units, so nothing
                            // here normalises: `radius` really is 0..10.
                            Text("Cohesion"),
                            ( EmberSliderWidget { value: bind(FlockSettings.cohesion), min: 0.0, max: 2.0 } ),
                            Text("Separation"),
                            ( EmberSliderWidget { value: bind(FlockSettings.separation), min: 0.0, max: 4.0 } ),
                            Text("Radius"),
                            ( EmberSliderWidget { value: bind(FlockSettings.radius), min: 0.5, max: 10.0 } ),
                            ( Node { flex_direction: Row, column_gap: Px(6.0) }
                              Children [
                                ( EmberButtonWidget { label: "Reset" }
                                  PanelActionId { action: 1 } ),
                                ( EmberButtonWidget { label: "Calm" }
                                  PanelActionId { action: 2 } ),
                              ] ),
                        ]
                    },
                )
                .icon("bird")
                .on_action(on_action),
            )
            .add_systems(Update, breathe)
            .add_systems(Update, flock);
    }
}

renzora_plugin::add!(FlockPlugin);
