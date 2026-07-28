use renzora_plugin::prelude::*;

#[derive(Component)]
pub struct Orbit {
    pub radius: f32,
    pub speed: f32,
    pub height: f32,
    pub angle: f32,
}

impl Default for Orbit {
    fn default() -> Self {
        Self {
            radius: 3.0,
            speed: 1.0,
            height: 0.0,
            angle: 0.0,
        }
    }
}

fn orbit(mut q: Query<(&mut Transform, &mut Orbit)>, time: Res<Time>) {
    for (t, o) in &mut q {
        o.angle += o.speed * time.delta_secs();
        t.translation.x = o.angle.cos() * o.radius;
        t.translation.z = o.angle.sin() * o.radius;
        t.translation.y = o.height;
    }
}

pub struct OrbitPlugin;

impl Plugin for OrbitPlugin {
    fn build(&self, app: &mut App) {
        app.register_component::<Orbit>()
            .add_systems(Update, orbit);
    }
}

renzora_plugin::add!(OrbitPlugin);
