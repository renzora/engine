//! Reference native plugin.
//!
//! Everything here is ordinary Bevy — the only Renzora-specific line is the
//! `renzora::plugin!` at the bottom. That is the whole point: a Bevy plugin from
//! anywhere drops in with one line added.
//!
//! What makes it work is that the engine and this library link ONE shared
//! `bevy_dylib` and one shared `renzora_dylib`, so `Plugin`, `World` and
//! `Transform` are the same types on both sides. A C-ABI plugin shares no types
//! and reaches the engine through a fixed function table instead; it can run in
//! a shipped game, which this cannot. See `crates/renzora_native_plugin`.

use bevy::prelude::*;

/// A plugin's own component type. Nothing special is needed to declare one —
/// `#[derive(Component)]` resolves its paths through `BevyManifest`, which is
/// why the build supplies `CARGO_MANIFEST_DIR` and a `Cargo.toml`.
#[derive(Component, Clone, Default)]
pub struct HelloMarker;

#[derive(Resource, Default)]
struct Reported(bool);

pub struct HelloNative;

impl Plugin for HelloNative {
    fn build(&self, app: &mut App) {
        info!("[hello-native] build() — a source-shipped plugin with full App access");
        app.init_resource::<Reported>().add_systems(Update, report_once);
    }
}

/// An exclusive-`&mut World` system: the access this whole mechanism exists to
/// provide, and the thing a C-ABI plugin cannot have.
fn report_once(world: &mut World) {
    if world.resource::<Reported>().0 {
        return;
    }
    world.resource_mut::<Reported>().0 = true;

    // A contract-crate call whose function body reads a process-global static.
    // It returns the host's translation, not an empty one, only because
    // `renzora` is a single shared image rather than a copy per plugin.
    let translated = renzora::lang::t("menu.file");
    let transforms = world.query::<&Transform>().iter(world).count();
    info!("[hello-native] t(\"menu.file\") = {translated:?}; {transforms} Transforms visible");
}

renzora::plugin!(HelloNative);
