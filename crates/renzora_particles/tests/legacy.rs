//! Old Hanabi effects in a real app: a scene line naming the retired component
//! loads as a Particle System, and a `.particle` file loads as an effect.
//!
//! Through the real scene parser and the real asset server on purpose. The
//! migration depends on two things neither the converter nor the component
//! can check alone: that the scene loader's short-name fallback lands the old
//! type path on the migration component, and that the asset server routes the
//! `.particle` extension to the converting loader.

use std::path::{Path, PathBuf};

use bevy::asset::io::embedded::EmbeddedAssetRegistry;
use bevy::ecs::entity::EntityHashMap;
use bevy::prelude::*;
use renzora_bsn::bsn::{BsnSerializer, SceneSerializer};
use renzora_particles::convert::format::HanabiEffectDefinition;
use renzora_particles::legacy::{HanabiEffect, InlineLegacyEffect};
use renzora_particles::{ParticlePlayback, ParticleRuntime, ParticleSystem, ParticlesPlugin, PlayState, VfxAsset};

fn app() -> App {
    renzora_test_harness::headless_app_with(|app| {
        app.add_plugins(ParticlesPlugin);
    })
}

/// Parse `text` strictly (an unresolved or undeserializable component is an
/// error, not a silently missing one) and spawn it.
fn load_scene(app: &mut App, text: &str) -> Vec<Entity> {
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    let scene = {
        let reg = registry.read();
        BsnSerializer.deserialize(text, &reg).expect("the old scene parses strictly")
    };
    let mut map = EntityHashMap::default();
    scene.write_to_world(app.world_mut(), &mut map).expect("the scene spawns");
    app.world_mut().flush();
    let mut entities: Vec<Entity> = map.values().copied().collect();
    entities.sort();
    entities
}

/// Exactly what the old editor wrote for an effect backed by a file, with the
/// type path of the crate that no longer exists.
const ASSET_LINE: &str = r#"renzora_hanabi::data::HanabiEffect: (source:Asset(path:"particles/fountain.particle"),playing:false,rate_multiplier:1.0,scale_multiplier:1.0,color_tint:(1.0,1.0,1.0,1.0),time_scale:1.0,variable_overrides:{}),"#;

#[test]
fn an_old_scene_component_becomes_a_particle_system_pointing_at_its_file() {
    let mut app = app();
    let text = format!(
        "// renzora interim bsn v1\nentity 4294967295 {{\n    bevy_ecs::name::Name: \"fountain\",\n    {ASSET_LINE}\n}}\n"
    );
    let entities = load_scene(&mut app, &text);
    let e = entities[0];
    let world = app.world();
    assert!(world.get::<HanabiEffect>(e).is_none(), "the old component is gone once migrated");
    let system = world.get::<ParticleSystem>(e).expect("a Particle System took its place");
    assert_eq!(system.path, "particles/fountain.particle");
    assert_eq!(world.get::<ParticlePlayback>(e).map(|p| p.state), Some(PlayState::Stopped), "playing: false carries over");
    assert_eq!(world.get::<Name>(e).map(|n| n.as_str()), Some("fountain"), "the rest of the entity is untouched");
}

#[test]
fn an_inline_old_effect_plays_from_memory_and_is_marked_for_the_editor() {
    let mut app = app();
    let def = HanabiEffectDefinition { name: "Inline Sparks".into(), ..Default::default() };
    let body = format!(
        "(source:Inline(definition:{}),playing:true,rate_multiplier:1.0,scale_multiplier:1.0,color_tint:(1.0,1.0,1.0,1.0),time_scale:1.0,variable_overrides:{{}})",
        ron::to_string(&def).unwrap()
    );
    let text = format!("// renzora interim bsn v1\nentity 4294967295 {{\n    renzora_hanabi::data::HanabiEffect: {body},\n}}\n");
    let e = load_scene(&mut app, &text)[0];
    let world = app.world();
    let system = world.get::<ParticleSystem>(e).expect("migrated");
    assert!(system.path.is_empty(), "an inline effect has no file yet");
    let asset = world.resource::<Assets<VfxAsset>>().get(&system.asset).expect("the conversion is loaded");
    assert_eq!(asset.name, "Inline Sparks");
    assert!(world.get::<InlineLegacyEffect>(e).is_some(), "the editor is told to write it out");
    assert_eq!(world.get::<ParticlePlayback>(e).map(|p| p.state), Some(PlayState::Playing));
}

/// What the MCP plugin's `spawn_particles` does: build the old component as a
/// dynamic value and insert it by its short name through reflection. It must
/// migrate like a scene line does, not panic in `FromReflect`.
#[test]
fn a_dynamic_insert_by_name_migrates_too() {
    use bevy::reflect::enums::{DynamicEnum, DynamicVariant};
    use bevy::reflect::structs::DynamicStruct;
    let mut app = app();
    let mut source = DynamicStruct::default();
    source.insert("path", "particles/torch.particle".to_string());
    let mut effect = DynamicStruct::default();
    effect.insert("source", DynamicEnum::new("Asset", DynamicVariant::Struct(source)));
    effect.insert("playing", true);
    effect.insert("rate_multiplier", 1.0f32);
    effect.insert("scale_multiplier", 1.0f32);
    effect.insert("color_tint", [1.0f32, 1.0, 1.0, 1.0]);
    effect.insert("time_scale", 1.0f32);

    let e = app.world_mut().spawn_empty().id();
    let registry = app.world().resource::<AppTypeRegistry>().clone();
    {
        let registry = registry.read();
        let reflect_component = registry
            .get_with_short_type_path("HanabiEffect")
            .and_then(|r| r.data::<ReflectComponent>())
            .expect("the old component is registered under its short name")
            .clone();
        let mut entity = app.world_mut().entity_mut(e);
        reflect_component.insert(&mut entity, &effect, &registry);
    }
    app.world_mut().flush();
    assert!(app.world().get::<HanabiEffect>(e).is_none());
    assert_eq!(app.world().get::<ParticleSystem>(e).map(|s| s.path.as_str()), Some("particles/torch.particle"));
}

/// A scene saved while the old runtime played a lit effect holds that effect's
/// `PointLight` on the same entity. The converted effect brings its own light,
/// so the saved one goes; an unlit effect leaves a light there alone.
#[test]
fn the_old_effects_own_light_is_replaced_not_doubled() {
    use renzora_particles::convert::format::{EffectSource, ParticleLightSettings};
    let mut app = app();
    let spawn = |app: &mut App, light: Option<ParticleLightSettings>| {
        let definition = HanabiEffectDefinition { light, ..Default::default() };
        let e = app.world_mut().spawn(bevy::light::PointLight::default()).id();
        app.world_mut()
            .entity_mut(e)
            .insert(HanabiEffect { source: EffectSource::Inline { definition }, ..Default::default() });
        e
    };
    let lit = spawn(&mut app, Some(ParticleLightSettings::default()));
    let unlit = spawn(&mut app, None);
    for _ in 0..5 {
        app.update();
    }
    let world = app.world();
    assert!(world.get::<bevy::light::PointLight>(lit).is_none(), "the converted effect lights the scene itself");
    assert!(world.get::<bevy::light::PointLight>(unlit).is_some(), "a light the effect does not replace stays");
    assert!(world.get::<renzora_particles::legacy::PendingLegacyLight>(lit).is_none());
    assert!(world.get::<renzora_particles::legacy::PendingLegacyLight>(unlit).is_none());
}

/// Register `text` as an embedded file, so the asset server reads it like any
/// project file without the test writing to disk.
fn embed(app: &mut App, path: &str, text: String) {
    app.world_mut().resource_mut::<EmbeddedAssetRegistry>().insert_asset(
        PathBuf::from("/legacy_test").join(path),
        Path::new(path),
        text.into_bytes(),
    );
}

#[test]
fn a_particle_file_loads_and_plays_as_an_effect() {
    let mut app = app();
    let def = HanabiEffectDefinition { name: "Old Fire".into(), ..Default::default() };
    embed(&mut app, "legacy_test/fire.particle", ron::to_string(&def).unwrap());
    let e = app.world_mut().spawn(ParticleSystem::from_path("embedded://legacy_test/fire.particle")).id();
    for _ in 0..400 {
        app.update();
        if app.world().get::<ParticleRuntime>(e).is_some() {
            break;
        }
    }
    let world = app.world();
    assert!(world.get::<ParticleRuntime>(e).is_some(), "the .particle never loaded as an effect");
    let handle = &world.get::<ParticleSystem>(e).unwrap().asset;
    let asset = world.resource::<Assets<VfxAsset>>().get(handle).expect("loaded");
    assert_eq!(asset.name, "Old Fire");
    assert_eq!(asset.emitters.len(), 1);
}

#[test]
fn an_unreadable_particle_file_fails_to_load_without_a_default_effect() {
    let mut app = app();
    embed(&mut app, "legacy_test/broken.particle", "(this is not an effect".to_string());
    let e = app.world_mut().spawn(ParticleSystem::from_path("embedded://legacy_test/broken.particle")).id();
    let server = app.world().resource::<AssetServer>().clone();
    for _ in 0..400 {
        app.update();
        let handle = app.world().get::<ParticleSystem>(e).unwrap().asset.clone();
        if server.load_state(&handle).is_failed() {
            break;
        }
    }
    let handle = app.world().get::<ParticleSystem>(e).unwrap().asset.clone();
    assert!(server.load_state(&handle).is_failed(), "a broken file is a load error");
    assert!(app.world().get::<ParticleRuntime>(e).is_none());
}
