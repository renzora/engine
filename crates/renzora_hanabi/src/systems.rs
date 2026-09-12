//! Runtime systems for syncing HanabiEffect with bevy_hanabi ParticleEffect.

use bevy::prelude::*;
use bevy::image::ImageSampler;
use bevy::render::render_resource::{Extent3d, TextureFormat, TextureUsages};
use bevy_hanabi::prelude::*;
use bevy_hanabi::EffectMaterial;
use std::path::PathBuf;

use crate::builder::build_complete_effect;
use crate::data::*;
use renzora::core::PickBounds2d;
use renzora::CurrentProject;

/// A built-in soft radial sprite bound to every particle effect, so quads render
/// as soft round blobs (modulated by the particle color) instead of hard squares
/// — the cheapest, highest-impact "de-blocking" step. Generated procedurally so
/// there's no asset file / VFS path to resolve.
#[derive(Resource, Default)]
pub struct ParticleSoftTexture(pub Handle<Image>);

/// Create the soft radial sprite (grayscale falloff in all RGBA channels, so it
/// softens both additive RGB and alpha-blended effects via Modulate).
pub fn setup_soft_particle_texture(
    mut images: ResMut<Assets<Image>>,
    mut commands: Commands,
) {
    let size = 64u32;
    let mut data = vec![0u8; (size * size * 4) as usize];
    let c = (size as f32 - 1.0) * 0.5;
    for y in 0..size {
        for x in 0..size {
            let dx = (x as f32 - c) / c;
            let dy = (y as f32 - c) / c;
            let d2 = dx * dx + dy * dy;
            // Soft gaussian falloff (no hard edge) so overlapping particles blend
            // into each other instead of reading as distinct circles.
            let a = (-d2 * 3.5).exp().clamp(0.0, 1.0);
            let v = (a * 255.0) as u8;
            let i = ((y * size + x) * 4) as usize;
            data[i] = v;
            data[i + 1] = v;
            data[i + 2] = v;
            data[i + 3] = v;
        }
    }
    let mut image = Image {
        data: Some(data),
        ..default()
    };
    image.texture_descriptor.size = Extent3d {
        width: size,
        height: size,
        depth_or_array_layers: 1,
    };
    image.texture_descriptor.format = TextureFormat::Rgba8Unorm;
    image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
    image.sampler = ImageSampler::linear();
    commands.insert_resource(ParticleSoftTexture(images.add(image)));
}

/// Grayscale fbm noise bound as the 2nd texture of effects with `erosion`, used
/// by the `ErosionModifier` to dissolve particles in organic wisps as they fade.
#[derive(Resource, Default)]
pub struct ParticleErosionNoise(pub Handle<Image>);

fn noise_hash(x: i32, y: i32) -> f32 {
    let mut h = (x.wrapping_mul(374_761_393).wrapping_add(y.wrapping_mul(668_265_263))) as u32;
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    (h as f32) / (u32::MAX as f32)
}

fn value_noise(x: f32, y: f32) -> f32 {
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (xf, yf) = (x - x.floor(), y - y.floor());
    let u = xf * xf * (3.0 - 2.0 * xf);
    let v = yf * yf * (3.0 - 2.0 * yf);
    let a = noise_hash(xi, yi);
    let b = noise_hash(xi + 1, yi);
    let c = noise_hash(xi, yi + 1);
    let d = noise_hash(xi + 1, yi + 1);
    let ab = a + (b - a) * u;
    let cd = c + (d - c) * u;
    ab + (cd - ab) * v
}

/// Create the erosion noise texture (4-octave fbm, grayscale in all channels).
pub fn setup_erosion_noise_texture(mut images: ResMut<Assets<Image>>, mut commands: Commands) {
    let size = 128u32;
    let mut data = vec![0u8; (size * size * 4) as usize];
    for y in 0..size {
        for x in 0..size {
            // Low frequency + few octaves => large, soft erosion blobs (wispy
            // dissolve) rather than fine grain that reads as static.
            let (mut f, mut amp, mut freq) = (0.0f32, 0.6f32, 2.0f32);
            for _ in 0..3 {
                let nx = x as f32 / size as f32 * freq;
                let ny = y as f32 / size as f32 * freq;
                f += value_noise(nx, ny) * amp;
                freq *= 2.0;
                amp *= 0.5;
            }
            let val = (f.clamp(0.0, 1.0) * 255.0) as u8;
            let i = ((y * size + x) * 4) as usize;
            data[i] = val;
            data[i + 1] = val;
            data[i + 2] = val;
            data[i + 3] = 255;
        }
    }
    let mut image = Image {
        data: Some(data),
        ..default()
    };
    image.texture_descriptor.size = Extent3d {
        width: size,
        height: size,
        depth_or_array_layers: 1,
    };
    image.texture_descriptor.format = TextureFormat::Rgba8Unorm;
    image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST;
    image.sampler = ImageSampler::linear();
    commands.insert_resource(ParticleErosionNoise(images.add(image)));
}

/// Build the `EffectMaterial` image list for an effect: soft sprite in slot 0,
/// plus erosion noise in slot 1 when the effect uses erosion. The order MUST
/// match the texture slots declared in `build_complete_effect`.
fn effect_images(
    def: &HanabiEffectDefinition,
    soft: &ParticleSoftTexture,
    noise: &ParticleErosionNoise,
) -> Vec<Handle<Image>> {
    if def.erosion {
        vec![soft.0.clone(), noise.0.clone()]
    } else {
        vec![soft.0.clone()]
    }
}

/// Resolve an effect definition from its source.
fn resolve_effect_definition(
    source: &EffectSource,
    project: Option<&CurrentProject>,
) -> HanabiEffectDefinition {
    match source {
        // No file chosen. Clearing the asset slot in the inspector leaves the
        // path empty rather than removing the component, which is a reasonable
        // thing for an emitter to be: selected, present, pointing at nothing
        // yet. Joining an empty path onto the project root yields the project
        // DIRECTORY, and the loader below then tried to read a folder as a
        // `.particle` and logged "cannot find the path specified" on every
        // sync. Answered here rather than at the read, because an empty path is
        // not a failure to report — it is a question nobody has answered.
        EffectSource::Asset { path } if path.trim().is_empty() => {
            // Nothing chosen means nothing emits.
            //
            // NOT `Default::default()`, which is a working effect that spawns 50
            // particles a second — clearing the asset slot then replaced the
            // chosen effect with a generic one rather than with silence, which
            // is what "I removed the file and some particles are still there"
            // was. The entity keeps its components so the inspector still has
            // something to show; it simply produces no particles until a file
            // is picked.
            HanabiEffectDefinition {
                spawn_rate: 0.0,
                spawn_count: 0,
                capacity: 1,
                ..Default::default()
            }
        }
        EffectSource::Asset { path } => {
            // Prefer the VFS-aware byte loader so `.particle` files bundled in
            // an exported `.rpak` load correctly (the editor's disk read can't
            // see into the archive). Falls back to a direct disk read.
            if let Some(bytes) = renzora::core::load_asset_bytes(path) {
                if let Ok(text) = std::str::from_utf8(&bytes) {
                    if let Ok(def) = ron::from_str::<HanabiEffectDefinition>(text) {
                        return def;
                    }
                }
            }
            let disk = match project {
                Some(proj) => proj.path.join(path),
                None => PathBuf::from(path),
            };
            load_effect_from_file(&disk).unwrap_or_default()
        }
        EffectSource::Inline { definition } => definition.clone(),
    }
}

/// Marker component to track that we've created the hanabi effect for this entity.
#[derive(Component)]
pub struct HanabiEffectSynced {
    /// The child entity carrying `ParticleEffect`.
    ///
    /// The effect deliberately does NOT live on the emitter. Tearing one down
    /// means undoing everything bevy_hanabi set up, and the part that cannot be
    /// undone is `SyncToRenderWorld`: it is a required component of
    /// `ParticleEffect`, required components are not removed with their
    /// requirer, and Bevy's own docs say it "should persist throughout the
    /// entity's entire lifecycle" — removing it panics `entity_sync_system` the
    /// moment it comes back. So the render-world mirror, and the `CachedEffect`
    /// holding the GPU buffers, outlive any attempt to strip the effect off an
    /// entity component by component.
    ///
    /// Despawning, on the other hand, cleans up completely — that was the one
    /// thing that reliably worked. So the effect gets an entity of its own that
    /// can be despawned, and the emitter keeps only `HanabiEffect` and this.
    pub effect_entity: Entity,
    pub effect_handle: Handle<EffectAsset>,
    /// Which effect this was built from: the asset path, or `None` for inline.
    ///
    /// Patching an `EffectAsset` in place is right for a scalar tweak — dragging
    /// a rate slider should not restart the emitter — and wrong when the effect
    /// changes *shape*. A different `.particle` can have a different particle
    /// layout, different texture slots, a ribbon where there was none, and the
    /// live GPU buffers were sized for the old one. Pointing an emitter at
    /// another file and watching it misbehave is what this exists to stop.
    ///
    /// The path rather than the whole `EffectSource`, so that an inline
    /// definition being edited in the particle editor keeps patching in place
    /// (`None == None`) while swapping the file, or switching between inline and
    /// a file, rebuilds. Comparing the definitions themselves would mean
    /// `PartialEq` down the entire tree to answer a question about identity.
    pub source_key: Option<String>,
}

/// Print a particle-sync line when `RENZORA_PARTICLE_LOG` is set.
///
/// Off by default, and taking a closure so the message is not formatted when it
/// is off. Swapping an effect's source touches five components across two
/// crates, and which of them survive a swap is not visible from the outside: the
/// emitter simply renders wrong, and every wrong state looks much like the
/// others. This says which branch ran and what was on the entity when it did.
fn particle_log(msg: impl FnOnce() -> String) {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ENABLED.get_or_init(|| {
        std::env::var_os("RENZORA_PARTICLE_LOG").is_some()
            || std::env::args().any(|a| a == "--particle-log")
    }) {
        bevy::log::info!("{}", msg());
    }
}

/// Report what an emitter looks like once the sync's commands have landed.
///
/// The other half of [`particle_log`]. `sync_hanabi_effects` queues its work
/// through `Commands`, so it cannot see the result of its own inserts and
/// removals — and the failures here are all about a component that did not come
/// back. Run after the sync, this says what the entity actually ended up with.
pub fn log_particle_state(
    emitters: Query<(Entity, &HanabiEffect, Option<&HanabiEffectSynced>)>,
    effect_entities: Query<(
        Option<&ParticleEffect>,
        Option<&CompiledParticleEffect>,
        Option<&EffectSpawner>,
        Option<&EffectMaterial>,
    )>,
    mut last: Local<std::collections::HashMap<Entity, (bool, bool, bool, bool, bool)>>,
) {
    for (entity, _hanabi, synced) in emitters.iter() {
        // Reported for the EMITTER but read from its effect entity, because that
        // is where the components live now. `alive` is the one that matters on a
        // swap: the old effect entity must be gone before a new one appears, or
        // bevy_hanabi's render-world cache is shared between them.
        let child = synced.map(|s| s.effect_entity);
        let (effect, compiled, spawner, material) = child
            .and_then(|c| effect_entities.get(c).ok())
            .map(|(a, b, c, d)| (a.is_some(), b.is_some(), c.is_some(), d.is_some()))
            .unwrap_or_default();
        let now = (child.is_some(), effect, compiled, spawner, material);
        // Only on change: this runs every frame over every emitter, and a line
        // per frame would bury the transition that matters.
        if last.get(&entity) == Some(&now) {
            continue;
        }
        last.insert(entity, now);
        particle_log(|| {
            format!(
                "[particles] {entity:?} state: effect_entity={:?} alive={} effect={} compiled={} spawner={} material={}",
                child, now.0, now.1, now.2, now.3, now.4
            )
        });
    }
}

/// The identity of an effect source, for [`HanabiEffectSynced::source_key`].
fn source_key(source: &EffectSource) -> Option<String> {
    match source {
        EffectSource::Asset { path } => Some(path.clone()),
        EffectSource::Inline { .. } => None,
    }
}

/// Drives the flicker of an effect's emitted `PointLight` (added when the effect
/// definition has `light`).
#[derive(Component)]
pub struct ParticleLightFlicker {
    pub base_intensity: f32,
    pub flicker: f32,
    pub phase: f32,
}

/// Build the `PointLight` + flicker marker for an effect's `light` settings.
fn light_bundle(
    l: &ParticleLightSettings,
    phase: f32,
) -> (PointLight, ParticleLightFlicker) {
    (
        PointLight {
            color: Color::linear_rgb(l.color[0], l.color[1], l.color[2]),
            intensity: l.intensity,
            range: l.range,
            shadow_maps_enabled: l.shadows,
            ..default()
        },
        ParticleLightFlicker {
            base_intensity: l.intensity,
            flicker: l.flicker,
            phase,
        },
    )
}

/// Wobble emitted-light intensity for a lively fire/torch flicker.
pub fn flicker_particle_lights(
    time: Res<Time>,
    mut q: Query<(&mut PointLight, &ParticleLightFlicker)>,
) {
    let t = time.elapsed_secs();
    for (mut light, f) in q.iter_mut() {
        if f.flicker <= 0.0 {
            continue;
        }
        // Two detuned sines = organic, non-repeating flicker.
        let n = ((t * 11.0 + f.phase).sin() * 0.6 + (t * 23.0 + f.phase * 1.7).sin() * 0.4) * 0.5;
        light.intensity = (f.base_intensity * (1.0 + f.flicker * n)).max(0.0);
    }
}

/// Editor pick box for the (spriteless) emitter entity, estimating where the
/// particles actually GO — emitter shape extents plus the travel envelope
/// (velocity reach, acceleration displacement, attractor positions) over the
/// particle lifetime. Without this, 2D picking falls back to a small fixed
/// marker box at the origin: unusable for a snow strip hundreds of pixels
/// wide, and mismatched for a sparks ring whose particles fly well beyond a
/// tiny emitter circle. The envelope is directional (min/max per axis), so a
/// fall-only effect gets a box below the emitter, not a symmetric balloon.
fn emitter_pick_bounds(def: &HanabiEffectDefinition) -> PickBounds2d {
    // Matches picker_2d::NODE_MARKER_HALF so tiny emitters keep the standard
    // grab size instead of degenerating to an unclickable sliver.
    const MIN_HALF: f32 = 20.0;

    // Emitter shape extents, symmetric around the origin.
    let emit = match &def.emit_shape {
        HanabiEmitShape::Point => Vec2::ZERO,
        HanabiEmitShape::Circle { radius, .. } | HanabiEmitShape::Sphere { radius, .. } => {
            Vec2::splat(*radius)
        }
        HanabiEmitShape::Cone {
            base_radius,
            top_radius,
            height,
            ..
        } => Vec2::new(base_radius.max(*top_radius), height * 0.5),
        HanabiEmitShape::Rect { half_extents, .. } => Vec2::from_array(*half_extents),
        HanabiEmitShape::Box { half_extents } => Vec2::new(half_extents[0], half_extents[1]),
    };
    let mut min = -emit;
    let mut max = emit;

    let t = def.lifetime_max.max(def.lifetime_min).max(0.0);
    let drag = def.linear_drag.max(0.0);
    // Distance an initial speed v covers before dying: drag decays it with
    // time constant 1/drag (total distance v/drag); undragged it just runs.
    let reach = |v: f32| {
        let v = v.abs();
        if drag > 0.01 {
            (v / drag).min(v * t)
        } else {
            v * t
        }
    };

    let speed = if def.velocity_speed_max > 0.0 {
        def.velocity_speed_min.abs().max(def.velocity_speed_max.abs())
    } else {
        def.velocity_magnitude.abs()
    };
    match def.velocity_mode {
        VelocityMode::Directional => {
            let dir = Vec3::from_array(def.velocity_direction)
                .truncate()
                .normalize_or_zero();
            let d = dir * reach(speed);
            min = min.min(min + d);
            max = max.max(max + d);
            // Spread fans out around the main direction on both axes.
            let s = reach(speed) * def.velocity_spread.abs();
            min -= Vec2::splat(s);
            max += Vec2::splat(s);
        }
        // In-plane (or spherical) bursts go everywhere equally.
        VelocityMode::Radial | VelocityMode::Random | VelocityMode::Tangent => {
            let d = reach(speed);
            min -= Vec2::splat(d);
            max += Vec2::splat(d);
        }
    }

    // Constant acceleration: dragged particles settle at terminal velocity
    // (a/drag) and coast for the lifetime; undragged ones integrate a*t²/2.
    let accel = Vec3::from_array(def.acceleration).truncate();
    let disp = if drag > 0.01 {
        accel / drag * t
    } else {
        accel * (0.5 * t * t)
    };
    min = min.min(min + disp);
    max = max.max(max + disp);

    // Attractors gather particles around their own positions.
    for att in &def.attractors {
        let p = Vec3::from_array(att.position).truncate();
        let r = att.radius.abs() + att.influence_dist.abs();
        min = min.min(p - Vec2::splat(r));
        max = max.max(p + Vec2::splat(r));
    }

    let half = ((max - min) * 0.5).max(Vec2::splat(MIN_HALF));
    PickBounds2d {
        half_extents: half,
        offset: (max + min) * 0.5,
    }
}

/// Sync HanabiEffect with bevy_hanabi ParticleEffect.
pub fn sync_hanabi_effects(
    mut commands: Commands,
    mut effects: ResMut<Assets<EffectAsset>>,
    query: Query<(Entity, &HanabiEffect, Option<&HanabiEffectSynced>), Changed<HanabiEffect>>,
    // The spawner lives on the effect's own entity now, so it is reached through
    // `HanabiEffectSynced::effect_entity` rather than found beside the emitter.
    mut spawners: Query<&mut EffectSpawner>,
    removed_query: Query<(Entity, &HanabiEffectSynced), Without<HanabiEffect>>,
    project: Option<Res<CurrentProject>>,
    soft: Res<ParticleSoftTexture>,
    noise: Res<ParticleErosionNoise>,
) {
    for (entity, effect_data, maybe_synced) in query.iter() {
        let definition = resolve_effect_definition(&effect_data.source, project.as_deref());
        let effect_asset = build_complete_effect(&definition);

        // Re-inserted on every sync so an emitter-shape edit resizes the box.
        commands
            .entity(entity)
            .try_insert(emitter_pick_bounds(&definition));

        // Patch in place only while the effect is the same one. See
        // `HanabiEffectSynced::source`.
        let key = source_key(&effect_data.source);
        let patch_in_place = maybe_synced.filter(|s| s.source_key == key);

        particle_log(|| {
            format!(
                "[particles] {entity:?} sync: {} | source {:?} -> {:?} | effect_entity={:?} | def spawn_rate={} capacity={}",
                if patch_in_place.is_some() {
                    "PATCH"
                } else if maybe_synced.is_some() {
                    "TEARDOWN (rebuild next frame)"
                } else {
                    "CREATE"
                },
                maybe_synced.and_then(|s| s.source_key.clone()),
                key,
                maybe_synced.map(|s| s.effect_entity),
                definition.spawn_rate,
                definition.capacity,
            )
        });

        if let Some(synced) = patch_in_place {
            // The spawner's settings come from the asset, but `tick_spawners`
            // only builds an `EffectSpawner` when one is ABSENT — so patching
            // the asset left the live spawner on the settings it was created
            // with. Spawn rate, burst count, period and duration all live there,
            // which is why editing those in a `.particle` and saving appeared to
            // do nothing while a colour change came through immediately: colour
            // is read per particle from the asset, spawning is not.
            //
            // Settings only, not the whole component: `cycle_time` and
            // `completed_cycle_count` are where the emitter is *up to*, and
            // resetting those on every tweak would restart the effect each time
            // a slider moved.
            if let Ok(mut spawner) = spawners.get_mut(synced.effect_entity) {
                spawner.settings = effect_asset.spawner;
            }
            if let Some(mut existing) = effects.get_mut(&synced.effect_handle) {
                *existing = effect_asset;
            }
            // The material goes back on with the asset, and leaving it off was a
            // real bug. `build_complete_effect` declares the effect's texture
            // slots, and `EffectMaterial` is what binds actual images to them —
            // slot 0 being the soft sprite every effect samples. Replacing the
            // asset without re-binding left the new slots empty, so particles
            // stopped sampling the sprite and rendered as small hard dots
            // instead of soft blobs. It showed as "the editor viewport is
            // broken and the runtime is fine": the runtime is a separate
            // process that built its effect through the create path below, with
            // a material, while the editor had rebuilt in place without one.
            commands
                .entity(synced.effect_entity)
                .try_insert(EffectMaterial {
                    images: effect_images(&definition, &soft, &noise),
                });
        } else if let Some(synced) = maybe_synced {
            // The source changed: despawn the old effect entity and let
            // `rehydrate_hanabi_effects` build a new one next frame.
            //
            // Despawn rather than rebuild in place, and a frame apart rather
            // than both at once. bevy_hanabi keeps a render-world mirror holding
            // a `CachedEffect` with the GPU buffers, torn down by an observer on
            // `Remove, CachedEffect` when that mirror despawns. Swapping the
            // asset on a living entity leaves the new effect sharing the old
            // one's cache, which is what "the particles appear but never move"
            // was.
            //
            // Leaving the emitter with `HanabiEffect` and no
            // `HanabiEffectSynced` is exactly what `rehydrate_hanabi_effects`
            // looks for, so this needs no extra state.
            particle_log(|| {
                format!(
                    "[particles] {entity:?} despawning effect entity {:?} for rebuild",
                    synced.effect_entity
                )
            });
            teardown_effect(&mut commands, entity, Some(synced.effect_entity));
        } else {
            // A brand-new emitter: nothing to tear down.
            let effect_handle = effects.add(effect_asset);
            let effect_entity = spawn_effect_entity(
                &mut commands,
                entity,
                effect_handle.clone(),
                effect_images(&definition, &soft, &noise),
            );
            commands.entity(entity).try_insert(HanabiEffectSynced {
                effect_entity,
                effect_handle,
                source_key: key,
            });
            // The light follows the definition in BOTH directions. Only adding
            // it left a `PointLight` burning on an emitter whose new effect has
            // no light at all, which reads as a stray light in the scene with
            // nothing selected to explain it.
            match &definition.light {
                Some(l) => {
                    let phase = ((entity.to_bits() % 997) as f32) * 0.618_034;
                    commands.entity(entity).try_insert(light_bundle(l, phase));
                }
                None => {
                    commands
                        .entity(entity)
                        .remove::<(PointLight, ParticleLightFlicker)>();
                }
            }
        }
    }

    for (entity, synced) in removed_query.iter() {
        // If deleting the component in the inspector produces no line here, the
        // component was not actually removed from the entity and the problem is
        // upstream of this system entirely.
        particle_log(|| {
            format!(
                "[particles] {entity:?} teardown: HanabiEffect is gone, despawning {:?}",
                synced.effect_entity
            )
        });
        teardown_effect(&mut commands, entity, Some(synced.effect_entity));
    }
}

/// Spawn the child entity that carries the effect, and wire the emitter to it.
///
/// The child is `HideInHierarchy` so it stays out of the hierarchy panel and out
/// of scene saves — it is rebuilt from `HanabiEffect` on load, and a copy baked
/// into the file would come back as a second, dead emitter. It is `Name`d
/// because an unnamed entity carrying a `Transform` is despawned on sight by the
/// engine's own guard.
///
/// No `Transform` offset: as a child at identity it sits exactly where the
/// emitter does, so a `World`-space effect emits from the emitter's position and
/// follows it when moved.
fn spawn_effect_entity(
    commands: &mut Commands,
    emitter: Entity,
    handle: Handle<EffectAsset>,
    images: Vec<Handle<Image>>,
) -> Entity {
    commands
        .spawn((
            Name::new("particle effect"),
            renzora::HideInHierarchy,
            ChildOf(emitter),
            Transform::default(),
            ParticleEffect::new(handle),
            EffectMaterial { images },
        ))
        .id()
}

/// Despawn the effect entity and forget it.
///
/// Despawning rather than stripping components: see
/// [`HanabiEffectSynced::effect_entity`] for why that distinction is the whole
/// reason this indirection exists.
fn teardown_effect(commands: &mut Commands, emitter: Entity, effect_entity: Option<Entity>) {
    if let Some(child) = effect_entity {
        commands.entity(child).try_despawn();
    }
    commands
        .entity(emitter)
        .remove::<(HanabiEffectSynced, PointLight, ParticleLightFlicker)>();
}

/// Apply runtime overrides (play/pause) to particle effects.
pub fn apply_runtime_overrides(
    emitters: Query<(&HanabiEffect, &HanabiEffectSynced), Changed<HanabiEffect>>,
    mut spawners: Query<&mut EffectSpawner>,
) {
    // Two queries because the spawner lives on the effect's own child entity,
    // not beside `HanabiEffect`. See `HanabiEffectSynced::effect_entity`.
    for (effect_data, synced) in emitters.iter() {
        if let Ok(mut spawner) = spawners.get_mut(synced.effect_entity) {
            spawner.active = effect_data.playing;
        }
    }
}

/// Rehydrate particle effects after scene load.
pub fn rehydrate_hanabi_effects(
    mut commands: Commands,
    mut effects: ResMut<Assets<EffectAsset>>,
    query: Query<(Entity, &HanabiEffect), Without<HanabiEffectSynced>>,
    project: Option<Res<CurrentProject>>,
    soft: Res<ParticleSoftTexture>,
    noise: Res<ParticleErosionNoise>,
) {
    for (entity, effect_data) in query.iter() {
        let definition = resolve_effect_definition(&effect_data.source, project.as_deref());
        let effect_asset = build_complete_effect(&definition);
        let effect_handle = effects.add(effect_asset);
        let effect_entity = spawn_effect_entity(
            &mut commands,
            entity,
            effect_handle.clone(),
            effect_images(&definition, &soft, &noise),
        );
        commands.entity(entity).try_insert((
            HanabiEffectSynced {
                effect_entity,
                effect_handle,
                source_key: source_key(&effect_data.source),
            },
            emitter_pick_bounds(&definition),
        ));
        if let Some(l) = &definition.light {
            let phase = ((entity.to_bits() % 997) as f32) * 0.618_034;
            commands.entity(entity).try_insert(light_bundle(l, phase));
        }
    }
}

/// Command queue for particle script commands.
#[derive(Resource, Default)]
pub struct ParticleCommandQueue {
    pub commands: Vec<ParticleCommand>,
}

pub enum ParticleCommand {
    Play(Entity),
    Pause(Entity),
    Stop(Entity),
    Reset(Entity),
    Burst {
        entity: Entity,
        count: u32,
    },
    SetRate {
        entity: Entity,
        multiplier: f32,
    },
    SetScale {
        entity: Entity,
        multiplier: f32,
    },
    SetTint {
        entity: Entity,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
    },
    SetVariable {
        entity: Entity,
        name: String,
        value: EffectVariable,
    },
}

/// Process particle commands from scripts.
pub fn process_particle_commands(
    mut commands: ResMut<ParticleCommandQueue>,
    mut emitters: Query<(&mut HanabiEffect, Option<&HanabiEffectSynced>)>,
    mut spawners: Query<&mut EffectSpawner>,
) {
    // The spawner is on the effect's child entity, so every arm below reaches
    // it through `HanabiEffectSynced` rather than beside `HanabiEffect`. Written
    // as a closure over the two queries because the alternative — leaving the
    // old single query in place — still compiles and silently stops every script
    // `play`/`pause`/`stop` from doing anything.
    macro_rules! spawner_of {
        ($synced:expr) => {
            $synced
                .map(|s: &HanabiEffectSynced| s.effect_entity)
                .and_then(|e| spawners.get_mut(e).ok())
        };
    }

    for cmd in commands.commands.drain(..) {
        match cmd {
            ParticleCommand::Play(entity) => {
                if let Ok((mut data, synced)) = emitters.get_mut(entity) {
                    data.playing = true;
                    if let Some(mut s) = spawner_of!(synced) {
                        s.active = true;
                    }
                }
            }
            ParticleCommand::Pause(entity) => {
                if let Ok((mut data, synced)) = emitters.get_mut(entity) {
                    data.playing = false;
                    if let Some(mut s) = spawner_of!(synced) {
                        s.active = false;
                    }
                }
            }
            ParticleCommand::Stop(entity) => {
                if let Ok((mut data, synced)) = emitters.get_mut(entity) {
                    data.playing = false;
                    if let Some(mut s) = spawner_of!(synced) {
                        s.active = false;
                        s.reset();
                    }
                }
            }
            ParticleCommand::Reset(entity) => {
                if let Ok((_, synced)) = emitters.get_mut(entity) {
                    if let Some(mut s) = spawner_of!(synced) {
                        s.reset();
                    }
                }
            }
            ParticleCommand::Burst { entity, count: _ } => {
                if let Ok((_, synced)) = emitters.get_mut(entity) {
                    if let Some(mut s) = spawner_of!(synced) {
                        s.reset();
                    }
                }
            }
            ParticleCommand::SetRate { entity, multiplier } => {
                if let Ok((mut data, _)) = emitters.get_mut(entity) {
                    data.rate_multiplier = multiplier;
                }
            }
            ParticleCommand::SetScale { entity, multiplier } => {
                if let Ok((mut data, _)) = emitters.get_mut(entity) {
                    data.scale_multiplier = multiplier;
                }
            }
            ParticleCommand::SetTint { entity, r, g, b, a } => {
                if let Ok((mut data, _)) = emitters.get_mut(entity) {
                    data.color_tint = [r, g, b, a];
                }
            }
            ParticleCommand::SetVariable {
                entity,
                name,
                value,
            } => {
                if let Ok((mut data, _)) = emitters.get_mut(entity) {
                    data.variable_overrides.insert(name, value);
                }
            }
        }
    }
}

/// Queue a rebuild when a `.particle` file is edited outside the editor.
///
/// [`hot_reload_saved_effects`] below already rebuilds every entity using an
/// effect, but only for paths the particle editor put in
/// `recently_saved_paths` — that is, only when the editor itself did the
/// saving. A `.particle` edited in a text editor, or arriving through a
/// `git pull`, went unnoticed until the scene was reopened.
///
/// Feeding the same queue rather than rebuilding here directly: the rebuild is
/// not trivial (resolve the definition, build the asset, patch it into the live
/// handle) and a second copy of it would be a second thing to keep correct.
pub fn queue_externally_edited_effects(
    mut editor_state: ResMut<ParticleEditorState>,
    mut changes: MessageReader<renzora::core::project_files::ProjectFileChanged>,
    self_writes: Option<Res<renzora::core::project_files::SelfWrites>>,
) {
    use renzora::core::project_files::AssetKind;

    if changes.is_empty() {
        return;
    }
    for change in changes.read() {
        if change.kind != AssetKind::Particle || !change.is_live() {
            continue;
        }
        // Skip the editor's own save. Without this the particle editor's save
        // would queue the path twice: once through `recently_saved_paths` and
        // again through the watcher event that save caused.
        if let Some(sw) = self_writes.as_ref() {
            if std::fs::read(&change.path).is_ok_and(|b| sw.matches(&change.path, &b)) {
                continue;
            }
        }
        // Project-relative, which is the form `EffectSource::Asset { path }`
        // holds and what the matcher below compares against.
        let path = change.relative.clone();
        if !editor_state.recently_saved_paths.contains(&path) {
            editor_state.recently_saved_paths.push(path);
        }
    }
}

/// Hot reload: when .particle files are saved, update all entities referencing them.
pub fn hot_reload_saved_effects(
    mut editor_state: ResMut<ParticleEditorState>,
    mut query: Query<&mut HanabiEffect>,
) {
    if editor_state.recently_saved_paths.is_empty() {
        return;
    }

    let saved_paths: Vec<String> = editor_state.recently_saved_paths.drain(..).collect();

    for mut effect_data in query.iter_mut() {
        let EffectSource::Asset { path } = &effect_data.source else {
            continue;
        };
        // Suffix either way, because one side is project-relative and the other
        // may be absolute depending on who reported the save.
        let matches = saved_paths.iter().any(|saved| {
            let saved = saved.replace('\\', "/");
            let path = path.replace('\\', "/");
            saved.ends_with(&path) || path.ends_with(&saved) || saved == path
        });

        // Marking it changed is the whole job. `sync_hanabi_effects` runs after
        // this in the same frame, sees `Changed<HanabiEffect>`, and does the
        // rebuild — re-reading the file, patching the asset, updating the
        // spawner's settings and re-binding the material on the effect entity.
        //
        // This used to do all of that itself, and after the effect moved onto
        // its own child entity the copy here was both redundant and wrong: it
        // inserted `EffectMaterial` on the EMITTER, where nothing reads it.
        // Duplicating the rebuild is how the two drift apart, which is most of
        // what went wrong in this file.
        if matches {
            effect_data.set_changed();
        }
    }
}
