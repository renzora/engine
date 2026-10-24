//! Property animation: sampling keyframe tracks bound to arbitrary component
//! fields and writing the interpolated values into the world.
//!
//! This is a lightweight custom sampler that runs *alongside* Bevy's
//! `AnimationPlayer` (which only drives skeletal bone curves). It works for any
//! entity — including ones with no skeleton/`AnimationPlayer`, like a sun light —
//! by writing `Transform` fields directly and other component fields through the
//! shared reflection writer (`renzora::reflection::set_reflected_field`), so the
//! exact same code path animates fields in the editor preview and the exported
//! runtime.

use bevy::asset::io::Reader;
use bevy::asset::{AssetLoader, LoadContext};
use bevy::ecs::system::SystemState;
use bevy::prelude::*;

use renzora::{sample_property_track, AnimMarker, PropertyTrack, TrackValue};

use crate::component::{AnimatorComponent, AnimatorState};
use crate::loader::AnimLoadError;

// ============================================================================
// PropertyClip asset + loader
// ============================================================================

/// The property-animation half of a `.anim` file, loaded as its own asset so it
/// is available in the exported runtime (Bevy's `AnimClipLoader` discards
/// everything but skeletal bone curves). Loaded with an explicit
/// `asset_server.load::<PropertyClip>(path)` so it never collides with the
/// `Handle<AnimationClip>` loader that shares the `.anim` extension.
#[derive(Asset, TypePath, Debug, Clone, Default)]
pub struct PropertyClip {
    pub duration: f32,
    pub tracks: Vec<PropertyTrack>,
    pub markers: Vec<AnimMarker>,
    /// The clip's raw bone keys (the rest of the `.anim`, property tracks and
    /// markers taken out). Bevy's `AnimationClip` keeps only type-erased
    /// curves, and retargeting and difference clips are built from keys.
    pub skeletal: Option<std::sync::Arc<renzora::AnimClip>>,
}

#[derive(Default, TypePath)]
pub struct PropertyClipLoader;

impl AssetLoader for PropertyClipLoader {
    type Asset = PropertyClip;
    type Settings = ();
    type Error = AnimLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let mut anim: renzora::AnimClip = ron::de::from_bytes(&bytes)?;
        let tracks = std::mem::take(&mut anim.property_tracks);
        let markers = std::mem::take(&mut anim.markers);
        Ok(PropertyClip {
            duration: anim.duration,
            tracks,
            markers,
            skeletal: Some(std::sync::Arc::new(anim)),
        })
    }

    fn extensions(&self) -> &[&str] {
        &["anim"]
    }
}

// ============================================================================
// Sampling + apply (shared by runtime playback and editor preview)
// ============================================================================

/// Sample every property track at `time` (seconds) and write the resulting
/// values onto the resolved target entities. Exclusive (`&mut World`) because
/// arbitrary-field writes go through reflection. When `verbose`, emits throttled
/// diagnostics (what it sampled, where it wrote, and why a write was skipped).
pub fn apply_property_tracks(
    world: &mut World,
    animator_entity: Entity,
    tracks: &[PropertyTrack],
    time: f32,
    verbose: bool,
) {
    for (i, track) in tracks.iter().enumerate() {
        // Disabled in the timeline: the field keeps whatever value it has. This
        // is the one sampler both play mode and the editor's scrub preview
        // call, so muting reads the same in both.
        if track.muted {
            continue;
        }
        if verbose
            && track.keys.len() >= 2
            && track.keys.iter().all(|k| k.value == track.keys[0].value)
        {
            warn!(
                "[prop-anim] track {} ({}.{}) has {} keys but ALL have the same value -> no motion. Re-pose the entity at a different time before keying.",
                i, track.component, track.field, track.keys.len()
            );
        }
        let Some(value) = sample_property_track(track, time) else {
            if verbose {
                info!("[prop-anim] track {} ({}.{}) has no keys", i, track.component, track.field);
            }
            continue;
        };
        let Some(target) = resolve_target(world, animator_entity, &track.target) else {
            if verbose {
                warn!("[prop-anim] track {} target '{}' not found under {:?}", i, track.target, animator_entity);
            }
            continue;
        };
        if verbose {
            info!(
                "[prop-anim] track {} {}.{} @ t={:.2} -> {:?} (target {:?})",
                i, track.component, track.field, time, value, target
            );
        }
        apply_track_value(world, target, track, value, verbose);
    }
}

/// Read the current live value of a track's field — the inverse of
/// [`apply_track_value`]. Used by the editor to capture keyframes (manual
/// "Add Key" and record mode). Transform fields read directly; others go
/// through the reflection reader.
pub fn read_track_value(
    world: &World,
    animator_entity: Entity,
    track: &PropertyTrack,
) -> Option<TrackValue> {
    let target = resolve_target(world, animator_entity, &track.target)?;
    if track.component.eq_ignore_ascii_case("transform") {
        let t = world.get::<Transform>(target)?;
        return match track.field.as_str() {
            "translation" => Some(TrackValue::Vec3(t.translation.to_array())),
            "scale" => Some(TrackValue::Vec3(t.scale.to_array())),
            // Record rotation as Euler degrees (preferring the typed cache) so
            // keyframes interpolate per-component — a 0→360 pair is a real spin.
            "rotation" => {
                let cache = world.get::<renzora::EditorEulerCache>(target);
                let deg = renzora::rotation_euler_deg(t.rotation, cache, "transform");
                Some(TrackValue::Vec3(deg.to_array()))
            }
            _ => None,
        };
    }
    let pv =
        renzora::reflection::get_reflected_field(world, target, &track.component, &track.field)?;
    TrackValue::from_property_value(&pv)
}

/// Resolve a track's `target` to a concrete entity: empty/`"self"` → the
/// animator entity; otherwise the nearest descendant whose `Name` matches.
fn resolve_target(world: &World, root: Entity, target: &str) -> Option<Entity> {
    if target.is_empty() || target == "self" {
        return Some(root);
    }
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if let Some(name) = world.get::<Name>(e) {
            if name.as_str() == target {
                return Some(e);
            }
        }
        if let Some(children) = world.get::<Children>(e) {
            stack.extend(children.iter());
        }
    }
    None
}

/// Write one sampled value onto a field. Transform translation/rotation/scale
/// take a direct typed path (fast + correct quaternion); everything else goes
/// through the shared reflection writer.
fn apply_track_value(world: &mut World, target: Entity, track: &PropertyTrack, value: TrackValue, verbose: bool) {
    if track.component.eq_ignore_ascii_case("transform") {
        // Euler-degree rotation (the spin-capable path): the sampled degrees were
        // component-lerped, so 0→360 is a full turn. The cache write needs an
        // exclusive borrow, so resolve the quaternion before touching Transform.
        if let ("rotation", TrackValue::Vec3(d)) = (track.field.as_str(), value) {
            let q = renzora::cache_euler_deg(world, target, "transform", Vec3::from_array(d));
            if let Some(mut t) = world.get_mut::<Transform>(target) {
                t.rotation = q;
            } else if verbose {
                warn!("[prop-anim] target {:?} has no Transform component", target);
            }
            return;
        }
        if let Some(mut t) = world.get_mut::<Transform>(target) {
            match (track.field.as_str(), value) {
                ("translation", TrackValue::Vec3(v)) => t.translation = Vec3::from_array(v),
                ("scale", TrackValue::Vec3(v)) => t.scale = Vec3::from_array(v),
                // Legacy quaternion rotation keys (older clips) — slerp result.
                ("rotation", TrackValue::Quat(q)) => t.rotation = Quat::from_array(q),
                _ if verbose => warn!(
                    "[prop-anim] transform field '{}' / value {:?} mismatch — not written",
                    track.field, value
                ),
                _ => {}
            }
        } else if verbose {
            warn!("[prop-anim] target {:?} has no Transform component", target);
        }
        return;
    }
    if let Some(pv) = value.to_property_value() {
        let ok = renzora::reflection::set_reflected_field(world, target, &track.component, &track.field, &pv);
        if verbose && !ok {
            warn!("[prop-anim] reflection write failed for {}.{}", track.component, track.field);
        }
    } else if verbose {
        warn!("[prop-anim] value {:?} has no reflectable form for {}.{}", value, track.component, track.field);
    }
}

// ============================================================================
// Runtime playback system
// ============================================================================

/// Throttle for property-animation diagnostics (logs at most ~2×/sec).
#[derive(Resource, Default)]
pub struct PropAnimDebug {
    pub last_log: f32,
}

/// Run condition: property animation plays in the exported runtime (no
/// `PlayModeState` resource) or whenever scripts are running in the editor —
/// both full Play and in-editor Simulate. While editing, the animation-editor
/// scrub preview drives sampling instead.
pub fn property_animation_active(play_mode: Option<Res<renzora::PlayModeState>>) -> bool {
    play_mode.is_none_or(|pm| pm.is_scripts_running())
}

/// Advance each animator's property-clip time and apply the sampled values.
///
/// Independent of the skeletal init path (`AnimationPlayer`/graph), so it works
/// for property-only entities that have no skeleton. The active clip is the
/// current clip if one is playing, else the animator's default/first clip, so
/// ambient property animation auto-plays like a default skeletal clip.
#[allow(clippy::type_complexity)]
pub fn apply_runtime_property_animation(world: &mut World) {
    let mut sys: SystemState<(
        Res<Time>,
        ResMut<PropAnimDebug>,
        Res<Assets<PropertyClip>>,
        Query<(Entity, &AnimatorComponent, &mut AnimatorState)>,
        Query<&AnimationPlayer>,
    )> = SystemState::new(world);

    let mut work: Vec<(Entity, Vec<PropertyTrack>, f32)> = Vec::new();
    let mut events: Vec<(Entity, String)> = Vec::new();
    let mut verbose = false;
    {
        // Bevy 0.19: `SystemState::get_mut` is now fallible. These params always
        // validate, so `.unwrap()` is safe.
        let (time, mut dbg, clips, mut animators, players) = sys.get_mut(world).unwrap();
        let dt = time.delta_secs();
        let now = time.elapsed_secs();
        if now - dbg.last_log > 0.5 {
            verbose = true;
            dbg.last_log = now;
        }
        for (entity, animator, mut state) in animators.iter_mut() {
            if state.prop_clip_handles.is_empty() {
                continue;
            }
            if state.is_paused {
                if verbose {
                    info!("[prop-anim] {:?} skipped: paused", entity);
                }
                continue;
            }
            if state.prop_stopped {
                if verbose {
                    info!("[prop-anim] {:?} skipped: stopped", entity);
                }
                continue;
            }
            let clip_name = state
                .current_clip
                .clone()
                .or_else(|| animator.default_clip.clone())
                .or_else(|| animator.clips.first().map(|s| s.name.clone()));
            let Some(clip_name) = clip_name else {
                if verbose {
                    info!("[prop-anim] {:?} skipped: no clip selected/default", entity);
                }
                continue;
            };
            let Some(handle) = state.prop_clip_handles.get(&clip_name) else {
                if verbose {
                    info!("[prop-anim] {:?} skipped: no PropertyClip handle for '{}'", entity, clip_name);
                }
                continue;
            };
            let Some(clip) = clips.get(handle) else {
                if verbose {
                    info!("[prop-anim] {:?} skipped: PropertyClip '{}' not loaded yet", entity, clip_name);
                }
                continue;
            };
            if clip.tracks.is_empty() {
                if verbose {
                    info!("[prop-anim] {:?} clip '{}' has no property tracks", entity, clip_name);
                }
                continue;
            }

            let looping = animator.get_slot(&clip_name).map(|s| s.looping).unwrap_or(true);
            // Runtime speed override (set by Play/SetSpeed scripts), default 1.0.
            let speed = state.prop_speed;

            let step = crate::markers::clock_step(state.prop_time, dt * speed, clip.duration, looping, !state.prop_started);
            state.prop_started = true;

            // A clip the skeletal player is playing has its markers fired from
            // the player's own clock (`root_motion::post_animation`), which is
            // the one the pose follows; firing them here too would fire each
            // twice. This clock only speaks for clips nothing else plays.
            let on_player = state.initialized
                && state.player_entity.and_then(|p| players.get(p).ok()).is_some_and(|player| {
                    state.layout.base.get(&clip_name).is_some_and(|node| player.is_playing_animation(*node))
                });
            if !on_player {
                let times: Vec<f32> = clip.markers.iter().map(|m| m.time).collect();
                for i in crate::markers::markers_crossed(&times, clip.duration, &step) {
                    events.push((entity, clip.markers[i].name.clone()));
                }
            }

            let t = step.to;
            state.prop_time = t;
            work.push((entity, clip.tracks.clone(), t));
        }
    }

    // Deliver animation events to scripts (broadcast via the inbox).
    if !events.is_empty() {
        if let Some(mut inbox) = world.get_resource_mut::<renzora::ScriptAnimEventInbox>() {
            for (entity, name) in events {
                inbox.pending.push(renzora::AnimEvent {
                    name,
                    entity_bits: entity.to_bits(),
                });
            }
        }
    }

    if verbose && !work.is_empty() {
        info!("[prop-anim] runtime applying {} animator(s)", work.len());
    }
    for (entity, tracks, t) in work {
        apply_property_tracks(world, entity, &tracks, t, verbose);
    }
}
