use bevy::prelude::*;

#[bevy_main]
fn main() {
    let mut app = renzora_runtime::build_runtime_app();
    app.run();
}
