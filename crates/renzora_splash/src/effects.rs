//! Whether the launcher and loading screen run their decorative effects.
//!
//! The setting itself is [`renzora::SplashEffects`], mirrored from the editor's
//! settings into [`renzora::SplashEffectsSetting`]; this module resolves its
//! `Auto` value against the primary monitor for the three places that ask: the
//! Light Chamber cinematic, its post camera, and the loading screen's haze.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::window::{Monitor, PrimaryMonitor};

/// The primary monitor's physical size, if winit has reported one yet.
fn primary_size<'a>(mut monitors: impl Iterator<Item = &'a Monitor>) -> Option<(u32, u32)> {
    monitors
        .next()
        .map(|m| (m.physical_width, m.physical_height))
}

/// System-param form, for the ordinary systems that gate on it.
#[derive(SystemParam)]
pub(crate) struct SplashEffects<'w, 's> {
    setting: Option<Res<'w, renzora::SplashEffectsSetting>>,
    monitors: Query<'w, 's, &'static Monitor, With<PrimaryMonitor>>,
}

impl SplashEffects<'_, '_> {
    pub(crate) fn reduced(&self) -> bool {
        self.setting
            .as_deref()
            .copied()
            .unwrap_or_default()
            .reduced(primary_size(self.monitors.iter()))
    }
}

/// World form, for the exclusive system that builds the loading screen.
pub(crate) fn reduced_in_world(world: &mut World) -> bool {
    let setting = world
        .get_resource::<renzora::SplashEffectsSetting>()
        .copied()
        .unwrap_or_default();
    let mut monitors = world.query_filtered::<&Monitor, With<PrimaryMonitor>>();
    setting.reduced(primary_size(monitors.iter(world)))
}
