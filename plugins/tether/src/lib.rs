use renzora_plugin::prelude::*;

/// Snaps an entity home once it drifts too far.
///
/// The point is the `insert` in `tether`: `Transform` is a HOST component, so it
/// crosses as the frozen 40-byte mirror and the host has to marshal it into
/// bevy's 48-byte layout. Passing the bytes straight through read past the
/// buffer and put rotation where scale belongs.
///
/// `register_component::<Transform>()` below is required, not decorative — a
/// component the plugin only ever *inserts* is never assigned an id otherwise,
/// and the insert silently does nothing.
#[derive(Component)]
#[repr(C)]
pub struct Tether {
    pub max_distance: f32,
    pub home_height: f32,
    pub scale: f32,
}

impl Default for Tether {
    fn default() -> Self {
        Self {
            max_distance: 6.0,
            home_height: 2.0,
            scale: 1.0,
        }
    }
}

fn tether(q: Query<(Entity, &Transform, &Tether)>, mut cmds: Commands) {
    for (e, t, cfg) in &q {
        if t.translation.length() <= cfg.max_distance {
            continue;
        }
        cmds.entity(e).insert(
            Transform::from_xyz(0.0, cfg.home_height, 0.0).with_scale(Vec3::splat(cfg.scale)),
        );
    }
}

pub struct TetherPlugin;

impl Plugin for TetherPlugin {
    fn build(&self, app: &mut App) {
        app.register_component::<Tether>()
            .register_component::<Transform>()
            .add_systems(Update, tether);
    }
}

renzora_plugin::add!(TetherPlugin);
