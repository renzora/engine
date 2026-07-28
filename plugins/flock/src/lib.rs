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

pub struct FlockPlugin;

impl Plugin for FlockPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FlockSettings>()
            .register_component::<Boid>()
            .register_component::<Leader>()
            .add_systems(Update, breathe)
            .add_systems(Update, flock);
    }
}

renzora_plugin::add!(FlockPlugin);
