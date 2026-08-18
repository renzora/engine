//! World-global wind.
//!
//! Two jobs:
//!
//! 1. **Evaluate.** Read `WorldEnvironment`'s authored wind section into the
//!    shared [`renzora::WindState`] resource every frame, including the gust
//!    envelope and the smoothed sea state. Grass, the cloud deck, the ocean and
//!    cloth all read that one resource — see [`renzora::wind`] for why they
//!    used to disagree.
//! 2. **Sway.** Give any mesh tagged [`WindSway`] a vertex-animated variant of
//!    its material, so trees, bushes and hand-modelled foliage move.
//!
//! # Materials are shared, not per-entity
//!
//! Swapping in a [`WindSwayMaterial`] per entity would give a 500-tree forest
//! 500 materials and 500 bind groups. Instead [`WindMaterialCache`] keys on the
//! source material plus the quantized per-mesh response, so a forest of
//! identical trees collapses back to one material — and the per-frame wind
//! update then touches a handful of assets rather than a thousand.

use bevy::pbr::MaterialPlugin;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use renzora::{WindSection, WindState, WindSway, WorldEnvironment};

pub mod material;
pub mod script_extension;

pub use material::{WindParams, WindSwayExt, WindSwayMaterial};

/// Time constant, in seconds, for the ocean's smoothed wind.
///
/// A real sea takes *hours* to build to a new wind, which is unusable in a game
/// — turn the dial and nothing happens for the rest of the session. 25 s keeps
/// the lag readable as inertia (the swell visibly arrives after the gust) while
/// still settling inside a play session. It also keeps the JONSWAP spectrum
/// rebake off the per-frame path, which is the practical constraint: that bake
/// is not a uniform write.
const SEA_STATE_TAU: f32 = 25.0;

/// The `StandardMaterial` an entity had before [`WindSway`] swapped it out, so
/// removing the component can put it back.
#[derive(Component, Clone, Debug)]
pub struct WindSwaySource(pub Handle<StandardMaterial>);

/// A [`WindSway`] inherited from an ancestor.
///
/// An imported model is not one entity with one mesh — it is a bare root
/// carrying `MeshInstanceData` with the real meshes spawned underneath it. That
/// makes the root the only place a `WindSway` can usefully live: the scene saver
/// deliberately drops GLTF descendants (they are regenerated from the model file
/// on load), so a sway tagged onto a child mesh is gone on the next load, while
/// the root has no material of its own to swap. Tagging the root and pushing the
/// settings down is what makes "add Wind Sway to my plant model" work *and*
/// persist.
///
/// Deliberately **not** `Reflect`, so it never reaches a scene file. It is
/// derived state — the authored `WindSway` on the ancestor is the truth, and a
/// saved copy on a child would go stale the moment the parent was edited.
#[derive(Component, Clone, Debug)]
pub struct InheritedWindSway(pub WindSway);

/// Dedupe key for [`WindMaterialCache`]. The per-mesh response values are
/// quantized to 1/64 before hashing — two trees authored at 1.0 and 1.0000001
/// response should share a material, and `f32` is not `Hash` anyway.
#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
struct WindMaterialKey {
    source: AssetId<StandardMaterial>,
    response: i32,
    flutter: i32,
    amplitude: i32,
    pivot_height: i32,
}

impl WindMaterialKey {
    fn new(source: AssetId<StandardMaterial>, sway: &WindSway) -> Self {
        let q = |v: f32| (v * 64.0).round() as i32;
        Self {
            source,
            response: q(sway.response),
            flutter: q(sway.flutter),
            amplitude: q(sway.amplitude),
            pivot_height: q(sway.pivot_height),
        }
    }
}

/// One `WindSwayMaterial` per distinct (source material, response) pair.
#[derive(Resource, Default)]
struct WindMaterialCache {
    by_key: HashMap<WindMaterialKey, Handle<WindSwayMaterial>>,
}

// ── Evaluate ────────────────────────────────────────────────────────────────

/// The gust envelope, 0..1 — the CPU mirror of `wind_gust` in
/// `wind_common.wgsl`, with the spatial term at zero (a CPU consumer is a force
/// on one body, not a field over a landscape). Keep the two in step or cloth
/// will gust out of phase with the grass beside it.
fn gust_envelope(frequency: f32, t: f32) -> f32 {
    let p = t * frequency * std::f32::consts::TAU;
    0.5 + 0.5 * (p.sin() * 0.6 + (p * 0.37 + 1.7).sin() * 0.4)
}

/// Read the authored section into [`WindState`].
///
/// The first entity carrying a `WorldEnvironment` wins. There is meant to be
/// exactly one — a second is a scene authoring mistake, and picking the first
/// deterministically beats blending two winds into something neither author
/// asked for.
fn evaluate_wind(
    mut state: ResMut<WindState>,
    sources: Query<&WorldEnvironment>,
    time: Res<Time>,
) {
    let default_section = WindSection::default();
    let section = sources
        .iter()
        .next()
        .map(|e| &e.wind)
        .unwrap_or(&default_section);

    if !section.enabled {
        // Zero rather than freeze: consumers read one number and never have to
        // re-check the flag, and a disabled wind that left the last speed in
        // place would keep the ocean churning.
        let previous_sea = state.sea_state_speed;
        *state = WindState::default();
        // The sea still has to *fall* to calm rather than snap, for the same
        // reason it rises slowly.
        state.sea_state_speed =
            previous_sea * (-time.delta_secs() / SEA_STATE_TAU).exp();
        return;
    }

    let rad = section.direction.to_radians();
    state.direction = Vec2::new(rad.cos(), rad.sin());
    state.speed = section.speed.max(0.0);
    state.gust_strength = section.gust_strength.clamp(0.0, 1.0);
    state.gust_frequency = section.gust_frequency.max(0.0);
    state.turbulence = section.turbulence.clamp(0.0, 1.0);
    state.gust = gust_envelope(state.gust_frequency, time.elapsed_secs());

    // Exponential smoothing, framerate-independent.
    let alpha = 1.0 - (-time.delta_secs() / SEA_STATE_TAU).exp();
    state.sea_state_speed += (state.speed - state.sea_state_speed) * alpha;
}

// ── Inheritance ─────────────────────────────────────────────────────────────

/// Push a [`WindSway`] down onto the meshes underneath it.
///
/// Two directions, because the two events that need it arrive from opposite
/// ends. Editing a `WindSway` is a top-down event: the root is known and its
/// descendants have to be refreshed. A GLTF finishing its load is a bottom-up
/// one: the new mesh appears frames after the root was tagged, long after any
/// top-down pass ran, so it has to look upward for itself.
///
/// A descendant carrying its own `WindSway` ends the inheritance: it keeps its
/// own settings and so does everything beneath it. That is what lets the
/// procedural tree give its trunk and its canopy different tunings while still
/// letting a whole imported model be tagged in one go.
fn propagate_wind_sway(
    mut commands: Commands,
    changed_roots: Query<(Entity, &WindSway), Changed<WindSway>>,
    new_meshes: Query<Entity, Added<MeshMaterial3d<StandardMaterial>>>,
    mut unsway: RemovedComponents<WindSway>,
    children: Query<&Children>,
    parents: Query<&ChildOf>,
    sways: Query<&WindSway>,
) {
    // Top-down: a root was added or edited.
    for (root, sway) in changed_roots.iter() {
        for descendant in descendants_without_own_sway(root, &children, &sways) {
            commands
                .entity(descendant)
                .insert(InheritedWindSway(sway.clone()));
        }
    }

    // Bottom-up: a mesh appeared (GLTF rehydration, a spawned child) and may
    // already sit under a tagged root.
    for mesh in new_meshes.iter() {
        if sways.get(mesh).is_ok() {
            continue; // authored directly on this mesh — nothing to inherit
        }
        let mut cursor = mesh;
        while let Ok(child_of) = parents.get(cursor) {
            let parent = child_of.parent();
            if let Ok(sway) = sways.get(parent) {
                commands.entity(mesh).insert(InheritedWindSway(sway.clone()));
                break;
            }
            cursor = parent;
        }
    }

    // A root lost its `WindSway`: take the inheritance off everything under it.
    // `children.get` failing means the entity was despawned rather than merely
    // untagged, and its descendants went with it.
    for root in unsway.read() {
        for descendant in descendants_without_own_sway(root, &children, &sways) {
            commands.entity(descendant).remove::<InheritedWindSway>();
        }
    }
}

/// Every descendant of `root`, stopping at any entity that carries its own
/// [`WindSway`] (that entity and its subtree are governed by it instead).
fn descendants_without_own_sway(
    root: Entity,
    children: &Query<&Children>,
    sways: &Query<&WindSway>,
) -> Vec<Entity> {
    let mut out = Vec::new();
    let mut stack: Vec<Entity> = children
        .get(root)
        .map(|c| c.iter().collect())
        .unwrap_or_default();
    while let Some(entity) = stack.pop() {
        if sways.get(entity).is_ok() {
            continue;
        }
        out.push(entity);
        if let Ok(kids) = children.get(entity) {
            stack.extend(kids.iter());
        }
    }
    out
}

// ── Sway material swap ──────────────────────────────────────────────────────

/// Give every newly-tagged entity a wind-animated copy of its material.
///
/// Settings come from the entity's own [`WindSway`] if it has one, otherwise
/// from an [`InheritedWindSway`] pushed down by [`propagate_wind_sway`] — so
/// this treats a hand-tagged mesh and a mesh under a tagged model root
/// identically.
///
/// Three ways in, and the third is the one that is easy to miss:
/// * `Changed<WindSway>` / `Changed<InheritedWindSway>` — a new tag, or an
///   edited `response` (which changes the cache key, so the entity should point
///   at a different material).
/// * `Changed<MeshMaterial3d<StandardMaterial>>` — the source material was
///   swapped out from under us, including by scene-load rehydration.
/// * `Without<MeshMaterial3d<WindSwayMaterial>>` — a tagged entity that has not
///   been converted yet. Without this term an entity whose source material was
///   still *loading* on the frame its `Changed` fired would be skipped once and
///   then never looked at again, and would silently never sway.
#[allow(clippy::type_complexity)]
fn apply_wind_sway(
    mut commands: Commands,
    mut cache: ResMut<WindMaterialCache>,
    mut wind_materials: ResMut<Assets<WindSwayMaterial>>,
    standard_materials: Res<Assets<StandardMaterial>>,
    wind: Option<Res<WindState>>,
    targets: Query<
        (
            Entity,
            Option<&WindSway>,
            Option<&InheritedWindSway>,
            Option<&MeshMaterial3d<StandardMaterial>>,
            Option<&WindSwaySource>,
        ),
        (
            // Cheap gate: only entities that carry a sway at all. Without it
            // the `Without<..>` term below would drag every static mesh in the
            // scene through this loop every frame.
            Or<(With<WindSway>, With<InheritedWindSway>)>,
            Or<(
                Changed<WindSway>,
                Changed<InheritedWindSway>,
                Changed<MeshMaterial3d<StandardMaterial>>,
                Without<MeshMaterial3d<WindSwayMaterial>>,
            )>,
        ),
    >,
) {
    let wind = wind.as_deref().copied().unwrap_or_default();
    for (entity, own, inherited, standard, existing_source) in targets.iter() {
        let Some(sway) = own.or(inherited.map(|i| &i.0)) else {
            continue;
        };

        // The source is whichever StandardMaterial is on the entity now, or the
        // one we stashed when we swapped it out on an earlier pass.
        let Some(source) = standard
            .map(|m| m.0.clone())
            .or_else(|| existing_source.map(|s| s.0.clone()))
        else {
            continue;
        };

        // A material that hasn't finished loading has nothing to clone yet.
        // Skipping is safe because the `Without<MeshMaterial3d<WindSwayMaterial>>`
        // term above keeps re-offering this entity every frame until the asset
        // lands.
        let Some(base) = standard_materials.get(&source) else {
            continue;
        };

        let key = WindMaterialKey::new(source.id(), sway);
        let handle = cache
            .by_key
            .entry(key)
            .or_insert_with(|| {
                wind_materials.add(WindSwayMaterial {
                    base: base.clone(),
                    extension: WindSwayExt {
                        wind: WindParams::build(&wind, sway),
                    },
                })
            })
            .clone();

        commands
            .entity(entity)
            .insert((MeshMaterial3d(handle), WindSwaySource(source)))
            .remove::<MeshMaterial3d<StandardMaterial>>();
    }
}

/// Put the original material back when an entity stops swaying — whether the
/// authored `WindSway` was removed from it directly, or the ancestor it was
/// inheriting from lost its own.
#[allow(clippy::type_complexity)]
fn restore_wind_sway(
    mut commands: Commands,
    mut removed_own: RemovedComponents<WindSway>,
    mut removed_inherited: RemovedComponents<InheritedWindSway>,
    still_swaying: Query<(), Or<(With<WindSway>, With<InheritedWindSway>)>>,
    sources: Query<&WindSwaySource>,
) {
    for entity in removed_own.read().chain(removed_inherited.read()) {
        // A tagged root also holds an `InheritedWindSway` on its children; when
        // both go in one frame the entity shows up twice, and an entity that
        // merely swapped one for the other must keep its material.
        if still_swaying.get(entity).is_ok() {
            continue;
        }
        let Ok(source) = sources.get(entity) else {
            continue;
        };
        commands
            .entity(entity)
            .insert(MeshMaterial3d(source.0.clone()))
            .remove::<(MeshMaterial3d<WindSwayMaterial>, WindSwaySource)>();
    }
}

/// Push the current wind into every live sway material.
///
/// Note what is deliberately NOT the filter here: `WindState.is_changed()`.
/// That resource is rewritten every frame (the gust envelope advances), so
/// change detection on it is always true and would buy nothing. The real filter
/// is the value comparison below — the shader re-derives the gust from
/// `globals.time`, so this uniform carries only the slowly-varying authored
/// wind and is already correct on almost every frame. A still scene therefore
/// costs one comparison per material and zero uploads.
fn sync_wind_materials(
    wind: Res<WindState>,
    mut wind_materials: ResMut<Assets<WindSwayMaterial>>,
    tagged: Query<(
        Option<&WindSway>,
        Option<&InheritedWindSway>,
        &MeshMaterial3d<WindSwayMaterial>,
    )>,
) {
    for (own, inherited, handle) in tagged.iter() {
        let Some(sway) = own.or(inherited.map(|i| &i.0)) else {
            continue;
        };
        let Some(material) = wind_materials.get(&handle.0) else {
            continue;
        };
        let next = WindParams::build(&wind, sway);
        if material.extension.wind == next {
            continue;
        }
        // Only now does `get_mut` fire, marking the asset modified and
        // rebuilding its bind group — on an actual wind change, not per frame.
        if let Some(material) = wind_materials.get_mut(&handle.0) {
            material.into_inner().extension.wind = next;
        }
    }
}

// ── Scripting ───────────────────────────────────────────────────────────────

fn handle_wind_script_actions(
    trigger: On<renzora::ScriptAction>,
    mut envs: Query<&mut WorldEnvironment>,
) {
    use renzora::ScriptActionValue;
    let action = trigger.event();
    let read = |k: &str| -> Option<f32> {
        match action.args.get(k) {
            Some(ScriptActionValue::Float(f)) => Some(*f),
            Some(ScriptActionValue::Int(i)) => Some(*i as f32),
            _ => None,
        }
    };
    let Some(mut env) = envs.iter_mut().next() else {
        return;
    };
    match action.name.as_str() {
        "set_wind" => {
            if let Some(v) = read("speed") {
                env.wind.speed = v.max(0.0);
            }
            if let Some(v) = read("direction") {
                env.wind.direction = v;
            }
            env.wind.enabled = true;
        }
        "set_wind_gusts" => {
            if let Some(v) = read("strength") {
                env.wind.gust_strength = v.clamp(0.0, 1.0);
            }
            if let Some(v) = read("frequency") {
                env.wind.gust_frequency = v.max(0.0);
            }
            if let Some(v) = read("turbulence") {
                env.wind.turbulence = v.clamp(0.0, 1.0);
            }
        }
        _ => {}
    }
}

// ── Plugin ──────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct WindPlugin;

impl Plugin for WindPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] WindPlugin");
        // `load_shader_library!`, not `embedded_asset!`, for the shared module:
        // the shader loader does not pull in a dependency that nothing holds a
        // handle to, so an `#import renzora_wind::common::…` would fail to
        // resolve at pipeline-build time with the file sitting right there.
        // This macro embeds it AND leaks a permanent handle so it stays loaded.
        // The two entry points are referenced by handle from the material, so
        // they need only be embedded.
        bevy::shader::load_shader_library!(app, "wind_common.wgsl");
        bevy::asset::embedded_asset!(app, "wind_sway.wgsl");
        bevy::asset::embedded_asset!(app, "wind_sway_prepass.wgsl");

        app.register_type::<WindSway>()
            .register_type::<WindState>()
            .init_resource::<WindState>()
            .init_resource::<WindMaterialCache>()
            .add_plugins(MaterialPlugin::<WindSwayMaterial>::default())
            .add_systems(
                Update,
                (
                    evaluate_wind,
                    propagate_wind_sway,
                    apply_wind_sway,
                    restore_wind_sway,
                    sync_wind_materials,
                )
                    .chain(),
            )
            .add_observer(handle_wind_script_actions);
    }

    fn finish(&self, app: &mut App) {
        let mut extensions = app.world_mut().get_resource_or_insert_with(
            renzora_scripting::extension::ScriptExtensions::default,
        );
        extensions.register(script_extension::WindScriptExtension);
    }
}

renzora::add!(WindPlugin);

#[cfg(test)]
mod tests {
    use super::*;

    /// The gust envelope has to stay inside 0..1 — it multiplies into a
    /// strength that a negative value would invert, flipping the whole
    /// landscape upwind for part of every cycle.
    #[test]
    fn gust_envelope_stays_in_unit_range() {
        for i in 0..2000 {
            let t = i as f32 * 0.05;
            let g = gust_envelope(0.15, t);
            assert!((0.0..=1.0).contains(&g), "gust {g} out of range at t={t}");
        }
    }

    /// `Time` without `TimePlugin`, advanced by hand — the plugin's first
    /// update reports a zero delta, which would make every rate-based
    /// assertion below trivially pass.
    fn app_with_wind(section: WindSection) -> App {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.init_resource::<WindState>();
        app.add_systems(Update, evaluate_wind);
        app.world_mut().spawn(WorldEnvironment {
            wind: section,
            ..default()
        });
        app
    }

    fn tick(app: &mut App, seconds: f32) {
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(std::time::Duration::from_secs_f32(seconds));
        app.update();
    }

    /// Disabling wind must not leave the ocean churning at the old sea state.
    #[test]
    fn disabled_wind_decays_the_sea_state() {
        let mut app = app_with_wind(WindSection {
            enabled: false,
            ..default()
        });
        app.world_mut().resource_mut::<WindState>().sea_state_speed = 10.0;
        tick(&mut app, 1.0);
        let state = *app.world().resource::<WindState>();
        assert_eq!(state.speed, 0.0);
        assert!(state.sea_state_speed < 10.0, "sea must fall towards calm");
        assert!(state.sea_state_speed > 0.0, "must decay, not snap to zero");
    }

    /// The sea lags the wind rather than tracking it — that lag is what keeps
    /// the JONSWAP spectrum rebake off the per-frame path.
    #[test]
    fn sea_state_lags_the_wind() {
        let mut app = app_with_wind(WindSection {
            speed: 12.0,
            ..default()
        });
        tick(&mut app, 1.0);
        let after_1s = app.world().resource::<WindState>().sea_state_speed;
        assert!(
            after_1s < 2.0,
            "one second in, the sea should barely have moved (got {after_1s})"
        );

        // Eight time constants later it has essentially arrived (exponential
        // smoothing approaches asymptotically — four constants still leaves
        // ~2% on the table, which is more than this tolerance allows).
        for _ in 0..40 {
            tick(&mut app, 5.0);
        }
        let settled = app.world().resource::<WindState>().sea_state_speed;
        assert!(
            (settled - 12.0).abs() < 0.1,
            "sea should reach the wind eventually (got {settled})"
        );
    }

    /// Direction is stored as a travel bearing and must survive the round trip
    /// through the unit vector — a sign error here points the whole world's
    /// foliage upwind.
    #[test]
    fn direction_round_trips_through_the_unit_vector() {
        for bearing in [0.0, 25.0, 90.0, 179.0, 270.0] {
            let mut app = app_with_wind(WindSection {
                direction: bearing,
                ..default()
            });
            tick(&mut app, 0.016);
            let got = app.world().resource::<WindState>().direction_degrees();
            let delta = (got - bearing).rem_euclid(360.0);
            let delta = delta.min(360.0 - delta);
            assert!(delta < 0.01, "{bearing}° came back as {got}°");
        }
    }

    /// Two plants with the same source material and the same response share one
    /// material asset; a different response gets its own.
    #[test]
    fn material_key_dedupes_on_response() {
        let source = AssetId::<StandardMaterial>::invalid();
        let a = WindSway::default();
        let b = WindSway {
            response: 1.0 + 1.0 / 512.0, // below the 1/64 quantum
            ..default()
        };
        let c = WindSway {
            response: 1.6,
            ..default()
        };
        assert_eq!(
            WindMaterialKey::new(source, &a),
            WindMaterialKey::new(source, &b)
        );
        assert_ne!(
            WindMaterialKey::new(source, &a),
            WindMaterialKey::new(source, &c)
        );
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    use renzora_bsn::DynamicSceneBuilder;

    /// `WindSway` must survive a scene save. The saver walks the type registry
    /// and serializes whatever reflects, so a type that is missing its
    /// registration — or its `Serialize` reflect impl — is dropped silently:
    /// no error, the component is just gone on the next load.
    #[test]
    fn wind_sway_reflects_into_a_saved_scene() {
        let mut app = App::new();
        app.register_type::<WindSway>();
        let entity = app
            .world_mut()
            .spawn((Name::new("plant"), WindSway::default()))
            .id();

        let scene = DynamicSceneBuilder::from_world(app.world())
            .deny_all_resources()
            .extract_entity(entity)
            .build();

        let registry = app.world().resource::<AppTypeRegistry>().read();
        let saved: Vec<String> = scene.entities[0]
            .components
            .iter()
            .map(|c| c.reflect_type_path().to_string())
            .collect();
        assert!(
            saved.iter().any(|p| p.ends_with("WindSway")),
            "WindSway missing from the saved entity; got {saved:?}"
        );

        // The saver drops any component that fails to serialize to RON, so a
        // reflectable-but-unserializable type would still vanish.
        for component in scene.entities[0].components.iter() {
            if component.reflect_type_path().ends_with("WindSway") {
                let ser = bevy::reflect::serde::TypedReflectSerializer::new(
                    component.as_partial_reflect(),
                    &registry,
                );
                assert!(
                    ron::ser::to_string(&ser).is_ok(),
                    "WindSway does not serialize to RON"
                );
            }
        }
    }
}

#[cfg(test)]
mod inheritance_tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.add_systems(Update, propagate_wind_sway);
        app
    }

    /// The reported bug. An imported model is a bare root carrying
    /// `MeshInstanceData` with its meshes spawned underneath; the scene saver
    /// drops those descendants because they are regenerated from the model file
    /// on load. So a `WindSway` tagged onto a child mesh vanished on reload,
    /// and one tagged onto the root — the only entity that *does* persist — had
    /// no material to act on and did nothing. Tagging the root has to reach the
    /// meshes.
    #[test]
    fn tagging_a_model_root_reaches_its_meshes() {
        let mut app = app();
        let root = app.world_mut().spawn(WindSway::default()).id();
        let mesh = app.world_mut().spawn(ChildOf(root)).id();
        let deep = app.world_mut().spawn(ChildOf(mesh)).id();
        app.update();

        assert!(app.world().get::<InheritedWindSway>(mesh).is_some());
        assert!(
            app.world().get::<InheritedWindSway>(deep).is_some(),
            "inheritance must reach the whole subtree, not just direct children"
        );
    }

    /// GLTF meshes are spawned frames after the root is tagged, so a purely
    /// top-down pass misses them entirely — they have to find the root
    /// themselves once they appear.
    #[test]
    fn meshes_that_load_later_still_pick_up_the_sway() {
        let mut app = app();
        let root = app.world_mut().spawn(WindSway::default()).id();
        app.update();

        // The model finishes loading well after the tag was applied.
        let late = app
            .world_mut()
            .spawn((
                ChildOf(root),
                MeshMaterial3d(Handle::<StandardMaterial>::default()),
            ))
            .id();
        app.update();

        assert!(
            app.world().get::<InheritedWindSway>(late).is_some(),
            "a mesh that appears after the root was tagged must still inherit"
        );
    }

    /// The procedural tree gives its trunk and its canopy different tunings, so
    /// an authored `WindSway` on a child has to win — and shield its own
    /// subtree from the ancestor's settings.
    #[test]
    fn an_authored_sway_on_a_child_wins() {
        let mut app = app();
        let root = app
            .world_mut()
            .spawn(WindSway {
                response: 0.5,
                ..default()
            })
            .id();
        let own = app
            .world_mut()
            .spawn((
                ChildOf(root),
                WindSway {
                    response: 1.6,
                    ..default()
                },
            ))
            .id();
        let under_own = app.world_mut().spawn(ChildOf(own)).id();
        app.update();

        assert!(
            app.world().get::<InheritedWindSway>(own).is_none(),
            "an entity with its own WindSway must not be overwritten"
        );
        // The subtree below `own` follows `own`, not the grandparent: the
        // nearest tagged ancestor wins, so a canopy under a re-tagged branch
        // does not silently revert to the trunk's stiffness.
        let deep = app
            .world()
            .get::<InheritedWindSway>(under_own)
            .expect("the subtree below a tagged child still inherits — from it");
        assert_eq!(
            deep.0.response, 1.6,
            "must inherit from the nearest tagged ancestor, not the outermost"
        );
    }

    /// Removing the tag from the root has to release the whole subtree,
    /// otherwise the meshes keep swaying with no component explaining why.
    #[test]
    fn untagging_the_root_releases_the_subtree() {
        let mut app = app();
        let root = app.world_mut().spawn(WindSway::default()).id();
        let mesh = app.world_mut().spawn(ChildOf(root)).id();
        app.update();
        assert!(app.world().get::<InheritedWindSway>(mesh).is_some());

        app.world_mut().entity_mut(root).remove::<WindSway>();
        app.update();
        assert!(
            app.world().get::<InheritedWindSway>(mesh).is_none(),
            "removing the root tag must clear the inheritance"
        );
    }
}
