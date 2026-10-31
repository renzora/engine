//! Filter an environment map only while its input is changing.
//!
//! Bevy re-filters every `GeneratedEnvironmentMapLight` into radiance and
//! irradiance maps on EVERY frame, for every entity carrying one, whether or not
//! its camera is active and whether or not the source cubemap changed
//! (`bevy_pbr::light_probe::generate`, no bake-once mode upstream, see
//! bevyengine/bevy#24517). The GPU work is small; the CPU side is not. Each
//! filter rebuilds a uniform buffer, a dozen texture views, 1x1 placeholder
//! textures and every bind group, all on the render thread, all holding the
//! device's locks. A profile of an idle editor on Bevy 0.19 measured it at
//! 7.2 ms of render thread per frame, and the lock traffic stretched unrelated
//! 16-byte uniform writes elsewhere in the prepare phase to milliseconds. On
//! Bevy main it is cheaper but not free: on the Bistro scene, about 1 ms of GPU
//! and 1 to 1.5 ms of render thread per idle frame.
//!
//! The editor carries several of these at once: the primary camera's sky bake,
//! the material preview's HDRI (present from startup, inactive while its panel
//! is closed, and static forever), the import previews and any reflection
//! probes. Only the sky can change without the component noticing.
//!
//! So each map gets a short filter window, re-armed by anything that changes
//! its input, and is frozen outside it. Freezing happens in the render world:
//! the extracted `RenderEnvironmentMap` is removed right after Bevy inserts it,
//! which is the one component every generator system needs. The main-world
//! component is never touched, so the bind-group layout that forces it to exist
//! from a camera's first render stays put, and code that writes its intensity
//! still finds it. While frozen, the filtered maps keep their last result,
//! which for an unchanged input is the right one.
//!
//! This replaces an earlier gate that removed the main-world component, stashed
//! it, and covered only the sky probe, so the material preview filtered a
//! static HDRI on every frame from startup.

use bevy::ecs::entity::{EntityHashMap, EntityHashSet};
use bevy::light::{AtmosphereEnvironmentMapLight, EnvironmentMapLight, GeneratedEnvironmentMapLight};
use bevy::pbr::generate::{extract_generated_environment_map_entities, RenderEnvironmentMap};
use bevy::prelude::*;
use bevy::render::sync_world::RenderEntity;
use bevy::render::{Extract, ExtractSchedule, RenderApp};

use crate::SkyBakeDirty;

/// Frames a map keeps filtering after its input last changed. One frame
/// produces a complete map once the source is on the GPU; the margin covers the
/// source image's upload landing a frame or two after the asset exists.
const FILTER_FRAMES: u32 = 8;

/// Below this the map contributes nothing, so filtering it is wasted.
const ACTIVE_EPS: f32 = 1e-4;

/// Per-map filter windows, and the set frozen this frame.
#[derive(Resource, Default)]
pub(crate) struct EnvMapFilterGate {
    windows: EntityHashMap<u32>,
    frozen: EntityHashSet,
    /// Whether each map's camera was active last frame. Re-arming on the
    /// camera becoming active has to compare against this: `Ref<Camera>`'s
    /// change flag is set by any system that takes the camera mutably, which
    /// the viewport does every frame, and keyed on that the sky probe never
    /// froze.
    was_active: EntityHashMap<bool>,
    /// What last re-armed each map, for the log line.
    armed_by: EntityHashMap<&'static str>,
    /// The diffuse and specular images each map was last filled into. Bevy
    /// creates them empty (the specular one with no data at all) and only the
    /// filter puts anything in them, so a pair the gate has not filled yet must
    /// be filled whatever else is true. Freezing a new pair because its camera
    /// was inactive or idle at that moment left it empty for good, and every
    /// rough surface then mirrored garbage: blue and beige blotches in the
    /// viewport, a black shadow side in the import preview.
    filled: EntityHashMap<(AssetId<Image>, AssetId<Image>)>,
    /// Frames left of a fill that no freeze condition may cut short.
    fill_left: EntityHashMap<u32>,
    /// How many maps filtered last frame, so the log line below fires only when
    /// that changes.
    filtering_logged: Option<usize>,
}

/// What [`update_env_map_filter_gate`] reads off each map. A named alias only so
/// the query stays under clippy's type-complexity bar.
type FilterGateData = (
    Entity,
    Ref<'static, GeneratedEnvironmentMapLight>,
    Option<Ref<'static, EnvironmentMapLight>>,
    Option<&'static AtmosphereEnvironmentMapLight>,
    Option<Ref<'static, Camera>>,
    Option<&'static Name>,
    Has<renzora::core::ViewportRedrawIdle>,
);

/// Decide, per map, whether it filters this frame.
///
/// A window re-arms when the generator component is added or edited (its
/// source or intensity moved), when Bevy attaches the `EnvironmentMapLight` the
/// filter writes into, when the source image asset changes, when the camera
/// holding it turns active, and, for an atmosphere probe, while the sky is
/// settling after a change. A map on an inactive camera or at zero intensity is
/// otherwise frozen, since nothing is looking at it, with one exception: images
/// it has never been filled into get a full window first, because a frozen map
/// keeps only what is already in them.
pub(crate) fn update_env_map_filter_gate(
    mut gate: ResMut<EnvMapFilterGate>,
    sky: Res<SkyBakeDirty>,
    mut image_events: MessageReader<AssetEvent<Image>>,
    maps: Query<FilterGateData>,
    mut timeline: Option<ResMut<renzora::load_timeline::LoadTimeline>>,
) {
    let changed_images: Vec<AssetId<Image>> = image_events
        .read()
        .filter_map(|event| match *event {
            AssetEvent::Added { id }
            | AssetEvent::Modified { id }
            | AssetEvent::LoadedWithDependencies { id } => Some(id),
            _ => None,
        })
        .collect();

    let gate = &mut *gate;
    let mut seen = EntityHashSet::default();
    gate.frozen.clear();

    for (entity, generated, env_light, atmosphere, camera, name, redraw_idle) in &maps {
        seen.insert(entity);

        let camera_active = camera.as_ref().is_none_or(|c| c.is_active);
        // A camera the viewport idled because nothing changed keeps its
        // `was_active` as it was. Nothing it bakes can have moved meanwhile (a
        // change is exactly what wakes it), so waking must not count as
        // "camera activated" and re-filter the sky on every viewport touch.
        let became_active = !redraw_idle
            && camera_active
            && !gate.was_active.insert(entity, camera_active).unwrap_or(false);
        // Images the filter has never written into, or that were re-uploaded
        // (which puts their empty starting data back).
        let targets = env_light.as_ref().map(|e| (e.diffuse_map.id(), e.specular_map.id()));
        let needs_fill = targets.is_some_and(|(diffuse, specular)| {
            gate.filled.get(&entity) != Some(&(diffuse, specular))
                || changed_images.contains(&diffuse)
                || changed_images.contains(&specular)
        });
        if needs_fill {
            if let Some(targets) = targets {
                gate.filled.insert(entity, targets);
            }
            gate.fill_left.insert(entity, FILTER_FRAMES);
        }
        let filling = gate.fill_left.get_mut(&entity).is_some_and(|left| {
            *left = left.saturating_sub(1);
            *left > 0
        });
        // Until a visible camera's map is filled, everything it lights shows
        // no sky light at all, which reads as a black or flat scene.
        if !filling && camera_active && gate.fill_left.get(&entity) == Some(&0) {
            if let Some(timeline) = timeline.as_mut().filter(|t| t.is_running()) {
                let label = name.map_or_else(|| format!("{entity}"), |n| n.to_string());
                timeline.mark("environment lighting filled", label);
            }
        }

        let rearm = if needs_fill {
            Some("filtered maps empty")
        } else if generated.is_changed() {
            Some("component changed")
        } else if env_light.as_ref().is_some_and(|e| e.is_added()) {
            Some("environment light added")
        } else if became_active {
            Some("camera activated")
        } else if changed_images.contains(&generated.environment_map.id()) {
            Some("source image changed")
        } else if atmosphere.is_some() && sky.frames > 0 {
            Some("sky changed")
        } else {
            None
        };

        let window = gate.windows.entry(entity).or_insert(FILTER_FRAMES);
        if let Some(reason) = rearm {
            *window = FILTER_FRAMES;
            gate.armed_by.insert(entity, reason);
        } else {
            *window = window.saturating_sub(1);
        }

        // The sky probe's strength lives on the atmosphere component; the
        // generator's own copy is only set once, when Bevy creates it.
        let intensity = atmosphere.map_or(generated.intensity, |a| a.intensity);
        if !filling && (*window == 0 || !camera_active || intensity <= ACTIVE_EPS) {
            gate.frozen.insert(entity);
        }
    }

    gate.filled.retain(|entity, _| seen.contains(entity));
    gate.fill_left.retain(|entity, _| seen.contains(entity));
    gate.windows.retain(|entity, _| seen.contains(entity));
    gate.was_active.retain(|entity, _| seen.contains(entity));
    gate.armed_by.retain(|entity, _| seen.contains(entity));

    // Say which maps are filtering, and what armed each, whenever the count
    // changes. A map that never freezes costs milliseconds of render thread
    // every frame and is otherwise invisible, and this is the only place that
    // knows why each one is live.
    let filtering: Vec<String> = maps
        .iter()
        .filter(|(entity, ..)| !gate.frozen.contains(entity))
        .map(|(entity, .., name, _)| {
            let label = name.map_or_else(|| format!("{entity}"), |n| n.to_string());
            let reason = gate.armed_by.get(&entity).copied().unwrap_or("first frame");
            format!("{label} ({reason})")
        })
        .collect();
    if gate.filtering_logged != Some(filtering.len()) {
        gate.filtering_logged = Some(filtering.len());
        info!(
            "[env-map] filtering {} of {} environment maps: {:?}",
            filtering.len(),
            seen.len(),
            filtering
        );
    }
}

/// Remove the just-extracted `RenderEnvironmentMap` from every frozen map, so
/// none of Bevy's generator systems see it this frame. Ordered after Bevy's
/// extraction, so this removal is applied after its insert.
fn freeze_env_map_filters(
    mut commands: Commands,
    gate: Extract<Option<Res<EnvMapFilterGate>>>,
    maps: Extract<Query<(Entity, RenderEntity), With<GeneratedEnvironmentMapLight>>>,
) {
    let Some(gate) = gate.as_ref() else {
        return;
    };
    for (entity, render_entity) in &maps {
        if gate.frozen.contains(&entity) {
            if let Ok(mut render) = commands.get_entity(render_entity) {
                render.remove::<RenderEnvironmentMap>();
            }
        }
    }
}

pub(crate) fn register_render(app: &mut App) {
    if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
        render_app.add_systems(
            ExtractSchedule,
            freeze_env_map_filters.after(extract_generated_environment_map_entities),
        );
    }
}

#[cfg(test)]
mod tests {
    use bevy::asset::uuid::Uuid;
    use bevy::ecs::system::RunSystemOnce;

    use super::*;

    fn image(n: u128) -> Handle<Image> {
        Handle::Uuid(Uuid::from_u128(n), std::marker::PhantomData)
    }

    /// A map on an inactive camera, already holding the images Bevy made for it.
    fn world() -> (World, Entity) {
        let mut world = World::new();
        world.init_resource::<EnvMapFilterGate>();
        world.init_resource::<SkyBakeDirty>();
        world.init_resource::<Messages<AssetEvent<Image>>>();
        let map = world
            .spawn((
                GeneratedEnvironmentMapLight { intensity: 1000.0, ..default() },
                EnvironmentMapLight {
                    diffuse_map: image(1),
                    specular_map: image(2),
                    ..default()
                },
                Camera { is_active: false, ..default() },
            ))
            .id();
        (world, map)
    }

    fn frozen_after(world: &mut World, map: Entity, frames: u32) -> bool {
        for _ in 0..frames {
            world.run_system_once(update_env_map_filter_gate).unwrap();
            world.clear_trackers();
        }
        world.resource::<EnvMapFilterGate>().frozen.contains(&map)
    }

    /// The regression: the preview cameras are inactive when Bevy hands them
    /// their images, and freezing them then left them empty for good.
    #[test]
    fn new_images_are_filled_even_on_an_inactive_camera() {
        let (mut world, map) = world();
        assert!(!frozen_after(&mut world, map, 1), "a first fill must not be frozen");
    }

    #[test]
    fn a_filled_map_on_an_inactive_camera_freezes() {
        let (mut world, map) = world();
        assert!(frozen_after(&mut world, map, FILTER_FRAMES + 1));
    }

    /// Bevy re-creating the images (a new pair) is a new fill, not a frozen map.
    #[test]
    fn replaced_images_are_filled_again() {
        let (mut world, map) = world();
        assert!(frozen_after(&mut world, map, FILTER_FRAMES + 1));
        world.get_mut::<EnvironmentMapLight>(map).unwrap().specular_map = image(3);
        assert!(!frozen_after(&mut world, map, 1));
    }
}
