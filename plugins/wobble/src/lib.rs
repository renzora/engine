use noise::{NoiseFn, Perlin};
use renzora_plugin::prelude::*;

#[derive(Component)]
pub struct Wobble {
    pub amplitude: f32,
    pub frequency: f32,
    pub seed: f32,
    pub time: f32,
}

impl Default for Wobble {
    fn default() -> Self {
        Self {
            amplitude: 0.35,
            frequency: 1.5,
            seed: 0.0,
            time: 0.0,
        }
    }
}

fn wobble(mut q: Query<(&mut Transform, &mut Wobble)>, time: Res<Time>) {
    let perlin = Perlin::new(1);
    for (t, w) in &mut q {
        w.time += time.delta_secs() * w.frequency;
        let n = perlin.get([w.time as f64, w.seed as f64]) as f32;
        let s = 1.0 + n * w.amplitude;
        t.scale.x = s;
        t.scale.y = s;
        t.scale.z = s;
    }
}

pub struct WobblePlugin;

impl Plugin for WobblePlugin {
    fn build(&self, app: &mut App) {
        app.register_component::<Wobble>()
            .add_systems(Update, wobble);
    }
}

renzora_plugin::add!(WobblePlugin);
