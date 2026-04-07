use bevy::prelude::*;
use dynamic_plugin_meta::add;

#[derive(Default)]
pub struct ExamplePlugin;

impl Plugin for ExamplePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, tick);
    }
}

fn tick(time: Res<Time>) {
    if (time.elapsed_secs() % 5.0) < time.delta_secs() {
        info!("[ExamplePlugin] Running at {:.0}s", time.elapsed_secs());
    }
}

add!(ExamplePlugin);
