//! Renzora Volumetric Fog: settings wrapper for Bevy's `VolumetricFog`, and
//! the rule that makes a Fog Volume work on its own.
//!
//! Bevy draws volumetric fog only inside a `FogVolume` (a unit cube placed and
//! scaled with a `Transform`), only on cameras carrying `VolumetricFog`, and
//! only lit by lights tagged `VolumetricLight`. All three have to meet, and
//! with any one missing nothing renders and nothing says why. Two of them were
//! missing in practice: no way to place a Fog Volume (the editor now has one in
//! Add Entity), and a camera only got `VolumetricFog` from a
//! [`VolumetricFogSettings`] the user had to know to add.
//!
//! So [`sync_volumetric_fog`] gives every routed camera `VolumetricFog`
//! whenever the scene holds a Fog Volume. The settings live on the Fog Volume
//! itself (its card shows them, so the fog is set up in one place); a
//! [`VolumetricFogSettings`] on a routed World Environment or camera, from a
//! scene saved before that, still wins. Without a Fog Volume the pass would
//! cost its 5 ms or so for nothing, so then the camera only carries it if
//! settings were routed to it explicitly.

use bevy::light::{FogVolume, VolumetricFog};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize, PartialEq)]
#[reflect(Component, Serialize, Deserialize, Default)]
pub struct VolumetricFogSettings {
    /// Color of the sky light the fog picks up. A pale blue reads as daylight
    /// haze; pure white as thick fog.
    pub ambient_color: (f32, f32, f32),
    /// How bright that sky light makes the fog. Bevy adds it AFTER exposure
    /// (sunlight in the fog is multiplied by exposure, this is not), so it is
    /// in on-screen units: about 1 is white whatever the scene's brightness.
    pub ambient_intensity: f32,
    /// Raymarch step count along each view ray. Higher = smoother god
    /// rays at higher cost. 64 is Bevy's default; 32 reads as cheaper
    /// but slightly banded, 96+ as crisp at noticeable perf cost.
    pub step_count: u32,
    /// Per-frame jitter that hides banding artifacts when paired with
    /// TAA. 0 = banded but stable, 1 = maximum jitter (TAA hides it).
    pub jitter: f32,
    pub enabled: bool,
}

impl Default for VolumetricFogSettings {
    fn default() -> Self {
        // The sky light is what keeps fog from reading as a dark veil. At the
        // old 0.08 (on-screen units, see `ambient_intensity`) fog over a sunlit
        // street was a dark grey far below the scene it covered, and every
        // pixel outside a sun shaft got darker: 5 levels over the Bistro at the
        // Fog Volume's default density, 10 at three times that. 0.4 with a
        // paler colour puts the fog's own glow near the scene's average, so it
        // reads as bright haze that the shafts stand out of.
        Self {
            ambient_color: (0.72, 0.78, 0.88),
            ambient_intensity: 0.4,
            step_count: 64,
            jitter: 0.5,
            enabled: true,
        }
    }
}

impl VolumetricFogSettings {
    fn to_bevy(&self) -> VolumetricFog {
        let (r, g, b) = self.ambient_color;
        VolumetricFog {
            ambient_color: Color::srgb(r, g, b),
            ambient_intensity: self.ambient_intensity,
            step_count: self.step_count,
            jitter: self.jitter,
        }
    }
}

/// What a camera should carry: its first routed source's settings if it has
/// one (`Some(None)` when they are switched off), else the defaults while the
/// scene has a Fog Volume, else nothing.
fn wanted(source: Option<&VolumetricFogSettings>, any_volume: bool) -> Option<VolumetricFogSettings> {
    match source {
        Some(s) if s.enabled => Some(s.clone()),
        Some(_) => None,
        None if any_volume => Some(VolumetricFogSettings::default()),
        None => None,
    }
}

/// Give each routed camera the `VolumetricFog` [`wanted`] says, whenever the
/// routing, the settings or the set of Fog Volumes changes.
#[allow(clippy::too_many_arguments)]
fn sync_volumetric_fog(
    mut commands: Commands,
    sources: Query<Ref<VolumetricFogSettings>>,
    volumes: Query<(Entity, Option<&VolumetricFogSettings>), With<FogVolume>>,
    added_volumes: Query<(), Added<FogVolume>>,
    mut removed_volumes: RemovedComponents<FogVolume>,
    mut removed_settings: RemovedComponents<VolumetricFogSettings>,
    routing: Res<renzora::EffectRouting>,
) {
    let volumes_changed = !added_volumes.is_empty() | (removed_volumes.read().count() > 0);
    let settings_removed = removed_settings.read().count() > 0;
    let settings_changed = sources.iter().any(|s| s.is_changed());
    if !(routing.is_changed() || volumes_changed || settings_removed || settings_changed) {
        return;
    }
    // The first Fog Volume (oldest, so the choice is stable) supplies the
    // settings when no routed source does.
    let mut volume_settings: Option<(Entity, Option<&VolumetricFogSettings>)> = None;
    for (entity, settings) in &volumes {
        if volume_settings.is_none_or(|(e, _)| entity < e) {
            volume_settings = Some((entity, settings));
        }
    }
    let any_volume = volume_settings.is_some();
    let from_volume = volume_settings.and_then(|(_, s)| s);
    for (target, source_list) in routing.iter() {
        let routed = source_list.iter().find_map(|&s| sources.get(s).ok());
        let source = routed.as_deref().or(from_volume);
        let Ok(mut ec) = commands.get_entity(*target) else {
            continue;
        };
        match wanted(source, any_volume) {
            Some(settings) => {
                ec.try_insert(settings.to_bevy());
            }
            None => {
                ec.try_remove::<VolumetricFog>();
            }
        }
    }
}

#[derive(Default)]
pub struct VolumetricFogPlugin;

impl Plugin for VolumetricFogPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] VolumetricFogPlugin");
        app.register_type::<VolumetricFogSettings>();
        app.add_systems(Update, sync_volumetric_fog);
    }
}

renzora::add!(VolumetricFogPlugin);

#[cfg(test)]
mod tests {
    use super::*;

    fn app(with_settings: bool) -> (App, Entity, Entity) {
        let mut app = App::new();
        app.add_systems(Update, sync_volumetric_fog);
        let source = app.world_mut().spawn_empty().id();
        if with_settings {
            app.world_mut().entity_mut(source).insert(VolumetricFogSettings::default());
        }
        let camera = app.world_mut().spawn_empty().id();
        app.insert_resource(renzora::EffectRouting { routes: vec![(camera, vec![source])] });
        (app, source, camera)
    }

    fn has_fog(app: &App, camera: Entity) -> bool {
        app.world().get::<VolumetricFog>(camera).is_some()
    }

    /// A Fog Volume on its own is enough: the camera gets volumetric fog
    /// without anyone adding the settings component.
    #[test]
    fn a_fog_volume_alone_turns_it_on_and_off() {
        let (mut app, _, camera) = app(false);
        app.update();
        assert!(!has_fog(&app, camera), "no volume, no settings: no pass to pay for");
        let volume = app.world_mut().spawn(FogVolume::default()).id();
        app.update();
        assert!(has_fog(&app, camera));
        app.world_mut().despawn(volume);
        app.update();
        assert!(!has_fog(&app, camera));
    }

    /// The settings on the Fog Volume itself reach the camera, and switching
    /// them off there turns the pass off.
    #[test]
    fn a_fog_volume_carries_its_own_settings() {
        let (mut app, _, camera) = app(false);
        let mut settings = VolumetricFogSettings { step_count: 32, ..default() };
        let volume = app.world_mut().spawn((FogVolume::default(), settings.clone())).id();
        app.update();
        assert_eq!(app.world().get::<VolumetricFog>(camera).map(|f| f.step_count), Some(32));
        settings.enabled = false;
        app.world_mut().entity_mut(volume).insert(settings);
        app.update();
        assert!(!has_fog(&app, camera));
    }

    /// Settings switched off win over a Fog Volume being present.
    #[test]
    fn disabled_settings_keep_it_off() {
        let (mut app, source, camera) = app(true);
        app.world_mut().spawn(FogVolume::default());
        app.update();
        assert!(has_fog(&app, camera));
        app.world_mut().get_mut::<VolumetricFogSettings>(source).unwrap().enabled = false;
        app.update();
        assert!(!has_fog(&app, camera));
    }

    /// Removing the settings falls back to the Fog Volume rule rather than
    /// leaving the old values on the camera.
    #[test]
    fn removing_settings_falls_back() {
        let (mut app, source, camera) = app(true);
        app.update();
        assert!(has_fog(&app, camera), "explicit settings work without a volume");
        app.world_mut().entity_mut(source).remove::<VolumetricFogSettings>();
        app.update();
        assert!(!has_fog(&app, camera));
    }

    /// Scripts and tools add components by reflection, which needs a
    /// registered default; without one the settings could only be added from
    /// the inspector.
    #[test]
    fn settings_can_be_created_by_reflection() {
        let mut registry = bevy::reflect::TypeRegistry::default();
        registry.register::<VolumetricFogSettings>();
        let default = registry
            .get(std::any::TypeId::of::<VolumetricFogSettings>())
            .and_then(|t| t.data::<ReflectDefault>());
        assert!(default.is_some());
    }
}
